#![cfg(test)]
use super::*;
fn setup() -> (tempfile::TempDir, Registry, Guard) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("maps.sqlite3");
    let map = store::create(
        &path,
        CreateMapRequest {
            backend: "codex".into(),
            thread_id: "target".into(),
            goal: "Goal 中文😀".into(),
            definition_of_done: "Human confirms".into(),
            structure: "hierarchy".into(),
            items: vec![
                CreateMapItem {
                    id: Some("root".into()),
                    parent_id: None,
                    kind: "topic".into(),
                    title: "Root".into(),
                    summary: "".into(),
                    state: "notStarted".into(),
                    position: 0,
                },
                CreateMapItem {
                    id: Some("child".into()),
                    parent_id: Some("root".into()),
                    kind: "topic".into(),
                    title: "Child".into(),
                    summary: "".into(),
                    state: "notStarted".into(),
                    position: 0,
                },
            ],
        },
    )
    .unwrap();
    let registry = Registry::default();
    let storage = Storage {
        path: Arc::new(path),
        lock: Arc::default(),
        workers: Arc::new(Semaphore::new(8)),
        backend: "codex".into(),
        thread: "target".into(),
    };
    let guard = registry
        .reserve(
            storage,
            Scope {
                id: map.id,
                revision: map.revision,
            },
            7,
            Arc::new(AtomicU64::new(7)),
            false,
        )
        .unwrap();
    (directory, registry, guard)
}
fn call(id: &str, turn: &str, target: &str) -> Value {
    json!({"id":id,"method":"item/tool/call","params":{"threadId":"target","turnId":turn,"callId":"same-call","tool":"update_session_map","arguments":{"operations":[{"op":"setCurrent","itemId":target}]}}})
}
fn update(revision: u64) -> protocol::Update {
    protocol::Update {
        revision,
        operations: vec![MapOperation::SetCurrent {
            item_id: Some("child".into()),
        }],
    }
}
#[tokio::test]
async fn routed_tool_is_owned_and_duplicate_delivery_cannot_change_the_map_twice() {
    let (_dir, registry, guard) = setup();
    guard.bind("turn-one").unwrap();
    let (input, mut output) = mpsc::channel(8);
    let (events, _) = broadcast::channel(8);
    assert!(registry.intercept(
        &call("rpc-one", "turn-one", "root"),
        7,
        input.clone(),
        events.clone()
    ));
    let reply = tokio::time::timeout(Duration::from_secs(3), output.recv())
        .await
        .unwrap()
        .unwrap();
    let reply: Value = serde_json::from_str(&reply).unwrap();
    assert_eq!(reply["result"]["success"], true);
    assert!(registry.intercept(
        &call("rpc-two", "turn-one", "root"),
        7,
        input.clone(),
        events.clone()
    ));
    let reply: Value = serde_json::from_str(&output.recv().await.unwrap()).unwrap();
    assert_eq!(reply["id"], "rpc-two");
    assert_eq!(reply["result"]["success"], true);
    assert!(registry.intercept(&call("rpc-three", "turn-one", "child"), 7, input, events));
    let reply: Value = serde_json::from_str(&output.recv().await.unwrap()).unwrap();
    assert_eq!(reply["result"]["success"], false);
    assert!(guard
        .finish(Some(Ok(update(1))))
        .await
        .contains("revision 2"));
    let map = guard.entry.storage.read().unwrap().unwrap();
    assert_eq!(map.revision, 2);
    assert_eq!(map.current_item_id.as_deref(), Some("root"));
    assert_eq!(map.last_synced_turn_id.as_deref(), Some("turn-one"));
}
#[tokio::test]
async fn early_or_foreign_calls_are_rejected_and_fallback_is_applied_once_after_ack() {
    let (_dir, _, guard) = setup();
    let reply = handle(&guard.entry, &call("early", "turn-one", "root"), 7);
    assert_eq!(reply.1["success"], false);
    assert_eq!(guard.entry.storage.read().unwrap().unwrap().revision, 1);
    guard.bind("turn-one").unwrap();
    let reply = handle(&guard.entry, &call("foreign", "other-turn", "root"), 7);
    assert_eq!(reply.1["success"], false);
    assert!(guard
        .finish(Some(Ok(update(1))))
        .await
        .contains("revision 2"));
    guard.finish(Some(Ok(update(1)))).await;
    assert_eq!(guard.entry.storage.read().unwrap().unwrap().revision, 2);
}
#[tokio::test]
async fn newer_human_changes_and_replacement_maps_are_never_rebased() {
    let (_dir, _, guard) = setup();
    guard.bind("turn-one").unwrap();
    let scope = guard.scope();
    store::apply_operations(
        &guard.entry.storage.path,
        "codex",
        "target",
        ApplyOperationsRequest {
            base_revision: scope.revision,
            map_id: Some(scope.id),
            actor: "user".into(),
            source_turn_id: None,
            operations: vec![MapOperation::SetState {
                item_id: "root".into(),
                state: "done".into(),
            }],
        },
    )
    .unwrap();
    let notice = guard.finish(Some(Ok(update(1)))).await;
    assert!(notice.contains("conflict"));
    let map = guard.entry.storage.read().unwrap().unwrap();
    assert_eq!(map.revision, 2);
    assert_eq!(
        map.items.iter().find(|i| i.id == "root").unwrap().state,
        "done"
    );
    let scope = Scope {
        id: map.id,
        revision: map.revision,
    };
    store::remove_at_version(
        &guard.entry.storage.path,
        "codex",
        "target",
        Some(scope.revision),
        Some(&scope.id),
    )
    .unwrap();
    let replacement = store::create(
        &guard.entry.storage.path,
        CreateMapRequest {
            backend: "codex".into(),
            thread_id: "target".into(),
            goal: "Replacement".into(),
            definition_of_done: "".into(),
            structure: "blank".into(),
            items: vec![],
        },
    )
    .unwrap();
    let registry = Registry::default();
    let old = registry
        .reserve(
            guard.entry.storage.clone(),
            scope,
            7,
            Arc::new(AtomicU64::new(7)),
            true,
        )
        .unwrap();
    old.bind("turn-one").unwrap();
    assert!(old.finish(Some(Ok(update(2)))).await.contains("replaced"));
    assert_eq!(
        guard.entry.storage.read().unwrap().unwrap().id,
        replacement.id
    );
}
#[tokio::test]
async fn recovery_recognizes_an_already_applied_turn_without_replaying_it() {
    let (_dir, _, guard) = setup();
    guard.bind("turn-one").unwrap();
    let original = guard.scope();
    guard.finish(Some(Ok(update(1)))).await;
    let registry = Registry::default();
    let recovered = registry
        .reserve(
            guard.entry.storage.clone(),
            original,
            9,
            Arc::new(AtomicU64::new(9)),
            true,
        )
        .unwrap();
    recovered.bind("turn-one").unwrap();
    assert_eq!(
        handle(&recovered.entry, &call("old-tool", "turn-one", "root"), 9).1["success"],
        false
    );
    assert!(recovered
        .finish(Some(Ok(update(1))))
        .await
        .contains("already updated"));
    assert_eq!(recovered.entry.storage.read().unwrap().unwrap().revision, 2);
}
#[tokio::test]
async fn restarted_provider_unsafe_completion_and_failed_write_block_mutation() {
    let (_dir, _, guard) = setup();
    guard.bind("turn-one").unwrap();
    guard.entry.epoch.store(8, Ordering::SeqCst);
    assert_eq!(
        handle(&guard.entry, &call("old", "turn-one", "root"), 7).1["success"],
        false
    );
    assert!(guard
        .finish(Some(Ok(update(1))))
        .await
        .contains("backend changed"));
    assert_eq!(guard.entry.storage.read().unwrap().unwrap().revision, 1);
    let (_dir, _, guard) = setup();
    guard.bind("turn-one").unwrap();
    let mut p = call("unsafe", "turn-one", "root");
    p["params"]["arguments"] =
        json!({"operations":[{"op":"setState","itemId":"root","state":"done"}]});
    assert_eq!(handle(&guard.entry, &p, 7).1["success"], false);
    let first = handle(
        &guard.entry,
        &call("invalid", "turn-one", "missing-item"),
        7,
    );
    assert_eq!(first.1["success"], false);
    guard.finish(Some(Ok(update(1)))).await;
    assert_eq!(guard.entry.storage.read().unwrap().unwrap().revision, 1);
}
#[test]
fn config_ownership_and_notice_budgets_are_explicit() {
    let (_dir, registry, guard) = setup();
    let thread = guard.entry.storage.thread.clone();
    drop(guard);
    registry.observe_configuration(&json!({"method":"thread/resume","params":{"threadId":thread,"developerInstructions":"Ordinary user instructions"}}));
    assert!(!registry.configured("target"));
    registry.observe_configuration(&json!({"method":"thread/resume","params":{"threadId":"target","developerInstructions":"This thread has an optional Session Map maintained by the client. authoritative data"}}));
    assert!(registry.configured("target"));
    assert!(!registry
        .entries
        .lock()
        .unwrap()
        .items
        .contains_key("target"));
    registry.observe_configuration(&json!({"method":"thread/resume","params":{"threadId":"target","developerInstructions":null}}));
    assert!(!registry.configured("target"));
    let bounded = protocol::notice(&"中文😀\0".repeat(5000));
    assert!(bounded.len() <= 4096);
    assert!(!bounded.contains('\0'));
    assert!(bounded.ends_with('…'));
}

#[test]
fn a_private_start_ack_binds_the_scope_before_the_next_provider_frame() {
    let (_dir, registry, guard) = setup();
    registry.observe_request(
        &json!({"id":"browser","method":"turn/start","params":{"threadId":"target"}}),
    );
    assert!(registry.entries.lock().unwrap().requests.is_empty());
    registry.observe_request(
        &json!({"id":"studio-native-read-42","method":"turn/start","params":{"threadId":"target"}}),
    );
    registry
        .observe_reply(&json!({"id":"studio-native-read-42","result":{"turn":{"id":"turn-one"}}}));
    assert_eq!(
        guard.entry.state.lock().unwrap().turn.as_deref(),
        Some("turn-one")
    );
    assert!(registry.entries.lock().unwrap().requests.is_empty());
    assert!(guard.bind("other-turn").is_err());
    let outcome = handle(&guard.entry, &call("immediate-tool", "turn-one", "root"), 7);
    assert_eq!(outcome.1["success"], true);
}
