use studio_teams::{
    queue::{Queue, State},
    Error, Session,
};
fn session() -> Session {
    Session {
        backend: "codex".into(),
        thread: "existing-thread".into(),
    }
}
fn queue() -> (tempfile::TempDir, Queue) {
    let dir = tempfile::tempdir().unwrap();
    let mut q = Queue::open(dir.path().join("queue.sqlite3")).unwrap();
    q.activate("teams", 1).unwrap();
    q.set_ready(&session(), true).unwrap();
    (dir, q)
}
fn add(q: &mut Queue, key: &str) -> studio_teams::queue::Entry {
    q.admit(
        session(),
        "teams",
        key,
        "teams",
        1,
        "Synthetic task pointer",
    )
    .unwrap()
}

#[test]
fn local_retries_are_idempotent_but_send_permission_is_single_use() {
    let (_d, mut q) = queue();
    let e = add(&mut q, "delivery");
    let again = add(&mut q, "delivery");
    assert_eq!(e.id, again.id);
    assert_eq!(
        q.admit(session(), "teams", "delivery", "teams", 1, "Changed")
            .unwrap_err(),
        Error::IdempotencyConflict
    );
    let e = q.authorize_head(&session()).unwrap().unwrap();
    let dispatch = e.dispatch_id.as_ref().unwrap();
    q.prepare_send(&e.id, dispatch).unwrap();
    assert_eq!(
        q.prepare_send(&e.id, dispatch).unwrap_err(),
        Error::InvalidState
    );
    q.accepted(&e.id, dispatch, "native-queued").unwrap();
    q.started(&e.id, dispatch, "native-turn").unwrap();
    q.settled(&e.id, "native-turn").unwrap();
    assert_eq!(add(&mut q, "delivery").state, State::Settled);
    assert!(q.authorize_head(&session()).unwrap().is_none());
}
#[test]
fn independent_consumers_cannot_both_authorize_or_send() {
    let (dir, mut q) = queue();
    add(&mut q, "one");
    let mut other = Queue::open(dir.path().join("queue.sqlite3")).unwrap();
    let e = q.authorize_head(&session()).unwrap().unwrap();
    assert!(other.authorize_head(&session()).unwrap().is_none());
    other
        .prepare_send(&e.id, e.dispatch_id.as_ref().unwrap())
        .unwrap();
    assert_eq!(
        q.prepare_send(&e.id, e.dispatch_id.as_ref().unwrap())
            .unwrap_err(),
        Error::InvalidState
    );
}
#[test]
fn reserve_and_fence_before_handoff_prevent_send() {
    let (_dir, mut q) = queue();
    let e = add(&mut q, "one");
    let a = q.authorize_head(&session()).unwrap().unwrap();
    let hold = q
        .reserve(&session(), "supervisor", 1, "Whole logical task")
        .unwrap();
    assert!(hold.granted);
    assert!(q.authorize_head(&session()).unwrap().is_none());
    assert_eq!(
        q.prepare_send(&e.id, a.dispatch_id.as_ref().unwrap())
            .unwrap_err(),
        Error::InvalidState
    );
    assert!(!q.release(&hold.id, 2).unwrap());
    assert!(q.release(&hold.id, 1).unwrap());
    let a = q.authorize_head(&session()).unwrap().unwrap();
    let fence = q.fence("teams", 1, "pause-1").unwrap();
    assert_eq!(fence.entries[0].state, State::Withdrawn);
    assert_eq!(
        q.prepare_send(&e.id, a.dispatch_id.as_ref().unwrap())
            .unwrap_err(),
        Error::InvalidState
    );
    assert_eq!(q.fence("teams", 1, "pause-1").unwrap().entries[0].id, e.id);
}
#[test]
fn crash_after_intent_freezes_and_no_match_is_not_evidence_of_rejection() {
    let (dir, mut q) = queue();
    add(&mut q, "one");
    add(&mut q, "two");
    let e = q.authorize_head(&session()).unwrap().unwrap();
    q.prepare_send(&e.id, e.dispatch_id.as_ref().unwrap())
        .unwrap();
    drop(q);
    let mut q = Queue::open(dir.path().join("queue.sqlite3")).unwrap();
    q.recover().unwrap();
    q.set_ready(&session(), true).unwrap();
    assert_eq!(q.get(&e.id).unwrap().state, State::Unknown);
    assert!(q.authorize_head(&session()).unwrap().is_none());
    assert_eq!(
        q.prepare_send(&e.id, e.dispatch_id.as_ref().unwrap())
            .unwrap_err(),
        Error::InvalidState
    );
    q.accepted(
        &e.id,
        e.dispatch_id.as_ref().unwrap(),
        "recovered-native-item",
    )
    .unwrap();
    assert_eq!(q.get(&e.id).unwrap().state, State::NativeQueued);
}
#[test]
fn crash_before_send_intent_can_requeue_but_requires_fresh_readiness() {
    let (dir, mut q) = queue();
    add(&mut q, "one");
    let old = q.authorize_head(&session()).unwrap().unwrap();
    drop(q);
    let mut q = Queue::open(dir.path().join("queue.sqlite3")).unwrap();
    q.recover().unwrap();
    assert!(q.authorize_head(&session()).unwrap().is_none());
    q.set_ready(&session(), true).unwrap();
    let next = q.authorize_head(&session()).unwrap().unwrap();
    assert_eq!(old.id, next.id);
    assert_ne!(old.dispatch_id, next.dispatch_id);
}
#[test]
fn receipts_can_arrive_before_rpc_ack_without_rewinding_started_state() {
    let (_d, mut q) = queue();
    add(&mut q, "one");
    let e = q.authorize_head(&session()).unwrap().unwrap();
    let d = e.dispatch_id.as_ref().unwrap();
    q.prepare_send(&e.id, d).unwrap();
    q.started(&e.id, d, "turn").unwrap();
    q.accepted(&e.id, d, "queued-id").unwrap();
    assert_eq!(q.get(&e.id).unwrap().state, State::Started);
    assert_eq!(
        q.withdrawn(&e.id, "queued-id").unwrap_err(),
        Error::ReconciliationRequired
    );
    q.settled(&e.id, "turn").unwrap();
    q.accepted(&e.id, d, "queued-id").unwrap();
    assert_eq!(q.get(&e.id).unwrap().state, State::Settled);
}
#[test]
fn later_supervision_is_pending_until_native_item_is_withdrawn() {
    let (_d, mut q) = queue();
    add(&mut q, "one");
    let e = q.authorize_head(&session()).unwrap().unwrap();
    let d = e.dispatch_id.as_ref().unwrap();
    q.prepare_send(&e.id, d).unwrap();
    q.accepted(&e.id, d, "native").unwrap();
    let hold = q
        .reserve(&session(), "supervision", 1, "New reservation")
        .unwrap();
    assert!(!hold.granted);
    assert_eq!(hold.pending.len(), 1);
    q.withdrawn(&e.id, "native").unwrap();
    assert!(
        q.reserve(&session(), "supervision", 1, "New reservation")
            .unwrap()
            .granted
    );
}
#[test]
fn explicit_not_submitted_receipt_requeues_only_if_scope_still_valid() {
    let (_d, mut q) = queue();
    add(&mut q, "one");
    let e = q.authorize_head(&session()).unwrap().unwrap();
    let d = e.dispatch_id.as_ref().unwrap();
    q.prepare_send(&e.id, d).unwrap();
    q.confirmed_not_submitted(&e.id, d, "Correlated native rejection")
        .unwrap();
    assert_eq!(q.get(&e.id).unwrap().state, State::Queued);
    let next = q.authorize_head(&session()).unwrap().unwrap();
    assert_ne!(next.dispatch_id, e.dispatch_id);
    let d = next.dispatch_id.as_ref().unwrap();
    q.prepare_send(&e.id, d).unwrap();
    q.fence("teams", 1, "pause").unwrap();
    q.confirmed_not_submitted(&e.id, d, "Correlated rejection after fence")
        .unwrap();
    assert_eq!(q.get(&e.id).unwrap().state, State::Withdrawn);
}
#[test]
fn abandoning_unknown_is_explicit_and_does_not_claim_native_cancellation() {
    let (_d, mut q) = queue();
    add(&mut q, "one");
    let e = q.authorize_head(&session()).unwrap().unwrap();
    let d = e.dispatch_id.as_ref().unwrap();
    q.prepare_send(&e.id, d).unwrap();
    q.uncertain(&e.id, d).unwrap();
    q.fence("teams", 1, "disable").unwrap();
    q.activate("human", 1).unwrap();
    q.admit(
        session(),
        "human",
        "human-message",
        "human",
        1,
        "Ordinary queued input",
    )
    .unwrap();
    assert!(q.authorize_head(&session()).unwrap().is_none());
    q.abandon_unknown(
        &e.id,
        d,
        "User inspected conversation, abandons automatic recovery",
    )
    .unwrap();
    assert_eq!(q.get(&e.id).unwrap().state, State::Abandoned);
    assert_eq!(
        q.authorize_head(&session()).unwrap().unwrap().source,
        "human"
    );
    assert_eq!(add(&mut q, "one").state, State::Abandoned);
}
#[test]
fn fifo_and_busy_observations_are_preserved_without_dropping_messages() {
    let (_d, mut q) = queue();
    let one = add(&mut q, "one");
    let two = add(&mut q, "two");
    q.set_ready(&session(), false).unwrap();
    assert!(q.authorize_head(&session()).unwrap().is_none());
    q.set_ready(&session(), true).unwrap();
    let e = q.authorize_head(&session()).unwrap().unwrap();
    assert_eq!(one.id, e.id);
    let d = e.dispatch_id.as_ref().unwrap();
    q.prepare_send(&e.id, d).unwrap();
    q.started(&e.id, d, "turn").unwrap();
    assert!(q.authorize_head(&session()).unwrap().is_none());
    q.settled(&e.id, "turn").unwrap();
    assert_eq!(q.authorize_head(&session()).unwrap().unwrap().id, two.id);
}

#[test]
fn abandoned_unknown_still_blocks_strict_reservation_until_late_receipt_settles() {
    let (_d, mut q) = queue();
    add(&mut q, "one");
    let e = q.authorize_head(&session()).unwrap().unwrap();
    let d = e.dispatch_id.as_ref().unwrap();
    q.prepare_send(&e.id, d).unwrap();
    q.uncertain(&e.id, d).unwrap();
    q.fence("teams", 1, "stop").unwrap();
    q.abandon_unknown(&e.id, d, "User abandons retry, outcome still unknown")
        .unwrap();
    assert_eq!(
        q.activate("teams", 2).unwrap_err(),
        Error::ReconciliationRequired
    );
    assert!(
        !q.reserve(&session(), "supervisor", 1, "Strict ownership")
            .unwrap()
            .granted
    );
    q.accepted(&e.id, d, "late-queue-id").unwrap();
    assert_eq!(q.get(&e.id).unwrap().state, State::Abandoned);
    q.started(&e.id, d, "late-turn").unwrap();
    assert!(
        !q.reserve(&session(), "supervisor", 1, "Strict ownership")
            .unwrap()
            .granted
    );
    q.settled(&e.id, "late-turn").unwrap();
    assert!(
        q.reserve(&session(), "supervisor", 1, "Strict ownership")
            .unwrap()
            .granted
    );
    q.activate("teams", 2).unwrap();
}

#[test]
fn two_owners_cannot_hold_the_same_idle_session() {
    let (dir, mut first) = queue();
    let mut second = Queue::open(dir.path().join("queue.sqlite3")).unwrap();
    let supervisor = first
        .reserve(&session(), "supervisor", 1, "work in progress")
        .unwrap();
    assert!(supervisor.granted);
    assert_eq!(
        second
            .reserve(&session(), "sender", 1, "user message")
            .unwrap_err(),
        Error::Blocked
    );
    assert_eq!(second.reservations(&session()).unwrap().len(), 1);
    first.release(&supervisor.id, 1).unwrap();
    assert!(
        second
            .reserve(&session(), "sender", 1, "user message")
            .unwrap()
            .granted
    );
}
