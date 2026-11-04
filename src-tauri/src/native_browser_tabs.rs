//! Opt-in address restoration. All I/O runs under the preferences lock on a worker.
use crate::{
    browser_policy::{validate_browser_url, BrowserPreferences},
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
use std::{
    io::Read,
    path::{Path, PathBuf},
};
const MAX_BYTES: usize = 128 * 1024;
const MAX_TABS: usize = 8;
const MAX_URL: usize = 8192;
fn bad(error: impl ToString) -> Error {
    (StatusCode::BAD_REQUEST, error.to_string())
}
fn path(settings: &Path) -> PathBuf {
    settings.with_file_name("gnome-browser-tabs.json")
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Tabs {
    pub tabs: Vec<String>,
    pub active: usize,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Put {
    expected_revision: String,
    tabs: Vec<String>,
    active: usize,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    revision: String,
    preferences: BrowserPreferences,
    #[serde(flatten)]
    selection: Tabs,
}
fn read(settings: &Path) -> Result<Value, Error> {
    let mut bytes = Vec::new();
    match std::fs::File::open(path(settings)) {
        Ok(file) => {
            file.take(MAX_BYTES as u64 + 1)
                .read_to_end(&mut bytes)
                .map_err(bad)?;
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(json!({})),
        Err(e) => return Err(bad(e)),
    }
    if bytes.len() > MAX_BYTES {
        return Err(bad("Saved browser tabs exceed the address budget"));
    }
    let raw: Value = serde_json::from_slice(&bytes).map_err(bad)?;
    if raw["version"] != 1
        || raw["nonce"].as_str().is_none()
        || raw.as_object().is_none_or(|o| {
            o.keys()
                .any(|k| !["version", "nonce", "tabs", "active"].contains(&k.as_str()))
        })
    {
        return Err(bad("Unsupported saved browser tabs document"));
    }
    selection(&raw)?;
    Ok(raw)
}
fn selection(raw: &Value) -> Result<Tabs, Error> {
    if raw.as_object().is_some_and(|o| o.is_empty()) {
        return Ok(Tabs::default());
    }
    let tabs: Tabs =
        serde_json::from_value(json!({"tabs":raw["tabs"],"active":raw["active"]})).map_err(bad)?;
    if tabs.tabs.len() > MAX_TABS
        || tabs.tabs.iter().any(|u| u.len() > MAX_URL)
        || tabs.active >= tabs.tabs.len().max(1)
    {
        return Err(bad("Invalid saved browser tabs"));
    }
    Ok(tabs)
}
fn write(settings: &Path, tabs: &Tabs) -> Result<(), Error> {
    let raw = json!({"version":1,"nonce":uuid::Uuid::new_v4().to_string(),"tabs":tabs.tabs,"active":tabs.active});
    let bytes = serde_json::to_vec(&raw).map_err(bad)?;
    if bytes.len() > MAX_BYTES {
        return Err(bad("Saved browser tabs exceed the address budget"));
    }
    document::write_atomic(&path(settings), &bytes)
}
/// Rotate the generation even for an empty session: an old request cannot cross
/// an off/on cycle and repopulate forgotten addresses (ABA protection).
pub fn reset(settings: &Path) -> Result<(), Error> {
    read(settings)?;
    write(settings, &Tabs::default())
}
fn preferences(settings: &Path) -> Result<BrowserPreferences, Error> {
    let raw = document::read(settings)?;
    let p = match raw.get("browser").filter(|v| !v.is_null()) {
        Some(v) => serde_json::from_value(v.clone()).map_err(bad)?,
        None => BrowserPreferences::default(),
    };
    p.validate().map_err(bad)?;
    Ok(p)
}
fn canonical(input: &str, p: &BrowserPreferences) -> Result<String, Error> {
    if input.len() > MAX_URL {
        return Err(bad("Browser address is too long to restore"));
    }
    let value = validate_browser_url(input, p).map_err(bad)?.to_string();
    if value.len() > MAX_URL {
        return Err(bad("Browser address is too long to restore"));
    }
    Ok(value)
}
fn allowed(tabs: Tabs, p: &BrowserPreferences) -> Tabs {
    if !p.restore_tabs || !p.enabled {
        return Tabs::default();
    }
    let mut result = Tabs::default();
    for (index, url) in tabs.tabs.iter().enumerate() {
        if let Ok(url) = canonical(url, p) {
            if index <= tabs.active {
                result.active = result.tabs.len();
            }
            result.tabs.push(url);
        }
    }
    result.active = result.active.min(result.tabs.len().saturating_sub(1));
    result
}
fn operation(settings: &Path, request: Option<Put>) -> Result<Snapshot, Error> {
    let p = preferences(settings)?;
    let mut raw = read(settings)?;
    // Also forget a preference changed by another client. No disabled late save
    // may recreate addresses. Reads never start a content engine.
    if !p.restore_tabs && !selection(&raw)?.tabs.is_empty() {
        reset(settings)?;
        raw = read(settings)?;
    }
    let revision = document::revision(&raw)?;
    if let Some(request) = request {
        if request.expected_revision != revision {
            return Err((
                StatusCode::CONFLICT,
                "Saved browser tabs changed. Reopen Browser before saving them again.".into(),
            ));
        }
        if !p.restore_tabs {
            return Err((
                StatusCode::CONFLICT,
                "Browser tab restoration is disabled".into(),
            ));
        }
        let tabs = Tabs {
            tabs: request.tabs,
            active: request.active,
        };
        if tabs.tabs.len() > MAX_TABS || tabs.active >= tabs.tabs.len().max(1) {
            return Err(bad("Invalid saved browser tabs"));
        }
        let canonical_tabs = tabs
            .tabs
            .iter()
            .map(|u| canonical(u, &p))
            .collect::<Result<Vec<_>, _>>()?;
        let tabs = Tabs {
            tabs: canonical_tabs,
            active: tabs.active,
        };
        if selection(&raw)? != tabs {
            write(settings, &tabs)?;
            raw = read(settings)?;
        }
    }
    Ok(Snapshot {
        revision: document::revision(&raw)?,
        selection: allowed(selection(&raw)?, &p),
        preferences: p,
    })
}
async fn run(state: crate::GatewayState, request: Option<Put>) -> Response<Body> {
    match tokio::task::spawn_blocking(move || {
        let result = (|| {
            let _lock = state
                .preferences_lock
                .lock()
                .map_err(|_| bad("Settings lock is unavailable"))?;
            operation(&state.preferences_path, request)
        })();
        match result {
            Ok(snapshot) => crate::json_response(StatusCode::OK, &snapshot),
            Err((status, error)) => crate::json_error(status, &error),
        }
    })
    .await
    {
        Ok(response) => response,
        Err(_) => crate::json_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Browser tab worker stopped",
        ),
    }
}
pub async fn get(State(state): State<crate::GatewayState>) -> Response<Body> {
    run(state, None).await
}
pub async fn put(
    State(state): State<crate::GatewayState>,
    body: Result<Json<Put>, JsonRejection>,
) -> Response<Body> {
    match body {
        Ok(Json(request)) => run(state, Some(request)).await,
        Err(_) => crate::json_error(
            StatusCode::BAD_REQUEST,
            "Invalid saved browser tabs request",
        ),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn settings(path: &Path, restore: bool) {
        document::write(path, &json!({"browser":{"restoreTabs":restore}})).unwrap();
    }
    fn put(path: &Path, revision: String, tabs: &[&str], active: usize) -> Result<Snapshot, Error> {
        operation(
            path,
            Some(Put {
                expected_revision: revision,
                tabs: tabs.iter().map(|s| s.to_string()).collect(),
                active,
            }),
        )
    }
    #[test]
    fn opt_in_atomic_addresses_revision_and_active_policy_filter() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("settings.json");
        settings(&p, true);
        let first = operation(&p, None).unwrap();
        assert!(!path(&p).exists());
        let saved = put(
            &p,
            first.revision.clone(),
            &[
                "https://example.com/你好",
                "http://localhost:3131/one",
                "https://example.org",
            ],
            1,
        )
        .unwrap();
        assert_eq!(
            saved.selection.tabs[0],
            "https://example.com/%E4%BD%A0%E5%A5%BD"
        );
        assert_eq!(saved.selection.active, 1);
        assert_eq!(
            put(&p, first.revision, &[], 0).err().unwrap().0,
            StatusCode::CONFLICT
        );
        let raw = document::read(&p).unwrap();
        let mut blocked = raw;
        blocked["browser"]["allowLocalhost"] = json!(false);
        document::write(&p, &blocked).unwrap();
        let filtered = operation(&p, None).unwrap();
        assert_eq!(filtered.selection.tabs.len(), 2);
        assert_eq!(filtered.selection.active, 0);
        assert!(put(&p, saved.revision, &["http://localhost:3131/one"], 0).is_err());
    }
    #[test]
    fn forget_and_generation_protect_against_late_saves_across_off_on() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("settings.json");
        settings(&p, true);
        let old = operation(&p, None).unwrap();
        reset(&p).unwrap();
        settings(&p, false);
        assert!(put(&p, old.revision.clone(), &["https://example.com"], 0).is_err());
        reset(&p).unwrap();
        settings(&p, true);
        assert_eq!(
            put(&p, old.revision, &["https://example.com"], 0)
                .err()
                .unwrap()
                .0,
            StatusCode::CONFLICT
        );
        let saved = put(
            &p,
            operation(&p, None).unwrap().revision,
            &["https://example.com"],
            0,
        )
        .unwrap();
        settings(&p, false);
        assert!(operation(&p, None).unwrap().selection.tabs.is_empty());
        assert!(!std::fs::read_to_string(path(&p))
            .unwrap()
            .contains("example.com"));
        settings(&p, true);
        assert!(put(&p, saved.revision, &["https://example.com"], 0).is_err());
    }
    #[test]
    fn malformed_oversized_future_and_invalid_urls_preserve_store() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("settings.json");
        settings(&p, true);
        let initial = operation(&p, None).unwrap();
        for u in [
            "file:///etc/passwd",
            "javascript:alert(1)",
            "https://user:secret@example.com/",
            "https://192.168.0.1/",
        ] {
            assert!(put(&p, initial.revision.clone(), &[u], 0).is_err());
        }
        assert!(put(&p, initial.revision.clone(), &["https://example.com"; 9], 0).is_err());
        assert!(put(&p, initial.revision.clone(), &["https://example.com"], 1).is_err());
        let huge = format!("https://example.com/{}", "中".repeat(1000));
        assert!(put(&p, initial.revision, &[&huge], 0).is_err());
        assert!(!path(&p).exists());
        for bytes in [
            b"{oops".to_vec(),
            serde_json::to_vec(&json!({"version":2})).unwrap(),
            vec![b' '; MAX_BYTES + 1],
        ] {
            std::fs::write(path(&p), &bytes).unwrap();
            assert!(operation(&p, None).is_err());
            assert!(reset(&p).is_err());
            assert_eq!(std::fs::read(path(&p)).unwrap(), bytes);
        }
    }
}
