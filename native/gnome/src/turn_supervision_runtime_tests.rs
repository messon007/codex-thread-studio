use super::*;
use crate::model::Backend;
use serde_json::json;
use std::sync::Mutex;
use tokio::sync::Semaphore;
struct Step<T> {
    value: T,
    gate: Option<Arc<Semaphore>>,
}
impl<T> Step<T> {
    fn now(value: T) -> Self {
        Self { value, gate: None }
    }
}
#[derive(Clone, Default)]
struct Fake(Arc<Mutex<Data>>);
#[derive(Default)]
struct Data {
    reads: VecDeque<Step<Result<Snapshot, String>>>,
    evaluations: VecDeque<Step<Result<(Decision, Option<String>), String>>>,
    calls: Vec<String>,
    inputs: Vec<String>,
}
impl Fake {
    fn new(reads: Vec<Snapshot>, choices: Vec<&str>) -> Self {
        Self(Arc::new(Mutex::new(Data{reads:reads.into_iter().map(|s|Step::now(Ok(s))).collect(),evaluations:choices.into_iter().map(|choice|Step::now(Decision::parse(&json!({"decision":choice,"reason":"Evidence checked 中文😀","prompt":if choice=="continue"{"Complete B"}else{""}}).to_string()).map(|d|(d,None)))).collect(),..Default::default()})))
    }
}
async fn wait<T>(step: Step<T>) -> T {
    if let Some(gate) = step.gate {
        gate.acquire().await.unwrap().forget();
    }
    step.value
}
impl Service for Fake {
    async fn read(&self, source: Source, _stop: Arc<AtomicBool>) -> Result<Snapshot, String> {
        let step = {
            let mut data = self.0.lock().unwrap();
            data.calls.push(format!("read:{}", source.context.thread));
            data.reads.pop_front().expect("Unexpected source read")
        };
        wait(step).await
    }
    async fn evaluate(
        &self,
        _id: u64,
        context: translation::Context,
        effort: String,
        input: String,
        _stop: Arc<AtomicBool>,
    ) -> Result<(Decision, Option<String>), String> {
        let step = {
            let mut data = self.0.lock().unwrap();
            data.calls.push(format!(
                "evaluate:{}:{}:{effort}",
                context.thread, context.model
            ));
            data.inputs.push(input);
            data.evaluations.pop_front().expect("Unexpected evaluation")
        };
        wait(step).await
    }
}
fn snapshot() -> Snapshot {
    Snapshot {
        context: Context {
            backend: "codex".into(),
            thread: "source".into(),
            generation: 7,
        },
        latest_turn: "root".into(),
        state: core::TurnState::Completed,
        active_turn: None,
        blocked: false,
        error: false,
        original: "Finish A and B".into(),
        reply: "A is done; B remains".into(),
    }
}
fn start() -> Start {
    Start {
        request: 1,
        context: translation::Context {
            backend: Backend {
                id: "codex".into(),
                name: "Test".into(),
                kind: "codex".into(),
                socket_path: "/ws/codex".into(),
            },
            socket: 7,
            thread: "source".into(),
            cwd: "/private/project".into(),
            model: "source-model".into(),
            ready: true,
        },
        turn: "root".into(),
        effort: "medium".into(),
    }
}
fn runtime(fake: Fake) -> Runtime<Fake> {
    Runtime::new(fake, "codex".into(), 7)
}
async fn next(r: &mut Runtime<Fake>) -> Effect {
    tokio::time::timeout(Duration::from_secs(3), r.next())
        .await
        .expect("No supervisor effect")
}
async fn prepare(r: &mut Runtime<Fake>) -> (u64, Value) {
    loop {
        match next(r).await {
            Effect::Prepare {
                id,
                request,
                context,
                root,
                text,
            } => {
                assert_eq!(request, 1);
                assert_eq!(context, snapshot().context);
                assert_eq!(root, "/private/project");
                return (
                    id,
                    json!({"threadId":"source","input":[{"type":"text","text":text}],"model":"current-model","effort":"high","serviceTier":"fast","approvalPolicy":"on-request","sandboxPolicy":{"type":"readOnly"}}),
                );
            }
            Effect::Changed(_) => {}
            other => panic!("Expected preparation, got {other:?}"),
        }
    }
}
async fn ready(r: &mut Runtime<Fake>) -> u64 {
    loop {
        match next(r).await {
            Effect::Ready { id } => return id,
            Effect::Changed(_) => {}
            other => panic!("Expected wire admission, got {other:?}"),
        }
    }
}
async fn changed(r: &mut Runtime<Fake>, stage: Stage) -> View {
    loop {
        match next(r).await {
            Effect::Changed(view) if view.stage == stage => return view,
            Effect::Changed(_) => {}
            other => panic!("Expected {stage:?}, got {other:?}"),
        }
    }
}

#[tokio::test]
async fn continuation_uses_three_source_reads_normal_preparation_and_single_wire_admission() {
    let s = snapshot();
    let fake = Fake::new(vec![s.clone(), s.clone(), s], vec!["continue"]);
    let mut r = runtime(fake.clone());
    r.start(start()).unwrap();
    let (id, params) = prepare(&mut r).await;
    assert!(r.holding("source"));
    assert!(r.prepared(id, params.clone()));
    assert_eq!(ready(&mut r).await, id);
    let send = r.dispatch(id).unwrap();
    assert_eq!(send, params);
    assert_eq!(send["serviceTier"], "fast");
    assert!(r.dispatch(id).is_none());
    assert!(r.acknowledge(id, Ok("next".into())));
    assert!(!r.acknowledge(id, Ok("duplicate".into())));
    match next(&mut r).await {
        Effect::Follow {
            id: handoff,
            receipt,
            prompt,
        } => {
            assert_eq!(handoff, id);
            assert!(receipt.continuing);
            assert_eq!(receipt.previous_turn, "root");
            assert_eq!(receipt.turn, "next");
            assert_eq!(prompt, "[Studio turn supervisor]\nComplete B");
        }
        other => panic!("{other:?}"),
    }
    let view = changed(&mut r, Stage::Waiting).await;
    assert_eq!(view.count, 1);
    assert_eq!(view.current, "next");
    assert_eq!(view.root, "root");
    r.tick();
    assert!(
        r.jobs.is_empty(),
        "routing handoff must precede another read"
    );
    assert!(r.followed(id, Ok(())));
    assert!(!r.followed(id, Ok(())));
    let data = fake.0.lock().unwrap();
    assert_eq!(
        data.calls,
        [
            "read:source",
            "evaluate:source:source-model:medium",
            "read:source",
            "read:source"
        ]
    );
    let input: Value = serde_json::from_str(&data.inputs[0]).unwrap();
    assert_eq!(input["originalUserMessage"], "Finish A and B");
}
#[tokio::test]
async fn complete_or_needs_user_never_prepares_a_source_turn() {
    for choice in ["complete", "needs_user", "error"] {
        let fake = Fake::new(vec![snapshot(), snapshot()], vec![choice]);
        let mut r = runtime(fake.clone());
        r.start(start()).unwrap();
        let view = changed(
            &mut r,
            if choice == "complete" {
                Stage::Complete
            } else {
                Stage::Stopped
            },
        )
        .await;
        assert!(!view.holding);
        assert!(!r.holding("source"));
        r.tick();
        assert!(r.jobs.is_empty());
        assert_eq!(fake.0.lock().unwrap().calls.len(), 3);
    }
}
#[tokio::test]
async fn changed_final_source_and_failed_host_preparation_never_release_send_params() {
    let mut changed_source = snapshot();
    changed_source.reply = "Edited after evaluator completion".into();
    let fake = Fake::new(
        vec![snapshot(), snapshot(), changed_source],
        vec!["continue"],
    );
    let mut r = runtime(fake);
    r.start(start()).unwrap();
    let (id, params) = prepare(&mut r).await;
    assert!(r.prepared(id, params));
    loop {
        match next(&mut r).await {
            Effect::Abandon { id: old, .. } => {
                assert_eq!(old, id);
                break;
            }
            Effect::Changed(_) => {}
            other => panic!("{other:?}"),
        }
    }
    assert!(r.dispatch(id).is_none());
    assert!(!r.holding("source"));
    let fake = Fake::new(vec![snapshot(), snapshot()], vec!["continue"]);
    let mut r = runtime(fake);
    r.start(start()).unwrap();
    let (id, _) = prepare(&mut r).await;
    assert!(r.preparation_failed(id, "Environment changed".into()));
    assert!(!r.preparation_failed(id, "duplicate".into()));
    assert!(matches!(next(&mut r).await, Effect::Abandon { .. }));
    assert!(!r.holding("source"));
}
#[tokio::test]
async fn stop_before_wire_clears_preparation_but_stop_after_wire_retains_real_late_receipt() {
    for sent in [false, true] {
        let fake = Fake::new(vec![snapshot(), snapshot(), snapshot()], vec!["continue"]);
        let mut r = runtime(fake);
        r.start(start()).unwrap();
        let (id, params) = prepare(&mut r).await;
        r.prepared(id, params);
        ready(&mut r).await;
        if sent {
            assert!(r.dispatch(id).is_some());
        }
        r.stop("source", "Stopped explicitly");
        assert_eq!(r.holding("source"), sent);
        if sent {
            assert!(r.start(start()).is_err());
            assert!(r.acknowledge(id, Ok("accepted-before-stop".into())));
            match next(&mut r).await {
                Effect::Follow {
                    id: handoff,
                    receipt,
                    prompt,
                } => {
                    assert_eq!(handoff, id);
                    assert!(!receipt.continuing);
                    assert_eq!(receipt.turn, "accepted-before-stop");
                    assert!(prompt.ends_with("Complete B"));
                }
                other => panic!("{other:?}"),
            }
            assert!(
                r.holding("source"),
                "late receipt retains handoff ownership"
            );
            assert!(r.followed(id, Ok(())));
        } else {
            assert!(r.dispatch(id).is_none());
            assert!(matches!(next(&mut r).await, Effect::Abandon { .. }));
            assert!(!r.acknowledge(id, Ok("unsent".into())));
        }
        assert!(!r.holding("source"));
    }
}
#[tokio::test]
async fn human_steer_keeps_evaluator_lease_until_cleanup_then_rechecks_new_source() {
    let fake = Fake::new(
        vec![snapshot(), snapshot(), snapshot(), snapshot()],
        vec!["continue", "complete"],
    );
    let gate = Arc::new(Semaphore::new(0));
    fake.0.lock().unwrap().evaluations[0].gate = Some(gate.clone());
    let mut r = runtime(fake.clone());
    r.start(start()).unwrap();
    changed(&mut r, Stage::Evaluating).await;
    r.human_action(
        "source",
        "turn/steer",
        &json!([{"type":"text","text":"Keep C too"}]),
    );
    r.tick();
    assert_eq!(r.jobs.len(), 1);
    assert!(r.holding("source"));
    assert!(r.start(start()).is_err());
    gate.add_permits(1);
    changed(&mut r, Stage::Waiting).await; // Steer update precedes the stale callback.
    while !r.jobs.is_empty() {
        let _ = next(&mut r).await;
    }
    r.tick();
    let view = changed(&mut r, Stage::Complete).await;
    assert!(!view.holding);
    let data = fake.0.lock().unwrap();
    assert_eq!(data.inputs.len(), 2);
    let input: Value = serde_json::from_str(&data.inputs[1]).unwrap();
    assert!(input["additionalUserMessages"][0]
        .as_str()
        .unwrap()
        .contains("Keep C too"));
}
#[tokio::test]
async fn invalid_identity_and_changed_host_input_do_not_reach_a_wire_turn() {
    let fake = Fake::new(vec![], vec![]);
    let mut r = runtime(fake.clone());
    for field in ["backend", "socket", "kind", "ready"] {
        let mut s = start();
        match field {
            "backend" => s.context.backend.id = "other".into(),
            "socket" => s.context.socket = 8,
            "kind" => s.context.backend.kind = "opencode".into(),
            _ => s.context.ready = false,
        }
        assert!(r.start(s).is_err());
    }
    assert!(fake.0.lock().unwrap().calls.is_empty());
    let fake = Fake::new(vec![snapshot(), snapshot()], vec!["continue"]);
    let mut r = runtime(fake);
    r.start(start()).unwrap();
    let (id, mut params) = prepare(&mut r).await;
    params["input"][0]["text"] = "Unsupplied instruction".into();
    assert!(!r.prepared(id, params));
    assert!(r.dispatch(id).is_none());
    assert!(matches!(next(&mut r).await, Effect::Abandon { .. }));
}
#[tokio::test]
async fn lost_acknowledgement_stops_without_replaying_and_option_changes_scope_to_the_source() {
    let fake = Fake::new(vec![snapshot(), snapshot(), snapshot()], vec!["continue"]);
    let mut r = runtime(fake.clone());
    let mut s = start();
    s.context.model = "initial".into();
    r.start(s).unwrap();
    r.options("source", "changed-model".into(), "high".into())
        .unwrap();
    let (id, params) = prepare(&mut r).await;
    r.prepared(id, params);
    ready(&mut r).await;
    r.dispatch(id).unwrap();
    assert!(r.acknowledge(id, Err("Transport closed after acceptance".into())));
    let view = changed(&mut r, Stage::Stopped).await;
    assert!(!view.holding);
    r.tick();
    assert!(r.jobs.is_empty());
    assert!(fake
        .0
        .lock()
        .unwrap()
        .calls
        .contains(&"evaluate:source:changed-model:high".into()));
}
#[tokio::test]
async fn shutdown_cancels_but_waits_for_owned_work_and_never_restarts_it() {
    let fake = Fake::new(vec![snapshot(), snapshot()], vec!["continue"]);
    let gate = Arc::new(Semaphore::new(0));
    fake.0.lock().unwrap().evaluations[0].gate = Some(gate.clone());
    let mut r = runtime(fake.clone());
    r.start(start()).unwrap();
    changed(&mut r, Stage::Evaluating).await;
    let cancel = r.entries["source"].cancel.clone();
    let waiter = tokio::spawn(async move {
        r.shutdown().await;
        r
    });
    tokio::task::yield_now().await;
    assert!(cancel.load(Ordering::Acquire));
    assert!(!waiter.is_finished());
    gate.add_permits(1);
    let r = waiter.await.unwrap();
    assert!(r.jobs.is_empty());
    assert!(!r.holding("source"));
    assert!(r.entries["source"].job.as_ref().unwrap().stage() == Stage::Stopped);
}

#[tokio::test]
async fn running_source_never_evaluates_until_a_later_authoritative_completed_read() {
    let mut running = snapshot();
    running.state = core::TurnState::InProgress;
    running.active_turn = Some("root".into());
    let fake = Fake::new(vec![running, snapshot(), snapshot()], vec!["complete"]);
    let mut r = runtime(fake.clone());
    r.start(start()).unwrap();
    while !r.jobs.is_empty() {
        assert!(matches!(next(&mut r).await, Effect::Changed(_)));
    }
    assert!(fake.0.lock().unwrap().inputs.is_empty());
    r.tick();
    let view = changed(&mut r, Stage::Complete).await;
    assert!(!view.holding);
    assert_eq!(fake.0.lock().unwrap().inputs.len(), 1);
}

#[tokio::test]
async fn session_and_worker_admission_are_bounded_and_stopped_inflight_owners_cannot_be_evicted() {
    let gate = Arc::new(Semaphore::new(0));
    let fake = Fake::new(vec![snapshot(), snapshot()], vec![]);
    for step in &mut fake.0.lock().unwrap().reads {
        step.gate = Some(gate.clone());
    }
    let mut r = runtime(fake.clone());
    for index in 0..ENTRIES {
        let mut start = start();
        start.context.thread = format!("source-{index:02}");
        r.start(start).unwrap();
        assert!(r.jobs.len() <= WORKERS);
    }
    assert_eq!(r.entries.len(), ENTRIES);
    assert_eq!(r.jobs.len(), WORKERS);
    let mut extra = start();
    extra.context.thread = "extra".into();
    assert!(r.start(extra.clone()).is_err());
    r.stop("source-00", "Stop");
    assert!(r.holding("source-00"));
    assert!(r.start(extra.clone()).is_err());
    r.stop("source-03", "Stop");
    r.start(extra).unwrap();
    assert_eq!(r.entries.len(), ENTRIES);
    assert!(r.entries.contains_key("source-00"));
    let keys = r.entries.keys().cloned().collect::<Vec<_>>();
    for key in keys {
        r.stop(&key, "Stop");
    }
    gate.add_permits(WORKERS);
    r.shutdown().await;
    assert!(r.jobs.is_empty());
    assert_eq!(fake.0.lock().unwrap().calls.len(), WORKERS);
}

#[tokio::test]
async fn stop_during_final_read_keeps_ownership_until_the_read_settles_and_never_sends() {
    let fake = Fake::new(vec![snapshot(), snapshot(), snapshot()], vec!["continue"]);
    let gate = Arc::new(Semaphore::new(0));
    fake.0.lock().unwrap().reads[2].gate = Some(gate.clone());
    let mut r = runtime(fake);
    r.start(start()).unwrap();
    let (id, params) = prepare(&mut r).await;
    r.prepared(id, params);
    r.stop("source", "Stop before final read settles");
    assert!(r.holding("source"));
    assert!(r.start(start()).is_err());
    gate.add_permits(1);
    loop {
        match next(&mut r).await {
            Effect::Abandon { id: old, .. } => {
                assert_eq!(old, id);
                break;
            }
            Effect::Changed(_) => {}
            other => panic!("{other:?}"),
        }
    }
    assert!(!r.holding("source"));
    assert!(r.dispatch(id).is_none());
}

#[tokio::test]
async fn failed_handoff_stops_continuation_and_preserves_the_accepted_source_receipt() {
    let fake = Fake::new(vec![snapshot(), snapshot(), snapshot()], vec!["continue"]);
    let mut r = runtime(fake);
    r.start(start()).unwrap();
    let (id, params) = prepare(&mut r).await;
    r.prepared(id, params);
    ready(&mut r).await;
    r.dispatch(id).unwrap();
    r.acknowledge(id, Ok("accepted".into()));
    assert!(
        matches!(next(&mut r).await,Effect::Follow{receipt:Follow{turn,..},..}if turn=="accepted")
    );
    r.tick();
    assert!(r.jobs.is_empty());
    assert!(r.holding("source"));
    assert!(r.followed(
        id,
        Err("Routed handoff not confirmed; inspect the accepted source turn".into())
    ));
    let view = changed(&mut r, Stage::Stopped).await;
    assert_eq!(view.current, "accepted");
    assert_eq!(view.count, 1);
    assert!(!view.holding);
    assert!(view.warning.contains("not confirmed"));
    r.tick();
    assert!(r.jobs.is_empty());
}

#[tokio::test]
async fn preparation_and_ack_deadlines_stop_without_automatic_replay() {
    for sent in [false, true] {
        let fake = Fake::new(vec![snapshot(), snapshot(), snapshot()], vec!["continue"]);
        let mut r = runtime(fake);
        r.start(start()).unwrap();
        let (id, params) = prepare(&mut r).await;
        if sent {
            r.prepared(id, params);
            ready(&mut r).await;
            r.dispatch(id).unwrap();
        }
        r.entries
            .get_mut("source")
            .unwrap()
            .pending
            .as_mut()
            .unwrap()
            .since = Instant::now() - Duration::from_secs(60);
        r.tick();
        assert!(r.jobs.is_empty());
        assert!(!r.holding("source"));
        assert!(r.dispatch(id).is_none());
        if !sent {
            assert!(matches!(next(&mut r).await, Effect::Abandon { .. }));
        }
        let view = changed(&mut r, Stage::Stopped).await;
        assert!(view
            .reason
            .contains(if sent { "acknowledgement" } else { "timed out" }));
        assert!(!r.acknowledge(id, Ok("late-after-timeout".into())));
        r.tick();
        assert!(r.jobs.is_empty());
    }
}

#[tokio::test]
async fn failed_fresh_read_retains_cleanup_uncertainty_and_releases_only_its_evaluator_lease() {
    let fake = Fake::new(vec![snapshot(), snapshot()], vec!["continue"]);
    {
        let mut data = fake.0.lock().unwrap();
        data.reads[1].value = Err("Source unavailable".into());
        data.evaluations[0].value = Decision::parse(
            &json!({"decision":"continue","reason":"B remains","prompt":"Complete B"}).to_string(),
        )
        .map(|decision| (decision, Some("Utility cleanup not confirmed".into())));
    }
    let mut r = runtime(fake);
    r.start(start()).unwrap();
    let view = changed(&mut r, Stage::Stopped).await;
    assert!(view.reason.contains("Source unavailable"));
    assert!(view.reason.contains("cleanup not confirmed"));
    assert!(!view.holding);
    assert!(!r.entries["source"].job.as_ref().unwrap().unsettled());
}

#[tokio::test]
async fn changed_source_options_or_permissions_invalidate_unsent_parameters() {
    for options in [false, true] {
        let fake = Fake::new(vec![snapshot(), snapshot(), snapshot()], vec!["continue"]);
        let mut r = runtime(fake);
        r.start(start()).unwrap();
        let (id, params) = prepare(&mut r).await;
        if options {
            r.options("source", "new-source-model".into(), "high".into())
                .unwrap();
        } else {
            r.invalidate_preparation("source");
        }
        assert!(!r.prepared(id, params));
        assert!(r.dispatch(id).is_none());
        assert!(matches!(next(&mut r).await, Effect::Abandon { .. }));
        let view = changed(&mut r, Stage::Stopped).await;
        assert!(view.reason.contains("options or permissions changed"));
        assert!(!view.holding);
    }
}
