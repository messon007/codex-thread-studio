//! General-purpose persistent queue. This module never sends RPCs.
//! A single host worker may call prepare_send once, then queue/add exactly once.
//! Restart/timeout after prepare_send is uncertain; never retry that dispatch automatically.
use crate::store::{id, identifier, json, text};
use crate::{Error, Result, Session};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use std::{path::Path, time::Duration};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum State {
    Queued,
    Authorized,
    Sending,
    NativeQueued,
    Started,
    Settled,
    Withdrawn,
    Unknown,
    Abandoned,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    pub seq: i64,
    pub id: String,
    pub session: Session,
    pub source: String,
    pub stable_id: String,
    pub scope: String,
    pub generation: u64,
    pub message: String,
    pub state: State,
    pub dispatch_id: Option<String>,
    pub native_queue_id: Option<String>,
    pub native_turn_id: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reservation {
    pub id: String,
    pub granted: bool,
    pub pending: Vec<Entry>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fence {
    pub intent_id: String,
    pub entries: Vec<Entry>,
}
pub struct Queue {
    db: Connection,
}
fn entry(body: String) -> Result<Entry> {
    Ok(serde_json::from_str(&body)?)
}
fn get(db: &Connection, id: &str) -> Result<Entry> {
    entry(
        db.query_row("SELECT body FROM entries WHERE id=?", [id], |r| r.get(0))
            .optional()?
            .ok_or(Error::NotFound)?,
    )
}
fn save(db: &Connection, e: &Entry) -> Result<()> {
    db.execute(
        "UPDATE entries SET state=?,body=? WHERE id=?",
        params![json(&e.state)?, json(e)?, e.id],
    )?;
    Ok(())
}
fn list(db: &Connection, session: &Session) -> Result<Vec<Entry>> {
    let mut s = db.prepare(
        "SELECT body FROM entries WHERE session=? AND state NOT IN (?,?,?) ORDER BY seq",
    )?;
    let rows = s
        .query_map(
            params![
                json(session)?,
                json(&State::Settled)?,
                json(&State::Withdrawn)?,
                json(&State::Abandoned)?
            ],
            |r| r.get::<_, String>(0),
        )?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    rows.into_iter().map(entry).collect()
}
fn holds(db: &Connection, session: &Session) -> Result<bool> {
    Ok(db.query_row(
        "SELECT EXISTS(SELECT 1 FROM holds WHERE session=?)",
        [json(session)?],
        |r| r.get(0),
    )?)
}
fn enabled(db: &Connection, scope: &str, generation: u64) -> Result<bool> {
    let value: Option<(u64, bool)> = db
        .query_row(
            "SELECT generation,enabled FROM scopes WHERE scope=?",
            [scope],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    Ok(value == Some((generation, true)))
}
impl Queue {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let db = Connection::open(path)?;
        db.busy_timeout(Duration::from_secs(5))?;
        db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;")?;
        let version: i64 = db.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if version > 1 {
            return Err(Error::Storage("unsupported queue schema version".into()));
        }
        db.execute_batch("BEGIN IMMEDIATE;
CREATE TABLE IF NOT EXISTS entries(seq INTEGER PRIMARY KEY AUTOINCREMENT,id TEXT UNIQUE NOT NULL,session TEXT NOT NULL,source TEXT NOT NULL,stable_id TEXT NOT NULL,state TEXT NOT NULL,body TEXT NOT NULL,UNIQUE(session,source,stable_id));
CREATE INDEX IF NOT EXISTS entries_session ON entries(session,seq);
CREATE TABLE IF NOT EXISTS scopes(scope TEXT PRIMARY KEY,generation INTEGER NOT NULL,enabled INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS readiness(session TEXT PRIMARY KEY,ready INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS holds(id TEXT PRIMARY KEY,session TEXT NOT NULL,owner TEXT NOT NULL,generation INTEGER NOT NULL,reason TEXT NOT NULL,UNIQUE(session,owner));
CREATE TABLE IF NOT EXISTS observations(seq INTEGER PRIMARY KEY AUTOINCREMENT,entry_id TEXT NOT NULL,kind TEXT NOT NULL,detail TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS fences(intent_id TEXT PRIMARY KEY,scope TEXT NOT NULL,generation INTEGER NOT NULL,body TEXT NOT NULL);
PRAGMA user_version=1; COMMIT;")?;
        Ok(Self { db })
    }
    /// Only the host activates a new scope generation after reconciling its previous generation.
    pub fn scope_state(&self, scope: &str) -> Result<Option<(u64, bool)>> {
        Ok(self
            .db
            .query_row(
                "SELECT generation,enabled FROM scopes WHERE scope=?",
                [scope],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?)
    }
    pub fn activate(&mut self, scope: &str, generation: u64) -> Result<()> {
        identifier(scope)?;
        if generation > i64::MAX as u64 {
            return Err(Error::InvalidArgument);
        }
        let tx = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let old: Option<(u64, bool)> = tx
            .query_row(
                "SELECT generation,enabled FROM scopes WHERE scope=?",
                [scope],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        if old == Some((generation, true)) {
            return Ok(());
        }
        if old.is_some_and(|(n, on)| generation <= n || on) {
            return Err(Error::RevisionConflict);
        }
        let mut s = tx.prepare("SELECT body FROM entries WHERE state NOT IN (?,?)")?;
        let rows = s
            .query_map(
                params![json(&State::Settled)?, json(&State::Withdrawn)?],
                |r| r.get::<_, String>(0),
            )?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        drop(s);
        for body in rows {
            if entry(body)?.scope == scope {
                return Err(Error::ReconciliationRequired);
            }
        }
        tx.execute("INSERT INTO scopes VALUES(?,?,1) ON CONFLICT(scope) DO UPDATE SET generation=excluded.generation,enabled=1",params![scope,generation])?;
        tx.commit()?;
        Ok(())
    }
    pub fn admit(
        &mut self,
        session: Session,
        source: &str,
        stable_id: &str,
        scope: &str,
        generation: u64,
        message: &str,
    ) -> Result<Entry> {
        identifier(&session.backend)?;
        identifier(&session.thread)?;
        identifier(source)?;
        identifier(stable_id)?;
        identifier(scope)?;
        text(message, 8192)?;
        let tx = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let old: Option<String> = tx
            .query_row(
                "SELECT body FROM entries WHERE session=? AND source=? AND stable_id=?",
                params![json(&session)?, source, stable_id],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(old) = old {
            let e = entry(old)?;
            if e.scope != scope || e.generation != generation || e.message != message {
                return Err(Error::IdempotencyConflict);
            }
            return Ok(e);
        }
        if !enabled(&tx, scope, generation)? {
            return Err(Error::InvalidState);
        }
        if list(&tx, &session)?.len() >= 100 {
            return Err(Error::LimitExceeded);
        }
        let mut e = Entry {
            seq: 0,
            id: id(),
            session,
            source: source.into(),
            stable_id: stable_id.into(),
            scope: scope.into(),
            generation,
            message: message.into(),
            state: State::Queued,
            dispatch_id: None,
            native_queue_id: None,
            native_turn_id: None,
        };
        tx.execute(
            "INSERT INTO entries(id,session,source,stable_id,state,body) VALUES(?,?,?,?,?,?)",
            params![
                e.id,
                json(&e.session)?,
                source,
                stable_id,
                json(&e.state)?,
                json(&e)?
            ],
        )?;
        e.seq = tx.last_insert_rowid();
        save(&tx, &e)?;
        tx.commit()?;
        Ok(e)
    }
    /// Host observation, not a prediction. A later competing turn is handled by native queue/add.
    pub fn set_ready(&mut self, session: &Session, ready: bool) -> Result<()> {
        self.db.execute("INSERT INTO readiness VALUES(?,?) ON CONFLICT(session) DO UPDATE SET ready=excluded.ready",params![json(session)?,ready])?;
        Ok(())
    }
    pub fn reserve(
        &mut self,
        session: &Session,
        owner: &str,
        generation: u64,
        reason: &str,
    ) -> Result<Reservation> {
        identifier(owner)?;
        text(reason, 2048)?;
        let tx = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let old: Option<(String, u64, String)> = tx
            .query_row(
                "SELECT id,generation,reason FROM holds WHERE session=? AND owner=?",
                params![json(session)?, owner],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?;
        if old.is_none() && holds(&tx, session)? {
            return Err(Error::Blocked);
        }
        let hold_id = if let Some((id, previous, previous_reason)) = old {
            if previous != generation || previous_reason != reason {
                return Err(Error::RevisionConflict);
            }
            id
        } else {
            let id = id();
            tx.execute(
                "INSERT INTO holds VALUES(?,?,?,?,?)",
                params![id, json(session)?, owner, generation, reason],
            )?;
            id
        };
        let mut pending = vec![];
        let mut unresolved = list(&tx, session)?;
        let mut stmt = tx.prepare("SELECT body FROM entries WHERE session=? AND state=?")?;
        let abandoned = stmt
            .query_map(params![json(session)?, json(&State::Abandoned)?], |r| {
                r.get::<_, String>(0)
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        drop(stmt);
        for body in abandoned {
            unresolved.push(entry(body)?);
        }
        for mut e in unresolved {
            if e.state == State::Authorized {
                e.state = State::Queued;
                e.dispatch_id = None;
                save(&tx, &e)?;
            } else if e.state != State::Queued {
                pending.push(e);
            }
        }
        tx.commit()?;
        Ok(Reservation {
            id: hold_id,
            granted: pending.is_empty(),
            pending,
        })
    }
    pub fn release(&mut self, reservation_id: &str, generation: u64) -> Result<bool> {
        Ok(self.db.execute(
            "DELETE FROM holds WHERE id=? AND generation=?",
            params![reservation_id, generation],
        )? == 1)
    }
    pub fn authorize_head(&mut self, session: &Session) -> Result<Option<Entry>> {
        let tx = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let ready: bool = tx
            .query_row(
                "SELECT ready FROM readiness WHERE session=?",
                [json(session)?],
                |r| r.get(0),
            )
            .optional()?
            .unwrap_or(false);
        if !ready || holds(&tx, session)? {
            return Ok(None);
        }
        let entries = list(&tx, session)?;
        if entries.iter().any(|e| e.state != State::Queued) {
            return Ok(None);
        }
        for mut e in entries {
            if !enabled(&tx, &e.scope, e.generation)? {
                e.state = State::Withdrawn;
                save(&tx, &e)?;
                continue;
            }
            e.state = State::Authorized;
            e.dispatch_id = Some(id());
            save(&tx, &e)?;
            tx.commit()?;
            return Ok(Some(e));
        }
        tx.commit()?;
        Ok(None)
    }
    /// The only permission to issue one native queue/add. Calling twice is an error, not replay.
    /// Release preparation when the host has not entered the native send path.
    pub fn return_authorized(&mut self, entry_id: &str, dispatch_id: &str) -> Result<Entry> {
        let tx = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut e = get(&tx, entry_id)?;
        if e.state != State::Authorized || e.dispatch_id.as_deref() != Some(dispatch_id) {
            return Err(Error::InvalidState);
        }
        e.state = State::Queued;
        e.dispatch_id = None;
        save(&tx, &e)?;
        tx.commit()?;
        Ok(e)
    }
    pub fn prepare_send(&mut self, entry_id: &str, dispatch_id: &str) -> Result<Entry> {
        let tx = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut e = get(&tx, entry_id)?;
        if e.state != State::Authorized || e.dispatch_id.as_deref() != Some(dispatch_id) {
            return Err(Error::InvalidState);
        }
        if holds(&tx, &e.session)? || !enabled(&tx, &e.scope, e.generation)? {
            return Err(Error::Blocked);
        }
        e.state = State::Sending;
        save(&tx, &e)?;
        tx.commit()?;
        Ok(e)
    }
    pub fn accepted(
        &mut self,
        entry_id: &str,
        dispatch_id: &str,
        native_queue_id: &str,
    ) -> Result<Entry> {
        identifier(native_queue_id)?;
        let tx = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut e = get(&tx, entry_id)?;
        if e.dispatch_id.as_deref() != Some(dispatch_id) {
            return Err(Error::InvalidState);
        }
        if e.native_queue_id
            .as_deref()
            .is_some_and(|id| id != native_queue_id)
        {
            return Err(Error::ReconciliationRequired);
        }
        if !matches!(
            e.state,
            State::Abandoned
                | State::Sending
                | State::Unknown
                | State::NativeQueued
                | State::Started
                | State::Settled
        ) {
            return Err(Error::InvalidState);
        }
        e.native_queue_id = Some(native_queue_id.into());
        if matches!(e.state, State::Sending | State::Unknown) {
            e.state = State::NativeQueued;
        }
        save(&tx, &e)?;
        tx.commit()?;
        Ok(e)
    }
    /// Host-correlated stable ID and turn receipt. State can arrive before the add response.
    pub fn started(
        &mut self,
        entry_id: &str,
        dispatch_id: &str,
        native_turn_id: &str,
    ) -> Result<Entry> {
        identifier(native_turn_id)?;
        let tx = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut e = get(&tx, entry_id)?;
        if e.dispatch_id.as_deref() != Some(dispatch_id)
            || !matches!(
                e.state,
                State::Abandoned
                    | State::Sending
                    | State::Unknown
                    | State::NativeQueued
                    | State::Started
                    | State::Settled
            )
        {
            return Err(Error::InvalidState);
        }
        if e.native_turn_id
            .as_deref()
            .is_some_and(|id| id != native_turn_id)
        {
            return Err(Error::ReconciliationRequired);
        }
        e.native_turn_id = Some(native_turn_id.into());
        if e.state != State::Settled {
            e.state = State::Started;
        }
        save(&tx, &e)?;
        tx.commit()?;
        Ok(e)
    }
    pub fn settled(&mut self, entry_id: &str, native_turn_id: &str) -> Result<()> {
        let tx = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut e = get(&tx, entry_id)?;
        if !matches!(e.state, State::Started | State::Settled)
            || e.native_turn_id.as_deref() != Some(native_turn_id)
        {
            return Err(Error::InvalidState);
        }
        e.state = State::Settled;
        save(&tx, &e)?;
        tx.commit()?;
        Ok(())
    }
    /// Only a positive, correlated native delete receipt permits this transition.
    pub fn withdrawn(&mut self, entry_id: &str, native_queue_id: &str) -> Result<()> {
        let tx = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut e = get(&tx, entry_id)?;
        if !matches!(
            e.state,
            State::NativeQueued | State::Unknown | State::Withdrawn | State::Abandoned
        ) || e.native_queue_id.as_deref() != Some(native_queue_id)
            || e.native_turn_id.is_some()
        {
            return Err(Error::ReconciliationRequired);
        }
        e.state = State::Withdrawn;
        save(&tx, &e)?;
        tx.commit()?;
        Ok(())
    }
    /// Call after taking exclusive worker ownership, never while another worker is sending.
    pub fn recover(&mut self) -> Result<()> {
        let tx = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut s = tx.prepare("SELECT body FROM entries WHERE state IN (?,?)")?;
        let rows = s
            .query_map(
                params![json(&State::Authorized)?, json(&State::Sending)?],
                |r| r.get::<_, String>(0),
            )?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        drop(s);
        for body in rows {
            let mut e = entry(body)?;
            if e.state == State::Authorized {
                e.state = State::Queued;
                e.dispatch_id = None;
            } else {
                e.state = State::Unknown;
            }
            save(&tx, &e)?;
        }
        // Stale idle observations are not authoritative after reconnect.
        tx.execute("UPDATE readiness SET ready=0", [])?;
        tx.commit()?;
        Ok(())
    }
    pub fn uncertain(&mut self, entry_id: &str, dispatch_id: &str) -> Result<()> {
        let tx = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut e = get(&tx, entry_id)?;
        if e.dispatch_id.as_deref() != Some(dispatch_id)
            || !matches!(
                e.state,
                State::Sending | State::NativeQueued | State::Unknown
            )
        {
            return Err(Error::InvalidState);
        }
        e.state = State::Unknown;
        save(&tx, &e)?;
        tx.commit()?;
        Ok(())
    }
    /// Only use with authoritative evidence that this exact RPC was never accepted.
    pub fn confirmed_not_submitted(
        &mut self,
        entry_id: &str,
        dispatch_id: &str,
        evidence: &str,
    ) -> Result<()> {
        text(evidence, 2048)?;
        let tx = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut e = get(&tx, entry_id)?;
        if e.dispatch_id.as_deref() != Some(dispatch_id)
            || !matches!(e.state, State::Sending | State::Unknown | State::Abandoned)
            || e.native_queue_id.is_some()
            || e.native_turn_id.is_some()
        {
            return Err(Error::ReconciliationRequired);
        }
        e.state = if enabled(&tx, &e.scope, e.generation)? {
            State::Queued
        } else {
            State::Withdrawn
        };
        e.dispatch_id = None;
        save(&tx, &e)?;
        tx.execute(
            "INSERT INTO observations(entry_id,kind,detail) VALUES(?,'confirmed_not_submitted',?)",
            params![entry_id, evidence],
        )?;
        tx.commit()?;
        Ok(())
    }
    /// Explicit USER reconciliation only. Releases queue ownership, NOT proof of native cancellation.
    /// Keep the unknown outcome and source fence; no automatic replay of the abandoned entry.
    pub fn abandon_unknown(
        &mut self,
        entry_id: &str,
        dispatch_id: &str,
        reason: &str,
    ) -> Result<()> {
        text(reason, 2048)?;
        let tx = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut e = get(&tx, entry_id)?;
        if e.dispatch_id.as_deref() != Some(dispatch_id)
            || e.state != State::Unknown
            || enabled(&tx, &e.scope, e.generation)?
        {
            return Err(Error::ReconciliationRequired);
        }
        e.state = State::Abandoned;
        save(&tx, &e)?;
        tx.execute(
            "INSERT INTO observations(entry_id,kind,detail) VALUES(?,'user_abandoned_unknown',?)",
            params![entry_id, reason],
        )?;
        tx.commit()?;
        Ok(())
    }
    pub fn fence(&mut self, scope: &str, generation: u64, intent_id: &str) -> Result<Fence> {
        identifier(intent_id)?;
        let tx = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let old: Option<(String, u64, String)> = tx
            .query_row(
                "SELECT scope,generation,body FROM fences WHERE intent_id=?",
                [intent_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?;
        if let Some((s, g, body)) = old {
            if s != scope || g != generation {
                return Err(Error::IdempotencyConflict);
            }
            return Ok(serde_json::from_str(&body)?);
        }
        if !enabled(&tx, scope, generation)? {
            return Err(Error::RevisionConflict);
        }
        tx.execute("UPDATE scopes SET enabled=0 WHERE scope=?", [scope])?;
        let mut s =
            tx.prepare("SELECT body FROM entries WHERE state NOT IN (?,?,?) ORDER BY seq")?;
        let rows = s
            .query_map(
                params![
                    json(&State::Settled)?,
                    json(&State::Withdrawn)?,
                    json(&State::Abandoned)?
                ],
                |r| r.get::<_, String>(0),
            )?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        drop(s);
        let mut entries = vec![];
        for body in rows {
            let mut e = entry(body)?;
            if e.scope != scope || e.generation != generation {
                continue;
            }
            if matches!(e.state, State::Queued | State::Authorized) {
                e.state = State::Withdrawn;
                save(&tx, &e)?;
            }
            entries.push(e);
        }
        let f = Fence {
            intent_id: intent_id.into(),
            entries,
        };
        tx.execute(
            "INSERT INTO fences VALUES(?,?,?,?)",
            params![intent_id, scope, generation, json(&f)?],
        )?;
        tx.commit()?;
        Ok(f)
    }
    pub fn get(&self, entry_id: &str) -> Result<Entry> {
        get(&self.db, entry_id)
    }
    pub fn pending(&self, session: &Session) -> Result<Vec<Entry>> {
        list(&self.db, session)
    }
}

/// Host-visible description of a persisted reservation, including pending reservations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReservationInfo {
    pub id: String,
    pub session: Session,
    pub owner: String,
    pub generation: u64,
    pub reason: String,
}
impl Queue {
    /// Withdraw work that provably has not reached prepare_send. A native delete requires a receipt.
    pub fn withdraw_local(&mut self, entry_id: &str) -> Result<Entry> {
        let tx = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut e = get(&tx, entry_id)?;
        match e.state {
            State::Queued | State::Authorized => {
                e.state = State::Withdrawn;
                save(&tx, &e)?;
            }
            State::Withdrawn => {}
            _ => return Err(Error::ReconciliationRequired),
        }
        tx.commit()?;
        Ok(e)
    }
    pub fn reservations(&self, session: &Session) -> Result<Vec<ReservationInfo>> {
        let mut s = self
            .db
            .prepare("SELECT id,owner,generation,reason FROM holds WHERE session=? ORDER BY id")?;
        let rows = s.query_map([json(session)?], |r| {
            Ok(ReservationInfo {
                id: r.get(0)?,
                session: session.clone(),
                owner: r.get(1)?,
                generation: r.get(2)?,
                reason: r.get(3)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }
    pub fn by_stable_id(
        &self,
        session: &Session,
        source: &str,
        stable_id: &str,
    ) -> Result<Option<Entry>> {
        self.db
            .query_row(
                "SELECT body FROM entries WHERE session=? AND source=? AND stable_id=?",
                params![json(session)?, source, stable_id],
                |r| r.get::<_, String>(0),
            )
            .optional()?
            .map(entry)
            .transpose()
    }
    /// Recovery inventory, including Abandoned entries whose native outcome is still unknown.
    pub fn all_pending(&self) -> Result<Vec<Entry>> {
        let mut s = self
            .db
            .prepare("SELECT body FROM entries WHERE state NOT IN (?,?) ORDER BY seq")?;
        let rows = s.query_map(
            params![json(&State::Settled)?, json(&State::Withdrawn)?],
            |r| r.get::<_, String>(0),
        )?;
        rows.map(|r| entry(r?)).collect()
    }
}
impl Queue {
    /// Re-enable a provably withdrawn input in a newer, already-active scope generation.
    /// The local ID and FIFO position stay stable. Native receipts belong to the old attempt.
    pub fn requeue_withdrawn(&mut self, entry_id: &str, generation: u64) -> Result<Entry> {
        let tx = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut e = get(&tx, entry_id)?;
        if e.state != State::Withdrawn || e.native_turn_id.is_some() {
            return Err(Error::InvalidState);
        }
        if generation <= e.generation || !enabled(&tx, &e.scope, generation)? {
            return Err(Error::RevisionConflict);
        }
        let prior = json(&(e.generation, &e.dispatch_id, &e.native_queue_id))?;
        tx.execute(
            "INSERT INTO observations(entry_id,kind,detail) VALUES(?,'requeued_withdrawn',?)",
            params![entry_id, prior],
        )?;
        e.generation = generation;
        e.dispatch_id = None;
        e.native_queue_id = None;
        e.native_turn_id = None;
        e.state = State::Queued;
        save(&tx, &e)?;
        tx.commit()?;
        Ok(e)
    }
}
