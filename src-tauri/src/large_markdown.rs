use crate::{
    json_error, json_response, resolve_review_path_with_shared, review_language,
    shared_document_directories, GatewayState, ReviewFileRequest,
};
use axum::{
    body::{to_bytes, Body},
    extract::{Path, Request, State},
    http::{header, Method, Response, StatusCode},
    Json,
};
use marknoria::server::Service;
use serde::Deserialize;
use serde_json::json;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

pub const THRESHOLD: u64 = 1024 * 1024;
const MAX_READERS: usize = 4;
const IDLE: Duration = Duration::from_secs(120);

struct Reader {
    service: Service,
    touched: Instant,
    editable: bool,
    source: std::path::PathBuf,
    root: std::path::PathBuf,
}

#[derive(Clone)]
pub struct Readers {
    entries: Arc<Mutex<HashMap<String, Reader>>>,
    opening: Arc<tokio::sync::Semaphore>,
    client: reqwest::Client,
}

impl Default for Readers {
    fn default() -> Self {
        Self {
            entries: Arc::new(Mutex::new(HashMap::new())),
            opening: Arc::new(tokio::sync::Semaphore::new(1)),
            client: reqwest::Client::builder()
                .no_proxy()
                .redirect(reqwest::redirect::Policy::none())
                .timeout(Duration::from_secs(30))
                .build()
                .expect("reader HTTP client"),
        }
    }
}

impl Readers {
    pub async fn open(
        &self,
        source: std::path::PathBuf,
        root: std::path::PathBuf,
        editable: bool,
    ) -> Result<serde_json::Value, (StatusCode, String)> {
        let _permit = self.opening.clone().try_acquire_owned().map_err(|_| {
            (
                StatusCode::TOO_MANY_REQUESTS,
                "Another document is opening".into(),
            )
        })?;
        {
            let mut entries = self.entries.lock().unwrap();
            entries.retain(|_, reader| reader.touched.elapsed() < IDLE);
            if entries.len() >= MAX_READERS {
                return Err((
                    StatusCode::TOO_MANY_REQUESTS,
                    "Close an existing large document first".into(),
                ));
            }
        }
        let service = if editable {
            Service::start_editable(source.clone()).await
        } else {
            Service::start(source.clone()).await
        }
        .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;
        let id = uuid::Uuid::new_v4().to_string();
        let token = service.token.clone();
        let empty = self.entries.lock().unwrap().is_empty();
        self.entries.lock().unwrap().insert(
            id.clone(),
            Reader {
                service,
                touched: Instant::now(),
                editable,
                source,
                root,
            },
        );
        if empty {
            let entries = Arc::downgrade(&self.entries);
            tokio::spawn(async move {
                loop {
                    tokio::time::sleep(Duration::from_secs(30)).await;
                    let Some(entries) = entries.upgrade() else {
                        break;
                    };
                    let mut entries = entries.lock().unwrap();
                    entries.retain(|_, reader| reader.touched.elapsed() < IDLE);
                    if entries.is_empty() {
                        break;
                    }
                }
            });
        }
        Ok(
            json!({"id":id,"url":format!("/markdown-reader/{id}/?embedded=1&diagrams=browser#{token}")}),
        )
    }
}

pub async fn try_open(
    state: &GatewayState,
    request: &ReviewFileRequest,
) -> Result<Option<serde_json::Value>, (StatusCode, String)> {
    if review_language(std::path::Path::new(&request.path)) != "markdown" {
        return Ok(None);
    }
    let shared = shared_document_directories(state)?;
    let review = ReviewFileRequest {
        root: request.root.clone(),
        path: request.path.clone(),
    };
    let (root, path, metadata, document_root) = tokio::task::spawn_blocking(move || {
        resolve_review_path_with_shared(&review, marknoria::MAX_SOURCE, "128 MiB Markdown", &shared)
    })
    .await
    .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))??;
    if metadata.len() < THRESHOLD {
        return Ok(None);
    }
    let read_only = !path.starts_with(&root);
    let reader = state
        .markdown_readers
        .open(path.clone(), root.clone(), !read_only)
        .await?;
    Ok(Some(json!({
        "root":root,"path":path,"relativePath":path.strip_prefix(&document_root).unwrap_or(&path),
        "documentRoot":document_root,"size":metadata.len(),"readOnly":read_only,"largeMarkdown":reader,
    })))
}

#[derive(Deserialize)]
pub struct CloseRequest {
    id: String,
}

pub async fn close(
    State(state): State<GatewayState>,
    Json(request): Json<CloseRequest>,
) -> Response<Body> {
    state
        .markdown_readers
        .entries
        .lock()
        .unwrap()
        .remove(&request.id);
    json_response(StatusCode::OK, &json!({"closed":true}))
}

fn allowed_path(path: &str) -> bool {
    matches!(
        path,
        "" | "reader.mjs"
            | "styles.css"
            | "source.mjs"
            | "position.mjs"
            | "editor.mjs"
            | "theme.js"
            | "checkbox.css"
            | "syntax-light.css"
            | "syntax-dark.css"
            | "tables.mjs"
            | "icons.mjs"
            | "files.mjs"
            | "diagrams.mjs"
            | "engine.mjs"
            | "vendor/purify.mjs"
            | "vendor/virtual-core.mjs"
            | "vendor/editor.mjs"
            | "vendor/mermaid.min.js"
            | "vendor/plantuml.js"
            | "vendor/plantuml-viz-global.js"
            | "vendor/d2.js"
            | "api/document"
            | "api/window"
            | "api/table"
            | "api/source"
            | "api/edit"
            | "api/search"
            | "api/diagram-source"
            | "api/renderer"
    )
}

fn edit_scope(source: &std::path::Path, root: &std::path::Path) -> bool {
    source.starts_with(root)
        && source.canonicalize().ok().as_deref() == Some(source)
        && root.canonicalize().ok().as_deref() == Some(root)
}

pub async fn page(
    State(state): State<GatewayState>,
    Path(id): Path<String>,
    request: Request,
) -> Response<Body> {
    proxy(state, id, String::new(), request).await
}

pub async fn asset(
    State(state): State<GatewayState>,
    Path((id, path)): Path<(String, String)>,
    request: Request,
) -> Response<Body> {
    proxy(state, id, path, request).await
}

async fn proxy(state: GatewayState, id: String, path: String, request: Request) -> Response<Body> {
    if state
        .security
        .validate_reader_origin(request.headers())
        .is_err()
    {
        return json_error(StatusCode::FORBIDDEN, "Reader origin rejected");
    }
    if !allowed_path(&path) {
        return json_error(StatusCode::NOT_FOUND, "Unknown reader route");
    }
    if request.method() != Method::GET && !(path == "api/edit" && request.method() == Method::POST)
    {
        return json_error(StatusCode::METHOD_NOT_ALLOWED, "Reader method rejected");
    }
    let (address, source, root) = {
        let mut entries = state.markdown_readers.entries.lock().unwrap();
        let Some(reader) = entries.get_mut(&id) else {
            return json_error(StatusCode::GONE, "Document closed or expired; reopen it");
        };
        if path.starts_with("api/")
            && request
                .headers()
                .get("x-reader-token")
                .is_none_or(|value| value.as_bytes() != reader.service.token.as_bytes())
        {
            return json_error(StatusCode::UNAUTHORIZED, "Reader token required");
        }
        reader.touched = Instant::now();
        if path == "api/edit" && !reader.editable {
            return json_error(StatusCode::FORBIDDEN, "This document is read only");
        }
        (
            reader.service.address,
            reader.source.clone(),
            reader.root.clone(),
        )
    };
    if path == "api/edit" {
        let valid = tokio::task::spawn_blocking(move || edit_scope(&source, &root))
            .await
            .unwrap_or(false);
        if !valid {
            return json_error(
                StatusCode::FORBIDDEN,
                "The document path changed; reopen it before editing",
            );
        }
    }
    let query = request
        .uri()
        .query()
        .map(|value| format!("?{value}"))
        .unwrap_or_default();
    let mut upstream = state.markdown_readers.client.request(
        request.method().clone(),
        format!("http://{address}/{path}{query}"),
    );
    for name in ["x-reader-token", "x-reader-document", "content-type"] {
        if let Some(value) = request.headers().get(name) {
            upstream = upstream.header(name, value);
        }
    }
    if request.method() == Method::POST {
        let bytes = match to_bytes(request.into_body(), 2 * 1024 * 1024).await {
            Ok(bytes) => bytes,
            Err(_) => {
                return json_error(
                    StatusCode::PAYLOAD_TOO_LARGE,
                    "Editing request exceeds 2 MiB",
                )
            }
        };
        upstream = upstream.body(bytes);
    }
    let response = match upstream.send().await {
        Ok(response) => response,
        Err(error) => {
            return json_error(
                StatusCode::BAD_GATEWAY,
                &format!("Reader unavailable: {error}"),
            )
        }
    };
    let mut builder = Response::builder().status(response.status())
        .header(header::CACHE_CONTROL, "no-store")
        .header("x-content-type-options", "nosniff")
        .header(header::REFERRER_POLICY, "no-referrer")
        .header(header::CONTENT_SECURITY_POLICY, "default-src 'none'; script-src 'self' 'wasm-unsafe-eval'; worker-src 'self' blob:; style-src 'self' 'unsafe-inline'; connect-src 'self' blob:; img-src blob: data:; font-src data:; object-src 'none'; base-uri 'none'; frame-ancestors 'self'; form-action 'none'");
    if let Some(content_type) = response.headers().get(header::CONTENT_TYPE) {
        builder = builder.header(header::CONTENT_TYPE, content_type);
    }
    if path.is_empty() && response.status().is_success() {
        let html = match response.text().await {
            Ok(html) => html,
            Err(error) => return json_error(StatusCode::BAD_GATEWAY, &error.to_string()),
        };
        let html = html.replace(
            "</head>",
            &format!(
                "<style>{}\n{}</style></head>",
                include_str!("../../ui/large-markdown-host.css"),
                include_str!("../../ui/vendor/marknoria-checkbox.css")
            ),
        );
        return builder
            .body(Body::from(html))
            .expect("reader page response");
    }
    builder
        .body(Body::from_stream(response.bytes_stream()))
        .expect("reader proxy response")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn embedded_routes_never_expose_browser_or_renderer_privileges() {
        for path in [
            "",
            "reader.mjs",
            "api/source",
            "api/edit",
            "editor.mjs",
            "vendor/editor.mjs",
            "api/table",
            "api/diagram-source",
            "engine.mjs",
            "vendor/mermaid.min.js",
        ] {
            assert!(allowed_path(path));
        }
        for path in [
            "api/open",
            "api/files",
            "render.html",
            "api/diagram",
            "../api/source",
            "http://example.com/",
        ] {
            assert!(!allowed_path(path));
        }
    }

    #[test]
    fn editing_revalidates_project_scope_and_rejects_retargeted_symlinks() {
        let root = std::env::temp_dir().join(format!("studio-edit-scope-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        let project = root.join("project");
        let shared = root.join("shared");
        std::fs::create_dir(&project).unwrap();
        std::fs::create_dir(&shared).unwrap();
        let inside = project.join("file.md");
        let outside = shared.join("file.md");
        std::fs::write(&inside, "# Project").unwrap();
        std::fs::write(&outside, "# Shared").unwrap();
        let project = project.canonicalize().unwrap();
        assert!(edit_scope(&inside.canonicalize().unwrap(), &project));
        assert!(!edit_scope(&outside.canonicalize().unwrap(), &project));
        #[cfg(unix)]
        {
            std::fs::remove_file(&inside).unwrap();
            std::os::unix::fs::symlink(&outside, &inside).unwrap();
            assert!(!edit_scope(&inside, &project));
            assert_eq!(std::fs::read_to_string(&outside).unwrap(), "# Shared");
        }
        std::fs::remove_dir_all(root).unwrap();
    }
}
