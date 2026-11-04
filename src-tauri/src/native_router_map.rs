//! Gateway-owned Map scopes for routed Codex targets. Never rebase or replay writes.
use crate::{
    session_map as store, session_map_protocol as protocol, session_map_types::*, GatewayState,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, VecDeque},
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use tokio::sync::{broadcast, mpsc, Notify, Semaphore};
#[cfg(test)]
#[path = "native_router_map_tests.rs"]
mod tests;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Scope {
    pub id: String,
    pub revision: u64,
}
#[derive(Clone)]
struct Storage {
    path: Arc<PathBuf>,
    lock: Arc<Mutex<()>>,
    workers: Arc<Semaphore>,
    backend: String,
    thread: String,
}
impl Storage {
    fn new(state: &GatewayState, backend: &str, thread: &str) -> Self {
        Self {
            path: state.session_maps_path.clone(),
            lock: state.session_maps_lock.clone(),
            workers: state.session_maps_workers.clone(),
            backend: backend.into(),
            thread: thread.into(),
        }
    }
    fn read(&self) -> Result<Option<SessionMap>, String> {
        let _permit = self
            .workers
            .clone()
            .try_acquire_owned()
            .map_err(|_| "Map storage is busy")?;
        let _guard = self
            .lock
            .lock()
            .map_err(|_| "Map storage lock unavailable")?;
        let map = store::find(&self.path, &self.backend, &self.thread)?;
        if let Some(map) = &map {
            crate::session_map_types::validate_snapshot(map)?;
            if map.backend != self.backend || map.thread_id != self.thread || map.revision == 0 {
                return Err(
                    "Stored Map identity/revision does not match its routed session".into(),
                );
            }
        }
        Ok(map)
    }
    fn apply(
        &self,
        scope: &Scope,
        turn: &str,
        operations: Vec<MapOperation>,
    ) -> Result<SessionMap, String> {
        let _permit = self
            .workers
            .clone()
            .try_acquire_owned()
            .map_err(|_| "Map storage is busy")?;
        let _guard = self
            .lock
            .lock()
            .map_err(|_| "Map storage lock unavailable")?;
        store::apply_operations(
            &self.path,
            &self.backend,
            &self.thread,
            ApplyOperationsRequest {
                base_revision: scope.revision,
                map_id: Some(scope.id.clone()),
                actor: "assistant".into(),
                source_turn_id: Some(turn.into()),
                operations,
            },
        )?
        .ok_or_else(|| "The Session Map was removed".into())
    }
}
struct Receipt {
    key: String,
    fingerprint: [u8; 32],
    result: Value,
}
struct Pending {
    key: String,
    fingerprint: [u8; 32],
    ids: Vec<Value>,
}
struct State {
    active: bool,
    turn: Option<String>,
    scope: Scope,
    busy: Option<Pending>,
    receipts: VecDeque<Receipt>,
    skip_fallback: bool,
    notice: String,
    recovered: bool,
}
struct Entry {
    generation: u64,
    epoch: Arc<AtomicU64>,
    storage: Storage,
    state: Mutex<State>,
    changed: Notify,
}
#[derive(Default)]
struct Entries {
    items: HashMap<String, Arc<Entry>>,
    order: VecDeque<String>,
    configured: VecDeque<String>,
    requests: HashMap<String, Arc<Entry>>,
}
#[derive(Clone)]
pub struct Registry {
    entries: Arc<Mutex<Entries>>,
    workers: Arc<Semaphore>,
}
impl Default for Registry {
    fn default() -> Self {
        Self {
            entries: Arc::default(),
            workers: Arc::new(Semaphore::new(8)),
        }
    }
}
pub struct Guard {
    entry: Arc<Entry>,
}
impl Drop for Guard {
    fn drop(&mut self) {
        if let Ok(mut state) = self.entry.state.lock() {
            state.active = false;
            state.skip_fallback = true;
        }
        self.entry.changed.notify_waiters();
    }
}
fn valid(value: &str, max: usize) -> bool {
    !value.is_empty() && value.len() <= max && !value.chars().any(char::is_control)
}
fn result(success: bool, text: &str) -> Value {
    json!({"success":success,"contentItems":[{"type":"inputText","text":text}]})
}
fn message(events: &broadcast::Sender<String>, entry: &Entry, revision: u64, notice: &str) {
    let _=events.send(json!({"method":"studio/sessionMap/changed","params":{"backend":entry.storage.backend,"threadId":entry.storage.thread,"mapId":entry.state.lock().ok().map(|s|s.scope.id.clone()),"revision":revision,"message":notice}}).to_string());
}
impl Registry {
    pub fn observe_request(&self, value: &Value) {
        self.observe_configuration(value);
        if value["method"] != "turn/start" {
            return;
        }
        let Some(id) = value["id"]
            .as_str()
            .filter(|s| s.starts_with("studio-native-read-"))
        else {
            return;
        };
        let Some(thread) = value["params"]["threadId"].as_str() else {
            return;
        };
        if let Ok(mut entries) = self.entries.lock() {
            entries
                .requests
                .retain(|_, e| e.state.lock().is_ok_and(|s| s.active && s.turn.is_none()));
            if entries.requests.len() >= 32 {
                return;
            }
            if let Some(entry) = entries
                .items
                .get(thread)
                .filter(|e| e.state.lock().is_ok_and(|s| s.active && s.turn.is_none()))
                .cloned()
            {
                entries.requests.insert(id.into(), entry);
            }
        }
    }
    pub fn observe_reply(&self, value: &Value) {
        if value.get("method").is_some() {
            return;
        }
        let Some(id) = value["id"].as_str() else {
            return;
        };
        let entry = self
            .entries
            .lock()
            .ok()
            .and_then(|mut e| e.requests.remove(id));
        if let Some(entry) = entry {
            if let Ok(mut state) = entry.state.lock() {
                if state.active && entry.epoch.load(Ordering::SeqCst) == entry.generation {
                    if let Some(turn) = value
                        .pointer("/result/turn/id")
                        .and_then(Value::as_str)
                        .filter(|s| valid(s, 256))
                    {
                        state.turn = Some(turn.into());
                    } else {
                        state.skip_fallback = true;
                    }
                }
            }
        }
    }
    /// Track only application Map configuration; ordinary developer instructions
    /// and external dynamic tools are never inferred to be ours.
    pub fn observe_configuration(&self, value: &Value) {
        if value["method"] != "thread/resume" {
            return;
        }
        let p = &value["params"];
        let Some(thread) = p["threadId"].as_str().filter(|s| valid(s, 256)) else {
            return;
        };
        let mapped = p["developerInstructions"].as_str().is_some_and(|s| {
            s.starts_with("This thread has an optional Session Map maintained by the client.")
        });
        let cleared = p.get("developerInstructions").is_some_and(Value::is_null);
        if !mapped && !cleared {
            return;
        }
        if let Ok(mut entries) = self.entries.lock() {
            if entries
                .items
                .get(thread)
                .is_some_and(|e| e.state.lock().is_ok_and(|s| !s.active))
            {
                entries.items.remove(thread);
                entries.order.retain(|t| t != thread);
            }
            entries.configured.retain(|t| t != thread);
            if mapped {
                entries.configured.push_back(thread.into());
                if entries.configured.len() > 1024 {
                    entries.configured.pop_front();
                }
            }
        }
    }
    pub fn configured(&self, thread: &str) -> bool {
        self.entries
            .lock()
            .is_ok_and(|e| e.configured.iter().any(|s| s == thread))
    }
    fn reserve(
        &self,
        storage: Storage,
        scope: Scope,
        generation: u64,
        epoch: Arc<AtomicU64>,
        recovered: bool,
    ) -> Result<Guard, String> {
        if !valid(&scope.id, 128) || scope.revision == 0 {
            return Err("Invalid captured Map identity/revision".into());
        }
        let mut entries = self
            .entries
            .lock()
            .map_err(|_| "Map scope registry unavailable")?;
        if entries
            .items
            .get(&storage.thread)
            .is_some_and(|e| e.state.lock().is_ok_and(|s| s.active))
        {
            return Err("Another routed turn owns this Map context".into());
        }
        if entries.items.len() >= 32 && !entries.items.contains_key(&storage.thread) {
            let evict = entries
                .order
                .iter()
                .find(|id| {
                    entries.items.get(*id).is_some_and(|e| {
                        e.state.lock().is_ok_and(|s| !s.active && s.busy.is_none())
                    })
                })
                .cloned()
                .ok_or("Too many active routed Map contexts")?;
            entries.items.remove(&evict);
            entries.order.retain(|id| id != &evict);
        }
        let entry = Arc::new(Entry {
            generation,
            epoch,
            storage,
            state: Mutex::new(State {
                active: true,
                turn: None,
                scope,
                busy: None,
                receipts: VecDeque::new(),
                skip_fallback: false,
                notice: String::new(),
                recovered,
            }),
            changed: Notify::new(),
        });
        entries.order.retain(|t| t != &entry.storage.thread);
        entries.order.push_back(entry.storage.thread.clone());
        entries
            .items
            .insert(entry.storage.thread.clone(), entry.clone());
        Ok(Guard { entry })
    }
    /// Claims only Map calls for registered routed scopes, before WebSocket
    /// broadcast. A browser or native viewer must not also answer these calls.
    pub fn intercept(
        &self,
        payload: &Value,
        generation: u64,
        input: mpsc::Sender<String>,
        events: broadcast::Sender<String>,
    ) -> bool {
        if payload["method"] != "item/tool/call"
            || payload["params"]["tool"] != "update_session_map"
        {
            return false;
        }
        let Some(thread) = payload["params"]["threadId"].as_str() else {
            return false;
        };
        let entry = self
            .entries
            .lock()
            .ok()
            .and_then(|e| e.items.get(thread).cloned());
        let Some(entry) = entry else {
            return false;
        };
        let id = payload["id"].clone();
        let payload = payload.clone();
        let permit = self.workers.clone().try_acquire_owned();
        tokio::spawn(async move {
            let work = match permit {
                Ok(permit) => {
                    let entry = entry.clone();
                    tokio::task::spawn_blocking(move || {
                        let _permit = permit;
                        handle(&entry, &payload, generation)
                    })
                    .await
                    .unwrap_or_else(|_| {
                        (
                            vec![id.clone()],
                            result(false, "Map worker stopped; the update was not replayed"),
                            None,
                        )
                    })
                }
                Err(_) => (
                    vec![id],
                    result(false, "Map workers are busy; no update was applied"),
                    None,
                ),
            };
            if let Some((revision, notice)) = work.2 {
                message(&events, &entry, revision, &notice);
            }
            for id in work.0 {
                let body = json!({"id":id,"result":work.1}).to_string();
                let _ = tokio::time::timeout(Duration::from_secs(5), input.send(body)).await;
            }
        });
        true
    }
}
fn handle(entry: &Entry, p: &Value, generation: u64) -> (Vec<Value>, Value, Option<(u64, String)>) {
    let id = p["id"].clone();
    let reject = |text: &str| (vec![id.clone()], result(false, text), None);
    if !id.is_number() && !id.as_str().is_some_and(|s| valid(s, 256)) {
        return reject("Invalid Map tool identity");
    }
    let turn = p["params"]["turnId"].as_str().unwrap_or("");
    let key = p["params"]["callId"]
        .as_str()
        .filter(|s| valid(s, 256))
        .map(str::to_owned)
        .unwrap_or_else(|| id.to_string());
    let args = match &p["params"]["arguments"] {
        Value::String(s) => s.clone(),
        v => v.to_string(),
    };
    let fingerprint: [u8; 32] = Sha256::digest(args.as_bytes()).into();
    let operations = match protocol::parse_operations(&args) {
        Ok(v) if !v.is_empty() => v,
        Ok(_) => return reject("No Map operations were provided"),
        Err(e) => return reject(&e),
    };
    let mut state = match entry.state.lock() {
        Ok(s) => s,
        Err(_) => return reject("Map context lock unavailable"),
    };
    if entry.generation != generation
        || entry.epoch.load(Ordering::SeqCst) != generation
        || state.turn.as_deref() != Some(turn)
    {
        return reject(
            "This Map tool call has no confirmed routed turn context; use the trailing fallback",
        );
    }
    if let Some(receipt) = state.receipts.iter().find(|r| r.key == key) {
        return if receipt.fingerprint == fingerprint {
            (vec![id], receipt.result.clone(), None)
        } else {
            reject("Map call identity was reused with different arguments")
        };
    }
    if !state.active
        || state.recovered
        || entry.generation != generation
        || entry.epoch.load(Ordering::SeqCst) != generation
        || state.turn.as_deref() != Some(turn)
    {
        return reject(
            "This Map tool call has no confirmed routed turn context; use the trailing fallback",
        );
    }
    if let Some(pending) = state.busy.as_mut() {
        if pending.key == key && pending.fingerprint == fingerprint && pending.ids.len() < 8 {
            pending.ids.push(id);
            return (Vec::new(), Value::Null, None);
        }
        return reject("Another Map update is in progress");
    }
    state.busy = Some(Pending {
        key: key.clone(),
        fingerprint,
        ids: vec![id],
    });
    let scope = state.scope.clone();
    drop(state);
    let applied = entry.storage.apply(&scope, turn, operations);
    let mut state = entry.state.lock().unwrap_or_else(|p| p.into_inner());
    let ids = state.busy.take().map(|p| p.ids).unwrap_or_default();
    // Once a database mutation was attempted, a fallback must not replay it.
    state.skip_fallback = true;
    let (ok, notice, revision) = match applied {
        Ok(map) => {
            state.scope.revision = map.revision;
            (
                true,
                format!("Session Map updated to revision {}.", map.revision),
                map.revision,
            )
        }
        Err(e) => (
            false,
            format!("Session Map update rejected: {e}"),
            state.scope.revision,
        ),
    };
    let notice = protocol::notice(&notice);
    state.notice = notice.clone();
    let reply = result(ok, &notice);
    state.receipts.push_back(Receipt {
        key,
        fingerprint,
        result: reply.clone(),
    });
    if state.receipts.len() > 128 {
        state.receipts.pop_front();
    }
    drop(state);
    entry.changed.notify_waiters();
    (ids, reply, Some((revision, notice)))
}
impl Guard {
    pub fn scope(&self) -> Scope {
        self.entry
            .state
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .scope
            .clone()
    }
    pub fn bind(&self, turn: &str) -> Result<(), String> {
        if !valid(turn, 256) {
            return Err("Invalid routed Map turn identity".into());
        }
        let mut state = self
            .entry
            .state
            .lock()
            .map_err(|_| "Map context lock unavailable")?;
        if state.turn.as_deref().is_some_and(|id| id != turn) {
            return Err("Routed Map turn identity changed after its ACK".into());
        }
        state.turn = Some(turn.into());
        Ok(())
    }
    pub async fn finish(&self, update: Option<Result<protocol::Update, String>>) -> String {
        loop {
            let notified = self.entry.changed.notified();
            if self.entry.state.lock().is_ok_and(|s| s.busy.is_none()) {
                break;
            }
            if tokio::time::timeout(Duration::from_secs(35), notified)
                .await
                .is_err()
            {
                return "Map update is unconfirmed; it was not replayed".into();
            }
        }
        let entry = self.entry.clone();
        tokio::task::spawn_blocking(move || -> Result<String, String> {
            let mut state = entry
                .state
                .lock()
                .map_err(|_| "Map context lock unavailable".to_owned())?;
            if state.skip_fallback {
                return Ok(state.notice.clone());
            }
            state.skip_fallback = true;
            let scope = state.scope.clone();
            let turn = state.turn.clone().ok_or("Map turn was not confirmed")?;
            drop(state);
            if entry.epoch.load(Ordering::SeqCst) != entry.generation {
                return Err("The backend changed; Map synchronization was not replayed".into());
            }
            let current = entry.storage.read()?.ok_or("The Session Map was removed")?;
            if current.id != scope.id {
                return Err(
                    "The Session Map was replaced; the captured update was rejected".into(),
                );
            }
            if current.last_synced_turn_id.as_deref() == Some(&turn) {
                return Ok("This routed turn already updated its Map".into());
            }
            let update = update.ok_or(
                "This response did not include a Map update block; use Update with AI if needed",
            )??;
            if update.revision != scope.revision {
                return Err("The Map update used a stale captured revision".into());
            }
            if update.operations.is_empty() {
                return Ok("This response did not require a Map change".into());
            }
            let map = entry.storage.apply(&scope, &turn, update.operations)?;
            if let Ok(mut state) = entry.state.lock() {
                state.scope.revision = map.revision;
            }
            Ok(format!("Session Map updated to revision {}.", map.revision))
        })
        .await
        .unwrap_or_else(|_| Err("Map worker stopped; the update was not replayed".into()))
        .map_or_else(
            |e| protocol::notice(&format!("Session Map update rejected: {e}")),
            |s| protocol::notice(&s),
        )
    }
    pub fn publish(&self, events: &broadcast::Sender<String>, notice: &str) {
        let revision = self.entry.state.lock().map_or(0, |s| s.scope.revision);
        message(events, &self.entry, revision, notice);
    }
}
pub async fn prepare(
    state: &GatewayState,
    session: &crate::native_router_backend::Session,
) -> Result<Option<Guard>, String> {
    let Some(server) = session.codex(state)? else {
        return Ok(None);
    };
    let storage = Storage::new(state, &session.backend, &session.id);
    let read = storage.clone();
    let map = tokio::task::spawn_blocking(move || read.read())
        .await
        .map_err(|_| "Map context worker stopped")??;
    let Some(map) = map else {
        if server.router_maps.configured(&session.id) {
            let params = json!({"threadId":session.id,"excludeTurns":true,"developerInstructions":null,"dynamicTools":[]});
            configure(server, params).await?;
        }
        return Ok(None);
    };
    let generation = server.current_generation();
    let scope = Scope {
        id: map.id.clone(),
        revision: map.revision,
    };
    let guard = server.router_maps.reserve(
        storage,
        scope,
        generation,
        server.generation_source(),
        false,
    )?;
    let mut params = tokio::task::spawn_blocking(move || protocol::configuration(&map))
        .await
        .map_err(|_| "Map context worker stopped")?;
    params["threadId"] = session.id.clone().into();
    params["excludeTurns"] = true.into();
    configure(server, params).await?;
    if server.current_generation() != generation {
        return Err("The backend changed during Map setup; the target turn was not sent".into());
    }
    Ok(Some(guard))
}
pub async fn recover(
    state: &GatewayState,
    session: &crate::native_router_backend::Session,
    scope: Scope,
    turn: &str,
) -> Result<Option<Guard>, String> {
    let Some(server) = session.codex(state)? else {
        return Ok(None);
    };
    let guard = server.router_maps.reserve(
        Storage::new(state, &session.backend, &session.id),
        scope,
        server.current_generation(),
        server.generation_source(),
        true,
    )?;
    guard.bind(turn)?;
    Ok(Some(guard))
}
pub(crate) async fn configure(
    server: &crate::codex_app_server::CodexAppServer,
    mut params: Value,
) -> Result<(), String> {
    let thread = params["threadId"].clone();
    match server.request_router("thread/resume", params.clone()).await {
        Ok(value) => {
            if value.pointer("/thread/id") != Some(&thread) {
                return Err("Map configuration returned a different thread".into());
            }
        }
        Err(_) => {
            params.as_object_mut().unwrap().remove("dynamicTools");
            let value = server.request_router("thread/resume", params).await?;
            if value.pointer("/thread/id") != Some(&thread) {
                return Err("Map configuration returned a different thread".into());
            }
        }
    }
    Ok(())
}
