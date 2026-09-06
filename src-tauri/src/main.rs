use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::ffi::OsString;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::net::{SocketAddr, TcpListener};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use axum::body::Body;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path as AxumPath, Query, Request, State};
use axum::http::{header, HeaderMap, Method, Response, StatusCode, Uri};
use axum::middleware::{self, Next};
use axum::response::{Html, IntoResponse};
use axum::routing::{any, get};
use axum::{Json, Router};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tauri::{WebviewUrl, WebviewWindowBuilder};

mod backend_config;
mod backend_runtime;
mod browser_runtime;
mod codex_app_server;
#[cfg(all(target_os = "linux", debug_assertions))]
mod dev_capture;
#[cfg(target_os = "linux")]
mod embedded_browser;
#[cfg(windows)]
mod embedded_browser_windows;
mod environment_config;
mod epub_reader;
mod favorites;
mod gateway_security;
mod git_review;
mod ollama;
mod opencode_server;
mod session_map;
mod session_state;
mod speech;
mod terminal_runtime;

use backend_config::{BackendDescriptor, ConfiguredCodexBackend};
use backend_runtime::{BackendRuntime, WslSettings};
use browser_runtime::BrowserPreferences;
#[cfg(not(windows))]
use codex_app_server::find_codex_binary;
use codex_app_server::CodexAppServer;
use favorites::{Favorite, MAX_FAVORITE_BODY_BYTES};
use gateway_security::{AuthorizationError, GatewaySecurity};
use git_review::{
    GitDiffRequest, GitDiffResponse, GitPathsRequest, GitRootRequest, GitStatusResponse,
};
#[cfg(not(windows))]
use opencode_server::find_opencode_binary;
use opencode_server::OpenCodeServer;
use session_map::{ApplyOperationsRequest, CreateMapRequest, MAX_MAP_BODY_BYTES};

const MAX_PREFERENCES_BODY: usize = 1024 * 1024;
const MAX_EDIT_FILE_BYTES: usize = 5 * 1024 * 1024;
const MAX_REVIEW_FILE_BYTES: u64 = MAX_EDIT_FILE_BYTES as u64;
const MAX_REVIEW_IMAGE_BYTES: u64 = 25 * 1024 * 1024;
const MAX_REVIEW_DOCUMENT_BYTES: u64 = 50 * 1024 * 1024;
const DEFAULT_REMOTE_STUDIO_ADDR: &str = "127.0.0.1:38080";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ServeOptions {
    listen: SocketAddr,
}

#[derive(Clone)]
struct GatewayState {
    codex: CodexAppServer,
    codex_backends: Arc<BTreeMap<String, CodexBackendInstance>>,
    backend_descriptors: Arc<Vec<BackendDescriptor>>,
    backend_config_path: Arc<PathBuf>,
    backend_config_error: Arc<Option<String>>,
    opencode: OpenCodeServer,
    preferences_path: Arc<PathBuf>,
    preferences_lock: Arc<Mutex<()>>,
    shared_document_directories: Arc<Mutex<Vec<String>>>,
    studio_path: Arc<PathBuf>,
    studio_lock: Arc<Mutex<()>>,
    session_maps_path: Arc<PathBuf>,
    session_maps_lock: Arc<Mutex<()>>,
    epub_reading_path: Arc<PathBuf>,
    epub_reading_lock: Arc<Mutex<()>>,
    environment_path: Arc<PathBuf>,
    environment_lock: Arc<Mutex<()>>,
    security: GatewaySecurity,
    embedded_browser: bool,
}

#[derive(Clone)]
struct CodexBackendInstance {
    command_line: String,
    server: CodexAppServer,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BackendRegistryInfo {
    backends: Vec<BackendDescriptor>,
    config_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    configuration_error: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct TypographyPreferences {
    ui_font_family: String,
    #[serde(default = "default_ui_font_size")]
    ui_font_size: f64,
    ui_font_weight: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    content_font_family: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    content_font_size: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    content_font_weight: Option<u16>,
    #[serde(default = "default_workspace_font_family")]
    workspace_font_family: String,
    #[serde(default = "default_workspace_font_size")]
    workspace_font_size: f64,
    code_font_family: String,
    code_font_size: f64,
    code_font_weight: u16,
    high_contrast: bool,
}

fn default_ui_font_size() -> f64 {
    14.0
}

fn default_workspace_font_family() -> String {
    "\"Noto Sans CJK SC\", \"Noto Sans SC\", \"Microsoft YaHei\", system-ui, sans-serif".to_string()
}

fn default_workspace_font_size() -> f64 {
    14.0
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct AnnotationDraft {
    id: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    excerpt: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    note: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    source: Option<AnnotationSourceReference>,
    // Legacy fields are preserved while old settings are migrated by the UI.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    quote: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    comment: String,
    created_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    item_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    turn_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    target: Option<AnnotationTarget>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct AnnotationSourceReference {
    provider: String,
    #[serde(default = "default_annotation_source_version")]
    version: u32,
    #[serde(default)]
    anchor: serde_json::Value,
}

fn default_annotation_source_version() -> u32 {
    1
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct AnnotationTarget {
    kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    file_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    root: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    base_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    start_offset: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    end_offset: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    start_line: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    end_line: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    prefix: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    suffix: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct OpeningMessage {
    #[serde(default)]
    text: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    responsibility: String,
    #[serde(default)]
    source: String,
    #[serde(default)]
    captured_at: String,
    #[serde(default)]
    truncated: bool,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct RouterResponsibility {
    #[serde(default)]
    description: String,
    #[serde(default)]
    fallback: String,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct RouterFallbackPreference {
    session_key: String,
    condition: String,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct ThreadRouterPreferences {
    // Legacy Codex-only controller identity. Kept for reading pre-registry settings.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    controller_backend: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    controllers: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    responsibilities: BTreeMap<String, RouterResponsibility>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    fallbacks: Vec<RouterFallbackPreference>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct MermaidPreferences {
    style: String,
    density: String,
    curve: String,
    layout: String,
    font_size: u8,
}

impl Default for MermaidPreferences {
    fn default() -> Self {
        Self {
            style: "auto".to_string(),
            density: "standard".to_string(),
            curve: "rounded".to_string(),
            layout: "auto".to_string(),
            font_size: 15,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct MarkdownPreferences {
    mode: String,
}

impl Default for MarkdownPreferences {
    fn default() -> Self {
        Self {
            mode: "technical".to_string(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct TranslationPreferences {
    #[serde(default = "default_translation_engine")]
    engine: String,
    #[serde(default = "default_ollama_model")]
    ollama_model: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    models: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    efforts: BTreeMap<String, String>,
}

impl Default for TranslationPreferences {
    fn default() -> Self {
        Self {
            engine: default_translation_engine(),
            ollama_model: default_ollama_model(),
            models: BTreeMap::new(),
            efforts: BTreeMap::new(),
        }
    }
}

fn default_translation_engine() -> String {
    "backend".to_string()
}

fn default_ollama_model() -> String {
    "gemma3:4b".to_string()
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct StudioPreferences {
    #[serde(default)]
    desktop_notifications: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    queue_depth: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    continue_behavior: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    language: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    theme: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    content_width: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    hidden_session_directories: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    shared_document_directories: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    session_directory_ignore: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    wsl_distribution: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    wsl_user: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    wsl_codex_binary: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    wsl_opencode_binary: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    typography: Option<TypographyPreferences>,
    #[serde(default)]
    mermaid: MermaidPreferences,
    #[serde(default)]
    markdown: MarkdownPreferences,
    #[serde(default)]
    translation: TranslationPreferences,
    #[serde(default)]
    browser: BrowserPreferences,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    thread_activity: BTreeMap<String, u64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    attention_threads: Vec<String>,
    // These fields are transient validation containers for the dedicated
    // session-state API and are never read from or written to settings.json.
    #[serde(skip)]
    annotation_drafts: BTreeMap<String, Vec<AnnotationDraft>>,
    #[serde(skip)]
    annotation_additional: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    annotation_prompt_templates: BTreeMap<String, String>,
    #[serde(skip)]
    opening_messages: BTreeMap<String, OpeningMessage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    router: Option<ThreadRouterPreferences>,
    #[serde(default)]
    sidebar_collapsed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    artifact_width_ratio: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    right_rail_width_ratio: Option<f64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BackendInfo {
    app_name: String,
    app_version: &'static str,
    binary: String,
    protocol: &'static str,
    transport: &'static str,
    router_workspace: String,
    execution_environment: &'static str,
    wsl_distribution: Option<String>,
    host_platform: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BrowserInfo {
    enabled: bool,
    available: bool,
    phase: u8,
    presentation: &'static str,
}

#[derive(Default, Deserialize)]
struct FavoriteQuery {
    #[serde(default)]
    q: String,
    limit: Option<usize>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AnnotationStateRequest {
    session_key: String,
    #[serde(default)]
    drafts: Vec<AnnotationDraft>,
    #[serde(default)]
    additional: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OpeningMessageStateRequest {
    session_key: String,
    message: Option<OpeningMessage>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PinSessionStateRequest {
    session_key: String,
    pinned: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TurnOptionsStateRequest {
    session_key: String,
    #[serde(default)]
    model: String,
    #[serde(default)]
    effort: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MessageQueueStateRequest {
    session_key: String,
    #[serde(default)]
    messages: Vec<session_state::QueuedMessage>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeleteSessionStateRequest {
    session_key: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReviewFileRequest {
    root: String,
    path: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReviewFileRequestWithHash {
    root: String,
    path: String,
    book_hash: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ReviewFileResponse {
    root: String,
    document_root: String,
    path: String,
    relative_path: String,
    content: String,
    hash: String,
    size: u64,
    line_count: usize,
    language: String,
    read_only: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceListRequest {
    root: String,
    #[serde(default)]
    path: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceEntry {
    name: String,
    path: String,
    kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    size: Option<u64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceListResponse {
    root: String,
    path: String,
    entries: Vec<WorkspaceEntry>,
    truncated: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceSaveRequest {
    root: String,
    path: String,
    content: String,
    expected_hash: String,
    #[serde(default)]
    overwrite: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceConflictResponse {
    error: WorkspaceConflictError,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceConflictError {
    message: &'static str,
    code: &'static str,
    actual_hash: String,
}

fn main() {
    let arguments = env::args_os().skip(1).collect::<Vec<_>>();
    match parse_serve_options(&arguments) {
        Ok(Some(options)) => {
            if let Err(error) = run_remote_server(options) {
                eprintln!("Codex Thread Studio remote server failed: {error}");
                std::process::exit(2);
            }
            return;
        }
        Ok(None) => {}
        Err(error) => {
            eprintln!("Codex Thread Studio remote server configuration is invalid: {error}");
            std::process::exit(2);
        }
    }
    #[cfg(all(target_os = "linux", debug_assertions))]
    if dev_capture::requested(&arguments) {
        match dev_capture::run_cli(&arguments) {
            Ok(true) => return,
            Ok(false) => {}
            Err(error) => {
                eprintln!("Developer command failed: {error}");
                std::process::exit(2);
            }
        }
    }
    #[cfg(not(all(target_os = "linux", debug_assertions)))]
    if arguments.first().is_some_and(|argument| {
        matches!(
            argument.to_str(),
            Some(
                "--dev-screenshot"
                    | "--dev-show-browser"
                    | "--dev-hide-browser"
                    | "--dev-exit-browser"
                    | "--dev-new-browser-tab"
                    | "--dev-show-browser-menu"
                    | "--dev-show-browser-info"
                    | "--dev-show-browser-downloads"
                    | "--dev-crash-browser-tab"
                    | "--dev-open-browser"
                    | "--dev-open-artifact"
                    | "--dev-open-workspace"
                    | "--dev-open-resources"
                    | "--dev-open-environment-settings"
                    | "--dev-click"
                    | "--dev-input"
            )
        )
    }) {
        eprintln!("Developer controls are available only in Linux debug builds");
        std::process::exit(2);
    }

    let gateway_listener = TcpListener::bind("127.0.0.1:0")
        .expect("failed to reserve a local Codex Thread Studio gateway port");
    gateway_listener
        .set_nonblocking(true)
        .expect("failed to configure Codex Thread Studio gateway socket");
    let gateway_addr = gateway_listener
        .local_addr()
        .expect("failed to determine Codex Thread Studio gateway address");
    let gateway_origin = format!("http://{gateway_addr}");
    let security = GatewaySecurity::new(gateway_origin);
    let initialization_script = security.initialization_script();
    #[cfg(target_os = "linux")]
    let embedded_browser = embedded_browser::is_supported();
    #[cfg(windows)]
    let embedded_browser = embedded_browser_windows::is_supported();
    #[cfg(not(any(target_os = "linux", windows)))]
    let embedded_browser = false;
    let (state, startup_preferences, _profile_lock) =
        initialize_gateway(security, embedded_browser).unwrap_or_else(|error| {
            eprintln!("Codex Thread Studio could not open its profile: {error}");
            std::process::exit(2);
        });
    let embedded_browser_preferences = startup_preferences.browser.clone();

    tauri::Builder::default()
        .setup(move |app| {
            let router = gateway_router(state.clone());
            tauri::async_runtime::spawn(async move {
                match tokio::net::TcpListener::from_std(gateway_listener) {
                    Ok(listener) => {
                        if let Err(error) = axum::serve(listener, router).await {
                            eprintln!("Codex Thread Studio gateway stopped: {error}");
                        }
                    }
                    Err(error) => eprintln!("Codex Thread Studio gateway failed: {error}"),
                }
            });

            let url = format!("http://{gateway_addr}/").parse()?;
            #[cfg(any(target_os = "linux", windows))]
            if embedded_browser {
                #[cfg(target_os = "linux")]
                embedded_browser::build(
                    app,
                    url,
                    &initialization_script,
                    embedded_browser_preferences.clone(),
                )?;
                #[cfg(windows)]
                embedded_browser_windows::build(
                    app,
                    url,
                    &initialization_script,
                    embedded_browser_preferences.clone(),
                )?;
            } else {
                let _window = WebviewWindowBuilder::new(app, "main", WebviewUrl::External(url))
                    .initialization_script(&initialization_script)
                    .title("Codex Thread Studio")
                    .inner_size(1400.0, 900.0)
                    .min_inner_size(980.0, 660.0)
                    .build()?;
                #[cfg(all(target_os = "linux", debug_assertions))]
                dev_capture::register_tauri_window(&_window)?;
            }
            #[cfg(all(target_os = "linux", debug_assertions))]
            if let Err(error) = dev_capture::start_server() {
                eprintln!("Codex Thread Studio developer capture is unavailable: {error}");
            }
            #[cfg(not(any(target_os = "linux", windows)))]
            WebviewWindowBuilder::new(app, "main", WebviewUrl::External(url))
                .initialization_script(&initialization_script)
                .title("Codex Thread Studio")
                .inner_size(1400.0, 900.0)
                .min_inner_size(980.0, 660.0)
                .build()?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("failed to run Codex Thread Studio desktop application");
}

fn parse_serve_options(arguments: &[OsString]) -> Result<Option<ServeOptions>, String> {
    if !arguments.iter().any(|argument| argument == "--serve") {
        return Ok(None);
    }

    let mut listen = DEFAULT_REMOTE_STUDIO_ADDR
        .parse::<SocketAddr>()
        .expect("the default remote Studio address is valid");
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].to_str() {
            Some("--serve") => index += 1,
            Some("--listen") => {
                let value = arguments
                    .get(index + 1)
                    .and_then(|argument| argument.to_str())
                    .ok_or_else(|| "--listen requires an IP address and port".to_string())?;
                listen = value.parse::<SocketAddr>().map_err(|_| {
                    format!("--listen must be an IP address and port, got {value:?}")
                })?;
                index += 2;
            }
            Some(argument) => {
                return Err(format!("unsupported option in server mode: {argument}"));
            }
            None => return Err("server options must be valid UTF-8".to_string()),
        }
    }

    if !listen.ip().is_loopback() {
        return Err(
            "--listen must use a loopback address; connect through an SSH tunnel".to_string(),
        );
    }
    Ok(Some(ServeOptions { listen }))
}

fn run_remote_server(options: ServeOptions) -> Result<(), String> {
    let listener = TcpListener::bind(options.listen)
        .map_err(|error| format!("unable to bind {}: {error}", options.listen))?;
    listener
        .set_nonblocking(true)
        .map_err(|error| format!("unable to configure the server socket: {error}"))?;
    let address = listener
        .local_addr()
        .map_err(|error| format!("unable to read the server address: {error}"))?;
    let origin = format!("http://{address}");
    let security = GatewaySecurity::new(origin.clone());
    let access_url = format!("{origin}/#token={}", security.token());
    let (state, _, _profile_lock) = initialize_gateway(security, false)?;
    let router = gateway_router(state);

    println!("Codex Thread Studio remote server is listening on {origin}");
    println!("Forward the same port over SSH, then open: {access_url}");
    println!("The embedded browser is unavailable in remote browser mode; workspace files, Git, and terminals run on this host.");

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("unable to start the asynchronous runtime: {error}"))?;
    runtime.block_on(async move {
        let listener = tokio::net::TcpListener::from_std(listener)
            .map_err(|error| format!("unable to start the server listener: {error}"))?;
        axum::serve(listener, router)
            .await
            .map_err(|error| format!("server stopped: {error}"))
    })
}

fn initialize_gateway(
    security: GatewaySecurity,
    embedded_browser: bool,
) -> Result<(GatewayState, StudioPreferences, fs::File), String> {
    let cli_path = augmented_cli_path();
    let preferences_path = studio_preferences_path();
    let profile_lock = lock_studio_profile(&preferences_path)?;
    let studio_path = preferences_path.with_file_name("studio.sqlite3");
    let session_maps_path = preferences_path.with_file_name("session-maps.sqlite3");
    let epub_reading_path = preferences_path.with_file_name("epub-reading.sqlite3");
    let environment_path = preferences_path.with_file_name("environments.json");
    let backend_config_path = backend_config::configuration_path(&preferences_path);
    let startup_preferences = match load_preferences(&preferences_path) {
        Ok(preferences) => {
            if let Err(error) = save_preferences(&preferences_path, &preferences) {
                eprintln!("Codex Thread Studio could not write default settings: {error}");
            }
            preferences
        }
        Err(error) => {
            eprintln!("Codex Thread Studio could not load startup settings: {error}");
            StudioPreferences::default()
        }
    };
    let (codex_binary, opencode_binary, runtime) =
        backend_configuration(&startup_preferences, cli_path);
    let configured_backends = backend_config::load(&backend_config_path);
    if let Some(error) = &configured_backends.error {
        eprintln!("Codex Thread Studio could not load local backends: {error}");
    }
    let codex = CodexAppServer::new(codex_binary.clone(), runtime.clone());
    let (codex_backends, backend_descriptors) = build_backend_registry(
        &codex_binary,
        codex.clone(),
        configured_backends.backends,
        runtime.clone(),
    );
    if let Err(error) = favorites::initialize(&studio_path) {
        eprintln!("Codex Thread Studio could not initialize favorites: {error}");
    }
    if let Err(error) = session_state::initialize(&studio_path) {
        eprintln!("Codex Thread Studio could not initialize session state: {error}");
    }
    if let Err(error) = session_map::initialize(&session_maps_path) {
        eprintln!("Codex Thread Studio could not initialize session maps: {error}");
    }
    if let Err(error) = epub_reader::initialize(&epub_reading_path) {
        eprintln!("Codex Thread Studio could not initialize EPUB reading state: {error}");
    }
    let state = GatewayState {
        codex,
        codex_backends: Arc::new(codex_backends),
        backend_descriptors: Arc::new(backend_descriptors),
        backend_config_path: Arc::new(backend_config_path),
        backend_config_error: Arc::new(configured_backends.error),
        opencode: OpenCodeServer::new(opencode_binary, runtime),
        preferences_path: Arc::new(preferences_path),
        preferences_lock: Arc::new(Mutex::new(())),
        shared_document_directories: Arc::new(Mutex::new(
            startup_preferences.shared_document_directories.clone(),
        )),
        studio_path: Arc::new(studio_path),
        studio_lock: Arc::new(Mutex::new(())),
        session_maps_path: Arc::new(session_maps_path),
        session_maps_lock: Arc::new(Mutex::new(())),
        epub_reading_path: Arc::new(epub_reading_path),
        epub_reading_lock: Arc::new(Mutex::new(())),
        environment_path: Arc::new(environment_path),
        environment_lock: Arc::new(Mutex::new(())),
        security,
        embedded_browser,
    };
    Ok((state, startup_preferences, profile_lock))
}

// Keep the returned handle alive for the entire desktop/server lifetime. Never
// unlink this file: doing so would let another process lock a different inode.
fn lock_studio_profile(preferences_path: &std::path::Path) -> Result<fs::File, String> {
    let parent = preferences_path
        .parent()
        .ok_or("profile path has no parent")?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let path = parent.join("studio-instance.lock");
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)
        .map_err(|error| format!("cannot open {}: {error}", path.display()))?;
    file.try_lock().map_err(|error| format!(
        "profile {} is already in use or cannot be locked ({error}); close the other desktop/server instance or use a separate XDG_CONFIG_HOME",
        parent.display(),
    ))?;
    Ok(file)
}

fn gateway_router(state: GatewayState) -> Router {
    let protected = Router::new()
        .route("/studio/codex", get(codex_info))
        .route("/studio/backends", get(backend_registry_info))
        .route("/studio/backend/{backend}", get(codex_instance_info))
        .route("/studio/opencode", get(opencode_info))
        .route("/studio/browser", get(browser_info))
        .route(
            "/studio/preferences",
            get(get_preferences).put(put_preferences),
        )
        .route("/studio/session-state", get(get_session_state))
        .route(
            "/studio/session-state/annotations",
            axum::routing::put(put_annotation_state),
        )
        .route(
            "/studio/session-state/opening-message",
            axum::routing::put(put_opening_message_state),
        )
        .route(
            "/studio/session-state/pin",
            axum::routing::put(put_session_pin),
        )
        .route(
            "/studio/session-state/turn-options",
            axum::routing::put(put_turn_options_state),
        )
        .route(
            "/studio/session-state/message-queue",
            axum::routing::put(put_message_queue_state),
        )
        .route(
            "/studio/session-state/session",
            axum::routing::delete(delete_session_state),
        )
        .route(
            "/studio/environment",
            get(get_environment_profile).put(put_environment_profile),
        )
        .route(
            "/studio/environment/apply",
            axum::routing::post(apply_environment_profile),
        )
        .route("/studio/client-log", axum::routing::post(client_log))
        .route("/studio/speech", get(speech::status).post(speech::speak))
        .route("/studio/speech/stop", axum::routing::post(speech::stop))
        .route("/studio/ollama/models", get(ollama::models))
        .route(
            "/studio/ollama/translate",
            axum::routing::post(ollama::translate),
        )
        .route(
            "/studio/ollama/continue-draft",
            axum::routing::post(ollama::continue_draft),
        )
        .route(
            "/studio/workspace/list",
            axum::routing::post(list_workspace_directory),
        )
        .route(
            "/studio/workspace/save",
            axum::routing::post(save_workspace_file),
        )
        .route("/studio/git/status", axum::routing::post(git_status))
        .route("/studio/git/diff", axum::routing::post(git_diff))
        .route("/studio/git/stage", axum::routing::post(git_stage))
        .route("/studio/git/unstage", axum::routing::post(git_unstage))
        .route("/studio/review-file", axum::routing::post(read_review_file))
        .route(
            "/studio/review-image",
            axum::routing::post(read_review_image),
        )
        .route("/studio/review-epub", axum::routing::post(read_review_epub))
        .route("/studio/review-pdf", axum::routing::post(read_review_pdf))
        .route(
            "/studio/review-spreadsheet",
            axum::routing::post(read_review_spreadsheet),
        )
        .route(
            "/studio/epub/state",
            axum::routing::post(get_epub_reading_state).put(put_epub_reading_state),
        )
        .route(
            "/studio/favorites",
            get(list_favorites).post(create_favorite),
        )
        .route("/studio/favorites/export", get(export_favorites))
        .route(
            "/studio/favorites/{id}",
            get(get_favorite)
                .put(update_favorite)
                .delete(delete_favorite),
        )
        .route(
            "/studio/session-map",
            axum::routing::post(create_session_map),
        )
        .route(
            "/studio/session-map/{backend}/{thread_id}",
            get(get_session_map).delete(delete_session_map),
        )
        .route(
            "/studio/session-map/{backend}/{thread_id}/operations",
            axum::routing::post(apply_session_map_operations),
        )
        .route(
            "/studio/session-map/{backend}/{thread_id}/undo",
            axum::routing::post(undo_session_map),
        )
        .route("/ws/codex", get(codex_app_server_ws))
        .route("/ws/codex-lifecycle", get(codex_lifecycle_ws))
        .route("/ws/codex/{backend}", get(codex_instance_ws))
        .route("/ws/terminal", get(terminal_ws))
        .route("/opencode/{*path}", any(proxy_opencode))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            require_gateway_auth,
        ));

    Router::new()
        .route("/", get(index))
        .route("/bootstrap-errors.js", get(bootstrap_errors_js))
        .route("/remote-bootstrap.js", get(remote_bootstrap_js))
        .route("/app.js", get(app_js))
        .route("/i18n.mjs", get(i18n_js))
        .route("/codex-native.mjs", get(codex_native_js))
        .route(
            "/codex-lifecycle-diagnostics.mjs",
            get(codex_lifecycle_diagnostics_js),
        )
        .route("/opencode-native.mjs", get(opencode_native_js))
        .route(
            "/opencode-event-reducer.mjs",
            get(opencode_event_reducer_js),
        )
        .route("/thread-history-tail.mjs", get(thread_history_tail_js))
        .route("/codex-history-loader.mjs", get(codex_history_loader_js))
        .route(
            "/history-load-coordinator.mjs",
            get(history_load_coordinator_js),
        )
        .route("/model-revision.mjs", get(model_revision_js))
        .route("/performance-monitor.mjs", get(performance_monitor_js))
        .route("/selection-translation.mjs", get(selection_translation_js))
        .route("/continuation-draft.mjs", get(continuation_draft_js))
        .route("/backends.mjs", get(backends_js))
        .route("/model-display.mjs", get(model_display_js))
        .route("/message-queue.mjs", get(message_queue_js))
        .route("/session-catalog.mjs", get(session_catalog_js))
        .route(
            "/serialized-state-writer.mjs",
            get(serialized_state_writer_js),
        )
        .route("/session-model-cache.mjs", get(session_model_cache_js))
        .route(
            "/session-model-preferences.mjs",
            get(session_model_preferences_js),
        )
        .route("/session-management.mjs", get(session_management_js))
        .route("/thread-catalog.mjs", get(thread_catalog_js))
        .route("/thread-fork.mjs", get(thread_fork_js))
        .route(
            "/thread-router-controller.mjs",
            get(thread_router_controller_js),
        )
        .route("/thread-workset.mjs", get(thread_workset_js))
        .route("/session-search.mjs", get(session_search_js))
        .route("/composer-tools.mjs", get(composer_tools_js))
        .route("/composer-images.mjs", get(composer_images_js))
        .route("/document-review.mjs", get(document_review_js))
        .route("/document-outline.mjs", get(document_outline_js))
        .route("/environment-profile.mjs", get(environment_profile_js))
        .route("/epub-reader.mjs", get(epub_reader_js))
        .route("/epub-comment-provider.mjs", get(epub_comment_provider_js))
        .route("/pdf-reader.mjs", get(pdf_reader_js))
        .route("/pdf-comment-provider.mjs", get(pdf_comment_provider_js))
        .route("/table-reader.mjs", get(table_reader_js))
        .route(
            "/table-comment-provider.mjs",
            get(table_comment_provider_js),
        )
        .route("/workspace-tools.mjs", get(workspace_tools_js))
        .route("/session-resources.mjs", get(session_resources_js))
        .route("/session-resources-ui.mjs", get(session_resources_ui_js))
        .route("/git-review.mjs", get(git_review_js))
        .route("/right-rail-layout.mjs", get(right_rail_layout_js))
        .route("/workspace-editor.mjs", get(workspace_editor_js))
        .route("/comment-core.mjs", get(comment_core_js))
        .route("/comment-markers.mjs", get(comment_markers_js))
        .route(
            "/browser-comment-provider.mjs",
            get(browser_comment_provider_js),
        )
        .route(
            "/comment-source-providers.mjs",
            get(comment_source_providers_js),
        )
        .route("/favorites.mjs", get(favorites_js))
        .route("/session-map.mjs", get(session_map_js))
        .route(
            "/session-map-controller.mjs",
            get(session_map_controller_js),
        )
        .route(
            "/review-notes-controller.mjs",
            get(review_notes_controller_js),
        )
        .route(
            "/document-workspace-controller.mjs",
            get(document_workspace_controller_js),
        )
        .route("/mermaid-config.mjs", get(mermaid_config_js))
        .route("/thread-router.mjs", get(thread_router_js))
        .route("/session-dispatch.mjs", get(session_dispatch_js))
        .route("/turn-navigator.mjs", get(turn_navigator_js))
        .route("/transcript-scroll.mjs", get(transcript_scroll_js))
        .route("/transcript-dom.mjs", get(transcript_dom_js))
        .route(
            "/transcript-presentation.mjs",
            get(transcript_presentation_js),
        )
        .route("/vendor/marked.esm.js", get(marked_js))
        .route("/vendor/purify.es.mjs", get(dompurify_js))
        .route("/vendor/mermaid.min.js", get(mermaid_js))
        .route("/vendor/epub.mjs", get(epub_vendor_js))
        .route("/vendor/pdf.min.mjs", get(pdf_vendor_js))
        .route("/vendor/pdf.worker.min.mjs", get(pdf_worker_vendor_js))
        .route("/vendor/artifact-table.mjs", get(table_vendor_js))
        .route(
            "/vendor/workspace-editor.mjs",
            get(workspace_editor_vendor_js),
        )
        .route(
            "/vendor/workspace-terminal.mjs",
            get(workspace_terminal_vendor_js),
        )
        .route("/vendor/xterm.css", get(xterm_css))
        .route("/vendor/github-markdown.css", get(github_markdown_css))
        .route("/styles.css", get(styles_css))
        .merge(protected)
        .with_state(state)
}

async fn require_gateway_auth(
    State(state): State<GatewayState>,
    request: Request,
    next: Next,
) -> Response<Body> {
    let websocket = request.uri().path().starts_with("/ws/");
    let authorization = if websocket {
        state.security.authorize_websocket(request.headers())
    } else {
        state.security.authorize_http(request.headers())
    };
    match authorization {
        Ok(()) => next.run(request).await,
        Err(error) => {
            if websocket {
                let reason = match error {
                    AuthorizationError::InvalidOrigin => "origin rejected",
                    AuthorizationError::InvalidHost => "host rejected",
                    AuthorizationError::MissingCredential => "credential missing",
                    AuthorizationError::InvalidCredential => "credential rejected",
                };
                eprintln!("Codex Thread Studio rejected App Server WebSocket: {reason}");
            }
            let status = match error {
                AuthorizationError::InvalidOrigin | AuthorizationError::InvalidHost => {
                    StatusCode::FORBIDDEN
                }
                AuthorizationError::MissingCredential | AuthorizationError::InvalidCredential => {
                    StatusCode::UNAUTHORIZED
                }
            };
            json_error(status, "Studio gateway authorization failed")
        }
    }
}

async fn index() -> impl IntoResponse {
    (
        [
            (header::CACHE_CONTROL, "no-store"),
            (
                header::CONTENT_SECURITY_POLICY,
                "default-src 'self' blob: data:; script-src 'self'; style-src 'self' 'unsafe-inline' blob:; img-src 'self' data: blob:; font-src 'self' data: blob:; media-src 'self' data: blob:; connect-src 'self' ws:; frame-src blob:; object-src 'none'; base-uri 'none'; form-action 'none'",
            ),
        ],
        Html(include_str!("../../ui/index.html")),
    )
}

async fn remote_bootstrap_js() -> impl IntoResponse {
    let host_platform =
        serde_json::to_string(std::env::consts::OS).expect("host platform is JSON text");
    javascript_owned(
        include_str!("../../ui/remote-bootstrap.js")
            .replace("__STUDIO_HOST_PLATFORM__", &host_platform),
    )
}

async fn read_review_file(
    State(state): State<GatewayState>,
    Json(request): Json<ReviewFileRequest>,
) -> Response<Body> {
    #[cfg(windows)]
    if state.codex.execution_environment() == "wsl" {
        let shared_directories = match configured_shared_document_directories(&state) {
            Ok(value) => value,
            Err((status, message)) => return json_error(status, &message),
        };
        return match state
            .codex
            .read_wsl_file(
                &request.root,
                &request.path,
                MAX_REVIEW_FILE_BYTES,
                &shared_directories,
            )
            .await
        {
            Ok(file) => match decode_review_text(file.content) {
                Ok(content) => {
                    let response = ReviewFileResponse {
                        document_root: file.document_root,
                        root: file.root,
                        path: file.path.clone(),
                        relative_path: file.relative_path,
                        hash: stable_content_hash(content.as_bytes()),
                        size: file.size,
                        line_count: if content.is_empty() {
                            0
                        } else {
                            content.lines().count()
                        },
                        language: review_language(std::path::Path::new(&file.path)).to_string(),
                        content,
                        read_only: file.read_only,
                    };
                    json_response(StatusCode::OK, &response)
                }
                Err(message) => json_error(StatusCode::UNSUPPORTED_MEDIA_TYPE, message),
            },
            Err(error) => {
                let status = match error.kind() {
                    std::io::ErrorKind::NotFound => StatusCode::NOT_FOUND,
                    std::io::ErrorKind::PermissionDenied => StatusCode::FORBIDDEN,
                    std::io::ErrorKind::FileTooLarge => StatusCode::PAYLOAD_TOO_LARGE,
                    _ => StatusCode::BAD_REQUEST,
                };
                json_error(status, &format!("unable to read WSL file: {error}"))
            }
        };
    }
    let shared_directories = match shared_document_directories(&state) {
        Ok(value) => value,
        Err((status, message)) => return json_error(status, &message),
    };
    match load_review_file_with_shared(&request, &shared_directories) {
        Ok(file) => json_response(StatusCode::OK, &file),
        Err((status, message)) => json_error(status, &message),
    }
}

async fn read_review_epub(
    State(state): State<GatewayState>,
    Json(request): Json<ReviewFileRequest>,
) -> Response<Body> {
    #[cfg(windows)]
    if state.codex.execution_environment() == "wsl" {
        return json_error(
            StatusCode::NOT_IMPLEMENTED,
            "opening EPUB files from WSL workspaces is not available yet",
        );
    }
    let shared_directories = match shared_document_directories(&state) {
        Ok(value) => value,
        Err((status, message)) => return json_error(status, &message),
    };
    let (_, path, _, _) = match resolve_epub_path_with_shared(&request, &shared_directories) {
        Ok(value) => value,
        Err((status, message)) => return json_error(status, &message),
    };
    let loaded = match tokio::task::spawn_blocking(move || epub_reader::load(&path)).await {
        Ok(result) => result,
        Err(error) => return gateway_error(&format!("EPUB validation task failed: {error}")),
    };
    match loaded {
        Ok(asset) => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "application/epub+zip")
            .header(header::CACHE_CONTROL, "no-store")
            .header("x-studio-epub-hash", asset.hash)
            .header(header::CONTENT_LENGTH, asset.size)
            .body(Body::from(asset.bytes))
            .unwrap_or_else(|error| gateway_error(&error.to_string())),
        Err(message) => json_error(StatusCode::UNSUPPORTED_MEDIA_TYPE, &message),
    }
}

async fn read_review_pdf(
    State(state): State<GatewayState>,
    Json(request): Json<ReviewFileRequest>,
) -> Response<Body> {
    read_review_binary(&state, request, "pdf").await
}

async fn read_review_spreadsheet(
    State(state): State<GatewayState>,
    Json(request): Json<ReviewFileRequest>,
) -> Response<Body> {
    read_review_binary(&state, request, "xlsx").await
}

async fn read_review_binary(
    state: &GatewayState,
    request: ReviewFileRequest,
    expected: &'static str,
) -> Response<Body> {
    #[cfg(windows)]
    let loaded = if state.codex.execution_environment() == "wsl" {
        match configured_shared_document_directories(state) {
            Ok(shared_directories) => state
                .codex
                .read_wsl_file(
                    &request.root,
                    &request.path,
                    MAX_REVIEW_DOCUMENT_BYTES,
                    &shared_directories,
                )
                .await
                .map(|file| (file.content, file.path))
                .map_err(|error| error.to_string()),
            Err((_, message)) => Err(message),
        }
    } else {
        match shared_document_directories(state) {
            Ok(shared_directories) => load_review_binary_with_shared(&request, &shared_directories)
                .map_err(|(_, message)| message),
            Err((_, message)) => Err(message),
        }
    };
    #[cfg(not(windows))]
    let loaded = match shared_document_directories(state) {
        Ok(shared_directories) => load_review_binary_with_shared(&request, &shared_directories)
            .map_err(|(_, message)| message),
        Err((_, message)) => Err(message),
    };

    let (bytes, path) = match loaded {
        Ok(value) => value,
        Err(message) => return json_error(StatusCode::BAD_REQUEST, &message),
    };
    let extension = std::path::Path::new(&path)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let valid = match expected {
        "pdf" => extension == "pdf" && bytes.starts_with(b"%PDF-"),
        "xlsx" => {
            extension == "xlsx"
                && bytes.starts_with(b"PK\x03\x04")
                && validate_spreadsheet_archive(&bytes).is_ok()
        }
        _ => false,
    };
    if !valid {
        return json_error(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            &format!("file is not a valid {expected} document"),
        );
    }
    let mime = if expected == "pdf" {
        "application/pdf"
    } else {
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
    };
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, mime)
        .header(header::CACHE_CONTROL, "no-store")
        .header("x-content-type-options", "nosniff")
        .header("x-studio-content-hash", stable_content_hash(&bytes))
        .header(header::CONTENT_LENGTH, bytes.len())
        .body(Body::from(bytes))
        .unwrap_or_else(|error| gateway_error(&error.to_string()))
}

fn validate_spreadsheet_archive(bytes: &[u8]) -> Result<(), String> {
    const MAX_ENTRIES: usize = 10_000;
    const MAX_UNCOMPRESSED_BYTES: u64 = 200 * 1024 * 1024;
    let cursor = std::io::Cursor::new(bytes);
    let mut archive =
        zip::ZipArchive::new(cursor).map_err(|error| format!("invalid XLSX ZIP: {error}"))?;
    if archive.len() > MAX_ENTRIES {
        return Err("XLSX contains too many ZIP entries".into());
    }
    let mut total = 0_u64;
    let mut content_types = false;
    let mut workbook = false;
    for index in 0..archive.len() {
        let entry = archive
            .by_index_raw(index)
            .map_err(|error| format!("invalid XLSX entry: {error}"))?;
        total = total
            .checked_add(entry.size())
            .ok_or_else(|| "XLSX expanded size overflow".to_string())?;
        if total > MAX_UNCOMPRESSED_BYTES {
            return Err("XLSX expanded content exceeds 200 MiB".into());
        }
        let name = entry.name().replace('\\', "/");
        if name == "[Content_Types].xml" {
            content_types = true;
        }
        if name == "xl/workbook.xml" {
            workbook = true;
        }
    }
    if !content_types || !workbook {
        return Err("XLSX package metadata is missing".into());
    }
    Ok(())
}

fn load_review_binary_with_shared(
    request: &ReviewFileRequest,
    shared_directories: &[PathBuf],
) -> Result<(Vec<u8>, String), (StatusCode, String)> {
    let (_root, path, _metadata, _document_root) = resolve_review_path_with_shared(
        request,
        MAX_REVIEW_DOCUMENT_BYTES,
        "50 MiB document",
        shared_directories,
    )?;
    let bytes = fs::read(&path).map_err(|error| {
        (
            StatusCode::BAD_REQUEST,
            format!("unable to read document: {error}"),
        )
    })?;
    Ok((bytes, path.to_string_lossy().into_owned()))
}

async fn get_epub_reading_state(
    State(state): State<GatewayState>,
    Json(request): Json<ReviewFileRequestWithHash>,
) -> Response<Body> {
    #[cfg(windows)]
    if state.codex.execution_environment() == "wsl" {
        return json_error(
            StatusCode::NOT_IMPLEMENTED,
            "EPUB reading state for WSL workspaces is not available yet",
        );
    }
    let review = ReviewFileRequest {
        root: request.root,
        path: request.path,
    };
    let shared_directories = match shared_document_directories(&state) {
        Ok(value) => value,
        Err((status, message)) => return json_error(status, &message),
    };
    let (_, path, _, _) = match resolve_epub_path_with_shared(&review, &shared_directories) {
        Ok(value) => value,
        Err((status, message)) => return json_error(status, &message),
    };
    let _guard = match state.epub_reading_lock.lock() {
        Ok(guard) => guard,
        Err(_) => return gateway_error("EPUB reading state lock is unavailable"),
    };
    match epub_reader::find_state(&state.epub_reading_path, &path, &request.book_hash) {
        Ok(Some(value)) => json_response(StatusCode::OK, &value),
        Ok(None) => StatusCode::NO_CONTENT.into_response(),
        Err(message) => json_error(StatusCode::BAD_REQUEST, &message),
    }
}

async fn put_epub_reading_state(
    State(state): State<GatewayState>,
    Json(reading): Json<epub_reader::ReadingState>,
) -> Response<Body> {
    #[cfg(windows)]
    if state.codex.execution_environment() == "wsl" {
        return json_error(
            StatusCode::NOT_IMPLEMENTED,
            "EPUB reading state for WSL workspaces is not available yet",
        );
    }
    let request = ReviewFileRequest {
        root: reading.root.clone(),
        path: reading.path.clone(),
    };
    let shared_directories = match shared_document_directories(&state) {
        Ok(value) => value,
        Err((status, message)) => return json_error(status, &message),
    };
    let (_, path, _, _) = match resolve_epub_path_with_shared(&request, &shared_directories) {
        Ok(value) => value,
        Err((status, message)) => return json_error(status, &message),
    };
    let _guard = match state.epub_reading_lock.lock() {
        Ok(guard) => guard,
        Err(_) => return gateway_error("EPUB reading state lock is unavailable"),
    };
    match epub_reader::save_state(&state.epub_reading_path, &path, reading) {
        Ok(value) => json_response(StatusCode::OK, &value),
        Err(message) => json_error(StatusCode::BAD_REQUEST, &message),
    }
}

async fn list_workspace_directory(
    State(_state): State<GatewayState>,
    Json(request): Json<WorkspaceListRequest>,
) -> Response<Body> {
    #[cfg(windows)]
    if _state.codex.execution_environment() == "wsl" {
        return json_error(
            StatusCode::NOT_IMPLEMENTED,
            "the workspace demo does not list WSL directories yet",
        );
    }
    match load_workspace_directory(&request) {
        Ok(value) => json_response(StatusCode::OK, &value),
        Err((status, message)) => json_error(status, &message),
    }
}

async fn save_workspace_file(
    State(_state): State<GatewayState>,
    Json(request): Json<WorkspaceSaveRequest>,
) -> Response<Body> {
    #[cfg(windows)]
    if _state.codex.execution_environment() == "wsl" {
        return json_error(
            StatusCode::NOT_IMPLEMENTED,
            "editing files in WSL workspaces is not available yet",
        );
    }
    match persist_workspace_file(&request) {
        Ok(value) => json_response(StatusCode::OK, &value),
        Err(WorkspaceSaveError::Conflict(actual_hash)) => {
            let payload = WorkspaceConflictResponse {
                error: WorkspaceConflictError {
                    message: "file changed on disk",
                    code: "workspace_file_conflict",
                    actual_hash,
                },
            };
            json_response(StatusCode::CONFLICT, &payload)
        }
        Err(WorkspaceSaveError::Http(status, message)) => json_error(status, &message),
    }
}

async fn git_status(
    State(state): State<GatewayState>,
    Json(request): Json<GitRootRequest>,
) -> Response<Body> {
    match load_git_status(&state, &request.root).await {
        Ok(status) => json_response(StatusCode::OK, &status),
        Err((status, message)) => json_error(status, &message),
    }
}

async fn git_diff(
    State(state): State<GatewayState>,
    Json(request): Json<GitDiffRequest>,
) -> Response<Body> {
    if let Err(message) = git_review::validate_relative_path(&request.path) {
        return json_error(StatusCode::BAD_REQUEST, &message);
    }
    let status = match load_git_status(&state, &request.root).await {
        Ok(status) => status,
        Err((status, message)) => return json_error(status, &message),
    };
    let Some(file) = status.files.iter().find(|file| file.path == request.path) else {
        return json_error(StatusCode::NOT_FOUND, "file has no reviewable Git changes");
    };
    let scope_available = match request.scope {
        git_review::GitDiffScope::Staged => file.staged,
        git_review::GitDiffScope::Unstaged => file.unstaged,
    };
    if !scope_available {
        return json_error(
            StatusCode::NOT_FOUND,
            "file has no changes in the requested Git scope",
        );
    }

    let root = status.root.clone();
    let untracked = file.untracked && request.scope == git_review::GitDiffScope::Unstaged;
    let arguments = git_review::diff_arguments(&root, &request.path, request.scope, untracked);
    let output = match run_git(&state, &arguments, true).await {
        Ok(output) if output.status.success() || (untracked && output.status.code() == Some(1)) => {
            output
        }
        Ok(output) => return git_command_error("read diff", output),
        Err(error) => return json_error(StatusCode::BAD_GATEWAY, &error),
    };
    let (content, truncated) = git_review::bounded_diff(output.stdout);
    let response = GitDiffResponse {
        root,
        path: request.path,
        scope: request.scope.into(),
        binary: content.contains("Binary files ") || content.contains("GIT binary patch"),
        content,
        truncated,
    };
    json_response(StatusCode::OK, &response)
}

async fn git_stage(
    State(state): State<GatewayState>,
    Json(request): Json<GitPathsRequest>,
) -> Response<Body> {
    mutate_git_paths(&state, request, true).await
}

async fn git_unstage(
    State(state): State<GatewayState>,
    Json(request): Json<GitPathsRequest>,
) -> Response<Body> {
    mutate_git_paths(&state, request, false).await
}

async fn mutate_git_paths(
    state: &GatewayState,
    request: GitPathsRequest,
    stage: bool,
) -> Response<Body> {
    if request.paths.is_empty() || request.paths.len() > 100 {
        return json_error(
            StatusCode::BAD_REQUEST,
            "select between 1 and 100 Git paths",
        );
    }
    for path in &request.paths {
        if let Err(message) = git_review::validate_relative_path(path) {
            return json_error(StatusCode::BAD_REQUEST, &message);
        }
    }
    let status = match load_git_status(state, &request.root).await {
        Ok(status) => status,
        Err((status, message)) => return json_error(status, &message),
    };
    if request
        .paths
        .iter()
        .any(|path| !status.files.iter().any(|file| file.path == *path))
    {
        return json_error(
            StatusCode::BAD_REQUEST,
            "Git path is not in the current change set",
        );
    }
    let arguments =
        git_review::mutation_arguments(&status.root, &request.paths, stage, status.has_head);
    let output = match run_git(state, &arguments, false).await {
        Ok(output) => output,
        Err(error) => return json_error(StatusCode::BAD_GATEWAY, &error),
    };
    if !output.status.success() {
        return git_command_error(
            if stage {
                "stage files"
            } else {
                "unstage files"
            },
            output,
        );
    }
    // The root was resolved and validated above in this same operation.
    match load_git_status_at_root(state, status.root).await {
        Ok(status) => json_response(StatusCode::OK, &status),
        Err((status, message)) => json_error(status, &message),
    }
}

async fn load_git_status(
    state: &GatewayState,
    requested_root: &str,
) -> Result<GitStatusResponse, (StatusCode, String)> {
    if requested_root.is_empty()
        || requested_root.contains('\0')
        || requested_root.contains('\n')
        || requested_root.contains('\r')
    {
        return Err((
            StatusCode::BAD_REQUEST,
            "project directory is invalid".to_owned(),
        ));
    }
    #[cfg(not(windows))]
    let requested_root = fs::canonicalize(requested_root)
        .map_err(|_| {
            (
                StatusCode::BAD_REQUEST,
                "project directory is unavailable".to_owned(),
            )
        })?
        .to_string_lossy()
        .into_owned();
    #[cfg(windows)]
    let requested_root = requested_root.to_owned();
    let root_arguments = git_review::repository_root_arguments(&requested_root);
    let root_output = run_git(state, &root_arguments, true)
        .await
        .map_err(|message| (StatusCode::BAD_GATEWAY, message))?;
    if !root_output.status.success() {
        let message = bounded_command_stderr(&root_output.stderr);
        return Err((
            StatusCode::BAD_REQUEST,
            if message.is_empty() {
                "project directory is not a Git repository".to_owned()
            } else {
                format!("unable to inspect Git repository: {message}")
            },
        ));
    }
    let root = git_review::parse_repository_root(&root_output.stdout)
        .map_err(|message| (StatusCode::BAD_GATEWAY, message))?;
    load_git_status_at_root(state, root).await
}

async fn load_git_status_at_root(
    state: &GatewayState,
    root: String,
) -> Result<GitStatusResponse, (StatusCode, String)> {
    let arguments = git_review::status_arguments(&root);
    let output = run_git(state, &arguments, true)
        .await
        .map_err(|message| (StatusCode::BAD_GATEWAY, message))?;
    if !output.status.success() {
        let message = bounded_command_stderr(&output.stderr);
        return Err((
            StatusCode::BAD_REQUEST,
            if message.is_empty() {
                "project directory is not a Git repository".to_owned()
            } else {
                format!("unable to inspect Git repository: {message}")
            },
        ));
    }
    git_review::parse_status(root, &output.stdout)
        .map_err(|message| (StatusCode::BAD_GATEWAY, message))
}

async fn run_git(
    state: &GatewayState,
    arguments: &[String],
    read_only: bool,
) -> Result<std::process::Output, String> {
    let arguments = arguments.iter().map(String::as_str).collect::<Vec<_>>();
    let environment = if read_only {
        vec![("GIT_OPTIONAL_LOCKS", "0"), ("GIT_CONFIG_NOSYSTEM", "1")]
    } else {
        vec![("GIT_CONFIG_NOSYSTEM", "1")]
    };
    tokio::time::timeout(
        std::time::Duration::from_secs(12),
        state
            .codex
            .run_workspace_command("git", &arguments, &environment),
    )
    .await
    .map_err(|_| "Git command timed out".to_owned())?
    .map_err(|error| format!("unable to start Git: {error}"))
}

fn git_command_error(action: &str, output: std::process::Output) -> Response<Body> {
    let detail = bounded_command_stderr(&output.stderr);
    let message = if detail.is_empty() {
        format!("unable to {action}")
    } else {
        format!("unable to {action}: {detail}")
    };
    json_error(StatusCode::BAD_REQUEST, &message)
}

fn bounded_command_stderr(stderr: &[u8]) -> String {
    String::from_utf8_lossy(&stderr[..stderr.len().min(8 * 1024)])
        .trim()
        .to_owned()
}

#[derive(Debug)]
enum WorkspaceSaveError {
    Conflict(String),
    Http(StatusCode, String),
}

fn persist_workspace_file(
    request: &WorkspaceSaveRequest,
) -> Result<ReviewFileResponse, WorkspaceSaveError> {
    let bytes = request.content.as_bytes();
    if bytes.len() > MAX_EDIT_FILE_BYTES {
        return Err(WorkspaceSaveError::Http(
            StatusCode::PAYLOAD_TOO_LARGE,
            "edited file exceeds the 5 MiB limit".to_string(),
        ));
    }
    let review_request = ReviewFileRequest {
        root: request.root.clone(),
        path: request.path.clone(),
    };
    let (_root, path, metadata) =
        resolve_review_path(&review_request, MAX_EDIT_FILE_BYTES as u64, "5 MiB editing")
            .map_err(|(status, message)| WorkspaceSaveError::Http(status, message))?;
    let current = fs::read(&path).map_err(|error| {
        WorkspaceSaveError::Http(
            StatusCode::BAD_REQUEST,
            format!("unable to read current file: {error}"),
        )
    })?;
    let actual_hash = stable_content_hash(&current);
    if !request.overwrite && actual_hash != request.expected_hash {
        return Err(WorkspaceSaveError::Conflict(actual_hash));
    }

    let parent = path.parent().ok_or_else(|| {
        WorkspaceSaveError::Http(
            StatusCode::BAD_REQUEST,
            "file has no parent directory".to_string(),
        )
    })?;
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("workspace-file");
    let temporary = parent.join(format!(".{name}.studio-save-{}.tmp", uuid::Uuid::new_v4()));
    let write_result = (|| -> std::io::Result<()> {
        let mut output = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)?;
        output.set_permissions(metadata.permissions())?;
        output.write_all(bytes)?;
        output.sync_all()?;
        fs::rename(&temporary, &path)?;
        #[cfg(unix)]
        fs::File::open(parent)?.sync_all()?;
        Ok(())
    })();
    if let Err(error) = write_result {
        let _ = fs::remove_file(&temporary);
        return Err(WorkspaceSaveError::Http(
            StatusCode::BAD_REQUEST,
            format!("unable to save file: {error}"),
        ));
    }

    load_review_file(&review_request)
        .map_err(|(status, message)| WorkspaceSaveError::Http(status, message))
}

fn resolve_epub_path_with_shared(
    request: &ReviewFileRequest,
    shared_directories: &[PathBuf],
) -> Result<(PathBuf, PathBuf, fs::Metadata, PathBuf), (StatusCode, String)> {
    if !request.path.to_ascii_lowercase().ends_with(".epub") {
        return Err((
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "only .epub publications can be opened by the EPUB reader".to_owned(),
        ));
    }
    resolve_review_path_with_shared(
        request,
        epub_reader::MAX_EPUB_BYTES,
        "128 MiB EPUB",
        shared_directories,
    )
}

fn load_workspace_directory(
    request: &WorkspaceListRequest,
) -> Result<WorkspaceListResponse, (StatusCode, String)> {
    const MAX_ENTRIES: usize = 500;
    let root = fs::canonicalize(&request.root).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            "project directory is unavailable".to_string(),
        )
    })?;
    let relative = PathBuf::from(&request.path);
    if relative.is_absolute() {
        return Err((
            StatusCode::BAD_REQUEST,
            "workspace paths must be relative to the project directory".to_string(),
        ));
    }
    let directory = fs::canonicalize(root.join(&relative)).map_err(|_| {
        (
            StatusCode::NOT_FOUND,
            "directory does not exist".to_string(),
        )
    })?;
    if !directory.starts_with(&root) {
        return Err((
            StatusCode::FORBIDDEN,
            "directory is outside the project directory".to_string(),
        ));
    }
    if !directory.is_dir() {
        return Err((
            StatusCode::BAD_REQUEST,
            "workspace path is not a directory".to_string(),
        ));
    }

    let mut entries = fs::read_dir(&directory)
        .map_err(|error| {
            (
                StatusCode::BAD_REQUEST,
                format!("unable to list directory: {error}"),
            )
        })?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let canonical = fs::canonicalize(entry.path()).ok()?;
            if !canonical.starts_with(&root) {
                return None;
            }
            let metadata = entry.metadata().ok()?;
            let kind = if metadata.is_dir() {
                "directory"
            } else if metadata.is_file() {
                "file"
            } else {
                return None;
            };
            let name = entry.file_name().to_string_lossy().into_owned();
            let path = entry
                .path()
                .strip_prefix(&root)
                .ok()?
                .to_string_lossy()
                .replace('\\', "/");
            Some(WorkspaceEntry {
                name,
                path,
                kind,
                size: (kind == "file").then_some(metadata.len()),
            })
        })
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| {
        (left.kind != "directory", left.name.to_lowercase())
            .cmp(&(right.kind != "directory", right.name.to_lowercase()))
    });
    let truncated = entries.len() > MAX_ENTRIES;
    entries.truncate(MAX_ENTRIES);
    Ok(WorkspaceListResponse {
        root: root.to_string_lossy().into_owned(),
        path: directory
            .strip_prefix(&root)
            .unwrap_or(&directory)
            .to_string_lossy()
            .replace('\\', "/"),
        entries,
        truncated,
    })
}

async fn read_review_image(
    State(state): State<GatewayState>,
    Json(request): Json<ReviewFileRequest>,
) -> Response<Body> {
    #[cfg(windows)]
    if state.codex.execution_environment() == "wsl" {
        let shared_directories = match configured_shared_document_directories(&state) {
            Ok(value) => value,
            Err((status, message)) => return json_error(status, &message),
        };
        return match state
            .codex
            .read_wsl_file(
                &request.root,
                &request.path,
                MAX_REVIEW_IMAGE_BYTES,
                &shared_directories,
            )
            .await
        {
            Ok(file) => match review_image_mime(std::path::Path::new(&file.path), &file.content) {
                Some(mime) => review_image_response(file.content, mime),
                None => json_error(
                    StatusCode::UNSUPPORTED_MEDIA_TYPE,
                    "only PNG, JPEG, WebP, GIF, and SVG images can be previewed",
                ),
            },
            Err(error) => {
                let status = match error.kind() {
                    std::io::ErrorKind::NotFound => StatusCode::NOT_FOUND,
                    std::io::ErrorKind::PermissionDenied => StatusCode::FORBIDDEN,
                    std::io::ErrorKind::FileTooLarge => StatusCode::PAYLOAD_TOO_LARGE,
                    _ => StatusCode::BAD_REQUEST,
                };
                json_error(status, &format!("unable to read WSL image: {error}"))
            }
        };
    }
    let shared_directories = match shared_document_directories(&state) {
        Ok(value) => value,
        Err((status, message)) => return json_error(status, &message),
    };
    match load_review_image_with_shared(&request, &shared_directories) {
        Ok((bytes, mime)) => review_image_response(bytes, mime),
        Err((status, message)) => json_error(status, &message),
    }
}

fn review_image_response(bytes: Vec<u8>, mime: &'static str) -> Response<Body> {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, mime)
        .header(header::CACHE_CONTROL, "no-store")
        .header("x-content-type-options", "nosniff")
        .body(Body::from(bytes))
        .expect("valid image response")
}

#[cfg(test)]
fn load_review_image(
    request: &ReviewFileRequest,
) -> Result<(Vec<u8>, &'static str), (StatusCode, String)> {
    load_review_image_with_shared(request, &[])
}

fn load_review_image_with_shared(
    request: &ReviewFileRequest,
    shared_directories: &[PathBuf],
) -> Result<(Vec<u8>, &'static str), (StatusCode, String)> {
    let (_root, path, _metadata, _document_root) = resolve_review_path_with_shared(
        request,
        MAX_REVIEW_IMAGE_BYTES,
        "25 MiB image",
        shared_directories,
    )?;
    let bytes = fs::read(&path).map_err(|error| {
        (
            StatusCode::BAD_REQUEST,
            format!("unable to read image: {error}"),
        )
    })?;
    let mime = review_image_mime(&path, &bytes).ok_or((
        StatusCode::UNSUPPORTED_MEDIA_TYPE,
        "only PNG, JPEG, WebP, GIF, and SVG images can be previewed".to_string(),
    ))?;
    Ok((bytes, mime))
}

fn review_image_mime(path: &std::path::Path, bytes: &[u8]) -> Option<&'static str> {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match extension.as_str() {
        "png" if bytes.starts_with(b"\x89PNG\r\n\x1a\n") => Some("image/png"),
        "jpg" | "jpeg" if bytes.starts_with(&[0xff, 0xd8, 0xff]) => Some("image/jpeg"),
        "gif" if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") => Some("image/gif"),
        "webp"
            if bytes.len() >= 12
                && bytes.starts_with(b"RIFF")
                && bytes.get(8..12) == Some(b"WEBP") =>
        {
            Some("image/webp")
        }
        "svg" if looks_like_svg(bytes) => Some("image/svg+xml"),
        _ => None,
    }
}

fn looks_like_svg(bytes: &[u8]) -> bool {
    let Ok(source) = std::str::from_utf8(bytes) else {
        return false;
    };
    let source = source.trim_start_matches('\u{feff}').trim_start();
    source
        .get(..source.len().min(4096))
        .is_some_and(|prefix| prefix.contains("<svg") && !prefix.contains("<html"))
}

fn load_review_file(
    request: &ReviewFileRequest,
) -> Result<ReviewFileResponse, (StatusCode, String)> {
    load_review_file_with_shared(request, &[])
}

fn load_review_file_with_shared(
    request: &ReviewFileRequest,
    shared_directories: &[PathBuf],
) -> Result<ReviewFileResponse, (StatusCode, String)> {
    let (root, path, metadata, document_root) = resolve_review_path_with_shared(
        request,
        MAX_REVIEW_FILE_BYTES,
        "5 MiB review",
        shared_directories,
    )?;
    let bytes = fs::read(&path).map_err(|error| {
        (
            StatusCode::BAD_REQUEST,
            format!("unable to read file: {error}"),
        )
    })?;
    let content = decode_review_text(bytes)
        .map_err(|message| (StatusCode::UNSUPPORTED_MEDIA_TYPE, message.to_string()))?;

    let relative_path = path
        .strip_prefix(&document_root)
        .unwrap_or(&path)
        .to_string_lossy()
        .replace('\\', "/");
    Ok(ReviewFileResponse {
        root: root.to_string_lossy().into_owned(),
        document_root: document_root.to_string_lossy().into_owned(),
        path: path.to_string_lossy().into_owned(),
        relative_path,
        hash: stable_content_hash(content.as_bytes()),
        size: metadata.len(),
        line_count: if content.is_empty() {
            0
        } else {
            content.lines().count()
        },
        language: review_language(&path).to_string(),
        content,
        read_only: !path.starts_with(&root),
    })
}

fn decode_review_text(bytes: Vec<u8>) -> Result<String, &'static str> {
    let content = String::from_utf8(bytes).map_err(|_| "only UTF-8 text files can be reviewed")?;
    if content.contains('\0') {
        return Err("file contains NUL bytes and cannot be reviewed as text");
    }

    let mut character_count = 0_usize;
    let mut suspicious_controls = 0_usize;
    for character in content.chars() {
        character_count += 1;
        if character.is_control()
            && !matches!(character, '\n' | '\r' | '\t' | '\u{000c}' | '\u{001b}')
        {
            suspicious_controls += 1;
        }
    }
    let allowed_controls = (character_count / 100).max(2);
    if suspicious_controls > allowed_controls {
        return Err("file contains too many control characters to be reviewed as text");
    }
    Ok(content)
}

fn resolve_review_path(
    request: &ReviewFileRequest,
    max_bytes: u64,
    limit_label: &str,
) -> Result<(PathBuf, PathBuf, fs::Metadata), (StatusCode, String)> {
    resolve_review_path_with_shared(request, max_bytes, limit_label, &[])
        .map(|(root, path, metadata, _)| (root, path, metadata))
}

fn resolve_review_path_with_shared(
    request: &ReviewFileRequest,
    max_bytes: u64,
    limit_label: &str,
    shared_directories: &[PathBuf],
) -> Result<(PathBuf, PathBuf, fs::Metadata, PathBuf), (StatusCode, String)> {
    let root = fs::canonicalize(&request.root).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            "project directory is unavailable".to_string(),
        )
    })?;
    let requested = PathBuf::from(&request.path);
    let candidate = if requested.is_absolute() {
        requested
    } else {
        root.join(requested)
    };
    let path = fs::canonicalize(candidate)
        .map_err(|_| (StatusCode::NOT_FOUND, "file does not exist".to_string()))?;
    let document_root = if path.starts_with(&root) {
        root.clone()
    } else if let Some(shared_root) = shared_directories
        .iter()
        .find(|shared_root| path.starts_with(shared_root))
    {
        shared_root.clone()
    } else {
        return Err((
            StatusCode::FORBIDDEN,
            "file is outside the project directory and shared document directories".to_string(),
        ));
    };
    let metadata = fs::metadata(&path).map_err(|error| {
        (
            StatusCode::BAD_REQUEST,
            format!("unable to inspect file: {error}"),
        )
    })?;
    if !metadata.is_file() {
        return Err((
            StatusCode::BAD_REQUEST,
            "path is not a regular file".to_string(),
        ));
    }
    if metadata.len() > max_bytes {
        return Err((
            StatusCode::PAYLOAD_TOO_LARGE,
            format!("file exceeds the {limit_label} limit"),
        ));
    }
    Ok((root, path, metadata, document_root))
}

fn stable_content_hash(bytes: &[u8]) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("fnv1a64:{hash:016x}")
}

fn review_language(path: &std::path::Path) -> &'static str {
    match path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "md" | "mdown" | "markdown" | "mkd" => "markdown",
        "rs" => "rust",
        "js" | "mjs" | "cjs" => "javascript",
        "ts" | "tsx" => "typescript",
        "py" => "python",
        "html" | "htm" => "html",
        "css" => "css",
        "json" => "json",
        "toml" => "toml",
        "yaml" | "yml" => "yaml",
        "sh" | "bash" => "shell",
        _ => "text",
    }
}

async fn app_js() -> impl IntoResponse {
    javascript(include_str!("../../ui/app.js"))
}

async fn bootstrap_errors_js() -> impl IntoResponse {
    javascript(include_str!("../../ui/bootstrap-errors.js"))
}

async fn i18n_js() -> impl IntoResponse {
    javascript(include_str!("../../ui/i18n.mjs"))
}

async fn codex_native_js() -> impl IntoResponse {
    javascript(include_str!("../../ui/codex-native.mjs"))
}

async fn codex_lifecycle_diagnostics_js() -> impl IntoResponse {
    javascript(include_str!("../../ui/codex-lifecycle-diagnostics.mjs"))
}

async fn opencode_native_js() -> impl IntoResponse {
    javascript(include_str!("../../ui/opencode-native.mjs"))
}

async fn opencode_event_reducer_js() -> impl IntoResponse {
    javascript(include_str!("../../ui/opencode-event-reducer.mjs"))
}

async fn thread_history_tail_js() -> impl IntoResponse {
    javascript(include_str!("../../ui/thread-history-tail.mjs"))
}

async fn codex_history_loader_js() -> impl IntoResponse {
    javascript(include_str!("../../ui/codex-history-loader.mjs"))
}

async fn history_load_coordinator_js() -> impl IntoResponse {
    javascript(include_str!("../../ui/history-load-coordinator.mjs"))
}

async fn model_revision_js() -> impl IntoResponse {
    javascript(include_str!("../../ui/model-revision.mjs"))
}

async fn performance_monitor_js() -> impl IntoResponse {
    javascript(include_str!("../../ui/performance-monitor.mjs"))
}

async fn selection_translation_js() -> impl IntoResponse {
    javascript(include_str!("../../ui/selection-translation.mjs"))
}

async fn continuation_draft_js() -> impl IntoResponse {
    javascript(include_str!("../../ui/continuation-draft.mjs"))
}

async fn backends_js() -> impl IntoResponse {
    javascript(include_str!("../../ui/backends.mjs"))
}

async fn model_display_js() -> impl IntoResponse {
    javascript(include_str!("../../ui/model-display.mjs"))
}

async fn message_queue_js() -> impl IntoResponse {
    javascript(include_str!("../../ui/message-queue.mjs"))
}

async fn session_catalog_js() -> impl IntoResponse {
    javascript(include_str!("../../ui/session-catalog.mjs"))
}

async fn serialized_state_writer_js() -> impl IntoResponse {
    javascript(include_str!("../../ui/serialized-state-writer.mjs"))
}

async fn session_model_cache_js() -> impl IntoResponse {
    javascript(include_str!("../../ui/session-model-cache.mjs"))
}

async fn session_model_preferences_js() -> impl IntoResponse {
    javascript(include_str!("../../ui/session-model-preferences.mjs"))
}

async fn session_management_js() -> impl IntoResponse {
    javascript(include_str!("../../ui/session-management.mjs"))
}

async fn transcript_presentation_js() -> impl IntoResponse {
    javascript(include_str!("../../ui/transcript-presentation.mjs"))
}

async fn transcript_dom_js() -> impl IntoResponse {
    javascript(include_str!("../../ui/transcript-dom.mjs"))
}

async fn thread_catalog_js() -> impl IntoResponse {
    javascript(include_str!("../../ui/thread-catalog.mjs"))
}

async fn thread_fork_js() -> impl IntoResponse {
    javascript(include_str!("../../ui/thread-fork.mjs"))
}

async fn thread_router_controller_js() -> impl IntoResponse {
    javascript(include_str!("../../ui/thread-router-controller.mjs"))
}

async fn thread_workset_js() -> impl IntoResponse {
    javascript(include_str!("../../ui/thread-workset.mjs"))
}

async fn session_search_js() -> impl IntoResponse {
    javascript(include_str!("../../ui/session-search.mjs"))
}

async fn composer_tools_js() -> impl IntoResponse {
    javascript(include_str!("../../ui/composer-tools.mjs"))
}

async fn composer_images_js() -> impl IntoResponse {
    javascript(include_str!("../../ui/composer-images.mjs"))
}

async fn document_review_js() -> impl IntoResponse {
    javascript(include_str!("../../ui/document-review.mjs"))
}

async fn document_outline_js() -> impl IntoResponse {
    javascript(include_str!("../../ui/document-outline.mjs"))
}

async fn environment_profile_js() -> impl IntoResponse {
    javascript(include_str!("../../ui/environment-profile.mjs"))
}

async fn epub_reader_js() -> impl IntoResponse {
    javascript(include_str!("../../ui/epub-reader.mjs"))
}

async fn epub_comment_provider_js() -> impl IntoResponse {
    javascript(include_str!("../../ui/epub-comment-provider.mjs"))
}

async fn workspace_tools_js() -> impl IntoResponse {
    javascript(include_str!("../../ui/workspace-tools.mjs"))
}

async fn session_resources_js() -> impl IntoResponse {
    javascript(include_str!("../../ui/session-resources.mjs"))
}

async fn session_resources_ui_js() -> impl IntoResponse {
    javascript(include_str!("../../ui/session-resources-ui.mjs"))
}

async fn git_review_js() -> impl IntoResponse {
    javascript(include_str!("../../ui/git-review.mjs"))
}

async fn right_rail_layout_js() -> impl IntoResponse {
    javascript(include_str!("../../ui/right-rail-layout.mjs"))
}

async fn workspace_editor_js() -> impl IntoResponse {
    javascript(include_str!("../../ui/workspace-editor.mjs"))
}

async fn comment_core_js() -> impl IntoResponse {
    javascript(include_str!("../../ui/comment-core.mjs"))
}

async fn comment_markers_js() -> impl IntoResponse {
    javascript(include_str!("../../ui/comment-markers.mjs"))
}

async fn browser_comment_provider_js() -> impl IntoResponse {
    javascript(include_str!("../../ui/browser-comment-provider.mjs"))
}

async fn comment_source_providers_js() -> impl IntoResponse {
    javascript(include_str!("../../ui/comment-source-providers.mjs"))
}

async fn favorites_js() -> impl IntoResponse {
    javascript(include_str!("../../ui/favorites.mjs"))
}

async fn session_map_js() -> impl IntoResponse {
    javascript(include_str!("../../ui/session-map.mjs"))
}

async fn session_map_controller_js() -> impl IntoResponse {
    javascript(include_str!("../../ui/session-map-controller.mjs"))
}

async fn review_notes_controller_js() -> impl IntoResponse {
    javascript(include_str!("../../ui/review-notes-controller.mjs"))
}

async fn document_workspace_controller_js() -> impl IntoResponse {
    javascript(include_str!("../../ui/document-workspace-controller.mjs"))
}

async fn mermaid_config_js() -> impl IntoResponse {
    javascript(include_str!("../../ui/mermaid-config.mjs"))
}

async fn thread_router_js() -> impl IntoResponse {
    javascript(include_str!("../../ui/thread-router.mjs"))
}

async fn session_dispatch_js() -> impl IntoResponse {
    javascript(include_str!("../../ui/session-dispatch.mjs"))
}

async fn turn_navigator_js() -> impl IntoResponse {
    javascript(include_str!("../../ui/turn-navigator.mjs"))
}

async fn transcript_scroll_js() -> impl IntoResponse {
    javascript(include_str!("../../ui/transcript-scroll.mjs"))
}

async fn marked_js() -> impl IntoResponse {
    javascript(include_str!("../../ui/vendor/marked.esm.js"))
}

async fn dompurify_js() -> impl IntoResponse {
    javascript(include_str!("../../ui/vendor/purify.es.mjs"))
}

async fn mermaid_js() -> impl IntoResponse {
    javascript(include_str!("../../ui/vendor/mermaid.min.js"))
}

async fn epub_vendor_js() -> impl IntoResponse {
    javascript(include_str!("../../ui/vendor/epub.mjs"))
}

async fn pdf_reader_js() -> impl IntoResponse {
    javascript(include_str!("../../ui/pdf-reader.mjs"))
}

async fn pdf_comment_provider_js() -> impl IntoResponse {
    javascript(include_str!("../../ui/pdf-comment-provider.mjs"))
}

async fn table_reader_js() -> impl IntoResponse {
    javascript(include_str!("../../ui/table-reader.mjs"))
}

async fn table_comment_provider_js() -> impl IntoResponse {
    javascript(include_str!("../../ui/table-comment-provider.mjs"))
}

async fn pdf_vendor_js() -> impl IntoResponse {
    javascript(include_str!("../../ui/vendor/pdf.min.mjs"))
}

async fn pdf_worker_vendor_js() -> impl IntoResponse {
    javascript(include_str!("../../ui/vendor/pdf.worker.min.mjs"))
}

async fn table_vendor_js() -> impl IntoResponse {
    javascript(include_str!("../../ui/vendor/artifact-table.mjs"))
}

async fn workspace_editor_vendor_js() -> impl IntoResponse {
    javascript(include_str!("../../ui/vendor/workspace-editor.mjs"))
}

async fn workspace_terminal_vendor_js() -> impl IntoResponse {
    javascript(include_str!("../../ui/vendor/workspace-terminal.mjs"))
}

fn javascript(source: &'static str) -> impl IntoResponse {
    (
        [
            (header::CONTENT_TYPE, "text/javascript; charset=utf-8"),
            (header::CACHE_CONTROL, "no-store"),
        ],
        source,
    )
}

fn javascript_owned(source: String) -> impl IntoResponse {
    (
        [
            (header::CONTENT_TYPE, "text/javascript; charset=utf-8"),
            (header::CACHE_CONTROL, "no-store"),
        ],
        source,
    )
}

async fn styles_css() -> impl IntoResponse {
    (
        [
            (header::CONTENT_TYPE, "text/css; charset=utf-8"),
            (header::CACHE_CONTROL, "no-store"),
        ],
        include_str!("../../ui/styles.css"),
    )
}

async fn github_markdown_css() -> impl IntoResponse {
    (
        [
            (header::CONTENT_TYPE, "text/css; charset=utf-8"),
            (header::CACHE_CONTROL, "public, max-age=31536000, immutable"),
        ],
        include_str!("../../ui/vendor/github-markdown.css"),
    )
}

async fn xterm_css() -> impl IntoResponse {
    (
        [
            (header::CONTENT_TYPE, "text/css; charset=utf-8"),
            (header::CACHE_CONTROL, "public, max-age=31536000, immutable"),
        ],
        include_str!("../../ui/vendor/xterm.css"),
    )
}

async fn codex_info(State(state): State<GatewayState>) -> impl IntoResponse {
    codex_backend_info(&state.codex, state.codex.binary())
}

async fn backend_registry_info(State(state): State<GatewayState>) -> Json<BackendRegistryInfo> {
    Json(BackendRegistryInfo {
        backends: state.backend_descriptors.as_ref().clone(),
        config_path: state.backend_config_path.to_string_lossy().into_owned(),
        configuration_error: state.backend_config_error.as_ref().clone(),
    })
}

async fn codex_instance_info(
    State(state): State<GatewayState>,
    AxumPath(backend): AxumPath<String>,
) -> Response<Body> {
    let Some(instance) = state.codex_backends.get(&backend) else {
        return json_error(StatusCode::NOT_FOUND, "configured backend was not found");
    };
    json_response(
        StatusCode::OK,
        &codex_backend_info(&instance.server, &instance.command_line).0,
    )
}

fn codex_backend_info(server: &CodexAppServer, command_line: &str) -> axum::Json<BackendInfo> {
    let router_workspace = if server.execution_environment() == "wsl" {
        PathBuf::from("/var/tmp/codex-thread-studio-router")
    } else {
        let workspace = studio_router_workspace_path();
        if let Err(error) = fs::create_dir_all(&workspace) {
            eprintln!("failed to create Studio Router workspace: {error}");
        }
        workspace
    };
    axum::Json(BackendInfo {
        app_name: "Codex Thread Studio".to_string(),
        app_version: env!("CARGO_PKG_VERSION"),
        binary: command_line.to_string(),
        protocol: "Codex App Server v2",
        transport: "stdio JSONL via Studio WebSocket",
        router_workspace: router_workspace.to_string_lossy().into_owned(),
        execution_environment: server.execution_environment(),
        wsl_distribution: server.wsl_distribution().map(str::to_string),
        host_platform: std::env::consts::OS,
    })
}

async fn opencode_info(State(state): State<GatewayState>) -> impl IntoResponse {
    let info = state.opencode.info().await;
    let status = if info.reachable {
        StatusCode::OK
    } else {
        StatusCode::BAD_GATEWAY
    };
    (status, axum::Json(info))
}

async fn browser_info(State(state): State<GatewayState>) -> Json<BrowserInfo> {
    Json(BrowserInfo {
        enabled: true,
        available: state.embedded_browser,
        phase: 2,
        presentation: if state.embedded_browser {
            "embedded-webview"
        } else {
            "unavailable"
        },
    })
}

async fn proxy_opencode(
    State(state): State<GatewayState>,
    AxumPath(path): AxumPath<String>,
    method: Method,
    headers: HeaderMap,
    uri: Uri,
    body: Body,
) -> Response<Body> {
    state.opencode.proxy(path, method, headers, uri, body).await
}

async fn codex_app_server_ws(
    ws: WebSocketUpgrade,
    State(state): State<GatewayState>,
) -> impl IntoResponse {
    let protocol = state.security.websocket_protocol();
    ws.protocols([protocol])
        .on_upgrade(move |socket| async move { state.codex.bridge(socket).await })
}

async fn codex_instance_ws(
    ws: WebSocketUpgrade,
    State(state): State<GatewayState>,
    AxumPath(backend): AxumPath<String>,
) -> Response<Body> {
    let Some(instance) = state.codex_backends.get(&backend).cloned() else {
        return json_error(StatusCode::NOT_FOUND, "configured backend was not found");
    };
    let protocol = state.security.websocket_protocol();
    ws.protocols([protocol])
        .on_upgrade(move |socket| async move { instance.server.bridge(socket).await })
        .into_response()
}

async fn codex_lifecycle_ws(
    ws: WebSocketUpgrade,
    State(state): State<GatewayState>,
) -> impl IntoResponse {
    let protocol = state.security.websocket_protocol();
    ws.protocols([protocol])
        .on_upgrade(move |socket| async move {
            bridge_codex_lifecycle(socket, state.codex_backends).await
        })
}

async fn bridge_codex_lifecycle(
    socket: WebSocket,
    backends: Arc<BTreeMap<String, CodexBackendInstance>>,
) {
    let (mut output, mut input) = socket.split();
    let (outbound, mut events) = tokio::sync::mpsc::channel::<String>(512);
    let mut forwarders = Vec::with_capacity(backends.len());

    for (backend, instance) in backends.iter() {
        let backend = backend.clone();
        let mut receiver = instance.server.subscribe_lifecycle();
        let outbound = outbound.clone();
        forwarders.push(tokio::spawn(async move {
            loop {
                let message = match receiver.recv().await {
                    Ok(payload) => serde_json::from_str::<serde_json::Value>(&payload)
                        .unwrap_or_else(|_| {
                            json!({
                                "method": "studio/appServer/protocolError",
                                "params": { "message": "Invalid lifecycle event JSON" }
                            })
                        }),
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => json!({
                        "method": "studio/appServer/lagged",
                        "params": { "skipped": skipped }
                    }),
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                };
                if outbound
                    .send(codex_lifecycle_envelope(&backend, message))
                    .await
                    .is_err()
                {
                    break;
                }
            }
        }));
    }
    drop(outbound);

    let ready = json!({
        "method": "studio/codexLifecycle/ready",
        "params": { "backends": backends.keys().collect::<Vec<_>>() }
    })
    .to_string();
    if output.send(Message::Text(ready.into())).await.is_err() {
        for forwarder in forwarders {
            forwarder.abort();
        }
        return;
    }

    loop {
        tokio::select! {
            browser_message = input.next() => {
                let Some(Ok(browser_message)) = browser_message else { break };
                match browser_message {
                    Message::Close(_) => break,
                    Message::Ping(value) => {
                        if output.send(Message::Pong(value)).await.is_err() { break; }
                    }
                    _ => {}
                }
            }
            event = events.recv() => {
                let Some(payload) = event else { break };
                if output.send(Message::Text(payload.into())).await.is_err() { break; }
            }
        }
    }

    for forwarder in forwarders {
        forwarder.abort();
    }
}

fn codex_lifecycle_envelope(backend: &str, message: serde_json::Value) -> String {
    json!({
        "method": "studio/codexLifecycle/event",
        "params": { "backend": backend, "message": message }
    })
    .to_string()
}

async fn terminal_ws(ws: WebSocketUpgrade, State(state): State<GatewayState>) -> impl IntoResponse {
    let protocol = state.security.websocket_protocol();
    ws.protocols([protocol])
        .on_upgrade(move |socket| terminal_runtime::bridge(socket, state.environment_path))
}

#[derive(Deserialize)]
struct EnvironmentQuery {
    root: String,
}

async fn get_environment_profile(
    State(state): State<GatewayState>,
    Query(query): Query<EnvironmentQuery>,
) -> Response<Body> {
    let _guard = match state.environment_lock.lock() {
        Ok(value) => value,
        Err(_) => return gateway_error("environment lock is unavailable"),
    };
    match environment_config::read_public(&state.environment_path, &query.root) {
        Ok(profile) => json_response(StatusCode::OK, &profile),
        Err(message) => json_error(StatusCode::BAD_REQUEST, &message),
    }
}

async fn put_environment_profile(
    State(state): State<GatewayState>,
    Json(request): Json<environment_config::UpdateEnvironmentProfile>,
) -> Response<Body> {
    let _guard = match state.environment_lock.lock() {
        Ok(value) => value,
        Err(_) => return gateway_error("environment lock is unavailable"),
    };
    match environment_config::update(&state.environment_path, request) {
        Ok(profile) => json_response(StatusCode::OK, &profile),
        Err(message) => json_error(StatusCode::BAD_REQUEST, &message),
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApplyEnvironmentRequest {
    root: String,
    thread_id: String,
    backend: String,
    #[serde(default)]
    include_thread: bool,
    #[serde(default)]
    exclude_turns: bool,
    #[serde(default)]
    initial_turns_page: Option<serde_json::Value>,
}

async fn apply_environment_profile(
    State(state): State<GatewayState>,
    Json(request): Json<ApplyEnvironmentRequest>,
) -> Response<Body> {
    let environment = {
        let _guard = match state.environment_lock.lock() {
            Ok(value) => value,
            Err(_) => return gateway_error("environment lock is unavailable"),
        };
        match environment_config::app_server_environment(&state.environment_path, &request.root) {
            Ok(value) => value,
            Err(message) => return json_error(StatusCode::BAD_REQUEST, &message),
        }
    };
    let Some(instance) = state.codex_backends.get(&request.backend) else {
        return json_error(
            StatusCode::BAD_REQUEST,
            "environment backend must be a configured Codex-compatible instance",
        );
    };
    let mut resume_params = json!({
        "threadId": request.thread_id,
        "config": { "shell_environment_policy": { "inherit": "all", "set": environment } }
    });
    if request.exclude_turns {
        resume_params["excludeTurns"] = json!(true);
    }
    if let Some(initial_turns_page) = request.initial_turns_page {
        resume_params["initialTurnsPage"] = initial_turns_page;
    }
    match instance
        .server
        .request("thread/resume", resume_params)
        .await
    {
        Ok(result) if request.include_thread => json_response(StatusCode::OK, &result),
        Ok(_) => json_response(StatusCode::OK, &json!({ "applied": true })),
        Err(message) => json_error(StatusCode::BAD_GATEWAY, &message),
    }
}

async fn get_preferences(State(state): State<GatewayState>) -> Response<Body> {
    let _guard = match state.preferences_lock.lock() {
        Ok(guard) => guard,
        Err(_) => return gateway_error("preferences lock is unavailable"),
    };
    match load_preferences(&state.preferences_path) {
        Ok(preferences) => json_response(StatusCode::OK, &preferences),
        Err(error) => gateway_error(&format!("failed to read Studio preferences: {error}")),
    }
}

// SQLite (including its busy timeout and the shared database mutex) must not
// block the async workers that deliver session notifications and HTTP streams.
async fn run_studio_database(
    operation: impl FnOnce() -> Response<Body> + Send + 'static,
) -> Response<Body> {
    match tokio::task::spawn_blocking(operation).await {
        Ok(response) => response,
        Err(error) => gateway_error(&format!("Studio database task failed: {error}")),
    }
}

async fn get_session_state(State(state): State<GatewayState>) -> Response<Body> {
    run_studio_database(move || {
        let _guard = match state.studio_lock.lock() {
            Ok(guard) => guard,
            Err(_) => return gateway_error("Studio database lock is unavailable"),
        };
        match session_state::load(&state.studio_path) {
            Ok(snapshot) => json_response(StatusCode::OK, &snapshot),
            Err(error) => gateway_error(&format!("failed to read session state: {error}")),
        }
    })
    .await
}

async fn put_annotation_state(State(state): State<GatewayState>, body: String) -> Response<Body> {
    run_studio_database(move || {
        let request = match parse_session_state_body::<AnnotationStateRequest>(&body) {
            Ok(request) => request,
            Err(response) => return response,
        };
        if request.session_key.is_empty()
            || request.session_key.len() > 320
            || !valid_router_session_key(&request.session_key)
        {
            return json_error(StatusCode::BAD_REQUEST, "session state key is invalid");
        }
        let mut preferences = StudioPreferences::default();
        preferences
            .annotation_drafts
            .insert(request.session_key.clone(), request.drafts.clone());
        if !request.additional.is_empty() {
            preferences
                .annotation_additional
                .insert(request.session_key.clone(), request.additional.clone());
        }
        if let Err(message) = validate_preferences(&preferences) {
            return json_error(StatusCode::BAD_REQUEST, &message);
        }
        let _guard = match state.studio_lock.lock() {
            Ok(guard) => guard,
            Err(_) => return gateway_error("Studio database lock is unavailable"),
        };
        match session_state::replace_annotations(
            &state.studio_path,
            &request.session_key,
            &request.drafts,
            &request.additional,
        ) {
            Ok(()) => StatusCode::NO_CONTENT.into_response(),
            Err(error) => gateway_error(&format!("failed to save annotation state: {error}")),
        }
    })
    .await
}

async fn put_opening_message_state(
    State(state): State<GatewayState>,
    body: String,
) -> Response<Body> {
    run_studio_database(move || {
        let request = match parse_session_state_body::<OpeningMessageStateRequest>(&body) {
            Ok(request) => request,
            Err(response) => return response,
        };
        if request.session_key.is_empty()
            || request.session_key.len() > 320
            || !valid_router_session_key(&request.session_key)
        {
            return json_error(StatusCode::BAD_REQUEST, "session state key is invalid");
        }
        let mut preferences = StudioPreferences::default();
        if let Some(message) = &request.message {
            preferences
                .opening_messages
                .insert(request.session_key.clone(), message.clone());
        }
        if let Err(message) = validate_preferences(&preferences) {
            return json_error(StatusCode::BAD_REQUEST, &message);
        }
        let _guard = match state.studio_lock.lock() {
            Ok(guard) => guard,
            Err(_) => return gateway_error("Studio database lock is unavailable"),
        };
        match session_state::put_opening_message(
            &state.studio_path,
            &request.session_key,
            request.message.as_ref(),
        ) {
            Ok(()) => StatusCode::NO_CONTENT.into_response(),
            Err(error) => gateway_error(&format!("failed to save opening message: {error}")),
        }
    })
    .await
}

async fn delete_session_state(State(state): State<GatewayState>, body: String) -> Response<Body> {
    run_studio_database(move || {
        let request = match parse_session_state_body::<DeleteSessionStateRequest>(&body) {
            Ok(request) => request,
            Err(response) => return response,
        };
        if request.session_key.is_empty()
            || request.session_key.len() > 320
            || !valid_router_session_key(&request.session_key)
        {
            return json_error(StatusCode::BAD_REQUEST, "session state key is invalid");
        }
        let _guard = match state.studio_lock.lock() {
            Ok(guard) => guard,
            Err(_) => return gateway_error("Studio database lock is unavailable"),
        };
        match session_state::delete_session(&state.studio_path, &request.session_key) {
            Ok(()) => StatusCode::NO_CONTENT.into_response(),
            Err(error) => gateway_error(&format!("failed to delete session state: {error}")),
        }
    })
    .await
}

async fn put_session_pin(State(state): State<GatewayState>, body: String) -> Response<Body> {
    run_studio_database(move || {
        let request = match parse_session_state_body::<PinSessionStateRequest>(&body) {
            Ok(request) => request,
            Err(response) => return response,
        };
        if request.session_key.is_empty()
            || request.session_key.len() > 320
            || !valid_router_session_key(&request.session_key)
        {
            return json_error(StatusCode::BAD_REQUEST, "session state key is invalid");
        }
        let _guard = match state.studio_lock.lock() {
            Ok(guard) => guard,
            Err(_) => return gateway_error("Studio database lock is unavailable"),
        };
        match session_state::set_pinned(&state.studio_path, &request.session_key, request.pinned) {
            Ok(pinned_sessions) => json_response(
                StatusCode::OK,
                &json!({ "pinnedSessions": pinned_sessions }),
            ),
            Err(error) if error == session_state::PIN_LIMIT_ERROR => {
                json_error(StatusCode::BAD_REQUEST, &error)
            }
            Err(error) => gateway_error(&format!("failed to save pinned session: {error}")),
        }
    })
    .await
}

async fn put_turn_options_state(State(state): State<GatewayState>, body: String) -> Response<Body> {
    run_studio_database(move || {
        let request = match parse_session_state_body::<TurnOptionsStateRequest>(&body) {
            Ok(request) => request,
            Err(response) => return response,
        };
        if request.session_key.is_empty()
            || request.session_key.len() > 320
            || !valid_router_session_key(&request.session_key)
            || (!request.model.is_empty() && !valid_runtime_value(&request.model, 256))
            || (!request.effort.is_empty() && !valid_runtime_value(&request.effort, 64))
        {
            return json_error(StatusCode::BAD_REQUEST, "session turn options are invalid");
        }
        let _guard = match state.studio_lock.lock() {
            Ok(guard) => guard,
            Err(_) => return gateway_error("Studio database lock is unavailable"),
        };
        match session_state::put_turn_options(
            &state.studio_path,
            &request.session_key,
            &request.model,
            &request.effort,
        ) {
            Ok(()) => StatusCode::NO_CONTENT.into_response(),
            Err(error) => gateway_error(&format!("failed to save session turn options: {error}")),
        }
    })
    .await
}

async fn put_message_queue_state(
    State(state): State<GatewayState>,
    body: String,
) -> Response<Body> {
    run_studio_database(move || {
        let request = match parse_session_state_body::<MessageQueueStateRequest>(&body) {
            Ok(request) => request,
            Err(response) => return response,
        };
        let invalid_message = request.messages.len() > 3
            || request.messages.iter().any(|message| {
                message.id.is_empty()
                    || !valid_runtime_value(&message.id, 128)
                    || message.text.len() > 256 * 1024
                    || message.input.len() > 32
                    || message.input.iter().any(|item| {
                        item.get("type").and_then(|value| value.as_str()) == Some("text")
                    })
            });
        if request.session_key.is_empty()
            || request.session_key.len() > 320
            || !valid_router_session_key(&request.session_key)
            || invalid_message
        {
            return json_error(StatusCode::BAD_REQUEST, "session message queue is invalid");
        }
        let _guard = match state.studio_lock.lock() {
            Ok(guard) => guard,
            Err(_) => return gateway_error("Studio database lock is unavailable"),
        };
        match session_state::replace_message_queue(
            &state.studio_path,
            &request.session_key,
            &request.messages,
        ) {
            Ok(()) => StatusCode::NO_CONTENT.into_response(),
            Err(error) => gateway_error(&format!("failed to save message queue: {error}")),
        }
    })
    .await
}

fn parse_session_state_body<T: for<'de> Deserialize<'de>>(body: &str) -> Result<T, Response<Body>> {
    if body.len() > MAX_PREFERENCES_BODY {
        return Err(json_error(
            StatusCode::PAYLOAD_TOO_LARGE,
            "session state payload is too large",
        ));
    }
    serde_json::from_str(body).map_err(|error| {
        json_error(
            StatusCode::BAD_REQUEST,
            &format!("invalid session state: {error}"),
        )
    })
}

fn shared_document_directories(state: &GatewayState) -> Result<Vec<PathBuf>, (StatusCode, String)> {
    let configured = configured_shared_document_directories(state)?;
    Ok(configured
        .iter()
        .filter_map(|directory| fs::canonicalize(directory).ok())
        .filter(|directory| directory.is_dir())
        .collect())
}

fn configured_shared_document_directories(
    state: &GatewayState,
) -> Result<Vec<String>, (StatusCode, String)> {
    state
        .shared_document_directories
        .lock()
        .map(|value| value.clone())
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "shared document directory lock is unavailable".to_string(),
            )
        })
}

async fn put_preferences(State(state): State<GatewayState>, body: String) -> Response<Body> {
    if body.len() > MAX_PREFERENCES_BODY {
        return json_error(
            StatusCode::PAYLOAD_TOO_LARGE,
            "preferences payload is too large",
        );
    }
    let preferences = match serde_json::from_str::<StudioPreferences>(&body) {
        Ok(preferences) => preferences,
        Err(error) => {
            return json_error(
                StatusCode::BAD_REQUEST,
                &format!("invalid preferences: {error}"),
            )
        }
    };
    if let Err(message) = validate_preferences(&preferences) {
        return json_error(StatusCode::BAD_REQUEST, &message);
    }
    let _guard = match state.preferences_lock.lock() {
        Ok(guard) => guard,
        Err(_) => return gateway_error("preferences lock is unavailable"),
    };
    match save_preferences(&state.preferences_path, &preferences) {
        Ok(()) => {
            let Ok(mut shared_directories) = state.shared_document_directories.lock() else {
                return gateway_error("shared document directory lock is unavailable");
            };
            *shared_directories = preferences.shared_document_directories.clone();
            json_response(StatusCode::OK, &preferences)
        }
        Err(error) => gateway_error(&format!("failed to save Studio preferences: {error}")),
    }
}

async fn client_log(body: String) -> StatusCode {
    let message = body.chars().take(16 * 1024).collect::<String>();
    eprintln!("Studio WebView: {message}");
    StatusCode::NO_CONTENT
}

async fn list_favorites(
    State(state): State<GatewayState>,
    Query(query): Query<FavoriteQuery>,
) -> Response<Body> {
    run_studio_database(move || {
        let _guard = match state.studio_lock.lock() {
            Ok(guard) => guard,
            Err(_) => return gateway_error("favorites lock is unavailable"),
        };
        match favorites::list(&state.studio_path, &query.q, query.limit.unwrap_or(100)) {
            Ok(items) => json_response(StatusCode::OK, &items),
            Err(error) => gateway_error(&format!("failed to read favorites: {error}")),
        }
    })
    .await
}

async fn get_favorite(
    State(state): State<GatewayState>,
    AxumPath(id): AxumPath<String>,
) -> Response<Body> {
    run_studio_database(move || {
        let _guard = match state.studio_lock.lock() {
            Ok(guard) => guard,
            Err(_) => return gateway_error("favorites lock is unavailable"),
        };
        match favorites::find(&state.studio_path, &id) {
            Ok(Some(favorite)) => json_response(StatusCode::OK, &favorite),
            Ok(None) => json_error(StatusCode::NOT_FOUND, "favorite not found"),
            Err(error) => gateway_error(&format!("failed to read favorites: {error}")),
        }
    })
    .await
}

async fn create_favorite(State(state): State<GatewayState>, body: String) -> Response<Body> {
    run_studio_database(move || {
        let favorite = match parse_favorite_body(&body) {
            Ok(favorite) => favorite,
            Err((status, message)) => return json_error(status, &message),
        };
        let _guard = match state.studio_lock.lock() {
            Ok(guard) => guard,
            Err(_) => return gateway_error("favorites lock is unavailable"),
        };
        match favorites::insert(&state.studio_path, favorite) {
            Ok(favorite) => json_response(StatusCode::CREATED, &favorite),
            Err(error) => json_error(StatusCode::BAD_REQUEST, &error),
        }
    })
    .await
}

async fn update_favorite(
    State(state): State<GatewayState>,
    AxumPath(id): AxumPath<String>,
    body: String,
) -> Response<Body> {
    run_studio_database(move || {
        let favorite = match parse_favorite_body(&body) {
            Ok(favorite) => favorite,
            Err((status, message)) => return json_error(status, &message),
        };
        let _guard = match state.studio_lock.lock() {
            Ok(guard) => guard,
            Err(_) => return gateway_error("favorites lock is unavailable"),
        };
        match favorites::update(&state.studio_path, &id, favorite) {
            Ok(Some(favorite)) => json_response(StatusCode::OK, &favorite),
            Ok(None) => json_error(StatusCode::NOT_FOUND, "favorite not found"),
            Err(error) => json_error(StatusCode::BAD_REQUEST, &error),
        }
    })
    .await
}

async fn delete_favorite(
    State(state): State<GatewayState>,
    AxumPath(id): AxumPath<String>,
) -> Response<Body> {
    run_studio_database(move || {
        let _guard = match state.studio_lock.lock() {
            Ok(guard) => guard,
            Err(_) => return gateway_error("favorites lock is unavailable"),
        };
        match favorites::remove(&state.studio_path, &id) {
            Ok(Some(favorite)) => json_response(StatusCode::OK, &favorite),
            Ok(None) => json_error(StatusCode::NOT_FOUND, "favorite not found"),
            Err(error) => gateway_error(&format!("failed to save favorites: {error}")),
        }
    })
    .await
}

async fn export_favorites(State(state): State<GatewayState>) -> Response<Body> {
    run_studio_database(move || {
        let _guard = match state.studio_lock.lock() {
            Ok(guard) => guard,
            Err(_) => return gateway_error("favorites lock is unavailable"),
        };
        match favorites::export_markdown(&state.studio_path) {
            Ok(markdown) => Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "text/markdown; charset=utf-8")
                .header(
                    header::CONTENT_DISPOSITION,
                    "attachment; filename=\"codex-thread-studio-favorites.md\"",
                )
                .body(Body::from(markdown))
                .expect("valid favorites export response"),
            Err(error) => gateway_error(&format!("failed to export favorites: {error}")),
        }
    })
    .await
}

async fn get_session_map(
    State(state): State<GatewayState>,
    AxumPath((backend, thread_id)): AxumPath<(String, String)>,
) -> Response<Body> {
    let _guard = match state.session_maps_lock.lock() {
        Ok(guard) => guard,
        Err(_) => return gateway_error("session map lock is unavailable"),
    };
    match session_map::find(&state.session_maps_path, &backend, &thread_id) {
        Ok(Some(map)) => json_response(StatusCode::OK, &map),
        Ok(None) => json_error(StatusCode::NOT_FOUND, "session map not found"),
        Err(error) => json_error(StatusCode::BAD_REQUEST, &error),
    }
}

async fn create_session_map(State(state): State<GatewayState>, body: String) -> Response<Body> {
    let request = match parse_map_body::<CreateMapRequest>(&body) {
        Ok(request) => request,
        Err((status, message)) => return json_error(status, &message),
    };
    let _guard = match state.session_maps_lock.lock() {
        Ok(guard) => guard,
        Err(_) => return gateway_error("session map lock is unavailable"),
    };
    match session_map::create(&state.session_maps_path, request) {
        Ok(map) => json_response(StatusCode::CREATED, &map),
        Err(error) if error.contains("already has a map") => {
            json_error(StatusCode::CONFLICT, &error)
        }
        Err(error) => json_error(StatusCode::BAD_REQUEST, &error),
    }
}

async fn apply_session_map_operations(
    State(state): State<GatewayState>,
    AxumPath((backend, thread_id)): AxumPath<(String, String)>,
    body: String,
) -> Response<Body> {
    let request = match parse_map_body::<ApplyOperationsRequest>(&body) {
        Ok(request) => request,
        Err((status, message)) => return json_error(status, &message),
    };
    let _guard = match state.session_maps_lock.lock() {
        Ok(guard) => guard,
        Err(_) => return gateway_error("session map lock is unavailable"),
    };
    match session_map::apply_operations(&state.session_maps_path, &backend, &thread_id, request) {
        Ok(Some(map)) => json_response(StatusCode::OK, &map),
        Ok(None) => json_error(StatusCode::NOT_FOUND, "session map not found"),
        Err(error) if error.contains("revision conflict") => {
            json_error(StatusCode::CONFLICT, &error)
        }
        Err(error) => json_error(StatusCode::BAD_REQUEST, &error),
    }
}

async fn undo_session_map(
    State(state): State<GatewayState>,
    AxumPath((backend, thread_id)): AxumPath<(String, String)>,
) -> Response<Body> {
    let _guard = match state.session_maps_lock.lock() {
        Ok(guard) => guard,
        Err(_) => return gateway_error("session map lock is unavailable"),
    };
    match session_map::undo(&state.session_maps_path, &backend, &thread_id) {
        Ok(Some(map)) => json_response(StatusCode::OK, &map),
        Ok(None) => json_error(StatusCode::NOT_FOUND, "session map not found"),
        Err(error) if error.contains("no map change") => json_error(StatusCode::CONFLICT, &error),
        Err(error) => json_error(StatusCode::BAD_REQUEST, &error),
    }
}

async fn delete_session_map(
    State(state): State<GatewayState>,
    AxumPath((backend, thread_id)): AxumPath<(String, String)>,
) -> Response<Body> {
    let _guard = match state.session_maps_lock.lock() {
        Ok(guard) => guard,
        Err(_) => return gateway_error("session map lock is unavailable"),
    };
    match session_map::remove(&state.session_maps_path, &backend, &thread_id) {
        Ok(Some(map)) => json_response(StatusCode::OK, &map),
        Ok(None) => json_error(StatusCode::NOT_FOUND, "session map not found"),
        Err(error) => json_error(StatusCode::BAD_REQUEST, &error),
    }
}

fn parse_map_body<T: for<'de> Deserialize<'de>>(body: &str) -> Result<T, (StatusCode, String)> {
    if body.len() > MAX_MAP_BODY_BYTES {
        return Err((
            StatusCode::PAYLOAD_TOO_LARGE,
            "session map payload is too large".to_string(),
        ));
    }
    serde_json::from_str(body).map_err(|error| {
        (
            StatusCode::BAD_REQUEST,
            format!("invalid session map payload: {error}"),
        )
    })
}

fn parse_favorite_body(body: &str) -> Result<Favorite, (StatusCode, String)> {
    if body.len() > MAX_FAVORITE_BODY_BYTES {
        return Err((
            StatusCode::PAYLOAD_TOO_LARGE,
            "favorite payload is too large".to_string(),
        ));
    }
    serde_json::from_str(body).map_err(|error| {
        (
            StatusCode::BAD_REQUEST,
            format!("invalid favorite payload: {error}"),
        )
    })
}

fn studio_preferences_path() -> PathBuf {
    if let Some(path) = env::var_os("XDG_CONFIG_HOME").filter(|path| !path.is_empty()) {
        return PathBuf::from(path)
            .join("codex-thread-studio")
            .join("settings.json");
    }
    if let Some(home) = env::var_os("HOME") {
        return PathBuf::from(home).join(".config/codex-thread-studio/settings.json");
    }
    env::temp_dir().join("codex-thread-studio-settings.json")
}

fn studio_router_workspace_path() -> PathBuf {
    if let Some(path) = env::var_os("XDG_DATA_HOME").filter(|path| !path.is_empty()) {
        return PathBuf::from(path).join("codex-thread-studio/router");
    }
    if let Some(home) = env::var_os("HOME") {
        return PathBuf::from(home).join(".local/share/codex-thread-studio/router");
    }
    env::temp_dir().join("codex-thread-studio-router")
}

fn load_preferences(path: &std::path::Path) -> Result<StudioPreferences, String> {
    match fs::read(path) {
        Ok(data) => serde_json::from_slice(&data).map_err(|error| error.to_string()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok(StudioPreferences::default())
        }
        Err(error) => Err(error.to_string()),
    }
}

fn save_preferences(path: &std::path::Path, preferences: &StudioPreferences) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or("preferences path has no parent directory")?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let data = serde_json::to_vec_pretty(preferences).map_err(|error| error.to_string())?;
    let temporary = path.with_extension(format!("json.tmp-{}", std::process::id()));
    fs::write(&temporary, data).map_err(|error| error.to_string())?;
    fs::rename(&temporary, path).map_err(|error| error.to_string())
}

fn validate_preferences(preferences: &StudioPreferences) -> Result<(), String> {
    preferences.browser.validate()?;
    if preferences
        .queue_depth
        .is_some_and(|depth| !(1..=3).contains(&depth))
    {
        return Err("queue depth must be between 1 and 3".to_string());
    }
    if preferences
        .continue_behavior
        .as_deref()
        .is_some_and(|behavior| {
            !matches!(behavior, "sessionModelDraft" | "ollamaDraft" | "quickSend")
        })
    {
        return Err(
            "Continue behavior must be sessionModelDraft, ollamaDraft, or quickSend".to_string(),
        );
    }
    if preferences
        .language
        .as_deref()
        .is_some_and(|language| !matches!(language, "system" | "zh-CN" | "en-US"))
    {
        return Err("language must be system, zh-CN, or en-US".to_string());
    }
    if preferences
        .theme
        .as_deref()
        .is_some_and(|theme| !matches!(theme, "light" | "dark"))
    {
        return Err("theme must be light or dark".to_string());
    }
    if preferences
        .content_width
        .as_deref()
        .is_some_and(|width| !matches!(width, "comfortable" | "wide" | "full"))
    {
        return Err("content width must be comfortable, wide, or full".to_string());
    }
    if preferences.hidden_session_directories.len() > 256
        || preferences
            .hidden_session_directories
            .iter()
            .any(|path| !valid_session_directory(path))
    {
        return Err("hidden session directories are invalid".to_string());
    }
    if preferences.shared_document_directories.len() > 256
        || preferences
            .shared_document_directories
            .iter()
            .any(|path| !valid_shared_document_directory(path))
    {
        return Err("shared document directories must be absolute local paths".to_string());
    }
    if preferences.session_directory_ignore.len() > 512
        || preferences
            .session_directory_ignore
            .iter()
            .any(|pattern| pattern.len() > 4096 || pattern.chars().any(char::is_control))
    {
        return Err("session directory ignore rules are invalid".to_string());
    }
    if !matches!(
        preferences.mermaid.style.as_str(),
        "auto" | "classic" | "neo" | "handDrawn" | "document"
    ) || !matches!(
        preferences.mermaid.density.as_str(),
        "compact" | "standard" | "loose"
    ) || !matches!(
        preferences.mermaid.curve.as_str(),
        "rounded" | "linear" | "step" | "basis"
    ) || !matches!(
        preferences.mermaid.layout.as_str(),
        "auto" | "dagre" | "elk"
    ) || !(12..=20).contains(&preferences.mermaid.font_size)
    {
        return Err("Mermaid rendering settings are invalid".to_string());
    }
    if !matches!(
        preferences.markdown.mode.as_str(),
        "reading" | "technical" | "compact"
    ) {
        return Err("Markdown mode must be reading, technical, or compact".to_string());
    }
    if !matches!(
        preferences.translation.engine.as_str(),
        "backend" | "ollama"
    ) || !valid_runtime_value(&preferences.translation.ollama_model, 256)
        || preferences.translation.models.len() > 32
        || preferences.translation.efforts.len() > 32
        || preferences
            .translation
            .models
            .iter()
            .any(|(backend, model)| {
                !valid_runtime_value(backend, 64) || !valid_runtime_value(model, 256)
            })
        || preferences
            .translation
            .efforts
            .iter()
            .any(|(backend, effort)| {
                !valid_runtime_value(backend, 64)
                    || !matches!(
                        effort.as_str(),
                        "minimal" | "low" | "medium" | "high" | "xhigh"
                    )
            })
    {
        return Err("translation model or reasoning effort settings are invalid".to_string());
    }
    if preferences
        .wsl_distribution
        .as_ref()
        .is_some_and(|value| !valid_runtime_value(value, 128))
        || preferences
            .wsl_user
            .as_ref()
            .is_some_and(|value| !valid_runtime_value(value, 128))
        || preferences
            .wsl_codex_binary
            .as_ref()
            .is_some_and(|value| !valid_runtime_value(value, 4096))
        || preferences
            .wsl_opencode_binary
            .as_ref()
            .is_some_and(|value| !valid_runtime_value(value, 4096))
    {
        return Err("WSL backend settings are invalid".to_string());
    }
    if preferences
        .right_rail_width_ratio
        .or(preferences.artifact_width_ratio)
        .is_some_and(|ratio| !(0.2..=0.65).contains(&ratio))
    {
        return Err("right rail width ratio must be between 0.2 and 0.65".to_string());
    }
    if preferences.thread_activity.len() > 2048
        || preferences
            .thread_activity
            .keys()
            .any(|key| key.len() > 321 || !valid_router_session_key(key))
    {
        return Err("thread activity preferences are invalid".to_string());
    }
    if preferences.attention_threads.len() > 2048
        || preferences
            .attention_threads
            .iter()
            .any(|key| key.len() > 321 || !valid_router_session_key(key))
    {
        return Err("attention thread preferences are invalid".to_string());
    }
    if let Some(typography) = &preferences.typography {
        if typography.ui_font_family.trim().is_empty()
            || typography.ui_font_family.len() > 512
            || !(11.0..=20.0).contains(&typography.ui_font_size)
            || typography.code_font_family.trim().is_empty()
            || typography.code_font_family.len() > 512
            || typography.workspace_font_family.trim().is_empty()
            || typography.workspace_font_family.len() > 512
            || ![400, 500, 600].contains(&typography.ui_font_weight)
            || ![400, 500, 600].contains(&typography.code_font_weight)
            || typography
                .content_font_family
                .as_ref()
                .is_some_and(|family| family.trim().is_empty() || family.len() > 512)
            || typography
                .content_font_size
                .is_some_and(|size| !(11.0..=24.0).contains(&size))
            || typography
                .content_font_weight
                .is_some_and(|weight| ![400, 500, 600].contains(&weight))
            || !(11.0..=20.0).contains(&typography.workspace_font_size)
            || !(11.0..=20.0).contains(&typography.code_font_size)
        {
            return Err("typography settings are invalid".to_string());
        }
    }
    let draft_count: usize = preferences.annotation_drafts.values().map(Vec::len).sum();
    if preferences.annotation_drafts.len() > 128
        || draft_count > 256
        || preferences
            .annotation_drafts
            .iter()
            .any(|(thread_id, drafts)| {
                thread_id.len() > 256
                    || drafts.len() > 32
                    || drafts.iter().any(|draft| {
                        draft.id.len() > 128
                            || draft.excerpt.len() > 16 * 1024
                            || draft.note.len() > 16 * 1024
                            || draft.quote.len() > 16 * 1024
                            || draft.comment.len() > 16 * 1024
                            || (draft.excerpt.is_empty() && draft.quote.is_empty())
                            || draft.created_at.len() > 128
                            || draft.source.as_ref().is_some_and(|source| {
                                source.provider.is_empty()
                                    || source.provider.len() > 64
                                    || source.version == 0
                                    || serde_json::to_vec(&source.anchor)
                                        .map_or(true, |anchor| anchor.len() > 32 * 1024)
                            })
                            || draft
                                .item_id
                                .as_ref()
                                .is_some_and(|value| value.len() > 256)
                            || draft
                                .turn_id
                                .as_ref()
                                .is_some_and(|value| value.len() > 256)
                            || draft.target.as_ref().is_some_and(|target| {
                                !matches!(target.kind.as_str(), "chatRange" | "fileRange")
                                    || target
                                        .file_path
                                        .as_ref()
                                        .is_some_and(|value| value.len() > 4096)
                                    || target.root.as_ref().is_some_and(|value| value.len() > 4096)
                                    || target
                                        .base_hash
                                        .as_ref()
                                        .is_some_and(|value| value.len() > 128)
                                    || target
                                        .prefix
                                        .as_ref()
                                        .is_some_and(|value| value.len() > 256)
                                    || target
                                        .suffix
                                        .as_ref()
                                        .is_some_and(|value| value.len() > 256)
                                    || (target.kind == "fileRange"
                                        && match (target.start_offset, target.end_offset) {
                                            (None, None) => false,
                                            (Some(start), Some(end)) => end <= start,
                                            _ => true,
                                        })
                                    || (target.kind == "fileRange"
                                        && match (target.start_line, target.end_line) {
                                            (None, None) => false,
                                            (Some(start), Some(end)) => start == 0 || end < start,
                                            _ => true,
                                        })
                                    || (target.kind == "fileRange"
                                        && target
                                            .file_path
                                            .as_deref()
                                            .unwrap_or_default()
                                            .is_empty())
                            })
                    })
            })
    {
        return Err("annotation drafts are too large".to_string());
    }
    if preferences.annotation_additional.len() > 128
        || preferences
            .annotation_additional
            .iter()
            .any(|(thread_id, value)| thread_id.len() > 256 || value.len() > 32 * 1024)
    {
        return Err("annotation additional text is too large".to_string());
    }
    if preferences.annotation_prompt_templates.len() > 2
        || preferences
            .annotation_prompt_templates
            .iter()
            .any(|(language, template)| {
                !matches!(language.as_str(), "zh-CN" | "en-US")
                    || template.len() > 32 * 1024
                    || !template.contains("{{annotations}}")
            })
    {
        return Err("localized annotation templates are invalid".to_string());
    }
    if preferences.opening_messages.len() > 2048
        || preferences.opening_messages.iter().any(|(id, message)| {
            id.is_empty()
                || id.len() > 320
                || message.text.len() > 16 * 1024
                || message.responsibility.chars().count() > 4096
                || message.source.len() > 64
                || message.captured_at.len() > 128
        })
    {
        return Err("opening message preferences are invalid".to_string());
    }
    if let Some(router) = &preferences.router {
        let mut controller_keys = router
            .controllers
            .iter()
            .map(|(backend, id)| format!("{backend}:{id}"))
            .collect::<BTreeSet<_>>();
        if let Some(id) = &router.thread_id {
            controller_keys.insert(format!("codex:{id}"));
        }
        if router
            .thread_id
            .as_ref()
            .is_some_and(|id| id.is_empty() || id.len() > 256)
            || router
                .controller_backend
                .as_ref()
                .is_some_and(|backend| !valid_router_backend(backend))
            || router.controllers.len() > 64
            || router.controllers.iter().any(|(backend, id)| {
                !valid_router_backend(backend) || id.is_empty() || id.len() > 256
            })
            || router.responsibilities.len() > 2048
            || router.responsibilities.iter().any(|(key, responsibility)| {
                key.is_empty()
                    || key.len() > 320
                    || controller_keys.contains(key)
                    || (key.contains(':') && !valid_router_session_key(key))
                    || router.thread_id.as_ref() == Some(key)
                    || responsibility.description.chars().count() > 4096
                    || !matches!(responsibility.fallback.as_str(), "none" | "fallback")
            })
            || router.fallbacks.len() > 3
            || router.fallbacks.iter().any(|fallback| {
                fallback.session_key.is_empty()
                    || fallback.session_key.len() > 320
                    || !valid_router_session_key(&fallback.session_key)
                    || controller_keys.contains(&fallback.session_key)
                    || fallback.condition.is_empty()
                    || fallback.condition.chars().count() > 4096
            })
            || router
                .fallbacks
                .iter()
                .map(|fallback| &fallback.session_key)
                .collect::<BTreeSet<_>>()
                .len()
                != router.fallbacks.len()
        {
            return Err("thread router preferences are invalid".to_string());
        }
    }
    Ok(())
}

fn valid_router_backend(value: &str) -> bool {
    backend_config::valid_backend_id(value)
}

fn valid_router_session_key(value: &str) -> bool {
    value.split_once(':').is_some_and(|(backend, id)| {
        valid_router_backend(backend) && !id.is_empty() && id.len() <= 256
    })
}

fn valid_runtime_value(value: &str, max_len: usize) -> bool {
    let value = value.trim();
    !value.is_empty()
        && value.len() <= max_len
        && !value.starts_with('-')
        && !value.chars().any(|character| character.is_control())
}

fn valid_session_directory(value: &str) -> bool {
    let value = value.trim();
    !value.is_empty()
        && value.len() <= 4096
        && !value.chars().any(|character| character.is_control())
}

fn valid_shared_document_directory(value: &str) -> bool {
    let value = value.trim();
    !value.is_empty()
        && value.len() <= 4096
        && !value.chars().any(|character| character.is_control())
        && (PathBuf::from(value).is_absolute() || (cfg!(windows) && value.starts_with('/')))
}

fn backend_configuration(
    preferences: &StudioPreferences,
    cli_path: OsString,
) -> (String, String, BackendRuntime) {
    let wsl = WslSettings {
        distribution: preferences
            .wsl_distribution
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
        user: preferences
            .wsl_user
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
    };

    #[cfg(windows)]
    let binaries = (
        preferences
            .wsl_codex_binary
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("codex")
            .to_string(),
        preferences
            .wsl_opencode_binary
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("opencode")
            .to_string(),
    );
    #[cfg(not(windows))]
    let binaries = (
        find_codex_binary(&cli_path),
        find_opencode_binary(&cli_path),
    );

    (binaries.0, binaries.1, BackendRuntime::new(cli_path, wsl))
}

fn build_backend_registry(
    codex_binary: &str,
    codex: CodexAppServer,
    configured: Vec<ConfiguredCodexBackend>,
    runtime: BackendRuntime,
) -> (
    BTreeMap<String, CodexBackendInstance>,
    Vec<BackendDescriptor>,
) {
    let mut builtins = backend_config::builtin_descriptors();
    let opencode = builtins.pop().expect("OpenCode descriptor");
    let codex_descriptor = builtins.pop().expect("Codex descriptor");
    let mut instances = BTreeMap::new();
    instances.insert(
        "codex".to_string(),
        CodexBackendInstance {
            command_line: codex_binary.to_string(),
            server: codex,
        },
    );
    let mut descriptors = vec![codex_descriptor];
    for backend in configured {
        let command_line = display_command(&backend.command, &backend.args);
        let server = CodexAppServer::with_prefix(backend.command, backend.args, runtime.clone());
        descriptors.push(backend.descriptor.clone());
        instances.insert(
            backend.descriptor.id.clone(),
            CodexBackendInstance {
                command_line,
                server,
            },
        );
    }
    descriptors.push(opencode);
    (instances, descriptors)
}

fn display_command(command: &str, args: &[String]) -> String {
    std::iter::once(command)
        .chain(args.iter().map(String::as_str))
        .map(|part| {
            if part.contains(char::is_whitespace) {
                format!("{part:?}")
            } else {
                part.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn augmented_cli_path() -> OsString {
    let original = env::var_os("PATH").unwrap_or_default();
    let mut paths = Vec::<PathBuf>::new();
    let mut add = |path: PathBuf| {
        if path.is_dir() && !paths.contains(&path) {
            paths.push(path);
        }
    };

    if let Some(path) = env::var_os("NVM_BIN") {
        add(PathBuf::from(path));
    }
    if let Some(path) = env::var_os("FNM_MULTISHELL_PATH") {
        let path = PathBuf::from(path);
        let bin = path.join("bin");
        add(if bin.is_dir() { bin } else { path });
    }
    if let Some(home) = env::var_os("HOME").map(PathBuf::from) {
        add(home.join(".local/bin"));
        add(home.join(".cargo/bin"));
        if let Ok(entries) = fs::read_dir(home.join(".nvm/versions/node")) {
            let mut versions = entries
                .flatten()
                .map(|entry| entry.path())
                .filter(|path| path.join("bin").is_dir())
                .collect::<Vec<_>>();
            versions.sort_by_key(|path| std::cmp::Reverse(node_version_key(path)));
            for version in versions {
                add(version.join("bin"));
            }
        }
    }
    for path in env::split_paths(&original) {
        add(path);
    }
    env::join_paths(paths).unwrap_or(original)
}

fn node_version_key(path: &std::path::Path) -> (u32, u32, u32) {
    let value = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .trim_start_matches('v');
    let mut parts = value.split('.').filter_map(|part| part.parse::<u32>().ok());
    (
        parts.next().unwrap_or_default(),
        parts.next().unwrap_or_default(),
        parts.next().unwrap_or_default(),
    )
}

fn json_response<T: Serialize>(status: StatusCode, value: &T) -> Response<Body> {
    match serde_json::to_vec(value) {
        Ok(body) => Response::builder()
            .status(status)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body))
            .expect("valid JSON response"),
        Err(error) => gateway_error(&format!("failed to encode response: {error}")),
    }
}

fn json_error(status: StatusCode, message: &str) -> Response<Body> {
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({ "error": { "message": message } }).to_string(),
        ))
        .expect("valid error response")
}

fn gateway_error(message: &str) -> Response<Body> {
    json_error(StatusCode::INTERNAL_SERVER_ERROR, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::Request;
    use tower::ServiceExt;

    #[test]
    fn studio_profile_lock_excludes_other_instances_and_releases_on_close() {
        let directory =
            env::temp_dir().join(format!("studio-profile-lock-{}", uuid::Uuid::new_v4()));
        let path = directory.join("settings.json");
        let first = lock_studio_profile(&path).expect("first owner");
        assert!(
            lock_studio_profile(&path).is_err(),
            "second instance must not write preferences"
        );
        let separate =
            lock_studio_profile(&directory.join("other/settings.json")).expect("separate profile");
        drop(separate);
        drop(first);
        let reopened =
            lock_studio_profile(&path).expect("released OS lock does not leave a stale lock");
        drop(reopened);
        fs::remove_dir_all(directory).expect("cleanup");
    }

    #[test]
    fn desktop_and_server_share_current_main_initialization() {
        let source = include_str!("main.rs");
        let initializer = source
            .split("fn initialize_gateway(")
            .nth(1)
            .unwrap()
            .split("fn lock_studio_profile(")
            .next()
            .unwrap();
        assert!(initializer.contains("studio.sqlite3"));
        assert!(initializer.contains("session_state::initialize(&studio_path)"));
        assert!(initializer.contains("shared_document_directories:"));
        assert!(!initializer.contains("favorites.sqlite3"));
        assert!(!initializer.contains("migrate_legacy_preferences"));
        assert!(
            initializer
                .find("lock_studio_profile(&preferences_path)")
                .unwrap()
                < initializer
                    .find("load_preferences(&preferences_path)")
                    .unwrap()
        );
    }

    #[test]
    fn remote_server_options_default_to_a_fixed_loopback_port() {
        assert_eq!(
            parse_serve_options(&[OsString::from("--serve")]),
            Ok(Some(ServeOptions {
                listen: "127.0.0.1:38080".parse().expect("test address"),
            }))
        );
        assert_eq!(
            parse_serve_options(&[
                OsString::from("--serve"),
                OsString::from("--listen"),
                OsString::from("[::1]:45200"),
            ]),
            Ok(Some(ServeOptions {
                listen: "[::1]:45200".parse().expect("test address"),
            }))
        );
    }

    #[test]
    fn remote_server_options_reject_network_exposure_and_unknown_flags() {
        let exposed = parse_serve_options(&[
            OsString::from("--serve"),
            OsString::from("--listen"),
            OsString::from("0.0.0.0:45100"),
        ]);
        assert!(exposed
            .expect_err("non-loopback listener must be rejected")
            .contains("loopback"));

        let unknown = parse_serve_options(&[OsString::from("--serve"), OsString::from("--public")]);
        assert!(unknown
            .expect_err("unknown server flag must be rejected")
            .contains("unsupported option"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn database_work_does_not_block_the_async_worker() {
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let task = tokio::spawn(run_studio_database(move || {
            let _ = started_tx.send(());
            let released = release_rx.recv_timeout(std::time::Duration::from_secs(2));
            assert!(released.is_ok(), "database work blocked the async worker");
            StatusCode::NO_CONTENT.into_response()
        }));
        started_rx.await.expect("database worker starts");
        release_tx
            .send(())
            .expect("async worker remains responsive");
        assert_eq!(
            task.await.expect("database request completes").status(),
            StatusCode::NO_CONTENT
        );
    }
    fn test_codex_backends() -> Arc<BTreeMap<String, CodexBackendInstance>> {
        Arc::new(BTreeMap::from([(
            "codex".to_string(),
            CodexBackendInstance {
                command_line: "codex".to_string(),
                server: CodexAppServer::new(
                    "codex".to_string(),
                    BackendRuntime::new(OsString::new(), WslSettings::default()),
                ),
            },
        )]))
    }

    fn test_backend_descriptors() -> Arc<Vec<BackendDescriptor>> {
        Arc::new(backend_config::builtin_descriptors())
    }

    fn secured_test_gateway(security: GatewaySecurity) -> GatewayState {
        let suffix = uuid::Uuid::new_v4().simple().to_string();
        GatewayState {
            codex: CodexAppServer::new(
                "codex".to_string(),
                BackendRuntime::new(OsString::new(), WslSettings::default()),
            ),
            codex_backends: test_codex_backends(),
            backend_descriptors: test_backend_descriptors(),
            backend_config_path: Arc::new(env::temp_dir().join("backends.json")),
            backend_config_error: Arc::new(None),
            opencode: OpenCodeServer::new(
                "opencode".to_string(),
                BackendRuntime::new(OsString::new(), WslSettings::default()),
            ),
            preferences_path: Arc::new(
                env::temp_dir().join(format!("codex-thread-studio-security-{suffix}.json")),
            ),
            preferences_lock: Arc::new(Mutex::new(())),
            shared_document_directories: Arc::new(Mutex::new(Vec::new())),
            studio_path: Arc::new(
                env::temp_dir().join(format!("codex-thread-studio-security-{suffix}.sqlite3")),
            ),
            studio_lock: Arc::new(Mutex::new(())),
            session_maps_path: Arc::new(env::temp_dir().join(format!(
                "codex-thread-studio-security-maps-{suffix}.sqlite3"
            ))),
            session_maps_lock: Arc::new(Mutex::new(())),
            epub_reading_path: Arc::new(env::temp_dir().join(format!(
                "codex-thread-studio-security-epub-{suffix}.sqlite3"
            ))),
            epub_reading_lock: Arc::new(Mutex::new(())),
            environment_path: Arc::new(env::temp_dir().join(format!(
                "codex-thread-studio-security-environment-{suffix}.json"
            ))),
            environment_lock: Arc::new(Mutex::new(())),
            security,
            embedded_browser: false,
        }
    }

    #[test]
    fn builds_independent_configured_codex_instances() {
        let runtime = BackendRuntime::new(OsString::new(), WslSettings::default());
        let configured = ConfiguredCodexBackend {
            descriptor: BackendDescriptor {
                id: "work-codex".to_string(),
                name: "Work Codex".to_string(),
                tag: "WK".to_string(),
                kind: "codex",
                adapter: "codex-app-server",
                info_path: "/studio/backend/work-codex".to_string(),
                socket_path: "/ws/codex/work-codex".to_string(),
                protocol: "Codex App Server v2",
                transport: "stdio JSONL via Studio WebSocket",
            },
            command: "company-launcher".to_string(),
            args: vec!["codex".to_string()],
        };
        let codex = CodexAppServer::new("codex".to_string(), runtime.clone());
        let (instances, descriptors) =
            build_backend_registry("codex", codex, vec![configured], runtime);

        assert_eq!(
            descriptors
                .iter()
                .map(|descriptor| descriptor.id.as_str())
                .collect::<Vec<_>>(),
            ["codex", "work-codex", "opencode"]
        );
        assert_eq!(instances.len(), 2);
        assert_eq!(instances["codex"].server.binary(), "codex");
        assert_eq!(instances["work-codex"].server.binary(), "company-launcher");
        assert_eq!(
            instances["work-codex"].command_line,
            "company-launcher codex"
        );
    }

    #[test]
    fn lifecycle_envelopes_keep_the_originating_backend() {
        let payload = codex_lifecycle_envelope(
            "ept-codex",
            json!({
                "method": "turn/completed",
                "params": { "threadId": "thread-1", "turn": { "id": "turn-1" } }
            }),
        );
        let value: serde_json::Value = serde_json::from_str(&payload).expect("valid envelope");
        assert_eq!(value["method"], "studio/codexLifecycle/event");
        assert_eq!(value["params"]["backend"], "ept-codex");
        assert_eq!(value["params"]["message"]["method"], "turn/completed");
    }

    #[test]
    fn protects_gateway_data_and_websocket_from_untrusted_pages() {
        let runtime = tokio::runtime::Runtime::new().expect("test runtime");
        runtime.block_on(async {
            const ORIGIN: &str = "http://127.0.0.1:41234";
            const TOKEN: &str = "0123456789abcdef0123456789abcdef";
            let security = GatewaySecurity::for_tests(ORIGIN, TOKEN);
            let protocol = security.websocket_protocol();
            let router = gateway_router(secured_test_gateway(security));

            for path in [
                "/studio/preferences",
                "/studio/speech",
                "/studio/session-state",
                "/studio/favorites",
                "/studio/session-map/codex/thread-1",
                "/studio/epub/state",
            ] {
                let response = router
                    .clone()
                    .oneshot(
                        Request::builder()
                            .uri(path)
                            .body(Body::empty())
                            .expect("unauthenticated request"),
                    )
                    .await
                    .expect("gateway response");
                assert_eq!(response.status(), StatusCode::UNAUTHORIZED, "{path}");
            }

            let authorized = router
                .clone()
                .oneshot(
                    Request::builder()
                        .uri("/studio/preferences")
                        .header(header::ORIGIN, ORIGIN)
                        .header(header::HOST, "127.0.0.1:41234")
                        .header(header::AUTHORIZATION, format!("Bearer {TOKEN}"))
                        .body(Body::empty())
                        .expect("authorized request"),
                )
                .await
                .expect("authorized response");
            assert_eq!(authorized.status(), StatusCode::OK);

            for path in ["/ws/codex", "/ws/codex-lifecycle"] {
                let websocket = router
                    .clone()
                    .oneshot(
                        Request::builder()
                            .uri(path)
                            .header(header::ORIGIN, "https://untrusted.example")
                            .header(header::HOST, "127.0.0.1:41234")
                            .header(header::SEC_WEBSOCKET_PROTOCOL, protocol.clone())
                            .body(Body::empty())
                            .expect("foreign websocket request"),
                    )
                    .await
                    .expect("foreign websocket response");
                assert_eq!(websocket.status(), StatusCode::FORBIDDEN, "{path}");
            }
        });
    }

    #[test]
    fn session_state_routes_round_trip_without_affecting_preferences_routes() {
        let runtime = tokio::runtime::Runtime::new().expect("test runtime");
        runtime.block_on(async {
            let state = secured_test_gateway(GatewaySecurity::disabled_for_tests());
            let database_path = state.studio_path.as_ref().clone();
            let preferences_path = state.preferences_path.as_ref().clone();
            let router = gateway_router(state);
            let annotation_body = json!({
                "sessionKey": "codex:thread-1",
                "drafts": [{
                    "id": "draft-1",
                    "excerpt": "selected text",
                    "createdAt": "2026-08-31T00:00:00Z"
                }],
                "additional": "overall note"
            })
            .to_string();

            let response = router
                .clone()
                .oneshot(
                    Request::builder()
                        .method(Method::PUT)
                        .uri("/studio/session-state/annotations")
                        .header(header::CONTENT_TYPE, "application/json")
                        .body(Body::from(annotation_body))
                        .expect("annotation request"),
                )
                .await
                .expect("annotation response");
            assert_eq!(response.status(), StatusCode::NO_CONTENT);

            let response = router
                .clone()
                .oneshot(
                    Request::builder()
                        .method(Method::PUT)
                        .uri("/studio/session-state/message-queue")
                        .header(header::CONTENT_TYPE, "application/json")
                        .body(Body::from(
                            r#"{"sessionKey":"codex:thread-1","messages":[{"id":"queued-1","text":"later","input":[],"createdAt":1}]}"#,
                        ))
                        .expect("message queue request"),
                )
                .await
                .expect("message queue response");
            assert_eq!(response.status(), StatusCode::NO_CONTENT);

            let response = router
                .clone()
                .oneshot(
                    Request::builder()
                        .method(Method::PUT)
                        .uri("/studio/session-state/pin")
                        .header(header::CONTENT_TYPE, "application/json")
                        .body(Body::from(
                            r#"{"sessionKey":"codex:thread-1","pinned":true}"#,
                        ))
                        .expect("pin request"),
                )
                .await
                .expect("pin response");
            assert_eq!(response.status(), StatusCode::OK);

            let response = router
                .clone()
                .oneshot(
                    Request::builder()
                        .method(Method::PUT)
                        .uri("/studio/session-state/turn-options")
                        .header(header::CONTENT_TYPE, "application/json")
                        .body(Body::from(
                            r#"{"sessionKey":"codex:thread-1","model":"gpt-session","effort":"high"}"#,
                        ))
                        .expect("turn options request"),
                )
                .await
                .expect("turn options response");
            assert_eq!(response.status(), StatusCode::NO_CONTENT);

            let response = router
                .clone()
                .oneshot(
                    Request::builder()
                        .uri("/studio/session-state")
                        .body(Body::empty())
                        .expect("session state request"),
                )
                .await
                .expect("session state response");
            assert_eq!(response.status(), StatusCode::OK);
            let body = axum::body::to_bytes(response.into_body(), MAX_PREFERENCES_BODY)
                .await
                .expect("session state body");
            let value: serde_json::Value =
                serde_json::from_slice(&body).expect("session state JSON");
            assert_eq!(
                value["annotationDrafts"]["codex:thread-1"][0]["id"],
                "draft-1"
            );
            assert_eq!(
                value["annotationAdditional"]["codex:thread-1"],
                "overall note"
            );
            assert_eq!(value["pinnedSessions"], json!(["codex:thread-1"]));
            assert_eq!(
                value["turnOptions"]["codex:thread-1"],
                json!({ "model": "gpt-session", "effort": "high" })
            );
            assert_eq!(
                value["messageQueues"]["codex:thread-1"][0]["text"],
                "later"
            );

            let response = router
                .clone()
                .oneshot(
                    Request::builder()
                        .method(Method::PUT)
                        .uri("/studio/preferences")
                        .header(header::CONTENT_TYPE, "application/json")
                        .body(Body::from(
                            json!({
                                "selectedBackend": "opencode",
                                "selectedThreads": { "opencode": "legacy-session" },
                                "translation": {
                                    "models": { "codex": "gpt-fast" },
                                    "efforts": { "codex": "low" }
                                },
                                "annotationDrafts": {
                                    "codex:thread-1": [{
                                        "id": "legacy-copy",
                                        "excerpt": "must not return to settings",
                                        "createdAt": "2026-08-31T00:00:00Z"
                                    }]
                                }
                            })
                            .to_string(),
                        ))
                        .expect("preferences request"),
                )
                .await
                .expect("preferences response");
            assert_eq!(response.status(), StatusCode::OK);
            let saved_preferences: serde_json::Value =
                serde_json::from_slice(&fs::read(&preferences_path).expect("saved preferences"))
                    .expect("preferences JSON");
            assert!(saved_preferences.get("annotationDrafts").is_none());
            assert!(saved_preferences.get("annotationAdditional").is_none());
            assert!(saved_preferences.get("openingMessages").is_none());
            assert!(saved_preferences.get("selectedBackend").is_none());
            assert!(saved_preferences.get("selectedThreads").is_none());
            assert_eq!(
                saved_preferences["translation"]["models"]["codex"],
                "gpt-fast"
            );
            assert_eq!(saved_preferences["translation"]["efforts"]["codex"], "low");

            let response = router
                .oneshot(
                    Request::builder()
                        .method(Method::DELETE)
                        .uri("/studio/session-state/session")
                        .header(header::CONTENT_TYPE, "application/json")
                        .body(Body::from(r#"{"sessionKey":"codex:thread-1"}"#))
                        .expect("delete request"),
                )
                .await
                .expect("delete response");
            assert_eq!(response.status(), StatusCode::NO_CONTENT);
            fs::remove_file(database_path).ok();
            fs::remove_file(preferences_path).ok();
        });
    }

    #[test]
    fn browser_labels_do_not_match_main_webview_capabilities() {
        let capability: serde_json::Value =
            serde_json::from_str(include_str!("../capabilities/default.json"))
                .expect("valid Tauri capability JSON");
        assert_eq!(capability["windows"], json!(["main"]));
        let labels = capability["windows"]
            .as_array()
            .expect("capability window labels");
        assert!(!labels.iter().any(|label| {
            label
                .as_str()
                .is_some_and(|label| label == "*" || label.starts_with("browser"))
        }));
    }

    #[test]
    fn serves_all_embedded_frontend_modules() {
        let runtime = tokio::runtime::Runtime::new().expect("test runtime");
        runtime.block_on(async {
            let state = GatewayState {
                codex: CodexAppServer::new(
                    "codex".to_string(),
                    BackendRuntime::new(OsString::new(), WslSettings::default()),
                ),
                codex_backends: test_codex_backends(),
                backend_descriptors: test_backend_descriptors(),
                backend_config_path: Arc::new(env::temp_dir().join("backends.json")),
                backend_config_error: Arc::new(None),
                opencode: OpenCodeServer::new(
                    "opencode".to_string(),
                    BackendRuntime::new(OsString::new(), WslSettings::default()),
                ),
                preferences_path: Arc::new(
                    env::temp_dir().join("codex-thread-studio-route-test.json"),
                ),
                preferences_lock: Arc::new(Mutex::new(())),
                shared_document_directories: Arc::new(Mutex::new(Vec::new())),
                studio_path: Arc::new(
                    env::temp_dir().join("codex-thread-studio-route-favorites-test.sqlite3"),
                ),
                studio_lock: Arc::new(Mutex::new(())),
                session_maps_path: Arc::new(
                    env::temp_dir().join("codex-thread-studio-route-maps-test.sqlite3"),
                ),
                session_maps_lock: Arc::new(Mutex::new(())),
                epub_reading_path: Arc::new(
                    env::temp_dir().join("codex-thread-studio-route-epub-test.sqlite3"),
                ),
                epub_reading_lock: Arc::new(Mutex::new(())),
                environment_path: Arc::new(
                    env::temp_dir().join("codex-thread-studio-route-environment-test.json"),
                ),
                environment_lock: Arc::new(Mutex::new(())),
                security: GatewaySecurity::disabled_for_tests(),
                embedded_browser: false,
            };
            let router = gateway_router(state);
            for path in [
                "/",
                "/bootstrap-errors.js",
                "/remote-bootstrap.js",
                "/app.js",
                "/i18n.mjs",
                "/codex-native.mjs",
                "/opencode-native.mjs",
                "/model-revision.mjs",
                "/performance-monitor.mjs",
                "/selection-translation.mjs",
                "/continuation-draft.mjs",
                "/backends.mjs",
                "/model-display.mjs",
                "/session-catalog.mjs",
                "/session-management.mjs",
                "/thread-catalog.mjs",
                "/thread-fork.mjs",
                "/thread-router-controller.mjs",
                "/thread-workset.mjs",
                "/session-search.mjs",
                "/composer-tools.mjs",
                "/composer-images.mjs",
                "/document-review.mjs",
                "/document-outline.mjs",
                "/epub-reader.mjs",
                "/epub-comment-provider.mjs",
                "/comment-markers.mjs",
                "/session-resources.mjs",
                "/session-resources-ui.mjs",
                "/favorites.mjs",
                "/session-map.mjs",
                "/session-map-controller.mjs",
                "/review-notes-controller.mjs",
                "/document-workspace-controller.mjs",
                "/mermaid-config.mjs",
                "/thread-router.mjs",
                "/session-dispatch.mjs",
                "/turn-navigator.mjs",
                "/transcript-scroll.mjs",
                "/transcript-dom.mjs",
                "/transcript-presentation.mjs",
                "/vendor/mermaid.min.js",
                "/vendor/epub.mjs",
                "/styles.css",
            ] {
                let response = router
                    .clone()
                    .oneshot(
                        Request::builder()
                            .uri(path)
                            .body(Body::empty())
                            .expect("request"),
                    )
                    .await
                    .expect("embedded asset response");
                assert_eq!(
                    response.status(),
                    StatusCode::OK,
                    "missing route for {path}"
                );
            }

            let bootstrap = router
                .clone()
                .oneshot(
                    Request::builder()
                        .uri("/remote-bootstrap.js")
                        .body(Body::empty())
                        .expect("bootstrap request"),
                )
                .await
                .expect("remote bootstrap response");
            let bootstrap = axum::body::to_bytes(bootstrap.into_body(), 16 * 1024)
                .await
                .expect("remote bootstrap body");
            let bootstrap = std::str::from_utf8(&bootstrap).expect("UTF-8 bootstrap");
            assert!(bootstrap.contains("fragment.get('token')"));
            assert!(bootstrap.contains("query.get('token')"));
            assert!(bootstrap.contains("remote: true"));
            assert!(bootstrap.contains(std::env::consts::OS));

            fn relative_imports(source: &str) -> Vec<String> {
                source
                    .lines()
                    .flat_map(|line| {
                        [
                            "from '",
                            "import '",
                            "from \"",
                            "import \"",
                            "import('",
                            "import(\"",
                        ]
                        .into_iter()
                        .filter_map(move |marker| {
                            let start = line.find(marker)? + marker.len();
                            let remainder = &line[start..];
                            let quote = marker.chars().last()?;
                            let end = remainder.find(quote)?;
                            let specifier = &remainder[..end];
                            specifier.starts_with("./").then(|| specifier.to_string())
                        })
                    })
                    .collect()
            }

            fn resolve_import(importer: &str, specifier: &str) -> String {
                let parent = importer.rsplit_once('/').map_or("", |(parent, _)| parent);
                let mut segments = parent
                    .split('/')
                    .filter(|segment| !segment.is_empty())
                    .collect::<Vec<_>>();
                for segment in specifier.split('/') {
                    match segment {
                        "" | "." => {}
                        ".." => {
                            segments.pop();
                        }
                        value => segments.push(value),
                    }
                }
                format!("/{}", segments.join("/"))
            }

            let mut pending = vec!["/app.js".to_string()];
            let mut visited = BTreeSet::new();
            while let Some(module) = pending.pop() {
                if !visited.insert(module.clone()) {
                    continue;
                }
                let response = router
                    .clone()
                    .oneshot(
                        Request::builder()
                            .uri(&module)
                            .body(Body::empty())
                            .expect("module request"),
                    )
                    .await
                    .expect("imported module response");
                assert_eq!(
                    response.status(),
                    StatusCode::OK,
                    "missing route for imported module {module}"
                );
                let source = axum::body::to_bytes(response.into_body(), 5 * 1024 * 1024)
                    .await
                    .expect("imported module body");
                let source = std::str::from_utf8(&source).expect("UTF-8 frontend module");
                pending.extend(
                    relative_imports(source)
                        .into_iter()
                        .map(|specifier| resolve_import(&module, &specifier)),
                );
            }
        });
    }

    #[test]
    fn exposes_the_compile_time_application_version() {
        let runtime = tokio::runtime::Runtime::new().expect("test runtime");
        runtime.block_on(async {
            let state = GatewayState {
                codex: CodexAppServer::new(
                    "codex".to_string(),
                    BackendRuntime::new(OsString::new(), WslSettings::default()),
                ),
                codex_backends: test_codex_backends(),
                backend_descriptors: test_backend_descriptors(),
                backend_config_path: Arc::new(env::temp_dir().join("backends.json")),
                backend_config_error: Arc::new(None),
                opencode: OpenCodeServer::new(
                    "opencode".to_string(),
                    BackendRuntime::new(OsString::new(), WslSettings::default()),
                ),
                preferences_path: Arc::new(
                    env::temp_dir().join("codex-thread-studio-version-test.json"),
                ),
                preferences_lock: Arc::new(Mutex::new(())),
                shared_document_directories: Arc::new(Mutex::new(Vec::new())),
                studio_path: Arc::new(
                    env::temp_dir().join("codex-thread-studio-version-favorites-test.sqlite3"),
                ),
                studio_lock: Arc::new(Mutex::new(())),
                session_maps_path: Arc::new(
                    env::temp_dir().join("codex-thread-studio-version-maps-test.sqlite3"),
                ),
                session_maps_lock: Arc::new(Mutex::new(())),
                epub_reading_path: Arc::new(
                    env::temp_dir().join("codex-thread-studio-version-epub-test.sqlite3"),
                ),
                epub_reading_lock: Arc::new(Mutex::new(())),
                environment_path: Arc::new(
                    env::temp_dir().join("codex-thread-studio-version-environment-test.json"),
                ),
                environment_lock: Arc::new(Mutex::new(())),
                security: GatewaySecurity::disabled_for_tests(),
                embedded_browser: false,
            };
            let response = gateway_router(state)
                .oneshot(
                    Request::builder()
                        .uri("/studio/codex")
                        .body(Body::empty())
                        .expect("request"),
                )
                .await
                .expect("version response");
            let body = axum::body::to_bytes(response.into_body(), 4096)
                .await
                .expect("version body");
            let value: serde_json::Value = serde_json::from_slice(&body).expect("version JSON");
            assert_eq!(value["appName"], "Codex Thread Studio");
            assert_eq!(value["appVersion"], env!("CARGO_PKG_VERSION"));
        });
    }

    #[test]
    fn session_map_api_keeps_missing_maps_absent_until_explicit_creation() {
        let runtime = tokio::runtime::Runtime::new().expect("test runtime");
        runtime.block_on(async {
            let unique = format!(
                "codex-thread-studio-map-api-{}-{}.sqlite3",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .expect("system time after epoch")
                    .as_nanos()
            );
            let maps_path = env::temp_dir().join(unique);
            let state = GatewayState {
                codex: CodexAppServer::new(
                    "codex".to_string(),
                    BackendRuntime::new(OsString::new(), WslSettings::default()),
                ),
                codex_backends: test_codex_backends(),
                backend_descriptors: test_backend_descriptors(),
                backend_config_path: Arc::new(env::temp_dir().join("backends.json")),
                backend_config_error: Arc::new(None),
                opencode: OpenCodeServer::new(
                    "opencode".to_string(),
                    BackendRuntime::new(OsString::new(), WslSettings::default()),
                ),
                preferences_path: Arc::new(
                    env::temp_dir().join("codex-thread-studio-map-api-settings.json"),
                ),
                preferences_lock: Arc::new(Mutex::new(())),
                shared_document_directories: Arc::new(Mutex::new(Vec::new())),
                studio_path: Arc::new(
                    env::temp_dir().join("codex-thread-studio-map-api-studio.sqlite3"),
                ),
                studio_lock: Arc::new(Mutex::new(())),
                session_maps_path: Arc::new(maps_path.clone()),
                session_maps_lock: Arc::new(Mutex::new(())),
                epub_reading_path: Arc::new(
                    env::temp_dir().join("codex-thread-studio-map-api-epub-test.sqlite3"),
                ),
                epub_reading_lock: Arc::new(Mutex::new(())),
                environment_path: Arc::new(
                    env::temp_dir().join("codex-thread-studio-map-api-environment-test.json"),
                ),
                environment_lock: Arc::new(Mutex::new(())),
                security: GatewaySecurity::disabled_for_tests(),
                embedded_browser: false,
            };
            let router = gateway_router(state);
            let missing = router
                .clone()
                .oneshot(
                    Request::builder()
                        .uri("/studio/session-map/codex/thread-optional")
                        .body(Body::empty())
                        .expect("missing map request"),
                )
                .await
                .expect("missing map response");
            assert_eq!(missing.status(), StatusCode::NOT_FOUND);

            let create = router
                .clone()
                .oneshot(
                    Request::builder()
                        .method(Method::POST)
                        .uri("/studio/session-map")
                        .header(header::CONTENT_TYPE, "application/json")
                        .body(Body::from(
                            json!({
                                "backend": "codex",
                                "threadId": "thread-optional",
                                "goal": "Keep the long conversation navigable",
                                "definitionOfDone": "All durable topics are visible",
                                "structure": "hierarchy",
                                "items": []
                            })
                            .to_string(),
                        ))
                        .expect("create map request"),
                )
                .await
                .expect("create map response");
            assert_eq!(create.status(), StatusCode::CREATED);
            let create_body = axum::body::to_bytes(create.into_body(), MAX_MAP_BODY_BYTES)
                .await
                .expect("create map body");
            let created: serde_json::Value =
                serde_json::from_slice(&create_body).expect("created map JSON");
            assert_eq!(created["revision"], 1);

            let update = router
                .clone()
                .oneshot(
                    Request::builder()
                        .method(Method::POST)
                        .uri("/studio/session-map/codex/thread-optional/operations")
                        .header(header::CONTENT_TYPE, "application/json")
                        .body(Body::from(
                            json!({
                                "baseRevision": 1,
                                "actor": "user",
                                "operations": [{
                                    "op": "addItem",
                                    "itemId": "topic-1",
                                    "parentId": null,
                                    "afterItemId": null,
                                    "title": "First topic",
                                    "kind": "topic",
                                    "summary": "",
                                    "state": "notStarted"
                                }]
                            })
                            .to_string(),
                        ))
                        .expect("update map request"),
                )
                .await
                .expect("update map response");
            assert_eq!(update.status(), StatusCode::OK);
            let update_body = axum::body::to_bytes(update.into_body(), MAX_MAP_BODY_BYTES)
                .await
                .expect("update map body");
            let updated: serde_json::Value =
                serde_json::from_slice(&update_body).expect("updated map JSON");
            assert_eq!(updated["revision"], 2);
            assert_eq!(updated["items"][0]["id"], "topic-1");

            for candidate in [
                maps_path.clone(),
                maps_path.with_extension("sqlite3-wal"),
                maps_path.with_extension("sqlite3-shm"),
            ] {
                fs::remove_file(candidate).ok();
            }
        });
    }

    #[test]
    fn favorite_api_preserves_native_message_anchors() {
        let runtime = tokio::runtime::Runtime::new().expect("test runtime");
        runtime.block_on(async {
            let unique = format!(
                "codex-thread-studio-favorites-api-{}-{}.sqlite3",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .expect("system time after epoch")
                    .as_nanos()
            );
            let studio_path = env::temp_dir().join(unique);
            let state = GatewayState {
                codex: CodexAppServer::new(
                    "codex".to_string(),
                    BackendRuntime::new(OsString::new(), WslSettings::default()),
                ),
                codex_backends: test_codex_backends(),
                backend_descriptors: test_backend_descriptors(),
                backend_config_path: Arc::new(env::temp_dir().join("backends.json")),
                backend_config_error: Arc::new(None),
                opencode: OpenCodeServer::new(
                    "opencode".to_string(),
                    BackendRuntime::new(OsString::new(), WslSettings::default()),
                ),
                preferences_path: Arc::new(
                    env::temp_dir().join("codex-thread-studio-favorites-api-settings.json"),
                ),
                preferences_lock: Arc::new(Mutex::new(())),
                shared_document_directories: Arc::new(Mutex::new(Vec::new())),
                studio_path: Arc::new(studio_path.clone()),
                studio_lock: Arc::new(Mutex::new(())),
                session_maps_path: Arc::new(
                    env::temp_dir().join("codex-thread-studio-favorites-api-maps-test.sqlite3"),
                ),
                session_maps_lock: Arc::new(Mutex::new(())),
                epub_reading_path: Arc::new(
                    env::temp_dir().join("codex-thread-studio-favorites-api-epub-test.sqlite3"),
                ),
                epub_reading_lock: Arc::new(Mutex::new(())),
                environment_path: Arc::new(
                    env::temp_dir().join("codex-thread-studio-favorites-api-environment-test.json"),
                ),
                environment_lock: Arc::new(Mutex::new(())),
                security: GatewaySecurity::disabled_for_tests(),
                embedded_browser: false,
            };
            let router = gateway_router(state);
            let favorite = json!({
                "id": "favorite-1",
                "backend": "codex",
                "threadId": "thread-1",
                "threadTitle": "Native UI",
                "projectPath": "/tmp/native-ui",
                "turnId": "turn-1",
                "itemId": "item-1",
                "title": "Structured favorite",
                "question": "How should this work?",
                "content": "Use the App Server item directly.",
                "note": "Keep the source anchor.",
                "tags": ["design"],
                "createdAt": "2026-07-24T00:00:00.000Z"
            });
            let create = router
                .clone()
                .oneshot(
                    Request::builder()
                        .method(Method::POST)
                        .uri("/studio/favorites")
                        .header(header::CONTENT_TYPE, "application/json")
                        .body(Body::from(favorite.to_string()))
                        .expect("create request"),
                )
                .await
                .expect("create response");
            assert_eq!(create.status(), StatusCode::CREATED);

            let list = router
                .clone()
                .oneshot(
                    Request::builder()
                        .uri("/studio/favorites?q=anchor")
                        .body(Body::empty())
                        .expect("list request"),
                )
                .await
                .expect("list response");
            assert_eq!(list.status(), StatusCode::OK);
            let body = axum::body::to_bytes(list.into_body(), MAX_FAVORITE_BODY_BYTES)
                .await
                .expect("list body");
            let value: serde_json::Value =
                serde_json::from_slice(&body).expect("favorite list JSON");
            assert_eq!(value["items"][0]["turnId"], "turn-1");
            assert_eq!(value["items"][0]["itemId"], "item-1");

            let export = router
                .clone()
                .oneshot(
                    Request::builder()
                        .uri("/studio/favorites/export")
                        .body(Body::empty())
                        .expect("export request"),
                )
                .await
                .expect("export response");
            assert_eq!(export.status(), StatusCode::OK);
            assert_eq!(
                export
                    .headers()
                    .get(header::CONTENT_TYPE)
                    .and_then(|value| value.to_str().ok()),
                Some("text/markdown; charset=utf-8")
            );
            let markdown = axum::body::to_bytes(export.into_body(), MAX_FAVORITE_BODY_BYTES)
                .await
                .expect("export body");
            assert!(
                String::from_utf8_lossy(&markdown).contains("Use the App Server item directly.")
            );

            let delete = router
                .oneshot(
                    Request::builder()
                        .method(Method::DELETE)
                        .uri("/studio/favorites/favorite-1")
                        .body(Body::empty())
                        .expect("delete request"),
                )
                .await
                .expect("delete response");
            assert_eq!(delete.status(), StatusCode::OK);
            fs::remove_file(studio_path).ok();
        });
    }

    #[test]
    fn validates_annotation_anchor_sizes() {
        let mut preferences = StudioPreferences::default();
        preferences.annotation_drafts.insert(
            "thread-1".to_string(),
            vec![AnnotationDraft {
                id: "draft-1".to_string(),
                quote: "selected output".to_string(),
                comment: "please clarify".to_string(),
                created_at: "2026-07-15T00:00:00Z".to_string(),
                item_id: Some("item-1".to_string()),
                turn_id: Some("turn-1".to_string()),
                target: None,
                ..AnnotationDraft::default()
            }],
        );
        assert!(validate_preferences(&preferences).is_ok());

        preferences.annotation_drafts.insert(
            "codex:thread-2".to_string(),
            vec![AnnotationDraft {
                id: "draft-provider".to_string(),
                excerpt: "selected web text".to_string(),
                source: Some(AnnotationSourceReference {
                    provider: "browser".to_string(),
                    version: 1,
                    anchor: serde_json::json!({"url": "https://example.com"}),
                }),
                created_at: "2026-08-12T00:00:00Z".to_string(),
                ..AnnotationDraft::default()
            }],
        );
        assert!(validate_preferences(&preferences).is_ok());

        let mut invalid = preferences.clone();
        invalid.annotation_drafts.insert(
            "thread-1".to_string(),
            vec![AnnotationDraft {
                id: "draft-2".to_string(),
                quote: "selected output".to_string(),
                comment: "please clarify".to_string(),
                created_at: "2026-07-16T00:00:00Z".to_string(),
                item_id: Some("item-2".to_string()),
                turn_id: Some("turn-2".to_string()),
                target: Some(AnnotationTarget {
                    kind: "fileRange".to_string(),
                    file_path: Some("/tmp/docs/change.md".to_string()),
                    root: Some("/tmp/docs".to_string()),
                    base_hash: Some("hash-1".to_string()),
                    start_offset: Some(10),
                    end_offset: Some(10),
                    start_line: Some(2),
                    end_line: Some(2),
                    prefix: None,
                    suffix: None,
                }),
                ..AnnotationDraft::default()
            }],
        );
        assert!(validate_preferences(&invalid).is_err());

        {
            let invalid_target = invalid
                .annotation_drafts
                .get_mut("thread-1")
                .and_then(|drafts| drafts.first_mut())
                .and_then(|draft| draft.target.as_mut())
                .expect("file target");
            invalid_target.start_offset = None;
            invalid_target.end_offset = Some(10);
        }
        assert!(validate_preferences(&invalid).is_err());

        {
            let invalid_target = invalid
                .annotation_drafts
                .get_mut("thread-1")
                .and_then(|drafts| drafts.first_mut())
                .and_then(|draft| draft.target.as_mut())
                .expect("file target");
            invalid_target.start_offset = Some(10);
            invalid_target.end_offset = None;
        }
        assert!(validate_preferences(&invalid).is_err());

        {
            let invalid_target = invalid
                .annotation_drafts
                .get_mut("thread-1")
                .and_then(|drafts| drafts.first_mut())
                .and_then(|draft| draft.target.as_mut())
                .expect("file target");
            invalid_target.start_offset = None;
            invalid_target.end_offset = None;
        }
        assert!(validate_preferences(&invalid).is_ok());

        {
            let invalid_target = invalid
                .annotation_drafts
                .get_mut("thread-1")
                .and_then(|drafts| drafts.first_mut())
                .and_then(|draft| draft.target.as_mut())
                .expect("file target");
            invalid_target.start_line = None;
            invalid_target.end_line = Some(2);
        }
        assert!(validate_preferences(&invalid).is_err());
    }

    #[test]
    fn review_file_reader_is_utf8_only_and_confined_to_project_root() {
        let base = env::temp_dir().join(format!(
            "codex-thread-studio-review-{}",
            uuid::Uuid::new_v4()
        ));
        let root = base.join("project");
        let outside = base.join("outside.md");
        fs::create_dir_all(root.join("docs")).expect("create review fixture");
        fs::write(root.join("docs/guide.md"), "# Guide\n\nHello\n").expect("write review file");
        fs::write(root.join("docs/diagram.customdsl"), "node -> target\n")
            .expect("write unknown text format");
        fs::write(root.join("docs/nul.customdsl"), b"node\0target").expect("write NUL fixture");
        fs::write(
            root.join("docs/control.customdsl"),
            b"text\x01\x02\x03\x04payload",
        )
        .expect("write control fixture");
        fs::write(root.join("docs/invalid.customdsl"), b"text\xffpayload")
            .expect("write invalid UTF-8 fixture");
        fs::write(&outside, "private").expect("write outside file");

        let file = load_review_file(&ReviewFileRequest {
            root: root.to_string_lossy().into_owned(),
            path: "docs/guide.md".to_string(),
        })
        .expect("read project file");
        assert_eq!(file.relative_path, "docs/guide.md");
        assert_eq!(file.language, "markdown");
        assert_eq!(file.line_count, 3);
        assert!(file.hash.starts_with("fnv1a64:"));

        let unknown_text = load_review_file(&ReviewFileRequest {
            root: root.to_string_lossy().into_owned(),
            path: "docs/diagram.customdsl".to_string(),
        })
        .expect("read unknown text format");
        assert_eq!(unknown_text.content, "node -> target\n");
        assert_eq!(unknown_text.language, "text");

        for path in [
            "docs/nul.customdsl",
            "docs/control.customdsl",
            "docs/invalid.customdsl",
        ] {
            let rejected = load_review_file(&ReviewFileRequest {
                root: root.to_string_lossy().into_owned(),
                path: path.to_string(),
            });
            assert!(matches!(
                rejected,
                Err((StatusCode::UNSUPPORTED_MEDIA_TYPE, _))
            ));
        }

        let escaped = load_review_file(&ReviewFileRequest {
            root: root.to_string_lossy().into_owned(),
            path: outside.to_string_lossy().into_owned(),
        });
        assert!(matches!(escaped, Err((StatusCode::FORBIDDEN, _))));

        fs::remove_file(root.join("docs/guide.md")).ok();
        fs::remove_file(root.join("docs/diagram.customdsl")).ok();
        fs::remove_file(root.join("docs/nul.customdsl")).ok();
        fs::remove_file(root.join("docs/control.customdsl")).ok();
        fs::remove_file(root.join("docs/invalid.customdsl")).ok();
        fs::remove_file(&outside).ok();
        fs::remove_dir(root.join("docs")).ok();
        fs::remove_dir(&root).ok();
        fs::remove_dir(&base).ok();
    }

    #[test]
    fn shared_document_reader_is_read_only_and_confined_to_configured_directories() {
        let base = env::temp_dir().join(format!(
            "codex-thread-studio-shared-review-{}",
            uuid::Uuid::new_v4()
        ));
        let root = base.join("project");
        let shared = base.join("shared");
        let outside = base.join("outside");
        fs::create_dir_all(&root).expect("create project fixture");
        fs::create_dir_all(shared.join("docs")).expect("create shared fixture");
        fs::create_dir_all(&outside).expect("create outside fixture");
        let shared_file = shared.join("docs/guide.md");
        let outside_file = outside.join("private.md");
        fs::write(&shared_file, "# Shared\n").expect("write shared file");
        fs::write(&outside_file, "private\n").expect("write outside file");
        let shared_root = fs::canonicalize(&shared).expect("canonical shared root");

        let file = load_review_file_with_shared(
            &ReviewFileRequest {
                root: root.to_string_lossy().into_owned(),
                path: shared_file.to_string_lossy().into_owned(),
            },
            std::slice::from_ref(&shared_root),
        )
        .expect("read configured shared file");
        assert!(file.read_only);
        assert_eq!(file.document_root, shared_root.to_string_lossy());
        assert_eq!(file.relative_path, "docs/guide.md");

        let escaped = load_review_file_with_shared(
            &ReviewFileRequest {
                root: root.to_string_lossy().into_owned(),
                path: outside_file.to_string_lossy().into_owned(),
            },
            std::slice::from_ref(&shared_root),
        );
        assert!(matches!(escaped, Err((StatusCode::FORBIDDEN, _))));

        let save = persist_workspace_file(&WorkspaceSaveRequest {
            root: root.to_string_lossy().into_owned(),
            path: shared_file.to_string_lossy().into_owned(),
            content: "changed\n".to_string(),
            expected_hash: file.hash,
            overwrite: false,
        });
        assert!(matches!(
            save,
            Err(WorkspaceSaveError::Http(StatusCode::FORBIDDEN, _))
        ));
        assert_eq!(fs::read_to_string(&shared_file).unwrap(), "# Shared\n");

        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&outside_file, shared.join("outside-link.md"))
                .expect("create escaping symlink");
            let linked = load_review_file_with_shared(
                &ReviewFileRequest {
                    root: root.to_string_lossy().into_owned(),
                    path: shared
                        .join("outside-link.md")
                        .to_string_lossy()
                        .into_owned(),
                },
                std::slice::from_ref(&shared_root),
            );
            assert!(matches!(linked, Err((StatusCode::FORBIDDEN, _))));
            fs::remove_file(shared.join("outside-link.md")).ok();
        }

        fs::remove_file(shared_file).ok();
        fs::remove_file(outside_file).ok();
        fs::remove_dir(shared.join("docs")).ok();
        fs::remove_dir(shared).ok();
        fs::remove_dir(outside).ok();
        fs::remove_dir(root).ok();
        fs::remove_dir(base).ok();
    }

    #[test]
    fn workspace_directory_listing_is_lazy_sorted_and_confined_to_root() {
        let base = env::temp_dir().join(format!(
            "codex-thread-studio-workspace-list-{}",
            uuid::Uuid::new_v4()
        ));
        let root = base.join("project");
        let outside = base.join("outside");
        fs::create_dir_all(root.join("docs")).expect("create workspace fixture");
        fs::create_dir_all(&outside).expect("create outside fixture");
        fs::write(root.join("README.md"), "# Workspace\n").expect("write workspace file");
        fs::write(root.join("docs/guide.md"), "# Guide\n").expect("write nested file");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&outside, root.join("outside-link"))
            .expect("create escaped symlink");

        let root_listing = load_workspace_directory(&WorkspaceListRequest {
            root: root.to_string_lossy().into_owned(),
            path: String::new(),
        })
        .expect("list project root");
        assert_eq!(root_listing.entries.len(), 2);
        assert_eq!(root_listing.entries[0].name, "docs");
        assert_eq!(root_listing.entries[0].kind, "directory");
        assert_eq!(root_listing.entries[1].name, "README.md");
        assert!(!root_listing
            .entries
            .iter()
            .any(|entry| entry.name == "outside-link"));

        let nested = load_workspace_directory(&WorkspaceListRequest {
            root: root.to_string_lossy().into_owned(),
            path: "docs".to_string(),
        })
        .expect("list nested directory");
        assert_eq!(nested.path, "docs");
        assert_eq!(nested.entries[0].path, "docs/guide.md");

        let escaped = load_workspace_directory(&WorkspaceListRequest {
            root: root.to_string_lossy().into_owned(),
            path: "../outside".to_string(),
        });
        assert!(matches!(escaped, Err((StatusCode::FORBIDDEN, _))));

        fs::remove_file(root.join("docs/guide.md")).ok();
        fs::remove_file(root.join("README.md")).ok();
        #[cfg(unix)]
        fs::remove_file(root.join("outside-link")).ok();
        fs::remove_dir(root.join("docs")).ok();
        fs::remove_dir(&outside).ok();
        fs::remove_dir(&root).ok();
        fs::remove_dir(&base).ok();
    }

    #[test]
    fn workspace_save_detects_conflicts_and_replaces_only_the_selected_file() {
        let base = env::temp_dir().join(format!(
            "codex-thread-studio-workspace-save-{}",
            uuid::Uuid::new_v4()
        ));
        let root = base.join("project");
        fs::create_dir_all(&root).expect("create workspace fixture");
        let path = root.join("notes.md");
        fs::write(&path, "original\n").expect("write workspace file");
        let original_hash = stable_content_hash(b"original\n");

        let saved = persist_workspace_file(&WorkspaceSaveRequest {
            root: root.to_string_lossy().into_owned(),
            path: "notes.md".to_string(),
            content: "edited\n".to_string(),
            expected_hash: original_hash,
            overwrite: false,
        })
        .expect("save current file");
        assert_eq!(saved.content, "edited\n");
        assert_eq!(
            fs::read_to_string(&path).expect("read saved file"),
            "edited\n"
        );

        fs::write(&path, "external change\n").expect("simulate external edit");
        let conflict = persist_workspace_file(&WorkspaceSaveRequest {
            root: root.to_string_lossy().into_owned(),
            path: "notes.md".to_string(),
            content: "stale editor\n".to_string(),
            expected_hash: saved.hash,
            overwrite: false,
        });
        assert!(matches!(conflict, Err(WorkspaceSaveError::Conflict(_))));
        assert_eq!(
            fs::read_to_string(&path).expect("read conflicted file"),
            "external change\n"
        );

        fs::remove_file(path).ok();
        fs::remove_dir(&root).ok();
        fs::remove_dir(&base).ok();
    }

    #[test]
    fn review_image_reader_validates_type_and_project_boundary() {
        let base = env::temp_dir().join(format!(
            "codex-thread-studio-image-review-{}",
            uuid::Uuid::new_v4()
        ));
        let root = base.join("project");
        let outside = base.join("outside.png");
        fs::create_dir_all(&root).expect("create image fixture");
        fs::write(root.join("preview.png"), b"\x89PNG\r\n\x1a\nfixture")
            .expect("write PNG fixture");
        fs::write(
            root.join("diagram.svg"),
            br#"<svg xmlns="http://www.w3.org/2000/svg"><text>diagram</text></svg>"#,
        )
        .expect("write SVG fixture");
        fs::write(root.join("fake.png"), b"not an image").expect("write fake image");
        fs::write(&outside, b"\x89PNG\r\n\x1a\nprivate").expect("write outside image");

        let (bytes, mime) = load_review_image(&ReviewFileRequest {
            root: root.to_string_lossy().into_owned(),
            path: "preview.png".to_string(),
        })
        .expect("read project image");
        assert_eq!(mime, "image/png");
        assert!(bytes.starts_with(b"\x89PNG"));

        let (svg, mime) = load_review_image(&ReviewFileRequest {
            root: root.to_string_lossy().into_owned(),
            path: "diagram.svg".to_string(),
        })
        .expect("read project SVG");
        assert_eq!(mime, "image/svg+xml");
        assert!(svg.starts_with(b"<svg"));

        let fake = load_review_image(&ReviewFileRequest {
            root: root.to_string_lossy().into_owned(),
            path: "fake.png".to_string(),
        });
        assert!(matches!(fake, Err((StatusCode::UNSUPPORTED_MEDIA_TYPE, _))));

        let escaped = load_review_image(&ReviewFileRequest {
            root: root.to_string_lossy().into_owned(),
            path: outside.to_string_lossy().into_owned(),
        });
        assert!(matches!(escaped, Err((StatusCode::FORBIDDEN, _))));

        fs::remove_file(root.join("preview.png")).ok();
        fs::remove_file(root.join("diagram.svg")).ok();
        fs::remove_file(root.join("fake.png")).ok();
        fs::remove_file(&outside).ok();
        fs::remove_dir(&root).ok();
        fs::remove_dir(&base).ok();
    }

    #[test]
    fn validates_content_width_modes() {
        for width in ["comfortable", "wide", "full"] {
            let preferences = StudioPreferences {
                content_width: Some(width.to_string()),
                ..StudioPreferences::default()
            };
            assert!(validate_preferences(&preferences).is_ok());
        }
        let preferences = StudioPreferences {
            content_width: Some("unbounded".to_string()),
            ..StudioPreferences::default()
        };
        assert!(validate_preferences(&preferences).is_err());
    }

    #[test]
    fn validates_message_queue_depth() {
        for depth in 1..=3 {
            let preferences = StudioPreferences {
                queue_depth: Some(depth),
                ..StudioPreferences::default()
            };
            assert!(validate_preferences(&preferences).is_ok());
        }
        let preferences = StudioPreferences {
            queue_depth: Some(4),
            ..StudioPreferences::default()
        };
        assert!(validate_preferences(&preferences).is_err());
    }

    #[test]
    fn validates_hidden_session_directories() {
        let preferences = StudioPreferences {
            hidden_session_directories: vec![
                "/home/user/archive".to_string(),
                r"C:\Users\User\Archive".to_string(),
            ],
            session_directory_ignore: vec![
                "**/node_modules/".to_string(),
                "!/home/user/archive/keep/".to_string(),
            ],
            ..StudioPreferences::default()
        };
        assert!(validate_preferences(&preferences).is_ok());

        let preferences = StudioPreferences {
            hidden_session_directories: vec!["invalid\npath".to_string()],
            ..StudioPreferences::default()
        };
        assert!(validate_preferences(&preferences).is_err());

        let preferences = StudioPreferences {
            session_directory_ignore: vec!["invalid\npattern".to_string()],
            ..StudioPreferences::default()
        };
        assert!(validate_preferences(&preferences).is_err());
    }

    #[test]
    fn validates_shared_document_directories_as_absolute_paths() {
        let preferences = StudioPreferences {
            shared_document_directories: vec![env::temp_dir().to_string_lossy().into_owned()],
            ..StudioPreferences::default()
        };
        assert!(validate_preferences(&preferences).is_ok());

        let relative = StudioPreferences {
            shared_document_directories: vec!["shared/docs".to_string()],
            ..StudioPreferences::default()
        };
        assert!(validate_preferences(&relative).is_err());

        let controlled = StudioPreferences {
            shared_document_directories: vec!["/shared\ndocs".to_string()],
            ..StudioPreferences::default()
        };
        assert!(validate_preferences(&controlled).is_err());
    }

    #[test]
    fn validates_rendering_preferences() {
        let defaults = StudioPreferences::default();
        assert_eq!(defaults.markdown.mode, "technical");
        assert_eq!(defaults.mermaid.style, "auto");
        assert!(validate_preferences(&defaults).is_ok());

        for mode in ["reading", "technical", "compact"] {
            let mut preferences = StudioPreferences::default();
            preferences.markdown.mode = mode.to_string();
            assert!(validate_preferences(&preferences).is_ok());
        }

        let mut invalid = StudioPreferences::default();
        invalid.mermaid.layout = "unbounded".to_string();
        assert!(validate_preferences(&invalid).is_err());

        let mut invalid = StudioPreferences::default();
        invalid.mermaid.font_size = 24;
        assert!(validate_preferences(&invalid).is_err());

        let mut translation = StudioPreferences::default();
        translation
            .translation
            .models
            .insert("codex".to_string(), "gpt-fast".to_string());
        translation
            .translation
            .efforts
            .insert("codex".to_string(), "low".to_string());
        assert!(validate_preferences(&translation).is_ok());

        translation.translation.engine = "ollama".to_string();
        translation.translation.ollama_model = "gemma3:4b".to_string();
        assert!(validate_preferences(&translation).is_ok());

        translation
            .translation
            .efforts
            .insert("codex".to_string(), "unbounded".to_string());
        assert!(validate_preferences(&translation).is_err());

        translation
            .translation
            .efforts
            .insert("codex".to_string(), "low".to_string());
        translation.translation.engine = "remote".to_string();
        assert!(validate_preferences(&translation).is_err());
    }

    #[test]
    fn validates_continue_behavior_preferences() {
        for behavior in ["sessionModelDraft", "ollamaDraft", "quickSend"] {
            let preferences = StudioPreferences {
                continue_behavior: Some(behavior.to_string()),
                ..StudioPreferences::default()
            };
            assert!(validate_preferences(&preferences).is_ok());
        }
        let invalid = StudioPreferences {
            continue_behavior: Some("automaticAgent".to_string()),
            ..StudioPreferences::default()
        };
        assert!(validate_preferences(&invalid).is_err());
    }

    #[test]
    fn browser_preferences_use_safe_partial_defaults_and_validate_origins() {
        let defaults = StudioPreferences::default();
        assert!(defaults.browser.enabled);
        assert!(!defaults.browser.restore_tabs);
        assert!(!defaults.browser.allow_private_network);
        assert!(!defaults.browser.agent.enabled);

        let partial: StudioPreferences = serde_json::from_value(json!({
            "browser": { "enabled": true }
        }))
        .expect("partial browser preferences");
        assert!(partial.browser.enabled);
        assert!(partial.browser.allow_localhost);
        assert!(!partial.browser.allow_private_network);
        assert!(validate_preferences(&partial).is_ok());

        let mut invalid = StudioPreferences::default();
        invalid.browser.agent.allowed_origins = vec!["javascript:alert(1)".to_string()];
        assert!(validate_preferences(&invalid).is_err());
    }

    #[test]
    fn validates_interface_language_preferences() {
        for language in ["system", "zh-CN", "en-US"] {
            let preferences = StudioPreferences {
                language: Some(language.to_string()),
                ..StudioPreferences::default()
            };
            assert!(validate_preferences(&preferences).is_ok());
        }
        let preferences = StudioPreferences {
            language: Some("ja-JP".to_string()),
            ..StudioPreferences::default()
        };
        assert!(validate_preferences(&preferences).is_err());

        let mut preferences = StudioPreferences::default();
        preferences.annotation_prompt_templates.insert(
            "zh-CN".to_string(),
            "\u{8bf7}\u{5904}\u{7406}:\n{{annotations}}".to_string(),
        );
        assert!(validate_preferences(&preferences).is_ok());
        preferences.annotation_prompt_templates.insert(
            "ja-JP".to_string(),
            "コメント：\n{{annotations}}".to_string(),
        );
        assert!(validate_preferences(&preferences).is_err());
    }

    #[test]
    fn validates_wsl_backend_preferences() {
        let preferences = StudioPreferences {
            wsl_distribution: Some("Ubuntu-24.04".to_string()),
            wsl_user: Some("rui".to_string()),
            wsl_codex_binary: Some("/home/rui/.local/bin/codex".to_string()),
            wsl_opencode_binary: Some("opencode".to_string()),
            ..StudioPreferences::default()
        };
        assert!(validate_preferences(&preferences).is_ok());

        let invalid = StudioPreferences {
            wsl_distribution: Some("Ubuntu\ninvalid".to_string()),
            ..StudioPreferences::default()
        };
        assert!(validate_preferences(&invalid).is_err());
    }

    #[test]
    fn validates_bounded_xlsx_package_metadata() {
        use zip::write::SimpleFileOptions;
        use zip::ZipWriter;
        let mut archive = ZipWriter::new(std::io::Cursor::new(Vec::new()));
        archive
            .start_file("[Content_Types].xml", SimpleFileOptions::default())
            .unwrap();
        archive.write_all(b"<Types/>").unwrap();
        archive
            .start_file("xl/workbook.xml", SimpleFileOptions::default())
            .unwrap();
        archive.write_all(b"<workbook/>").unwrap();
        let bytes = archive.finish().unwrap().into_inner();
        assert!(validate_spreadsheet_archive(&bytes).is_ok());
        assert!(validate_spreadsheet_archive(b"PK\x03\x04not-a-zip").is_err());
    }

    #[test]
    fn validates_namespaced_thread_activity_preferences() {
        let mut preferences = StudioPreferences::default();
        preferences
            .thread_activity
            .insert("codex:thread-1".to_string(), 1_784_879_063_243);
        preferences
            .thread_activity
            .insert("opencode:session-1".to_string(), 1_784_879_063_244);
        preferences
            .thread_activity
            .insert("company-codex:session-2".to_string(), 1_784_879_063_245);
        preferences
            .attention_threads
            .push("codex:thread-1".to_string());
        assert!(validate_preferences(&preferences).is_ok());

        preferences
            .thread_activity
            .insert("Invalid:thread-1".to_string(), 1);
        assert!(validate_preferences(&preferences).is_err());

        preferences.thread_activity.remove("Invalid:thread-1");
        preferences
            .attention_threads
            .push("Invalid:thread-1".to_string());
        assert!(validate_preferences(&preferences).is_err());
    }

    #[test]
    fn validates_thread_router_preferences() {
        const MULTIBYTE_TEXT: &str = "\u{4f1a}";
        let mut preferences = StudioPreferences {
            router: Some(ThreadRouterPreferences {
                thread_id: None,
                controller_backend: Some("opencode".to_string()),
                controllers: BTreeMap::from([
                    ("codex".to_string(), "router-codex".to_string()),
                    ("opencode".to_string(), "router-opencode".to_string()),
                ]),
                responsibilities: BTreeMap::from([
                    (
                        "codex:learn-thread".to_string(),
                        RouterResponsibility {
                            description: "Books and structured learning".to_string(),
                            fallback: "fallback".to_string(),
                        },
                    ),
                    (
                        "opencode:general-thread".to_string(),
                        RouterResponsibility {
                            description: "Requests without a better match".to_string(),
                            fallback: "fallback".to_string(),
                        },
                    ),
                ]),
                fallbacks: vec![RouterFallbackPreference {
                    session_key: "codex:learn-thread".to_string(),
                    condition: "No regular session is suitable for the user query.".to_string(),
                }],
            }),
            ..StudioPreferences::default()
        };
        assert!(validate_preferences(&preferences).is_ok());

        preferences.opening_messages.insert(
            "codex:unicode-thread".to_string(),
            OpeningMessage {
                text: String::new(),
                responsibility: MULTIBYTE_TEXT.repeat(4096),
                source: "manual".to_string(),
                captured_at: String::new(),
                truncated: false,
            },
        );
        preferences.router.as_mut().unwrap().fallbacks[0].condition = MULTIBYTE_TEXT.repeat(4096);
        assert!(validate_preferences(&preferences).is_ok());

        preferences
            .opening_messages
            .get_mut("codex:unicode-thread")
            .unwrap()
            .responsibility = MULTIBYTE_TEXT.repeat(4097);
        assert!(validate_preferences(&preferences).is_err());
        preferences
            .opening_messages
            .get_mut("codex:unicode-thread")
            .unwrap()
            .responsibility = MULTIBYTE_TEXT.repeat(4096);
        preferences.router.as_mut().unwrap().fallbacks[0].condition = MULTIBYTE_TEXT.repeat(4097);
        assert!(validate_preferences(&preferences).is_err());
        preferences.router.as_mut().unwrap().fallbacks[0].condition = MULTIBYTE_TEXT.repeat(4096);

        preferences
            .router
            .as_mut()
            .unwrap()
            .responsibilities
            .insert(
                "opencode:router-opencode".to_string(),
                RouterResponsibility {
                    description: String::new(),
                    fallback: "none".to_string(),
                },
            );
        assert!(validate_preferences(&preferences).is_err());

        let router = preferences.router.as_mut().unwrap();
        router.responsibilities.clear();
        router.fallbacks.push(router.fallbacks[0].clone());
        assert!(validate_preferences(&preferences).is_err());
    }
}
