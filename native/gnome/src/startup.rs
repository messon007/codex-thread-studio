//! Read-only startup discovery. Failure never retires the command/document lanes.
use crate::{gateway::Api, model::Backend, protocol::Event};
use reqwest::Method;
use std::time::Duration;

#[derive(Debug)]
pub enum Update {
    Launching,
    LaunchFailed(String),
    Loaded(Box<Event>),
    Retry { attempt: u8, delay: Duration },
    Failed(String),
}
const RETRIES: u8 = 8;
fn retry_delay(failed_attempt: u8, retryable: bool) -> Option<Duration> {
    (retryable && failed_attempt < RETRIES)
        .then(|| Duration::from_millis((500u64 << failed_attempt).min(30_000)))
}
async fn publish(events: &async_channel::Sender<Event>, generation: u64, update: Update) -> bool {
    events
        .send(Event::Startup { generation, update })
        .await
        .is_ok()
}
async fn loaded(events: &async_channel::Sender<Event>, generation: u64, event: Event) {
    publish(events, generation, Update::Loaded(Box::new(event))).await;
}
async fn registry(api: &Api) -> Result<Vec<Backend>, (String, bool)> {
    let (status, bytes, _) = api
        .response_bytes(Method::GET, "/studio/backends", None, None)
        .await
        .map_err(|error| (error, true))?;
    crate::worker::compute(move || {
        if !status.is_success() {
            let value = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
            return Ok(Err((
                crate::gateway::response_error(status, &value),
                status.is_server_error() || status.as_u16() == 408 || status.as_u16() == 429,
            )));
        }
        let value: serde_json::Value = serde_json::from_slice(&bytes)
            .map_err(|_| "Gateway returned invalid backend registry JSON".to_owned())?;
        let backends = serde_json::from_value(value["backends"].clone())
            .map_err(|_| "Gateway returned an invalid backend registry".to_owned())?;
        Ok(Ok(backends))
    })
    .await
    .map_err(|error| (error, false))?
}
pub async fn run(api: Api, events: async_channel::Sender<Event>, generation: u64) {
    let result = api
        .json(Method::GET, "/studio/native/preferences", None)
        .await;
    loaded(
        &events,
        generation,
        Event::Http {
            tag: "native-preferences-load".into(),
            result,
        },
    )
    .await;
    let result = crate::appearance::execute(&api, crate::appearance::Request::Read).await;
    loaded(
        &events,
        generation,
        Event::Appearance {
            dialog: 0,
            serial: 0,
            result,
        },
    )
    .await;
    let result = crate::language::execute(&api, crate::language::Request::Read).await;
    loaded(&events, generation, Event::Language { serial: 0, result }).await;
    let result = crate::message_flow::execute(&api, crate::message_flow::Request::Read).await;
    loaded(
        &events,
        generation,
        Event::MessageFlow { serial: 0, result },
    )
    .await;
    let result =
        crate::session_visibility::execute(&api, crate::session_visibility::Request::Read).await;
    loaded(
        &events,
        generation,
        Event::SessionVisibility {
            dialog: 0,
            serial: 0,
            result,
        },
    )
    .await;
    let result = crate::notifications::execute(&api, crate::notifications::Request::Read).await;
    loaded(
        &events,
        generation,
        Event::NotificationSettings { serial: 0, result },
    )
    .await;
    discover(&api, &events, generation).await;
}
async fn discover(api: &Api, events: &async_channel::Sender<Event>, generation: u64) {
    for attempt in 0..=RETRIES {
        match registry(api).await {
            Ok(backends) => {
                loaded(events, generation, Event::Backends(backends)).await;
                return;
            }
            Err((error, retryable)) => {
                let Some(delay) = retry_delay(attempt, retryable) else {
                    publish(events, generation, Update::Failed(error)).await;
                    return;
                };
                let attempt = attempt + 1;
                if !publish(events, generation, Update::Retry { attempt, delay }).await {
                    return;
                }
                tokio::time::sleep(delay).await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    async fn server(responses: Vec<(u16, &'static str)>) -> (Api, tokio::task::JoinHandle<()>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let api = Api::from_access_url(&format!(
            "http://{}/#token=0123456789abcdef0123456789abcdef",
            listener.local_addr().unwrap()
        ))
        .unwrap();
        let task = tokio::spawn(async move {
            for (status, body) in responses {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut request = [0; 8192];
                let n = socket.read(&mut request).await.unwrap();
                assert!(std::str::from_utf8(&request[..n])
                    .unwrap()
                    .starts_with("GET /studio/backends HTTP/1.1\r\n"));
                let response = format!("HTTP/1.1 {status} Fixture\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
                socket.write_all(response.as_bytes()).await.unwrap();
            }
        });
        (api, task)
    }
    #[test]
    fn finite_retry_budget_never_retries_permanent_rejection() {
        let delays: Vec<_> = (0..=10)
            .filter_map(|i| retry_delay(i, true).map(|d| d.as_millis()))
            .collect();
        assert_eq!(delays, [500, 1000, 2000, 4000, 8000, 16000, 30000, 30000]);
        for i in 0..=u8::MAX {
            assert_eq!(retry_delay(i, false), None);
        }
    }
    #[tokio::test]
    async fn temporary_http_failures_recover_without_mutating_requests() {
        let (api, task) = server(vec![
            (503, "temporary unavailable"),
            (429, r#"{"error":"busy"}"#),
            (200, r#"{"backends":[]}"#),
        ])
        .await;
        let (events, output) = async_channel::bounded(16);
        tokio::time::timeout(Duration::from_secs(5), discover(&api, &events, 7))
            .await
            .unwrap();
        for expected in 1..=2 {
            assert!(
                matches!(output.recv().await.unwrap(), Event::Startup { generation: 7, update: Update::Retry { attempt, .. } } if attempt == expected)
            );
        }
        assert!(
            matches!(output.recv().await.unwrap(), Event::Startup { generation: 7, update: Update::Loaded(event) } if matches!(*event, Event::Backends(ref v) if v.is_empty()))
        );
        task.await.unwrap();
    }
    #[tokio::test]
    async fn credentials_and_invalid_success_payloads_fail_without_retries() {
        for (status, body) in [
            (401, r#"{"error":"literal denied"}"#),
            (200, "not json"),
            (200, r#"{"backends":false}"#),
        ] {
            let (api, task) = server(vec![(status, body)]).await;
            let (events, output) = async_channel::bounded(16);
            discover(&api, &events, 11).await;
            assert!(matches!(
                output.recv().await.unwrap(),
                Event::Startup {
                    generation: 11,
                    update: Update::Failed(_)
                }
            ));
            assert!(output.is_empty());
            task.await.unwrap();
        }
    }
    #[tokio::test]
    async fn aborting_backoff_cannot_publish_an_old_generation() {
        let (api, task) = server(vec![(503, "busy")]).await;
        let (events, output) = async_channel::bounded(16);
        let discovery = tokio::spawn(async move { discover(&api, &events, 3).await });
        assert!(matches!(
            output.recv().await.unwrap(),
            Event::Startup {
                generation: 3,
                update: Update::Retry { .. }
            }
        ));
        discovery.abort();
        assert!(discovery.await.unwrap_err().is_cancelled());
        tokio::time::sleep(Duration::from_millis(600)).await;
        assert!(output.is_empty());
        task.await.unwrap();
    }
}
