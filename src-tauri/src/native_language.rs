//! Narrow interface-language transaction shared with all Studio clients.
use crate::preferences_document::{read, revision, write, Error};
use axum::{
    body::Body,
    extract::{rejection::JsonRejection, State},
    http::{Response, StatusCode},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
#[derive(Serialize)]
struct Snapshot {
    revision: String,
    language: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Patch {
    expected_revision: String,
    language: String,
}
fn snapshot(raw: &Value) -> Result<Snapshot, Error> {
    let language = match raw.get("language") {
        None | Some(Value::Null) => "system",
        Some(Value::String(value)) if matches!(value.as_str(), "system" | "zh-CN" | "en-US") => {
            value
        }
        _ => {
            return Err((
                StatusCode::BAD_REQUEST,
                "Invalid interface language preference".into(),
            ))
        }
    };
    Ok(Snapshot {
        revision: revision(&json!({"language":language}))?,
        language: language.into(),
    })
}
fn save(path: &std::path::Path, patch: Patch) -> Result<Snapshot, Error> {
    if !matches!(patch.language.as_str(), "system" | "zh-CN" | "en-US") {
        return Err((
            StatusCode::BAD_REQUEST,
            "Language must be system, zh-CN, or en-US".into(),
        ));
    }
    let mut raw = read(path)?;
    if snapshot(&raw)?.revision != patch.expected_revision {
        return Err((
            StatusCode::CONFLICT,
            "Language changed in another client. Reload before saving; your choice is retained."
                .into(),
        ));
    }
    raw["language"] = json!(patch.language);
    let result = snapshot(&raw)?;
    write(path, &raw)?;
    Ok(result)
}
async fn operation(state: crate::GatewayState, patch: Option<Patch>) -> Response<Body> {
    match tokio::task::spawn_blocking(move || {
        let _lock = state.preferences_lock.lock().map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Settings lock is unavailable".to_string(),
            )
        })?;
        match patch {
            Some(patch) => save(&state.preferences_path, patch),
            None => snapshot(&read(&state.preferences_path)?),
        }
    })
    .await
    {
        Ok(Ok(value)) => crate::json_response(StatusCode::OK, &value),
        Ok(Err((status, error))) => crate::json_error(status, &error),
        Err(_) => crate::json_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Language settings worker stopped",
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
        Err(_) => crate::json_error(StatusCode::BAD_REQUEST, "Invalid language settings request"),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn language_write_preserves_other_fields_and_rejects_stale_choices() {
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("preferences.json");
        let raw = json!({"language":"system","future":{"keep":[1,2]},"browser":{"future":true},"typography":{"untouched":true}});
        write(&path, &raw).unwrap();
        let base = snapshot(&raw).unwrap();
        let mut other = raw.clone();
        other["anotherClient"] = json!({"keep":true});
        write(&path, &other).unwrap();
        let saved = save(
            &path,
            Patch {
                expected_revision: base.revision.clone(),
                language: "zh-CN".into(),
            },
        )
        .unwrap();
        other["language"] = json!("zh-CN");
        assert_eq!(read(&path).unwrap(), other);
        assert_eq!(
            save(
                &path,
                Patch {
                    expected_revision: base.revision,
                    language: "en-US".into()
                }
            )
            .err()
            .unwrap()
            .0,
            StatusCode::CONFLICT
        );
        assert_eq!(read(&path).unwrap(), other);
        assert_eq!(
            save(
                &path,
                Patch {
                    expected_revision: saved.revision,
                    language: "invalid".into()
                }
            )
            .err()
            .unwrap()
            .0,
            StatusCode::BAD_REQUEST
        );
        assert_eq!(read(&path).unwrap(), other);
        assert_eq!(snapshot(&json!({})).unwrap().language, "system");
        assert_eq!(
            snapshot(&json!({"language":null})).unwrap().language,
            "system"
        );
        assert!(snapshot(&json!({"language":42})).is_err());
        assert!(serde_json::from_value::<Patch>(
            json!({"expectedRevision":"x","language":"system","extra":true})
        )
        .is_err());
    }
}
