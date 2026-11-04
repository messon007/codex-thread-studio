//! Owned background routing and delivery. GTK consumes typed snapshots only.
use super::configuration_revision;
use crate::{
    json_error, json_response, native_router_backend::Session, native_router_plan as plan,
    GatewayState,
};
use axum::{
    body::Body,
    extract::{rejection::JsonRejection, Query, State},
    http::{Response, StatusCode},
    Json,
};
use futures_util::{stream, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::sync::{watch, Semaphore};
#[path = "native_router_archive.rs"]
mod archive;
#[path = "native_router_follow.rs"]
mod follow;
pub use archive::get_archive;
#[path = "native_router_images.rs"]
pub(crate) mod images;
#[path = "native_router_references.rs"]
mod references;
#[path = "native_router_store.rs"]
mod store;
pub use follow::{post_follow, post_prepare, post_scope};

pub struct Runtime {
    owner: String,
    slots: Arc<Semaphore>,
    catalogs: Semaphore,
    inputs: Semaphore,
    searches: Arc<Semaphore>,
    archives: Arc<Semaphore>,
    recovered: AtomicBool,
    stops: Mutex<HashMap<String, StopState>>,
    environments: Mutex<HashMap<String, (u64, HashSet<String>)>>,
    transitions: tokio::sync::Mutex<()>,
}
struct StopState {
    sender: watch::Sender<bool>,
    command_pending: bool,
}
struct PendingCommand<'a> {
    runtime: &'a Runtime,
    id: &'a str,
}
impl Drop for PendingCommand<'_> {
    fn drop(&mut self) {
        if let Ok(mut stops) = self.runtime.stops.lock() {
            if let Some(stop) = stops.get_mut(self.id) {
                stop.command_pending = false;
            }
        }
    }
}
impl Default for Runtime {
    fn default() -> Self {
        Self {
            owner: uuid::Uuid::new_v4().to_string(),
            slots: Arc::new(Semaphore::new(4)),
            catalogs: Semaphore::new(2),
            inputs: Semaphore::new(2),
            searches: Arc::new(Semaphore::new(2)),
            archives: Arc::new(Semaphore::new(2)),
            recovered: AtomicBool::new(false),
            stops: Mutex::new(HashMap::new()),
            environments: Mutex::new(HashMap::new()),
            transitions: tokio::sync::Mutex::new(()),
        }
    }
}
impl Runtime {
    fn request_stop(&self, id: &str) -> Result<bool, String> {
        let stops = self
            .stops
            .lock()
            .map_err(|_| "Router control lock unavailable")?;
        match stops.get(id) {
            Some(stop) if stop.command_pending => {
                Err(crate::native_router_backend::command::STOP_NOTICE.into())
            }
            Some(stop) => Ok(stop.sender.send(true).is_ok()),
            None => Ok(false),
        }
    }
    fn begin_command<'a>(&'a self, id: &'a str) -> Result<Option<PendingCommand<'a>>, String> {
        let mut stops = self
            .stops
            .lock()
            .map_err(|_| "Router control lock unavailable")?;
        let stop = stops
            .get_mut(id)
            .ok_or("Router request is no longer active")?;
        // Serializes Stop with the final pre-send check. Never hold this lock
        // across HTTP, SQLite or backend initialization.
        if *stop.sender.borrow() {
            return Ok(None);
        }
        stop.command_pending = true;
        Ok(Some(PendingCommand { runtime: self, id }))
    }
    pub(crate) fn environment_applied(
        &self,
        backend: &str,
        generation: u64,
        thread: &str,
    ) -> Result<bool, String> {
        let mut environments = self
            .environments
            .lock()
            .map_err(|_| "Environment scope registry unavailable")?;
        let entry = environments
            .entry(backend.into())
            .or_insert_with(|| (generation, HashSet::new()));
        if entry.0 > generation {
            return Ok(false);
        }
        if entry.0 < generation {
            *entry = (generation, HashSet::new());
        }
        Ok(entry.1.contains(thread))
    }
    pub(crate) fn note_environment(
        &self,
        backend: &str,
        generation: u64,
        thread: &str,
        applied: bool,
    ) -> Result<(), String> {
        if thread.is_empty() || thread.len() > 256 || thread.chars().any(char::is_control) {
            return Err("Invalid environment session scope".into());
        }
        let mut environments = self
            .environments
            .lock()
            .map_err(|_| "Environment scope registry unavailable")?;
        let entry = environments
            .entry(backend.into())
            .or_insert_with(|| (generation, HashSet::new()));
        if entry.0 > generation {
            return Ok(());
        }
        if entry.0 < generation {
            *entry = (generation, HashSet::new());
        }
        let total = environments
            .values()
            .map(|entry| entry.1.len())
            .sum::<usize>();
        let entry = environments
            .get_mut(backend)
            .ok_or("Environment scope registry changed")?;
        if applied {
            if total >= 4096 && !entry.1.contains(thread) {
                return Err("Too many managed session environments; restart the backend before adding another".into());
            }
            entry.1.insert(thread.into());
        } else {
            entry.1.remove(thread);
        }
        Ok(())
    }
}
#[cfg(test)]
mod environment_tests {
    use super::*;
    #[test]
    fn pending_command_stop_is_explicit_and_pre_send_cancellation_wins() {
        let runtime = Runtime::default();
        let (sender, receiver) = watch::channel(false);
        runtime.stops.lock().unwrap().insert(
            "job".into(),
            StopState {
                sender,
                command_pending: false,
            },
        );
        let guard = runtime.begin_command("job").unwrap().unwrap();
        assert!(runtime
            .request_stop("job")
            .unwrap_err()
            .contains("will not guess"));
        assert!(!*receiver.borrow());
        drop(guard);
        assert!(runtime.request_stop("job").unwrap());
        assert!(*receiver.borrow());
        assert!(runtime.begin_command("job").unwrap().is_none());
        assert!(!runtime.request_stop("other").unwrap());
    }
    #[test]
    fn session_environment_ownership_is_shared_bounded_and_generation_scoped() {
        let runtime = Runtime::default();
        runtime
            .note_environment("codex", 1, "thread", true)
            .unwrap();
        assert!(runtime.environment_applied("codex", 1, "thread").unwrap());
        assert!(!runtime.environment_applied("other", 1, "thread").unwrap());
        assert!(!runtime.environment_applied("codex", 2, "thread").unwrap());
        runtime.note_environment("codex", 2, "new", true).unwrap();
        runtime.note_environment("codex", 1, "old", true).unwrap();
        assert!(runtime.environment_applied("codex", 2, "new").unwrap());
        assert!(!runtime.environment_applied("codex", 2, "old").unwrap());
        runtime.note_environment("codex", 2, "new", false).unwrap();
        assert!(!runtime.environment_applied("codex", 2, "new").unwrap());
        for index in 0..4096 {
            runtime
                .note_environment("codex", 2, &index.to_string(), true)
                .unwrap();
        }
        assert!(runtime
            .note_environment("codex", 2, "overflow", true)
            .is_err());
        assert!(runtime.note_environment("codex", 2, "", true).is_err());
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Supervision {
    pub previous_turn_id: String,
    pub prompt: String,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Job {
    #[serde(default)]
    pub supervision: Option<Supervision>,
    #[serde(default)]
    pub stop_requested: bool,
    #[serde(default)]
    pub map_scope: Option<crate::native_router_map::Scope>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub map_notice: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub dispatch_notice: String,
    #[serde(default)]
    pub reference_labels: Vec<String>,
    #[serde(default)]
    pub image_count: usize,
    pub id: String,
    #[serde(default)]
    pub fingerprint: String,
    pub controller: String,
    pub created_at: i64,
    pub prompt: String,
    pub status: String,
    pub controller_turn_id: String,
    pub target: Option<plan::Candidate>,
    pub target_turn_id: String,
    pub decision: Option<plan::Decision>,
    pub response: String,
    #[serde(default)]
    pub context_excerpt: String,
    #[serde(default)]
    pub response_items: Vec<crate::native_router_backend::ResponseItem>,
    pub error: String,
    pub unread: bool,
    #[serde(default)]
    pub catalog_warnings: Vec<String>,
}
impl Job {
    pub fn terminal(&self) -> bool {
        matches!(
            self.status.as_str(),
            "completed" | "failed" | "clarify" | "stopped"
        )
    }
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Target {
    pub session_key: String,
    pub cwd: String,
}
#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct Options {
    model: String,
    effort: String,
    service_tier: String,
}
impl Options {
    fn apply(&self, params: &mut Value, codex: bool) {
        if !self.model.is_empty() {
            params["model"] = self.model.clone().into();
        }
        if codex {
            if !self.effort.is_empty() {
                params["effort"] = self.effort.clone().into();
            }
            if !self.service_tier.is_empty() {
                params["serviceTier"] = self.service_tier.clone().into();
            }
        }
    }
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Policy {
    session_key: String,
    cwd: String,
    access: String,
}
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Start {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    references: Vec<references::Reference>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    images: Vec<String>,
    request_id: String,
    controller: String,
    expected_revision: String,
    prompt: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    queued_message_id: Option<String>,
    #[serde(default)]
    explicit_target: Option<Target>,
    #[serde(default)]
    options: Options,
    #[serde(default)]
    policies: Vec<Policy>,
}
fn clip(value: &str, limit: usize) -> String {
    let mut end = value.len().min(limit);
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    if end == value.len() {
        value.into()
    } else {
        format!(
            "{}\n… Open the source session for the full response.",
            &value[..end]
        )
    }
}
async fn disk<T: Send + 'static>(
    state: &GatewayState,
    work: impl FnOnce(&GatewayState) -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    let state = state.clone();
    tokio::task::spawn_blocking(move || {
        let _guard = state
            .studio_lock
            .lock()
            .map_err(|_| "Studio database lock is unavailable")?;
        work(&state)
    })
    .await
    .map_err(|_| "Router storage worker stopped")?
}
async fn save(state: &GatewayState, job: &Job, claim: bool) -> Result<(), String> {
    let job = job.clone();
    disk(state, move |state| {
        store::save(&state.studio_path, &job, &state.router_runtime.owner, claim)
    })
    .await
}
async fn recover(state: &GatewayState) -> Result<(), String> {
    if state.router_runtime.recovered.load(Ordering::Acquire) {
        return Ok(());
    }
    // Publish recovery only after every recovered monitor owns its stop entry.
    // Follow/Stop and terminal worker removal use this same gate. Publishing an
    // eager flag could let a concurrent handoff install a monitor before the
    // stale recovered snapshot, yielding two owners for the same routed job.
    let _transition = state.router_runtime.transitions.lock().await;
    if state.router_runtime.recovered.load(Ordering::Acquire) {
        return Ok(());
    }
    let jobs = match disk(state, |state| {
        store::recover(&state.studio_path, &state.router_runtime.owner)
    })
    .await
    {
        Ok(jobs) => jobs,
        Err(error) => return Err(error),
    };
    for job in jobs {
        let permit = state
            .router_runtime
            .slots
            .clone()
            .try_acquire_owned()
            .map_err(|_| "Router recovery workers are busy")?;
        spawn(state.clone(), job, None, permit)?;
    }
    state
        .router_runtime
        .recovered
        .store(true, Ordering::Release);
    Ok(())
}
fn spawn(
    state: GatewayState,
    mut job: Job,
    start: Option<(
        Start,
        crate::StudioPreferences,
        BTreeMap<String, crate::OpeningMessage>,
    )>,
    permit: tokio::sync::OwnedSemaphorePermit,
) -> Result<(), String> {
    let (stop, receiver) = watch::channel(job.stop_requested);
    state
        .router_runtime
        .stops
        .lock()
        .map_err(|_| "Router control lock unavailable")?
        .insert(
            job.id.clone(),
            StopState {
                sender: stop,
                command_pending: false,
            },
        );
    tokio::spawn(async move {
        let _permit = permit;
        let mut start = start;
        let mut recovering = start.is_none();
        loop {
            let result = if let Some((request, preferences, messages)) = start.take() {
                route(
                    &state,
                    &mut job,
                    request,
                    preferences,
                    messages,
                    receiver.clone(),
                )
                .await
            } else {
                monitor(&state, &mut job, receiver.clone(), recovering, None).await
            };
            // Follow and terminal save/worker removal share one short transition
            // gate. No provider RPC is awaited while this gate is held.
            let _transition = state.router_runtime.transitions.lock().await;
            let id = job.id.clone();
            let owner = state.router_runtime.owner.clone();
            let current = disk(&state, move |state| {
                store::owned(&state.studio_path, &id, &owner)
            })
            .await;
            if let Ok(Some(current)) = current {
                if current.target_turn_id != job.target_turn_id {
                    let same = current
                        .target
                        .as_ref()
                        .zip(job.target.as_ref())
                        .is_some_and(|(a, b)| a.session_key == b.session_key && a.cwd == b.cwd);
                    if same && current.status == "running" {
                        job = current;
                        recovering = false;
                        continue;
                    }
                    eprintln!(
                        "Native Router {}: source ownership changed; old worker stopped",
                        job.id
                    );
                    if let Ok(mut stops) = state.router_runtime.stops.lock() {
                        stops.remove(&job.id);
                    }
                    break;
                }
            }
            job.stop_requested |= *receiver.borrow();
            if let Err(error) = result {
                job.status = "failed".into();
                job.error = clip(&error, 4096);
                job.unread = true;
            }
            if let Err(error) = save(&state, &job, false).await {
                eprintln!(
                    "Native Router {}: delivery identity retained for recovery: {error}",
                    job.id
                );
            }
            if let Ok(mut stops) = state.router_runtime.stops.lock() {
                stops.remove(&job.id);
            }
            break;
        }
    });
    Ok(())
}
async fn load_catalog(
    state: &GatewayState,
    backend: &crate::BackendDescriptor,
) -> Result<Vec<(String, Value)>, String> {
    let mut rows = Vec::new();
    let mut cursor = String::new();
    let mut seen = HashSet::new();
    for page in 0..10 {
        let (data, next) = if let Some(instance) = state.codex_backends.get(&backend.id) {
            let mut value=crate::native_router_backend::read(&instance.server,"thread/list",json!({"limit":100,"cursor":if cursor.is_empty(){Value::Null}else{json!(cursor)},"useStateDbOnly":true,"archived":false})).await?;
            let next = value["nextCursor"].as_str().unwrap_or("").to_owned();
            let data = value
                .get_mut("data")
                .and_then(Value::as_array_mut)
                .map(std::mem::take)
                .ok_or("Invalid Router target catalog")?;
            (data, next)
        } else {
            let mut value = crate::native_utility_sessions::catalog(state, &cursor).await?;
            let next = value["nextCursor"].as_str().unwrap_or("").to_owned();
            let data = value
                .get_mut("data")
                .and_then(Value::as_array_mut)
                .map(std::mem::take)
                .ok_or("Invalid OpenCode Router catalog")?;
            (data, next)
        };
        if data.len() > 100 {
            return Err("Router catalog page exceeds 100 sessions".into());
        }
        rows.extend(data.into_iter().map(|row| (backend.id.clone(), row)));
        if next.is_empty() {
            return Ok(rows);
        }
        if next.len() > 4096 || !seen.insert(next.clone()) || page == 9 {
            return Err(
                "Router catalog is incomplete or repeats a cursor; choose a target explicitly"
                    .into(),
            );
        }
        cursor = next;
    }
    Err("Router catalog did not finish".into())
}
async fn catalog(
    state: &GatewayState,
    preferences: &crate::StudioPreferences,
    messages: BTreeMap<String, crate::OpeningMessage>,
) -> Result<(Vec<plan::Candidate>, Vec<String>), String> {
    let backends = state
        .backend_descriptors
        .iter()
        .filter(|backend| matches!(backend.kind, "codex" | "opencode"))
        .cloned()
        .collect::<Vec<_>>();
    let mut loads = stream::iter(backends.into_iter().map(|backend| {
        let state = state.clone();
        async move {
            let result = load_catalog(&state, &backend).await;
            (backend.id, result)
        }
    }))
    .buffer_unordered(2);
    let mut rows = Vec::new();
    let mut warnings = Vec::new();
    while let Some((backend, result)) = loads.next().await {
        match result {
            Ok(data) => rows.extend(data),
            Err(error) => warnings.push(format!("{backend}: {}", clip(&error, 512))),
        }
        if rows.len() > 16_000 {
            return Err("Router catalog exceeds its processing budget".into());
        }
    }
    let preferences = preferences.clone();
    let candidates =
        tokio::task::spawn_blocking(move || plan::project(&preferences, &messages, rows))
            .await
            .map_err(|_| "Router catalog worker stopped")??;
    Ok((candidates, warnings))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TargetQuery {
    controller: String,
}
/// Reuse the routing planner's eligibility rules; the picker receives metadata,
/// never opening messages, responsibility text or conversation history.
pub async fn get_targets(
    State(state): State<GatewayState>,
    query: Result<Query<TargetQuery>, axum::extract::rejection::QueryRejection>,
) -> Response<Body> {
    let query = match query {
        Ok(Query(query)) => query,
        Err(_) => return json_error(StatusCode::BAD_REQUEST, "Invalid Router target query"),
    };
    if query.controller.len() > 320 || !crate::valid_router_session_key(&query.controller) {
        return json_error(StatusCode::BAD_REQUEST, "Invalid Router controller");
    }
    let Ok(_permit) = state.router_runtime.catalogs.try_acquire() else {
        return json_error(
            StatusCode::TOO_MANY_REQUESTS,
            "Router target discovery is busy",
        );
    };
    let loaded = tokio::task::spawn_blocking({
        let state = state.clone();
        move || {
            let _preferences = state
                .preferences_lock
                .lock()
                .map_err(|_| "Preferences lock unavailable")?;
            let preferences = crate::load_preferences(&state.preferences_path)?;
            if plan::controller(&preferences)? != query.controller {
                return Err(
                    "Select the active Router controller before choosing a target".to_owned(),
                );
            }
            let revision = configuration_revision(&preferences.router)?;
            let _studio = state
                .studio_lock
                .lock()
                .map_err(|_| "Studio lock unavailable")?;
            let messages = crate::session_state::opening_messages(&state.studio_path)?;
            Ok::<_, String>((query.controller, revision, preferences, messages))
        }
    })
    .await;
    let (controller, revision, preferences, messages) = match loaded {
        Ok(Ok(value)) => value,
        Ok(Err(error)) => return json_error(StatusCode::CONFLICT, &error),
        Err(_) => {
            return json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Router target worker stopped",
            )
        }
    };
    match catalog(&state, &preferences, messages).await {
        Ok((targets, warnings)) => {
            let value = tokio::task::spawn_blocking(move || {
                let targets = targets.into_iter().map(|target| json!({"sessionKey":target.session_key,"backend":target.backend,"id":target.id,"title":target.title,"cwd":target.cwd})).collect::<Vec<_>>();
                json!({"controller":controller,"revision":revision,"targets":targets,"warnings":warnings})
            }).await;
            match value {
                Ok(value) => json_response(StatusCode::OK, &value),
                Err(_) => json_error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Router target projection stopped",
                ),
            }
        }
        Err(error) => json_error(StatusCode::BAD_GATEWAY, &error),
    }
}
async fn route(
    state: &GatewayState,
    job: &mut Job,
    request: Start,
    preferences: crate::StudioPreferences,
    messages: BTreeMap<String, crate::OpeningMessage>,
    mut stop: watch::Receiver<bool>,
) -> Result<(), String> {
    let controller = Session::new(&job.controller, "")?;
    let cwd = if controller
        .codex(state)?
        .map(|server| server.execution_environment() == "wsl")
        .unwrap_or(state.opencode.execution_environment() == "wsl")
    {
        "/var/tmp/codex-thread-studio-router".into()
    } else {
        let path = crate::studio_router_workspace_path();
        tokio::task::spawn_blocking(move || {
            std::fs::canonicalize(path)
                .map(|value| value.to_string_lossy().into_owned())
                .map_err(|e| e.to_string())
        })
        .await
        .map_err(|_| "Router directory worker stopped")??
    };
    let controller = Session { cwd, ..controller };
    let selected_refs = request.references.clone();
    let resolved = tokio::task::spawn_blocking(move || references::resolve(selected_refs))
        .await
        .map_err(|_| "Reference resolution worker stopped")??;
    job.reference_labels = resolved.labels.clone();
    let candidates = if let Some(explicit) = &request.explicit_target {
        let session = Session::new(&explicit.session_key, &explicit.cwd)?;
        let row = session.summary(state).await?;
        let preferences = preferences.clone();
        let messages = messages.clone();
        let backend = session.backend.clone();
        tokio::task::spawn_blocking(move || {
            plan::project(&preferences, &messages, vec![(backend, row)])
        })
        .await
        .map_err(|_| "Router target worker stopped")??
    } else {
        let (candidates, warnings) = catalog(state, &preferences, messages).await?;
        job.catalog_warnings = warnings;
        candidates
    };
    if candidates.is_empty() {
        return Err("No available Router target session; open Router settings first".into());
    }
    if *stop.borrow() {
        job.status = "stopped".into();
        return Ok(());
    }
    let decision = if let Some(explicit) = request.explicit_target {
        if !candidates
            .iter()
            .any(|candidate| candidate.session_key == explicit.session_key)
        {
            return Err("The selected Router target is hidden or unavailable".into());
        }
        plan::Decision {
            action: "dispatch".into(),
            target_session_key: explicit.session_key,
            forwarded_prompt: request.prompt.clone(),
            reason: String::new(),
            message: String::new(),
        }
    } else {
        if controller.summary(state).await?["busy"] == true {
            return Err("The Router controller is already busy".into());
        }
        controller.prepare(state, true).await?;
        let controller_key = job.controller.clone();
        let recent = disk(state, move |state| {
            store::recent(&state.studio_path, &controller_key)
        })
        .await?;
        let projected = candidates.clone();
        let warnings = job.catalog_warnings.clone();
        let (instructions, schema) = tokio::task::spawn_blocking(move || {
            let mut instructions=plan::instructions(&projected)?;
            if !warnings.is_empty(){instructions.push_str("\nSome backend catalogs are unavailable (diagnostic data follows). Do not assume these backends have no sessions. Ask for clarification when the request requires a missing session/backend.\n");instructions.push_str(&serde_json::to_string(&warnings).map_err(|e|e.to_string())?);}
            plan::append_recent(&mut instructions,recent,&projected)?;
            Ok::<_, String>((instructions, plan::schema(&projected)))
        })
        .await
        .map_err(|_| "Router planning worker stopped")??;
        let mut params =
            json!({"input":[{"type":"text","text":request.prompt}],"outputSchema":schema});
        if let Some(text) = resolved.controller_text()? {
            params["input"]
                .as_array_mut()
                .unwrap()
                .push(json!({"type":"text","text":text}));
        }
        images::append(&mut params, &request.images);
        let codex = controller.codex(state)?.is_some();
        request.options.apply(&mut params, codex);
        if codex {
            params["additionalContext"] = json!({"codex-thread-studio/thread-router":{"kind":"application","value":instructions}});
            params["approvalPolicy"] = json!("never");
            params["sandboxPolicy"] = json!({"type":"readOnly"});
        } else {
            params["developerInstructions"] = instructions.into();
        }
        // The pre-existing routing record is the controller send intent.
        job.controller_turn_id = controller.start(state, params).await?;
        save(state, job, false).await?;
        let text = match wait(state, &controller, &job.controller_turn_id, &mut stop, None).await? {
            Wait::Completed(text) => text,
            Wait::Stopped => {
                job.status = "stopped".into();
                return Ok(());
            }
            Wait::Replaced => return Err("Router planning ownership changed".into()),
        };
        let projected = candidates.clone();
        tokio::task::spawn_blocking(move || plan::decision(&text.text, &projected))
            .await
            .map_err(|_| "Router decision worker stopped")??
    };
    if *stop.borrow() {
        job.status = "stopped".into();
        return Ok(());
    }
    job.decision = Some(decision.clone());
    if decision.action == "clarify" {
        job.status = "clarify".into();
        job.response = decision.message;
        job.unread = true;
        return Ok(());
    }
    let target = candidates
        .into_iter()
        .find(|candidate| candidate.session_key == decision.target_session_key)
        .ok_or("Router target disappeared")?;
    let session = Session::new(&target.session_key, &target.cwd)?;
    let codex = session.codex(state)?.is_some();
    if !resolved.files.is_empty()
        && session
            .codex(state)?
            .map(|server| server.execution_environment() == "wsl")
            .unwrap_or(state.opencode.execution_environment() == "wsl")
    {
        return Err("Local file references cannot be delivered into a WSL backend; open the target conversation to select its files".into());
    }
    let mut skill_inventory = None;
    let mut plugin_inventory = None;
    for reference in &resolved.refs {
        let method = match reference {
            references::Reference::Skill { .. } => "skills/list",
            references::Reference::Mention { .. } => "plugin/list",
            _ => continue,
        };
        if !codex {
            let references::Reference::Skill { name, path } = reference else {
                return Err("OpenCode does not accept Codex plugin references; choose a target with this plugin enabled".into());
            };
            if name != path {
                return Err("Choose an OpenCode skill from the target project's inventory".into());
            }
            if skill_inventory.is_none() {
                skill_inventory = Some(
                    state
                        .opencode
                        .router_request(
                            crate::opencode_server::RouterOperation::Commands,
                            &session.cwd,
                        )
                        .await?,
                );
            }
            if !crate::native_router_backend::command::contains(
                skill_inventory.as_ref().unwrap(),
                name,
                path,
            ) {
                return Err("The selected OpenCode skill is unavailable in the target project; refresh its inventory".into());
            }
            continue;
        }
        let slot = if method == "skills/list" {
            &mut skill_inventory
        } else {
            &mut plugin_inventory
        };
        if slot.is_none() {
            let server = session
                .codex(state)?
                .ok_or("Target metadata backend is unavailable")?;
            *slot = Some(
                crate::native_router_backend::read(
                    server,
                    method,
                    if method == "skills/list" {
                        json!({"cwds":[session.cwd],"forceReload":false})
                    } else {
                        json!({"cwds":[session.cwd],"forceRefetch":false})
                    },
                )
                .await?,
            );
        }
        if !references::inventory_contains(reference, slot.as_ref().unwrap())? {
            return Err("The selected skill or plugin is not enabled in the target project; refresh the reference in its source conversation".into());
        }
    }
    let selected_refs = request.references.clone();
    let expected = resolved.files.clone();
    tokio::task::spawn_blocking(move || {
        if references::resolve(selected_refs)?.files != expected {
            return Err("A referenced file changed its project path before dispatch".to_owned());
        }
        Ok(())
    })
    .await
    .map_err(|_| "Reference validation worker stopped")??;
    let mut forwarded = decision.forwarded_prompt.clone();
    let reference_input = resolved.target_inputs(codex, &mut forwarded)?;
    if let Some(decision) = &mut job.decision {
        decision.forwarded_prompt = forwarded.clone();
    }
    job.target = Some(target);
    job.status = "dispatching".into();
    let command = !codex
        && resolved
            .refs
            .iter()
            .any(|r| matches!(r, references::Reference::Skill { .. }));
    if command {
        job.dispatch_notice = crate::native_router_backend::command::STOP_NOTICE.into();
    }
    save(state, job, true).await?;
    if session.summary(state).await?["busy"] == true {
        return Err("The target session is busy; the request was not sent".into());
    }
    session.prepare(state, false).await?;
    if session.summary(state).await?["busy"] == true {
        return Err("The target became busy before dispatch; the request was not sent".into());
    }
    let key = session.key();
    let options = disk(state, move |state| {
        crate::session_state::load_turn_options(&state.studio_path, &key)
    })
    .await?;
    let mut params = json!({"input":[{"type":"text","text":forwarded}]});
    images::append(&mut params, &request.images);
    params["input"]
        .as_array_mut()
        .unwrap()
        .extend(reference_input);
    Options {
        model: options.model,
        effort: options.effort,
        service_tier: options.service_tier,
    }
    .apply(&mut params, codex);
    if let Some(policy) = request
        .policies
        .iter()
        .find(|policy| policy.session_key == session.key())
    {
        if policy.cwd != session.cwd {
            return Err("The target permission scope changed; choose its permissions again".into());
        }
        if !codex && policy.access != "inherit" {
            return Err("OpenCode uses its server/project tool permissions; Studio overrides are unavailable".into());
        }
        if codex {
            match policy.access.as_str() {
                "readOnly" => {
                    params["approvalPolicy"] = json!("on-request");
                    params["sandboxPolicy"] = json!({"type":"readOnly"});
                }
                "workspaceWrite" => {
                    params["approvalPolicy"] = json!("on-request");
                    params["sandboxPolicy"] = json!({"type":"workspaceWrite","writableRoots":[session.cwd],"networkAccess":false});
                }
                "fullAccess" => {
                    params["approvalPolicy"] = json!("never");
                    params["sandboxPolicy"] = json!({"type":"dangerFullAccess"});
                }
                _ => {}
            }
        }
    }
    if *stop.borrow() {
        job.status = "stopped".into();
        return Ok(());
    }
    // No retry around this mutation: failure can mean accepted without an ACK.
    let map = crate::native_router_map::prepare(state, &session).await?;
    job.map_scope = map.as_ref().map(|g| g.scope());
    if map.is_some() {
        save(state, job, true).await?;
    }
    if *stop.borrow() {
        job.status = "stopped".into();
        return Ok(());
    }
    let pending_command = if command {
        match state.router_runtime.begin_command(&job.id)? {
            Some(guard) => Some(guard),
            None => {
                job.status = "stopped".into();
                return Ok(());
            }
        }
    } else {
        None
    };
    job.target_turn_id = session.start(state, params).await?;
    if let Some(map) = &map {
        map.bind(&job.target_turn_id)?;
    }
    drop(pending_command);
    job.dispatch_notice.clear();
    // Monitoring needs only the server-owned turn identity. Release potentially
    // large submitted payloads as soon as delivery is confirmed.
    drop(request.images);
    drop(request.references);
    job.status = "running".into();
    save(state, job, false).await?;
    monitor(state, job, stop, false, map).await
}
enum Wait {
    Completed(crate::native_router_backend::Turn),
    Stopped,
    Replaced,
}
async fn wait(
    state: &GatewayState,
    session: &Session,
    id: &str,
    stop: &mut watch::Receiver<bool>,
    job: Option<&Job>,
) -> Result<Wait, String> {
    loop {
        if let Some(job) = job {
            let controller = job.controller.clone();
            let job_id = job.id.clone();
            let current = disk(state, move |state| {
                store::status(&state.studio_path, &controller, &job_id)
            })
            .await?;
            if current["targetTurnId"].as_str() != Some(id) {
                return Ok(Wait::Replaced);
            }
        }
        let turn = session.turn(state, id).await?;
        if turn.done {
            return if turn.success {
                Ok(Wait::Completed(turn))
            } else {
                Err(if turn.error.is_empty() {
                    "Routed turn did not complete successfully".into()
                } else {
                    turn.error
                })
            };
        }
        if *stop.borrow() {
            session.stop(state, id).await?;
            return Ok(Wait::Stopped);
        }
        tokio::select! {_=tokio::time::sleep(Duration::from_millis(750))=>{},_=stop.changed()=>{}}
    }
}
async fn monitor(
    state: &GatewayState,
    job: &mut Job,
    mut stop: watch::Receiver<bool>,
    recovering: bool,
    map: Option<crate::native_router_map::Guard>,
) -> Result<(), String> {
    let target = job
        .target
        .as_ref()
        .ok_or("Missing routed target identity")?;
    let session = Session::new(&target.session_key, &target.cwd)?;
    if recovering
        && session.summary(state).await?["busy"] != true
        && !session.turn(state, &job.target_turn_id).await?.done
    {
        return Err("The routed turn is not active after restart, and its final outcome is unconfirmed. Inspect the source session; the request was not replayed.".into());
    }
    let map = if recovering && map.is_none() {
        if let Some(scope) = job.map_scope.clone() {
            crate::native_router_map::recover(state, &session, scope, &job.target_turn_id).await?
        } else {
            None
        }
    } else {
        map
    };
    let text = match wait(state, &session, &job.target_turn_id, &mut stop, Some(job)).await? {
        Wait::Completed(text) => text,
        Wait::Stopped => {
            job.status = "stopped".into();
            return Ok(());
        }
        Wait::Replaced => {
            // Finish the original Map scope against its own turn, never the
            // continuation. The native source client owns the next Map scope.
            if let Some(map) = &map {
                let previous = session.turn(state, &job.target_turn_id).await?;
                if previous.done && previous.success {
                    let notice = map.finish(previous.map_update).await;
                    if let Some(server) = session.codex(state)? {
                        map.publish(&server.map_events(), &notice);
                    }
                }
            }
            return Ok(());
        }
    };
    job.status = "completed".into();
    if let Some(map) = &map {
        job.map_notice = map.finish(text.map_update.clone()).await;
        if let Some(server) = session.codex(state)? {
            map.publish(&server.map_events(), &job.map_notice);
        }
    }
    job.response = clip(&text.text, 32 * 1024);
    job.context_excerpt = plan::tail(&text.text, 4000);
    job.response_items = text.items;
    job.unread = true;
    Ok(())
}
pub async fn post_job(
    State(state): State<GatewayState>,
    body: Result<Json<Start>, JsonRejection>,
) -> Response<Body> {
    let mut request = match body {
        Ok(Json(value)) => value,
        Err(_) => return json_error(StatusCode::BAD_REQUEST, "Invalid native Router request"),
    };
    if uuid::Uuid::parse_str(&request.request_id).is_err()
        || request
            .queued_message_id
            .as_ref()
            .is_some_and(|id| id != &request.request_id)
        || request.controller.len() > 320
        || request.expected_revision.len() > 64
        || !crate::valid_router_session_key(&request.controller)
        || request.explicit_target.as_ref().is_some_and(|target| {
            target.cwd.len() > 4096
                || target.session_key.len() > 320
                || !crate::valid_router_session_key(&target.session_key)
        })
        || request.prompt.trim().is_empty()
            && request.images.is_empty()
            && request.references.is_empty()
        || request.references.len() > 32
        || request.images.len() > 4
        || request.queued_message_id.is_some() && !request.images.is_empty()
        || request.prompt.len() > 64 * 1024
        || request.policies.len() > 128
        || request.options.model.len() > 256
        || request.options.effort.len() > 64
        || request.options.service_tier.len() > 64
        || request.policies.iter().any(|policy| {
            !crate::valid_router_session_key(&policy.session_key)
                || policy.session_key.len() > 320
                || policy.cwd.len() > 4096
                || !matches!(
                    policy.access.as_str(),
                    "inherit" | "readOnly" | "workspaceWrite" | "fullAccess"
                )
        })
    {
        return json_error(
            StatusCode::BAD_REQUEST,
            "Router prompt/options exceed their limits",
        );
    }
    request
        .policies
        .sort_by(|a, b| a.session_key.cmp(&b.session_key));
    if request
        .policies
        .windows(2)
        .any(|values| values[0].session_key == values[1].session_key)
    {
        return json_error(
            StatusCode::BAD_REQUEST,
            "Duplicate Router permission scopes",
        );
    }
    if let Err(error) = recover(&state).await {
        return json_error(StatusCode::INTERNAL_SERVER_ERROR, &error);
    }
    let Ok(_input_permit) = state.router_runtime.inputs.try_acquire() else {
        return json_error(
            StatusCode::TOO_MANY_REQUESTS,
            "Router input workers are busy",
        );
    };
    let checked = tokio::task::spawn_blocking(move || {
        use sha2::{Digest, Sha256};
        use std::io::Write;
        images::validate(&request.images)?;
        references::validate(&request.references)?;
        let mut raw = serde_json::to_value(&request).map_err(|error| error.to_string())?;
        raw.as_object_mut().unwrap().remove("expectedRevision");
        // Stream canonical JSON into the digest instead of retaining a second
        // complete base64 request buffer solely for idempotency hashing.
        struct HashWriter(Sha256);
        impl Write for HashWriter {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                self.0.update(bytes);
                Ok(bytes.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let mut writer = HashWriter(Sha256::new());
        serde_json::to_writer(&mut writer, &raw).map_err(|error| error.to_string())?;
        let mut hex = String::with_capacity(64);
        for byte in writer.0.finalize() {
            use std::fmt::Write;
            let _ = write!(&mut hex, "{byte:02x}");
        }
        let queued_input = raw["references"].as_array().cloned().unwrap_or_default();
        Ok::<_, String>((request, hex, queued_input))
    })
    .await;
    let (request, fingerprint, queued_input) = match checked {
        Ok(Ok(value)) => value,
        Ok(Err(error)) => return json_error(StatusCode::BAD_REQUEST, &error),
        Err(_) => {
            return json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Router input worker stopped",
            )
        }
    };
    let requested_id = request.request_id.clone();
    match disk(&state, move |state| {
        store::find(&state.studio_path, &requested_id)
    })
    .await
    {
        Ok(Some(job)) => {
            return if job.fingerprint == fingerprint {
                json_response(StatusCode::OK, &job)
            } else {
                json_error(
                    StatusCode::CONFLICT,
                    "Router request ID was already used for different input",
                )
            }
        }
        Ok(None) => {}
        Err(error) => return json_error(StatusCode::INTERNAL_SERVER_ERROR, &error),
    }
    // Resolve queued file metadata before consuming the saved head. Retrying an
    // already accepted UUID above must still work after a file has disappeared.
    let reference_labels = if request.queued_message_id.is_some() {
        let refs = request.references.clone();
        match tokio::task::spawn_blocking(move || {
            references::resolve(refs).map(|resolved| resolved.labels)
        })
        .await
        {
            Ok(Ok(labels)) => labels,
            Ok(Err(error)) => return json_error(StatusCode::BAD_REQUEST, &error),
            Err(_) => {
                return json_error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Router reference worker stopped",
                )
            }
        }
    } else {
        Vec::new()
    };
    drop(_input_permit);
    let permit = match state.router_runtime.slots.clone().try_acquire_owned() {
        Ok(permit) => permit,
        Err(_) => {
            return json_error(
                StatusCode::TOO_MANY_REQUESTS,
                "Four Router requests are already being tracked",
            )
        }
    };
    let revision = request.expected_revision.clone();
    let controller = request.controller.clone();
    let config = tokio::task::spawn_blocking({
        let state = state.clone();
        move || {
            let _preferences = state
                .preferences_lock
                .lock()
                .map_err(|_| "Preferences lock unavailable")?;
            let preferences = crate::load_preferences(&state.preferences_path)?;
            if configuration_revision(&preferences.router)? != revision
                || plan::controller(&preferences)? != controller
            {
                return Err("Router configuration changed; reload before sending".into());
            }
            let _studio = state
                .studio_lock
                .lock()
                .map_err(|_| "Studio lock unavailable")?;
            Ok::<_, String>((
                preferences,
                crate::session_state::opening_messages(&state.studio_path)?,
            ))
        }
    })
    .await;
    let (preferences, messages) = match config {
        Ok(Ok(value)) => value,
        Ok(Err(error)) => return json_error(StatusCode::CONFLICT, &error),
        Err(_) => {
            return json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Router configuration worker stopped",
            )
        }
    };
    let job = Job {
        supervision: None,
        stop_requested: false,
        map_scope: None,
        map_notice: String::new(),
        dispatch_notice: String::new(),
        reference_labels,
        image_count: request.images.len(),
        id: request.request_id.clone(),
        fingerprint: fingerprint.clone(),
        controller: request.controller.clone(),
        created_at: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            .try_into()
            .unwrap_or(i64::MAX),
        prompt: request.prompt.clone(),
        status: "routing".into(),
        controller_turn_id: String::new(),
        target: None,
        target_turn_id: String::new(),
        decision: None,
        response: String::new(),
        context_excerpt: String::new(),
        response_items: Vec::new(),
        error: String::new(),
        unread: false,
        catalog_warnings: Vec::new(),
    };
    let pending = job.clone();
    let queued = request.queued_message_id.clone();
    if let Err(error) = disk(&state, move |state| {
        store::reserve_queued(
            &state.studio_path,
            &pending,
            &state.router_runtime.owner,
            queued.as_deref().map(|id| (id, queued_input.as_slice())),
        )
    })
    .await
    {
        let requested_id = job.id.clone();
        if let Ok(Some(existing)) = disk(&state, move |state| {
            store::find(&state.studio_path, &requested_id)
        })
        .await
        {
            if existing.fingerprint == fingerprint {
                return json_response(StatusCode::OK, &existing);
            }
        }
        return json_error(StatusCode::CONFLICT, &error);
    }
    let response = json_response(StatusCode::ACCEPTED, &job);
    if let Err(error) = spawn(state, job, Some((request, preferences, messages)), permit) {
        return json_error(StatusCode::INTERNAL_SERVER_ERROR, &error);
    }
    response
}
#[derive(Deserialize)]
pub struct Read {
    #[serde(default)]
    controller: String,
    #[serde(default)]
    id: String,
    before: Option<i64>,
    after: Option<i64>,
    #[serde(default, rename = "statusOnly")]
    status_only: bool,
    #[serde(default)]
    query: String,
    #[serde(default, rename = "caseSensitive")]
    case_sensitive: bool,
}
pub async fn get_jobs(
    State(state): State<GatewayState>,
    Query(query): Query<Read>,
) -> Response<Body> {
    if (query.before.is_some() && query.after.is_some())
        || query.before.is_some_and(|c| c <= 0)
        || query.after.is_some_and(|c| c <= 0)
        || query.id.len() > 64
        || query.controller.len() > 320
        || (!query.id.is_empty() && uuid::Uuid::parse_str(&query.id).is_err())
        || query.id.is_empty() && !crate::valid_router_session_key(&query.controller)
        || query.status_only
            && (query.id.is_empty() || !crate::valid_router_session_key(&query.controller))
        || !query.query.is_empty() && !query.id.is_empty()
    {
        return json_error(StatusCode::BAD_REQUEST, "Invalid Router history query");
    }
    if let Err(error) = store::search::validate(&query.query) {
        return json_error(StatusCode::BAD_REQUEST, &error);
    }
    if let Err(error) = recover(&state).await {
        return json_error(StatusCode::INTERNAL_SERVER_ERROR, &error);
    }
    if !query.query.is_empty() {
        let Ok(permit) = state.router_runtime.searches.clone().try_acquire_owned() else {
            return json_error(
                StatusCode::TOO_MANY_REQUESTS,
                "Router search workers are busy; try again",
            );
        };
        let path = state.studio_path.clone();
        let result = tokio::task::spawn_blocking(move || {
            let _permit = permit;
            store::search::page(
                &path,
                &query.controller,
                &query.query,
                query.case_sensitive,
                query.before,
                query.after,
            )
        })
        .await;
        return match result {
            Ok(Ok(value)) => json_response(StatusCode::OK, &value),
            Ok(Err(error)) => json_error(StatusCode::BAD_REQUEST, &error),
            Err(_) => json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Router search worker stopped",
            ),
        };
    }
    let result = disk(&state, move |state| {
        let value = if query.id.is_empty() {
            let mut rows=store::list_range(&state.studio_path,&query.controller,query.before,query.after)?;
            rows.truncate(20);
            let first=rows.first().and_then(|row|row["cursor"].as_i64());let last=rows.last().and_then(|row|row["cursor"].as_i64());
            let (older,newer)=store::neighbors(&state.studio_path,&query.controller,first,last)?;
            let next=if older{last}else{None};let previous=if newer{first}else{None};
            let summary=store::summary(&state.studio_path,&query.controller)?;
            json!({"data":rows,"nextCursor":next,"newerCursor":previous,"unreadCount":summary["unreadCount"],"activeIds":summary["activeIds"],"controllerBusy":summary["controllerBusy"]})
        } else if query.status_only {store::status(&state.studio_path,&query.controller,&query.id)?} else {store::view(&state.studio_path,&query.id)?};
        Ok(json_response(StatusCode::OK, &value))
    })
    .await;
    match result {
        Ok(response) => response,
        Err(error) => json_error(StatusCode::BAD_REQUEST, &error),
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Stop {
    id: String,
}
pub async fn stop_job(
    State(state): State<GatewayState>,
    body: Result<Json<Stop>, JsonRejection>,
) -> Response<Body> {
    let id = match body {
        Ok(Json(value)) if value.id.len() <= 64 => value.id,
        _ => return json_error(StatusCode::BAD_REQUEST, "Invalid Router stop request"),
    };
    let _transition = state.router_runtime.transitions.lock().await;
    let stopped = match state.router_runtime.request_stop(&id) {
        Ok(stopped) => stopped,
        Err(error) => return json_error(StatusCode::CONFLICT, &error),
    };
    if stopped {
        let owner = state.router_runtime.owner.clone();
        let noted = id.clone();
        if let Err(error) = disk(&state, move |state| {
            store::note_stop(&state.studio_path, &noted, &owner)
        })
        .await
        {
            return json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                &format!("Stop requested, but its bookkeeping failed: {error}"),
            );
        }
        json_response(StatusCode::ACCEPTED, &json!({"stopping":true}))
    } else {
        json_error(
            StatusCode::CONFLICT,
            "Router request is not active; refresh its state",
        )
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Seen {
    controller: String,
    seen: Vec<store::Receipt>,
}
pub async fn acknowledge_jobs(
    State(state): State<GatewayState>,
    body: Result<Json<Seen>, JsonRejection>,
) -> Response<Body> {
    let request = match body {
        Ok(Json(request)) => request,
        Err(_) => return json_error(StatusCode::BAD_REQUEST, "Invalid Router read receipt"),
    };
    let mut ids = std::collections::HashSet::new();
    if !crate::valid_router_session_key(&request.controller)
        || request.controller.len() > 320
        || request.seen.is_empty()
        || request.seen.len() > 20
        || request.seen.iter().any(|seen| {
            uuid::Uuid::parse_str(&seen.id).is_err()
                || seen.receipt.len() != 32
                || !seen.receipt.bytes().all(|c| c.is_ascii_hexdigit())
                || !ids.insert(&seen.id)
        })
    {
        return json_error(StatusCode::BAD_REQUEST, "Invalid Router read receipt");
    }
    let result = disk(&state, move |state| {
        store::acknowledge(&state.studio_path, &request.controller, &request.seen)
    })
    .await;
    match result {
        Ok(acknowledged) => json_response(StatusCode::OK, &json!({"acknowledged":acknowledged})),
        Err(error) => json_error(StatusCode::INTERNAL_SERVER_ERROR, &error),
    }
}
