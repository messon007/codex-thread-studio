use super::*;
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Run {
    table: usize,
    sql: String,
    revision: String,
    #[serde(default)]
    operation_id: Option<String>,
}
#[derive(Deserialize)]
pub(super) struct Schema {
    table: usize,
}
#[derive(Deserialize)]
pub(super) struct Page {
    id: String,
    #[serde(default)]
    row: usize,
}
#[derive(Deserialize)]
pub(super) struct Close {
    id: String,
}
fn bad(error: impl std::fmt::Display) -> ApiError {
    (StatusCode::BAD_REQUEST, error.to_string())
}
pub(super) async fn schema(
    State(app): State<App>,
    Query(query): Query<Schema>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    super::service::query_authority(&app)?;
    let document = app.documents.select(&headers)?;
    let index = index(&document)?;
    Ok(Json(
        serde_json::to_value(crate::core::table_query::schema(&index, query.table).map_err(bad)?)
            .map_err(failure)?,
    ))
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
            "Document revision changed; reopen SQL".into(),
        ));
    }
    let permit = app
        .work
        .clone()
        .try_acquire_owned()
        .map_err(|_| (StatusCode::TOO_MANY_REQUESTS, "Reader is busy".into()))?;
    let query_permit = document.query_work.clone().try_acquire_owned()
        .map_err(|_| (StatusCode::TOO_MANY_REQUESTS, "Another query is running for this document".into()))?;
    let operation = document.operation(query.operation_id.as_deref())?;
    tokio::task::spawn_blocking(move || {
        let _query_permit = query_permit;
        let _permit = permit;
        let result = document
            .queries
            .try_lock()
            .map_err(|_| {
                (
                    StatusCode::TOO_MANY_REQUESTS,
                    "A table query is already running".into(),
                )
            })?
            .run(
                &document.directory,
                &index,
                query.table,
                &query.sql,
                operation.flag.clone(),
            )
            .map_err(bad)?;
        Ok(Json(serde_json::to_value(result).map_err(failure)?))
    })
    .await
    .map_err(failure)?
}
pub(super) async fn page(
    State(app): State<App>,
    Query(query): Query<Page>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    super::service::query_authority(&app)?;
    let document = app.documents.select(&headers)?;
    let index = index(&document)?;
    let permit = app
        .work
        .clone()
        .try_acquire_owned()
        .map_err(|_| (StatusCode::TOO_MANY_REQUESTS, "Reader is busy".into()))?;
    let query_permit = document.query_work.clone().try_acquire_owned()
        .map_err(|_| (StatusCode::TOO_MANY_REQUESTS, "Another query is running for this document".into()))?;
    tokio::task::spawn_blocking(move || {
        let _query_permit = query_permit;
        let _permit = permit;
        let result = document
            .queries
            .try_lock()
            .map_err(|_| {
                (
                    StatusCode::TOO_MANY_REQUESTS,
                    "A table query is already running".into(),
                )
            })?
            .page(&index, &query.id, query.row)
            .map_err(bad)?;
        Ok(Json(serde_json::to_value(result).map_err(failure)?))
    })
    .await
    .map_err(failure)?
}
pub(super) async fn close(
    State(app): State<App>,
    headers: HeaderMap,
    Json(query): Json<Close>,
) -> Result<Json<Value>, ApiError> {
    super::service::query_authority(&app)?;
    let document = app.documents.select(&headers)?;
    let _query_permit = document.query_work.clone().try_acquire_owned()
        .map_err(|_| (StatusCode::TOO_MANY_REQUESTS, "Another query is running for this document".into()))?;
    document
        .queries
        .try_lock()
        .map_err(|_| {
            (
                StatusCode::TOO_MANY_REQUESTS,
                "A table query is already running".into(),
            )
        })?
        .close(&query.id);
    Ok(Json(json!({"closed":true})))
}
