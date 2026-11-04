//! Immutable PDF snapshots; PDFium executes only in a bounded, isolated process.
use axum::{
    body::{Body, Bytes},
    extract::{rejection::JsonRejection, State},
    http::{Response, StatusCode},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{HashMap, VecDeque},
    io::Read,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
const MAX_FILE: usize = 50 * 1024 * 1024;
#[cfg(target_os = "linux")]
const MAX_OUTPUT: usize = 20 * 1024 * 1024;
const MAX_META: usize = 2 * 1024 * 1024;
const MAX_TEXT: usize = 256 * 1024;
const MAGIC: &[u8; 8] = b"STPDF001";
#[cfg(target_os = "linux")]
use std::io::Write;
type Error = (StatusCode, String);
#[cfg(target_os = "linux")]
#[path = "native_pdf_links.rs"]
mod navigation;
fn bad(error: impl ToString) -> Error {
    (StatusCode::BAD_REQUEST, error.to_string())
}
fn missing() -> Error {
    (
        StatusCode::GONE,
        "PDF snapshot expired or closed; reopen the file".into(),
    )
}

#[derive(Deserialize)]
#[serde(tag = "operation", rename_all = "camelCase", deny_unknown_fields)]
pub enum Request {
    Open {
        id: String,
        root: String,
        path: String,
        #[serde(default)]
        password: Option<String>,
    },
    Page {
        id: String,
        page: usize,
    },
    Search {
        id: String,
        query: String,
        from: usize,
        #[serde(default)]
        case_sensitive: bool,
    },
    Select {
        id: String,
        page: usize,
        #[serde(default)]
        points: Option<[[f32; 2]; 2]>,
        #[serde(default)]
        range: Option<[usize; 2]>,
    },
    Close {
        id: String,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Heading {
    title: String,
    page: usize,
    depth: usize,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Page {
    #[serde(default)]
    document_id: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    path: String,
    #[serde(default)]
    hash: String,
    page: usize,
    pages: usize,
    text: String,
    text_limited: bool,
    width: f32,
    height: f32,
    #[serde(default)]
    outline: Vec<Heading>,
    #[serde(default)]
    outline_limited: bool,
    #[serde(default)]
    links: Vec<crate::pdf_navigation::Link>,
    #[serde(default)]
    links_limited: bool,
    #[serde(default)]
    links_unavailable: bool,
}
#[derive(Clone, Serialize, Deserialize)]
struct Work {
    operation: String,
    page: usize,
    query: String,
    case_sensitive: bool,
    password: Option<String>,
    #[serde(default)]
    selection: Option<SelectionInput>,
}
#[derive(Clone, Serialize, Deserialize)]
struct SelectionInput {
    points: Option<[[f32; 2]; 2]>,
    range: Option<[usize; 2]>,
}
impl SelectionInput {
    fn valid(&self) -> bool {
        match (self.points, self.range) {
            (Some(points), None) => points
                .iter()
                .flatten()
                .all(|v| v.is_finite() && (0.0..=1.0).contains(v)),
            (None, Some([a, b])) => a < b && b <= 128000 && b - a <= 16384,
            _ => false,
        }
    }
}
impl Work {
    fn page(page: usize) -> Self {
        Self {
            operation: "page".into(),
            page,
            query: String::new(),
            case_sensitive: false,
            password: None,
            selection: None,
        }
    }
}
struct Source {
    bytes: Arc<[u8]>,
    password: Option<String>,
    name: String,
    path: String,
    hash: String,
    touched: Instant,
    cancel: tokio::sync::watch::Sender<bool>,
}
#[derive(Default)]
struct Cache {
    sources: HashMap<String, Source>,
    pages: VecDeque<((String, usize), Arc<[u8]>)>,
    weight: usize,
    opening: HashMap<String, tokio::sync::watch::Sender<bool>>,
    closed: VecDeque<(String, Instant)>,
}
struct Opening {
    id: String,
    cache: Arc<Mutex<Cache>>,
    cancel: tokio::sync::watch::Sender<bool>,
}
impl Drop for Opening {
    fn drop(&mut self) {
        if let Ok(mut cache) = self.cache.lock() {
            if cache
                .opening
                .get(&self.id)
                .is_some_and(|sender| sender.same_channel(&self.cancel))
            {
                cache.opening.remove(&self.id);
            }
        }
    }
}
fn valid_id(id: &str) -> bool {
    uuid::Uuid::parse_str(id).is_ok_and(|id_value| id_value.to_string() == id)
}
#[derive(Clone)]
pub struct Pdfs {
    cache: Arc<Mutex<Cache>>,
    workers: Arc<tokio::sync::Semaphore>,
    closing: Arc<tokio::sync::Semaphore>,
}
impl Default for Pdfs {
    fn default() -> Self {
        Self {
            cache: Arc::default(),
            workers: Arc::new(tokio::sync::Semaphore::new(2)),
            closing: Arc::new(tokio::sync::Semaphore::new(1)),
        }
    }
}
fn unpack(bytes: &[u8]) -> Result<(Page, &[u8]), String> {
    if bytes.len() < 12 || &bytes[..8] != MAGIC {
        return Err("Invalid PDF worker response".into());
    }
    let size = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    if size > MAX_META || size > bytes.len() - 12 {
        return Err("PDF metadata exceeds its budget".into());
    }
    let page: Page =
        serde_json::from_slice(&bytes[12..12 + size]).map_err(|_| "Invalid PDF page metadata")?;
    if page.pages == 0
        || page.pages > i32::MAX as usize
        || page.page >= page.pages
        || page.text.len() > MAX_TEXT
        || page.text.contains('\0')
        || !page.width.is_finite()
        || !page.height.is_finite()
        || page.width <= 0.0
        || page.height <= 0.0
        || page.outline.len() > 500
        || page.links.len() > crate::pdf_navigation::MAX_LINKS
        || page.links.iter().any(|link| !link.valid(page.pages))
        || page
            .links
            .iter()
            .map(|link| link.rects.len())
            .sum::<usize>()
            > crate::pdf_navigation::MAX_REGIONS
        || page.outline.iter().any(|h| {
            h.page >= page.pages || h.title.len() > 2048 || h.title.contains('\0') || h.depth > 16
        })
    {
        return Err("PDF page exceeds its presentation budget".into());
    }
    let pixels = &bytes[12 + size..];
    if pixels.len() < 24 || &pixels[..8] != b"STIMG001" {
        return Err("Missing PDF raster".into());
    }
    let number =
        |offset| u32::from_le_bytes(pixels[offset..offset + 4].try_into().unwrap()) as usize;
    let (w, h) = (number(16), number(20));
    if w == 0 || h == 0 || w > 2048 || h > 2048 || pixels.len() != 24 + w * h * 4 {
        return Err("PDF raster exceeds its budget".into());
    }
    Ok((page, pixels))
}
fn pack(page: &Page, pixels: &[u8]) -> Result<Vec<u8>, String> {
    let meta = serde_json::to_vec(page).map_err(|e| e.to_string())?;
    if meta.len() > MAX_META {
        return Err("PDF metadata exceeds its budget".into());
    }
    let mut out = Vec::with_capacity(12 + meta.len() + pixels.len());
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&(meta.len() as u32).to_le_bytes());
    out.extend_from_slice(&meta);
    out.extend_from_slice(pixels);
    Ok(out)
}
enum Reply {
    Pixels(Arc<[u8]>),
    Json(Value),
}
impl Pdfs {
    fn opening(&self, id: &str) -> Result<Opening, Error> {
        if !valid_id(id) {
            return Err(bad("Invalid PDF snapshot ID"));
        }
        let mut cache = self.cache.lock().map_err(bad)?;
        cache
            .closed
            .retain(|(_, when)| when.elapsed() < Duration::from_secs(60));
        if cache.closed.iter().any(|(closed, _)| closed == id) {
            return Err(missing());
        }
        if cache.sources.contains_key(id) || cache.opening.contains_key(id) {
            return Err((
                StatusCode::CONFLICT,
                "PDF snapshot ID already exists".into(),
            ));
        }
        let cancel = tokio::sync::watch::channel(false).0;
        cache.opening.insert(id.into(), cancel.clone());
        Ok(Opening {
            id: id.into(),
            cache: self.cache.clone(),
            cancel,
        })
    }
    async fn process(
        &self,
        request: Request,
        shared: Vec<std::path::PathBuf>,
    ) -> Result<Reply, Error> {
        match request {
            Request::Close { id } => {
                if !valid_id(&id) {
                    return Err(bad("Invalid PDF snapshot ID"));
                }
                let (source, removed) = {
                    let mut cache = self.cache.lock().map_err(bad)?;
                    if let Some(opening) = cache.opening.remove(&id) {
                        opening.send_replace(true);
                    }
                    cache.closed.retain(|(closed, when)| {
                        closed != &id && when.elapsed() < Duration::from_secs(60)
                    });
                    while cache.closed.len() >= 128 {
                        cache.closed.pop_front();
                    }
                    cache.closed.push_back((id.clone(), Instant::now()));
                    let source = cache.sources.remove(&id);
                    let mut removed = Vec::new();
                    let mut keep = VecDeque::new();
                    while let Some(entry) = cache.pages.pop_front() {
                        if entry.0 .0 == id {
                            cache.weight -= entry.1.len();
                            removed.push(entry);
                        } else {
                            keep.push_back(entry);
                        }
                    }
                    cache.pages = keep;
                    drop(cache);
                    if let Some(source) = &source {
                        source.cancel.send_replace(true);
                    }
                    (source, removed)
                };
                tokio::task::spawn_blocking(move || drop((source, removed)))
                    .await
                    .map_err(bad)?;
                Ok(Reply::Json(json!({"closed":true})))
            }
            Request::Open {
                id,
                root,
                path,
                password,
            } => {
                let opening = self.opening(&id)?;
                let cancel = opening.cancel.clone();
                if root.len() > 4096
                    || path.len() > 8192
                    || password
                        .as_ref()
                        .is_some_and(|p| p.len() > 512 || p.contains('\0'))
                {
                    return Err(bad("PDF path/password exceeds its budget"));
                }
                let source = tokio::task::spawn_blocking(move || {
                    let (_, path, _, _) = crate::resolve_review_path_with_shared(
                        &crate::ReviewFileRequest { root, path },
                        MAX_FILE as u64,
                        "50 MiB PDF",
                        &shared,
                    )?;
                    if !path
                        .extension()
                        .and_then(|v| v.to_str())
                        .is_some_and(|v| v.eq_ignore_ascii_case("pdf"))
                    {
                        return Err(bad("Choose a PDF file"));
                    }
                    let mut bytes = Vec::new();
                    std::fs::File::open(&path)
                        .map_err(bad)?
                        .take((MAX_FILE + 1) as u64)
                        .read_to_end(&mut bytes)
                        .map_err(bad)?;
                    if bytes.len() > MAX_FILE {
                        return Err((
                            StatusCode::PAYLOAD_TOO_LARGE,
                            "PDF grew beyond 50 MiB while opening".into(),
                        ));
                    }
                    if !bytes.starts_with(b"%PDF-") {
                        return Err(bad("File is not a PDF document"));
                    }
                    Ok(Source {
                        hash: crate::stable_content_hash(&bytes),
                        bytes: bytes.into(),
                        password,
                        name: path
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .into(),
                        path: path.to_string_lossy().into(),
                        touched: Instant::now(),
                        cancel,
                    })
                })
                .await
                .map_err(bad)??;
                let mut work = Work::page(0);
                work.operation = "open".into();
                work.password = source.password.clone();
                let rendered = run(source.bytes.clone(), work, source.cancel.subscribe())
                    .await
                    .map_err(bad)?;
                let cache = self.cache.clone();
                let output = tokio::task::spawn_blocking(move || {
                    let (mut page, pixels) = unpack(&rendered).map_err(bad)?;
                    page.document_id = id.clone();
                    page.name = source.name.clone();
                    page.path = source.path.clone();
                    page.hash = source.hash.clone();
                    let output: Arc<[u8]> = pack(&page, pixels).map_err(bad)?.into();
                    let mut cache = cache.lock().map_err(bad)?;
                    if *source.cancel.borrow() {
                        return Err(missing());
                    }
                    // Expiry is reclaimed on open; full snapshot destruction stays
                    // on this worker and outside the shared cache lock.
                    let expired = cache
                        .sources
                        .iter()
                        .filter(|(_, v)| v.touched.elapsed() > Duration::from_secs(600))
                        .map(|(k, _)| k.clone())
                        .collect::<Vec<_>>();
                    let old = expired
                        .into_iter()
                        .filter_map(|id| cache.sources.remove(&id))
                        .collect::<Vec<_>>();
                    if cache.sources.len() >= 4 {
                        drop(cache);
                        drop(old);
                        return Err((
                            StatusCode::TOO_MANY_REQUESTS,
                            "Close an existing PDF before opening another".into(),
                        ));
                    }
                    cache.sources.insert(id.clone(), source);
                    drop(cache);
                    drop(old);
                    Ok((id, output))
                })
                .await
                .map_err(bad)??;
                self.cache_page(&output.0, 0, output.1.clone()).await?;
                Ok(Reply::Pixels(output.1))
            }
            Request::Page { id, page } => {
                let source = self.source(&id)?;
                let hit = {
                    let mut cache = self.cache.lock().map_err(bad)?;
                    cache
                        .pages
                        .iter()
                        .position(|(key, _)| key == &(id.clone(), page))
                        .map(|position| {
                            let entry = cache.pages.remove(position).unwrap();
                            let data = entry.1.clone();
                            cache.pages.push_back(entry);
                            data
                        })
                };
                if let Some(hit) = hit {
                    return Ok(Reply::Pixels(hit));
                }
                let mut work = Work::page(page);
                work.password = source.1;
                let bytes = run(source.0, work, source.2).await.map_err(bad)?;
                let expected = id.clone();
                let output = tokio::task::spawn_blocking(move || {
                    let (mut meta, pixels) = unpack(&bytes).map_err(bad)?;
                    if meta.page != page {
                        return Err(bad("PDF worker returned another page"));
                    }
                    meta.document_id = expected;
                    Ok::<Arc<[u8]>, Error>(pack(&meta, pixels).map_err(bad)?.into())
                })
                .await
                .map_err(bad)??;
                self.cache_page(&id, page, output.clone()).await?;
                Ok(Reply::Pixels(output))
            }
            Request::Search {
                id,
                query,
                from,
                case_sensitive,
            } => {
                if query.is_empty() || query.len() > 256 || query.contains('\0') {
                    return Err(bad("Enter between 1 and 256 bytes of search text"));
                }
                let (bytes, password, cancel) = self.source(&id)?;
                let work = Work {
                    operation: "search".into(),
                    page: from,
                    query,
                    case_sensitive,
                    password,
                    selection: None,
                };
                let output = run(bytes, work, cancel).await.map_err(bad)?;
                if output.len() > 256 * 1024 {
                    return Err(bad("PDF search exceeded its response budget"));
                }
                let value = tokio::task::spawn_blocking(move || {
                    serde_json::from_slice::<Value>(&output).map_err(bad)
                })
                .await
                .map_err(bad)??;
                Ok(Reply::Json(value))
            }
            Request::Select {
                id,
                page,
                points,
                range,
            } => {
                let selection = SelectionInput { points, range };
                if !selection.valid() {
                    return Err(bad("Invalid PDF text selection"));
                }
                let (bytes, password, cancel) = self.source(&id)?;
                let mut work = Work::page(page);
                work.operation = "select".into();
                work.password = password;
                work.selection = Some(selection);
                let output = run(bytes, work, cancel).await.map_err(bad)?;
                if output.len() > 65536 {
                    return Err(bad("PDF selection exceeded its response budget"));
                }
                let mut value = tokio::task::spawn_blocking(move || {
                    serde_json::from_slice::<Value>(&output).map_err(bad)
                })
                .await
                .map_err(bad)??;
                let cache = self.cache.lock().map_err(bad)?;
                let source = cache.sources.get(&id).ok_or_else(missing)?;
                value["documentId"] = json!(id);
                value["hash"] = json!(source.hash);
                Ok(Reply::Json(value))
            }
        }
    }
    fn source(
        &self,
        id: &str,
    ) -> Result<
        (
            Arc<[u8]>,
            Option<String>,
            tokio::sync::watch::Receiver<bool>,
        ),
        Error,
    > {
        let mut cache = self.cache.lock().map_err(bad)?;
        let source = cache.sources.get_mut(id).ok_or_else(missing)?;
        source.touched = Instant::now();
        Ok((
            source.bytes.clone(),
            source.password.clone(),
            source.cancel.subscribe(),
        ))
    }
    async fn cache_page(&self, id: &str, page: usize, pixels: Arc<[u8]>) -> Result<(), Error> {
        let cache = self.cache.clone();
        let id = id.to_owned();
        tokio::task::spawn_blocking(move || {
            let mut cache = cache.lock().map_err(bad)?;
            if !cache.sources.contains_key(&id) {
                return Err(missing());
            }
            let mut removed = Vec::new();
            while cache.pages.len() >= 16 || cache.weight + pixels.len() > 32 * 1024 * 1024 {
                let Some(old) = cache.pages.pop_front() else {
                    break;
                };
                cache.weight -= old.1.len();
                removed.push(old);
            }
            cache.weight += pixels.len();
            cache.pages.push_back(((id.into(), page), pixels));
            drop(cache);
            drop(removed);
            Ok(())
        })
        .await
        .map_err(bad)?
    }
}
pub async fn handle(
    State(state): State<crate::GatewayState>,
    body: Result<Json<Request>, JsonRejection>,
) -> Response<Body> {
    let request = match body {
        Ok(Json(r)) => r,
        Err(_) => return crate::json_error(StatusCode::BAD_REQUEST, "Invalid native PDF request"),
    };
    let lane = if matches!(request, Request::Close { .. }) {
        state.pdfs.closing.clone()
    } else {
        state.pdfs.workers.clone()
    };
    let _permit = match lane.try_acquire_owned() {
        Ok(p) => p,
        Err(_) => {
            return crate::json_error(
                StatusCode::TOO_MANY_REQUESTS,
                "PDF workers are busy; try again",
            )
        }
    };
    let shared = if matches!(request, Request::Open { .. }) {
        let state = state.clone();
        match tokio::task::spawn_blocking(move || crate::shared_document_directories(&state)).await
        {
            Ok(Ok(d)) => d,
            Ok(Err((s, e))) => return crate::json_error(s, &e),
            Err(_) => {
                return crate::json_error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "PDF settings worker stopped",
                )
            }
        }
    } else {
        Vec::new()
    };
    match state.pdfs.process(request, shared).await {
        Ok(Reply::Pixels(p)) => Response::builder()
            .header("content-type", "application/x-studio-pdf")
            .header("cache-control", "no-store")
            .header("x-content-type-options", "nosniff")
            .body(Body::from(Bytes::from_owner(p)))
            .unwrap(),
        Ok(Reply::Json(v)) => crate::json_response(StatusCode::OK, &v),
        Err((s, e)) => crate::json_error(s, &e),
    }
}

#[cfg(target_os = "linux")]
async fn run(
    bytes: Arc<[u8]>,
    work: Work,
    mut cancel: tokio::sync::watch::Receiver<bool>,
) -> Result<Vec<u8>, String> {
    if *cancel.borrow() {
        return Err("PDF snapshot closed".into());
    }
    use std::process::Stdio;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let executable = std::env::current_exe().map_err(|e| e.to_string())?;
    let library = std::env::var_os("STUDIO_GNOME_PDFIUM_LIBRARY")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| executable.with_file_name("libpdfium.so"))
        .canonicalize()
        .map_err(|_| "PDFium is missing; run scripts/install-gnome-pdf.sh for the native bundle")?;
    let mut command = tokio::process::Command::new("bwrap");
    command.args([
        "--unshare-all",
        "--die-with-parent",
        "--new-session",
        "--clearenv",
        "--setenv",
        "LANG",
        "C.UTF-8",
        "--setenv",
        "HOME",
        "/nonexistent",
        "--ro-bind",
        "/usr",
        "/usr",
        "--symlink",
        "usr/lib",
        "/lib",
        "--symlink",
        "usr/lib64",
        "/lib64",
        "--proc",
        "/proc",
        "--dev",
        "/dev",
        "--size",
        "1048576",
        "--tmpfs",
        "/tmp",
        "--dir",
        "/app",
        "--chdir",
        "/tmp",
    ]);
    if std::path::Path::new("/etc/fonts").exists() {
        command.args(["--ro-bind", "/etc/fonts", "/etc/fonts"]);
    }
    command
        .arg("--ro-bind")
        .arg(executable)
        .arg("/app/worker")
        .arg("--ro-bind")
        .arg(library)
        .arg("/app/libpdfium.so")
        .args(["--", "/app/worker", "--native-pdf-worker"]);
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    unsafe {
        command.pre_exec(|| {
            for (resource, value) in [
                (libc::RLIMIT_AS, 768 * 1024 * 1024),
                (libc::RLIMIT_CPU, 12),
                (libc::RLIMIT_NOFILE, 64),
                (libc::RLIMIT_CORE, 0),
                (libc::RLIMIT_FSIZE, 1024 * 1024),
            ] {
                let limit = libc::rlimit {
                    rlim_cur: value,
                    rlim_max: value,
                };
                if libc::setrlimit(resource, &limit) != 0 {
                    return Err(std::io::Error::last_os_error());
                }
            }
            Ok(())
        });
    }
    let mut child = command
        .spawn()
        .map_err(|e| format!("Unable to start isolated PDF worker: {e}"))?;
    let mut input = child.stdin.take().unwrap();
    let output = child.stdout.take().unwrap();
    let header = serde_json::to_vec(&work).map_err(|e| e.to_string())?;
    let operation = async {
        let send = async move {
            input
                .write_all(&(header.len() as u32).to_le_bytes())
                .await?;
            input.write_all(&header).await?;
            input.write_all(&bytes).await?;
            input.shutdown().await?;
            drop(input);
            Ok::<_, std::io::Error>(())
        };
        let read = async {
            let mut data = Vec::new();
            output
                .take((MAX_OUTPUT + 1) as u64)
                .read_to_end(&mut data)
                .await?;
            Ok::<_, std::io::Error>(data)
        };
        let (sent, read) = tokio::join!(send, read);
        sent.map_err(|e| e.to_string())?;
        let data = read.map_err(|e| e.to_string())?;
        if data.len() > MAX_OUTPUT {
            return Err("PDF worker exceeded its response budget".into());
        }
        let status = child.wait().await.map_err(|e| e.to_string())?;
        if !status.success() {
            return Err("PDF worker stopped or exceeded its resource limit".into());
        }
        if data.starts_with(b"{\"error\"") {
            if data.len() > 8192 {
                return Err("PDF worker error exceeded its response budget".into());
            }
            let value: Value = serde_json::from_slice(&data).map_err(|e| e.to_string())?;
            return Err(value["error"].as_str().unwrap_or("Cannot read PDF").into());
        }
        Ok(data)
    };
    let outcome = tokio::select! {result=tokio::time::timeout(Duration::from_secs(20), operation)=>result,_=cancel.changed()=>Ok(Err("PDF snapshot closed".into()))};
    match outcome {
        Ok(Ok(data)) => Ok(data),
        other => {
            let _ = child.kill().await;
            let _ = child.wait().await;
            match other {
                Ok(Err(e)) => Err(e),
                _ => Err("PDF worker exceeded its 20 second deadline".into()),
            }
        }
    }
}
#[cfg(not(target_os = "linux"))]
async fn run(
    _: Arc<[u8]>,
    _: Work,
    _: tokio::sync::watch::Receiver<bool>,
) -> Result<Vec<u8>, String> {
    Err("Native PDF workers require Linux".into())
}

pub fn worker() -> Option<Result<(), String>> {
    if std::env::args().nth(1).as_deref() != Some("--native-pdf-worker") {
        return None;
    }
    #[cfg(target_os = "linux")]
    let result = worker_linux();
    #[cfg(not(target_os = "linux"))]
    let result = Err("Native PDF workers require Linux".into());
    Some(result)
}
#[cfg(target_os = "linux")]
fn worker_linux() -> Result<(), String> {
    use pdfium_render::prelude::*;
    let mut input = std::io::stdin().lock();
    let mut size = [0; 4];
    input.read_exact(&mut size).map_err(|e| e.to_string())?;
    let size = u32::from_le_bytes(size) as usize;
    if size > 8192 {
        return Err("PDF worker request too large".into());
    }
    let mut header = vec![0; size];
    input.read_exact(&mut header).map_err(|e| e.to_string())?;
    let work: Work = serde_json::from_slice(&header).map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    input
        .take((MAX_FILE + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > MAX_FILE {
        return Err("PDF worker input too large".into());
    }
    let operation = || -> Result<Vec<u8>, String> {
        let bindings = Pdfium::bind_to_library("/app/libpdfium.so")
            .map_err(|e| format!("PDFium library: {e}"))?;
        let pdfium = Pdfium::new(bindings);
        let document = pdfium
            .load_pdf_from_byte_slice(&bytes, work.password.as_deref())
            .map_err(|e| {
                if format!("{e:?}").contains("PasswordError") {
                    "PDF password required or incorrect".into()
                } else {
                    format!("Cannot read PDF: {e}")
                }
            })?;
        let pages = document.pages().len() as usize;
        if pages == 0 || pages > i32::MAX as usize || work.page >= pages {
            return Err("PDF page is outside the document".into());
        }
        fn text(page: &PdfPage<'_>) -> Result<(String, bool), String> {
            let text = page.text().map_err(|e| e.to_string())?;
            if text.len() > 128000 {
                return Ok((String::new(), true));
            }
            let mut value = text.all().replace('\0', "�");
            let limited = value.len() > MAX_TEXT;
            if limited {
                let mut end = MAX_TEXT;
                while !value.is_char_boundary(end) {
                    end -= 1;
                }
                value.truncate(end);
            }
            Ok((value, limited))
        }
        if work.operation == "search" {
            let regex = regex::RegexBuilder::new(&regex::escape(&work.query))
                .case_insensitive(!work.case_sensitive)
                .build()
                .map_err(|e| e.to_string())?;
            let mut hits = Vec::new();
            let mut next = work.page;
            let mut partial = false;
            for index in work.page..(work.page + 32).min(pages) {
                let page = document
                    .pages()
                    .get(index as i32)
                    .map_err(|e| e.to_string())?;
                let (value, limited) = text(&page)?;
                partial |= limited;
                for found in regex.find_iter(&value) {
                    let mut a = found.start().saturating_sub(60);
                    while !value.is_char_boundary(a) {
                        a += 1;
                    }
                    let mut b = (found.end() + 100).min(value.len());
                    while !value.is_char_boundary(b) {
                        b -= 1;
                    }
                    let snippet = format!(
                        "{}{}{}",
                        if a > 0 { "…" } else { "" },
                        &value[a..b],
                        if b < value.len() { "…" } else { "" }
                    );
                    hits.push(json!({"page":index,"snippet":snippet}));
                    if hits.len() >= 100 {
                        partial = true;
                        break;
                    }
                }
                next = index + 1;
                if hits.len() >= 100 {
                    break;
                }
            }
            return serde_json::to_vec(
                &json!({"hits":hits,"next":if next<pages{Some(next)}else{None},"partial":partial}),
            )
            .map_err(|e| e.to_string());
        }
        let page = document
            .pages()
            .get(work.page as i32)
            .map_err(|e| e.to_string())?;
        let width = page.width().value;
        let height = page.height().value;
        if !width.is_finite()
            || !height.is_finite()
            || width <= 0.0
            || height <= 0.0
            || width > 100000.0
            || height > 100000.0
        {
            return Err("Invalid PDF page dimensions".into());
        }
        if work.operation == "select" {
            let input = work
                .selection
                .as_ref()
                .filter(|s| s.valid())
                .ok_or("Invalid PDF text selection")?;
            let text = page.text().map_err(|e| e.to_string())?;
            if text.len() > 128000 {
                return Err("Page text exceeds the selection limit".into());
            }
            let chars = text.chars();
            // Device conversion uses the same PDFium rotation rules as rendering.
            // A square virtual viewport represents normalized coordinates exactly.
            let config = PdfRenderConfig::new().set_target_size(10000, 10000);
            let [a, b] = if let Some([from, to]) = input.points {
                let at = |point: [f32; 2]| -> Result<usize, String> {
                    let (x, y) = page
                        .pixels_to_points(
                            (point[0] * 10000.0).round() as i32,
                            (point[1] * 10000.0).round() as i32,
                            &config,
                        )
                        .map_err(|e| e.to_string())?;
                    chars
                        .get_char_near_point(x, PdfPoints::new(6.0), y, PdfPoints::new(6.0))
                        .map(|c| c.index())
                        .ok_or("Start and finish the selection on text".into())
                };
                let (from, to) = (at(from)?, at(to)?);
                [from.min(to), from.max(to) + 1]
            } else {
                input.range.unwrap()
            };
            if a >= b || b > chars.len() || b - a > 16384 {
                return Err("Select a shorter passage".into());
            }
            let mut units = Vec::with_capacity(b - a);
            for index in a..b {
                let value = chars.get(index).map_err(|e| e.to_string())?.unicode_value();
                if let Some(c) = char::from_u32(value) {
                    units.extend_from_slice(c.encode_utf16(&mut [0; 2]));
                } else if value <= 65535 {
                    units.push(value as u16);
                } else {
                    units.push(0xfffd);
                }
            }
            let excerpt = String::from_utf16_lossy(&units).replace('\0', "�");
            if excerpt.trim().is_empty() || excerpt.len() > 16384 {
                return Err("Select a passage of at most 16 KiB".into());
            }
            let segments = text.segments_subset(a, b - a);
            if segments.len() > 128 {
                return Err("Select a passage with at most 128 text regions".into());
            }
            let mut rects = Vec::new();
            for index in 0..segments.len() {
                let rect = segments.get(index).map_err(|e| e.to_string())?.bounds();
                let (x1, y1) = page
                    .points_to_pixels(rect.left(), rect.top(), &config)
                    .map_err(|e| e.to_string())?;
                let (x2, y2) = page
                    .points_to_pixels(rect.right(), rect.bottom(), &config)
                    .map_err(|e| e.to_string())?;
                let (left, top, right, bottom) = (
                    x1.min(x2).clamp(0, 10000),
                    y1.min(y2).clamp(0, 10000),
                    x1.max(x2).clamp(0, 10000),
                    y1.max(y2).clamp(0, 10000),
                );
                if left < right && top < bottom {
                    rects.push(json!({"x":left as f32/10000.0,"y":top as f32/10000.0,"width":(right-left) as f32/10000.0,"height":(bottom-top) as f32/10000.0}));
                }
            }
            return serde_json::to_vec(
                &json!({"page":work.page,"excerpt":excerpt,"range":[a,b],"rects":rects}),
            )
            .map_err(|e| e.to_string());
        }
        let (text, text_limited) = text(&page)?;
        let (links, links_limited, links_unavailable) = navigation::navigation_links(&page, pages);
        let config = PdfRenderConfig::new()
            .set_target_width(1536)
            .set_maximum_width(2048)
            .set_maximum_height(2048)
            .set_format(PdfBitmapFormat::BGRA)
            .render_annotations(true)
            .set_clear_color(PdfColor::WHITE);
        let bitmap = page
            .render_with_config(&config)
            .map_err(|e| e.to_string())?;
        let (w, h) = (bitmap.width() as u32, bitmap.height() as u32);
        if w == 0 || h == 0 || w > 2048 || h > 2048 {
            return Err("PDF page raster exceeds its budget".into());
        }
        let mut pixels = bitmap.as_rgba_bytes();
        if pixels.len() != w as usize * h as usize * 4 {
            return Err("Invalid PDF bitmap stride".into());
        }
        for pixel in pixels.chunks_exact_mut(4) {
            let alpha = u16::from(pixel[3]);
            for channel in &mut pixel[..3] {
                *channel = ((u16::from(*channel) * alpha + 127) / 255) as u8;
            }
        }
        let mut raster = b"STIMG001".to_vec();
        for value in [width.ceil() as u32, height.ceil() as u32, w, h] {
            raster.extend_from_slice(&value.to_le_bytes());
        }
        raster.extend_from_slice(&pixels);
        let mut outline = Vec::new();
        let mut outline_limited = false;
        if work.operation == "open" {
            for bookmark in document.bookmarks().iter().take(501) {
                if outline.len() >= 500 {
                    outline_limited = true;
                    break;
                }
                if let Some(index) = bookmark
                    .destination()
                    .and_then(|d| d.page_index().ok())
                    .and_then(|index| usize::try_from(index).ok())
                    .filter(|p| *p < pages)
                {
                    let mut depth = 0;
                    let mut parent = bookmark.parent();
                    while let Some(current) = parent {
                        if depth >= 16 {
                            break;
                        }
                        depth += 1;
                        parent = current.parent();
                    }
                    outline.push(Heading {
                        title: bookmark
                            .title()
                            .unwrap_or_default()
                            .chars()
                            .filter(|c| !c.is_control())
                            .take(256)
                            .collect(),
                        page: index,
                        depth,
                    });
                }
            }
        }
        pack(
            &Page {
                document_id: String::new(),
                name: String::new(),
                path: String::new(),
                hash: String::new(),
                page: work.page,
                pages,
                text,
                text_limited,
                width,
                height,
                outline,
                outline_limited,
                links,
                links_limited,
                links_unavailable,
            },
            &raster,
        )
    };
    let data = match operation() {
        Ok(v) => v,
        Err(error) => serde_json::to_vec(&json!({"error":error})).map_err(|e| e.to_string())?,
    };
    std::io::stdout()
        .lock()
        .write_all(&data)
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn close_cancels_pending_open_and_reserves_its_identity_against_late_arrival() {
        let pdfs = Pdfs::default();
        let id = uuid::Uuid::new_v4().to_string();
        let opening = pdfs.opening(&id).unwrap();
        let receiver = opening.cancel.subscribe();
        pdfs.process(Request::Close { id: id.clone() }, Vec::new())
            .await
            .unwrap();
        assert!(*receiver.borrow());
        assert!(matches!(pdfs.opening(&id), Err((StatusCode::GONE, _))));
        assert!(pdfs.cache.lock().unwrap().opening.is_empty());
        let fresh = uuid::Uuid::new_v4().to_string();
        let next = pdfs.opening(&fresh).unwrap();
        drop(opening);
        assert!(pdfs.cache.lock().unwrap().opening.contains_key(&fresh));
        drop(next);
        assert!(pdfs.cache.lock().unwrap().opening.is_empty());
        // A close can reach the service before its older open request.
        let late = uuid::Uuid::new_v4().to_string();
        pdfs.process(Request::Close { id: late.clone() }, Vec::new())
            .await
            .unwrap();
        assert!(matches!(pdfs.opening(&late), Err((StatusCode::GONE, _))));
    }
    #[test]
    fn pixel_protocol_rejects_truncated_or_mismatched_pdf_rasters() {
        let page = Page {
            document_id: "test".into(),
            name: String::new(),
            path: String::new(),
            hash: String::new(),
            page: 0,
            pages: 1,
            text: "Unicode 中文".into(),
            text_limited: false,
            width: 1.0,
            height: 1.0,
            outline: Vec::new(),
            outline_limited: false,
            links: Vec::new(),
            links_limited: false,
            links_unavailable: false,
        };
        let mut image = b"STIMG001".to_vec();
        for size in [1u32, 1, 1, 1] {
            image.extend_from_slice(&size.to_le_bytes());
        }
        image.extend_from_slice(&[255; 4]);
        let mut data = pack(&page, &image).unwrap();
        assert_eq!(unpack(&data).unwrap().0.text, "Unicode 中文");
        data.pop();
        assert!(unpack(&data).is_err());
        assert!(unpack(b"STPDF001").is_err());
    }
}
