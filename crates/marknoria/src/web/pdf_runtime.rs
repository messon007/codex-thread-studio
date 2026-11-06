use super::*;
use tokio::io::{AsyncReadExt, AsyncSeekExt};

include!(concat!(env!("CARGO_MANIFEST_DIR"), "/web/vendor/pdf-assets.rs"));

pub(super) async fn resource(uri: axum::http::Uri) -> Response {
    // Comparison adapters rewrite the URI but retain their outer path params.
    // Match only this router's fixed resource prefix, independent of those params.
    match uri.path().strip_prefix("/vendor/pdf/").and_then(pdf_asset) {
        Some((mime, body)) => ([(header::CONTENT_TYPE, mime)], body).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct OriginalQuery { revision: String }

// PDF.js requests several disjoint byte ranges in parallel. Rejecting ordinary
// bursts immediately can leave its loading task waiting for a failed range.
// Keep the existing two active transfers, with a bounded, cancelable queue.
async fn stream_permit(
    work: &Arc<Semaphore>, waiters: &Arc<Semaphore>, cancelled: impl Fn() -> bool,
) -> Result<tokio::sync::OwnedSemaphorePermit, ApiError> {
    let gone = || (StatusCode::GONE, "Original document was closed".into());
    if cancelled() { return Err(gone()); }
    if let Ok(permit) = work.clone().try_acquire_owned() { return Ok(permit); }
    let _waiting = waiters.clone().try_acquire_owned()
        .map_err(|_| (StatusCode::TOO_MANY_REQUESTS, "Original transfer queue is full".into()))?;
    let acquire = work.clone().acquire_owned();
    tokio::pin!(acquire);
    let deadline = tokio::time::sleep(Duration::from_secs(5));
    tokio::pin!(deadline);
    let mut interval = tokio::time::interval(Duration::from_millis(20));
    loop {
        tokio::select! {
            result = &mut acquire => {
                let permit = result.map_err(|_| gone())?;
                if cancelled() { return Err(gone()); }
                return Ok(permit);
            },
            _ = &mut deadline => return Err((StatusCode::TOO_MANY_REQUESTS, "Original transfer waited more than 5 seconds".into())),
            _ = interval.tick() => if cancelled() { return Err(gone()); },
        }
    }
}

/// Immutable granted container only. Stream in 64 KiB chunks; no cache path,
/// sibling-resource or arbitrary filesystem parameter is accepted.
pub(super) async fn original(State(app): State<App>, headers: HeaderMap, Query(query): Query<OriginalQuery>) -> Result<Response, ApiError> {
    let document = app.documents.select(&headers)?;
    let index = index(&document)?;
    if index.revision != query.revision { return Err((StatusCode::CONFLICT, "Original document revision changed".into())); }
    if !matches!(index.format, crate::core::document_format::DocumentFormat::Pdf | crate::core::document_format::DocumentFormat::Epub) {
        return Err((StatusCode::BAD_REQUEST, "Original container streaming requires PDF or EPUB".into()));
    }
    let total = index.original_bytes.ok_or((StatusCode::BAD_REQUEST, "Original container unavailable".into()))? as u64;
    let (start, end, partial) = if let Some(range) = headers.get(header::RANGE) {
        let range = range.to_str().ok().and_then(|value| value.strip_prefix("bytes=")).and_then(|value| value.split_once('-'))
            .and_then(|(start, end)| Some((start.parse::<u64>().ok()?, end.parse::<u64>().ok()?)));
        let Some((start, end)) = range else { return Err((StatusCode::RANGE_NOT_SATISFIABLE, "Expected one explicit byte range".into())); };
        if start > end || end >= total || end - start >= 8 * 1024 * 1024 { return Err((StatusCode::RANGE_NOT_SATISFIABLE, "Original range is out of bounds or exceeds 8 MiB".into())); }
        (start, end, true)
    } else { (0, total.saturating_sub(1), false) };
    let permit = stream_permit(&app.work, &app.original_waiters, ||
        app.cancelled.load(Ordering::Relaxed) || document.is_cancelled()).await?;
    let mut file = tokio::fs::File::open(document.directory.join("original.bin")).await.map_err(failure)?;
    if file.metadata().await.map_err(failure)?.len() != total { return Err((StatusCode::CONFLICT, "Original snapshot length changed".into())); }
    file.seek(std::io::SeekFrom::Start(start)).await.map_err(failure)?;
    let length = if total == 0 { 0 } else { end - start + 1 };
    let (sender, receiver) = tokio::sync::mpsc::channel::<Result<Vec<u8>, std::io::Error>>(1);
    tokio::spawn(async move {
        let mut remaining = length;
        let mut interval = tokio::time::interval(Duration::from_millis(20));
        while remaining > 0 && !app.cancelled.load(Ordering::Relaxed) && !document.is_cancelled() && !sender.is_closed() {
            let mut bytes = vec![0; remaining.min(64 * 1024) as usize];
            if let Err(error) = file.read_exact(&mut bytes).await { let _ = sender.try_send(Err(error)); break; }
            remaining -= bytes.len() as u64;
            let slot = loop {
                tokio::select! {
                    slot = sender.reserve() => break slot.ok(),
                    _ = interval.tick() => if app.cancelled.load(Ordering::Relaxed) || document.is_cancelled() { break None; },
                }
            };
            let Some(slot) = slot else { break; }; slot.send(Ok(bytes));
        }
        // Release the file before the work permit: Windows cache removal must
        // not race a slow client retaining an open original-container handle.
        drop(file); drop(permit);
    });
    let stream = futures_util::stream::unfold(receiver, |mut receiver| async move { receiver.recv().await.map(|item| (item, receiver)) });
    let mut response = axum::http::Response::builder()
        .status(if partial { StatusCode::PARTIAL_CONTENT } else { StatusCode::OK })
        .header(header::CONTENT_TYPE, if index.format == crate::core::document_format::DocumentFormat::Pdf { "application/pdf" } else { "application/epub+zip" })
        .header(header::ACCEPT_RANGES, "bytes").header(header::CONTENT_LENGTH, length)
        .header("x-reader-revision", &index.revision);
    if partial { response = response.header(header::CONTENT_RANGE, format!("bytes {start}-{end}/{total}")); }
    response.body(axum::body::Body::from_stream(stream)).map_err(failure)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn original_transfer_waits_without_increasing_active_work() {
        let work = Arc::new(Semaphore::new(2));
        let waiters = Arc::new(Semaphore::new(16));
        let first = work.clone().acquire_owned().await.unwrap();
        let second = work.clone().acquire_owned().await.unwrap();
        let mut waiting = Box::pin(stream_permit(&work, &waiters, || false));
        tokio::select! {
            result = &mut waiting => panic!("Transfer did not queue: {result:?}"),
            _ = tokio::time::sleep(Duration::from_millis(20)) => {},
        }
        assert_eq!(waiters.available_permits(), 15);
        assert_eq!(work.available_permits(), 0);
        drop(first);
        let transfer = waiting.await.unwrap();
        assert_eq!(work.available_permits(), 0);
        assert_eq!(waiters.available_permits(), 16);
        drop(transfer);
        drop(second);
        assert_eq!(work.available_permits(), 2);
    }

    #[tokio::test]
    async fn original_transfer_queue_has_a_cap_deadline_and_cancellation() {
        let work = Arc::new(Semaphore::new(2));
        let waiters = Arc::new(Semaphore::new(16));
        let _active = work.clone().acquire_many_owned(2).await.unwrap();
        let full = waiters.clone().acquire_many_owned(16).await.unwrap();
        assert_eq!(stream_permit(&work, &waiters, || false).await.unwrap_err().0, StatusCode::TOO_MANY_REQUESTS);
        drop(full);
        let error = stream_permit(&work, &waiters, || false).await.unwrap_err();
        assert_eq!(error.1, "Original transfer waited more than 5 seconds");
        assert_eq!(waiters.available_permits(), 16);
        let cancelled = AtomicBool::new(false);
        let mut waiting = Box::pin(stream_permit(&work, &waiters, || cancelled.load(Ordering::Relaxed)));
        tokio::select! {
            result = &mut waiting => panic!("Transfer did not queue: {result:?}"),
            _ = tokio::time::sleep(Duration::from_millis(20)) => {},
        }
        cancelled.store(true, Ordering::Relaxed);
        assert_eq!(tokio::time::timeout(Duration::from_secs(1), waiting).await.unwrap().unwrap_err().0, StatusCode::GONE);
        assert_eq!(waiters.available_permits(), 16);
        // Dropping a request future also releases its queue slot immediately.
        let mut dropped = Box::pin(stream_permit(&work, &waiters, || false));
        tokio::select! {
            result = &mut dropped => panic!("Transfer did not queue: {result:?}"),
            _ = tokio::time::sleep(Duration::from_millis(20)) => {},
        }
        assert_eq!(waiters.available_permits(), 15);
        drop(dropped);
        assert_eq!(waiters.available_permits(), 16);
    }
}
