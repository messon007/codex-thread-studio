use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::{broadcast, mpsc, Mutex};

use crate::backend_runtime::BackendRuntime;
#[cfg(windows)]
use crate::backend_runtime::RuntimeFile;
use crate::codex_connection::{unsupported_command, ConnectionMode};

const INITIALIZE_REQUEST_ID: i64 = -7_301;
const MAX_CLIENT_MESSAGE_BYTES: usize = 32 * 1024 * 1024;
static INTERNAL_REQUEST_ID: AtomicU64 = AtomicU64::new(1);
const PRIVATE_REQUEST_PREFIX: &str = "studio-native-read-";
type PrivateRequests = Arc<std::sync::Mutex<HashMap<String, tokio::sync::oneshot::Sender<Value>>>>;
struct PrivateRequestGuard {
    requests: PrivateRequests,
    id: String,
}
impl Drop for PrivateRequestGuard {
    fn drop(&mut self) {
        if let Ok(mut pending) = self.requests.lock() {
            pending.remove(&self.id);
        }
    }
}

#[derive(Clone)]
pub struct CodexAppServer {
    binary: Arc<str>,
    prefix_args: Arc<[String]>,
    runtime: Arc<BackendRuntime>,
    connection_mode: Option<ConnectionMode>,
    startup: Arc<Mutex<()>>,
    process: Arc<Mutex<Option<ProcessConnection>>>,
    events: broadcast::Sender<String>,
    lifecycle_events: broadcast::Sender<String>,
    generation: Arc<AtomicU64>,
    private_requests: PrivateRequests,
    pub(crate) router_maps: crate::native_router_map::Registry,
}

#[derive(Clone)]
struct ProcessConnection {
    generation: u64,
    input: mpsc::Sender<String>,
    ready: bool,
    initialization: Option<Value>,
    shared: bool,
}

impl CodexAppServer {
    pub fn new(binary: String, runtime: BackendRuntime) -> Self {
        let mut server = Self::with_prefix(binary, Vec::new(), runtime);
        server.connection_mode = None;
        server
    }

    pub fn with_prefix(binary: String, prefix_args: Vec<String>, runtime: BackendRuntime) -> Self {
        let (events, _) = broadcast::channel(2_048);
        let (lifecycle_events, _) = broadcast::channel(256);
        Self {
            binary: Arc::from(binary),
            prefix_args: Arc::from(prefix_args),
            runtime: Arc::new(runtime),
            connection_mode: Some(ConnectionMode::Embedded),
            startup: Arc::new(Mutex::new(())),
            process: Arc::new(Mutex::new(None)),
            events,
            lifecycle_events,
            generation: Arc::new(AtomicU64::new(0)),
            private_requests: Arc::default(),
            router_maps: Default::default(),
        }
    }

    pub fn with_connection_mode(mut self, mode: ConnectionMode) -> Self {
        self.connection_mode = Some(mode);
        self
    }

    pub fn binary(&self) -> &str {
        &self.binary
    }

    pub fn execution_environment(&self) -> &'static str {
        self.runtime.environment()
    }
    pub(crate) fn current_generation(&self) -> u64 {
        self.generation.load(Ordering::SeqCst)
    }
    pub(crate) fn generation_source(&self) -> Arc<AtomicU64> {
        self.generation.clone()
    }
    pub(crate) fn map_events(&self) -> broadcast::Sender<String> {
        self.events.clone()
    }

    pub fn wsl_distribution(&self) -> Option<&str> {
        self.runtime.wsl_distribution()
    }

    pub async fn run_workspace_command(
        &self,
        binary: &str,
        args: &[&str],
        environment: &[(&str, &str)],
    ) -> std::io::Result<std::process::Output> {
        self.runtime
            .command(binary, args, environment)
            .output()
            .await
    }

    #[cfg(windows)]
    pub async fn read_wsl_file(
        &self,
        root: &str,
        path: &str,
        max_bytes: u64,
        shared_document_directories: &[String],
    ) -> std::io::Result<RuntimeFile> {
        self.runtime
            .read_wsl_file(root, path, max_bytes, shared_document_directories)
            .await
    }

    async fn shared_daemon(&self) -> Result<bool, String> {
        let mode = match self.connection_mode {
            Some(mode) => mode,
            None => ConnectionMode::builtin()?,
        };
        if mode == ConnectionMode::Embedded {
            return Ok(false);
        }
        for command in [
            &["app-server", "proxy", "--help"][..],
            &["app-server", "daemon", "start", "--help"][..],
        ] {
            let args = self
                .prefix_args
                .iter()
                .map(String::as_str)
                .chain(command.iter().copied())
                .collect::<Vec<_>>();
            let output = tokio::time::timeout(
                std::time::Duration::from_secs(15),
                self.run_workspace_command(self.binary.as_ref(), &args, &[]),
            )
            .await
            .map_err(|_| "Timed out checking Codex shared-daemon support".to_string())?
            .map_err(|error| format!("Unable to check Codex shared-daemon support: {error}"))?;
            if !output.status.success() {
                let error = String::from_utf8_lossy(&output.stderr);
                if mode == ConnectionMode::Auto
                    && output.status.code() == Some(2)
                    && unsupported_command(&error)
                {
                    return Ok(false);
                }
                return Err(format!(
                    "Shared Codex daemon requires a CLI with app-server daemon/proxy support: {}",
                    error.chars().take(2048).collect::<String>()
                ));
            }
        }
        let args = self
            .prefix_args
            .iter()
            .map(String::as_str)
            .chain(["app-server", "daemon", "start"])
            .collect::<Vec<_>>();
        let output = tokio::time::timeout(
            std::time::Duration::from_secs(45),
            self.runtime
                .command(self.binary.as_ref(), &args, &[])
                .daemon_output(),
        )
        .await
        .map_err(|_| "Timed out starting the shared Codex daemon".to_string())?
        .map_err(|error| format!("Unable to start the shared Codex daemon: {error}"))?;
        if !output.status.success() {
            return Err(format!(
                "Unable to start the shared Codex daemon: {}",
                String::from_utf8_lossy(&output.stderr)
                    .chars()
                    .take(2048)
                    .collect::<String>()
            ));
        }
        Ok(true)
    }

    async fn ensure_started(&self) -> Result<(), String> {
        let _startup = self.startup.lock().await;
        if self.process.lock().await.is_some() {
            return Ok(());
        }

        let shared = self.shared_daemon().await?;
        let generation = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        let mut arguments = self
            .prefix_args
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>();
        if shared {
            arguments.extend(["app-server", "proxy"]);
        } else {
            arguments.extend(["app-server", "--stdio", "-c", "thread_unload_delay_secs=0"]);
        }
        let mut command =
            self.runtime
                .command(self.binary.as_ref(), &arguments, &[("LOG_FORMAT", "json")]);
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = command.spawn().map_err(|error| {
            format!(
                "failed to start `{}` app-server: {error}. Check the configured backend command or CODEX_THREAD_STUDIO_CODEX_BIN when the executable is installed outside PATH.",
                self.binary
            )
        })?;
        let stdin = child
            .take_stdin()
            .ok_or("codex app-server stdin is unavailable")?;
        let stdout = child
            .take_stdout()
            .ok_or("codex app-server stdout is unavailable")?;
        let stderr = child
            .take_stderr()
            .ok_or("codex app-server stderr is unavailable")?;
        let (input, mut input_rx) = mpsc::channel::<String>(512);

        *self.process.lock().await = Some(ProcessConnection {
            generation,
            input: input.clone(),
            ready: false,
            initialization: None,
            shared,
        });
        self.emit(json!({
            "method": "studio/appServer/status",
            "params": { "state": "starting", "binary": self.binary.as_ref(), "generation": generation }
        }));

        let events = self.events.clone();
        let (upstream_tx, mut upstream_rx) = mpsc::channel::<String>(512);
        if shared {
            // The official proxy carries a WebSocket byte stream over stdio,
            // rather than JSONL. The daemon owns independent client connections.
            let transport = tokio::io::join(stdout, stdin);
            let config = tokio_tungstenite::tungstenite::protocol::WebSocketConfig::default()
                .max_message_size(Some(64 * 1024 * 1024))
                .max_frame_size(Some(64 * 1024 * 1024));
            let handshake = tokio::time::timeout(
                std::time::Duration::from_secs(15),
                tokio_tungstenite::client_async_with_config(
                    "ws://localhost/",
                    transport,
                    Some(config),
                ),
            )
            .await;
            let socket = match handshake {
                Ok(Ok((socket, _))) => socket,
                error => {
                    self.mark_stopped(generation, "shared daemon handshake failed")
                        .await;
                    return Err(format!("Shared Codex daemon handshake failed: {error:?}"));
                }
            };
            let (mut writer, mut reader) = socket.split();
            tokio::spawn(async move {
                use tokio_tungstenite::tungstenite::Message as UpstreamMessage;
                // The official byte proxy can retain stdout after its socket
                // closes until stdin is written. Pings detect this half-close.
                let mut heartbeat = tokio::time::interval(std::time::Duration::from_secs(5));
                heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
                let mut last_received = tokio::time::Instant::now();
                loop {
                    tokio::select! {
                        _ = heartbeat.tick() => {
                            if last_received.elapsed() >= std::time::Duration::from_secs(30) { break; }
                            if writer.send(UpstreamMessage::Ping(Vec::new().into())).await.is_err() { break; }
                        }
                        line = input_rx.recv() => {
                            let Some(line) = line else { break };
                            if writer.send(UpstreamMessage::Text(line.into())).await.is_err() { break; }
                        }
                        message = reader.next() => match message {
                            Some(Ok(UpstreamMessage::Text(text))) => { last_received = tokio::time::Instant::now(); if upstream_tx.send(text.to_string()).await.is_err() { break; } }
                            Some(Ok(UpstreamMessage::Ping(data))) => { last_received = tokio::time::Instant::now(); if writer.send(UpstreamMessage::Pong(data)).await.is_err() { break; } }
                            Some(Ok(UpstreamMessage::Pong(_))) => { last_received = tokio::time::Instant::now(); }
                            Some(Ok(UpstreamMessage::Close(_))) | Some(Err(_)) | None => break,
                            _ => {}
                        }
                    }
                }
            });
        } else {
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
            tokio::spawn(async move {
                let mut lines = BufReader::new(stdout).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    if upstream_tx.send(line).await.is_err() {
                        break;
                    }
                }
            });
        }

        let reader_server = self.clone();
        let reader_input = input.clone();
        tokio::spawn(async move {
            while let Some(line) = upstream_rx.recv().await {
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
                    reader_server
                        .mark_ready(
                            generation,
                            value.get("result").cloned().unwrap_or(Value::Null),
                        )
                        .await;
                    continue;
                }
                if reader_server.router_maps.intercept(
                    &value,
                    generation,
                    reader_input.clone(),
                    reader_server.events.clone(),
                ) {
                    continue;
                }
                reader_server.emit(value);
            }
            reader_server
                .mark_stopped(generation, "App Server transport closed")
                .await;
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
                            "experimentalApi": true,
                            "mcpServerOpenaiFormElicitation": true,
                            "extensions": {
                                "openai/form": {}
                            }
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

    async fn mark_ready(&self, generation: u64, initialization: Value) {
        let mut process = self.process.lock().await;
        let Some(connection) = process
            .as_mut()
            .filter(|value| value.generation == generation)
        else {
            return;
        };
        connection.ready = true;
        connection.initialization = Some(initialization.clone());
        let shared = connection.shared;
        drop(process);
        self.emit(json!({
            "method": "studio/appServer/status",
            "params": {
                "state": "ready",
                "binary": self.binary.as_ref(),
                "generation": generation,
                "initialization": initialization,
                "clientCapabilities": studio_client_capabilities(shared)
            }
        }));
    }

    async fn mark_stopped(&self, generation: u64, reason: &str) {
        let mut process = self.process.lock().await;
        if process
            .as_ref()
            .is_some_and(|connection| connection.generation == generation)
        {
            *process = None;
            if let Ok(mut pending) = self.private_requests.lock() {
                pending.clear();
            }
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
        let shared = process.as_ref().is_some_and(|connection| connection.shared);
        let (state, generation, initialization) = match process.as_ref() {
            Some(connection) if connection.ready => (
                "ready",
                connection.generation,
                connection.initialization.clone().unwrap_or(Value::Null),
            ),
            Some(connection) => ("starting", connection.generation, Value::Null),
            None => (
                "stopped",
                self.generation.load(Ordering::SeqCst),
                Value::Null,
            ),
        };
        json!({
            "method": "studio/appServer/status",
            "params": {
                "state": state,
                "binary": self.binary.as_ref(),
                "generation": generation,
                "initialization": initialization,
                "clientCapabilities": studio_client_capabilities(shared)
            }
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
        self.router_maps.observe_request(&value);
        input
            .send(value.to_string())
            .await
            .map_err(|_| "codex app-server input is closed".to_string())
    }

    pub async fn request(&self, method: &str, params: Value) -> Result<Value, String> {
        self.ensure_started().await?;
        let ready = self
            .process
            .lock()
            .await
            .as_ref()
            .is_some_and(|connection| connection.ready);
        if !ready {
            return Err("codex app-server is still initializing".to_string());
        }
        let id = format!(
            "studio-internal-{}",
            INTERNAL_REQUEST_ID.fetch_add(1, Ordering::Relaxed)
        );
        let mut events = self.events.subscribe();
        self.send(json!({ "id": id, "method": method, "params": params }))
            .await?;
        let response = tokio::time::timeout(std::time::Duration::from_secs(30), async move {
            loop {
                let payload = events.recv().await.map_err(|error| error.to_string())?;
                let value: Value =
                    serde_json::from_str(&payload).map_err(|error| error.to_string())?;
                if value.get("id").and_then(Value::as_str) == Some(id.as_str()) {
                    break Ok::<Value, String>(value);
                }
            }
        })
        .await
        .map_err(|_| "codex app-server environment request timed out".to_string())??;
        if let Some(error) = response.get("error") {
            Err(error
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("codex app-server rejected environment configuration")
                .to_string())
        } else {
            Ok(response.get("result").cloned().unwrap_or(Value::Null))
        }
    }

    fn emit(&self, value: Value) {
        self.router_maps.observe_reply(&value);
        // Large native reads are correlated directly to their HTTP operation.
        // Never rebroadcast them into every live conversation's WebSocket.
        if value.get("method").is_none() {
            if let Some(id) = value
                .get("id")
                .and_then(Value::as_str)
                .filter(|id| id.starts_with(PRIVATE_REQUEST_PREFIX))
            {
                let sender = self
                    .private_requests
                    .lock()
                    .ok()
                    .and_then(|mut pending| pending.remove(id));
                if let Some(sender) = sender {
                    let _ = sender.send(value);
                }
                return;
            }
        }
        let lifecycle = is_lifecycle_message(&value);
        let payload = value.to_string();
        let _ = self.events.send(payload.clone());
        if lifecycle {
            let _ = self.lifecycle_events.send(payload);
        }
    }

    pub fn subscribe_lifecycle(&self) -> broadcast::Receiver<String> {
        self.lifecycle_events.subscribe()
    }

    pub async fn request_private_read(&self, method: &str, params: Value) -> Result<Value, String> {
        if !matches!(
            method,
            "thread/searchOccurrences"
                | "thread/turns/list"
                | "thread/items/list"
                | "thread/read"
                | "thread/list"
        ) {
            return Err("Native operation is not read-only".into());
        }
        self.request_private(method, params).await
    }

    /// The server-owned Router uses this allowlist for provisioning, scoped
    /// delivery, and read-only target metadata; clients cannot supply methods.
    pub(crate) async fn request_router(
        &self,
        method: &str,
        params: Value,
    ) -> Result<Value, String> {
        if !matches!(
            method,
            "thread/start"
                | "thread/name/set"
                | "thread/read"
                | "thread/list"
                | "thread/resume"
                | "thread/turns/list"
                | "turn/start"
                | "turn/interrupt"
                | "skills/list"
                | "plugin/list"
        ) {
            return Err("Unsupported Router setup operation".into());
        }
        self.request_private(method, params).await
    }

    async fn request_private(&self, method: &str, params: Value) -> Result<Value, String> {
        self.ensure_started().await?;
        if !self
            .process
            .lock()
            .await
            .as_ref()
            .is_some_and(|connection| connection.ready)
        {
            return Err("codex app-server is still initializing".into());
        }
        let id = format!(
            "{PRIVATE_REQUEST_PREFIX}{}",
            INTERNAL_REQUEST_ID.fetch_add(1, Ordering::Relaxed)
        );
        let (sender, response) = tokio::sync::oneshot::channel();
        {
            let mut pending = self
                .private_requests
                .lock()
                .map_err(|_| "Native read registry is unavailable")?;
            if pending.len() >= 16 {
                return Err("Too many pending native reads".into());
            }
            pending.insert(id.clone(), sender);
        }
        let _guard = PrivateRequestGuard {
            requests: self.private_requests.clone(),
            id: id.clone(),
        };
        self.send(json!({"id":id,"method":method,"params":params}))
            .await?;
        let mut value = tokio::time::timeout(std::time::Duration::from_secs(30), response)
            .await
            .map_err(|_| "Native conversation read timed out")?
            .map_err(|_| "Native conversation read was interrupted")?;
        if let Some(error) = value.get("error") {
            if error.get("code").and_then(Value::as_i64) == Some(-32601) {
                return Err(format!(
                    "method not found: {}",
                    error
                        .get("message")
                        .and_then(Value::as_str)
                        .unwrap_or("native read")
                ));
            }
            return Err(error
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("App Server rejected the native read")
                .into());
        }
        Ok(value
            .get_mut("result")
            .map(Value::take)
            .unwrap_or(Value::Null))
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
                            if value.get("id").and_then(Value::as_str).is_some_and(|id| id.starts_with(PRIVATE_REQUEST_PREFIX)) {
                                let _ = output.send(Message::Text(client_error("request id uses a reserved native namespace").into())).await;
                                continue;
                            }
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

fn is_lifecycle_message(value: &Value) -> bool {
    matches!(
        value.get("method").and_then(Value::as_str),
        Some(
            "studio/appServer/status" | "thread/status/changed" | "turn/started" | "turn/completed"
        )
    )
}

fn studio_client_capabilities(shared: bool) -> Value {
    json!({
        "sharedSessions": shared,
        "experimentalApi": true,
        "requestUserInput": true,
        "mcpElicitation": true,
        "reconnectReconciliation": true,
        "desktopNotifications": true
    })
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

#[cfg_attr(windows, allow(dead_code))]
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

#[cfg_attr(windows, allow(dead_code))]
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
#[cfg_attr(windows, allow(dead_code))]
fn is_executable(path: &Path) -> bool {
    path.is_file()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn embedded_mode_does_not_probe_or_start_a_daemon() {
        let server = CodexAppServer::with_prefix(
            "nonexistent-fixture-binary".into(),
            Vec::new(),
            BackendRuntime::new(
                std::ffi::OsString::new(),
                crate::backend_runtime::WslSettings::default(),
            ),
        );
        assert!(!server.shared_daemon().await.unwrap());
        assert!(!studio_client_capabilities(false)["sharedSessions"]
            .as_bool()
            .unwrap());
        assert!(studio_client_capabilities(true)["sharedSessions"]
            .as_bool()
            .unwrap());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn shared_start_failure_is_reported_and_old_cli_fallback_is_explicit() {
        use std::os::unix::fs::PermissionsExt;
        let directory =
            std::env::temp_dir().join(format!("studio-daemon-probe-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&directory).unwrap();
        let binary = directory.join("codex-fixture");
        std::fs::write(
            &binary,
            r#"#!/bin/sh
case "$*" in
  'profile app-server proxy --help'|'profile app-server daemon start --help') exit 0 ;;
  'profile app-server daemon start') echo 'fixture start rejected' >&2; exit 1 ;;
  *) echo 'unexpected command' >&2; exit 42 ;;
esac
"#,
        )
        .unwrap();
        std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o700)).unwrap();
        let server = CodexAppServer::with_prefix(
            binary.to_string_lossy().into_owned(),
            vec!["profile".into()],
            BackendRuntime::new(
                std::env::var_os("PATH").unwrap_or_default(),
                crate::backend_runtime::WslSettings::default(),
            ),
        )
        .with_connection_mode(ConnectionMode::Auto);
        assert!(server
            .shared_daemon()
            .await
            .unwrap_err()
            .contains("fixture start rejected"));
        std::fs::write(
            &binary,
            "#!/bin/sh\necho \"error: unrecognized subcommand 'proxy'\" >&2\nexit 2\n",
        )
        .unwrap();
        assert!(!server.shared_daemon().await.unwrap());
        let strict = server.with_connection_mode(ConnectionMode::Shared);
        assert!(strict.shared_daemon().await.is_err());
        std::fs::remove_dir_all(directory).unwrap();
    }

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

    #[test]
    fn keeps_configured_prefix_arguments_separate_from_the_binary() {
        let server = CodexAppServer::with_prefix(
            "custom-launcher".to_string(),
            vec!["codex-runtime".to_string()],
            BackendRuntime::new(std::ffi::OsString::new(), Default::default()),
        );
        assert_eq!(server.binary(), "custom-launcher");
        assert_eq!(server.prefix_args.as_ref(), &["codex-runtime".to_string()]);
    }

    #[test]
    fn isolates_low_volume_lifecycle_messages_for_background_subscribers() {
        for method in [
            "studio/appServer/status",
            "thread/status/changed",
            "turn/started",
            "turn/completed",
        ] {
            assert!(is_lifecycle_message(
                &json!({ "method": method, "params": {} })
            ));
        }
        for method in [
            "item/agentMessage/delta",
            "item/commandExecution/outputDelta",
            "item/completed",
            "thread/tokenUsage/updated",
        ] {
            assert!(!is_lifecycle_message(
                &json!({ "method": method, "params": {} })
            ));
        }
    }

    #[test]
    fn publishes_only_lifecycle_messages_to_the_background_channel() {
        let server = CodexAppServer::new(
            "codex".to_string(),
            BackendRuntime::new(std::ffi::OsString::new(), Default::default()),
        );
        let mut lifecycle = server.subscribe_lifecycle();
        server.emit(json!({
            "method": "turn/completed",
            "params": { "threadId": "thread-1", "turn": { "id": "turn-1" } }
        }));
        let completed = lifecycle.try_recv().expect("lifecycle event");
        assert_eq!(
            serde_json::from_str::<Value>(&completed).expect("valid JSON")["method"],
            "turn/completed"
        );

        server.emit(json!({
            "method": "item/agentMessage/delta",
            "params": { "delta": "not copied" }
        }));
        assert!(matches!(
            lifecycle.try_recv(),
            Err(broadcast::error::TryRecvError::Empty)
        ));
    }
    #[tokio::test]
    async fn private_history_replies_never_enter_desktop_streams() {
        let server = CodexAppServer::new(
            "codex".into(),
            BackendRuntime::new(std::ffi::OsString::new(), Default::default()),
        );
        let mut public = server.events.subscribe();
        let (sender, receiver) = tokio::sync::oneshot::channel();
        server
            .private_requests
            .lock()
            .unwrap()
            .insert("studio-native-read-test".into(), sender);
        server.emit(json!({"id":"studio-native-read-test","result":{"thread":{"id":"private"}}}));
        assert_eq!(receiver.await.unwrap()["result"]["thread"]["id"], "private");
        assert!(matches!(
            public.try_recv(),
            Err(broadcast::error::TryRecvError::Empty)
        ));
        server
            .emit(json!({"id":"studio-native-read-cancelled","result":{"data":["not broadcast"]}}));
        assert!(matches!(
            public.try_recv(),
            Err(broadcast::error::TryRecvError::Empty)
        ));
        server.emit(json!({"id":42,"result":{"data":["normal reply"]}}));
        assert_eq!(
            serde_json::from_str::<Value>(&public.try_recv().unwrap()).unwrap()["id"],
            42
        );
        let (sender, _) = tokio::sync::oneshot::channel();
        server
            .private_requests
            .lock()
            .unwrap()
            .insert("guard".into(), sender);
        drop(PrivateRequestGuard {
            requests: server.private_requests.clone(),
            id: "guard".into(),
        });
        assert!(server.private_requests.lock().unwrap().is_empty());
    }
}
