//! Native adapters for the shared reader query service, not another evaluator.
use crate::gateway::Api;
use reqwest::Method;
use serde::Deserialize;
use serde_json::{json, Value};

pub fn jq_supported(name: &str) -> bool {
    std::path::Path::new(name)
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "json" | "jsonl" | "ndjson"
            )
        })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Context {
    pub key: u64,
    pub generation: u64,
    pub revision: String,
}

#[derive(Debug)]
pub enum Operation {
    Jq {
        filter: String,
        slurp: bool,
    },
    JqPage {
        id: String,
        page: usize,
    },
    JqClose {
        id: String,
    },
    SqlSchema {
        table: usize,
    },
    Sql {
        table: usize,
        sql: String,
    },
    SqlPage {
        table: usize,
        id: String,
        row: usize,
    },
    SqlClose {
        id: String,
    },
    SqlFormat {
        sql: String,
        compact: bool,
    },
}

fn result_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
}

impl Operation {
    fn payload(&self, revision: &str) -> Result<Value, String> {
        Ok(match self {
            Self::Jq { filter, slurp } => {
                if filter.trim().is_empty() || filter.len() > 16384 || filter.contains('\0') {
                    return Err("Enter a jq filter of at most 16 KiB".into());
                }
                json!({"revision":revision,"filter":filter,"slurp":slurp})
            }
            Self::JqPage { id, page } if result_id(id) && *page < 10000 => {
                json!({"revision":revision,"id":id,"page":page})
            }
            Self::JqClose { id } if result_id(id) => {
                json!({"revision":revision,"id":id,"close":true})
            }
            Self::SqlSchema { table } if *table < 100000 => json!({"table":table}),
            Self::Sql { table, sql }
                if *table < 100000
                    && !sql.trim().is_empty()
                    && sql.len() <= 16384
                    && !sql.contains('\0') =>
            {
                json!({"table":table,"sql":sql,"revision":revision})
            }
            Self::SqlPage { table, id, row }
                if *table < 100000 && result_id(id) && *row < 1000000 =>
            {
                json!({"id":id,"row":row})
            }
            Self::SqlClose { id } if result_id(id) => json!({"id":id}),
            Self::SqlFormat { sql, compact } if sql.len() <= 65536 && !sql.contains('\0') => {
                json!({"operation":"format","sql":sql,"compact":compact})
            }
            _ => return Err("Invalid reader query result identity or page".into()),
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JqPage {
    kind: String,
    pub id: String,
    pub revision: String,
    pub page: usize,
    pub pages: usize,
    pub total_values: usize,
    pub text: String,
    pub elapsed_ms: f64,
}

impl JqPage {
    fn validate(&self, revision: &str, operation: &Operation) -> Result<(), String> {
        if self.kind != "jqQuery"
            || self.revision != revision
            || !result_id(&self.id)
            || self.pages == 0
            || self.pages > 10000
            || self.page >= self.pages
            || self.text.len() > 48 * 1024
            || self.text.contains('\0')
            || !self.elapsed_ms.is_finite()
            || self.elapsed_ms < 0.0
            || self.total_values > 256 * 1024 * 1024
            || match operation {
                Operation::Jq { .. } => self.page != 0,
                Operation::JqPage { id, page } => self.id != *id || self.page != *page,
                Operation::JqClose { .. } => true,
                _ => true,
            }
        {
            return Err(
                "Query response does not match the requested document or page budget".into(),
            );
        }
        Ok(())
    }
}

#[derive(Debug)]
pub enum Reply {
    Jq(JqPage),
    Schema(SqlSchema),
    Sql(SqlPage),
    Formatted(String),
    Closed,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SqlSchema {
    pub revision: String,
    pub table: usize,
    pub source_rows: usize,
    pub columns: Vec<String>,
    pub positional_columns: Vec<String>,
    pub default_sql: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SqlPage {
    kind: String,
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
    elapsed_ms: f64,
    #[serde(skip)]
    pub copy_text: String,
}

impl SqlPage {
    fn validate(&self, revision: &str, operation: &Operation) -> Result<(), String> {
        let (table, requested) = match operation {
            Operation::Sql { table, sql } if self.sql == *sql => (*table, 0),
            Operation::SqlPage { table, id, row } if self.id == *id => (*table, *row),
            _ => return Err("SQL result does not match the requested query".into()),
        };
        let text_bytes: usize = self.columns.iter().map(String::len).sum::<usize>()
            + self
                .rows
                .iter()
                .flatten()
                .filter_map(Option::as_ref)
                .map(String::len)
                .sum::<usize>();
        if self.kind != "tableQuery"
            || self.revision != revision
            || self.table != table
            || !result_id(&self.id)
            || self.columns.is_empty()
            || self.columns.len() > 512
            || self.page_size == 0
            || self.page_size > 50
            || self.rows.len() * self.columns.len() > 512
            || self.total_rows > 1000000
            || self.row_start > self.row_end
            || self.row_end > self.total_rows
            || self.row_end - self.row_start != self.rows.len()
            || self.rows.len() > self.page_size
            || self.row_start != requested / self.page_size * self.page_size
            || !self.elapsed_ms.is_finite()
            || self.elapsed_ms < 0.0
            || text_bytes > 128 * 1024
            || self.columns.iter().any(|value| value.contains('\0'))
            || self.rows.iter().any(|row| {
                row.len() != self.columns.len()
                    || row.iter().flatten().any(|value| value.contains('\0'))
            })
        {
            return Err("SQL result exceeds its page budget or source identity".into());
        }
        Ok(())
    }
    fn prepare_copy(&mut self) {
        fn cell(value: &str) -> String {
            if value.contains(['\t', '\n', '\r', '"']) {
                format!("\"{}\"", value.replace('"', "\"\""))
            } else {
                value.into()
            }
        }
        let mut lines = Vec::with_capacity(self.rows.len() + 1);
        lines.push(
            self.columns
                .iter()
                .map(|value| cell(value))
                .collect::<Vec<_>>()
                .join("\t"),
        );
        for row in &self.rows {
            lines.push(
                row.iter()
                    .map(|value| cell(value.as_deref().unwrap_or("NULL")))
                    .collect::<Vec<_>>()
                    .join("\t"),
            );
        }
        self.copy_text = lines.join("\n");
    }
}

pub async fn execute(
    api: &Api,
    reader: &str,
    document: &str,
    revision: &str,
    operation: Operation,
) -> Result<Reply, String> {
    let payload = operation.payload(revision)?;
    if matches!(operation, Operation::SqlFormat { .. }) {
        return match crate::tables::execute(api, "format", payload).await? {
            crate::tables::Reply::Format(sql) => Ok(Reply::Formatted(sql)),
            _ => Err("Unexpected SQL formatter response".into()),
        };
    }
    let (method, route, body) = match &operation {
        Operation::SqlSchema { table } => (
            Method::GET,
            format!("table-query-schema?table={table}"),
            None,
        ),
        Operation::SqlPage { id, row, .. } => {
            (Method::GET, format!("table-query?id={id}&row={row}"), None)
        }
        Operation::Sql { .. } => (Method::POST, "table-query".into(), Some(&payload)),
        Operation::SqlClose { .. } => (Method::POST, "table-query-close".into(), Some(&payload)),
        _ => (Method::POST, "jq-query".into(), Some(&payload)),
    };
    let value = api
        .json_document(
            method,
            &format!("/studio/native/markdown/{reader}/{route}"),
            body,
            Some(document),
        )
        .await?;
    if matches!(
        operation,
        Operation::JqClose { .. } | Operation::SqlClose { .. }
    ) {
        return if value["closed"] == true {
            Ok(Reply::Closed)
        } else {
            Err("The query result was not closed".into())
        };
    }
    if let Operation::SqlSchema { table } = operation {
        let schema: SqlSchema = serde_json::from_value(value)
            .map_err(|error| format!("Invalid SQL schema: {error}"))?;
        if schema.revision != revision
            || schema.table != table
            || schema.columns.len() > 256
            || schema.columns.len() != schema.positional_columns.len()
            || schema.default_sql.len() > 16384
            || schema
                .columns
                .iter()
                .any(|name| name.len() > 2048 || name.contains('\0'))
        {
            return Err("SQL schema does not match the source table".into());
        }
        return Ok(Reply::Schema(schema));
    }
    if matches!(operation, Operation::Sql { .. } | Operation::SqlPage { .. }) {
        let page: SqlPage = serde_json::from_value(value)
            .map_err(|error| format!("Invalid SQL result: {error}"))?;
        page.validate(revision, &operation)?;
        return crate::worker::compute(move || {
            let mut page = page;
            page.prepare_copy();
            Ok(Reply::Sql(page))
        })
        .await;
    }
    let page: JqPage = serde_json::from_value(value)
        .map_err(|error| format!("Invalid query response: {error}"))?;
    page.validate(revision, &operation)?;
    Ok(Reply::Jq(page))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sql_pages_preserve_nulls_and_clipboard_values_without_crossing_sources() {
        let mut page = SqlPage {
            kind: "tableQuery".into(),
            id: "result-id".into(),
            revision: "source-revision".into(),
            table: 3,
            sql: "SELECT x".into(),
            columns: vec!["Text".into(), "Maybe".into()],
            rows: vec![vec![Some("中文😀\tline\n\"quoted\"".into()), None]],
            row_start: 0,
            row_end: 1,
            page_size: 50,
            total_rows: 1,
            elapsed_ms: 2.0,
            copy_text: String::new(),
        };
        let operation = Operation::Sql {
            table: 3,
            sql: "SELECT x".into(),
        };
        assert!(page.validate("source-revision", &operation).is_ok());
        assert!(page.validate("other-revision", &operation).is_err());
        assert!(page
            .validate(
                "source-revision",
                &Operation::SqlPage {
                    table: 4,
                    id: "result-id".into(),
                    row: 0
                }
            )
            .is_err());
        page.prepare_copy();
        assert_eq!(
            page.copy_text,
            "Text\tMaybe\n\"中文😀\tline\n\"\"quoted\"\"\"\tNULL"
        );
        page.rows[0].push(Some("extra".into()));
        assert!(page.validate("source-revision", &operation).is_err());
    }
    #[test]
    fn stale_or_misrouted_pages_and_oversized_text_are_rejected() {
        let mut page = JqPage {
            kind: "jqQuery".into(),
            id: "result-id".into(),
            revision: "source-revision".into(),
            page: 1,
            pages: 2,
            total_values: 2,
            text: "\"中文😀\"\n".into(),
            elapsed_ms: 1.0,
        };
        let operation = Operation::JqPage {
            id: "result-id".into(),
            page: 1,
        };
        assert!(page.validate("source-revision", &operation).is_ok());
        assert!(page.validate("old-revision", &operation).is_err());
        assert!(page
            .validate(
                "source-revision",
                &Operation::JqPage {
                    id: "other".into(),
                    page: 1
                }
            )
            .is_err());
        page.text = "x".repeat(48 * 1024 + 1);
        assert!(page.validate("source-revision", &operation).is_err());
    }
}
