use std::{
    path::PathBuf,
    process::Stdio,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

use futures_util::StreamExt;
use reqwest::Method;
use serde_json::Value;
use tokio::{
    io::{AsyncBufReadExt, BufReader, Lines},
    process::{Child, ChildStdin, ChildStdout, Command},
};
use url::Url;

const MAX_RESPONSE_BYTES: usize = 32 * 1024 * 1024;

#[derive(Clone)]
pub struct Api {
    retired: Arc<AtomicBool>,
    timeout: Duration,
    pub origin: Url,
    token: String,
    client: reqwest::Client,
}

impl Api {
    pub fn from_access_url(raw: &str) -> Result<Self, String> {
        let mut origin = Url::parse(raw).map_err(|_| "Invalid gateway address")?;
        if origin.scheme() != "http"
            || origin.host_str() != Some("127.0.0.1")
            || origin.port().is_none()
            || origin.path() != "/"
            || origin.query().is_some()
            || !origin.username().is_empty()
            || origin.password().is_some()
        {
            return Err("The native gateway must be a private loopback endpoint".into());
        }
        let token = origin
            .fragment()
            .and_then(|s| s.strip_prefix("token="))
            .filter(|s| s.len() == 32 && s.bytes().all(|b| b.is_ascii_hexdigit()))
            .ok_or("Invalid gateway credential")?
            .to_owned();
        origin.set_fragment(None);
        let client = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(5))
            .read_timeout(Duration::from_secs(45))
            .build()
            .map_err(|e| e.to_string())?;
        Ok(Self {
            retired: Arc::new(AtomicBool::new(false)),
            timeout: Duration::from_secs(35),
            origin,
            token,
            client,
        })
    }

    pub fn socket_request(
        &self,
        path: &str,
    ) -> Result<tokio_tungstenite::tungstenite::http::Request<()>, String> {
        self.check_active()?;
        use tokio_tungstenite::tungstenite::{client::IntoClientRequest, http::HeaderValue};
        if !path.starts_with("/ws/") || path.contains(['?', '#']) {
            return Err("Invalid gateway WebSocket path".into());
        }
        let mut url = self.origin.clone();
        url.set_scheme("ws")
            .map_err(|_| "Invalid WebSocket scheme")?;
        url.set_path(path);
        let mut request = url
            .as_str()
            .into_client_request()
            .map_err(|e| e.to_string())?;
        let headers = request.headers_mut();
        headers.insert(
            "Origin",
            HeaderValue::from_str(self.origin.as_str().trim_end_matches('/'))
                .map_err(|e| e.to_string())?,
        );
        headers.insert(
            "Sec-WebSocket-Protocol",
            HeaderValue::from_str(&format!("codex-thread-studio.auth.{}", self.token))
                .map_err(|e| e.to_string())?,
        );
        Ok(request)
    }

    pub async fn json(
        &self,
        method: Method,
        path: &str,
        body: Option<&Value>,
    ) -> Result<Value, String> {
        self.json_document(method, path, body, None).await
    }

    /// Preserve HTTP rejection status for durable requests whose transport errors
    /// must retain the original idempotency identity.
    pub async fn json_status(
        &self,
        method: Method,
        path: &str,
        body: &Value,
    ) -> Result<(reqwest::StatusCode, Value), String> {
        self.json_status_optional(method, path, Some(body)).await
    }
    pub async fn json_status_optional(
        &self,
        method: Method,
        path: &str,
        body: Option<&Value>,
    ) -> Result<(reqwest::StatusCode, Value), String> {
        let (status, bytes, _) = self.response_bytes(method, path, body, None).await?;
        crate::worker::compute(move || {
            let value = if bytes.is_empty() {
                Value::Null
            } else {
                serde_json::from_slice(&bytes).map_err(|_| "Gateway returned invalid JSON")?
            };
            Ok((status, value))
        })
        .await
    }
    pub async fn command(&self, path: &str, body: &Value) -> Result<Value, String> {
        // OpenCode command endpoints return their completed result rather than
        // an immediate prompt acknowledgement. Keep their finite wait separate
        // from ordinary API requests and the independent SSE connection.
        let timeout = Duration::from_secs(300);
        let client = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(5))
            .read_timeout(timeout)
            .build()
            .map_err(|error| error.to_string())?;
        let api = Self {
            client,
            timeout,
            ..self.clone()
        };
        api.json(Method::POST, path, Some(body)).await
    }

    pub async fn json_document(
        &self,
        method: Method,
        path: &str,
        body: Option<&Value>,
        document: Option<&str>,
    ) -> Result<Value, String> {
        let (status, bytes, _) = self.response_bytes(method, path, body, document).await?;
        let value = if bytes.is_empty() {
            Value::Null
        } else {
            if bytes.len() > 64 * 1024 {
                crate::worker::compute(move || {
                    serde_json::from_slice::<Value>(&bytes)
                        .map_err(|_| "Gateway returned invalid JSON".into())
                })
                .await?
            } else {
                serde_json::from_slice::<Value>(&bytes)
                    .map_err(|_| "Gateway returned invalid JSON")?
            }
        };
        if !status.is_success() {
            return Err(response_error(status, &value));
        }
        Ok(value)
    }

    pub async fn text(&self, path: &str) -> Result<String, String> {
        let (status, bytes, _) = self.response_bytes(Method::GET, path, None, None).await?;
        crate::worker::compute(move || {
            if !status.is_success() {
                let value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
                return Err(response_error(status, &value));
            }
            String::from_utf8(bytes).map_err(|_| "Gateway returned invalid UTF-8".into())
        })
        .await
    }

    pub async fn history_page(&self, path: &str) -> Result<(Value, Option<String>), String> {
        let (status, bytes, headers) = self.response_bytes(Method::GET, path, None, None).await?;
        let cursor = headers
            .get("x-next-cursor")
            .map(|value| {
                value
                    .to_str()
                    .ok()
                    .filter(|value| value.len() <= 4096 && !value.chars().any(char::is_control))
                    .map(str::to_owned)
                    .ok_or_else(|| "OpenCode returned an invalid history cursor".to_string())
            })
            .transpose()?
            .filter(|value| !value.is_empty());
        crate::worker::compute(move || {
            let value: Value = serde_json::from_slice(&bytes)
                .map_err(|_| "Gateway returned invalid history JSON")?;
            if !status.is_success() {
                return Err(response_error(status, &value));
            }
            Ok((value, cursor))
        })
        .await
    }

    pub async fn get_binary_with_headers(
        &self,
        path: &str,
    ) -> Result<(Vec<u8>, reqwest::header::HeaderMap), String> {
        let (status, bytes, headers) = self.response_bytes(Method::GET, path, None, None).await?;
        if status.is_success() {
            Ok((bytes, headers))
        } else {
            Err(response_error(
                status,
                &serde_json::from_slice(&bytes).unwrap_or(Value::Null),
            ))
        }
    }
    pub async fn binary(&self, path: &str, body: &Value) -> Result<Vec<u8>, String> {
        self.binary_with_headers(path, body)
            .await
            .map(|(bytes, _)| bytes)
    }
    pub async fn binary_with_headers(
        &self,
        path: &str,
        body: &Value,
    ) -> Result<(Vec<u8>, reqwest::header::HeaderMap), String> {
        let (status, bytes, headers) = self
            .response_bytes(Method::POST, path, Some(body), None)
            .await?;
        if status.is_success() {
            return Ok((bytes, headers));
        }
        let value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        Err(response_error(status, &value))
    }
    pub(crate) async fn response_bytes(
        &self,
        method: Method,
        path: &str,
        body: Option<&Value>,
        document: Option<&str>,
    ) -> Result<(reqwest::StatusCode, Vec<u8>, reqwest::header::HeaderMap), String> {
        self.check_active()?;
        if !(path.starts_with("/studio/") || path.starts_with("/opencode/"))
            || path.contains('#')
            || path.split('/').any(|p| p == "..")
        {
            return Err("Invalid native gateway resource".into());
        }
        let endpoint = format!("{}{}", self.origin.as_str().trim_end_matches('/'), path);
        let mut request = self
            .client
            .request(method, endpoint)
            .timeout(self.timeout)
            .bearer_auth(&self.token);
        if let Some(body) = body {
            request = request.json(body);
        }
        if let Some(document) = document {
            request = request.header("x-reader-document", document);
        }
        let response = request
            .send()
            .await
            .map_err(|e| format!("Gateway request failed: {}", e.without_url()))?;
        let status = response.status();
        let headers = response.headers().clone();
        if response
            .content_length()
            .is_some_and(|size| size > MAX_RESPONSE_BYTES as u64)
        {
            return Err("Gateway response exceeds the native view limit".into());
        }
        let mut bytes = Vec::new();
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| e.without_url().to_string())?;
            if bytes.len().saturating_add(chunk.len()) > MAX_RESPONSE_BYTES {
                return Err("Gateway response exceeds the native view limit".into());
            }
            bytes.extend_from_slice(&chunk);
        }
        Ok((status, bytes, headers))
    }

    #[cfg(test)]
    pub async fn inline_image(
        &self,
        bytes: Vec<u8>,
    ) -> Result<(Vec<u8>, reqwest::header::HeaderMap), String> {
        self.inline_image_mode(bytes, false).await
    }
    pub async fn inline_animation(
        &self,
        bytes: Vec<u8>,
    ) -> Result<(Vec<u8>, reqwest::header::HeaderMap), String> {
        self.inline_image_mode(bytes, true).await
    }
    async fn inline_image_mode(
        &self,
        bytes: Vec<u8>,
        animation: bool,
    ) -> Result<(Vec<u8>, reqwest::header::HeaderMap), String> {
        self.check_active()?;
        if bytes.is_empty() || bytes.len() > crate::conversation_images::MAX_IMAGE {
            return Err("Invalid inline image size".into());
        }
        let endpoint = self
            .origin
            .join("studio/native/image/inline")
            .map_err(|_| "Invalid image endpoint")?;
        let request = self
            .client
            .post(endpoint)
            .bearer_auth(&self.token)
            .timeout(self.timeout)
            .header("content-type", "application/octet-stream")
            .body(bytes);
        let request = if animation {
            request.header("x-studio-animation", "1")
        } else {
            request
        };
        let response = request
            .send()
            .await
            .map_err(|e| e.without_url().to_string())?;
        let status = response.status();
        let headers = response.headers().clone();
        let mut stream = response.bytes_stream();
        let mut bytes = Vec::new();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| e.without_url().to_string())?;
            if bytes.len() + chunk.len() > MAX_RESPONSE_BYTES {
                return Err("Image response exceeds its budget".into());
            }
            bytes.extend_from_slice(&chunk);
        }
        if !status.is_success() {
            return Err(response_error(
                status,
                &serde_json::from_slice(&bytes).unwrap_or(Value::Null),
            ));
        }
        Ok((bytes, headers))
    }

    pub async fn opencode_events(&self) -> Result<reqwest::Response, String> {
        self.check_active()?;
        let endpoint = self
            .origin
            .join("studio/native/opencode-events")
            .map_err(|_| "Invalid event endpoint")?;
        let response = self
            .client
            .get(endpoint)
            .bearer_auth(&self.token)
            .header("Accept", "text/event-stream")
            .send()
            .await
            .map_err(|error| error.without_url().to_string())?;
        if !response.status().is_success() {
            return Err(format!("OpenCode events HTTP {}", response.status()));
        }
        if !response
            .headers()
            .get("content-type")
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value.starts_with("text/event-stream"))
        {
            return Err("OpenCode did not return an event stream".into());
        }
        Ok(response)
    }
    pub fn retire(&self) {
        self.retired.store(true, Ordering::Release);
    }
    fn check_active(&self) -> Result<(), String> {
        if self.retired.load(Ordering::Acquire) {
            Err("The owned gateway stopped; this API handle is retired".into())
        } else {
            Ok(())
        }
    }
}

pub(crate) fn response_error(status: reqwest::StatusCode, value: &Value) -> String {
    value
        .pointer("/error/message")
        .or_else(|| value.get("error"))
        .and_then(Value::as_str)
        .unwrap_or(status.as_str())
        .to_owned()
}

pub struct ManagedGateway {
    pub api: Api,
    child: Child,
    parent_pipe: Option<ChildStdin>,
}

pub struct GatewayLaunch {
    child: Child,
    parent_pipe: Option<ChildStdin>,
    lines: Option<Lines<BufReader<ChildStdout>>>,
}
impl GatewayLaunch {
    pub fn spawn() -> Result<Self, String> {
        let executable = gateway_executable()?;
        Self::spawn_at(&executable)
    }
    fn spawn_at(executable: &std::path::Path) -> Result<Self, String> {
        Self::spawn_command(Command::new(executable), executable)
    }
    fn spawn_command(mut command: Command, executable: &std::path::Path) -> Result<Self, String> {
        let mut child = command
            .args(["--serve", "--listen", "127.0.0.1:0", "--parent-pipe"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            // The parent pipe is also closed by the OS if this app crashes.
            // Let the gateway clean up snapshots/backends on EOF; an immediate
            // kill-on-drop would bypass its temporary-directory destructors.
            .kill_on_drop(false)
            .spawn()
            .map_err(|e| {
                format!(
                    "Cannot start {}: {e}. Build the native gateway first.",
                    executable.display()
                )
            })?;
        let parent_pipe = child.stdin.take();
        let stdout = child.stdout.take().ok_or("Gateway output is unavailable")?;
        Ok(Self {
            child,
            parent_pipe,
            lines: Some(BufReader::new(stdout).lines()),
        })
    }
    pub async fn ready(&mut self) -> Result<Api, String> {
        let lines = self.lines.as_mut().ok_or("Gateway output is unavailable")?;
        let api = tokio::time::timeout(Duration::from_secs(15), async {
            for _ in 0..8 {
                let line = lines.next_line().await.map_err(|e| e.to_string())?
                    .ok_or("Gateway exited before becoming ready; check the terminal for profile/dependency errors")?;
                if let Some((_, access)) = line.split_once("then open: ") {
                    return Api::from_access_url(access.trim());
                }
            }
            Err("Gateway did not provide its private endpoint".into())
        }).await.map_err(|_| "Gateway startup timed out")??;
        Ok(api)
    }
    pub fn finish(mut self, api: Api) -> ManagedGateway {
        // Never forward the credential-bearing startup output to logs.
        if let Some(mut lines) = self.lines.take() {
            tokio::spawn(async move { while let Ok(Some(_)) = lines.next_line().await {} });
        }
        ManagedGateway {
            api,
            child: self.child,
            parent_pipe: self.parent_pipe,
        }
    }
    pub async fn shutdown(mut self) {
        self.parent_pipe.take();
        stop_child(&mut self.child).await;
    }
}

impl ManagedGateway {
    pub async fn wait(&mut self) -> Result<std::process::ExitStatus, String> {
        self.child.wait().await.map_err(|error| error.to_string())
    }
    pub async fn shutdown(mut self) {
        self.api.retire();
        self.parent_pipe.take(); // EOF shuts down the server and its managed backends.
        stop_child(&mut self.child).await;
    }
}
async fn stop_child(child: &mut Child) {
    if tokio::time::timeout(Duration::from_secs(5), child.wait())
        .await
        .is_err()
    {
        let _ = child.kill().await;
        let _ = child.wait().await;
    }
}

fn gateway_executable() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os("CODEX_THREAD_STUDIO_GATEWAY_BIN") {
        return Ok(PathBuf::from(path));
    }
    Ok(std::env::current_exe()
        .map_err(|e| e.to_string())?
        .with_file_name("codex-thread-studio-gateway"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn retiring_api_clones_prevents_new_reads_writes_commands_images_and_sockets() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let api = Api::from_access_url(&format!(
            "http://{}/#token=0123456789abcdef0123456789abcdef",
            listener.local_addr().unwrap()
        ))
        .unwrap();
        let old = api.clone();
        api.retire();
        tokio::time::timeout(Duration::from_secs(2), async {
            for method in [Method::GET, Method::PATCH] {
                assert!(old
                    .json(
                        method,
                        "/studio/private",
                        Some(&serde_json::json!({"private":"must not send"}))
                    )
                    .await
                    .unwrap_err()
                    .contains("retired"));
            }
            assert!(old
                .command(
                    "/opencode/session/private/command",
                    &serde_json::json!({"command":"must not run"})
                )
                .await
                .is_err());
            assert!(old.inline_image(vec![1, 2, 3]).await.is_err());
            assert!(old.opencode_events().await.is_err());
            assert!(old.socket_request("/ws/codex").is_err());
        })
        .await
        .expect("Retired API attempted network I/O");
        assert!(
            tokio::time::timeout(Duration::from_millis(30), listener.accept())
                .await
                .is_err()
        );
    }
    #[cfg(target_os = "linux")]
    fn launch_fixture(body: &str) -> (tempfile::TempDir, GatewayLaunch) {
        use std::os::unix::fs::PermissionsExt;
        let root = tempfile::tempdir().unwrap();
        let executable = root.path().join("private-gateway");
        std::fs::write(&executable, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700)).unwrap();
        // Invoke the interpreter directly: concurrent test children can briefly
        // inherit a just-closed script writer before exec, causing ETXTBSY if
        // the kernel itself executes that newly written script.
        let mut command = Command::new("/bin/sh");
        command.arg(&executable);
        let launch = GatewayLaunch::spawn_command(command, &executable).unwrap();
        (root, launch)
    }
    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn cancelled_endpoint_wait_retains_owned_child_for_graceful_cleanup() {
        let (_root, mut launch) = launch_fixture("while IFS= read -r line; do :; done");
        let pid = launch.child.id().unwrap();
        assert!(
            tokio::time::timeout(Duration::from_millis(30), launch.ready())
                .await
                .is_err()
        );
        assert!(launch.child.try_wait().unwrap().is_none());
        launch.shutdown().await;
        assert!(!std::path::Path::new(&format!("/proc/{pid}")).exists());
    }
    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn early_exit_and_invalid_endpoints_leave_no_owned_process() {
        let root = tempfile::tempdir().unwrap();
        assert!(GatewayLaunch::spawn_at(&root.path().join("missing-gateway")).is_err());
        for body in ["exit 23", "printf 'ready, then open: https://example.com/\n'; while IFS= read -r line; do :; done"] {
            let (_root, mut launch) = launch_fixture(body);
            let pid = launch.child.id().unwrap();
            assert!(launch.ready().await.is_err());
            launch.shutdown().await;
            assert!(!std::path::Path::new(&format!("/proc/{pid}")).exists());
        }
    }
    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn valid_launch_transfers_pipe_ownership_until_managed_shutdown() {
        let (_root, mut launch) = launch_fixture("printf 'ready, then open: http://127.0.0.1:1234/#token=0123456789abcdef0123456789abcdef\n'; while IFS= read -r line; do :; done");
        let pid = launch.child.id().unwrap();
        let api = launch.ready().await.unwrap();
        assert!(launch.child.try_wait().unwrap().is_none());
        let gateway = launch.finish(api);
        assert_eq!(gateway.api.origin.host_str(), Some("127.0.0.1"));
        gateway.shutdown().await;
        assert!(!std::path::Path::new(&format!("/proc/{pid}")).exists());
    }
    #[tokio::test]
    async fn long_commands_preserve_normal_request_deadlines() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let mut tasks = Vec::new();
            for _ in 0..2 {
                let (mut socket, _) = listener.accept().await.unwrap();
                tasks.push(tokio::spawn(async move{let mut bytes=[0;8192];let _=socket.read(&mut bytes).await;tokio::time::sleep(Duration::from_millis(150)).await;let _=socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 11\r\nConnection: close\r\n\r\n{\"ok\":true}").await;}));
            }
            for task in tasks {
                task.await.unwrap();
            }
        });
        let mut api = Api::from_access_url(&format!(
            "http://{address}/#token=0123456789abcdef0123456789abcdef"
        ))
        .unwrap();
        api.timeout = Duration::from_millis(50);
        assert!(api.json(Method::GET, "/studio/test", None).await.is_err());
        assert_eq!(
            api.command(
                "/opencode/session/test/command",
                &serde_json::json!({"command":"build"})
            )
            .await
            .unwrap()["ok"],
            true
        );
        assert_eq!(api.timeout, Duration::from_millis(50));
        tokio::time::timeout(Duration::from_secs(3), server)
            .await
            .unwrap()
            .unwrap();
    }
    #[test]
    fn endpoint_and_credentials_are_confined_to_loopback() {
        let token = "0123456789abcdef0123456789abcdef";
        let api = Api::from_access_url(&format!("http://127.0.0.1:1234/#token={token}")).unwrap();
        assert!(api.origin.fragment().is_none());
        assert_eq!(
            api.socket_request("/ws/codex").unwrap().headers()["Origin"],
            "http://127.0.0.1:1234"
        );
        for host in [
            "example.com",
            "localhost",
            "127.0.0.1.evil",
            "user@127.0.0.1",
        ] {
            assert!(Api::from_access_url(&format!("http://{host}:1234/#token={token}")).is_err());
        }
        assert!(Api::from_access_url("http://127.0.0.1:1234/#token=short").is_err());
        assert!(api.socket_request("https://example.com").is_err());
    }
}
