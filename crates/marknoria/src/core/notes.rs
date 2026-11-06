//! Local comments/favorites. No agent, network, or source-file write capability.
mod marks;
pub use marks::{TextRange, TextScope, TextSegment};
use crate::core::Index;
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    io,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};
fn error(e: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, e.to_string())
}
fn is_busy(e: &rusqlite::Error) -> bool {
    matches!(e.sqlite_error_code(), Some(rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked))
}
fn sql_error(e: rusqlite::Error) -> io::Error {
    io::Error::new(if is_busy(&e) { io::ErrorKind::WouldBlock } else { io::ErrorKind::InvalidInput }, e)
}
fn is_false(value: &bool) -> bool { !*value }
/// Durable SQL output reference. Result coordinates are never source-cell
/// coordinates; the query is retained as data and is not re-executed on open.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QueryProvenance {
    pub table: usize,
    pub sql: String,
    pub row: usize,
    pub column: usize,
    pub column_name: String,
    pub total_rows: usize,
    pub source_rows: usize,
    pub value_is_null: bool,
}
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct Anchor {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_range: Option<TextRange>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query: Option<QueryProvenance>,
    #[serde(skip_serializing_if = "is_false")]
    pub quote_truncated: bool,
    pub location: Option<crate::core::reader::Location>,
    pub line: Option<usize>,
    pub page: Option<usize>,
    pub table: Option<usize>,
    pub row: Option<usize>,
    pub column: Option<usize>,
    pub label: Option<String>,
}
#[derive(Debug, Deserialize)]
#[serde(tag = "operation", rename_all = "camelCase", deny_unknown_fields)]
pub enum Request {
    Marks { revision: String, scopes: Vec<String>, #[serde(default)] offset: usize },
    Get { id: String },
    List {
        #[serde(default)]
        offset: usize,
        kind: Option<String>,
    },
    Add {
        kind: String,
        quote: String,
        #[serde(default)]
        comment: String,
        #[serde(default)]
        anchor: Anchor,
        revision: String,
    },
    Update {
        id: String,
        comment: String,
    },
    Delete {
        id: String,
    },
    Clear {
        kind: Option<String>,
    },
    Copy {
        id: String,
    },
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Note {
    id: String,
    kind: String,
    quote: String,
    comment: String,
    anchor: Anchor,
    revision: String,
    created: u64,
    stale: bool,
}
fn db(directory: &Path) -> io::Result<Connection> {
    let mut dir = std::fs::DirBuilder::new();
    dir.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        dir.mode(0o700);
    }
    dir.create(directory)?;
    // Only idempotent connection/schema setup is retried. Simultaneous first
    // opens can race while SQLite switches journals or creates the schema;
    // BUSY during an upgrade is not always handled by SQLite's busy timeout.
    // Drop the connection/snapshot before retrying; never replay user mutations.
    let started = std::time::Instant::now();
    let mut delay = 5;
    loop {
        let result = (|| -> rusqlite::Result<Connection> {
            let db = Connection::open(directory.join("notes.sqlite"))?;
            db.busy_timeout(std::time::Duration::from_millis(100))?;
            let journal: String = db.query_row("PRAGMA journal_mode", [], |row| row.get(0))?;
            if !journal.eq_ignore_ascii_case("wal") {
                db.query_row("PRAGMA journal_mode=WAL", [], |row| row.get::<_, String>(0))?;
            }
            db.execute_batch("PRAGMA foreign_keys=ON; PRAGMA journal_size_limit=8388608; PRAGMA max_page_count=16384; PRAGMA cache_size=-1024;")?;
            let version: i64 = db.query_row("PRAGMA user_version", [], |row| row.get(0))?;
            if version < 1 {
                let transaction = rusqlite::Transaction::new_unchecked(&db, rusqlite::TransactionBehavior::Immediate)?;
                let version: i64 = transaction.query_row("PRAGMA user_version", [], |row| row.get(0))?;
                if version < 1 {
                    transaction.execute_batch("CREATE TABLE IF NOT EXISTS notes(id TEXT PRIMARY KEY, document TEXT NOT NULL, revision TEXT NOT NULL, kind TEXT NOT NULL, quote TEXT NOT NULL, comment TEXT NOT NULL, anchor TEXT NOT NULL, created INTEGER NOT NULL, dedup TEXT UNIQUE NOT NULL); CREATE INDEX IF NOT EXISTS notes_document ON notes(document,created); CREATE TABLE IF NOT EXISTS comment_scopes(document TEXT NOT NULL,revision TEXT NOT NULL,scope TEXT NOT NULL,note_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,PRIMARY KEY(note_id,scope)); CREATE INDEX IF NOT EXISTS comment_scope_lookup ON comment_scopes(document,revision,scope); CREATE TABLE IF NOT EXISTS comment_scope_documents(document TEXT PRIMARY KEY); PRAGMA user_version=1;")?;
                }
                transaction.commit()?;
            }
            db.busy_timeout(std::time::Duration::from_secs(2))?;
            Ok(db)
        })();
        match result {
            Ok(db) => return Ok(db),
            Err(e) if started.elapsed() < std::time::Duration::from_secs(2)
                && is_busy(&e) => {
                std::thread::sleep(std::time::Duration::from_millis(delay));
                delay = (delay * 2).min(80);
            }
            Err(e) => return Err(sql_error(e)),
        }
    }
}
fn text(value: &str, max: usize) -> io::Result<()> {
    if value.len() > max || value.contains('\0') {
        return Err(error("Comment or quote exceeds its text budget"));
    }
    Ok(())
}
fn identity(id: &str) -> io::Result<()> {
    uuid::Uuid::parse_str(id).map_err(error)?;
    Ok(())
}
fn note(row: &rusqlite::Row<'_>, index: &Index) -> rusqlite::Result<Note> {
    let revision: String = row.get(6)?;
    let anchor: String = row.get(4)?;
    Ok(Note {
        id: row.get(0)?,
        kind: row.get(1)?,
        quote: row.get(2)?,
        comment: row.get(3)?,
        anchor: serde_json::from_str(&anchor).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(4, rusqlite::types::Type::Text, Box::new(e))
        })?,
        created: row.get(5)?,
        stale: revision != index.revision,
        revision,
    })
}
const SELECT: &str = "SELECT id,kind,quote,comment,anchor,created,revision FROM notes";
pub fn execute(
    directory: &Path,
    path: &Path,
    index: &Index,
    request: Request,
) -> io::Result<Value> {
    let document = path.to_string_lossy();
    execute_named(directory, document.as_ref(), &path.file_name().unwrap_or_default().to_string_lossy(), index, request)
}

/// Host-owned stable identity, including immutable in-memory documents. File
/// callers keep using `execute` so existing notebook keys do not change.
pub fn execute_for_identity(
    directory: &Path,
    document_key: &str,
    index: &Index,
    request: Request,
) -> io::Result<Value> {
    execute_named(directory, document_key, &index.name, index, request)
}
fn execute_named(directory: &Path, document_key: &str, display_name: &str, index: &Index, request: Request) -> io::Result<Value> {
    if document_key.is_empty() || document_key.len() > 32768 || document_key.contains('\0') {
        return Err(error("Invalid notebook document identity"));
    }
    let document: std::borrow::Cow<'_, str> = std::borrow::Cow::Borrowed(document_key);
    let db = db(directory)?;
    match request {
        Request::Marks { revision, scopes, offset } => marks::read(&db, document.as_ref(), index, &revision, scopes, offset),
        Request::Get { id } => {
            identity(&id)?;
            db.query_row(&format!("{SELECT} WHERE id=? AND document=?"), params![id,document.as_ref()], |row| note(row,index))
                .map_err(sql_error).and_then(|note| serde_json::to_value(note).map_err(error))
        }
        Request::List { offset, kind } => {
            if offset > 10000
                || kind
                    .as_ref()
                    .is_some_and(|k| !matches!(k.as_str(), "comment" | "favorite"))
            {
                return Err(error("Invalid note filter"));
            }
            let total: usize = db
                .query_row(
                    "SELECT COUNT(*) FROM notes WHERE document=? AND (? IS NULL OR kind=?)",
                    params![document.as_ref(), kind, kind],
                    |r| r.get(0),
                )
                .map_err(sql_error)?;
            let mut statement=db.prepare(&format!("{SELECT} WHERE document=? AND (? IS NULL OR kind=?) ORDER BY created DESC,id LIMIT 64 OFFSET ?")).map_err(sql_error)?;
            let rows = statement
                .query_map(params![document.as_ref(), kind, kind, offset], |r| {
                    note(r, index)
                })
                .map_err(sql_error)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(sql_error)?;
            let document_total: usize = db
                .query_row(
                    "SELECT COUNT(*) FROM notes WHERE document=?",
                    [document.as_ref()],
                    |r| r.get(0),
                )
                .map_err(sql_error)?;
            Ok(
                json!({"kind":"notes","items":rows,"total":total,"documentTotal":document_total,"offset":offset,"limit":64}),
            )
        }
        Request::Add {
            kind,
            quote,
            comment,
            mut anchor,
            revision,
        } => {
            if !matches!(kind.as_str(), "comment" | "favorite")
                || revision != index.revision
                || (quote.trim().is_empty() && !matches!(anchor.location, Some(crate::core::reader::Location::Pdf { .. })))
            {
                return Err(error("Choose text from the current document revision"));
            }
            text(&quote, 32 * 1024)?;
            text(&comment, 16 * 1024)?;
            if let Some(query) = &anchor.query {
                text(&query.sql, crate::core::table_query::SQL_BYTES)?;
                text(&query.column_name, 4096)?;
                if anchor.location.is_some() || anchor.table.is_some() || anchor.line.is_some()
                    || anchor.page.is_some() || anchor.row.is_some() || anchor.column.is_some()
                    || query.sql.trim().is_empty() || query.row >= query.total_rows
                    || query.column >= 256 || !index.tables.get(query.table)
                        .is_some_and(|table| table.rows == query.source_rows)
                {
                    return Err(error("Invalid durable SQL result provenance"));
                }
            }
            if let Some(location) = &anchor.location {
                use crate::core::{document_format::DocumentFormat, reader::Location};
                location.validate()?;
                let valid = match location {
                    Location::Pdf { pages } => index.format == DocumentFormat::Pdf && pages.iter().all(|p| p.page <= 2000),
                    Location::Epub { .. } => index.format == DocumentFormat::Epub,
                    Location::EpubChapter { .. } => index.format == DocumentFormat::Epub,
                    Location::Source { end_byte, .. } => index.format.is_text() && *end_byte <= index.source_bytes,
                    Location::Line { line } => *line <= index.source_lines,
                    Location::Cell { table, row, column } => index.tables.get(*table).is_some_and(|t| *row < t.rows && *column < t.columns.len()),
                    Location::Projection { page, .. } => *page < index.pages.len(),
                    Location::QueryCell { .. } => false,
                };
                if !valid { return Err(error("Typed note location is outside the document")); }
            }
            if let Some(range) = &anchor.text_range { range.validate(index, &anchor)?; }
            if anchor
                .line
                .is_some_and(|n| n == 0 || n > index.source_lines)
                || anchor.page.is_some_and(|n| n >= index.pages.len())
                || anchor.table.is_some_and(|n| n >= index.tables.len())
                || anchor.column.is_some_and(|n| n >= 256)
                || anchor.row.is_some_and(|n| n >= 1_000_000)
            {
                return Err(error("Note anchor is outside the document"));
            }
            if let Some(table) = anchor.table.and_then(|id| index.tables.get(id)) {
                if anchor.row.is_some_and(|r| r >= table.rows)
                    || anchor.column.is_some_and(|c| c >= table.columns.len())
                {
                    return Err(error("Note cell is outside its table"));
                }
            }
            anchor.label = if let Some(query) = &anchor.query {
                Some(format!("SQL result · row {} · column {} ({})", query.row + 1, query.column + 1, query.column_name))
            } else if let Some(crate::core::reader::Location::Pdf { pages }) = &anchor.location {
                Some(format!("PDF page {}", pages[0].page))
            } else if let Some(crate::core::reader::Location::Epub { .. }) = &anchor.location {
                Some("EPUB selection".into())
            } else if let Some(crate::core::reader::Location::EpubChapter { href }) = &anchor.location {
                Some(href.clone())
            } else if let Some(table) = anchor.table.and_then(|id| index.tables.get(id)) {
                Some(format!(
                    "{}{}{}",
                    table.label.as_deref().unwrap_or("Table"),
                    anchor
                        .row
                        .map(|r| format!(" · row {}", r + 1))
                        .unwrap_or_default(),
                    anchor
                        .column
                        .map(|c| format!(" · column {}", c + 1))
                        .unwrap_or_default()
                ))
            } else {
                anchor
                    .page
                    .and_then(|p| index.toc.iter().filter(|h| h.page <= p).last())
                    .map(|h| h.label.clone())
            };
            let id = uuid::Uuid::new_v4().to_string();
            let scopes = if kind == "comment" { marks::keys(index, &anchor)? } else { Vec::new() };
            let anchor = serde_json::to_string(&anchor)?;
            if anchor.len() > 32768 { return Err(error("Note anchor exceeds its transport budget")); }
            let key = if kind == "favorite" {
                Sha256::digest(serde_json::to_vec(&(
                    document.as_ref(),
                    &revision,
                    &quote,
                    &anchor,
                ))?)
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>()
            } else {
                id.clone()
            };
            let created = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(error)?
                .as_millis() as u64;
            let transaction = rusqlite::Transaction::new_unchecked(&db, rusqlite::TransactionBehavior::Immediate).map_err(sql_error)?;
            let count: u64 = transaction
                .query_row("SELECT COUNT(*) FROM notes", [], |r| r.get(0))
                .map_err(sql_error)?;
            if count >= 10000 {
                return Err(error("Local notebook exceeds 10,000 items"));
            }
            // A new notebook has no legacy comments to migrate. Publish that
            // fact atomically with its first note so readers need no write lock.
            transaction.execute("INSERT OR IGNORE INTO comment_scope_documents(document) SELECT ? WHERE NOT EXISTS(SELECT 1 FROM notes WHERE document=? AND kind='comment')", params![document.as_ref(),document.as_ref()]).map_err(sql_error)?;
            transaction.execute(
                "INSERT INTO notes VALUES(?,?,?,?,?,?,?,?,?) ON CONFLICT(dedup) DO NOTHING",
                params![
                    id,
                    document.as_ref(),
                    revision,
                    kind,
                    quote,
                    comment,
                    anchor,
                    created,
                    key
                ],
            )
            .map_err(sql_error)?;
            let id: String = transaction
                .query_row("SELECT id FROM notes WHERE dedup=?", [key], |r| r.get(0))
                .map_err(sql_error)?;
            marks::register(&transaction, document.as_ref(), &revision, &id, &scopes)?;
            transaction.commit().map_err(sql_error)?;
            Ok(json!({"kind":"noteSaved","id":id}))
        }
        Request::Update { id, comment } => {
            identity(&id)?;
            text(&comment, 16 * 1024)?;
            if db
                .execute(
                    "UPDATE notes SET comment=? WHERE id=? AND document=?",
                    params![comment, id, document.as_ref()],
                )
                .map_err(sql_error)?
                == 0
            {
                return Err(error("Note not found in this document"));
            }
            Ok(json!({"kind":"noteSaved","id":id}))
        }
        Request::Delete { id } => {
            identity(&id)?;
            if db
                .execute(
                    "DELETE FROM notes WHERE id=? AND document=?",
                    params![id, document.as_ref()],
                )
                .map_err(sql_error)?
                == 0
            {
                return Err(error("Note not found in this document"));
            }
            Ok(json!({"kind":"noteDeleted","id":id}))
        }
        Request::Clear { kind } => {
            if kind
                .as_ref()
                .is_some_and(|k| !matches!(k.as_str(), "comment" | "favorite"))
            {
                return Err(error("Invalid note filter"));
            }
            let deleted = db
                .execute(
                    "DELETE FROM notes WHERE document=? AND (? IS NULL OR kind=?)",
                    params![document.as_ref(), kind, kind],
                )
                .map_err(sql_error)?;
            Ok(json!({"kind":"notesCleared","deleted":deleted}))
        }
        Request::Copy { id } => {
            identity(&id)?;
            let record = db
                .query_row(
                    &format!("{SELECT} WHERE id=? AND document=?"),
                    params![id, document.as_ref()],
                    |r| note(r, index),
                )
                .map_err(sql_error)?;
            let location = record
                .anchor
                .line
                .map(|n| format!(" · line {n}"))
                .or_else(|| {
                    record
                        .anchor
                        .label
                        .as_ref()
                        .map(|label| format!(" · {label}"))
                })
                .or_else(|| record.anchor.page.map(|n| format!(" · page {}", n + 1)))
                .unwrap_or_default();
            let quote = record
                .quote
                .lines()
                .map(|line| format!("> {line}"))
                .collect::<Vec<_>>()
                .join("\n");
            let quote = if record.anchor.quote_truncated { format!("Selected text excerpt (truncated):\n\n{quote}") } else { quote };
            let comment = if record.comment.is_empty() {
                String::new()
            } else {
                format!("\n\n{}", record.comment)
            };
            let typed = if let Some(query) = &record.anchor.query {
                format!("\n\nSQL result provenance (snapshot, not a source cell): {}", serde_json::to_string(query).map_err(error)?)
            } else {
                record.anchor.location.as_ref().map(|value| serde_json::to_string(value).map(|json| format!("\n\nLocation: {json}"))).transpose().map_err(error)?.unwrap_or_default()
            };
            Ok(json!({"kind":"noteCopy","text":format!("{}{}\n\n{}{}{}",display_name,location,quote,comment,typed)}))
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn clear_removes_all_kinds_and_pages_only_for_current_document() {
        let dir = tempfile::tempdir().unwrap();
        let index = Index {
            revision: "r".into(),
            ..Default::default()
        };
        // Seed multiple result pages in one transaction; do not fsync per fixture.
        let mut connection = db(dir.path()).unwrap();
        let transaction = connection.transaction().unwrap();
        for document in ["one.md", "two.md", "three.md"] {
            for n in 0..70 {
                let id = uuid::Uuid::new_v4().to_string();
                transaction.execute("INSERT INTO notes VALUES(?,?,?,?,?,?,?,?,?)", params![id, document, "r", if n % 2 == 0 { "favorite" } else { "comment" }, format!("Quote {n}"), "", "{}", n, id]).unwrap();
            }
        }
        transaction.commit().unwrap();
        drop(connection);
        let filtered = execute(
            dir.path(),
            Path::new("one.md"),
            &index,
            Request::List {
                offset: 0,
                kind: Some("favorite".into()),
            },
        )
        .unwrap();
        assert_eq!(filtered["total"], 35);
        assert_eq!(filtered["documentTotal"], 70);
        assert!(execute(dir.path(), Path::new("one.md"), &index, Request::Clear { kind: Some("invalid".into()) }).is_err());
        assert_eq!(
            execute(
                dir.path(),
                Path::new("one.md"),
                &index,
                Request::Clear {
                    kind: Some("comment".into())
                }
            )
            .unwrap()["deleted"],
            35
        );
        assert_eq!(
            execute(
                dir.path(),
                Path::new("one.md"),
                &index,
                Request::List {
                    offset: 0,
                    kind: Some("favorite".into())
                }
            )
            .unwrap()["total"],
            35
        );
        assert_eq!(
            execute(
                dir.path(),
                Path::new("one.md"),
                &index,
                Request::Clear {
                    kind: Some("favorite".into())
                }
            )
            .unwrap()["deleted"],
            35
        );
        assert_eq!(
            execute(
                dir.path(),
                Path::new("two.md"),
                &index,
                Request::Clear { kind: None }
            )
            .unwrap()["deleted"],
            70
        );
        assert_eq!(
            execute(
                dir.path(),
                Path::new("one.md"),
                &index,
                Request::List {
                    offset: 0,
                    kind: None
                }
            )
            .unwrap()["total"],
            0
        );
        assert_eq!(
            execute(
                dir.path(),
                Path::new("three.md"),
                &index,
                Request::List {
                    offset: 0,
                    kind: None
                }
            )
            .unwrap()["total"],
            70
        );
        assert_eq!(
            execute(
                dir.path(),
                Path::new("one.md"),
                &index,
                Request::Clear { kind: None }
            )
            .unwrap()["deleted"],
            0
        );
    }
    #[test]
    fn persistence_copy_revision_and_document_isolation() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("document.md");
        let mut index = Index {
            revision: "revision1".into(),
            source_lines: 3,
            ..Default::default()
        };
        let saved = execute(
            dir.path(),
            &path,
            &index,
            Request::Add {
                kind: "comment".into(),
                quote: "中文😀\r\n001".into(),
                comment: "A note".into(),
                anchor: Anchor {
                    line: Some(2),
                    ..Default::default()
                },
                revision: index.revision.clone(),
            },
        )
        .unwrap();
        let id = saved["id"].as_str().unwrap().to_string();
        assert_eq!(
            execute(
                dir.path(),
                &path,
                &index,
                Request::List {
                    offset: 0,
                    kind: None
                }
            )
            .unwrap()["total"],
            1
        );
        let copy = execute(dir.path(), &path, &index, Request::Copy { id: id.clone() }).unwrap();
        assert!(
            copy["text"]
                .as_str()
                .unwrap()
                .contains("> 中文😀\n> 001\n\nA note")
        );
        assert!(
            execute(
                dir.path(),
                Path::new("other"),
                &index,
                Request::Copy { id: id.clone() }
            )
            .is_err()
        );
        index.revision = "revision2".into();
        assert_eq!(
            execute(
                dir.path(),
                &path,
                &index,
                Request::List {
                    offset: 0,
                    kind: None
                }
            )
            .unwrap()["items"][0]["stale"],
            true
        );
        execute(
            dir.path(),
            &path,
            &index,
            Request::Update {
                id: id.clone(),
                comment: "Updated".into(),
            },
        )
        .unwrap();
        execute(dir.path(), &path, &index, Request::Delete { id }).unwrap();
        assert_eq!(
            execute(
                dir.path(),
                &path,
                &index,
                Request::List {
                    offset: 0,
                    kind: None
                }
            )
            .unwrap()["total"],
            0
        );
    }
    #[test]
    fn favorites_deduplicate_and_reject_stale_or_oversized_input() {
        let d = tempfile::tempdir().unwrap();
        let i = Index {
            revision: "r".into(),
            ..Default::default()
        };
        for _ in 0..2 {
            execute(
                d.path(),
                Path::new("a.md"),
                &i,
                Request::Add {
                    kind: "favorite".into(),
                    quote: "Quote".into(),
                    comment: String::new(),
                    anchor: Anchor::default(),
                    revision: "r".into(),
                },
            )
            .unwrap();
        }
        assert_eq!(
            execute(
                d.path(),
                Path::new("a.md"),
                &i,
                Request::List {
                    offset: 0,
                    kind: None
                }
            )
            .unwrap()["total"],
            1
        );
        assert!(
            execute(
                d.path(),
                Path::new("a.md"),
                &i,
                Request::Add {
                    kind: "comment".into(),
                    quote: "x".repeat(32769),
                    comment: String::new(),
                    anchor: Anchor::default(),
                    revision: "r".into()
                }
            )
            .is_err()
        );
    }
}
