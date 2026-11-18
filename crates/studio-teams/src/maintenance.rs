use super::*;

fn unfinished_member_reference(
    db: &Connection,
    g: &Group,
    removed: &[String],
    archive: bool,
) -> Result<bool> {
    let mut q = db.prepare("SELECT body FROM tasks WHERE group_id=?")?;
    let rows = q.query_map([&g.id], |r| r.get::<_, String>(0))?;
    let includes = |a: &Actor| matches!(a,Actor::Member(m) if removed.contains(m));
    for row in rows {
        let t: Task = decode(row?)?;
        if !t.status.terminal()
            && (archive
                || removed.contains(&t.assignee)
                || includes(&t.requester)
                || t.questions.iter().any(|q| {
                    q.attempt_id == t.attempt_id
                        && q.answer.is_none()
                        && (includes(&q.to) || includes(&q.asked_by))
                }))
        {
            return Ok(true);
        }
        if removed.contains(&t.assignee) || archive {
            if let Some(claim) = &t.claim {
                if matches!(
                    turn_state(db, claim)?,
                    TurnState::Active | TurnState::Unknown
                ) {
                    return Ok(true);
                }
            }
        }
        for review in &t.reviews {
            if let Some(turn) = &review.turn {
                let removing_reviewer = g
                    .members
                    .iter()
                    .any(|m| m.session == turn.session && removed.contains(&m.id));
                if (archive || removing_reviewer)
                    && t.barriers.contains(turn)
                    && turn_state(db, turn)? != TurnState::Succeeded
                {
                    return Ok(true);
                }
            }
        }
    }
    Ok(false)
}
/// The host must fence/resolve independent native queue receipts before member removal/archival.
pub(super) fn manage_group(
    tx: &Transaction<'_>,
    g: &mut Group,
    c: &Caller,
    cmd: &Command,
) -> Result<Option<Reply>> {
    let expected = match cmd {
        Command::SetGroupProfile {
            expected_revision, ..
        }
        | Command::ChangeMembers {
            expected_revision, ..
        }
        | Command::ArchiveGroup { expected_revision } => *expected_revision,
        _ => return Ok(None),
    };
    if c.actor != Actor::User {
        return Err(Error::Forbidden);
    }
    if expected != g.revision {
        return Err(Error::RevisionConflict);
    }
    match cmd {
        Command::SetGroupProfile { name, members, .. } => {
            if name.is_none() && members.is_empty() {
                return Err(Error::InvalidArgument);
            }
            if !distinct(members.iter().map(|m| m.member_id.as_str())) {
                return Err(Error::InvalidArgument);
            }
            if let Some(name) = name {
                text(name, 800)?;
                g.name = name.clone();
            }
            for p in members {
                text(&p.display_name, 800)?;
                text(&p.responsibility, 2048)?;
                let m = g
                    .members
                    .iter_mut()
                    .find(|m| m.id == p.member_id)
                    .ok_or(Error::NotFound)?;
                m.name.clone_from(&p.display_name);
                m.responsibility.clone_from(&p.responsibility);
            }
        }
        Command::ChangeMembers {
            add,
            remove_member_ids,
            ..
        } => {
            if add.is_empty() && remove_member_ids.is_empty() {
                return Err(Error::InvalidArgument);
            }
            if !distinct(remove_member_ids.iter().map(String::as_str))
                || remove_member_ids
                    .iter()
                    .any(|id| !g.members.iter().any(|m| &m.id == id))
            {
                return Err(Error::InvalidArgument);
            }
            if !(1..=5).contains(&(g.members.len() - remove_member_ids.len() + add.len())) {
                return Err(Error::LimitExceeded);
            }
            if unfinished_member_reference(tx, g, remove_member_ids, false)? {
                return Err(Error::Blocked);
            }
            for draft in add {
                identifier(&draft.session.backend)?;
                identifier(&draft.session.thread)?;
                text(&draft.name, 800)?;
                text(&draft.responsibility, 2048)?;
                let used:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM members WHERE backend=?1 AND thread=?2 UNION SELECT 1 FROM controllers WHERE backend=?1 AND thread=?2)",params![draft.session.backend,draft.session.thread],|r|r.get(0))?;
                if used {
                    return Err(Error::InvalidState);
                }
                let m = Member {
                    id: id(),
                    session: draft.session.clone(),
                    name: draft.name.clone(),
                    responsibility: draft.responsibility.clone(),
                };
                tx.execute(
                    "INSERT INTO members VALUES(?,?,?,?)",
                    params![m.id, g.id, m.session.backend, m.session.thread],
                )?;
                g.members.push(m);
            }
            for member_id in remove_member_ids {
                tx.execute(
                    "DELETE FROM members WHERE group_id=? AND id=?",
                    params![g.id, member_id],
                )?;
            }
            g.members.retain(|m| !remove_member_ids.contains(&m.id));
            if g.orchestrator
                .as_ref()
                .is_some_and(|id| remove_member_ids.contains(id))
            {
                g.orchestrator = None;
            }
            if g.members.len() < 2 {
                g.paused = true;
            }
        }
        Command::ArchiveGroup { .. } => {
            if unfinished_member_reference(tx, g, &[], true)? {
                return Err(Error::Blocked);
            }
            tx.execute("DELETE FROM members WHERE group_id=?", [&g.id])?;
            g.archived = true;
            g.paused = true;
        }
        _ => unreachable!(),
    }
    g.revision += 1;
    g.roster_version += 1;
    save_group(tx, g)?;
    event(tx, &g.id, &c.actor, "roster_changed", g)?;
    Ok(Some(Reply {
        task: None,
        group: Some(g.clone()),
        entity_id: None,
        effect_ids: vec![],
        replayed: false,
    }))
}
fn release_effect_barrier(
    tx: &Transaction<'_>,
    t: &Task,
    turn: &Turn,
    purpose: Option<&str>,
) -> Result<()> {
    let mut q = tx.prepare("SELECT body FROM effects WHERE task_id=? AND withdrawn=0")?;
    let rows = q
        .query_map([&t.id], |r| r.get::<_, String>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    for row in rows {
        let mut e: Effect = decode(row)?;
        if e.attempt_id != t.attempt_id
            || purpose.is_some_and(|p| p != e.purpose)
            || !e.barriers.contains(turn)
        {
            continue;
        }
        e.barriers.retain(|b| b != turn);
        tx.execute(
            "UPDATE effects SET body=? WHERE id=?",
            params![json(&e)?, e.id],
        )?;
    }
    Ok(())
}
fn record_evidence(values: &[Evidence]) -> Result<()> {
    if values.is_empty() {
        return Err(Error::InvalidArgument);
    }
    evidence(values)
}
pub(super) fn maintain_task(
    tx: &Transaction<'_>,
    t: &mut Task,
    c: &Caller,
    cmd: &Command,
) -> Result<Option<Reply>> {
    if !matches!(
        cmd,
        Command::GrantReworkBudget { .. }
            | Command::ReconcileExecution { .. }
            | Command::ReconcileResult { .. }
            | Command::ReconcileReview { .. }
    ) {
        return Ok(None);
    }
    if c.actor != Actor::User {
        return Err(Error::Forbidden);
    }
    let mut effects = vec![];
    match cmd {
        Command::GrantReworkBudget {
            additional_attempts,
            reason,
            ..
        } => {
            if !(1..=3).contains(additional_attempts) {
                return Err(Error::LimitExceeded);
            }
            text(reason, 32768)?;
            t.extra_attempts = t
                .extra_attempts
                .checked_add(*additional_attempts)
                .ok_or(Error::LimitExceeded)?;
        }
        Command::ReconcileExecution {
            claim_id,
            confirmed_settled,
            evidence,
            ..
        } => {
            record_evidence(evidence)?;
            let claim = t.claim.as_ref().ok_or(Error::ClaimConflict)?;
            if !matches!(t.status, Status::Working | Status::Waiting)
                || t.claim_id.as_deref().unwrap_or(&claim.id) != claim_id
            {
                return Err(Error::ClaimConflict);
            }
            let state = turn_state(tx, claim)?;
            if !matches!(
                state,
                TurnState::Failed | TurnState::Interrupted | TurnState::Unknown
            ) && !(state == TurnState::Succeeded && *confirmed_settled)
            {
                return Err(Error::InvalidState);
            }
            t.reconciled_claim_id =
                (*confirmed_settled && state != TurnState::Succeeded).then(|| claim_id.clone());
        }
        Command::ReconcileResult {
            result_id,
            allow_review,
            evidence,
            ..
        } => {
            record_evidence(evidence)?;
            let result = t
                .results
                .iter()
                .find(|r| r.attempt_id == t.attempt_id && r.id == *result_id)
                .ok_or(Error::NotFound)?;
            let status = turn_state(tx, &result.turn)?;
            if t.status != Status::Submitted
                || !(matches!(status, TurnState::Failed | TurnState::Interrupted)
                    || (status == TurnState::Succeeded && *allow_review))
            {
                return Err(Error::InvalidState);
            }
            if *allow_review {
                if status != TurnState::Succeeded {
                    t.reconciled_result_id = Some(result_id.clone());
                    release_effect_barrier(tx, t, &result.turn, Some("review"))?;
                }
            } else {
                if t.attempts >= 4u32.saturating_add(t.extra_attempts) {
                    return Err(Error::LimitExceeded);
                }
                let summary =
                    "User requested rework after the submission turn did not settle successfully.";
                t.reviews.push(Review {
                    result_id: result_id.clone(),
                    accepted: false,
                    assessment: Assessment {
                        summary: summary.into(),
                        criteria: t
                            .acceptance
                            .iter()
                            .map(|a| AssessmentItem {
                                criterion_id: a.id.clone(),
                                verdict: Verdict::NotChecked,
                                explanation: summary.into(),
                                evidence: vec![],
                            })
                            .collect(),
                    },
                    requested_changes: Some(summary.into()),
                    turn: None,
                });
                withdraw(tx, &t.id, false)?;
                t.attempts += 1;
                t.attempt_id = id();
                t.claim = None;
                t.claim_id = None;
                t.reconciled_result_id = None;
                t.reconciled_claim_id = None;
                t.ready = false;
                t.status = Status::Pending;
                t.blockers.clear();
                effects.push(effect(
                    tx,
                    t,
                    Actor::Member(t.assignee.clone()),
                    "execute",
                    t.barriers.clone(),
                )?);
            }
        }
        Command::ReconcileReview {
            result_id,
            barrier_turn_id,
            release,
            evidence,
            ..
        } => {
            record_evidence(evidence)?;
            let turn = t
                .reviews
                .iter()
                .find(|r| r.result_id == *result_id)
                .and_then(|r| r.turn.as_ref())
                .filter(|turn| turn.id == *barrier_turn_id)
                .cloned()
                .ok_or(Error::NotFound)?;
            let status = turn_state(tx, &turn)?;
            if !t.barriers.contains(&turn)
                || !(matches!(status, TurnState::Failed | TurnState::Interrupted)
                    || (status == TurnState::Succeeded && *release))
            {
                return Err(Error::InvalidState);
            }
            if *release && status != TurnState::Succeeded {
                t.barriers.retain(|b| b != &turn);
                release_effect_barrier(tx, t, &turn, None)?;
            }
        }
        _ => unreachable!(),
    }
    t.revision += 1;
    save_task(tx, t)?;
    event(
        tx,
        &t.group_id,
        &c.actor,
        "task_changed",
        &serde_json::json!({"task":TaskReceipt::from(&*t),"command":cmd}),
    )?;
    effects.extend(refresh_waiters(tx, &t.group_id)?);
    Ok(Some(Reply {
        task: Some(TaskReceipt::from(&*t)),
        group: None,
        entity_id: None,
        effect_ids: effects,
        replayed: false,
    }))
}
