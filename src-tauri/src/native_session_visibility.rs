//! Scoped, explicit-save session visibility preferences, shared with Router policy.
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
fn bad(e: impl ToString) -> Error {
    (StatusCode::BAD_REQUEST, e.to_string())
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Values {
    pub hidden_session_directories: Vec<String>,
    pub session_directory_ignore: Vec<String>,
}
impl Values {
    fn validate(&self) -> Result<(), Error> {
        crate::session_directory_policy::validate(
            &self.hidden_session_directories,
            &self.session_directory_ignore,
        )
        .map_err(bad)?;
        crate::session_directory_policy::Filter::new(
            &self.hidden_session_directories,
            &self.session_directory_ignore,
        )
        .map_err(bad)?;
        Ok(())
    }
}
#[derive(Debug, Serialize)]
struct Snapshot {
    revision: String,
    values: Value,
    #[serde(rename = "validationError", skip_serializing_if = "Option::is_none")]
    validation_error: Option<String>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Patch {
    expected_revision: String,
    values: Values,
}
fn projection(raw: &Value) -> Value {
    json!({
        "hiddenSessionDirectories":raw.get("hiddenSessionDirectories").cloned().unwrap_or_else(||json!([])),
        "sessionDirectoryIgnore":raw.get("sessionDirectoryIgnore").cloned().unwrap_or_else(||json!([]))
    })
}
fn snapshot(raw: &Value) -> Result<Snapshot, Error> {
    let values = projection(raw);
    // Return the bounded original projection even when it cannot be applied.
    // Recovery must not require deserializing or compiling the old values first.
    let validation_error = match serde_json::from_value::<Values>(values.clone()) {
        Ok(values) => values.validate().err().map(|(_, e)| e),
        Err(_) => Some("Saved visibility values must be lists of strings".into()),
    };
    Ok(Snapshot {
        revision: document::revision(&values)?,
        values,
        validation_error,
    })
}
fn save(path: &Path, patch: Patch) -> Result<Snapshot, Error> {
    if patch.expected_revision.len() != 64
        || !patch
            .expected_revision
            .bytes()
            .all(|b| b.is_ascii_hexdigit())
    {
        return Err(bad("Invalid session-visibility revision"));
    }
    patch.values.validate()?;
    let mut raw = document::read(path)?;
    if document::revision(&projection(&raw))? != patch.expected_revision {
        return Err((StatusCode::CONFLICT,"Session visibility changed in another client. Reload before saving; your edits are retained.".into()));
    }
    raw["hiddenSessionDirectories"] = json!(patch.values.hidden_session_directories);
    raw["sessionDirectoryIgnore"] = json!(patch.values.session_directory_ignore);
    let values = projection(&raw);
    let result = Snapshot {
        revision: document::revision(&values)?,
        values,
        validation_error: None,
    };
    document::write(path, &raw)?;
    Ok(result)
}
async fn operation(state: crate::GatewayState, patch: Option<Patch>) -> Response<Body> {
    match tokio::task::spawn_blocking(move || {
        let result = (|| {
            let _lock = state
                .preferences_lock
                .lock()
                .map_err(|_| bad("Settings lock is unavailable"))?;
            match patch {
                Some(p) => save(&state.preferences_path, p),
                None => snapshot(&document::read(&state.preferences_path)?),
            }
        })();
        match result {
            Ok(s) => crate::json_response(StatusCode::OK, &s),
            Err((status, e)) => crate::json_error(status, &e),
        }
    })
    .await
    {
        Ok(response) => response,
        Err(_) => crate::json_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Session-visibility worker stopped",
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
        Ok(Json(p)) => operation(state, Some(p)).await,
        Err(_) => crate::json_error(
            StatusCode::BAD_REQUEST,
            "Invalid session-visibility request",
        ),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scoped_save_keeps_unrelated_changes_and_rejects_stale_invalid_rules() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("settings.json");
        document::write(&path,&json!({"future":{"keep":true},"sharedDocumentDirectories":["/shared"],"language":"zh-CN"})).unwrap();
        let initial = snapshot(&document::read(&path).unwrap()).unwrap();
        let mut other = document::read(&path).unwrap();
        other["contentWidth"] = json!("wide");
        document::write(&path, &other).unwrap();
        let values = Values {
            hidden_session_directories: vec!["/private".into()],
            session_directory_ignore: vec![
                "# comment".into(),
                "**/build/".into(),
                "!/public/build/keep/".into(),
            ],
        };
        let saved = save(
            &path,
            Patch {
                expected_revision: initial.revision.clone(),
                values: values.clone(),
            },
        )
        .unwrap();
        assert_eq!(saved.values, json!(values));
        let raw = document::read(&path).unwrap();
        assert_eq!(raw["future"]["keep"], true);
        assert_eq!(raw["sharedDocumentDirectories"], json!(["/shared"]));
        assert_eq!(raw["contentWidth"], "wide");
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(
            save(
                &path,
                Patch {
                    expected_revision: initial.revision,
                    values: values.clone()
                }
            )
            .unwrap_err()
            .0,
            StatusCode::CONFLICT
        );
        assert!(save(
            &path,
            Patch {
                expected_revision: saved.revision,
                values: Values {
                    hidden_session_directories: vec![],
                    session_directory_ignore: vec!["bad\nrule".into()]
                }
            }
        )
        .is_err());
        assert_eq!(std::fs::read(path).unwrap(), bytes);
    }
    #[test]
    fn malformed_stored_fields_are_readable_and_repairable_without_touching_other_settings() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("settings.json");
        let raw = json!({"hiddenSessionDirectories":{"broken":true},"sessionDirectoryIgnore":["bad\u{0000}rule"],"future":{"keep":42},"language":"zh-CN"});
        document::write(&path, &raw).unwrap();
        let before = std::fs::read(&path).unwrap();
        let recovery = snapshot(&raw).unwrap();
        assert!(recovery.validation_error.is_some());
        assert_eq!(recovery.values, projection(&raw));
        assert_eq!(std::fs::read(&path).unwrap(), before);
        let valid = Values {
            hidden_session_directories: vec!["/hidden".into()],
            session_directory_ignore: vec!["**/build/".into()],
        };
        let saved = save(
            &path,
            Patch {
                expected_revision: recovery.revision.clone(),
                values: valid.clone(),
            },
        )
        .unwrap();
        assert!(saved.validation_error.is_none());
        let raw = document::read(&path).unwrap();
        assert_eq!(raw["future"]["keep"], 42);
        assert_eq!(raw["language"], "zh-CN");
        assert_eq!(
            save(
                &path,
                Patch {
                    expected_revision: recovery.revision,
                    values: valid
                }
            )
            .unwrap_err()
            .0,
            StatusCode::CONFLICT
        );
    }
    #[test]
    fn stored_compile_budget_failure_can_be_repaired_but_not_saved_again() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("settings.json");
        let values = Values {
            hidden_session_directories: vec![],
            session_directory_ignore: (0..500)
                .map(|i| format!("{i:03}{}", "a?".repeat(2046)))
                .collect(),
        };
        let raw = json!({"hiddenSessionDirectories":values.hidden_session_directories,"sessionDirectoryIgnore":values.session_directory_ignore,"future":{"keep":true}});
        document::write(&path, &raw).unwrap();
        let original = std::fs::read(&path).unwrap();
        let recovery = snapshot(&raw).unwrap();
        assert!(recovery
            .validation_error
            .as_deref()
            .is_some_and(|e| e.contains("filter budget")));
        assert!(save(
            &path,
            Patch {
                expected_revision: recovery.revision.clone(),
                values
            }
        )
        .is_err());
        assert_eq!(std::fs::read(&path).unwrap(), original);
        let repaired = save(
            &path,
            Patch {
                expected_revision: recovery.revision,
                values: Values {
                    hidden_session_directories: vec![],
                    session_directory_ignore: vec!["**/build/".into()],
                },
            },
        )
        .unwrap();
        assert!(repaired.validation_error.is_none());
        assert_eq!(document::read(&path).unwrap()["future"]["keep"], true);
    }
}
