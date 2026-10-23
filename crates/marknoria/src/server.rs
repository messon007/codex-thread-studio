use axum::{
    Json, Router,
    extract::{Query, Request, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
};
#[path = "compare_runtime.rs"]
mod compare_runtime;
#[path = "edit_runtime.rs"]
mod edit_runtime;
#[path = "files.rs"]
mod files;
#[path = "panes.rs"]
mod panes;
#[path = "renderer.rs"]
mod renderer;
#[path = "worker.rs"]
mod worker;
use crate::source::read_source;
use crate::{Index, audit_tables, build_index, read_diagram, read_page, read_table, search};
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
    editable: bool,
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
    let rendering_page = request.uri().path() == "/render.html";
    let comparison_page = request.uri().path() == "/compare.html";
    let comparison_preview = request.uri().path().starts_with("/comparison/")
        || request.uri().path().starts_with("/pane/");
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
    let policy = if rendering_page {
        "default-src 'none'; script-src 'self' 'wasm-unsafe-eval'; worker-src 'self' blob:; style-src 'self' 'unsafe-inline'; connect-src 'none'; img-src data: blob:; font-src data:; object-src 'none'; base-uri 'none'; frame-ancestors 'none'; form-action 'none'"
    } else if comparison_page {
        "default-src 'none'; script-src 'self'; worker-src 'none'; style-src 'self' 'unsafe-inline'; connect-src 'self' blob:; img-src blob:; frame-src 'self'; object-src 'none'; base-uri 'none'; frame-ancestors 'none'; form-action 'none'"
    } else if comparison_preview {
        "default-src 'none'; script-src 'self'; worker-src 'none'; style-src 'self' 'unsafe-inline'; connect-src 'self' blob:; img-src blob:; object-src 'none'; base-uri 'none'; frame-ancestors 'self'; form-action 'none'"
    } else {
        "default-src 'none'; script-src 'self'; worker-src 'none'; style-src 'self' 'unsafe-inline'; connect-src 'self' blob:; img-src blob:; object-src 'none'; base-uri 'none'; frame-ancestors 'none'; form-action 'none'"
    };
    response.headers_mut().insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(policy),
    );
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

async fn diagram_source(
    State(app): State<App>,
    Query(query): Query<DiagramQuery>,
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
        let metadata = index
            .diagrams
            .get(query.id)
            .ok_or((StatusCode::BAD_REQUEST, "Diagram out of range".into()))?;
        let source = read_diagram(&document.directory, &index, query.id).map_err(failure)?;
        Ok(Json(
            json!({"id":query.id,"revision":index.revision,"kind":metadata.kind,"source":source}),
        ))
    })
    .await
    .map_err(failure)?
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
        if let Some(error) = &progress.error { return Ok(Json(json!({"state":"error", "error":error,"documentId":document.id,"editable":app.editable}))); }
        let Some(index) = &progress.index else { return Ok(Json(json!({"state":"indexing","documentId":document.id,"editable":app.editable}))); };
        let pages = (0..index.pages.len().min(3)).map(|page| Ok(json!({"id":page,"html":read_page(&document.directory, index, page).map_err(failure)?}))).collect::<Result<Vec<_>, ApiError>>()?;
        Ok(Json(json!({"state":"ready", "documentId":document.id,"index":index.as_ref(), "initial":pages,"editable":app.editable})))
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
        let matches = search(&document.directory, &index, &query.query).map_err(failure)?;
        Ok(Json(
            json!({"matches":matches,"limit":100,"mode":"case-sensitive-source-lines"}),
        ))
    })
    .await
    .map_err(failure)?
}

#[derive(Deserialize)]
struct SourceQuery {
    line: Option<usize>,
    offset: Option<usize>,
    #[serde(default)]
    previous: bool,
}

async fn source_window(
    State(app): State<App>,
    Query(query): Query<SourceQuery>,
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
        let result = read_source(
            &document.directory,
            &index,
            query.line,
            query.offset,
            query.previous,
        )
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::InvalidInput {
                (StatusCode::BAD_REQUEST, error.to_string())
            } else {
                failure(error)
            }
        })?;
        Ok(Json(json!({"revision":index.revision,"result":result})))
    })
    .await
    .map_err(failure)?
}

fn assets() -> Router<App> {
    Router::new()
        .route(
            "/theme.js",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/javascript")],
                    include_str!("../web/theme.js"),
                )
            }),
        )
        .route(
            "/editor.mjs",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/javascript")],
                    include_str!("../web/editor.mjs"),
                )
            }),
        )
        .route(
            "/vendor/editor.mjs",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/javascript")],
                    include_str!("../../../ui/vendor/marknoria-editor.mjs"),
                )
            }),
        )
        .route(
            "/syntax-light.css",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/css")],
                    crate::highlight::css(false),
                )
            }),
        )
        .route(
            "/syntax-dark.css",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/css")],
                    crate::highlight::css(true),
                )
            }),
        )
        .route(
            "/reader-tools.mjs",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/javascript")],
                    include_str!("../web/reader-tools.mjs"),
                )
            }),
        )
        .route(
            "/position.mjs",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/javascript")],
                    include_str!("../web/position.mjs"),
                )
            }),
        )
        .route(
            "/compare-source.mjs",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/javascript")],
                    include_str!("../web/compare-source.mjs"),
                )
            }),
        )
        .route(
            "/checkbox.css",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/css")],
                    include_str!("../web/checkbox.css"),
                )
            }),
        )
        .route(
            "/compare.html",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
                    include_str!("../web/compare.html"),
                )
            }),
        )
        .route(
            "/compare.mjs",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/javascript")],
                    include_str!("../web/compare.mjs"),
                )
            }),
        )
        .route(
            "/compare.css",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/css")],
                    include_str!("../web/compare.css"),
                )
            }),
        )
        .route(
            "/icons.mjs",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/javascript")],
                    include_str!("../web/icons.mjs"),
                )
            }),
        )
        .route(
            "/source.mjs",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/javascript")],
                    include_str!("../web/source.mjs"),
                )
            }),
        )
        .route(
            "/files.mjs",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/javascript")],
                    include_str!("../web/files.mjs"),
                )
            }),
        )
        .route(
            "/tables.mjs",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/javascript")],
                    include_str!("../web/tables.mjs"),
                )
            }),
        )
        .route(
            "/render.html",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
                    include_str!("../web/render.html"),
                )
            }),
        )
        .route(
            "/engine.mjs",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/javascript")],
                    include_str!("../web/engine.mjs"),
                )
            }),
        )
        .route(
            "/diagrams.mjs",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/javascript")],
                    include_str!("../web/diagrams.mjs"),
                )
            }),
        )
        .route(
            "/vendor/mermaid.min.js",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/javascript")],
                    include_str!("../../../ui/vendor/mermaid.min.js"),
                )
            }),
        )
        .route(
            "/vendor/plantuml.js",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/javascript")],
                    include_str!("../../../ui/vendor/plantuml.js"),
                )
            }),
        )
        .route(
            "/vendor/plantuml-viz-global.js",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/javascript")],
                    include_str!("../../../ui/vendor/plantuml-viz-global.js"),
                )
            }),
        )
        .route(
            "/vendor/d2.js",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/javascript")],
                    include_str!("../../../ui/vendor/d2.js"),
                )
            }),
        )
        .route(
            "/",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
                    include_str!("../web/index.html"),
                )
            }),
        )
        .route(
            "/reader.mjs",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/javascript")],
                    include_str!("../web/reader.mjs"),
                )
            }),
        )
        .route(
            "/styles.css",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/css")],
                    include_str!("../web/styles.css"),
                )
            }),
        )
        .route(
            "/vendor/virtual-core.mjs",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/javascript")],
                    include_str!("../../../ui/vendor/marknoria-virtual-core.mjs"),
                )
            }),
        )
        .route(
            "/vendor/purify.mjs",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/javascript")],
                    include_str!("../../../ui/vendor/purify.es.mjs"),
                )
            }),
        )
}

pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
    if let Some(result) = compare_runtime::worker() {
        return result.map_err(Into::into);
    }
    if let Some(result) = index_worker() {
        return result.map_err(Into::into);
    }
    let arguments: Vec<_> = env::args().collect();
    if arguments.get(1).is_some_and(|argument| argument == "audit") {
        let directory = files::temporary_directory("markdown-reader-audit-")?;
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
    let listener =
        tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, options.port)).await?;
    let app = App {
        editable: true,
        editor: Arc::new(edit_runtime::Editor::default()),
        comparisons: Arc::new(compare_runtime::Comparisons::new()),
        comparison_documents: Arc::new(files::Documents::new(options.root.clone())),
        documents: Arc::new(files::Documents::new(options.root)),
        token: uuid::Uuid::new_v4().to_string(),
        host: listener.local_addr()?.to_string(),
        work: Arc::new(Semaphore::new(2)),
        open_work: Arc::new(Semaphore::new(1)),
        diagram_work: Arc::new(Semaphore::new(1)),
        diagram_waiters: Arc::new(Semaphore::new(4)),
        cancelled: Arc::new(AtomicBool::new(false)),
    };
    if let Some(source) = options.source {
        app.documents.open(source, files::cache_directory()?)?;
    }
    let cancelled = app.cancelled.clone();
    let documents = app.documents.clone();
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
                if let Ok(mut renderer) = document.renderer.try_lock() {
                    renderer.expire();
                }
            }
            if let Some(document) = maintenance_documents.peek() {
                if let Ok(mut renderer) = document.renderer.try_lock() {
                    renderer.expire();
                }
            }
            std::thread::sleep(Duration::from_millis(200));
        }
    });
    println!("http://{}/#{}", app.host, app.token);
    let router = reader_routes()
        .route(
            "/comparison/{id}/{side}/",
            get(compare_runtime::preview_page),
        )
        .route(
            "/comparison/{id}/{side}/{*path}",
            get(compare_runtime::preview_asset),
        )
        .route("/pane/{id}/{side}/", get(panes::page))
        .route("/pane/{id}/{side}/{*path}", get(panes::asset))
        .route(
            "/api/panes",
            get(panes::status).post(panes::open).delete(panes::close),
        )
        .route("/api/files", get(files::browse))
        .route("/api/open", post(files::open_server))
        .route(
            "/api/edit",
            get(edit_runtime::page).post(edit_runtime::save),
        )
        .route(
            "/api/compare",
            get(compare_runtime::status)
                .post(compare_runtime::open)
                .delete(compare_runtime::close),
        )
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
    comparisons.shutdown();
    comparison_documents.shutdown();
    result?;
    Ok(())
}

fn reader_routes() -> Router<App> {
    assets()
        .route("/api/document", get(document))
        .route("/api/window", get(window))
        .route("/api/table", get(table_window))
        .route("/api/search", get(find))
        .route("/api/source", get(source_window))
        .route("/api/diagram", get(diagram))
        .route("/api/diagram-source", get(diagram_source))
        .route("/api/renderer", get(renderer_status))
}

pub fn index_worker() -> Option<std::io::Result<()>> {
    let arguments: Vec<_> = env::args_os().collect();
    if arguments
        .get(1)
        .is_none_or(|argument| argument != "--marknoria-index")
    {
        return None;
    }
    Some((|| {
        let source = arguments
            .get(2)
            .ok_or_else(|| std::io::Error::other("Missing input file"))?;
        let output = arguments
            .get(3)
            .ok_or_else(|| std::io::Error::other("Missing cache directory"))?;
        build_index(Path::new(source), Path::new(output))?;
        Ok(())
    })())
}

pub struct Service {
    pub address: std::net::SocketAddr,
    pub token: String,
    stop: Option<tokio::sync::oneshot::Sender<()>>,
}

impl Drop for Service {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
    }
}

impl Service {
    pub async fn start(source: std::path::PathBuf) -> std::io::Result<Self> {
        Self::start_with_editing(source, false).await
    }

    pub async fn start_editable(source: std::path::PathBuf) -> std::io::Result<Self> {
        Self::start_with_editing(source, true).await
    }

    async fn start_with_editing(
        source: std::path::PathBuf,
        editable: bool,
    ) -> std::io::Result<Self> {
        let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).await?;
        let address = listener.local_addr()?;
        let documents = Arc::new(files::Documents::new(
            source.parent().unwrap_or(Path::new(".")).to_owned(),
        ));
        documents.open(source, files::cache_directory()?)?;
        let token = uuid::Uuid::new_v4().to_string();
        let app = App {
            editable,
            editor: Arc::new(edit_runtime::Editor::default()),
            comparisons: Arc::new(compare_runtime::Comparisons::new()),
            comparison_documents: Arc::new(files::Documents::new(documents.root.clone())),
            documents: documents.clone(),
            token: token.clone(),
            host: address.to_string(),
            work: Arc::new(Semaphore::new(2)),
            open_work: Arc::new(Semaphore::new(1)),
            diagram_work: Arc::new(Semaphore::new(1)),
            diagram_waiters: Arc::new(Semaphore::new(4)),
            cancelled: Arc::new(AtomicBool::new(false)),
        };
        let router = assets()
            .route("/api/document", get(document))
            .route("/api/window", get(window))
            .route("/api/table", get(table_window))
            .route("/api/search", get(find))
            .route("/api/source", get(source_window))
            .route("/api/diagram", get(diagram))
            .route("/api/diagram-source", get(diagram_source))
            .route("/api/renderer", get(renderer_status));
        let router = if editable {
            router.route(
                "/api/edit",
                get(edit_runtime::page).post(edit_runtime::save),
            )
        } else {
            router
        };
        let router = router
            .layer(middleware::from_fn_with_state(app.clone(), guard))
            .with_state(app);
        let (stop, stopped) = tokio::sync::oneshot::channel();
        let maintenance_documents = documents.clone();
        let maintenance = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(500));
            loop {
                interval.tick().await;
                if let Some(document) = maintenance_documents.peek() {
                    if let Ok(mut renderer) = document.renderer.try_lock() {
                        renderer.expire();
                    }
                }
            }
        });
        tokio::spawn(async move {
            let closing = documents.clone();
            let _ = axum::serve(listener, router)
                .with_graceful_shutdown(async move {
                    let _ = stopped.await;
                    closing.cancel();
                })
                .await;
            maintenance.abort();
            let _ = tokio::task::spawn_blocking(move || documents.shutdown()).await;
        });
        Ok(Self {
            address,
            token,
            stop: Some(stop),
        })
    }
}
