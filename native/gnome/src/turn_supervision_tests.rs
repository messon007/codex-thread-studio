use super::*;
use serde_json::json;
fn snapshot() -> Snapshot {
    Snapshot {
        context: Context {
            backend: "codex".into(),
            thread: "thread".into(),
            generation: 1,
        },
        latest_turn: "root".into(),
        state: TurnState::Completed,
        active_turn: None,
        blocked: false,
        error: false,
        original: "Finish A and B".into(),
        reply: "A is done. Shall I do B?".into(),
    }
}
fn job(s: &Snapshot) -> Job {
    Job::start(s.context.clone(), "codex", &s.latest_turn, s, None).unwrap()
}
fn decision(choice: &str, prompt: &str) -> Result<Decision, String> {
    Decision::parse(
        &json!({"decision":choice,"reason":"B remains 中文😀","prompt":prompt}).to_string(),
    )
}
fn dispatch(j: &mut Job, s: &Snapshot, prompt: &str) -> Dispatch {
    let eval = j.poll(s).unwrap();
    let prepared = j
        .evaluated(&eval.ticket, decision("continue", prompt), s)
        .unwrap();
    j.prepared(&prepared, s).unwrap()
}
#[test]
fn only_codex_latest_eligible_turn_with_an_authorized_original_can_start() {
    let mut s = snapshot();
    assert!(Job::start(s.context.clone(), "opencode", "root", &s, None).is_err());
    assert!(Job::start(s.context.clone(), "codex", "old", &s, None).is_err());
    s.blocked = true;
    assert!(Job::start(s.context.clone(), "codex", "root", &s, None).is_err());
    s.blocked = false;
    s.original.clear();
    assert!(Job::start(s.context.clone(), "codex", "root", &s, None).is_err());
    let j = Job::start(
        s.context.clone(),
        "codex",
        "root",
        &s,
        Some("Exact accepted request 中文😀"),
    )
    .unwrap();
    assert_eq!(j.original, "Exact accepted request 中文😀");
    let mut other = s.context.clone();
    other.backend = "other".into();
    assert!(Job::start(other, "codex", "root", &s, Some("accepted")).is_err());
}
#[test]
fn running_or_idle_without_completed_status_does_not_evaluate() {
    let mut s = snapshot();
    s.state = TurnState::InProgress;
    let mut j = job(&s);
    assert!(j.poll(&s).is_none());
    s.state = TurnState::Completed;
    s.active_turn = Some("root".into());
    assert!(j.poll(&s).is_none());
    s.active_turn = None;
    assert!(j.poll(&s).is_some());
    assert!(j.poll(&s).is_none());
}
#[test]
fn completion_evaluates_once_and_retains_logical_root_and_full_context() {
    let mut s = snapshot();
    let mut j = job(&s);
    let eval = j.poll(&s).unwrap();
    let input: Value = serde_json::from_str(&eval.input).unwrap();
    assert_eq!(input["originalUserMessage"], s.original);
    assert_eq!(input["latestAssistantMessage"], s.reply);
    assert!(j.poll(&s).is_none());
    let ticket = j
        .evaluated(&eval.ticket, decision("continue", "Complete B"), &s)
        .unwrap();
    assert_eq!(j.stage, Stage::Preparing);
    assert!(j.poll(&s).is_none());
    let send = j.prepared(&ticket, &s).unwrap();
    assert_eq!(send.text, "[Studio turn supervisor]\nComplete B");
    assert!(j.prepared(&ticket, &s).is_none());
    let follow = j.acknowledged(&send.ticket, Ok("next".into())).unwrap();
    assert!(follow.continuing);
    assert_eq!(follow.previous_turn, "root");
    assert_eq!(follow.turn, "next");
    assert!(j
        .acknowledged(&send.ticket, Ok("duplicate".into()))
        .is_none());
    assert_eq!(
        (&j.root, &j.current, j.count),
        (&"root".to_owned(), &"next".to_owned(), 1)
    );
    s.latest_turn = "next".into();
    s.reply = "B is done".into();
    let input: Value = serde_json::from_str(&j.poll(&s).unwrap().input).unwrap();
    assert_eq!(input["previousSupervisorPrompts"], json!(["Complete B"]));
    assert_eq!(
        input["previousAssistantMessages"],
        json!(["A is done. Shall I do B?"])
    );
}
#[test]
fn terminal_decisions_never_prepare_a_source_send() {
    for choice in ["complete", "needs_user", "error"] {
        let s = snapshot();
        let mut j = job(&s);
        let e = j.poll(&s).unwrap();
        assert!(j.evaluated(&e.ticket, decision(choice, ""), &s).is_none());
        assert!(!j.active());
        assert!(!j.unsettled());
        assert!(j.poll(&s).is_none());
        assert_eq!(
            j.stage,
            if choice == "complete" {
                Stage::Complete
            } else {
                Stage::Stopped
            }
        );
    }
}
#[test]
fn human_mutations_invalidate_evaluation_but_keep_lease_until_it_settles() {
    for method in [
        "turn/start",
        "turn/interrupt",
        "thread/archive",
        "thread/delete",
        "thread/rollback",
        "thread/compact/start",
        "thread/shellCommand",
    ] {
        let s = snapshot();
        let mut j = job(&s);
        let e = j.poll(&s).unwrap();
        j.human_action(method, &json!([]));
        assert!(!j.active() && j.unsettled());
        assert!(j
            .evaluated(&e.ticket, decision("continue", "Do B"), &s)
            .is_none());
        assert!(!j.unsettled());
        assert!(j.poll(&s).is_none());
    }
}
#[test]
fn steering_revises_context_and_prevents_a_second_evaluation_until_old_work_settles() {
    let s = snapshot();
    let mut j = job(&s);
    let e = j.poll(&s).unwrap();
    j.human_action(
        "turn/steer",
        &json!([{"type":"text","text":"Also C 中文😀"}]),
    );
    assert_eq!(j.stage, Stage::Waiting);
    assert!(j.poll(&s).is_none());
    assert!(j
        .evaluated(&e.ticket, decision("continue", "Old stale decision"), &s)
        .is_none());
    let e = j.poll(&s).unwrap();
    let v: Value = serde_json::from_str(&e.input).unwrap();
    assert!(v["additionalUserMessages"][0]
        .as_str()
        .unwrap()
        .contains("Also C 中文😀"));
}
#[test]
fn fresh_preparation_rejects_approval_newer_turn_disconnect_and_changed_reply() {
    for change in 0..4 {
        let s = snapshot();
        let mut j = job(&s);
        let e = j.poll(&s).unwrap();
        let t = j
            .evaluated(&e.ticket, decision("continue", "Do B"), &s)
            .unwrap();
        let mut fresh = s.clone();
        match change {
            0 => fresh.blocked = true,
            1 => fresh.latest_turn = "human".into(),
            2 => fresh.context.generation += 1,
            _ => fresh.reply = "Changed response under the same identity".into(),
        }
        assert!(j.prepared(&t, &fresh).is_none());
        assert!(!j.active() && !j.unsettled());
    }
}
#[test]
fn changed_reply_or_original_during_evaluation_invalidates_its_decision() {
    for change_original in [true, false] {
        let s = snapshot();
        let mut j = job(&s);
        let e = j.poll(&s).unwrap();
        let mut fresh = s.clone();
        if change_original {
            fresh.original = "Changed task".into();
        } else {
            fresh.reply = "Changed reply".into();
        }
        assert!(j
            .evaluated(&e.ticket, decision("continue", "Do B"), &fresh)
            .is_none());
        assert!(!j.active());
    }
}
#[test]
fn early_continuation_notification_waits_for_ack_instead_of_stopping_ownership() {
    let s = snapshot();
    let mut j = job(&s);
    let send = dispatch(&mut j, &s, "Do B");
    let mut early = s.clone();
    early.latest_turn = "accepted-next".into();
    early.state = TurnState::InProgress;
    early.active_turn = Some(early.latest_turn.clone());
    assert!(j.poll(&early).is_none());
    assert_eq!(j.stage, Stage::Sending);
    let follow = j
        .acknowledged(&send.ticket, Ok("accepted-next".into()))
        .unwrap();
    assert!(follow.continuing);
    assert_eq!(j.current, "accepted-next");
    assert!(j.poll(&early).is_none());
}
#[test]
fn stopping_an_inflight_send_still_returns_its_handoff_receipt_without_resuming() {
    let s = snapshot();
    let mut j = job(&s);
    let send = dispatch(&mut j, &s, "Do B");
    j.stop("User stopped supervision");
    assert!(j.unsettled());
    let follow = j
        .acknowledged(&send.ticket, Ok("accepted-next".into()))
        .unwrap();
    assert!(!follow.continuing);
    assert_eq!(follow.previous_turn, "root");
    assert_eq!(j.stage, Stage::Stopped);
    assert!(!j.unsettled());
    assert_eq!(j.count, 0);
    assert!(j.poll(&s).is_none());
}
#[test]
fn uncertain_or_invalid_ack_is_never_retried() {
    for result in [
        Err("Timeout after admission".into()),
        Ok(String::new()),
        Ok("root".into()),
        Ok("bad\0id".into()),
    ] {
        let s = snapshot();
        let mut j = job(&s);
        let send = dispatch(&mut j, &s, "Do B");
        assert!(j.acknowledged(&send.ticket, result).is_none());
        assert!(!j.active() && !j.unsettled());
        assert!(j.poll(&s).is_none());
        assert!(j.prepared(&send.ticket, &s).is_none());
    }
}
#[test]
fn owners_and_backends_isolate_otherwise_identical_turn_ids() {
    let s = snapshot();
    let mut first = job(&s);
    let old = first.poll(&s).unwrap();
    let mut replacement = job(&s);
    let own = replacement.poll(&s).unwrap();
    assert!(replacement
        .evaluated(&old.ticket, decision("continue", "wrong owner"), &s)
        .is_none());
    assert!(replacement.unsettled());
    assert!(replacement
        .evaluated(&own.ticket, decision("complete", ""), &s)
        .is_none());
    assert_eq!(replacement.stage, Stage::Complete);
    let mut other = s.clone();
    other.context.backend = "second-codex".into();
    let mut second = job(&other);
    assert!(second.poll(&other).is_some());
    first.stop("stop first");
    assert!(second.active());
}
#[test]
fn malformed_or_oversized_decisions_do_not_dispatch() {
    for text in [
        "null",
        "{}",
        "not json",
        r#"{"decision":"other","reason":"","prompt":""}"#,
        r#"{"decision":"continue","reason":"","prompt":"  "}"#,
        r#"{"decision":"continue","reason":"","prompt":"ok","approve":true}"#,
    ] {
        assert!(Decision::parse(text).is_err(), "{text}");
    }
    assert!(decision("continue", &"😀".repeat(2001)).is_err());
    assert!(decision("continue", &"😀".repeat(2000)).is_ok());
    assert!(decision("continue", "\u{feff}").is_err());
    assert!(decision("continue", "bad\0prompt").is_err());
}
#[test]
fn utf16_context_budget_rejects_whole_input_without_truncating_evidence() {
    let mut s = snapshot();
    s.reply = "中".repeat(90_000);
    let mut j = job(&s);
    let e = j.poll(&s).unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&e.input).unwrap()["latestAssistantMessage"],
        s.reply
    );
    let mut s = snapshot();
    s.reply = "😀".repeat(50_000);
    let mut j = job(&s);
    assert!(j.poll(&s).is_none());
    assert!(!j.active());
}
#[test]
fn five_continuations_are_a_hard_limit_but_completion_can_still_be_evaluated() {
    for complete_at_limit in [true, false] {
        let mut s = snapshot();
        let mut j = job(&s);
        for n in 0..MAX_CONTINUATIONS {
            let send = dispatch(&mut j, &s, &format!("Step {n}"));
            let next = format!("next-{n}");
            assert!(
                j.acknowledged(&send.ticket, Ok(next.clone()))
                    .unwrap()
                    .continuing
            );
            s.latest_turn = next;
            s.reply = format!("Finished step {n}");
        }
        let e = j.poll(&s).unwrap();
        assert!(j
            .evaluated(
                &e.ticket,
                decision(
                    if complete_at_limit {
                        "complete"
                    } else {
                        "continue"
                    },
                    "Another step"
                ),
                &s
            )
            .is_none());
        assert_eq!(j.count, MAX_CONTINUATIONS);
        assert!(!j.active());
        assert_eq!(
            j.stage,
            if complete_at_limit {
                Stage::Complete
            } else {
                Stage::Stopped
            }
        );
    }
}
#[test]
fn repeated_continuation_and_missing_or_failed_source_stop_without_further_send() {
    let mut s = snapshot();
    let mut j = job(&s);
    let send = dispatch(&mut j, &s, "Same prompt");
    j.acknowledged(&send.ticket, Ok("next".into()));
    s.latest_turn = "next".into();
    let e = j.poll(&s).unwrap();
    assert!(j
        .evaluated(&e.ticket, decision("continue", "Same prompt"), &s)
        .is_none());
    assert!(!j.active());
    for state in [TurnState::Failed, TurnState::Interrupted] {
        let mut s = snapshot();
        let mut j = job(&s);
        s.state = state;
        assert!(j.poll(&s).is_none());
        assert!(!j.active());
    }
    let mut s = snapshot();
    let mut j = job(&s);
    s.reply.clear();
    assert!(j.poll(&s).is_none());
    assert!(!j.active());
}
