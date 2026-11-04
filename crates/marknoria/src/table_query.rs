//! Read-only SQLite over complete indexed tables, with bounded private results.
use crate::Index;
use kuchikiki::traits::TendrilSink;
use rusqlite::{
    Connection, OpenFlags,
    config::DbConfig,
    hooks::{AuthAction, AuthContext, Authorization},
    limits::Limit,
    types::ValueRef,
};
use serde::Serialize;
use std::{
    collections::{HashSet, VecDeque},
    fs::File,
    io::{self, BufRead, BufReader, BufWriter, Read, Seek, SeekFrom, Write},
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use tempfile::TempDir;

const SQL_BYTES: usize = 16 * 1024;
const RESULT_BYTES: u64 = 32 * 1024 * 1024;
fn error(e: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, e.to_string())
}
fn safe_text(value: &str) -> bool {
    !value
        .chars()
        .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'))
}
fn quoted(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Schema {
    pub revision: String,
    pub table: usize,
    pub source_rows: usize,
    pub columns: Vec<String>,
    pub positional_columns: Vec<String>,
    pub default_sql: String,
}
pub fn schema(index: &Index, table: usize) -> io::Result<Schema> {
    let metadata = index
        .tables
        .get(table)
        .ok_or_else(|| error("Table out of range"))?;
    if metadata.columns.len() > 256 {
        return Err(error("SQL supports at most 256 data columns per table"));
    }
    let mut used = HashSet::from(["_row".to_string(), "_line".into()]);
    let mut names = Vec::new();
    for (i, name) in metadata.columns.iter().enumerate() {
        let base = if name.trim().is_empty() {
            format!("column_{}", i + 1)
        } else {
            name.chars().take(256).collect()
        };
        let mut candidate = base.clone();
        let mut suffix = 2;
        while !used.insert(candidate.to_ascii_lowercase()) {
            candidate = format!("{base}_{suffix}");
            suffix += 1;
        }
        names.push(candidate);
    }
    Ok(Schema {
        revision: index.revision.clone(),
        table,
        source_rows: metadata.rows,
        positional_columns: (1..=names.len()).map(|i| format!("c{i}")).collect(),
        columns: names,
        default_sql: if metadata.columns.len() > 254 {
            format!(
                "SELECT {} FROM t",
                (1..=metadata.columns.len())
                    .map(|i| format!("c{i}"))
                    .collect::<Vec<_>>()
                    .join(",")
            )
        } else {
            "SELECT * FROM data".into()
        },
    })
}
struct Input {
    table: usize,
    revision: String,
    directory: TempDir,
}
struct ResultSet {
    id: String,
    table: usize,
    revision: String,
    sql: String,
    columns: Vec<String>,
    total: usize,
    max_row: usize,
    elapsed: f64,
    directory: TempDir,
}
#[derive(Default)]
pub struct Queries {
    input: Option<Input>,
    results: VecDeque<ResultSet>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Window {
    pub kind: &'static str,
    pub id: String,
    pub revision: String,
    pub table: usize,
    pub sql: String,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<Option<String>>>,
    pub row_start: usize,
    pub row_end: usize,
    pub page_size: usize,
    pub total_rows: usize,
    pub elapsed_ms: f64,
}

fn limited(connection: &Connection) -> io::Result<()> {
    for (limit, value) in [
        (Limit::SQLITE_LIMIT_LENGTH, 128 * 1024),
        (Limit::SQLITE_LIMIT_SQL_LENGTH, SQL_BYTES as i32),
        (Limit::SQLITE_LIMIT_COLUMN, 512),
        (Limit::SQLITE_LIMIT_EXPR_DEPTH, 64),
        (Limit::SQLITE_LIMIT_COMPOUND_SELECT, 16),
        (Limit::SQLITE_LIMIT_VDBE_OP, 100_000),
        (Limit::SQLITE_LIMIT_ATTACHED, 0),
        (Limit::SQLITE_LIMIT_LIKE_PATTERN_LENGTH, 1024),
        (Limit::SQLITE_LIMIT_VARIABLE_NUMBER, 512),
        (Limit::SQLITE_LIMIT_WORKER_THREADS, 0),
    ] {
        connection.set_limit(limit, value).map_err(error)?;
    }
    connection
        .set_db_config(DbConfig::SQLITE_DBCONFIG_DEFENSIVE, true)
        .map_err(error)?;
    connection
        .set_db_config(DbConfig::SQLITE_DBCONFIG_TRUSTED_SCHEMA, false)
        .map_err(error)?;
    Ok(())
}
fn build_input(
    parent: &Path,
    index: &Index,
    table: usize,
    cancelled: &AtomicBool,
) -> io::Result<Input> {
    let schema = schema(index, table)?;
    let started = Instant::now();
    let directory = tempfile::Builder::new()
        .prefix("sql-input-")
        .tempdir_in(parent)?;
    let mut db = Connection::open(directory.path().join("data.sqlite")).map_err(error)?;
    limited(&db)?;
    db.execute_batch("PRAGMA page_size=4096; PRAGMA max_page_count=32768; PRAGMA cache_size=-4096; PRAGMA journal_mode=OFF; PRAGMA synchronous=OFF; PRAGMA temp_store=MEMORY;").map_err(error)?;
    db.set_limit(Limit::SQLITE_LIMIT_SQL_LENGTH, 128 * 1024)
        .map_err(error)?;
    let declarations = schema
        .columns
        .iter()
        .map(|c| format!("{} TEXT", quoted(c)))
        .collect::<Vec<_>>()
        .join(",");
    db.execute_batch(&format!("CREATE TABLE data(_row INTEGER PRIMARY KEY, _line INTEGER,{declarations}); CREATE VIEW t AS SELECT _row,_line,{} FROM data;",schema.columns.iter().enumerate().map(|(i,c)|format!("{} AS c{}",quoted(c),i+1)).collect::<Vec<_>>().join(","))).map_err(error)?;
    let tx = db.transaction().map_err(error)?;
    let mut insert = tx
        .prepare(&format!(
            "INSERT INTO data VALUES({})",
            (0..schema.columns.len() + 2)
                .map(|_| "?")
                .collect::<Vec<_>>()
                .join(",")
        ))
        .map_err(error)?;
    let metadata = &index.tables[table];
    let mut number = 0;
    for page in metadata.first_page..metadata.end_page {
        if cancelled.load(Ordering::Relaxed) || started.elapsed() > Duration::from_secs(15) {
            return Err(error(
                "SQL input preparation cancelled or exceeded 15 seconds",
            ));
        }
        let html = crate::read_page(parent, index, page)?;
        let dom = kuchikiki::parse_html().one(html);
        for row in dom.select("tbody tr").map_err(|_| error("Invalid table"))? {
            number += 1;
            let line = row
                .attributes
                .borrow()
                .get("data-source-line")
                .and_then(|s| s.parse::<i64>().ok())
                .unwrap_or(0);
            let mut values = vec![
                rusqlite::types::Value::Integer(number),
                rusqlite::types::Value::Integer(line),
            ];
            values.extend(
                row.as_node()
                    .select("td")
                    .map_err(|_| error("Invalid cells"))?
                    .map(|cell| rusqlite::types::Value::Text(cell.text_contents())),
            );
            if values.len() != schema.columns.len() + 2 {
                return Err(error("Indexed table column count changed"));
            }
            insert
                .execute(rusqlite::params_from_iter(values))
                .map_err(error)?;
        }
    }
    if number as usize != metadata.rows {
        return Err(error("Indexed table row count changed"));
    }
    drop(insert);
    tx.commit().map_err(error)?;
    drop(db);
    Ok(Input {
        table,
        revision: index.revision.clone(),
        directory,
    })
}

fn authorization(context: AuthContext<'_>) -> Authorization {
    match context.action {
        AuthAction::Select | AuthAction::Recursive => Authorization::Allow,
        AuthAction::Read { table_name, .. } if matches!(table_name, "data" | "t") => {
            Authorization::Allow
        }
        AuthAction::Function { function_name }
            if matches!(
                function_name.to_ascii_lowercase().as_str(),
                "count"
                    | "sum"
                    | "total"
                    | "avg"
                    | "min"
                    | "max"
                    | "coalesce"
                    | "ifnull"
                    | "nullif"
                    | "iif"
                    | "if"
                    | "like"
                    | "glob"
                    | "lower"
                    | "upper"
                    | "length"
                    | "instr"
                    | "substr"
                    | "substring"
                    | "replace"
                    | "trim"
                    | "ltrim"
                    | "rtrim"
                    | "round"
                    | "abs"
                    | "typeof"
                    | "unicode"
                    | "hex"
                    | "group_concat"
                    | "string_agg"
                    | "date"
                    | "time"
                    | "datetime"
                    | "strftime"
                    | "julianday"
                    | "unixepoch"
                    | "json_extract"
                    | "json_valid"
                    | "json_array_length"
                    | "row_number"
                    | "rank"
                    | "dense_rank"
                    | "percent_rank"
                    | "cume_dist"
                    | "ntile"
                    | "lag"
                    | "lead"
                    | "first_value"
                    | "last_value"
                    | "nth_value"
            ) =>
        {
            Authorization::Allow
        }
        _ => Authorization::Deny,
    }
}
impl Queries {
    pub fn run(
        &mut self,
        parent: &Path,
        index: &Index,
        table: usize,
        sql: &str,
        cancelled: Arc<AtomicBool>,
    ) -> io::Result<Window> {
        if sql.trim().is_empty() || sql.len() > SQL_BYTES || !safe_text(sql) {
            return Err(error("Enter one SELECT query of at most 16 KiB"));
        }
        let request_started = Instant::now();
        if self
            .input
            .as_ref()
            .is_none_or(|i| i.table != table || i.revision != index.revision)
        {
            self.input = None;
            self.input = Some(build_input(parent, index, table, &cancelled)?);
        }
        let db = Connection::open_with_flags(
            self.input
                .as_ref()
                .unwrap()
                .directory
                .path()
                .join("data.sqlite"),
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(error)?;
        limited(&db)?;
        // Load our own quoted schema before applying the user SQL-size limit:
        // wide headers can make trusted CREATE statements longer than 16 KiB.
        db.set_limit(Limit::SQLITE_LIMIT_SQL_LENGTH, 128 * 1024)
            .map_err(error)?;
        {
            let _schema = db.prepare("SELECT _row FROM data LIMIT 0").map_err(error)?;
        }
        db.set_limit(Limit::SQLITE_LIMIT_SQL_LENGTH, SQL_BYTES as i32)
            .map_err(error)?;
        db.execute_batch(
            "PRAGMA query_only=ON; PRAGMA cache_size=-4096; PRAGMA temp_store=MEMORY;",
        )
        .map_err(error)?;
        // The database contains only the current table. No file/extension,
        // pragma, schema, mutation or arbitrary allocator functions are exposed.
        db.authorizer(Some(authorization)).map_err(error)?;
        let started = Instant::now();
        let mut instructions = 0u64;
        db.progress_handler(
            1000,
            Some(move || {
                instructions += 1000;
                cancelled.load(Ordering::Relaxed)
                    || started.elapsed() > Duration::from_secs(5)
                    || instructions > 20_000_000
            }),
        )
        .map_err(error)?;
        let mut statement = db.prepare(sql).map_err(error)?;
        if !statement.readonly() || statement.column_count() == 0 {
            return Err(error("Only read-only SELECT queries are supported"));
        }
        let columns = statement
            .column_names()
            .into_iter()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        if columns.len() > 256
            || columns.iter().map(String::len).sum::<usize>() > SQL_BYTES
            || columns.iter().any(|name| !safe_text(name))
        {
            return Err(error("Query columns exceed the display budget"));
        }
        let directory = tempfile::Builder::new()
            .prefix("sql-result-")
            .tempdir_in(parent)?;
        let mut rows_file = BufWriter::new(File::create(directory.path().join("rows.jsonl"))?);
        let mut offsets = BufWriter::new(File::create(directory.path().join("offsets.bin"))?);
        let mut result = statement.query([]).map_err(error)?;
        let mut total = 0;
        let mut bytes = 0u64;
        let mut max_row = 1;
        while let Some(row) = result.next().map_err(error)? {
            let mut cells = Vec::with_capacity(columns.len());
            for column in 0..columns.len() {
                let cell = match row.get_ref(column).map_err(error)? {
                    ValueRef::Null => None,
                    ValueRef::Integer(n) => Some(n.to_string()),
                    ValueRef::Real(n) => Some(n.to_string()),
                    ValueRef::Text(bytes) => {
                        let text = std::str::from_utf8(bytes).map_err(error)?;
                        if !safe_text(text) {
                            return Err(error(
                                "Query produced control characters; use hex() to display binary values",
                            ));
                        }
                        Some(text.to_owned())
                    }
                    ValueRef::Blob(bytes) => Some(format!(
                        "0x{}",
                        bytes.iter().map(|b| format!("{b:02x}")).collect::<String>()
                    )),
                };
                cells.push(cell);
            }
            let encoded = serde_json::to_vec(&cells)?;
            if total >= 1_000_000
                || encoded.len() > crate::MAX_HTML / 2
                || bytes + encoded.len() as u64 + 1 > RESULT_BYTES
            {
                return Err(error(
                    "Query result exceeds 1,000,000 rows, 64 KiB per row or 32 MiB; narrow the query with WHERE or LIMIT",
                ));
            }
            offsets.write_all(&bytes.to_le_bytes())?;
            rows_file.write_all(&encoded)?;
            rows_file.write_all(b"\n")?;
            max_row = max_row.max(encoded.len() + 1);
            bytes += encoded.len() as u64 + 1;
            total += 1;
        }
        rows_file.flush()?;
        offsets.flush()?;
        let snapshot = ResultSet {
            id: uuid::Uuid::new_v4().to_string(),
            table,
            revision: index.revision.clone(),
            sql: sql.into(),
            columns,
            total,
            max_row,
            elapsed: request_started.elapsed().as_secs_f64() * 1000.0,
            directory,
        };
        let window = snapshot.window(0)?;
        self.results.push_back(snapshot);
        if self.results.len() > 4 {
            self.results.pop_front();
        }
        Ok(window)
    }
    pub fn page(&self, index: &Index, id: &str, row: usize) -> io::Result<Window> {
        self.results
            .iter()
            .find(|r| r.id == id && r.revision == index.revision)
            .ok_or_else(|| error("Query result expired; run the query again"))?
            .window(row)
    }
    pub fn close(&mut self, id: &str) {
        self.results.retain(|r| r.id != id);
    }
}
impl ResultSet {
    fn window(&self, row: usize) -> io::Result<Window> {
        if row >= self.total.max(1) {
            return Err(error("Query result row is out of range"));
        }
        let metadata_bytes = serde_json::to_vec(&(
            &self.id,
            &self.revision,
            self.table,
            &self.sql,
            &self.columns,
            self.total,
            self.elapsed,
        ))?
        .len()
            + 1024;
        let available = crate::MAX_HTML.saturating_sub(metadata_bytes);
        if self.total > 0 && self.max_row > available {
            return Err(error(
                "Query page exceeds the 128 KiB transport budget; select fewer columns or shorter aliases",
            ));
        }
        let page_size = 50
            .min(512 / self.columns.len().max(1))
            .min(available / self.max_row)
            .max(1);
        let row_start = row / page_size * page_size;
        let row_end = (row_start + page_size).min(self.total);
        let mut rows = Vec::new();
        if self.total > 0 {
            let mut offsets = File::open(self.directory.path().join("offsets.bin"))?;
            offsets.seek(SeekFrom::Start(row_start as u64 * 8))?;
            let mut b = [0; 8];
            offsets.read_exact(&mut b)?;
            let mut file = File::open(self.directory.path().join("rows.jsonl"))?;
            file.seek(SeekFrom::Start(u64::from_le_bytes(b)))?;
            let mut reader = BufReader::new(file);
            for _ in row_start..row_end {
                let mut line = String::new();
                reader
                    .by_ref()
                    .take(self.max_row as u64 + 1)
                    .read_line(&mut line)?;
                rows.push(serde_json::from_str(&line)?);
            }
        }
        Ok(Window {
            kind: "tableQuery",
            id: self.id.clone(),
            revision: self.revision.clone(),
            table: self.table,
            sql: self.sql.clone(),
            columns: self.columns.clone(),
            rows,
            row_start,
            row_end,
            page_size,
            total_rows: self.total,
            elapsed_ms: self.elapsed,
        })
    }
}
