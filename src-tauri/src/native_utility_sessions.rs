//! Gateway-owned temporary OpenCode sessions. Catalogs share exact ownership
//! filtering and retain the cursor of the original, unfiltered upstream page.
use crate::{opencode_server::RouterOperation as Op, GatewayState};
use axum::{
    body::Body,
    extract::{rejection::JsonRejection, State},
    http::{Response, StatusCode},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::{Arc, Mutex},
    time::Duration,
};

const MAX_ACTIVE: usize = 4;
const MAX_RETIRED: usize = 256;
type Error = (StatusCode, String);
type Result<T> = std::result::Result<T, Error>;
fn bad(message: impl Into<String>) -> Error {
    (StatusCode::BAD_REQUEST, message.into())
}
fn upstream(message: impl Into<String>) -> Error {
    (StatusCode::BAD_GATEWAY, message.into())
}
fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 256
        && !matches!(id, "." | "..")
        && !id.chars().any(char::is_control)
}

#[derive(Clone, Debug)]
struct Record {
    lease: String,
    backend: String,
    source: String,
    cwd: String,
    name: String,
    id: Option<String>,
    // Filtering evidence only. An event/catalog observation never authorizes
    // prompting, interruption or cleanup without the SDK creation reply.
    observed_id: Option<String>,
    closing: bool,
    result: Option<std::result::Result<(), String>>,
    hidden: bool,
}
#[derive(Default)]
pub struct Visibility {
    ids: HashMap<String, HashSet<String>>,
    events: HashMap<String, HashSet<String>>,
    retired: HashMap<String, HashMap<String, Vec<(String, String)>>>,
    pending: Vec<Record>,
}
impl Visibility {
    pub fn hidden(&self, backend: &str, row: &Value) -> bool {
        let Some(id) = row["id"].as_str() else {
            return false;
        };
        self.ids.get(backend).is_some_and(|ids| ids.contains(id))
            || self
                .retired
                .get(backend)
                .and_then(|ids| ids.get(id))
                .is_some_and(|owners| {
                    owners.iter().any(|(name, cwd)| {
                        row["title"].as_str().or_else(|| row["name"].as_str())
                            == Some(name.as_str())
                            && row["directory"].as_str().or_else(|| row["cwd"].as_str())
                                == Some(cwd.as_str())
                    })
                })
            || self.pending.iter().any(|record| {
                record.backend == backend
                    && record.source != id
                    && row["title"].as_str().or_else(|| row["name"].as_str())
                        == Some(record.name.as_str())
                    && row["directory"].as_str().or_else(|| row["cwd"].as_str())
                        == Some(record.cwd.as_str())
            })
    }
}
#[derive(Default)]
struct Registry {
    records: HashMap<String, Record>,
    retired: VecDeque<String>,
    visibility: Arc<Visibility>,
}
impl Registry {
    fn publish(&mut self) {
        let mut visibility = Visibility::default();
        for record in self.records.values().filter(|record| record.hidden) {
            if let Some(id) = record.id.as_ref().or(record.observed_id.as_ref()) {
                visibility
                    .events
                    .entry(record.backend.clone())
                    .or_default()
                    .insert(id.clone());
            }
            if let Some(id) = &record.id {
                if record.result.is_none() {
                    visibility
                        .ids
                        .entry(record.backend.clone())
                        .or_default()
                        .insert(id.clone());
                } else {
                    // A late page still carries the old nonce/project. A user
                    // adopting a leftover task or a restored/reused SDK id must
                    // not disappear behind a previous cleanup receipt.
                    visibility
                        .retired
                        .entry(record.backend.clone())
                        .or_default()
                        .entry(id.clone())
                        .or_default()
                        .push((record.name.clone(), record.cwd.clone()));
                }
            } else {
                visibility.pending.push(record.clone());
            }
        }
        self.visibility = Arc::new(visibility);
    }
    fn reserve(&mut self, backend: String, source: String, cwd: String) -> Result<Record> {
        self.reserve_named(backend, source, cwd, "translation")
    }
    fn reserve_named(
        &mut self,
        backend: String,
        source: String,
        cwd: String,
        purpose: &str,
    ) -> Result<Record> {
        if !matches!(purpose, "translation" | "continuation") {
            return Err(bad("Invalid utility purpose"));
        }
        if self
            .records
            .values()
            .filter(|record| record.result.is_none())
            .count()
            >= MAX_ACTIVE
        {
            return Err((
                StatusCode::TOO_MANY_REQUESTS,
                "Temporary session workers are busy; wait for cleanup".into(),
            ));
        }
        let lease = uuid::Uuid::new_v4().simple().to_string();
        let record = Record {
            lease: lease.clone(),
            backend,
            source,
            cwd,
            name: format!("Studio {purpose} {lease}"),
            id: None,
            observed_id: None,
            closing: false,
            result: None,
            hidden: true,
        };
        self.records.insert(lease, record.clone());
        self.publish();
        Ok(record)
    }
    fn bind(&mut self, lease: &str, id: &str) -> Result<()> {
        let record = self
            .records
            .get_mut(lease)
            .ok_or_else(|| bad("Unknown utility receipt"))?;
        if !valid_id(id) || id == record.source {
            return Err(upstream(
                "The backend did not return an isolated translation identity",
            ));
        }
        record.id = Some(id.into());
        record.observed_id = None;
        self.publish();
        Ok(())
    }
    fn observe_metadata(&mut self, row: &Value) -> bool {
        let Some(id) = row["id"].as_str().filter(|id| valid_id(id)) else {
            return false;
        };
        // Partial status/message events cannot prove adoption of a session.
        if row["title"].as_str().is_none() || row["directory"].as_str().is_none() {
            return false;
        }
        let mut changed = false;
        for record in self.records.values_mut().filter(|r| r.hidden) {
            if record.id.is_none()
                && record.observed_id.is_none()
                && record.source != id
                && metadata_matches(record, row)
            {
                record.observed_id = Some(id.into());
                changed = true;
            } else if record.result.is_some()
                && record.id.as_deref().or(record.observed_id.as_deref()) == Some(id)
                && !metadata_matches(record, row)
            {
                // A retained cleanup receipt must not suppress the events of a
                // user-adopted session. This never retries an old mutation.
                record.hidden = false;
                changed = true;
            }
        }
        changed
    }
    fn finish(&mut self, lease: &str, result: std::result::Result<(), String>, detached: bool) {
        if let Some(record) = self.records.get_mut(lease) {
            let fresh = record.result.is_none();
            record.result = Some(result);
            record.hidden = !detached;
            if fresh {
                self.retired.push_back(lease.into());
            }
        }
        while self.retired.len() > MAX_RETIRED {
            if let Some(lease) = self.retired.pop_front() {
                self.records.remove(&lease);
            }
        }
        self.publish();
    }
}
#[derive(Clone)]
pub struct Store {
    registry: Arc<Mutex<Registry>>,
    catalogs: Arc<tokio::sync::Semaphore>,
    creations: Arc<tokio::sync::Semaphore>,
    pub(crate) event_streams: Arc<tokio::sync::Semaphore>,
}
impl Default for Store {
    fn default() -> Self {
        Self {
            registry: Arc::new(Mutex::new(Registry::default())),
            catalogs: Arc::new(tokio::sync::Semaphore::new(4)),
            creations: Arc::new(tokio::sync::Semaphore::new(MAX_ACTIVE)),
            event_streams: Arc::new(tokio::sync::Semaphore::new(4)),
        }
    }
}
impl Store {
    fn with<T>(&self, work: impl FnOnce(&mut Registry) -> Result<T>) -> Result<T> {
        let mut registry = self
            .registry
            .lock()
            .map_err(|_| upstream("Utility ownership lock is unavailable"))?;
        work(&mut registry)
    }
    pub fn visibility(&self) -> std::result::Result<Arc<Visibility>, String> {
        self.with(|registry| Ok(registry.visibility.clone()))
            .map_err(|(_, message)| message)
    }
    pub(crate) fn suppress_event(&self, value: &Value) -> std::result::Result<bool, String> {
        let event = value.get("payload").unwrap_or(value);
        let properties = &event["properties"];
        let session_event = event["type"]
            .as_str()
            .is_some_and(|kind| kind.starts_with("session."));
        self.with(|registry| {
            if session_event && registry.observe_metadata(&properties["info"]) {
                registry.publish();
            }
            let id = properties["sessionID"]
                .as_str()
                .or_else(|| {
                    properties
                        .pointer("/info/sessionID")
                        .and_then(Value::as_str)
                })
                .or_else(|| {
                    properties
                        .pointer("/part/sessionID")
                        .and_then(Value::as_str)
                })
                .or_else(|| {
                    session_event
                        .then(|| properties.pointer("/info/id").and_then(Value::as_str))
                        .flatten()
                });
            Ok(id.is_some_and(|id| {
                registry
                    .visibility
                    .events
                    .get("opencode")
                    .is_some_and(|ids| ids.contains(id))
            }))
        })
        .map_err(|(_, message)| message)
    }
    fn observe_catalog(&self, value: &Value) -> std::result::Result<(), String> {
        self.with(|registry| {
            let mut changed = false;
            for row in value.as_array().into_iter().flatten().take(100) {
                changed |= registry.observe_metadata(row);
            }
            if changed {
                registry.publish();
            }
            Ok(())
        })
        .map_err(|(_, message)| message)
    }
}

#[derive(Deserialize)]
#[serde(tag = "operation", rename_all = "camelCase", deny_unknown_fields)]
pub enum Request {
    #[serde(rename_all = "camelCase")]
    Create {
        backend: String,
        source_thread_id: String,
        cwd: String,
        #[serde(default)]
        purpose: Option<String>,
    },
    Catalog {
        backend: String,
        #[serde(default)]
        cursor: Option<String>,
    },
    Close {
        lease: String,
        #[serde(default)]
        interrupt: bool,
    },
}
pub(crate) fn configured(state: &GatewayState, backend: &str) -> Result<()> {
    if backend != "opencode"
        || !state
            .backend_descriptors
            .iter()
            .any(|b| b.id == backend && b.kind == "opencode")
    {
        return Err((
            StatusCode::NOT_FOUND,
            "The OpenCode backend is not configured".into(),
        ));
    }
    Ok(())
}
fn owns(record: &Record, value: &Value) -> bool {
    record.id.as_deref() == value["id"].as_str()
        && value["id"].as_str() != Some(record.source.as_str())
        && metadata_matches(record, value)
}
fn metadata_matches(record: &Record, value: &Value) -> bool {
    value["title"].as_str() == Some(record.name.as_str())
        && value["directory"].as_str() == Some(record.cwd.as_str())
}
async fn create(
    state: &GatewayState,
    backend: String,
    source: String,
    cwd: String,
    purpose: Option<String>,
) -> Result<Value> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(35);
    configured(state, &backend)?;
    let purpose = purpose.as_deref().unwrap_or("translation");
    if !matches!(purpose, "translation" | "continuation") {
        return Err(bad("Invalid utility purpose"));
    }
    if !valid_id(&source)
        || cwd.is_empty()
        || cwd.len() > 4096
        || cwd.chars().any(char::is_control)
        || !std::path::Path::new(&cwd).is_absolute()
    {
        return Err(bad("Invalid utility source identity or project directory"));
    }
    let _permit = state
        .utility_sessions
        .creations
        .try_acquire()
        .map_err(|_| {
            (
                StatusCode::TOO_MANY_REQUESTS,
                "Utility creation workers are busy".into(),
            )
        })?;
    // Prove the source's project before making any mutation. A source session is
    // never hidden, prompted, interrupted, renamed or deleted by this service.
    let original = tokio::time::timeout_at(
        deadline,
        state
            .opencode
            .router_request(Op::UtilityRead(&source), &cwd),
    )
    .await
    .map_err(|_| upstream("Utility source verification timed out"))?
    .map_err(upstream)?;
    if original["id"].as_str() != Some(source.as_str())
        || original["directory"].as_str() != Some(cwd.as_str())
    {
        return Err((
            StatusCode::CONFLICT,
            "The source project changed before translation".into(),
        ));
    }
    if state
        .utility_sessions
        .visibility()
        .map_err(upstream)?
        .hidden(&backend, &original)
    {
        return Err(bad("A utility task cannot be a conversation source"));
    }
    drop(original);
    let record = state
        .utility_sessions
        .with(|registry| registry.reserve_named(backend, source, cwd, purpose))?;
    let result = tokio::time::timeout_at(deadline, async {
        let body = json!({
            "title": record.name,
            "permission": crate::opencode_server::router_permissions()
        });
        let value = state
            .opencode
            .router_request(Op::UtilityCreate(&body), &record.cwd)
            .await
            .map_err(upstream)?;
        let id = value["id"]
            .as_str()
            .filter(|id| valid_id(id))
            .ok_or_else(|| upstream("The backend returned no utility identity"))?;
        if id == record.source
            || value["title"].as_str() != Some(record.name.as_str())
            || value["directory"].as_str() != Some(record.cwd.as_str())
        {
            return Err(upstream(
                "The backend did not confirm an isolated utility identity",
            ));
        }
        state
            .utility_sessions
            .with(|registry| registry.bind(&record.lease, id))?;
        if !crate::opencode_permissions::tools_disabled(&value["permission"]) {
            // Known newly-created identity, but never send a model prompt when
            // its final permission rule does not deny every tool.
            let _ = close(state, record.lease.clone(), false).await;
            return Err(upstream(
                "OpenCode did not confirm isolated translation permissions",
            ));
        }
        Ok(json!({
            "lease": record.lease,
            "session": {"id": id, "title": record.name, "directory": record.cwd}
        }))
    })
    .await
    .unwrap_or_else(|_| {
        Err(upstream(
            "Utility creation was not confirmed; it was not resent",
        ))
    });
    if let Err((_, error)) = &result {
        state.utility_sessions.with(|registry| {
            if registry
                .records
                .get(&record.lease)
                .is_some_and(|r| r.result.is_none())
            {
                registry.finish(&record.lease, Err(error.clone()), false);
            }
            Ok(())
        })?;
    }
    result
}
async fn close(state: &GatewayState, lease: String, interrupt: bool) -> Result<Value> {
    if lease.len() != 32 || !lease.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(bad("Invalid utility receipt"));
    }
    let record = state.utility_sessions.with(|registry| {
        let record = registry
            .records
            .get_mut(&lease)
            .ok_or_else(|| (StatusCode::NOT_FOUND, "Unknown utility receipt".into()))?;
        if let Some(result) = &record.result {
            return match result {
                Ok(()) => Ok(record.clone()),
                Err(error) => Err((
                    StatusCode::CONFLICT,
                    format!("{error}; cleanup is not replayed"),
                )),
            };
        }
        if record.closing {
            return Err((
                StatusCode::CONFLICT,
                "Utility cleanup is already running; it was not resent".into(),
            ));
        }
        if record.id.is_none() {
            return Err((
                StatusCode::CONFLICT,
                "Utility creation is still running".into(),
            ));
        }
        record.closing = true;
        Ok(record.clone())
    })?;
    if record.result.is_some() {
        return Ok(json!({"closed":true}));
    }
    let mut detached = false;
    let result = tokio::time::timeout(Duration::from_secs(10), async {
        let id = record.id.as_deref().ok_or("Missing utility identity")?;
        let current = state
            .opencode
            .router_request(Op::UtilityRead(id), &record.cwd)
            .await?;
        if !owns(&record, &current) {
            detached = true;
            return Err("Utility identity changed before cleanup".to_owned());
        }
        if interrupt {
            state
                .opencode
                .router_request(Op::Abort(id), &record.cwd)
                .await?;
        }
        let deleted = state
            .opencode
            .router_request(Op::UtilityDelete(id), &record.cwd)
            .await?;
        if !deleted.is_null() && deleted != true {
            return Err("OpenCode did not confirm utility deletion; it was not resent".into());
        }
        Ok::<_, String>(())
    })
    .await
    .unwrap_or_else(|_| Err("Utility cleanup was not confirmed; it was not resent".into()));
    state.utility_sessions.with(|registry| {
        registry.finish(&lease, result.clone(), detached);
        Ok(())
    })?;
    result.map(|()| json!({"closed":true})).map_err(upstream)
}

fn catalog_page(value: Value, visibility: &Visibility) -> std::result::Result<Value, String> {
    let rows = value.as_array().ok_or("Invalid OpenCode session catalog")?;
    if rows.len() > 100 {
        return Err("OpenCode catalog page exceeds 100 sessions".into());
    }
    // Calculate before filtering, even when the last/all rows are utilities.
    let cursor = if rows.len() == 100 {
        Some(
            rows.last()
                .and_then(|row| row.pointer("/time/updated").and_then(Value::as_i64))
                .ok_or("OpenCode did not provide a catalog cursor")?
                .to_string(),
        )
    } else {
        None
    };
    let mut output = Vec::new();
    for row in rows {
        if row["parentID"].as_str().is_some_and(|id| !id.is_empty())
            || row["ephemeral"] == true
            || row
                .pointer("/time/archived")
                .and_then(Value::as_i64)
                .is_some_and(|time| time > 0)
            || visibility.hidden("opencode", row)
        {
            continue;
        }
        let id = row["id"]
            .as_str()
            .filter(|id| valid_id(id))
            .ok_or("Invalid OpenCode catalog identity")?;
        let cwd = row["directory"]
            .as_str()
            .filter(|cwd| cwd.len() <= 4096)
            .ok_or("Invalid OpenCode catalog directory")?;
        let title = row["title"]
            .as_str()
            .unwrap_or("")
            .chars()
            .take(300)
            .collect::<String>();
        // Drop unbounded/irrelevant SDK summary/diff fields on this Rust worker.
        output.push(json!({
            "id": id, "title": title, "directory": cwd, "parentID": null,
            "time": {
                "updated": row.pointer("/time/updated").and_then(Value::as_i64),
                "archived": row.pointer("/time/archived").and_then(Value::as_i64)
            }
        }));
    }
    Ok(json!({"data":output,"nextCursor":cursor}))
}
pub async fn catalog(state: &GatewayState, cursor: &str) -> std::result::Result<Value, String> {
    if cursor.len() > 4096 || cursor.chars().any(char::is_control) {
        return Err("Invalid OpenCode catalog cursor".into());
    }
    let _permit = state
        .utility_sessions
        .catalogs
        .try_acquire()
        .map_err(|_| "OpenCode catalog workers are busy")?;
    let value = state
        .opencode
        .router_request(Op::Catalog(cursor), "")
        .await?;
    let ownership = state.utility_sessions.clone();
    tokio::task::spawn_blocking(move || {
        ownership.observe_catalog(&value)?;
        let visibility = ownership.visibility()?;
        catalog_page(value, &visibility)
    })
    .await
    .map_err(|_| "OpenCode catalog worker stopped")?
}
async fn dispatch(state: GatewayState, request: Request) -> Result<Value> {
    match request {
        Request::Create {
            backend,
            source_thread_id,
            cwd,
            purpose,
        } => create(&state, backend, source_thread_id, cwd, purpose).await,
        Request::Close { lease, interrupt } => close(&state, lease, interrupt).await,
        Request::Catalog { backend, cursor } => {
            configured(&state, &backend)?;
            if cursor
                .as_ref()
                .is_some_and(|value| value.len() > 4096 || value.chars().any(char::is_control))
            {
                return Err(bad("Invalid OpenCode catalog cursor"));
            }
            catalog(&state, cursor.as_deref().unwrap_or(""))
                .await
                .map_err(upstream)
        }
    }
}
pub async fn post(
    State(state): State<GatewayState>,
    request: std::result::Result<Json<Request>, JsonRejection>,
) -> Response<Body> {
    let request = match request {
        Ok(Json(value)) => value,
        Err(error) => {
            return crate::json_error(
                if error.status() == StatusCode::PAYLOAD_TOO_LARGE {
                    StatusCode::PAYLOAD_TOO_LARGE
                } else {
                    StatusCode::BAD_REQUEST
                },
                "Invalid utility session request",
            )
        }
    };
    // Ownership bookkeeping survives a caller disconnect during session creation.
    // Unknown mutations are never resent, and no guessed identity is deleted.
    let result = tokio::spawn(dispatch(state, request))
        .await
        .unwrap_or_else(|_| Err(upstream("Utility session worker stopped")));
    match result {
        Ok(value) => crate::json_response(StatusCode::OK, &value),
        Err((status, message)) => crate::json_error(status, &message),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn row(id: &str, name: &str, cwd: &str) -> Value {
        json!({"id":id,"title":name,"directory":cwd,"time":{"updated":42}})
    }
    #[test]
    fn continuation_receipts_keep_exact_ownership_and_reject_arbitrary_purposes() {
        let mut registry = Registry::default();
        assert!(registry
            .reserve_named(
                "opencode".into(),
                "source".into(),
                "/project".into(),
                "untrusted"
            )
            .is_err());
        assert!(registry.records.is_empty());
        let record = registry
            .reserve_named(
                "opencode".into(),
                "source".into(),
                "/project".into(),
                "continuation",
            )
            .unwrap();
        assert!(record.name.starts_with("Studio continuation "));
        assert!(!registry
            .visibility
            .hidden("opencode", &row("source", &record.name, "/project")));
        assert!(!registry.visibility.hidden(
            "opencode",
            &row("user", "Studio continuation user notes", "/project")
        ));
        registry.bind(&record.lease, "owned").unwrap();
        assert!(owns(
            registry.records.get(&record.lease).unwrap(),
            &row("owned", &record.name, "/project")
        ));
        assert!(!owns(
            registry.records.get(&record.lease).unwrap(),
            &row("owned", &record.name, "/other-project")
        ));
    }
    #[test]
    fn exact_pending_nonce_source_guard_and_backend_namespace() {
        let mut registry = Registry::default();
        let record = registry
            .reserve("opencode".into(), "source".into(), "/project".into())
            .unwrap();
        assert!(registry
            .visibility
            .hidden("opencode", &row("new", &record.name, "/project")));
        for value in [
            row("source", &record.name, "/project"),
            row("other", &record.name, "/elsewhere"),
            row("user", "Studio translation user notes", "/project"),
        ] {
            assert!(!registry.visibility.hidden("opencode", &value));
        }
        registry.bind(&record.lease, "owned").unwrap();
        assert!(registry
            .visibility
            .hidden("opencode", &row("owned", "new name", "/project")));
        assert!(!registry
            .visibility
            .hidden("another-backend", &row("owned", &record.name, "/project")));
        assert!(registry.bind(&record.lease, "source").is_err());
    }
    #[test]
    fn retired_identity_filters_late_catalog_and_detachment_restores_visibility() {
        let mut registry = Registry::default();
        let record = registry
            .reserve("opencode".into(), "source".into(), "/project".into())
            .unwrap();
        registry.bind(&record.lease, "owned").unwrap();
        registry.finish(&record.lease, Ok(()), false);
        assert!(registry
            .visibility
            .hidden("opencode", &row("owned", &record.name, "/project")));
        assert!(!registry.visibility.hidden(
            "opencode",
            &row("owned", "User adopted session", "/project")
        ));
        registry.finish(&record.lease, Err("adopted".into()), true);
        assert!(!registry
            .visibility
            .hidden("opencode", &row("owned", "user adopted", "/project")));
    }
    #[test]
    fn filtering_preserves_original_full_page_cursor_even_when_every_row_is_hidden() {
        let mut registry = Registry::default();
        let mut values = Vec::new();
        for i in 0..100 {
            let record = registry
                .reserve("opencode".into(), "source".into(), "/project".into())
                .unwrap();
            let id = format!("new-{i}");
            registry.bind(&record.lease, &id).unwrap();
            registry.finish(&record.lease, Ok(()), false);
            values.push(row(&id, &record.name, "/project"));
        }
        let value = catalog_page(json!(values), &registry.visibility).unwrap();
        assert!(value["data"].as_array().unwrap().is_empty());
        assert_eq!(value["nextCursor"], "42");
        assert!(catalog_page(json!(vec![json!({}); 101]), &registry.visibility).is_err());
    }
    #[test]
    fn metadata_projection_never_retains_upstream_objects_or_summary_payloads() {
        let mut value = row("user", "User session", "/project");
        let private = json!({"private": "x".repeat(1024 * 1024)});
        value["parentID"] = private.clone();
        value["time"]["updated"] = private.clone();
        value["time"]["archived"] = private.clone();
        value["summary"] = private;
        let page = catalog_page(json!([value]), &Visibility::default()).unwrap();
        assert!(page.to_string().len() < 512);
        assert!(page["data"][0]["parentID"].is_null());
        assert!(page["data"][0]["time"]["updated"].is_null());
        assert!(page["data"][0]["time"]["archived"].is_null());
        assert!(page["data"][0].get("summary").is_none());
    }
    #[test]
    fn event_observation_filters_exact_pending_identity_without_authorizing_cleanup() {
        let ownership = Store::default();
        let record = ownership
            .with(|r| r.reserve("opencode".into(), "source".into(), "/project".into()))
            .unwrap();
        let created = json!({"payload":{"type":"session.created","properties":{"info":row("pending", &record.name, "/project")}}});
        assert!(ownership.suppress_event(&created).unwrap());
        ownership
            .with(|r| {
                assert!(r.records[&record.lease].id.is_none());
                assert_eq!(
                    r.records[&record.lease].observed_id.as_deref(),
                    Some("pending")
                );
                Ok(())
            })
            .unwrap();
        for properties in [
            json!({"sessionID":"pending"}),
            json!({"info":{"sessionID":"pending"}}),
            json!({"part":{"sessionID":"pending"}}),
        ] {
            assert!(ownership
                .suppress_event(&json!({"type":"message.updated","properties":properties}))
                .unwrap());
        }
        for value in [
            json!({"type":"session.created","properties":{"info":row("source", &record.name, "/project")}}),
            json!({"type":"session.created","properties":{"info":row("user", "Studio translation user notes", "/project")}}),
            json!({"type":"server.connected"}),
        ] {
            assert!(!ownership.suppress_event(&value).unwrap());
        }
        ownership
            .with(|r| r.bind(&record.lease, "confirmed"))
            .unwrap();
        assert!(!ownership
            .suppress_event(&json!({"type":"session.idle","properties":{"sessionID":"pending"}}))
            .unwrap());
        assert!(ownership
            .suppress_event(&json!({"type":"session.idle","properties":{"sessionID":"confirmed"}}))
            .unwrap());
    }
    #[test]
    fn late_events_are_hidden_but_observed_user_adoption_restores_event_visibility() {
        for via_catalog in [false, true] {
            let ownership = Store::default();
            let record = ownership
                .with(|r| r.reserve("opencode".into(), "source".into(), "/project".into()))
                .unwrap();
            ownership
                .with(|r| {
                    r.bind(&record.lease, "owned")?;
                    r.finish(&record.lease, Ok(()), false);
                    Ok(())
                })
                .unwrap();
            let event = json!({"type":"session.status","properties":{"sessionID":"owned","status":{"type":"busy"}}});
            assert!(ownership.suppress_event(&event).unwrap());
            let adopted = row("owned", "User adopted session", "/project");
            if via_catalog {
                ownership.observe_catalog(&json!([adopted])).unwrap();
            } else {
                assert!(!ownership
                    .suppress_event(
                        &json!({"type":"session.updated","properties":{"info":adopted}})
                    )
                    .unwrap());
            }
            assert!(!ownership.suppress_event(&event).unwrap());
            assert!(!ownership
                .visibility()
                .unwrap()
                .hidden("opencode", &row("owned", &record.name, "/project")));
        }
    }
    #[test]
    fn admission_and_tombstones_are_bounded_and_unknown_creation_does_not_guess_an_id() {
        let mut registry = Registry::default();
        let mut active = Vec::new();
        for _ in 0..MAX_ACTIVE {
            active.push(
                registry
                    .reserve("opencode".into(), "source".into(), "/project".into())
                    .unwrap(),
            );
        }
        assert_eq!(
            registry
                .reserve("opencode".into(), "source".into(), "/project".into())
                .unwrap_err()
                .0,
            StatusCode::TOO_MANY_REQUESTS
        );
        for record in active {
            registry.finish(&record.lease, Err("unknown creation".into()), false);
        }
        for _ in 0..MAX_RETIRED + 3 {
            let record = registry
                .reserve("opencode".into(), "source".into(), "/project".into())
                .unwrap();
            registry.finish(&record.lease, Ok(()), false);
        }
        assert_eq!(registry.records.len(), MAX_RETIRED);
        assert_eq!(registry.retired.len(), MAX_RETIRED);
    }
}
