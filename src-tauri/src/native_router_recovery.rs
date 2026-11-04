//! Explicit adoption of an existing controller after a non-replayable create.
use super::*;
use serde::Serialize;
static READS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(2);

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Inspect {
    backend: String,
    expected_revision: String,
    #[serde(default)]
    thread_id: Option<String>,
}
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Adopt {
    backend: String,
    expected_revision: String,
    journal_revision: String,
    thread_id: String,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Journal {
    workspace: String,
    thread_id: Option<String>,
}
fn journal_revision(backend: &str, journal: &Option<Journal>) -> Result<String, String> {
    super::super::revision(&(backend, journal))
}
fn workspace_matches(state: &GatewayState, id: &str, journal: &Journal) -> Result<(), Error> {
    let expected = if backend(state, id)?.environment() == "wsl" {
        "/var/tmp/codex-thread-studio-router".to_owned()
    } else {
        std::fs::canonicalize(crate::studio_router_workspace_path())
            .map_err(|_| {
                conflict(
                    "The dedicated Router workspace is unavailable; its saved identity was kept.",
                )
            })?
            .to_string_lossy()
            .into_owned()
    };
    if Path::new(&expected) != Path::new(&journal.workspace) {
        return Err(conflict("The dedicated Router workspace changed. Recovery will not adopt a session from another directory."));
    }
    Ok(())
}
fn saved(path: &Path, backend: &str) -> Result<Option<Journal>, String> {
    database(path)?
        .query_row(
            "SELECT workspace,thread_id FROM native_router_setup WHERE backend=?1",
            [backend],
            |row| {
                Ok(Journal {
                    workspace: row.get(0)?,
                    thread_id: row.get(1)?,
                })
            },
        )
        .optional()
        .map_err(|e| e.to_string())
}
fn configured<'a>(preferences: &'a crate::StudioPreferences, backend: &str) -> Option<&'a str> {
    preferences
        .router
        .as_ref()
        .and_then(|router| {
            router.controllers.get(backend).or_else(|| {
                (backend == "codex")
                    .then_some(router.thread_id.as_ref())
                    .flatten()
            })
        })
        .map(String::as_str)
}
fn backend<'a>(state: &'a GatewayState, id: &str) -> Result<Backend<'a>, Error> {
    if let Some(instance) = state.codex_backends.get(id) {
        Ok(Backend::Codex(&instance.server))
    } else if id == "opencode"
        && state
            .backend_descriptors
            .iter()
            .any(|b| b.id == id && b.kind == "opencode")
    {
        Ok(Backend::OpenCode(&state.opencode))
    } else {
        Err(Error(
            StatusCode::BAD_REQUEST,
            "Choose a configured Router backend".into(),
        ))
    }
}
fn recovery_state(state: &GatewayState, backend: &str, expected: &str) -> Result<Value, Error> {
    let _preferences = state
        .preferences_lock
        .lock()
        .map_err(|_| "Preferences lock is unavailable".to_string())?;
    let preferences = crate::load_preferences(&state.preferences_path)?;
    let revision = configuration_revision(&preferences.router)?;
    if revision != expected {
        return Err(conflict(
            "Router settings changed. Reload settings before recovery; local edits are retained.",
        ));
    }
    let _studio = state
        .studio_lock
        .lock()
        .map_err(|_| "Studio database lock is unavailable".to_string())?;
    let journal = saved(&state.studio_path, backend)?;
    if let Some(journal) = &journal {
        workspace_matches(state, backend, journal)?;
    }
    let known = configured(&preferences, backend);
    Ok(
        json!({"backend":backend,"revision":revision,"journalRevision":journal_revision(backend,&journal)?,"state":if known.is_some(){"configured"}else if journal.as_ref().is_some_and(|j|j.thread_id.is_some()){"known"}else if journal.is_some(){"uncertain"}else{"none"},"workspace":journal.as_ref().map(|j|&j.workspace),"threadId":known.or_else(||journal.as_ref().and_then(|j|j.thread_id.as_deref()))}),
    )
}
fn candidate(thread: &Value, id: &str, workspace: &str) -> Result<Value, Error> {
    verify(thread, id, workspace)?;
    if thread
        .get("parentThreadId")
        .is_some_and(|value| !value.is_null() && !value.is_string())
        || ["ephemeral", "archived"].iter().any(|key| {
            thread
                .get(*key)
                .is_some_and(|value| !value.is_null() && !value.is_boolean())
        })
    {
        return Err(conflict("Backend returned invalid controller lineage or archive metadata. Inspect the source session before adoption."));
    }
    let status = thread["status"]
        .as_str()
        .or_else(|| thread.pointer("/status/type").and_then(Value::as_str))
        .unwrap_or("");
    if thread["archived"] == true || !matches!(status, "idle" | "notLoaded") {
        return Err(conflict("Only an idle, unarchived root session in the dedicated Router workspace can be adopted. Inspect the source session first."));
    }
    let title = thread["name"]
        .as_str()
        .or_else(|| thread["title"].as_str())
        .unwrap_or(id);
    Ok(
        json!({"threadId":id,"workspace":workspace,"title":title.chars().filter(|c|!c.is_control()).take(300).collect::<String>(),"status":status}),
    )
}
async fn inspect_metadata(
    backend: &Backend<'_>,
    id: &str,
    workspace: &str,
) -> Result<Value, Error> {
    backend
        .probe()
        .await
        .map_err(|e| Error(StatusCode::BAD_GATEWAY, e))?;
    let mut thread = backend
        .read(id, workspace)
        .await
        .map_err(|e| Error(StatusCode::BAD_GATEWAY, e))?;
    if let Backend::OpenCode(server) = backend {
        let statuses = server
            .router_request(crate::opencode_server::RouterOperation::Status, workspace)
            .await
            .map_err(|e| Error(StatusCode::BAD_GATEWAY, e))?;
        if !statuses.is_object() {
            return Err(Error(
                StatusCode::BAD_GATEWAY,
                "OpenCode returned no usable session status inventory".into(),
            ));
        }
        thread["status"] = if statuses.get(id).is_none() {
            json!("idle")
        } else {
            statuses[id]["type"].clone()
        };
    }
    Ok(thread)
}
async fn inspect(state: GatewayState, request: Inspect) -> Result<Value, Error> {
    let engine = backend(&state, &request.backend)?;
    let mut result = disk({
        let state = state.clone();
        let request = request.clone();
        move || recovery_state(&state, &request.backend, &request.expected_revision)
    })
    .await?;
    if let Some(id) = request.thread_id {
        if !matches!(result["state"].as_str(), Some("uncertain" | "known")) {
            return Err(conflict(
                "There is no unfinished controller creation to recover.",
            ));
        }
        if result["threadId"].as_str().is_some_and(|known| known != id) {
            return Err(conflict("The setup journal already holds another controller identity. Verify that controller instead."));
        }
        let workspace = result["workspace"]
            .as_str()
            .ok_or_else(|| conflict("The interrupted workspace is unavailable"))?;
        let thread = inspect_metadata(&engine, &id, workspace).await?;
        result["candidate"] = candidate(&thread, &id, workspace)?;
    }
    Ok(result)
}
fn claim_identity(state: &GatewayState, request: &Adopt) -> Result<(), Error> {
    let _preferences = state
        .preferences_lock
        .lock()
        .map_err(|_| "Preferences lock is unavailable".to_string())?;
    let preferences = crate::load_preferences(&state.preferences_path)?;
    if configuration_revision(&preferences.router)? != request.expected_revision
        || configured(&preferences, &request.backend).is_some()
    {
        return Err(conflict(
            "Router settings changed before adoption. Reload settings; identities were kept.",
        ));
    }
    let _studio = state
        .studio_lock
        .lock()
        .map_err(|_| "Studio database lock is unavailable".to_string())?;
    // Recheck under the write mutex: the initial metadata review held no lock.
    let journal = saved(&state.studio_path, &request.backend)?;
    if journal_revision(&request.backend, &journal)? != request.journal_revision {
        return Err(conflict("The setup journal changed before adoption."));
    }
    let journal = journal.ok_or_else(|| conflict("No interrupted workspace"))?;
    workspace_matches(state, &request.backend, &journal)?;
    if journal
        .thread_id
        .as_deref()
        .is_some_and(|id| id != request.thread_id)
    {
        return Err(conflict(
            "The journal already identifies another controller; it was kept.",
        ));
    }
    remember(
        &state.studio_path,
        &request.backend,
        &journal.workspace,
        &request.thread_id,
    )
    .map_err(Into::into)
}
async fn adopt(state: GatewayState, request: Adopt) -> Result<Value, Error> {
    let _active = Active::claim(&request.backend)?;
    let engine = backend(&state, &request.backend)?;
    let reviewed = inspect(
        state.clone(),
        Inspect {
            backend: request.backend.clone(),
            expected_revision: request.expected_revision.clone(),
            thread_id: None,
        },
    )
    .await?;
    if reviewed["journalRevision"] != request.journal_revision
        || !matches!(reviewed["state"].as_str(), Some("uncertain" | "known"))
        || reviewed["threadId"]
            .as_str()
            .is_some_and(|id| id != request.thread_id)
    {
        return Err(conflict(
            "The controller setup journal changed. Reload recovery before adopting a session.",
        ));
    }
    let workspace = reviewed["workspace"]
        .as_str()
        .ok_or_else(|| conflict("No interrupted workspace"))?;
    // Re-read the exact identity immediately before retaining it. No creation,
    // rename or history reads occur on either recovery endpoint.
    let thread = inspect_metadata(&engine, &request.thread_id, workspace).await?;
    candidate(&thread, &request.thread_id, workspace)?;
    disk({
        let state = state.clone();
        let request = request.clone();
        move || claim_identity(&state, &request)
    })
    .await?;
    engine
        .restrict(thread, &request.thread_id, workspace)
        .await?;
    let snapshot=disk({let state=state.clone();let request=request.clone();move||{
        let _preferences=state.preferences_lock.lock().map_err(|_|"Preferences lock is unavailable".to_string())?;
        let mut preferences=crate::load_preferences(&state.preferences_path)?;
        if configured(&preferences,&request.backend).is_some_and(|id|id!=request.thread_id){return Err(conflict("Another controller was configured during adoption. Both identities were kept; reload settings."));}
        let _studio=state.studio_lock.lock().map_err(|_|"Studio database lock is unavailable".to_string())?;
        let messages=crate::session_state::opening_messages(&state.studio_path)?;
        preferences.router.get_or_insert_with(ThreadRouterPreferences::default).controllers.insert(request.backend.clone(),request.thread_id.clone());
        save_scoped(&state.preferences_path,&preferences,Mutation::Controller(&request.backend,&request.thread_id))?;
        serde_json::to_value(snapshot(&preferences,messages)?).map_err(|e|e.to_string().into())
    }}).await?;
    Ok(
        json!({"snapshot":snapshot,"backend":request.backend,"threadId":request.thread_id,"created":false,"adopted":true,"warning":null}),
    )
}
pub async fn inspect_controller(
    State(state): State<GatewayState>,
    body: Result<Json<Inspect>, JsonRejection>,
) -> Response<Body> {
    let request = match body {
        Ok(Json(r))
            if r.backend.len() <= 64
                && r.expected_revision.len() <= 64
                && r.thread_id.as_deref().is_none_or(valid_id) =>
        {
            r
        }
        _ => {
            return json_error(
                StatusCode::BAD_REQUEST,
                "Invalid controller recovery request",
            )
        }
    };
    if let Err(error) = Active::check(&request.backend) {
        return respond(Err(error));
    }
    let Ok(_permit) = READS.try_acquire() else {
        return json_error(
            StatusCode::TOO_MANY_REQUESTS,
            "Controller recovery reads are busy; try again",
        );
    };
    respond(inspect(state, request).await)
}
pub async fn adopt_controller(
    State(state): State<GatewayState>,
    body: Result<Json<Adopt>, JsonRejection>,
) -> Response<Body> {
    let request = match body {
        Ok(Json(r))
            if r.backend.len() <= 64
                && r.expected_revision.len() <= 64
                && r.journal_revision.len() == 16
                && valid_id(&r.thread_id) =>
        {
            r
        }
        _ => {
            return json_error(
                StatusCode::BAD_REQUEST,
                "Invalid controller adoption request",
            )
        }
    };
    let Ok(permit) = SETUP_SLOTS.try_acquire() else {
        return json_error(
            StatusCode::TOO_MANY_REQUESTS,
            "Controller setup is busy; try again",
        );
    };
    match tokio::spawn(async move {
        let _permit = permit;
        adopt(state, request).await
    })
    .await
    {
        Ok(result) => respond(result),
        Err(_) => json_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Controller adoption stopped. Reload settings; creation was not repeated.",
        ),
    }
}
fn respond(result: Result<Value, Error>) -> Response<Body> {
    match result {
        Ok(value) => json_response(StatusCode::OK, &value),
        Err(Error(status, message)) => json_error(status, &message),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn adoption_review_requires_idle_root_identity_and_journal_tokens_are_scoped() {
        let valid = json!({"id":"created","cwd":"/router","name":"Recovered 中文","status":{"type":"notLoaded"}});
        assert_eq!(
            candidate(&valid, "created", "/router").unwrap()["title"],
            "Recovered 中文"
        );
        for patch in [
            json!({"archived":true}),
            json!({"status":"active"}),
            json!({"status":"retry"}),
            json!({"status":null}),
            json!({"parentThreadId":"parent"}),
            json!({"parentThreadId":false}),
            json!({"ephemeral":"false"}),
            json!({"archived":0}),
            json!({"ephemeral":true}),
            json!({"cwd":"/ordinary-project"}),
            json!({"id":"other"}),
        ] {
            let mut value = valid.clone();
            for (k, v) in patch.as_object().unwrap() {
                value[k] = v.clone();
            }
            assert!(
                candidate(&value, "created", "/router").is_err(),
                "accepted {patch}"
            );
        }
        let pending = Some(Journal {
            workspace: "/router".into(),
            thread_id: None,
        });
        let known = Some(Journal {
            workspace: "/router".into(),
            thread_id: Some("created".into()),
        });
        assert_ne!(
            journal_revision("codex", &pending).unwrap(),
            journal_revision("opencode", &pending).unwrap()
        );
        assert_ne!(
            journal_revision("codex", &pending).unwrap(),
            journal_revision("codex", &known).unwrap()
        );
        let id = format!("recovery-test-{}", uuid::Uuid::new_v4());
        let owned = Active::claim(&id).unwrap();
        assert!(Active::claim(&id).is_err());
        assert!(Active::check(&id).is_err());
        drop(owned);
        assert!(Active::check(&id).is_ok());
    }
}
