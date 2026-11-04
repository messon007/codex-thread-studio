//! Scoped shared Router configuration; never replace unrelated preferences or
//! an opening message's text/capture metadata when editing its responsibility.
use crate::{
    json_error, json_response, GatewayState, OpeningMessage, RouterFallbackPreference,
    StudioPreferences, ThreadRouterPreferences,
};
use axum::{
    body::Body,
    extract::{rejection::JsonRejection, State},
    http::{Response, StatusCode},
    Json,
};
use serde::{Deserialize, Serialize};
#[path = "native_router_setup.rs"]
mod setup;
pub use setup::post_controller;
pub use setup::recovery::{adopt_controller, inspect_controller};
#[path = "native_router_dispatch.rs"]
mod dispatch;
pub(crate) use dispatch::images as image_input;
pub use dispatch::{
    acknowledge_jobs, get_archive, get_jobs, get_targets, post_follow, post_job, post_prepare,
    post_scope, stop_job, Runtime,
};
use std::{
    collections::{hash_map::DefaultHasher, BTreeMap},
    hash::{Hash, Hasher},
    io::Write,
    path::Path,
};

enum Mutation<'a> {
    Configuration,
    Responsibility(&'a str),
    Controller(&'a str, &'a str),
}
/// Keep unknown fields from newer clients too; only write this form's fields.
fn save_scoped(
    path: &Path,
    preferences: &StudioPreferences,
    mutation: Mutation<'_>,
) -> Result<(), String> {
    let mut raw: serde_json::Value = match std::fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(|error| error.to_string())?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => serde_json::json!({}),
        Err(error) => return Err(error.to_string()),
    };
    if !raw.is_object() {
        return Err("Studio preferences are not an object".into());
    }
    let router = preferences
        .router
        .as_ref()
        .ok_or("No Router configuration to save")?;
    if !raw["router"].is_object() {
        raw["router"] = serde_json::json!({});
    }
    match mutation {
        Mutation::Controller(backend, id) => {
            if raw["router"]
                .get("controllers")
                .is_some_and(|value| !value.is_object())
            {
                return Err("Router controllers are not an object".into());
            }
            if raw["router"].get("controllers").is_none() {
                raw["router"]["controllers"] = serde_json::json!({});
            }
            raw["router"]["controllers"][backend] = id.into();
        }
        Mutation::Configuration => {
            raw["router"]["controllerBackend"] = serde_json::to_value(&router.controller_backend)
                .map_err(|error| error.to_string())?;
            raw["router"]["fallbacks"] =
                serde_json::to_value(&router.fallbacks).map_err(|error| error.to_string())?;
            for (key, role) in &router.responsibilities {
                if let Some(entry) = raw["router"]["responsibilities"]
                    .get_mut(key)
                    .and_then(serde_json::Value::as_object_mut)
                {
                    entry.insert("fallback".into(), role.fallback.clone().into());
                }
            }
        }
        Mutation::Responsibility(key) => {
            for (legacy, role) in &router.responsibilities {
                if legacy == key || (!legacy.contains(':') && key == format!("codex:{legacy}")) {
                    if let Some(entry) = raw["router"]["responsibilities"]
                        .get_mut(legacy)
                        .and_then(serde_json::Value::as_object_mut)
                    {
                        entry.insert("description".into(), role.description.clone().into());
                    }
                }
            }
        }
    }
    let parent = path.parent().ok_or("Missing preferences directory")?;
    std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let mut temporary =
        tempfile::NamedTempFile::new_in(parent).map_err(|error| error.to_string())?;
    serde_json::to_writer_pretty(&mut temporary, &raw).map_err(|error| error.to_string())?;
    temporary.flush().map_err(|error| error.to_string())?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|error| error.to_string())?;
    temporary.persist(path).map_err(|error| error.to_string())?;
    Ok(())
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Opening {
    responsibility: String,
    opening_message: String,
    revision: String,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Snapshot {
    revision: String,
    controller_backend: String,
    controllers: BTreeMap<String, String>,
    fallbacks: Vec<RouterFallbackPreference>,
    openings: BTreeMap<String, Opening>,
    empty_opening_revision: String,
}
pub(crate) fn revision(value: &impl Serialize) -> Result<String, String> {
    let mut hash = DefaultHasher::new();
    serde_json::to_vec(value)
        .map_err(|error| error.to_string())?
        .hash(&mut hash);
    Ok(format!("{:016x}", hash.finish()))
}
fn configuration_revision(router: &Option<ThreadRouterPreferences>) -> Result<String, String> {
    revision(&router.as_ref().map(|router|serde_json::json!({"controllerBackend":router.controller_backend,"controllers":router.controllers,"threadId":router.thread_id,"fallbacks":router.fallbacks,"legacyFallbacks":router.responsibilities.iter().map(|(key,role)|(key,&role.fallback)).collect::<BTreeMap<_,_>>()})))
}
fn opening(message: Option<&OpeningMessage>) -> Result<Opening, String> {
    Ok(Opening {
        responsibility: message
            .map(|message| message.responsibility.clone())
            .unwrap_or_default(),
        opening_message: message
            .map(|message| message.text.chars().take(800).collect())
            .unwrap_or_default(),
        revision: revision(&message)?,
    })
}
fn snapshot(
    preferences: &StudioPreferences,
    messages: BTreeMap<String, OpeningMessage>,
) -> Result<Snapshot, String> {
    let router = preferences.router.clone().unwrap_or_default();
    let mut controllers = router.controllers.clone();
    if let Some(id) = &router.thread_id {
        controllers
            .entry("codex".into())
            .or_insert_with(|| id.clone());
    }
    let mut openings = messages
        .into_iter()
        .map(|(key, message)| Ok((key, opening(Some(&message))?)))
        .collect::<Result<BTreeMap<_, _>, String>>()?;
    for (key, responsibility) in &router.responsibilities {
        let key = if key.contains(':') {
            key.clone()
        } else {
            format!("codex:{key}")
        };
        if !openings.contains_key(&key) {
            let mut entry = opening(None)?;
            entry.responsibility = responsibility.description.clone();
            openings.insert(key, entry);
        }
    }
    let mut fallbacks = router.fallbacks;
    if fallbacks.is_empty() {
        for (key, responsibility) in &router.responsibilities {
            let key = if key.contains(':') {
                key.clone()
            } else {
                format!("codex:{key}")
            };
            if matches!(
                responsibility.fallback.as_str(),
                "fallback" | "learning" | "general"
            ) && !controllers
                .iter()
                .any(|(backend, id)| key == format!("{backend}:{id}"))
                && fallbacks.len() < 3
            {
                fallbacks.push(RouterFallbackPreference {
                    session_key: key,
                    condition: "No regular session is suitable for the user query.".into(),
                });
            }
        }
    }
    Ok(Snapshot {
        revision: configuration_revision(&preferences.router)?,
        controller_backend: router.controller_backend.unwrap_or_else(|| {
            if controllers.contains_key("codex") {
                "codex".into()
            } else {
                controllers
                    .keys()
                    .next()
                    .cloned()
                    .unwrap_or_else(|| "codex".into())
            }
        }),
        controllers,
        fallbacks,
        openings,
        empty_opening_revision: revision(&Option::<OpeningMessage>::None)?,
    })
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Patch {
    expected_revision: String,
    controller_backend: String,
    fallbacks: Vec<RouterFallbackPreference>,
}
fn patch_preferences(
    preferences: &mut StudioPreferences,
    patch: Patch,
    backends: &[crate::BackendDescriptor],
) -> Result<(), String> {
    if configuration_revision(&preferences.router)? != patch.expected_revision {
        return Err("Router settings changed; reload before saving".into());
    }
    let previous = preferences
        .router
        .as_ref()
        .and_then(|router| router.controller_backend.as_deref())
        .unwrap_or("codex");
    let has_controller = preferences.router.as_ref().is_some_and(|router| {
        router.controllers.contains_key(&patch.controller_backend)
            || (patch.controller_backend == "codex" && router.thread_id.is_some())
    });
    if previous != patch.controller_backend && !has_controller {
        return Err(
            "This backend has no saved Router controller; set up its controller before switching"
                .into(),
        );
    }
    if !backends.iter().any(|backend| {
        backend.id == patch.controller_backend && matches!(backend.kind, "codex" | "opencode")
    }) {
        return Err("Choose a configured Router backend".into());
    }
    if patch.fallbacks.iter().any(|fallback| {
        !fallback
            .session_key
            .split_once(':')
            .is_some_and(|(id, _)| backends.iter().any(|backend| backend.id == id))
    }) {
        return Err("A fallback backend is no longer configured".into());
    }
    let router = preferences
        .router
        .get_or_insert_with(ThreadRouterPreferences::default);
    router.controller_backend = Some(patch.controller_backend);
    router.fallbacks = patch.fallbacks;
    // The explicit native selection supersedes the legacy fallback flags,
    // including when the user intentionally removes all fallbacks.
    for responsibility in router.responsibilities.values_mut() {
        responsibility.fallback = "none".into();
    }
    crate::validate_preferences(preferences)
}
async fn operation(state: GatewayState, patch: Option<Patch>) -> Response<Body> {
    match tokio::task::spawn_blocking(move || {
        let _preferences = match state.preferences_lock.lock() {
            Ok(guard) => guard,
            Err(_) => {
                return json_error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Preferences lock is unavailable",
                )
            }
        };
        let mut preferences = match crate::load_preferences(&state.preferences_path) {
            Ok(value) => value,
            Err(error) => return json_error(StatusCode::INTERNAL_SERVER_ERROR, &error),
        };
        let _studio = match state.studio_lock.lock() {
            Ok(guard) => guard,
            Err(_) => {
                return json_error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Studio database lock is unavailable",
                )
            }
        };
        // Read the reply metadata before committing preferences, so a broken
        // database cannot turn a reported failed save into a committed update.
        let messages = match crate::session_state::opening_messages(&state.studio_path) {
            Ok(messages) => messages,
            Err(error) => return json_error(StatusCode::INTERNAL_SERVER_ERROR, &error),
        };
        if let Some(patch) = patch {
            if let Err(error) =
                patch_preferences(&mut preferences, patch, &state.backend_descriptors)
            {
                return json_error(
                    if error.contains("changed; reload") {
                        StatusCode::CONFLICT
                    } else {
                        StatusCode::BAD_REQUEST
                    },
                    &error,
                );
            }
            if let Err(error) = save_scoped(
                &state.preferences_path,
                &preferences,
                Mutation::Configuration,
            ) {
                return json_error(StatusCode::INTERNAL_SERVER_ERROR, &error);
            }
        }
        match snapshot(&preferences, messages) {
            Ok(value) => json_response(StatusCode::OK, &value),
            Err(error) => json_error(StatusCode::INTERNAL_SERVER_ERROR, &error),
        }
    })
    .await
    {
        Ok(response) => response,
        Err(_) => json_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Router settings worker stopped",
        ),
    }
}
pub async fn get(State(state): State<GatewayState>) -> Response<Body> {
    operation(state, None).await
}
pub async fn patch(
    State(state): State<GatewayState>,
    body: Result<Json<Patch>, JsonRejection>,
) -> Response<Body> {
    match body {
        Ok(Json(patch)) => operation(state, Some(patch)).await,
        Err(_) => json_error(StatusCode::BAD_REQUEST, "Invalid Router settings update"),
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Responsibility {
    session_key: String,
    expected_revision: String,
    responsibility: String,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SavedResponsibility {
    session_key: String,
    #[serde(flatten)]
    opening: Opening,
}
fn save_responsibility(
    path: &Path,
    request: Responsibility,
) -> Result<SavedResponsibility, String> {
    if !crate::valid_router_session_key(&request.session_key)
        || request.session_key.len() > 320
        || request.responsibility.chars().count() > 4096
    {
        return Err(
            "Responsibility must identify a session and contain at most 4,096 characters".into(),
        );
    }
    let value = crate::native_session_info::update(
        path,
        crate::native_session_info::Edit {
            session_key: request.session_key,
            expected_revision: request.expected_revision,
            question: None,
            responsibility: Some(request.responsibility),
            captured_at: String::new(),
            capture_only: false,
            truncated: false,
        },
        "",
    )?;
    Ok(SavedResponsibility {
        session_key: value.session_key,
        opening: opening(value.message.as_ref())?,
    })
}
pub(crate) fn synchronize_responsibility(path: &Path, key: &str, text: &str) -> Result<(), String> {
    let mut preferences = crate::load_preferences(path)?;
    if sync_legacy_responsibility(&mut preferences, key, text) {
        save_scoped(path, &preferences, Mutation::Responsibility(key))?;
    }
    Ok(())
}
pub async fn responsibility(
    State(state): State<GatewayState>,
    body: Result<Json<Responsibility>, JsonRejection>,
) -> Response<Body> {
    let request = match body {
        Ok(Json(request)) => request,
        Err(_) => return json_error(StatusCode::BAD_REQUEST, "Invalid responsibility update"),
    };
    match tokio::task::spawn_blocking(move||{
        let _preferences=match state.preferences_lock.lock(){Ok(guard)=>guard,Err(_)=>return json_error(StatusCode::INTERNAL_SERVER_ERROR,"Preferences lock is unavailable")};
        let _studio=match state.studio_lock.lock(){Ok(guard)=>guard,Err(_)=>return json_error(StatusCode::INTERNAL_SERVER_ERROR,"Studio database lock is unavailable")};
        match save_responsibility(&state.studio_path,request){Ok(value)=>{
            let result=(||->Result<(),String>{let mut preferences=crate::load_preferences(&state.preferences_path)?;if sync_legacy_responsibility(&mut preferences,&value.session_key,&value.opening.responsibility){save_scoped(&state.preferences_path,&preferences,Mutation::Responsibility(&value.session_key))?;}Ok(())})();
            match result{Ok(())=>json_response(StatusCode::OK,&value),Err(error)=>json_error(StatusCode::INTERNAL_SERVER_ERROR,&format!("Responsibility saved; legacy settings could not be synchronized. Reload before retrying: {error}"))}
        },Err(error)=>json_error(if error.contains("changed; reload"){StatusCode::CONFLICT}else{StatusCode::BAD_REQUEST},&error)}
    }).await{Ok(response)=>response,Err(_)=>json_error(StatusCode::INTERNAL_SERVER_ERROR,"Responsibility worker stopped")}
}
fn sync_legacy_responsibility(preferences: &mut StudioPreferences, key: &str, text: &str) -> bool {
    let Some(router) = preferences.router.as_mut() else {
        return false;
    };
    let mut changed = false;
    for (legacy, role) in &mut router.responsibilities {
        if (legacy == key || (!legacy.contains(':') && key == format!("codex:{legacy}")))
            && role.description != text
        {
            role.description = text.into();
            changed = true;
        }
    }
    changed
}

#[cfg(test)]
mod tests {
    #[test]
    fn legacy_snapshot_and_runtime_agree_on_active_controller_and_fallback_aliases() {
        let preferences:crate::StudioPreferences=serde_json::from_value(serde_json::json!({"router":{"controllers":{"opencode":"router"},"responsibilities":{"legacy":{"description":"Role","fallback":"general"}}}})).unwrap();
        let config = super::snapshot(&preferences, std::collections::BTreeMap::new()).unwrap();
        assert_eq!(config.controller_backend, "opencode");
        assert_eq!(
            crate::native_router_plan::controller(&preferences).unwrap(),
            "opencode:router"
        );
        assert_eq!(config.fallbacks.len(), 1);
        assert_eq!(config.fallbacks[0].session_key, "codex:legacy");
    }
    use super::*;
    use serde_json::json;
    #[test]
    fn narrow_responsibility_commit_preserves_unknown_capture_fields_and_rejects_a_racing_writer() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("studio.sqlite3");
        crate::session_state::initialize(&path).unwrap();
        let original = json!({"text":"opening","source":"capture","futureCapture":{"range":[1,2]},"responsibility":"old"});
        let old = serde_json::to_string(&original).unwrap();
        crate::session_state::replace_opening_message(&path, "codex:target", None, &old).unwrap();
        let current: OpeningMessage = serde_json::from_value(original.clone()).unwrap();
        save_responsibility(
            &path,
            Responsibility {
                session_key: "codex:target".into(),
                expected_revision: revision(&Some(current)).unwrap(),
                responsibility: "new".into(),
            },
        )
        .unwrap();
        let saved: serde_json::Value = serde_json::from_str(
            &crate::session_state::opening_message_raw(&path, "codex:target")
                .unwrap()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(saved["futureCapture"], original["futureCapture"]);
        assert_eq!(saved["text"], original["text"]);
        assert_eq!(saved["responsibility"], "new");
        assert!(crate::session_state::replace_opening_message(
            &path,
            "codex:target",
            Some(&old),
            &old
        )
        .unwrap_err()
        .contains("changed"));
        assert!(
            crate::session_state::replace_opening_message(&path, "codex:target", None, &old)
                .is_err()
        );
    }
    #[test]
    fn changing_backend_cannot_leave_a_working_router_without_a_controller() {
        let mut preferences = StudioPreferences {
            router: Some(ThreadRouterPreferences {
                controller_backend: Some("codex".into()),
                controllers: BTreeMap::from([("codex".into(), "existing".into())]),
                ..Default::default()
            }),
            ..Default::default()
        };
        let before = preferences.clone();
        let patch=serde_json::from_value(json!({"expectedRevision":configuration_revision(&preferences.router).unwrap(),"controllerBackend":"opencode","fallbacks":[]})).unwrap();
        assert!(patch_preferences(
            &mut preferences,
            patch,
            &crate::backend_config::builtin_descriptors()
        )
        .unwrap_err()
        .contains("no saved Router controller"));
        assert_eq!(preferences, before);
    }
    #[test]
    fn scoped_file_writes_preserve_unknown_top_level_router_and_legacy_fields() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("settings.json");
        let original = json!({"future":{"enabled":true},"queueDepth":3,"router":{"controllerBackend":"codex","controllers":{"codex":"keep","opencode":"keep-opencode"},"futureRouter":{"revision":7},"responsibilities":{"codex:target":{"description":"old","fallback":"fallback","futureRole":[1,2]}}}});
        std::fs::write(&path, serde_json::to_vec(&original).unwrap()).unwrap();
        let mut preferences = crate::load_preferences(&path).unwrap();
        let patch=serde_json::from_value(json!({"expectedRevision":configuration_revision(&preferences.router).unwrap(),"controllerBackend":"opencode","fallbacks":[]})).unwrap();
        patch_preferences(
            &mut preferences,
            patch,
            &crate::backend_config::builtin_descriptors(),
        )
        .unwrap();
        save_scoped(&path, &preferences, Mutation::Configuration).unwrap();
        assert!(sync_legacy_responsibility(
            &mut preferences,
            "codex:target",
            ""
        ));
        save_scoped(
            &path,
            &preferences,
            Mutation::Responsibility("codex:target"),
        )
        .unwrap();
        let saved: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(saved["future"], original["future"]);
        assert_eq!(
            saved["router"]["futureRouter"],
            original["router"]["futureRouter"]
        );
        assert_eq!(
            saved["router"]["responsibilities"]["codex:target"]["futureRole"],
            json!([1, 2])
        );
        assert_eq!(
            saved["router"]["controllers"],
            original["router"]["controllers"]
        );
        assert_eq!(saved["router"]["controllerBackend"], "opencode");
        assert_eq!(
            saved["router"]["responsibilities"]["codex:target"]["description"],
            ""
        );
    }
    #[test]
    fn clearing_legacy_fields_stays_cleared_and_revisions_track_only_router_configuration() {
        let mut preferences = StudioPreferences {
            router: Some(ThreadRouterPreferences {
                controller_backend: Some("codex".into()),
                responsibilities: BTreeMap::from([(
                    "codex:target".into(),
                    crate::RouterResponsibility {
                        description: "old".into(),
                        fallback: "fallback".into(),
                    },
                )]),
                ..Default::default()
            }),
            ..Default::default()
        };
        let first = snapshot(&preferences, BTreeMap::new()).unwrap();
        assert_eq!(first.fallbacks.len(), 1);
        assert_eq!(first.openings["codex:target"].responsibility, "old");
        assert!(sync_legacy_responsibility(
            &mut preferences,
            "codex:target",
            ""
        ));
        assert_eq!(
            configuration_revision(&preferences.router).unwrap(),
            first.revision
        );
        let explicit = BTreeMap::from([("codex:target".into(), OpeningMessage::default())]);
        assert!(
            snapshot(&preferences, explicit).unwrap().openings["codex:target"]
                .responsibility
                .is_empty()
        );
        let patch = serde_json::from_value(
            json!({"expectedRevision":first.revision,"controllerBackend":"codex","fallbacks":[]}),
        )
        .unwrap();
        patch_preferences(
            &mut preferences,
            patch,
            &crate::backend_config::builtin_descriptors(),
        )
        .unwrap();
        assert!(snapshot(&preferences, BTreeMap::new())
            .unwrap()
            .fallbacks
            .is_empty());
    }
    #[test]
    fn settings_patch_preserves_controller_identity_legacy_data_and_unrelated_fields() {
        let mut preferences = StudioPreferences {
            theme: Some("dark".into()),
            queue_depth: Some(3),
            router: Some(ThreadRouterPreferences {
                thread_id: Some("old".into()),
                controllers: BTreeMap::from([("codex".into(), "existing".into())]),
                responsibilities: BTreeMap::from([(
                    "codex:target".into(),
                    crate::RouterResponsibility {
                        description: "legacy".into(),
                        fallback: "none".into(),
                    },
                )]),
                ..Default::default()
            }),
            ..Default::default()
        };
        let original = configuration_revision(&preferences.router).unwrap();
        let backends = crate::backend_config::builtin_descriptors();
        let patch = serde_json::from_value(
            json!({"expectedRevision":original,"controllerBackend":"codex","fallbacks":[]}),
        )
        .unwrap();
        patch_preferences(&mut preferences, patch, &backends).unwrap();
        assert_eq!(preferences.theme.as_deref(), Some("dark"));
        assert_eq!(preferences.queue_depth, Some(3));
        let router = preferences.router.as_ref().unwrap();
        assert_eq!(router.controllers["codex"], "existing");
        assert_eq!(router.thread_id.as_deref(), Some("old"));
        assert_eq!(
            router.responsibilities["codex:target"].description,
            "legacy"
        );
        let stale = serde_json::from_value(
            json!({"expectedRevision":original,"controllerBackend":"codex","fallbacks":[]}),
        )
        .unwrap();
        assert!(patch_preferences(&mut preferences, stale, &backends)
            .unwrap_err()
            .contains("changed"));
        assert!(serde_json::from_value::<Patch>(json!({"expectedRevision":"x","controllerBackend":"codex","fallbacks":[],"controllers":{"codex":"replacement"}})).is_err());
    }
    #[test]
    fn responsibility_preserves_capture_text_and_rejects_stale_editor() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("studio.sqlite3");
        let original = OpeningMessage {
            text: "Original opening text".into(),
            source: "history".into(),
            captured_at: "timestamp".into(),
            truncated: true,
            ..Default::default()
        };
        crate::session_state::put_opening_message(&path, "codex:target", Some(&original)).unwrap();
        let rev = revision(&Some(original.clone())).unwrap();
        let saved = save_responsibility(
            &path,
            Responsibility {
                session_key: "codex:target".into(),
                expected_revision: rev.clone(),
                responsibility: "Native 中文 responsibility".into(),
            },
        )
        .unwrap();
        assert_eq!(saved.opening.responsibility, "Native 中文 responsibility");
        let current = crate::session_state::opening_message(&path, "codex:target")
            .unwrap()
            .unwrap();
        assert_eq!(current.text, original.text);
        assert_eq!(current.source, original.source);
        assert_eq!(current.captured_at, original.captured_at);
        assert!(current.truncated);
        assert!(save_responsibility(
            &path,
            Responsibility {
                session_key: "codex:target".into(),
                expected_revision: rev,
                responsibility: "stale".into()
            }
        )
        .unwrap_err()
        .contains("changed"));
    }
}
