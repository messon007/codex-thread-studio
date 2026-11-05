//! One-document HTTP adapter. Startup grants are supplied only by trusted hosts.
use super::*;
use crate::core::reader::FileGrant;
use std::{future::IntoFuture, io, path::PathBuf};

#[derive(Clone, Copy, Debug, Default)]
pub enum DiagramMode {
    #[default]
    Backend,
    Browser,
}

#[derive(Clone, Debug)]
pub struct ServiceOptions {
    projection: crate::core::ProjectionProfile,
    queries: bool,
    notes_directory: Option<PathBuf>,
    parent_origins: Vec<String>,
    diagrams: DiagramMode,
}
impl Default for ServiceOptions {
    fn default() -> Self {
        Self {
            projection: crate::core::ProjectionProfile::Web,
            queries: true,
            notes_directory: None,
            parent_origins: Vec::new(),
            diagrams: DiagramMode::Backend,
        }
    }
}
impl ServiceOptions {
    pub fn projection(mut self, profile: crate::core::ProjectionProfile) -> Self {
        self.projection = profile;
        self
    }
    pub fn queries(mut self, enabled: bool) -> Self {
        self.queries = enabled;
        self
    }
    pub fn notes_directory(mut self, path: PathBuf) -> Self {
        self.notes_directory = Some(path);
        self
    }
    pub fn diagrams(mut self, mode: DiagramMode) -> Self {
        self.diagrams = mode;
        self
    }
    /// Exact HTTP(S) origins, without credentials, path, query or fragment.
    /// This permits framing only; API requests still require this service's token.
    pub fn parent_origin(mut self, origin: &str) -> io::Result<Self> {
        if self.parent_origins.len() >= 8 || !valid_origin(origin) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Expected an exact HTTP(S) parent origin",
            ));
        }
        if !self.parent_origins.iter().any(|value| value == origin) {
            self.parent_origins.push(origin.into());
        }
        Ok(self)
    }
}
fn valid_origin(origin: &str) -> bool {
    if origin.len() > 2048 || origin.contains(['@', '#', '\\']) {
        return false;
    }
    let Ok(uri) = origin.parse::<axum::http::Uri>() else {
        return false;
    };
    let Some(authority) = uri.authority() else {
        return false;
    };
    let host = authority.host();
    if host.is_empty()
        || !host
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b".-_:[]".contains(&b))
    {
        return false;
    }
    matches!(uri.scheme_str(), Some("http" | "https"))
        && uri.authority().is_some()
        && uri.query().is_none()
        && origin
            == format!(
                "{}://{}",
                uri.scheme_str().unwrap(),
                uri.authority().unwrap()
            )
}

pub struct Service {
    pub address: std::net::SocketAddr,
    pub token: String,
    stop: Option<tokio::sync::oneshot::Sender<()>>,
    task: Option<tokio::task::JoinHandle<io::Result<()>>>,
    browser_diagrams: bool,
}
impl Drop for Service {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
    }
}
impl Service {
    /// Compatibility entry: one read-only file, queries allowed, notes disabled.
    pub async fn start(source: PathBuf) -> io::Result<Self> {
        Self::start_with_options(FileGrant::read_only(source)?, ServiceOptions::default()).await
    }
    pub async fn start_with_options(grant: FileGrant, options: ServiceOptions) -> io::Result<Self> {
        let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).await?;
        let address = listener.local_addr()?;
        let documents = Arc::new(files::Documents::new(
            grant.path().parent().unwrap().to_owned(),
        ).with_projection(options.projection));
        documents.open(
            grant.path().to_owned(),
            crate::runtime::cache::cache_directory()?,
        )?;
        let token = uuid::Uuid::new_v4().to_string();
        let browser_diagrams = matches!(options.diagrams, DiagramMode::Browser);
        let editor = Arc::new(edit_runtime::Editor::default());
        let app = App {
            editable: grant.writable(),
            queries: options.queries,
            notes_directory: options.notes_directory,
            embedded: true,
            parent_origins: Arc::new(options.parent_origins),
            browser_diagrams,
            editor: editor.clone(),
            comparisons: Arc::new(compare_runtime::Comparisons::new()),
            comparison_documents: Arc::new(files::Documents::new(documents.root.clone())),
            documents: documents.clone(),
            token: token.clone(),
            host: address.to_string(),
            work: Arc::new(Semaphore::new(2)),
            open_work: Arc::new(Semaphore::new(1)),
            diagram_work: Arc::new(Semaphore::new(1)),
            diagram_waiters: Arc::new(Semaphore::new(4)),
            cancelled: Arc::new(AtomicBool::new(false)),
        };
        // No browse/open/compare/pane routes. A supplied file grants no directory API.
        let closing = app.clone();
        let router = reader_routes()
            .route(
                "/api/edit",
                get(edit_runtime::page).post(edit_runtime::save),
            )
            .layer(middleware::from_fn_with_state(app.clone(), guard))
            .with_state(app);
        let (stop, mut stopped) = tokio::sync::oneshot::channel();
        let task = tokio::spawn(async move {
            let maintenance_documents = documents.clone();
            let maintenance = tokio::spawn(async move {
                let mut interval = tokio::time::interval(Duration::from_millis(500));
                loop {
                    interval.tick().await;
                    if let Some(document) = maintenance_documents.peek() {
                        document.expire_operations();
                        if let Ok(mut renderer) = document.renderer.try_lock() {
                            renderer.expire();
                        }
                    }
                }
            });
            // Dropping serve stops accepting and drops outstanding HTTP futures.
            // Document cancellation also interrupts supervised index/render/query work.
            tokio::select! {
                _ = &mut stopped => (),
                _ = axum::serve(listener, router).into_future() => (),
            }
            closing.cancelled.store(true, Ordering::Relaxed);
            documents.cancel();
            maintenance.abort();
            let _ = maintenance.await;
            // Drain accepted work before reporting cache cleanup complete.
            let _work = closing.work.acquire_many(2).await;
            let _open = closing.open_work.acquire().await;
            let _diagrams = closing.diagram_work.acquire().await;
            tokio::task::spawn_blocking(move || {
                editor.clear();
                documents.shutdown_checked()
            })
            .await
            .map_err(io::Error::other)?
        });
        Ok(Self {
            address,
            token,
            stop: Some(stop),
            task: Some(task),
            browser_diagrams,
        })
    }
    pub fn url(&self) -> String {
        format!(
            "http://{}/?embedded=1{}#{}",
            self.address,
            if self.browser_diagrams {
                "&diagrams=browser"
            } else {
                ""
            },
            self.token
        )
    }
    /// Await owned service/index/cache cleanup before destroying the host runtime.
    pub async fn close(mut self) -> io::Result<()> {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(task) = self.task.take() {
            task.await.map_err(io::Error::other)??;
        }
        Ok(())
    }
}

pub(super) fn query_authority(app: &App) -> Result<(), ApiError> {
    if app.queries {
        Ok(())
    } else {
        Err((
            StatusCode::FORBIDDEN,
            "Queries were disabled by the host".into(),
        ))
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct OperationRequest {
    action: String,
    revision: String,
    id: Option<String>,
}
pub(super) async fn operation(
    State(app): State<App>,
    headers: HeaderMap,
    Json(request): Json<OperationRequest>,
) -> Result<Json<Value>, ApiError> {
    query_authority(&app)?;
    let document = app.documents.select(&headers)?;
    if index(&document)?.revision != request.revision {
        return Err((StatusCode::CONFLICT, "Document revision changed".into()));
    }
    match request.action.as_str() {
        "begin" => {
            document.expire_operations();
            let operation = document
                .operations
                .begin()
                .map_err(|e| (StatusCode::TOO_MANY_REQUESTS, e.to_string()))?;
            let id = operation.id();
            document
                .reserved_operations
                .lock()
                .map_err(failure)?
                .insert(id.clone(), (std::time::Instant::now(), operation));
            Ok(Json(json!({"id":id,"accepted":true})))
        }
        "cancel" => {
            let id = request
                .id
                .ok_or((StatusCode::BAD_REQUEST, "Operation ID required".into()))?;
            let cancelled = document
                .operations
                .cancel(&id)
                .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
            document
                .reserved_operations
                .lock()
                .map_err(failure)?
                .remove(&id);
            Ok(Json(json!({"id":id,"cancelled":cancelled})))
        }
        _ => Err((StatusCode::BAD_REQUEST, "Unknown operation action".into())),
    }
}
pub(super) async fn capabilities(State(app): State<App>) -> Json<Value> {
    Json(json!({"apiVersion":crate::core::reader::API_VERSION,
        "bridgeVersion":crate::core::reader::BRIDGE_VERSION,
        "embedded":app.embedded,"edit":app.editable,"queries":app.queries,"charts":app.queries,
        "chartProfile":{"datasetScope":"complete-input","valuePrecision":"float64","exactValueText":true,"maxPoints":crate::core::table_query::MAX_CHART_POINTS,"maxBytes":crate::core::MAX_HTML},
        "notes":app.notes_directory.is_some(),"queryNotes":app.notes_directory.is_some() && app.queries,"parentOrigins":app.parent_origins.as_ref(),
        "diagrams":if app.browser_diagrams {"browser"} else {"backend"},
        "datasetScope":"complete-input"}))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn framing_requires_exact_http_origins() {
        for good in [
            "http://127.0.0.1:1234",
            "https://example.com",
            "http://[::1]:8080",
        ] {
            assert!(valid_origin(good), "{good}");
        }
        for bad in [
            "*",
            "null",
            "file://test",
            "https://example.com/",
            "http://u:p@host",
            "https://host/a",
            "https://host?x",
            "https://host#x",
            "https://host\n",
            "https://*.example.com",
            "https://host;script-src",
            "https://%2a.example.com",
            "https://host'",
        ] {
            assert!(!valid_origin(bad), "{bad}");
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct SourceSelection {
    revision: String,
    start: usize,
    end: usize,
    from_utf16: usize,
    to_utf16: usize,
}
pub(super) async fn selection(
    State(app): State<App>,
    headers: HeaderMap,
    Json(request): Json<SourceSelection>,
) -> Result<Json<crate::core::reader::Selection>, ApiError> {
    let document = app.documents.select(&headers)?;
    let index = index(&document)?;
    let _permit = app
        .work
        .clone()
        .try_acquire_owned()
        .map_err(|_| (StatusCode::TOO_MANY_REQUESTS, "Reader is busy".into()))?;
    tokio::task::spawn_blocking(move || {
        let _permit = _permit;
        crate::core::reader::selection_from_source(
            &document.directory,
            &index,
            &document.id,
            &request.revision,
            request.start,
            request.end,
            request.from_utf16,
            request.to_utf16,
        )
        .map(Json)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))
    })
    .await
    .map_err(failure)?
}
