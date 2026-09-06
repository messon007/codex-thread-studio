use std::fs;
use std::path::Path;
use std::time::Duration;

use rusqlite::types::Type;
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};

use crate::backend_config::valid_backend_id;

pub const MAX_FAVORITE_BODY_BYTES: usize = 384 * 1024;
const MAX_FAVORITES: usize = 2_000;
const MAX_CONTENT_BYTES: usize = 192 * 1024;
const MAX_NOTE_BYTES: usize = 16 * 1024;
const MAX_RESULTS: usize = 2_000;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Favorite {
    pub id: String,
    #[serde(default = "default_scope")]
    pub scope: String,
    pub backend: String,
    pub thread_id: String,
    pub thread_title: String,
    pub project_path: String,
    pub turn_id: String,
    pub item_id: String,
    pub title: String,
    pub question: String,
    pub content: String,
    pub note: String,
    pub tags: Vec<String>,
    pub created_at: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FavoriteSummary {
    pub id: String,
    pub scope: String,
    pub backend: String,
    pub thread_id: String,
    pub thread_title: String,
    pub project_path: String,
    pub turn_id: String,
    pub item_id: String,
    pub title: String,
    pub question_snippet: String,
    pub snippet: String,
    pub tags: Vec<String>,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FavoriteList {
    pub items: Vec<FavoriteSummary>,
    pub total: usize,
    pub all_total: usize,
}

fn default_scope() -> String {
    "message".to_string()
}

pub fn initialize(path: &Path) -> Result<(), String> {
    connection(path).map(|_| ())
}

pub fn list(path: &Path, query: &str, limit: usize) -> Result<FavoriteList, String> {
    let items = load_all(&connection(path)?)?;
    let all_total = items.len();
    let query = query.trim().to_lowercase();
    let matches = items
        .iter()
        .filter(|favorite| {
            let tags = favorite.tags.join(" ");
            query.is_empty()
                || [
                    favorite.title.as_str(),
                    favorite.question.as_str(),
                    favorite.content.as_str(),
                    favorite.note.as_str(),
                    favorite.thread_title.as_str(),
                    favorite.project_path.as_str(),
                    favorite.backend.as_str(),
                    tags.as_str(),
                ]
                .iter()
                .any(|value| value.to_lowercase().contains(&query))
        })
        .collect::<Vec<_>>();
    let total = matches.len();
    let items = matches
        .into_iter()
        .take(limit.clamp(1, MAX_RESULTS))
        .map(summary)
        .collect();
    Ok(FavoriteList {
        items,
        total,
        all_total,
    })
}

pub fn insert(path: &Path, favorite: Favorite) -> Result<Favorite, String> {
    validate(&favorite)?;
    let connection = connection(path)?;
    let count: usize = connection
        .query_row("SELECT COUNT(*) FROM favorites", [], |row| row.get(0))
        .map_err(sql_error)?;
    if count >= MAX_FAVORITES {
        return Err(format!("at most {MAX_FAVORITES} favorites can be stored"));
    }
    if find_with_connection(&connection, &favorite.id)?.is_some() {
        return Err("favorite id already exists".to_string());
    }
    if favorite.scope == "message" && message_source_exists(&connection, &favorite)? {
        return Err("this message is already a favorite".to_string());
    }
    insert_row(&connection, &favorite).map_err(sql_error)?;
    Ok(favorite)
}

pub fn update(path: &Path, id: &str, favorite: Favorite) -> Result<Option<Favorite>, String> {
    validate(&favorite)?;
    if favorite.id != id {
        return Err("favorite id cannot be changed".to_string());
    }
    let connection = connection(path)?;
    if find_with_connection(&connection, id)?.is_none() {
        return Ok(None);
    }
    let changed = connection
        .execute(
            "UPDATE favorites SET
               scope = ?2, backend = ?3, thread_id = ?4, thread_title = ?5,
               project_path = ?6, turn_id = ?7, item_id = ?8, title = ?9,
               question = ?10, content = ?11, note = ?12, tags_json = ?13,
               created_at = ?14
             WHERE id = ?1",
            params![
                favorite.id,
                favorite.scope,
                favorite.backend,
                favorite.thread_id,
                favorite.thread_title,
                favorite.project_path,
                favorite.turn_id,
                favorite.item_id,
                favorite.title,
                favorite.question,
                favorite.content,
                favorite.note,
                tags_json(&favorite.tags),
                favorite.created_at,
            ],
        )
        .map_err(|error| {
            if is_unique_constraint(&error) {
                "this message is already a favorite".to_string()
            } else {
                sql_error(error)
            }
        })?;
    Ok((changed > 0).then_some(favorite))
}

pub fn remove(path: &Path, id: &str) -> Result<Option<Favorite>, String> {
    let mut connection = connection(path)?;
    let transaction = connection.transaction().map_err(sql_error)?;
    let favorite = find_with_connection(&transaction, id)?;
    if favorite.is_some() {
        transaction
            .execute("DELETE FROM favorites WHERE id = ?1", [id])
            .map_err(sql_error)?;
    }
    transaction.commit().map_err(sql_error)?;
    Ok(favorite)
}

pub fn find(path: &Path, id: &str) -> Result<Option<Favorite>, String> {
    find_with_connection(&connection(path)?, id)
}

pub fn export_markdown(path: &Path) -> Result<String, String> {
    let items = load_all(&connection(path)?)?;
    let mut output = format!(
        "# Codex Thread Studio Favorites\n\nExported favorites: {}\n",
        items.len()
    );
    for (index, favorite) in items.iter().enumerate() {
        output.push_str(&format!(
            "\n---\n\n## {}. {}\n\n- Created: {}\n- Backend: {}\n- Thread: {}\n- Project: {}\n- Source: `{}` / `{}`\n",
            index + 1,
            metadata(&favorite.title),
            metadata(&favorite.created_at),
            metadata(&favorite.backend),
            metadata(&favorite.thread_title),
            metadata(&favorite.project_path),
            metadata(&favorite.turn_id),
            metadata(&favorite.item_id),
        ));
        if !favorite.tags.is_empty() {
            output.push_str(&format!("- Tags: {}\n", favorite.tags.join(", ")));
        }
        if !favorite.question.trim().is_empty() {
            output.push_str(&format!("\n### Question\n\n{}\n", favorite.question.trim()));
        }
        output.push_str(&format!("\n### Favorite\n\n{}\n", favorite.content.trim()));
        if !favorite.note.trim().is_empty() {
            output.push_str(&format!("\n### Note\n\n{}\n", favorite.note.trim()));
        }
    }
    Ok(output)
}

pub fn validate(favorite: &Favorite) -> Result<(), String> {
    if favorite.id.trim().is_empty() || favorite.id.len() > 128 {
        return Err("favorite id must contain between 1 and 128 bytes".to_string());
    }
    if !valid_backend_id(&favorite.backend) {
        return Err("favorite backend id is invalid".to_string());
    }
    if !matches!(favorite.scope.as_str(), "message" | "selection") {
        return Err("favorite scope must be message or selection".to_string());
    }
    if favorite.thread_id.trim().is_empty()
        || favorite.thread_id.len() > 256
        || favorite.turn_id.trim().is_empty()
        || favorite.turn_id.len() > 256
        || favorite.item_id.trim().is_empty()
        || favorite.item_id.len() > 256
    {
        return Err("favorite source anchor is invalid".to_string());
    }
    if favorite.thread_title.len() > 512
        || favorite.project_path.len() > 2_048
        || favorite.created_at.trim().is_empty()
        || favorite.created_at.len() > 128
    {
        return Err("favorite source metadata is too large".to_string());
    }
    if favorite.title.trim().is_empty() || favorite.title.len() > 512 {
        return Err("favorite title must contain between 1 and 512 bytes".to_string());
    }
    if favorite.content.trim().is_empty() || favorite.content.len() > MAX_CONTENT_BYTES {
        return Err(format!(
            "favorite content must contain between 1 and {MAX_CONTENT_BYTES} bytes"
        ));
    }
    if favorite.question.len() > MAX_CONTENT_BYTES {
        return Err(format!(
            "favorite question must fit within {MAX_CONTENT_BYTES} bytes"
        ));
    }
    if favorite.note.len() > MAX_NOTE_BYTES {
        return Err(format!(
            "favorite note must fit within {MAX_NOTE_BYTES} bytes"
        ));
    }
    if favorite.tags.len() > 20
        || favorite
            .tags
            .iter()
            .any(|tag| tag.trim().is_empty() || tag.len() > 128)
    {
        return Err("favorite tags are invalid".to_string());
    }
    Ok(())
}

fn connection(path: &Path) -> Result<Connection, String> {
    let parent = path
        .parent()
        .ok_or_else(|| "favorites database path has no parent directory".to_string())?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let connection = Connection::open(path).map_err(sql_error)?;
    connection
        .busy_timeout(Duration::from_secs(3))
        .map_err(sql_error)?;
    connection
        .execute_batch(
            "PRAGMA journal_mode = WAL;
             CREATE TABLE IF NOT EXISTS favorites (
               id TEXT PRIMARY KEY,
               scope TEXT NOT NULL,
               backend TEXT NOT NULL,
               thread_id TEXT NOT NULL,
               thread_title TEXT NOT NULL,
               project_path TEXT NOT NULL,
               turn_id TEXT NOT NULL,
               item_id TEXT NOT NULL,
               title TEXT NOT NULL,
               question TEXT NOT NULL,
               content TEXT NOT NULL,
               note TEXT NOT NULL,
               tags_json TEXT NOT NULL,
               created_at TEXT NOT NULL
             );
             CREATE INDEX IF NOT EXISTS favorites_created_at
               ON favorites(created_at DESC);
             CREATE UNIQUE INDEX IF NOT EXISTS favorite_message_source
               ON favorites(backend, thread_id, turn_id, item_id)
               WHERE scope = 'message';",
        )
        .map_err(sql_error)?;
    Ok(connection)
}

fn insert_row(connection: &Connection, favorite: &Favorite) -> rusqlite::Result<usize> {
    connection.execute(
        "INSERT INTO favorites (
           id, scope, backend, thread_id, thread_title, project_path, turn_id,
           item_id, title, question, content, note, tags_json, created_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
        params![
            favorite.id,
            favorite.scope,
            favorite.backend,
            favorite.thread_id,
            favorite.thread_title,
            favorite.project_path,
            favorite.turn_id,
            favorite.item_id,
            favorite.title,
            favorite.question,
            favorite.content,
            favorite.note,
            tags_json(&favorite.tags),
            favorite.created_at,
        ],
    )
}

fn load_all(connection: &Connection) -> Result<Vec<Favorite>, String> {
    let mut statement = connection
        .prepare(
            "SELECT id, scope, backend, thread_id, thread_title, project_path,
                    turn_id, item_id, title, question, content, note, tags_json, created_at
             FROM favorites ORDER BY created_at DESC, id ASC",
        )
        .map_err(sql_error)?;
    let rows = statement
        .query_map([], favorite_from_row)
        .map_err(sql_error)?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(sql_error)
}

fn find_with_connection(connection: &Connection, id: &str) -> Result<Option<Favorite>, String> {
    connection
        .query_row(
            "SELECT id, scope, backend, thread_id, thread_title, project_path,
                    turn_id, item_id, title, question, content, note, tags_json, created_at
             FROM favorites WHERE id = ?1",
            [id],
            favorite_from_row,
        )
        .optional()
        .map_err(sql_error)
}

fn message_source_exists(connection: &Connection, favorite: &Favorite) -> Result<bool, String> {
    connection
        .query_row(
            "SELECT EXISTS(
               SELECT 1 FROM favorites
               WHERE scope = 'message' AND backend = ?1 AND thread_id = ?2
                 AND turn_id = ?3 AND item_id = ?4
             )",
            params![
                favorite.backend,
                favorite.thread_id,
                favorite.turn_id,
                favorite.item_id
            ],
            |row| row.get(0),
        )
        .map_err(sql_error)
}

fn favorite_from_row(row: &Row<'_>) -> rusqlite::Result<Favorite> {
    let tags_json: String = row.get(12)?;
    let tags = serde_json::from_str(&tags_json).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(12, Type::Text, Box::new(error))
    })?;
    Ok(Favorite {
        id: row.get(0)?,
        scope: row.get(1)?,
        backend: row.get(2)?,
        thread_id: row.get(3)?,
        thread_title: row.get(4)?,
        project_path: row.get(5)?,
        turn_id: row.get(6)?,
        item_id: row.get(7)?,
        title: row.get(8)?,
        question: row.get(9)?,
        content: row.get(10)?,
        note: row.get(11)?,
        tags,
        created_at: row.get(13)?,
    })
}

fn summary(favorite: &Favorite) -> FavoriteSummary {
    FavoriteSummary {
        id: favorite.id.clone(),
        scope: favorite.scope.clone(),
        backend: favorite.backend.clone(),
        thread_id: favorite.thread_id.clone(),
        thread_title: favorite.thread_title.clone(),
        project_path: favorite.project_path.clone(),
        turn_id: favorite.turn_id.clone(),
        item_id: favorite.item_id.clone(),
        title: favorite.title.clone(),
        question_snippet: snippet(&favorite.question, 140),
        snippet: snippet(&favorite.content, 280),
        tags: favorite.tags.clone(),
        created_at: favorite.created_at.clone(),
    }
}

fn snippet(content: &str, limit: usize) -> String {
    let normalized = content.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut characters = normalized.chars();
    let snippet = characters.by_ref().take(limit).collect::<String>();
    if characters.next().is_some() {
        format!("{snippet}…")
    } else {
        snippet
    }
}

fn tags_json(tags: &[String]) -> String {
    serde_json::to_string(tags).unwrap_or_else(|_| "[]".to_string())
}

fn metadata(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn is_unique_constraint(error: &rusqlite::Error) -> bool {
    matches!(
        error,
        rusqlite::Error::SqliteFailure(code, _)
            if code.code == rusqlite::ErrorCode::ConstraintViolation
    )
}

fn sql_error(error: rusqlite::Error) -> String {
    error.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn favorite(id: &str, title: &str, content: &str) -> Favorite {
        Favorite {
            id: id.to_string(),
            scope: "message".to_string(),
            backend: "codex".to_string(),
            thread_id: "thread-1".to_string(),
            thread_title: "Books".to_string(),
            project_path: "/tmp/books".to_string(),
            turn_id: format!("turn-{id}"),
            item_id: format!("item-{id}"),
            title: title.to_string(),
            question: "What should we build?".to_string(),
            content: content.to_string(),
            note: "Remember this".to_string(),
            tags: vec!["design".to_string()],
            created_at: "2026-07-24T00:00:00.000Z".to_string(),
        }
    }

    fn database(name: &str) -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("codex-thread-studio-{name}-{nonce}.sqlite3"))
    }

    fn cleanup(path: &Path) {
        for candidate in [
            path.to_path_buf(),
            path.with_extension("json"),
            path.with_extension("sqlite3-wal"),
            path.with_extension("sqlite3-shm"),
        ] {
            fs::remove_file(candidate).ok();
        }
    }

    #[test]
    fn stores_searches_updates_and_exports_sqlite_favorites() {
        let path = database("crud");
        insert(&path, favorite("a", "Architecture", "A durable event log")).unwrap();
        insert(&path, favorite("b", "Testing", "A regression suite")).unwrap();
        assert_eq!(list(&path, "event", 20).unwrap().items[0].id, "a");
        let mut changed = find(&path, "a").unwrap().unwrap();
        changed.note = "Updated note".to_string();
        update(&path, "a", changed).unwrap();
        assert_eq!(find(&path, "a").unwrap().unwrap().note, "Updated note");
        let markdown = export_markdown(&path).unwrap();
        assert!(markdown.contains("# Codex Thread Studio Favorites"));
        assert!(markdown.contains("A durable event log"));
        cleanup(&path);
    }

    #[test]
    fn allows_multiple_selected_excerpts_from_one_message() {
        let path = database("selection");
        let mut first = favorite("selection-a", "First excerpt", "first");
        first.scope = "selection".to_string();
        let mut second = first.clone();
        second.id = "selection-b".to_string();
        second.title = "Second excerpt".to_string();
        second.content = "second".to_string();
        insert(&path, first).unwrap();
        assert_eq!(insert(&path, second).unwrap().scope, "selection");
        cleanup(&path);
    }

    #[test]
    fn validates_native_source_anchors() {
        assert!(validate(&favorite("a", "Useful", "content")).is_ok());
        let mut configured = favorite("configured", "Configured answer", "content");
        configured.backend = "company-codex".to_string();
        assert!(validate(&configured).is_ok());
        let mut invalid = favorite("a", "", "content");
        assert!(validate(&invalid).is_err());
        invalid = favorite("a", "Useful", "");
        assert!(validate(&invalid).is_err());
        invalid = favorite("a", "Useful", "content");
        invalid.item_id.clear();
        assert!(validate(&invalid).is_err());
    }
}
