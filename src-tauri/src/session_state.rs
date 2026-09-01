use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{AnnotationDraft, OpeningMessage};

pub(crate) const MAX_PINNED_SESSIONS: usize = 10;
pub(crate) const PIN_LIMIT_ERROR: &str = "at most 10 sessions can be pinned";

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SessionTurnOptions {
    pub(crate) model: String,
    pub(crate) effort: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct QueuedMessage {
    pub(crate) id: String,
    pub(crate) text: String,
    #[serde(default)]
    pub(crate) input: Vec<Value>,
    pub(crate) created_at: i64,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SessionStateSnapshot {
    pub(crate) annotation_drafts: BTreeMap<String, Vec<AnnotationDraft>>,
    pub(crate) annotation_additional: BTreeMap<String, String>,
    pub(crate) opening_messages: BTreeMap<String, OpeningMessage>,
    pub(crate) pinned_sessions: Vec<String>,
    pub(crate) turn_options: BTreeMap<String, SessionTurnOptions>,
    pub(crate) message_queues: BTreeMap<String, Vec<QueuedMessage>>,
}

pub(crate) fn initialize(path: &Path) -> Result<(), String> {
    connection(path).map(|_| ())
}

pub(crate) fn load(path: &Path) -> Result<SessionStateSnapshot, String> {
    let connection = connection(path)?;
    let mut snapshot = SessionStateSnapshot::default();

    let mut statement = connection
        .prepare(
            "SELECT session_key, drafts_json FROM session_annotation_drafts ORDER BY session_key",
        )
        .map_err(sql_error)?;
    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(sql_error)?;
    for row in rows {
        let (session_key, drafts_json) = row.map_err(sql_error)?;
        let drafts = serde_json::from_str(&drafts_json)
            .map_err(|error| format!("invalid annotation drafts for {session_key}: {error}"))?;
        snapshot.annotation_drafts.insert(session_key, drafts);
    }
    drop(statement);

    let mut statement = connection
        .prepare(
            "SELECT session_key, content FROM session_annotation_additional ORDER BY session_key",
        )
        .map_err(sql_error)?;
    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(sql_error)?;
    for row in rows {
        let (session_key, content) = row.map_err(sql_error)?;
        snapshot.annotation_additional.insert(session_key, content);
    }
    drop(statement);

    let mut statement = connection
        .prepare(
            "SELECT session_key, message_json FROM session_opening_messages ORDER BY session_key",
        )
        .map_err(sql_error)?;
    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(sql_error)?;
    for row in rows {
        let (session_key, message_json) = row.map_err(sql_error)?;
        let message = serde_json::from_str(&message_json)
            .map_err(|error| format!("invalid opening message for {session_key}: {error}"))?;
        snapshot.opening_messages.insert(session_key, message);
    }
    drop(statement);

    snapshot.pinned_sessions = load_pins_with_connection(&connection)?;

    let mut statement = connection
        .prepare("SELECT session_key, model, effort FROM session_turn_options ORDER BY session_key")
        .map_err(sql_error)?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                SessionTurnOptions {
                    model: row.get(1)?,
                    effort: row.get(2)?,
                },
            ))
        })
        .map_err(sql_error)?;
    for row in rows {
        let (session_key, options) = row.map_err(sql_error)?;
        snapshot.turn_options.insert(session_key, options);
    }
    drop(statement);

    let mut statement = connection
        .prepare("SELECT session_key, queue_json FROM session_message_queues ORDER BY session_key")
        .map_err(sql_error)?;
    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(sql_error)?;
    for row in rows {
        let (session_key, queue_json) = row.map_err(sql_error)?;
        let queue = serde_json::from_str(&queue_json)
            .map_err(|error| format!("invalid message queue for {session_key}: {error}"))?;
        snapshot.message_queues.insert(session_key, queue);
    }

    Ok(snapshot)
}

pub(crate) fn set_pinned(
    path: &Path,
    session_key: &str,
    pinned: bool,
) -> Result<Vec<String>, String> {
    set_pinned_at(path, session_key, pinned, now_ms()?)
}

pub(crate) fn replace_annotations(
    path: &Path,
    session_key: &str,
    drafts: &[AnnotationDraft],
    additional: &str,
) -> Result<(), String> {
    let drafts_json = serde_json::to_string(drafts).map_err(|error| error.to_string())?;
    let mut connection = connection(path)?;
    let transaction = connection.transaction().map_err(sql_error)?;
    if drafts.is_empty() {
        transaction
            .execute(
                "DELETE FROM session_annotation_drafts WHERE session_key = ?1",
                [session_key],
            )
            .map_err(sql_error)?;
    } else {
        transaction
            .execute(
                "INSERT INTO session_annotation_drafts(session_key, drafts_json) VALUES (?1, ?2)
                 ON CONFLICT(session_key) DO UPDATE SET drafts_json = excluded.drafts_json",
                params![session_key, drafts_json],
            )
            .map_err(sql_error)?;
    }
    if additional.is_empty() {
        transaction
            .execute(
                "DELETE FROM session_annotation_additional WHERE session_key = ?1",
                [session_key],
            )
            .map_err(sql_error)?;
    } else {
        transaction
            .execute(
                "INSERT INTO session_annotation_additional(session_key, content) VALUES (?1, ?2)
                 ON CONFLICT(session_key) DO UPDATE SET content = excluded.content",
                params![session_key, additional],
            )
            .map_err(sql_error)?;
    }
    transaction.commit().map_err(sql_error)
}

pub(crate) fn put_opening_message(
    path: &Path,
    session_key: &str,
    message: Option<&OpeningMessage>,
) -> Result<(), String> {
    let connection = connection(path)?;
    if let Some(message) = message {
        let message_json = serde_json::to_string(message).map_err(|error| error.to_string())?;
        connection
            .execute(
                "INSERT INTO session_opening_messages(session_key, message_json) VALUES (?1, ?2)
                 ON CONFLICT(session_key) DO UPDATE SET message_json = excluded.message_json",
                params![session_key, message_json],
            )
            .map_err(sql_error)?;
    } else {
        connection
            .execute(
                "DELETE FROM session_opening_messages WHERE session_key = ?1",
                [session_key],
            )
            .map_err(sql_error)?;
    }
    Ok(())
}

pub(crate) fn put_turn_options(
    path: &Path,
    session_key: &str,
    model: &str,
    effort: &str,
) -> Result<(), String> {
    let connection = connection(path)?;
    if model.is_empty() && effort.is_empty() {
        connection
            .execute(
                "DELETE FROM session_turn_options WHERE session_key = ?1",
                [session_key],
            )
            .map_err(sql_error)?;
    } else {
        connection
            .execute(
                "INSERT INTO session_turn_options(session_key, model, effort) VALUES (?1, ?2, ?3)
                 ON CONFLICT(session_key) DO UPDATE SET model = excluded.model, effort = excluded.effort",
                params![session_key, model, effort],
            )
            .map_err(sql_error)?;
    }
    Ok(())
}

pub(crate) fn replace_message_queue(
    path: &Path,
    session_key: &str,
    messages: &[QueuedMessage],
) -> Result<(), String> {
    let connection = connection(path)?;
    if messages.is_empty() {
        connection
            .execute(
                "DELETE FROM session_message_queues WHERE session_key = ?1",
                [session_key],
            )
            .map_err(sql_error)?;
    } else {
        let queue_json = serde_json::to_string(messages).map_err(|error| error.to_string())?;
        connection
            .execute(
                "INSERT INTO session_message_queues(session_key, queue_json) VALUES (?1, ?2)
                 ON CONFLICT(session_key) DO UPDATE SET queue_json = excluded.queue_json",
                params![session_key, queue_json],
            )
            .map_err(sql_error)?;
    }
    Ok(())
}

pub(crate) fn delete_session(path: &Path, session_key: &str) -> Result<(), String> {
    let mut connection = connection(path)?;
    let transaction = connection.transaction().map_err(sql_error)?;
    transaction
        .execute(
            "DELETE FROM session_annotation_drafts WHERE session_key = ?1",
            [session_key],
        )
        .map_err(sql_error)?;
    transaction
        .execute(
            "DELETE FROM session_annotation_additional WHERE session_key = ?1",
            [session_key],
        )
        .map_err(sql_error)?;
    transaction
        .execute(
            "DELETE FROM session_opening_messages WHERE session_key = ?1",
            [session_key],
        )
        .map_err(sql_error)?;
    transaction
        .execute(
            "DELETE FROM session_pins WHERE session_key = ?1",
            [session_key],
        )
        .map_err(sql_error)?;
    transaction
        .execute(
            "DELETE FROM session_turn_options WHERE session_key = ?1",
            [session_key],
        )
        .map_err(sql_error)?;
    transaction
        .execute(
            "DELETE FROM session_message_queues WHERE session_key = ?1",
            [session_key],
        )
        .map_err(sql_error)?;
    transaction.commit().map_err(sql_error)
}

fn set_pinned_at(
    path: &Path,
    session_key: &str,
    pinned: bool,
    pinned_at: i64,
) -> Result<Vec<String>, String> {
    let mut connection = connection(path)?;
    let transaction = connection.transaction().map_err(sql_error)?;
    let exists = transaction
        .query_row(
            "SELECT 1 FROM session_pins WHERE session_key = ?1",
            [session_key],
            |_| Ok(()),
        )
        .optional()
        .map_err(sql_error)?
        .is_some();
    if pinned && !exists {
        let count: usize = transaction
            .query_row("SELECT COUNT(*) FROM session_pins", [], |row| row.get(0))
            .map_err(sql_error)?;
        if count >= MAX_PINNED_SESSIONS {
            return Err(PIN_LIMIT_ERROR.to_string());
        }
        let newest_pinned_at: Option<i64> = transaction
            .query_row("SELECT MAX(pinned_at) FROM session_pins", [], |row| {
                row.get(0)
            })
            .map_err(sql_error)?;
        let effective_pinned_at = newest_pinned_at
            .map(|value| pinned_at.max(value.saturating_add(1)))
            .unwrap_or(pinned_at);
        transaction
            .execute(
                "INSERT INTO session_pins(session_key, pinned_at) VALUES (?1, ?2)",
                params![session_key, effective_pinned_at],
            )
            .map_err(sql_error)?;
    } else if !pinned && exists {
        transaction
            .execute(
                "DELETE FROM session_pins WHERE session_key = ?1",
                [session_key],
            )
            .map_err(sql_error)?;
    }
    transaction.commit().map_err(sql_error)?;
    load_pins_with_connection(&connection)
}

fn load_pins_with_connection(connection: &Connection) -> Result<Vec<String>, String> {
    let mut statement = connection
        .prepare("SELECT session_key FROM session_pins ORDER BY pinned_at ASC, session_key ASC")
        .map_err(sql_error)?;
    let pins = statement
        .query_map([], |row| row.get(0))
        .map_err(sql_error)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(sql_error)?;
    Ok(pins)
}

fn connection(path: &Path) -> Result<Connection, String> {
    let parent = path
        .parent()
        .ok_or_else(|| "session state database path has no parent directory".to_string())?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let connection = Connection::open(path).map_err(sql_error)?;
    connection
        .busy_timeout(Duration::from_secs(3))
        .map_err(sql_error)?;
    connection
        .execute_batch(
            "PRAGMA journal_mode = WAL;
             CREATE TABLE IF NOT EXISTS session_annotation_drafts (
               session_key TEXT PRIMARY KEY,
               drafts_json TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS session_annotation_additional (
               session_key TEXT PRIMARY KEY,
               content TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS session_opening_messages (
               session_key TEXT PRIMARY KEY,
               message_json TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS session_pins (
               session_key TEXT PRIMARY KEY,
               pinned_at INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS session_turn_options (
               session_key TEXT PRIMARY KEY,
               model TEXT NOT NULL,
               effort TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS session_message_queues (
               session_key TEXT PRIMARY KEY,
               queue_json TEXT NOT NULL
             );",
        )
        .map_err(sql_error)?;
    Ok(connection)
}

fn sql_error(error: rusqlite::Error) -> String {
    error.to_string()
}

fn now_ms() -> Result<i64, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(i64::MAX as u128) as i64)
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn database_path(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "codex-thread-studio-session-state-{name}-{}.sqlite3",
            Uuid::new_v4().simple()
        ))
    }

    fn draft(id: &str) -> AnnotationDraft {
        AnnotationDraft {
            id: id.to_string(),
            excerpt: "selected text".to_string(),
            created_at: "2026-08-31T00:00:00Z".to_string(),
            ..AnnotationDraft::default()
        }
    }

    fn opening(text: &str) -> OpeningMessage {
        OpeningMessage {
            text: text.to_string(),
            source: "history".to_string(),
            ..OpeningMessage::default()
        }
    }

    #[test]
    fn stores_updates_and_deletes_session_state() {
        let path = database_path("crud");
        initialize(&path).unwrap();
        replace_annotations(&path, "codex:one", &[draft("a"), draft("b")], "overall").unwrap();
        put_opening_message(&path, "codex:one", Some(&opening("hello"))).unwrap();
        set_pinned_at(&path, "codex:one", true, 100).unwrap();
        put_turn_options(&path, "codex:one", "gpt-session", "high").unwrap();
        replace_message_queue(
            &path,
            "codex:one",
            &[QueuedMessage {
                id: "queued-1".to_string(),
                text: "later".to_string(),
                input: vec![],
                created_at: 123,
            }],
        )
        .unwrap();

        let state = load(&path).unwrap();
        assert_eq!(state.annotation_drafts["codex:one"].len(), 2);
        assert_eq!(state.annotation_additional["codex:one"], "overall");
        assert_eq!(state.opening_messages["codex:one"].text, "hello");
        assert_eq!(state.pinned_sessions, ["codex:one"]);
        assert_eq!(
            state.turn_options["codex:one"],
            SessionTurnOptions {
                model: "gpt-session".to_string(),
                effort: "high".to_string(),
            }
        );
        assert_eq!(state.message_queues["codex:one"][0].text, "later");

        replace_annotations(&path, "codex:one", &[], "").unwrap();
        put_opening_message(&path, "codex:one", None).unwrap();
        set_pinned_at(&path, "codex:one", false, 0).unwrap();
        put_turn_options(&path, "codex:one", "", "").unwrap();
        replace_message_queue(&path, "codex:one", &[]).unwrap();
        let state = load(&path).unwrap();
        assert!(state.annotation_drafts.is_empty());
        assert!(state.annotation_additional.is_empty());
        assert!(state.opening_messages.is_empty());
        assert!(state.pinned_sessions.is_empty());
        assert!(state.turn_options.is_empty());
        assert!(state.message_queues.is_empty());
        fs::remove_file(path).ok();
    }

    #[test]
    fn delete_session_removes_all_state_for_only_that_session() {
        let path = database_path("delete");
        for key in ["codex:one", "opencode:two"] {
            replace_annotations(&path, key, &[draft(key)], key).unwrap();
            put_opening_message(&path, key, Some(&opening(key))).unwrap();
            set_pinned_at(&path, key, true, if key == "codex:one" { 1 } else { 2 }).unwrap();
            put_turn_options(&path, key, key, "medium").unwrap();
            replace_message_queue(
                &path,
                key,
                &[QueuedMessage {
                    id: key.to_string(),
                    text: key.to_string(),
                    input: vec![],
                    created_at: 1,
                }],
            )
            .unwrap();
        }
        delete_session(&path, "codex:one").unwrap();

        let state = load(&path).unwrap();
        assert!(!state.annotation_drafts.contains_key("codex:one"));
        assert!(state.annotation_drafts.contains_key("opencode:two"));
        assert!(!state.opening_messages.contains_key("codex:one"));
        assert!(state.opening_messages.contains_key("opencode:two"));
        assert_eq!(state.pinned_sessions, ["opencode:two"]);
        assert!(!state.turn_options.contains_key("codex:one"));
        assert_eq!(state.turn_options["opencode:two"].model, "opencode:two");
        assert!(!state.message_queues.contains_key("codex:one"));
        assert_eq!(state.message_queues["opencode:two"][0].text, "opencode:two");
        fs::remove_file(path).ok();
    }

    #[test]
    fn pins_are_ordered_and_limited_to_ten_sessions() {
        let path = database_path("pins");
        for index in 0..MAX_PINNED_SESSIONS {
            set_pinned_at(&path, &format!("codex:{index}"), true, index as i64).unwrap();
        }
        let pins = load(&path).unwrap().pinned_sessions;
        assert_eq!(pins.first().map(String::as_str), Some("codex:0"));
        assert_eq!(pins.last().map(String::as_str), Some("codex:9"));
        assert_eq!(
            set_pinned_at(&path, "codex:overflow", true, 11).unwrap_err(),
            PIN_LIMIT_ERROR
        );
        set_pinned_at(&path, "codex:4", false, 0).unwrap();
        set_pinned_at(&path, "codex:replacement", true, 12).unwrap();
        assert_eq!(
            load(&path)
                .unwrap()
                .pinned_sessions
                .last()
                .map(String::as_str),
            Some("codex:replacement")
        );
        fs::remove_file(path).ok();
    }

    #[test]
    fn shares_the_studio_database_without_schema_conflicts() {
        let path = database_path("shared");
        crate::favorites::initialize(&path).unwrap();
        replace_annotations(&path, "codex:one", &[draft("shared")], "").unwrap();
        crate::favorites::initialize(&path).unwrap();

        let connection = Connection::open(&path).unwrap();
        let favorite_tables: usize = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'favorites'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(favorite_tables, 1);
        assert_eq!(load(&path).unwrap().annotation_drafts["codex:one"].len(), 1);
        fs::remove_file(path).ok();
    }
}
