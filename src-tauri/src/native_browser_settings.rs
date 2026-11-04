//! Revision-checked browser navigation settings; preserve unrelated/future JSON.
use crate::{
    browser_policy::{BrowserPreferences, NavigationPreferences},
    preferences_document::{self as document, Error},
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
fn bad(error: impl ToString) -> Error {
    (StatusCode::BAD_REQUEST, error.to_string())
}
const FIELDS: [&str; 5] = [
    "enabled",
    "allowHttp",
    "allowPrivateNetwork",
    "allowLocalhost",
    "previewJavascript",
];
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Patch {
    expected_revision: String,
    values: NavigationPreferences,
    #[serde(default)]
    restore_tabs: Option<bool>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    revision: String,
    values: NavigationPreferences,
    restore_tabs: bool,
}
fn projection(raw: &Value) -> Value {
    let browser = raw.get("browser").filter(|v| !v.is_null());
    let mut values = serde_json::Map::new();
    for field in FIELDS.into_iter().chain(["restoreTabs"]) {
        values.insert(
            field.into(),
            browser
                .and_then(|v| v.get(field))
                .cloned()
                .unwrap_or(Value::Null),
        );
    }
    json!({"container":if browser.is_some(){"object"}else{"default"},"values":values})
}
fn snapshot(raw: &Value) -> Result<Snapshot, Error> {
    let browser = match raw.get("browser").filter(|v| !v.is_null()) {
        Some(value) => serde_json::from_value::<BrowserPreferences>(value.clone()).map_err(bad)?,
        None => BrowserPreferences::default(),
    };
    browser.validate().map_err(bad)?;
    Ok(Snapshot {
        revision: document::revision(&projection(raw))?,
        values: NavigationPreferences::from_browser(&browser),
        restore_tabs: browser.restore_tabs,
    })
}
fn save(path: &Path, request: Patch) -> Result<Snapshot, Error> {
    if request.expected_revision.len() != 64
        || !request
            .expected_revision
            .bytes()
            .all(|c| c.is_ascii_hexdigit())
    {
        return Err(bad("Invalid browser settings revision"));
    }
    let mut raw = document::read(path)?;
    if snapshot(&raw)?.revision != request.expected_revision {
        return Err((StatusCode::CONFLICT,"Browser settings changed in another client. Reload before saving; your edits are retained.".into()));
    }
    if raw.get("browser").is_none_or(Value::is_null) {
        raw["browser"] = json!({});
    }
    let values = serde_json::to_value(request.values).map_err(bad)?;
    for field in FIELDS {
        raw["browser"][field] = values[field].clone();
    }
    if let Some(restore) = request.restore_tabs {
        let previous = raw["browser"]["restoreTabs"].as_bool().unwrap_or(false);
        if restore != previous {
            // Forget/rotate before publishing the new preference; a failed
            // settings write conservatively forgets rather than exposing tabs.
            crate::native_browser_tabs::reset(path)?;
        }
        raw["browser"]["restoreTabs"] = json!(restore);
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
            if let Some(request) = request {
                save(&state.preferences_path, request)
            } else {
                snapshot(&document::read(&state.preferences_path)?)
            }
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
            "Browser settings worker stopped",
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
        Ok(Json(request)) => operation(state, Some(request)).await,
        Err(_) => crate::json_error(StatusCode::BAD_REQUEST, "Invalid browser settings request"),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scoped_navigation_save_preserves_agent_future_fields_and_other_clients() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let mut raw = json!({"theme":"dark","browser":{"restoreTabs":true,"embeddedWidth":900,"agent":{"enabled":true,"allowedOrigins":["https://example.com"]},"future":{"keep":42}}});
        document::write(&path, &raw).unwrap();
        let original = snapshot(&raw).unwrap();
        raw["language"] = json!("zh-CN");
        raw["browser"]["agent"]["profile"] = json!("isolated");
        document::write(&path, &raw).unwrap();
        let mut values = original.values;
        values.allow_http = false;
        let saved = save(
            &path,
            Patch {
                expected_revision: original.revision.clone(),
                values: values.clone(),
                restore_tabs: None,
            },
        )
        .unwrap();
        assert_eq!(saved.values, values);
        assert_ne!(saved.revision, original.revision);
        let expected = document::read(&path).unwrap();
        assert_eq!(expected["browser"]["agent"], raw["browser"]["agent"]);
        assert_eq!(expected["browser"]["future"], raw["browser"]["future"]);
        assert_eq!(expected["browser"]["restoreTabs"], true);
        assert_eq!(expected["language"], "zh-CN");
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(
            save(
                &path,
                Patch {
                    expected_revision: original.revision,
                    values,
                    restore_tabs: None,
                }
            )
            .err()
            .unwrap()
            .0,
            StatusCode::CONFLICT
        );
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
    }
    #[test]
    fn restore_toggle_is_revision_checked_and_forgets_addresses() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        document::write(
            &path,
            &json!({"browser":{"restoreTabs":false},"unrelated":"preserved"}),
        )
        .unwrap();
        let initial = snapshot(&document::read(&path).unwrap()).unwrap();
        let request: Patch = serde_json::from_value(
            json!({"expectedRevision":initial.revision,"values":initial.values,"restoreTabs":true}),
        )
        .unwrap();
        let on = save(&path, request).unwrap();
        assert!(on.restore_tabs);
        let state_path = path.with_file_name("gnome-browser-tabs.json");
        let mut state: Value =
            serde_json::from_slice(&std::fs::read(&state_path).unwrap()).unwrap();
        state["tabs"] = json!(["https://example.com/private"]);
        std::fs::write(&state_path, serde_json::to_vec(&state).unwrap()).unwrap();
        let nonce = state["nonce"].clone();
        let off = save(
            &path,
            serde_json::from_value(
                json!({"expectedRevision":on.revision,"values":on.values,"restoreTabs":false}),
            )
            .unwrap(),
        )
        .unwrap();
        assert!(!off.restore_tabs);
        let empty: Value = serde_json::from_slice(&std::fs::read(&state_path).unwrap()).unwrap();
        assert_eq!(empty["tabs"], json!([]));
        assert_ne!(empty["nonce"], nonce);
        assert_eq!(document::read(&path).unwrap()["unrelated"], "preserved");
        assert!(serde_json::from_value::<Patch>(
            json!({"expectedRevision":off.revision,"values":off.values,"restoreTabs":"true"})
        )
        .is_err());
    }
    #[test]
    fn invalid_browser_types_are_errors_without_writing() {
        for raw in [
            json!({"browser":"invalid"}),
            json!({"browser":{"allowHttp":"yes"}}),
            json!({"browser":{"agent":{"provider":"unknown"}}}),
        ] {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("settings.json");
            document::write(&path, &raw).unwrap();
            let bytes = std::fs::read(&path).unwrap();
            assert!(snapshot(&raw).is_err());
            assert_eq!(std::fs::read(&path).unwrap(), bytes);
        }
    }
}
