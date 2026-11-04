//! Narrow shared desktop-notification preference; file work runs on Rust workers.
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
#[derive(Serialize, Debug)]
struct Snapshot {
    revision: String,
    enabled: Option<bool>,
    #[serde(rename = "validationError", skip_serializing_if = "Option::is_none")]
    validation_error: Option<&'static str>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Patch {
    expected_revision: String,
    enabled: bool,
}
fn projection(raw: &Value) -> Value {
    raw.get("desktopNotifications")
        .cloned()
        .unwrap_or(json!(false))
}
fn snapshot(raw: &Value) -> Result<Snapshot, Error> {
    let value = projection(raw);
    let enabled = value.as_bool();
    Ok(Snapshot {
        revision: document::revision(&value)?,
        enabled,
        validation_error: enabled
            .is_none()
            .then_some("Saved desktopNotifications must be a boolean"),
    })
}
fn save(path: &Path, patch: Patch) -> Result<Snapshot, Error> {
    if patch.expected_revision.len() != 64
        || !patch
            .expected_revision
            .bytes()
            .all(|b| b.is_ascii_hexdigit())
    {
        return Err((
            StatusCode::BAD_REQUEST,
            "Invalid notification-settings revision".into(),
        ));
    }
    let mut raw = document::read(path)?;
    if snapshot(&raw)?.revision != patch.expected_revision {
        return Err((
            StatusCode::CONFLICT,
            "Notification settings changed in another client. Reload before changing them.".into(),
        ));
    }
    raw["desktopNotifications"] = json!(patch.enabled);
    let saved = snapshot(&raw)?;
    document::write(path, &raw)?;
    Ok(saved)
}
async fn operation(state: crate::GatewayState, patch: Option<Patch>) -> Response<Body> {
    match tokio::task::spawn_blocking(move || {
        let result = (|| {
            let _lock = state.preferences_lock.lock().map_err(|_| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Settings lock is unavailable".into(),
                )
            })?;
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
            "Notification settings worker stopped",
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
            "Invalid notification settings request",
        ),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scoped_toggle_preserves_unrelated_changes_and_repairs_only_the_boolean() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("settings.json");
        let raw = json!({"future":{"keep":true},"desktopNotifications":{"broken":true}});
        document::write(&path, &raw).unwrap();
        let first = snapshot(&raw).unwrap();
        assert!(first.enabled.is_none());
        let mut other = raw;
        other["language"] = json!("zh-CN");
        document::write(&path, &other).unwrap();
        let saved = save(
            &path,
            Patch {
                expected_revision: first.revision.clone(),
                enabled: true,
            },
        )
        .unwrap();
        assert_eq!(saved.enabled, Some(true));
        let raw = document::read(&path).unwrap();
        assert_eq!(raw["future"]["keep"], true);
        assert_eq!(raw["language"], "zh-CN");
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(
            save(
                &path,
                Patch {
                    expected_revision: first.revision,
                    enabled: false
                }
            )
            .unwrap_err()
            .0,
            StatusCode::CONFLICT
        );
        assert_eq!(std::fs::read(path).unwrap(), bytes);
    }
}
