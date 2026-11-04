//! Explicit editing of the startup backend file. No registry reload or process launch.
use crate::{
    backend_config,
    preferences_document::{self as document, Error},
};
use axum::{
    body::Body,
    extract::{rejection::JsonRejection, State},
    http::{Response, StatusCode},
    Json,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    io::Read,
    path::{Path, PathBuf},
};
const EMPTY: &str = "{\n  \"version\": 1,\n  \"backends\": []\n}\n";
#[derive(Serialize)]
pub struct Snapshot {
    revision: String,
    path: String,
    source: String,
    exists: bool,
    #[serde(flatten)]
    check: Check,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Check {
    validation_error: Option<String>,
    backend_count: Option<usize>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Validate {
    source: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Patch {
    expected_revision: String,
    source: String,
}
struct Loaded {
    snapshot: Snapshot,
    target: PathBuf,
}
fn bad(error: impl ToString) -> Error {
    (StatusCode::BAD_REQUEST, error.to_string())
}
fn bounded(error: String) -> String {
    if error.len() <= 4096 {
        error
    } else {
        format!("{}…", error.chars().take(1024).collect::<String>())
    }
}
fn check(source: &str, path: &Path) -> Check {
    match backend_config::parse_source(source, path) {
        Ok(backends) => Check {
            validation_error: None,
            backend_count: Some(backends.len()),
        },
        Err(error) => Check {
            validation_error: Some(bounded(error)),
            backend_count: None,
        },
    }
}
fn budget(source: &str) -> Result<(), Error> {
    if source.len() as u64 > backend_config::MAX_CONFIG_BYTES {
        Err(bad("Backend configuration exceeds 256 KiB"))
    } else {
        Ok(())
    }
}
fn target(path: &Path, depth: usize) -> Result<PathBuf, Error> {
    if depth > 16 {
        return Err(bad("Backend configuration has too many symbolic links"));
    }
    match std::fs::canonicalize(path) {
        Ok(path) => Ok(path),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            if std::fs::symlink_metadata(path).is_ok_and(|meta| meta.is_symlink()) {
                let link = std::fs::read_link(path).map_err(bad)?;
                return target(
                    &if link.is_absolute() {
                        link
                    } else {
                        path.parent().unwrap_or(Path::new(".")).join(link)
                    },
                    depth + 1,
                );
            }
            let absolute = std::path::absolute(path).map_err(bad)?;
            if let (Some(parent), Some(name)) = (absolute.parent(), absolute.file_name()) {
                // Resolve the existing ancestor even when several directories
                // are missing. Windows canonical paths use a verbatim prefix;
                // creating a parent must not change the document's identity.
                return Ok(target(parent, depth)?.join(name));
            }
            Ok(absolute)
        }
        Err(error) => Err(bad(error)),
    }
}
fn load(path: &Path) -> Result<Loaded, Error> {
    let target = target(path, 0)?;
    let (exists, source) = match std::fs::metadata(&target) {
        Ok(metadata) => {
            if !metadata.is_file() {
                return Err(bad("Backend configuration is not a regular file"));
            }
            if metadata.len() > backend_config::MAX_CONFIG_BYTES {
                return Err(bad("Backend configuration exceeds 256 KiB"));
            }
            let mut source = String::new();
            std::fs::File::open(&target)
                .and_then(|file| {
                    file.take(backend_config::MAX_CONFIG_BYTES + 1)
                        .read_to_string(&mut source)
                })
                .map_err(bad)?;
            budget(&source)?;
            (true, source)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => (false, EMPTY.into()),
        Err(error) => return Err(bad(error)),
    };
    let mut hash = Sha256::new();
    hash.update(b"studio-backend-file-v1\0");
    hash.update(target.as_os_str().as_encoded_bytes());
    hash.update([0, u8::from(exists)]);
    hash.update(source.as_bytes());
    let snapshot = Snapshot {
        revision: hash
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
        path: path.to_string_lossy().into_owned(),
        check: check(&source, path),
        source,
        exists,
    };
    Ok(Loaded { snapshot, target })
}
struct PreparedSave {
    configured: PathBuf,
    target: PathBuf,
    revision: String,
    source: String,
}
fn prepare_save(path: &Path, patch: Patch) -> Result<PreparedSave, Error> {
    budget(&patch.source)?;
    if patch.expected_revision.len() != 64
        || !patch
            .expected_revision
            .bytes()
            .all(|c| c.is_ascii_hexdigit())
    {
        return Err(bad("Invalid backend configuration revision"));
    }
    let current = load(path)?;
    if current.snapshot.revision != patch.expected_revision {
        return Err((StatusCode::CONFLICT, "Backend configuration changed outside this editor. Reload before saving; your edits are retained.".into()));
    }
    if let Some(error) = check(&patch.source, path).validation_error {
        return Err(bad(error));
    }
    Ok(PreparedSave {
        configured: path.into(),
        target: current.target,
        revision: patch.expected_revision,
        source: patch.source,
    })
}
impl PreparedSave {
    fn commit(self) -> Result<Snapshot, Error> {
        document::write_atomic_checked(&self.target, self.source.as_bytes(), || {
            if load(&self.configured)?.snapshot.revision != self.revision {
                return Err((StatusCode::CONFLICT, "Backend configuration changed while saving. Reload to review the current file; your edits are retained.".into()));
            }
            Ok(())
        })?;
        let current = load(&self.configured)?;
        if current.target != self.target || current.snapshot.source != self.source {
            return Err((StatusCode::CONFLICT, "Backend configuration changed while saving. Reload to review the current file; your edits are retained.".into()));
        }
        Ok(current.snapshot)
    }
}
fn save(path: &Path, patch: Patch) -> Result<Snapshot, Error> {
    prepare_save(path, patch)?.commit()
}
async fn operation(state: crate::GatewayState, patch: Option<Patch>) -> Response<Body> {
    match tokio::task::spawn_blocking(move || {
        let _lock = state
            .preferences_lock
            .lock()
            .map_err(|_| bad("Settings lock is unavailable"))?;
        if let Some(patch) = patch {
            save(&state.backend_config_path, patch)
        } else {
            load(&state.backend_config_path).map(|loaded| loaded.snapshot)
        }
    })
    .await
    {
        Ok(Ok(snapshot)) => crate::json_response(StatusCode::OK, &snapshot),
        Ok(Err((status, error))) => crate::json_error(status, &error),
        Err(_) => crate::json_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Backend configuration worker stopped",
        ),
    }
}
pub async fn get(State(state): State<crate::GatewayState>) -> Response<Body> {
    operation(state, None).await
}
pub async fn put(
    State(state): State<crate::GatewayState>,
    body: Result<Json<Patch>, JsonRejection>,
) -> Response<Body> {
    match body {
        Ok(Json(patch)) => operation(state, Some(patch)).await,
        Err(_) => crate::json_error(
            StatusCode::BAD_REQUEST,
            "Invalid backend configuration request",
        ),
    }
}
pub async fn validate(
    State(state): State<crate::GatewayState>,
    body: Result<Json<Validate>, JsonRejection>,
) -> Response<Body> {
    let Ok(Json(request)) = body else {
        return crate::json_error(
            StatusCode::BAD_REQUEST,
            "Invalid backend validation request",
        );
    };
    match tokio::task::spawn_blocking(move || {
        budget(&request.source)?;
        Ok::<_, Error>(check(&request.source, &state.backend_config_path))
    })
    .await
    {
        Ok(Ok(check)) => crate::json_response(StatusCode::OK, &check),
        Ok(Err((status, error))) => crate::json_error(status, &error),
        Err(_) => crate::json_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Backend configuration worker stopped",
        ),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_explicit_save_repairs_invalid_json_and_rejects_stale_or_invalid_edits() {
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("nested/backends.json");
        let missing = load(&path).unwrap().snapshot;
        assert!(!missing.exists && !path.parent().unwrap().exists());
        let missing_target = load(&path).unwrap().target;
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let after_mkdir = load(&path).unwrap();
        assert_eq!(after_mkdir.target, missing_target);
        assert_eq!(after_mkdir.snapshot.revision, missing.revision);
        std::fs::remove_dir(path.parent().unwrap()).unwrap();
        let original = "{\r\n  \"version\":1, \"backends\": []\r\n}\r\n";
        let base = save(
            &path,
            Patch {
                expected_revision: missing.revision,
                source: original.into(),
            },
        )
        .unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), original.as_bytes());
        assert_eq!(
            save(
                &path,
                Patch {
                    expected_revision: base.revision.clone(),
                    source: "{\"unknown\":true}".into()
                }
            )
            .err()
            .unwrap()
            .0,
            StatusCode::BAD_REQUEST
        );
        assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
        std::fs::write(&path, "broken JSON").unwrap();
        assert_eq!(
            save(
                &path,
                Patch {
                    expected_revision: base.revision,
                    source: original.into()
                }
            )
            .err()
            .unwrap()
            .0,
            StatusCode::CONFLICT
        );
        let invalid = load(&path).unwrap().snapshot;
        assert!(invalid.check.validation_error.is_some());
        save(
            &path,
            Patch {
                expected_revision: invalid.revision,
                source: original.into(),
            },
        )
        .unwrap();
        let current = load(&path).unwrap().snapshot;
        assert!(save(
            &path,
            Patch {
                expected_revision: current.revision,
                source: " ".repeat(256 * 1024 + 1)
            }
        )
        .is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
    }
    #[test]
    fn changes_after_preparation_abort_the_commit_without_erasing_external_edits() {
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("backends.json");
        std::fs::write(&path, EMPTY).unwrap();
        let revision = load(&path).unwrap().snapshot.revision;
        let save = prepare_save(
            &path,
            Patch {
                expected_revision: revision,
                source: EMPTY.replace("\n", "\r\n"),
            },
        )
        .unwrap();
        let outside = "{\"version\":1,\"backends\":[]}\n";
        std::fs::write(&path, outside).unwrap();
        assert_eq!(save.commit().err().unwrap().0, StatusCode::CONFLICT);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), outside);
        assert!(!std::fs::read_dir(folder.path()).unwrap().any(|entry| entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".tmp")));
    }
    #[cfg(unix)]
    #[test]
    fn symlink_targets_are_preserved_and_repointing_is_a_conflict_even_for_equal_bytes() {
        let folder = tempfile::tempdir().unwrap();
        let a = folder.path().join("a.json");
        let b = folder.path().join("b.json");
        let link = folder.path().join("backends.json");
        std::os::unix::fs::symlink("a.json", &link).unwrap();
        let missing = load(&link).unwrap().snapshot;
        save(
            &link,
            Patch {
                expected_revision: missing.revision,
                source: EMPTY.into(),
            },
        )
        .unwrap();
        assert!(std::fs::symlink_metadata(&link).unwrap().is_symlink());
        assert_eq!(std::fs::read_to_string(&a).unwrap(), EMPTY);
        std::fs::write(&b, EMPTY).unwrap();
        let base = load(&link).unwrap().snapshot;
        std::fs::remove_file(&link).unwrap();
        std::os::unix::fs::symlink("b.json", &link).unwrap();
        assert_eq!(
            save(
                &link,
                Patch {
                    expected_revision: base.revision,
                    source: EMPTY.into()
                }
            )
            .err()
            .unwrap()
            .0,
            StatusCode::CONFLICT
        );
        assert_eq!(std::fs::read_to_string(&b).unwrap(), EMPTY);
    }
}
