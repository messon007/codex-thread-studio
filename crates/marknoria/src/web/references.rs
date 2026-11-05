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
    _input: Option<tempfile::TempDir>,
    touched: Instant,
}
pub(super) struct Resources {
    roots: Vec<PathBuf>,
    provider: Option<RootProvider>,
    entries: Mutex<HashMap<String, Entry>>,
}
impl Resources {
    pub fn new(roots: Vec<PathBuf>) -> Self {
        Self {
            roots,
            provider: None,
            entries: Mutex::new(HashMap::new()),
        }
    }
    pub fn with_provider(mut self, provider: Option<RootProvider>) -> Self {
        self.provider = provider;
        self
    }
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
        let base = url::Url::from_file_path(source).map_err(|_| denied())?;
        let target = base.join(target).map_err(|_| denied())?;
        if target.scheme() != "file" {
            return Err(denied());
        }
        let requested = target.to_file_path().map_err(|_| denied())?;
        if requested
            .components()
            .any(|part| matches!(part, std::path::Component::ParentDir))
            || !roots.iter().any(|root| {
                let root = url::Url::from_file_path(root)
                    .ok()
                    .and_then(|url| url.to_file_path().ok());
                root.is_some_and(|root| {
                    #[cfg(windows)]
                    {
                        requested.to_string_lossy().to_lowercase().starts_with(
                            &(root
                                .to_string_lossy()
                                .trim_end_matches(['\\', '/'])
                                .to_lowercase()
                                + "\\"),
                        )
                    }
                    #[cfg(not(windows))]
                    {
                        requested.starts_with(root)
                    }
                })
            })
        {
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
        let source = if inline {
            None
        } else {
            let target = references::target(&parent.directory, index, id).map_err(failure)?;
            Some(
                self.local(&parent.source, &target)
                    .map_err(|error| (StatusCode::FORBIDDEN, error.to_string()))?,
            )
        };
        let key = format!("{}:{}:{id}", parent.id, index.revision);
        let mut entries = self.entries.lock().map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Reference cache unavailable".into(),
            )
        })?;
        if entries
            .get(&key)
            .is_some_and(|entry| entry.source != source)
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
        let (path, input) = if inline {
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
    #[test]
    fn dynamic_roots_use_one_current_policy_snapshot_per_resolution() {
        let project = tempfile::tempdir().unwrap();
        let shared = tempfile::tempdir().unwrap();
        let source = project.path().join("source.md");
        std::fs::write(&source, "# Project").unwrap();
        let asset = shared.path().join("picture.png");
        std::fs::write(&asset, b"owned fixture").unwrap();
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
