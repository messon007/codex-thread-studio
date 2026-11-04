//! Transactional Router ownership; a durable intent is never a retry instruction.
use super::Job;
use rusqlite::{params, Connection, OptionalExtension};
use std::{path::Path, time::Duration};
#[path = "native_router_search.rs"]
pub mod search;
fn db(path: &Path) -> Result<Connection, String> {
    let mut db = Connection::open(path).map_err(|e| e.to_string())?;
    db.busy_timeout(Duration::from_secs(3))
        .map_err(|e| e.to_string())?;
    db.execute_batch("PRAGMA journal_mode=WAL;
      CREATE TABLE IF NOT EXISTS native_router_jobs(id TEXT PRIMARY KEY,controller TEXT NOT NULL,created INTEGER NOT NULL,owner TEXT NOT NULL,phase TEXT NOT NULL,json TEXT NOT NULL,receipt TEXT NOT NULL DEFAULT '',unread INTEGER NOT NULL DEFAULT 0);
      CREATE INDEX IF NOT EXISTS native_router_jobs_page ON native_router_jobs(controller);
      CREATE TABLE IF NOT EXISTS native_router_claims(session_key TEXT PRIMARY KEY,job_id TEXT NOT NULL);
      CREATE TABLE IF NOT EXISTS router_dispatches(controller TEXT NOT NULL,turn_key TEXT NOT NULL,dispatch TEXT NOT NULL,PRIMARY KEY(controller,turn_key));
      CREATE INDEX IF NOT EXISTS router_dispatches_page ON router_dispatches(controller);").map_err(|e|e.to_string())?;
    // Migrate older native job records once. The payload itself remains intact.
    let columns = {
        let mut statement = db
            .prepare("PRAGMA table_info(native_router_jobs)")
            .map_err(|e| e.to_string())?;
        let names = statement
            .query_map([], |row| row.get::<_, String>(1))
            .map_err(|e| e.to_string())?;
        names
            .collect::<Result<std::collections::HashSet<_>, _>>()
            .map_err(|e| e.to_string())?
    };
    if !columns.contains("receipt") || !columns.contains("unread") {
        let tx = db.transaction().map_err(|e| e.to_string())?;
        if !columns.contains("receipt") {
            tx.execute_batch("ALTER TABLE native_router_jobs ADD COLUMN receipt TEXT NOT NULL DEFAULT ''; UPDATE native_router_jobs SET receipt=lower(hex(randomblob(16)));").map_err(|e|e.to_string())?;
        }
        if !columns.contains("unread") {
            tx.execute_batch("ALTER TABLE native_router_jobs ADD COLUMN unread INTEGER NOT NULL DEFAULT 0; UPDATE native_router_jobs SET unread=CASE WHEN json_valid(json) THEN COALESCE(json_extract(json,'$.unread'),0) ELSE 0 END;").map_err(|e|e.to_string())?;
        }
        tx.commit().map_err(|e| e.to_string())?;
    }
    db.execute_batch("CREATE INDEX IF NOT EXISTS native_router_unread ON native_router_jobs(controller,unread); CREATE INDEX IF NOT EXISTS native_router_active ON native_router_jobs(controller,phase);").map_err(|e|e.to_string())?;
    db.execute_batch("CREATE INDEX IF NOT EXISTS native_router_source ON native_router_jobs(json_extract(json,'$.target.sessionKey'),json_extract(json,'$.targetTurnId')) WHERE json_valid(json);").map_err(|e|e.to_string())?;
    Ok(db)
}
fn write(db: &Connection, job: &Job, owner: &str, insert: bool) -> Result<(), String> {
    write_transition(db, job, owner, insert, None)
}
fn write_transition(
    db: &Connection,
    job: &Job,
    owner: &str,
    insert: bool,
    previous: Option<&str>,
) -> Result<(), String> {
    let mut value = serde_json::to_value(job).map_err(|e| e.to_string())?;
    let mut expected = String::new();
    if !insert {
        let raw: String = db
            .query_row(
                "SELECT json FROM native_router_jobs WHERE id=?1 AND owner=?2",
                params![job.id, owner],
                |row| row.get(0),
            )
            .map_err(|_| "Router delivery ownership changed; request was not replayed")?;
        let mut stored: serde_json::Value =
            serde_json::from_str(&raw).map_err(|e| e.to_string())?;
        expected = stored["targetTurnId"]
            .as_str()
            .ok_or("Invalid stored Router target identity")?
            .into();
        let valid = match previous {
            Some(previous) => expected == previous,
            None => expected.is_empty() || expected == job.target_turn_id,
        };
        if !valid {
            return Err(
                "Router target turn changed; the older worker cannot overwrite its handoff".into(),
            );
        }
        if stored["stopRequested"] == true {
            value["stopRequested"] = true.into();
        }
        let known = value.as_object().ok_or("Invalid Router record")?;
        let fields = stored
            .as_object_mut()
            .ok_or("Invalid stored Router record")?;
        fields.extend(
            known
                .iter()
                .map(|(key, value)| (key.clone(), value.clone())),
        );
        value = stored;
    }
    let json = serde_json::to_string(&value).map_err(|e| e.to_string())?;
    if json.len() > 2 * 1024 * 1024 {
        return Err("Router delivery record exceeds 2 MiB".into());
    }
    if insert {
        db.execute(
            "INSERT INTO native_router_jobs(id,controller,created,owner,phase,json,receipt,unread) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
            params![
                job.id,
                job.controller,
                job.created_at,
                owner,
                job.status,
                json, uuid::Uuid::new_v4().simple().to_string(), job.unread
            ],
        )
        .map_err(|e| e.to_string())?;
    } else {
        let changed = db
            .execute(
                "UPDATE native_router_jobs SET json=?3,phase=?4,receipt=?5,unread=?6 WHERE id=?1 AND owner=?2 AND COALESCE(json_extract(json,'$.targetTurnId'),'')=?7",
                params![job.id, owner, json, job.status, uuid::Uuid::new_v4().simple().to_string(),job.unread,expected],
            )
            .map_err(|e| e.to_string())?;
        if changed != 1 {
            return Err("Router delivery ownership changed; request was not replayed".into());
        }
    }
    // Shared history keeps the exact delivery identities without duplicating
    // source text/base64 or exceeding the existing browser record budget.
    let question = job
        .supervision
        .as_ref()
        .map(|s| s.prompt.as_str())
        .unwrap_or("");
    let decision=job.decision.as_ref().map(|value|serde_json::json!({"action":value.action,"targetSessionKey":value.target_session_key,"forwardedPrompt":question,"reason":value.reason,"message":value.message}));
    let mirror = serde_json::json!({"status":job.status,"decision":decision,"targetTurnId":job.target_turn_id,"controllerTurnId":job.controller_turn_id,"nativeJobId":job.id,"requestedAt":job.created_at,"unread":job.unread,"error":job.error});
    let key = format!(
        "{}:native-{}",
        job.controller
            .split_once(':')
            .ok_or("Invalid controller")?
            .0,
        job.id
    );
    db.execute("INSERT INTO router_dispatches(controller,turn_key,dispatch) VALUES(?1,?2,?3) ON CONFLICT(controller,turn_key) DO UPDATE SET dispatch=excluded.dispatch",params![job.controller,key,mirror.to_string()]).map_err(|e|e.to_string())?;
    if job.status == "running" {
        db.execute(
            "DELETE FROM native_router_claims WHERE job_id=?1 AND session_key=?2",
            params![job.id, job.controller],
        )
        .map_err(|e| e.to_string())?;
    }
    if job.terminal() {
        db.execute(
            "DELETE FROM native_router_claims WHERE job_id=?1",
            [&job.id],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}
pub fn owned(path: &Path, id: &str, owner: &str) -> Result<Option<Job>, String> {
    let raw: Option<String> = db(path)?
        .query_row(
            "SELECT json FROM native_router_jobs WHERE id=?1 AND owner=?2",
            params![id, owner],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    raw.map(|raw| serde_json::from_str(&raw).map_err(|e| e.to_string()))
        .transpose()
}
pub fn source_job(
    path: &Path,
    key: &str,
    previous: &str,
    next: &str,
) -> Result<Option<Job>, String> {
    let db = db(path)?;
    let mut statement=db.prepare("SELECT json FROM native_router_jobs WHERE json_valid(json) AND json_extract(json,'$.target.sessionKey')=?1 AND json_extract(json,'$.targetTurnId') IN (?2,?3) AND (json_extract(json,'$.targetTurnId')=?2 OR json_extract(json,'$.supervision.previousTurnId')=?2) ORDER BY rowid DESC LIMIT 2").map_err(|e|e.to_string())?;
    let rows = statement
        .query_map(params![key, previous, next], |row| row.get::<_, String>(0))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    if rows.len() > 1 {
        return Err("More than one routed request claims this source turn".into());
    }
    rows.into_iter()
        .next()
        .map(|raw| serde_json::from_str(&raw).map_err(|e| e.to_string()))
        .transpose()
}
struct SourceDetails {
    id: String,
    controller: String,
    ready: bool,
}
/// Indexed metadata only; the reply and its rich source items stay on disk.
fn source_details(
    path: &Path,
    key: &str,
    turn: &str,
    cwd: &str,
) -> Result<Option<SourceDetails>, String> {
    let mut connection = db(path)?;
    let db = connection.transaction().map_err(|e| e.to_string())?;
    let mut statement=db.prepare("SELECT id,phase,json_extract(json,'$.target.cwd'),json_extract(json,'$.target.backend')||':'||json_extract(json,'$.target.id'),COALESCE(json_extract(json,'$.stopRequested'),0),json_type(json,'$.mapScope'),controller FROM native_router_jobs WHERE json_valid(json) AND json_extract(json,'$.target.sessionKey')=?1 AND json_extract(json,'$.targetTurnId')=?2 LIMIT 2").map_err(|e|e.to_string())?;
    let rows = statement
        .query_map(params![key, turn], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, bool>(4)?,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, String>(6)?,
            ))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    if rows.len() > 1 {
        return Err("More than one routed request claims this source turn".into());
    }
    let claim: Option<String> = db
        .query_row(
            "SELECT job_id FROM native_router_claims WHERE session_key=?1",
            [key],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    let Some((id, phase, root, identity, stopped, map, controller)) = rows.into_iter().next()
    else {
        return if claim.is_some() {
            Err("Another or unconfirmed routed request owns this source".into())
        } else {
            Ok(None)
        };
    };
    if root != cwd || identity != key || claim.as_ref().is_some_and(|claimed| claimed != &id) {
        return Err("Router source identity or ownership changed".into());
    }
    if stopped {
        return Err("Router stop was requested; supervision cannot continue".into());
    }
    if !crate::valid_router_session_key(&controller)
        || controller.len() > 320
        || uuid::Uuid::parse_str(&id).is_err()
    {
        return Err("Invalid routed controller identity".into());
    }
    if map
        .as_deref()
        .is_some_and(|kind| !matches!(kind, "object" | "null"))
    {
        return Err("Invalid routed Map scope".into());
    }
    Ok(Some(SourceDetails {
        id,
        controller,
        ready: phase != "running" || map.as_deref() != Some("object"),
    }))
}
pub fn source_ready(path: &Path, key: &str, turn: &str, cwd: &str) -> Result<(bool, bool), String> {
    Ok(source_details(path, key, turn, cwd)?.map_or((false, true), |scope| (true, scope.ready)))
}
pub fn source_scope(
    path: &Path,
    key: &str,
    turn: &str,
    cwd: &str,
) -> Result<Option<serde_json::Value>, String> {
    Ok(source_details(path, key, turn, cwd)?
        .map(|scope| serde_json::json!({"id":scope.id,"controller":scope.controller})))
}
pub fn follow(
    path: &Path,
    owner: &str,
    id: &str,
    key: &str,
    cwd: &str,
    previous: &str,
    next: &str,
    prompt: &str,
    stop_requested: bool,
) -> Result<Job, String> {
    if previous.is_empty()
        || next.is_empty()
        || previous == next
        || previous.len() > 256
        || next.len() > 256
        || previous.chars().chain(next.chars()).any(char::is_control)
        || !prompt.starts_with("[Studio turn supervisor]\n")
        || prompt.len() > 16 * 1024 + 64
    {
        return Err("Invalid supervised source receipt".into());
    }
    let mut db = db(path)?;
    let tx = db.transaction().map_err(|e| e.to_string())?;
    let raw: String = tx
        .query_row(
            "SELECT json FROM native_router_jobs WHERE id=?1",
            [id],
            |row| row.get(0),
        )
        .map_err(|_| "Router request is no longer available")?;
    let mut job: Job = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
    let target = job.target.as_ref().ok_or("Router source has no target")?;
    if target.session_key != key
        || format!("{}:{}", target.backend, target.id) != key
        || target.cwd != cwd
    {
        return Err("Router source identity or project changed".into());
    }
    if job.target_turn_id == next
        && job
            .supervision
            .as_ref()
            .is_some_and(|s| s.previous_turn_id == previous && s.prompt == prompt)
    {
        return Ok(job);
    }
    if job.target_turn_id != previous
        || !matches!(
            job.status.as_str(),
            "running" | "completed" | "failed" | "stopped"
        )
    {
        return Err("The routed turn changed before its supervised receipt was recorded".into());
    }
    let claim: Option<String> = tx
        .query_row(
            "SELECT job_id FROM native_router_claims WHERE session_key=?1",
            [key],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    if claim.as_ref().is_some_and(|claimed| claimed != id) {
        return Err("Another routed request owns this source session".into());
    }
    tx.execute(
        "UPDATE native_router_jobs SET owner=?2 WHERE id=?1",
        params![id, owner],
    )
    .map_err(|e| e.to_string())?;
    tx.execute(
        "INSERT OR IGNORE INTO native_router_claims VALUES(?1,?2)",
        params![key, id],
    )
    .map_err(|e| e.to_string())?;
    job.target_turn_id = next.into();
    job.status = "running".into();
    job.response.clear();
    job.response_items.clear();
    job.context_excerpt.clear();
    job.error.clear();
    job.unread = false;
    job.map_scope = None;
    job.map_notice.clear();
    job.stop_requested |= stop_requested;
    job.supervision = Some(super::Supervision {
        previous_turn_id: previous.into(),
        prompt: prompt.into(),
    });
    write_transition(&tx, &job, owner, false, Some(previous))?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(job)
}
pub fn note_stop(path: &Path, id: &str, owner: &str) -> Result<(), String> {
    let changed=db(path)?.execute("UPDATE native_router_jobs SET json=json_set(json,'$.stopRequested',json('true')) WHERE id=?1 AND owner=?2",params![id,owner]).map_err(|e|e.to_string())?;
    if changed != 1 {
        return Err("Router stop ownership changed".into());
    }
    Ok(())
}
#[cfg(test)]
pub fn reserve(path: &Path, job: &Job, owner: &str) -> Result<(), String> {
    reserve_queued(path, job, owner, None)
}
pub fn reserve_queued(
    path: &Path,
    job: &Job,
    owner: &str,
    queued: Option<(&str, &[serde_json::Value])>,
) -> Result<(), String> {
    let mut db = db(path)?;
    let tx = db.transaction().map_err(|e| e.to_string())?;
    if let Some((id, input)) = queued {
        let raw: String = tx
            .query_row(
                "SELECT queue_json FROM session_message_queues WHERE session_key=?1",
                [&job.controller],
                |row| row.get(0),
            )
            .map_err(|_| "Queued Router message no longer exists")?;
        let mut messages: Vec<serde_json::Value> =
            serde_json::from_str(&raw).map_err(|_| "Invalid Router queue")?;
        let first = messages
            .first()
            .cloned()
            .map(serde_json::from_value::<crate::session_state::QueuedMessage>)
            .transpose()
            .map_err(|_| "Invalid Router queue head")?;
        if id != job.id
            || first.as_ref().is_none_or(|message| {
                message.id != id || message.text != job.prompt || message.input != input
            })
        {
            return Err("Queued Router message changed; reload the queue before sending".into());
        }
        messages.remove(0);
        if messages.is_empty() {
            tx.execute(
                "DELETE FROM session_message_queues WHERE session_key=?1",
                [&job.controller],
            )
            .map_err(|e| e.to_string())?;
        } else {
            tx.execute(
                "UPDATE session_message_queues SET queue_json=?2 WHERE session_key=?1",
                params![
                    job.controller,
                    serde_json::to_string(&messages).map_err(|e| e.to_string())?
                ],
            )
            .map_err(|e| e.to_string())?;
        }
    }
    tx.execute(
        "INSERT INTO native_router_claims VALUES(?1,?2)",
        params![job.controller, job.id],
    )
    .map_err(|_| "This Router has an active or unreconciled request".to_owned())?;
    write(&tx, job, owner, true)?;
    tx.commit().map_err(|e| e.to_string())
}
pub fn status(path: &Path, controller: &str, id: &str) -> Result<serde_json::Value, String> {
    let status: Option<(String,String)> = db(path)?
        .query_row(
            "SELECT phase,COALESCE(json_extract(json,'$.targetTurnId'),'') FROM native_router_jobs WHERE controller=?1 AND id=?2",
            params![controller, id],
            |row| Ok((row.get(0)?,row.get(1)?)),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    Ok(
        serde_json::json!({"controller":controller,"id":id,"status":status.as_ref().map(|row|&row.0),"targetTurnId":status.as_ref().map(|row|row.1.as_str()).unwrap_or("")}),
    )
}
pub fn save(path: &Path, job: &Job, owner: &str, target_claim: bool) -> Result<(), String> {
    let mut db = db(path)?;
    let tx = db.transaction().map_err(|e| e.to_string())?;
    if target_claim {
        let target = job.target.as_ref().ok_or("Missing Router target")?;
        let previous: Option<String> = tx
            .query_row(
                "SELECT job_id FROM native_router_claims WHERE session_key=?1",
                [&target.session_key],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        if previous.as_ref().is_some_and(|value| value != &job.id) {
            return Err("The target session already has a routed request".into());
        }
        tx.execute(
            "INSERT OR IGNORE INTO native_router_claims VALUES(?1,?2)",
            params![target.session_key, job.id],
        )
        .map_err(|e| e.to_string())?;
    }
    write(&tx, job, owner, false)?;
    tx.commit().map_err(|e| e.to_string())
}
#[cfg(test)]
pub fn read(path: &Path, id: &str) -> Result<Job, String> {
    find(path, id)?.ok_or_else(|| "Router request was not found".into())
}
pub fn find(path: &Path, id: &str) -> Result<Option<Job>, String> {
    let json: Option<String> = db(path)?
        .query_row(
            "SELECT json FROM native_router_jobs WHERE id=?1",
            [id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    json.map(|json| serde_json::from_str(&json).map_err(|e| e.to_string()))
        .transpose()
}
/// Bounded lookback and SQL projection avoid loading full reply trees or the
/// entire native history just to resolve a follow-up reference.
pub fn recent(
    path: &Path,
    controller: &str,
) -> Result<Vec<crate::native_router_plan::Recent>, String> {
    let db = db(path)?;
    let mut statement=db.prepare("SELECT json_extract(json,'$.target.sessionKey'),json_extract(json,'$.target.cwd'),json_extract(json,'$.targetTurnId'),substr(json_extract(json,'$.prompt'),-1000),substr(COALESCE(NULLIF(json_extract(json,'$.contextExcerpt'),''),json_extract(json,'$.response')),-4000) FROM native_router_jobs WHERE controller=?1 AND phase='completed' AND json_extract(json,'$.controller')=?1 AND json_extract(json,'$.status')='completed' AND COALESCE(json_extract(json,'$.targetTurnId'),'')!='' ORDER BY rowid DESC LIMIT 32").map_err(|e|e.to_string())?;
    let rows = statement
        .query_map([controller], |row| {
            Ok(crate::native_router_plan::Recent {
                session_key: row.get(0)?,
                cwd: row.get(1)?,
                target_turn_id: row.get(2)?,
                request_excerpt: row.get(3)?,
                response_excerpt: row.get(4)?,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}
#[cfg(test)]
pub fn list(
    path: &Path,
    controller: &str,
    before: Option<i64>,
) -> Result<Vec<serde_json::Value>, String> {
    list_range(path, controller, before, None)
}
pub fn list_range(
    path: &Path,
    controller: &str,
    before: Option<i64>,
    after: Option<i64>,
) -> Result<Vec<serde_json::Value>, String> {
    let db = db(path)?;
    list_range_on(&db, controller, before, after, false)
}
fn list_range_on(
    db: &Connection,
    controller: &str,
    before: Option<i64>,
    after: Option<i64>,
    filtered: bool,
) -> Result<Vec<serde_json::Value>, String> {
    let filter = if filtered {
        " AND studio_router_contains(json)"
    } else {
        ""
    };
    let sql = if after.is_some() {
        format!("SELECT rowid,json,receipt FROM native_router_jobs WHERE controller=?1 AND (?2 IS NULL OR rowid<?2) AND rowid>?3{filter} ORDER BY rowid ASC LIMIT 21")
    } else {
        format!("SELECT rowid,json,receipt FROM native_router_jobs WHERE controller=?1 AND (?2 IS NULL OR rowid<?2) AND (?3 IS NULL OR rowid>?3){filter} ORDER BY rowid DESC LIMIT 21")
    };
    let mut statement = db.prepare(&sql).map_err(|e| e.to_string())?;
    let rows = statement
        .query_map(params![controller, before, after], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(|e| e.to_string())?;
    let mut values:Vec<serde_json::Value>=rows.map(|row|{let (cursor,json,receipt)=row.map_err(|e|e.to_string())?;let job:Job=serde_json::from_str(&json).map_err(|e|e.to_string())?;Ok(serde_json::json!({"cursor":cursor,"receipt":receipt,"id":job.id,"controller":job.controller,"createdAt":job.created_at,"status":job.status,"target":job.target.as_ref().map(|target|serde_json::json!({"sessionKey":target.session_key,"title":target.title,"cwd":target.cwd})),"unread":job.unread,"error":job.error}))}).collect::<Result<_,String>>()?;
    if after.is_some() {
        if values.len() > 20 {
            values.pop();
        }
        values.reverse();
    }
    Ok(values)
}
pub fn neighbors(
    path: &Path,
    controller: &str,
    first: Option<i64>,
    last: Option<i64>,
) -> Result<(bool, bool), String> {
    let db = db(path)?;
    neighbors_on(&db, controller, first, last, false)
}
fn neighbors_on(
    db: &Connection,
    controller: &str,
    first: Option<i64>,
    last: Option<i64>,
    filtered: bool,
) -> Result<(bool, bool), String> {
    let filter = if filtered {
        " AND studio_router_contains(json)"
    } else {
        ""
    };
    db.query_row(&format!("SELECT EXISTS(SELECT 1 FROM native_router_jobs WHERE controller=?1 AND rowid<?2{filter}),EXISTS(SELECT 1 FROM native_router_jobs WHERE controller=?1 AND rowid>?3{filter})"),params![controller,last,first],|row|Ok((row.get(0)?,row.get(1)?))).map_err(|e|e.to_string())
}
pub fn view(path: &Path, id: &str) -> Result<serde_json::Value, String> {
    let db = db(path)?;
    let (raw, receipt): (String, String) = db
        .query_row(
            "SELECT json,receipt FROM native_router_jobs WHERE id=?1",
            [id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|_| "Router request was not found".to_string())?;
    let mut value: serde_json::Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
    value
        .as_object_mut()
        .ok_or("Invalid Router record")?
        .insert("receipt".into(), receipt.into());
    Ok(value)
}
pub fn summary(path: &Path, controller: &str) -> Result<serde_json::Value, String> {
    let db = db(path)?;
    summary_on(&db, controller)
}
fn summary_on(db: &Connection, controller: &str) -> Result<serde_json::Value, String> {
    let unread: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM native_router_jobs WHERE controller=?1 AND unread!=0",
            [controller],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    let mut statement=db.prepare("SELECT id FROM native_router_jobs WHERE controller=?1 AND phase IN ('routing','dispatching','running') ORDER BY rowid DESC LIMIT 5").map_err(|e|e.to_string())?;
    let ids = statement
        .query_map([controller], |row| row.get::<_, String>(0))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    if ids.len() > 4 {
        return Err("Too many active Router records".into());
    }
    let busy:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM native_router_jobs WHERE controller=?1 AND phase IN ('routing','dispatching'))",[controller],|row|row.get(0)).map_err(|e|e.to_string())?;
    Ok(serde_json::json!({"unreadCount":unread,"activeIds":ids,"controllerBusy":busy}))
}
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Receipt {
    pub id: String,
    pub receipt: String,
}
/// Confirm only the exact terminal revision seen by this client. Never rewrite
/// a job from an older UI snapshot or touch another controller's record.
pub fn acknowledge(
    path: &Path,
    controller: &str,
    seen: &[Receipt],
) -> Result<Vec<Receipt>, String> {
    let mut db = db(path)?;
    let tx = db.transaction().map_err(|e| e.to_string())?;
    let mut acknowledged = Vec::new();
    for seen in seen {
        let row: Option<(String, String)> = tx
            .query_row(
                "SELECT json,receipt FROM native_router_jobs WHERE id=?1 AND controller=?2",
                params![seen.id, controller],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        let Some((raw, revision)) = row else {
            continue;
        };
        if revision != seen.receipt {
            continue;
        }
        let job: Job = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
        if !job.terminal() {
            continue;
        }
        tx.execute("UPDATE native_router_jobs SET unread=0,json=json_set(json,'$.unread',json('false')) WHERE id=?1 AND controller=?2 AND receipt=?3",params![seen.id,controller,seen.receipt]).map_err(|e|e.to_string())?;
        let backend = controller
            .split_once(':')
            .ok_or("Invalid Router controller")?
            .0;
        tx.execute("UPDATE router_dispatches SET dispatch=json_set(dispatch,'$.unread',json('false')) WHERE controller=?1 AND turn_key=?2",params![controller,format!("{backend}:native-{}",seen.id)]).map_err(|e|e.to_string())?;
        acknowledged.push(seen.clone());
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok(acknowledged)
}
pub fn recover(path: &Path, owner: &str) -> Result<Vec<Job>, String> {
    let mut db = db(path)?;
    let tx = db.transaction().map_err(|e| e.to_string())?;
    let jobs = {
        let mut statement=tx.prepare("SELECT json FROM native_router_jobs WHERE owner!=?1 AND phase IN ('routing','dispatching','running') LIMIT 5").map_err(|e|e.to_string())?;
        let rows = statement
            .query_map([owner], |row| row.get::<_, String>(0))
            .map_err(|e| e.to_string())?;
        rows.map(|row| {
            serde_json::from_str::<Job>(&row.map_err(|e| e.to_string())?).map_err(|e| e.to_string())
        })
        .collect::<Result<Vec<_>, _>>()?
    };
    if jobs.len() > 4 {
        return Err("Too many unfinished Router requests to recover safely".into());
    }
    let mut running = Vec::new();
    for mut job in jobs {
        tx.execute(
            "UPDATE native_router_jobs SET owner=?2 WHERE id=?1",
            params![job.id, owner],
        )
        .map_err(|e| e.to_string())?;
        if job.status == "running" && !job.target_turn_id.is_empty() && job.target.is_some() {
            running.push(job);
        } else {
            job.status = "failed".into();
            job.unread = true;
            job.error="Router delivery was interrupted before its identity was confirmed. Check the backend conversation; the request was not replayed.".into();
            write(&tx, &job, owner, false)?;
        }
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok(running)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn persisted_search_is_literal_unicode_scoped_and_keeps_live_metadata() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("studio.sqlite3");
        let db = db(&path).unwrap();
        let mut identities = Vec::new();
        for index in 0..45 {
            let mut entry = job("codex:router");
            entry.status = "completed".into();
            entry.prompt = format!("Request {index} ÉTÉ []%_.* ");
            entry.fingerprint = "PRIVATE_FINGERPRINT_NOT_SEARCHABLE".into();
            entry.unread = index == 0 || index == 40;
            if index == 0 {
                entry
                    .response_items
                    .push(crate::native_router_backend::ResponseItem {
                        id: "source".into(),
                        text: format!("{} source-only中文😀", "x".repeat(24 * 1024)),
                        excerpt: false,
                    });
                entry.target = Some(target());
                entry.context_excerpt = "stored tail-only中文😀".into();
                entry.target.as_mut().unwrap().title = "Target architecture".into();
            }
            write(&db, &entry, "fixture", true).unwrap();
            identities.push(entry.id);
        }
        let mut other = job("opencode:router");
        other.status = "completed".into();
        other.prompt = "ÉTÉ another controller".into();
        write(&db, &other, "fixture", true).unwrap();
        let mut live = job("codex:router");
        live.status = "running".into();
        live.prompt = "Unrelated live job".into();
        write(&db, &live, "fixture", true).unwrap();
        let search = |needle: &str, sensitive, before, after| {
            search::page(&path, "codex:router", needle, sensitive, before, after).unwrap()
        };
        let first = search("été", false, None, None);
        assert_eq!(first["data"].as_array().unwrap().len(), 20);
        assert_eq!(first["unreadCount"], 2);
        assert_eq!(first["activeIds"], serde_json::json!([live.id]));
        assert_eq!(first["controllerBusy"], false);
        assert_eq!(first["newerCursor"], serde_json::Value::Null);
        assert_eq!(
            search("été", true, None, None)["data"],
            serde_json::json!([])
        );
        assert_eq!(search("[]%_.*", false, None, None)["data"], first["data"]);
        assert_eq!(
            search("ÉTÉ []%_.*  ", false, None, None)["data"],
            serde_json::json!([]),
            "Trailing whitespace was trimmed"
        );
        assert_eq!(
            search("PRIVATE_FINGERPRINT_NOT_SEARCHABLE", false, None, None)["data"],
            serde_json::json!([])
        );
        let tail = search("tail-only中文😀", false, None, None);
        assert_eq!(tail["data"][0]["id"], identities[0]);
        assert_eq!(
            search("source-only中文😀", false, None, None)["data"][0]["id"],
            identities[0]
        );
        assert_eq!(
            search("Target architecture", false, None, None)["data"][0]["id"],
            identities[0]
        );
        let middle = search("été", false, first["nextCursor"].as_i64(), None);
        let last = search("été", false, middle["nextCursor"].as_i64(), None);
        assert_eq!(last["data"].as_array().unwrap().len(), 5);
        assert!(last["nextCursor"].is_null());
        let newer = search("été", false, None, last["newerCursor"].as_i64());
        assert_eq!(newer["data"], middle["data"]);
        let mut found = std::collections::HashSet::new();
        for row in [first, middle, last]
            .iter()
            .flat_map(|p| p["data"].as_array().unwrap())
        {
            assert!(found.insert(row["id"].as_str().unwrap().to_owned()));
        }
        assert_eq!(found, identities.into_iter().collect());
        assert_eq!(
            view(&path, &other.id).unwrap()["prompt"],
            "ÉTÉ another controller"
        );
        assert!(search::page_with_budget(
            &path,
            "codex:router",
            "été",
            false,
            None,
            None,
            Duration::ZERO
        )
        .unwrap_err()
        .contains("budget"));
        assert_eq!(summary(&path, "codex:router").unwrap()["unreadCount"], 2);
        for invalid in ["x".repeat(513), "中文".repeat(257), "nul\0input".into()] {
            assert!(search::validate(&invalid).is_err());
        }
        assert!(search::validate("literal\tline\nwith space ").is_ok());
    }
    #[test]
    fn queue_head_and_delivery_intent_commit_together_and_rollback_on_conflict() {
        let path = std::env::temp_dir().join(format!(
            "studio-router-queue-{}.sqlite3",
            uuid::Uuid::new_v4()
        ));
        let mut first = job("codex:router");
        let second = job("codex:router");
        let input = vec![
            serde_json::json!({"type":"file","path":"说明 note.md","root":"/project"}),
            serde_json::json!({"type":"skill","name":"build","path":"/skills/build/SKILL.md"}),
        ];
        let messages = vec![
            crate::session_state::QueuedMessage {
                id: first.id.clone(),
                text: first.prompt.clone(),
                input: input.clone(),
                created_at: 1,
            },
            crate::session_state::QueuedMessage {
                id: second.id.clone(),
                text: second.prompt.clone(),
                input: vec![],
                created_at: 2,
            },
        ];
        crate::session_state::replace_message_queue(&path, &first.controller, &messages).unwrap();
        let mut extended = serde_json::to_value(&messages).unwrap();
        extended[1]["futureExtension"] = serde_json::json!({"keep":42});
        db(&path)
            .unwrap()
            .execute(
                "UPDATE session_message_queues SET queue_json=?2 WHERE session_key=?1",
                params![first.controller, extended.to_string()],
            )
            .unwrap();
        let mut stale = first.clone();
        stale.prompt = "Changed after enqueue".into();
        assert!(reserve_queued(&path, &stale, "owner", Some((&stale.id, &input))).is_err());
        let mut changed = input.clone();
        changed[0]["root"] = "/other-project".into();
        for wrong in [vec![], changed, input.iter().rev().cloned().collect()] {
            assert!(reserve_queued(&path, &first, "owner", Some((&first.id, &wrong))).is_err());
        }
        assert!(find(&path, &first.id).unwrap().is_none());
        assert_eq!(
            crate::session_state::load_message_queue(&path, &first.controller).unwrap(),
            messages
        );
        reserve_queued(&path, &first, "owner", Some((&first.id, &input))).unwrap();
        assert_eq!(
            crate::session_state::load_message_queue(&path, &first.controller)
                .unwrap()
                .len(),
            1
        );
        assert!(find(&path, &first.id).unwrap().is_some());
        assert_eq!(db(&path).unwrap().query_row("SELECT json_extract(queue_json,'$[0].futureExtension.keep') FROM session_message_queues WHERE session_key=?1",[&first.controller],|row|row.get::<_,i64>(0)).unwrap(),42);
        assert!(reserve_queued(&path, &second, "owner", Some((&second.id, &[]))).is_err());
        assert_eq!(
            crate::session_state::load_message_queue(&path, &first.controller).unwrap()[0].id,
            second.id
        );
        assert!(find(&path, &second.id).unwrap().is_none());
        assert!(status(&path, "opencode:router", &first.id).unwrap()["status"].is_null());
        assert_eq!(
            status(&path, &first.controller, &first.id).unwrap()["status"],
            "routing"
        );
        first.status = "completed".into();
        save(&path, &first, "owner", false).unwrap();
        reserve_queued(&path, &second, "owner", Some((&second.id, &[]))).unwrap();
        assert!(
            crate::session_state::load_message_queue(&path, &first.controller)
                .unwrap()
                .is_empty()
        );
        drop(std::fs::remove_file(path));
    }
    fn job(controller: &str) -> Job {
        Job {
            supervision: None,
            stop_requested: false,
            map_scope: None,
            map_notice: String::new(),
            dispatch_notice: String::new(),
            reference_labels: Vec::new(),
            image_count: 0,
            id: uuid::Uuid::new_v4().to_string(),
            fingerprint: "request-fingerprint".into(),
            catalog_warnings: Vec::new(),
            controller: controller.into(),
            created_at: 1,
            prompt: "User query".into(),
            status: "routing".into(),
            controller_turn_id: String::new(),
            target: None,
            target_turn_id: String::new(),
            decision: None,
            response: String::new(),
            context_excerpt: String::new(),
            response_items: Vec::new(),
            error: String::new(),
            unread: false,
        }
    }
    fn target() -> crate::native_router_plan::Candidate {
        crate::native_router_plan::Candidate {
            session_key: "codex:target".into(),
            backend: "codex".into(),
            id: "target".into(),
            cwd: "/project".into(),
            title: "Target".into(),
            responsibility: String::new(),
            fallback_condition: String::new(),
            opening_message: String::new(),
        }
    }
    fn running(path: &Path) -> Job {
        let mut value = job("codex:router");
        reserve(path, &value, "owner").unwrap();
        value.target = Some(target());
        value.target_turn_id = "previous".into();
        value.status = "running".into();
        save(path, &value, "owner", true).unwrap();
        value
    }
    const CONTINUATION: &str = "[Studio turn supervisor]\nFinish the remaining work 中文😀";
    #[test]
    fn source_scope_is_small_metadata_and_does_not_wait_for_or_decode_a_running_map_reply() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("studio.sqlite3");
        let old = running(&path);
        let connection = db(&path).unwrap();
        connection.execute("UPDATE native_router_jobs SET json=json_set(json,'$.mapScope',json('{}'),'$.responseItems',?2) WHERE id=?1",params![old.id,"PRIVATE reply bytes ".repeat(50000)]).unwrap();
        assert!(
            read(&path, &old.id).is_err(),
            "fixture must require no full Job deserialization"
        );
        let scope = source_scope(&path, "codex:target", "previous", "/project")
            .unwrap()
            .unwrap();
        assert_eq!(
            scope,
            serde_json::json!({"id":old.id,"controller":"codex:router"})
        );
        assert!(scope.to_string().len() < 160);
        assert_eq!(
            source_ready(&path, "codex:target", "previous", "/project").unwrap(),
            (true, false)
        );
        assert!(source_scope(&path, "codex:target", "previous", "/other").is_err());
        assert!(source_scope(&path, "codex:target", "unknown", "/project").is_err());
        connection.execute("UPDATE native_router_claims SET job_id='foreign-job' WHERE session_key='codex:target'",[]).unwrap();
        assert!(source_scope(&path, "codex:target", "previous", "/project").is_err());
    }
    #[test]
    fn source_scope_distinguishes_unrouted_from_stopped_ambiguous_or_corrupted_ownership() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("studio.sqlite3");
        assert!(source_scope(&path, "codex:target", "previous", "/project")
            .unwrap()
            .is_none());
        let old = running(&path);
        note_stop(&path, &old.id, "owner").unwrap();
        assert!(source_scope(&path, "codex:target", "previous", "/project").is_err());
        let connection = db(&path).unwrap();
        connection.execute("UPDATE native_router_jobs SET json=json_set(json,'$.stopRequested',json('false')) WHERE id=?1",[&old.id]).unwrap();
        let mut other = job("codex:other-router");
        reserve(&path, &other, "owner").unwrap();
        other.target = Some(target());
        other.target_turn_id = "previous".into();
        other.status = "running".into();
        save(&path, &other, "owner", false).unwrap();
        assert!(source_scope(&path, "codex:target", "previous", "/project")
            .unwrap_err()
            .contains("More than one"));
    }
    #[test]
    fn supervised_handoff_clears_old_source_and_blocks_stale_workers_without_erasing_metadata() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("studio.sqlite3");
        let mut old = running(&path);
        old.response = "OLD response".into();
        old.response_items = vec![crate::native_router_backend::ResponseItem {
            id: "old-item".into(),
            text: "OLD response".into(),
            excerpt: false,
        }];
        old.context_excerpt = "OLD context".into();
        old.error = "OLD error".into();
        old.unread = true;
        old.map_notice = "OLD map notice".into();
        old.controller_turn_id = "controller-plan".into();
        save(&path, &old, "owner", false).unwrap();
        let connection = db(&path).unwrap();
        connection.execute("UPDATE native_router_jobs SET json=json_set(json,'$.futureMetadata',json(?2)) WHERE id=?1",params![old.id,"{\"preserve\":[1,2,3]}"]).unwrap();
        let next = follow(
            &path,
            "owner",
            &old.id,
            "codex:target",
            "/project",
            "previous",
            "next",
            CONTINUATION,
            false,
        )
        .unwrap();
        assert_eq!(next.status, "running");
        assert_eq!(next.target_turn_id, "next");
        assert_eq!(next.controller_turn_id, "controller-plan");
        assert_eq!(next.prompt, old.prompt);
        assert!(
            next.response.is_empty()
                && next.response_items.is_empty()
                && next.context_excerpt.is_empty()
                && next.error.is_empty()
                && !next.unread
                && next.map_notice.is_empty()
                && next.map_scope.is_none()
        );
        let recorded = view(&path, &old.id).unwrap();
        assert_eq!(
            recorded["futureMetadata"],
            serde_json::json!({"preserve":[1,2,3]})
        );
        assert_eq!(recorded["supervision"]["prompt"], CONTINUATION);
        old.status = "completed".into();
        assert!(save(&path, &old, "owner", false)
            .unwrap_err()
            .contains("older worker"));
        assert_eq!(read(&path, &old.id).unwrap().target_turn_id, "next");
        let receipt = recorded["receipt"].clone();
        // Duplicate delivery acknowledgement must not change ownership, receipts,
        // stop state, claims or any newly collected reply.
        follow(
            &path,
            "different-owner",
            &old.id,
            "codex:target",
            "/project",
            "previous",
            "next",
            CONTINUATION,
            true,
        )
        .unwrap();
        assert!(owned(&path, &old.id, "different-owner").unwrap().is_none());
        assert_eq!(view(&path, &old.id).unwrap()["receipt"], receipt);
        assert!(!read(&path, &old.id).unwrap().stop_requested);
        assert!(source_job(&path, "codex:target", "previous", "next")
            .unwrap()
            .is_some());
    }
    #[test]
    fn supervised_receipt_cannot_steal_another_route_and_failed_updates_roll_back() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("studio.sqlite3");
        let mut first = running(&path);
        first.status = "completed".into();
        save(&path, &first, "owner", false).unwrap();
        let mut second = job("codex:other-router");
        reserve(&path, &second, "owner").unwrap();
        second.target = Some(target());
        second.status = "dispatching".into();
        save(&path, &second, "owner", true).unwrap();
        assert!(follow(
            &path,
            "new-owner",
            &first.id,
            "codex:target",
            "/project",
            "previous",
            "next",
            CONTINUATION,
            false
        )
        .unwrap_err()
        .contains("Another routed request"));
        assert!(owned(&path, &first.id, "owner").unwrap().is_some());
        assert_eq!(read(&path, &first.id).unwrap().target_turn_id, "previous");
        for (key, cwd, previous) in [
            ("codex:foreign", "/project", "previous"),
            ("codex:target", "/foreign", "previous"),
            ("codex:target", "/project", "unrelated"),
        ] {
            assert!(follow(
                &path,
                "new-owner",
                &first.id,
                key,
                cwd,
                previous,
                "next",
                CONTINUATION,
                false
            )
            .is_err());
        }
        second.status = "completed".into();
        save(&path, &second, "owner", false).unwrap();
        let next = follow(
            &path,
            "new-owner",
            &first.id,
            "codex:target",
            "/project",
            "previous",
            "next",
            CONTINUATION,
            true,
        )
        .unwrap();
        assert!(next.stop_requested);
        assert!(owned(&path, &first.id, "owner").unwrap().is_none());
        assert!(owned(&path, &first.id, "new-owner").unwrap().is_some());
    }
    #[test]
    fn router_preparation_checks_claims_and_sticky_stops_before_native_map_configuration() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("studio.sqlite3");
        assert_eq!(
            source_ready(&path, "codex:target", "previous", "/project").unwrap(),
            (false, true)
        );
        let old = running(&path);
        assert_eq!(
            source_ready(&path, "codex:target", "previous", "/project").unwrap(),
            (true, true)
        );
        let connection = db(&path).unwrap();
        connection.execute("UPDATE native_router_jobs SET json=json_set(json,'$.mapScope',json('{}')) WHERE id=?1",[&old.id]).unwrap();
        assert_eq!(
            source_ready(&path, "codex:target", "previous", "/project").unwrap(),
            (true, false)
        );
        connection
            .execute(
                "UPDATE native_router_jobs SET phase='completed' WHERE id=?1",
                [&old.id],
            )
            .unwrap();
        assert_eq!(
            source_ready(&path, "codex:target", "previous", "/project").unwrap(),
            (true, true)
        );
        assert!(source_ready(&path, "codex:target", "previous", "/different-project").is_err());
        assert!(source_ready(&path, "codex:target", "other-turn", "/project").is_err());
        note_stop(&path, &old.id, "owner").unwrap();
        // A same-turn worker snapshot predating Stop cannot erase that intent.
        save(&path, &old, "owner", false).unwrap();
        assert!(read(&path, &old.id).unwrap().stop_requested);
        assert!(source_ready(&path, "codex:target", "previous", "/project")
            .unwrap_err()
            .contains("stop"));
        let followed = follow(
            &path,
            "owner",
            &old.id,
            "codex:target",
            "/project",
            "previous",
            "next",
            CONTINUATION,
            false,
        )
        .unwrap();
        assert!(read(&path, &followed.id).unwrap().stop_requested);
        let plan:Vec<String>=connection.prepare("EXPLAIN QUERY PLAN SELECT id FROM native_router_jobs WHERE json_valid(json) AND json_extract(json,'$.target.sessionKey')=?1 AND json_extract(json,'$.targetTurnId')=?2").unwrap().query_map(params!["codex:target","next"],|row|row.get(3)).unwrap().map(Result::unwrap).collect();
        assert!(
            plan.iter().any(|s| s.contains("native_router_source")),
            "{plan:?}"
        );
    }
    #[test]
    fn claims_are_transactional_and_release_controller_while_preserving_target_ownership() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("studio.sqlite3");
        let mut first = job("codex:router");
        reserve(&path, &first, "owner").unwrap();
        let second = job("codex:router");
        assert!(reserve(&path, &second, "owner").is_err());
        first.target = Some(target());
        first.status = "dispatching".into();
        save(&path, &first, "owner", true).unwrap();
        first.status = "running".into();
        first.target_turn_id = "target-turn".into();
        save(&path, &first, "owner", false).unwrap();
        reserve(&path, &second, "owner").unwrap();
        let mut second = second;
        second.target = Some(target());
        second.status = "dispatching".into();
        assert!(save(&path, &second, "owner", true).is_err());
        // A failed owner check rolls back the target claim too.
        second.target.as_mut().unwrap().session_key = "codex:other".into();
        assert!(save(&path, &second, "old-owner", true).is_err());
        assert_eq!(
            db(&path)
                .unwrap()
                .query_row(
                    "SELECT COUNT(*) FROM native_router_claims WHERE session_key='codex:other'",
                    [],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
            0
        );
        first.status = "completed".into();
        first.unread = true;
        save(&path, &first, "owner", false).unwrap();
        second.target = Some(target());
        save(&path, &second, "owner", true).unwrap();
        let shared = crate::router_history::load(&path, "codex:router").unwrap();
        assert!(shared
            .iter()
            .any(|record| record.dispatch["targetTurnId"] == "target-turn"
                && record.dispatch["status"] == "completed"));
    }
    #[test]
    fn restart_monitors_only_confirmed_deliveries_and_never_replays_ambiguous_intents() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("studio.sqlite3");
        let pending = job("codex:routing");
        reserve(&path, &pending, "old").unwrap();
        let mut running = job("opencode:router");
        reserve(&path, &running, "old").unwrap();
        running.status = "running".into();
        running.target = Some(target());
        running.target_turn_id = "confirmed-turn".into();
        save(&path, &running, "old", true).unwrap();
        let recovered = recover(&path, "new").unwrap();
        assert_eq!(recovered.len(), 1);
        assert_eq!(recovered[0].target_turn_id, "confirmed-turn");
        let failed = read(&path, &pending.id).unwrap();
        assert_eq!(failed.status, "failed");
        assert!(failed.error.contains("not replayed"));
        assert!(save(&path, &running, "old", false).is_err());
        assert!(recover(&path, "new").unwrap().is_empty());
        running.status = "completed".into();
        save(&path, &running, "new", false).unwrap();
        assert!(find(&path, "missing").unwrap().is_none());
    }
    #[test]
    fn history_cursor_is_stable_when_timestamps_match_and_status_changes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("studio.sqlite3");
        let mut identities = std::collections::HashSet::new();
        for _ in 0..26 {
            let mut job = job("codex:router");
            job.status = "completed".into();
            identities.insert(job.id.clone());
            reserve(&path, &job, "owner").unwrap();
        }
        let first = list(&path, "codex:router", None).unwrap();
        assert_eq!(first.len(), 21);
        let cursor = first[19]["cursor"].as_i64().unwrap();
        let second = list(&path, "codex:router", Some(cursor)).unwrap();
        assert_eq!(second.len(), 6);
        let saved = first
            .iter()
            .take(20)
            .chain(&second)
            .map(|row| row["id"].as_str().unwrap().to_owned())
            .collect::<std::collections::HashSet<_>>();
        assert_eq!(saved, identities);
    }
    #[test]
    fn receipts_reject_stale_active_and_foreign_views_and_preserve_other_unread_jobs() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("studio.sqlite3");
        let mut first = job("codex:router");
        reserve(&path, &first, "owner").unwrap();
        let old = Receipt {
            id: first.id.clone(),
            receipt: view(&path, &first.id).unwrap()["receipt"]
                .as_str()
                .unwrap()
                .into(),
        };
        assert!(
            acknowledge(&path, "codex:router", std::slice::from_ref(&old))
                .unwrap()
                .is_empty()
        );
        first.status = "completed".into();
        first.unread = true;
        first.response = "New final reply".into();
        save(&path, &first, "owner", false).unwrap();
        let current = Receipt {
            id: first.id.clone(),
            receipt: view(&path, &first.id).unwrap()["receipt"]
                .as_str()
                .unwrap()
                .into(),
        };
        assert_ne!(old.receipt, current.receipt);
        assert!(acknowledge(&path, "codex:router", &[old])
            .unwrap()
            .is_empty());
        assert!(
            acknowledge(&path, "opencode:router", std::slice::from_ref(&current))
                .unwrap()
                .is_empty()
        );
        let mut other = job("codex:router");
        other.status = "completed".into();
        other.unread = true;
        reserve(&path, &other, "owner").unwrap();
        assert_eq!(summary(&path, "codex:router").unwrap()["unreadCount"], 2);
        db(&path).unwrap().execute("UPDATE native_router_jobs SET json=json_set(json,'$.futureExtension',42) WHERE id=?1",[&first.id]).unwrap();
        db(&path).unwrap().execute("UPDATE router_dispatches SET dispatch=json_set(dispatch,'$.futureExtension',43) WHERE turn_key=?1",[format!("codex:native-{}",first.id)]).unwrap();
        assert_eq!(
            acknowledge(&path, "codex:router", std::slice::from_ref(&current))
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            acknowledge(&path, "codex:router", std::slice::from_ref(&current))
                .unwrap()
                .len(),
            1
        );
        let seen = view(&path, &first.id).unwrap();
        assert_eq!(seen["receipt"], current.receipt);
        assert_eq!(seen["futureExtension"], 42);
        assert_eq!(seen["response"], "New final reply");
        assert_eq!(seen["unread"], false);
        assert!(read(&path, &other.id).unwrap().unread);
        assert_eq!(summary(&path, "codex:router").unwrap()["unreadCount"], 1);
        let mirrored = crate::router_history::load(&path, "codex:router")
            .unwrap()
            .into_iter()
            .find(|record| record.dispatch["nativeJobId"] == first.id)
            .unwrap();
        assert_eq!(mirrored.dispatch["unread"], false);
        assert_eq!(mirrored.dispatch["futureExtension"], 43);
    }
    #[test]
    fn legacy_migration_keeps_payload_owner_cursor_and_unread_beyond_the_newest_page() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("studio.sqlite3");
        let old = Connection::open(&path).unwrap();
        old.execute_batch("CREATE TABLE native_router_jobs(id TEXT PRIMARY KEY,controller TEXT NOT NULL,created INTEGER NOT NULL,owner TEXT NOT NULL,phase TEXT NOT NULL,json TEXT NOT NULL);").unwrap();
        let mut oldest = job("codex:router");
        oldest.status = "completed".into();
        oldest.unread = true;
        let raw = serde_json::to_string(&oldest).unwrap();
        old.execute(
            "INSERT INTO native_router_jobs VALUES(?1,?2,1,'original-owner','completed',?3)",
            params![oldest.id, oldest.controller, raw],
        )
        .unwrap();
        drop(old);
        let migrated = view(&path, &oldest.id).unwrap();
        let receipt = migrated["receipt"].as_str().unwrap().to_owned();
        assert_eq!(receipt.len(), 32);
        for _ in 0..25 {
            let mut newer = job("codex:router");
            newer.status = "completed".into();
            reserve(&path, &newer, "owner").unwrap();
        }
        assert!(!list(&path, "codex:router", None)
            .unwrap()
            .iter()
            .any(|row| row["id"] == oldest.id));
        assert_eq!(summary(&path, "codex:router").unwrap()["unreadCount"], 1);
        assert_eq!(view(&path, &oldest.id).unwrap()["receipt"], receipt);
        let (owner, cursor, json): (String, i64, String) = db(&path)
            .unwrap()
            .query_row(
                "SELECT owner,rowid,json FROM native_router_jobs WHERE id=?1",
                [&oldest.id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!((owner, cursor, json), ("original-owner".into(), 1, raw));
        assert_eq!(
            acknowledge(
                &path,
                "codex:router",
                &[Receipt {
                    id: oldest.id,
                    receipt
                }]
            )
            .unwrap()
            .len(),
            1
        );
        assert_eq!(summary(&path, "codex:router").unwrap()["unreadCount"], 0);
    }
    #[test]
    fn bidirectional_pages_keep_stable_boundaries_when_new_records_arrive() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("studio.sqlite3");
        for _ in 0..45 {
            let mut record = job("codex:router");
            record.status = "completed".into();
            reserve(&path, &record, "owner").unwrap();
        }
        let cursor = |row: &serde_json::Value| row["cursor"].as_i64().unwrap();
        let mut newest = list_range(&path, "codex:router", None, None).unwrap();
        newest.truncate(20);
        assert_eq!((cursor(&newest[0]), cursor(&newest[19])), (45, 26));
        let mut middle = list_range(&path, "codex:router", Some(26), None).unwrap();
        middle.truncate(20);
        assert_eq!((cursor(&middle[0]), cursor(&middle[19])), (25, 6));
        let oldest = list_range(&path, "codex:router", Some(6), None).unwrap();
        assert_eq!(oldest.len(), 5);
        assert_eq!(
            neighbors(&path, "codex:router", Some(5), Some(1)).unwrap(),
            (false, true)
        );
        let mut incoming = job("codex:router");
        incoming.status = "completed".into();
        reserve(&path, &incoming, "owner").unwrap();
        let back = list_range(&path, "codex:router", None, Some(5)).unwrap();
        assert_eq!(back, middle);
        let near_latest = list_range(&path, "codex:router", None, Some(25)).unwrap();
        assert_eq!(
            (cursor(&near_latest[0]), cursor(&near_latest[19])),
            (45, 26)
        );
        assert_eq!(
            neighbors(&path, "codex:router", Some(45), Some(26)).unwrap(),
            (true, true)
        );
        let mut latest = list_range(&path, "codex:router", None, None).unwrap();
        latest.truncate(20);
        assert_eq!((cursor(&latest[0]), cursor(&latest[19])), (46, 27));
        assert_eq!(
            neighbors(&path, "codex:router", Some(46), Some(27)).unwrap(),
            (true, false)
        );
    }
    #[test]
    fn recent_context_uses_completed_same_controller_tails_and_legacy_preview_fallback() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("studio.sqlite3");
        for (controller, status, has_tail) in [
            ("codex:router", "completed", false),
            ("codex:router", "failed", true),
            ("opencode:router", "completed", true),
            ("codex:router", "completed", true),
        ] {
            let mut record = job(controller);
            record.status = status.into();
            record.target = Some(target());
            record.target_turn_id = format!("{controller}-{status}-{has_tail}");
            record.response = "Legacy preview 中文😀".into();
            record.context_excerpt = if has_tail {
                "end 中文😀".repeat(5000)
            } else {
                String::new()
            };
            reserve(&path, &record, "owner").unwrap();
            if !has_tail {
                db(&path).unwrap().execute("UPDATE native_router_jobs SET json=json_remove(json,'$.contextExcerpt') WHERE id=?1",[record.id]).unwrap();
            }
        }
        let recent = recent(&path, "codex:router").unwrap();
        assert_eq!(recent.len(), 2);
        assert!(recent[0].response_excerpt.chars().count() <= 4000);
        assert!(recent[0].response_excerpt.ends_with("end 中文😀"));
        assert_eq!(recent[1].response_excerpt, "Legacy preview 中文😀");
        assert!(recent.iter().all(|r| r.session_key == "codex:target"));
    }
}
