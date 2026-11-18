//! Durable receipts for existing Studio user submissions in team member sessions.
//! This does not change their input/options. It prevents a second page or lost HTTP
//! response from executing the same queued message twice.
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::path::Path;
use studio_teams::Session;

type Result<T> = std::result::Result<T, String>;
fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}
fn payload_hash(value: &Value) -> Result<String> {
    Ok(Sha256::digest(serde_json::to_vec(value).map_err(err)?)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}
pub struct Dispatches {
    db: Connection,
}
impl Dispatches {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let db = Connection::open(path).map_err(err)?;
        db.busy_timeout(std::time::Duration::from_secs(5))
            .map_err(err)?;
        db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;
CREATE TABLE IF NOT EXISTS user_dispatches(backend TEXT NOT NULL,thread TEXT NOT NULL,id TEXT NOT NULL,payload_hash TEXT NOT NULL,reservation TEXT NOT NULL,response TEXT,PRIMARY KEY(backend,thread,id));").map_err(err)?;
        Ok(Self { db })
    }
    pub fn lookup(&self, session: &Session, id: &str, payload: &Value) -> Result<Option<Value>> {
        let row: Option<(String,Option<String>)> = self.db.query_row("SELECT payload_hash,response FROM user_dispatches WHERE backend=? AND thread=? AND id=?",params![session.backend,session.thread,id],|r| Ok((r.get(0)?,r.get(1)?))).optional().map_err(err)?;
        match row {
            None => Ok(None),
            Some((hash, response)) => {
                if hash != payload_hash(payload)? {
                    return Err(
                        "Submission ID already belongs to different input or turn options".into(),
                    );
                }
                response.map(|s|serde_json::from_str(&s).map_err(err)).transpose()?.map(Some).ok_or_else(||"This user submission may already have executed. Inspect its native receipt; it will not be retried automatically.".into())
            }
        }
    }
    pub fn prepare(
        &mut self,
        session: &Session,
        id: &str,
        payload: &Value,
        reservation: &str,
    ) -> Result<()> {
        if id.is_empty() || id.len() > 256 || reservation.is_empty() {
            return Err("Invalid dispatch identity".into());
        }
        self.db
            .execute(
                "INSERT INTO user_dispatches VALUES(?,?,?,?,?,NULL)",
                params![
                    session.backend,
                    session.thread,
                    id,
                    payload_hash(payload)?,
                    reservation
                ],
            )
            .map_err(err)?;
        Ok(())
    }
    pub fn accepted(&mut self, session: &Session, id: &str, response: &Value) -> Result<()> {
        if response["turn"]["id"].as_str().is_none() {
            return Err("No native turn receipt in response".into());
        }
        let body = serde_json::to_string(response).map_err(err)?;
        let changed=self.db.execute("UPDATE user_dispatches SET response=? WHERE backend=? AND thread=? AND id=? AND response IS NULL",params![body,session.backend,session.thread,id]).map_err(err)?;
        if changed == 0 {
            let old: Option<String> = self
                .db
                .query_row(
                    "SELECT response FROM user_dispatches WHERE backend=? AND thread=? AND id=?",
                    params![session.backend, session.thread, id],
                    |r| r.get(0),
                )
                .optional()
                .map_err(err)?
                .flatten();
            let old: Value = old
                .map(|s| serde_json::from_str(&s).map_err(err))
                .transpose()?
                .ok_or("Missing native user-submission receipt")?;
            if old["turn"]["id"] != response["turn"]["id"] {
                return Err("Conflicting native user-submission receipt".into());
            }
            // A terminal event can win the race with the original start reply.
            if old["turn"]["status"] == "inProgress" {
                self.db.execute("UPDATE user_dispatches SET response=? WHERE backend=? AND thread=? AND id=?",params![body,session.backend,session.thread,id]).map_err(err)?;
            }
        }
        Ok(())
    }
    /// A native userMessage clientId proves acceptance even if the RPC reply was lost.
    /// Returns the matching reservation to release; never creates a new dispatch.
    pub fn bind_native(
        &mut self,
        session: &Session,
        client_id: &str,
        turn: &Value,
    ) -> Result<Option<String>> {
        let row:Option<(String,Option<String>)>=self.db.query_row("SELECT reservation,response FROM user_dispatches WHERE backend=? AND thread=? AND id=?",params![session.backend,session.thread,client_id],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(err)?;
        let Some((reservation, old)) = row else {
            return Ok(None);
        };
        if let Some(old) = old {
            let old: Value = serde_json::from_str(&old).map_err(err)?;
            if old["turn"]["id"] != turn["id"] {
                return Err("Conflicting native client message receipt".into());
            }
        } else {
            self.accepted(session, client_id, &json!({"turn":turn}))?;
        }
        Ok(Some(reservation))
    }
    pub fn observe_turn(&mut self, session: &Session, turn: &Value) -> Result<()> {
        let Some(id) = turn["id"].as_str() else {
            return Err("Native turn missing ID".into());
        };
        // Terminal observations replace the original acceptance snapshot, so another
        // page replaying a receipt does not revive an already completed turn.
        if turn["status"] == "inProgress" {
            return Ok(());
        }
        self.db.execute("UPDATE user_dispatches SET response=? WHERE backend=? AND thread=? AND json_extract(response,'$.turn.id')=?",params![json!({"turn":turn}).to_string(),session.backend,session.thread,id]).map_err(err)?;
        Ok(())
    }
    pub fn pending(&self) -> Result<Vec<Value>> {
        let mut stmt = self
            .db
            .prepare(
                "SELECT backend,thread,id,reservation FROM user_dispatches WHERE response IS NULL",
            )
            .map_err(err)?;
        let rows=stmt.query_map([],|r|Ok(json!({"session":{"backend":r.get::<_,String>(0)?,"thread":r.get::<_,String>(1)?},"id":r.get::<_,String>(2)?,"reservation":r.get::<_,String>(3)?}))).map_err(err)?;
        rows.collect::<std::result::Result<_, _>>().map_err(err)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn session() -> Session {
        Session {
            backend: "codex".into(),
            thread: "existing".into(),
        }
    }
    #[test]
    fn disk_receipt_replay_preserves_payload_and_never_repeats_unknown_send() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("dispatch.sqlite3");
        let params = json!({"threadId":"existing","clientUserMessageId":"stable-message","input":[{"type":"image","url":"data:image/png;base64,fixture"}],"model":"existing-model","effort":"high"});
        let mut db = Dispatches::open(&path).unwrap();
        assert_eq!(
            db.lookup(&session(), "stable-message", &params).unwrap(),
            None
        );
        db.prepare(&session(), "stable-message", &params, "reservation")
            .unwrap();
        drop(db);
        let mut db = Dispatches::open(&path).unwrap();
        assert!(db
            .lookup(&session(), "stable-message", &params)
            .unwrap_err()
            .contains("not be retried"));
        assert!(db
            .prepare(&session(), "stable-message", &params, "other-owner")
            .is_err());
        let response = json!({"turn":{"id":"native-turn","status":"inProgress","items":[]}});
        db.accepted(&session(), "stable-message", &response)
            .unwrap();
        assert_eq!(
            db.lookup(&session(), "stable-message", &params).unwrap(),
            Some(response)
        );
        let mut changed = params;
        changed["model"] = json!("different-model");
        assert!(db
            .lookup(&session(), "stable-message", &changed)
            .unwrap_err()
            .contains("different input"));
        assert!(db.pending().unwrap().is_empty());
    }
    #[test]
    fn receipt_cannot_be_replaced_by_a_different_native_turn() {
        let mut db = Dispatches::open(":memory:").unwrap();
        db.prepare(&session(), "message", &json!({}), "hold")
            .unwrap();
        db.accepted(&session(), "message", &json!({"turn":{"id":"one"}}))
            .unwrap();
        assert!(db
            .accepted(&session(), "message", &json!({"turn":{"id":"two"}}))
            .is_err());
    }
}
