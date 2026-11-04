//! Native OpenCode SSE boundary. The legacy proxy stays byte-transparent;
//! owned utility events are removed here before native presentation/projection.
use crate::{native_utility_sessions as utility, GatewayState};
use axum::{
    body::Body,
    extract::State,
    http::{header, HeaderMap, Method, Response, StatusCode},
    response::{sse::Event, IntoResponse, Sse},
};
use futures_util::{stream, StreamExt};
use sse_stream::SseByteStream;
use std::{
    io,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};

const EVENT_BYTES: usize = 16 * 1024 * 1024;
fn error(message: impl Into<String>) -> io::Error {
    io::Error::other(message.into())
}

pub async fn get(State(state): State<GatewayState>) -> Response<Body> {
    if let Err((status, message)) = utility::configured(&state, "opencode") {
        return crate::json_error(status, &message);
    }
    let permit = match state
        .utility_sessions
        .event_streams
        .clone()
        .try_acquire_owned()
    {
        Ok(permit) => permit,
        Err(_) => {
            return crate::json_error(
                StatusCode::TOO_MANY_REQUESTS,
                "Native OpenCode event streams are busy",
            )
        }
    };
    // Fixed SDK route and method; never forward a caller-selected URL/credential.
    let mut headers = HeaderMap::new();
    headers.insert(header::ACCEPT, "text/event-stream".parse().unwrap());
    let response = match tokio::time::timeout(
        Duration::from_secs(35),
        state.opencode.proxy(
            Method::GET,
            headers,
            "/opencode/global/event".parse().expect("fixed event route"),
            Body::empty(),
        ),
    )
    .await
    {
        Ok(response) => response,
        Err(_) => {
            return crate::json_error(StatusCode::BAD_GATEWAY, "OpenCode event startup timed out")
        }
    };
    if !response.status().is_success() {
        return response;
    }
    if !response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.starts_with("text/event-stream"))
    {
        return crate::json_error(
            StatusCode::BAD_GATEWAY,
            "OpenCode did not return an event stream",
        );
    }
    let budget = Arc::new(AtomicUsize::new(0));
    let counted = budget.clone();
    let chunks = response.into_body().into_data_stream().map(move |chunk| {
        let chunk = chunk.map_err(|_| error("OpenCode event transport was interrupted"))?;
        if counted
            .fetch_add(chunk.len(), Ordering::Relaxed)
            .saturating_add(chunk.len())
            > EVENT_BYTES
        {
            return Err(error("OpenCode event exceeds the native stream limit"));
        }
        Ok(chunk)
    });
    let events = Box::pin(SseByteStream::new(chunks));
    let stream = stream::try_unfold(
        (events, state.utility_sessions, permit, budget),
        |(mut events, ownership, permit, budget)| async move {
            while let Some(event) = events.next().await {
                let event = event.map_err(|_| error("OpenCode sent an invalid event frame"))?;
                budget.store(0, Ordering::Relaxed);
                let filter = ownership.clone();
                let event = tokio::task::spawn_blocking(move || {
                    if let Some(data) = &event.data {
                        let value: serde_json::Value = serde_json::from_str(data)
                            .map_err(|_| error("OpenCode sent invalid event JSON"))?;
                        if filter.suppress_event(&value).map_err(error)? {
                            return Ok(None);
                        }
                    }
                    Ok::<_, io::Error>(Some(event))
                })
                .await
                .map_err(|_| error("OpenCode event worker stopped"))??;
                let Some(event) = event else {
                    continue;
                };
                // Mature parsers/encoders own UTF-8, CRLF, multiline data and
                // event/id/retry framing. The payload itself is not rewritten.
                let mut frame = Event::default();
                if let Some(data) = event.data {
                    frame = frame.data(data);
                }
                if let Some(kind) = event.event {
                    frame = frame.event(kind);
                }
                if let Some(id) = event.id {
                    frame = frame.id(id);
                }
                if let Some(retry) = event.retry {
                    frame = frame.retry(Duration::from_millis(retry));
                }
                return Ok(Some((frame, (events, ownership, permit, budget))));
            }
            Ok::<_, io::Error>(None)
        },
    );
    let mut response = Sse::new(stream)
        .keep_alive(axum::response::sse::KeepAlive::default())
        .into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, "no-store".parse().unwrap());
    response
        .headers_mut()
        .insert("x-accel-buffering", "no".parse().unwrap());
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn fragmented_bom_and_unicode_data_preserve_the_original_frame_without_panicking() {
        for prefix in ["", "\u{feff}"] {
            for newline in ["\n", "\r", "\r\n"] {
                let input = format!("{prefix}id: metadata-only{newline}retry: 22{newline}{newline}data: {{\"text\":\"中文😀\"}}{newline}{newline}");
                for width in [1, 2, 3, 7, 1024] {
                    let chunks = input
                        .as_bytes()
                        .chunks(width)
                        .map(|chunk| Ok::<_, io::Error>(axum::body::Bytes::copy_from_slice(chunk)));
                    let mut stream = SseByteStream::new(stream::iter(chunks));
                    let metadata = stream.next().await.unwrap().unwrap();
                    assert!(metadata.data.is_none());
                    assert_eq!(metadata.id.as_deref(), Some("metadata-only"));
                    assert_eq!(metadata.retry, Some(22));
                    let event = stream.next().await.unwrap().unwrap();
                    assert_eq!(event.data.as_deref(), Some("{\"text\":\"中文😀\"}"));
                    assert!(stream.next().await.is_none());
                }
            }
        }
    }
}
