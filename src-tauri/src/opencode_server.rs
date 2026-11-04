use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::body::{to_bytes, Body};
use axum::http::{header, HeaderMap, Method, Response, StatusCode, Uri};
use futures_util::StreamExt;
use serde::Serialize;
use serde_json::json;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::Mutex;

use crate::backend_runtime::{BackendRuntime, RuntimeChild};

/// Bounded server-owned transport; callers cannot supply arbitrary routes.
#[derive(Clone, Copy)]
pub(crate) enum RouterOperation<'a> {
    Probe,
    Create,
    UtilityCreate(&'a serde_json::Value),
    UtilityRead(&'a str),
    UtilityDelete(&'a str),
    Read(&'a str),
    Restrict(&'a str),
    Catalog(&'a str),
    Messages(&'a str, usize),
    History(&'a str, usize, Option<&'a str>),
    Prompt(&'a str, &'a serde_json::Value),
    Commands,
    Command(&'a str, &'a serde_json::Value),
    Status,
    Abort(&'a str),
}

pub(crate) fn router_permissions() -> serde_json::Value {
    json!([{"permission":"*","pattern":"*","action":"deny"}])
}

const MAX_PROXY_BODY_BYTES: usize = 32 * 1024 * 1024;
const START_ATTEMPTS: usize = 50;
const PROCESS_POLL_INTERVAL: Duration = Duration::from_secs(1);
const HEALTH_CACHE_TTL: Duration = Duration::from_secs(5);

fn proxy_target(base_url: &str, uri: &Uri) -> Result<reqwest::Url, String> {
    // Axum's Path extractor has already decoded parameter escapes. Forward the
    // original encoded URI so IDs containing '/', '?' or '#' stay one segment.
    let path = uri
        .path()
        .strip_prefix("/opencode/")
        .ok_or("Invalid OpenCode proxy path")?;
    let query = uri.query().map(|q| format!("?{q}")).unwrap_or_default();
    reqwest::Url::parse(&format!("{base_url}/{path}{query}"))
        .map_err(|_| "Invalid OpenCode proxy address".into())
}

#[derive(Clone)]
pub struct OpenCodeServer {
    binary: Arc<str>,
    runtime: Arc<BackendRuntime>,
    process: Arc<Mutex<Option<ProcessConnection>>>,
    startup: Arc<Mutex<()>>,
    generation: Arc<AtomicU64>,
    client: reqwest::Client,
}

#[derive(Clone)]
struct ProcessConnection {
    checked_at: Instant,
    generation: u64,
    base_url: String,
    password: String,
    child: Arc<Mutex<RuntimeChild>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenCodeInfo {
    pub app_name: &'static str,
    pub app_version: &'static str,
    pub binary: String,
    pub protocol: &'static str,
    pub transport: &'static str,
    pub execution_environment: &'static str,
    pub wsl_distribution: Option<String>,
    pub host_platform: &'static str,
    pub backend_version: Option<String>,
    pub reachable: bool,
    pub error: Option<String>,
}

impl OpenCodeServer {
    pub fn new(binary: String, runtime: BackendRuntime) -> Self {
        Self {
            binary: Arc::from(binary),
            runtime: Arc::new(runtime),
            process: Arc::new(Mutex::new(None)),
            startup: Arc::new(Mutex::new(())),
            generation: Arc::new(AtomicU64::new(0)),
            client: reqwest::Client::builder()
                .connect_timeout(Duration::from_secs(2))
                .no_proxy()
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .expect("failed to build OpenCode HTTP client"),
        }
    }

    pub(crate) fn execution_environment(&self) -> &'static str {
        self.runtime.environment()
    }

    pub(crate) async fn router_request(
        &self,
        operation: RouterOperation<'_>,
        workspace: &str,
    ) -> Result<serde_json::Value, String> {
        self.router_page(operation, workspace)
            .await
            .map(|(value, _)| value)
    }

    pub(crate) async fn router_page(
        &self,
        operation: RouterOperation<'_>,
        workspace: &str,
    ) -> Result<(serde_json::Value, Option<String>), String> {
        let connection = self.ensure_started().await?;
        let mut url =
            reqwest::Url::parse(&connection.base_url).map_err(|_| "Invalid OpenCode address")?;
        let (method, body) = match operation {
            RouterOperation::Commands => {
                url.set_path("/command");
                (Method::GET, None)
            }
            RouterOperation::Catalog(cursor) => {
                url.set_path("/experimental/session");
                url.query_pairs_mut()
                    .append_pair("limit", "100")
                    .append_pair("roots", "true")
                    .append_pair("archived", "false");
                if !cursor.is_empty() {
                    url.query_pairs_mut().append_pair("cursor", cursor);
                }
                (Method::GET, None)
            }
            RouterOperation::Status => {
                url.set_path("/session/status");
                (Method::GET, None)
            }
            RouterOperation::Probe => {
                url.set_path("/global/health");
                (Method::GET, None)
            }
            RouterOperation::Create => {
                url.set_path("/session");
                (
                    Method::POST,
                    Some(json!({"title":"Thread Router","permission":router_permissions()})),
                )
            }
            RouterOperation::UtilityCreate(body) => {
                url.set_path("/session");
                (Method::POST, Some(body.clone()))
            }
            RouterOperation::Read(id)
            | RouterOperation::UtilityRead(id)
            | RouterOperation::UtilityDelete(id)
            | RouterOperation::Restrict(id)
            | RouterOperation::Messages(id, _)
            | RouterOperation::History(id, _, _)
            | RouterOperation::Prompt(id, _)
            | RouterOperation::Command(id, _)
            | RouterOperation::Abort(id) => {
                if id.is_empty()
                    || matches!(id, "." | "..")
                    || id.len() > 256
                    || id.chars().any(char::is_control)
                {
                    return Err("Invalid Router session ID".into());
                }
                url.path_segments_mut()
                    .map_err(|_| "Invalid OpenCode address")?
                    .clear()
                    .push("session")
                    .push(id);
                match operation {
                    RouterOperation::Read(_) | RouterOperation::UtilityRead(_) => {
                        (Method::GET, None)
                    }
                    RouterOperation::UtilityDelete(_) => (Method::DELETE, None),
                    RouterOperation::Messages(_, limit) | RouterOperation::History(_, limit, _) => {
                        url.path_segments_mut().unwrap().push("message");
                        if !(2..=100).contains(&limit) {
                            return Err("Invalid Router message window".into());
                        }
                        url.query_pairs_mut()
                            .append_pair("limit", &limit.to_string());
                        if let RouterOperation::History(_, _, Some(cursor)) = operation {
                            if cursor.is_empty()
                                || cursor.len() > 4096
                                || cursor.chars().any(char::is_control)
                            {
                                return Err("Invalid OpenCode history cursor".into());
                            }
                            url.query_pairs_mut().append_pair("before", cursor);
                        }
                        (Method::GET, None)
                    }
                    RouterOperation::Prompt(_, body) => {
                        url.path_segments_mut().unwrap().push("prompt_async");
                        (Method::POST, Some(body.clone()))
                    }
                    RouterOperation::Command(_, body) => {
                        url.path_segments_mut().unwrap().push("command");
                        (Method::POST, Some(body.clone()))
                    }
                    RouterOperation::Abort(_) => {
                        url.path_segments_mut().unwrap().push("abort");
                        (Method::POST, Some(json!({})))
                    }
                    _ => (
                        Method::PATCH,
                        Some(json!({"permission":router_permissions()})),
                    ),
                }
            }
        };
        if !matches!(
            operation,
            RouterOperation::Probe | RouterOperation::Catalog(_)
        ) {
            url.query_pairs_mut().append_pair("directory", workspace);
        }
        let mut request = self
            .client
            .request(method, url)
            .basic_auth("opencode", Some(&connection.password))
            .timeout(Duration::from_secs(
                if matches!(operation, RouterOperation::Command(_, _)) {
                    300
                } else {
                    30
                },
            ));
        if let Some(body) = body {
            request = request.json(&body);
        }
        // Bounded server-owned operations; callers never supply arbitrary paths.
        let response = request
            .send()
            .await
            .map_err(|error| format!("OpenCode Router request failed: {}", error.without_url()))?;
        let status = response.status();
        let limit = if matches!(
            operation,
            RouterOperation::Messages(_, _)
                | RouterOperation::History(_, _, _)
                | RouterOperation::Command(_, _)
                | RouterOperation::Catalog(_)
                | RouterOperation::UtilityRead(_)
        ) {
            MAX_PROXY_BODY_BYTES
        } else if matches!(operation, RouterOperation::Commands) {
            16 * 1024 * 1024
        } else {
            1024 * 1024
        };
        if response
            .content_length()
            .is_some_and(|length| length > limit as u64)
        {
            return Err("OpenCode Router response exceeds its byte budget".into());
        }
        let cursor = if matches!(operation, RouterOperation::History(_, _, _)) {
            response
                .headers()
                .get("x-next-cursor")
                .map(|v| {
                    v.to_str()
                        .ok()
                        .filter(|s| s.len() <= 4096 && !s.chars().any(char::is_control))
                        .map(str::to_owned)
                        .ok_or("Invalid OpenCode history cursor")
                })
                .transpose()?
                .filter(|s| !s.is_empty())
        } else {
            None
        };
        let mut bytes = Vec::new();
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|_| "OpenCode Router metadata was interrupted")?;
            if bytes.len().saturating_add(chunk.len()) > limit {
                return Err("OpenCode Router response exceeds its byte budget".into());
            }
            bytes.extend_from_slice(&chunk);
        }
        if status == StatusCode::NO_CONTENT && bytes.is_empty() {
            return Ok((serde_json::Value::Null, cursor));
        }
        let commands = matches!(operation, RouterOperation::Commands);
        let command = matches!(operation, RouterOperation::Command(_, _));
        let value: serde_json::Value = tokio::task::spawn_blocking(move || {
            if status.is_success() {
                if commands {
                    return crate::native_router_backend::command::inventory(&bytes);
                }
                if command {
                    return crate::native_router_backend::command::acknowledgement(&bytes);
                }
            }
            serde_json::from_slice(&bytes)
                .map_err(|_| "OpenCode Router returned invalid JSON".to_owned())
        })
        .await
        .map_err(|_| "OpenCode Router metadata worker stopped")??;
        if !status.is_success() {
            let message = value
                .get("message")
                .or_else(|| value.pointer("/error/message"))
                .or_else(|| value.get("error"))
                .and_then(serde_json::Value::as_str)
                .unwrap_or("OpenCode rejected Router setup");
            return Err(format!(
                "OpenCode HTTP {status}: {}",
                message.chars().take(512).collect::<String>()
            ));
        }
        Ok((value, cursor))
    }

    pub async fn info(&self) -> OpenCodeInfo {
        match self.ensure_started().await {
            Ok(connection) => match self
                .authenticated_get(&connection, "/global/health")
                .timeout(Duration::from_secs(10))
                .send()
                .await
            {
                Ok(response) if response.status().is_success() => {
                    let value = response
                        .json::<serde_json::Value>()
                        .await
                        .unwrap_or_default();
                    OpenCodeInfo {
                        app_name: "Codex Thread Studio",
                        app_version: env!("CARGO_PKG_VERSION"),
                        binary: self.binary.to_string(),
                        protocol: "OpenCode Server API",
                        transport: "HTTP/OpenAPI + SSE via Studio gateway",
                        execution_environment: self.runtime.environment(),
                        wsl_distribution: self.runtime.wsl_distribution().map(str::to_string),
                        host_platform: std::env::consts::OS,
                        backend_version: value
                            .get("version")
                            .and_then(serde_json::Value::as_str)
                            .map(str::to_string),
                        reachable: true,
                        error: None,
                    }
                }
                Ok(response) => {
                    self.error_info(format!("OpenCode returned HTTP {}", response.status()))
                }
                Err(error) => self.error_info(error.to_string()),
            },
            Err(error) => self.error_info(error),
        }
    }

    fn error_info(&self, error: String) -> OpenCodeInfo {
        OpenCodeInfo {
            app_name: "Codex Thread Studio",
            app_version: env!("CARGO_PKG_VERSION"),
            binary: self.binary.to_string(),
            protocol: "OpenCode Server API",
            transport: "HTTP/OpenAPI + SSE via Studio gateway",
            execution_environment: self.runtime.environment(),
            wsl_distribution: self.runtime.wsl_distribution().map(str::to_string),
            host_platform: std::env::consts::OS,
            backend_version: None,
            reachable: false,
            error: Some(error),
        }
    }

    async fn ensure_started(&self) -> Result<ProcessConnection, String> {
        if let Some(connection) = self.fresh_connection().await {
            return Ok(connection);
        }
        // Coalesce startup/health probes without holding the process lock over I/O.
        let _startup = self.startup.lock().await;
        if let Some(connection) = self.fresh_connection().await {
            return Ok(connection);
        }
        let existing = self.process.lock().await.clone();
        if let Some(mut connection) = existing {
            if self.health_is_ready(&connection).await {
                connection.checked_at = Instant::now();
                let mut process = self.process.lock().await;
                // The exit monitor may have removed this process while the
                // health request was in flight. Do not resurrect its cache.
                if process
                    .as_ref()
                    .is_some_and(|active| active.generation == connection.generation)
                {
                    *process = Some(connection.clone());
                    return Ok(connection);
                }
            }
            *self.process.lock().await = None;
            if let Err(error) = connection.child.lock().await.kill_tree().await {
                eprintln!("Unable to stop the unhealthy OpenCode Server: {error}");
            }
        }

        let listener = TcpListener::bind("127.0.0.1:0")
            .map_err(|error| format!("failed to reserve an OpenCode port: {error}"))?;
        let port = listener
            .local_addr()
            .map_err(|error| format!("failed to inspect the OpenCode port: {error}"))?
            .port();
        drop(listener);

        let generation = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        let password = uuid::Uuid::new_v4().simple().to_string();
        let port_string = port.to_string();
        let mut command = self.runtime.command(
            self.binary.as_ref(),
            &["serve", "--hostname", "127.0.0.1", "--port", &port_string],
            &[("OPENCODE_SERVER_PASSWORD", password.as_str())],
        );
        command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = command
            .spawn()
            .map_err(|error| {
                format!(
                    "failed to start `{}` serve: {error}. Set CODEX_THREAD_STUDIO_OPENCODE_BIN if OpenCode is installed outside PATH.",
                    self.binary
                )
            })?;

        if let Some(stdout) = child.take_stdout() {
            tokio::spawn(log_lines(stdout, "stdout"));
        }
        if let Some(stderr) = child.take_stderr() {
            tokio::spawn(log_lines(stderr, "stderr"));
        }

        let child = Arc::new(Mutex::new(child));
        let base_url = format!("http://127.0.0.1:{port}");
        let connection = ProcessConnection {
            checked_at: Instant::now(),
            generation,
            base_url: base_url.clone(),
            password,
            child: child.clone(),
        };
        for _ in 0..START_ATTEMPTS {
            if self.health_is_ready(&connection).await {
                let connection = ProcessConnection {
                    checked_at: Instant::now(),
                    ..connection
                };
                *self.process.lock().await = Some(connection.clone());
                self.monitor_process(connection.clone());
                return Ok(connection);
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        let _ = child.lock().await.kill_tree().await;
        let mut process = self.process.lock().await;
        if process
            .as_ref()
            .is_some_and(|connection| connection.generation == generation)
        {
            *process = None;
        }

        Err(format!(
            "`{}` did not become ready within 5 seconds",
            self.binary
        ))
    }

    async fn fresh_connection(&self) -> Option<ProcessConnection> {
        self.process
            .lock()
            .await
            .as_ref()
            .filter(|connection| connection.checked_at.elapsed() < HEALTH_CACHE_TTL)
            .cloned()
    }

    fn monitor_process(&self, connection: ProcessConnection) {
        let server = self.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(PROCESS_POLL_INTERVAL).await;
                match connection.child.lock().await.try_wait() {
                    Ok(None) => continue,
                    Ok(Some(_)) => break,
                    Err(error) => {
                        eprintln!("Unable to monitor the OpenCode Server: {error}");
                        break;
                    }
                }
            }
            let mut process = server.process.lock().await;
            if process
                .as_ref()
                .is_some_and(|active| active.generation == connection.generation)
            {
                *process = None;
            }
        });
    }

    async fn health_is_ready(&self, connection: &ProcessConnection) -> bool {
        self.authenticated_get(connection, "/global/health")
            .timeout(Duration::from_secs(10))
            .send()
            .await
            .is_ok_and(|response| response.status().is_success())
    }

    fn authenticated_get(
        &self,
        connection: &ProcessConnection,
        path: &str,
    ) -> reqwest::RequestBuilder {
        self.client
            .get(format!("{}{path}", connection.base_url))
            .basic_auth("opencode", Some(&connection.password))
    }

    pub async fn proxy(
        &self,
        method: Method,
        headers: HeaderMap,
        uri: Uri,
        body: Body,
    ) -> Response<Body> {
        let connection = match self.ensure_started().await {
            Ok(connection) => connection,
            Err(error) => return json_error(StatusCode::BAD_GATEWAY, &error),
        };
        let target = match proxy_target(&connection.base_url, &uri) {
            Ok(url) => url,
            Err(error) => return json_error(StatusCode::BAD_REQUEST, &error),
        };
        let body = match to_bytes(body, MAX_PROXY_BODY_BYTES).await {
            Ok(body) => body,
            Err(error) => return json_error(StatusCode::PAYLOAD_TOO_LARGE, &error.to_string()),
        };

        let mut request = self
            .client
            .request(method, target)
            .basic_auth("opencode", Some(&connection.password));
        for (name, value) in &headers {
            if matches!(
                name.as_str(),
                "host"
                    | "origin"
                    | "referer"
                    | "authorization"
                    | "content-length"
                    | "connection"
                    | "accept-encoding"
            ) {
                continue;
            }
            request = request.header(name, value);
        }

        let upstream = match request.body(body).send().await {
            Ok(response) => response,
            Err(error) => return json_error(StatusCode::BAD_GATEWAY, &error.to_string()),
        };
        let status = upstream.status();
        let upstream_headers = upstream.headers().clone();
        let stream = upstream.bytes_stream();
        let mut response = Response::builder().status(status);
        for (name, value) in &upstream_headers {
            if matches!(
                name.as_str(),
                "content-length" | "transfer-encoding" | "connection"
            ) {
                continue;
            }
            response = response.header(name, value);
        }
        response
            .header(header::CACHE_CONTROL, "no-store")
            .body(Body::from_stream(stream))
            .unwrap_or_else(|error| json_error(StatusCode::BAD_GATEWAY, &error.to_string()))
    }
}

async fn log_lines<R>(reader: R, stream: &'static str)
where
    R: tokio::io::AsyncRead + Unpin,
{
    let mut lines = BufReader::new(reader).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        eprintln!("OpenCode {stream}: {line}");
    }
}

fn json_error(status: StatusCode, message: &str) -> Response<Body> {
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({ "error": { "message": message } }).to_string(),
        ))
        .expect("valid OpenCode proxy error")
}

#[cfg_attr(windows, allow(dead_code))]
pub fn find_opencode_binary(path: &std::ffi::OsStr) -> String {
    if let Ok(value) = std::env::var("CODEX_THREAD_STUDIO_OPENCODE_BIN") {
        if !value.trim().is_empty() {
            return value;
        }
    }
    find_on_path("opencode", path)
        .unwrap_or_else(|| PathBuf::from("opencode"))
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
    #[test]
    fn proxy_preserves_encoded_identity_segments_and_query_scope() {
        let uri:Uri="/opencode/permission/per_a%2Fb%3Fc%23d%25/reply?directory=%2Fproject+%E4%B8%AD%E6%96%87&extra=1".parse().unwrap();
        let target = proxy_target("http://127.0.0.1:12345", &uri).unwrap();
        assert_eq!(target.path(), "/permission/per_a%2Fb%3Fc%23d%25/reply");
        assert_eq!(
            target.query_pairs().collect::<Vec<_>>(),
            vec![
                ("directory".into(), "/project 中文".into()),
                ("extra".into(), "1".into())
            ]
        );
        assert_eq!(target.host_str(), Some("127.0.0.1"));
        assert!(proxy_target("http://127.0.0.1:12345", &"/other/path".parse().unwrap()).is_err());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn concurrent_requests_share_a_health_probe_and_fresh_requests_skip_it() {
        use crate::backend_runtime::WslSettings;
        use axum::{routing::get, Router};
        let calls = Arc::new(AtomicU64::new(0));
        let counted = calls.clone();
        let router = Router::new().route(
            "/global/health",
            get(move || {
                let calls = counted.clone();
                async move {
                    calls.fetch_add(1, Ordering::SeqCst);
                    StatusCode::OK
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base_url = format!("http://{}", listener.local_addr().unwrap());
        let http = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        let runtime = BackendRuntime::new(std::ffi::OsString::new(), WslSettings::default());
        let child = runtime.command("/bin/sleep", &["30"], &[]).spawn().unwrap();
        let child = Arc::new(Mutex::new(child));
        let server = OpenCodeServer::new("unused-in-test".to_string(), runtime);
        *server.process.lock().await = Some(ProcessConnection {
            checked_at: Instant::now() - HEALTH_CACHE_TTL,
            generation: 1,
            base_url,
            password: String::new(),
            child: child.clone(),
        });
        let (first, second, third) = tokio::join!(
            server.ensure_started(),
            server.ensure_started(),
            server.ensure_started()
        );
        assert!(first.is_ok() && second.is_ok() && third.is_ok());
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert!(server.ensure_started().await.is_ok());
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        child.lock().await.kill_tree().await.unwrap();
        http.abort();
    }

    #[test]
    fn streaming_proxy_client_has_no_global_request_timeout() {
        let source = include_str!("opencode_server.rs").replace("\r\n", "\n");
        for newline in ["\n", "\r\n"] {
            let checkout = source.replace('\n', newline);
            // Check the timeout policy, not checkout line endings or rustfmt
            // indentation. CI on Windows commonly uses CRLF source files.
            let compact = checkout.split_whitespace().collect::<String>();
            let builder = compact
                .split("client:reqwest::Client::builder()")
                .nth(1)
                .and_then(|value| value.split(".build()").next())
                .expect("OpenCode client builder");
            assert!(builder.contains(".connect_timeout(Duration::from_secs(2))"));
            assert!(!builder.contains(".timeout("));
            assert!(compact.contains("authenticated_get(&connection,\"/global/health\").timeout(Duration::from_secs(10))"));
        }
    }

    #[test]
    fn configured_binary_takes_precedence() {
        std::env::set_var("CODEX_THREAD_STUDIO_OPENCODE_BIN", "/tmp/custom-opencode");
        assert_eq!(
            find_opencode_binary(std::ffi::OsStr::new("")),
            "/tmp/custom-opencode"
        );
        std::env::remove_var("CODEX_THREAD_STUDIO_OPENCODE_BIN");
    }
}
