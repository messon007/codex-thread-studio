use std::collections::BTreeMap;
use std::env;
use std::ffi::OsString;
use std::fs;
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use axum::body::Body;
use axum::extract::ws::WebSocketUpgrade;
use axum::extract::State;
use axum::http::{header, Response, StatusCode};
use axum::response::{Html, IntoResponse};
use axum::routing::get;
use axum::Router;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tauri::{WebviewUrl, WebviewWindowBuilder};

mod codex_app_server;

use codex_app_server::{find_codex_binary, CodexAppServer};

const MAX_PREFERENCES_BODY: usize = 1024 * 1024;

#[derive(Clone)]
struct GatewayState {
    codex: CodexAppServer,
    preferences_path: Arc<PathBuf>,
    preferences_lock: Arc<Mutex<()>>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct TypographyPreferences {
    ui_font_family: String,
    ui_font_weight: u16,
    code_font_family: String,
    code_font_size: f64,
    code_font_weight: u16,
    high_contrast: bool,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct AnnotationDraft {
    id: String,
    quote: String,
    comment: String,
    created_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    item_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    turn_id: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct StudioPreferences {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    theme: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    content_width: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    typography: Option<TypographyPreferences>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    selected_thread: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    annotation_drafts: BTreeMap<String, Vec<AnnotationDraft>>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    annotation_additional: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    annotation_prompt_template: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CodexInfo {
    app_name: &'static str,
    app_version: &'static str,
    binary: String,
    protocol: &'static str,
    transport: &'static str,
}

fn main() {
    let cli_path = augmented_cli_path();
    let codex_binary = find_codex_binary(&cli_path);
    let preferences_path = studio_preferences_path();
    if let Err(error) = migrate_legacy_preferences(&preferences_path) {
        eprintln!("Codex Thread Studio could not migrate legacy settings: {error}");
    }
    let state = GatewayState {
        codex: CodexAppServer::new(codex_binary, cli_path),
        preferences_path: Arc::new(preferences_path),
        preferences_lock: Arc::new(Mutex::new(())),
    };

    let gateway_listener = TcpListener::bind("127.0.0.1:0")
        .expect("failed to reserve a local Codex Thread Studio gateway port");
    gateway_listener
        .set_nonblocking(true)
        .expect("failed to configure Codex Thread Studio gateway socket");
    let gateway_addr = gateway_listener
        .local_addr()
        .expect("failed to determine Codex Thread Studio gateway address");

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
            WebviewWindowBuilder::new(app, "main", WebviewUrl::External(url))
                .title("Codex Thread Studio")
                .inner_size(1400.0, 900.0)
                .min_inner_size(980.0, 660.0)
                .build()?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("failed to run Codex Thread Studio desktop application");
}

fn gateway_router(state: GatewayState) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/app.js", get(app_js))
        .route("/codex-native.mjs", get(codex_native_js))
        .route("/composer-tools.mjs", get(composer_tools_js))
        .route("/turn-navigator.mjs", get(turn_navigator_js))
        .route("/vendor/marked.esm.js", get(marked_js))
        .route("/vendor/purify.es.mjs", get(dompurify_js))
        .route("/vendor/github-markdown.css", get(github_markdown_css))
        .route("/styles.css", get(styles_css))
        .route("/studio/codex", get(codex_info))
        .route(
            "/studio/preferences",
            get(get_preferences).put(put_preferences),
        )
        .route("/ws/codex", get(codex_app_server_ws))
        .with_state(state)
}

async fn index() -> impl IntoResponse {
    (
        [(header::CACHE_CONTROL, "no-store")],
        Html(include_str!("../../ui/index.html")),
    )
}

async fn app_js() -> impl IntoResponse {
    javascript(include_str!("../../ui/app.js"))
}

async fn codex_native_js() -> impl IntoResponse {
    javascript(include_str!("../../ui/codex-native.mjs"))
}

async fn composer_tools_js() -> impl IntoResponse {
    javascript(include_str!("../../ui/composer-tools.mjs"))
}

async fn turn_navigator_js() -> impl IntoResponse {
    javascript(include_str!("../../ui/turn-navigator.mjs"))
}

async fn marked_js() -> impl IntoResponse {
    javascript(include_str!("../../ui/vendor/marked.esm.js"))
}

async fn dompurify_js() -> impl IntoResponse {
    javascript(include_str!("../../ui/vendor/purify.es.mjs"))
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

async fn codex_info(State(state): State<GatewayState>) -> impl IntoResponse {
    axum::Json(CodexInfo {
        app_name: "Codex Thread Studio",
        app_version: env!("CARGO_PKG_VERSION"),
        binary: state.codex.binary().to_string(),
        protocol: "Codex App Server v2",
        transport: "stdio JSONL via Studio WebSocket",
    })
}

async fn codex_app_server_ws(
    ws: WebSocketUpgrade,
    State(state): State<GatewayState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| async move { state.codex.bridge(socket).await })
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
        Ok(()) => json_response(StatusCode::OK, &preferences),
        Err(error) => gateway_error(&format!("failed to save Studio preferences: {error}")),
    }
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

fn legacy_preferences_path() -> PathBuf {
    if let Some(path) = env::var_os("XDG_CONFIG_HOME").filter(|path| !path.is_empty()) {
        return PathBuf::from(path)
            .join("agent-deck-studio")
            .join("codex-native-settings.json");
    }
    if let Some(home) = env::var_os("HOME") {
        return PathBuf::from(home).join(".config/agent-deck-studio/codex-native-settings.json");
    }
    env::temp_dir().join("agent-deck-studio-codex-native-settings.json")
}

fn migrate_legacy_preferences(target: &std::path::Path) -> Result<(), String> {
    migrate_preferences_file(&legacy_preferences_path(), target)
}

fn migrate_preferences_file(
    legacy: &std::path::Path,
    target: &std::path::Path,
) -> Result<(), String> {
    if target.exists() || !legacy.exists() {
        return Ok(());
    }
    let preferences = load_preferences(legacy)?;
    validate_preferences(&preferences)?;
    save_preferences(target, &preferences)
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
    if preferences
        .selected_thread
        .as_ref()
        .is_some_and(|value| value.len() > 256)
    {
        return Err("selected thread id is too long".to_string());
    }
    if let Some(typography) = &preferences.typography {
        if typography.ui_font_family.trim().is_empty()
            || typography.ui_font_family.len() > 512
            || typography.code_font_family.trim().is_empty()
            || typography.code_font_family.len() > 512
            || ![400, 500, 600].contains(&typography.ui_font_weight)
            || ![400, 500, 600].contains(&typography.code_font_weight)
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
                            || draft.quote.len() > 16 * 1024
                            || draft.comment.len() > 16 * 1024
                            || draft.created_at.len() > 128
                            || draft
                                .item_id
                                .as_ref()
                                .is_some_and(|value| value.len() > 256)
                            || draft
                                .turn_id
                                .as_ref()
                                .is_some_and(|value| value.len() > 256)
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
    if preferences
        .annotation_prompt_template
        .as_ref()
        .is_some_and(|template| template.len() > 32 * 1024 || !template.contains("{{annotations}}"))
    {
        return Err("annotation template must contain {{annotations}}".to_string());
    }
    Ok(())
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
    fn serves_all_embedded_frontend_modules() {
        let runtime = tokio::runtime::Runtime::new().expect("test runtime");
        runtime.block_on(async {
            let state = GatewayState {
                codex: CodexAppServer::new("codex".to_string(), OsString::new()),
                preferences_path: Arc::new(
                    env::temp_dir().join("codex-thread-studio-route-test.json"),
                ),
                preferences_lock: Arc::new(Mutex::new(())),
            };
            let router = gateway_router(state);
            for path in [
                "/",
                "/app.js",
                "/codex-native.mjs",
                "/composer-tools.mjs",
                "/turn-navigator.mjs",
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
        });
    }

    #[test]
    fn exposes_the_compile_time_application_version() {
        let runtime = tokio::runtime::Runtime::new().expect("test runtime");
        runtime.block_on(async {
            let state = GatewayState {
                codex: CodexAppServer::new("codex".to_string(), OsString::new()),
                preferences_path: Arc::new(
                    env::temp_dir().join("codex-thread-studio-version-test.json"),
                ),
                preferences_lock: Arc::new(Mutex::new(())),
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
            }],
        );
        assert!(validate_preferences(&preferences).is_ok());
    }

    #[test]
    fn rejects_a_template_without_annotations_slot() {
        let preferences = StudioPreferences {
            annotation_prompt_template: Some("no placeholder".to_string()),
            ..StudioPreferences::default()
        };
        assert!(validate_preferences(&preferences).is_err());
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
    fn migrates_legacy_preferences_once() {
        let unique = format!(
            "codex-thread-studio-migration-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time after epoch")
                .as_nanos()
        );
        let root = env::temp_dir().join(unique);
        let legacy = root.join("legacy/settings.json");
        let target = root.join("current/settings.json");
        let preferences = StudioPreferences {
            theme: Some("dark".to_string()),
            content_width: Some("wide".to_string()),
            ..StudioPreferences::default()
        };
        save_preferences(&legacy, &preferences).expect("write legacy settings");
        migrate_preferences_file(&legacy, &target).expect("migrate settings");
        assert_eq!(load_preferences(&target).unwrap(), preferences);

        let replacement = StudioPreferences {
            theme: Some("light".to_string()),
            ..StudioPreferences::default()
        };
        save_preferences(&legacy, &replacement).expect("replace legacy settings");
        migrate_preferences_file(&legacy, &target).expect("skip existing target");
        assert_eq!(load_preferences(&target).unwrap(), preferences);
        fs::remove_dir_all(root).expect("remove migration test directory");
    }
}
