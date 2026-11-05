use crate::runtime::cache::cache_directory;
use crate::runtime::process::supervise_owned;
use super::{ApiError, App, Progress, failure, renderer};
use axum::{
    Json,
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
};
use crate::core::{Index, MAX_SOURCE};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    fs,
    io,
    path::{Component, Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::Duration,
};
use tokio::sync::Semaphore;

pub struct Document {
    projection: crate::core::ProjectionProfile,
    pub content_token: String,
    pub id: String,
    pub selected_path: Option<String>,
    pub source: PathBuf,
    pub directory: PathBuf,
    pub progress: Mutex<Progress>,
    pub renderer: Mutex<renderer::Renderer>,
    pub queries: Mutex<crate::core::table_query::Queries>,
    pub query_work: Arc<Semaphore>,
    pub jq: Mutex<crate::core::jq_query::Queries>,
    pub assets: Mutex<()>,
    pub operations: crate::core::reader::Operations,
    pub reserved_operations: Mutex<std::collections::HashMap<String, (std::time::Instant, crate::core::reader::Operation)>>,
    pub cancelled: Arc<AtomicBool>,
    owner_cancelled: Option<Arc<AtomicBool>>,
    cache: Mutex<Option<tempfile::TempDir>>,
}

pub struct Documents {
    owner_cancelled: Option<Arc<AtomicBool>>,
    projection: crate::core::ProjectionProfile,
    pub root: PathBuf,
    selected_directory: Option<PathBuf>,
    current: Mutex<Option<Arc<Document>>>,
    jobs: Mutex<Vec<JoinHandle<()>>>,
    index_work: Arc<Semaphore>,
}

fn bad(message: &str) -> ApiError {
    (StatusCode::BAD_REQUEST, message.into())
}

pub fn markdown_name(name: &str) -> bool {
    crate::core::document_format::DocumentFormat::for_name(name).is_some()
}

pub fn confined_path(root: &Path, relative: &str) -> io::Result<PathBuf> {
    if Path::new(relative)
        .components()
        .any(|part| !matches!(part, Component::Normal(_)))
        && !relative.is_empty()
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "Choose a file inside the server folder",
        ));
    }
    let path = root.join(relative).canonicalize()?;
    if !path.starts_with(root) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "Path escapes the server folder",
        ));
    }
    Ok(path)
}

impl Document {
    pub(super) fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed) || self.owner_cancelled.as_ref().is_some_and(|owner| owner.load(Ordering::Relaxed))
    }
    pub(super) fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
        self.operations.close();
        self.reserved_operations.lock().unwrap().clear();
    }
    pub(super) fn expire_operations(&self) {
        self.reserved_operations.lock().unwrap().retain(|_, (registered, operation)|
            registered.elapsed() < Duration::from_secs(30) && !operation.cancelled());
    }
    pub(super) fn operation(&self, id: Option<&str>) -> Result<crate::core::reader::Operation, ApiError> {
        self.expire_operations();
        let operation = if let Some(id) = id {
            self.reserved_operations.lock().unwrap().remove(id).map(|(_, operation)| operation)
                .ok_or((StatusCode::CONFLICT, "Operation was not reserved, was cancelled, or was already used".into()))?
        } else {
            self.operations.begin().map_err(|e| (StatusCode::TOO_MANY_REQUESTS, e.to_string()))?
        };
        self.operations.check(&operation).map_err(|e| (StatusCode::CONFLICT, e.to_string()))?;
        Ok(operation)
    }
}

impl Documents {
    pub fn new(root: PathBuf) -> Self {
        Self {
            owner_cancelled: None,
            projection: crate::core::ProjectionProfile::Web,
            root,
            selected_directory: None,
            current: Mutex::new(None),
            jobs: Mutex::new(Vec::new()),
            index_work: {
                static WORK: std::sync::OnceLock<Arc<Semaphore>> = std::sync::OnceLock::new();
                WORK.get_or_init(|| Arc::new(Semaphore::new(1))).clone()
            },
        }
    }

    pub fn with_selected_directory(mut self, directory: Option<PathBuf>) -> Self {
        self.selected_directory = directory;
        self
    }

    pub fn with_projection(mut self, projection: crate::core::ProjectionProfile) -> Self {
        self.projection = projection;
        self
    }
    pub(super) fn with_owner_cancellation(mut self, owner: Arc<AtomicBool>) -> Self {
        self.owner_cancelled = Some(owner);
        self
    }

    pub fn resolve_path(&self, path: &str) -> io::Result<PathBuf> {
        if crate::web::selected_files::is_handle(path) {
            crate::web::selected_files::resolve(self.selected_directory.as_deref(), path)
        } else {
            confined_path(&self.root, path)
        }
    }

    pub fn peek(&self) -> Option<Arc<Document>> {
        self.current.lock().unwrap().clone()
    }

    pub fn select(&self, headers: &HeaderMap) -> Result<Arc<Document>, ApiError> {
        let document = self
            .peek()
            .ok_or((StatusCode::CONFLICT, "Open a document first".into()))?;
        if headers
            .get("x-reader-document")
            .is_some_and(|value| value.as_bytes() != document.id.as_bytes())
        {
            return Err((
                StatusCode::CONFLICT,
                "The active document changed. Reload this page.".into(),
            ));
        }
        Ok(document)
    }

    pub fn open(&self, source: PathBuf, cache: tempfile::TempDir) -> io::Result<Arc<Document>> {
        let selected = source.strip_prefix(&self.root).ok().map(|path| path.to_string_lossy().replace('\\', "/"));
        self.open_named(source, selected, cache)
    }

    pub fn open_named(&self, source: PathBuf, selected_path: Option<String>, cache: tempfile::TempDir) -> io::Result<Arc<Document>> {
        let document = Arc::new(Document {
            projection: self.projection,
            content_token: uuid::Uuid::new_v4().to_string(),
            id: uuid::Uuid::new_v4().to_string(),
            selected_path,
            source: source.clone(),
            directory: cache.path().to_owned(),
            progress: Mutex::new(Progress {
                index: None,
                error: None,
            }),
            renderer: Mutex::new(renderer::Renderer::new()),
            queries: Mutex::new(Default::default()),
            query_work: Arc::new(Semaphore::new(1)),
            jq: Mutex::new(Default::default()),
            assets: Mutex::new(()),
            operations: Default::default(),
            reserved_operations: Default::default(),
            cancelled: Arc::new(AtomicBool::new(false)),
            owner_cancelled: self.owner_cancelled.clone(),
            cache: Mutex::new(Some(cache)),
        });
        let worker = document.clone();
        let index_work = self.index_work.clone();
        let job = thread::Builder::new()
            .name("markdown-index".into())
            .spawn(move || {
                let permit = loop {
                    if worker.is_cancelled() {
                        return;
                    }
                    if let Ok(permit) = index_work.clone().try_acquire_owned() {
                        break permit;
                    }
                    thread::sleep(Duration::from_millis(20));
                };
                index_document(&worker, &source);
                drop(permit);
            })?;
        let old = self.current.lock().unwrap().replace(document.clone());
        let mut jobs = self.jobs.lock().unwrap();
        jobs.retain(|job| !job.is_finished());
        jobs.push(job);
        if let Some(old) = old {
            old.cancel();
            jobs.push(thread::spawn(move || drop(old)));
        }
        Ok(document)
    }

    pub fn cancel(&self) {
        if let Some(document) = self.peek() {
            document.cancel();
        }
    }

    pub fn shutdown(&self) { let _ = self.shutdown_checked(); }

    pub(super) fn shutdown_checked(&self) -> io::Result<()> {
        self.cancel();
        for job in self.jobs.lock().unwrap().drain(..) {
            job.join().map_err(|_| io::Error::other("Document worker panicked during shutdown"))?;
        }
        if let Some(document) = self.current.lock().unwrap().take() {
            // The service drains accepted work first. Explicit cleanup avoids
            // depending on the final request Arc being dropped at an exact instant.
            *document.renderer.lock().unwrap() = renderer::Renderer::new();
            *document.queries.lock().unwrap() = Default::default();
            *document.jq.lock().unwrap() = Default::default();
            if let Some(cache) = document.cache.lock().unwrap().take() { cache.close()?; }
        }
        Ok(())
    }
}

fn index_document(document: &Document, source: &Path) {
    let outcome = (|| -> Result<Index, String> {
        let mut command = Command::new(crate::runtime::process::self_executable().map_err(|error| format!("Cannot locate the document worker: {error}"))?);
        command.arg("--marknoria-index")
            .arg(source)
            .arg(&document.directory)
            .env("MARKNORIA_NATIVE_WORKER_GROUP", "1")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        if document.projection == crate::core::ProjectionProfile::Web { command.arg("--web-profile"); }
        let mut command = process_wrap::std::CommandWrap::from(command);
        #[cfg(unix)]
        command.wrap(process_wrap::std::ProcessGroup::leader());
        #[cfg(windows)]
        {
            let mut flags = process_wrap::std::CreationFlags(Default::default());
            flags.0.0 = 0x08000000; // CREATE_NO_WINDOW for supervised CLI helpers.
            command.wrap(flags).wrap(process_wrap::std::JobObject);
        }
        let mut child = command.spawn()
            .map_err(|error| format!("Cannot start the document worker for {}: {error}", source.display()))?;
        supervise_owned(child.as_mut(), &document.cancelled, document.owner_cancelled.as_deref())?;
        let bytes =
            fs::read(document.directory.join("index.json")).map_err(|error| format!("Cannot read the completed document index: {error}"))?;
        serde_json::from_slice(&bytes).map_err(|error| error.to_string())
    })();
    let mut progress = document.progress.lock().unwrap();
    match outcome {
        Ok(index) => progress.index = Some(Arc::new(index)),
        Err(mut error) => {
            if error.contains("QuotaExceeded") || error.contains("Disk quota exceeded") || error.contains("No space left on device") {
                error.push_str(&format!(". Cache storage exhausted at {}. Choose a disk-backed READER_CACHE_DIR with available quota, then reopen the file.", document.directory.display()));
            }
            if let Some(cache) = document.cache.lock().unwrap().take() {
                if let Err(cleanup) = cache.close() {
                    error.push_str(&format!(". Could not remove partial cache: {cleanup}"));
                }
            }
            progress.error = Some(error);
        }
    }
}

#[derive(Deserialize)]
pub struct PathQuery {
    #[serde(default)]
    pub path: String,
}

pub async fn browse(
    State(app): State<App>,
    Query(query): Query<PathQuery>,
) -> Result<Json<Value>, ApiError> {
    if query.path.len() > 4096 {
        return Err(bad("Path is too long"));
    }
    let _permit = app
        .work
        .clone()
        .try_acquire_owned()
        .map_err(|_| (StatusCode::TOO_MANY_REQUESTS, "Reader is busy".into()))?;
    tokio::task::spawn_blocking(move || {
        let _permit = _permit;
        let root = &app.documents.root;
        let directory = confined_path(root, &query.path).map_err(|_| bad("Folder is outside the server folder or unavailable"))?;
        let mut entries = Vec::new();
        let mut truncated = false;
        for (position, entry) in fs::read_dir(&directory).map_err(failure)?.enumerate() {
            if position >= 10_000 { truncated = true; break; }
            let Ok(entry) = entry else { continue; };
            let name = entry.file_name();
            let Some(name) = name.to_str() else { continue; };
            if name.starts_with('.') { continue; }
            let Ok(path) = entry.path().canonicalize() else { continue; };
            if !path.starts_with(root) { continue; }
            let Ok(metadata) = fs::metadata(&path) else { continue; };
            if !metadata.is_dir() && !(metadata.is_file() && markdown_name(name)) { continue; }
            let relative = path.strip_prefix(root).map_err(failure)?.to_string_lossy().replace('\\', "/");
            entries.push(json!({"name":name,"path":relative,"directory":metadata.is_dir(),"bytes":metadata.len()}));
        }
        entries.sort_by(|left, right| right["directory"].as_bool().cmp(&left["directory"].as_bool()).then_with(|| left["name"].as_str().unwrap().to_lowercase().cmp(&right["name"].as_str().unwrap().to_lowercase())));
        if entries.len() > 1000 { entries.truncate(1000); truncated = true; }
        Ok(Json(json!({"root":root.display().to_string(),"path":directory.strip_prefix(root).map_err(failure)?.to_string_lossy(),"entries":entries,"truncated":truncated})))
    }).await.map_err(failure)?
}

pub async fn open_server(
    State(app): State<App>,
    Query(query): Query<PathQuery>,
) -> Result<Json<Value>, ApiError> {
    if query.path.len() > 4096 {
        return Err(bad("Path is too long"));
    }
    let permit = app
        .open_work
        .clone()
        .try_acquire_owned()
        .map_err(|_| (StatusCode::CONFLICT, "Another file is being opened".into()))?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let path = app.documents.resolve_path(&query.path)
            .map_err(|_| bad("File is outside the server folder or unavailable"))?;
        if !path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(markdown_name)
        {
            return Err(bad("Choose a supported document"));
        }
        let metadata = path.metadata().map_err(failure)?;
        if !metadata.is_file() || metadata.len() > MAX_SOURCE {
            return Err(bad("Choose a regular supported file of at most 128 MiB"));
        }
        let document = app
            .documents
            .open_named(path, Some(query.path), cache_directory().map_err(failure)?)
            .map_err(failure)?;
        app.comparisons.cancel();
        app.editor.clear();
        Ok(Json(json!({"documentId":document.id})))
    })
    .await
    .map_err(failure)?
}

pub struct Options {
    pub source: Option<PathBuf>,
    pub root: PathBuf,
    pub port: u16,
}

pub fn options(arguments: &[String]) -> Result<Options, Box<dyn std::error::Error>> {
    let mut source = None;
    let mut root = None;
    let mut port = None;
    let mut arguments = arguments.iter();
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--root" => {
                root = Some(PathBuf::from(
                    arguments.next().ok_or("--root requires a directory")?,
                ))
            }
            "--port" => {
                port = Some(
                    arguments
                        .next()
                        .ok_or("--port requires a number")?
                        .parse()?,
                )
            }
            value if value.starts_with('-') => {
                return Err(format!("Unknown option: {value}").into());
            }
            value if source.is_none() => source = Some(PathBuf::from(value).canonicalize()?),
            value if port.is_none() => port = Some(value.parse()?),
            _ => {
                return Err(
                    "Usage: marknoria-server [FILE [PORT]] [--port PORT] [--root DIRECTORY]"
                        .into(),
                );
            }
        }
    }
    let root = root
        .or_else(|| {
            source
                .as_ref()
                .and_then(|path| path.parent().map(Path::to_owned))
        })
        .unwrap_or(std::env::current_dir()?)
        .canonicalize()?;
    if !root.is_dir() {
        return Err("--root must be a directory".into());
    }
    Ok(Options {
        source,
        root,
        port: port.unwrap_or(0),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn names_and_server_paths_are_confined() {
        assert!(markdown_name("中文 file.MD"));
        for name in [
            "../../secret.md",
            "/tmp/doc.md",
            "file\\doc.md",
            "script.exe",
            "../doc.md",
        ] {
            assert!(!markdown_name(name));
        }
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().canonicalize().unwrap();
        fs::write(root.as_path().join("file.md"), "# Test").unwrap();
        assert_eq!(
            confined_path(root.as_path(), "file.md").unwrap(),
            root.as_path().join("file.md")
        );
        for path in ["../secret.md", "/etc/passwd", "folder/../file.md"] {
            assert!(confined_path(root.as_path(), path).is_err());
        }
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink("/etc", root.as_path().join("escape")).unwrap();
            assert!(confined_path(root.as_path(), "escape/passwd").is_err());
        }
    }
    #[test]
    fn startup_does_not_require_a_file() {
        let settings = options(&["--port".into(), "38700".into()]).unwrap();
        assert!(settings.source.is_none());
        assert_eq!(settings.port, 38700);
    }
}
