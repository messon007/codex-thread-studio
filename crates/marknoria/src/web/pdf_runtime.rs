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
    let permit = app.work.clone().try_acquire_owned().map_err(|_| (StatusCode::TOO_MANY_REQUESTS, "Reader is busy".into()))?;
    let (start, end, partial) = if let Some(range) = headers.get(header::RANGE) {
        let range = range.to_str().ok().and_then(|value| value.strip_prefix("bytes=")).and_then(|value| value.split_once('-'))
            .and_then(|(start, end)| Some((start.parse::<u64>().ok()?, end.parse::<u64>().ok()?)));
        let Some((start, end)) = range else { return Err((StatusCode::RANGE_NOT_SATISFIABLE, "Expected one explicit byte range".into())); };
        if start > end || end >= total || end - start >= 8 * 1024 * 1024 { return Err((StatusCode::RANGE_NOT_SATISFIABLE, "Original range is out of bounds or exceeds 8 MiB".into())); }
        (start, end, true)
    } else { (0, total.saturating_sub(1), false) };
    let mut file = tokio::fs::File::open(document.directory.join("original.bin")).await.map_err(failure)?;
    if file.metadata().await.map_err(failure)?.len() != total { return Err((StatusCode::CONFLICT, "Original snapshot length changed".into())); }
    file.seek(std::io::SeekFrom::Start(start)).await.map_err(failure)?;
    let length = if total == 0 { 0 } else { end - start + 1 };
    let (sender, receiver) = tokio::sync::mpsc::channel::<Result<Vec<u8>, std::io::Error>>(1);
    tokio::spawn(async move {
        let mut remaining = length;
        let mut interval = tokio::time::interval(Duration::from_millis(20));
        while remaining > 0 && !document.cancelled.load(Ordering::Relaxed) && !sender.is_closed() {
            let mut bytes = vec![0; remaining.min(64 * 1024) as usize];
            if let Err(error) = file.read_exact(&mut bytes).await { let _ = sender.try_send(Err(error)); break; }
            remaining -= bytes.len() as u64;
            let slot = loop {
                tokio::select! {
                    slot = sender.reserve() => break slot.ok(),
                    _ = interval.tick() => if document.cancelled.load(Ordering::Relaxed) { break None; },
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
