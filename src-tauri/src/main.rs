use std::collections::BTreeMap;
use std::env;
use std::ffi::OsString;
use std::fs;
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use axum::body::Body;
use axum::extract::ws::WebSocketUpgrade;
use axum::extract::{Path as AxumPath, Query, State};
use axum::http::{header, HeaderMap, Method, Response, StatusCode, Uri};
use axum::response::{Html, IntoResponse};
use axum::routing::{any, get};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tauri::{WebviewUrl, WebviewWindowBuilder};

mod codex_app_server;
mod favorites;
mod opencode_server;
mod session_map;

use codex_app_server::{find_codex_binary, CodexAppServer};
use favorites::{Favorite, MAX_FAVORITE_BODY_BYTES};
use opencode_server::{find_opencode_binary, OpenCodeServer};
use session_map::{ApplyOperationsRequest, CreateMapRequest, MAX_MAP_BODY_BYTES};

const MAX_PREFERENCES_BODY: usize = 1024 * 1024;
const MAX_REVIEW_FILE_BYTES: u64 = 2 * 1024 * 1024;

#[derive(Clone)]
struct GatewayState {
    codex: CodexAppServer,
    opencode: OpenCodeServer,
    preferences_path: Arc<PathBuf>,
    preferences_lock: Arc<Mutex<()>>,
    favorites_path: Arc<PathBuf>,
    favorites_lock: Arc<Mutex<()>>,
    session_maps_path: Arc<PathBuf>,
    session_maps_lock: Arc<Mutex<()>>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    target: Option<AnnotationTarget>,
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
    prefix: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    suffix: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct OpeningMessage {
    text: String,
    source: String,
    captured_at: String,
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
struct ThreadRouterPreferences {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    responsibilities: BTreeMap<String, RouterResponsibility>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct StudioPreferences {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    language: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    theme: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    content_width: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    typography: Option<TypographyPreferences>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    selected_thread: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    selected_backend: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    selected_threads: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    thread_activity: BTreeMap<String, u64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    attention_threads: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    annotation_drafts: BTreeMap<String, Vec<AnnotationDraft>>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    annotation_additional: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    annotation_prompt_template: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    annotation_prompt_templates: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    opening_messages: BTreeMap<String, OpeningMessage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    router: Option<ThreadRouterPreferences>,
    #[serde(default)]
    sidebar_collapsed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    artifact_width_ratio: Option<f64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BackendInfo {
    app_name: &'static str,
    app_version: &'static str,
    binary: String,
    protocol: &'static str,
    transport: &'static str,
    router_workspace: String,
}

#[derive(Default, Deserialize)]
struct FavoriteQuery {
    #[serde(default)]
    q: String,
    limit: Option<usize>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReviewFileRequest {
    root: String,
    path: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ReviewFileResponse {
    root: String,
    path: String,
    relative_path: String,
    content: String,
    hash: String,
    size: u64,
    line_count: usize,
    language: String,
}

fn main() {
    let cli_path = augmented_cli_path();
    let codex_binary = find_codex_binary(&cli_path);
    let opencode_binary = find_opencode_binary(&cli_path);
    let preferences_path = studio_preferences_path();
    let favorites_path = preferences_path.with_file_name("favorites.sqlite3");
    let session_maps_path = preferences_path.with_file_name("session-maps.sqlite3");
    if let Err(error) = migrate_legacy_preferences(&preferences_path) {
        eprintln!("Codex Thread Studio could not migrate legacy settings: {error}");
    }
    if let Err(error) = favorites::initialize(&favorites_path) {
        eprintln!("Codex Thread Studio could not initialize favorites: {error}");
    }
    if let Err(error) = session_map::initialize(&session_maps_path) {
        eprintln!("Codex Thread Studio could not initialize session maps: {error}");
    }
    let state = GatewayState {
        codex: CodexAppServer::new(codex_binary, cli_path),
        opencode: OpenCodeServer::new(opencode_binary, augmented_cli_path()),
        preferences_path: Arc::new(preferences_path),
        preferences_lock: Arc::new(Mutex::new(())),
        favorites_path: Arc::new(favorites_path),
        favorites_lock: Arc::new(Mutex::new(())),
        session_maps_path: Arc::new(session_maps_path),
        session_maps_lock: Arc::new(Mutex::new(())),
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
        .route("/i18n.mjs", get(i18n_js))
        .route("/codex-native.mjs", get(codex_native_js))
        .route("/opencode-native.mjs", get(opencode_native_js))
        .route("/thread-catalog.mjs", get(thread_catalog_js))
        .route("/thread-workset.mjs", get(thread_workset_js))
        .route("/composer-tools.mjs", get(composer_tools_js))
        .route("/document-review.mjs", get(document_review_js))
        .route("/favorites.mjs", get(favorites_js))
        .route("/session-map.mjs", get(session_map_js))
        .route("/thread-router.mjs", get(thread_router_js))
        .route("/turn-navigator.mjs", get(turn_navigator_js))
        .route("/transcript-scroll.mjs", get(transcript_scroll_js))
        .route(
            "/transcript-presentation.mjs",
            get(transcript_presentation_js),
        )
        .route("/vendor/marked.esm.js", get(marked_js))
        .route("/vendor/purify.es.mjs", get(dompurify_js))
        .route("/vendor/github-markdown.css", get(github_markdown_css))
        .route("/styles.css", get(styles_css))
        .route("/studio/codex", get(codex_info))
        .route("/studio/opencode", get(opencode_info))
        .route(
            "/studio/preferences",
            get(get_preferences).put(put_preferences),
        )
        .route("/studio/client-log", axum::routing::post(client_log))
        .route("/studio/review-file", axum::routing::post(read_review_file))
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
        .route("/opencode/{*path}", any(proxy_opencode))
        .with_state(state)
}

async fn index() -> impl IntoResponse {
    (
        [(header::CACHE_CONTROL, "no-store")],
        Html(include_str!("../../ui/index.html")),
    )
}

async fn read_review_file(Json(request): Json<ReviewFileRequest>) -> Response<Body> {
    match load_review_file(&request) {
        Ok(file) => json_response(StatusCode::OK, &file),
        Err((status, message)) => json_error(status, &message),
    }
}

fn load_review_file(
    request: &ReviewFileRequest,
) -> Result<ReviewFileResponse, (StatusCode, String)> {
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
    if !path.starts_with(&root) {
        return Err((
            StatusCode::FORBIDDEN,
            "file is outside the project directory".to_string(),
        ));
    }
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
    if metadata.len() > MAX_REVIEW_FILE_BYTES {
        return Err((
            StatusCode::PAYLOAD_TOO_LARGE,
            "file exceeds the 2 MiB review limit".to_string(),
        ));
    }
    let bytes = fs::read(&path).map_err(|error| {
        (
            StatusCode::BAD_REQUEST,
            format!("unable to read file: {error}"),
        )
    })?;
    let content = String::from_utf8(bytes).map_err(|_| {
        (
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "only UTF-8 text files can be reviewed".to_string(),
        )
    })?;
    let relative_path = path
        .strip_prefix(&root)
        .unwrap_or(&path)
        .to_string_lossy()
        .into_owned();
    Ok(ReviewFileResponse {
        root: root.to_string_lossy().into_owned(),
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
    })
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

async fn i18n_js() -> impl IntoResponse {
    javascript(include_str!("../../ui/i18n.mjs"))
}

async fn codex_native_js() -> impl IntoResponse {
    javascript(include_str!("../../ui/codex-native.mjs"))
}

async fn opencode_native_js() -> impl IntoResponse {
    javascript(include_str!("../../ui/opencode-native.mjs"))
}

async fn transcript_presentation_js() -> impl IntoResponse {
    javascript(include_str!("../../ui/transcript-presentation.mjs"))
}

async fn thread_catalog_js() -> impl IntoResponse {
    javascript(include_str!("../../ui/thread-catalog.mjs"))
}

async fn thread_workset_js() -> impl IntoResponse {
    javascript(include_str!("../../ui/thread-workset.mjs"))
}

async fn composer_tools_js() -> impl IntoResponse {
    javascript(include_str!("../../ui/composer-tools.mjs"))
}

async fn document_review_js() -> impl IntoResponse {
    javascript(include_str!("../../ui/document-review.mjs"))
}

async fn favorites_js() -> impl IntoResponse {
    javascript(include_str!("../../ui/favorites.mjs"))
}

async fn session_map_js() -> impl IntoResponse {
    javascript(include_str!("../../ui/session-map.mjs"))
}

async fn thread_router_js() -> impl IntoResponse {
    javascript(include_str!("../../ui/thread-router.mjs"))
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
    let router_workspace = studio_router_workspace_path();
    if let Err(error) = fs::create_dir_all(&router_workspace) {
        eprintln!("failed to create Studio Router workspace: {error}");
    }
    axum::Json(BackendInfo {
        app_name: "Codex Thread Studio",
        app_version: env!("CARGO_PKG_VERSION"),
        binary: state.codex.binary().to_string(),
        protocol: "Codex App Server v2",
        transport: "stdio JSONL via Studio WebSocket",
        router_workspace: router_workspace.to_string_lossy().into_owned(),
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

async fn client_log(body: String) -> StatusCode {
    let message = body.chars().take(16 * 1024).collect::<String>();
    eprintln!("Studio WebView: {message}");
    StatusCode::NO_CONTENT
}

async fn list_favorites(
    State(state): State<GatewayState>,
    Query(query): Query<FavoriteQuery>,
) -> Response<Body> {
    let _guard = match state.favorites_lock.lock() {
        Ok(guard) => guard,
        Err(_) => return gateway_error("favorites lock is unavailable"),
    };
    match favorites::list(&state.favorites_path, &query.q, query.limit.unwrap_or(100)) {
        Ok(items) => json_response(StatusCode::OK, &items),
        Err(error) => gateway_error(&format!("failed to read favorites: {error}")),
    }
}

async fn get_favorite(
    State(state): State<GatewayState>,
    AxumPath(id): AxumPath<String>,
) -> Response<Body> {
    let _guard = match state.favorites_lock.lock() {
        Ok(guard) => guard,
        Err(_) => return gateway_error("favorites lock is unavailable"),
    };
    match favorites::find(&state.favorites_path, &id) {
        Ok(Some(favorite)) => json_response(StatusCode::OK, &favorite),
        Ok(None) => json_error(StatusCode::NOT_FOUND, "favorite not found"),
        Err(error) => gateway_error(&format!("failed to read favorites: {error}")),
    }
}

async fn create_favorite(State(state): State<GatewayState>, body: String) -> Response<Body> {
    let favorite = match parse_favorite_body(&body) {
        Ok(favorite) => favorite,
        Err((status, message)) => return json_error(status, &message),
    };
    let _guard = match state.favorites_lock.lock() {
        Ok(guard) => guard,
        Err(_) => return gateway_error("favorites lock is unavailable"),
    };
    match favorites::insert(&state.favorites_path, favorite) {
        Ok(favorite) => json_response(StatusCode::CREATED, &favorite),
        Err(error) => json_error(StatusCode::BAD_REQUEST, &error),
    }
}

async fn update_favorite(
    State(state): State<GatewayState>,
    AxumPath(id): AxumPath<String>,
    body: String,
) -> Response<Body> {
    let favorite = match parse_favorite_body(&body) {
        Ok(favorite) => favorite,
        Err((status, message)) => return json_error(status, &message),
    };
    let _guard = match state.favorites_lock.lock() {
        Ok(guard) => guard,
        Err(_) => return gateway_error("favorites lock is unavailable"),
    };
    match favorites::update(&state.favorites_path, &id, favorite) {
        Ok(Some(favorite)) => json_response(StatusCode::OK, &favorite),
        Ok(None) => json_error(StatusCode::NOT_FOUND, "favorite not found"),
        Err(error) => json_error(StatusCode::BAD_REQUEST, &error),
    }
}

async fn delete_favorite(
    State(state): State<GatewayState>,
    AxumPath(id): AxumPath<String>,
) -> Response<Body> {
    let _guard = match state.favorites_lock.lock() {
        Ok(guard) => guard,
        Err(_) => return gateway_error("favorites lock is unavailable"),
    };
    match favorites::remove(&state.favorites_path, &id) {
        Ok(Some(favorite)) => json_response(StatusCode::OK, &favorite),
        Ok(None) => json_error(StatusCode::NOT_FOUND, "favorite not found"),
        Err(error) => gateway_error(&format!("failed to save favorites: {error}")),
    }
}

async fn export_favorites(State(state): State<GatewayState>) -> Response<Body> {
    let _guard = match state.favorites_lock.lock() {
        Ok(guard) => guard,
        Err(_) => return gateway_error("favorites lock is unavailable"),
    };
    match favorites::export_markdown(&state.favorites_path) {
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
    if preferences
        .artifact_width_ratio
        .is_some_and(|ratio| !(0.2..=0.65).contains(&ratio))
    {
        return Err("artifact width ratio must be between 0.2 and 0.65".to_string());
    }
    if preferences
        .selected_thread
        .as_ref()
        .is_some_and(|value| value.len() > 256)
    {
        return Err("selected thread id is too long".to_string());
    }
    if preferences
        .selected_backend
        .as_deref()
        .is_some_and(|backend| !matches!(backend, "codex" | "opencode"))
    {
        return Err("selected backend must be codex or opencode".to_string());
    }
    if preferences.selected_threads.len() > 2
        || preferences
            .selected_threads
            .iter()
            .any(|(backend, thread_id)| {
                !matches!(backend.as_str(), "codex" | "opencode") || thread_id.len() > 256
            })
    {
        return Err("selected backend threads are invalid".to_string());
    }
    if preferences.thread_activity.len() > 2048
        || preferences.thread_activity.keys().any(|key| {
            key.len() > 272 || !(key.starts_with("codex:") || key.starts_with("opencode:"))
        })
    {
        return Err("thread activity preferences are invalid".to_string());
    }
    if preferences.attention_threads.len() > 2048
        || preferences.attention_threads.iter().any(|key| {
            key.len() > 272 || !(key.starts_with("codex:") || key.starts_with("opencode:"))
        })
    {
        return Err("attention thread preferences are invalid".to_string());
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
                            || draft.target.as_ref().is_some_and(|target| {
                                !matches!(target.kind.as_str(), "chatRange" | "fileRange")
                                    || target.file_path.as_ref().is_some_and(|value| value.len() > 4096)
                                    || target.root.as_ref().is_some_and(|value| value.len() > 4096)
                                    || target.base_hash.as_ref().is_some_and(|value| value.len() > 128)
                                    || target.prefix.as_ref().is_some_and(|value| value.len() > 256)
                                    || target.suffix.as_ref().is_some_and(|value| value.len() > 256)
                                    || matches!((target.start_offset, target.end_offset), (Some(start), Some(end)) if start > end)
                                    || (target.kind == "fileRange" && target.file_path.as_deref().unwrap_or_default().is_empty())
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
    if preferences
        .annotation_prompt_template
        .as_ref()
        .is_some_and(|template| template.len() > 32 * 1024 || !template.contains("{{annotations}}"))
    {
        return Err("annotation template must contain {{annotations}}".to_string());
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
                || message.source.len() > 64
                || message.captured_at.len() > 128
        })
    {
        return Err("opening message preferences are invalid".to_string());
    }
    if let Some(router) = &preferences.router {
        if router
            .thread_id
            .as_ref()
            .is_some_and(|id| id.is_empty() || id.len() > 256)
            || router.responsibilities.len() > 2048
            || router.responsibilities.iter().any(|(id, responsibility)| {
                id.is_empty()
                    || id.len() > 256
                    || router.thread_id.as_ref() == Some(id)
                    || responsibility.description.len() > 4096
                    || !matches!(responsibility.fallback.as_str(), "none" | "fallback")
            })
        {
            return Err("thread router preferences are invalid".to_string());
        }
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
                opencode: OpenCodeServer::new("opencode".to_string(), OsString::new()),
                preferences_path: Arc::new(
                    env::temp_dir().join("codex-thread-studio-route-test.json"),
                ),
                preferences_lock: Arc::new(Mutex::new(())),
                favorites_path: Arc::new(
                    env::temp_dir().join("codex-thread-studio-route-favorites-test.sqlite3"),
                ),
                favorites_lock: Arc::new(Mutex::new(())),
                session_maps_path: Arc::new(
                    env::temp_dir().join("codex-thread-studio-route-maps-test.sqlite3"),
                ),
                session_maps_lock: Arc::new(Mutex::new(())),
            };
            let router = gateway_router(state);
            for path in [
                "/",
                "/app.js",
                "/i18n.mjs",
                "/codex-native.mjs",
                "/opencode-native.mjs",
                "/thread-catalog.mjs",
                "/thread-workset.mjs",
                "/composer-tools.mjs",
                "/document-review.mjs",
                "/favorites.mjs",
                "/session-map.mjs",
                "/turn-navigator.mjs",
                "/transcript-scroll.mjs",
                "/transcript-presentation.mjs",
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
                opencode: OpenCodeServer::new("opencode".to_string(), OsString::new()),
                preferences_path: Arc::new(
                    env::temp_dir().join("codex-thread-studio-version-test.json"),
                ),
                preferences_lock: Arc::new(Mutex::new(())),
                favorites_path: Arc::new(
                    env::temp_dir().join("codex-thread-studio-version-favorites-test.sqlite3"),
                ),
                favorites_lock: Arc::new(Mutex::new(())),
                session_maps_path: Arc::new(
                    env::temp_dir().join("codex-thread-studio-version-maps-test.sqlite3"),
                ),
                session_maps_lock: Arc::new(Mutex::new(())),
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
                codex: CodexAppServer::new("codex".to_string(), OsString::new()),
                opencode: OpenCodeServer::new("opencode".to_string(), OsString::new()),
                preferences_path: Arc::new(
                    env::temp_dir().join("codex-thread-studio-map-api-settings.json"),
                ),
                preferences_lock: Arc::new(Mutex::new(())),
                favorites_path: Arc::new(
                    env::temp_dir().join("codex-thread-studio-map-api-favorites.sqlite3"),
                ),
                favorites_lock: Arc::new(Mutex::new(())),
                session_maps_path: Arc::new(maps_path.clone()),
                session_maps_lock: Arc::new(Mutex::new(())),
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
            let favorites_path = env::temp_dir().join(unique);
            let state = GatewayState {
                codex: CodexAppServer::new("codex".to_string(), OsString::new()),
                opencode: OpenCodeServer::new("opencode".to_string(), OsString::new()),
                preferences_path: Arc::new(
                    env::temp_dir().join("codex-thread-studio-favorites-api-settings.json"),
                ),
                preferences_lock: Arc::new(Mutex::new(())),
                favorites_path: Arc::new(favorites_path.clone()),
                favorites_lock: Arc::new(Mutex::new(())),
                session_maps_path: Arc::new(
                    env::temp_dir().join("codex-thread-studio-favorites-api-maps-test.sqlite3"),
                ),
                session_maps_lock: Arc::new(Mutex::new(())),
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
            fs::remove_file(favorites_path).ok();
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
            }],
        );
        assert!(validate_preferences(&preferences).is_ok());
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

        let escaped = load_review_file(&ReviewFileRequest {
            root: root.to_string_lossy().into_owned(),
            path: outside.to_string_lossy().into_owned(),
        });
        assert!(matches!(escaped, Err((StatusCode::FORBIDDEN, _))));

        fs::remove_file(root.join("docs/guide.md")).ok();
        fs::remove_file(&outside).ok();
        fs::remove_dir(root.join("docs")).ok();
        fs::remove_dir(&root).ok();
        fs::remove_dir(&base).ok();
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
        preferences
            .annotation_prompt_templates
            .insert("zh-CN".to_string(), "请处理：\n{{annotations}}".to_string());
        assert!(validate_preferences(&preferences).is_ok());
        preferences.annotation_prompt_templates.insert(
            "ja-JP".to_string(),
            "コメント：\n{{annotations}}".to_string(),
        );
        assert!(validate_preferences(&preferences).is_err());
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
            .attention_threads
            .push("codex:thread-1".to_string());
        assert!(validate_preferences(&preferences).is_ok());

        preferences
            .thread_activity
            .insert("unknown:thread-1".to_string(), 1);
        assert!(validate_preferences(&preferences).is_err());

        preferences.thread_activity.remove("unknown:thread-1");
        preferences
            .attention_threads
            .push("unknown:thread-1".to_string());
        assert!(validate_preferences(&preferences).is_err());
    }

    #[test]
    fn validates_thread_router_preferences() {
        let mut preferences = StudioPreferences::default();
        preferences.router = Some(ThreadRouterPreferences {
            thread_id: Some("router-thread".to_string()),
            responsibilities: BTreeMap::from([
                (
                    "learn-thread".to_string(),
                    RouterResponsibility {
                        description: "Books and structured learning".to_string(),
                        fallback: "fallback".to_string(),
                    },
                ),
                (
                    "general-thread".to_string(),
                    RouterResponsibility {
                        description: "Requests without a better match".to_string(),
                        fallback: "fallback".to_string(),
                    },
                ),
            ]),
        });
        assert!(validate_preferences(&preferences).is_ok());

        preferences
            .router
            .as_mut()
            .unwrap()
            .responsibilities
            .insert(
                "router-thread".to_string(),
                RouterResponsibility {
                    description: String::new(),
                    fallback: "none".to_string(),
                },
            );
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
