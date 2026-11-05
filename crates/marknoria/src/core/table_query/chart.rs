//! Complete-input plot projection. Visible table pages are never chart input.
use super::*;
use serde::Deserialize;

pub const MAX_CHART_POINTS: usize = 2000;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ChartSource {
    Table { table: usize },
    Query { query_id: String },
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChartPoint {
    pub row: usize,
    pub label: String,
    pub label_truncated: bool,
    pub value: Option<f64>,
    pub value_text: Option<String>,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Chart {
    pub revision: String,
    pub source: ChartSource,
    pub dataset_scope: &'static str,
    pub value_precision: &'static str,
    pub input_rows: usize,
    pub rows: usize,
    pub x_name: String,
    pub y_name: String,
    pub points: Vec<ChartPoint>,
}

impl Queries {
    pub fn chart(
        &mut self,
        parent: &Path,
        index: &Index,
        source: ChartSource,
        x: Option<usize>,
        y: usize,
        cancelled: Arc<AtomicBool>,
    ) -> io::Result<Chart> {
        if cancelled.load(Ordering::Relaxed) {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "Chart cancelled",
            ));
        }
        match source.clone() {
            ChartSource::Query { query_id } => {
                self.chart_result(index, &query_id, source, x, y, &cancelled)
            }
            ChartSource::Table { table } => {
                let metadata = schema(index, table)?;
                if metadata.source_rows > MAX_CHART_POINTS {
                    return Err(error(
                        "Chart exceeds 2,000 points; use SQL to filter or aggregate the complete source table",
                    ));
                }
                if y >= metadata.columns.len() || x.is_some_and(|x| x >= metadata.columns.len()) {
                    return Err(error("Chart column is out of range"));
                }
                let label = x
                    .map(|x| format!("c{}", x + 1))
                    .unwrap_or_else(|| "_row".into());
                let sql = format!("SELECT {label},c{} FROM t ORDER BY _row", y + 1);
                // The temporary projection must not evict the host's four live
                // SQL results. Input preparation/cache still uses this owner.
                let saved = std::mem::take(&mut self.results);
                let outcome = (|| {
                    let result = self.run(parent, index, table, &sql, cancelled.clone())?;
                    let mut chart =
                        self.chart_result(index, &result.id, source, Some(0), 1, &cancelled)?;
                    chart.x_name = x
                        .map(|x| metadata.columns[x].clone())
                        .unwrap_or_else(|| "Row".into());
                    chart.y_name = metadata.columns[y].clone();
                    chart.dataset_scope = "complete-source-table";
                    if serde_json::to_vec(&chart)?.len() > crate::core::MAX_HTML {
                        return Err(error(
                            "Chart exceeds the 128 KiB transport budget; shorten labels using SQL",
                        ));
                    }
                    Ok(chart)
                })();
                self.results = saved;
                outcome
            }
        }
    }

    fn chart_result(
        &self,
        index: &Index,
        id: &str,
        source: ChartSource,
        x: Option<usize>,
        y: usize,
        cancelled: &AtomicBool,
    ) -> io::Result<Chart> {
        let result = self
            .results
            .iter()
            .find(|r| r.id == id && r.revision == index.revision)
            .ok_or_else(|| error("Query result expired; run the query again"))?;
        if result.total > MAX_CHART_POINTS {
            return Err(error(
                "Chart exceeds 2,000 points; aggregate or filter the complete SQL result",
            ));
        }
        if y >= result.columns.len() || x.is_some_and(|x| x >= result.columns.len()) {
            return Err(error("Chart column is out of range"));
        }
        let mut chart = Chart {
            revision: index.revision.clone(),
            source,
            dataset_scope: "complete-query-result",
            value_precision: "float64",
            input_rows: index.tables[result.table].rows,
            rows: result.total,
            x_name: x
                .map(|x| result.columns[x].clone())
                .unwrap_or_else(|| "Row".into()),
            y_name: result.columns[y].clone(),
            points: Vec::new(),
        };
        let mut bytes = serde_json::to_vec(&chart)?.len() + 1024;
        let started = Instant::now();
        let mut input = BufReader::new(File::open(result.directory.path().join("rows.jsonl"))?);
        for row in 0..result.total {
            if cancelled.load(Ordering::Relaxed) {
                return Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "Chart cancelled",
                ));
            }
            if started.elapsed() > Duration::from_secs(2) {
                return Err(error("Chart projection exceeded two seconds"));
            }
            let mut line = String::new();
            input
                .by_ref()
                .take(result.max_row as u64 + 1)
                .read_line(&mut line)?;
            let cells: Vec<Option<String>> = serde_json::from_str(&line)?;
            let raw = cells
                .get(y)
                .ok_or_else(|| error("Query result column count changed"))?
                .clone();
            let value = raw.as_deref().filter(|s| !s.trim().is_empty()).map(|text| {
                let value = text.trim().parse::<f64>().map_err(|_| error(format!("Chart row {} is not numeric; use SQL CAST/NULLIF or select a numeric column", row + 1)))?;
                if !value.is_finite() { return Err(error("Chart values must be finite; filter non-finite values using SQL")); }
                if text.len() > 1024 { return Err(error("Chart numeric label exceeds its budget; use SQL CAST")); }
                Ok(value)
            }).transpose()?;
            let label = if let Some(x) = x {
                cells
                    .get(x)
                    .ok_or_else(|| error("Query result column count changed"))?
                    .as_deref()
                    .unwrap_or("NULL")
                    .to_owned()
            } else {
                (row + 1).to_string()
            };
            let mut end = label.len().min(128);
            while !label.is_char_boundary(end) {
                end -= 1;
            }
            let point = ChartPoint {
                row,
                label: label[..end].to_owned(),
                label_truncated: end < label.len(),
                value,
                value_text: raw,
            };
            bytes += serde_json::to_vec(&point)?.len() + 1;
            if bytes > crate::core::MAX_HTML {
                return Err(error(
                    "Chart exceeds the 128 KiB transport budget; use SQL to shorten labels or aggregate rows",
                ));
            }
            chart.points.push(point);
        }
        Ok(chart)
    }
}
