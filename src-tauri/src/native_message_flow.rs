//! Revision-checked shared composer preferences; no provider work.
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

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Values {
    queue_depth: u8,
    continue_behavior: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Patch {
    expected_revision: String,
    values: Values,
}
#[derive(Serialize)]
pub struct Snapshot {
    revision: String,
    values: Values,
}
fn bad(message: impl ToString) -> Error {
    (StatusCode::BAD_REQUEST, message.to_string())
}
fn projection(raw: &Value) -> Value {
    json!({"queueDepth":raw.get("queueDepth"),"continueBehavior":raw.get("continueBehavior")})
}
fn validate(values: &Values) -> Result<(), Error> {
    if !(1..=3).contains(&values.queue_depth)
        || !matches!(
            values.continue_behavior.as_str(),
            "sessionModelDraft" | "ollamaDraft" | "quickSend"
        )
    {
        return Err(bad("Invalid message-flow settings"));
    }
    Ok(())
}
fn snapshot(raw: &Value) -> Result<Snapshot, Error> {
    let depth = raw
        .get("queueDepth")
        .filter(|v| !v.is_null())
        .map(|v| {
            v.as_u64()
                .filter(|n| (1..=3).contains(n))
                .ok_or_else(|| bad("Invalid message queue depth"))
        })
        .transpose()?
        .unwrap_or(1);
    let behavior = raw
        .get("continueBehavior")
        .filter(|v| !v.is_null())
        .map(|v| v.as_str().ok_or_else(|| bad("Invalid Continue behavior")))
        .transpose()?
        .unwrap_or("sessionModelDraft");
    let values = Values {
        queue_depth: depth as u8,
        continue_behavior: behavior.into(),
    };
    validate(&values)?;
    Ok(Snapshot {
        revision: document::revision(&projection(raw))?,
        values,
    })
}
fn save(path: &Path, patch: Patch) -> Result<Snapshot, Error> {
    validate(&patch.values)?;
    let mut raw = document::read(path)?;
    if patch.expected_revision.len() != 64
        || !patch
            .expected_revision
            .bytes()
            .all(|b| b.is_ascii_hexdigit())
    {
        return Err(bad("Invalid message-flow settings revision"));
    }
    if document::revision(&projection(&raw))? != patch.expected_revision {
        return Err((StatusCode::CONFLICT,"Message-flow settings changed in another client. Reload before saving; your edits are retained.".into()));
    }
    let original = document::revision(&raw)?;
    raw["queueDepth"] = json!(patch.values.queue_depth);
    raw["continueBehavior"] = json!(patch.values.continue_behavior);
    let next = snapshot(&raw)?;
    let bytes = serde_json::to_vec_pretty(&raw).map_err(bad)?;
    if bytes.len() > 2 * 1024 * 1024 {
        return Err(bad("Studio preferences exceed the settings budget"));
    }
    document::write_atomic_checked(path, &bytes, || {
        if document::revision(&document::read(path)?)? != original {
            return Err((
                StatusCode::CONFLICT,
                "Message-flow settings changed before saving; reload and retry".into(),
            ));
        }
        Ok(())
    })?;
    Ok(next)
}
async fn operation(state: crate::GatewayState, patch: Option<Patch>) -> Response<Body> {
    match tokio::task::spawn_blocking(move || {
        let run = || -> Result<Snapshot, Error> {
            let _lock = state
                .preferences_lock
                .lock()
                .map_err(|_| bad("Settings lock is unavailable"))?;
            if let Some(patch) = patch {
                save(&state.preferences_path, patch)
            } else {
                snapshot(&document::read(&state.preferences_path)?)
            }
        };
        match run() {
            Ok(value) => crate::json_response(StatusCode::OK, &value),
            Err((status, error)) => crate::json_error(status, &error),
        }
    })
    .await
    {
        Ok(response) => response,
        Err(_) => crate::json_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Message-flow settings worker stopped",
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
            "Invalid message-flow settings request",
        ),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn defaults_scoped_updates_and_conflicts_preserve_other_preferences() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        document::write(&path, &json!({"future":{"keep":true}})).unwrap();
        let first = snapshot(&document::read(&path).unwrap()).unwrap();
        assert_eq!(first.values.queue_depth, 1);
        assert_eq!(first.values.continue_behavior, "sessionModelDraft");
        document::write(
            &path,
            &json!({"future":{"keep":"changed"},"translation":{"ollamaModel":"local"}}),
        )
        .unwrap();
        let next = save(
            &path,
            Patch {
                expected_revision: first.revision.clone(),
                values: Values {
                    queue_depth: 3,
                    continue_behavior: "ollamaDraft".into(),
                },
            },
        )
        .unwrap();
        let saved = document::read(&path).unwrap();
        assert_eq!(saved["future"]["keep"], "changed");
        assert_eq!(saved["translation"]["ollamaModel"], "local");
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(
            save(
                &path,
                Patch {
                    expected_revision: first.revision,
                    values: next.values
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
    fn malformed_values_and_revisions_cannot_replace_valid_settings() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        document::write(&path, &json!({})).unwrap();
        let first = snapshot(&json!({})).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        for (depth, behavior) in [
            (0, "quickSend"),
            (4, "sessionModelDraft"),
            (1, "automaticAgent"),
        ] {
            assert!(save(
                &path,
                Patch {
                    expected_revision: first.revision.clone(),
                    values: Values {
                        queue_depth: depth,
                        continue_behavior: behavior.into()
                    }
                }
            )
            .is_err());
        }
        assert!(save(
            &path,
            Patch {
                expected_revision: "invalid".into(),
                values: first.values
            }
        )
        .is_err());
        for raw in [
            json!({"queueDepth":-1}),
            json!({"queueDepth":1.5}),
            json!({"continueBehavior":true}),
            json!({"continueBehavior":"unknown"}),
        ] {
            assert!(snapshot(&raw).is_err());
        }
        assert_eq!(std::fs::read(path).unwrap(), bytes);
    }
}
