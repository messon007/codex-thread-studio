use super::{ApiError, App, files};
use crate::edit;
use axum::{
    Json,
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{io, path::PathBuf, sync::Mutex};

struct Session {
    document: String,
    path: PathBuf,
    snapshot: edit::Snapshot,
}

#[derive(Default)]
pub(super) struct Editor {
    current: Mutex<Option<Session>>,
}

impl Editor {
    pub(super) fn clear(&self) {
        self.current.lock().unwrap().take();
    }
}

fn failure(error: io::Error) -> ApiError {
    let status = match error.kind() {
        io::ErrorKind::WouldBlock | io::ErrorKind::NotFound => StatusCode::CONFLICT,
        io::ErrorKind::InvalidInput | io::ErrorKind::InvalidData => StatusCode::BAD_REQUEST,
        io::ErrorKind::PermissionDenied => StatusCode::FORBIDDEN,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    };
    (
        status,
        format!("{error}. The current page has not been discarded."),
    )
}

fn validate(app: &App, headers: &HeaderMap) -> Result<std::sync::Arc<files::Document>, ApiError> {
    if !app.editable {
        return Err((StatusCode::FORBIDDEN, "This reader is read only".into()));
    }
    if !headers.contains_key("x-reader-document") {
        return Err((StatusCode::CONFLICT, "Document identity is required".into()));
    }
    let document = app.documents.select(headers)?;
    if document.source.canonicalize().ok().as_ref() != Some(&document.source)
        || !document.source.starts_with(&app.documents.root)
    {
        return Err((
            StatusCode::CONFLICT,
            "The file path changed; reopen it before editing".into(),
        ));
    }
    Ok(document)
}

fn response(
    session: &Session,
    window: crate::source::SourceWindow,
    warning: Option<String>,
) -> Json<Value> {
    Json(
        json!({"documentId":session.document,"revision":session.snapshot.index.revision,
        "sourceBytes":session.snapshot.index.source_bytes,"sourceLines":session.snapshot.index.source_lines,
        "maxPageBytes":edit::MAX_PAGE,"window":window,"warning":warning}),
    )
}

#[derive(Deserialize)]
pub(super) struct Page {
    line: Option<usize>,
    offset: Option<usize>,
    #[serde(default)]
    previous: bool,
    #[serde(default)]
    reload: bool,
}

pub(super) async fn page(
    State(app): State<App>,
    headers: HeaderMap,
    Query(query): Query<Page>,
) -> Result<Json<Value>, ApiError> {
    let permit = app.open_work.clone().try_acquire_owned().map_err(|_| {
        (
            StatusCode::TOO_MANY_REQUESTS,
            "Another file operation is running".into(),
        )
    })?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let document = validate(&app, &headers)?;
        let mut current = app.editor.current.lock().map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Editor lock failed".into(),
            )
        })?;
        if query.reload
            || current.as_ref().is_none_or(|session| {
                session.document != document.id || session.path != document.source
            })
        {
            let snapshot = edit::snapshot(
                &document.source,
                files::temporary_directory("marknoria-edit-").map_err(failure)?,
            )
            .map_err(failure)?;
            let id = if query.reload {
                let next = app
                    .documents
                    .open(
                        document.source.clone(),
                        files::cache_directory().map_err(failure)?,
                    )
                    .map_err(failure)?;
                app.comparisons.cancel();
                next.id.clone()
            } else {
                document.id.clone()
            };
            *current = Some(Session {
                document: id,
                path: document.source.clone(),
                snapshot,
            });
        }
        let session = current.as_ref().unwrap();
        let line = query
            .line
            .map(|line| line.clamp(1, session.snapshot.index.source_lines));
        let window = session
            .snapshot
            .page(line, query.offset, query.previous)
            .map_err(failure)?;
        Ok(response(session, window, None))
    })
    .await
    .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?
}

pub(super) async fn save(
    State(app): State<App>,
    headers: HeaderMap,
    Json(request): Json<edit::Save>,
) -> Result<Json<Value>, ApiError> {
    let permit = app.open_work.clone().try_acquire_owned().map_err(|_| {
        (
            StatusCode::TOO_MANY_REQUESTS,
            "Another file operation is running".into(),
        )
    })?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let document = validate(&app, &headers)?;
        let mut current = app.editor.current.lock().map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Editor lock failed".into(),
            )
        })?;
        let session = current
            .as_mut()
            .filter(|session| session.document == document.id && session.path == document.source)
            .ok_or((
                StatusCode::CONFLICT,
                "Open this file in Edit before saving".into(),
            ))?;
        let directory = files::temporary_directory("marknoria-edit-").map_err(failure)?;
        let reader_cache = files::cache_directory().map_err(failure)?;
        let commit =
            edit::save(&session.snapshot, &session.path, &request, directory).map_err(failure)?;
        session.snapshot = commit.snapshot;
        app.comparisons.cancel();
        let mut warning = commit.warning;
        match app.documents.open(session.path.clone(), reader_cache) {
            Ok(document) => session.document = document.id.clone(),
            Err(error) => {
                warning = Some(format!(
                    "File saved, but starting the reading index failed: {error}. Reopen the file."
                ))
            }
        }
        Ok(response(session, commit.window, warning))
    })
    .await
    .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?
}
