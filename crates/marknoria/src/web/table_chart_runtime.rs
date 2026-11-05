use super::*;
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Run {
    revision: String,
    source: crate::core::table_query::ChartSource,
    x: Option<usize>,
    y: usize,
    #[serde(default)]
    operation_id: Option<String>,
}
pub(super) async fn run(
    State(app): State<App>,
    headers: HeaderMap,
    Json(query): Json<Run>,
) -> Result<Json<Value>, ApiError> {
    super::service::query_authority(&app)?;
    let document = app.documents.select(&headers)?;
    let index = index(&document)?;
    if query.revision != index.revision {
        return Err((
            StatusCode::CONFLICT,
            "Document revision changed; reopen the chart".into(),
        ));
    }
    let permit = app
        .work
        .clone()
        .try_acquire_owned()
        .map_err(|_| (StatusCode::TOO_MANY_REQUESTS, "Reader is busy".into()))?;
    let query_permit = document
        .query_work
        .clone()
        .try_acquire_owned()
        .map_err(|_| {
            (
                StatusCode::TOO_MANY_REQUESTS,
                "Another query is running for this document".into(),
            )
        })?;
    let operation = document.operation(query.operation_id.as_deref())?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let _query_permit = query_permit;
        let result = document
            .queries
            .try_lock()
            .map_err(|_| {
                (
                    StatusCode::TOO_MANY_REQUESTS,
                    "A table query is already running".into(),
                )
            })?
            .chart(
                &document.directory,
                &index,
                query.source,
                query.x,
                query.y,
                operation.flag.clone(),
            )
            .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
        Ok(Json(serde_json::to_value(result).map_err(failure)?))
    })
    .await
    .map_err(failure)?
}
