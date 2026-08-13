use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use axum::body::{to_bytes, Body};
use axum::http::{header, HeaderMap, Method, Response, StatusCode, Uri};
use serde::Serialize;
use serde_json::json;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::Mutex;

use crate::backend_runtime::{BackendRuntime, RuntimeChild};

const MAX_PROXY_BODY_BYTES: usize = 4 * 1024 * 1024;
const START_ATTEMPTS: usize = 50;
const PROCESS_POLL_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Clone)]
pub struct OpenCodeServer {
    binary: Arc<str>,
    runtime: Arc<BackendRuntime>,
    process: Arc<Mutex<Option<ProcessConnection>>>,
    generation: Arc<AtomicU64>,
    client: reqwest::Client,
}

#[derive(Clone)]
struct ProcessConnection {
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
            generation: Arc::new(AtomicU64::new(0)),
            client: reqwest::Client::builder()
                .connect_timeout(Duration::from_secs(2))
                .timeout(Duration::from_secs(10))
                .build()
                .expect("failed to build OpenCode HTTP client"),
        }
    }

    pub async fn info(&self) -> OpenCodeInfo {
        match self.ensure_started().await {
            Ok(connection) => match self
                .authenticated_get(&connection, "/global/health")
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
        let mut process = self.process.lock().await;
        if let Some(connection) = process.as_ref().cloned() {
            if self.health_is_ready(&connection).await {
                return Ok(connection);
            }
            *process = None;
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
            generation,
            base_url: base_url.clone(),
            password,
            child: child.clone(),
        };
        *process = Some(connection.clone());
        drop(process);

        for _ in 0..START_ATTEMPTS {
            if self.health_is_ready(&connection).await {
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
        path: String,
        method: Method,
        headers: HeaderMap,
        uri: Uri,
        body: Body,
    ) -> Response<Body> {
        let connection = match self.ensure_started().await {
            Ok(connection) => connection,
            Err(error) => return json_error(StatusCode::BAD_GATEWAY, &error),
        };
        let query = uri
            .query()
            .map(|value| format!("?{value}"))
            .unwrap_or_default();
        let target = format!("{}/{path}{query}", connection.base_url);
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
    fn configured_binary_takes_precedence() {
        std::env::set_var("CODEX_THREAD_STUDIO_OPENCODE_BIN", "/tmp/custom-opencode");
        assert_eq!(
            find_opencode_binary(std::ffi::OsStr::new("")),
            "/tmp/custom-opencode"
        );
        std::env::remove_var("CODEX_THREAD_STUDIO_OPENCODE_BIN");
    }
}
