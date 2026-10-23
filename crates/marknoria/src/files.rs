pub(super) use super::worker::supervise;
use super::{ApiError, App, Progress, failure, renderer};
use crate::{Index, MAX_SOURCE};
use axum::{
    Json,
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    fs, io,
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
    pub id: String,
    pub source: PathBuf,
    pub directory: PathBuf,
    pub progress: Mutex<Progress>,
    pub renderer: Mutex<renderer::Renderer>,
    pub cancelled: Arc<AtomicBool>,
    cache: Mutex<Option<tempfile::TempDir>>,
}

pub struct Documents {
    pub root: PathBuf,
    current: Mutex<Option<Arc<Document>>>,
    jobs: Mutex<Vec<JoinHandle<()>>>,
    index_work: Arc<Semaphore>,
}

fn bad(message: &str) -> ApiError {
    (StatusCode::BAD_REQUEST, message.into())
}

pub fn markdown_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 255
        && !name.contains(['/', '\\', '\0'])
        && Path::new(name)
            .extension()
            .and_then(|value| value.to_str())
            .is_some_and(|value| {
                matches!(
                    value.to_ascii_lowercase().as_str(),
                    "md" | "markdown" | "mdown" | "mkd"
                )
            })
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

impl Documents {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            current: Mutex::new(None),
            jobs: Mutex::new(Vec::new()),
            index_work: {
                static WORK: std::sync::OnceLock<Arc<Semaphore>> = std::sync::OnceLock::new();
                WORK.get_or_init(|| Arc::new(Semaphore::new(1))).clone()
            },
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
        let document = Arc::new(Document {
            id: uuid::Uuid::new_v4().to_string(),
            source: source.clone(),
            directory: cache.path().to_owned(),
            progress: Mutex::new(Progress {
                index: None,
                error: None,
            }),
            renderer: Mutex::new(renderer::Renderer::new()),
            cancelled: Arc::new(AtomicBool::new(false)),
            cache: Mutex::new(Some(cache)),
        });
        let worker = document.clone();
        let index_work = self.index_work.clone();
        let job = thread::Builder::new()
            .name("markdown-index".into())
            .spawn(move || {
                let permit = loop {
                    if worker.cancelled.load(Ordering::Relaxed) {
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
            old.cancelled.store(true, Ordering::Relaxed);
            jobs.push(thread::spawn(move || drop(old)));
        }
        Ok(document)
    }

    pub fn cancel(&self) {
        if let Some(document) = self.peek() {
            document.cancelled.store(true, Ordering::Relaxed);
        }
    }

    pub fn shutdown(&self) {
        self.cancel();
        for job in self.jobs.lock().unwrap().drain(..) {
            let _ = job.join();
        }
        self.current.lock().unwrap().take();
    }
}

fn index_document(document: &Document, source: &Path) {
    let outcome = (|| -> Result<Index, String> {
        let mut child = Command::new(std::env::current_exe().map_err(|error| error.to_string())?)
            .arg("--marknoria-index")
            .arg(source)
            .arg(&document.directory)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| error.to_string())?;
        supervise(&mut child, &document.cancelled)?;
        let bytes =
            fs::read(document.directory.join("index.json")).map_err(|error| error.to_string())?;
        serde_json::from_slice(&bytes).map_err(|error| error.to_string())
    })();
    let mut progress = document.progress.lock().unwrap();
    match outcome {
        Ok(index) => progress.index = Some(Arc::new(index)),
        Err(mut error) => {
            if error.contains("QuotaExceeded")
                || error.contains("Disk quota exceeded")
                || error.contains("No space left on device")
            {
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

pub fn cache_directory() -> io::Result<tempfile::TempDir> {
    temporary_directory("markdown-reader-")
}

fn storage_root(
    configured: Option<PathBuf>,
    temporary: Option<PathBuf>,
    xdg: Option<PathBuf>,
    home: Option<PathBuf>,
) -> io::Result<PathBuf> {
    if let Some(path) = configured.or(temporary) {
        return Ok(path);
    }
    if let Some(path) = xdg.filter(|path| path.is_absolute()) {
        return Ok(path.join("markdown-reader"));
    }
    if let Some(path) = home.filter(|path| path.is_absolute()) {
        return Ok(path.join(".cache/markdown-reader"));
    }
    Err(io::Error::other(
        "Set READER_CACHE_DIR to a disk-backed directory; HOME/XDG_CACHE_HOME are unavailable",
    ))
}

pub fn temporary_directory(prefix: &str) -> io::Result<tempfile::TempDir> {
    let variable = |name| {
        std::env::var_os(name)
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
    };
    let cache_home = variable("XDG_CACHE_HOME").or_else(|| {
        if cfg!(windows) {
            variable("LOCALAPPDATA")
        } else {
            None
        }
    });
    let home = variable("HOME").or_else(|| variable("USERPROFILE"));
    let root = storage_root(
        variable("READER_CACHE_DIR"),
        variable("TMPDIR"),
        cache_home,
        home,
    )?;
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(&root)?;
    tempfile::Builder::new()
        .prefix(prefix)
        .tempdir_in(&root)
        .map_err(|error| io::Error::new(error.kind(), format!("Cannot create reader cache in {}: {error}. Set READER_CACHE_DIR to a disk-backed directory with available quota.", root.display())))
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
        let path = confined_path(&app.documents.root, &query.path)
            .map_err(|_| bad("File is outside the server folder or unavailable"))?;
        if !path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(markdown_name)
        {
            return Err(bad("Choose a Markdown file"));
        }
        let metadata = path.metadata().map_err(failure)?;
        if !metadata.is_file() || metadata.len() > MAX_SOURCE {
            return Err(bad("Choose a regular Markdown file of at most 128 MiB"));
        }
        let document = app
            .documents
            .open(path, cache_directory().map_err(failure)?)
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
                    "Usage: marknoria-server [FILE [PORT]] [--port PORT] [--root DIRECTORY]".into(),
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
    fn cache_storage_prefers_explicit_paths_and_defaults_outside_tmp() {
        let path = |value: &str| Some(PathBuf::from(value));
        assert_eq!(
            storage_root(
                path("/explicit"),
                path("/temporary"),
                path("/cache"),
                path("/home/user")
            )
            .unwrap(),
            PathBuf::from("/explicit")
        );
        assert_eq!(
            storage_root(None, path("/temporary"), path("/cache"), path("/home/user")).unwrap(),
            PathBuf::from("/temporary")
        );
        assert_eq!(
            storage_root(None, None, path("/cache"), path("/home/user")).unwrap(),
            PathBuf::from("/cache/markdown-reader")
        );
        assert_eq!(
            storage_root(None, None, path("relative"), path("/home/user")).unwrap(),
            PathBuf::from("/home/user/.cache/markdown-reader")
        );
        assert!(storage_root(None, None, None, None).is_err());
    }
    #[test]
    fn names_and_server_paths_are_confined() {
        assert!(markdown_name("中文 file.MD"));
        for name in [
            "../../secret.md",
            "/tmp/doc.md",
            "file\\doc.md",
            "script.html",
            "README",
            "../doc.md",
        ] {
            assert!(!markdown_name(name));
        }
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("file.md"), "# Test").unwrap();
        assert_eq!(
            confined_path(root.path(), "file.md").unwrap(),
            root.path().join("file.md")
        );
        for path in ["../secret.md", "/etc/passwd", "folder/../file.md"] {
            assert!(confined_path(root.path(), path).is_err());
        }
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink("/etc", root.path().join("escape")).unwrap();
            assert!(confined_path(root.path(), "escape/passwd").is_err());
        }
    }
    #[test]
    fn startup_does_not_require_a_file() {
        let settings = options(&["--port".into(), "38700".into()]).unwrap();
        assert!(settings.source.is_none());
        assert_eq!(settings.port, 38700);
    }
}
