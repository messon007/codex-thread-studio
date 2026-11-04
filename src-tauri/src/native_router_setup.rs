//! Durable, explicit controller provisioning. Never replay an uncertain create.
use super::{configuration_revision, save_scoped, snapshot, Mutation};
use crate::{json_error, json_response, GatewayState, ThreadRouterPreferences};
use axum::{
    body::Body,
    extract::{rejection::JsonRejection, State},
    http::{Response, StatusCode},
    Json,
};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{path::Path, time::Duration};
#[path = "native_router_recovery.rs"]
pub mod recovery;

static ACTIVE: std::sync::LazyLock<std::sync::Mutex<std::collections::HashSet<String>>> =
    std::sync::LazyLock::new(Default::default);
struct Active(String);
impl Active {
    fn claim(backend: &str) -> Result<Self, Error> {
        if !ACTIVE
            .lock()
            .map_err(|_| "Router setup lock is unavailable".to_string())?
            .insert(backend.into())
        {
            return Err(conflict("Controller setup is still running for this backend. Wait and reload; no additional session was created."));
        }
        Ok(Self(backend.into()))
    }
    fn check(backend: &str) -> Result<(), Error> {
        if ACTIVE
            .lock()
            .map_err(|_| "Router setup lock is unavailable".to_string())?
            .contains(backend)
        {
            return Err(conflict("Controller setup is still running for this backend. Wait and reload before recovery."));
        }
        Ok(())
    }
}
impl Drop for Active {
    fn drop(&mut self) {
        if let Ok(mut active) = ACTIVE.lock() {
            active.remove(&self.0);
        }
    }
}

static SETUP_SLOTS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(2);

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Setup {
    backend: String,
    expected_revision: String,
}
#[derive(Debug)]
struct Error(StatusCode, String);
impl From<String> for Error {
    fn from(message: String) -> Self {
        Self(StatusCode::INTERNAL_SERVER_ERROR, message)
    }
}
fn conflict(message: impl Into<String>) -> Error {
    Error(StatusCode::CONFLICT, message.into())
}
fn database(path: &Path) -> Result<Connection, String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let db = Connection::open(path).map_err(|e| e.to_string())?;
    db.busy_timeout(Duration::from_secs(3))
        .map_err(|e| e.to_string())?;
    db.execute_batch(
        "PRAGMA journal_mode=WAL;
        CREATE TABLE IF NOT EXISTS native_router_setup (
            backend TEXT PRIMARY KEY, workspace TEXT NOT NULL, thread_id TEXT
        );",
    )
    .map_err(|e| e.to_string())?;
    Ok(db)
}
fn journal(
    path: &Path,
    backend: &str,
    workspace: &str,
    configured: Option<&str>,
) -> Result<Option<String>, Error> {
    let db = database(path)?;
    let previous = db
        .query_row(
            "SELECT workspace,thread_id FROM native_router_setup WHERE backend=?1",
            [backend],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?)),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    if let Some(id) = configured {
        if let Some((_, Some(previous))) = &previous {
            if previous != id {
                return Err(conflict("The saved Router ID differs from its setup journal; review both IDs before continuing"));
            }
        }
        return Ok(Some(id.into()));
    }
    if let Some((directory, id)) = previous {
        if directory != workspace {
            return Err(conflict(
                "Router setup workspace changed; the existing controller will not be replaced",
            ));
        }
        return id.map(Some).ok_or_else(|| conflict("Router creation has an uncertain result. Inspect the backend sessions before retrying; Studio will not create a duplicate."));
    }
    let inserted = db.execute("INSERT OR IGNORE INTO native_router_setup(backend,workspace,thread_id) VALUES(?1,?2,NULL)", params![backend,workspace]).map_err(|e| e.to_string())?;
    if inserted != 1 {
        return Err(conflict(
            "Router creation is already pending; reload before continuing",
        ));
    }
    Ok(None)
}
fn remember(path: &Path, backend: &str, workspace: &str, id: &str) -> Result<(), String> {
    let changed = database(path)?.execute("UPDATE native_router_setup SET thread_id=?3 WHERE backend=?1 AND workspace=?2 AND (thread_id IS NULL OR thread_id=?3)", params![backend,workspace,id]).map_err(|e| e.to_string())?;
    if changed != 1 {
        return Err("Router setup journal changed; the new controller was not registered. Inspect backend sessions before retrying.".into());
    }
    Ok(())
}
fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 256 && !id.chars().any(char::is_control)
}
fn verify(thread: &Value, id: &str, workspace: &str) -> Result<(), Error> {
    if !valid_id(id)
        || thread["id"].as_str() != Some(id)
        || thread["cwd"].as_str().map(Path::new) != Some(Path::new(workspace))
        || thread["parentThreadId"]
            .as_str()
            .is_some_and(|id| !id.is_empty())
        || thread["ephemeral"].as_bool() == Some(true)
        || thread["archived"].as_bool() == Some(true)
    {
        return Err(conflict("The Router session does not match its saved ID and dedicated workspace; Studio will not create a replacement"));
    }
    Ok(())
}
async fn disk<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, Error> + Send + 'static,
) -> Result<T, Error> {
    tokio::task::spawn_blocking(work).await.map_err(|_| {
        Error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Router setup worker stopped".into(),
        )
    })?
}
enum Backend<'a> {
    Codex(&'a crate::codex_app_server::CodexAppServer),
    OpenCode(&'a crate::opencode_server::OpenCodeServer),
}
impl Backend<'_> {
    fn environment(&self) -> &'static str {
        match self {
            Self::Codex(server) => server.execution_environment(),
            Self::OpenCode(server) => server.execution_environment(),
        }
    }
    async fn probe(&self) -> Result<(), String> {
        match self {
            Self::OpenCode(server) => {
                server
                    .router_request(crate::opencode_server::RouterOperation::Probe, "")
                    .await?;
                Ok(())
            }
            Self::Codex(server) => {
                for attempt in 0..40 {
                    match server
                        .request_router(
                            "thread/list",
                            json!({"limit":1,"useStateDbOnly":true,"archived":false}),
                        )
                        .await
                    {
                        Ok(_) => return Ok(()),
                        Err(error) if error.contains("still initializing") && attempt < 39 => {
                            tokio::time::sleep(Duration::from_millis(50)).await
                        }
                        Err(error) => return Err(error),
                    }
                }
                Err("Router backend did not initialize".into())
            }
        }
    }
    async fn read(&self, id: &str, workspace: &str) -> Result<Value, String> {
        match self {
            Self::Codex(server) => Ok(server
                .request_router("thread/read", json!({"threadId":id,"includeTurns":false}))
                .await?["thread"]
                .take()),
            Self::OpenCode(server) => server
                .router_request(crate::opencode_server::RouterOperation::Read(id), workspace)
                .await
                .map(normalize_opencode),
        }
    }
    async fn create(&self, workspace: &str) -> Result<Value, String> {
        match self {
            Self::Codex(server) => Ok(server
                .request_router(
                    "thread/start",
                    json!({"cwd":workspace,"approvalPolicy":"never","sandbox":"read-only"}),
                )
                .await?["thread"]
                .take()),
            Self::OpenCode(server) => server
                .router_request(crate::opencode_server::RouterOperation::Create, workspace)
                .await
                .map(normalize_opencode),
        }
    }
    async fn restrict(&self, thread: Value, id: &str, workspace: &str) -> Result<(), Error> {
        if let Self::OpenCode(server) = self {
            if !opencode_tools_disabled(&thread["permission"]) {
                let restricted=server.router_request(crate::opencode_server::RouterOperation::Restrict(id),workspace).await.map_err(|error|Error(StatusCode::BAD_GATEWAY,format!("Router identity was kept, but its tool permissions could not be restricted. {error}")))?;
                let restricted = normalize_opencode(restricted);
                verify(&restricted, id, workspace)?;
                if !opencode_tools_disabled(&restricted["permission"]) {
                    return Err(Error(StatusCode::BAD_GATEWAY,"OpenCode did not confirm the Router tool restriction; its identity was kept and no routing message was sent".into()));
                }
            }
        }
        Ok(())
    }
    async fn rename(&self, id: &str) -> Option<String> {
        match self {
            Self::Codex(server) => server
                .request_router(
                    "thread/name/set",
                    json!({"threadId":id,"name":"Thread Router"}),
                )
                .await
                .err()
                .map(|error| {
                    format!("Controller created; its display name could not be set: {error}")
                }),
            Self::OpenCode(_) => None, // The creation API sets the title atomically.
        }
    }
}
fn normalize_opencode(mut value: Value) -> Value {
    json!({"id":value["id"].take(),"cwd":value["directory"].take(),"parentThreadId":value["parentID"].take(),"permission":value["permission"].take(),"title":value["title"].take(),"archived":value.pointer("/time/archived").and_then(Value::as_i64).is_some_and(|time|time>0)})
}
use crate::opencode_permissions::tools_disabled as opencode_tools_disabled;
async fn provision(state: GatewayState, request: Setup) -> Result<Value, Error> {
    let _active = Active::claim(&request.backend)?;
    let backend = if let Some(instance) = state.codex_backends.get(&request.backend) {
        Backend::Codex(&instance.server)
    } else if request.backend == "opencode"
        && state
            .backend_descriptors
            .iter()
            .any(|backend| backend.id == request.backend && backend.kind == "opencode")
    {
        Backend::OpenCode(&state.opencode)
    } else {
        return Err(Error(
            StatusCode::BAD_REQUEST,
            "Choose a configured Router backend".into(),
        ));
    };
    // Readiness is checked before reserving a non-replayable mutation.
    backend
        .probe()
        .await
        .map_err(|error| Error(StatusCode::BAD_GATEWAY, error))?;
    let workspace = if backend.environment() == "wsl" {
        "/var/tmp/codex-thread-studio-router".into()
    } else {
        let path = crate::studio_router_workspace_path();
        disk(move || {
            std::fs::create_dir_all(&path).map_err(|error| error.to_string())?;
            std::fs::canonicalize(path)
                .map(|path| path.to_string_lossy().into_owned())
                .map_err(|error| error.to_string().into())
        })
        .await?
    };
    let previous = disk({
        let state = state.clone();
        let request = request.clone();
        let workspace = workspace.clone();
        move || {
            let _preferences = state
                .preferences_lock
                .lock()
                .map_err(|_| "Preferences lock is unavailable".to_string())?;
            let preferences = crate::load_preferences(&state.preferences_path)?;
            if configuration_revision(&preferences.router)? != request.expected_revision {
                return Err(conflict(
                    "Router settings changed; reload before setting up its controller",
                ));
            }
            let _studio = state
                .studio_lock
                .lock()
                .map_err(|_| "Studio database lock is unavailable".to_string())?;
            // Validate reply metadata before any backend mutation or reservation.
            crate::session_state::opening_messages(&state.studio_path)?;
            let configured = preferences.router.as_ref().and_then(|router| {
                router.controllers.get(&request.backend).or_else(|| {
                    (request.backend == "codex")
                        .then_some(router.thread_id.as_ref())
                        .flatten()
                })
            });
            journal(
                &state.studio_path,
                &request.backend,
                &workspace,
                configured.map(String::as_str),
            )
        }
    })
    .await?;
    let created = previous.is_none();
    let thread = if let Some(id) = previous {
        let thread = backend.read(&id, &workspace).await.map_err(|error| {
            Error(
                StatusCode::BAD_GATEWAY,
                format!("Unable to read the saved Router controller; its ID was kept. {error}"),
            )
        })?;
        verify(&thread, &id, &workspace)?;
        thread
    } else {
        let thread=backend.create(&workspace).await.map_err(|error|Error(StatusCode::BAD_GATEWAY,format!("Router creation result is uncertain; reload and inspect backend sessions before retrying. {error}")))?;
        let id = thread["id"].as_str().filter(|id| valid_id(id)).ok_or_else(|| conflict("Router creation returned no usable ID. Inspect backend sessions; Studio will not retry creation automatically."))?.to_owned();
        // Persist identity before rename, directory validation or preferences I/O.
        disk({
            let state = state.clone();
            let backend = request.backend.clone();
            let workspace = workspace.clone();
            let id = id.clone();
            move || {
                let _studio = state
                    .studio_lock
                    .lock()
                    .map_err(|_| "Studio database lock is unavailable".to_string())?;
                remember(&state.studio_path, &backend, &workspace, &id).map_err(Into::into)
            }
        })
        .await?;
        verify(&thread, &id, &workspace)?;
        thread
    };
    let id = thread["id"].as_str().unwrap().to_owned();
    backend.restrict(thread, &id, &workspace).await?;
    // Naming failure cannot invalidate or lose a successfully created identity.
    let warning = if created {
        backend.rename(&id).await
    } else {
        None
    };
    let snapshot = disk({
        let state=state.clone();let backend=request.backend.clone();let id=id.clone();
        move || {
            let _preferences=state.preferences_lock.lock().map_err(|_| "Preferences lock is unavailable".to_string())?;
            let mut preferences=crate::load_preferences(&state.preferences_path)?;
            let _studio=state.studio_lock.lock().map_err(|_| "Studio database lock is unavailable".to_string())?;
            let messages=crate::session_state::opening_messages(&state.studio_path)?;
            let router=preferences.router.get_or_insert_with(ThreadRouterPreferences::default);
            let previous=router.controllers.get(&backend).or_else(|| (backend=="codex").then_some(router.thread_id.as_ref()).flatten());
            if previous.is_some_and(|previous| previous!=&id) { return Err(conflict("A different Router controller was configured during setup. Both identities were kept; reload and review before continuing.")); }
            router.controllers.insert(backend.clone(),id.clone());
            // Do not switch the active provider or touch conditions while setup runs.
            save_scoped(&state.preferences_path,&preferences,Mutation::Controller(&backend,&id))?;
            let snapshot=snapshot(&preferences,messages)?;
            serde_json::to_value(snapshot).map_err(|e|e.to_string().into())
        }
    }).await?;
    Ok(
        json!({"snapshot":snapshot,"backend":request.backend,"threadId":id,"created":created,"warning":warning}),
    )
}

pub async fn post_controller(
    State(state): State<GatewayState>,
    body: Result<Json<Setup>, JsonRejection>,
) -> Response<Body> {
    let request = match body {
        Ok(Json(request)) => request,
        Err(_) => return json_error(StatusCode::BAD_REQUEST, "Invalid Router controller setup"),
    };
    if request.backend.len() > 64 || request.expected_revision.len() > 64 {
        return json_error(StatusCode::BAD_REQUEST, "Invalid Router controller setup");
    }
    let permit = match SETUP_SLOTS.try_acquire() {
        Ok(permit) => permit,
        Err(_) => {
            return json_error(
                StatusCode::TOO_MANY_REQUESTS,
                "Router setup is busy; reload before retrying",
            )
        }
    };
    // The owned task survives an HTTP timeout/dialog cancellation. Its durable
    // reservation prevents another request or restart from replaying creation.
    match tokio::spawn(async move {
        let _permit = permit;
        provision(state, request).await
    })
    .await
    {
        Ok(Ok(value)) => json_response(StatusCode::OK, &value),
        Ok(Err(Error(status, message))) => json_error(status, &message),
        Err(_) => json_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Router setup stopped. Reload before retrying; creation will not be replayed.",
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn opencode_confirmation_uses_ordered_rules_without_accepting_later_exceptions() {
        let deny = json!({"permission":"*","pattern":"*","action":"deny"});
        let allow = json!({"permission":"bash","pattern":"*","action":"allow"});
        assert!(opencode_tools_disabled(&json!([allow, deny])));
        assert!(opencode_tools_disabled(&json!([deny, deny])));
        assert!(!opencode_tools_disabled(&json!([deny, allow])));
        assert!(!opencode_tools_disabled(&json!([])));
        assert!(!opencode_tools_disabled(&json!(null)));
        assert!(!opencode_tools_disabled(
            &json!([{"permission":"*","pattern":"/project/*","action":"deny"}])
        ));
    }
    #[test]
    fn durable_intent_never_replays_uncertain_creation_and_recovers_known_identity() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("studio.sqlite");
        assert_eq!(journal(&path, "codex", "/router", None).unwrap(), None);
        assert!(journal(&path, "codex", "/router", None)
            .unwrap_err()
            .1
            .contains("uncertain"));
        remember(&path, "codex", "/router", "saved-id").unwrap();
        assert_eq!(
            journal(&path, "codex", "/router", None).unwrap(),
            Some("saved-id".into())
        );
        assert!(journal(&path, "codex", "/other", None).is_err());
        assert!(journal(&path, "codex", "/router", Some("different-id")).is_err());
        assert!(remember(&path, "codex", "/router", "replacement").is_err());
        assert_eq!(journal(&path, "deepseek", "/router", None).unwrap(), None);
    }
    #[test]
    fn recovery_requires_exact_identity_workspace_and_root_session() {
        let valid = json!({"id":"saved-id","cwd":"/router","turns":[]});
        verify(&valid, "saved-id", "/router").unwrap();
        for invalid in [
            json!({"id":"other","cwd":"/router"}),
            json!({"id":"saved-id","cwd":"/project"}),
            json!({"id":"saved-id","cwd":"/router","parentThreadId":"parent"}),
            json!({"id":"saved-id","cwd":"/router","ephemeral":true}),
            json!({"id":"saved-id","cwd":"/router","archived":true}),
        ] {
            assert!(verify(&invalid, "saved-id", "/router").is_err());
        }
    }
}
