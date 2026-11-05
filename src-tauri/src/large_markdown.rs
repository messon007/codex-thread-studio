use crate::{
    json_error, json_response, resolve_review_path_with_shared, review_language,
    shared_document_directories, GatewayState, ReviewFileRequest,
};
use axum::{
    body::{to_bytes, Body},
    extract::{Path, Request, State},
    http::{header, Method, Response, StatusCode},
    Json,
};
use marknoria::{
    reader::FileGrant,
    web::{DiagramMode, Service, ServiceOptions},
};
use serde::Deserialize;
use serde_json::json;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tower::ServiceExt;

pub const THRESHOLD: u64 = 1024 * 1024;
const MAX_READERS: usize = 4;
const IDLE: Duration = Duration::from_secs(120);

struct Reader {
    service: Service,
    touched: Instant,
    editable: bool,
    source: std::path::PathBuf,
    root: std::path::PathBuf,
}

#[derive(Default)]
struct ReaderMaintenance {
    task: Mutex<Option<tokio::task::JoinHandle<()>>>,
}
impl ReaderMaintenance {
    fn ensure(&self, entries: &Arc<Mutex<HashMap<String, Reader>>>) {
        let mut task = self.task.lock().unwrap();
        if task.as_ref().is_some_and(|task| !task.is_finished()) {
            return;
        }
        let entries = Arc::downgrade(entries);
        *task = Some(tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(30)).await;
                let Some(entries) = entries.upgrade() else {
                    break;
                };
                let expired = take_expired(&mut entries.lock().unwrap());
                close_readers(expired).await;
            }
        }));
    }
}
impl Drop for ReaderMaintenance {
    fn drop(&mut self) {
        if let Some(task) = self
            .task
            .get_mut()
            .unwrap_or_else(|error| error.into_inner())
            .take()
        {
            task.abort();
        }
    }
}

#[derive(Clone)]
pub struct Readers {
    entries: Arc<Mutex<HashMap<String, Reader>>>,
    opening: Arc<tokio::sync::Semaphore>,
    client: reqwest::Client,
    maintenance: Arc<ReaderMaintenance>,
}

impl Default for Readers {
    fn default() -> Self {
        Self {
            entries: Arc::new(Mutex::new(HashMap::new())),
            maintenance: Arc::new(ReaderMaintenance::default()),
            opening: Arc::new(tokio::sync::Semaphore::new(1)),
            client: reqwest::Client::builder()
                .no_proxy()
                .redirect(reqwest::redirect::Policy::none())
                .timeout(Duration::from_secs(30))
                .build()
                .expect("reader HTTP client"),
        }
    }
}

fn take_expired(entries: &mut HashMap<String, Reader>) -> Vec<Reader> {
    let ids: Vec<_> = entries
        .iter()
        .filter(|(_, reader)| reader.touched.elapsed() >= IDLE)
        .map(|(id, _)| id.clone())
        .collect();
    ids.into_iter()
        .filter_map(|id| entries.remove(&id))
        .collect()
}

async fn close_readers(readers: Vec<Reader>) {
    for reader in readers {
        if let Err(error) = reader.service.close().await {
            eprintln!("Studio reader cleanup failed: {error}");
        }
    }
}

impl Readers {
    pub async fn open(
        &self,
        source: std::path::PathBuf,
        root: std::path::PathBuf,
        editable: bool,
    ) -> Result<serde_json::Value, (StatusCode, String)> {
        let _permit = self.opening.clone().try_acquire_owned().map_err(|_| {
            (
                StatusCode::TOO_MANY_REQUESTS,
                "Another document is opening".into(),
            )
        })?;
        let expired = take_expired(&mut self.entries.lock().unwrap());
        close_readers(expired).await;
        if self.entries.lock().unwrap().len() >= MAX_READERS {
            return Err((
                StatusCode::TOO_MANY_REQUESTS,
                "Close an existing document first".into(),
            ));
        }
        let grant = if editable {
            FileGrant::read_write(&source)
        } else {
            FileGrant::read_only(&source)
        }
        .map_err(|error| (StatusCode::FORBIDDEN, error.to_string()))?;
        let service = Service::start_with_options(
            grant,
            ServiceOptions::default().diagrams(DiagramMode::Browser),
        )
        .await
        .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;
        let id = uuid::Uuid::new_v4().to_string();
        let token = service.token.clone();
        self.entries.lock().unwrap().insert(
            id.clone(),
            Reader {
                service,
                touched: Instant::now(),
                editable,
                source,
                root,
            },
        );
        self.maintenance.ensure(&self.entries);
        Ok(
            json!({"id":id,"url":format!("/markdown-reader/{id}/?embedded=1&diagrams=browser#{token}")}),
        )
    }
}

pub async fn try_open(
    state: &GatewayState,
    request: &ReviewFileRequest,
) -> Result<Option<serde_json::Value>, (StatusCode, String)> {
    let json = matches!(
        std::path::Path::new(&request.path)
            .extension()
            .and_then(|value| value.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("json" | "jsonl" | "ndjson" | "geojson" | "jsonc")
    );
    if !json && review_language(std::path::Path::new(&request.path)) != "markdown" {
        return Ok(None);
    }
    let shared = shared_document_directories(state)?;
    let review = ReviewFileRequest {
        root: request.root.clone(),
        path: request.path.clone(),
    };
    let (root, path, metadata, document_root) = tokio::task::spawn_blocking(move || {
        resolve_review_path_with_shared(
            &review,
            marknoria::reader::Budgets::default().source_bytes,
            "128 MiB Markdown",
            &shared,
        )
    })
    .await
    .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))??;
    if !json && metadata.len() < THRESHOLD {
        return Ok(None);
    }
    let read_only = !path.starts_with(&root);
    let reader = state
        .markdown_readers
        .open(path.clone(), root.clone(), !read_only)
        .await?;
    Ok(Some(json!({
        "root":root,"path":path,"relativePath":path.strip_prefix(&document_root).unwrap_or(&path),
        "documentRoot":document_root,"size":metadata.len(),"readOnly":read_only,"largeMarkdown":reader,
    })))
}

/// General shared Web reader entry. The project/shared-directory grant remains
/// Studio's responsibility; source writes are never granted to binary formats.
pub async fn open_document(
    State(state): State<GatewayState>,
    Json(request): Json<ReviewFileRequest>,
) -> Response<Body> {
    #[cfg(windows)]
    if state.codex.execution_environment() == "wsl" {
        return json_error(
            StatusCode::NOT_IMPLEMENTED,
            "Shared reader byte transport for WSL is not implemented yet",
        );
    }
    let shared = match shared_document_directories(&state) {
        Ok(shared) => shared,
        Err((status, message)) => return json_error(status, &message),
    };
    let resolved = tokio::task::spawn_blocking(move || {
        resolve_review_path_with_shared(
            &request,
            marknoria::reader::Budgets::default().source_bytes,
            "128 MiB document",
            &shared,
        )
    })
    .await;
    let (root, path, metadata, document_root) = match resolved {
        Ok(Ok(value)) => value,
        Ok(Err((status, message))) => return json_error(status, &message),
        Err(error) => return json_error(StatusCode::INTERNAL_SERVER_ERROR, &error.to_string()),
    };
    let Some(format) = marknoria::reader::DocumentFormat::for_path(&path) else {
        return json_error(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "Unsupported shared reader document format",
        );
    };
    let read_only = !path.starts_with(&root) || !format.is_text();
    match state
        .markdown_readers
        .open(path.clone(), root.clone(), !read_only)
        .await
    {
        Ok(reader) => json_response(
            StatusCode::OK,
            &json!({
                "root":root,"path":path,"relativePath":path.strip_prefix(&document_root).unwrap_or(&path),
                "documentRoot":document_root,"size":metadata.len(),"readOnly":read_only,
                "format":format,"sourceAvailable":format.is_text(),"reader":reader,
            }),
        ),
        Err((status, message)) => json_error(status, &message),
    }
}

#[derive(Deserialize)]
pub struct CloseRequest {
    id: String,
}

pub async fn close(
    State(state): State<GatewayState>,
    Json(request): Json<CloseRequest>,
) -> Response<Body> {
    let removed = state
        .markdown_readers
        .entries
        .lock()
        .unwrap()
        .remove(&request.id);
    if let Some(reader) = removed {
        if let Err(error) = reader.service.close().await {
            return json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                &format!("Reader cleanup failed: {error}"),
            );
        }
    }
    json_response(StatusCode::OK, &json!({"closed":true}))
}

fn valid_path(path: &str) -> bool {
    path.len() <= 4096
        && !path.contains(['\\', '\0'])
        && !path.split('/').any(|part| part == "." || part == "..")
}

fn allowed_path(path: &str) -> bool {
    valid_path(path)
        && (matches!(
            path,
            "" | "api/document"
                | "api/capabilities"
                | "api/operations"
                | "api/selection/source"
                | "api/window"
                | "api/projection"
                | "api/table"
                | "api/source"
                | "api/edit"
                | "api/search"
                | "api/diagram-source"
                | "api/renderer"
                | "api/table-query"
                | "api/table-query/schema"
                | "api/table-query/close"
                | "api/table-chart"
                | "api/jq-query"
                | "api/notes"
                | "api/asset"
                | "api/asset-info"
                | "api/original"
                | "api/epub"
        ) || path.strip_prefix("epub-resource/").is_some_and(|suffix| {
            let mut parts = suffix.splitn(3, '/');
            uuid::Uuid::parse_str(parts.next().unwrap_or("")).is_ok()
                && uuid::Uuid::parse_str(parts.next().unwrap_or("")).is_ok()
                && parts.next().is_some_and(|name| !name.is_empty())
        }))
}

fn edit_scope(source: &std::path::Path, root: &std::path::Path) -> bool {
    source.starts_with(root)
        && source.canonicalize().ok().as_deref() == Some(source)
        && root.canonicalize().ok().as_deref() == Some(root)
}

pub async fn page(
    State(state): State<GatewayState>,
    Path(id): Path<String>,
    request: Request,
) -> Response<Body> {
    proxy(state, id, String::new(), request).await
}

pub async fn asset(
    State(state): State<GatewayState>,
    Path((id, path)): Path<(String, String)>,
    request: Request,
) -> Response<Body> {
    proxy(state, id, path, request).await
}

async fn proxy(state: GatewayState, id: String, path: String, request: Request) -> Response<Body> {
    if state
        .security
        .validate_reader_origin(request.headers())
        .is_err()
    {
        return json_error(StatusCode::FORBIDDEN, "Reader origin rejected");
    }
    if !valid_path(&path) || path == "render.html" {
        return json_error(StatusCode::NOT_FOUND, "Unknown reader route");
    }
    if !path.is_empty() && !path.starts_with("api/") && !path.starts_with("epub-resource/") {
        if request.method() != Method::GET {
            return json_error(StatusCode::METHOD_NOT_ALLOWED, "Reader method rejected");
        }
        {
            let mut entries = state.markdown_readers.entries.lock().unwrap();
            let Some(reader) = entries.get_mut(&id) else {
                return json_error(StatusCode::GONE, "Document closed or expired; reopen it");
            };
            reader.touched = Instant::now();
        }
        let prefix = format!("/markdown-reader/{id}/");
        let Some(raw) = request.uri().path().strip_prefix(&prefix) else {
            return json_error(StatusCode::BAD_REQUEST, "Invalid reader route prefix");
        };
        // The upstream stateless router alone defines its resource inventory.
        // It has no document, path, query or write endpoints. No host headers,
        // token or request body enter this asset request.
        let asset = Request::builder()
            .uri(format!("/{raw}"))
            .body(Body::empty())
            .unwrap();
        return marknoria::web::reader_assets()
            .oneshot(asset)
            .await
            .unwrap();
    }
    if !allowed_path(&path) {
        return json_error(StatusCode::NOT_FOUND, "Unknown reader route");
    }
    if request.method() != Method::GET
        && !(matches!(
            path.as_str(),
            "api/edit"
                | "api/table-query"
                | "api/table-query/close"
                | "api/jq-query"
                | "api/table-chart"
                | "api/operations"
                | "api/selection/source"
                | "api/notes"
        ) && request.method() == Method::POST)
    {
        return json_error(StatusCode::METHOD_NOT_ALLOWED, "Reader method rejected");
    }
    let (address, source, root) = {
        let mut entries = state.markdown_readers.entries.lock().unwrap();
        let Some(reader) = entries.get_mut(&id) else {
            return json_error(StatusCode::GONE, "Document closed or expired; reopen it");
        };
        if path.starts_with("api/")
            && request
                .headers()
                .get("x-reader-token")
                .is_none_or(|value| value.as_bytes() != reader.service.token.as_bytes())
        {
            return json_error(StatusCode::UNAUTHORIZED, "Reader token required");
        }
        reader.touched = Instant::now();
        if path == "api/edit" && !reader.editable {
            return json_error(StatusCode::FORBIDDEN, "This document is read only");
        }
        (
            reader.service.address,
            reader.source.clone(),
            reader.root.clone(),
        )
    };
    if path == "api/edit" {
        let valid = tokio::task::spawn_blocking(move || edit_scope(&source, &root))
            .await
            .unwrap_or(false);
        if !valid {
            return json_error(
                StatusCode::FORBIDDEN,
                "The document path changed; reopen it before editing",
            );
        }
    }
    let query = request
        .uri()
        .query()
        .map(|value| format!("?{value}"))
        .unwrap_or_default();
    // Relay the raw encoded suffix: decoded archive names can contain # or %.
    // Authorization still uses the decoded allowlist above; the service validates
    // the immutable archive capability/member and fixed PDF resource inventory.
    let raw_prefix = format!("/markdown-reader/{id}/");
    let Some(raw_path) = request.uri().path().strip_prefix(&raw_prefix) else {
        return json_error(StatusCode::BAD_REQUEST, "Invalid reader route prefix");
    };
    let mut upstream = state.markdown_readers.client.request(
        request.method().clone(),
        format!("http://{address}/{raw_path}{query}"),
    );
    for name in [
        "x-reader-token",
        "x-reader-document",
        "content-type",
        "range",
    ] {
        if let Some(value) = request.headers().get(name) {
            upstream = upstream.header(name, value);
        }
    }
    if request.method() == Method::POST {
        let bytes = match to_bytes(request.into_body(), 2 * 1024 * 1024).await {
            Ok(bytes) => bytes,
            Err(_) => {
                return json_error(
                    StatusCode::PAYLOAD_TOO_LARGE,
                    "Reader request exceeds 2 MiB",
                )
            }
        };
        upstream = upstream.body(bytes);
    }
    let response = match upstream.send().await {
        Ok(response) => response,
        Err(error) => {
            return json_error(
                StatusCode::BAD_GATEWAY,
                &format!("Reader unavailable: {error}"),
            )
        }
    };
    let mut builder = Response::builder()
        .status(response.status())
        .header(header::CACHE_CONTROL, "no-store")
        .header("x-content-type-options", "nosniff")
        .header(header::REFERRER_POLICY, "no-referrer");
    // Preserve the reader's restrictive policy and range/revision metadata.
    // Do not inject CSS, rewrite source HTML or forward Studio credentials.
    for name in [
        "content-type",
        "content-security-policy",
        "content-range",
        "accept-ranges",
        "content-length",
        "x-reader-revision",
    ] {
        if let Some(value) = response.headers().get(name) {
            builder = builder.header(name, value);
        }
    }
    builder
        .body(Body::from_stream(response.bytes_stream()))
        .expect("reader proxy response")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn one_maintenance_task_is_owned_by_the_registry_not_each_document() {
        let entries = Arc::new(Mutex::new(HashMap::new()));
        let maintenance = ReaderMaintenance::default();
        maintenance.ensure(&entries);
        let task = maintenance.task.lock().unwrap().as_ref().unwrap().id();
        for _ in 0..100 {
            maintenance.ensure(&entries);
        }
        assert_eq!(
            maintenance.task.lock().unwrap().as_ref().unwrap().id(),
            task
        );
        let aborted = maintenance
            .task
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .abort_handle();
        drop(maintenance);
        for _ in 0..100 {
            if aborted.is_finished() {
                break;
            }
            tokio::task::yield_now().await;
        }
        assert!(aborted.is_finished());
    }

    #[tokio::test]
    async fn maintenance_restarts_after_its_runtime_task_is_cancelled() {
        let entries = Arc::new(Mutex::new(HashMap::new()));
        let maintenance = ReaderMaintenance::default();
        maintenance.ensure(&entries);
        let old = maintenance.task.lock().unwrap().as_ref().unwrap().id();
        maintenance.task.lock().unwrap().as_ref().unwrap().abort();
        for _ in 0..100 {
            if maintenance
                .task
                .lock()
                .unwrap()
                .as_ref()
                .unwrap()
                .is_finished()
            {
                break;
            }
            tokio::task::yield_now().await;
        }
        maintenance.ensure(&entries);
        assert_ne!(maintenance.task.lock().unwrap().as_ref().unwrap().id(), old);
    }

    #[test]
    fn embedded_routes_never_expose_browser_or_renderer_privileges() {
        for path in [
            "",
            "api/source",
            "api/edit",
            "api/table",
            "api/diagram-source",
            "api/asset-info",
        ] {
            assert!(allowed_path(path));
        }
        for path in [
            "api/open",
            "api/files",
            "render.html",
            "api/diagram",
            "../api/source",
            "http://example.com/",
        ] {
            assert!(!allowed_path(path));
        }
    }

    #[tokio::test]
    async fn shared_static_router_owns_modules_without_document_authority() {
        for path in [
            "/image-frames.mjs",
            "/image-view.mjs",
            "/image-view.css",
            "/reader.mjs",
            "/host-client.mjs",
        ] {
            let response = marknoria::web::reader_assets()
                .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK, "{path}");
        }
        for path in [
            "/api/source",
            "/api/asset-info",
            "/api/open",
            "/api/files",
            "/not-a-reader-module.mjs",
        ] {
            let response = marknoria::web::reader_assets()
                .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::NOT_FOUND, "{path}");
        }
        assert!(!valid_path("../reader.mjs"));
        assert!(!valid_path("vendor/../reader.mjs"));
    }

    #[test]
    fn editing_revalidates_project_scope_and_rejects_retargeted_symlinks() {
        let root = std::env::temp_dir().join(format!("studio-edit-scope-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        let project = root.join("project");
        let shared = root.join("shared");
        std::fs::create_dir(&project).unwrap();
        std::fs::create_dir(&shared).unwrap();
        let inside = project.join("file.md");
        let outside = shared.join("file.md");
        std::fs::write(&inside, "# Project").unwrap();
        std::fs::write(&outside, "# Shared").unwrap();
        let project = project.canonicalize().unwrap();
        assert!(edit_scope(&inside.canonicalize().unwrap(), &project));
        assert!(!edit_scope(&outside.canonicalize().unwrap(), &project));
        #[cfg(unix)]
        {
            std::fs::remove_file(&inside).unwrap();
            std::os::unix::fs::symlink(&outside, &inside).unwrap();
            assert!(!edit_scope(&inside, &project));
            assert_eq!(std::fs::read_to_string(&outside).unwrap(), "# Shared");
        }
        std::fs::remove_dir_all(root).unwrap();
    }
}
