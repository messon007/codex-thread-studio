//! Native table transport and validation, on Rust workers rather than GTK.
use crate::gateway::Api;
use serde::Deserialize;
use serde_json::Value;
use std::sync::Arc;

pub fn supports(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| matches!(value.to_ascii_lowercase().as_str(), "csv" | "tsv" | "xlsx"))
}
#[derive(Debug, Clone, Deserialize)]
pub struct Sheet {
    pub name: String,
    pub rows: usize,
    pub columns: usize,
    pub truncated: bool,
    pub sql: String,
}
#[derive(Debug, Deserialize)]
pub struct Opened {
    pub id: String,
    pub name: String,
    pub path: String,
    pub hash: String,
    pub delimiter: Option<String>,
    pub sheets: Vec<Sheet>,
    pub page: Page,
}
#[derive(Debug, Deserialize)]
pub struct Cell {
    pub text: String,
    pub clipped: bool,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Page {
    pub document_id: String,
    pub sheet: usize,
    pub query_id: Option<String>,
    pub columns: Vec<String>,
    pub offset: usize,
    pub total: usize,
    pub page_size: usize,
    pub rows: Vec<Vec<Cell>>,
    pub truncated: bool,
}
impl Page {
    fn validate(&self) -> Result<(), String> {
        if self.document_id.is_empty()
            || self.document_id.len() > 64
            || self.sheet >= 64
            || self
                .query_id
                .as_ref()
                .is_some_and(|id| id.is_empty() || id.len() > 64)
            || self.columns.is_empty()
            || self.columns.len() > 200
            || self.rows.len() * self.columns.len() > 512
            || self.rows.len() > 50
            || self.total > 10000
            || self.page_size == 0
            || self.page_size > 50
            || self.offset > self.total
            || self.rows.len() + self.offset > self.total
            || self
                .columns
                .iter()
                .any(|column| column.len() > 64 * 1024 || column.contains('\0'))
            || self.rows.iter().any(|row| {
                row.len() != self.columns.len()
                    || row
                        .iter()
                        .any(|cell| cell.text.len() > 2048 || cell.text.contains('\0'))
            })
        {
            return Err("Table page exceeds its presentation budget".into());
        }
        Ok(())
    }
}
#[derive(Debug, Deserialize)]
pub struct Point {
    pub label: String,
    pub value: f64,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Chart {
    pub column: String,
    pub max_abs: f64,
    pub points: Vec<Point>,
}
#[derive(Debug, Deserialize)]
pub struct Comment {
    pub text: String,
    pub anchor: crate::annotations::TableAnchor,
}
#[derive(Debug)]
pub enum Reply {
    Opened(Opened),
    Page(Arc<Page>),
    Cell(String),
    Chart(Chart),
    Comment(Comment),
    Format(String),
    Closed,
}
#[derive(Debug)]
pub struct Response {
    pub generation: u64,
    pub serial: u64,
    pub kind: &'static str,
    pub result: Result<Reply, String>,
}
pub async fn execute(api: &Api, kind: &str, body: Value) -> Result<Reply, String> {
    if kind == "resume" {
        return Box::pin(crate::viewer_resume::table(api, body)).await;
    }
    if kind == "close" {
        for attempt in 0..3 {
            let (status, raw) = api
                .json_status(reqwest::Method::POST, "/studio/native/tables", &body)
                .await?;
            if status == reqwest::StatusCode::TOO_MANY_REQUESTS && attempt < 2 {
                tokio::time::sleep(std::time::Duration::from_millis(40 * (attempt + 1))).await;
                continue;
            }
            return if status.is_success() {
                Ok(Reply::Closed)
            } else {
                Err(crate::gateway::response_error(status, &raw))
            };
        }
    }
    let raw = api
        .json(reqwest::Method::POST, "/studio/native/tables", Some(&body))
        .await?;
    let kind = kind.to_owned();
    crate::worker::compute(move || match kind.as_str() {
        "open" => {
            let opened: Opened =
                serde_json::from_value(raw).map_err(|_| "Invalid table metadata")?;
            if opened.sheets.is_empty()
                || opened.sheets.len() > 64
                || opened.id.len() > 64
                || opened.name.len() > 4096
                || opened.name.contains('\0')
                || opened.path.len() > 8192
                || opened.hash.len() > 128
                || opened.sheets.iter().any(|sheet| {
                    sheet.name.len() > 1024
                        || sheet.name.contains('\0')
                        || sheet.rows > 10000
                        || sheet.columns > 200
                        || sheet.sql.len() > 64 * 1024
                })
            {
                return Err("Table metadata exceeds its budget".into());
            }
            opened.page.validate()?;
            if opened.page.document_id != opened.id
                || opened.page.sheet != 0
                || opened.page.query_id.is_some()
            {
                return Err("Table metadata and initial page identities differ".into());
            }
            Ok(Reply::Opened(opened))
        }
        "page" | "query" => {
            let page: Page = serde_json::from_value(raw).map_err(|_| "Invalid table page")?;
            page.validate()?;
            Ok(Reply::Page(Arc::new(page)))
        }
        "cell" | "copy" => {
            let text = raw["text"].as_str().ok_or("Invalid table cell")?;
            if text.len() > 512 * 1024 || text.contains('\0') {
                return Err("Cell text exceeds its native display budget".into());
            }
            Ok(Reply::Cell(text.into()))
        }
        "chart" => {
            let chart: Chart = serde_json::from_value(raw).map_err(|_| "Invalid table chart")?;
            if chart.points.len() > 100
                || !chart.max_abs.is_finite()
                || chart.max_abs <= 0.0
                || chart.column.len() > 64 * 1024
                || chart.column.contains('\0')
                || chart.points.iter().any(|point| {
                    !point.value.is_finite()
                        || point.label.contains('\0')
                        || point.label.chars().count() > 160
                })
            {
                return Err("Chart data exceeds its budget".into());
            }
            Ok(Reply::Chart(chart))
        }
        "comment" | "source" => {
            let comment: Comment =
                serde_json::from_value(raw).map_err(|_| "Invalid table comment response")?;
            if comment.text.len() > 16384 || comment.text.contains('\0') {
                return Err("Table comment text exceeds its budget".into());
            }
            comment.anchor.validate()?;
            Ok(Reply::Comment(comment))
        }
        "format" => Ok(Reply::Format(
            raw["sql"]
                .as_str()
                .filter(|text| text.len() <= 64 * 1024 && !text.contains('\0'))
                .ok_or("Invalid formatted SQL")?
                .into(),
        )),
        "close" => Ok(Reply::Closed),
        _ => Err("Unsupported table operation".into()),
    })
    .await
}
