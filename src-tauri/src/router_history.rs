//! Durable delivery metadata; never automatically resends an interrupted request.
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{path::Path, time::Duration};

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Record {
    pub controller: String,
    pub turn_key: String,
    pub dispatch: Value,
}

fn connection(path: &Path) -> Result<Connection, String> {
    let db = Connection::open(path).map_err(|e| e.to_string())?;
    db.busy_timeout(Duration::from_secs(5))
        .map_err(|e| e.to_string())?;
    db.execute_batch("CREATE TABLE IF NOT EXISTS router_dispatches (controller TEXT NOT NULL, turn_key TEXT NOT NULL, dispatch TEXT NOT NULL, PRIMARY KEY(controller, turn_key));").map_err(|e| e.to_string())?;
    Ok(db)
}

pub fn save(path: &Path, record: &Record) -> Result<(), String> {
    if !crate::valid_router_session_key(&record.controller)
        || !crate::valid_router_session_key(&record.turn_key)
        || record.controller.len() > 320
        || record.turn_key.len() > 320
        || !record.dispatch.is_object()
    {
        return Err("Invalid Router history record".into());
    }
    let json = serde_json::to_string(&record.dispatch).map_err(|e| e.to_string())?;
    if json.len() > 128 * 1024 {
        return Err("Router record is too large".into());
    }
    connection(path)?.execute("INSERT INTO router_dispatches(controller, turn_key, dispatch) VALUES (?1, ?2, ?3) ON CONFLICT(controller, turn_key) DO UPDATE SET dispatch=excluded.dispatch", params![record.controller, record.turn_key, json]).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn load(path: &Path, controller: &str) -> Result<Vec<Record>, String> {
    let db = connection(path)?;
    let mut statement = db
        .prepare("SELECT turn_key, dispatch FROM router_dispatches WHERE controller=?1")
        .map_err(|e| e.to_string())?;
    let rows = statement
        .query_map([controller], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|e| e.to_string())?;
    rows.map(|row| {
        let (turn_key, json) = row.map_err(|e| e.to_string())?;
        Ok(Record {
            controller: controller.into(),
            turn_key,
            dispatch: serde_json::from_str(&json).map_err(|e| e.to_string())?,
        })
    })
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preserves_turn_links_and_isolates_controllers() {
        let path =
            std::env::temp_dir().join(format!("studio-router-{}.sqlite3", uuid::Uuid::new_v4()));
        let mut record = Record {
            controller: "codex:router".into(),
            turn_key: "codex:turn".into(),
            dispatch: serde_json::json!({"status":"running","targetTurnId":"target-turn"}),
        };
        save(&path, &record).unwrap();
        record.dispatch["status"] = "completed".into();
        save(&path, &record).unwrap();
        let saved = load(&path, "codex:router").unwrap();
        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0].dispatch["targetTurnId"], "target-turn");
        assert_eq!(saved[0].dispatch["status"], "completed");
        assert!(load(&path, "opencode:router").unwrap().is_empty());
        std::fs::remove_file(path).unwrap();
    }
}
