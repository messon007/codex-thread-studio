//! HTTP document adapter and browser/WebView asset routes.
use crate::runtime::renderer;
pub mod selected_files;
use axum::{
    Json, Router,
    extract::{Query, Request, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
};
mod files;
mod compare_runtime;
mod panes;
mod edit_runtime;
mod table_query_runtime;
mod table_chart_runtime;
mod document_runtime;
mod pdf_runtime;
mod epub_runtime;
mod service;
mod references;
pub use service::{Service, ServiceOptions, DiagramMode};
use crate::core::{
    Index, audit_tables, build_index, read_diagram, read_page, read_table, search,
};
use crate::core::source::read_source;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    env,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tokio::sync::Semaphore;

#[derive(Clone)]
struct App {
    references: Arc<references::Resources>,
    editable: bool,
    queries: bool,
    notes_directory: Option<std::path::PathBuf>,
    embedded: bool,
    parent_origins: Arc<Vec<String>>,
    browser_diagrams: bool,
    editor: Arc<edit_runtime::Editor>,
    comparisons: Arc<compare_runtime::Comparisons>,
    documents: Arc<files::Documents>,
    comparison_documents: Arc<files::Documents>,
    token: String,
    host: String,
    work: Arc<Semaphore>,
    open_work: Arc<Semaphore>,
    diagram_work: Arc<Semaphore>,
    diagram_waiters: Arc<Semaphore>,
    cancelled: Arc<AtomicBool>,
}

struct Progress {
    index: Option<Arc<Index>>,
    error: Option<String>,
}

type ApiError = (StatusCode, String);

fn failure(error: impl std::fmt::Display) -> ApiError {
    (StatusCode::INTERNAL_SERVER_ERROR, error.to_string())
}

async fn guard(State(app): State<App>, request: Request, next: Next) -> Response {
    if app.cancelled.load(Ordering::Relaxed) { return StatusCode::GONE.into_response(); }
    let browser_diagrams = app.browser_diagrams || request.uri().query()
        .is_some_and(|query| query.split('&').any(|part| part == "diagrams=browser"));
    let rendering_page = request.uri().path() == "/render.html";
    let comparison_page = request.uri().path() == "/compare.html";
    let comparison_preview = request.uri().path().starts_with("/comparison/") || request.uri().path().starts_with("/pane/");
    let epub_resource = request.uri().path().starts_with("/epub-resource/") || request.uri().path().contains("/epub-resource/");
    let host = request
        .headers()
        .get(header::HOST)
        .and_then(|value| value.to_str().ok());
    let origin = request
        .headers()
        .get(header::ORIGIN)
        .and_then(|value| value.to_str().ok());
    let expected_origin = format!("http://{}", app.host);
    if host != Some(app.host.as_str()) || origin.is_some_and(|value| value != expected_origin) {
        return StatusCode::FORBIDDEN.into_response();
    }
    if request.uri().path().starts_with("/api/")
        && request
            .headers()
            .get("x-reader-token")
            .and_then(|value| value.to_str().ok())
            != Some(app.token.as_str())
    {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let mut response = next.run(request).await;
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    let policy = if epub_resource {
        "default-src 'none'; script-src 'none'; style-src 'self' 'unsafe-inline'; connect-src 'none'; img-src 'self' data: blob:; font-src 'self' data: blob:; object-src 'none'; base-uri 'none'; frame-ancestors 'self'; form-action 'none'"
    } else if rendering_page {
        "default-src 'none'; script-src 'self' 'wasm-unsafe-eval'; worker-src 'self' blob:; style-src 'self' 'unsafe-inline'; connect-src 'none'; img-src data: blob:; font-src data:; object-src 'none'; base-uri 'none'; frame-ancestors 'none'; form-action 'none'"
    } else if comparison_page {
        "default-src 'none'; script-src 'self'; worker-src 'none'; style-src 'self' 'unsafe-inline'; connect-src 'self' blob:; img-src blob:; frame-src 'self'; object-src 'none'; base-uri 'none'; frame-ancestors 'none'; form-action 'none'"
    } else if comparison_preview {
        "default-src 'none'; script-src 'self' 'wasm-unsafe-eval'; worker-src 'self' blob:; style-src 'self' 'unsafe-inline'; connect-src 'self' blob:; img-src blob:; font-src 'self' data: blob:; object-src 'none'; base-uri 'none'; frame-ancestors 'self'; form-action 'none'"
    } else if browser_diagrams {
        "default-src 'none'; script-src 'self' 'wasm-unsafe-eval'; worker-src 'self' blob:; style-src 'self' 'unsafe-inline'; connect-src 'self' blob:; img-src blob:; font-src 'self' data: blob:; object-src 'none'; base-uri 'none'; frame-ancestors 'none'; form-action 'none'"
    } else {
        "default-src 'none'; script-src 'self' 'wasm-unsafe-eval'; worker-src 'self' blob:; style-src 'self' 'unsafe-inline'; connect-src 'self' blob:; img-src blob:; font-src 'self' data: blob:; object-src 'none'; base-uri 'none'; frame-ancestors 'none'; form-action 'none'"
    };
    let policy = if !rendering_page && !comparison_page && !epub_resource {
        policy.replace("img-src blob:;", "img-src 'self' data: blob:;")
            .replace("object-src 'none';", "frame-src 'self' blob:; object-src 'none';")
    } else { policy.to_owned() };
    let policy = if app.embedded && !rendering_page && !comparison_page && !comparison_preview {
        let ancestors = std::iter::once("'self'".to_owned())
            .chain(app.parent_origins.iter().cloned()).collect::<Vec<_>>().join(" ");
        policy.replace("frame-ancestors 'none'", &format!("frame-ancestors {ancestors}"))
    } else { policy.to_owned() };
    response.headers_mut().insert(header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_str(&policy).expect("validated reader policy"));
    response.headers_mut().insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    response
}

#[derive(Deserialize)]
struct DiagramQuery {
    id: usize,
}

async fn diagram(
    State(app): State<App>,
    Query(query): Query<DiagramQuery>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let document = app.documents.select(&headers)?;
    let index = index(&document)?;
    if query.id >= index.diagrams.len() {
        return Err((StatusCode::BAD_REQUEST, "Diagram out of range".into()));
    }
    let slot = app
        .diagram_waiters
        .clone()
        .try_acquire_owned()
        .map_err(|_| {
            (
                StatusCode::TOO_MANY_REQUESTS,
                "Diagram renderer is busy".into(),
            )
        })?;
    let permit = tokio::time::timeout(
        Duration::from_secs(25),
        app.diagram_work.clone().acquire_owned(),
    )
    .await
    .map_err(|_| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            "Diagram queue wait exceeded 25 seconds".into(),
        )
    })?
    .map_err(failure)?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let _slot = slot;
        let metadata = &index.diagrams[query.id];
        let source = read_diagram(&document.directory, &index, query.id).map_err(failure)?;
        let result = document
            .renderer
            .lock()
            .map_err(failure)?
            .render(
                query.id,
                &metadata.kind,
                &source,
                &format!("http://{}", app.host),
                document.cancelled.clone(),
            )
            .map_err(failure)?;
        Ok(Json(
            json!({"id":query.id,"revision":index.revision,"result":result}),
        ))
    })
    .await
    .map_err(failure)?
}

async fn renderer_status(State(app): State<App>) -> Json<Value> {
    let Some(document) = app.documents.peek() else {
        return Json(json!({"mode":"idle","cacheBytes":0}));
    };
    Json(match document.renderer.try_lock() {
        Ok(renderer) => renderer.snapshot(),
        Err(_) => json!({"busy":true}),
    })
}

async fn diagram_source(State(app): State<App>, Query(query): Query<DiagramQuery>, headers: HeaderMap) -> Result<Json<Value>, ApiError> {
    let document = app.documents.select(&headers)?;
    let permit = app.work.clone().try_acquire_owned().map_err(|_| (StatusCode::TOO_MANY_REQUESTS, "Reader is busy".into()))?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let index = index(&document)?;
        let metadata = index.diagrams.get(query.id).ok_or((StatusCode::BAD_REQUEST, "Diagram out of range".into()))?;
        let source = read_diagram(&document.directory, &index, query.id).map_err(failure)?;
        Ok(Json(json!({"id":query.id,"revision":index.revision,"kind":metadata.kind,"source":source})))
    }).await.map_err(failure)?
}

async fn document(State(app): State<App>, headers: HeaderMap) -> Result<Json<Value>, ApiError> {
    if app.documents.peek().is_none() {
        return Ok(Json(json!({"state":"idle"})));
    }
    let document = app.documents.select(&headers)?;
    let permit = app
        .work
        .clone()
        .try_acquire_owned()
        .map_err(|_| (StatusCode::TOO_MANY_REQUESTS, "Reader is busy".into()))?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let progress = document.progress.lock().map_err(failure)?;
        let source_available=crate::core::document_format::DocumentFormat::for_path(&document.source).is_some_and(|f|f.is_text());
        let editable=app.editable&&source_available;
        if let Some(error) = &progress.error { return Ok(Json(json!({"state":"error", "error":error,"documentId":document.id,"editable":editable,"sourceAvailable":source_available}))); }
        let Some(index) = &progress.index else { return Ok(Json(json!({"state":"indexing","documentId":document.id,"editable":editable,"sourceAvailable":source_available}))); };
        let pages = (0..index.pages.len().min(3)).map(|page| Ok(json!({"id":page,"html":read_page(&document.directory, index, page).map_err(failure)?}))).collect::<Result<Vec<_>, ApiError>>()?;
        Ok(Json(json!({"state":"ready", "documentId":document.id,"index":index.as_ref(), "initial":pages,"resourceId":crate::core::reader::file_identity(&document.source),"editable":app.editable && index.format.is_text(),"sourceAvailable":index.format.is_text()})))
    }).await.map_err(failure)?
}

fn index(document: &files::Document) -> Result<Arc<Index>, ApiError> {
    document
        .progress
        .lock()
        .map_err(failure)?
        .index
        .clone()
        .ok_or((StatusCode::CONFLICT, "Document is not ready".into()))
}

#[derive(Deserialize)]
struct PageQuery {
    target: usize,
}

async fn window(
    State(app): State<App>,
    Query(query): Query<PageQuery>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let document = app.documents.select(&headers)?;
    let permit = app
        .work
        .clone()
        .try_acquire_owned()
        .map_err(|_| (StatusCode::TOO_MANY_REQUESTS, "Reader is busy".into()))?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let index = index(&document)?;
        if query.target >= index.pages.len() { return Err((StatusCode::BAD_REQUEST, "Page out of range".into())); }
        let start = query.target.saturating_sub(2);
        let end = (query.target + 3).min(index.pages.len());
        let pages = (start..end).map(|page| Ok(json!({"id":page,"html":read_page(&document.directory, &index, page).map_err(failure)?}))).collect::<Result<Vec<_>, ApiError>>()?;
        Ok(Json(json!({"revision":index.revision, "pages":pages})))
    }).await.map_err(failure)?
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProjectionQuery {
    revision: String,
    start: usize,
    count: usize,
}
async fn projection_window(
    State(app): State<App>,
    Query(query): Query<ProjectionQuery>,
    headers: HeaderMap,
) -> Result<Json<crate::core::reader::Window>, ApiError> {
    let document = app.documents.select(&headers)?;
    let permit = app.work.clone().try_acquire_owned()
        .map_err(|_| (StatusCode::TOO_MANY_REQUESTS, "Reader is busy".into()))?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let index = index(&document)?;
        if query.revision != index.revision {
            return Err((StatusCode::CONFLICT, "Projection revision changed".into()));
        }
        if index.renderer.is_some() {
            return Err((StatusCode::BAD_REQUEST, "Original-container documents require their renderer".into()));
        }
        if query.count == 0 || query.count > crate::core::reader::Budgets::default().max_window_pages
            || (query.start >= index.pages.len() && !(query.start == 0 && index.pages.is_empty())) {
            return Err((StatusCode::BAD_REQUEST, "Projection window exceeds its page budget".into()));
        }
        let pages = (query.start..query.start.saturating_add(query.count).min(index.pages.len()))
            .map(|page| {
                let metadata = &index.pages[page];
                let source = index.format.is_text();
                Ok(crate::core::reader::ProjectionPage {
                    page, source_line: metadata.line,
                    source_start_byte: source.then_some(metadata.start),
                    source_end_byte: source.then_some(metadata.end),
                    table: metadata.table, row_start: metadata.row_start,
                    html: read_page(&document.directory, &index, page).map_err(failure)?,
                })
            }).collect::<Result<Vec<_>, ApiError>>()?;
        Ok(Json(crate::core::reader::Window { revision: index.revision.clone(), total_pages: index.pages.len(), pages }))
    }).await.map_err(failure)?
}

#[derive(Deserialize)]
struct SearchQuery {
    query: String,
}

#[derive(Deserialize)]
struct TableQuery {
    id: usize,
    #[serde(default)]
    row: usize,
    columns: Option<String>,
}

async fn table_window(
    State(app): State<App>,
    Query(query): Query<TableQuery>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let document = app.documents.select(&headers)?;
    let index = index(&document)?;
    let metadata = index
        .tables
        .get(query.id)
        .ok_or((StatusCode::BAD_REQUEST, "Table out of range".into()))?;
    let columns = match query.columns {
        Some(value) if value.len() <= 2048 => value
            .split(',')
            .map(str::parse::<usize>)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid columns".into()))?,
        Some(_) => {
            return Err((
                StatusCode::BAD_REQUEST,
                "Column selection exceeds budget".into(),
            ));
        }
        None => (0..metadata.columns.len()).collect(),
    };
    if columns.is_empty()
        || columns.len() > metadata.columns.len()
        || columns.windows(2).any(|pair| pair[0] >= pair[1])
        || columns
            .last()
            .is_some_and(|column| *column >= metadata.columns.len())
        || query.row >= metadata.rows.max(1)
    {
        return Err((
            StatusCode::BAD_REQUEST,
            "Invalid table row or columns".into(),
        ));
    }
    let permit = app
        .work
        .clone()
        .try_acquire_owned()
        .map_err(|_| (StatusCode::TOO_MANY_REQUESTS, "Reader is busy".into()))?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let result = read_table(&document.directory, &index, query.id, query.row, &columns)
            .map_err(failure)?;
        Ok(Json(
            json!({"revision":index.revision,"table":query.id,"result":result}),
        ))
    })
    .await
    .map_err(failure)?
}

async fn find(
    State(app): State<App>,
    Query(query): Query<SearchQuery>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let document = app.documents.select(&headers)?;
    if query.query.is_empty() || query.query.len() > 256 {
        return Err((
            StatusCode::BAD_REQUEST,
            "Search requires 1–256 UTF-8 bytes".into(),
        ));
    }
    let permit = app
        .work
        .clone()
        .try_acquire_owned()
        .map_err(|_| (StatusCode::TOO_MANY_REQUESTS, "Reader is busy".into()))?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let index = index(&document)?;
        if index.renderer.is_some() { return Err((StatusCode::CONFLICT, "Search requires the active container renderer; use the public reader search command".into())); }
        let matches = search(&document.directory, &index, &query.query).map_err(failure)?;
        Ok(Json(
            json!({"matches":matches,"limit":100,"mode":"case-sensitive-source-lines"}),
        ))
    })
    .await
    .map_err(failure)?
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceQuery {
    line: Option<usize>,
    offset: Option<usize>,
    end: Option<usize>,
    #[serde(default)]
    previous: bool,
}

async fn source_window(State(app): State<App>, Query(query): Query<SourceQuery>, headers: HeaderMap) -> Result<Json<Value>, ApiError> {
    let document = app.documents.select(&headers)?;
    let permit = app.work.clone().try_acquire_owned().map_err(|_| (StatusCode::TOO_MANY_REQUESTS, "Reader is busy".into()))?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let index = index(&document)?;
        if !index.format.is_text(){return Err((StatusCode::BAD_REQUEST,"Source view applies to text documents; use the extracted page text instead".into()));}
        let window = if let Some(end) = query.end {
            let start = query.offset.filter(|start| query.line.is_none() && !query.previous && *start <= end && end - *start <= crate::core::source::SOURCE_BYTES)
                .ok_or_else(|| (StatusCode::BAD_REQUEST,"Invalid bounded source range".into()))?;
            crate::core::source::read_range(&document.directory, &index, start, end)
        } else { read_source(&document.directory, &index, query.line, query.offset, query.previous) };
        let result = window.map_err(|error| {
            if error.kind() == std::io::ErrorKind::InvalidInput { (StatusCode::BAD_REQUEST, error.to_string()) }
            else { failure(error) }
        })?;
        Ok(Json(json!({"revision":index.revision,"language":crate::core::highlight::language_for_name(&index.name),"result":result})))
    }).await.map_err(failure)?
}

fn assets() -> Router {
    Router::new()
        .route("/document-prose.css", get(|| async { ([(header::CONTENT_TYPE, "text/css")], include_str!("../../web/document-prose.css")) }))
        .route("/projection-html.mjs", get(|| async { ([(header::CONTENT_TYPE, "text/javascript")], include_str!("../../web/projection-html.mjs")) }))
        .route("/projection-view.mjs", get(|| async { ([(header::CONTENT_TYPE, "text/javascript")], include_str!("../../web/projection-view.mjs")) }))
        .route("/projection-view.css", get(|| async { ([(header::CONTENT_TYPE, "text/css")], include_str!("../../web/projection-view.css")) }))
        .route("/theme.js", get(|| async { ([(header::CONTENT_TYPE, "text/javascript")], include_str!("../../web/theme.js")) }))
        .route("/editor.mjs", get(|| async { ([(header::CONTENT_TYPE, "text/javascript")], include_str!("../../web/editor.mjs")) }))
        .route("/vendor/editor.mjs", get(|| async { ([(header::CONTENT_TYPE, "text/javascript")], include_str!("../../web/vendor/editor.mjs")) }))
        .route("/syntax-light.css", get(|| async { ([(header::CONTENT_TYPE, "text/css")], crate::core::highlight::css(false)) }))
        .route("/syntax-dark.css", get(|| async { ([(header::CONTENT_TYPE, "text/css")], crate::core::highlight::css(true)) }))
        .route("/reader-tools.mjs", get(|| async { ([(header::CONTENT_TYPE, "text/javascript")], include_str!("../../web/reader-tools.mjs")) }))
        .route("/table-controls.mjs", get(|| async { ([(header::CONTENT_TYPE, "text/javascript")], include_str!("../../web/table-controls.mjs")) }))
        .route("/table-layout.mjs", get(|| async { ([(header::CONTENT_TYPE, "text/javascript")], include_str!("../../web/table-layout.mjs")) }))
        .route("/public-reader.mjs", get(|| async { ([(header::CONTENT_TYPE, "text/javascript")], include_str!("../../web/public-reader.mjs")) }))
        .route("/reader-bridge.mjs", get(|| async { ([(header::CONTENT_TYPE, "text/javascript")], include_str!("../../web/reader-bridge.mjs")) }))
        .route("/host-client.mjs", get(|| async { ([(header::CONTENT_TYPE, "text/javascript")], include_str!("../../web/host-client.mjs")) }))
        .route("/query-operation.mjs", get(|| async { ([(header::CONTENT_TYPE, "text/javascript")], include_str!("../../web/query-operation.mjs")) }))
        .route("/table-query.mjs", get(|| async { ([(header::CONTENT_TYPE, "text/javascript")], include_str!("../../web/table-query.mjs")) }))
        .route("/assets.mjs", get(|| async { ([(header::CONTENT_TYPE, "text/javascript")], include_str!("../../web/assets.mjs")) }))
        .route("/image-view.mjs", get(|| async { ([(header::CONTENT_TYPE, "text/javascript")], include_str!("../../web/image-view.mjs")) }))
        .route("/image-view.css", get(|| async { ([(header::CONTENT_TYPE, "text/css")], include_str!("../../web/image-view.css")) }))
        .route("/image-frames.mjs", get(|| async { ([(header::CONTENT_TYPE, "text/javascript")], include_str!("../../web/image-frames.mjs")) }))
        .route("/references.mjs", get(|| async { ([(header::CONTENT_TYPE, "text/javascript")], include_str!("../../web/references.mjs")) }))
        .route("/presentation.mjs", get(|| async { ([(header::CONTENT_TYPE, "text/javascript")], include_str!("../../web/presentation.mjs")) }))
        .route("/reader-ui.mjs", get(|| async { ([(header::CONTENT_TYPE, "text/javascript")], include_str!("../../web/reader-ui.mjs")) }))
        .route("/translation.mjs", get(|| async { ([(header::CONTENT_TYPE, "text/javascript")], include_str!("../../web/translation.mjs")) }))
        .route("/notes.mjs", get(|| async { ([(header::CONTENT_TYPE, "text/javascript")], include_str!("../../web/notes.mjs")) }))
        .route("/jq-query.css", get(|| async { ([(header::CONTENT_TYPE, "text/css")], include_str!("../../web/jq-query.css")) }))
        .route("/jq-query.mjs", get(|| async { ([(header::CONTENT_TYPE, "text/javascript")], include_str!("../../web/jq-query.mjs")) }))
        .route("/position.mjs", get(|| async { ([(header::CONTENT_TYPE, "text/javascript")], include_str!("../../web/position.mjs")) }))
        .route("/compare-source.mjs", get(|| async { ([(header::CONTENT_TYPE, "text/javascript")], include_str!("../../web/compare-source.mjs")) }))
        .route("/checkbox.css", get(|| async { ([(header::CONTENT_TYPE, "text/css")], include_str!("../../web/checkbox.css")) }))
        .route("/compare.html", get(|| async { ([(header::CONTENT_TYPE, "text/html; charset=utf-8")], include_str!("../../web/compare.html")) }))
        .route("/compare.mjs", get(|| async { ([(header::CONTENT_TYPE, "text/javascript")], include_str!("../../web/compare.mjs")) }))
        .route("/compare.css", get(|| async { ([(header::CONTENT_TYPE, "text/css")], include_str!("../../web/compare.css")) }))
        .route("/icons.mjs", get(|| async { ([(header::CONTENT_TYPE, "text/javascript")], include_str!("../../web/icons.mjs")) }))
        .route("/source.mjs", get(|| async { ([(header::CONTENT_TYPE, "text/javascript")], include_str!("../../web/source.mjs")) }))
        .route(
            "/files.mjs",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/javascript")],
                    include_str!("../../web/files.mjs"),
                )
            }),
        )
        .route(
            "/tables.mjs",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/javascript")],
                    include_str!("../../web/tables.mjs"),
                )
            }),
        )
        .route(
            "/render.html",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
                    crate::runtime::assets::RENDER_PAGE,
                )
            }),
        )
        .route(
            "/engine.mjs",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/javascript")],
                    crate::runtime::assets::ENGINE,
                )
            }),
        )
        .route(
            "/diagrams.mjs",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/javascript")],
                    include_str!("../../web/diagrams.mjs"),
                )
            }),
        )
        .route(
            "/vendor/mermaid.min.js",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/javascript")],
                    crate::runtime::assets::MERMAID,
                )
            }),
        )
        .route(
            "/vendor/plantuml.js",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/javascript")],
                    crate::runtime::assets::PLANTUML,
                )
            }),
        )
        .route(
            "/vendor/plantuml-viz-global.js",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/javascript")],
                    crate::runtime::assets::PLANTUML_VIZ,
                )
            }),
        )
        .route(
            "/vendor/d2.js",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/javascript")],
                    crate::runtime::assets::D2,
                )
            }),
        )
        .route(
            "/",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
                    include_str!("../../web/index.html"),
                )
            }),
        )
        .route(
            "/reader.mjs",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/javascript")],
                    include_str!("../../web/reader.mjs"),
                )
            }),
        )
        .route(
            "/styles.css",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/css")],
                    include_str!("../../web/styles.css"),
                )
            }),
        )
        .route(
            "/vendor/virtual-core.mjs",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/javascript")],
                    include_str!("../../web/vendor/virtual-core.mjs"),
                )
            }),
        )
        .route(
            "/vendor/purify.mjs",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/javascript")],
                    crate::runtime::assets::PURIFY,
                )
            }),
        )
}

pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
    if let Some(result) = compare_runtime::worker() { return result.map_err(Into::into); }
    if let Some(result) = index_worker() {
        return result.map_err(Into::into);
    }
    let arguments: Vec<_> = env::args().collect();
    if arguments.get(1).is_some_and(|argument| argument == "audit") {
        let directory = crate::runtime::cache::temporary_directory("markdown-reader-audit-")?;
        let result = build_index(
            Path::new(arguments.get(2).ok_or("Missing input file")?),
            directory.path(),
        )?;
        let audit = audit_tables(directory.path(), &result)?;
        println!(
            "{}",
            json!({"sourceBytes":result.source_bytes,"revision":result.revision,"pages":result.pages.len(),"headings":result.toc.len(),"indexMs":result.index_ms,"peakRssKiB":result.peak_rss_kib,"tableAudit":audit})
        );
        return Ok(());
    }
    if arguments.get(1).is_some_and(|argument| argument == "index") {
        let result = build_index(
            Path::new(arguments.get(2).ok_or("Missing input file")?),
            Path::new(arguments.get(3).ok_or("Missing private output directory")?),
        )?;
        println!(
            "{}",
            json!({"sourceBytes":result.source_bytes,"pages":result.pages.len(),"headings":result.toc.len(),"indexMs":result.index_ms,"peakRssKiB":result.peak_rss_kib})
        );
        return Ok(());
    }
    if arguments
        .iter()
        .skip(1)
        .any(|argument| matches!(argument.as_str(), "--help" | "-h"))
    {
        println!(
            "Usage: marknoria-server [FILE [PORT]] [--port PORT] [--root DIRECTORY]\n\nStart without FILE and choose a server document in the browser.\nBrowsing is confined to --root (default: current directory, or initial file's parent).\nOriginal files are never modified. Browser-device uploads are not supported."
        );
        return Ok(());
    }
    let options = files::options(&arguments[1..])?;
    let selected_directory = env::var_os(crate::web::selected_files::DIRECTORY_ENV)
        .map(std::path::PathBuf::from)
        .map(|path| path.canonicalize()).transpose()?;
    let listener =
        tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, options.port)).await?;
    let app = App {
        references: Arc::new(references::Resources::new(vec![options.root.clone()])),
        editable: true,
        queries: true,
        notes_directory: crate::runtime::config::directory(),
        embedded: false,
        parent_origins: Arc::new(Vec::new()),
        browser_diagrams: false,
        editor: Arc::new(edit_runtime::Editor::default()),
        comparisons: Arc::new(compare_runtime::Comparisons::new()),
        comparison_documents: Arc::new(files::Documents::new(options.root.clone()).with_selected_directory(selected_directory.clone())),
        documents: Arc::new(files::Documents::new(options.root).with_selected_directory(selected_directory)),
        token: uuid::Uuid::new_v4().to_string(),
        host: listener.local_addr()?.to_string(),
        work: Arc::new(Semaphore::new(2)),
        open_work: Arc::new(Semaphore::new(1)),
        diagram_work: Arc::new(Semaphore::new(1)),
        diagram_waiters: Arc::new(Semaphore::new(4)),
        cancelled: Arc::new(AtomicBool::new(false)),
    };
    if let Some(source) = options.source {
        app.documents.open(source, crate::runtime::cache::cache_directory()?)?;
    }
    let cancelled = app.cancelled.clone();
    let documents = app.documents.clone();
    let references = app.references.clone();
    let comparisons = app.comparisons.clone();
    let comparison_documents = app.comparison_documents.clone();
    let maintenance_right = comparison_documents.clone();
    let maintenance_documents = documents.clone();
    let maintenance_comparisons = comparisons.clone();
    let maintenance_cancelled = cancelled.clone();
    let maintenance = std::thread::spawn(move || {
        while !maintenance_cancelled.load(Ordering::Relaxed) {
            maintenance_comparisons.expire_renderers();
            if let Some(document) = maintenance_right.peek() {
                if let Ok(mut renderer) = document.renderer.try_lock() { renderer.expire(); }
            }
            if let Some(document) = maintenance_documents.peek() {
                document.expire_operations();
                if let Ok(mut renderer) = document.renderer.try_lock() {
                    renderer.expire();
                }
            }
            std::thread::sleep(Duration::from_millis(200));
        }
    });
    println!("http://{}/#{}", app.host, app.token);
    let router = reader_routes()
        .route("/comparison/{id}/{side}/", get(compare_runtime::preview_page))
        .route("/comparison/{id}/{side}/{*path}", get(compare_runtime::preview_asset).post(compare_runtime::preview_asset))
        .route("/pane/{id}/{side}/", get(panes::page))
        .route("/pane/{id}/{side}/{*path}", get(panes::asset).post(panes::asset))
        .route("/api/panes", get(panes::status).post(panes::open).delete(panes::close))
        .route("/api/files", get(files::browse))
        .route("/api/open", post(files::open_server))
        .route("/api/edit", get(edit_runtime::page).post(edit_runtime::save))
        .route("/api/compare", get(compare_runtime::status).post(compare_runtime::open).delete(compare_runtime::close))
        .route("/api/compare/window", get(compare_runtime::window))
        .route("/api/compare/hunk", get(compare_runtime::hunk))
        .route("/api/compare/line", get(compare_runtime::line))
        .route("/api/compare/row", get(compare_runtime::row))
        .layer(middleware::from_fn_with_state(app.clone(), guard))
        .with_state(app);
    let result = axum::serve(listener, router)
        .with_graceful_shutdown({
            let cancelled = cancelled.clone();
            let documents = documents.clone();
            async move {
                let _ = tokio::signal::ctrl_c().await;
                cancelled.store(true, Ordering::Relaxed);
                documents.cancel();
            }
        })
        .await;
    cancelled.store(true, Ordering::Relaxed);
    let _ = maintenance.join();
    documents.shutdown();
    references.shutdown();
    comparisons.shutdown();
    comparison_documents.shutdown();
    result?;
    Ok(())
}

/// Static public reader assets only. No document, query, file or write routes.
/// Hosts may nest this router under their own UI prefix and security policy.
pub fn reader_assets() -> Router {
    assets()
        .route("/notes.css", get(|| async { ([(header::CONTENT_TYPE, "text/css")], include_str!("../../web/notes.css")) }))
        .route("/table-query.css", get(|| async { ([(header::CONTENT_TYPE, "text/css")], include_str!("../../web/table-query.css")) }))
        .route("/table-view.css", get(|| async { ([(header::CONTENT_TYPE, "text/css")], include_str!("../../web/table-view.css")) }))
        .route("/table-view.mjs", get(|| async { ([(header::CONTENT_TYPE, "text/javascript")], include_str!("../../web/table-view.mjs")) }))
        .route("/component-styles.mjs", get(|| async { ([(header::CONTENT_TYPE, "text/javascript")], include_str!("../../web/component-styles.mjs")) }))
        .route("/table-plot.mjs", get(|| async { ([(header::CONTENT_TYPE, "text/javascript")], include_str!("../../web/table-plot.mjs")) }))
        .route("/table-plot.css", get(|| async { ([(header::CONTENT_TYPE, "text/css")], include_str!("../../web/table-plot.css")) }))
        .route("/clipboard.mjs", get(|| async { ([(header::CONTENT_TYPE, "text/javascript")], include_str!("../../web/clipboard.mjs")) }))
        .route("/table-cells.css", get(|| async { ([(header::CONTENT_TYPE, "text/css")], include_str!("../../web/table-cells.css")) }))
        .route("/table-cells.mjs", get(|| async { ([(header::CONTENT_TYPE, "text/javascript")], include_str!("../../web/table-cells.mjs")) }))
        .route("/epub-reader.mjs", get(|| async { ([(header::CONTENT_TYPE, "text/javascript")], include_str!("../../web/epub-reader.mjs")) }))
        .route("/epub-reader.css", get(|| async { ([(header::CONTENT_TYPE, "text/css")], include_str!("../../web/epub-reader.css")) }))
        .route("/vendor/epub.mjs", get(|| async { ([(header::CONTENT_TYPE, "text/javascript")], include_str!("../../web/vendor/epub.mjs")) }))
        .route("/vendor/pdf/{*path}", get(pdf_runtime::resource))
        .route("/pdf-reader.mjs", get(|| async { ([(header::CONTENT_TYPE, "text/javascript")], include_str!("../../web/pdf-reader.mjs")) }))
        .route("/pdf-reader.css", get(|| async { ([(header::CONTENT_TYPE, "text/css")], include_str!("../../web/pdf-reader.css")) }))
}

fn reader_routes() -> Router<App> {
    reader_assets().with_state(())
        .route("/api/reference", get(references::info))
        .route("/api/reference-image", get(references::image))
        .route("/api/epub", get(epub_runtime::description))
        .route("/epub-resource/{document}/{capability}/{*path}", get(epub_runtime::resource))
        .route("/api/original", get(pdf_runtime::original))
        .route("/api/capabilities", get(service::capabilities))
        .route("/api/operations", post(service::operation))
        .route("/api/selection/source", post(service::selection))
        .route("/api/document", get(document))
        .route("/api/window", get(window))
        .route("/api/projection", get(projection_window))
        .route("/api/table", get(table_window))
        .route("/api/table-chart", post(table_chart_runtime::run))
        .route("/api/table-query", get(table_query_runtime::page).post(table_query_runtime::run))
        .route("/api/table-query/schema", get(table_query_runtime::schema))
        .route("/api/table-query/close", post(table_query_runtime::close))
        .route("/api/asset", get(document_runtime::asset))
        .route("/api/asset-info", get(document_runtime::asset_info))
        .route("/api/notes", post(document_runtime::notes))
        .route("/api/jq-query", post(document_runtime::jq))
        .route("/api/search", get(find))
        .route("/api/source", get(source_window))
        .route("/api/diagram", get(diagram))
        .route("/api/diagram-source", get(diagram_source))
        .route("/api/renderer", get(renderer_status))
}

pub fn index_worker() -> Option<std::io::Result<()>> {
    crate::core::reader::worker_entry()
}
