use super::*;
#[derive(Deserialize)]
pub(super) struct Run {
    revision: String,
    filter: Option<String>,
    #[serde(default)]
    slurp: bool,
    id: Option<String>,
    #[serde(default)]
    page: usize,
    #[serde(default)]
    close: bool,
}
pub(super) async fn run(
    State(app): State<App>,
    headers: HeaderMap,
    Json(query): Json<Run>,
) -> Result<Json<Value>, ApiError> {
    let document = app.documents.select(&headers)?;
    let index = index(&document)?;
    if index.revision != query.revision {
        return Err((StatusCode::CONFLICT, "Document revision changed".into()));
    }
    let permit = app
        .work
        .clone()
        .try_acquire_owned()
        .map_err(|_| (StatusCode::TOO_MANY_REQUESTS, "Reader is busy".into()))?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let mut queries = document.jq.try_lock().map_err(|_| {
            (
                StatusCode::TOO_MANY_REQUESTS,
                "A jq query is running".into(),
            )
        })?;
        let result = if let Some(filter) = query.filter {
            queries.run(
                &document.directory,
                &index,
                &filter,
                query.slurp,
                &env::current_exe().map_err(failure)?,
                &document.cancelled,
            )
        } else if let Some(id) = query.id {
            if query.close {
                queries.close(&id);
                return Ok(Json(json!({"closed":true})));
            }
            queries.page(&index, &id, query.page)
        } else {
            return Err((
                StatusCode::BAD_REQUEST,
                "Missing filter or result id".into(),
            ));
        };
        Ok(Json(
            serde_json::to_value(result.map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?)
                .map_err(failure)?,
        ))
    })
    .await
    .map_err(failure)?
}
