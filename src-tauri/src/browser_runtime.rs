use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use tokio_tungstenite::tungstenite::Message;
use url::{Host, Url};

pub const MAX_BROWSER_URL_BYTES: usize = 8 * 1024;
const DEVTOOLS_ACTIVE_PORT: &str = "DevToolsActivePort";
const GLOBAL_BROWSER_KEY: &str = "studio";

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct BrowserAgentPreferences {
    pub enabled: bool,
    pub provider: String,
    pub profile: String,
    pub approval: String,
    #[serde(default)]
    pub allowed_origins: Vec<String>,
}

impl Default for BrowserAgentPreferences {
    fn default() -> Self {
        Self {
            enabled: false,
            provider: "playwright-mcp".to_string(),
            profile: "persistent".to_string(),
            approval: "interactive".to_string(),
            allowed_origins: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct BrowserPreferences {
    /// Kept as a compatibility switch for Phase 0 settings. Launching the
    /// browser from the Studio UI is itself the explicit enable action.
    pub enabled: bool,
    pub restore_tabs: bool,
    pub allow_http: bool,
    pub allow_private_network: bool,
    pub allow_localhost: bool,
    pub preview_javascript: bool,
    pub external_open_fallback: bool,
    pub embedded_width: u32,
    #[serde(default)]
    pub agent: BrowserAgentPreferences,
}

impl Default for BrowserPreferences {
    fn default() -> Self {
        Self {
            enabled: false,
            restore_tabs: true,
            allow_http: true,
            allow_private_network: false,
            allow_localhost: true,
            preview_javascript: true,
            external_open_fallback: true,
            embedded_width: 720,
            agent: BrowserAgentPreferences::default(),
        }
    }
}

impl BrowserPreferences {
    pub fn validate(&self) -> Result<(), String> {
        if !(480..=2400).contains(&self.embedded_width) {
            return Err("embedded browser width must be between 480 and 2400".to_string());
        }
        if self.agent.provider != "playwright-mcp" {
            return Err("browser agent provider must be playwright-mcp".to_string());
        }
        if !matches!(self.agent.profile.as_str(), "persistent" | "isolated") {
            return Err("browser agent profile must be persistent or isolated".to_string());
        }
        if !matches!(self.agent.approval.as_str(), "interactive" | "read-only") {
            return Err("browser agent approval must be interactive or read-only".to_string());
        }
        if self.agent.allowed_origins.len() > 128
            || self.agent.allowed_origins.iter().any(|origin| {
                origin.len() > MAX_BROWSER_URL_BYTES || validate_http_origin(origin).is_err()
            })
        {
            return Err("browser agent allowed origins are invalid".to_string());
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct BrowserController {
    preferences: BrowserPreferences,
    data_root: PathBuf,
    browser_override: Option<PathBuf>,
    client: reqwest::Client,
    runtimes: Arc<Mutex<HashMap<String, BrowserRuntime>>>,
    launch_lock: Arc<tokio::sync::Mutex<()>>,
    events: broadcast::Sender<BrowserWorkspaceStatus>,
}

struct BrowserRuntime {
    profile_directory: PathBuf,
    endpoint: Option<String>,
    executable: Option<PathBuf>,
    child: Option<Child>,
    launched_at: Option<u64>,
    last_error: Option<String>,
    browser_name: Option<String>,
    tabs: Vec<BrowserTab>,
    monitor_endpoint: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BrowserNavigation {
    pub url: Url,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserWorkspaceRequest {
    #[serde(default)]
    pub url: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserTabRequest {
    pub url: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserTabActionRequest {
    pub tab_id: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserWorkspaceStatus {
    pub profile_directory: String,
    pub running: bool,
    pub browser_name: Option<String>,
    pub executable: Option<String>,
    pub process_id: Option<u32>,
    pub launched_at: Option<u64>,
    pub last_error: Option<String>,
    pub tabs: Vec<BrowserTab>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserTab {
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub url: String,
    #[serde(default, rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub favicon_url: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BrowserVersion {
    #[serde(default, rename = "Browser")]
    browser: String,
    #[serde(default, rename = "webSocketDebuggerUrl")]
    web_socket_debugger_url: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CdpTargetInfo {
    target_id: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    url: String,
    #[serde(default, rename = "type")]
    kind: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BrowserPolicyError {
    Disabled,
    EmptyUrl,
    UrlTooLong,
    InvalidUrl,
    SchemeNotAllowed(String),
    HttpNotAllowed,
    LocalhostNotAllowed,
    PrivateNetworkNotAllowed,
}

impl std::fmt::Display for BrowserPolicyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Disabled => write!(formatter, "Browser Workspace is disabled"),
            Self::EmptyUrl => write!(formatter, "browser URL is empty"),
            Self::UrlTooLong => write!(formatter, "browser URL is too long"),
            Self::InvalidUrl => write!(formatter, "browser URL is invalid"),
            Self::SchemeNotAllowed(scheme) => {
                write!(formatter, "browser URL scheme '{scheme}' is not allowed")
            }
            Self::HttpNotAllowed => write!(formatter, "HTTP browser navigation is disabled"),
            Self::LocalhostNotAllowed => {
                write!(formatter, "localhost browser navigation is disabled")
            }
            Self::PrivateNetworkNotAllowed => {
                write!(formatter, "private-network browser navigation is disabled")
            }
        }
    }
}

impl BrowserController {
    pub fn new(preferences: BrowserPreferences) -> Self {
        let browser_override = env::var_os("CODEX_THREAD_STUDIO_BROWSER")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from);
        Self::with_runtime(preferences, browser_data_root(), browser_override)
    }

    fn with_runtime(
        preferences: BrowserPreferences,
        data_root: PathBuf,
        browser_override: Option<PathBuf>,
    ) -> Self {
        let (events, _) = broadcast::channel(128);
        Self {
            preferences,
            data_root,
            browser_override,
            client: reqwest::Client::builder()
                .connect_timeout(Duration::from_secs(2))
                .timeout(Duration::from_secs(5))
                .build()
                .expect("valid browser HTTP client"),
            runtimes: Arc::new(Mutex::new(HashMap::new())),
            launch_lock: Arc::new(tokio::sync::Mutex::new(())),
            events,
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.preferences.enabled
    }

    pub fn browser_available(&self) -> bool {
        find_browser_executable(self.browser_override.as_deref()).is_ok()
    }

    pub fn subscribe(&self) -> broadcast::Receiver<BrowserWorkspaceStatus> {
        self.events.subscribe()
    }

    pub fn prepare_navigation(&self, input: &str) -> Result<BrowserNavigation, BrowserPolicyError> {
        if !self.is_enabled() {
            return Err(BrowserPolicyError::Disabled);
        }
        self.prepare_explicit_navigation(input)
    }

    /// Browser launch and address submission are explicit user actions, so
    /// they remain usable for settings written by the Phase 0 disabled-by-
    /// default prototype. URL and private-network policy still apply.
    pub fn prepare_explicit_navigation(
        &self,
        input: &str,
    ) -> Result<BrowserNavigation, BrowserPolicyError> {
        let url = validate_browser_url(input, &self.preferences)?;
        Ok(BrowserNavigation { url })
    }

    pub async fn workspace_status(&self) -> Result<BrowserWorkspaceStatus, String> {
        let key = GLOBAL_BROWSER_KEY.to_string();
        self.ensure_runtime_entry(&key)?;
        let endpoint = self.reconnect_endpoint(&key).await?;
        if let Some(endpoint) = endpoint {
            return self.status_from_endpoint(&key, &endpoint).await;
        }
        self.offline_status(&key)
    }

    pub async fn launch_workspace(
        &self,
        request: BrowserWorkspaceRequest,
    ) -> Result<BrowserWorkspaceStatus, String> {
        let _launch_guard = self.launch_lock.lock().await;
        let navigation = request
            .url
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .map(|value| self.prepare_explicit_navigation(value))
            .transpose()
            .map_err(|error| error.to_string())?;
        let key = GLOBAL_BROWSER_KEY.to_string();
        self.ensure_runtime_entry(&key)?;

        if let Some(endpoint) = self.reconnect_endpoint(&key).await? {
            if let Some(navigation) = navigation {
                self.create_tab_at(&endpoint, navigation.url.as_str())
                    .await?;
            }
            return self.status_from_endpoint(&key, &endpoint).await;
        }

        let executable = find_browser_executable(self.browser_override.as_deref())?;
        let profile_directory = self.profile_directory(&key);
        fs::create_dir_all(&profile_directory)
            .map_err(|error| format!("Unable to create Browser profile: {error}"))?;
        let _ = fs::remove_file(profile_directory.join(DEVTOOLS_ACTIVE_PORT));

        let initial_url = navigation
            .as_ref()
            .map(|item| item.url.as_str())
            .unwrap_or("about:blank");
        let mut command = Command::new(&executable);
        command
            .arg(format!("--user-data-dir={}", profile_directory.display()))
            .arg("--remote-debugging-port=0")
            .arg("--no-first-run")
            .arg("--no-default-browser-check")
            .arg("--new-window")
            .arg(initial_url)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        #[cfg(target_os = "linux")]
        command.arg("--class=codex-thread-studio-browser");
        let child = command
            .spawn()
            .map_err(|error| format!("Unable to start {}: {error}", executable.display()))?;
        {
            let mut runtimes = self.runtime_lock()?;
            let runtime = runtimes
                .get_mut(&key)
                .ok_or_else(|| "Browser runtime disappeared during launch".to_string())?;
            runtime.executable = Some(executable);
            runtime.launched_at = Some(unix_millis());
            runtime.last_error = None;
            runtime.child = Some(child);
        }

        let endpoint = match self.wait_for_endpoint(&key).await {
            Ok(endpoint) => endpoint,
            Err(error) => {
                if let Ok(mut runtimes) = self.runtimes.lock() {
                    if let Some(runtime) = runtimes.get_mut(&key) {
                        runtime.last_error = Some(error.clone());
                        if let Some(mut child) = runtime.child.take() {
                            let _ = child.kill();
                            let _ = child.wait();
                        }
                    }
                }
                return Err(error);
            }
        };
        self.status_from_endpoint(&key, &endpoint).await
    }

    pub async fn create_tab(&self, request: BrowserTabRequest) -> Result<BrowserTab, String> {
        let navigation = self
            .prepare_explicit_navigation(&request.url)
            .map_err(|error| error.to_string())?;
        let key = GLOBAL_BROWSER_KEY.to_string();
        self.ensure_runtime_entry(&key)?;
        let endpoint = self
            .reconnect_endpoint(&key)
            .await?
            .ok_or_else(|| "Browser Workspace is not running".to_string())?;
        self.create_tab_at(&endpoint, navigation.url.as_str()).await
    }

    pub async fn activate_tab(&self, request: BrowserTabActionRequest) -> Result<(), String> {
        self.tab_action(request, "activate").await
    }

    pub async fn close_tab(&self, request: BrowserTabActionRequest) -> Result<(), String> {
        self.tab_action(request, "close").await
    }

    async fn tab_action(
        &self,
        request: BrowserTabActionRequest,
        action: &str,
    ) -> Result<(), String> {
        validate_tab_id(&request.tab_id)?;
        let key = GLOBAL_BROWSER_KEY.to_string();
        self.ensure_runtime_entry(&key)?;
        let endpoint = self
            .reconnect_endpoint(&key)
            .await?
            .ok_or_else(|| "Browser Workspace is not running".to_string())?;
        let response = self
            .client
            .get(format!("{endpoint}/json/{action}/{}", request.tab_id))
            .send()
            .await
            .map_err(|error| format!("Unable to contact Browser Workspace: {error}"))?;
        if !response.status().is_success() {
            return Err(format!(
                "Browser tab {action} failed: HTTP {}",
                response.status()
            ));
        }
        Ok(())
    }

    fn ensure_runtime_entry(&self, key: &str) -> Result<(), String> {
        let mut runtimes = self.runtime_lock()?;
        runtimes
            .entry(key.to_string())
            .or_insert_with(|| BrowserRuntime {
                profile_directory: self.profile_directory(key),
                endpoint: None,
                executable: None,
                child: None,
                launched_at: None,
                last_error: None,
                browser_name: None,
                tabs: Vec::new(),
                monitor_endpoint: None,
            });
        Ok(())
    }

    async fn reconnect_endpoint(&self, key: &str) -> Result<Option<String>, String> {
        let existing = {
            let mut runtimes = self.runtime_lock()?;
            let runtime = runtimes
                .get_mut(key)
                .ok_or_else(|| "Browser runtime was not initialized".to_string())?;
            if let Some(child) = runtime.child.as_mut() {
                if let Ok(Some(_)) = child.try_wait() {
                    runtime.child = None;
                    runtime.endpoint = None;
                }
            }
            runtime.endpoint.clone()
        };
        if let Some(endpoint) = existing {
            if self.browser_version(&endpoint).await.is_ok() {
                return Ok(Some(endpoint));
            }
        }
        let profile = {
            self.runtime_lock()?
                .get(key)
                .map(|runtime| runtime.profile_directory.clone())
                .ok_or_else(|| "Browser runtime was not initialized".to_string())?
        };
        let endpoint = endpoint_from_profile(&profile).ok();
        if let Some(candidate) = endpoint {
            if self.browser_version(&candidate).await.is_ok() {
                if let Some(runtime) = self.runtime_lock()?.get_mut(key) {
                    runtime.endpoint = Some(candidate.clone());
                    runtime.last_error = None;
                }
                return Ok(Some(candidate));
            }
        }
        if let Some(runtime) = self.runtime_lock()?.get_mut(key) {
            runtime.endpoint = None;
        }
        Ok(None)
    }

    async fn wait_for_endpoint(&self, key: &str) -> Result<String, String> {
        for _ in 0..100 {
            if let Some(endpoint) = self.reconnect_endpoint(key).await? {
                return Ok(endpoint);
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        Err("Chromium started but its DevTools endpoint did not become ready".to_string())
    }

    async fn status_from_endpoint(
        &self,
        key: &str,
        endpoint: &str,
    ) -> Result<BrowserWorkspaceStatus, String> {
        let version = self.browser_version(endpoint).await?;
        let tabs = self.list_tabs_at(endpoint).await?;
        let status = {
            let mut runtimes = self.runtime_lock()?;
            let runtime = runtimes
                .get_mut(key)
                .ok_or_else(|| "Browser runtime was not initialized".to_string())?;
            runtime.endpoint = Some(endpoint.to_string());
            runtime.last_error = None;
            runtime.browser_name = Some(version.browser.clone());
            runtime.tabs = tabs;
            runtime_status(runtime, true)
        };
        self.emit(status.clone());
        if !version.web_socket_debugger_url.is_empty() {
            self.ensure_cdp_monitor(
                key.to_string(),
                endpoint.to_string(),
                version.web_socket_debugger_url,
            )?;
        }
        Ok(status)
    }

    fn offline_status(&self, key: &str) -> Result<BrowserWorkspaceStatus, String> {
        let mut runtimes = self.runtime_lock()?;
        let runtime = runtimes
            .get_mut(key)
            .ok_or_else(|| "Browser runtime was not initialized".to_string())?;
        let process_id = runtime.child.as_ref().map(Child::id);
        if process_id.is_some() {
            runtime.last_error =
                Some("Browser process is running but DevTools is unavailable".to_string());
        }
        runtime.browser_name = None;
        runtime.tabs.clear();
        let status = runtime_status(runtime, false);
        self.emit(status.clone());
        Ok(status)
    }

    async fn browser_version(&self, endpoint: &str) -> Result<BrowserVersion, String> {
        self.client
            .get(format!("{endpoint}/json/version"))
            .send()
            .await
            .map_err(|error| format!("Unable to connect to Chromium DevTools: {error}"))?
            .error_for_status()
            .map_err(|error| format!("Chromium DevTools rejected the request: {error}"))?
            .json::<BrowserVersion>()
            .await
            .map_err(|error| format!("Invalid Chromium DevTools response: {error}"))
    }

    async fn list_tabs_at(&self, endpoint: &str) -> Result<Vec<BrowserTab>, String> {
        let mut tabs = self
            .client
            .get(format!("{endpoint}/json/list"))
            .send()
            .await
            .map_err(|error| format!("Unable to list Browser tabs: {error}"))?
            .error_for_status()
            .map_err(|error| format!("Browser tab listing failed: {error}"))?
            .json::<Vec<BrowserTab>>()
            .await
            .map_err(|error| format!("Invalid Browser tab response: {error}"))?;
        tabs.retain(|tab| tab.kind == "page" && !tab.url.starts_with("devtools://"));
        Ok(tabs)
    }

    async fn create_tab_at(&self, endpoint: &str, url: &str) -> Result<BrowserTab, String> {
        let encoded = url::form_urlencoded::byte_serialize(url.as_bytes()).collect::<String>();
        self.client
            .put(format!("{endpoint}/json/new?{encoded}"))
            .send()
            .await
            .map_err(|error| format!("Unable to create Browser tab: {error}"))?
            .error_for_status()
            .map_err(|error| format!("Browser tab creation failed: {error}"))?
            .json::<BrowserTab>()
            .await
            .map_err(|error| format!("Invalid Browser tab response: {error}"))
    }

    fn profile_directory(&self, _key: &str) -> PathBuf {
        self.data_root.join("profiles").join("default")
    }

    fn runtime_lock(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, HashMap<String, BrowserRuntime>>, String> {
        self.runtimes
            .lock()
            .map_err(|_| "Browser runtime registry is unavailable".to_string())
    }

    fn ensure_cdp_monitor(
        &self,
        key: String,
        endpoint: String,
        websocket_url: String,
    ) -> Result<(), String> {
        {
            let mut runtimes = self.runtime_lock()?;
            let runtime = runtimes
                .get_mut(&key)
                .ok_or_else(|| "Browser runtime was not initialized".to_string())?;
            if runtime.monitor_endpoint.as_deref() == Some(websocket_url.as_str()) {
                return Ok(());
            }
            runtime.monitor_endpoint = Some(websocket_url.clone());
        }
        let controller = self.clone();
        tokio::spawn(async move {
            controller
                .run_cdp_monitor(key, endpoint, websocket_url)
                .await;
        });
        Ok(())
    }

    async fn run_cdp_monitor(&self, key: String, endpoint: String, websocket_url: String) {
        let connection = tokio_tungstenite::connect_async(&websocket_url).await;
        let (mut socket, _) = match connection {
            Ok(connection) => connection,
            Err(error) => {
                self.finish_cdp_monitor(
                    &key,
                    &websocket_url,
                    Some(format!("Unable to connect to Chromium events: {error}")),
                );
                return;
            }
        };
        for command in [
            serde_json::json!({"id": 1, "method": "Target.setDiscoverTargets", "params": {"discover": true}}),
            serde_json::json!({"id": 2, "method": "Target.getTargets"}),
        ] {
            if socket
                .send(Message::Text(command.to_string().into()))
                .await
                .is_err()
            {
                self.finish_cdp_monitor(
                    &key,
                    &websocket_url,
                    Some("Chromium event stream closed during setup".to_string()),
                );
                return;
            }
        }

        while let Some(message) = socket.next().await {
            let text = match message {
                Ok(Message::Text(text)) => text,
                Ok(Message::Close(_)) | Err(_) => break,
                _ => continue,
            };
            let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
                continue;
            };
            let changed = if value.get("id").and_then(serde_json::Value::as_u64) == Some(2) {
                value
                    .pointer("/result/targetInfos")
                    .and_then(serde_json::Value::as_array)
                    .map(|targets| self.replace_cdp_targets(&key, targets))
                    .unwrap_or(false)
            } else {
                match value.get("method").and_then(serde_json::Value::as_str) {
                    Some("Target.targetCreated") | Some("Target.targetInfoChanged") => value
                        .pointer("/params/targetInfo")
                        .map(|target| self.upsert_cdp_target(&key, target))
                        .unwrap_or(false),
                    Some("Target.targetDestroyed") => value
                        .pointer("/params/targetId")
                        .and_then(serde_json::Value::as_str)
                        .map(|target_id| self.remove_cdp_target(&key, target_id))
                        .unwrap_or(false),
                    _ => false,
                }
            };
            if changed {
                self.emit_runtime(&key, true);
            }
        }

        // A closed DevTools WebSocket is itself the lifecycle event. Do not
        // poll /json/list; the next explicit launch/status request reconnects.
        self.finish_cdp_monitor(
            &key,
            &websocket_url,
            Some(format!(
                "Chromium event stream disconnected from {endpoint}"
            )),
        );
    }

    fn replace_cdp_targets(&self, key: &str, values: &[serde_json::Value]) -> bool {
        let tabs = values.iter().filter_map(cdp_tab).collect::<Vec<_>>();
        let Ok(mut runtimes) = self.runtimes.lock() else {
            return false;
        };
        let Some(runtime) = runtimes.get_mut(key) else {
            return false;
        };
        runtime.tabs = tabs;
        true
    }

    fn upsert_cdp_target(&self, key: &str, value: &serde_json::Value) -> bool {
        let target_id = value
            .get("targetId")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        let Ok(mut runtimes) = self.runtimes.lock() else {
            return false;
        };
        let Some(runtime) = runtimes.get_mut(key) else {
            return false;
        };
        if let Some(updated) = cdp_tab(value) {
            if let Some(existing) = runtime.tabs.iter_mut().find(|tab| tab.id == updated.id) {
                *existing = updated;
            } else {
                runtime.tabs.push(updated);
            }
            true
        } else if let Some(index) = runtime.tabs.iter().position(|tab| tab.id == target_id) {
            runtime.tabs.remove(index);
            true
        } else {
            false
        }
    }

    fn remove_cdp_target(&self, key: &str, target_id: &str) -> bool {
        let Ok(mut runtimes) = self.runtimes.lock() else {
            return false;
        };
        let Some(runtime) = runtimes.get_mut(key) else {
            return false;
        };
        let before = runtime.tabs.len();
        runtime.tabs.retain(|tab| tab.id != target_id);
        runtime.tabs.len() != before
    }

    fn finish_cdp_monitor(&self, key: &str, websocket_url: &str, error: Option<String>) {
        if let Ok(mut runtimes) = self.runtimes.lock() {
            if let Some(runtime) = runtimes.get_mut(key) {
                if runtime.monitor_endpoint.as_deref() != Some(websocket_url) {
                    return;
                }
                runtime.monitor_endpoint = None;
                runtime.endpoint = None;
                runtime.last_error = error;
                runtime.tabs.clear();
                runtime.browser_name = None;
            }
        }
        self.emit_runtime(key, false);
    }

    fn emit_runtime(&self, key: &str, running: bool) {
        let status = self.runtimes.lock().ok().and_then(|runtimes| {
            runtimes
                .get(key)
                .map(|runtime| runtime_status(runtime, running))
        });
        if let Some(status) = status {
            self.emit(status);
        }
    }

    fn emit(&self, status: BrowserWorkspaceStatus) {
        let _ = self.events.send(status);
    }
}

fn runtime_status(runtime: &BrowserRuntime, running: bool) -> BrowserWorkspaceStatus {
    BrowserWorkspaceStatus {
        profile_directory: runtime.profile_directory.to_string_lossy().into_owned(),
        running,
        browser_name: runtime.browser_name.clone(),
        executable: runtime
            .executable
            .as_ref()
            .map(|path| path.to_string_lossy().into_owned()),
        process_id: runtime.child.as_ref().map(Child::id),
        launched_at: runtime.launched_at,
        last_error: runtime.last_error.clone(),
        tabs: runtime.tabs.clone(),
    }
}

fn cdp_tab(value: &serde_json::Value) -> Option<BrowserTab> {
    let target = serde_json::from_value::<CdpTargetInfo>(value.clone()).ok()?;
    if target.kind != "page" || target.url.starts_with("devtools://") {
        return None;
    }
    Some(BrowserTab {
        id: target.target_id,
        title: target.title,
        url: target.url,
        kind: target.kind,
        description: String::new(),
        favicon_url: String::new(),
    })
}

pub fn validate_browser_url(
    input: &str,
    preferences: &BrowserPreferences,
) -> Result<Url, BrowserPolicyError> {
    let input = input.trim();
    if input.is_empty() {
        return Err(BrowserPolicyError::EmptyUrl);
    }
    if input.len() > MAX_BROWSER_URL_BYTES {
        return Err(BrowserPolicyError::UrlTooLong);
    }
    let url = Url::parse(input).map_err(|_| BrowserPolicyError::InvalidUrl)?;
    match url.scheme() {
        "https" => {}
        "http" if preferences.allow_http => {}
        "http" => return Err(BrowserPolicyError::HttpNotAllowed),
        scheme => return Err(BrowserPolicyError::SchemeNotAllowed(scheme.to_string())),
    }
    if is_localhost(&url) && !preferences.allow_localhost {
        return Err(BrowserPolicyError::LocalhostNotAllowed);
    }
    if is_private_network(&url) && !is_localhost(&url) && !preferences.allow_private_network {
        return Err(BrowserPolicyError::PrivateNetworkNotAllowed);
    }
    if url.host_str().is_none() || !url.username().is_empty() || url.password().is_some() {
        return Err(BrowserPolicyError::InvalidUrl);
    }
    Ok(url)
}

fn validate_http_origin(input: &str) -> Result<(), ()> {
    let url = Url::parse(input).map_err(|_| ())?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || url.path() != "/"
        || url.query().is_some()
        || url.fragment().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(());
    }
    Ok(())
}

fn validate_tab_id(input: &str) -> Result<(), String> {
    if input.is_empty()
        || input.len() > 256
        || !input
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err("Browser tab id is invalid".to_string());
    }
    Ok(())
}

fn endpoint_from_profile(profile: &Path) -> Result<String, String> {
    let contents = fs::read_to_string(profile.join(DEVTOOLS_ACTIVE_PORT))
        .map_err(|error| format!("DevTools endpoint is unavailable: {error}"))?;
    let port = contents
        .lines()
        .next()
        .ok_or_else(|| "DevToolsActivePort is empty".to_string())?
        .parse::<u16>()
        .map_err(|_| "DevToolsActivePort contains an invalid port".to_string())?;
    Ok(format!("http://127.0.0.1:{port}"))
}

fn browser_data_root() -> PathBuf {
    if let Some(path) =
        env::var_os("CODEX_THREAD_STUDIO_BROWSER_DATA_DIR").filter(|value| !value.is_empty())
    {
        return PathBuf::from(path);
    }
    if let Some(path) = env::var_os("XDG_DATA_HOME").filter(|value| !value.is_empty()) {
        return PathBuf::from(path).join("codex-thread-studio/browser");
    }
    if let Some(home) = env::var_os("HOME") {
        return PathBuf::from(home).join(".local/share/codex-thread-studio/browser");
    }
    env::temp_dir().join("codex-thread-studio-browser")
}

fn find_browser_executable(override_path: Option<&Path>) -> Result<PathBuf, String> {
    if let Some(path) = override_path {
        if executable_file(path) {
            return Ok(path.to_path_buf());
        }
        return Err(format!(
            "Configured Chromium executable is not usable: {}",
            path.display()
        ));
    }
    #[cfg(windows)]
    let candidates = ["chrome.exe", "msedge.exe", "brave.exe", "chromium.exe"];
    #[cfg(not(windows))]
    let candidates = [
        "chromium",
        "chromium-browser",
        "google-chrome",
        "google-chrome-stable",
        "brave-browser",
        "microsoft-edge",
        "microsoft-edge-stable",
        "vivaldi",
    ];
    if let Some(path) = env::var_os("PATH") {
        for directory in env::split_paths(&path) {
            for candidate in candidates {
                let path = directory.join(candidate);
                if executable_file(&path) {
                    return Ok(path);
                }
            }
        }
    }
    Err(
        "No Chromium-based browser was found. Install Chromium, Chrome, Brave, or Edge; or set CODEX_THREAD_STUDIO_BROWSER."
            .to_string(),
    )
}

fn executable_file(path: &Path) -> bool {
    let Ok(metadata) = fs::metadata(path) else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

fn unix_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

fn is_localhost(url: &Url) -> bool {
    match url.host() {
        Some(Host::Domain(host)) => host.eq_ignore_ascii_case("localhost"),
        Some(Host::Ipv4(address)) => address.is_loopback(),
        Some(Host::Ipv6(address)) => address.is_loopback(),
        None => false,
    }
}

fn is_private_network(url: &Url) -> bool {
    match url.host() {
        Some(Host::Domain(host)) => host.ends_with(".local") || host.ends_with(".localhost"),
        Some(Host::Ipv4(address)) => {
            address.is_private()
                || address.is_link_local()
                || address.is_loopback()
                || address.is_unspecified()
        }
        Some(Host::Ipv6(address)) => {
            let first = address.segments()[0];
            address.is_loopback()
                || address.is_unspecified()
                || first & 0xfe00 == 0xfc00
                || first & 0xffc0 == 0xfe80
        }
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feature_flag_is_disabled_by_default_but_explicit_launch_is_validated() {
        let preferences = BrowserPreferences::default();
        let controller = BrowserController::new(preferences);
        assert!(!controller.is_enabled());
        assert_eq!(
            controller.prepare_navigation("https://example.com"),
            Err(BrowserPolicyError::Disabled)
        );
        assert!(controller
            .prepare_explicit_navigation("https://example.com")
            .is_ok());
    }

    #[test]
    fn allows_http_and_https_with_explicitly_enabled_defaults() {
        let preferences = BrowserPreferences {
            enabled: true,
            ..BrowserPreferences::default()
        };
        let controller = BrowserController::new(preferences);
        assert!(controller
            .prepare_navigation("https://example.com/docs")
            .is_ok());
        assert!(controller
            .prepare_navigation("http://localhost:3000/")
            .is_ok());
    }

    #[test]
    fn rejects_active_local_and_credentialed_urls() {
        let preferences = BrowserPreferences {
            enabled: true,
            ..BrowserPreferences::default()
        };
        for (input, scheme) in [
            ("file:///tmp/index.html", "file"),
            ("javascript:alert(1)", "javascript"),
            ("data:text/html,hello", "data"),
            ("custom://example/path", "custom"),
        ] {
            assert_eq!(
                validate_browser_url(input, &preferences),
                Err(BrowserPolicyError::SchemeNotAllowed(scheme.to_string()))
            );
        }
        assert_eq!(
            validate_browser_url("https://user:password@example.com", &preferences),
            Err(BrowserPolicyError::InvalidUrl)
        );
    }

    #[test]
    fn separates_localhost_from_other_private_networks() {
        let mut preferences = BrowserPreferences {
            enabled: true,
            ..BrowserPreferences::default()
        };
        assert!(validate_browser_url("http://127.0.0.2:3000", &preferences).is_ok());
        assert_eq!(
            validate_browser_url("http://192.168.1.20", &preferences),
            Err(BrowserPolicyError::PrivateNetworkNotAllowed)
        );
        preferences.allow_private_network = true;
        assert!(validate_browser_url("http://192.168.1.20", &preferences).is_ok());
    }

    #[test]
    fn validates_future_agent_origins_as_origins_not_urls() {
        let mut preferences = BrowserPreferences::default();
        preferences.agent.allowed_origins = vec!["https://example.com/path".to_string()];
        assert!(preferences.validate().is_err());
        preferences.agent.allowed_origins = vec!["https://example.com".to_string()];
        assert!(preferences.validate().is_ok());
    }

    #[test]
    fn every_session_uses_one_global_browser_profile() {
        let controller = BrowserController::with_runtime(
            BrowserPreferences::default(),
            PathBuf::from("/tmp/studio-browser-test"),
            None,
        );
        assert_eq!(
            controller.profile_directory("first-project"),
            controller.profile_directory("second-project")
        );
        assert!(controller
            .profile_directory("anything")
            .ends_with("profiles/default"));
    }

    #[test]
    fn reads_only_loopback_endpoint_from_devtools_file() {
        let root = env::temp_dir().join(format!(
            "codex-thread-browser-test-{}-{}",
            std::process::id(),
            unix_millis()
        ));
        fs::create_dir_all(&root).expect("test profile");
        fs::write(
            root.join(DEVTOOLS_ACTIVE_PORT),
            "43123\n/devtools/browser/test\n",
        )
        .expect("DevToolsActivePort");
        assert_eq!(
            endpoint_from_profile(&root).expect("endpoint"),
            "http://127.0.0.1:43123"
        );
        fs::remove_dir_all(root).expect("remove test profile");
    }

    #[test]
    fn parses_chromium_version_and_browser_websocket_fields() {
        let version: BrowserVersion = serde_json::from_value(serde_json::json!({
            "Browser": "Chrome/140.0",
            "webSocketDebuggerUrl": "ws://127.0.0.1:43123/devtools/browser/test"
        }))
        .expect("version payload");
        assert_eq!(version.browser, "Chrome/140.0");
        assert_eq!(
            version.web_socket_debugger_url,
            "ws://127.0.0.1:43123/devtools/browser/test"
        );
    }

    #[test]
    fn maps_only_page_targets_from_cdp_events() {
        let page = cdp_tab(&serde_json::json!({
            "targetId": "page-1",
            "type": "page",
            "title": "Example",
            "url": "https://example.com/"
        }))
        .expect("page target");
        assert_eq!(page.id, "page-1");
        assert_eq!(page.title, "Example");
        assert!(cdp_tab(&serde_json::json!({
            "targetId": "worker-1",
            "type": "service_worker",
            "url": "https://example.com/sw.js"
        }))
        .is_none());
    }
}
