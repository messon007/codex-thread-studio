use super::*;

pub(super) async fn description(State(app): State<App>, headers: HeaderMap) -> Result<Json<Value>, ApiError> {
    let document = app.documents.select(&headers)?;
    let index = index(&document)?;
    if index.renderer != Some(crate::core::DocumentRenderer::Epubjs) { return Err((StatusCode::BAD_REQUEST, "The active document is not a reflowable EPUB".into())); }
    let package = index.epub_package.as_deref().ok_or((StatusCode::CONFLICT, "EPUB package unavailable".into()))?;
    let scope = format!("./epub-resource/{}/{}/", document.id, document.content_token);
    Ok(Json(json!({"revision":index.revision,"scope":scope,"package":package,"cfiDomProfile":"studio-epub-v1","budgets":{"resourceBytes":10*1024*1024,"chapterBytes":2*1024*1024,"chapterNodes":10000,"chapterDepth":64,"expandedBytes":200*1024*1024}})))
}

fn decoded_path(value: &str) -> Option<String> {
    let mut bytes = Vec::with_capacity(value.len());
    let mut input = value.bytes();
    while let Some(byte) = input.next() {
        bytes.push(if byte == b'%' {
            let a = (input.next()? as char).to_digit(16)?;
            let b = (input.next()? as char).to_digit(16)?;
            (a * 16 + b) as u8
        } else { byte });
    }
    String::from_utf8(bytes).ok()
}

/// An independent, revocable, read-only capability for members of this one
/// immutable archive. It conveys no reader API token, write or folder authority.
pub(super) async fn resource(State(app): State<App>, uri: axum::http::Uri) -> Result<Response, ApiError> {
    let path = uri.path().strip_prefix("/epub-resource/").ok_or((StatusCode::NOT_FOUND, "Unknown EPUB resource".into()))?;
    let mut parts = path.splitn(3, '/');
    let (id, capability, path) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""), parts.next().unwrap_or(""));
    let document = app.documents.peek().ok_or((StatusCode::GONE, "EPUB closed".into()))?;
    if id != document.id || capability != document.content_token || document.cancelled.load(Ordering::Relaxed) { return Err((StatusCode::FORBIDDEN, "EPUB resource was not granted".into())); }
    let index = index(&document)?;
    if index.renderer != Some(crate::core::DocumentRenderer::Epubjs) { return Err((StatusCode::FORBIDDEN, "EPUB resource was not granted".into())); }
    let path = decoded_path(path).ok_or((StatusCode::BAD_REQUEST, "Invalid UTF-8 resource name".into()))?;
    let permit = app.work.clone().try_acquire_owned().map_err(|_| (StatusCode::TOO_MANY_REQUESTS, "Reader is busy".into()))?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let (bytes, mime) = crate::core::documents::epub_resource(&document.directory, &index, &path).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
        Ok(([(header::CONTENT_TYPE, mime), (header::X_CONTENT_TYPE_OPTIONS, "nosniff")], bytes).into_response())
    }).await.map_err(failure)?
}
