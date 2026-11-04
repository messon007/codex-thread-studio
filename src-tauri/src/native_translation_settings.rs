//! Narrow, revision-checked shared translation preferences. Disk work stays off HTTP.
use crate::{
    preferences_document::{self as document, Error},
    TranslationPreferences,
};
use axum::{
    body::Body,
    extract::{rejection::JsonRejection, State},
    http::{Response, StatusCode},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::Path;
const FIELDS: [&str; 4] = ["engine", "ollamaModel", "models", "efforts"];
fn bad(error: impl ToString) -> Error {
    (StatusCode::BAD_REQUEST, error.to_string())
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Patch {
    expected_revision: String,
    values: TranslationPreferences,
}
#[derive(Serialize)]
pub struct Snapshot {
    revision: String,
    values: TranslationPreferences,
    backends: Vec<Value>,
}
fn projection(raw: &Value) -> Value {
    let t = raw.get("translation").filter(|v| !v.is_null());
    let mut values = serde_json::Map::new();
    for field in FIELDS {
        values.insert(
            field.into(),
            t.and_then(|t| t.get(field)).cloned().unwrap_or(Value::Null),
        );
    }
    json!({"container":if t.is_some(){"object"}else{"default"},"values":values})
}
pub(super) fn preferences(raw: &Value) -> Result<TranslationPreferences, Error> {
    let values = match raw.get("translation").filter(|v| !v.is_null()) {
        Some(v) => serde_json::from_value(v.clone()).map_err(bad)?,
        None => TranslationPreferences::default(),
    };
    validate(&values)?;
    Ok(values)
}
fn validate(values: &TranslationPreferences) -> Result<(), Error> {
    crate::validate_preferences(&crate::StudioPreferences {
        translation: values.clone(),
        ..Default::default()
    })
    .map_err(bad)
}
fn snapshot(raw: &Value) -> Result<Snapshot, Error> {
    Ok(Snapshot {
        revision: document::revision(&projection(raw))?,
        values: preferences(raw)?,
        backends: Vec::new(),
    })
}
fn save(path: &Path, request: Patch) -> Result<Snapshot, Error> {
    if request.expected_revision.len() != 64
        || !request
            .expected_revision
            .bytes()
            .all(|c| c.is_ascii_hexdigit())
    {
        return Err(bad("Invalid translation settings revision"));
    }
    validate(&request.values)?;
    let mut raw = document::read(path)?;
    if snapshot(&raw)?.revision != request.expected_revision {
        return Err((StatusCode::CONFLICT,"Translation settings changed in another client. Reload before saving; your edits are retained.".into()));
    }
    if raw.get("translation").is_none_or(Value::is_null) {
        raw["translation"] = json!({});
    }
    let values = serde_json::to_value(request.values).map_err(bad)?;
    for field in FIELDS {
        raw["translation"][field] = values.get(field).cloned().unwrap_or_else(|| json!({}));
    }
    let result = snapshot(&raw)?;
    document::write(path, &raw)?;
    Ok(result)
}
async fn operation(state: crate::GatewayState, request: Option<Patch>) -> Response<Body> {
    match tokio::task::spawn_blocking(move || {
        let result = (|| -> Result<Snapshot, Error> {
            let _lock = state
                .preferences_lock
                .lock()
                .map_err(|_| bad("Settings lock is unavailable"))?;
            let mut result = if let Some(request) = request {
                save(&state.preferences_path, request)?
            } else {
                snapshot(&document::read(&state.preferences_path)?)?
            };
            result.backends = state
                .backend_descriptors
                .iter()
                .map(|b| json!({"id":b.id,"name":b.name,"kind":b.kind}))
                .collect();
            Ok(result)
        })();
        match result {
            Ok(v) => crate::json_response(StatusCode::OK, &v),
            Err((status, error)) => crate::json_error(status, &error),
        }
    })
    .await
    {
        Ok(r) => r,
        Err(_) => crate::json_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Translation settings worker stopped",
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
            "Invalid translation settings request",
        ),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scoped_save_and_conflict_preserve_other_clients_and_backends() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let mut raw = json!({"translation":{"engine":"backend","ollamaModel":"gemma3:4b","models":{"codex":"original","custom":"custom-model"},"efforts":{"custom":"high"},"future":{"keep":42}},"other":{"keep":true}});
        document::write(&path, &raw).unwrap();
        let first = snapshot(&raw).unwrap();
        raw["unrelated"] = json!("concurrent");
        raw["translation"]["future"] = json!({"keep":43});
        document::write(&path, &raw).unwrap();
        let mut values = first.values;
        values
            .models
            .insert("codex".into(), "translation-model".into());
        values.efforts.insert("codex".into(), "low".into());
        let next = save(
            &path,
            Patch {
                expected_revision: first.revision.clone(),
                values: values.clone(),
            },
        )
        .unwrap();
        assert_eq!(next.values.models["custom"], "custom-model");
        let saved = document::read(&path).unwrap();
        assert_eq!(saved["translation"]["future"]["keep"], 43);
        assert_eq!(saved["unrelated"], "concurrent");
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(
            save(
                &path,
                Patch {
                    expected_revision: first.revision,
                    values
                }
            )
            .err()
            .unwrap()
            .0,
            StatusCode::CONFLICT
        );
        assert_eq!(std::fs::read(path).unwrap(), bytes);
    }
    #[test]
    fn invalid_replacements_and_map_budgets_do_not_write() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        document::write(&path, &json!({})).unwrap();
        let first = snapshot(&json!({})).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        for value in [
            json!({"engine":"remote"}),
            json!({"ollamaModel":""}),
            json!({"models":{"codex":"bad\nmodel"}}),
            json!({"efforts":{"codex":"invalid"}}),
        ] {
            let values: TranslationPreferences = serde_json::from_value(value).unwrap();
            assert!(save(
                &path,
                Patch {
                    expected_revision: first.revision.clone(),
                    values
                }
            )
            .is_err());
            assert_eq!(std::fs::read(&path).unwrap(), bytes);
        }
        let mut values = first.values;
        for i in 0..33 {
            values.models.insert(format!("backend-{i}"), "model".into());
        }
        assert!(save(
            &path,
            Patch {
                expected_revision: first.revision,
                values
            }
        )
        .is_err());
        assert_eq!(std::fs::read(path).unwrap(), bytes);
    }
}
