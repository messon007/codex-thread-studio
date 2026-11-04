//! Explicit, revision-checked shared-folder configuration; unrelated settings survive.
use crate::preferences_document::{self as document, Error};
use axum::{
    body::Body,
    extract::{rejection::JsonRejection, State},
    http::{Response, StatusCode},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::Path;
fn bad(error: impl ToString) -> Error {
    (StatusCode::BAD_REQUEST, error.to_string())
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Patch {
    expected_revision: String,
    directories: Vec<String>,
}
#[derive(Serialize, Debug)]
pub struct Snapshot {
    revision: String,
    directories: Vec<String>,
}
fn validate(paths: &[String]) -> Result<(), Error> {
    if paths.len() > 256
        || paths.iter().any(|p| {
            p.is_empty()
                || p.len() > 4096
                || p.chars().any(char::is_control)
                || !Path::new(p).is_absolute()
        })
    {
        return Err(bad(
            "Choose at most 256 absolute folder paths (maximum 4096 bytes each)",
        ));
    }
    Ok(())
}
fn snapshot(raw: &Value) -> Result<Snapshot, Error> {
    let directories: Vec<String> = serde_json::from_value(
        raw.get("sharedDocumentDirectories")
            .cloned()
            .unwrap_or_else(|| json!([])),
    )
    .map_err(bad)?;
    validate(&directories)?;
    let revision = document::revision(&json!(directories))?;
    Ok(Snapshot {
        revision,
        directories,
    })
}
fn save(path: &Path, request: Patch) -> Result<Snapshot, Error> {
    if request.expected_revision.len() != 64
        || !request
            .expected_revision
            .bytes()
            .all(|b| b.is_ascii_hexdigit())
    {
        return Err(bad("Invalid shared-folder revision"));
    }
    validate(&request.directories)?;
    let mut raw = document::read(path)?;
    if snapshot(&raw)?.revision != request.expected_revision {
        return Err((StatusCode::CONFLICT,"Shared folders changed in another client. Reload before saving; your edits are retained.".into()));
    }
    raw["sharedDocumentDirectories"] = json!(request.directories);
    let saved = snapshot(&raw)?;
    document::write(path, &raw)?;
    Ok(saved)
}
async fn operation(state: crate::GatewayState, patch: Option<Patch>) -> Response<Body> {
    match tokio::task::spawn_blocking(move || {
        let result = (|| -> Result<Snapshot, Error> {
            let _lock = state
                .preferences_lock
                .lock()
                .map_err(|_| bad("Settings lock is unavailable"))?;
            // Take the live-policy lock before writing, so a failed lock cannot
            // leave durable configuration newer than the installed access policy.
            let mut live = state
                .shared_document_directories
                .lock()
                .map_err(|_| bad("Shared-folder policy is unavailable"))?;
            let value = if let Some(patch) = patch {
                save(&state.preferences_path, patch)?
            } else {
                snapshot(&document::read(&state.preferences_path)?)?
            };
            *live = value.directories.clone();
            Ok(value)
        })();
        match result {
            Ok(value) => crate::json_response(StatusCode::OK, &value),
            Err((status, error)) => crate::json_error(status, &error),
        }
    })
    .await
    {
        Ok(response) => response,
        Err(_) => crate::json_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Shared-folder settings worker stopped",
        ),
    }
}
pub async fn get(State(state): State<crate::GatewayState>) -> Response<Body> {
    operation(state, None).await
}
pub async fn patch(
    State(state): State<crate::GatewayState>,
    body: Result<Json<Patch>, JsonRejection>,
) -> Response<Body> {
    match body {
        Ok(Json(patch)) => operation(state, Some(patch)).await,
        Err(_) => crate::json_error(
            StatusCode::BAD_REQUEST,
            "Invalid shared-folder settings request",
        ),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scoped_changes_preserve_unknown_settings_and_conflicts_do_not_write() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("settings.json");
        document::write(
            &path,
            &json!({"theme":"dark","future":{"keep":true},"typography":{"futureFont":42}}),
        )
        .unwrap();
        let first = snapshot(&document::read(&path).unwrap()).unwrap();
        let mut other = document::read(&path).unwrap();
        other["language"] = json!("zh-CN");
        document::write(&path, &other).unwrap();
        let value = temp
            .path()
            .join("not mounted 中文 # ")
            .to_string_lossy()
            .into_owned();
        let saved = save(
            &path,
            Patch {
                expected_revision: first.revision.clone(),
                directories: vec![value.clone()],
            },
        )
        .unwrap();
        assert_eq!(saved.directories, vec![value]);
        let raw = document::read(&path).unwrap();
        assert_eq!(raw["future"]["keep"], true);
        assert_eq!(raw["typography"]["futureFont"], 42);
        assert_eq!(raw["language"], "zh-CN");
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(
            save(
                &path,
                Patch {
                    expected_revision: first.revision,
                    directories: vec![]
                }
            )
            .unwrap_err()
            .0,
            StatusCode::CONFLICT
        );
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        assert!(save(
            &path,
            Patch {
                expected_revision: saved.revision,
                directories: vec!["relative".into()]
            }
        )
        .is_err());
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
    }
}
