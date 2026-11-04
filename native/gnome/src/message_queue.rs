//! Durable queue data shares Studio's database; delivery state remains in memory.
use crate::{gateway::Api, model::text, turn_options::Options};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
};
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    pub id: String,
    pub text: String,
    #[serde(default)]
    pub input: Arc<Vec<Value>>,
    pub created_at: i64,
    #[serde(skip)]
    pub bytes: usize,
}
impl Message {
    pub fn validate(&self) -> Result<(), String> {
        if self.id.is_empty()
            || self.id.len() > 128
            || self.text.len() > 256 * 1024
            || self.text.encode_utf16().count() > 65536
            || self.input.len() > 32
            || self
                .input
                .iter()
                .any(|part| !part.is_object() || part["type"] == "text")
        {
            return Err(
                "Queued messages must contain at most 65,536 characters and 32 non-text references"
                    .into(),
            );
        }
        if self.text.trim().is_empty() && self.input.is_empty() {
            return Err("A queued message cannot be empty".into());
        }
        Ok(())
    }
}
#[derive(Debug, Clone)]
pub struct Context {
    pub backend: String,
    pub thread: String,
    pub root: String,
    pub kind: String,
    pub generation: u64,
}
impl Context {
    pub fn key(&self) -> String {
        format!("{}:{}", self.backend, self.thread)
    }
}
#[derive(Debug)]
pub enum Request {
    Read,
    Save(Vec<Arc<Message>>),
    Prepare,
}
#[derive(Debug)]
pub enum Reply {
    Read {
        messages: Vec<Arc<Message>>,
        limit: usize,
    },
    Saved,
    Prepared(Options),
}
pub async fn execute(api: &Api, key: &str, request: Request) -> Result<Reply, String> {
    let query = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("sessionKey", key)
        .finish();
    let path = format!("/studio/session-state/message-queue?{query}");
    match request {
        Request::Read => {
            let (raw, preferences) = tokio::try_join!(
                api.json(reqwest::Method::GET, &path, None),
                api.json(reqwest::Method::GET, "/studio/preferences", None)
            )?;
            crate::worker::compute(move || {
                let mut messages: Vec<Message> =
                    serde_json::from_value(raw).map_err(|_| "Stored queue is invalid")?;
                if messages.len() > 3 {
                    return Err("Stored queue exceeds three messages".into());
                }
                for message in &mut messages {
                    message.validate()?;
                    message.bytes = serde_json::to_vec(message)
                        .map_err(|error| error.to_string())?
                        .len();
                }
                Ok(Reply::Read {
                    messages: messages.into_iter().map(Arc::new).collect(),
                    limit: preferences["queueDepth"].as_u64().unwrap_or(1).clamp(1, 3) as usize,
                })
            })
            .await
        }
        Request::Save(messages) => {
            let key = key.to_owned();
            let body =
                crate::worker::compute(move || Ok(json!({"sessionKey":key,"messages":messages})))
                    .await?;
            api.json(
                reqwest::Method::PUT,
                "/studio/session-state/message-queue",
                Some(&body),
            )
            .await?;
            Ok(Reply::Saved)
        }
        Request::Prepare => {
            let value = api
                .json(
                    reqwest::Method::GET,
                    &format!("/studio/session-state/turn-options?{query}"),
                    None,
                )
                .await?;
            serde_json::from_value(value)
                .map(Reply::Prepared)
                .map_err(|_| "Invalid queued turn options".into())
        }
    }
}
pub struct Saving {
    pub serial: u64,
    pub messages: Vec<Arc<Message>>,
    pub original: Option<String>,
    pub acknowledged: bool,
}
pub struct Flight {
    pub id: String,
    pub generation: u64,
    pub turn: Option<String>,
    pub completed: bool,
    pub acknowledged: bool,
    pub persisted: bool,
}
pub struct Entry {
    pub router: bool,
    pub router_wait: Option<String>,
    pub context: Context,
    pub messages: Vec<Arc<Message>>,
    pub limit: usize,
    pub loaded: bool,
    pub load_serial: u64,
    pub flow_epoch: u64,
    pub paused: bool,
    pub error: Option<String>,
    pub saving: Option<Saving>,
    pub preparing: Option<u64>,
    pub flight: Option<Flight>,
    pub active: bool,
    pub released: bool,
    pub connected: bool,
    pub revision: u64,
    pub editing: bool,
    finished: VecDeque<String>,
}
impl Entry {
    pub fn new(context: Context, serial: u64) -> Self {
        Self {
            router: false,
            router_wait: None,
            context,
            messages: Vec::new(),
            limit: 1,
            flow_epoch: 0,
            loaded: false,
            load_serial: serial,
            paused: true,
            error: None,
            saving: None,
            preparing: None,
            flight: None,
            active: false,
            released: false,
            connected: true,
            revision: 1,
            editing: false,
            finished: VecDeque::new(),
        }
    }
    pub fn touch(&mut self) {
        self.revision = self.revision.wrapping_add(1);
    }
    pub fn busy(&self) -> bool {
        self.preparing.is_some()
            || self.saving.is_some()
            || self
                .flight
                .as_ref()
                .is_some_and(|flight| !flight.acknowledged)
    }
    pub fn pause(&mut self, reason: &str) {
        self.paused = true;
        if !reason.is_empty()
            && (!self.loaded
                || !self.messages.is_empty()
                || self.saving.is_some()
                || self.flight.is_some())
        {
            self.error = Some(reason.into());
        }
        self.touch();
    }
    pub fn can_prepare(&self) -> bool {
        self.loaded
            && self.connected
            && !self.paused
            && !self.active
            && self.released
            && !self.editing
            && !self.messages.is_empty()
            && !self.busy()
            && self.flight.is_none()
            && self.router_wait.is_none()
    }
    pub fn store(
        &mut self,
        serial: u64,
        messages: Vec<Arc<Message>>,
        original: Option<String>,
        acknowledged: bool,
    ) -> Result<Request, String> {
        if self.saving.is_some() || (!acknowledged && self.busy()) {
            return Err("Wait for the pending queue change".into());
        }
        self.saving = Some(Saving {
            serial,
            messages: messages.clone(),
            original,
            acknowledged,
        });
        self.touch();
        Ok(Request::Save(messages))
    }
    pub fn stored(&mut self, serial: u64, result: Result<(), String>) -> Option<String> {
        if self
            .saving
            .as_ref()
            .is_none_or(|saving| saving.serial != serial)
        {
            return None;
        }
        let saving = self.saving.take().unwrap();
        match result {
            Ok(()) => {
                self.messages = saving.messages;
                if saving.acknowledged {
                    if let Some(flight) = &mut self.flight {
                        flight.persisted = true;
                    }
                }
                self.settle();
                self.touch();
                saving.original
            }
            Err(error) => {
                self.pause(&if saving.acknowledged{format!("Message accepted, but queue could not be saved. Check the conversation before resuming: {error}")}else{error});
                self.flight = None;
                None
            }
        }
    }
    pub fn settle(&mut self) {
        if self
            .flight
            .as_ref()
            .is_some_and(|flight| flight.acknowledged && flight.persisted && flight.completed)
        {
            self.flight = None;
        }
    }
    pub fn observe(&mut self, method: &str, params: &Value) {
        if self.router {
            return;
        }
        let turn = params
            .pointer("/turn/id")
            .and_then(Value::as_str)
            .or(params["turnId"].as_str())
            .unwrap_or("");
        match method {
            "turn/started" => {
                if !turn.is_empty() && self.finished.iter().any(|id| id == turn) {
                    return;
                }
                self.active = true;
                self.released = false;
                if let Some(flight) = &mut self.flight {
                    if flight.turn.is_none() {
                        flight.turn = Some(turn.into());
                    }
                }
            }
            "turn/completed" => {
                if !turn.is_empty() && self.finished.iter().any(|id| id == turn) {
                    return;
                }
                let was_active = self.active;
                self.active = false;
                let status = params
                    .pointer("/turn/status")
                    .and_then(Value::as_str)
                    .unwrap_or(if self.context.kind == "opencode" {
                        "completed"
                    } else {
                        "unknown"
                    });
                if status == "completed" {
                    if was_active || !turn.is_empty() {
                        self.released = true;
                    }
                } else {
                    self.pause("Previous turn did not complete normally");
                }
                if let Some(flight) = &mut self.flight {
                    if flight.turn.as_deref() == Some(turn) || turn.is_empty() && was_active {
                        flight.completed = true;
                    }
                }
                if !turn.is_empty() {
                    self.finished.push_back(turn.into());
                    if self.finished.len() > 64 {
                        self.finished.pop_front();
                    }
                }
                self.settle();
            }
            "thread/status/changed" => {
                let status = params["status"]
                    .as_str()
                    .unwrap_or_else(|| text(&params["status"], "type"));
                if matches!(status, "active" | "running" | "busy" | "retry") {
                    self.active = true;
                    self.released = false;
                } else if status == "idle" {
                    self.active = false;
                } else if matches!(status, "failed" | "systemError") {
                    self.pause("Session failed; check it before resuming");
                }
            }
            "error" => self.pause("Turn failed; check the conversation before resuming"),
            _ => return,
        }
        self.touch();
    }
    /// A controller turn ending is not a routed delivery ending. Only the
    /// durable, controller-scoped Router job can release this queue gate.
    pub fn observe_router(&mut self, id: &str, status: &str) {
        if !self.router {
            return;
        }
        let terminal = matches!(status, "completed" | "clarify" | "failed" | "stopped");
        let waiting = self.router_wait.as_deref() == Some(id);
        let flying = self.flight.as_ref().is_some_and(|flight| flight.id == id);
        if !waiting && !flying {
            return;
        }
        if flying {
            let flight = self.flight.as_mut().unwrap();
            flight.acknowledged = true;
            // The gateway removes this exact head in the intent transaction.
            flight.persisted = true;
            flight.turn = Some(id.into());
            flight.completed = terminal;
            self.messages.retain(|message| message.id != id);
        }
        self.active = !terminal;
        self.released = status == "completed";
        if terminal {
            if waiting {
                self.router_wait = None;
            }
            if status != "completed" {
                self.pause("Router request did not complete normally. Check the reply before resuming the queue.");
            }
            self.settle();
        }
        self.touch();
    }
}
#[derive(Default)]
pub struct Book {
    pub entries: HashMap<String, Entry>,
    pub sequence: u64,
    pub dirty: bool,
}
impl Book {
    pub fn serial(&mut self) -> u64 {
        self.sequence += 1;
        self.sequence
    }
    pub fn pause_backend(&mut self, backend: &str, reason: &str) {
        for entry in self
            .entries
            .values_mut()
            .filter(|entry| entry.context.backend == backend)
        {
            entry.connected = false;
            entry.active = false;
            if !entry.router {
                entry.flight = None;
            }
            entry.preparing = None;
            entry.pause(reason);
        }
        self.dirty = true;
    }
    pub fn prune(&mut self, selected: &str) {
        while self.entries.len() > 128
            || self
                .entries
                .values()
                .flat_map(|entry| entry.messages.iter())
                .map(|message| message.bytes)
                .sum::<usize>()
                > 32 * 1024 * 1024
        {
            let candidate = self
                .entries
                .iter()
                .filter(|(key, entry)| {
                    key.as_str() != selected
                        && (entry.paused || entry.messages.is_empty())
                        && !entry.busy()
                        && entry.flight.is_none()
                        && !entry.editing
                })
                .min_by_key(|(_, entry)| entry.load_serial)
                .map(|(key, _)| key.clone());
            if let Some(key) = candidate {
                self.entries.remove(&key);
            } else {
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn router_queue_waits_for_owned_delivery_and_remains_paused_on_failure_or_reconnect() {
        let mut entry = entry();
        entry.router = true;
        entry.router_wait = Some("manual".into());
        entry.observe(
            "turn/completed",
            &json!({"turn":{"id":"controller","status":"completed"}}),
        );
        assert!(!entry.can_prepare());
        entry.observe_router("foreign", "completed");
        assert!(entry.active);
        entry.observe_router("manual", "completed");
        assert!(entry.can_prepare());
        entry.flight = Some(Flight {
            id: "one".into(),
            generation: 3,
            turn: None,
            completed: false,
            acknowledged: false,
            persisted: false,
        });
        entry.observe_router("one", "running");
        assert_eq!(entry.messages[0].id, "two");
        assert!(!entry.can_prepare());
        let mut book = Book::default();
        book.entries.insert("codex:thread".into(), entry);
        book.pause_backend("codex", "Disconnected");
        let entry = book.entries.get_mut("codex:thread").unwrap();
        assert!(entry.flight.is_some());
        entry.observe_router("one", "completed");
        assert!(entry.flight.is_none() && entry.paused);
        assert!(!entry.can_prepare());
        entry.connected = true;
        entry.paused = false;
        entry.router_wait = Some("failed".into());
        entry.observe_router("failed", "clarify");
        assert!(entry.paused && entry.router_wait.is_none());
        assert_eq!(entry.messages[0].id, "two");
    }
    fn message(id: &str) -> Arc<Message> {
        Arc::new(Message {
            id: id.into(),
            text: format!("queued {id}"),
            input: Arc::new(Vec::new()),
            created_at: 1,
            bytes: 100,
        })
    }
    fn entry() -> Entry {
        let mut entry = Entry::new(
            Context {
                backend: "codex".into(),
                thread: "thread".into(),
                root: "/tmp/project".into(),
                kind: "codex".into(),
                generation: 3,
            },
            1,
        );
        entry.loaded = true;
        entry.messages = vec![message("one"), message("two")];
        entry.paused = false;
        entry.active = true;
        entry
    }
    #[test]
    fn completed_before_ack_waits_for_durable_dequeue_and_ignores_duplicates() {
        let mut entry = entry();
        entry.observe(
            "turn/completed",
            &json!({"turn":{"id":"previous","status":"completed"}}),
        );
        assert!(entry.can_prepare());
        entry.flight = Some(Flight {
            id: "one".into(),
            generation: 3,
            turn: None,
            completed: false,
            acknowledged: false,
            persisted: false,
        });
        entry.released = false;
        entry.observe("turn/started", &json!({"turn":{"id":"queued"}}));
        entry.observe(
            "turn/completed",
            &json!({"turn":{"id":"queued","status":"completed"}}),
        );
        assert!(!entry.can_prepare());
        entry.flight.as_mut().unwrap().acknowledged = true;
        entry.store(2, vec![message("two")], None, true).unwrap();
        assert!(!entry.can_prepare());
        entry.stored(2, Ok(()));
        assert!(entry.can_prepare());
        assert_eq!(entry.messages[0].id, "two");
        entry.observe("turn/started", &json!({"turn":{"id":"next"}}));
        entry.observe(
            "turn/completed",
            &json!({"turn":{"id":"previous","status":"completed"}}),
        );
        assert!(entry.active);
        assert!(!entry.can_prepare());
    }
    #[test]
    fn stop_disconnect_restart_and_failures_never_advance_automatically() {
        let mut entry = entry();
        entry.pause("Stopped");
        entry.observe(
            "turn/completed",
            &json!({"turn":{"id":"previous","status":"completed"}}),
        );
        assert!(!entry.can_prepare());
        entry.paused = false;
        entry.active = true;
        entry.observe(
            "turn/completed",
            &json!({"turn":{"id":"failed","status":"failed"}}),
        );
        assert!(entry.paused);
        assert!(!entry.can_prepare());
        let mut book = Book::default();
        book.entries.insert("codex:thread".into(), entry);
        book.pause_backend("codex", "disconnected");
        let entry = &book.entries["codex:thread"];
        assert!(!entry.connected && entry.paused);
        let restored = Entry::new(entry.context.clone(), 4);
        assert!(restored.paused);
        assert!(!restored.can_prepare());
    }
    #[test]
    fn failed_persistence_retains_messages_draft_and_references() {
        let mut entry = entry();
        let original = entry.messages.clone();
        entry
            .store(2, vec![message("new")], Some("new draft".into()), false)
            .unwrap();
        assert!(entry.stored(2, Err("quota exceeded".into())).is_none());
        assert_eq!(entry.messages[0].id, original[0].id);
        assert!(entry.paused);
        let refs = Arc::new(vec![
            json!({"type":"skill","path":"/tmp/skill/SKILL.md","name":"test"}),
        ]);
        let mut message = (*message("reference")).clone();
        message.input = refs.clone();
        message.text = "edited".into();
        message.validate().unwrap();
        assert!(Arc::ptr_eq(&refs, &message.input));
        entry.store(3, Vec::new(), None, true).unwrap();
        entry.stored(3, Err("disk unavailable".into()));
        assert!(entry.error.as_deref().unwrap().contains("accepted"));
        assert!(!entry.messages.is_empty());
    }
}
