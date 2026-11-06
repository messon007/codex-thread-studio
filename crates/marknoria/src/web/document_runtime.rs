use super::*;
use axum::response::Response;
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct JqRequest {
    #[serde(default)] filter: Option<String>,
    #[serde(default)] slurp: bool,
    #[serde(default)] id: Option<String>,
    #[serde(default)] page: usize,
    revision: String,
    #[serde(default)] operation_id: Option<String>,
    #[serde(default)] close: bool,
}
pub(super) async fn jq(
    State(app):State<App>,headers:HeaderMap,Json(request):Json<JqRequest>,
)->Result<Json<Value>,ApiError>{
    super::service::query_authority(&app)?;
    let document=app.documents.select(&headers)?;let index=index(&document)?;
    if request.revision!=index.revision{return Err((StatusCode::CONFLICT,"Document revision changed; reopen jq".into()));}
    let permit=app.work.clone().try_acquire_owned().map_err(|_|(StatusCode::TOO_MANY_REQUESTS,"Reader is busy".into()))?;
    let query_permit = document.query_work.clone().try_acquire_owned()
        .map_err(|_| (StatusCode::TOO_MANY_REQUESTS, "Another query is running for this document".into()))?;
    let operation = document.operation(request.operation_id.as_deref())?;
    tokio::task::spawn_blocking(move||{
        let _query_permit = query_permit;
        let _permit=permit;
        let mut queries=document.jq.try_lock().map_err(|_|(StatusCode::TOO_MANY_REQUESTS,"A jq query is already running".into()))?;
        if request.close {if let Some(id)=request.id{queries.close(&id);}return Ok(Json(json!({"closed":true})));}
        let bad=|e:std::io::Error|(StatusCode::BAD_REQUEST,e.to_string());
        let result=if let Some(filter)=request.filter {
            let executable=crate::runtime::process::self_executable().map_err(bad)?;
            queries.run(&document.directory,&index,&filter,request.slurp,&executable,&operation.flag).map_err(bad)?
        }else{queries.page(&index,request.id.as_deref().ok_or_else(||(StatusCode::BAD_REQUEST,"Missing query ID".into()))?,request.page).map_err(bad)?};
        Ok(Json(serde_json::to_value(result).map_err(failure)?))
    }).await.map_err(failure)?
}
pub(super) async fn query_snapshot(
    State(app): State<App>,
    headers: HeaderMap,
    Json(request): Json<crate::core::reader::QuerySnapshotRequest>,
) -> Result<Json<crate::core::reader::QuerySnapshot>, ApiError> {
    super::service::query_authority(&app)?;
    let document = app.documents.select(&headers)?;
    let index = index(&document)?;
    if request.revision != index.revision {
        return Err((StatusCode::CONFLICT, "Document revision changed; reopen SQL".into()));
    }
    let permit = app.work.clone().try_acquire_owned()
        .map_err(|_| (StatusCode::TOO_MANY_REQUESTS, "Reader is busy".into()))?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        document.queries.try_lock()
            .map_err(|_| (StatusCode::TOO_MANY_REQUESTS, "SQL result storage is busy".into()))?
            .snapshot(&index, &request)
            .map(Json)
            .map_err(|error| (StatusCode::BAD_REQUEST, error.to_string()))
    }).await.map_err(failure)?
}
pub(super) async fn notes(
    State(app): State<App>,
    headers: HeaderMap,
    Json(mut request): Json<crate::core::notes::Request>,
) -> Result<Json<Value>, ApiError> {
    let document = app.documents.select(&headers)?;
    let index = index(&document)?;
    let directory = app.notes_directory.clone()
        .ok_or((StatusCode::FORBIDDEN, "Note storage was not granted by the host".into()))?;
    let permit = app
        .work
        .clone()
        .try_acquire_owned()
        .map_err(|_| (StatusCode::TOO_MANY_REQUESTS, "Reader is busy".into()))?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        if matches!(&request, crate::core::notes::Request::Add { anchor, .. }
            if anchor.query.is_some() || matches!(&anchor.location, Some(crate::core::reader::Location::QueryCell { .. }))) {
            document.queries.try_lock()
                .map_err(|_| (StatusCode::TOO_MANY_REQUESTS, "SQL result storage is busy".into()))?
                .prepare_note(&index, &mut request)
                .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
        }
        (if let Some(identity) = &document.note_identity {
            crate::core::notes::execute_for_identity(&directory, identity, &index, request)
        } else {
            crate::core::notes::execute(&directory, &document.source, &index, request)
        })
            .map(Json)
            .map_err(|e| (if e.kind() == std::io::ErrorKind::WouldBlock { StatusCode::TOO_MANY_REQUESTS } else { StatusCode::BAD_REQUEST }, e.to_string()))
    })
    .await
    .map_err(failure)?
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct AssetInfoQuery {
    id: usize,
    revision: String,
}
pub(super) async fn asset_info(
    State(app): State<App>,
    headers: HeaderMap,
    Query(query): Query<AssetInfoQuery>,
) -> Result<Json<crate::core::reader::AssetInfo>, ApiError> {
    let document = app.documents.select(&headers)?;
    let index = index(&document)?;
    if query.revision != index.revision {
        return Err((StatusCode::CONFLICT, "Document revision changed".into()));
    }
    crate::core::reader::projected_asset_info(&index, query.id)
        .map(Json)
        .map_err(|error| (StatusCode::BAD_REQUEST, error.to_string()))
}

#[derive(Deserialize)]
pub(super) struct AssetQuery {
    id: usize,
    #[serde(default)]
    frame: usize,
}
pub(super) async fn asset(
    State(app): State<App>,
    headers: HeaderMap,
    Query(query): Query<AssetQuery>,
) -> Result<Response, ApiError> {
    let document = app.documents.select(&headers)?;
    let index = index(&document)?;
    let permit = app
        .work
        .clone()
        .try_acquire_owned()
        .map_err(|_| (StatusCode::TOO_MANY_REQUESTS, "Reader is busy".into()))?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let _lock = document
            .assets
            .try_lock()
            .map_err(|_| (StatusCode::TOO_MANY_REQUESTS, "Image worker is busy".into()))?;
        let path = crate::core::documents::asset(&document.directory, &index, query.id, query.frame)
            .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
        let bytes = std::fs::read(path).map_err(failure)?;
        axum::http::Response::builder()
            .header("content-type", "image/png")
            .header("x-reader-revision", &index.revision)
            .body(axum::body::Body::from(bytes))
            .map_err(failure)
    })
    .await
    .map_err(failure)?
}
