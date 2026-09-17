use std::collections::BTreeSet;
use std::time::{Duration, Instant};

use rusqlite::hooks::{AuthAction, AuthContext, Authorization};
use rusqlite::limits::Limit;
use rusqlite::types::ValueRef;
use rusqlite::{params_from_iter, Connection};
use serde::{Deserialize, Serialize};

pub const MAX_TABLE_QUERY_BODY_BYTES: usize = 16 * 1024 * 1024;
const MAX_SOURCE_ROWS: usize = 10_000;
const MAX_COLUMNS: usize = 200;
const MAX_SQL_BYTES: usize = 64 * 1024;
const MAX_RESULT_ROWS: usize = 5_000;
const MAX_RESULT_BYTES: usize = 8 * 1024 * 1024;
const MAX_SQLITE_VALUE_BYTES: i32 = 512 * 1024;
const QUERY_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TableQueryRequest {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub sql: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TableQueryResponse {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub truncated: bool,
}

pub fn execute(request: TableQueryRequest) -> Result<TableQueryResponse, String> {
    validate_request(&request)?;
    let columns = normalize_columns(&request.columns);
    let mut connection = Connection::open_in_memory().map_err(sql_error)?;
    configure_limits(&connection)?;
    create_source_table(&mut connection, &columns, &request.rows)?;
    query_source(&connection, &request.sql)
}

fn configure_limits(connection: &Connection) -> Result<(), String> {
    for (limit, value) in [
        (Limit::SQLITE_LIMIT_LENGTH, MAX_SQLITE_VALUE_BYTES),
        (Limit::SQLITE_LIMIT_SQL_LENGTH, MAX_SQL_BYTES as i32),
        (Limit::SQLITE_LIMIT_COLUMN, MAX_COLUMNS as i32),
        (Limit::SQLITE_LIMIT_EXPR_DEPTH, 100),
        (Limit::SQLITE_LIMIT_COMPOUND_SELECT, 20),
        (Limit::SQLITE_LIMIT_FUNCTION_ARG, 50),
        (Limit::SQLITE_LIMIT_ATTACHED, 0),
        (Limit::SQLITE_LIMIT_LIKE_PATTERN_LENGTH, 4_096),
        (Limit::SQLITE_LIMIT_WORKER_THREADS, 0),
    ] {
        connection.set_limit(limit, value).map_err(sql_error)?;
    }
    Ok(())
}

fn validate_request(request: &TableQueryRequest) -> Result<(), String> {
    if request.columns.is_empty() || request.columns.len() > MAX_COLUMNS {
        return Err(format!(
            "table must contain between 1 and {MAX_COLUMNS} columns"
        ));
    }
    if request.rows.len() > MAX_SOURCE_ROWS {
        return Err(format!(
            "table exceeds the {MAX_SOURCE_ROWS} row query limit"
        ));
    }
    if request
        .rows
        .iter()
        .any(|row| row.len() > request.columns.len())
    {
        return Err("table row contains more values than declared columns".to_string());
    }
    let sql = request.sql.trim();
    if sql.is_empty() || sql.len() > MAX_SQL_BYTES {
        return Err(format!(
            "SQL must contain between 1 and {MAX_SQL_BYTES} bytes"
        ));
    }
    Ok(())
}

fn normalize_columns(columns: &[String]) -> Vec<String> {
    let mut used = BTreeSet::new();
    columns
        .iter()
        .enumerate()
        .map(|(index, value)| {
            let mut base = value
                .replace('\0', "")
                .trim()
                .chars()
                .take(128)
                .collect::<String>();
            if base.is_empty() {
                base = format!("column_{}", index + 1);
            }
            let mut candidate = base.clone();
            let mut suffix = 2;
            while !used.insert(candidate.to_lowercase()) {
                candidate = format!("{base}_{suffix}");
                suffix += 1;
            }
            candidate
        })
        .collect()
}

fn create_source_table(
    connection: &mut Connection,
    columns: &[String],
    rows: &[Vec<String>],
) -> Result<(), String> {
    let declaration = columns
        .iter()
        .map(|column| format!("{} TEXT", quote_identifier(column)))
        .collect::<Vec<_>>()
        .join(", ");
    connection
        .execute(&format!("CREATE TABLE data ({declaration})"), [])
        .map_err(sql_error)?;
    let placeholders = (1..=columns.len())
        .map(|index| format!("?{index}"))
        .collect::<Vec<_>>()
        .join(", ");
    let transaction = connection.transaction().map_err(sql_error)?;
    {
        let mut statement = transaction
            .prepare(&format!("INSERT INTO data VALUES ({placeholders})"))
            .map_err(sql_error)?;
        for row in rows {
            let values =
                (0..columns.len()).map(|index| row.get(index).map(String::as_str).unwrap_or(""));
            statement
                .execute(params_from_iter(values))
                .map_err(sql_error)?;
        }
    }
    transaction.commit().map_err(sql_error)
}

fn query_source(connection: &Connection, sql: &str) -> Result<TableQueryResponse, String> {
    connection
        .authorizer(Some(|context: AuthContext<'_>| match context.action {
            AuthAction::Select | AuthAction::Function { .. } | AuthAction::Recursive => {
                Authorization::Allow
            }
            AuthAction::Read { table_name, .. } if table_name.eq_ignore_ascii_case("data") => {
                Authorization::Allow
            }
            _ => Authorization::Deny,
        }))
        .map_err(sql_error)?;
    let started = Instant::now();
    connection
        .progress_handler(10_000, Some(move || started.elapsed() >= QUERY_TIMEOUT))
        .map_err(sql_error)?;

    let mut statement = connection.prepare(sql.trim()).map_err(sql_error)?;
    if !statement.readonly() || statement.column_count() == 0 {
        return Err("only one read-only SELECT or WITH query is allowed".to_string());
    }
    let column_count = statement.column_count();
    if column_count > MAX_COLUMNS {
        return Err(format!(
            "query result exceeds the {MAX_COLUMNS} column limit"
        ));
    }
    let columns = statement
        .column_names()
        .iter()
        .map(|value| (*value).to_string())
        .collect::<Vec<_>>();
    let mut output = Vec::new();
    let mut output_bytes = 0_usize;
    let mut rows = statement.query([]).map_err(sql_error)?;
    let mut truncated = false;
    while let Some(row) = rows.next().map_err(sql_error)? {
        if output.len() >= MAX_RESULT_ROWS {
            truncated = true;
            break;
        }
        let mut values = Vec::with_capacity(column_count);
        for index in 0..column_count {
            values.push(value_text(row.get_ref(index).map_err(sql_error)?));
        }
        let row_bytes = values.iter().map(String::len).sum::<usize>();
        if output_bytes.saturating_add(row_bytes) > MAX_RESULT_BYTES {
            truncated = true;
            break;
        }
        output_bytes += row_bytes;
        output.push(values);
    }
    Ok(TableQueryResponse {
        columns,
        rows: output,
        truncated,
    })
}

fn quote_identifier(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

fn value_text(value: ValueRef<'_>) -> String {
    match value {
        ValueRef::Null => String::new(),
        ValueRef::Integer(value) => value.to_string(),
        ValueRef::Real(value) => value.to_string(),
        ValueRef::Text(value) => String::from_utf8_lossy(value).into_owned(),
        ValueRef::Blob(value) => format!("[BLOB: {} bytes]", value.len()),
    }
}

fn sql_error(error: rusqlite::Error) -> String {
    if error
        .to_string()
        .to_ascii_lowercase()
        .contains("interrupted")
    {
        "query exceeded the 2 second execution limit".to_string()
    } else {
        error.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(sql: &str) -> TableQueryRequest {
        TableQueryRequest {
            columns: vec!["name".into(), "score".into(), "group".into()],
            rows: vec![
                vec!["alpha".into(), "82".into(), "A".into()],
                vec!["beta".into(), "91".into(), "B".into()],
                vec!["gamma".into(), "105".into(), "A".into()],
            ],
            sql: sql.into(),
        }
    }

    #[test]
    fn projects_filters_and_sorts_table_data() {
        let result = execute(request(
            "SELECT name, CAST(score AS INTEGER) AS points FROM data WHERE \"group\" = 'A' ORDER BY points DESC",
        ))
        .expect("query");
        assert_eq!(result.columns, vec!["name", "points"]);
        assert_eq!(result.rows, vec![vec!["gamma", "105"], vec!["alpha", "82"]]);
        assert!(!result.truncated);
    }

    #[test]
    fn supports_read_only_common_table_expressions() {
        let result = execute(request(
            "WITH ranked AS (SELECT name, score FROM data) SELECT name FROM ranked ORDER BY CAST(score AS INTEGER) DESC LIMIT 1",
        ))
        .expect("query");
        assert_eq!(result.rows, vec![vec!["gamma"]]);
    }

    #[test]
    fn rejects_writes_pragmas_attachments_and_multiple_statements() {
        for sql in [
            "DELETE FROM data",
            "PRAGMA table_info(data)",
            "ATTACH DATABASE '/tmp/example.sqlite3' AS other",
            "SELECT * FROM data; SELECT * FROM data",
        ] {
            assert!(execute(request(sql)).is_err(), "accepted {sql}");
        }
    }

    #[test]
    fn bounds_values_created_by_sql_functions() {
        assert!(execute(request("SELECT randomblob(600000) FROM data LIMIT 1")).is_err());
    }

    #[test]
    fn normalizes_blank_and_duplicate_column_names() {
        let result = execute(TableQueryRequest {
            columns: vec!["".into(), "Name".into(), "name".into()],
            rows: vec![vec!["1".into(), "first".into(), "second".into()]],
            sql: "SELECT column_1, Name, name_2 FROM data".into(),
        })
        .expect("query");
        assert_eq!(result.columns, vec!["column_1", "Name", "name_2"]);
        assert_eq!(result.rows, vec![vec!["1", "first", "second"]]);
    }
}
