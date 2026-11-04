//! Bounded, lazy directory pages. All filesystem and ignore work runs off the executor.
use axum::{
    body::Body,
    extract::State,
    http::{Response, StatusCode},
    Json,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, OnceLock},
    time::{Duration, Instant},
};

const PAGE: usize = 200;
const MAX_ENTRIES: usize = 10_000;
const MAX_BYTES: usize = 4 * 1024 * 1024;
type Error = (StatusCode, String);
fn bad(message: impl ToString) -> Error {
    (StatusCode::BAD_REQUEST, message.to_string())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Request {
    root: String,
    #[serde(default)]
    path: String,
    #[serde(default)]
    query: String,
    #[serde(default)]
    show_hidden: bool,
    #[serde(default)]
    show_ignored: bool,
    #[serde(default)]
    cursor: Option<String>,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    name: String,
    path: String,
    kind: &'static str,
    size: Option<u64>,
    read_only: bool,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Directory {
    path: String,
    entries: Vec<Entry>,
    next_cursor: Option<String>,
    scan_truncated: bool,
    total: usize,
    elapsed_ms: u128,
    read_only: bool,
}

struct Resolved {
    project: PathBuf,
    path: PathBuf,
    metadata: fs::Metadata,
}
fn resolve(
    root: &str,
    relative: &str,
    shared: &[PathBuf],
    allow_project_absolute: bool,
) -> Result<Resolved, Error> {
    if root.is_empty()
        || root.len() > 4096
        || relative.len() > 8192
        || root.chars().chain(relative.chars()).any(char::is_control)
    {
        return Err(bad("Invalid project or shared path"));
    }
    let root = fs::canonicalize(root).map_err(|_| bad("Project directory is unavailable"))?;
    if !root.is_dir() {
        return Err(bad("Project root is not a directory"));
    }
    let path = fs::canonicalize(root.join(relative))
        .map_err(|_| (StatusCode::NOT_FOUND, "Directory is unavailable".into()))?;
    let project = path.starts_with(&root);
    let shared = shared.iter().any(|p| path.starts_with(p));
    if !project && !shared {
        return Err((
            StatusCode::FORBIDDEN,
            "Path is outside the project and configured shared directories".into(),
        ));
    }
    if Path::new(relative).is_absolute() && project && !shared && !allow_project_absolute {
        return Err(bad("Expected a project-relative or configured shared path"));
    }
    let metadata = fs::metadata(&path).map_err(bad)?;
    Ok(Resolved {
        project: root,
        path,
        metadata,
    })
}
pub fn directory(
    root: &str,
    relative: &str,
    shared: &[PathBuf],
) -> Result<(PathBuf, PathBuf), Error> {
    let item = resolve(root, relative, shared, false)?;
    if !item.metadata.is_dir() {
        return Err(bad("Path is not a directory"));
    }
    Ok((item.project, item.path))
}
#[cfg(test)]
fn load(request: Request) -> Result<Directory, Error> {
    load_with_shared(request, &[])
}
fn load_with_shared(request: Request, shared: &[PathBuf]) -> Result<Directory, Error> {
    let started = Instant::now();
    if request.query.len() > 256 || request.query.chars().any(char::is_control) {
        return Err(bad("File filter exceeds its budget"));
    }
    let (root, path) = directory(&request.root, &request.path, shared)?;
    let external = !path.starts_with(&root);
    let absolute_entries = external || Path::new(&request.path).is_absolute();
    let query = request.query.to_lowercase();
    let mut builder = ignore::WalkBuilder::new(&path);
    builder
        .max_depth(Some(1))
        .follow_links(false)
        .hidden(!request.show_hidden)
        .ignore(!request.show_ignored)
        .git_ignore(!request.show_ignored)
        .git_exclude(!request.show_ignored)
        .git_global(!request.show_ignored)
        .require_git(false);
    let mut entries = Vec::new();
    let mut weight = 0usize;
    let mut truncated = false;
    for result in builder.build() {
        if started.elapsed() > Duration::from_secs(2) {
            truncated = true;
            break;
        }
        let entry = match result {
            Ok(entry) => entry,
            Err(_) => {
                truncated = true;
                continue;
            }
        };
        if entry.depth() == 0 {
            continue;
        }
        let Some(name) = entry
            .file_name()
            .to_str()
            .filter(|n| n.len() <= 1024 && !n.chars().any(char::is_control))
        else {
            truncated = true;
            continue;
        };
        // Never descend through symlink directories (including ancestor cycles).
        let Ok(canonical) = fs::canonicalize(entry.path()) else {
            continue;
        };
        if !canonical.starts_with(&root) && !shared.iter().any(|p| canonical.starts_with(p)) {
            continue;
        }
        let Ok(metadata) = fs::metadata(&canonical) else {
            continue;
        };
        let kind = if metadata.is_dir() && !entry.path_is_symlink() {
            "directory"
        } else if metadata.is_file() {
            "file"
        } else {
            continue;
        };
        let entry_path = if absolute_entries {
            entry.path()
        } else {
            entry.path().strip_prefix(&root).map_err(bad)?
        };
        let relative = entry_path
            .to_str()
            .filter(|p| p.len() <= 8192 && !p.chars().any(char::is_control));
        let Some(relative) = relative else {
            truncated = true;
            continue;
        };
        #[cfg(windows)]
        let relative = relative.replace('\\', "/");
        #[cfg(not(windows))]
        let relative = relative.to_owned();
        if kind == "file" && !query.is_empty() && !relative.to_lowercase().contains(&query) {
            continue;
        }
        let size = name.len() + relative.len() + 128;
        if entries.len() >= MAX_ENTRIES || weight + size > MAX_BYTES {
            truncated = true;
            break;
        }
        weight += size;
        entries.push(Entry {
            name: name.into(),
            path: relative,
            kind,
            size: (kind == "file").then_some(metadata.len()),
            read_only: !canonical.starts_with(&root),
        });
    }
    entries.sort_by_cached_key(|e| (e.kind != "directory", e.name.to_lowercase(), e.name.clone()));
    let mut digest = Sha256::new();
    digest.update(root.as_os_str().as_encoded_bytes());
    digest.update(request.path.as_bytes());
    digest.update(query.as_bytes());
    digest.update([
        request.show_hidden as u8,
        request.show_ignored as u8,
        truncated as u8,
    ]);
    digest.update(serde_json::to_vec(&entries).map_err(bad)?);
    let revision = digest
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    let offset = if let Some(cursor) = request.cursor {
        if cursor.len() > 80 {
            return Err(bad("Invalid directory cursor"));
        }
        let (hash, offset) = cursor
            .split_once(':')
            .ok_or_else(|| bad("Invalid directory cursor"))?;
        if hash != revision {
            return Err((
                StatusCode::CONFLICT,
                "Directory changed; refresh this folder".into(),
            ));
        }
        offset
            .parse::<usize>()
            .ok()
            .filter(|n| *n <= entries.len() && *n <= MAX_ENTRIES && n % PAGE == 0)
            .ok_or_else(|| bad("Invalid directory cursor"))?
    } else {
        0
    };
    let total = entries.len();
    let end = (offset + PAGE).min(total);
    let next_cursor = (end < total).then(|| format!("{revision}:{end}"));
    let entries = entries.into_iter().skip(offset).take(PAGE).collect();
    Ok(Directory {
        path: request.path,
        entries,
        next_cursor,
        scan_truncated: truncated,
        total,
        elapsed_ms: started.elapsed().as_millis(),
        read_only: external,
    })
}
async fn work<T: Serialize + Send + 'static>(
    run: impl FnOnce() -> Result<T, Error> + Send + 'static,
) -> Response<Body> {
    static WORKERS: OnceLock<Arc<tokio::sync::Semaphore>> = OnceLock::new();
    let Ok(permit) = WORKERS
        .get_or_init(|| Arc::new(tokio::sync::Semaphore::new(2)))
        .clone()
        .try_acquire_owned()
    else {
        return crate::json_error(
            StatusCode::TOO_MANY_REQUESTS,
            "Files workers are busy; try again",
        );
    };
    match tokio::task::spawn_blocking(move || {
        let _permit = permit;
        match run() {
            Ok(value) => crate::json_response(StatusCode::OK, &value),
            Err((status, message)) => crate::json_error(status, &message),
        }
    })
    .await
    {
        Ok(response) => response,
        Err(_) => crate::json_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Directory worker stopped",
        ),
    }
}
pub async fn handle(
    State(state): State<crate::GatewayState>,
    Json(request): Json<Request>,
) -> Response<Body> {
    work(move || {
        let shared = crate::shared_document_directories(&state)?;
        load_with_shared(request, &shared)
    })
    .await
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RootsRequest {
    root: String,
}
#[derive(Serialize)]
struct Roots {
    roots: Vec<Entry>,
}
fn roots(root: &str, shared: &[PathBuf]) -> Result<Roots, Error> {
    let (project, _) = directory(root, "", shared)?;
    let candidates = shared
        .iter()
        .filter(|p| !p.starts_with(&project))
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();
    let mut seen = Vec::<PathBuf>::new();
    let mut roots = vec![Entry {
        name: "Project".into(),
        path: String::new(),
        kind: "directory",
        size: None,
        read_only: false,
    }];
    for path in &candidates {
        if seen.iter().any(|parent| path.starts_with(parent)) {
            continue;
        }
        seen.push(path.clone());
        let Some(full) = path
            .to_str()
            .filter(|s| s.len() <= 8192 && !s.chars().any(char::is_control))
        else {
            continue;
        };
        let name = path.file_name().and_then(|s| s.to_str()).unwrap_or(full);
        roots.push(Entry {
            name: format!("Shared · {name}"),
            path: full.into(),
            kind: "directory",
            size: None,
            read_only: true,
        });
    }
    Ok(Roots { roots })
}
pub async fn list_roots(
    State(state): State<crate::GatewayState>,
    Json(request): Json<RootsRequest>,
) -> Response<Body> {
    work(move || {
        let shared = crate::shared_document_directories(&state)?;
        roots(&request.root, &shared)
    })
    .await
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocationRequest {
    root: String,
    path: String,
}
#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
struct Location {
    path: String,
    directory: bool,
    read_only: bool,
}
fn location(request: LocationRequest, shared: &[PathBuf]) -> Result<Location, Error> {
    let item = resolve(&request.root, &request.path, shared, true)?;
    if !item.metadata.is_file() && !item.metadata.is_dir() {
        return Err(bad("Choose a regular file or directory"));
    }
    let path = item
        .path
        .to_str()
        .filter(|p| p.len() <= 8192 && !p.chars().any(char::is_control))
        .ok_or_else(|| bad("Item path cannot be represented"))?;
    Ok(Location {
        path: path.into(),
        directory: item.metadata.is_dir(),
        read_only: !item.path.starts_with(&item.project),
    })
}
pub async fn item_location(
    State(state): State<crate::GatewayState>,
    Json(request): Json<LocationRequest>,
) -> Response<Body> {
    work(move || {
        let shared = crate::shared_document_directories(&state)?;
        location(request, &shared)
    })
    .await
}
#[cfg(test)]
mod tests {
    use super::*;
    fn request(root: &Path) -> Request {
        Request {
            root: root.to_string_lossy().into(),
            path: String::new(),
            query: String::new(),
            show_hidden: false,
            show_ignored: false,
            cursor: None,
        }
    }
    #[test]
    fn shared_scopes_keep_project_identity_and_revoke_new_access() {
        let temp = tempfile::tempdir().unwrap();
        let project = temp.path().join("project");
        let shared = temp.path().join("shared");
        let outside = temp.path().join("shared-other");
        for path in [&project, &shared, &outside] {
            fs::create_dir(path).unwrap();
        }
        let project = fs::canonicalize(project).unwrap();
        let shared = fs::canonicalize(shared).unwrap();
        let outside = fs::canonicalize(outside).unwrap();
        fs::create_dir(shared.join("nested")).unwrap();
        fs::write(shared.join("说明.md"), "# Shared").unwrap();
        fs::write(outside.join("secret.md"), "secret").unwrap();
        let allowed = vec![
            shared.clone(),
            shared.join("nested"),
            shared.clone(),
            project.clone(),
        ];
        let roots = roots(project.to_str().unwrap(), &allowed).unwrap();
        assert_eq!(roots.roots.len(), 2);
        assert!(roots.roots[1].read_only);
        let mut read = request(&project);
        read.path = shared.to_string_lossy().into();
        let page = load_with_shared(read, &allowed).unwrap();
        assert!(page.read_only);
        assert!(page
            .entries
            .iter()
            .all(|e| e.read_only && Path::new(&e.path).is_absolute()));
        let file = LocationRequest {
            root: project.to_string_lossy().into(),
            path: shared.join("说明.md").to_string_lossy().into(),
        };
        assert!(location(file, &allowed).unwrap().read_only);
        let root_location = location(
            LocationRequest {
                root: project.to_string_lossy().into(),
                path: String::new(),
            },
            &allowed,
        )
        .unwrap();
        assert!(root_location.directory && !root_location.read_only);
        fs::write(project.join("project.md"), "# Project").unwrap();
        let mut nested_project = request(&project);
        nested_project.path = project.to_string_lossy().into();
        let parent = fs::canonicalize(temp.path()).unwrap();
        let nested_project = load_with_shared(nested_project, &[parent]).unwrap();
        assert!(!nested_project.read_only);
        assert!(nested_project
            .entries
            .iter()
            .all(|e| Path::new(&e.path).is_absolute() && !e.read_only));
        let mut revoked = request(&project);
        revoked.path = shared.to_string_lossy().into();
        assert_eq!(
            load_with_shared(revoked, &[]).unwrap_err().0,
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            location(
                LocationRequest {
                    root: project.to_string_lossy().into(),
                    path: outside.join("secret.md").to_string_lossy().into()
                },
                &allowed
            )
            .unwrap_err()
            .0,
            StatusCode::FORBIDDEN
        );
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(outside.join("secret.md"), shared.join("escape.md"))
                .unwrap();
            std::os::unix::fs::symlink(shared.join("说明.md"), project.join("shared-link.md"))
                .unwrap();
            assert!(
                load_with_shared(request(&project), &allowed)
                    .unwrap()
                    .entries
                    .iter()
                    .find(|e| e.name == "shared-link.md")
                    .unwrap()
                    .read_only
            );
            let mut read = request(&project);
            read.path = shared.to_string_lossy().into();
            assert!(!load_with_shared(read, &allowed)
                .unwrap()
                .entries
                .iter()
                .any(|e| e.name == "escape.md"));
        }
    }
    #[test]
    fn pages_ignore_rules_and_changed_cursor_are_explicit() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        fs::write(root.join(".gitignore"), "*.tmp\n!keep.tmp\n").unwrap();
        fs::write(root.join(".ignore"), "custom.log\n").unwrap();
        for name in ["ignored.tmp", "keep.tmp", "custom.log", ".secret"] {
            fs::write(root.join(name), "x").unwrap();
        }
        for n in 0..240 {
            fs::write(root.join(format!("f{n:03}.md")), "x").unwrap();
        }
        fs::create_dir(root.join("nested")).unwrap();
        let first = load(request(root)).unwrap();
        assert_eq!(first.entries.len(), PAGE);
        assert_eq!(first.entries[0].kind, "directory");
        assert_eq!(first.total, 242);
        assert!(!first.scan_truncated);
        let mut next = request(root);
        next.cursor = first.next_cursor.clone();
        let tail = load(next).unwrap();
        assert_eq!(tail.entries.len(), 42);
        assert!(tail.entries.iter().any(|e| e.name == "keep.tmp"));
        assert!(tail.next_cursor.is_none());
        fs::write(root.join("new.md"), "x").unwrap();
        let mut next = request(root);
        next.cursor = first.next_cursor;
        assert_eq!(load(next).unwrap_err().0, StatusCode::CONFLICT);
        let mut filter = request(root);
        filter.query = "f001.md ".into();
        assert_eq!(load(filter).unwrap().total, 1); // directory retained, trailing space literal
        let mut all = request(root);
        all.show_ignored = true;
        all.show_hidden = true;
        assert_eq!(load(all).unwrap().total, 248);
    }
    #[cfg(unix)]
    #[test]
    fn symlinks_and_traversal_do_not_escape_or_cycle() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("project");
        fs::create_dir(&root).unwrap();
        fs::write(temp.path().join("outside"), "secret").unwrap();
        std::os::unix::fs::symlink(temp.path().join("outside"), root.join("escape")).unwrap();
        std::os::unix::fs::symlink(&root, root.join("loop")).unwrap();
        assert!(load(request(&root)).unwrap().entries.is_empty());
        let mut escape = request(&root);
        escape.path = "..".into();
        assert_eq!(load(escape).unwrap_err().0, StatusCode::FORBIDDEN);
    }
}
