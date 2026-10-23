use super::{ApiError, App, failure, files};
use crate::compare::{self, ComparisonIndex};
use axum::{
    Json,
    extract::{Path, Query, Request, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    fs,
    path::PathBuf,
    process::{Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::JoinHandle,
};
use tokio::sync::Semaphore;
use tower::ServiceExt;

pub(super) struct Comparison {
    id: String,
    left: String,
    right: String,
    directory: PathBuf,
    cache: Mutex<Option<tempfile::TempDir>>,
    state: Mutex<Result<Option<Arc<ComparisonIndex>>, String>>,
    cancelled: AtomicBool,
    previews: Mutex<[Option<Arc<files::Documents>>; 2]>,
    readers: Option<[String; 2]>,
}

impl Drop for Comparison {
    fn drop(&mut self) {
        for documents in self
            .previews
            .get_mut()
            .unwrap()
            .iter_mut()
            .filter_map(Option::take)
        {
            documents.shutdown();
        }
    }
}

pub(super) struct Comparisons {
    current: Mutex<Option<Arc<Comparison>>>,
    jobs: Mutex<Vec<JoinHandle<()>>>,
    work: Arc<Semaphore>,
}

impl Comparisons {
    pub(super) fn for_readers(&self, readers: [&str; 2]) -> Option<String> {
        self.current
            .lock()
            .ok()?
            .as_ref()
            .filter(|job| {
                job.readers
                    .as_ref()
                    .is_some_and(|ids| ids[0] == readers[0] && ids[1] == readers[1])
            })
            .map(|job| job.id.clone())
    }
    pub(super) fn new() -> Self {
        Self {
            current: Mutex::new(None),
            jobs: Mutex::new(Vec::new()),
            work: Arc::new(Semaphore::new(1)),
        }
    }
    pub(super) fn cancel(&self) {
        if let Some(job) = self.current.lock().unwrap().take() {
            job.cancelled.store(true, Ordering::Relaxed);
        }
    }
    pub(super) fn shutdown(&self) {
        self.cancel();
        for job in self.jobs.lock().unwrap().drain(..) {
            let _ = job.join();
        }
    }
    pub(super) fn expire_renderers(&self) {
        if let Some(job) = self.current.lock().unwrap().as_ref() {
            for documents in job.previews.lock().unwrap().iter().flatten() {
                if let Some(document) = documents.peek() {
                    if let Ok(mut renderer) = document.renderer.try_lock() {
                        renderer.expire();
                    }
                }
            }
        }
    }
    fn select(&self, headers: &HeaderMap) -> Result<Arc<Comparison>, ApiError> {
        let job = self.current.lock().map_err(failure)?.clone().ok_or((
            StatusCode::GONE,
            "Comparison closed; choose two files again".into(),
        ))?;
        if headers
            .get("x-reader-comparison")
            .is_none_or(|value| value.as_bytes() != job.id.as_bytes())
        {
            return Err((
                StatusCode::CONFLICT,
                "Comparison changed; reopen this tab".into(),
            ));
        }
        Ok(job)
    }
}

#[derive(Deserialize)]
pub(super) struct OpenRequest {
    left: String,
    right: String,
    readers: Option<[String; 2]>,
}

pub(super) async fn open(
    State(app): State<App>,
    Json(request): Json<OpenRequest>,
) -> Result<Json<Value>, ApiError> {
    if request.left.len() > 4096 || request.right.len() > 4096 {
        return Err((StatusCode::BAD_REQUEST, "Path is too long".into()));
    }
    let permit = app
        .comparisons
        .work
        .clone()
        .try_acquire_owned()
        .map_err(|_| {
            (
                StatusCode::CONFLICT,
                "Comparison is running; cancel it before opening another".into(),
            )
        })?;
    tokio::task::spawn_blocking(move || {
        let root = &app.documents.root;
        let validate = |relative: &str| -> Result<PathBuf, ApiError> {
            let path = files::confined_path(root, relative).map_err(|_| {
                (
                    StatusCode::BAD_REQUEST,
                    "Comparison file is outside the server folder or unavailable".into(),
                )
            })?;
            if !path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(files::markdown_name)
                || !path.metadata().map_err(failure)?.is_file()
            {
                return Err((
                    StatusCode::BAD_REQUEST,
                    "Choose regular Markdown files".into(),
                ));
            }
            Ok(path)
        };
        let pinned = if let Some(ids) = &request.readers {
            let documents = [app.documents.peek(), app.comparison_documents.peek()];
            let mut selected = Vec::new();
            for (position, document) in documents.into_iter().enumerate() {
                let document = document
                    .filter(|document| document.id == ids[position])
                    .ok_or((StatusCode::CONFLICT, "Reading pane changed".into()))?;
                let expected = if position == 0 {
                    &request.left
                } else {
                    &request.right
                };
                if document
                    .source
                    .strip_prefix(root)
                    .ok()
                    .is_none_or(|path| path != std::path::Path::new(expected))
                {
                    return Err((StatusCode::CONFLICT, "Reading pane path changed".into()));
                }
                let source = if super::index(&document).is_ok() {
                    document.directory.join("source.md")
                } else if document.progress.lock().map_err(failure)?.error.is_some() {
                    validate(expected)?
                } else {
                    return Err((
                        StatusCode::CONFLICT,
                        "Reading pane is still indexing".into(),
                    ));
                };
                selected.push((document, source));
            }
            Some(selected)
        } else {
            None
        };
        let left = match &pinned {
            Some(documents) => documents[0].1.clone(),
            None => validate(&request.left)?,
        };
        let right = match &pinned {
            Some(documents) => documents[1].1.clone(),
            None => validate(&request.right)?,
        };
        if left
            .metadata()
            .map_err(failure)?
            .len()
            .saturating_add(right.metadata().map_err(failure)?.len())
            > compare::MAX_COMBINED
        {
            return Err((
                StatusCode::PAYLOAD_TOO_LARGE,
                "Comparison inputs exceed the combined 128 MiB limit".into(),
            ));
        }
        let cache = files::temporary_directory("markdown-compare-").map_err(failure)?;
        let job = Arc::new(Comparison {
            id: uuid::Uuid::new_v4().to_string(),
            left: request.left,
            right: request.right,
            directory: cache.path().to_owned(),
            cache: Mutex::new(Some(cache)),
            state: Mutex::new(Ok(None)),
            cancelled: AtomicBool::new(false),
            previews: Mutex::new([None, None]),
            readers: request.readers,
        });
        let worker = job.clone();
        let handle = std::thread::Builder::new()
            .name("markdown-compare".into())
            .spawn(move || {
                let _permit = permit;
                let _pinned = pinned;
                let outcome = (|| {
                    let mut child =
                        Command::new(std::env::current_exe().map_err(|error| error.to_string())?)
                            .arg("--marknoria-compare")
                            .arg(left)
                            .arg(right)
                            .arg(&worker.directory)
                            .stdin(Stdio::null())
                            .stdout(Stdio::null())
                            .stderr(Stdio::piped())
                            .spawn()
                            .map_err(|error| error.to_string())?;
                    files::supervise(&mut child, &worker.cancelled)?;
                    let bytes = fs::read(worker.directory.join("comparison.json"))
                        .map_err(|error| error.to_string())?;
                    serde_json::from_slice::<ComparisonIndex>(&bytes)
                        .map(Arc::new)
                        .map_err(|error| error.to_string())
                })();
                *worker.state.lock().unwrap() = match outcome {
                    Ok(index) => Ok(Some(index)),
                    Err(mut error) => {
                        if let Some(cache) = worker.cache.lock().unwrap().take() {
                            if let Err(cleanup) = cache.close() {
                                error.push_str(&format!("; cache cleanup failed: {cleanup}"));
                            }
                        }
                        Err(format!("{error}. Cache: {}", worker.directory.display()))
                    }
                };
            })
            .map_err(failure)?;
        app.comparisons.cancel();
        *app.comparisons.current.lock().map_err(failure)? = Some(job.clone());
        let mut jobs = app.comparisons.jobs.lock().map_err(failure)?;
        jobs.retain(|job| !job.is_finished());
        jobs.push(handle);
        Ok(Json(json!({"comparisonId":job.id})))
    })
    .await
    .map_err(failure)?
}

pub(super) async fn status(
    State(app): State<App>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let job = app.comparisons.select(&headers)?;
    let state = job.state.lock().map_err(failure)?;
    Ok(Json(match &*state {
        Ok(Some(index)) => {
            json!({"state":"ready","comparisonId":job.id,"left":job.left,"right":job.right,"rows":index.rows,"changes":index.changes.len(),"added":index.added,"removed":index.removed,"leftLines":index.left_lines,"rightLines":index.right_lines,"leftHash":index.left_hash,"rightHash":index.right_hash,"boundedAlignment":index.bounded_alignment,"elapsedMs":index.elapsed_ms,"peakRssKiB":index.peak_rss_kib})
        }
        Ok(None) => {
            json!({"state":"comparing","comparisonId":job.id,"left":job.left,"right":job.right})
        }
        Err(error) => json!({"state":"error","error":error}),
    }))
}

pub(super) async fn close(
    State(app): State<App>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let mut current = app.comparisons.current.lock().map_err(failure)?;
    if current.as_ref().is_some_and(|job| {
        headers
            .get("x-reader-comparison")
            .is_some_and(|value| value.as_bytes() == job.id.as_bytes())
    }) {
        if let Some(job) = current.take() {
            job.cancelled.store(true, Ordering::Relaxed);
        }
        return Ok(Json(json!({"closed":true})));
    }
    Err((
        StatusCode::CONFLICT,
        "Comparison changed or already closed".into(),
    ))
}

#[derive(Deserialize)]
pub(super) struct WindowQuery {
    #[serde(default)]
    row: usize,
}

fn ready(job: &Comparison) -> Result<Arc<ComparisonIndex>, ApiError> {
    job.state
        .lock()
        .map_err(failure)?
        .as_ref()
        .map_err(|error| (StatusCode::BAD_REQUEST, error.clone()))?
        .clone()
        .ok_or((StatusCode::CONFLICT, "Comparison is not ready".into()))
}

pub(super) async fn window(
    State(app): State<App>,
    headers: HeaderMap,
    Query(query): Query<WindowQuery>,
) -> Result<Json<Value>, ApiError> {
    let job = app.comparisons.select(&headers)?;
    let permit = app
        .work
        .clone()
        .try_acquire_owned()
        .map_err(|_| (StatusCode::TOO_MANY_REQUESTS, "Reader is busy".into()))?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let index = ready(&job)?;
        let rows = compare::read_rows(&job.directory, &index, query.row)
            .map_err(|error| (StatusCode::BAD_REQUEST, error.to_string()))?;
        Ok(Json(json!({"comparisonId":job.id,"rows":rows})))
    })
    .await
    .map_err(failure)?
}

#[derive(Deserialize)]
pub(super) struct HunkQuery {
    index: usize,
}

pub(super) async fn hunk(
    State(app): State<App>,
    headers: HeaderMap,
    Query(query): Query<HunkQuery>,
) -> Result<Json<Value>, ApiError> {
    let job = app.comparisons.select(&headers)?;
    let permit = app
        .work
        .clone()
        .try_acquire_owned()
        .map_err(|_| (StatusCode::TOO_MANY_REQUESTS, "Reader is busy".into()))?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let index = ready(&job)?;
        let row = *index.changes.get(query.index)
            .ok_or((StatusCode::BAD_REQUEST, "Change out of range".into()))?;
        let lines = compare::hunk_lines(&job.directory, &index, row).map_err(failure)?;
        Ok(Json(json!({"comparisonId":job.id,"index":query.index,"row":row,"leftLine":lines[0],"rightLine":lines[1]})))
    }).await.map_err(failure)?
}

pub(super) async fn preview_page(
    State(app): State<App>,
    Path((id, side)): Path<(String, String)>,
    request: Request,
) -> Response {
    preview(app, id, side, String::new(), request).await
}

pub(super) async fn preview_asset(
    State(app): State<App>,
    Path((id, side, path)): Path<(String, String, String)>,
    request: Request,
) -> Response {
    preview(app, id, side, path, request).await
}

async fn preview(
    mut app: App,
    id: String,
    side: String,
    path: String,
    request: Request,
) -> Response {
    let selected = (|| -> Result<Arc<files::Documents>, ApiError> {
        let position = match side.as_str() {
            "left" => 0,
            "right" => 1,
            _ => return Err((StatusCode::BAD_REQUEST, "Invalid comparison side".into())),
        };
        let job = app
            .comparisons
            .current
            .lock()
            .map_err(failure)?
            .clone()
            .filter(|job| job.id == id && !job.cancelled.load(Ordering::Relaxed))
            .ok_or((StatusCode::GONE, "Comparison closed or replaced".into()))?;
        ready(&job)?;
        let mut previews = job.previews.lock().map_err(failure)?;
        if let Some(documents) = &previews[position] {
            return Ok(documents.clone());
        }
        let documents = Arc::new(files::Documents::new(job.directory.clone()));
        documents
            .open(
                job.directory.join(format!("{side}.md")),
                files::cache_directory().map_err(failure)?,
            )
            .map_err(failure)?;
        previews[position] = Some(documents.clone());
        Ok(documents)
    })();
    app.documents = match selected {
        Ok(documents) => documents,
        Err(error) => return error.into_response(),
    };
    serve_reader(app, path, request).await
}

pub(super) async fn serve_reader(mut app: App, path: String, mut request: Request) -> Response {
    app.editable = false;
    if !matches!(
        path.as_str(),
        "" | "styles.css"
            | "checkbox.css"
            | "reader.mjs"
            | "source.mjs"
            | "position.mjs"
            | "theme.js"
            | "syntax-light.css"
            | "syntax-dark.css"
            | "vendor/editor.mjs"
            | "editor.mjs"
            | "tables.mjs"
            | "icons.mjs"
            | "files.mjs"
            | "diagrams.mjs"
            | "vendor/virtual-core.mjs"
            | "vendor/purify.mjs"
            | "api/document"
            | "api/window"
            | "api/table"
            | "api/source"
            | "api/search"
            | "api/diagram"
            | "api/renderer"
    ) {
        return StatusCode::NOT_FOUND.into_response();
    }
    if path.starts_with("api/")
        && request
            .headers()
            .get("x-reader-token")
            .is_none_or(|token| token.as_bytes() != app.token.as_bytes())
    {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let query = request
        .uri()
        .query()
        .map(|query| format!("?{query}"))
        .unwrap_or_default();
    *request.uri_mut() = match format!("/{path}{query}").parse() {
        Ok(uri) => uri,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };
    match super::reader_routes()
        .with_state(app)
        .oneshot(request)
        .await
    {
        Ok(response) => response,
        Err(error) => match error {},
    }
}

#[derive(Deserialize)]
pub(super) struct RowQuery {
    side: String,
    line: usize,
}

pub(super) async fn row(
    State(app): State<App>,
    headers: HeaderMap,
    Query(query): Query<RowQuery>,
) -> Result<Json<Value>, ApiError> {
    let job = app.comparisons.select(&headers)?;
    let permit = app
        .work
        .clone()
        .try_acquire_owned()
        .map_err(|_| (StatusCode::TOO_MANY_REQUESTS, "Reader is busy".into()))?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let index = ready(&job)?;
        let row = compare::row_for_line(&job.directory, &index, &query.side, query.line)
            .map_err(|error| (StatusCode::BAD_REQUEST, error.to_string()))?;
        Ok(Json(json!({"comparisonId":job.id,"row":row})))
    })
    .await
    .map_err(failure)?
}

#[derive(Deserialize)]
pub(super) struct LineQuery {
    row: usize,
    side: String,
    #[serde(default)]
    offset: usize,
    #[serde(default)]
    previous: bool,
}

pub(super) async fn line(
    State(app): State<App>,
    headers: HeaderMap,
    Query(query): Query<LineQuery>,
) -> Result<Json<Value>, ApiError> {
    let job = app.comparisons.select(&headers)?;
    let permit = app
        .work
        .clone()
        .try_acquire_owned()
        .map_err(|_| (StatusCode::TOO_MANY_REQUESTS, "Reader is busy".into()))?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let index = ready(&job)?;
        let result = compare::read_line(
            &job.directory,
            &index,
            query.row,
            &query.side,
            query.offset,
            query.previous,
        )
        .map_err(|error| (StatusCode::BAD_REQUEST, error.to_string()))?;
        Ok(Json(json!({"comparisonId":job.id,"result":result})))
    })
    .await
    .map_err(failure)?
}

pub(super) fn worker() -> Option<std::io::Result<()>> {
    let arguments: Vec<_> = std::env::args_os().collect();
    if arguments
        .get(1)
        .is_none_or(|argument| argument != "--marknoria-compare")
    {
        return None;
    }
    Some((|| {
        if arguments.len() != 5 {
            return Err(std::io::Error::other(
                "Comparison requires two files and a private cache directory",
            ));
        }
        compare::build_comparison(
            arguments[2].as_ref(),
            arguments[3].as_ref(),
            arguments[4].as_ref(),
        )?;
        Ok(())
    })())
}
