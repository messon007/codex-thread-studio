//! Connection-owned supervision coordinator. GTK supplies explicit opt-in and
//! current source options/policies; the host prepares ordinary environment/Map
//! context, then this worker re-reads authority immediately before wire dispatch.
use crate::{
    conversation_pages::Source,
    gateway::Api,
    session_search::Context,
    translation,
    turn_supervision::{self as core, Decision, Follow, Job, Snapshot, Stage, Ticket},
};
use serde_json::Value;
use std::{
    collections::{BTreeMap, HashMap, VecDeque},
    future::Future,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

const ENTRIES: usize = 16;
const WORKERS: usize = 2;
const PREPARE_TIMEOUT: Duration = Duration::from_secs(40);
const ACK_TIMEOUT: Duration = Duration::from_secs(35);
// Disjoint from ordinary low native request IDs and exactly representable by JS.
static REQUEST: AtomicU64 = AtomicU64::new(1 << 52);
fn request_id() -> Result<u64, String> {
    REQUEST
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| {
            (n < (1 << 53) - 1).then_some(n + 1)
        })
        .map_err(|_| "Supervision request identities exhausted".into())
}
#[derive(Clone, Debug)]
pub struct Start {
    pub request: u64,
    pub context: translation::Context,
    pub turn: String,
    pub effort: String,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct View {
    pub request: u64,
    pub context: Context,
    pub root: String,
    pub current: String,
    pub stage: Stage,
    pub count: usize,
    pub reason: String,
    pub warning: String,
    pub holding: bool,
}
#[derive(Debug)]
pub enum Effect {
    Changed(View),
    /// Host applies current source options/access, environment and Session Map.
    Prepare {
        id: u64,
        request: u64,
        context: Context,
        root: String,
        text: String,
    },
    /// Call `dispatch(id)` immediately before writing. Never cache/replay params.
    Ready {
        id: u64,
    },
    /// Clear any unsent Map preparation by settling its ordinary request identity.
    Abandon {
        id: u64,
        reason: String,
    },
    /// Process even after Stop; a successful source acknowledgement is real.
    Follow {
        id: u64,
        receipt: Follow,
        prompt: String,
    },
}
pub trait Service: Clone + Send + Sync + 'static {
    fn read(
        &self,
        source: Source,
        stop: Arc<AtomicBool>,
    ) -> impl Future<Output = Result<Snapshot, String>> + Send;
    fn evaluate(
        &self,
        id: u64,
        context: translation::Context,
        effort: String,
        input: String,
        stop: Arc<AtomicBool>,
    ) -> impl Future<Output = Result<(Decision, Option<String>), String>> + Send;
}
#[derive(Clone)]
pub struct Worker {
    api: Api,
}
impl Worker {
    pub fn new(api: Api) -> Self {
        Self { api }
    }
}
impl Service for Worker {
    async fn read(&self, source: Source, stop: Arc<AtomicBool>) -> Result<Snapshot, String> {
        crate::turn_supervision_io::read(&self.api, &source, &stop).await
    }
    async fn evaluate(
        &self,
        id: u64,
        context: translation::Context,
        effort: String,
        input: String,
        stop: Arc<AtomicBool>,
    ) -> Result<(Decision, Option<String>), String> {
        crate::turn_supervision_io::evaluate(&self.api, id, &context, effort, input, &stop).await
    }
}
#[derive(Clone)]
enum Work {
    Read,
    Evaluate(Ticket),
    Final(u64, Ticket),
}
struct Task {
    key: String,
    owner: u64,
    work: Work,
}
enum ResultValue {
    Read(Result<Snapshot, String>),
    Evaluated(Result<(Decision, Option<String>, Snapshot), String>),
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Host,
    Reading,
    Ready,
    Ack,
}
struct Pending {
    id: u64,
    ticket: Ticket,
    phase: Phase,
    params: Option<Value>,
    text: String,
    since: Instant,
}
struct Entry {
    owner: u64,
    start: Start,
    source: Source,
    job: Option<Job>,
    stopped: Option<String>,
    busy: bool,
    cancel: Arc<AtomicBool>,
    pending: Option<Pending>,
    handoff: Option<u64>,
    warning: String,
}
impl Entry {
    fn active(&self) -> bool {
        self.stopped.is_none() && self.job.as_ref().is_none_or(Job::active)
    }
    fn unsettled(&self) -> bool {
        self.busy
            || self.pending.is_some()
            || self.handoff.is_some()
            || self.job.as_ref().is_some_and(Job::unsettled)
    }
    fn view(&self) -> View {
        let job = self.job.as_ref();
        View {
            request: self.start.request,
            context: self.source.context.clone(),
            root: job.map_or_else(|| self.start.turn.clone(), |j| j.root_turn().into()),
            current: job.map_or_else(|| self.start.turn.clone(), |j| j.current_turn().into()),
            stage: job.map_or(
                if self.stopped.is_some() {
                    Stage::Stopped
                } else {
                    Stage::Waiting
                },
                Job::stage,
            ),
            count: job.map_or(0, Job::continuation_count),
            reason: display(job.map_or_else(|| self.stopped.as_deref().unwrap_or(""), Job::reason)),
            warning: display(&self.warning),
            holding: self.active() || self.unsettled(),
        }
    }
}
fn display(text: &str) -> String {
    text.chars()
        .take(4096)
        .map(|c| if c == '\0' { '�' } else { c })
        .collect()
}
pub struct Runtime<S: Service = Worker> {
    service: S,
    backend: String,
    generation: u64,
    owner: u64,
    entries: BTreeMap<String, Entry>,
    jobs: tokio::task::JoinSet<ResultValue>,
    tasks: HashMap<tokio::task::Id, Task>,
    effects: VecDeque<Effect>,
    changed: BTreeMap<String, View>,
    cursor: Option<String>,
    closing: bool,
}
impl<S: Service> Drop for Runtime<S> {
    fn drop(&mut self) {
        for entry in self.entries.values() {
            entry.cancel.store(true, Ordering::Release);
        }
        // The host normally drains shutdown. An early scope departure must still
        // allow an already-owned utility to run its cancellation/cleanup path.
        self.jobs.detach_all();
    }
}
impl<S: Service> Runtime<S> {
    pub fn new(service: S, backend: String, generation: u64) -> Self {
        Self {
            service,
            backend,
            generation,
            owner: 0,
            entries: BTreeMap::new(),
            jobs: tokio::task::JoinSet::new(),
            tasks: HashMap::new(),
            effects: VecDeque::new(),
            changed: BTreeMap::new(),
            cursor: None,
            closing: false,
        }
    }
    pub fn has_work(&self) -> bool {
        !self.jobs.is_empty() || !self.effects.is_empty() || !self.changed.is_empty()
    }
    fn changed(&mut self, key: &str) {
        if let Some(e) = self.entries.get(key) {
            self.changed.insert(key.into(), e.view());
        }
    }
    pub fn holding(&self, thread: &str) -> bool {
        self.entries
            .get(thread)
            .is_some_and(|e| e.active() || e.unsettled())
    }
    pub fn stop_all(&mut self, reason: &str) {
        let keys = self.entries.keys().cloned().collect::<Vec<_>>();
        for key in keys {
            self.stop(&key, reason);
        }
    }
    pub fn preparation(&self, id: u64) -> Option<(Source, String)> {
        self.entries.values().find_map(|e| {
            let p = e
                .pending
                .as_ref()
                .filter(|p| p.id == id && p.phase == Phase::Host)?;
            e.active().then(|| (e.source.clone(), p.text.clone()))
        })
    }
    pub fn source(&self, thread: &str) -> Option<Source> {
        self.entries.get(thread).map(|e| e.source.clone())
    }
    pub fn preparation_turn(&self, id: u64) -> Option<String> {
        self.entries.values().find_map(|e| {
            e.pending
                .as_ref()
                .filter(|p| p.id == id && p.phase == Phase::Host)
                .map(|p| p.ticket.turn().to_owned())
        })
    }
    pub fn awaiting_ack(&self) -> bool {
        self.entries
            .values()
            .any(|e| e.pending.as_ref().is_some_and(|p| p.phase == Phase::Ack))
    }
    pub fn begin_shutdown(&mut self) {
        if !self.closing {
            self.closing = true;
            self.stop_all("Studio closed or disconnected");
        }
    }
    pub fn unsent(&self, thread: &str) -> Option<u64> {
        self.entries
            .get(thread)?
            .pending
            .as_ref()
            .filter(|p| p.phase != Phase::Ack)
            .map(|p| p.id)
    }
    /// Notifications supply guards, never completion evidence for evaluation.
    pub fn observe(&mut self, payload: &Value, attention: bool) -> bool {
        if payload.get("method").is_none() {
            if let Some(id) = payload["id"].as_u64() {
                let result = if let Some(error) = payload.get("error") {
                    Err(error["message"]
                        .as_str()
                        .unwrap_or("Supervised source request failed")
                        .into())
                } else {
                    payload
                        .pointer("/result/turn/id")
                        .and_then(Value::as_str)
                        .map(str::to_owned)
                        .ok_or_else(|| "No source acknowledgement; inspect the conversation".into())
                };
                return self.acknowledge(id, result);
            }
            return false;
        }
        let p = &payload["params"];
        let Some(thread) = p["threadId"].as_str() else {
            return false;
        };
        let Some(e) = self.entries.get(thread) else {
            return false;
        };
        let method = payload["method"].as_str().unwrap_or("");
        let changed = method == "turn/started"
            && !e.pending.as_ref().is_some_and(|p| p.phase == Phase::Ack)
            && p.pointer("/turn/id").and_then(Value::as_str)
                != e.job.as_ref().map(Job::current_turn);
        let failed = method == "error"
            || method == "thread/archived"
            || method == "thread/deleted"
            || method == "turn/completed"
                && (p.pointer("/turn/status").is_some_and(|s| s != "completed")
                    || p.pointer("/turn/error").is_some_and(|s| !s.is_null()));
        let flags = method == "thread/status/changed"
            && p.pointer("/status/activeFlags")
                .is_some_and(|v| v.as_array().is_none_or(|a| !a.is_empty()));
        if attention || flags {
            self.stop(thread, "Connection or approval requires attention");
        } else if changed || failed {
            self.stop(thread, "The conversation changed or stopped");
        }
        false
    }
    pub fn start(&mut self, start: Start) -> Result<(), String> {
        let c = &start.context;
        if self.closing
            || start.request == 0
            || c.backend.kind != "codex"
            || c.backend.id != self.backend
            || c.socket != self.generation
            || !c.ready
            || c.thread.is_empty()
            || c.thread.len() > 256
            || c.thread.chars().any(char::is_control)
            || start.turn.is_empty()
            || start.turn.len() > 256
            || start.turn.chars().any(char::is_control)
            || c.cwd.is_empty()
            || c.cwd.len() > 4096
            || c.cwd.contains('\0')
            || c.model.len() > 256
            || start.effort.len() > 64
        {
            return Err("Supervision requires the current running Codex conversation".into());
        }
        let key = c.thread.clone();
        if self.holding(&key) {
            return Err("This session already has supervised work or an unsettled receipt".into());
        }
        if !self.entries.contains_key(&key) && self.entries.len() >= ENTRIES {
            let evict = self
                .entries
                .iter()
                .find(|(_, e)| !e.active() && !e.unsettled())
                .map(|(k, _)| k.clone())
                .ok_or("Too many supervised sessions; stop an existing supervisor first")?;
            self.entries.remove(&evict);
            self.changed.remove(&evict);
        }
        self.owner = self
            .owner
            .checked_add(1)
            .ok_or("Supervision ownership exhausted")?;
        let source = Source {
            context: Context {
                backend: self.backend.clone(),
                thread: key.clone(),
                generation: self.generation,
            },
            backend_kind: "codex".into(),
            directory: c.cwd.clone(),
        };
        self.entries.insert(
            key.clone(),
            Entry {
                owner: self.owner,
                start,
                source,
                job: None,
                stopped: None,
                busy: false,
                cancel: Arc::new(AtomicBool::new(false)),
                pending: None,
                handoff: None,
                warning: String::new(),
            },
        );
        self.changed(&key);
        self.tick();
        Ok(())
    }
    /// A source option change never changes identity or resumes a stopped job.
    pub fn options(&mut self, thread: &str, model: String, effort: String) -> Result<(), String> {
        if model.len() > 256 || effort.len() > 64 {
            return Err("Invalid supervised source options".into());
        }
        let mut changed = false;
        if let Some(e) = self.entries.get_mut(thread) {
            changed = e.start.context.model != model || e.start.effort != effort;
            e.start.context.model = model;
            e.start.effort = effort;
        }
        if changed {
            self.invalidate_preparation(thread);
        }
        Ok(())
    }
    /// Host invokes this for changes to Fast, access or other source-send options.
    /// Already-written turns retain their receipt; future preparation uses new values.
    pub fn invalidate_preparation(&mut self, thread: &str) {
        if self
            .entries
            .get(thread)
            .is_some_and(|e| e.pending.as_ref().is_some_and(|p| p.phase != Phase::Ack))
        {
            self.stop(
                thread,
                "Source options or permissions changed during preparation",
            );
        }
    }
    pub fn stop(&mut self, thread: &str, reason: &str) {
        let mut abandoned = None;
        if let Some(e) = self.entries.get_mut(thread) {
            if e.active() {
                e.stopped = Some(reason.into());
                if let Some(j) = e.job.as_mut() {
                    j.stop(reason);
                }
            }
            e.cancel.store(true, Ordering::Release);
            if e.pending
                .as_ref()
                .is_some_and(|p| p.phase != Phase::Ack && p.phase != Phase::Reading)
            {
                let p = e.pending.take().unwrap();
                if let Some(j) = e.job.as_mut() {
                    j.acknowledged(&p.ticket, Err(reason.into()));
                }
                abandoned = Some(p.id);
            }
        }
        if let Some(id) = abandoned {
            self.effects.push_back(Effect::Abandon {
                id,
                reason: reason.into(),
            });
        }
        self.changed(thread);
    }
    pub fn human_action(&mut self, thread: &str, method: &str, input: &Value) {
        if !matches!(
            method,
            "turn/steer"
                | "turn/start"
                | "turn/interrupt"
                | "thread/archive"
                | "thread/delete"
                | "thread/rollback"
                | "thread/compact/start"
                | "thread/shellCommand"
        ) {
            return;
        }
        let steer = method == "turn/steer";
        if let Some(e) = self.entries.get_mut(thread) {
            if let Some(j) = e.job.as_mut() {
                j.human_action(method, input);
            } else {
                self.stop(thread, "Stopped by user activity");
                return;
            }
            if !steer || !e.job.as_ref().unwrap().active() {
                self.stop(thread, "Stopped by user activity");
                return;
            }
            // The old evaluation retains its lease but is asked to clean up now.
            e.cancel.store(true, Ordering::Release);
            if !e.busy {
                e.cancel = Arc::new(AtomicBool::new(false));
            }
            if let Some(p) = e.pending.take() {
                self.effects.push_back(Effect::Abandon {
                    id: p.id,
                    reason: "Stopped by user activity".into(),
                });
            }
        }
        self.changed(thread);
    }
    fn spawn(&mut self, key: &str, work: Work) {
        let Some(e) = self.entries.get_mut(key) else {
            return;
        };
        e.busy = true;
        e.cancel = Arc::new(AtomicBool::new(false));
        let stop = e.cancel.clone();
        let source = e.source.clone();
        let service = self.service.clone();
        let task = Task {
            key: key.into(),
            owner: e.owner,
            work: work.clone(),
        };
        let handle = self
            .jobs
            .spawn(async move { ResultValue::Read(service.read(source, stop).await) });
        self.tasks.insert(handle.id(), task);
    }
    pub fn tick(&mut self) {
        if self.closing {
            return;
        }
        let expired = self
            .entries
            .iter()
            .filter_map(|(k, e)| {
                e.pending
                    .as_ref()
                    .filter(|p| {
                        p.since.elapsed()
                            > if p.phase == Phase::Ack {
                                ACK_TIMEOUT
                            } else {
                                PREPARE_TIMEOUT
                            }
                    })
                    .map(|_| k.clone())
            })
            .collect::<Vec<_>>();
        for key in expired {
            if let Some(e) = self.entries.get_mut(&key) {
                if let Some(p) = e.pending.as_ref().filter(|p| p.phase == Phase::Ack) {
                    let id = p.id;
                    self.acknowledge(
                        id,
                        Err(
                            "No send acknowledgement; inspect the conversation before continuing"
                                .into(),
                        ),
                    );
                } else {
                    self.stop(
                        &key,
                        "Source preparation timed out; no continuation was sent",
                    );
                }
            }
        }
        let mut keys = self.entries.keys().cloned().collect::<Vec<_>>();
        if let Some(cursor) = &self.cursor {
            let split = keys.partition_point(|k| k <= cursor);
            keys.rotate_left(split);
        }
        for key in keys {
            if self.jobs.len() >= WORKERS {
                break;
            }
            let eligible = self.entries.get(&key).is_some_and(|e| {
                e.active()
                    && !e.busy
                    && e.pending.is_none()
                    && e.handoff.is_none()
                    && e.job
                        .as_ref()
                        .is_none_or(|j| j.stage() == Stage::Waiting && !j.unsettled())
            });
            if eligible {
                self.cursor = Some(key.clone());
                self.spawn(&key, Work::Read);
            }
        }
    }
    /// Host calls after ordinary environment/Map preparation, using source options.
    /// This starts the final authoritative read; it does not release a turn yet.
    pub fn prepared(&mut self, id: u64, params: Value) -> bool {
        let key = self
            .entries
            .iter()
            .find(|(_, e)| {
                e.pending
                    .as_ref()
                    .is_some_and(|p| p.id == id && p.phase == Phase::Host)
            })
            .map(|(k, _)| k.clone());
        let Some(key) = key else {
            return false;
        };
        let e = self.entries.get_mut(&key).unwrap();
        let active = e.active();
        let p = e.pending.as_mut().unwrap();
        let expected = e.job.as_ref().and_then(|j| j.preparing_text(&p.ticket));
        let valid = active
            && params["threadId"].as_str() == Some(key.as_str())
            && params["input"].as_array().is_some_and(|a| {
                a.len() == 1
                    && a[0]["type"] == "text"
                    && a[0]["text"].as_str() == expected.as_deref()
            })
            && params.to_string().len() <= 32 * 1024;
        if !valid {
            self.stop(
                &key,
                "Supervised source parameters changed; no continuation was sent",
            );
            return false;
        }
        let e = self.entries.get_mut(&key).unwrap();
        let p = e.pending.as_mut().unwrap();
        p.params = Some(params);
        p.phase = Phase::Reading;
        p.since = Instant::now();
        let ticket = p.ticket.clone();
        self.spawn(&key, Work::Final(id, ticket));
        true
    }
    /// Single-use wire admission. No other method returns sendable parameters.
    pub fn dispatch(&mut self, id: u64) -> Option<Value> {
        let key = self
            .entries
            .iter()
            .find(|(_, e)| {
                e.pending
                    .as_ref()
                    .is_some_and(|p| p.id == id && p.phase == Phase::Ready)
            })
            .map(|(k, _)| k.clone())?;
        if !self.entries[&key].active() {
            self.stop(&key, "Supervision stopped before dispatch");
            return None;
        }
        let p = self.entries.get_mut(&key)?.pending.as_mut()?;
        p.phase = Phase::Ack;
        p.since = Instant::now();
        p.params.take()
    }
    pub fn preparation_failed(&mut self, id: u64, reason: String) -> bool {
        let key = self
            .entries
            .iter()
            .find(|(_, e)| {
                e.pending
                    .as_ref()
                    .is_some_and(|p| p.id == id && p.phase != Phase::Ack)
            })
            .map(|(k, _)| k.clone());
        if let Some(key) = key {
            self.stop(&key, &reason);
            true
        } else {
            false
        }
    }
    pub fn acknowledge(&mut self, id: u64, result: Result<String, String>) -> bool {
        let key = self
            .entries
            .iter()
            .find(|(_, e)| {
                e.pending
                    .as_ref()
                    .is_some_and(|p| p.id == id && p.phase == Phase::Ack)
            })
            .map(|(k, _)| k.clone());
        let Some(key) = key else {
            return false;
        };
        let e = self.entries.get_mut(&key).unwrap();
        let p = e.pending.take().unwrap();
        let prompt = p.text;
        if let Some(receipt) = e
            .job
            .as_mut()
            .and_then(|j| j.acknowledged(&p.ticket, result))
        {
            // Keep the exact submitted text for routed source/question provenance.
            e.handoff = Some(id);
            self.effects.push_back(Effect::Follow {
                id,
                receipt,
                prompt,
            });
        }
        self.changed(&key);
        true
    }
    /// A routed handoff must settle before the next evaluation or a new owner.
    /// Normal sessions acknowledge this after applying their source receipt.
    pub fn followed(&mut self, id: u64, result: Result<(), String>) -> bool {
        let key = self
            .entries
            .iter()
            .find(|(_, e)| e.handoff == Some(id))
            .map(|(k, _)| k.clone());
        let Some(key) = key else {
            return false;
        };
        let e = self.entries.get_mut(&key).unwrap();
        e.handoff = None;
        if let Err(error) = result {
            e.warning = error.clone();
            self.stop(&key, &error);
        }
        self.changed(&key);
        true
    }
    fn completed(&mut self, task: Task, result: Result<ResultValue, String>) {
        let Some(e) = self
            .entries
            .get_mut(&task.key)
            .filter(|e| e.owner == task.owner)
        else {
            return;
        };
        e.busy = false;
        match (task.work, result) {
            (Work::Read, Ok(ResultValue::Read(Ok(snapshot)))) => {
                if !e.active() {
                    self.changed(&task.key);
                    return;
                }
                if e.job.is_none() {
                    match Job::start(
                        e.source.context.clone(),
                        "codex",
                        &e.start.turn,
                        &snapshot,
                        None,
                    ) {
                        Ok(job) => e.job = Some(job),
                        Err(error) => {
                            self.stop(&task.key, &error);
                            return;
                        }
                    }
                }
                if let Some(evaluation) = e.job.as_mut().unwrap().poll(&snapshot) {
                    let context = e.start.context.clone();
                    let effort = e.start.effort.clone();
                    let source = e.source.clone();
                    let stop = Arc::new(AtomicBool::new(false));
                    e.cancel = stop.clone();
                    e.busy = true;
                    let id = match request_id() {
                        Ok(id) => id,
                        Err(error) => {
                            e.job
                                .as_mut()
                                .unwrap()
                                .evaluation_failed(&evaluation.ticket, error);
                            e.busy = false;
                            self.changed(&task.key);
                            return;
                        }
                    };
                    let service = self.service.clone();
                    let ticket = evaluation.ticket;
                    let work = Work::Evaluate(ticket.clone());
                    let next = Task {
                        key: task.key.clone(),
                        owner: e.owner,
                        work,
                    };
                    let handle = self.jobs.spawn(async move {
                        let result = async {
                            let (decision, warning) = service
                                .evaluate(id, context, effort, evaluation.input, stop.clone())
                                .await?;
                            let source =
                                service.read(source, stop).await.map_err(
                                    |error| match &warning {
                                        Some(warning) => format!("{error}; {warning}"),
                                        None => error,
                                    },
                                )?;
                            Ok((decision, warning, source))
                        }
                        .await;
                        ResultValue::Evaluated(result)
                    });
                    self.tasks.insert(handle.id(), next);
                }
            }
            (Work::Evaluate(ticket), result) => {
                let result = match result {
                    Ok(ResultValue::Evaluated(result)) => result,
                    Err(error) => Err(error),
                    _ => Err("Invalid supervisor worker result".into()),
                };
                match result {
                    Ok((decision, warning, snapshot)) => {
                        e.warning = warning.unwrap_or_default();
                        if let Some(ticket) = e
                            .job
                            .as_mut()
                            .and_then(|j| j.evaluated(&ticket, Ok(decision), &snapshot))
                        {
                            let id = match request_id() {
                                Ok(id) => id,
                                Err(error) => {
                                    self.stop(&task.key, &error);
                                    return;
                                }
                            };
                            let text = e.job.as_ref().unwrap().preparing_text(&ticket).unwrap();
                            e.pending = Some(Pending {
                                id,
                                ticket,
                                phase: Phase::Host,
                                params: None,
                                text: text.clone(),
                                since: Instant::now(),
                            });
                            self.effects.push_back(Effect::Prepare {
                                id,
                                request: e.start.request,
                                context: e.source.context.clone(),
                                root: e.source.directory.clone(),
                                text,
                            });
                        }
                    }
                    Err(error) => {
                        if let Some(j) = e.job.as_mut() {
                            j.evaluation_failed(&ticket, error);
                        }
                    }
                }
            }
            (Work::Final(id, ticket), result) => {
                let source = match result {
                    Ok(ResultValue::Read(result)) => result,
                    Err(error) => Err(error),
                    _ => Err("Invalid supervisor worker result".into()),
                };
                let ready = match source {
                    Ok(snapshot) => e
                        .job
                        .as_mut()
                        .and_then(|j| j.prepared(&ticket, &snapshot))
                        .is_some(),
                    Err(error) => {
                        if let Some(j) = e.job.as_mut() {
                            j.stop(error);
                        }
                        false
                    }
                };
                if ready && e.active() {
                    if let Some(p) = e.pending.as_mut().filter(|p| p.id == id) {
                        p.phase = Phase::Ready;
                        p.since = Instant::now();
                        self.effects.push_back(Effect::Ready { id });
                    }
                } else {
                    let reason = e
                        .job
                        .as_ref()
                        .map(|j| j.reason().to_owned())
                        .unwrap_or_else(|| "Supervision stopped".into());
                    e.pending = None;
                    self.effects.push_back(Effect::Abandon { id, reason });
                }
            }
            (_, Ok(ResultValue::Read(Err(error)))) | (_, Err(error)) => {
                self.stop(&task.key, &error);
            }
            _ => {
                self.stop(&task.key, "Invalid supervisor worker result");
            }
        }
        self.changed(&task.key);
    }
    pub async fn next(&mut self) -> Effect {
        loop {
            if let Some(effect) = self.effects.pop_front() {
                return effect;
            }
            if let Some((_, view)) = self.changed.pop_first() {
                return Effect::Changed(view);
            }
            match self.jobs.join_next_with_id().await {
                Some(Ok((id, result))) => {
                    if let Some(task) = self.tasks.remove(&id) {
                        self.completed(task, Ok(result));
                    }
                }
                Some(Err(error)) => {
                    if let Some(task) = self.tasks.remove(&error.id()) {
                        self.completed(
                            task,
                            Err("Supervision worker failed; no request was retried".into()),
                        );
                    }
                }
                None => std::future::pending::<()>().await,
            }
        }
    }
    pub async fn shutdown(&mut self) {
        self.begin_shutdown();
        // Utility workers receive cancellation and retain their cleanup ownership.
        let _ = tokio::time::timeout(Duration::from_secs(45), async {
            while let Some(result) = self.jobs.join_next_with_id().await {
                match result {
                    Ok((id, result)) => {
                        if let Some(task) = self.tasks.remove(&id) {
                            self.completed(task, Ok(result));
                        }
                    }
                    Err(error) => {
                        if let Some(task) = self.tasks.remove(&error.id()) {
                            self.completed(task, Err("Supervisor worker stopped".into()));
                        }
                    }
                }
            }
        })
        .await;
    }
}

#[cfg(test)]
#[path = "turn_supervision_runtime_tests.rs"]
mod tests;
