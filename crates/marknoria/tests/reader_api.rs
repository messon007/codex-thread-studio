//! Consumers use only the stable reader facade, not controller/cache internals.
use marknoria::reader::View;
use marknoria::reader::{
    Change, FileGrant, Location, NoteAnchor, NoteRequest, Options, Precision, Reader, SaveRequest,
};
use std::{fs, io};

fn root() -> tempfile::TempDir {
    marknoria::runtime::cache::temporary_directory("mn-public-reader-").unwrap()
}
fn options(root: &tempfile::TempDir) -> Options {
    Options::default().cache_root(root.path().join("cache"))
}

#[test]
fn web_pdf_snapshot_preserves_original_locations_without_native_tools() {
    use marknoria::reader::{DocumentRenderer, PageRect, PdfRegion, ProjectionProfile};
    let root = root();
    let bytes = b"%PDF-1.4\n% Page discovery is deferred to the renderer.\n";
    let reader = Reader::from_bytes("public.pdf", bytes, options(&root).projection(ProjectionProfile::Web).notes_directory(root.path().join("notes"))).unwrap();
    let descriptor = reader.descriptor();
    assert_eq!(descriptor.renderer, Some(DocumentRenderer::Pdfjs));
    assert!(!descriptor.capabilities.source && !descriptor.capabilities.edit);
    assert_eq!(reader.original_range(&descriptor.revision, 0, bytes.len()).unwrap(), bytes);
    assert_eq!(reader.search(&descriptor.revision, "page").err().unwrap().kind(), io::ErrorKind::Unsupported);
    let location = Location::Pdf { pages: vec![PdfRegion { page: 2, rects: vec![PageRect { x: 0.1, y: 0.2, width: 0.3, height: 0.4 }] }] };
    let position = reader.reading_position(View::Preview, location.clone()).unwrap();
    assert!(reader.restore_position(&position, false).unwrap().requires_renderer);
    let note = reader.notes(NoteRequest::Add { kind: "comment".into(), quote: String::new(), comment: "Selected region".into(), revision: descriptor.revision.clone(), anchor: NoteAnchor { location: Some(location.clone()), ..Default::default() } }).unwrap();
    let notes = reader.notes(NoteRequest::List { offset: 0, kind: None }).unwrap();
    assert_eq!(notes["items"][0]["anchor"]["location"], serde_json::to_value(&location).unwrap());
    let reference = reader.notes(NoteRequest::Copy { id: note["id"].as_str().unwrap().into() }).unwrap();
    assert!(reference["text"].as_str().unwrap().contains("\"rects\""));
    reader.close().unwrap();
    assert_eq!(fs::read_dir(root.path().join("cache")).unwrap().count(), 0);
    assert!(Reader::from_bytes("invalid.pdf", b"not a pdf", options(&root).projection(ProjectionProfile::Web)).is_err());
}

#[test]
fn web_epub_exposes_only_bounded_original_members_and_preserves_cfi() {
    use marknoria::reader::{DocumentRenderer, ProjectionProfile};
    use std::io::Write;
    let root = root();
    let mut archive = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let files = [
        ("META-INF/container.xml", "<container><rootfiles><rootfile full-path=\"OPS/book.opf\"/></rootfiles></container>"),
        ("OPS/book.opf", "<package><manifest><item id=\"a\" href=\"é😀.xhtml\"/></manifest><spine><itemref idref=\"a\"/></spine></package>"),
        ("OPS/é😀.xhtml", "<!DOCTYPE html><html><head><title>Public</title></head><body><p>Public 中文😀.</p></body></html>"),
    ];
    for (name, text) in files { archive.start_file(name, zip::write::SimpleFileOptions::default()).unwrap(); archive.write_all(text.as_bytes()).unwrap(); }
    archive.start_file("OPS/too-large.xhtml", zip::write::SimpleFileOptions::default()).unwrap(); archive.write_all(&vec![b'x'; 2 * 1024 * 1024 + 1]).unwrap();
    let bytes = archive.finish().unwrap().into_inner();
    let reader = Reader::from_bytes("public.epub", &bytes, options(&root).projection(ProjectionProfile::Web)).unwrap();
    let descriptor = reader.descriptor();
    assert_eq!(descriptor.renderer, Some(DocumentRenderer::Epubjs));
    assert_eq!(descriptor.epub_package.as_deref(), Some("OPS/book.opf"));
    assert!(!descriptor.capabilities.source && !descriptor.capabilities.edit);
    let (chapter, mime) = reader.epub_resource(&descriptor.revision, "OPS/é😀.xhtml").unwrap();
    assert_eq!(mime, "application/xhtml+xml"); assert!(String::from_utf8(chapter).unwrap().contains("中文😀"));
    for name in ["../outside", "/etc/passwd", "OPS/too-large.xhtml", "OPS/missing.xhtml"] { assert!(reader.epub_resource(&descriptor.revision, name).is_err(), "{name}"); }
    assert!(reader.epub_resource("stale", "OPS/book.opf").is_err());
    let location = Location::Epub { cfi: "epubcfi(/6/2[a]!/4/2/1:3)".into(), href: Some("OPS/é😀.xhtml".into()) };
    let saved = reader.reading_position(View::Preview, location.clone()).unwrap();
    let restored = reader.restore_position(&saved, false).unwrap(); assert!(restored.requires_renderer); assert_eq!(restored.position.location, location);
    reader.close().unwrap(); assert_eq!(fs::read_dir(root.path().join("cache")).unwrap().count(), 0);
}

#[test]
fn headless_host_opens_projects_queries_and_disposes_complete_inputs() {
    let root = root();
    let columns = vec!["名字".into(), "value".into()];
    let rows: Vec<_> = (0..2501)
        .map(|n| vec![format!("row {n} 中文😀"), n.to_string()])
        .collect();
    let mut reader = Reader::from_table(&columns, &rows, options(&root)).unwrap();
    let descriptor = reader.descriptor();
    assert_eq!(descriptor.capabilities.dataset_scope, "complete-input");
    assert!(!descriptor.capabilities.edit);
    assert!(descriptor.capabilities.sql);
    assert!(
        reader.window(&descriptor.revision, 0, 1).unwrap().pages[0]
            .html
            .contains("中文")
    );
    let operations = reader.operations();
    let query = operations.begin().unwrap();
    let result = reader
        .sql(
            &descriptor.revision,
            0,
            "SELECT COUNT(*),SUM(CAST(c2 AS INTEGER)) FROM t",
            &query,
        )
        .unwrap();
    assert_eq!(
        result.rows[0],
        vec![Some("2501".into()), Some("3126250".into())]
    );
    assert!(
        reader
            .sql(
                &descriptor.revision,
                0,
                "ATTACH DATABASE ':memory:' AS stolen",
                &query
            )
            .is_err()
    );
    assert!(reader.sql("stale", 0, "SELECT * FROM t", &query).is_err());
    assert!(reader.window(&descriptor.revision, 0, 6).is_err());
    assert!(
        reader
            .begin_edit(&descriptor.revision, None, None, false)
            .is_err()
    );
    query.cancel();
    assert_eq!(
        reader
            .sql(&descriptor.revision, 0, "SELECT * FROM t", &query)
            .err()
            .unwrap()
            .kind(),
        io::ErrorKind::Interrupted
    );
    reader.close().unwrap();
    assert!(query.cancelled());
    assert!(operations.begin().is_err());
    assert_eq!(fs::read_dir(root.path().join("cache")).unwrap().count(), 0);
}

#[test]
fn exact_source_selections_keep_utf8_utf16_bom_and_crlf_distinct() {
    let root = root();
    let original = "\u{feff}A😀B\r\n中文\r\n";
    let reader = Reader::from_bytes("source.txt", original.as_bytes(), options(&root)).unwrap();
    let metadata = reader.descriptor();
    let selection = reader
        .source_selection(&metadata.revision, 0, original.len(), 2, 7)
        .unwrap();
    assert_eq!(selection.quote, "😀B\n中");
    assert_eq!(
        selection.location,
        Location::Source {
            start_byte: 4,
            end_byte: 14
        }
    );
    assert_eq!(selection.precision, Precision::Exact);
    assert!(
        reader
            .source_selection(&metadata.revision, 0, original.len(), 3, 7)
            .is_err(),
        "Surrogate splitting must fail"
    );
    let other = Reader::from_bytes("source.txt", original.as_bytes(), options(&root)).unwrap();
    assert_eq!(other.descriptor().resource_id, metadata.resource_id);
    assert!(other.validate_selection(&selection).is_err());
    assert_eq!(
        reader
            .original_range(&metadata.revision, 0, original.len())
            .unwrap(),
        original.as_bytes()
    );
    let mut forged = selection.clone();
    forged.quote = "wrong".into();
    assert!(reader.validate_selection(&forged).is_err());
    let position = reader
        .reading_position(View::Source, selection.location.clone())
        .unwrap();
    assert!(
        reader
            .restore_position(&position, false)
            .unwrap()
            .same_revision
    );
    let mut unauthorized = position.clone();
    unauthorized.view = View::Edit;
    assert!(reader.restore_position(&unauthorized, false).is_err());
    let mut unsupported = position;
    unsupported.version += 1;
    assert!(reader.restore_position(&unsupported, false).is_err());
}

#[test]
fn save_refresh_and_conflict_have_separate_revision_outcomes() {
    let root = root();
    let path = root.path().join("source.md");
    fs::write(&path, "# Original\r\n中文😀\r\n").unwrap();
    let mut reader = Reader::open(FileGrant::read_write(&path).unwrap(), options(&root)).unwrap();
    let before = reader.descriptor();
    let old_operation = reader.operations().begin().unwrap();
    let edit = reader
        .begin_edit(&before.revision, None, None, false)
        .unwrap();
    let request = SaveRequest {
        revision: edit.revision,
        start: edit.window.start,
        end: edit.window.end,
        changes: vec![Change {
            from: 0,
            to: 0,
            insert: "Saved\n".into(),
        }],
    };
    let saved = reader.save(&request).unwrap();
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        "Saved\r\n# Original\r\n中文😀\r\n"
    );
    assert!(saved.needs_refresh);
    assert_ne!(saved.revision, saved.preview_revision);
    assert_eq!(saved.preview_revision, before.revision);
    assert_eq!(
        reader
            .edit_window(&saved.revision, None, None, false)
            .unwrap()
            .revision,
        saved.revision
    );
    assert!(reader.descriptor().needs_refresh);
    let refreshed = reader.refresh().unwrap();
    assert_eq!(refreshed.revision, saved.revision);
    assert!(!refreshed.needs_refresh);
    assert!(old_operation.cancelled());
    assert!(reader.window(&before.revision, 0, 1).is_err());
    let edit = reader
        .begin_edit(&refreshed.revision, None, None, false)
        .unwrap();
    fs::write(&path, "# External\r\n").unwrap();
    let request = SaveRequest {
        revision: edit.revision,
        start: edit.window.start,
        end: edit.window.end,
        changes: vec![Change {
            from: 0,
            to: 0,
            insert: "Do not overwrite\n".into(),
        }],
    };
    assert!(reader.save(&request).is_err());
    assert_eq!(fs::read_to_string(path).unwrap(), "# External\r\n");
}

#[test]
fn explicit_capabilities_and_stable_memory_notes_do_not_cross_documents() {
    let root = root();
    let notes = root.path().join("notes");
    let opts = || options(&root).notes_directory(notes.clone());
    let reader = Reader::from_bytes("notes.md", b"# Public note", opts()).unwrap();
    let revision = reader.descriptor().revision;
    reader
        .notes(NoteRequest::Add {
            kind: "favorite".into(),
            quote: "Public note".into(),
            comment: String::new(),
            anchor: NoteAnchor::default(),
            revision,
        })
        .unwrap();
    reader.close().unwrap();
    let reopened = Reader::from_bytes("notes.md", b"# Public note", opts()).unwrap();
    assert_eq!(
        reopened
            .notes(NoteRequest::List {
                offset: 0,
                kind: None
            })
            .unwrap()["total"],
        1
    );
    let other = Reader::from_bytes("other.md", b"# Other", opts()).unwrap();
    assert_eq!(
        other
            .notes(NoteRequest::List {
                offset: 0,
                kind: None
            })
            .unwrap()["total"],
        0
    );
    let mut blocked = Reader::from_table(
        &["value".into()],
        &[vec!["42".into()]],
        options(&root).queries(false),
    )
    .unwrap();
    let descriptor = blocked.descriptor();
    assert!(!descriptor.capabilities.sql);
    let operation = blocked.operations().begin().unwrap();
    assert_eq!(
        blocked
            .sql(&descriptor.revision, 0, "SELECT * FROM t", &operation)
            .err()
            .unwrap()
            .kind(),
        io::ErrorKind::PermissionDenied
    );
    assert!(
        blocked
            .notes(NoteRequest::List {
                offset: 0,
                kind: None
            })
            .is_err()
    );
}

#[test]
fn query_cells_expire_with_owned_results_and_note_excerpts_stay_explicit() {
    use marknoria::reader::Selection;
    let root = root();
    let mut reader = Reader::from_table(&["Name".into()], &[vec!["中文😀".into()]], options(&root).notes_directory(root.path().join("notes"))).unwrap();
    let descriptor = reader.descriptor(); let operation = reader.operations().begin().unwrap();
    let result = reader.sql(&descriptor.revision, 0, "SELECT c1 FROM t", &operation).unwrap();
    let mut selection = Selection { document_id: descriptor.id.clone(), revision: descriptor.revision.clone(), quote: "中文😀".into(), quote_truncated: false, precision: Precision::Exact, viewport_rect: None, location: Location::QueryCell { query_id: result.id.clone(), row: 0, column: 0 } };
    reader.validate_selection(&selection).unwrap();
    selection.quote = "forged quote".into(); assert!(reader.validate_selection(&selection).is_err()); selection.quote = "中文😀".into();
    selection.location = Location::QueryCell { query_id: result.id.clone(), row: 0, column: result.columns.len() };
    assert!(reader.validate_selection(&selection).is_err());
    selection.location = Location::QueryCell { query_id: result.id.clone(), row: 1, column: 0 };
    assert!(reader.validate_selection(&selection).is_err());
    selection.location = Location::QueryCell { query_id: result.id.clone(), row: 0, column: 0 };
    reader.close_sql(&result.id); assert!(reader.validate_selection(&selection).is_err());
    let note = reader.notes(NoteRequest::Add { kind: "comment".into(), quote: "中文".into(), comment: String::new(), revision: descriptor.revision.clone(), anchor: NoteAnchor { quote_truncated: true, location: Some(Location::Cell { table: 0, row: 0, column: 0 }), ..Default::default() } }).unwrap();
    let copied = reader.notes(NoteRequest::Copy { id: note["id"].as_str().unwrap().into() }).unwrap();
    assert!(copied["text"].as_str().unwrap().contains("excerpt (truncated)"));
    let notes = reader.notes(NoteRequest::List { offset: 0, kind: None }).unwrap(); assert_eq!(notes["items"][0]["anchor"]["quoteTruncated"], true);
    reader.close().unwrap();
}

#[test]
fn charts_use_complete_inputs_preserve_query_results_and_reject_over_budget_data() {
    use marknoria::reader::ChartSource;
    let root = root();
    let columns = vec!["Name".into(), "Value".into()];
    let rows = (0..123).map(|n| vec![if n == 3 { "中文😀".repeat(100) } else { format!("row {n}") }, n.to_string()]).collect::<Vec<_>>();
    let mut reader = Reader::from_table(&columns, &rows, options(&root)).unwrap();
    let descriptor = reader.descriptor(); assert!(descriptor.capabilities.charts);
    let operation = reader.operations().begin().unwrap();
    let mut live = Vec::new(); for _ in 0..4 { live.push(reader.sql(&descriptor.revision,0,"SELECT c1,c2 FROM t",&operation).unwrap().id); }
    let chart = reader.chart(&descriptor.revision,ChartSource::Table{table:0},Some(0),1,&operation).unwrap();
    assert_eq!(chart.dataset_scope,"complete-source-table"); assert_eq!(chart.rows,123); assert_eq!(chart.points.len(),123); assert_eq!(chart.points[122].value,Some(122.0));
    assert_eq!(chart.x_name,"Name"); assert_eq!(chart.y_name,"Value"); assert!(chart.points[3].label_truncated); assert!("中文😀".repeat(100).starts_with(&chart.points[3].label));
    for id in &live { reader.sql_page(&descriptor.revision,id,0).unwrap(); }
    let query = reader.sql(&descriptor.revision,0,"SELECT c1,CASE WHEN c2='1' THEN NULL ELSE -CAST(c2 AS REAL) END AS value FROM t ORDER BY CAST(c2 AS INTEGER) DESC",&operation).unwrap();
    let chart = reader.chart(&descriptor.revision,ChartSource::Query{query_id:query.id.clone()},None,1,&operation).unwrap();
    assert_eq!(chart.rows,123); assert_eq!(chart.points[0].value,Some(-122.0)); assert_eq!(chart.points[121].value,None); assert_eq!(chart.dataset_scope,"complete-query-result");
    assert!(reader.chart(&descriptor.revision,ChartSource::Query{query_id:query.id.clone()},None,0,&operation).is_err());
    reader.close_sql(&query.id); assert!(reader.chart(&descriptor.revision,ChartSource::Query{query_id:query.id},None,1,&operation).is_err());
    let infinite=reader.sql(&descriptor.revision,0,"SELECT 1e999 AS n",&operation).unwrap();
    assert!(reader.chart(&descriptor.revision,ChartSource::Query{query_id:infinite.id},None,0,&operation).unwrap_err().to_string().contains("finite"));
    let long_number=reader.sql(&descriptor.revision,0,&format!("SELECT '{}' AS n", "0".repeat(1024)+"1"),&operation).unwrap();
    assert!(reader.chart(&descriptor.revision,ChartSource::Query{query_id:long_number.id},None,0,&operation).unwrap_err().to_string().contains("numeric label"));
    assert!(reader.chart("stale",ChartSource::Table{table:0},None,1,&operation).is_err());
    operation.cancel(); assert_eq!(reader.chart(&descriptor.revision,ChartSource::Table{table:0},None,1,&operation).unwrap_err().kind(),io::ErrorKind::Interrupted);
    reader.close().unwrap();
    let rows = vec![vec!["point".into(),"1".into()];2001];
    let mut large = Reader::from_table(&columns,&rows,options(&root)).unwrap(); let descriptor=large.descriptor(); let operation=large.operations().begin().unwrap();
    assert!(large.chart(&descriptor.revision,ChartSource::Table{table:0},Some(0),1,&operation).unwrap_err().to_string().contains("2,000 points"));
    let query=large.sql(&descriptor.revision,0,"SELECT COUNT(*),SUM(CAST(c2 AS INTEGER)) FROM t",&operation).unwrap();
    let chart=large.chart(&descriptor.revision,ChartSource::Query{query_id:query.id},None,1,&operation).unwrap(); assert_eq!(chart.points[0].value,Some(2001.0)); assert_eq!(chart.input_rows,2001);
    large.close().unwrap();
    let rows=vec![vec!["label".repeat(25),"0".repeat(1023)+"1"];123];
    let mut wide=Reader::from_table(&columns,&rows,options(&root)).unwrap(); let descriptor=wide.descriptor();let operation=wide.operations().begin().unwrap();
    assert!(wide.chart(&descriptor.revision,ChartSource::Table{table:0},Some(0),1,&operation).unwrap_err().to_string().contains("128 KiB"));wide.close().unwrap();
    assert_eq!(fs::read_dir(root.path().join("cache")).unwrap().count(),0);
}

#[test]
fn sql_notes_retain_verified_snapshot_provenance_after_reopen() {
    let root = root();
    let opts = || options(&root).notes_directory(root.path().join("notes")).resource_id("public-query-notes".into());
    let mut reader = Reader::from_bytes("notes.csv", b"ID,Value\r\n001,7\r\n002,8\r\n", opts()).unwrap();
    let descriptor = reader.descriptor();
    assert!(descriptor.capabilities.query_notes);
    let operation = reader.operations().begin().unwrap();
    let result = reader.sql(&descriptor.revision, 0, "SELECT c1 FROM t ORDER BY c1 DESC", &operation).unwrap();
    let request = || NoteRequest::Add {
        kind: "comment".into(), quote: "002".into(), comment: "Derived result".into(),
        anchor: NoteAnchor { location:Some(Location::QueryCell { query_id:result.id.clone(), row:0, column:0 }), ..Default::default() },
        revision: descriptor.revision.clone(),
    };
    let id = reader.notes(request()).unwrap()["id"].as_str().unwrap().to_owned();
    reader.close_sql(&result.id);
    assert!(reader.notes(request()).is_err());
    reader.close().unwrap();
    let reopened = Reader::from_bytes("notes.csv", b"ID,Value\r\n001,7\r\n002,8\r\n", opts()).unwrap();
    let list = reopened.notes(NoteRequest::List { offset:0, kind:None }).unwrap();
    assert_eq!(list["total"], 1);
    assert_eq!(list["items"][0]["quote"], "002");
    assert_eq!(list["items"][0]["anchor"]["query"]["sql"], "SELECT c1 FROM t ORDER BY c1 DESC");
    assert!(list["items"][0]["anchor"]["location"].is_null());
    let copy = reopened.notes(NoteRequest::Copy { id }).unwrap();
    assert!(copy["text"].as_str().unwrap().contains("snapshot, not a source cell"));
}

#[test]
fn projected_images_expose_metadata_and_png_frames_without_cache_paths() {
    let root = root();
    // Generated two-pixel RGBA PNG; no external file or image package is needed.
    let original: &[u8] = &[137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 2, 0, 0, 0, 1, 8, 6, 0, 0, 0, 244, 34, 127, 138, 0, 0, 0, 14, 73, 68, 65, 84, 120, 156, 99, 248, 207, 192, 0, 66, 255, 1, 15, 249, 3, 253, 133, 17, 153, 118, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130];
    let reader = Reader::from_bytes("pixels.png", original, options(&root)).unwrap();
    let descriptor = reader.descriptor();
    assert_eq!(descriptor.assets, 1);
    assert!(descriptor.capabilities.assets && !descriptor.capabilities.source);
    let info = reader.asset_info(&descriptor.revision, 0).unwrap();
    assert_eq!((info.width, info.height), (2, 1));
    assert!(info.dimensions_exact);
    assert_eq!(info.alt, "pixels.png");
    assert_eq!(info.frames.len(), 1);
    assert_eq!(info.frames[0].frame, 0);
    assert!(info.pdf_page.is_none());
    let value = serde_json::to_value(&info).unwrap();
    let mut keys: Vec<_> = value.as_object().unwrap().keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(keys, vec!["alt", "dimensionsExact", "frames", "height", "id", "pdfPage", "plays", "revision", "width"]);
    assert_eq!(value["frames"][0].as_object().unwrap().len(), 2);
    assert_eq!(reader.original_range(&descriptor.revision, 0, original.len()).unwrap(), original);
    assert!(reader.asset(&descriptor.revision, 0, 0).unwrap().starts_with(b"\x89PNG\r\n\x1a\n"));
    assert!(reader.asset_info(&descriptor.revision, 1).is_err());
    assert!(reader.asset_info(&"f".repeat(64), 0).is_err());
    assert!(reader.asset(&descriptor.revision, 0, 1).is_err());
    reader.close().unwrap();
    assert_eq!(fs::read_dir(root.path().join("cache")).unwrap().count(), 0);
}
