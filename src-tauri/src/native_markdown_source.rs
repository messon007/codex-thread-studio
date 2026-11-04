//! Native-only compact source transport. The immutable original remains editable.
use super::*;
use axum::extract::Query;
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
};
use studio_markdown_native::SourceWindow;

#[path = "native_markdown_references.rs"]
mod references;

#[path = "native_markdown_table_text.rs"]
mod selection;
pub(super) use selection::{read as table_text, Budget as TableTextBudget};

type Error = (StatusCode, String);
fn bad(e: impl ToString) -> Error {
    (StatusCode::BAD_REQUEST, e.to_string())
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FragmentQuery {
    target: usize,
    revision: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TableQuery {
    id: usize,
    row: Option<usize>,
    columns: Option<String>,
    revision: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ImageQuery {
    page: usize,
    image: usize,
    revision: String,
    #[serde(default)]
    original: bool,
    #[serde(default)]
    animation: bool,
}
fn query<T: serde::de::DeserializeOwned>(request: &Request) -> Result<T, Error> {
    Query::<T>::try_from_uri(request.uri())
        .map(|q| q.0)
        .map_err(|_| bad("Invalid native source query"))
}
fn snapshot(
    state: &GatewayState,
    id: &str,
    request: &Request,
    revision: Option<&str>,
) -> Result<marknoria::server::ReaderSnapshot, Error> {
    let mut entries = state.markdown_readers.entries.lock().map_err(bad)?;
    let reader = entries.get_mut(id).ok_or((
        StatusCode::GONE,
        "Document closed or expired; reopen it".into(),
    ))?;
    reader.touched = Instant::now();
    let snapshot = reader
        .service
        .native_snapshot()
        .map_err(|e| (StatusCode::CONFLICT, e.to_string()))?;
    if revision.is_some_and(|r| r != snapshot.revision)
        || request
            .headers()
            .get("x-reader-document")
            .is_some_and(|h| h.as_bytes() != snapshot.document_id.as_bytes())
    {
        return Err((
            StatusCode::CONFLICT,
            "Document snapshot changed; reopen it".into(),
        ));
    }
    Ok(snapshot)
}
fn current(state: &GatewayState, id: &str, revision: &str, document: &str) -> Result<(), Error> {
    let entries = state.markdown_readers.entries.lock().map_err(bad)?;
    let reader = entries.get(id).ok_or((
        StatusCode::GONE,
        "Document closed during source read".into(),
    ))?;
    if !reader
        .service
        .native_snapshot()
        .is_ok_and(|s| s.revision == revision && s.document_id == document)
    {
        return Err((
            StatusCode::CONFLICT,
            "Document snapshot changed during source read".into(),
        ));
    }
    Ok(())
}
fn raw(
    snapshot: &marknoria::server::ReaderSnapshot,
    start: usize,
    end: usize,
    line: usize,
) -> Result<SourceWindow, String> {
    if start > end
        || end > snapshot.index.source_bytes
        || end - start > studio_markdown_native::source_projection::MAX_SOURCE
    {
        return Err("Invalid source window".into());
    }
    let mut file = File::open(&snapshot.source).map_err(|e| e.to_string())?;
    if file.metadata().map_err(|e| e.to_string())?.len() != snapshot.index.source_bytes as u64 {
        return Err("Document snapshot size changed".into());
    }
    file.seek(SeekFrom::Start(start as u64))
        .map_err(|e| e.to_string())?;
    let mut bytes = vec![0; end - start];
    file.read_exact(&mut bytes).map_err(|e| e.to_string())?;
    let text = String::from_utf8(bytes).map_err(|_| "Document source is not UTF-8")?;
    let end_line = line + text.match_indices('\n').count() - usize::from(text.ends_with('\n'));
    Ok(SourceWindow {
        start,
        end,
        line,
        end_line,
        text,
        projection: Vec::new(),
        references: Vec::new(),
    })
}
pub(super) fn window(
    snapshot: &marknoria::server::ReaderSnapshot,
    start: usize,
    end: usize,
    line: usize,
    page: usize,
) -> Result<SourceWindow, String> {
    let resolver = references::Resolver::open(snapshot)?;
    let lookup = |label: &str| resolver.lookup(label);
    let source = studio_markdown_native::source_projection::project_with_references(
        raw(snapshot, start, end, line)?,
        page,
        (!resolver.is_empty())
            .then_some(&lookup as &studio_markdown_native::references::Lookup<'_>),
    )?;
    resolver.check()?;
    if source.text.len() > 512 * 1024 {
        return Err("Native source exceeds the fragment formatting budget".into());
    }
    Ok(source)
}
pub(super) fn fragment(
    snapshot: &marknoria::server::ReaderSnapshot,
    target: usize,
    headers: &mut HashMap<usize, SourceWindow>,
) -> Result<serde_json::Value, String> {
    let page = snapshot
        .index
        .pages
        .get(target)
        .ok_or("Page out of range")?;
    let source = window(snapshot, page.start, page.end, page.line, target)?;
    let header = if let Some(table) = page.table.filter(|_| page.row_start > 0) {
        let first = snapshot.index.tables[table].first_page;
        if !headers.contains_key(&first) {
            let metadata = &snapshot.index.pages[first];
            let mut header = raw(snapshot, metadata.start, metadata.end, metadata.line)?;
            let end = header
                .text
                .split_inclusive('\n')
                .take(2)
                .map(str::len)
                .sum::<usize>();
            header.text.truncate(end);
            header.end = header.start + end;
            header.end_line = header.line + header.text.match_indices('\n').count()
                - usize::from(header.text.ends_with('\n'));
            let resolver = references::Resolver::open(snapshot)?;
            let lookup = |label: &str| resolver.lookup(label);
            let header = studio_markdown_native::source_projection::project_with_references(
                header,
                first,
                (!resolver.is_empty())
                    .then_some(&lookup as &studio_markdown_native::references::Lookup<'_>),
            )?;
            resolver.check()?;
            if header.text.len() > 128 * 1024 {
                return Err("Native table header exceeds its formatting budget".into());
            }
            headers.insert(first, header);
        }
        headers.get(&first).cloned()
    } else {
        None
    };
    Ok(
        json!({"documentId":snapshot.document_id,"revision":snapshot.revision,"page":target,"metadata":page,"source":source,"header":header}),
    )
}
fn table(
    snapshot: &marknoria::server::ReaderSnapshot,
    q: TableQuery,
) -> Result<serde_json::Value, String> {
    let table = snapshot
        .index
        .tables
        .get(q.id)
        .ok_or("Table out of range")?;
    let columns = match q.columns {
        Some(s) if s.len() <= 2048 => s
            .split(',')
            .map(str::parse::<usize>)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| "Invalid columns")?,
        Some(_) => return Err("Column selection exceeds budget".into()),
        None => (0..table.columns.len().min(512)).collect(),
    };
    if columns.is_empty()
        || columns.len() > 512
        || columns.iter().any(|c| *c >= table.columns.len())
        || columns.windows(2).any(|p| p[0] >= p[1])
        || q.row.unwrap_or(0) >= table.rows.max(1)
    {
        return Err("Invalid columns".into());
    }
    let byte_limit =
        marknoria::MAX_HTML.saturating_sub(table.header_bytes + 1024) / table.max_row_bytes.max(1);
    let page_size = 50.min(512 / columns.len()).min(byte_limit).max(1);
    // `row` is a source-row cursor. Column changes can change page_size; rounding
    // that cursor down would move the reader to an unrelated earlier context.
    let start = q.row.unwrap_or(0);
    let end = (start + page_size).min(table.rows);
    let mut fragments = Vec::new();
    let mut headers = HashMap::new();
    for (id, p) in snapshot
        .index
        .pages
        .iter()
        .enumerate()
        .skip(table.first_page)
        .take(table.end_page - table.first_page)
    {
        if p.rows > 0 && p.row_start < end && p.row_start + p.rows > start
            || p.rows == 0 && table.rows == 0
        {
            if fragments.len() >= 50 {
                return Err("Table page exceeds its fragment budget".into());
            }
            fragments.push(fragment(snapshot, id, &mut headers)?);
        }
    }
    Ok(
        json!({"columns":columns,"rowStart":start,"rowEnd":end,"pageSize":page_size,"documentId":snapshot.document_id,"revision":snapshot.revision,"table":q.id,"totalRows":table.rows,"fragments":fragments}),
    )
}
pub(super) async fn handle(
    state: GatewayState,
    id: String,
    operation: &str,
    request: Request,
) -> Response<Body> {
    let result = handle_inner(&state, &id, operation, request).await;
    match result {
        Ok(response) => response,
        Err((status, error)) => json_error(status, &error),
    }
}
async fn handle_inner(
    state: &GatewayState,
    id: &str,
    operation: &str,
    request: Request,
) -> Result<Response<Body>, Error> {
    if request.method() != Method::GET {
        return Err((
            StatusCode::METHOD_NOT_ALLOWED,
            "Native source method rejected".into(),
        ));
    }
    if operation == "image" {
        let q: ImageQuery = query(&request)?;
        if q.revision.len() != 64
            || !q.revision.bytes().all(|b| b.is_ascii_hexdigit())
            || q.image >= studio_markdown_native::source_projection::MAX_IMAGES
        {
            return Err(bad("Invalid document image reference"));
        }
        let permit = state.images.admit()?;
        let snapshot = snapshot(state, id, &request, Some(&q.revision))?;
        let revision = snapshot.revision.clone();
        let document = snapshot.document_id.clone();
        let images = state.images.clone();
        let response = tokio::task::spawn_blocking(move || {
            let _permit = permit;
            let p = snapshot
                .index
                .pages
                .get(q.page)
                .ok_or_else(|| bad("Image page out of range"))?;
            let source = raw(&snapshot, p.start, p.end, p.line).map_err(bad)?;
            let resolver = references::Resolver::open(&snapshot).map_err(bad)?;
            let lookup = |label: &str| resolver.lookup(label);
            let image = studio_markdown_native::source_projection::image_with_references(
                &source.text,
                q.image,
                (!resolver.is_empty())
                    .then_some(&lookup as &studio_markdown_native::references::Lookup<'_>),
            )
            .map_err(bad)?;
            resolver.check().map_err(bad)?;
            drop(source);
            images.document_response(image.bytes, q.original, q.animation)
        })
        .await
        .map_err(bad)??;
        current(state, id, &revision, &document)?;
        return Ok(response);
    }
    let lane = if matches!(operation, "table-text" | "table-prefix") {
        &state.markdown_readers.selecting
    } else {
        &state.markdown_readers.projecting
    };
    let permit = lane.clone().try_acquire_owned().map_err(|_| {
        (
            StatusCode::TOO_MANY_REQUESTS,
            "Native source worker is busy".into(),
        )
    })?;
    if operation == "table-prefix" {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct PrefixQuery {
            id: usize,
            characters: usize,
            revision: String,
        }
        let q: PrefixQuery = query(&request)?;
        if q.characters > studio_markdown_native::flow::BUFFER_BYTES / 4
            || q.revision.len() != 64
            || !q.revision.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err(bad("Invalid table prefix request"));
        }
        let snapshot = snapshot(state, id, &request, Some(&q.revision))?;
        let revision = snapshot.revision.clone();
        let document = snapshot.document_id.clone();
        let value=tokio::task::spawn_blocking(move|| {
            let _permit=permit;
            let mut budget=TableTextBudget::default();
            let prefix=selection::prefix(&snapshot,q.id,q.characters,&mut budget)?;
            Ok::<_,String>(json!({"documentId":snapshot.document_id,"revision":snapshot.revision,"prefix":prefix,"work":{"sourcePages":budget.pages,"sourceBytes":budget.bytes}}))
        }).await.map_err(bad)?.map_err(bad)?;
        current(state, id, &revision, &document)?;
        return Ok(json_response(StatusCode::OK, &value));
    }
    if operation == "table-text" {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct SelectionQuery {
            ids: String,
            limit: usize,
            revision: String,
        }
        let q: SelectionQuery = query(&request)?;
        if q.ids.len() > 512
            || q.limit > studio_markdown_native::flow::BUFFER_BYTES
            || q.revision.len() != 64
            || !q.revision.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err(bad("Invalid table selection request"));
        }
        let ids = q
            .ids
            .split(',')
            .map(str::parse::<usize>)
            .collect::<Result<Vec<_>, _>>()
            .map_err(bad)?;
        if ids.is_empty()
            || ids.len() > 32
            || ids.iter().enumerate().any(|(i, id)| ids[..i].contains(id))
        {
            return Err(bad("Invalid table selection identities"));
        }
        let snapshot = snapshot(state, id, &request, Some(&q.revision))?;
        let revision = snapshot.revision.clone();
        let document = snapshot.document_id.clone();
        let value = tokio::task::spawn_blocking(move || {
            let _permit = permit;
            let mut budget = TableTextBudget::default();
            let mut remaining = q.limit;
            let mut tables = Vec::new();
            for id in ids {
                let table = table_text(&snapshot, id, remaining, &mut budget)?
                    .ok_or("Table selection exceeds its byte budget")?;
                remaining -= table.text.len();
                tables.push(table);
            }
            Ok::<_, String>(json!({
                "documentId": snapshot.document_id,
                "revision": snapshot.revision,
                "tables": tables
            }))
        })
        .await
        .map_err(bad)?
        .map_err(bad)?;
        current(state, id, &revision, &document)?;
        return Ok(json_response(StatusCode::OK, &value));
    }
    let (target, table, revision) = if operation == "fragment" {
        let q: FragmentQuery = query(&request)?;
        (Some(q.target), None, q.revision)
    } else {
        let q: TableQuery = query(&request)?;
        let revision = q.revision.clone();
        (None, Some(q), revision)
    };
    let snapshot = snapshot(state, id, &request, revision.as_deref())?;
    let revision = snapshot.revision.clone();
    let document = snapshot.document_id.clone();
    let value = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        if let Some(target) = target {
            fragment(&snapshot, target, &mut HashMap::new())
        } else {
            self::table(&snapshot, table.unwrap())
        }
    })
    .await
    .map_err(bad)?
    .map_err(bad)?;
    current(state, id, &revision, &document)?;
    Ok(json_response(StatusCode::OK, &value))
}
