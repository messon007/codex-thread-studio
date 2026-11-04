#[path = "native_markdown_source.rs"]
mod native_source;
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
use marknoria::server::Service;
use serde::Deserialize;
use serde_json::json;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

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

#[derive(Clone)]
pub struct Readers {
    entries: Arc<Mutex<HashMap<String, Reader>>>,
    opening: Arc<tokio::sync::Semaphore>,
    client: reqwest::Client,
    comparison: Arc<Mutex<Option<Arc<NativeComparison>>>>,
    comparing: Arc<tokio::sync::Semaphore>,
    locating: Arc<tokio::sync::Semaphore>,
    projecting: Arc<tokio::sync::Semaphore>,
    selecting: Arc<tokio::sync::Semaphore>,
}

struct NativeComparison {
    id: String,
    left: String,
    right: String,
    cache: tempfile::TempDir,
    index: marknoria::compare::ComparisonIndex,
}

impl Default for Readers {
    fn default() -> Self {
        Self {
            entries: Arc::new(Mutex::new(HashMap::new())),
            opening: Arc::new(tokio::sync::Semaphore::new(1)),
            comparison: Arc::new(Mutex::new(None)),
            comparing: Arc::new(tokio::sync::Semaphore::new(1)),
            locating: Arc::new(tokio::sync::Semaphore::new(1)),
            projecting: Arc::new(tokio::sync::Semaphore::new(1)),
            selecting: Arc::new(tokio::sync::Semaphore::new(2)),
            client: reqwest::Client::builder()
                .no_proxy()
                .redirect(reqwest::redirect::Policy::none())
                .timeout(Duration::from_secs(30))
                .build()
                .expect("reader HTTP client"),
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
        {
            let mut entries = self.entries.lock().unwrap();
            entries.retain(|_, reader| reader.touched.elapsed() < IDLE);
            if entries.len() >= MAX_READERS {
                return Err((
                    StatusCode::TOO_MANY_REQUESTS,
                    "Close an existing large document first".into(),
                ));
            }
        }
        let service = if editable {
            Service::start_editable(source.clone()).await
        } else {
            Service::start(source.clone()).await
        }
        .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;
        let id = uuid::Uuid::new_v4().to_string();
        let token = service.token.clone();
        let empty = self.entries.lock().unwrap().is_empty();
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
        if empty {
            let entries = Arc::downgrade(&self.entries);
            tokio::spawn(async move {
                loop {
                    tokio::time::sleep(Duration::from_secs(30)).await;
                    let Some(entries) = entries.upgrade() else {
                        break;
                    };
                    let mut entries = entries.lock().unwrap();
                    entries.retain(|_, reader| reader.touched.elapsed() < IDLE);
                    if entries.is_empty() {
                        break;
                    }
                }
            });
        }
        Ok(
            json!({"id":id,"url":format!("/markdown-reader/{id}/?embedded=1&diagrams=browser#{token}")}),
        )
    }
}

pub async fn try_open(
    state: &GatewayState,
    request: &ReviewFileRequest,
) -> Result<Option<serde_json::Value>, (StatusCode, String)> {
    open_with_threshold(state, request, THRESHOLD, true).await
}

async fn open_with_threshold(
    state: &GatewayState,
    request: &ReviewFileRequest,
    threshold: u64,
    allow_editing: bool,
) -> Result<Option<serde_json::Value>, (StatusCode, String)> {
    if review_language(std::path::Path::new(&request.path)) != "markdown" {
        return Ok(None);
    }
    let shared = shared_document_directories(state)?;
    let review = ReviewFileRequest {
        root: request.root.clone(),
        path: request.path.clone(),
    };
    let (root, path, metadata, document_root) = tokio::task::spawn_blocking(move || {
        resolve_review_path_with_shared(&review, marknoria::MAX_SOURCE, "128 MiB Markdown", &shared)
    })
    .await
    .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))??;
    if metadata.len() < threshold {
        return Ok(None);
    }
    let read_only = !allow_editing || !path.starts_with(&root);
    let reader = state
        .markdown_readers
        .open(path.clone(), root.clone(), !read_only)
        .await?;
    Ok(Some(json!({
        "root":root,"path":path,"relativePath":path.strip_prefix(&document_root).unwrap_or(&path),
        "documentRoot":document_root,"size":metadata.len(),"readOnly":read_only,"largeMarkdown":reader,
    })))
}

pub async fn native_open(
    State(state): State<GatewayState>,
    Json(request): Json<ReviewFileRequest>,
) -> Response<Body> {
    native_open_impl(&state, &request, true).await
}

pub async fn native_open_readonly(
    State(state): State<GatewayState>,
    Json(request): Json<ReviewFileRequest>,
) -> Response<Body> {
    native_open_impl(&state, &request, false).await
}

async fn native_open_impl(
    state: &GatewayState,
    request: &ReviewFileRequest,
    allow_editing: bool,
) -> Response<Body> {
    match open_with_threshold(state, request, 0, allow_editing).await {
        Ok(Some(mut value)) => {
            let object = value.as_object_mut().unwrap();
            let reader = object.remove("largeMarkdown").unwrap();
            object.insert("readerId".into(), reader["id"].clone());
            json_response(StatusCode::OK, &value)
        }
        Ok(None) => json_error(
            StatusCode::BAD_REQUEST,
            "The native Markdown reader requires a Markdown file",
        ),
        Err((status, error)) => json_error(status, &error),
    }
}

// These routes use the normal gateway bearer token. Reader capability tokens
// remain in the registry and are injected only into the private service request.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LocateQuoteRequest {
    revision: String,
    query: studio_markdown_native::quote::Query,
}

pub async fn native_locate_quote(
    State(state): State<GatewayState>,
    Path(id): Path<String>,
    Json(request): Json<LocateQuoteRequest>,
) -> Response<Body> {
    if request.revision.len() > 128 || request.query.validate().is_err() {
        return json_error(StatusCode::BAD_REQUEST, "Invalid comment quote request");
    }
    let permit = match state.markdown_readers.locating.clone().try_acquire_owned() {
        Ok(permit) => permit,
        Err(_) => {
            return json_error(
                StatusCode::TOO_MANY_REQUESTS,
                "A comment lookup is still running",
            )
        }
    };
    let snapshot = {
        let mut entries = state.markdown_readers.entries.lock().unwrap();
        let Some(reader) = entries.get_mut(&id) else {
            return json_error(StatusCode::GONE, "Document closed or expired; reopen it");
        };
        reader.touched = Instant::now();
        match reader.service.native_snapshot() {
            Ok(snapshot) if snapshot.revision == request.revision => snapshot,
            Ok(_) => {
                return json_error(StatusCode::CONFLICT, "Document snapshot changed; reopen it")
            }
            Err(error) => return json_error(StatusCode::CONFLICT, &error.to_string()),
        }
    };
    let result = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        locate_quote(&snapshot, request.query)
    })
    .await;
    {
        let entries = state.markdown_readers.entries.lock().unwrap();
        let Some(reader) = entries.get(&id) else {
            return json_error(StatusCode::GONE, "Document closed during comment lookup");
        };
        if !reader
            .service
            .native_snapshot()
            .is_ok_and(|s| s.revision == request.revision)
        {
            return json_error(
                StatusCode::CONFLICT,
                "Document snapshot changed during comment lookup",
            );
        }
    }
    match result {
        Ok(Ok(located)) => json_response(
            StatusCode::OK,
            &json!({"revision":request.revision,"located":located}),
        ),
        Ok(Err(error)) => json_error(StatusCode::BAD_REQUEST, &error),
        Err(_) => json_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Comment lookup worker stopped",
        ),
    }
}

fn locate_quote(
    snapshot: &marknoria::server::ReaderSnapshot,
    query: studio_markdown_native::quote::Query,
) -> Result<studio_markdown_native::quote::Located, String> {
    use studio_markdown_native::{quote::Locator, Fragment};
    let mut locator = Locator::new(query, &snapshot.revision)?;
    let file = std::fs::File::open(&snapshot.source).map_err(|e| e.to_string())?;
    if file.metadata().map_err(|e| e.to_string())?.len() != snapshot.index.source_bytes as u64 {
        return Err("Comment snapshot size changed".into());
    }
    locator
        .scan_literal(file, marknoria::MAX_SOURCE as usize)
        .map_err(|e| e.to_string())?;
    if locator.needs_preview() {
        let index = &snapshot.index;
        let mut headers = HashMap::new();
        let mut table_budget = native_source::TableTextBudget::default();
        for (page, metadata) in index.pages.iter().enumerate() {
            if locator.preview_ambiguous() {
                break;
            }
            if locator.needs_flow() {
                if let Some(id) = metadata.table {
                    if page != index.tables[id].first_page {
                        continue;
                    }
                    match native_source::table_text(
                        snapshot,
                        id,
                        studio_markdown_native::quote::MAX_QUOTE,
                        &mut table_budget,
                    ) {
                        Ok(Some(table)) => locator.flow_table(table),
                        Ok(None) => locator.flow_boundary(),
                        Err(_) => locator.limited(),
                    }
                    continue;
                }
            }
            let fragment =
                native_source::fragment(snapshot, page, &mut headers).and_then(|value| {
                    serde_json::from_value::<Fragment>(value).map_err(|e| e.to_string())
                });
            match fragment.and_then(|f| studio_markdown_native::parse_readable(&f)) {
                Ok(blocks) if locator.needs_flow() => {
                    match studio_markdown_native::flow::project_blocks(blocks) {
                        Ok(projection) => locator.flow(&projection),
                        Err(_) => locator.limited(),
                    }
                }
                Ok(blocks) => locator.preview(&blocks),
                Err(_) => locator.limited(),
            }
        }
    }
    Ok(locator.finish())
}

pub async fn native_data(
    State(state): State<GatewayState>,
    Path((id, operation)): Path<(String, String)>,
    mut request: Request,
) -> Response<Body> {
    if matches!(
        operation.as_str(),
        "fragment" | "table-source" | "image" | "table-text" | "table-prefix"
    ) {
        return native_source::handle(state, id, &operation, request).await;
    }
    let path = match operation.as_str() {
        "document" => "api/native-document",
        "fragment" => "api/fragment-source",
        "source" => "api/source",
        "edit" => "api/edit",
        "table" => "api/table",
        "table-source" => "api/table-source",
        "search" => "api/search",
        "diagram-source" => "api/diagram-source",
        "heartbeat" => "heartbeat",
        _ => return json_error(StatusCode::NOT_FOUND, "Unknown native reader operation"),
    };
    if request.method() != Method::GET && !(path == "api/edit" && request.method() == Method::POST)
    {
        return json_error(
            StatusCode::METHOD_NOT_ALLOWED,
            "Native reader method rejected",
        );
    }
    let token = {
        let mut entries = state.markdown_readers.entries.lock().unwrap();
        let Some(reader) = entries.get_mut(&id) else {
            return json_error(StatusCode::GONE, "Document closed or expired; reopen it");
        };
        reader.touched = Instant::now();
        reader.service.token.clone()
    };
    if path == "heartbeat" {
        return json_response(StatusCode::OK, &json!({"alive":true}));
    }
    let token = match token.parse() {
        Ok(token) => token,
        Err(_) => {
            return json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Reader credential is invalid",
            )
        }
    };
    request.headers_mut().insert("x-reader-token", token);
    let response = proxy(state, id, path.into(), request).await;
    if response.status().is_success() {
        return response;
    }
    let (parts, body) = response.into_parts();
    let bytes = match to_bytes(body, 8 * 1024).await {
        Ok(bytes) => bytes,
        Err(_) => return json_error(parts.status, "Native reader request failed"),
    };
    if serde_json::from_slice::<serde_json::Value>(&bytes).is_ok() {
        return Response::from_parts(parts, Body::from(bytes));
    }
    json_error(
        parts.status,
        std::str::from_utf8(&bytes).unwrap_or("Native reader request failed"),
    )
}

#[derive(Deserialize)]
pub struct NativeCompareRequest {
    left: String,
    right: String,
}

pub async fn native_compare(
    State(state): State<GatewayState>,
    Json(request): Json<NativeCompareRequest>,
) -> Response<Body> {
    let permit = match state.markdown_readers.comparing.clone().try_acquire_owned() {
        Ok(permit) => permit,
        Err(_) => {
            return json_error(
                StatusCode::TOO_MANY_REQUESTS,
                "A comparison is still running",
            )
        }
    };
    let snapshots = {
        let mut entries = state.markdown_readers.entries.lock().unwrap();
        let snapshot = |entries: &mut HashMap<String, Reader>,
                        id: &str|
         -> Result<marknoria::server::ReaderSnapshot, String> {
            let reader = entries.get_mut(id).ok_or("Compared document was closed")?;
            reader.touched = Instant::now();
            reader
                .service
                .native_snapshot()
                .map_err(|error| error.to_string())
        };
        match (
            snapshot(&mut entries, &request.left),
            snapshot(&mut entries, &request.right),
        ) {
            (Ok(left), Ok(right)) => (left, right),
            (Err(error), _) | (_, Err(error)) => return json_error(StatusCode::CONFLICT, &error),
        }
    };
    let left = request.left;
    let right = request.right;
    let result = tokio::task::spawn_blocking(move || -> Result<NativeComparison, String> {
        let cache = marknoria::server::native_temporary_directory("studio-native-compare-")
            .map_err(|error| error.to_string())?;
        let index = marknoria::compare::build_comparison(
            &snapshots.0.source,
            &snapshots.1.source,
            cache.path(),
        )
        .map_err(|error| error.to_string())?;
        if index.left_hash != snapshots.0.revision || index.right_hash != snapshots.1.revision {
            return Err("Comparison snapshot identity changed".into());
        }
        Ok(NativeComparison {
            id: uuid::Uuid::new_v4().to_string(),
            left,
            right,
            cache,
            index,
        })
    })
    .await;
    let comparison = match result {
        Ok(Ok(value)) => Arc::new(value),
        Ok(Err(error)) => return json_error(StatusCode::BAD_REQUEST, &error),
        Err(error) => return json_error(StatusCode::INTERNAL_SERVER_ERROR, &error.to_string()),
    };
    {
        let entries = state.markdown_readers.entries.lock().unwrap();
        if !entries.contains_key(&comparison.left) || !entries.contains_key(&comparison.right) {
            return json_error(StatusCode::GONE, "Compared documents were closed");
        }
    }
    let metadata = json!({"id":comparison.id,"changes":comparison.index.changes.len(),"added":comparison.index.added,"removed":comparison.index.removed,"leftHash":comparison.index.left_hash,"rightHash":comparison.index.right_hash,"boundedAlignment":comparison.index.bounded_alignment,"elapsedMs":comparison.index.elapsed_ms});
    let previous = state
        .markdown_readers
        .comparison
        .lock()
        .unwrap()
        .replace(comparison);
    tokio::task::spawn_blocking(move || drop(previous));
    drop(permit);
    json_response(StatusCode::OK, &metadata)
}

pub async fn native_compare_change(
    State(state): State<GatewayState>,
    Path((id, change)): Path<(String, usize)>,
) -> Response<Body> {
    let comparison = state
        .markdown_readers
        .comparison
        .lock()
        .unwrap()
        .as_ref()
        .filter(|entry| entry.id == id)
        .cloned();
    let Some(comparison) = comparison else {
        return json_error(
            StatusCode::GONE,
            "Comparison closed; compare the files again",
        );
    };
    {
        let entries = state.markdown_readers.entries.lock().unwrap();
        if !entries.contains_key(&comparison.left) || !entries.contains_key(&comparison.right) {
            return json_error(StatusCode::GONE, "Compared documents were closed");
        }
    }
    let result=tokio::task::spawn_blocking(move ||->Result<serde_json::Value,String>{
        let row=*comparison.index.changes.get(change).ok_or("Change is out of range")?;
        let lines=marknoria::compare::hunk_lines(comparison.cache.path(),&comparison.index,row).map_err(|error|error.to_string())?;
        let mut left=Vec::new();let mut right=Vec::new();let mut start=row;
        // Only line coordinates cross IPC. Source text remains in the separate
        // bounded native readers, including when a source line is very long.
        for _ in 0..3{
            let rows=marknoria::compare::read_rows(comparison.cache.path(),&comparison.index,start).map_err(|error|error.to_string())?;
            if rows.is_empty(){break;}
            start=rows.last().unwrap().row+1;
            for row in rows{if row.kind!="equal"{if let Some(cell)=row.left{left.push(cell.line);}if let Some(cell)=row.right{right.push(cell.line);}}}
            if start>=comparison.index.rows{break;}
        }
        Ok(json!({"index":change,"leftLine":lines[0].max(1),"rightLine":lines[1].max(1),"leftChanged":left,"rightChanged":right}))
    }).await;
    match result {
        Ok(Ok(value)) => json_response(StatusCode::OK, &value),
        Ok(Err(error)) => json_error(StatusCode::BAD_REQUEST, &error),
        Err(error) => json_error(StatusCode::INTERNAL_SERVER_ERROR, &error.to_string()),
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
    state
        .markdown_readers
        .entries
        .lock()
        .unwrap()
        .remove(&request.id);
    let previous = {
        let mut comparison = state.markdown_readers.comparison.lock().unwrap();
        if comparison
            .as_ref()
            .is_some_and(|entry| entry.left == request.id || entry.right == request.id)
        {
            comparison.take()
        } else {
            None
        }
    };
    if previous.is_some() {
        tokio::task::spawn_blocking(move || drop(previous));
    }
    json_response(StatusCode::OK, &json!({"closed":true}))
}

fn allowed_path(path: &str) -> bool {
    matches!(
        path,
        "" | "reader.mjs"
            | "styles.css"
            | "source.mjs"
            | "position.mjs"
            | "editor.mjs"
            | "theme.js"
            | "checkbox.css"
            | "syntax-light.css"
            | "syntax-dark.css"
            | "tables.mjs"
            | "icons.mjs"
            | "files.mjs"
            | "diagrams.mjs"
            | "engine.mjs"
            | "vendor/purify.mjs"
            | "vendor/virtual-core.mjs"
            | "vendor/editor.mjs"
            | "vendor/mermaid.min.js"
            | "vendor/plantuml.js"
            | "vendor/plantuml-viz-global.js"
            | "vendor/d2.js"
            | "api/document"
            | "api/native-document"
            | "api/fragment-source"
            | "api/table-source"
            | "api/window"
            | "api/table"
            | "api/source"
            | "api/edit"
            | "api/search"
            | "api/diagram-source"
            | "api/renderer"
    )
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
    if !allowed_path(&path) {
        return json_error(StatusCode::NOT_FOUND, "Unknown reader route");
    }
    if request.method() != Method::GET && !(path == "api/edit" && request.method() == Method::POST)
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
    let mut upstream = state.markdown_readers.client.request(
        request.method().clone(),
        format!("http://{address}/{path}{query}"),
    );
    for name in ["x-reader-token", "x-reader-document", "content-type"] {
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
                    "Editing request exceeds 2 MiB",
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
    let mut builder = Response::builder().status(response.status())
        .header(header::CACHE_CONTROL, "no-store")
        .header("x-content-type-options", "nosniff")
        .header(header::REFERRER_POLICY, "no-referrer")
        .header(header::CONTENT_SECURITY_POLICY, "default-src 'none'; script-src 'self' 'wasm-unsafe-eval'; worker-src 'self' blob:; style-src 'self' 'unsafe-inline'; connect-src 'self' blob:; img-src blob: data:; font-src data:; object-src 'none'; base-uri 'none'; frame-ancestors 'self'; form-action 'none'");
    if let Some(content_type) = response.headers().get(header::CONTENT_TYPE) {
        builder = builder.header(header::CONTENT_TYPE, content_type);
    }
    if path.is_empty() && response.status().is_success() {
        let html = match response.text().await {
            Ok(html) => html,
            Err(error) => return json_error(StatusCode::BAD_GATEWAY, &error.to_string()),
        };
        let html = html.replace(
            "</head>",
            &format!(
                "<style>{}\n{}</style></head>",
                include_str!("../../ui/large-markdown-host.css"),
                include_str!("../../ui/vendor/marknoria-checkbox.css")
            ),
        );
        return builder
            .body(Body::from(html))
            .expect("reader page response");
    }
    builder
        .body(Body::from_stream(response.bytes_stream()))
        .expect("reader proxy response")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn embedded_routes_never_expose_browser_or_renderer_privileges() {
        for path in [
            "",
            "reader.mjs",
            "api/source",
            "api/edit",
            "editor.mjs",
            "vendor/editor.mjs",
            "api/table",
            "api/diagram-source",
            "engine.mjs",
            "vendor/mermaid.min.js",
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
