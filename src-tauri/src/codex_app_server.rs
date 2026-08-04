use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::sync::{broadcast, mpsc, Mutex};

const INITIALIZE_REQUEST_ID: i64 = -7_301;
const MAX_CLIENT_MESSAGE_BYTES: usize = 4 * 1024 * 1024;

#[derive(Clone)]
pub struct CodexAppServer {
    binary: Arc<str>,
    path: Arc<std::ffi::OsString>,
    process: Arc<Mutex<Option<ProcessConnection>>>,
    events: broadcast::Sender<String>,
    generation: Arc<AtomicU64>,
}

#[derive(Clone)]
struct ProcessConnection {
    generation: u64,
    input: mpsc::Sender<String>,
    ready: bool,
}

impl CodexAppServer {
    pub fn new(binary: String, path: std::ffi::OsString) -> Self {
        let (events, _) = broadcast::channel(2_048);
        Self {
            binary: Arc::from(binary),
            path: Arc::new(path),
            process: Arc::new(Mutex::new(None)),
            events,
            generation: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn binary(&self) -> &str {
        &self.binary
    }

    async fn ensure_started(&self) -> Result<(), String> {
        if self.process.lock().await.is_some() {
            return Ok(());
        }

        let generation = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        let mut command = Command::new(self.binary.as_ref());
        command
            .args(["app-server", "--stdio"])
            .env("PATH", self.path.as_ref())
            .env("LOG_FORMAT", "json")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(false);

        let mut child = command.spawn().map_err(|error| {
            format!(
                "failed to start `{}` app-server: {error}. Set CODEX_THREAD_STUDIO_CODEX_BIN if Codex is installed outside PATH.",
                self.binary
            )
        })?;
        let stdin = child
            .stdin
            .take()
            .ok_or("codex app-server stdin is unavailable")?;
        let stdout = child
            .stdout
            .take()
            .ok_or("codex app-server stdout is unavailable")?;
        let stderr = child
            .stderr
            .take()
            .ok_or("codex app-server stderr is unavailable")?;
        let (input, mut input_rx) = mpsc::channel::<String>(512);

        *self.process.lock().await = Some(ProcessConnection {
            generation,
            input: input.clone(),
            ready: false,
        });
        self.emit(json!({
            "method": "studio/appServer/status",
            "params": { "state": "starting", "binary": self.binary.as_ref(), "generation": generation }
        }));

        let events = self.events.clone();
        tokio::spawn(async move {
            let mut stdin = stdin;
            while let Some(line) = input_rx.recv().await {
                if stdin.write_all(line.as_bytes()).await.is_err()
                    || stdin.write_all(b"\n").await.is_err()
                    || stdin.flush().await.is_err()
                {
                    break;
                }
            }
        });

        let reader_server = self.clone();
        let reader_input = input.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let value = match serde_json::from_str::<Value>(&line) {
                    Ok(value) => value,
                    Err(_) => {
                        let _ = events.send(json!({
                            "method": "studio/appServer/protocolError",
                            "params": { "message": "App Server emitted invalid JSON", "line": line }
                        }).to_string());
                        continue;
                    }
                };

                if value.get("id").and_then(Value::as_i64) == Some(INITIALIZE_REQUEST_ID) {
                    if value.get("error").is_some() {
                        let _ = events.send(value.to_string());
                        reader_server
                            .mark_stopped(generation, "initialization failed")
                            .await;
                        continue;
                    }
                    let initialized = json!({ "method": "initialized", "params": {} }).to_string();
                    if reader_input.send(initialized).await.is_err() {
                        reader_server
                            .mark_stopped(generation, "stdin closed during initialization")
                            .await;
                        break;
                    }
                    reader_server.mark_ready(generation).await;
                    continue;
                }
                let _ = events.send(value.to_string());
            }
        });

        let stderr_events = self.events.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let _ = stderr_events.send(
                    json!({
                        "method": "studio/appServer/log",
                        "params": { "line": line }
                    })
                    .to_string(),
                );
            }
        });

        let wait_server = self.clone();
        tokio::spawn(async move {
            let reason = match child.wait().await {
                Ok(status) => format!("exited with {status}"),
                Err(error) => format!("could not be monitored: {error}"),
            };
            wait_server.mark_stopped(generation, &reason).await;
        });

        input
            .send(
                json!({
                    "method": "initialize",
                    "id": INITIALIZE_REQUEST_ID,
                    "params": {
                        "capabilities": {
                            "experimentalApi": true
                        },
                        "clientInfo": {
                            "name": "codex_thread_studio",
                            "title": "Codex Thread Studio",
                            "version": env!("CARGO_PKG_VERSION")
                        }
                    }
                })
                .to_string(),
            )
            .await
            .map_err(|_| "codex app-server stopped before initialization".to_string())
    }

    async fn mark_ready(&self, generation: u64) {
        let mut process = self.process.lock().await;
        let Some(connection) = process
            .as_mut()
            .filter(|value| value.generation == generation)
        else {
            return;
        };
        connection.ready = true;
        drop(process);
        self.emit(json!({
            "method": "studio/appServer/status",
            "params": { "state": "ready", "binary": self.binary.as_ref(), "generation": generation }
        }));
    }

    async fn mark_stopped(&self, generation: u64, reason: &str) {
        let mut process = self.process.lock().await;
        if process
            .as_ref()
            .is_some_and(|connection| connection.generation == generation)
        {
            *process = None;
        } else {
            return;
        }
        drop(process);
        self.emit(json!({
            "method": "studio/appServer/status",
            "params": { "state": "stopped", "reason": reason, "generation": generation }
        }));
    }

    async fn status_message(&self) -> String {
        let process = self.process.lock().await;
        let (state, generation) = match process.as_ref() {
            Some(connection) if connection.ready => ("ready", connection.generation),
            Some(connection) => ("starting", connection.generation),
            None => ("stopped", self.generation.load(Ordering::SeqCst)),
        };
        json!({
            "method": "studio/appServer/status",
            "params": { "state": state, "binary": self.binary.as_ref(), "generation": generation }
        })
        .to_string()
    }

    async fn send(&self, value: Value) -> Result<(), String> {
        validate_client_message(&value)?;
        let input = self
            .process
            .lock()
            .await
            .as_ref()
            .map(|connection| connection.input.clone())
            .ok_or("codex app-server is not running")?;
        input
            .send(value.to_string())
            .await
            .map_err(|_| "codex app-server input is closed".to_string())
    }

    fn emit(&self, value: Value) {
        let _ = self.events.send(value.to_string());
    }

    pub async fn bridge(&self, socket: WebSocket) {
        let (mut output, mut input) = socket.split();
        let mut events = self.events.subscribe();
        if let Err(error) = self.ensure_started().await {
            let payload = json!({
                "method": "studio/appServer/status",
                "params": { "state": "error", "message": error, "binary": self.binary.as_ref() }
            });
            let _ = output.send(Message::Text(payload.to_string().into())).await;
            return;
        }
        if output
            .send(Message::Text(self.status_message().await.into()))
            .await
            .is_err()
        {
            return;
        }

        loop {
            tokio::select! {
                browser_message = input.next() => {
                    let Some(Ok(browser_message)) = browser_message else { break };
                    let text = match browser_message {
                        Message::Text(text) => text.to_string(),
                        Message::Close(_) => break,
                        Message::Ping(value) => {
                            if output.send(Message::Pong(value)).await.is_err() { break; }
                            continue;
                        }
                        _ => continue,
                    };
                    if text.len() > MAX_CLIENT_MESSAGE_BYTES {
                        let _ = output.send(Message::Text(client_error("message is too large").into())).await;
                        continue;
                    }
                    match serde_json::from_str::<Value>(&text) {
                        Ok(value) => {
                            if let Err(error) = self.send(value).await {
                                let _ = output.send(Message::Text(client_error(&error).into())).await;
                            }
                        }
                        Err(error) => {
                            let _ = output.send(Message::Text(client_error(&format!("invalid JSON: {error}")).into())).await;
                        }
                    }
                }
                event = events.recv() => {
                    let payload = match event {
                        Ok(payload) => payload,
                        Err(broadcast::error::RecvError::Lagged(skipped)) => json!({
                            "method": "studio/appServer/lagged",
                            "params": { "skipped": skipped }
                        }).to_string(),
                        Err(broadcast::error::RecvError::Closed) => break,
                    };
                    if output.send(Message::Text(payload.into())).await.is_err() { break; }
                }
            }
        }
    }
}

fn validate_client_message(value: &Value) -> Result<(), String> {
    let object = value
        .as_object()
        .ok_or("App Server message must be a JSON object")?;
    if object.get("id").and_then(Value::as_i64) == Some(INITIALIZE_REQUEST_ID) {
        return Err("request id is reserved by Studio".to_string());
    }
    if matches!(
        object.get("method").and_then(Value::as_str),
        Some("initialize" | "initialized")
    ) {
        return Err("Studio owns the App Server initialization handshake".to_string());
    }
    if !object.contains_key("method") && !object.contains_key("id") {
        return Err("message must be a request, notification, or response".to_string());
    }
    Ok(())
}

fn client_error(message: &str) -> String {
    json!({
        "method": "studio/appServer/clientError",
        "params": { "message": message }
    })
    .to_string()
}

pub fn find_codex_binary(path: &std::ffi::OsStr) -> String {
    if let Ok(value) = std::env::var("CODEX_THREAD_STUDIO_CODEX_BIN") {
        if !value.trim().is_empty() {
            return value;
        }
    }
    find_on_path("codex", path)
        .unwrap_or_else(|| PathBuf::from("codex"))
        .to_string_lossy()
        .into_owned()
}

fn find_on_path(name: &str, path: &std::ffi::OsStr) -> Option<PathBuf> {
    std::env::split_paths(path)
        .map(|directory| directory.join(name))
        .find(|candidate| is_executable(candidate))
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    path.metadata()
        .map(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable(path: &Path) -> bool {
    path.is_file()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_initialization_messages_from_the_webview() {
        assert!(validate_client_message(&json!({"method": "initialize", "id": 1})).is_err());
        assert!(validate_client_message(&json!({"method": "initialized"})).is_err());
    }

    #[test]
    fn accepts_requests_notifications_and_approval_responses() {
        assert!(
            validate_client_message(&json!({"method": "thread/read", "id": 1, "params": {}}))
                .is_ok()
        );
        assert!(validate_client_message(&json!({"method": "custom/event", "params": {}})).is_ok());
        assert!(
            validate_client_message(&json!({"id": 9, "result": {"decision": "accept"}})).is_ok()
        );
    }
}
