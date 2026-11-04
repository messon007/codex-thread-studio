//! Narrow, conflict-checked session information updates in the shared opening row.
//! Unknown capture fields survive both native editors and Router responsibility saves.
use crate::{json_error, json_response, GatewayState, OpeningMessage, StudioPreferences};
use axum::{
    body::Body,
    extract::{rejection::JsonRejection, Query, State},
    http::{Response, StatusCode},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::BTreeMap, path::Path};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Read {
    session_key: String,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Edit {
    pub session_key: String,
    pub expected_revision: String,
    pub question: Option<String>,
    pub responsibility: Option<String>,
    #[serde(default)]
    pub captured_at: String,
    #[serde(default)]
    pub capture_only: bool,
    #[serde(default)]
    pub truncated: bool,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Snapshot {
    pub session_key: String,
    pub revision: String,
    pub message: Option<OpeningMessage>,
    pub responsibility: String,
}
fn valid_key(key: &str) -> Result<(), String> {
    if key.len() > 320 || !crate::valid_router_session_key(key) {
        Err("Invalid session information key".into())
    } else {
        Ok(())
    }
}
fn read_raw(
    path: &Path,
    key: &str,
) -> Result<(Option<String>, Option<Value>, Option<OpeningMessage>), String> {
    valid_key(key)?;
    let raw = crate::session_state::opening_message_raw(path, key)?;
    let value: Option<Value> = raw
        .as_ref()
        .map(|s| serde_json::from_str(s).map_err(|e| format!("Invalid opening message: {e}")))
        .transpose()?;
    if value.as_ref().is_some_and(|v| !v.is_object()) {
        return Err("Invalid opening message object".into());
    }
    let message = value
        .as_ref()
        .map(|v| {
            serde_json::from_value(v.clone()).map_err(|e| format!("Invalid opening message: {e}"))
        })
        .transpose()?;
    Ok((raw, value, message))
}
fn snapshot(key: &str, message: Option<OpeningMessage>, legacy: &str) -> Result<Snapshot, String> {
    Ok(Snapshot {
        session_key: key.into(),
        revision: crate::native_router::revision(&message)?,
        responsibility: message
            .as_ref()
            .map_or_else(|| legacy.into(), |m| m.responsibility.clone()),
        message,
    })
}
fn read(path: &Path, key: &str, legacy: &str) -> Result<Snapshot, String> {
    snapshot(key, read_raw(path, key)?.2, legacy)
}
pub(crate) fn legacy_role(preferences: &StudioPreferences, key: &str) -> String {
    preferences
        .router
        .as_ref()
        .and_then(|r| {
            r.responsibilities.get(key).or_else(|| {
                key.strip_prefix("codex:")
                    .and_then(|id| r.responsibilities.get(id))
            })
        })
        .map(|r| r.description.clone())
        .unwrap_or_default()
}
pub(crate) fn update(path: &Path, request: Edit, legacy: &str) -> Result<Snapshot, String> {
    valid_key(&request.session_key)?;
    if request.question.is_none() && request.responsibility.is_none() {
        return Err("No session information changes were supplied".into());
    }
    if request
        .question
        .as_ref()
        .is_some_and(|q| q.len() > 16 * 1024)
    {
        return Err("Opening questions support at most 16 KiB of UTF-8 text".into());
    }
    if request
        .responsibility
        .as_ref()
        .is_some_and(|r| r.chars().count() > 4096)
    {
        return Err("Responsibilities support at most 4,096 characters".into());
    }
    if request.question.is_some()
        && (request.captured_at.is_empty()
            || request.captured_at.len() > 128
            || request.captured_at.chars().any(char::is_control))
    {
        return Err("Invalid opening question capture timestamp".into());
    }
    if request.capture_only && (request.question.is_none() || request.responsibility.is_some()) {
        return Err("History capture may only supply an opening question".into());
    }
    let (previous, raw, current) = read_raw(path, &request.session_key)?;
    // A racing manual/history capture wins over automatic identification.
    if request.capture_only && current.as_ref().is_some_and(|m| !m.text.is_empty()) {
        return snapshot(&request.session_key, current, legacy);
    }
    if crate::native_router::revision(&current)? != request.expected_revision {
        return Err("The opening message or responsibility changed; reload before saving".into());
    }
    let mut message = current.unwrap_or_else(|| OpeningMessage {
        responsibility: legacy.into(),
        ..Default::default()
    });
    let mut updated = raw.unwrap_or(serde_json::to_value(&message).map_err(|e| e.to_string())?);
    if let Some(question) = request.question {
        if message.text != question || request.capture_only {
            message.text = question;
            message.source = if request.capture_only {
                "history"
            } else {
                "manual"
            }
            .into();
            message.captured_at = request.captured_at;
            message.truncated = request.capture_only && request.truncated;
            updated["text"] = message.text.clone().into();
            updated["source"] = message.source.clone().into();
            updated["capturedAt"] = message.captured_at.clone().into();
            updated["truncated"] = message.truncated.into();
        }
    }
    if let Some(responsibility) = request.responsibility {
        message.responsibility = responsibility;
        updated["responsibility"] = message.responsibility.clone().into();
    }
    crate::validate_preferences(&StudioPreferences {
        opening_messages: BTreeMap::from([(request.session_key.clone(), message.clone())]),
        ..Default::default()
    })?;
    let replacement = serde_json::to_string(&updated).map_err(|e| e.to_string())?;
    crate::session_state::replace_opening_message(
        path,
        &request.session_key,
        previous.as_deref(),
        &replacement,
    )?;
    snapshot(&request.session_key, Some(message), legacy)
}
pub async fn get(State(state): State<GatewayState>, Query(request): Query<Read>) -> Response<Body> {
    match tokio::task::spawn_blocking(move || {
        let _preferences = state
            .preferences_lock
            .lock()
            .map_err(|_| "Preferences lock is unavailable")?;
        let _studio = state
            .studio_lock
            .lock()
            .map_err(|_| "Studio database lock is unavailable")?;
        let preferences = crate::load_preferences(&state.preferences_path)?;
        read(
            &state.studio_path,
            &request.session_key,
            &legacy_role(&preferences, &request.session_key),
        )
    })
    .await
    {
        Ok(Ok(value)) => json_response(StatusCode::OK, &value),
        Ok(Err(error)) => json_error(StatusCode::BAD_REQUEST, &error),
        Err(_) => json_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Session information worker stopped",
        ),
    }
}
pub async fn patch(
    State(state): State<GatewayState>,
    body: Result<Json<Edit>, JsonRejection>,
) -> Response<Body> {
    let request = match body {
        Ok(Json(request)) => request,
        Err(_) => {
            return json_error(
                StatusCode::BAD_REQUEST,
                "Invalid session information update",
            )
        }
    };
    match tokio::task::spawn_blocking(move||->Result<Snapshot,String> {
        let _preferences=state.preferences_lock.lock().map_err(|_|"Preferences lock is unavailable")?;
        let _studio=state.studio_lock.lock().map_err(|_|"Studio database lock is unavailable")?;
        let preferences=crate::load_preferences(&state.preferences_path)?;
        let role=request.responsibility.is_some();
        let legacy=legacy_role(&preferences,&request.session_key);
        let value=update(&state.studio_path,request,&legacy)?;
        if role {crate::native_router::synchronize_responsibility(&state.preferences_path,&value.session_key,&value.responsibility).map_err(|e|format!("Session information saved; legacy settings could not be synchronized. Reload before retrying: {e}"))?;}
        Ok(value)
    }).await {
        Ok(Ok(value))=>json_response(StatusCode::OK,&value),Ok(Err(error))=>json_error(if error.contains("changed; reload"){StatusCode::CONFLICT}else if error.contains("saved; legacy"){StatusCode::INTERNAL_SERVER_ERROR}else{StatusCode::BAD_REQUEST},&error),Err(_)=>json_error(StatusCode::INTERNAL_SERVER_ERROR,"Session information worker stopped"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn edit(revision: String) -> Edit {
        Edit {
            session_key: "codex:target".into(),
            expected_revision: revision,
            question: None,
            responsibility: None,
            captured_at: "2026-11-03T00:00:00Z".into(),
            capture_only: false,
            truncated: false,
        }
    }
    #[test]
    fn full_edit_preserves_future_fields_and_rejects_stale_cross_client_writes() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("studio.sqlite3");
        crate::session_state::initialize(&path).unwrap();
        let raw=json!({"text":"old","responsibility":"role","source":"capture","capturedAt":"old date","truncated":true,"futureCapture":{"range":[1,2]}}).to_string();
        crate::session_state::replace_opening_message(&path, "codex:target", None, &raw).unwrap();
        let old = read(&path, "codex:target", "").unwrap();
        let mut request = edit(old.revision.clone());
        request.question = Some("新问题😀".into());
        request.responsibility = Some("new role".into());
        let saved = update(&path, request, "").unwrap();
        assert_ne!(old.revision, saved.revision);
        let raw: Value = serde_json::from_str(
            &crate::session_state::opening_message_raw(&path, "codex:target")
                .unwrap()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(raw["futureCapture"], json!({"range":[1,2]}));
        assert_eq!(raw["text"], "新问题😀");
        assert_eq!(raw["responsibility"], "new role");
        assert_eq!(raw["source"], "manual");
        assert_eq!(raw["truncated"], false);
        let mut stale = edit(old.revision);
        stale.responsibility = Some("overwrite".into());
        assert!(update(&path, stale, "")
            .unwrap_err()
            .contains("changed; reload"));
        let mut role = edit(saved.revision);
        role.responsibility = Some("narrow role".into());
        let value = update(&path, role, "").unwrap().message.unwrap();
        assert_eq!(value.text, "新问题😀");
        assert_eq!(value.captured_at, "2026-11-03T00:00:00Z");
    }
    #[test]
    fn automatic_capture_preserves_legacy_role_and_never_overwrites_an_identified_question() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("studio.sqlite3");
        crate::session_state::initialize(&path).unwrap();
        let initial = read(&path, "codex:target", "legacy role").unwrap();
        assert_eq!(initial.responsibility, "legacy role");
        let mut request = edit(initial.revision.clone());
        request.question = Some("first question".into());
        request.capture_only = true;
        request.truncated = true;
        let saved = update(&path, request, "legacy role").unwrap();
        let message = saved.message.unwrap();
        assert_eq!(message.responsibility, "legacy role");
        assert_eq!(message.source, "history");
        assert!(message.truncated);
        let mut late = edit(initial.revision);
        late.question = Some("wrong later question".into());
        late.capture_only = true;
        assert_eq!(
            update(&path, late, "").unwrap().message.unwrap().text,
            "first question"
        );
    }
    #[test]
    fn invalid_fields_are_rejected_before_creating_a_row() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("studio.sqlite3");
        crate::session_state::initialize(&path).unwrap();
        let revision = read(&path, "codex:target", "").unwrap().revision;
        let mut invalid = edit(revision.clone());
        invalid.question = Some("中".repeat(6000));
        assert!(update(&path, invalid, "").is_err());
        let mut invalid = edit(revision);
        invalid.responsibility = Some("😀".repeat(4097));
        assert!(update(&path, invalid, "").is_err());
        assert!(
            crate::session_state::opening_message_raw(&path, "codex:target")
                .unwrap()
                .is_none()
        );
    }
}
