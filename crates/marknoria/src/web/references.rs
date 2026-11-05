//! Explicit host folder grants and a small supervised image-snapshot cache.
use super::*;
use crate::core::references::{self, Kind};
use std::{
    collections::HashMap,
    fs::File,
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::Mutex,
    time::Instant,
};

const IMAGES: usize = 4;
const IMAGE_BYTES: u64 = 25 * 1024 * 1024;
pub(super) const MAX_ROOTS: usize = 257;
/// One explicitly authorized host image. `bytes:None` is valid only when the
/// host revalidated the supplied cache identity and that snapshot still exists.
/// Identity is a host cache key, not a claim of exact content-hash precision.
pub struct HostImage {
    pub name: String,
    pub identity: String,
    pub bytes: Option<Vec<u8>>,
}
#[derive(Clone)]
pub(super) struct ImageLoader(Arc<dyn Fn(&str, Option<&str>, u64, &AtomicBool) -> io::Result<HostImage> + Send + Sync>);
impl std::fmt::Debug for ImageLoader {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { formatter.write_str("HostImageLoader") }
}
impl ImageLoader {
    pub fn new(loader: impl Fn(&str, Option<&str>, u64, &AtomicBool) -> io::Result<HostImage> + Send + Sync + 'static) -> Self { Self(Arc::new(loader)) }
}
#[derive(Clone)]
pub(super) struct RootProvider(Arc<dyn Fn() -> io::Result<Vec<PathBuf>> + Send + Sync>);
impl std::fmt::Debug for RootProvider {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("ReferenceRootProvider")
    }
}
impl RootProvider {
    pub fn new(provider: impl Fn() -> io::Result<Vec<PathBuf>> + Send + Sync + 'static) -> Self {
        Self(Arc::new(provider))
    }
}
struct Entry {
    documents: files::Documents,
    source: Option<PathBuf>,
    host_identity: Option<String>,
    _input: Option<tempfile::TempDir>,
    touched: Instant,
}
pub(super) struct Resources {
    roots: Vec<PathBuf>,
    provider: Option<RootProvider>,
    image_loader: Option<ImageLoader>,
    entries: Mutex<HashMap<String, Entry>>,
}
fn requested_in_roots(requested: &Path, roots: &[PathBuf]) -> bool {
    roots.iter().any(|root| {
        #[cfg(windows)]
        {
            // URL paths omit Windows' verbatim prefix. Compare like forms,
            // before any filesystem access that could initiate a UNC request.
            url::Url::from_file_path(root).ok().and_then(|url| url.to_file_path().ok())
                .is_some_and(|root| requested.to_string_lossy().to_lowercase().starts_with(
                    &(root.to_string_lossy().trim_end_matches(['\\', '/'])
                        .to_lowercase() + "\\"),
                ))
        }
        #[cfg(not(windows))]
        {
            requested.starts_with(root)
        }
    })
}
impl Resources {
    pub fn new(roots: Vec<PathBuf>) -> Self {
        Self {
            roots,
            provider: None,
            image_loader: None,
            entries: Mutex::new(HashMap::new()),
        }
    }
    pub fn with_provider(mut self, provider: Option<RootProvider>) -> Self {
        self.provider = provider;
        self
    }
    pub fn with_image_loader(mut self, loader: Option<ImageLoader>) -> Self {
        self.image_loader = loader;
        self
    }
    pub fn host_images(&self) -> bool { self.image_loader.is_some() }
    fn current_roots(&self) -> io::Result<Vec<PathBuf>> {
        let mut roots = self.roots.clone();
        if let Some(provider) = &self.provider {
            for root in (provider.0)()? {
                let root = root.canonicalize()?;
                if !root.is_dir() {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "Reference roots must be directories",
                    ));
                }
                if !roots.contains(&root) {
                    roots.push(root);
                }
                if roots.len() > MAX_ROOTS {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "Reference roots exceed 257 distinct directories",
                    ));
                }
            }
        }
        Ok(roots)
    }
    fn local(&self, source: &Path, target: &str) -> io::Result<PathBuf> {
        // One host-policy snapshot per resolution, also on image-cache hits.
        let roots = self.current_roots()?;
        let denied = || {
            io::Error::new(
                io::ErrorKind::PermissionDenied,
                "Reference is outside explicitly authorized folders",
            )
        };
        if roots.is_empty() {
            return Err(denied());
        }
        if target.len() > references::MAX_LINK_BYTES || target.chars().any(char::is_control) {
            return Err(denied());
        }
        let normalized = target.replace('\\', "/");
        let (base, target) = if normalized.starts_with('/') && !normalized.starts_with("//") {
            let root = roots
                .iter()
                .find(|root| source.starts_with(root))
                .ok_or_else(denied)?;
            (
                url::Url::from_directory_path(root).map_err(|_| denied())?,
                normalized.trim_start_matches('/'),
            )
        } else {
            (
                url::Url::from_file_path(source).map_err(|_| denied())?,
                normalized.as_str(),
            )
        };
        let target = base.join(target).map_err(|_| denied())?;
        if target.scheme() != "file" {
            return Err(denied());
        }
        let requested = target.to_file_path().map_err(|_| denied())?;
        if requested
            .components()
            .any(|part| matches!(part, std::path::Component::ParentDir))
        {
            return Err(denied());
        }
        // Unix directory aliases (macOS /var -> /private/var) do not resolve
        // the final file symlink. On Windows preserve the original lexical
        // authorization before filesystem access, including UNC/SMB paths.
        #[cfg(not(windows))]
        let requested = requested
            .parent().ok_or_else(denied)?.canonicalize().map_err(|_| denied())?
            .join(requested.file_name().ok_or_else(denied)?);
        if !requested_in_roots(&requested, &roots) {
            return Err(denied());
        }
        let path = requested.canonicalize()?;
        if !roots.iter().any(|root| path.starts_with(root)) || !path.is_file() {
            return Err(denied());
        }
        Ok(path)
    }
    fn image(
        &self,
        parent: &files::Document,
        index: &Index,
        id: usize,
    ) -> Result<Arc<files::Document>, ApiError> {
        if parent.is_cancelled() {
            return Err((StatusCode::GONE, "Owning document was closed".into()));
        }
        if index
            .references
            .get(id)
            .is_none_or(|reference| reference.kind != Kind::Image)
        {
            return Err((StatusCode::BAD_REQUEST, "Reference is not an image".into()));
        }
        let inline = index.references[id].inline;
        let key = format!("{}:{}:{id}", parent.id, index.revision);
        // Authorization precedes every metadata/frame cache lookup. Only the
        // trusted host receives the previous key; markup cannot supply a key.
        let host_image = if !inline && let Some(loader) = &self.image_loader {
            let hint = self.entries.lock().map_err(failure)?.get(&key).and_then(|entry| entry.host_identity.clone());
            let target = references::target(&parent.directory, index, id).map_err(failure)?;
            let image = (loader.0)(&target, hint.as_deref(), IMAGE_BYTES, &parent.cancelled)
                .map_err(|error| (StatusCode::FORBIDDEN, error.to_string()))?;
            if image.identity.is_empty() || image.identity.len() > 256 || image.identity.chars().any(char::is_control)
                || image.name.is_empty() || image.name.len() > 255 || image.name.contains(['/', '\\'])
                || image.name.chars().any(char::is_control)
                || crate::core::document_format::DocumentFormat::for_name(&image.name) != Some(crate::core::document_format::DocumentFormat::Image) {
                return Err((StatusCode::BAD_REQUEST, "Invalid host image identity or basename".into()));
            }
            if image.bytes.as_ref().is_some_and(|bytes| bytes.len() as u64 > IMAGE_BYTES) {
                return Err((StatusCode::PAYLOAD_TOO_LARGE, "Host image exceeds 25 MiB".into()));
            }
            if image.bytes.is_none() && hint.as_deref() != Some(image.identity.as_str()) {
                return Err((StatusCode::BAD_REQUEST, "Uncached host image requires a snapshot".into()));
            }
            Some(image)
        } else { None };
        let host_identity = host_image.as_ref().map(|image| image.identity.clone());
        if parent.is_cancelled() { return Err((StatusCode::GONE, "Owning document was closed".into())); }
        let source = if inline || host_image.is_some() {
            None
        } else {
            let target = references::target(&parent.directory, index, id).map_err(failure)?;
            Some(
                self.local(&parent.source, &target)
                    .map_err(|error| (StatusCode::FORBIDDEN, error.to_string()))?,
            )
        };
        let mut entries = self.entries.lock().map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Reference cache unavailable".into(),
            )
        })?;
        if entries
            .get(&key)
            .is_some_and(|entry| entry.source != source || entry.host_identity != host_identity)
        {
            if entries[&key]
                .documents
                .peek()
                .is_some_and(|document| Arc::strong_count(&document) > 2)
            {
                return Err((
                    StatusCode::TOO_MANY_REQUESTS,
                    "Previous image snapshot is still in use".into(),
                ));
            }
            if let Some(entry) = entries.remove(&key) {
                entry.documents.shutdown();
            }
        }
        if let Some(entry) = entries.get_mut(&key) {
            entry.touched = Instant::now();
            return entry
                .documents
                .peek()
                .ok_or((StatusCode::GONE, "Image snapshot was closed".into()));
        }
        if entries.len() >= IMAGES {
            let victim = entries
                .iter()
                .filter(|(_, entry)| {
                    entry.documents.peek().is_some_and(|document| {
                        let progress = document.progress.lock().unwrap();
                        Arc::strong_count(&document) <= 2
                            && (document.is_cancelled()
                                || progress.index.is_some()
                                || progress.error.is_some())
                    })
                })
                .min_by_key(|(_, entry)| entry.touched)
                .map(|(key, _)| key.clone());
            let Some(victim) = victim else {
                return Err((
                    StatusCode::TOO_MANY_REQUESTS,
                    "Image snapshots are preparing".into(),
                ));
            };
            if let Some(entry) = entries.remove(&victim) {
                entry.documents.shutdown();
            }
        }
        let (path, input) = if let Some(image) = host_image {
            let bytes = image.bytes.ok_or((StatusCode::TOO_MANY_REQUESTS, "Host image snapshot was evicted; retry".into()))?;
            let input = crate::runtime::cache::temporary_directory("mn-host-reference-").map_err(failure)?;
            let path = input.path().join(image.name);
            File::create(&path).and_then(|mut file| file.write_all(&bytes)).map_err(failure)?;
            (path, Some(input))
        } else if inline {
            let target = references::target(&parent.directory, index, id).map_err(failure)?;
            let parsed = data_url::DataUrl::process(&target)
                .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid inline image URL".into()))?;
            let mime = parsed.mime_type();
            let extension = match (mime.type_.as_str(), mime.subtype.as_str()) {
                ("image", "png") => "png",
                ("image", "jpeg") => "jpg",
                ("image", "gif") => "gif",
                ("image", "webp") => "webp",
                ("image", "svg+xml") => "svg",
                _ => {
                    return Err((
                        StatusCode::UNSUPPORTED_MEDIA_TYPE,
                        "Unsupported inline image type".into(),
                    ));
                }
            };
            let (bytes, _) = parsed
                .decode_to_vec()
                .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid inline image data".into()))?;
            if bytes.len() as u64 > IMAGE_BYTES {
                return Err((StatusCode::PAYLOAD_TOO_LARGE, "Image exceeds 25 MiB".into()));
            }
            let input = crate::runtime::cache::temporary_directory("mn-reference-input-")
                .map_err(failure)?;
            let path = input.path().join(format!("image.{extension}"));
            File::create(&path)
                .and_then(|mut file| file.write_all(&bytes))
                .map_err(failure)?;
            (path, Some(input))
        } else {
            let source = source.as_ref().unwrap();
            if crate::core::document_format::DocumentFormat::for_path(&source)
                != Some(crate::core::document_format::DocumentFormat::Image)
            {
                return Err((
                    StatusCode::UNSUPPORTED_MEDIA_TYPE,
                    "Reference is not a supported image file".into(),
                ));
            }
            let file = File::open(&source).map_err(failure)?;
            if !file.metadata().map_err(failure)?.is_file()
                || file.metadata().map_err(failure)?.len() > IMAGE_BYTES
            {
                return Err((StatusCode::PAYLOAD_TOO_LARGE, "Image exceeds 25 MiB".into()));
            }
            // Snapshot only this authorized file, with a growth check, before
            // passing untrusted decoding to the existing supervised worker.
            let input = crate::runtime::cache::temporary_directory("mn-reference-input-")
                .map_err(failure)?;
            let path = input.path().join(source.file_name().unwrap());
            let mut output = File::create(&path).map_err(failure)?;
            if io::copy(&mut file.take(IMAGE_BYTES + 1), &mut output).map_err(failure)?
                > IMAGE_BYTES
            {
                return Err((
                    StatusCode::PAYLOAD_TOO_LARGE,
                    "Image grew beyond 25 MiB".into(),
                ));
            }
            (path, Some(input))
        };
        let documents = files::Documents::new(path.parent().unwrap().into())
            .with_owner_cancellation(parent.cancelled.clone());
        let image = documents
            .open(
                path,
                crate::runtime::cache::cache_directory().map_err(failure)?,
            )
            .map_err(failure)?;
        entries.insert(
            key,
            Entry {
                documents,
                source,
                host_identity,
                _input: input,
                touched: Instant::now(),
            },
        );
        Ok(image)
    }
    pub fn shutdown(&self) {
        for (_, entry) in self.entries.lock().unwrap().drain() {
            entry.documents.shutdown();
        }
    }
}
impl Drop for Resources {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Request {
    id: usize,
    revision: String,
    image_revision: Option<String>,
    frame: Option<usize>,
}

pub(super) async fn info(
    State(app): State<App>,
    headers: HeaderMap,
    Query(request): Query<Request>,
) -> Result<Json<Value>, ApiError> {
    let document = app.documents.select(&headers)?;
    let index = index(&document)?;
    if request.revision != index.revision {
        return Err((StatusCode::CONFLICT, "Document revision changed".into()));
    }
    let permit = app
        .work
        .clone()
        .try_acquire_owned()
        .map_err(|_| (StatusCode::TOO_MANY_REQUESTS, "Reader is busy".into()))?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let reference = references::info(&document.directory,&index,request.id).map_err(|error| (StatusCode::BAD_REQUEST,error.to_string()))?;
        let local = if reference.kind == Kind::Link {
            reference.target.as_deref().filter(|target| !target.starts_with('#')).and_then(|target| app.references.local(&document.source,target).ok())
        } else { None };
        Ok(Json(json!({"reference":reference,"localPath":local,"openPath":local.as_ref().and_then(|path| path.strip_prefix(&app.documents.root).ok())})))
    }).await.map_err(failure)?
}

pub(super) async fn image(
    State(app): State<App>,
    headers: HeaderMap,
    Query(request): Query<Request>,
) -> Result<Response, ApiError> {
    let parent = app.documents.select(&headers)?;
    let index = index(&parent)?;
    if request.revision != index.revision {
        return Err((StatusCode::CONFLICT, "Document revision changed".into()));
    }
    let permit = app
        .work
        .clone()
        .try_acquire_owned()
        .map_err(|_| (StatusCode::TOO_MANY_REQUESTS, "Reader is busy".into()))?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let image = app.references.image(&parent,&index,request.id)?;
        let progress = image.progress.lock().unwrap();
        if let Some(error) = &progress.error {
            let safe = error.replace(image.directory.to_string_lossy().as_ref(),"[image-cache]")
                .replace(image.source.parent().unwrap().to_string_lossy().as_ref(),"[image-input]");
            return Err((StatusCode::BAD_REQUEST,safe.chars().take(800).collect()));
        }
        let Some(image_index) = &progress.index else { return Ok((StatusCode::ACCEPTED,Json(json!({"state":"indexing"}))).into_response()); };
        if request.image_revision.as_ref().is_some_and(|revision| revision != &image_index.revision) { return Err((StatusCode::CONFLICT,"Image resource changed".into())); }
        if let Some(frame) = request.frame {
            let path = crate::core::documents::asset(&image.directory,image_index,0,frame).map_err(|error| (StatusCode::BAD_REQUEST,error.to_string()))?;
            let bytes = std::fs::read(path).map_err(failure)?;
            return Ok(([(header::CONTENT_TYPE,"image/png".to_owned()),(header::HeaderName::from_static("x-reader-revision"),index.revision.clone())],bytes).into_response());
        }
        let mut metadata = crate::core::reader::projected_asset_info(image_index,0).map_err(failure)?;
        metadata.revision = index.revision.clone();
        Ok(Json(json!({"state":"ready","reference":request.id,"imageRevision":image_index.revision,"asset":metadata})).into_response())
    }).await.map_err(failure)?
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(windows)]
    #[test]
    fn windows_request_path_gate_rejects_unc_before_filesystem_resolution() {
        let roots = vec![PathBuf::from(r"\\?\C:\project")];
        // Exercise the pure pre-I/O predicate: never connect to this host.
        let remote = url::Url::parse("file://ungranted-host/share/image.png")
            .unwrap().to_file_path().unwrap();
        assert!(!requested_in_roots(&remote, &roots));
        assert!(!requested_in_roots(Path::new(r"C:\project-other\image.png"), &roots));
        assert!(requested_in_roots(Path::new(r"C:\PROJECT\image.png"), &roots));
    }
    #[cfg(unix)]
    #[test]
    fn directory_aliases_preserve_both_file_boundaries_and_revocation() {
        let project = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let root = project.path().canonicalize().unwrap();
        let source = root.join("source.md");
        let image = root.join("picture.png");
        std::fs::write(&source, "# Project").unwrap();
        std::fs::write(&image, b"owned fixture").unwrap();
        let directory_alias = outside.path().join("project-alias");
        std::os::unix::fs::symlink(&root, &directory_alias).unwrap();
        let target = url::Url::from_file_path(directory_alias.join("picture.png"))
            .unwrap().to_string();
        let current = Arc::new(Mutex::new(vec![root.clone()]));
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let provider = {
            let current = current.clone();
            let calls = calls.clone();
            RootProvider::new(move || {
                calls.fetch_add(1, Ordering::Relaxed);
                Ok(current.lock().unwrap().clone())
            })
        };
        let resources = Resources::new(vec![]).with_provider(Some(provider));
        assert_eq!(resources.local(&source, &target).unwrap(), image);
        current.lock().unwrap().clear();
        assert!(resources.local(&source, &target).is_err());
        *current.lock().unwrap() = vec![root.clone()];
        let hidden = outside.path().join("outside.png");
        std::fs::write(&hidden, b"outside fixture").unwrap();
        std::os::unix::fs::symlink(&hidden, root.join("escape.png")).unwrap();
        assert!(resources.local(&source, "escape.png").is_err());
        let return_link = outside.path().join("return.png");
        std::os::unix::fs::symlink(&image, &return_link).unwrap();
        assert!(resources.local(&source,
            url::Url::from_file_path(return_link).unwrap().as_str()).is_err());
        let unavailable = outside.path().join("missing-directory").join("file.png");
        let error = resources.local(&source,
            url::Url::from_file_path(unavailable).unwrap().as_str()).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
        assert_eq!(calls.load(Ordering::Relaxed), 5);
    }
    #[test]
    fn dynamic_roots_use_one_current_policy_snapshot_per_resolution() {
        let project = tempfile::tempdir().unwrap();
        let shared = tempfile::tempdir().unwrap();
        let source = project.path().join("source.md");
        std::fs::write(&source, "# Project").unwrap();
        let source = source.canonicalize().unwrap();
        let asset = shared.path().join("picture.png");
        std::fs::write(&asset, b"owned fixture").unwrap();
        // Windows retains lexical authorization before filesystem access, so
        // use its canonical grant spelling rather than a possible temp alias.
        // Unix deliberately keeps the raw URL to exercise directory aliases.
        #[cfg(windows)]
        let asset = asset.canonicalize().unwrap();
        let target = url::Url::from_file_path(&asset).unwrap().to_string();
        let current = Arc::new(Mutex::new(Vec::<PathBuf>::new()));
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let provider = {
            let current = current.clone();
            let calls = calls.clone();
            RootProvider::new(move || {
                calls.fetch_add(1, Ordering::Relaxed);
                Ok(current.lock().unwrap().clone())
            })
        };
        let resources = Resources::new(vec![project.path().canonicalize().unwrap()])
            .with_provider(Some(provider));
        assert!(resources.local(&source, &target).is_err());
        *current.lock().unwrap() = vec![shared.path().into(), shared.path().into()];
        assert_eq!(
            resources.local(&source, &target).unwrap(),
            asset.canonicalize().unwrap()
        );
        current.lock().unwrap().clear();
        assert!(resources.local(&source, &target).is_err());
        assert_eq!(calls.load(Ordering::Relaxed), 3);
        assert_eq!(
            resources.local(&source, "source.md").unwrap(),
            source.canonicalize().unwrap()
        );
    }
    #[test]
    fn local_resources_require_explicit_roots_and_canonicalize_encoded_paths() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let source = root.path().join("source.md");
        std::fs::write(&source, "# Test").unwrap();
        let source = source.canonicalize().unwrap();
        let image = root.path().join("中文 pic.png");
        std::fs::write(&image, b"test").unwrap();
        let hidden = outside.path().join("private.png");
        std::fs::write(&hidden, b"private").unwrap();
        assert!(
            Resources::new(vec![])
                .local(&source, "中文%20pic.png")
                .is_err()
        );
        let resources = Resources::new(vec![root.path().canonicalize().unwrap()]);
        assert_eq!(
            resources
                .local(&source, "%E4%B8%AD%E6%96%87%20pic.png?view=1#part")
                .unwrap(),
            image.canonicalize().unwrap()
        );
        assert_eq!(
            resources.local(&source, "/中文%20pic.png").unwrap(),
            image.canonicalize().unwrap()
        );
        let nested = root.path().join("images");
        std::fs::create_dir(&nested).unwrap();
        std::fs::write(nested.join("picture.png"), b"test").unwrap();
        assert_eq!(
            resources.local(&source, "images\\picture.png").unwrap(),
            nested.join("picture.png").canonicalize().unwrap()
        );
        assert!(
            resources
                .local(&source, "https://example.invalid/image.png")
                .is_err()
        );
        assert!(
            resources
                .local(&source, url::Url::from_file_path(&hidden).unwrap().as_str())
                .is_err()
        );
        #[cfg(unix)]
        {
            let alias = root.path().join("alias.png");
            std::os::unix::fs::symlink(&image, &alias).unwrap();
            assert_eq!(
                resources.local(&source, "alias.png").unwrap(),
                image.canonicalize().unwrap()
            );
            std::fs::remove_file(&alias).unwrap();
            std::os::unix::fs::symlink(&hidden, &alias).unwrap();
            assert!(resources.local(&source, "alias.png").is_err());
        }
    }
}
