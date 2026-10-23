function replace(source, before, after) {
  if (!source.includes(before) || source.indexOf(before) !== source.lastIndexOf(before)) throw Error(`MarkNoria embedding passage changed: ${before.slice(0, 100)}`)
  return source.replace(before, after)
}

export function embedSource(path, source) {
  if (path === 'src/files.rs') {
    const cacheStart = source.indexOf('    fn cache_storage_prefers_explicit_paths_and_defaults_outside_tmp()')
    const cacheEnd = source.indexOf('    #[test]', cacheStart)
    if (cacheStart < 0 || cacheEnd < 0) throw Error('MarkNoria cache fixture changed')
    let cacheFixture = source.slice(cacheStart, cacheEnd)
    cacheFixture = replace(cacheFixture, 'let path = |value: &str| Some(PathBuf::from(value));', `let rooted = |value: &str| if cfg!(windows) { PathBuf::from(format!("C:{value}")) } else { PathBuf::from(value) };
        let path = |value: &str| Some(if value.starts_with('/') { rooted(value) } else { PathBuf::from(value) });`)
    cacheFixture = cacheFixture.replaceAll('PathBuf::from("/', 'rooted("/')
    source = source.slice(0, cacheStart) + cacheFixture + source.slice(cacheEnd)
    const start = source.indexOf('    fn names_and_server_paths_are_confined()')
    const end = source.indexOf('    #[test]', start)
    if (start < 0 || end < 0) throw Error('MarkNoria path fixture changed')
    let fixture = source.slice(start, end)
    fixture = replace(fixture, 'let root = tempfile::tempdir().unwrap();', 'let directory = tempfile::tempdir().unwrap();\n        let root = directory.path().canonicalize().unwrap();')
    fixture = fixture.replaceAll('root.path()', 'root.as_path()')
    source = source.slice(0, start) + fixture + source.slice(end)
  }
  if (path === 'src/edit.rs') {
    source = replace(source, '            path,\n            &Save {', '            &path.canonicalize()?,\n            &Save {')
    source = replace(source, 'save(&current, &path, &request, failed)', 'save(&current, &path.canonicalize().unwrap(), &request, failed)')
    source = replace(source, '    temporary.persist(path).map_err(|error| error.error)?;', `    #[cfg(windows)]
    {
        drop(original_identity);
        drop(live_identity);
        drop(original);
    }
    temporary.persist(path).map_err(|error| io::Error::new(error.error.kind(), format!("Replacing the saved file failed: {}", error.error)))?;`)
    source = replace(source, '        .set_permissions(metadata.permissions())?;', '        .set_permissions(metadata.permissions()).map_err(|error| io::Error::new(error.kind(), format!("Preserving file permissions failed: {error}")))?;')
  }
  if (path === 'src/server.rs') {
    source = replace(source, '    pub async fn start(source: std::path::PathBuf) -> std::io::Result<Self> {', `    pub async fn start(source: std::path::PathBuf) -> std::io::Result<Self> {
        Self::start_with_editing(source, false).await
    }

    pub async fn start_editable(source: std::path::PathBuf) -> std::io::Result<Self> {
        Self::start_with_editing(source, true).await
    }

    async fn start_with_editing(source: std::path::PathBuf, editable: bool) -> std::io::Result<Self> {`)
    source = replace(source, '            editable: false,', '            editable,')
    source = replace(source, `            .route("/api/renderer", get(renderer_status))
            .layer(middleware::from_fn_with_state(app.clone(), guard)).with_state(app);`, `            .route("/api/renderer", get(renderer_status));
        let router = if editable { router.route("/api/edit", get(edit_runtime::page).post(edit_runtime::save)) } else { router };
        let router = router.layer(middleware::from_fn_with_state(app.clone(), guard)).with_state(app);`)
  }
  if (path === 'web/reader.mjs') {
    source = replace(source, 'if (!state.visible) notice.hidden = !state.dirty', 'notice.hidden = state.visible || !state.dirty')
    source = replace(source, 'if (!response.editable || !response.documentId || embedded) return', 'if (!response.editable || !response.documentId || comparisonPreview) return')
    source = replace(source, "try {\n  if (!token)", `window.marknoriaHost = {
  get state() {
    return {
      ready: Boolean(index || editor), readingReady: Boolean(index || editor?.needsRefresh), editable,
      view: editor?.visible ? 'edit' : source?.visible ? 'source' : 'preview',
      dirty: Boolean(editor?.dirty), busy: Boolean(editor?.busy || switching || contentsLoading),
      outlineOpen: document.getElementById('contents-toggle').getAttribute('aria-expanded') === 'true',
      hasOutline: Boolean(index?.toc.length), lineCount: editor?.data?.sourceLines ?? index?.sourceLines,
      size: editor?.data?.sourceBytes ?? index?.sourceBytes, documentId,
      status: editor?.visible || editor?.dirty ? document.getElementById('edit-status').textContent : '',
      error: document.getElementById('welcome-title').textContent === 'Could not open this document' ? status.textContent : '',
    }
  },
  async setView(view) {
    if (editable) return switchView(view)
    if (view === 'source') return source?.show()
    if (view === 'preview') document.getElementById('preview-mode').click()
  },
  save: () => editor?.save() ?? false,
  leave: () => editor?.leaving(),
  async navigate(match) {
    if (editor?.visible) {
      if (editor.needsRefresh) await refreshReading()
      return editor.go(match.line, index.revision)
    }
    if (source?.visible) return source.go(match.line)
    return jump(match.page, null, match.tableRow)
  },
}

try {
  if (!token)`)
  }
  return source
}
