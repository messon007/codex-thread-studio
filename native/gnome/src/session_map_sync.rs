//! Socket-owned Map coordination. All SQLite/HTTP/JSON work runs outside GTK.
use crate::{
    gateway::Api,
    session_map::{self as data, Anchor, Snapshot},
    session_map_ai as ai,
    session_map_text::{Notice, Update},
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
    time::{Duration, Instant},
};
#[cfg(test)]
#[path = "session_map_sync_tests.rs"]
mod tests;
#[derive(Clone, Debug)]
struct Version {
    id: String,
    revision: u64,
}
struct Start {
    id: u64,
    params: Value,
    thread: String,
    version: Option<Version>,
    configuration: Value,
    fallback: bool,
    guarded: bool,
    since: Instant,
}
struct Turn {
    version: Version,
    turn: Option<String>,
    update: Option<Result<Update, String>>,
    completed: bool,
    skip_fallback: bool,
    busy: bool,
}
struct Call {
    thread: String,
    turn: String,
    key: String,
    ids: Vec<Value>,
    fingerprint: [u8; 32],
}
struct Cached {
    thread: String,
    turn: String,
    key: String,
    result: Value,
    fingerprint: [u8; 32],
}
pub struct Tool {
    id: Value,
    thread: String,
    turn: String,
    key: String,
    fingerprint: [u8; 32],
    operations: Result<Vec<crate::session_map::types::MapOperation>, String>,
}
enum Job {
    Loaded {
        id: u64,
        result: Result<Option<(Version, Value)>, String>,
    },
    Applied {
        thread: String,
        turn: String,
        call: Option<Call>,
        result: Result<Option<Arc<Snapshot>>, data::Failure>,
    },
}
#[derive(Debug)]
pub enum Effect {
    Send(Value),
    Start {
        id: u64,
        params: Value,
    },
    GuardedStart {
        id: u64,
        params: Value,
    },
    Failed {
        id: u64,
        message: String,
    },
    Changed {
        thread: String,
        result: Result<Option<Arc<Snapshot>>, data::Failure>,
        message: String,
    },
}
pub struct Runtime {
    api: Api,
    backend: String,
    sequence: u64,
    epoch: Option<u64>,
    starts: HashMap<u64, Start>,
    resumes: HashMap<String, Start>,
    deferred: HashMap<u64, Start>,
    sent: HashMap<u64, String>,
    turns: HashMap<String, Turn>,
    configured: VecDeque<String>,
    calls: HashMap<String, Call>,
    cached: VecDeque<Cached>,
    jobs: tokio::task::JoinSet<Job>,
    effects: VecDeque<Effect>,
}
fn failure(message: impl Into<String>) -> data::Failure {
    data::Failure {
        message: message.into(),
        conflict: false,
        uncertain: false,
    }
}
fn tool_result(result: Result<&str, &str>) -> Value {
    let (success, text) = match result {
        Ok(s) => (true, s),
        Err(s) => (false, s),
    };
    json!({"success":success,"contentItems":[{"type":"inputText","text":text}]})
}
fn tool_request(payload: &Value) -> bool {
    payload["method"] == "item/tool/call" && payload["params"]["tool"] == "update_session_map"
}
pub fn is_tool(payload: &Value) -> bool {
    tool_request(payload)
}
pub fn prepare_tool(payload: &Value) -> Option<Tool> {
    if !is_tool(payload) {
        return None;
    }
    let p = &payload["params"];
    let id = payload["id"].clone();
    let arguments = match &p["arguments"] {
        Value::String(s) => s.clone(),
        v => v.to_string(),
    };
    let fingerprint = Sha256::digest(arguments.as_bytes()).into();
    Some(Tool {
        id: id.clone(),
        thread: p["threadId"].as_str().unwrap_or("").into(),
        turn: p["turnId"].as_str().unwrap_or("").into(),
        key: p["callId"]
            .as_str()
            .filter(|s| !s.is_empty() && s.len() <= 256)
            .map(str::to_owned)
            .unwrap_or_else(|| id.to_string()),
        fingerprint,
        operations: ai::parse_operations(&arguments),
    })
}
pub fn configuration(snapshot: &Snapshot) -> Value {
    crate::session_map_protocol::configuration(&snapshot.map)
}
impl Runtime {
    pub fn new(api: Api, backend: String) -> Self {
        Self {
            api,
            backend,
            sequence: 0,
            epoch: None,
            starts: HashMap::new(),
            resumes: HashMap::new(),
            deferred: HashMap::new(),
            sent: HashMap::new(),
            turns: HashMap::new(),
            configured: VecDeque::new(),
            calls: HashMap::new(),
            cached: VecDeque::new(),
            jobs: tokio::task::JoinSet::new(),
            effects: VecDeque::new(),
        }
    }
    pub fn has_work(&self) -> bool {
        !self.effects.is_empty() || !self.jobs.is_empty()
    }
    pub fn map_deleted(&mut self, thread: String) {
        if !self.configured.contains(&thread) {
            self.configured.push_back(thread);
        }
        if self.configured.len() > 1024 {
            self.configured.pop_front();
        }
    }
    pub fn start(&mut self, id: u64, params: Value) {
        self.begin(id, params, false);
    }
    pub fn start_guarded(&mut self, id: u64, params: Value) {
        self.begin(id, params, true);
    }
    fn begin(&mut self, id: u64, params: Value, guarded: bool) {
        let thread = params["threadId"].as_str().unwrap_or("").to_owned();
        let occupied = self
            .starts
            .values()
            .chain(self.resumes.values())
            .chain(self.deferred.values())
            .any(|s| s.thread == thread);
        if thread.is_empty()
            || thread.len() > 256
            || occupied
            || self.starts.len() + self.resumes.len() + self.deferred.len() >= 8
            || self.jobs.len() >= 8
        {
            self.effects.push_back(Effect::Failed{id,message:"Map context preparation is busy or has an invalid thread; the turn was not sent".into()});
            return;
        }
        if self.turns.get(&thread).is_some_and(|t| !t.completed) {
            self.effects.push_back(Effect::Failed {
                id,
                message: "A mapped turn is already active in this conversation".into(),
            });
            return;
        }
        self.starts.insert(
            id,
            Start {
                id,
                params,
                thread: thread.clone(),
                version: None,
                configuration: Value::Null,
                fallback: false,
                guarded,
                since: Instant::now(),
            },
        );
        if !self.turns.get(&thread).is_some_and(|t| t.busy) {
            self.load(id);
        }
    }
    fn load(&mut self, id: u64) {
        let Some(start) = self.starts.get(&id) else {
            return;
        };
        let anchor = Anchor {
            backend: self.backend.clone(),
            thread: start.thread.clone(),
        };
        let api = self.api.clone();
        self.jobs.spawn(async move {
            let result = match data::execute(&api, data::Request::Load(anchor)).await {
                Ok(Some(snapshot)) => {
                    crate::worker::compute(move || {
                        Ok(Some((
                            Version {
                                id: snapshot.map.id.clone(),
                                revision: snapshot.map.revision,
                            },
                            configuration(&snapshot),
                        )))
                    })
                    .await
                }
                Ok(None) => Ok(None),
                Err(e) => Err(format!("Unable to load Map context: {}", e.message)),
            };
            Job::Loaded { id, result }
        });
    }
    fn resume(&mut self, start: Start) {
        // Resume may already have configured the backend when its reply is lost
        // or cancelled. A later no-Map turn must conservatively clear it.
        self.configured.retain(|thread| thread != &start.thread);
        self.configured.push_back(start.thread.clone());
        if self.configured.len() > 1024 {
            self.configured.pop_front();
        }
        self.sequence += 1;
        let id = format!("gnome-map-context-{}-{}", std::process::id(), self.sequence);
        let mut params = start.configuration.clone();
        params["threadId"] = start.thread.clone().into();
        params["excludeTurns"] = true.into();
        self.effects.push_back(Effect::Send(
            json!({"id":id,"method":"thread/resume","params":params}),
        ));
        self.resumes.insert(id, start);
    }
    fn send(&mut self, start: Start) {
        if start.guarded {
            // The resume has already configured the backend even though no turn
            // exists yet. Retain that fact so a later no-Map send clears it.
            if start.version.is_some() {
                self.configured.retain(|thread| thread != &start.thread);
                self.configured.push_back(start.thread.clone());
                if self.configured.len() > 1024 {
                    self.configured.pop_front();
                }
            } else {
                self.configured.retain(|thread| thread != &start.thread);
            }
            self.effects.push_back(Effect::GuardedStart {
                id: start.id,
                params: start.params.clone(),
            });
            self.deferred.insert(start.id, start);
            return;
        }
        if let Err(message) = self.track(&start) {
            self.effects.push_back(Effect::Failed {
                id: start.id,
                message,
            });
            return;
        }
        self.effects.push_back(Effect::Start {
            id: start.id,
            params: start.params,
        });
    }
    fn track(&mut self, start: &Start) -> Result<(), String> {
        if self.turns.get(&start.thread).is_some_and(|t| !t.completed) {
            return Err(
                "A mapped turn became active before dispatch; the turn was not sent".into(),
            );
        }
        if let Some(version) = start.version.clone() {
            if self.turns.len() >= 32 && !self.turns.contains_key(&start.thread) {
                let evict = self
                    .turns
                    .iter()
                    .find(|(_, t)| t.completed && !t.busy)
                    .map(|(k, _)| k.clone());
                if let Some(key) = evict {
                    self.turns.remove(&key);
                } else {
                    return Err("Too many mapped turns are active; the turn was not sent".into());
                }
            }
            self.turns.insert(
                start.thread.clone(),
                Turn {
                    version,
                    turn: None,
                    update: None,
                    completed: false,
                    skip_fallback: false,
                    busy: false,
                },
            );
            if let Some(i) = self.configured.iter().position(|t| t == &start.thread) {
                self.configured.remove(i);
            }
            self.configured.push_back(start.thread.clone());
            if self.configured.len() > 1024 {
                self.configured.pop_front();
            }
            self.sent.insert(start.id, start.thread.clone());
        } else {
            self.turns.remove(&start.thread);
            self.configured.retain(|t| t != &start.thread);
        }
        Ok(())
    }
    pub fn release_guarded(&mut self, id: u64) -> Result<Value, String> {
        let start = self
            .deferred
            .remove(&id)
            .ok_or("Guarded Map preparation was cancelled or already consumed")?;
        self.track(&start)?;
        Ok(start.params)
    }
    pub fn cancel_start(&mut self, id: u64) {
        self.starts.remove(&id);
        self.deferred.remove(&id);
        let replies = self
            .resumes
            .iter()
            .filter(|(_, s)| s.id == id)
            .map(|(key, _)| key.clone())
            .collect::<std::collections::HashSet<_>>();
        self.resumes.retain(|_, s| s.id != id);
        self.effects.retain(|effect| match effect {
            Effect::Start { id: pending, .. }
            | Effect::GuardedStart { id: pending, .. }
            | Effect::Failed { id: pending, .. } => *pending != id,
            Effect::Send(value) => !value["id"]
                .as_str()
                .is_some_and(|key| replies.contains(key)),
            _ => true,
        });
    }
    pub fn cancel_guarded(&mut self, thread: &str) {
        let ids = self
            .starts
            .values()
            .chain(self.resumes.values())
            .chain(self.deferred.values())
            .filter(|s| s.guarded && s.thread == thread)
            .map(|s| s.id)
            .collect::<Vec<_>>();
        for id in ids {
            self.cancel_start(id);
        }
    }
    pub fn incoming(&mut self, payload: &Value, tool: Option<Tool>) -> bool {
        if payload["method"] == "studio/appServer/status" {
            let epoch = payload["params"]["generation"].as_u64();
            let changed = self.epoch.is_some() && epoch.is_some() && self.epoch != epoch;
            if changed
                || matches!(
                    payload["params"]["state"].as_str(),
                    Some("starting" | "error" | "stopped")
                )
            {
                self.reset();
            }
            if epoch.is_some() {
                self.epoch = epoch;
            }
        }
        if let Some(id) = payload["id"].as_str() {
            if let Some(mut start) = self.resumes.remove(id) {
                if let Some(error) = payload.get("error") {
                    let message = error["message"]
                        .as_str()
                        .unwrap_or("Map context was rejected");
                    let lower = message.to_ascii_lowercase();
                    if !start.fallback
                        && (error["code"] == -32602
                            || lower.contains("dynamictool")
                            || lower.contains("dynamic tool"))
                    {
                        start.fallback = true;
                        start
                            .configuration
                            .as_object_mut()
                            .unwrap()
                            .remove("dynamicTools");
                        self.resume(start);
                    } else {
                        self.effects.push_back(Effect::Failed {
                            id: start.id,
                            message: format!(
                                "Map context was not applied; the turn was not sent: {message}"
                            ),
                        });
                    }
                } else if payload.pointer("/result/thread/id").and_then(Value::as_str)
                    != Some(start.thread.as_str())
                {
                    self.effects.push_back(Effect::Failed {
                        id: start.id,
                        message: "Map context reply has the wrong thread; the turn was not sent"
                            .into(),
                    });
                } else {
                    self.send(start);
                }
                return true;
            }
            // Timed-out internal replies never become application responses.
            if id.starts_with("gnome-map-context-") {
                return true;
            }
        }
        if payload.get("method").is_none() {
            if let Some(id) = payload["id"].as_u64() {
                if let Some(thread) = self.sent.remove(&id) {
                    if payload.get("error").is_some() {
                        self.turns.remove(&thread);
                    } else if let Some(turn) =
                        payload.pointer("/result/turn/id").and_then(Value::as_str)
                    {
                        if let Some(t) = self.turns.get_mut(&thread) {
                            if t.turn.is_none() {
                                t.turn = Some(turn.into());
                            }
                        }
                    }
                }
            }
        }
        if let Some(tool) = tool {
            self.tool(tool);
            return true;
        }
        false
    }
    fn reset(&mut self) {
        let mut failed = std::collections::HashSet::new();
        failed.extend(self.starts.keys().copied());
        failed.extend(self.resumes.values().map(|s| s.id));
        failed.extend(self.deferred.keys().copied());
        failed.extend(self.sent.keys().copied());
        for effect in self.effects.drain(..) {
            if let Effect::Start { id, .. }
            | Effect::GuardedStart { id, .. }
            | Effect::Failed { id, .. } = effect
            {
                failed.insert(id);
            }
        }
        self.jobs = tokio::task::JoinSet::new();
        self.starts.clear();
        self.resumes.clear();
        self.deferred.clear();
        self.sent.clear();
        self.turns.clear();
        self.calls.clear();
        self.cached.clear();
        self.configured.clear();
        for id in failed {
            self.effects.push_back(Effect::Failed{id,message:"The backend changed during Map synchronization; inspect the conversation before retrying".into()});
        }
    }
    pub fn notice(&mut self, notice: Notice) {
        match notice {
            Notice::Started { thread, turn } => {
                if let Some(t) = self.turns.get_mut(&thread) {
                    if t.turn.is_none() {
                        t.turn = Some(turn);
                    }
                }
            }
            Notice::Message {
                thread,
                turn,
                update,
            } => {
                if let Some(t) = self.turns.get_mut(&thread) {
                    if t.turn.as_deref().is_none_or(|u| u == turn) && !t.completed {
                        t.turn = Some(turn);
                        t.update = update;
                    }
                }
            }
            Notice::Completed {
                thread,
                turn,
                completed,
            } => {
                if let Some(t) = self.turns.get_mut(&thread) {
                    if t.completed || t.turn.as_deref().is_some_and(|u| u != turn) {
                        return;
                    }
                    t.turn = Some(turn);
                    t.completed = true;
                    self.sent.retain(|_, source| source != &thread);
                    if !completed {
                        t.skip_fallback = true;
                    }
                    self.finish(&thread);
                }
            }
        }
    }
    fn finish(&mut self, thread: &str) {
        let Some(t) = self.turns.get_mut(thread) else {
            return;
        };
        if !t.completed || t.busy {
            return;
        }
        if t.skip_fallback {
            t.update = None;
            self.unblock(thread);
            return;
        }
        t.skip_fallback = true;
        let turn = t.turn.clone().unwrap_or_default();
        let update = match t.update.take() {
            Some(Ok(u)) => u,
            Some(Err(e)) => {
                self.changed_error(thread, e);
                self.unblock(thread);
                return;
            }
            None => {
                self.changed_error(thread,"This response did not include a Map update block; use Update with AI if needed".into());
                self.unblock(thread);
                return;
            }
        };
        if update.revision != t.version.revision {
            self.changed_error(thread, "The Map update used a stale revision".into());
            self.unblock(thread);
            return;
        }
        if update.operations.is_empty() {
            self.effects.push_back(Effect::Changed {
                thread: thread.into(),
                result: Ok(None),
                message: "This response did not require a Map change".into(),
            });
            self.unblock(thread);
            return;
        }
        t.busy = true;
        let version = t.version.clone();
        self.apply(thread.into(), turn, version, update.operations, None);
    }
    fn changed_error(&mut self, thread: &str, message: String) {
        self.effects.push_back(Effect::Changed {
            thread: thread.into(),
            result: Err(failure(message)),
            message: "Map update rejected".into(),
        });
    }
    fn unblock(&mut self, thread: &str) {
        let ids = self
            .starts
            .values()
            .filter(|s| s.thread == thread)
            .map(|s| s.id)
            .collect::<Vec<_>>();
        for id in ids {
            self.load(id);
        }
    }
    fn tool(&mut self, tool: Tool) {
        let Tool {
            id,
            thread,
            turn,
            key,
            fingerprint,
            operations,
        } = tool;
        let reject = |this: &mut Self, message: &str| {
            this.effects.push_back(Effect::Send(
                json!({"id":id,"result":tool_result(Err(message))}),
            ));
        };
        if !matches!(&id,Value::String(s)if s.len()<=256) && !id.is_number() {
            reject(self, "Invalid Map tool request identity");
            return;
        }
        if let Some(c) = self
            .cached
            .iter()
            .find(|c| c.thread == thread && c.turn == turn && c.key == key)
        {
            if c.fingerprint != fingerprint {
                reject(
                    self,
                    "Map tool identity was reused with different arguments",
                );
            } else {
                self.effects
                    .push_back(Effect::Send(json!({"id":id,"result":c.result})));
            }
            return;
        }
        let call_key = format!("{thread}\0{turn}\0{key}");
        if let Some(c) = self.calls.get_mut(&call_key) {
            if c.fingerprint != fingerprint {
                reject(
                    self,
                    "Map tool identity was reused with different arguments",
                );
            } else if c.ids.len() < 8 {
                c.ids.push(id);
            } else {
                reject(self, "Too many repeated Map tool requests");
            }
            return;
        }
        let Some(t) = self.turns.get_mut(&thread) else {
            reject(self, "This turn has no captured Session Map context");
            return;
        };
        if turn.is_empty()
            || turn.len() > 256
            || t.completed
            || t.turn.as_deref().is_some_and(|u| u != turn)
            || t.busy
            || self.jobs.len() >= 8
        {
            reject(
                self,
                "Map context is stale, completed, or another update is in progress",
            );
            return;
        }
        let operations = match operations {
            Ok(ops) if !ops.is_empty() => ops,
            Ok(_) => {
                reject(self, "No Map operations were provided");
                return;
            }
            Err(e) => {
                reject(self, &e);
                return;
            }
        };
        t.turn = Some(turn.clone());
        t.busy = true;
        let version = t.version.clone();
        let call = Call {
            thread: thread.clone(),
            turn: turn.clone(),
            key,
            ids: vec![id],
            fingerprint,
        };
        self.calls.insert(
            call_key,
            Call {
                thread: thread.clone(),
                turn: turn.clone(),
                key: call.key.clone(),
                ids: vec![],
                fingerprint,
            },
        );
        self.apply(thread, turn, version, operations, Some(call));
    }
    fn apply(
        &mut self,
        thread: String,
        turn: String,
        version: Version,
        operations: Vec<crate::session_map::types::MapOperation>,
        call: Option<Call>,
    ) {
        let api = self.api.clone();
        let request = data::Request::Apply {
            anchor: Anchor {
                backend: self.backend.clone(),
                thread: thread.clone(),
            },
            map_id: version.id,
            revision: version.revision,
            actor: data::Actor::Assistant,
            source_turn: Some(turn.clone()),
            operations,
        };
        self.jobs.spawn(async move {
            let result = data::execute(&api, request).await;
            Job::Applied {
                thread,
                turn,
                call,
                result,
            }
        });
    }
    fn completed(&mut self, job: Job) {
        match job {
            Job::Loaded { id, result } => {
                let Some(mut start) = self.starts.remove(&id) else {
                    return;
                };
                match result {
                    Err(message) => self.effects.push_back(Effect::Failed { id, message }),
                    Ok(Some((version, configuration))) => {
                        start.version = Some(version);
                        start.configuration = configuration;
                        self.resume(start);
                    }
                    Ok(None) => {
                        if self.configured.iter().any(|t| t == &start.thread) {
                            start.configuration =
                                json!({"developerInstructions":null,"dynamicTools":[]});
                            self.resume(start);
                        } else {
                            self.send(start);
                        }
                    }
                }
            }
            Job::Applied {
                thread,
                turn,
                call,
                result,
            } => {
                if let Some(t) = self
                    .turns
                    .get_mut(&thread)
                    .filter(|t| t.turn.as_deref() == Some(&turn))
                {
                    t.busy = false;
                    if let Ok(Some(s)) = &result {
                        t.version.revision = s.map.revision;
                        t.skip_fallback = true;
                    }
                    if result.as_ref().is_err_and(|e| e.conflict || e.uncertain) {
                        t.skip_fallback = true;
                    }
                }
                let message = match &result {
                    Ok(Some(s)) => format!("Session Map updated to revision {}.", s.map.revision),
                    Ok(None) => "Map update returned no snapshot".into(),
                    Err(e) => format!("Session Map update rejected: {}", e.message),
                };
                if let Some(mut call) = call {
                    let pending_key = format!("{}\0{}\0{}", call.thread, call.turn, call.key);
                    if let Some(extra) = self.calls.remove(&pending_key) {
                        call.ids.extend(extra.ids);
                    }
                    let response = tool_result(if result.is_ok() {
                        Ok(&message)
                    } else {
                        Err(&message)
                    });
                    for id in call.ids {
                        self.effects
                            .push_back(Effect::Send(json!({"id":id,"result":response})));
                    }
                    self.cached.push_back(Cached {
                        thread: call.thread,
                        turn: call.turn,
                        key: call.key,
                        result: response,
                        fingerprint: call.fingerprint,
                    });
                    if self.cached.len() > 128 {
                        self.cached.pop_front();
                    }
                }
                self.effects.push_back(Effect::Changed {
                    thread: thread.clone(),
                    result,
                    message,
                });
                self.finish(&thread);
            }
        }
    }
    pub async fn next(&mut self) -> Effect {
        loop {
            if let Some(effect) = self.effects.pop_front() {
                return effect;
            }
            if let Some(job) = self.jobs.join_next().await {
                match job {
                    Ok(job) => self.completed(job),
                    Err(_) => {
                        for (_, s) in self.starts.drain() {
                            self.effects.push_back(Effect::Failed {
                                id: s.id,
                                message: "Map worker stopped; the turn was not sent".into(),
                            });
                        }
                    }
                }
            } else {
                std::future::pending::<()>().await;
            }
        }
    }
    pub fn expire(&mut self) {
        let ids = self
            .starts
            .iter()
            .filter(|(_, s)| s.since.elapsed() > Duration::from_secs(35))
            .map(|(id, _)| *id)
            .collect::<Vec<_>>();
        for id in ids {
            self.starts.remove(&id);
            self.effects.push_back(Effect::Failed {
                id,
                message: "Map context timed out; the turn was not sent".into(),
            });
        }
        let ids = self
            .resumes
            .iter()
            .filter(|(_, s)| s.since.elapsed() > Duration::from_secs(35))
            .map(|(id, _)| id.clone())
            .collect::<Vec<_>>();
        for id in ids {
            if let Some(s) = self.resumes.remove(&id) {
                self.effects.push_back(Effect::Failed {
                    id: s.id,
                    message: "Map context timed out; the turn was not sent".into(),
                });
            }
        }
    }
}
