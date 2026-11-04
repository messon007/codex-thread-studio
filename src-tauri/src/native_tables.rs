//! Immutable table snapshots and bounded pages, shared by native document UIs.
use crate::{
    json_error, json_response, table_query,
    tabular::{self, Workbook},
    GatewayState, ReviewFileRequest,
};
use axum::{
    body::Body,
    extract::{rejection::JsonRejection, State},
    http::{Response, StatusCode},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    io::Read,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
const MAX_FILE: u64 = 20 * 1024 * 1024;
const CACHE_MEMORY: usize = 256 * 1024 * 1024;
const IDLE: Duration = Duration::from_secs(600);
struct Entry {
    source: Arc<Workbook>,
    path: String,
    hash: String,
    touched: Instant,
    result: Option<QueryResult>,
    query_serial: u64,
}
struct QueryResult {
    id: String,
    sheet: usize,
    data: Arc<table_query::TableQueryResponse>,
    weight: usize,
    sql: String,
}
#[derive(Clone)]
pub struct Tables {
    entries: Arc<Mutex<HashMap<String, Entry>>>,
    workers: Arc<tokio::sync::Semaphore>,
    closing: Arc<tokio::sync::Semaphore>,
}
impl Default for Tables {
    fn default() -> Self {
        Self {
            entries: Arc::new(Mutex::new(HashMap::new())),
            workers: Arc::new(tokio::sync::Semaphore::new(2)),
            closing: Arc::new(tokio::sync::Semaphore::new(1)),
        }
    }
}
#[derive(Deserialize, Debug)]
#[serde(tag = "operation", rename_all = "camelCase", deny_unknown_fields)]
pub enum Request {
    Format {
        sql: String,
        compact: bool,
    },
    Open {
        root: String,
        path: String,
        #[serde(default)]
        delimiter: Option<String>,
    },
    Page {
        id: String,
        sheet: usize,
        offset: usize,
        #[serde(default)]
        query: Option<String>,
    },
    Query {
        id: String,
        sheet: usize,
        sql: String,
    },
    Cell {
        id: String,
        sheet: usize,
        row: usize,
        column: usize,
        #[serde(default)]
        query: Option<String>,
    },
    Comment {
        id: String,
        sheet: usize,
        row: usize,
        column: usize,
        #[serde(default)]
        query: Option<String>,
    },
    Chart {
        id: String,
        sheet: usize,
        #[serde(default)]
        query: Option<String>,
    },
    Close {
        id: String,
    },
}
type Error = (StatusCode, String);
fn bad(error: impl ToString) -> Error {
    (StatusCode::BAD_REQUEST, error.to_string())
}
fn missing() -> Error {
    (
        StatusCode::GONE,
        "Table snapshot expired or closed; reopen the file".into(),
    )
}
fn result_weight(result: &table_query::TableQueryResponse) -> usize {
    result
        .rows
        .iter()
        .map(|row| {
            2 * std::mem::size_of::<Vec<String>>()
                + row
                    .iter()
                    .map(|value| value.len() + 2 * std::mem::size_of::<String>())
                    .sum::<usize>()
        })
        .sum::<usize>()
}
fn cache_weight(entries: &HashMap<String, Entry>) -> usize {
    entries
        .values()
        .map(|entry| {
            entry.source.weight
                + entry.path.len()
                + entry.hash.len()
                + entry.result.as_ref().map_or(0, |result| result.weight)
        })
        .sum()
}
impl Tables {
    fn source(&self, id: &str) -> Result<Arc<Workbook>, Error> {
        let mut entries = self.entries.lock().map_err(bad)?;
        let entry = entries.get_mut(id).ok_or_else(missing)?;
        entry.touched = Instant::now();
        Ok(entry.source.clone())
    }
    fn grid(
        &self,
        id: &str,
        sheet: usize,
        query: Option<&str>,
    ) -> Result<(Arc<Workbook>, Option<Arc<table_query::TableQueryResponse>>), Error> {
        let mut entries = self.entries.lock().map_err(bad)?;
        let entry = entries.get_mut(id).ok_or_else(missing)?;
        if sheet >= entry.source.sheets.len() {
            return Err(bad("Invalid worksheet"));
        }
        entry.touched = Instant::now();
        let result = if let Some(query) = query {
            Some(
                entry
                    .result
                    .as_ref()
                    .filter(|result| result.id == query && result.sheet == sheet)
                    .ok_or_else(|| {
                        (
                            StatusCode::CONFLICT,
                            "Query result was replaced; run the query again".into(),
                        )
                    })?
                    .data
                    .clone(),
            )
        } else {
            None
        };
        Ok((entry.source.clone(), result))
    }
    fn page(
        &self,
        id: &str,
        sheet: usize,
        offset: usize,
        query: Option<&str>,
    ) -> Result<Value, Error> {
        let (source, result) = self.grid(id, sheet, query)?;
        let raw = &source.sheets[sheet];
        let (columns, rows, truncated) = if let Some(result) = &result {
            (result.columns.clone(), &result.rows, result.truncated)
        } else {
            (
                (0..raw.width.max(1)).map(tabular::column_name).collect(),
                &raw.rows,
                raw.truncated,
            )
        };
        let size = (512 / columns.len().max(1)).clamp(1, 50);
        let start = (offset / size * size).min(rows.len().saturating_sub(1) / size * size);
        let values = rows
            .iter()
            .skip(start)
            .take(size)
            .map(|row| {
                (0..columns.len())
                    .map(|column| {
                        let value = row.get(column).map(String::as_str).unwrap_or("");
                        let mut end = value.len().min(2048);
                        while !value.is_char_boundary(end) {
                            end -= 1;
                        }
                        json!({"text":&value[..end],"clipped":end<value.len()})
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        Ok(
            json!({"documentId":id,"sheet":sheet,"queryId":query,"columns":columns,"offset":start,"total":rows.len(),"pageSize":size,"rows":values,"truncated":truncated}),
        )
    }
    pub fn process(&self, request: Request, shared: &[std::path::PathBuf]) -> Result<Value, Error> {
        match request {
            Request::Format { sql, compact } => {
                Ok(json!({"sql":tabular::format_sql(&sql,compact).map_err(bad)?}))
            }
            Request::Open {
                root,
                path,
                delimiter,
            } => {
                if root.len() > 4096 || path.len() > 4096 {
                    return Err(bad("File path exceeds its limit"));
                }
                let (_, path, _, _) = crate::resolve_review_path_with_shared(
                    &ReviewFileRequest { root, path },
                    MAX_FILE,
                    "20 MiB table",
                    shared,
                )?;
                let extension = path
                    .extension()
                    .and_then(|value| value.to_str())
                    .unwrap_or("")
                    .to_ascii_lowercase();
                let mut bytes = Vec::new();
                std::fs::File::open(&path)
                    .map_err(bad)?
                    .take(MAX_FILE + 1)
                    .read_to_end(&mut bytes)
                    .map_err(bad)?;
                if bytes.len() as u64 > MAX_FILE {
                    return Err((
                        StatusCode::PAYLOAD_TOO_LARGE,
                        "File grew beyond 20 MiB while opening".into(),
                    ));
                }
                let hash = crate::stable_content_hash(&bytes);
                let source =
                    Arc::new(tabular::load(bytes, &extension, delimiter.as_deref()).map_err(bad)?);
                if source.sheets.is_empty() {
                    return Err(bad("Workbook contains no worksheets"));
                }
                let sheets=source.sheets.iter().map(|sheet|{
                    let columns=tabular::sql_columns(sheet);
                    json!({"name":sheet.name,"rows":sheet.rows.len(),"columns":sheet.width,"truncated":sheet.truncated,"sql":tabular::default_sql(&columns,false)})
                }).collect::<Vec<_>>();
                let id = uuid::Uuid::new_v4().to_string();
                {
                    let mut entries = self.entries.lock().map_err(bad)?;
                    let expired = entries
                        .iter()
                        .filter(|(_, entry)| entry.touched.elapsed() >= IDLE)
                        .map(|(id, _)| id.clone())
                        .collect::<Vec<_>>();
                    let old = expired
                        .into_iter()
                        .filter_map(|id| entries.remove(&id))
                        .collect::<Vec<_>>();
                    if entries.len() >= 4
                        || cache_weight(&entries)
                            + source.weight
                            + path.to_string_lossy().len()
                            + hash.len()
                            > CACHE_MEMORY
                    {
                        drop(entries);
                        drop(old);
                        return Err((
                            StatusCode::TOO_MANY_REQUESTS,
                            "Close an existing table before opening another".into(),
                        ));
                    }
                    entries.insert(
                        id.clone(),
                        Entry {
                            source: source.clone(),
                            path: path.to_string_lossy().into_owned(),
                            hash: hash.clone(),
                            touched: Instant::now(),
                            result: None,
                            query_serial: 0,
                        },
                    );
                    drop(entries);
                    drop(old);
                }
                Ok(
                    json!({"id":id,"name":path.file_name().unwrap_or_default().to_string_lossy(),"path":path.to_string_lossy(),"hash":hash,"delimiter":source.delimiter,"sheets":sheets,"page":self.page(&id,0,0,None)?}),
                )
            }
            Request::Page {
                id,
                sheet,
                offset,
                query,
            } => self.page(&id, sheet, offset, query.as_deref()),
            Request::Query { id, sheet, sql } => {
                let source = self.source(&id)?;
                let raw = source
                    .sheets
                    .get(sheet)
                    .ok_or_else(|| bad("Invalid worksheet"))?;
                let serial = {
                    let mut entries = self.entries.lock().map_err(bad)?;
                    let entry = entries.get_mut(&id).ok_or_else(missing)?;
                    entry.query_serial += 1;
                    entry.query_serial
                };
                let result = Arc::new(
                    table_query::execute(table_query::TableQueryRequest {
                        columns: tabular::sql_columns(raw),
                        rows: raw.rows.iter().skip(1).cloned().collect(),
                        sql: sql.clone(),
                    })
                    .map_err(bad)?,
                );
                if result
                    .rows
                    .iter()
                    .flatten()
                    .any(|value| value.contains('\0'))
                    || result.columns.iter().any(|value| value.contains('\0'))
                {
                    return Err(bad("Query result contains NUL text"));
                }
                let key = uuid::Uuid::new_v4().to_string();
                let weight = result_weight(&result) + sql.len();
                {
                    let mut entries = self.entries.lock().map_err(bad)?;
                    let entry = entries.get(&id).ok_or_else(missing)?;
                    if entry.query_serial != serial {
                        return Err((
                            StatusCode::CONFLICT,
                            "A newer query superseded this result".into(),
                        ));
                    }
                    let previous = entry.result.as_ref().map_or(0, |result| result.weight);
                    if cache_weight(&entries).saturating_sub(previous) + weight > CACHE_MEMORY {
                        return Err(bad("Query result exceeds the table cache budget"));
                    }
                    let old = entries.get_mut(&id).unwrap().result.replace(QueryResult {
                        id: key.clone(),
                        sheet,
                        data: result,
                        weight,
                        sql,
                    });
                    drop(entries);
                    drop(old);
                }
                self.page(&id, sheet, 0, Some(&key))
            }
            Request::Cell {
                id,
                sheet,
                row,
                column,
                query,
            } => {
                let (source, result) = self.grid(&id, sheet, query.as_deref())?;
                let rows = result
                    .as_ref()
                    .map_or(&source.sheets[sheet].rows, |result| &result.rows);
                // Ragged source rows have explicit empty cells in the grid.
                let width = result
                    .as_ref()
                    .map_or(source.sheets[sheet].width.max(1), |result| {
                        result.columns.len()
                    });
                if row >= rows.len() || column >= width {
                    return Err(bad("Cell is outside the current table"));
                }
                Ok(json!({"text":rows[row].get(column).map(String::as_str).unwrap_or("")}))
            }
            Request::Chart { id, sheet, query } => {
                let (source, result) = self.grid(&id, sheet, query.as_deref())?;
                let raw = &source.sheets[sheet];
                let chart = if let Some(result) = &result {
                    tabular::chart(&result.columns, &result.rows)
                } else {
                    tabular::chart(&tabular::sql_columns(raw), raw.rows.get(1..).unwrap_or(&[]))
                }
                .map_err(bad)?;
                serde_json::to_value(chart).map_err(bad)
            }
            Request::Comment {
                id,
                sheet,
                row,
                column,
                query,
            } => {
                // Freeze source identity, query text and result data together.
                let (source, result, sql, path, hash) = {
                    let mut entries = self.entries.lock().map_err(bad)?;
                    let entry = entries.get_mut(&id).ok_or_else(missing)?;
                    if sheet >= entry.source.sheets.len() {
                        return Err(bad("Invalid worksheet"));
                    }
                    let result = if let Some(query) = &query {
                        Some(
                            entry
                                .result
                                .as_ref()
                                .filter(|r| r.id == *query && r.sheet == sheet)
                                .ok_or_else(|| {
                                    (
                                        StatusCode::CONFLICT,
                                        "Query result was replaced; run the query again".into(),
                                    )
                                })?,
                        )
                    } else {
                        None
                    };
                    let frozen = (
                        entry.source.clone(),
                        result.map(|r| r.data.clone()),
                        result.map(|r| r.sql.clone()),
                        entry.path.clone(),
                        entry.hash.clone(),
                    );
                    entry.touched = Instant::now();
                    frozen
                };
                if sql.as_ref().is_some_and(|s| s.len() > 16384) {
                    return Err(bad(
                        "Comments support SQL up to 16 KiB; simplify the query before commenting",
                    ));
                }
                let raw = &source.sheets[sheet];
                let (rows, width, header) = if let Some(result) = &result {
                    (
                        &result.rows,
                        result.columns.len(),
                        result.columns.get(column).map(String::as_str).unwrap_or(""),
                    )
                } else {
                    (
                        &raw.rows,
                        raw.width.max(1),
                        raw.rows
                            .first()
                            .and_then(|r| r.get(column))
                            .map(String::as_str)
                            .unwrap_or(""),
                    )
                };
                if row >= rows.len() || column >= width {
                    return Err(bad("Cell is outside the current table"));
                }
                let text = rows[row].get(column).map(String::as_str).unwrap_or("");
                if text.len() > 16384 {
                    return Err(bad("Comments support cell values up to 16 KiB"));
                }
                Ok(
                    json!({"text":text,"anchor":{"filePath":path,"documentHash":hash,"sheet":raw.name,"range":format!("{}{}",tabular::column_name(column),row+1),"header":header.chars().take(128).collect::<String>(),"sql":sql,"delimiter":source.delimiter}}),
                )
            }
            Request::Close { id } => {
                let old = self.entries.lock().map_err(bad)?.remove(&id);
                drop(old);
                Ok(json!({"closed":true}))
            }
        }
    }
}
pub async fn handle(
    State(state): State<GatewayState>,
    body: Result<Json<Request>, JsonRejection>,
) -> Response<Body> {
    let request = match body {
        Ok(Json(request)) => request,
        Err(_) => return json_error(StatusCode::BAD_REQUEST, "Invalid native table request"),
    };
    let lane = if matches!(request, Request::Close { .. }) {
        state.tables.closing.clone()
    } else {
        state.tables.workers.clone()
    };
    let permit = match lane.try_acquire_owned() {
        Ok(permit) => permit,
        Err(_) => {
            return json_error(
                StatusCode::TOO_MANY_REQUESTS,
                "Table workers are busy; try again",
            )
        }
    };
    let work = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let result = (|| {
            let shared = if matches!(request, Request::Open { .. }) {
                crate::shared_document_directories(&state)?
            } else {
                Vec::new()
            };
            state.tables.process(request, &shared)
        })();
        match result {
            Ok(value) => json_response(StatusCode::OK, &value),
            Err((status, error)) => json_error(status, &error),
        }
    })
    .await;
    match work {
        Ok(response) => response,
        Err(_) => json_error(StatusCode::INTERNAL_SERVER_ERROR, "Table worker stopped"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn snapshots_page_query_and_reject_stale_results_and_outside_paths() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("data.csv");
        std::fs::write(&path, "name,score\none,2\ntwo,12\n").unwrap();
        let tables = Tables::default();
        let opened = tables
            .process(
                Request::Open {
                    root: dir.path().to_string_lossy().into_owned(),
                    path: "data.csv".into(),
                    delimiter: None,
                },
                &[],
            )
            .unwrap();
        let id = opened["id"].as_str().unwrap().to_owned();
        assert_eq!(opened["page"]["rows"][1][1]["text"], "2");
        std::fs::write(&path, "changed\n").unwrap();
        let comment = tables
            .process(
                Request::Comment {
                    id: id.clone(),
                    sheet: 0,
                    row: 1,
                    column: 1,
                    query: None,
                },
                &[],
            )
            .unwrap();
        assert_eq!(comment["text"], "2");
        assert_eq!(comment["anchor"]["documentHash"], opened["hash"]);
        assert_eq!(comment["anchor"]["range"], "B2");
        let result=tables.process(Request::Query{id:id.clone(),sheet:0,sql:"SELECT name, CAST(score AS INTEGER) AS score FROM data WHERE CAST(score AS INTEGER)>5 ORDER BY score DESC".into()},&[]).unwrap();
        assert_eq!(result["rows"][0][0]["text"], "two");
        let query = result["queryId"].as_str().unwrap().to_owned();
        assert!(tables
            .process(
                Request::Query {
                    id: id.clone(),
                    sheet: 0,
                    sql: "DELETE FROM data".into()
                },
                &[]
            )
            .is_err());
        assert!(tables
            .process(
                Request::Page {
                    id: id.clone(),
                    sheet: 0,
                    offset: 0,
                    query: Some(query.clone())
                },
                &[]
            )
            .is_ok());
        tables
            .process(
                Request::Query {
                    id: id.clone(),
                    sheet: 0,
                    sql: "SELECT name FROM data".into(),
                },
                &[],
            )
            .unwrap();
        assert_eq!(
            tables
                .process(
                    Request::Page {
                        id: id.clone(),
                        sheet: 0,
                        offset: 0,
                        query: Some(query)
                    },
                    &[]
                )
                .unwrap_err()
                .0,
            StatusCode::CONFLICT
        );
        tables
            .process(Request::Close { id: id.clone() }, &[])
            .unwrap();
        assert_eq!(
            tables
                .process(
                    Request::Page {
                        id,
                        sheet: 0,
                        offset: 0,
                        query: None
                    },
                    &[]
                )
                .unwrap_err()
                .0,
            StatusCode::GONE
        );
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink("/etc/passwd", dir.path().join("escape.csv")).unwrap();
            assert_eq!(
                tables
                    .process(
                        Request::Open {
                            root: dir.path().to_string_lossy().into_owned(),
                            path: "escape.csv".into(),
                            delimiter: None
                        },
                        &[]
                    )
                    .unwrap_err()
                    .0,
                StatusCode::FORBIDDEN
            );
        }
    }
}
