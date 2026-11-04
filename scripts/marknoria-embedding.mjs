function replace(source, before, after) {
  if (!source.includes(before) || source.indexOf(before) !== source.lastIndexOf(before)) throw Error(`MarkNoria embedding passage changed: ${before.slice(0, 100)}`)
  return source.replace(before, after)
}

export function embedSource(path, source) {
  if (path === 'src/lib.rs') {
    source = replace(source, "    let mut events = Parser::new_ext(&parse_source, options)\n        .into_offset_iter()\n        .peekable();\n", "    let parser = Parser::new_ext(&parse_source, options);\n    // Persist only label/record ranges in the small native metadata. Large\n    // destinations stay on disk and are read lazily by native worker requests.\n    let mut definitions: Vec<_> = parser.reference_definitions().iter().collect();\n    definitions.sort_by_key(|(_, definition)| definition.span.start);\n    let mut reference_data = BufWriter::new(File::create(directory.join(\"references.bin\"))?);\n    let mut reference_ranges = Vec::new();\n    let mut reference_offset = 0usize;\n    for (label, definition) in definitions {\n        let title = definition.title.as_deref().unwrap_or(\"\");\n        reference_ranges.push((label, reference_offset, definition.dest.len(), title.len()));\n        reference_data.write_all(definition.dest.as_bytes())?;\n        reference_data.write_all(title.as_bytes())?;\n        reference_offset += definition.dest.len() + title.len();\n    }\n    reference_data.flush()?;\n    let mut reference_metadata = BufWriter::new(File::create(directory.join(\"references.json\"))?);\n    serde_json::to_writer(&mut reference_metadata, &reference_ranges)?;\n    reference_metadata.flush()?;\n    writer.reference_bytes = reference_offset as u64;\n    drop(reference_ranges);\n    let mut events = parser.into_offset_iter().peekable();\n")
    source = replace(source, "    offset: u64,\n    diagrams:", "    offset: u64,\n    reference_bytes: u64,\n    diagrams:")
    source = replace(source, "        offset: 0,\n        diagrams:", "        offset: 0,\n        reference_bytes: 0,\n        diagrams:")
    source = replace(source, "if self.offset + rendered.len() as u64 > MAX_CACHE", "if self.offset + self.reference_bytes + rendered.len() as u64 > MAX_CACHE")
    source = replace(source, "    #[test]\n    fn syntax_highlighting_does_not_make_valid_code_exceed_fragment_budget()", "    #[test]\n    fn native_reference_records_keep_first_definitions_without_inflating_index() {\n        let text = \"[Key]: <first file.png> \\\"First title\\\"\\n[key]: wrong.png\\n\\n> [nested]: nested.png\\n>   \\\"Nested title\\\"\\n\\n[KEY] and ![image][nested].\\n\\n```text\\n[code]: unused.png\\n```\\n\";\n        let (directory,index) = fixture(text);\n        let records: Vec<(String,usize,usize,usize)> = serde_json::from_slice(&fs::read(directory.path().join(\"references.json\")).unwrap()).unwrap();\n        assert_eq!(records.len(),2);\n        let bytes=fs::read(directory.path().join(\"references.bin\")).unwrap();\n        let mut output=Vec::new();\n        for (label,offset,destination,title) in records {\n            output.push((label,String::from_utf8(bytes[offset..offset+destination].to_vec()).unwrap(),String::from_utf8(bytes[offset+destination..offset+destination+title].to_vec()).unwrap()));\n        }\n        assert_eq!(output,vec![(\"Key\".into(),\"first file.png\".into(),\"First title\".into()),(\"nested\".into(),\"nested.png\".into(),\"Nested title\".into())]);\n        let metadata=serde_json::to_string(&index).unwrap();\n        assert!(!metadata.contains(\"first file.png\") && !metadata.contains(\"Nested title\"));\n        assert_eq!(fs::read_to_string(directory.path().join(\"source.md\")).unwrap(),text);\n    }\n    #[test]\n    fn syntax_highlighting_does_not_make_valid_code_exceed_fragment_budget()")
    source = replace(source, '    let mut pending_span = 0..0;', '    let mut pending_span = 0..0;\n    let mut pending_image_bytes = 0usize;')
    source = replace(source, '                pending_span.start = span.start;', '                pending_span.start = span.start;\n                pending_image_bytes = 0;')
    source = replace(source, '        match &event {\n            Event::Start(_) => depth += 1,\n            Event::End(_) => depth = depth.saturating_sub(1),\n            _ => {}\n        }\n        pending_span.end = span.end;', `        if let Event::Start(Tag::Image { dest_url, .. }) = &event {
            if dest_url.len()>8192 && dest_url.get(..11).is_some_and(|s|s.eq_ignore_ascii_case("data:image/")) {
                // Images become bounded alt text in the cache. Charge their
                // source span separately; native transport projects these URLs.
                pending_image_bytes = pending_image_bytes.saturating_add(dest_url.len().min(span.end.saturating_sub(span.start)));
            }
        }
        match &event {
            Event::Start(_) => depth += 1,
            Event::End(_) => depth = depth.saturating_sub(1),
            _ => {}
        }
        pending_span.end = span.end;`)
    source = replace(source, '            || pending_span.end.saturating_sub(pending_span.start) > MAX_HTML', '            || (depth == 0 && pending_span.end.saturating_sub(pending_span.start).saturating_sub(pending_image_bytes) > MAX_HTML)')
    source = replace(source, "    #[test]\n    fn syntax_highlighting_does_not_make_valid_code_exceed_fragment_budget()", "    #[test]\n    fn oversized_data_images_keep_original_offsets_without_inflating_html() {\n        let uri = format!(\"data:image/png;base64,{}\", \"QUFB\".repeat(200_000));\n        let text = format!(\"# Start\\n\\n![big]({uri})\\n\\n# After\\n\");\n        let (directory, index) = fixture(&text);\n        assert_eq!(index.source_bytes, text.len());\n        assert!(index.pages.iter().any(|p| p.end - p.start > 512 * 1024));\n        let html = (0..index.pages.len())\n            .map(|p| read_page(directory.path(), &index, p).unwrap())\n            .collect::<String>();\n        assert!(html.len() < 1024 && html.contains(\"[image: big]\"));\n        assert_eq!(\n            fs::read_to_string(directory.path().join(\"source.md\")).unwrap(),\n            text\n        );\n        assert_eq!(index.toc[1].line, 5);\n        let path = directory.path().join(\"code.md\");\n        fs::write(&path, format!(\"```text\\n![literal]({uri})\\n```\\n\")).unwrap();\n        assert!(build_index(&path, &directory.path().join(\"code-cache\")).is_err());\n    }\n    #[test]\n    fn syntax_highlighting_does_not_make_valid_code_exceed_fragment_budget()")
  }
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
    source = replace(source, 'pub struct Service {', `pub struct ReaderSnapshot {
    pub document_id: String,
    pub source: std::path::PathBuf,
    pub revision: String,
    pub index: Arc<crate::Index>,
    _document: Arc<files::Document>,
}

pub fn native_temporary_directory(prefix: &str) -> std::io::Result<tempfile::TempDir> {
    files::temporary_directory(prefix)
}

pub struct Service {
    documents: Arc<files::Documents>,`)
    source = replace(source, 'impl Service {', `impl Service {
    pub fn native_snapshot(&self) -> std::io::Result<ReaderSnapshot> {
        let document = self.documents.peek().ok_or_else(|| std::io::Error::other("Document closed"))?;
        let index = {
            let progress = document.progress.lock().map_err(|_| std::io::Error::other("Document lock failed"))?;
            progress.index.as_ref().ok_or_else(|| std::io::Error::other("Wait for document indexing"))?.clone()
        };
        Ok(ReaderSnapshot { document_id: document.id.clone(), source: document.directory.join("source.md"), revision: index.revision.clone(), index, _document: document })
    }
`)
    source = replace(source, '        let (stop, stopped) = tokio::sync::oneshot::channel();', '        let service_documents = documents.clone();\n        let (stop, stopped) = tokio::sync::oneshot::channel();')
    source = replace(source, 'Ok(Self { address, token, stop: Some(stop) })', 'Ok(Self { address, token, stop: Some(stop), documents: service_documents })')
    const nativeRoute = '.route("/api/window", get(window))'
    if (source.split(nativeRoute).length !== 3) throw Error('MarkNoria reader route count changed')
    source = source.replaceAll(nativeRoute, `${nativeRoute}
        .route("/api/native-document", get(native_document))
        .route("/api/fragment-source", get(fragment_source))
        .route("/api/table-source", get(table_source))`)
    source = replace(source, 'async fn window(\n', `// Native clients reuse the index/source cache without mounting generated HTML.
async fn native_document(State(app): State<App>, headers: HeaderMap) -> Result<Json<Value>, ApiError> {
    let document = app.documents.select(&headers)?;
    let permit = app.work.clone().try_acquire_owned().map_err(|_| (StatusCode::TOO_MANY_REQUESTS, "Reader is busy".into()))?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let progress = document.progress.lock().map_err(failure)?;
        if let Some(error) = &progress.error { return Ok(Json(json!({"state":"error","error":error,"documentId":document.id,"editable":app.editable}))); }
        let Some(index) = &progress.index else { return Ok(Json(json!({"state":"indexing","documentId":document.id,"editable":app.editable}))); };
        Ok(Json(json!({"state":"ready","documentId":document.id,"index":index.as_ref(),"editable":app.editable})))
    }).await.map_err(failure)?
}

fn native_fragment(document: &files::Document, index: &Index, target: usize) -> Result<Value, ApiError> {
    let metadata = index.pages.get(target).ok_or((StatusCode::BAD_REQUEST,"Page out of range".into()))?;
    let source = crate::source::read_range(&document.directory,index,metadata.start,metadata.end).map_err(failure)?;
    let header = if let Some(table) = metadata.table.filter(|_| metadata.row_start > 0) {
        let first = &index.pages[index.tables[table].first_page];
        let original = crate::source::read_range(&document.directory,index,first.start,first.end).map_err(failure)?;
        let text = original.text.split_inclusive('\\n').take(2).collect::<String>();
        Some(json!({"start":original.start,"end":original.start+text.len(),"line":original.line,"text":text}))
    } else { None };
    Ok(json!({"documentId":document.id,"revision":index.revision,"page":target,"metadata":metadata,"source":source,"header":header}))
}

async fn fragment_source(State(app): State<App>, Query(query): Query<PageQuery>, headers: HeaderMap) -> Result<Json<Value>, ApiError> {
    let document = app.documents.select(&headers)?;
    let permit = app.work.clone().try_acquire_owned().map_err(|_| (StatusCode::TOO_MANY_REQUESTS, "Reader is busy".into()))?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let index = index(&document)?;
        native_fragment(&document,&index,query.target).map(Json)
    }).await.map_err(failure)?
}

async fn table_source(State(app): State<App>, Query(query): Query<TableQuery>, headers: HeaderMap) -> Result<Json<Value>, ApiError> {
    let document = app.documents.select(&headers)?;
    let permit = app.work.clone().try_acquire_owned().map_err(|_| (StatusCode::TOO_MANY_REQUESTS,"Reader is busy".into()))?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let index = index(&document)?;
        let table = index.tables.get(query.id).ok_or((StatusCode::BAD_REQUEST,"Table out of range".into()))?;
        let columns = match query.columns {
            Some(value) if value.len() <= 2048 => value.split(',').map(str::parse::<usize>).collect::<Result<Vec<_>,_>>().map_err(|_|(StatusCode::BAD_REQUEST,"Invalid columns".into()))?,
            Some(_) => return Err((StatusCode::BAD_REQUEST,"Column selection exceeds budget".into())),
            None => (0..table.columns.len()).collect(),
        };
        if columns.is_empty() || columns.len()>table.columns.len() || columns.windows(2).any(|pair|pair[0]>=pair[1])
            || columns.last().is_some_and(|column|*column>=table.columns.len()) || query.row>=table.rows.max(1) {
            return Err((StatusCode::BAD_REQUEST,"Invalid table row or columns".into()));
        }
        let byte_limit = crate::MAX_HTML.saturating_sub(table.header_bytes+1024)/table.max_row_bytes.max(1);
        let page_size = 50.min(512/columns.len()).min(byte_limit).max(1);
        let row_start = query.row/page_size*page_size;
        let row_end = (row_start+page_size).min(table.rows);
        let pages = &index.pages[table.first_page..table.end_page];
        let first = pages.partition_point(|page|page.row_start+page.rows<=row_start);
        let mut fragments = Vec::new();
        for (position,page) in pages.iter().enumerate().skip(first) {
            if page.row_start>=row_end {break;}
            fragments.push(native_fragment(&document,&index,table.first_page+position)?);
        }
        if fragments.is_empty() {
            fragments.push(native_fragment(&document,&index,table.first_page)?);
        }
        Ok(Json(json!({"documentId":document.id,"revision":index.revision,"table":query.id,"columns":columns,"rowStart":row_start,"rowEnd":row_end,"pageSize":page_size,"totalRows":table.rows,"fragments":fragments})))
    }).await.map_err(failure)?
}

async fn window(
`)
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
