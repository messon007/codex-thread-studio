//! Read-only SQLite over complete indexed tables, with bounded private results.
mod chart;
pub use chart::{Chart, ChartPoint, ChartSource, MAX_CHART_POINTS};
use crate::core::{Index, blocks::Block};
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

pub(crate) const SQL_BYTES: usize = 16 * 1024;
pub(crate) const RESULT_BYTES: u64 = 32 * 1024 * 1024;
const INPUT_TEXT_BYTES: usize = 128 * 1024;
// A JSON control character can occupy six bytes. Reserve punctuation, the
// physical line number and up to256 cells without increasing SQLite's limit.
const INPUT_RECORD_BYTES: usize = 6 * INPUT_TEXT_BYTES + 4096;

fn read_input_record(input: &mut impl BufRead, buffer: &mut Vec<u8>) -> io::Result<Option<crate::core::documents::TableRecord>> {
    buffer.clear();
    input.take(INPUT_RECORD_BYTES as u64 + 1).read_until(b'\n', buffer)?;
    if buffer.is_empty() {
        return Ok(None);
    }
    if buffer.len() > INPUT_RECORD_BYTES {
        return Err(error("Complete table row exceeds the serialized SQL input budget"));
    }
    let record: crate::core::documents::TableRecord = serde_json::from_slice(buffer)?;
    let bytes = record.cells.iter().try_fold(0usize, |total, cell| total.checked_add(cell.len()));
    if bytes.is_none_or(|bytes| bytes > INPUT_TEXT_BYTES) {
        return Err(error("Complete table row exceeds the SQL input text budget"));
    }
    Ok(Some(record))
}
fn error(e: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, e.to_string())
}
// SQLite offsets are UTF-8 bytes; report human-readable character columns.
fn query_error(e: rusqlite::Error) -> io::Error {
    if let rusqlite::Error::SqlInputError { msg, sql, offset, .. } = &e {
        if let Ok(offset) = usize::try_from(*offset) {
            if let Some(prefix) = sql.get(..offset) {
                let line = prefix.bytes().filter(|&b| b == b'\n').count() + 1;
                let column = prefix.rsplit('\n').next().unwrap_or("").chars().count() + 1;
                return error(format!("{msg} at line {line}, column {column}"));
            }
        }
        return error(msg);
    }
    error(e)
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
        (Limit::SQLITE_LIMIT_LENGTH, INPUT_TEXT_BYTES as i32),
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
    db.execute_batch("PRAGMA hard_heap_limit=67108864; PRAGMA page_size=4096; PRAGMA max_page_count=32768; PRAGMA cache_size=-4096; PRAGMA journal_mode=OFF; PRAGMA synchronous=OFF; PRAGMA temp_store=MEMORY;").map_err(error)?;
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
    if let Some(file) = &metadata.data_file {
        let file = File::open(parent.join(file))?;
        let mut input = BufReader::new(file);
        let mut buffer = Vec::new();
        loop {
            if cancelled.load(Ordering::Relaxed) || started.elapsed()>Duration::from_secs(15) {return Err(error("SQL input preparation cancelled or exceeded 15 seconds"));}
            let Some(record) = read_input_record(&mut input, &mut buffer)? else { break };
            if cancelled.load(Ordering::Relaxed) || started.elapsed()>Duration::from_secs(15) {return Err(error("SQL input preparation cancelled or exceeded 15 seconds"));}
            if record.cells.len()!=schema.columns.len() { return Err(error("Indexed table column count changed")); }
            number+=1;
            let mut values=vec![rusqlite::types::Value::Integer(number as i64),rusqlite::types::Value::Integer(record.line as i64)];
            values.extend(record.cells.into_iter().map(rusqlite::types::Value::Text));
            insert.execute(rusqlite::params_from_iter(values)).map_err(|e| {
                if e.sqlite_error_code() == Some(rusqlite::ErrorCode::TooBig) {
                    error("Complete table row exceeds the SQLite record budget")
                } else { error(e) }
            })?;
        }
    } else if let Some(delimiter) = crate::core::csv_document::indexed_delimiter(index) {
        let mut csv = csv::ReaderBuilder::new()
            .delimiter(delimiter)
            .flexible(true)
            .from_path(parent.join("source.md"))
            .map_err(error)?;
        csv.headers().map_err(error)?;
        let mut lines = BufReader::new(File::open(parent.join("csv-lines.bin"))?);
        let mut record = csv::StringRecord::new();
        while csv.read_record(&mut record).map_err(error)? {
            if number % 1000 == 0
                && (cancelled.load(Ordering::Relaxed)
                    || started.elapsed() > Duration::from_secs(15))
            {
                return Err(error(
                    "SQL input preparation cancelled or exceeded 15 seconds",
                ));
            }
            number += 1;
            let mut line = [0; 8];
            lines.read_exact(&mut line)?;
            let mut values = vec![
                rusqlite::types::Value::Integer(number as i64),
                rusqlite::types::Value::Integer(u64::from_le_bytes(line) as i64),
            ];
            values.extend(
                (0..schema.columns.len())
                    .map(|i| rusqlite::types::Value::Text(record.get(i).unwrap_or("").to_owned())),
            );
            insert
                .execute(rusqlite::params_from_iter(values))
                .map_err(error)?;
        }
    } else {
        for page in metadata.first_page..metadata.end_page {
            if cancelled.load(Ordering::Relaxed) || started.elapsed() > Duration::from_secs(15) {
                return Err(error(
                    "SQL input preparation cancelled or exceeded 15 seconds",
                ));
            }
            for block in crate::core::blocks::read_blocks(parent, index, page)? {
                if let Block::Table {
                    rows, row_lines, ..
                } = block
                {
                    for (r, row) in rows.iter().enumerate() {
                        number += 1;
                        let mut values = vec![
                            rusqlite::types::Value::Integer(number as i64),
                            rusqlite::types::Value::Integer(
                                row_lines.get(r).copied().unwrap_or(0) as i64
                            ),
                        ];
                        values.extend(row.iter().map(|spans| {
                            rusqlite::types::Value::Text(
                                spans.iter().map(|s| s.text.as_str()).collect(),
                            )
                        }));
                        if values.len() != schema.columns.len() + 2 {
                            return Err(error("Indexed table column count changed"));
                        }
                        insert
                            .execute(rusqlite::params_from_iter(values))
                            .map_err(error)?;
                    }
                }
            }
        }
    }
    if number != metadata.rows {
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn complete_table_records_are_bounded_before_json_parsing() {
        let mut input = io::Cursor::new(vec![b'x'; INPUT_RECORD_BYTES + 4096]);
        let mut buffer = Vec::new();
        let failure = read_input_record(&mut input, &mut buffer).err().unwrap();
        assert!(failure.to_string().contains("serialized SQL input budget"));
        assert_eq!(input.position(), INPUT_RECORD_BYTES as u64 + 1);
        assert_eq!(buffer.len(), INPUT_RECORD_BYTES + 1);

        let record = crate::core::documents::TableRecord {
            line: 42, cells: vec!["x".repeat(INPUT_TEXT_BYTES + 1)],
        };
        let mut input = io::Cursor::new(serde_json::to_vec(&record).unwrap());
        assert!(read_input_record(&mut input, &mut buffer).err().unwrap().to_string().contains("SQL input text budget"));
    }

    #[test]
    fn complete_table_records_preserve_escaped_text_tail_and_next_record() {
        let value = format!("{}汉😀TAIL", "\u{0001}".repeat(INPUT_TEXT_BYTES - 20));
        let first = crate::core::documents::TableRecord { line: 3, cells: vec![value.clone()] };
        let second = crate::core::documents::TableRecord { line: 4, cells: vec!["001".into(), "".into()] };
        let mut source = serde_json::to_vec(&first).unwrap();
        assert!(source.len() > INPUT_TEXT_BYTES);
        source.push(b'\n');
        source.extend(serde_json::to_vec(&second).unwrap());
        let mut input = io::Cursor::new(source);
        let mut buffer = Vec::new();
        let first = read_input_record(&mut input, &mut buffer).unwrap().unwrap();
        assert_eq!((first.line, first.cells), (3, vec![value]));
        let second = read_input_record(&mut input, &mut buffer).unwrap().unwrap();
        assert_eq!((second.line, second.cells), (4, vec!["001".into(), "".into()]));
        assert!(read_input_record(&mut input, &mut buffer).unwrap().is_none());
    }
    #[test]
    fn sqlite_diagnostics_use_engine_offsets_and_unicode_columns() {
        let db = Connection::open_in_memory().unwrap();
        let sql = "SELECT '中';\nSELECT )";
        // A direct second statement demonstrates a real engine-provided offset.
        let error = db.prepare("SELECT '中',\n )").err().unwrap();
        let diagnostic = query_error(error).to_string();
        assert!(diagnostic.contains("line 2, column 2"), "{diagnostic}");
        let diagnostic = query_error(rusqlite::Error::SqlInputError {
            error: rusqlite::ffi::Error::new(1), msg: "syntax error".into(),
            sql: sql.into(), offset: "SELECT '中';\nSELECT ".len() as i32,
        }).to_string();
        assert!(diagnostic.ends_with("line 2, column 8"), "{diagnostic}");
    }
    #[test]
    fn query_note_provenance_survives_results_and_rejects_forgery() {
        use crate::core::notes::{self, Request};
        let (d, index, cancel) = fixture();
        let mut queries = Queries::default();
        let sql = "SELECT '001' AS identifier, NULL AS empty, COUNT(*) AS total FROM data";
        let result = queries.run(d.path(), &index, 0, sql, cancel).unwrap();
        let request = |quote: &str, column: usize| -> Request {
            serde_json::from_value(serde_json::json!({"operation":"add","kind":"favorite",
                "quote":quote,"revision":index.revision,
                "anchor":{"location":{"kind":"queryCell","queryId":result.id,"row":0,"column":column}}})).unwrap()
        };
        assert!(queries.prepare_note(&index, &mut request("forged", 0)).is_err());
        assert!(queries.prepare_note(&index, &mut request("", 0)).is_err());
        assert!(queries.prepare_note(&index, &mut request("01", 0)).is_ok(), "A selected substring must retain its query-cell provenance");
        assert!(queries.prepare_note(&index, &mut request("001", 99)).is_err());
        let other = Queries::default();
        assert!(other.prepare_note(&index, &mut request("001", 0)).is_err());
        let mut saved = Vec::new();
        for (quote, column) in [("001", 0), ("NULL", 1), ("123", 2)] {
            let mut note = request(quote, column);
            queries.prepare_note(&index, &mut note).unwrap();
            assert!(queries.prepare_note(&index, &mut note).is_err(), "Client-supplied provenance accepted");
            saved.push(notes::execute(d.path(), &d.path().join("data.csv"), &index, note).unwrap()["id"].as_str().unwrap().to_owned());
        }
        queries.close(&result.id);
        assert!(queries.prepare_note(&index, &mut request("001", 0)).is_err());
        drop(queries);
        let listed = notes::execute(d.path(), &d.path().join("data.csv"), &index, Request::List { offset:0, kind:None }).unwrap();
        assert_eq!(listed["total"], 3);
        for note in listed["items"].as_array().unwrap() {
            assert!(note["anchor"]["location"].is_null());
            assert_eq!(note["anchor"]["query"]["sql"], sql);
            assert_eq!(note["anchor"]["query"]["row"], 0);
            assert_eq!(note["anchor"]["query"]["sourceRows"], 123);
            assert_eq!(note["anchor"]["query"]["totalRows"], 1);
        }
        let copied = notes::execute(d.path(), &d.path().join("data.csv"), &index, Request::Copy { id:saved[0].clone() }).unwrap();
        let copied = copied["text"].as_str().unwrap();
        assert!(copied.contains("> 001") && copied.contains("snapshot, not a source cell") && copied.contains(sql));
        let mut changed: Index = serde_json::from_value(serde_json::to_value(&index).unwrap()).unwrap(); changed.revision = "changed".into();
        let stale = notes::execute(d.path(), &d.path().join("data.csv"), &changed, Request::List { offset:0, kind:None }).unwrap();
        assert!(stale["items"].as_array().unwrap().iter().all(|note| note["stale"] == true));
    }
    fn fixture() -> (TempDir, Index, Arc<AtomicBool>) {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("data.csv");
        std::fs::write(
            &p,
            format!(
                "Name,Value,Name,\"quote\"\"列\"\r\n{}",
                (0..123)
                    .map(|n| format!("row-{n},{n},中文😀,001\r\n"))
                    .collect::<String>()
            ),
        )
        .unwrap();
        let index = crate::core::build_index(&p, d.path()).unwrap();
        (d, index, Arc::new(AtomicBool::new(false)))
    }
    #[test]
    fn widest_csv_columns_remain_queryable_and_transport_is_byte_bounded() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("wide.csv");
        let csv = format!(
            "{}\n{}\n",
            (0..256)
                .map(|n| format!("field{n}"))
                .collect::<Vec<_>>()
                .join(","),
            (0..256)
                .map(|n| format!("value{n}"))
                .collect::<Vec<_>>()
                .join(",")
        );
        std::fs::write(&p, csv).unwrap();
        let index = crate::core::build_index(&p, d.path()).unwrap();
        let mut q = Queries::default();
        let s = schema(&index, 0).unwrap();
        let w = q
            .run(
                d.path(),
                &index,
                0,
                &s.default_sql,
                Arc::new(AtomicBool::new(false)),
            )
            .unwrap();
        assert_eq!(w.columns.len(), 256);
        assert_eq!(w.rows[0][255], Some("value255".into()));
        assert!(serde_json::to_vec(&w).unwrap().len() <= crate::core::MAX_HTML);
    }
    #[test]
    fn source_cell_quoting_and_sql_metadata_are_included_in_the_transport_bound() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("escaped.csv");
        let header = (0..254)
            .map(|n| format!("field{n:03}{}", "\\".repeat(56)))
            .collect::<Vec<_>>()
            .join(",");
        let row = format!("{}{}", "\\".repeat(32 * 1024 - 1024), ",".repeat(253));
        std::fs::write(&p, format!("{header}\n{row}\n")).unwrap();
        let index = crate::core::build_index(&p, d.path()).unwrap();
        let mut q = Queries::default();
        let sql = format!("SELECT * FROM data /*{}*/", "\\".repeat(15 * 1024));
        match q.run(d.path(), &index, 0, &sql, Arc::new(AtomicBool::new(false))) {
            Ok(w) => assert!(serde_json::to_vec(&w).unwrap().len() <= crate::core::MAX_HTML),
            Err(e) => assert!(e.to_string().contains("transport budget"), "{e}"),
        }
    }
    #[test]
    fn full_table_select_filter_sort_aggregate_unicode_and_exact_strings() {
        let (d, index, c) = fixture();
        let mut q = Queries::default();
        let s = schema(&index, 0).unwrap();
        assert_eq!(s.columns, vec!["Name", "Value", "Name_2", "quote\"列"]);
        let result=q.run(d.path(),&index,0,"SELECT Name, Value, \"quote\"\"列\" FROM data WHERE CAST(Value AS INTEGER)>=100 ORDER BY CAST(Value AS INTEGER) DESC",c.clone()).unwrap();
        assert_eq!(result.total_rows, 23);
        assert_eq!(
            result.rows[0],
            vec![
                Some("row-122".into()),
                Some("122".into()),
                Some("001".into())
            ]
        );
        let r = q
            .run(
                d.path(),
                &index,
                0,
                "SELECT COUNT(*), SUM(CAST(c2 AS INTEGER)), MAX(_row) FROM t",
                c.clone(),
            )
            .unwrap();
        assert_eq!(
            r.rows[0],
            vec![Some("123".into()), Some("7503".into()), Some("123".into())]
        );
        let r = q
            .run(
                d.path(),
                &index,
                0,
                "SELECT _row, _line, c3 FROM t WHERE c2='122'",
                c,
            )
            .unwrap();
        assert_eq!(
            r.rows[0],
            vec![
                Some("123".into()),
                Some("124".into()),
                Some("中文😀".into())
            ]
        );
    }
    #[test]
    fn results_are_paged_revision_bound_empty_safe_and_owned() {
        let (d, mut index, c) = fixture();
        let mut q = Queries::default();
        let r = q
            .run(d.path(), &index, 0, "SELECT * FROM data", c.clone())
            .unwrap();
        assert_eq!(r.total_rows, 123);
        assert_eq!(r.rows.len(), 50);
        let last = q.page(&index, &r.id, 122).unwrap();
        assert_eq!(last.row_start, 100);
        assert_eq!(last.row_end, 123);
        assert_eq!(last.rows.len(), 23);
        let empty = q
            .run(
                d.path(),
                &index,
                0,
                "SELECT Name FROM data WHERE 0",
                c.clone(),
            )
            .unwrap();
        assert_eq!(empty.total_rows, 0);
        assert_eq!(empty.columns, vec!["Name"]);
        assert!(empty.rows.is_empty());
        for _ in 0..4 {
            q.run(d.path(), &index, 0, "SELECT 1", c.clone()).unwrap();
        }
        assert!(q.page(&index, &r.id, 0).is_err());
        let r = q.run(d.path(), &index, 0, "SELECT 1", c).unwrap();
        q.close(&r.id);
        assert!(q.page(&index, &r.id, 0).is_err());
        index.revision = "new".into();
        assert!(q.page(&index, &empty.id, 0).is_err());
        drop(q);
        assert!(
            !std::fs::read_dir(d.path()).unwrap().any(|e| e
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with("sql-"))
        );
    }
    #[test]
    fn sql_cannot_write_attach_read_files_load_extensions_or_run_multiple_statements() {
        let (d, index, c) = fixture();
        let mut q = Queries::default();
        let before = std::fs::read(d.path().join("data.csv")).unwrap();
        for sql in [
            "DELETE FROM data",
            "UPDATE data SET Value='x'",
            "DROP TABLE data",
            "ATTACH DATABASE '/tmp/not-created-by-marknoria' AS other",
            "PRAGMA table_info(data)",
            "SELECT * FROM sqlite_schema",
            "SELECT readfile('/etc/passwd')",
            "SELECT load_extension('/tmp/example')",
            "SELECT randomblob(999999999)",
            "SELECT printf('%999999999s','x')",
            "SELECT 1; SELECT 2",
            "SELECT '\0'",
            "SELECT * FROM absent",
            "SELECT CAST(X'00' AS TEXT)",
            "SELECT CAST(X'01' AS TEXT)",
            "SELECT CAST(X'ff' AS TEXT)",
        ] {
            assert!(
                q.run(d.path(), &index, 0, sql, c.clone()).is_err(),
                "accepted {sql:?}"
            );
        }
        assert_eq!(std::fs::read(d.path().join("data.csv")).unwrap(), before);
        assert_eq!(
            q.run(d.path(), &index, 0, "SELECT COUNT(*) FROM data", c)
                .unwrap()
                .rows[0][0],
            Some("123".into())
        );
    }
    #[test]
    fn recursive_queries_and_cancellation_are_bounded_and_recoverable() {
        let (d, index, c) = fixture();
        let mut q = Queries::default();
        let started = Instant::now();
        assert!(q.run(d.path(),&index,0,"WITH RECURSIVE n(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM n) SELECT SUM(x) FROM n",c.clone()).is_err());
        assert!(started.elapsed() < Duration::from_secs(8));
        c.store(true, Ordering::Relaxed);
        assert!(q.run(d.path(),&index,0,"WITH RECURSIVE n(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM n) SELECT SUM(x) FROM n",c.clone()).is_err());
        c.store(false, Ordering::Relaxed);
        assert_eq!(
            q.run(d.path(), &index, 0, "SELECT 9223372036854775807", c)
                .unwrap()
                .rows[0][0],
            Some("9223372036854775807".into())
        );
    }
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
        let mut db = Connection::open_with_flags(
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
        let mut statement = db.prepare(sql).map_err(query_error)?;
        if !statement.readonly() || statement.column_count() == 0 {
            return Err(error("Only read-only SELECT queries are supported"));
        }
        let columns = statement
            .column_names()
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
                || encoded.len() > crate::core::MAX_HTML / 2
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
    /// Resolve a live, document-owned result into bounded durable provenance.
    /// Never accept client-supplied SQL as evidence of a previously run query.
    pub fn prepare_note(&self, index: &Index, request: &mut crate::core::notes::Request) -> io::Result<()> {
        use crate::core::{notes::Request, reader::Location};
        let Request::Add { quote, anchor, revision, .. } = request else { return Ok(()) };
        if anchor.query.is_some() { return Err(error("Supply a live query cell, not client-supplied SQL provenance")); }
        if matches!(&anchor.location, Some(Location::QueryCell { .. })) {
            let text_range = anchor.text_range.take();
            if let Some(range) = &text_range {
                let Some(Location::QueryCell { query_id, row, column }) = &anchor.location else { unreachable!() };
                if range.segments.iter().any(|segment| !matches!(&segment.scope, crate::core::notes::TextScope::QueryCell { query_id: q, row: r, column: c } if (q,r,c)==(query_id,row,column))) {
                    return Err(error("Comment result selector belongs to another query cell"));
                }
            }
            *anchor = self.snapshot(index, &crate::core::reader::QuerySnapshotRequest {
                revision: revision.clone(), location: anchor.location.clone().unwrap(),
                quote: quote.clone(), quote_truncated: anchor.quote_truncated,
            })?.anchor;
            anchor.text_range = text_range;
        }
        Ok(())
    }
    pub fn snapshot(&self, index: &Index, request: &crate::core::reader::QuerySnapshotRequest)
        -> io::Result<crate::core::reader::QuerySnapshot> {
        use crate::core::{notes::{Anchor, QueryProvenance}, reader::{Location, QuerySnapshot}};
        if request.revision != index.revision { return Err(error("Document revision changed; reopen SQL")); }
        if request.quote.len() > 32 * 1024 || request.quote.contains('\0') {
            return Err(error("Query snapshot quote exceeds its text budget"));
        }
        request.location.validate()?;
        let Location::QueryCell { query_id, row, column } = &request.location
            else { return Err(error("A live query result cell is required")); };
        let window = self.page(index, query_id, *row)?;
        let value = window.rows.get(row - window.row_start).and_then(|values| values.get(*column))
            .ok_or_else(|| error("Query note cell is outside its result"))?;
        let value_is_null = value.is_none();
        let value = value.as_deref().unwrap_or("NULL");
        // A cell viewer can quote an exact selected substring. The owned result
        // remains the authority; unrelated text and empty snippets are rejected.
        if request.quote.is_empty() || !value.contains(&request.quote) {
            return Err(error("Note quote does not match its query result cell"));
        }
        Ok(QuerySnapshot {
            revision: index.revision.clone(), quote: request.quote.clone(),
            anchor: Anchor {
                quote_truncated: request.quote_truncated,
                query: Some(QueryProvenance {
                    table: window.table, sql: window.sql, row: *row, column: *column,
                    column_name: window.columns[*column].clone(), total_rows: window.total_rows,
                    source_rows: index.tables[window.table].rows, value_is_null,
                }),
                ..Default::default()
            },
        })
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
        let available = crate::core::MAX_HTML.saturating_sub(metadata_bytes);
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
