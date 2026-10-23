use super::{ApiError, App, compare_runtime, failure, files};
use axum::{
    Json,
    extract::{Path, Query, Request, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;

fn description(documents: &files::Documents) -> Option<Value> {
    let document = documents.peek()?;
    let progress = document.progress.lock().unwrap();
    Some(
        json!({"id":document.id,"path":document.source.strip_prefix(&documents.root).ok()?.to_string_lossy().replace('\\', "/"),
        "ready":progress.index.is_some(),"error":progress.error}),
    )
}

pub(super) async fn status(State(app): State<App>) -> Json<Value> {
    let left = description(&app.documents);
    let right = description(&app.comparison_documents);
    let comparison = left.as_ref().zip(right.as_ref()).and_then(|(left, right)| {
        app.comparisons
            .for_readers([left["id"].as_str()?, right["id"].as_str()?])
    });
    Json(json!({"left":left,"right":right,"comparisonId":comparison}))
}

#[derive(Deserialize)]
pub(super) struct OpenRequest {
    side: String,
    path: String,
}

pub(super) async fn open(
    State(mut app): State<App>,
    Json(request): Json<OpenRequest>,
) -> Result<Json<Value>, ApiError> {
    app.documents = select(&app, &request.side)?;
    files::open_server(State(app), Query(files::PathQuery { path: request.path })).await
}

fn select(app: &App, side: &str) -> Result<Arc<files::Documents>, ApiError> {
    match side {
        "left" => Ok(app.documents.clone()),
        "right" => Ok(app.comparison_documents.clone()),
        _ => Err((StatusCode::BAD_REQUEST, "Invalid reading pane".into())),
    }
}

pub(super) async fn close(State(app): State<App>) -> Result<Json<Value>, ApiError> {
    let permit = app
        .open_work
        .clone()
        .try_acquire_owned()
        .map_err(|_| (StatusCode::CONFLICT, "Another file is being opened".into()))?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        app.comparisons.cancel();
        app.comparison_documents.shutdown();
        Json(json!({"closed":true}))
    })
    .await
    .map_err(failure)
}

pub(super) async fn page(
    State(app): State<App>,
    Path((id, side)): Path<(String, String)>,
    request: Request,
) -> Response {
    serve(app, id, side, String::new(), request).await
}

pub(super) async fn asset(
    State(app): State<App>,
    Path((id, side, path)): Path<(String, String, String)>,
    request: Request,
) -> Response {
    serve(app, id, side, path, request).await
}

async fn serve(mut app: App, id: String, side: String, path: String, request: Request) -> Response {
    app.documents = match select(&app, &side) {
        Ok(documents) => documents,
        Err(error) => return error.into_response(),
    };
    if app
        .documents
        .peek()
        .is_none_or(|document| document.id != id)
    {
        return StatusCode::GONE.into_response();
    }
    compare_runtime::serve_reader(app, path, request).await
}
