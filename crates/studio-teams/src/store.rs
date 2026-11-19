use crate::*;
use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};
use serde::{de::DeserializeOwned, Serialize};
use std::{collections::BTreeSet, path::Path, time::Duration};

pub struct Store {
    db: Connection,
}
pub(crate) fn id() -> String {
    uuid::Uuid::new_v4().to_string()
}
pub(crate) fn json<T: Serialize>(value: &T) -> Result<String> {
    Ok(serde_json::to_string(value)?)
}
fn decode<T: DeserializeOwned>(value: String) -> Result<T> {
    Ok(serde_json::from_str(&value)?)
}
pub(crate) fn text(value: &str, max: usize) -> Result<()> {
    if value.trim().is_empty() {
        Err(Error::InvalidArgument)
    } else if value.len() > max {
        Err(Error::LimitExceeded)
    } else {
        Ok(())
    }
}
pub(crate) fn identifier(value: &str) -> Result<()> {
    text(value, 128)?;
    if !value.is_ascii() {
        return Err(Error::InvalidArgument);
    }
    Ok(())
}
fn continuation(value: &Continuation) -> Result<()> {
    text(&value.summary, 4096)?;
    text(&value.next_action, 4096)
}
fn distinct<'a>(values: impl IntoIterator<Item = &'a str>) -> bool {
    let mut seen = BTreeSet::new();
    values.into_iter().all(|v| seen.insert(v))
}
fn group(db: &Connection, group_id: &str) -> Result<Group> {
    decode(
        db.query_row("SELECT body FROM groups WHERE id=?", [group_id], |r| {
            r.get(0)
        })
        .optional()?
        .ok_or(Error::NotFound)?,
    )
}
fn task(db: &Connection, group_id: &str, task_id: &str) -> Result<Task> {
    decode(
        db.query_row(
            "SELECT body FROM tasks WHERE group_id=? AND id=?",
            params![group_id, task_id],
            |r| r.get(0),
        )
        .optional()?
        .ok_or(Error::NotFound)?,
    )
}
fn save_task(db: &Connection, t: &Task) -> Result<()> {
    db.execute("INSERT INTO tasks(id,group_id,status,body) VALUES(?,?,?,?) ON CONFLICT(id) DO UPDATE SET status=excluded.status,body=excluded.body", params![t.id,t.group_id,json(&t.status)?,json(t)?])?;
    Ok(())
}
fn save_group(db: &Connection, g: &Group) -> Result<()> {
    db.execute(
        "UPDATE groups SET body=? WHERE id=?",
        params![json(g)?, g.id],
    )?;
    Ok(())
}
fn access(g: &Group, caller: &Caller) -> Result<()> {
    if let Actor::Member(member_id) = &caller.actor {
        if g.archived {
            return Err(Error::Forbidden);
        }
        let member = g
            .members
            .iter()
            .find(|m| &m.id == member_id)
            .ok_or(Error::Forbidden)?;
        if caller.turn.as_ref().map(|t| &t.session) != Some(&member.session) {
            return Err(Error::Forbidden);
        }
    }
    Ok(())
}
fn turn_state(db: &Connection, turn: &Turn) -> Result<TurnState> {
    db.query_row(
        "SELECT state FROM turns WHERE turn_key=?",
        [json(turn)?],
        |r| r.get::<_, String>(0),
    )
    .optional()?
    .map(decode)
    .transpose()
    .map(|v| v.unwrap_or(TurnState::Unknown))
}
fn active_turn(db: &Connection, caller: &Caller) -> Result<Turn> {
    let turn = caller.turn.clone().ok_or(Error::TurnNotActive)?;
    if turn_state(db, &turn)? != TurnState::Active {
        return Err(Error::TurnNotActive);
    }
    Ok(turn)
}
fn same_claim(db: &Connection, t: &Task, caller: &Caller) -> Result<()> {
    if caller.actor != Actor::Member(t.assignee.clone()) {
        return Err(Error::Forbidden);
    }
    if t.claim.as_ref() != Some(&active_turn(db, caller)?) {
        return Err(Error::ClaimConflict);
    }
    Ok(())
}
fn settled(db: &Connection, barriers: &[Turn]) -> Result<bool> {
    for b in barriers {
        if turn_state(db, b)? != TurnState::Succeeded {
            return Ok(false);
        }
    }
    Ok(true)
}
fn unblocked(db: &Connection, t: &Task) -> Result<bool> {
    unblocked_in(db, t, None)
}
fn barriers_ready(db: &Connection, barriers: &[Turn], current: Option<&Turn>) -> Result<bool> {
    for b in barriers {
        if Some(b) != current && turn_state(db, b)? != TurnState::Succeeded {
            return Ok(false);
        }
    }
    Ok(true)
}
fn unblocked_in(db: &Connection, t: &Task, current: Option<&Turn>) -> Result<bool> {
    if t.questions
        .iter()
        .any(|q| q.attempt_id == t.attempt_id && q.blocking && q.answer.is_none())
        || t.blockers.iter().any(|b| b.resolution.is_none())
        || !settled(db, &t.barriers)?
    {
        return Ok(false);
    }
    for d in &t.dependencies {
        let dep = task(db, &t.group_id, d)?;
        let same_parent_review = dep.parent_id.as_deref() == Some(t.id.as_str())
            && dep.requester == Actor::Member(t.assignee.clone())
            && dep
                .reviews
                .last()
                .is_some_and(|r| r.accepted && r.turn.as_ref() == current && current.is_some());
        let exception = if same_parent_review { current } else { None };
        if dep.status != Status::Completed || !barriers_ready(db, &dep.barriers, exception)? {
            return Ok(false);
        }
    }
    Ok(true)
}
fn event(
    db: &Connection,
    group_id: &str,
    actor: &Actor,
    kind: &str,
    data: &impl Serialize,
) -> Result<()> {
    db.execute(
        "INSERT INTO events(group_id,actor,kind,data) VALUES(?,?,?,?)",
        params![group_id, json(actor)?, kind, json(data)?],
    )?;
    Ok(())
}
fn effect(
    db: &Connection,
    t: &Task,
    recipient: Actor,
    purpose: &str,
    barriers: Vec<Turn>,
) -> Result<String> {
    let e = Effect {
        entity_id: None,
        id: id(),
        group_id: t.group_id.clone(),
        task_id: t.id.clone(),
        attempt_id: t.attempt_id.clone(),
        recipient,
        purpose: purpose.into(),
        barriers,
        withdrawn: false,
    };
    db.execute(
        "INSERT INTO effects(id,group_id,task_id,body,withdrawn) VALUES(?,?,?,?,0)",
        params![e.id, e.group_id, e.task_id, json(&e)?],
    )?;
    Ok(e.id)
}

fn entity_effect(
    db: &Connection,
    t: &Task,
    recipient: Actor,
    purpose: &str,
    entity_id: &str,
) -> Result<String> {
    let id = effect(db, t, recipient, purpose, vec![])?;
    let mut e: Effect =
        decode(db.query_row("SELECT body FROM effects WHERE id=?", [&id], |r| r.get(0))?)?;
    e.entity_id = Some(entity_id.into());
    db.execute(
        "UPDATE effects SET body=? WHERE id=?",
        params![json(&e)?, id],
    )?;
    Ok(id)
}
fn withdraw_entity(db: &Connection, task_id: &str, entity_id: &str) -> Result<()> {
    let mut s = db.prepare("SELECT body FROM effects WHERE task_id=? AND withdrawn=0")?;
    let rows = s
        .query_map([task_id], |r| r.get::<_, String>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    for body in rows {
        let e: Effect = decode(body)?;
        if e.entity_id.as_deref() == Some(entity_id) {
            db.execute("UPDATE effects SET withdrawn=1 WHERE id=?", [e.id])?;
        }
    }
    Ok(())
}
// Reserve capacity for answers before accepting new questions; completion is never backpressured.
fn conversation_budget(t: &Task) -> Result<()> {
    let questions: usize = t
        .questions
        .iter()
        .map(|q| {
            q.text.len()
                + q.answer.as_ref().map_or(
                    if q.attempt_id == t.attempt_id {
                        32768
                    } else {
                        0
                    },
                    String::len,
                )
        })
        .sum();
    let blockers: usize = t
        .blockers
        .iter()
        .map(|b| b.detail.len() + b.resolution.as_ref().map_or(32768, String::len))
        .sum();
    if questions + blockers > 262144 {
        return Err(Error::LimitExceeded);
    }
    Ok(())
}
fn withdraw(db: &Connection, task_id: &str, execution_only: bool) -> Result<()> {
    let mut stmt = db.prepare("SELECT id,body FROM effects WHERE task_id=? AND withdrawn=0")?;
    let entries = stmt
        .query_map([task_id], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    for (id, body) in entries {
        let e: Effect = decode(body)?;
        if !execution_only || matches!(e.purpose.as_str(), "execute" | "ready") {
            db.execute("UPDATE effects SET withdrawn=1 WHERE id=?", [id])?;
        }
    }
    Ok(())
}
fn ready(db: &Connection, t: &mut Task, effects: &mut Vec<String>) -> Result<()> {
    if t.status != Status::Waiting {
        return Ok(());
    }
    let now = unblocked(db, t)?;
    if now && !t.ready {
        t.wait_generation += 1;
        effects.push(effect(
            db,
            t,
            Actor::Member(t.assignee.clone()),
            "ready",
            t.barriers.clone(),
        )?);
    }
    if !now && t.ready {
        withdraw(db, &t.id, true)?;
    }
    t.ready = now;
    Ok(())
}
fn refresh_waiters(db: &Connection, group_id: &str) -> Result<Vec<String>> {
    let mut stmt = db.prepare("SELECT body FROM tasks WHERE group_id=? AND status=?")?;
    let entries = stmt
        .query_map(params![group_id, json(&Status::Waiting)?], |r| {
            r.get::<_, String>(0)
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let mut effects = vec![];
    for body in entries {
        let mut t: Task = decode(body)?;
        let before = t.ready;
        ready(db, &mut t, &mut effects)?;
        if before != t.ready {
            t.revision += 1;
            save_task(db, &t)?;
            event(
                db,
                group_id,
                &Actor::User,
                "readiness_changed",
                &TaskReceipt::from(&t),
            )?;
        }
    }
    Ok(effects)
}

impl Store {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let db = Connection::open(path)?;
        db.busy_timeout(Duration::from_secs(5))?;
        db.execute_batch(
            "PRAGMA foreign_keys=ON; PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;",
        )?;
        let version: i64 = db.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if version > 1 {
            return Err(Error::Storage("unsupported Teams schema version".into()));
        }
        db.execute_batch("BEGIN IMMEDIATE;
CREATE TABLE IF NOT EXISTS wire_meta(k INTEGER PRIMARY KEY CHECK(k=1),instance TEXT NOT NULL);
INSERT OR IGNORE INTO wire_meta VALUES(1,lower(hex(randomblob(16))));
CREATE TABLE IF NOT EXISTS groups(id TEXT PRIMARY KEY,body TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS members(id TEXT PRIMARY KEY,group_id TEXT NOT NULL REFERENCES groups(id),backend TEXT NOT NULL,thread TEXT NOT NULL,UNIQUE(backend,thread));
CREATE TABLE IF NOT EXISTS controllers(backend TEXT NOT NULL,thread TEXT NOT NULL,PRIMARY KEY(backend,thread));
CREATE TABLE IF NOT EXISTS tasks(seq INTEGER PRIMARY KEY AUTOINCREMENT,id TEXT UNIQUE NOT NULL,group_id TEXT NOT NULL REFERENCES groups(id),status TEXT NOT NULL,body TEXT NOT NULL);
CREATE INDEX IF NOT EXISTS tasks_group_status ON tasks(group_id,status);
CREATE TABLE IF NOT EXISTS commands(scope TEXT NOT NULL,actor TEXT NOT NULL,request_id TEXT NOT NULL,payload TEXT NOT NULL,response TEXT NOT NULL,PRIMARY KEY(scope,actor,request_id));
CREATE TABLE IF NOT EXISTS events(seq INTEGER PRIMARY KEY AUTOINCREMENT,group_id TEXT NOT NULL,actor TEXT NOT NULL,kind TEXT NOT NULL,data TEXT NOT NULL);
CREATE INDEX IF NOT EXISTS events_group ON events(group_id,seq);
CREATE TABLE IF NOT EXISTS effects(seq INTEGER PRIMARY KEY AUTOINCREMENT,id TEXT UNIQUE NOT NULL,group_id TEXT NOT NULL,task_id TEXT NOT NULL,body TEXT NOT NULL,withdrawn INTEGER NOT NULL);
CREATE INDEX IF NOT EXISTS effects_group ON effects(group_id,seq);
CREATE TABLE IF NOT EXISTS blocker_decisions(id TEXT PRIMARY KEY,group_id TEXT NOT NULL,task_id TEXT NOT NULL,attempt_id TEXT NOT NULL,blocker_id TEXT NOT NULL,UNIQUE(task_id,attempt_id,blocker_id));
CREATE TABLE IF NOT EXISTS turns(turn_key TEXT PRIMARY KEY,state TEXT NOT NULL);
PRAGMA user_version=1; COMMIT;")?;
        Ok(Self { db })
    }
    /// Host-owned classification. The integration must register current and historical Router threads.
    pub fn exclude_controller(&mut self, session: &Session) -> Result<()> {
        let tx = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let exists: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM members WHERE backend=? AND thread=?)",
            params![session.backend, session.thread],
            |r| r.get(0),
        )?;
        if exists {
            return Err(Error::InvalidState);
        }
        tx.execute(
            "INSERT OR IGNORE INTO controllers VALUES(?,?)",
            params![session.backend, session.thread],
        )?;
        tx.commit()?;
        Ok(())
    }
    pub fn create_group(
        &mut self,
        caller: &Caller,
        request_id: &str,
        name: &str,
        members: Vec<MemberDraft>,
    ) -> Result<Group> {
        self.create_group_with_orchestrator(caller, request_id, name, members, None)
    }
    pub fn create_group_with_orchestrator(
        &mut self,
        caller: &Caller,
        request_id: &str,
        name: &str,
        members: Vec<MemberDraft>,
        orchestrator: Option<&Session>,
    ) -> Result<Group> {
        if caller.actor != Actor::User {
            return Err(Error::Forbidden);
        }
        identifier(request_id)?;
        text(name, 800)?;
        if !(2..=5).contains(&members.len()) {
            return Err(Error::LimitExceeded);
        }
        for m in &members {
            identifier(&m.session.backend)?;
            identifier(&m.session.thread)?;
            text(&m.name, 800)?;
            text(&m.responsibility, 2048)?;
        }
        let payload = match orchestrator {
            None => json(&(name, &members))?,
            Some(session) => json(&(name, &members, session))?,
        };
        let tx = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some(value) = cached(
            &tx,
            "instance:create_group",
            &Actor::User,
            request_id,
            &payload,
        )? {
            return decode(value)?;
        }
        tx.execute_batch("SAVEPOINT create_group")?;
        let outcome: Result<Group> = (|| {
            let mut seen = BTreeSet::new();
            for m in &members {
                if !seen.insert((&m.session.backend, &m.session.thread)) {
                    return Err(Error::InvalidArgument);
                }
                let used:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM members WHERE backend=?1 AND thread=?2 UNION SELECT 1 FROM controllers WHERE backend=?1 AND thread=?2)",params![m.session.backend,m.session.thread],|r|r.get(0))?;
                if used {
                    return Err(Error::InvalidState);
                }
            }
            let mut g = Group {
                id: id(),
                name: name.into(),
                revision: 1,
                paused: false,
                archived: false,
                roster_version: 1,
                orchestrator: None,
                members: members
                    .into_iter()
                    .map(|m| Member {
                        id: id(),
                        session: m.session,
                        name: m.name,
                        responsibility: m.responsibility,
                    })
                    .collect(),
            };
            if let Some(session) = orchestrator {
                g.orchestrator = Some(
                    g.members
                        .iter()
                        .find(|m| &m.session == session)
                        .ok_or(Error::InvalidArgument)?
                        .id
                        .clone(),
                );
            }
            tx.execute("INSERT INTO groups VALUES(?,?)", params![g.id, json(&g)?])?;
            for m in &g.members {
                tx.execute(
                    "INSERT INTO members VALUES(?,?,?,?)",
                    params![m.id, g.id, m.session.backend, m.session.thread],
                )?;
            }
            event(&tx, &g.id, &Actor::User, "group_created", &g)?;
            Ok(g)
        })();
        match &outcome {
            Ok(_) => tx.execute_batch("RELEASE create_group")?,
            Err(Error::Storage(_)) => return outcome,
            Err(_) => tx.execute_batch("ROLLBACK TO create_group; RELEASE create_group")?,
        }
        cache(
            &tx,
            "instance:create_group",
            &Actor::User,
            request_id,
            &payload,
            &json(&outcome)?,
        )?;
        tx.commit()?;
        outcome
    }
    /// Host control-plane only: record a user's decision for a specific still-open blocker.
    pub fn approve_blocker(
        &mut self,
        caller: &Caller,
        group_id: &str,
        version: &TaskVersion,
        blocker_id: &str,
    ) -> Result<String> {
        if caller.actor != Actor::User {
            return Err(Error::Forbidden);
        }
        let tx = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let t = task(&tx, group_id, &version.task_id)?;
        if t.attempt_id != version.attempt_id {
            return Err(Error::StaleAttempt);
        }
        if t.revision != version.expected_revision {
            return Err(Error::RevisionConflict);
        }
        if t.status != Status::Waiting
            || !t
                .blockers
                .iter()
                .any(|b| b.id == blocker_id && b.needs_user && b.resolution.is_none())
        {
            return Err(Error::InvalidState);
        }
        let decision:String=tx.query_row("INSERT INTO blocker_decisions VALUES(?,?,?,?,?) ON CONFLICT(task_id,attempt_id,blocker_id) DO UPDATE SET blocker_id=excluded.blocker_id RETURNING id",params![id(),group_id,t.id,t.attempt_id,blocker_id],|r|r.get(0))?;
        tx.commit()?;
        Ok(decision)
    }
    /// Host-only deletion. The host holds its dispatch gate and rejects live queue receipts.
    /// Native sessions are untouched; command receipts survive to prevent replay.
    pub fn delete_group(
        &mut self,
        caller: &Caller,
        group_id: &str,
        request_id: &str,
        expected_revision: u64,
    ) -> Result<()> {
        if caller.actor != Actor::User {
            return Err(Error::Forbidden);
        }
        identifier(request_id)?;
        let payload = json(&(group_id, expected_revision))?;
        let tx = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some(value) = cached(
            &tx,
            "instance:delete_group",
            &Actor::User,
            request_id,
            &payload,
        )? {
            return decode(value)?;
        }
        let g = group(&tx, group_id)?;
        if g.revision != expected_revision {
            return Err(Error::RevisionConflict);
        }
        if maintenance::unfinished_member_reference(&tx, &g, &[], true)? {
            return Err(Error::Blocked);
        }
        tx.execute("DELETE FROM members WHERE group_id=?", [group_id])?;
        tx.execute("DELETE FROM tasks WHERE group_id=?", [group_id])?;
        tx.execute("DELETE FROM effects WHERE group_id=?", [group_id])?;
        tx.execute("DELETE FROM blocker_decisions WHERE group_id=?", [group_id])?;
        tx.execute("DELETE FROM events WHERE group_id=?", [group_id])?;
        tx.execute("DELETE FROM groups WHERE id=?", [group_id])?;
        event(
            &tx,
            group_id,
            &Actor::User,
            "group_deleted",
            &serde_json::json!({"group_id":group_id}),
        )?;
        let result: Result<()> = Ok(());
        cache(
            &tx,
            "instance:delete_group",
            &Actor::User,
            request_id,
            &payload,
            &json(&result)?,
        )?;
        tx.commit()?;
        result
    }

    /// Host-only listing. Do not expose this on the member MCP transport.
    pub fn groups(&self) -> Result<Vec<Group>> {
        let mut query = self.db.prepare("SELECT body FROM groups ORDER BY rowid")?;
        let rows = query.query_map([], |r| r.get::<_, String>(0))?;
        rows.map(|r| decode(r?)).collect()
    }
    /// Resolve the caller from a backend-qualified native session supplied by the host.
    pub fn member_for_session(&self, session: &Session) -> Result<Option<(Group, Member)>> {
        let group_id: Option<String> = self
            .db
            .query_row(
                "SELECT group_id FROM members WHERE backend=? AND thread=?",
                params![session.backend, session.thread],
                |r| r.get(0),
            )
            .optional()?;
        let Some(group_id) = group_id else {
            return Ok(None);
        };
        let group = group(&self.db, &group_id)?;
        let member = group
            .members
            .iter()
            .find(|m| &m.session == session)
            .cloned()
            .ok_or_else(|| Error::Storage("membership index is inconsistent".into()))?;
        Ok(Some((group, member)))
    }
    pub(crate) fn cursor_namespace(&self) -> Result<String> {
        Ok(self
            .db
            .query_row("SELECT instance FROM wire_meta WHERE k=1", [], |r| r.get(0))?)
    }
    pub(crate) fn wire_tasks(
        &self,
        group_id: &str,
        after: i64,
        statuses: &[Status],
        assignee: Option<&str>,
        requester: Option<&Actor>,
        limit: usize,
    ) -> Result<Vec<(i64, serde_json::Value)>> {
        let mut q=self.db.prepare("SELECT seq,json_object('task_id',id,'attempt_id',body->>'$.attempt_id','revision',body->>'$.revision','ready',body->'$.ready','title',body->>'$.title','status',lower(body->>'$.status'),'assignee_member_id',body->>'$.assignee','requester',body->'$.requester','parent_task_id',body->>'$.parent_id') FROM tasks WHERE group_id=?1 AND seq>?2 AND (?3='[]' OR body->>'$.status' IN (SELECT value FROM json_each(?3))) AND (?4 IS NULL OR body->>'$.assignee'=?4) AND (?5 IS NULL OR body->'$.requester'=?5) ORDER BY seq LIMIT ?6")?;
        let rows = q.query_map(
            params![
                group_id,
                after,
                json(&statuses)?,
                assignee,
                requester.map(json).transpose()?,
                limit as i64
            ],
            |r| Ok((r.get(0)?, r.get::<_, String>(1)?)),
        )?;
        rows.map(|row| {
            let (seq, body) = row?;
            Ok((seq, decode(body)?))
        })
        .collect()
    }
    pub(crate) fn wire_history(
        &self,
        group_id: &str,
        task_id: &str,
        after: i64,
        watermark: i64,
        limit: usize,
    ) -> Result<Vec<Event>> {
        let mut q=self.db.prepare("SELECT seq,actor,kind,data FROM events WHERE group_id=?1 AND seq>?2 AND seq<=?3 AND ((kind='task_delegated' AND data->>'$.id'=?4) OR (kind='readiness_changed' AND data->>'$.id'=?4) OR (kind='task_changed' AND data->>'$.task.id'=?4) OR (kind='child_attached' AND (data->>'$.parent.id'=?4 OR data->>'$.child_id'=?4))) ORDER BY seq LIMIT ?5")?;
        let rows = q.query_map(
            params![group_id, after, watermark, task_id, limit as i64],
            |r| {
                Ok((
                    r.get(0)?,
                    r.get::<_, String>(1)?,
                    r.get(2)?,
                    r.get::<_, String>(3)?,
                ))
            },
        )?;
        rows.map(|row| {
            let (seq, actor, kind, data) = row?;
            Ok(Event {
                seq,
                group_id: group_id.into(),
                actor: decode(actor)?,
                kind,
                data: decode(data)?,
            })
        })
        .collect()
    }
    pub(crate) fn read_watermark(&self, group_id: &str) -> Result<i64> {
        Ok(self.db.query_row(
            "SELECT COALESCE(MAX(seq),0) FROM events WHERE group_id=?",
            [group_id],
            |r| r.get(0),
        )?)
    }
    pub(crate) fn authorize_tool(&self, caller: &Caller, group_id: &str) -> Result<Group> {
        let g = self.context(caller, group_id)?;
        if matches!(caller.actor, Actor::Member(_)) {
            active_turn(&self.db, caller)?;
        }
        Ok(g)
    }
    pub fn context(&self, caller: &Caller, group_id: &str) -> Result<Group> {
        let g = group(&self.db, group_id)?;
        access(&g, caller)?;
        Ok(g)
    }
    pub fn task(&self, caller: &Caller, group_id: &str, task_id: &str) -> Result<Task> {
        self.context(caller, group_id)?;
        task(&self.db, group_id, task_id)
    }
    pub fn tasks(
        &self,
        caller: &Caller,
        group_id: &str,
        after: i64,
        limit: usize,
    ) -> Result<Vec<(i64, Task)>> {
        self.context(caller, group_id)?;
        if limit == 0 || limit > 100 {
            return Err(Error::LimitExceeded);
        }
        let mut s = self.db.prepare(
            "SELECT seq,body FROM tasks WHERE group_id=? AND seq>? ORDER BY seq LIMIT ?",
        )?;
        let rows = s
            .query_map(params![group_id, after, limit as i64], |r| {
                Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        rows.into_iter()
            .map(|(seq, body)| Ok((seq, decode(body)?)))
            .collect()
    }
    pub fn events(
        &self,
        caller: &Caller,
        group_id: &str,
        after: i64,
        limit: usize,
    ) -> Result<Vec<Event>> {
        self.context(caller, group_id)?;
        if limit == 0 || limit > 100 {
            return Err(Error::LimitExceeded);
        }
        let mut s=self.db.prepare("SELECT seq,actor,kind,data FROM events WHERE group_id=? AND seq>? ORDER BY seq LIMIT ?")?;
        let rows = s
            .query_map(params![group_id, after, limit as i64], |r| {
                Ok((
                    r.get(0)?,
                    r.get::<_, String>(1)?,
                    r.get(2)?,
                    r.get::<_, String>(3)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        rows.into_iter()
            .map(|(seq, actor, kind, data)| {
                Ok(Event {
                    seq,
                    group_id: group_id.into(),
                    actor: decode(actor)?,
                    kind,
                    data: decode(data)?,
                })
            })
            .collect()
    }
    /// Host-only candidate effects. The future dispatcher must fence/revalidate at handoff.
    pub fn effects(&self, group_id: &str, after: i64, limit: usize) -> Result<Vec<(i64, Effect)>> {
        if limit == 0 || limit > 100 {
            return Err(Error::LimitExceeded);
        }
        let mut s=self.db.prepare("SELECT seq,body,withdrawn FROM effects WHERE group_id=? AND seq>? ORDER BY seq LIMIT ?")?;
        let rows = s
            .query_map(params![group_id, after, limit as i64], |r| {
                Ok((r.get(0)?, r.get::<_, String>(1)?, r.get::<_, bool>(2)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        rows.into_iter()
            .map(|(seq, body, withdrawn)| {
                let mut e: Effect = decode(body)?;
                e.withdrawn = withdrawn;
                Ok((seq, e))
            })
            .collect()
    }
    pub fn effect_ready(&self, candidate: &Effect) -> Result<bool> {
        let (body, withdrawn): (String, bool) = self
            .db
            .query_row(
                "SELECT body,withdrawn FROM effects WHERE id=?",
                [&candidate.id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?
            .ok_or(Error::NotFound)?;
        let e: Effect = decode(body)?;
        let g = group(&self.db, &e.group_id)?;
        let t = task(&self.db, &e.group_id, &e.task_id)?;
        if g.paused
            || withdrawn
            || t.status.terminal()
            || t.attempt_id != e.attempt_id
            || !settled(&self.db, &e.barriers)?
        {
            return Ok(false);
        }
        if e.purpose == "question" {
            return Ok(t.questions.iter().any(|q| {
                Some(&q.id) == e.entity_id.as_ref()
                    && q.attempt_id == t.attempt_id
                    && q.answer.is_none()
            }));
        }
        if e.purpose == "needs_user" {
            return Ok(t
                .blockers
                .iter()
                .any(|b| Some(&b.id) == e.entity_id.as_ref() && b.resolution.is_none()));
        }
        if matches!(e.purpose.as_str(), "execute" | "ready") {
            return unblocked(&self.db, &t);
        }
        Ok(true)
    }
    /// Adapter observations only, not an Agent command. Terminal states never return to active.
    pub fn recover_turns(&mut self) -> Result<()> {
        self.db.execute(
            "UPDATE turns SET state=? WHERE state=?",
            params![json(&TurnState::Unknown)?, json(&TurnState::Active)?],
        )?;
        Ok(())
    }
    pub fn recover_backend_turns(&mut self, backend: &str) -> Result<()> {
        self.db.execute("UPDATE turns SET state=? WHERE state=? AND json_extract(turn_key,'$.session.backend')=?",params![json(&TurnState::Unknown)?,json(&TurnState::Active)?,backend])?;
        Ok(())
    }
    /// Historical terminal evidence is valid across connection generations for this native turn.
    pub fn reconcile_native_terminal(
        &mut self,
        session: &Session,
        id: &str,
        status: TurnState,
    ) -> Result<()> {
        if !matches!(
            status,
            TurnState::Succeeded | TurnState::Interrupted | TurnState::Failed
        ) {
            return Err(Error::InvalidArgument);
        }
        let mut q=self.db.prepare("SELECT turn_key FROM turns WHERE json_extract(turn_key,'$.session.backend')=? AND json_extract(turn_key,'$.session.thread')=? AND json_extract(turn_key,'$.id')=?")?;
        let rows = q
            .query_map(params![session.backend, session.thread, id], |r| {
                r.get::<_, String>(0)
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        drop(q);
        for row in rows {
            let turn: Turn = decode(row)?;
            if self.observed_turn(&turn)? != status {
                self.observe_turn(&turn, status)?;
            }
        }
        Ok(())
    }
    pub fn observed_turn(&self, turn: &Turn) -> Result<TurnState> {
        turn_state(&self.db, turn)
    }
    pub fn command_details(
        &self,
        caller: &Caller,
        group_id: &str,
        request_id: &str,
    ) -> Result<Option<(Command, Result<Reply>)>> {
        self.context(caller, group_id)?;
        let row: Option<(String, String)> = self
            .db
            .query_row(
                "SELECT payload,response FROM commands WHERE scope=? AND actor=? AND request_id=?",
                params![group_id, json(&caller.actor)?, request_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        row.map(|(payload, response)| {
            let (_, cmd): (u32, Command) = decode(payload)?;
            Ok((cmd, decode(response)?))
        })
        .transpose()
    }
    pub fn observe_turn(&mut self, turn: &Turn, state: TurnState) -> Result<()> {
        let tx = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let prior = turn_state(&tx, turn)?;
        if state == TurnState::Active {
            // Unknown records require authoritative terminal reconciliation, not late Active replay.
            let exists: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM turns WHERE turn_key=?)",
                [json(turn)?],
                |r| r.get(0),
            )?;
            if exists && prior == TurnState::Unknown {
                return Err(Error::ReconciliationRequired);
            }
            let mut stmt = tx.prepare("SELECT turn_key FROM turns WHERE state=?")?;
            let keys = stmt
                .query_map([json(&TurnState::Active)?], |r| r.get::<_, String>(0))?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            drop(stmt);
            for key in keys {
                let old: Turn = decode(key.clone())?;
                if old.session == turn.session && old != *turn {
                    tx.execute(
                        "UPDATE turns SET state=? WHERE turn_key=?",
                        params![json(&TurnState::Unknown)?, key],
                    )?;
                }
            }
        }
        if matches!(
            prior,
            TurnState::Succeeded | TurnState::Failed | TurnState::Interrupted
        ) && prior != state
        {
            return Err(Error::InvalidState);
        }
        tx.execute("INSERT INTO turns VALUES(?,?) ON CONFLICT(turn_key) DO UPDATE SET state=excluded.state",params![json(turn)?,json(&state)?])?;
        let mut stmt = tx.prepare("SELECT id FROM groups")?;
        let groups = stmt
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        drop(stmt);
        for g in groups {
            refresh_waiters(&tx, &g)?;
        }
        tx.commit()?;
        Ok(())
    }
    pub fn execute(
        &mut self,
        caller: &Caller,
        group_id: &str,
        request_id: &str,
        command: Command,
    ) -> Result<Reply> {
        identifier(request_id)?;
        let payload = json(&(1, &command))?;
        if payload.len() > 131072 {
            return Err(Error::LimitExceeded);
        }
        let tx = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let g = group(&tx, group_id)?;
        access(&g, caller)?; // Always authorize before replay.
        if let Some(response) = cached(&tx, group_id, &caller.actor, request_id, &payload)? {
            let mut reply: Result<Reply> = decode(response)?;
            if let Ok(r) = &mut reply {
                r.replayed = true;
            }
            return reply;
        }
        if matches!(caller.actor, Actor::Member(_)) {
            active_turn(&tx, caller)?;
        }
        tx.execute_batch("SAVEPOINT domain_command")?;
        let result = apply(&tx, g, caller, command);
        match &result {
            Ok(_) => tx.execute_batch("RELEASE domain_command")?,
            Err(Error::Storage(_)) => return result,
            Err(_) => tx.execute_batch("ROLLBACK TO domain_command; RELEASE domain_command")?,
        }
        cache(
            &tx,
            group_id,
            &caller.actor,
            request_id,
            &payload,
            &json(&result)?,
        )?;
        tx.commit()?;
        result
    }
    pub fn command(
        &self,
        caller: &Caller,
        group_id: &str,
        request_id: &str,
    ) -> Result<Option<Result<Reply>>> {
        self.context(caller, group_id)?;
        self.db
            .query_row(
                "SELECT response FROM commands WHERE scope=? AND actor=? AND request_id=?",
                params![group_id, json(&caller.actor)?, request_id],
                |r| r.get::<_, String>(0),
            )
            .optional()?
            .map(decode)
            .transpose()
    }
}
fn cached(
    db: &Connection,
    scope: &str,
    actor: &Actor,
    key: &str,
    payload: &str,
) -> Result<Option<String>> {
    let row: Option<(String, String)> = db
        .query_row(
            "SELECT payload,response FROM commands WHERE scope=? AND actor=? AND request_id=?",
            params![scope, json(actor)?, key],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    match row {
        Some((old, response)) if old == payload => Ok(Some(response)),
        Some(_) => Err(Error::IdempotencyConflict),
        None => Ok(None),
    }
}
fn cache(
    db: &Connection,
    scope: &str,
    actor: &Actor,
    key: &str,
    payload: &str,
    response: &str,
) -> Result<()> {
    db.execute(
        "INSERT INTO commands VALUES(?,?,?,?,?)",
        params![scope, json(actor)?, key, payload, response],
    )?;
    Ok(())
}
fn evidence(items: &[Evidence]) -> Result<()> {
    if items.len() > 20 {
        return Err(Error::LimitExceeded);
    }
    for e in items {
        text(&e.locator, 2048)?;
        text(&e.description, 2048)?;
    }
    Ok(())
}
fn depends_on(
    db: &Connection,
    group_id: &str,
    start: &str,
    target: &str,
    seen: &mut BTreeSet<String>,
) -> Result<bool> {
    if start == target {
        return Ok(true);
    }
    if !seen.insert(start.into()) {
        return Ok(false);
    }
    for dependency in task(db, group_id, start)?.dependencies {
        if depends_on(db, group_id, &dependency, target, seen)? {
            return Ok(true);
        }
    }
    Ok(false)
}
fn apply(tx: &Transaction<'_>, mut g: Group, caller: &Caller, command: Command) -> Result<Reply> {
    let audit = serde_json::to_value(&command)?;
    let mut effects = vec![];
    let mut entity_id = None;
    if g.archived {
        return Err(Error::InvalidState);
    }
    if let Some(reply) = manage_group(tx, &mut g, caller, &command)? {
        return Ok(reply);
    }
    let command = match command {
        Command::GroupGoal {
            expected_revision,
            to_member_id,
            title,
            request,
            acceptance,
            continuation,
        } => {
            if caller.actor != Actor::User {
                return Err(Error::Forbidden);
            }
            if g.revision != expected_revision {
                return Err(Error::RevisionConflict);
            }
            let to_member_id = to_member_id
                .or_else(|| g.orchestrator.clone())
                .ok_or(Error::InvalidArgument)?;
            Command::Delegate {
                delegation: Delegation {
                    to_member_id,
                    title,
                    request,
                    acceptance,
                    continuation,
                    dependencies: vec![],
                    parent: None,
                },
            }
        }
        other => other,
    };
    if let Command::SetPaused {
        expected_revision,
        paused,
    } = command
    {
        if caller.actor != Actor::User {
            return Err(Error::Forbidden);
        }
        if g.revision != expected_revision {
            return Err(Error::RevisionConflict);
        }
        if !paused && g.members.len() < 2 {
            return Err(Error::InvalidState);
        }
        g.paused = paused;
        g.revision += 1;
        save_group(tx, &g)?;
        event(tx, &g.id, &caller.actor, "group_pause_changed", &g)?;
        return Ok(Reply {
            group: Some(g),
            task: None,
            entity_id: None,
            effect_ids: effects,
            replayed: false,
        });
    }
    if let Command::SetOrchestrator {
        expected_revision,
        member_id,
    } = command
    {
        if caller.actor != Actor::User {
            return Err(Error::Forbidden);
        }
        if g.revision != expected_revision {
            return Err(Error::RevisionConflict);
        }
        if member_id
            .as_ref()
            .is_some_and(|id| !g.members.iter().any(|m| &m.id == id))
        {
            return Err(Error::InvalidArgument);
        }
        g.orchestrator = member_id;
        g.roster_version += 1;
        g.revision += 1;
        save_group(tx, &g)?;
        event(tx, &g.id, &caller.actor, "orchestrator_changed", &g)?;
        return Ok(Reply {
            group: Some(g),
            task: None,
            entity_id: None,
            effect_ids: effects,
            replayed: false,
        });
    }
    if let Command::Delegate { delegation: d } = command {
        if g.paused {
            return Err(Error::GroupPaused);
        }
        if !g.members.iter().any(|m| m.id == d.to_member_id) {
            return Err(Error::Forbidden);
        }
        text(&d.title, 800)?;
        if d.title.chars().count() > 200 {
            return Err(Error::LimitExceeded);
        }
        text(&d.request, 32768)?;
        if d.acceptance.is_empty() || d.acceptance.len() > 20 || d.dependencies.len() > 20 {
            return Err(Error::LimitExceeded);
        }
        if !distinct(d.acceptance.iter().map(|c| c.id.as_str()))
            || !distinct(d.dependencies.iter().map(String::as_str))
        {
            return Err(Error::InvalidArgument);
        }
        for c in &d.acceptance {
            identifier(&c.id)?;
            text(&c.text, 2048)?;
        }
        if let Some(c) = &d.continuation {
            continuation(c)?;
        }
        let count: i64 = tx.query_row(
            "SELECT COUNT(*) FROM tasks WHERE group_id=? AND status NOT IN (?,?)",
            params![g.id, json(&Status::Completed)?, json(&Status::Cancelled)?],
            |r| r.get(0),
        )?;
        if count >= 100 {
            return Err(Error::LimitExceeded);
        }
        for dependency in &d.dependencies {
            task(tx, &g.id, dependency)?;
        }
        let mut parent = if let Some(p) = &d.parent {
            let t = task(tx, &g.id, &p.task_id)?;
            if t.revision != p.expected_revision {
                return Err(Error::RevisionConflict);
            }
            if t.attempt_id != p.attempt_id {
                return Err(Error::StaleAttempt);
            }
            if !matches!(t.status, Status::Working | Status::Waiting) {
                return Err(Error::InvalidState);
            }
            same_claim(tx, &t, caller)?;
            if t.dependencies.len() >= 20 {
                return Err(Error::LimitExceeded);
            }
            for dep in &d.dependencies {
                if depends_on(tx, &g.id, dep, &t.id, &mut BTreeSet::new())? {
                    return Err(Error::InvalidArgument);
                }
            }
            Some(t)
        } else {
            None
        };
        let mut t = Task {
            id: id(),
            group_id: g.id.clone(),
            requester: caller.actor.clone(),
            assignee: d.to_member_id,
            title: d.title,
            request: d.request,
            acceptance: d.acceptance,
            dependencies: d.dependencies,
            parent_id: parent.as_ref().map(|p| p.id.clone()),
            requester_continuation: d.continuation,
            checkpoint: None,
            status: Status::Pending,
            revision: 1,
            attempt_id: id(),
            attempts: 1,
            extra_attempts: 0,
            reconciled_claim_id: None,
            reconciled_result_id: None,
            ready: false,
            wait_generation: 0,
            claim: None,
            claim_id: None,
            questions: vec![],
            blockers: vec![],
            results: vec![],
            reviews: vec![],
            barriers: vec![],
        };
        if unblocked(tx, &t)? {
            effects.push(effect(
                tx,
                &t,
                Actor::Member(t.assignee.clone()),
                "execute",
                vec![],
            )?);
        } else {
            t.status = Status::Waiting;
            t.wait_generation = 1;
        }
        save_task(tx, &t)?;
        if let Some(p) = &mut parent {
            p.dependencies.push(t.id.clone());
            p.status = Status::Waiting;
            p.wait_generation += 1;
            p.ready = false;
            p.revision += 1;
            withdraw(tx, &p.id, true)?;
            save_task(tx, p)?;
            event(
                tx,
                &g.id,
                &caller.actor,
                "child_attached",
                &serde_json::json!({"parent":TaskReceipt::from(&*p),"child_id":t.id}),
            )?;
        }
        event(tx, &g.id, &caller.actor, "task_delegated", &t)?;
        return Ok(Reply {
            task: Some(TaskReceipt::from(&t)),
            group: None,
            entity_id: None,
            effect_ids: effects,
            replayed: false,
        });
    }
    let version = command.version().ok_or(Error::InvalidArgument)?;
    let mut t = task(tx, &g.id, &version.task_id)?;
    if t.revision != version.expected_revision {
        return Err(Error::RevisionConflict);
    }
    if t.attempt_id != version.attempt_id {
        return Err(Error::StaleAttempt);
    }
    if t.status.terminal() && !matches!(command, Command::ReconcileReview { .. }) {
        return Err(Error::TaskTerminal);
    }
    if let Some(reply) = maintain_task(tx, &mut t, caller, &command)? {
        return Ok(reply);
    }
    let mut related = None;
    let mut approved = false;
    let command = match command {
        Command::ReclaimById {
            task,
            previous_claim_id,
            continuation,
        } => {
            let previous_turn = t.claim.clone().ok_or(Error::ClaimConflict)?;
            if previous_claim_id != t.claim_id.as_deref().unwrap_or(&previous_turn.id) {
                return Err(Error::ClaimConflict);
            }
            Command::Reclaim {
                task,
                previous_turn,
                continuation,
            }
        }
        Command::AskRelated {
            task,
            to,
            question,
            blocking,
            continuation,
            related_question_id,
        } => {
            if !t.questions.iter().any(|q| q.id == related_question_id) {
                return Err(Error::NotFound);
            }
            related = Some(related_question_id);
            Command::Ask {
                task,
                to,
                question,
                blocking,
                continuation,
            }
        }
        Command::ResolveApproved {
            task,
            blocker_id,
            resolution,
            user_decision_id,
        } => {
            approved=tx.query_row("SELECT EXISTS(SELECT 1 FROM blocker_decisions WHERE id=? AND group_id=? AND task_id=? AND attempt_id=? AND blocker_id=?)",params![user_decision_id,t.group_id,t.id,t.attempt_id,blocker_id],|r|r.get(0))?;
            if !approved {
                return Err(Error::Forbidden);
            }
            Command::Resolve {
                task,
                blocker_id,
                resolution,
            }
        }
        other => other,
    };
    match command {
        Command::Begin { .. } | Command::Resume { .. } => {
            let resume = matches!(command, Command::Resume { .. });
            if g.paused {
                return Err(Error::GroupPaused);
            }
            if caller.actor != Actor::Member(t.assignee.clone()) {
                return Err(Error::Forbidden);
            }
            if t.status
                != if resume {
                    Status::Waiting
                } else {
                    Status::Pending
                }
            {
                return Err(Error::InvalidState);
            }
            if !unblocked_in(tx, &t, if resume { caller.turn.as_ref() } else { None })? {
                return Err(Error::Blocked);
            }
            let turn = active_turn(tx, caller)?;
            if let Some(previous) = &t.claim {
                if previous != &turn
                    && turn_state(tx, previous)? != TurnState::Succeeded
                    && t.reconciled_claim_id.as_deref()
                        != Some(t.claim_id.as_deref().unwrap_or(&previous.id))
                {
                    return Err(Error::ReconciliationRequired);
                }
            }
            t.reconciled_claim_id = None;
            t.claim_id = Some(id());
            t.claim = Some(turn);
            t.status = Status::Working;
            t.ready = false;
            withdraw(tx, &t.id, true)?;
        }
        Command::Reclaim {
            previous_turn,
            continuation: c,
            ..
        } => {
            if g.paused {
                return Err(Error::GroupPaused);
            }
            if caller.actor != Actor::Member(t.assignee.clone()) {
                return Err(Error::Forbidden);
            }
            if t.status != Status::Working || t.claim.as_ref() != Some(&previous_turn) {
                return Err(Error::ClaimConflict);
            }
            if turn_state(tx, &previous_turn)? != TurnState::Succeeded
                && (t.reconciled_claim_id.is_none()
                    || t.reconciled_claim_id.as_deref()
                        != t.claim_id.as_deref().or(Some(&previous_turn.id)))
            {
                return Err(Error::ReconciliationRequired);
            }
            continuation(&c)?;
            t.reconciled_claim_id = None;
            t.claim_id = Some(id());
            t.claim = Some(active_turn(tx, caller)?);
            t.checkpoint = Some(c);
        }
        Command::Checkpoint {
            continuation: c, ..
        } => {
            if !matches!(t.status, Status::Working | Status::Waiting) {
                return Err(Error::InvalidState);
            }
            same_claim(tx, &t, caller)?;
            continuation(&c)?;
            t.checkpoint = Some(c);
        }
        Command::Wait {
            needs_user,
            detail,
            continuation: c,
            ..
        } => {
            if t.status != Status::Working {
                return Err(Error::InvalidState);
            }
            same_claim(tx, &t, caller)?;
            text(&detail, 32768)?;
            continuation(&c)?;
            if t.blockers.len() + t.questions.len() >= 256 {
                return Err(Error::LimitExceeded);
            }
            let blocker = Blocker {
                id: id(),
                needs_user,
                detail,
                resolution: None,
            };
            entity_id = Some(blocker.id.clone());
            t.blockers.push(blocker);
            conversation_budget(&t)?;
            t.status = Status::Waiting;
            t.ready = false;
            t.wait_generation += 1;
            t.checkpoint = Some(c);
            withdraw(tx, &t.id, true)?;
            if needs_user {
                effects.push(entity_effect(
                    tx,
                    &t,
                    Actor::User,
                    "needs_user",
                    entity_id.as_ref().unwrap(),
                )?);
            }
        }
        Command::Resolve {
            blocker_id,
            resolution,
            ..
        } => {
            if !matches!(t.status, Status::Working | Status::Waiting) {
                return Err(Error::InvalidState);
            }
            if caller.actor != Actor::User && caller.actor != Actor::Member(t.assignee.clone()) {
                return Err(Error::Forbidden);
            }
            text(&resolution, 32768)?;
            let blocker = t
                .blockers
                .iter_mut()
                .find(|b| b.id == blocker_id)
                .ok_or(Error::NotFound)?;
            if blocker.needs_user && caller.actor != Actor::User && !approved {
                return Err(Error::Forbidden);
            }
            if blocker.resolution.is_some() {
                return Err(Error::InvalidState);
            }
            blocker.resolution = Some(resolution);
            withdraw_entity(tx, &t.id, &blocker_id)?;
            ready(tx, &mut t, &mut effects)?;
        }
        Command::Ask {
            to,
            question,
            blocking,
            continuation: c,
            ..
        } => {
            if g.paused {
                same_claim(tx, &t, caller)?;
            }
            if caller.actor != t.requester && caller.actor != Actor::Member(t.assignee.clone()) {
                return Err(Error::Forbidden);
            }
            if to == caller.actor {
                return Err(Error::InvalidArgument);
            }
            if let Actor::Member(id) = &to {
                if !g.members.iter().any(|m| &m.id == id) {
                    return Err(Error::Forbidden);
                }
            }
            text(&question, 32768)?;
            if t.questions.len() + t.blockers.len() >= 256
                || t.questions
                    .iter()
                    .filter(|q| q.attempt_id == t.attempt_id && q.answer.is_none())
                    .count()
                    >= 20
            {
                return Err(Error::LimitExceeded);
            }
            if blocking {
                if !matches!(t.status, Status::Working | Status::Waiting) {
                    return Err(Error::InvalidState);
                }
                same_claim(tx, &t, caller)?;
                let c = c.ok_or(Error::InvalidArgument)?;
                continuation(&c)?;
                t.checkpoint = Some(c);
                t.status = Status::Waiting;
                t.ready = false;
                t.wait_generation += 1;
                withdraw(tx, &t.id, true)?;
            } else if let Some(c) = &c {
                continuation(c)?;
            }
            let q = Question {
                related_question_id: related,
                id: id(),
                attempt_id: t.attempt_id.clone(),
                asked_by: caller.actor.clone(),
                to: to.clone(),
                text: question,
                blocking,
                answer: None,
            };
            entity_id = Some(q.id.clone());
            t.questions.push(q);
            conversation_budget(&t)?;
            effects.push(entity_effect(
                tx,
                &t,
                to,
                "question",
                entity_id.as_ref().unwrap(),
            )?);
        }
        Command::Answer {
            question_id,
            answer,
            ..
        } => {
            text(&answer, 32768)?;
            let q = t
                .questions
                .iter_mut()
                .find(|q| q.id == question_id && q.attempt_id == t.attempt_id)
                .ok_or(Error::NotFound)?;
            if caller.actor != Actor::User && caller.actor != q.to {
                return Err(Error::Forbidden);
            }
            if q.answer.is_some() {
                return Err(Error::InvalidState);
            }
            q.answer = Some(answer);
            let recipient = q.asked_by.clone();
            let before = t.ready;
            withdraw_entity(tx, &t.id, &question_id)?;
            ready(tx, &mut t, &mut effects)?;
            // A ready notification includes the task snapshot pointer and thus the answer.
            if before == t.ready || recipient != Actor::Member(t.assignee.clone()) {
                effects.push(effect(tx, &t, recipient, "answer", vec![])?);
            }
        }
        Command::Submit { submission, .. } => {
            if t.status != Status::Working {
                return Err(Error::InvalidState);
            }
            same_claim(tx, &t, caller)?;
            if !unblocked_in(tx, &t, caller.turn.as_ref())? {
                return Err(Error::Blocked);
            }
            text(&submission.summary, 32768)?;
            if submission.limitations.len() > 32768 {
                return Err(Error::LimitExceeded);
            }
            evidence(&submission.evidence)?;
            if submission.criteria.len() != t.acceptance.len()
                || !distinct(submission.criteria.iter().map(|c| c.criterion_id.as_str()))
            {
                return Err(Error::InvalidArgument);
            }
            for c in &submission.criteria {
                text(&c.explanation, 2048)?;
                if !t.acceptance.iter().any(|a| a.id == c.criterion_id)
                    || c.evidence_indexes.len() > 20
                    || c.evidence_indexes
                        .iter()
                        .any(|&i| i >= submission.evidence.len())
                {
                    return Err(Error::InvalidArgument);
                }
            }
            let result = TaskResult {
                id: id(),
                attempt_id: t.attempt_id.clone(),
                submission,
                turn: active_turn(tx, caller)?,
            };
            entity_id = Some(result.id.clone());
            let barrier = result.turn.clone();
            t.results.push(result);
            t.status = Status::Submitted;
            effects.push(effect(
                tx,
                &t,
                t.requester.clone(),
                "review",
                vec![barrier],
            )?);
        }
        Command::Review {
            result_id,
            accept,
            assessment,
            requested_changes,
            ..
        } => {
            if caller.actor != Actor::User && caller.actor != t.requester {
                return Err(Error::Forbidden);
            }
            if t.status != Status::Submitted {
                return Err(Error::InvalidState);
            }
            let result = t
                .results
                .iter()
                .find(|r| r.id == result_id && r.attempt_id == t.attempt_id)
                .ok_or(Error::NotFound)?;
            match turn_state(tx, &result.turn)? {
                TurnState::Succeeded => {}
                TurnState::Active => return Err(Error::ResultNotReady),
                TurnState::Failed | TurnState::Interrupted
                    if t.reconciled_result_id.as_ref() == Some(&result.id) => {}
                _ => return Err(Error::ReconciliationRequired),
            }
            text(&assessment.summary, 32768)?;
            if assessment.criteria.len() != t.acceptance.len()
                || !distinct(assessment.criteria.iter().map(|a| a.criterion_id.as_str()))
            {
                return Err(Error::InvalidArgument);
            }
            for a in &assessment.criteria {
                text(&a.explanation, 2048)?;
                evidence(&a.evidence)?;
                if !t.acceptance.iter().any(|c| c.id == a.criterion_id) {
                    return Err(Error::InvalidArgument);
                }
            }
            let all_pass = assessment
                .criteria
                .iter()
                .all(|a| a.verdict == Verdict::Pass);
            if accept != all_pass {
                return Err(Error::InvalidArgument);
            }
            if accept && requested_changes.is_some() {
                return Err(Error::InvalidArgument);
            }
            if !accept {
                text(
                    requested_changes.as_deref().ok_or(Error::InvalidArgument)?,
                    32768,
                )?;
                if t.attempts >= 4u32.saturating_add(t.extra_attempts) {
                    return Err(Error::LimitExceeded);
                }
            }
            let turn = if caller.actor == Actor::User {
                None
            } else {
                Some(active_turn(tx, caller)?)
            };
            t.reviews.push(Review {
                result_id,
                accepted: accept,
                assessment,
                requested_changes,
                turn: turn.clone(),
            });
            if let Some(turn) = turn {
                t.barriers.push(turn);
            }
            withdraw(tx, &t.id, false)?;
            if accept {
                t.status = Status::Completed;
            } else {
                t.attempt_id = id();
                t.attempts += 1;
                t.claim = None;
                t.claim_id = None;
                t.reconciled_claim_id = None;
                t.reconciled_result_id = None;
                t.blockers.clear();
                t.ready = false;
                t.status = Status::Pending;
                effects.push(effect(
                    tx,
                    &t,
                    Actor::Member(t.assignee.clone()),
                    "execute",
                    t.barriers.clone(),
                )?);
            }
        }
        Command::Cancel { reason, .. } => {
            if caller.actor != Actor::User && caller.actor != t.requester {
                return Err(Error::Forbidden);
            }
            text(&reason, 32768)?;
            t.status = Status::Cancelled;
            t.ready = false;
            withdraw(tx, &t.id, false)?;
        }
        _ => return Err(Error::InvalidArgument),
    }
    t.revision += 1;
    save_task(tx, &t)?;
    event(
        tx,
        &g.id,
        &caller.actor,
        "task_changed",
        &serde_json::json!({"command":audit,"task":TaskReceipt::from(&t),"entity_id":entity_id}),
    )?;
    // Dependency readiness changes and their notifications commit with the accepting command.
    effects.extend(refresh_waiters(tx, &g.id)?);
    let t = task(tx, &g.id, &t.id)?;
    Ok(Reply {
        task: Some(TaskReceipt::from(&t)),
        group: None,
        entity_id,
        effect_ids: effects,
        replayed: false,
    })
}

#[path = "maintenance.rs"]
mod maintenance;
use maintenance::{maintain_task, manage_group};
