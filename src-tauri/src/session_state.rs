use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::time::Duration;

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

use crate::{AnnotationDraft, OpeningMessage};

const SETTINGS_MIGRATION_KEY: &str = "settings_session_state_v1";

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SessionStateSnapshot {
    pub(crate) annotation_drafts: BTreeMap<String, Vec<AnnotationDraft>>,
    pub(crate) annotation_additional: BTreeMap<String, String>,
    pub(crate) opening_messages: BTreeMap<String, OpeningMessage>,
}

pub(crate) fn initialize(path: &Path) -> Result<(), String> {
    connection(path).map(|_| ())
}

pub(crate) fn migration_complete(path: &Path) -> Result<bool, String> {
    let connection = connection(path)?;
    connection
        .query_row(
            "SELECT value FROM session_state_meta WHERE key = ?1",
            [SETTINGS_MIGRATION_KEY],
            |_| Ok(()),
        )
        .optional()
        .map(|value| value.is_some())
        .map_err(sql_error)
}

pub(crate) fn migrate_legacy_settings(
    path: &Path,
    annotation_drafts: &BTreeMap<String, Vec<AnnotationDraft>>,
    annotation_additional: &BTreeMap<String, String>,
    opening_messages: &BTreeMap<String, OpeningMessage>,
) -> Result<(), String> {
    let mut connection = connection(path)?;
    if migration_complete_with_connection(&connection)? {
        return Ok(());
    }

    let transaction = connection.transaction().map_err(sql_error)?;
    for (session_key, drafts) in annotation_drafts {
        let drafts_json = serde_json::to_string(drafts).map_err(|error| error.to_string())?;
        transaction
            .execute(
                "INSERT OR IGNORE INTO session_annotation_drafts(session_key, drafts_json)
                 VALUES (?1, ?2)",
                params![session_key, drafts_json],
            )
            .map_err(sql_error)?;
    }
    for (session_key, content) in annotation_additional {
        transaction
            .execute(
                "INSERT OR IGNORE INTO session_annotation_additional(session_key, content)
                 VALUES (?1, ?2)",
                params![session_key, content],
            )
            .map_err(sql_error)?;
    }
    for (session_key, message) in opening_messages {
        let message_json = serde_json::to_string(message).map_err(|error| error.to_string())?;
        transaction
            .execute(
                "INSERT OR IGNORE INTO session_opening_messages(session_key, message_json)
                 VALUES (?1, ?2)",
                params![session_key, message_json],
            )
            .map_err(sql_error)?;
    }
    transaction
        .execute(
            "INSERT INTO session_state_meta(key, value) VALUES (?1, '1')",
            [SETTINGS_MIGRATION_KEY],
        )
        .map_err(sql_error)?;
    transaction.commit().map_err(sql_error)
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

    Ok(snapshot)
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
    transaction.commit().map_err(sql_error)
}

fn migration_complete_with_connection(connection: &Connection) -> Result<bool, String> {
    connection
        .query_row(
            "SELECT value FROM session_state_meta WHERE key = ?1",
            [SETTINGS_MIGRATION_KEY],
            |_| Ok(()),
        )
        .optional()
        .map(|value| value.is_some())
        .map_err(sql_error)
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
             CREATE TABLE IF NOT EXISTS session_state_meta (
               key TEXT PRIMARY KEY,
               value TEXT NOT NULL
             );
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
             );",
        )
        .map_err(sql_error)?;
    Ok(connection)
}

fn sql_error(error: rusqlite::Error) -> String {
    error.to_string()
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

        let state = load(&path).unwrap();
        assert_eq!(state.annotation_drafts["codex:one"].len(), 2);
        assert_eq!(state.annotation_additional["codex:one"], "overall");
        assert_eq!(state.opening_messages["codex:one"].text, "hello");

        replace_annotations(&path, "codex:one", &[], "").unwrap();
        put_opening_message(&path, "codex:one", None).unwrap();
        let state = load(&path).unwrap();
        assert!(state.annotation_drafts.is_empty());
        assert!(state.annotation_additional.is_empty());
        assert!(state.opening_messages.is_empty());
        fs::remove_file(path).ok();
    }

    #[test]
    fn legacy_migration_is_transactional_and_idempotent() {
        let path = database_path("migration");
        let drafts = BTreeMap::from([("codex:one".to_string(), vec![draft("legacy")])]);
        let additional = BTreeMap::from([("codex:one".to_string(), "legacy note".to_string())]);
        let openings = BTreeMap::from([("codex:one".to_string(), opening("legacy opening"))]);

        migrate_legacy_settings(&path, &drafts, &additional, &openings).unwrap();
        replace_annotations(&path, "codex:one", &[draft("new")], "new note").unwrap();
        put_opening_message(&path, "codex:one", Some(&opening("new opening"))).unwrap();
        migrate_legacy_settings(&path, &drafts, &additional, &openings).unwrap();

        let state = load(&path).unwrap();
        assert_eq!(state.annotation_drafts["codex:one"][0].id, "new");
        assert_eq!(state.annotation_additional["codex:one"], "new note");
        assert_eq!(state.opening_messages["codex:one"].text, "new opening");
        assert!(migration_complete(&path).unwrap());
        fs::remove_file(path).ok();
    }

    #[test]
    fn delete_session_removes_all_state_for_only_that_session() {
        let path = database_path("delete");
        for key in ["codex:one", "opencode:two"] {
            replace_annotations(&path, key, &[draft(key)], key).unwrap();
            put_opening_message(&path, key, Some(&opening(key))).unwrap();
        }
        delete_session(&path, "codex:one").unwrap();

        let state = load(&path).unwrap();
        assert!(!state.annotation_drafts.contains_key("codex:one"));
        assert!(state.annotation_drafts.contains_key("opencode:two"));
        assert!(!state.opening_messages.contains_key("codex:one"));
        assert!(state.opening_messages.contains_key("opencode:two"));
        fs::remove_file(path).ok();
    }

    #[test]
    fn shares_the_favorites_database_without_schema_conflicts() {
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
