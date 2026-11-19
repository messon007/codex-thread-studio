use studio_teams::*;
use tempfile::TempDir;

#[derive(Debug)]
struct Applied {
    task: Option<Task>,
    entity_id: Option<String>,
    effect_ids: Vec<String>,
}
struct Fixture {
    dir: TempDir,
    store: Store,
    group: Group,
    a: Caller,
    b: Caller,
    c: Caller,
}
impl Fixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(dir.path().join("teams.sqlite3")).unwrap();
        let members = (0..3)
            .map(|i| MemberDraft {
                session: Session {
                    backend: "codex".into(),
                    thread: format!("thread-{i}"),
                },
                name: format!("member-{i}"),
                responsibility: "Test fixture".into(),
            })
            .collect();
        let group = store
            .create_group(&Caller::user(), "create", "Group", members)
            .unwrap();
        let callers: Vec<_> = group
            .members
            .iter()
            .map(|m| {
                Caller::member(
                    &m.id,
                    Turn {
                        session: m.session.clone(),
                        generation: "process-1".into(),
                        id: format!("turn-{}", m.id),
                    },
                )
            })
            .collect();
        for c in &callers {
            store
                .observe_turn(c.turn.as_ref().unwrap(), TurnState::Active)
                .unwrap();
        }
        Self {
            dir,
            store,
            group,
            a: callers[0].clone(),
            b: callers[1].clone(),
            c: callers[2].clone(),
        }
    }
    fn run(&mut self, who: &Caller, cmd: Command) -> Result<Applied> {
        let r = self
            .store
            .execute(who, &self.group.id, &uuid::Uuid::new_v4().to_string(), cmd)?;
        let task = r
            .task
            .map(|t| self.store.task(who, &self.group.id, &t.id))
            .transpose()?;
        Ok(Applied {
            task,
            entity_id: r.entity_id,
            effect_ids: r.effect_ids,
        })
    }
    fn delegate(
        &mut self,
        who: &Caller,
        to: usize,
        dependencies: Vec<String>,
        parent: Option<Parent>,
    ) -> Task {
        self.run(
            who,
            Command::Delegate {
                delegation: Delegation {
                    to_member_id: self.group.members[to].id.clone(),
                    title: "Verify fixture".into(),
                    request: "Complete the synthetic check".into(),
                    acceptance: vec![Criterion {
                        id: "check".into(),
                        text: "Check passes".into(),
                    }],
                    dependencies,
                    parent,
                    continuation: None,
                },
            },
        )
        .unwrap()
        .task
        .unwrap()
    }
    fn get(&self, id: &str) -> Task {
        self.store.task(&self.a, &self.group.id, id).unwrap()
    }
}
fn v(t: &Task) -> TaskVersion {
    TaskVersion {
        task_id: t.id.clone(),
        expected_revision: t.revision,
        attempt_id: t.attempt_id.clone(),
    }
}
fn checkpoint() -> Continuation {
    Continuation {
        summary: "Saved context".into(),
        next_action: "Continue check".into(),
    }
}
fn submission() -> Submission {
    Submission {
        summary: "Checked".into(),
        evidence: vec![],
        criteria: vec![CriterionResult {
            criterion_id: "check".into(),
            status: CriterionStatus::Met,
            explanation: "Synthetic assertion passed".into(),
            evidence_indexes: vec![],
        }],
        limitations: "Synthetic only".into(),
    }
}
fn assessment(pass: bool) -> Assessment {
    Assessment {
        summary: "Reproduced check".into(),
        criteria: vec![AssessmentItem {
            criterion_id: "check".into(),
            verdict: if pass { Verdict::Pass } else { Verdict::Fail },
            explanation: "Independent assertion".into(),
            evidence: vec![],
        }],
    }
}

#[test]
fn task_waiting_does_not_lock_session_and_answers_do_not_complete_tasks() {
    let mut f = Fixture::new();
    let (a, b, c) = (f.a.clone(), f.b.clone(), f.c.clone());
    let t = f.delegate(&a, 1, vec![], None);
    let t = f
        .run(&b, Command::Begin { task: v(&t) })
        .unwrap()
        .task
        .unwrap();
    let reply = f
        .run(
            &b,
            Command::Ask {
                task: v(&t),
                to: a.actor.clone(),
                question: "Clarify the check".into(),
                blocking: true,
                continuation: Some(checkpoint()),
            },
        )
        .unwrap();
    let question_id = reply.entity_id.unwrap();
    let waiting = reply.task.unwrap();
    assert_eq!(waiting.status, Status::Waiting);
    let unrelated = f.delegate(&a, 1, vec![], None);
    assert_eq!(
        f.run(
            &b,
            Command::Begin {
                task: v(&unrelated)
            }
        )
        .unwrap()
        .task
        .unwrap()
        .status,
        Status::Working
    );
    assert_eq!(
        f.run(
            &c,
            Command::Answer {
                task: v(&waiting),
                question_id: question_id.clone(),
                answer: "Forged answer".into()
            }
        )
        .unwrap_err(),
        Error::Forbidden
    );
    let answered = f
        .run(
            &a,
            Command::Answer {
                task: v(&waiting),
                question_id,
                answer: "Use the original criterion".into(),
            },
        )
        .unwrap();
    let ready = answered.task.unwrap();
    assert!(ready.ready);
    assert_eq!(ready.status, Status::Waiting);
    assert_eq!(answered.effect_ids.len(), 1);
    let working = f
        .run(&b, Command::Resume { task: v(&ready) })
        .unwrap()
        .task
        .unwrap();
    let submitted = f
        .run(
            &b,
            Command::Submit {
                task: v(&working),
                submission: submission(),
            },
        )
        .unwrap()
        .task
        .unwrap();
    assert_eq!(submitted.status, Status::Submitted);
    let review = Command::Review {
        task: v(&submitted),
        result_id: submitted.results[0].id.clone(),
        accept: true,
        assessment: assessment(true),
        requested_changes: None,
    };
    assert_eq!(
        f.run(&a, review.clone()).unwrap_err(),
        Error::ResultNotReady
    );
    f.store
        .observe_turn(b.turn.as_ref().unwrap(), TurnState::Succeeded)
        .unwrap();
    assert_eq!(
        f.run(&a, review).unwrap().task.unwrap().status,
        Status::Completed
    );
}

#[test]
fn replay_conflict_and_restart_are_durable_without_duplicate_effects() {
    let mut f = Fixture::new();
    let command = Command::Delegate {
        delegation: Delegation {
            to_member_id: f.group.members[1].id.clone(),
            title: "One".into(),
            request: "Check once".into(),
            acceptance: vec![Criterion {
                id: "c".into(),
                text: "Pass".into(),
            }],
            dependencies: vec![],
            parent: None,
            continuation: None,
        },
    };
    let first = f
        .store
        .execute(&f.a, &f.group.id, "stable", command.clone())
        .unwrap();
    f.store = Store::open(f.dir.path().join("teams.sqlite3")).unwrap();
    let second = f
        .store
        .execute(&f.a, &f.group.id, "stable", command.clone())
        .unwrap();
    assert!(second.replayed);
    assert_eq!(first.task.unwrap().id, second.task.unwrap().id);
    assert_eq!(first.effect_ids, second.effect_ids);
    assert_eq!(f.store.effects(&f.group.id, 0, 100).unwrap().len(), 1);
    let mut changed = command;
    if let Command::Delegate { delegation } = &mut changed {
        delegation.request = "Different intent".into();
    }
    assert_eq!(
        f.store
            .execute(&f.a, &f.group.id, "stable", changed)
            .unwrap_err(),
        Error::IdempotencyConflict
    );
    let mut forged = f.a.clone();
    forged.turn.as_mut().unwrap().session.thread = "wrong-thread".into();
    assert_eq!(
        f.store.command(&forged, &f.group.id, "stable").unwrap_err(),
        Error::Forbidden
    );
}

#[test]
fn stale_revision_rejection_is_replayed_even_after_state_changes() {
    let mut f = Fixture::new();
    let (a, b) = (f.a.clone(), f.b.clone());
    let t = f.delegate(&a, 1, vec![], None);
    let mut bad = v(&t);
    bad.expected_revision = 0;
    let cmd = Command::Begin { task: bad };
    assert_eq!(
        f.store
            .execute(&b, &f.group.id, "rejected", cmd.clone())
            .unwrap_err(),
        Error::RevisionConflict
    );
    f.run(&b, Command::Begin { task: v(&t) }).unwrap();
    assert_eq!(
        f.store
            .execute(&b, &f.group.id, "rejected", cmd)
            .unwrap_err(),
        Error::RevisionConflict
    );
}

#[test]
fn child_completion_waits_for_reviewer_turn_before_parent_ready() {
    let mut f = Fixture::new();
    let (a, b) = (f.a.clone(), f.b.clone());
    let parent = f.delegate(&Caller::user(), 0, vec![], None);
    let parent = f
        .run(&a, Command::Begin { task: v(&parent) })
        .unwrap()
        .task
        .unwrap();
    let child = f.delegate(
        &a,
        1,
        vec![],
        Some(Parent {
            task_id: parent.id.clone(),
            expected_revision: parent.revision,
            attempt_id: parent.attempt_id.clone(),
        }),
    );
    assert_eq!(f.get(&parent.id).status, Status::Waiting);
    let child = f
        .run(&b, Command::Begin { task: v(&child) })
        .unwrap()
        .task
        .unwrap();
    let child = f
        .run(
            &b,
            Command::Submit {
                task: v(&child),
                submission: submission(),
            },
        )
        .unwrap()
        .task
        .unwrap();
    f.store
        .observe_turn(b.turn.as_ref().unwrap(), TurnState::Succeeded)
        .unwrap();
    f.run(
        &a,
        Command::Review {
            task: v(&child),
            result_id: child.results[0].id.clone(),
            accept: true,
            assessment: assessment(true),
            requested_changes: None,
        },
    )
    .unwrap();
    assert!(!f.get(&parent.id).ready);
    f.store
        .observe_turn(a.turn.as_ref().unwrap(), TurnState::Succeeded)
        .unwrap();
    let parent = f.get(&parent.id);
    assert!(parent.ready);
    let mut next = a.clone();
    next.turn.as_mut().unwrap().id = "next-turn".into();
    f.store
        .observe_turn(next.turn.as_ref().unwrap(), TurnState::Active)
        .unwrap();
    assert_eq!(
        f.run(&next, Command::Resume { task: v(&parent) })
            .unwrap()
            .task
            .unwrap()
            .status,
        Status::Working
    );
}

#[test]
fn cancelled_dependency_does_not_release_waiting_task() {
    let mut f = Fixture::new();
    let a = f.a.clone();
    let dep = f.delegate(&a, 1, vec![], None);
    let t = f.delegate(&a, 2, vec![dep.id.clone()], None);
    f.run(
        &a,
        Command::Cancel {
            task: v(&dep),
            reason: "No longer needed".into(),
        },
    )
    .unwrap();
    assert!(!f.get(&t.id).ready);
    let c = f.c.clone();
    assert_eq!(
        f.run(
            &c,
            Command::Resume {
                task: v(&f.get(&t.id))
            }
        )
        .unwrap_err(),
        Error::Blocked
    );
}

#[test]
fn dependency_cycles_and_stale_claims_are_rejected_atomically() {
    let mut f = Fixture::new();
    let a = f.a.clone();
    let parent = f.delegate(&Caller::user(), 0, vec![], None);
    let parent = f
        .run(&a, Command::Begin { task: v(&parent) })
        .unwrap()
        .task
        .unwrap();
    let bad = Command::Delegate {
        delegation: Delegation {
            to_member_id: f.group.members[1].id.clone(),
            title: "Cycle".into(),
            request: "Would wait on parent".into(),
            acceptance: parent.acceptance.clone(),
            dependencies: vec![parent.id.clone()],
            parent: Some(Parent {
                task_id: parent.id.clone(),
                expected_revision: parent.revision,
                attempt_id: parent.attempt_id.clone(),
            }),
            continuation: None,
        },
    };
    assert_eq!(f.run(&a, bad).unwrap_err(), Error::InvalidArgument);
    assert_eq!(f.store.tasks(&a, &f.group.id, 0, 100).unwrap().len(), 1);
    let mut wrong = a.clone();
    wrong.turn.as_mut().unwrap().id = "other-turn".into();
    f.store
        .observe_turn(wrong.turn.as_ref().unwrap(), TurnState::Active)
        .unwrap();
    assert_eq!(
        f.run(
            &wrong,
            Command::Checkpoint {
                task: v(&parent),
                continuation: checkpoint()
            }
        )
        .unwrap_err(),
        Error::ClaimConflict
    );
}

#[test]
fn failed_turn_cannot_be_reclaimed_or_relabelled_success_by_an_agent() {
    let mut f = Fixture::new();
    let (a, b) = (f.a.clone(), f.b.clone());
    let t = f.delegate(&a, 1, vec![], None);
    let t = f
        .run(&b, Command::Begin { task: v(&t) })
        .unwrap()
        .task
        .unwrap();
    f.store
        .observe_turn(b.turn.as_ref().unwrap(), TurnState::Failed)
        .unwrap();
    assert_eq!(
        f.store
            .observe_turn(b.turn.as_ref().unwrap(), TurnState::Succeeded)
            .unwrap_err(),
        Error::InvalidState
    );
    let mut next = b.clone();
    next.turn.as_mut().unwrap().id = "recovery".into();
    f.store
        .observe_turn(next.turn.as_ref().unwrap(), TurnState::Active)
        .unwrap();
    assert_eq!(
        f.run(
            &next,
            Command::Reclaim {
                task: v(&t),
                previous_turn: b.turn.unwrap(),
                continuation: checkpoint()
            }
        )
        .unwrap_err(),
        Error::ReconciliationRequired
    );
}

#[test]
fn needs_user_cannot_be_self_approved_and_pause_preserves_receipts() {
    let mut f = Fixture::new();
    let (a, b) = (f.a.clone(), f.b.clone());
    let t = f.delegate(&a, 1, vec![], None);
    let t = f
        .run(&b, Command::Begin { task: v(&t) })
        .unwrap()
        .task
        .unwrap();
    let r = f
        .run(
            &b,
            Command::Wait {
                task: v(&t),
                needs_user: true,
                detail: "Need a user decision".into(),
                continuation: checkpoint(),
            },
        )
        .unwrap();
    let blocker = r.entity_id.unwrap();
    let t = r.task.unwrap();
    assert_eq!(
        f.run(
            &b,
            Command::Resolve {
                task: v(&t),
                blocker_id: blocker.clone(),
                resolution: "I approve".into()
            }
        )
        .unwrap_err(),
        Error::Forbidden
    );
    let t = f
        .run(
            &Caller::user(),
            Command::Resolve {
                task: v(&t),
                blocker_id: blocker,
                resolution: "Approved fixture".into(),
            },
        )
        .unwrap()
        .task
        .unwrap();
    let t = f
        .run(&b, Command::Resume { task: v(&t) })
        .unwrap()
        .task
        .unwrap();
    f.run(
        &Caller::user(),
        Command::SetPaused {
            expected_revision: 1,
            paused: true,
        },
    )
    .unwrap();
    let submitted = f
        .run(
            &b,
            Command::Submit {
                task: v(&t),
                submission: submission(),
            },
        )
        .unwrap();
    assert_eq!(submitted.task.unwrap().status, Status::Submitted);
    for (_, e) in f.store.effects(&f.group.id, 0, 100).unwrap() {
        assert!(!f.store.effect_ready(&e).unwrap());
    }
}

#[test]
fn rework_changes_attempt_and_keeps_previous_result_immutable() {
    let mut f = Fixture::new();
    let b = f.b.clone();
    let t = f.delegate(&Caller::user(), 1, vec![], None);
    let t = f
        .run(&b, Command::Begin { task: v(&t) })
        .unwrap()
        .task
        .unwrap();
    let t = f
        .run(
            &b,
            Command::Submit {
                task: v(&t),
                submission: submission(),
            },
        )
        .unwrap()
        .task
        .unwrap();
    f.store
        .observe_turn(b.turn.as_ref().unwrap(), TurnState::Succeeded)
        .unwrap();
    let old_attempt = t.attempt_id.clone();
    let result_id = t.results[0].id.clone();
    let revised = f
        .run(
            &Caller::user(),
            Command::Review {
                task: v(&t),
                result_id: result_id.clone(),
                accept: false,
                assessment: assessment(false),
                requested_changes: Some("Recheck original criterion".into()),
            },
        )
        .unwrap()
        .task
        .unwrap();
    assert_ne!(revised.attempt_id, old_attempt);
    assert_eq!(revised.results[0].id, result_id);
    assert_eq!(revised.status, Status::Pending);
    let mut next = b;
    next.turn.as_mut().unwrap().id = "rework-turn".into();
    f.store
        .observe_turn(next.turn.as_ref().unwrap(), TurnState::Active)
        .unwrap();
    let mut stale = v(&revised);
    stale.attempt_id = old_attempt;
    assert_eq!(
        f.run(&next, Command::Begin { task: stale }).unwrap_err(),
        Error::StaleAttempt
    );
}

#[test]
fn failed_persistence_rolls_back_task_effect_and_command_together() {
    let mut f = Fixture::new();
    let db = rusqlite::Connection::open(f.dir.path().join("teams.sqlite3")).unwrap();
    db.execute_batch("CREATE TRIGGER fail_effect BEFORE INSERT ON effects BEGIN SELECT RAISE(ABORT,'fixture disk failure'); END;").unwrap();
    let cmd = Command::Delegate {
        delegation: Delegation {
            to_member_id: f.group.members[1].id.clone(),
            title: "Atomic".into(),
            request: "No partial state".into(),
            acceptance: vec![Criterion {
                id: "check".into(),
                text: "Pass".into(),
            }],
            dependencies: vec![],
            parent: None,
            continuation: None,
        },
    };
    assert!(matches!(
        f.store.execute(&f.a, &f.group.id, "disk", cmd.clone()),
        Err(Error::Storage(_))
    ));
    assert!(f.store.tasks(&f.a, &f.group.id, 0, 100).unwrap().is_empty());
    assert!(f
        .store
        .command(&f.a, &f.group.id, "disk")
        .unwrap()
        .is_none());
    db.execute_batch("DROP TRIGGER fail_effect").unwrap();
    assert!(f.store.execute(&f.a, &f.group.id, "disk", cmd).is_ok());
}

#[test]
fn groups_have_five_member_limit_unique_sessions_and_optional_member_owner() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = Store::open(dir.path().join("teams.sqlite3")).unwrap();
    let drafts: Vec<_> = (0..6)
        .map(|i| MemberDraft {
            session: Session {
                backend: "codex".into(),
                thread: i.to_string(),
            },
            name: "Member".into(),
            responsibility: "Work".into(),
        })
        .collect();
    assert_eq!(
        store
            .create_group(&Caller::user(), "six", "Too many", drafts.clone())
            .unwrap_err(),
        Error::LimitExceeded
    );
    store.exclude_controller(&drafts[5].session).unwrap();
    assert_eq!(
        store
            .create_group(
                &Caller::user(),
                "router",
                "Router",
                vec![drafts[0].clone(), drafts[5].clone()]
            )
            .unwrap_err(),
        Error::InvalidState
    );
    let g = store
        .create_group(&Caller::user(), "five", "Five", drafts[..5].to_vec())
        .unwrap();
    assert!(g.orchestrator.is_none());
    assert_eq!(
        store
            .create_group(
                &Caller::user(),
                "duplicate",
                "Overlap",
                drafts[..2].to_vec()
            )
            .unwrap_err(),
        Error::InvalidState
    );
    let r = store
        .execute(
            &Caller::user(),
            &g.id,
            "owner",
            Command::SetOrchestrator {
                expected_revision: 1,
                member_id: Some(g.members[0].id.clone()),
            },
        )
        .unwrap();
    assert_eq!(r.group.unwrap().members.len(), 5);
}

#[test]
fn reviewer_can_explicitly_resume_parent_in_its_same_turn() {
    let mut f = Fixture::new();
    let (a, b) = (f.a.clone(), f.b.clone());
    let p = f.delegate(&Caller::user(), 0, vec![], None);
    let p = f
        .run(&a, Command::Begin { task: v(&p) })
        .unwrap()
        .task
        .unwrap();
    let child = f.delegate(
        &a,
        1,
        vec![],
        Some(Parent {
            task_id: p.id.clone(),
            expected_revision: p.revision,
            attempt_id: p.attempt_id.clone(),
        }),
    );
    let child = f
        .run(&b, Command::Begin { task: v(&child) })
        .unwrap()
        .task
        .unwrap();
    let child = f
        .run(
            &b,
            Command::Submit {
                task: v(&child),
                submission: submission(),
            },
        )
        .unwrap()
        .task
        .unwrap();
    f.store
        .observe_turn(b.turn.as_ref().unwrap(), TurnState::Succeeded)
        .unwrap();
    f.run(
        &a,
        Command::Review {
            task: v(&child),
            result_id: child.results[0].id.clone(),
            accept: true,
            assessment: assessment(true),
            requested_changes: None,
        },
    )
    .unwrap();
    let p = f.get(&p.id);
    assert!(!p.ready);
    let p = f
        .run(&a, Command::Resume { task: v(&p) })
        .unwrap()
        .task
        .unwrap();
    let p = f
        .run(
            &a,
            Command::Submit {
                task: v(&p),
                submission: submission(),
            },
        )
        .unwrap()
        .task
        .unwrap();
    assert_eq!(
        f.run(
            &Caller::user(),
            Command::Review {
                task: v(&p),
                result_id: p.results[0].id.clone(),
                accept: true,
                assessment: assessment(true),
                requested_changes: None
            }
        )
        .unwrap_err(),
        Error::ResultNotReady
    );
}

#[test]
fn answered_question_is_withdrawn_without_removing_other_question_notifications() {
    let mut f = Fixture::new();
    let (a, b) = (f.a.clone(), f.b.clone());
    let t = f.delegate(&a, 1, vec![], None);
    let mut t = f
        .run(&b, Command::Begin { task: v(&t) })
        .unwrap()
        .task
        .unwrap();
    let q = f
        .run(
            &b,
            Command::Ask {
                task: v(&t),
                to: a.actor.clone(),
                question: "First?".into(),
                blocking: false,
                continuation: None,
            },
        )
        .unwrap();
    let q1 = q.entity_id.unwrap();
    t = q.task.unwrap();
    let q = f
        .run(
            &b,
            Command::Ask {
                task: v(&t),
                to: a.actor.clone(),
                question: "Second?".into(),
                blocking: false,
                continuation: None,
            },
        )
        .unwrap();
    let q2 = q.entity_id.unwrap();
    t = q.task.unwrap();
    f.run(
        &a,
        Command::Answer {
            task: v(&t),
            question_id: q1.clone(),
            answer: "First response".into(),
        },
    )
    .unwrap();
    let effects = f.store.effects(&f.group.id, 0, 100).unwrap();
    let first = &effects
        .iter()
        .find(|(_, e)| e.entity_id.as_ref() == Some(&q1))
        .unwrap()
        .1;
    let second = &effects
        .iter()
        .find(|(_, e)| e.entity_id.as_ref() == Some(&q2))
        .unwrap()
        .1;
    assert!(!f.store.effect_ready(first).unwrap());
    assert!(f.store.effect_ready(second).unwrap());
}

#[test]
fn paused_group_accepts_question_from_existing_claim_but_holds_delivery() {
    let mut f = Fixture::new();
    let (a, b) = (f.a.clone(), f.b.clone());
    let t = f.delegate(&a, 1, vec![], None);
    let t = f
        .run(&b, Command::Begin { task: v(&t) })
        .unwrap()
        .task
        .unwrap();
    f.run(
        &Caller::user(),
        Command::SetPaused {
            expected_revision: 1,
            paused: true,
        },
    )
    .unwrap();
    let r = f
        .run(
            &b,
            Command::Ask {
                task: v(&t),
                to: a.actor.clone(),
                question: "Clarify current work".into(),
                blocking: true,
                continuation: Some(checkpoint()),
            },
        )
        .unwrap();
    let effects = f.store.effects(&f.group.id, 0, 100).unwrap();
    assert!(!f
        .store
        .effect_ready(
            &effects
                .iter()
                .find(|(_, e)| e.id == r.effect_ids[0])
                .unwrap()
                .1
        )
        .unwrap());
}

#[test]
fn new_active_turn_invalidates_old_turn_and_restart_requires_reconciliation() {
    let mut f = Fixture::new();
    let (a, b) = (f.a.clone(), f.b.clone());
    let t = f.delegate(&a, 1, vec![], None);
    let t = f
        .run(&b, Command::Begin { task: v(&t) })
        .unwrap()
        .task
        .unwrap();
    let mut next = b.clone();
    next.turn.as_mut().unwrap().id = "newer-turn".into();
    f.store
        .observe_turn(next.turn.as_ref().unwrap(), TurnState::Active)
        .unwrap();
    assert_eq!(
        f.run(
            &b,
            Command::Submit {
                task: v(&t),
                submission: submission()
            }
        )
        .unwrap_err(),
        Error::TurnNotActive
    );
    assert_eq!(
        f.store
            .observe_turn(b.turn.as_ref().unwrap(), TurnState::Active)
            .unwrap_err(),
        Error::ReconciliationRequired
    );
    f.store.recover_turns().unwrap();
    assert_eq!(
        f.store
            .observe_turn(next.turn.as_ref().unwrap(), TurnState::Active)
            .unwrap_err(),
        Error::ReconciliationRequired
    );
}

#[test]
fn deterministic_group_rejection_cannot_be_reused_for_a_different_intent() {
    let mut f = Fixture::new();
    let drafts: Vec<_> = f.group.members[..2]
        .iter()
        .map(|m| MemberDraft {
            session: m.session.clone(),
            name: m.name.clone(),
            responsibility: m.responsibility.clone(),
        })
        .collect();
    assert_eq!(
        f.store
            .create_group(&Caller::user(), "collision", "Overlap", drafts.clone())
            .unwrap_err(),
        Error::InvalidState
    );
    let changed: Vec<_> = drafts
        .into_iter()
        .map(|mut m| {
            m.session.thread.push_str("-different");
            m
        })
        .collect();
    assert_eq!(
        f.store
            .create_group(&Caller::user(), "collision", "New", changed)
            .unwrap_err(),
        Error::IdempotencyConflict
    );
}

#[test]
fn audit_growth_is_linear_and_replay_receipts_do_not_copy_task_history() {
    let mut f = Fixture::new();
    let (a, b) = (f.a.clone(), f.b.clone());
    let t = f.delegate(&a, 1, vec![], None);
    let mut t = f
        .run(&b, Command::Begin { task: v(&t) })
        .unwrap()
        .task
        .unwrap();
    for i in 0..40 {
        let r = f
            .run(
                &b,
                Command::Ask {
                    task: v(&t),
                    to: a.actor.clone(),
                    question: format!("{i}:{}", "q".repeat(2000)),
                    blocking: false,
                    continuation: None,
                },
            )
            .unwrap();
        t = r.task.unwrap();
        t = f
            .run(
                &a,
                Command::Answer {
                    task: v(&t),
                    question_id: r.entity_id.unwrap(),
                    answer: "a".repeat(2000),
                },
            )
            .unwrap()
            .task
            .unwrap();
    }
    let db = rusqlite::Connection::open(f.dir.path().join("teams.sqlite3")).unwrap();
    let receipt_max: i64 = db
        .query_row(
            "SELECT MAX(length(response)) FROM commands WHERE scope=?",
            [&f.group.id],
            |r| r.get(0),
        )
        .unwrap();
    assert!(receipt_max < 2048, "receipt bytes: {receipt_max}");
    let audit_bytes: i64 = db
        .query_row("SELECT SUM(length(data)) FROM events", [], |r| r.get(0))
        .unwrap();
    assert!(audit_bytes < 400_000, "audit bytes: {audit_bytes}");
    db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)").unwrap();
    let size = std::fs::metadata(f.dir.path().join("teams.sqlite3"))
        .unwrap()
        .len();
    assert!(size < 3_000_000, "database bytes: {size}");
}

#[test]
fn same_turn_exception_never_skips_self_rework_or_unrelated_dependency_barriers() {
    let mut f = Fixture::new();
    let a = f.a.clone();
    let t = f.delegate(&a, 0, vec![], None);
    let t = f
        .run(&a, Command::Begin { task: v(&t) })
        .unwrap()
        .task
        .unwrap();
    let t = f
        .run(
            &a,
            Command::Submit {
                task: v(&t),
                submission: submission(),
            },
        )
        .unwrap()
        .task
        .unwrap();
    f.store
        .observe_turn(a.turn.as_ref().unwrap(), TurnState::Succeeded)
        .unwrap();
    let mut reviewer = a.clone();
    reviewer.turn.as_mut().unwrap().id = "review-turn".into();
    f.store
        .observe_turn(reviewer.turn.as_ref().unwrap(), TurnState::Active)
        .unwrap();
    let t = f
        .run(
            &reviewer,
            Command::Review {
                task: v(&t),
                result_id: t.results[0].id.clone(),
                accept: false,
                assessment: assessment(false),
                requested_changes: Some("Fix original check".into()),
            },
        )
        .unwrap()
        .task
        .unwrap();
    assert_eq!(
        f.run(&reviewer, Command::Begin { task: v(&t) })
            .unwrap_err(),
        Error::Blocked
    );
    let b = f.b.clone();
    let dep = f.delegate(&reviewer, 1, vec![], None);
    let waiting = f.delegate(&reviewer, 0, vec![dep.id.clone()], None);
    let dep = f
        .run(&b, Command::Begin { task: v(&dep) })
        .unwrap()
        .task
        .unwrap();
    let dep = f
        .run(
            &b,
            Command::Submit {
                task: v(&dep),
                submission: submission(),
            },
        )
        .unwrap()
        .task
        .unwrap();
    f.store
        .observe_turn(b.turn.as_ref().unwrap(), TurnState::Succeeded)
        .unwrap();
    f.run(
        &reviewer,
        Command::Review {
            task: v(&dep),
            result_id: dep.results[0].id.clone(),
            accept: true,
            assessment: assessment(true),
            requested_changes: None,
        },
    )
    .unwrap();
    assert_eq!(
        f.run(
            &reviewer,
            Command::Resume {
                task: v(&f.get(&waiting.id))
            }
        )
        .unwrap_err(),
        Error::Blocked
    );
}

#[test]
fn superseded_questions_do_not_reserve_future_answer_capacity() {
    let mut f = Fixture::new();
    let b = f.b.clone();
    let mut t = f.delegate(&Caller::user(), 1, vec![], None);
    t = f
        .run(&b, Command::Begin { task: v(&t) })
        .unwrap()
        .task
        .unwrap();
    for _ in 0..7 {
        t = f
            .run(
                &b,
                Command::Ask {
                    task: v(&t),
                    to: Actor::User,
                    question: "Nonblocking question".into(),
                    blocking: false,
                    continuation: None,
                },
            )
            .unwrap()
            .task
            .unwrap();
    }
    t = f
        .run(
            &b,
            Command::Submit {
                task: v(&t),
                submission: submission(),
            },
        )
        .unwrap()
        .task
        .unwrap();
    f.store
        .observe_turn(b.turn.as_ref().unwrap(), TurnState::Succeeded)
        .unwrap();
    t = f
        .run(
            &Caller::user(),
            Command::Review {
                task: v(&t),
                result_id: t.results[0].id.clone(),
                accept: false,
                assessment: assessment(false),
                requested_changes: Some("Recheck".into()),
            },
        )
        .unwrap()
        .task
        .unwrap();
    let mut next = b;
    next.turn.as_mut().unwrap().id = "new-attempt".into();
    f.store
        .observe_turn(next.turn.as_ref().unwrap(), TurnState::Active)
        .unwrap();
    t = f
        .run(&next, Command::Begin { task: v(&t) })
        .unwrap()
        .task
        .unwrap();
    assert!(f
        .run(
            &next,
            Command::Ask {
                task: v(&t),
                to: Actor::User,
                question: "Current question".into(),
                blocking: false,
                continuation: None
            }
        )
        .is_ok());
}

#[test]
fn user_can_release_a_failed_submission_for_review_without_promoting_its_turn() {
    let mut f = Fixture::new();
    let a = f.a.clone();
    let b = f.b.clone();
    let user = Caller::user();
    let t = f.delegate(&a, 1, vec![], None);
    let t = f
        .run(&b, Command::Begin { task: v(&t) })
        .unwrap()
        .task
        .unwrap();
    let t = f
        .run(
            &b,
            Command::Submit {
                task: v(&t),
                submission: submission(),
            },
        )
        .unwrap()
        .task
        .unwrap();
    let turn = b.turn.as_ref().unwrap();
    f.store.observe_turn(turn, TurnState::Failed).unwrap();
    let id = t.results[0].id.clone();
    let cmd = Command::ReconcileResult {
        task: v(&t),
        result_id: id.clone(),
        allow_review: true,
        evidence: vec![Evidence {
            kind: EvidenceKind::Note,
            locator: "fixture".into(),
            description: "User inspected completed files".into(),
        }],
    };
    assert_eq!(f.run(&a, cmd.clone()).unwrap_err(), Error::Forbidden);
    let t = f.run(&user, cmd).unwrap().task.unwrap();
    let accepted = f
        .run(
            &a,
            Command::Review {
                task: v(&t),
                result_id: id,
                accept: true,
                assessment: assessment(true),
                requested_changes: None,
            },
        )
        .unwrap()
        .task
        .unwrap();
    assert_eq!(accepted.status, Status::Completed);
    assert_eq!(f.store.observed_turn(turn).unwrap(), TurnState::Failed);
}

#[test]
fn roster_edits_are_atomic_and_archive_releases_only_unused_member_bindings() {
    let mut f = Fixture::new();
    let g = f.group.clone();
    let user = Caller::user();
    let remove = Command::ChangeMembers {
        expected_revision: g.revision,
        add: vec![],
        remove_member_ids: vec![g.members[2].id.clone()],
    };
    assert_eq!(
        f.store
            .execute(&f.a, &g.id, "remove", remove.clone())
            .unwrap_err(),
        Error::Forbidden
    );
    let g = f
        .store
        .execute(&user, &g.id, "remove", remove)
        .unwrap()
        .group
        .unwrap();
    assert_eq!(g.members.len(), 2);
    assert!(f
        .store
        .member_for_session(&f.c.turn.as_ref().unwrap().session)
        .unwrap()
        .is_none());
    let cmd = Command::ArchiveGroup {
        expected_revision: g.revision,
    };
    let archived = f
        .store
        .execute(&user, &g.id, "archive", cmd.clone())
        .unwrap()
        .group
        .unwrap();
    assert!(archived.archived && archived.paused);
    assert!(
        f.store
            .execute(&user, &g.id, "archive", cmd)
            .unwrap()
            .replayed
    );
    for m in g.members {
        assert!(f.store.member_for_session(&m.session).unwrap().is_none());
    }
}

#[test]
fn recovered_manual_submission_uses_its_native_terminal_receipt() {
    let mut f = Fixture::new();
    let b = f.b.clone();
    let user = Caller::user();
    let t = f.delegate(&user, 1, vec![], None);
    let t = f
        .run(&b, Command::Begin { task: v(&t) })
        .unwrap()
        .task
        .unwrap();
    let t = f
        .run(
            &b,
            Command::Submit {
                task: v(&t),
                submission: submission(),
            },
        )
        .unwrap()
        .task
        .unwrap();
    let turn = b.turn.unwrap();
    f.store
        .recover_backend_turns(&turn.session.backend)
        .unwrap();
    let cmd = Command::ReconcileResult {
        task: v(&t),
        result_id: t.results[0].id.clone(),
        allow_review: true,
        evidence: vec![Evidence {
            kind: EvidenceKind::Note,
            locator: "receipt".into(),
            description: "Checked the native turn".into(),
        }],
    };
    assert_eq!(f.run(&user, cmd.clone()).unwrap_err(), Error::InvalidState);
    f.store
        .reconcile_native_terminal(&turn.session, &turn.id, TurnState::Succeeded)
        .unwrap();
    let refreshed = f.run(&user, cmd).unwrap().task.unwrap();
    assert_eq!(f.store.observed_turn(&turn).unwrap(), TurnState::Succeeded);
    assert!(refreshed.reconciled_result_id.is_none());
    assert_eq!(refreshed.status, Status::Submitted);
    let complete = f
        .run(
            &user,
            Command::Review {
                task: v(&refreshed),
                result_id: t.results[0].id.clone(),
                accept: true,
                assessment: assessment(true),
                requested_changes: None,
            },
        )
        .unwrap()
        .task
        .unwrap();
    assert_eq!(complete.status, Status::Completed);
}

#[test]
fn delete_group_requires_user_and_revision_and_replays_after_reopen() {
    let mut f = Fixture::new();
    let g = f.group.clone();
    assert!(matches!(
        f.store.delete_group(&f.a, &g.id, "delete", g.revision),
        Err(Error::Forbidden)
    ));
    assert!(matches!(
        f.store
            .delete_group(&Caller::user(), &g.id, "delete", g.revision + 1),
        Err(Error::RevisionConflict)
    ));
    f.store
        .delete_group(&Caller::user(), &g.id, "delete", g.revision)
        .unwrap();
    assert!(f.store.groups().unwrap().is_empty());
    assert!(f
        .store
        .member_for_session(&g.members[0].session)
        .unwrap()
        .is_none());
    assert!(matches!(
        f.store.context(&Caller::user(), &g.id),
        Err(Error::NotFound)
    ));
    let mut reopened = Store::open(f.dir.path().join("teams.sqlite3")).unwrap();
    reopened
        .delete_group(&Caller::user(), &g.id, "delete", g.revision)
        .unwrap();
    assert!(reopened
        .delete_group(&Caller::user(), &g.id, "delete", g.revision + 1)
        .is_err());
    let members = g
        .members
        .iter()
        .map(|m| MemberDraft {
            session: m.session.clone(),
            name: m.name.clone(),
            responsibility: m.responsibility.clone(),
        })
        .collect();
    reopened
        .create_group(&Caller::user(), "replacement", "Replacement", members)
        .unwrap();
}

#[test]
fn delete_group_rejects_live_work_and_removes_cancelled_history() {
    let mut f = Fixture::new();
    let task = f.delegate(&Caller::user(), 0, vec![], None);
    assert!(matches!(
        f.store
            .delete_group(&Caller::user(), &f.group.id, "delete", f.group.revision),
        Err(Error::Blocked)
    ));
    f.run(
        &Caller::user(),
        Command::Cancel {
            task: TaskVersion {
                task_id: task.id.clone(),
                expected_revision: task.revision,
                attempt_id: task.attempt_id,
            },
            reason: "No longer needed".into(),
        },
    )
    .unwrap();
    f.store
        .delete_group(&Caller::user(), &f.group.id, "delete", f.group.revision)
        .unwrap();
    assert!(matches!(
        f.store.task(&Caller::user(), &f.group.id, &task.id),
        Err(Error::NotFound)
    ));
    assert!(f.store.groups().unwrap().is_empty());
}

#[test]
fn deleting_a_cancelled_task_group_still_waits_for_the_native_claim() {
    let mut f = Fixture::new();
    let task = f.delegate(&Caller::user(), 0, vec![], None);
    let actor = f.a.clone();
    let working = f
        .run(&actor, Command::Begin { task: v(&task) })
        .unwrap()
        .task
        .unwrap();
    f.run(
        &Caller::user(),
        Command::Cancel {
            task: v(&working),
            reason: "Stop the collaboration".into(),
        },
    )
    .unwrap();
    assert!(matches!(
        f.store
            .delete_group(&Caller::user(), &f.group.id, "delete", f.group.revision),
        Err(Error::Blocked)
    ));
    f.store
        .observe_turn(actor.turn.as_ref().unwrap(), TurnState::Unknown)
        .unwrap();
    assert!(matches!(
        f.store
            .delete_group(&Caller::user(), &f.group.id, "delete", f.group.revision),
        Err(Error::Blocked)
    ));
    f.store
        .observe_turn(actor.turn.as_ref().unwrap(), TurnState::Succeeded)
        .unwrap();
    f.store
        .delete_group(&Caller::user(), &f.group.id, "delete", f.group.revision)
        .unwrap();
}

#[test]
fn empty_group_accepts_one_member_at_a_time_with_a_single_atomic_owner() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = Store::open(dir.path().join("teams.sqlite3")).unwrap();
    let user = Caller::user();
    let mut group = store
        .create_group(&user, "create-empty", "New group", vec![])
        .unwrap();
    assert!(group.paused && group.members.is_empty() && group.orchestrator.is_none());
    let draft = |n: usize| MemberDraft {
        session: Session {
            backend: "codex".into(),
            thread: format!("thread-{n}"),
        },
        name: format!("Member {n}"),
        responsibility: "Development".into(),
    };
    let add = Command::AddMember {
        expected_revision: group.revision,
        member: draft(0),
        make_orchestrator: true,
    };
    group = store
        .execute(&user, &group.id, "add-first", add.clone())
        .unwrap()
        .group
        .unwrap();
    assert_eq!(group.members.len(), 1);
    assert_eq!(
        group.orchestrator.as_deref(),
        Some(group.members[0].id.as_str())
    );
    assert!(
        store
            .execute(&user, &group.id, "add-first", add)
            .unwrap()
            .replayed
    );
    assert!(matches!(
        store.execute(
            &user,
            &group.id,
            "resume-one",
            Command::SetPaused {
                expected_revision: group.revision,
                paused: false
            }
        ),
        Err(Error::InvalidState)
    ));
    assert!(matches!(
        store.execute(
            &user,
            &group.id,
            "second-owner",
            Command::AddMember {
                expected_revision: group.revision,
                member: draft(1),
                make_orchestrator: true
            }
        ),
        Err(Error::InvalidState)
    ));
    assert!(store
        .member_for_session(&draft(1).session)
        .unwrap()
        .is_none());
    let after_rejection = store.context(&user, &group.id).unwrap();
    assert_eq!(after_rejection.revision, group.revision);
    for n in 1..5 {
        group = store
            .execute(
                &user,
                &group.id,
                &format!("add-{n}"),
                Command::AddMember {
                    expected_revision: group.revision,
                    member: draft(n),
                    make_orchestrator: false,
                },
            )
            .unwrap()
            .group
            .unwrap();
    }
    assert_eq!(group.members.len(), 5);
    assert_eq!(
        group.orchestrator.as_deref(),
        Some(group.members[0].id.as_str())
    );
    assert!(matches!(
        store.execute(
            &user,
            &group.id,
            "sixth",
            Command::AddMember {
                expected_revision: group.revision,
                member: draft(5),
                make_orchestrator: false
            }
        ),
        Err(Error::LimitExceeded)
    ));
    assert!(matches!(
        store.execute(
            &user,
            &group.id,
            "stale-owner",
            Command::SetOrchestrator {
                expected_revision: group.revision - 1,
                member_id: Some(group.members[1].id.clone())
            }
        ),
        Err(Error::RevisionConflict)
    ));
    group = store
        .execute(
            &user,
            &group.id,
            "change-owner",
            Command::SetOrchestrator {
                expected_revision: group.revision,
                member_id: Some(group.members[1].id.clone()),
            },
        )
        .unwrap()
        .group
        .unwrap();
    assert_eq!(
        group.orchestrator.as_deref(),
        Some(group.members[1].id.as_str())
    );
    group = store
        .execute(
            &user,
            &group.id,
            "remove-all",
            Command::ChangeMembers {
                expected_revision: group.revision,
                add: vec![],
                remove_member_ids: group.members.iter().map(|m| m.id.clone()).collect(),
            },
        )
        .unwrap()
        .group
        .unwrap();
    assert!(group.paused && group.members.is_empty() && group.orchestrator.is_none());
    store
        .delete_group(&user, &group.id, "delete-empty", group.revision)
        .unwrap();
}
