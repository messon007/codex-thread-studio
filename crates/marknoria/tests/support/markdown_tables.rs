// Regression contracts for complete Markdown table input and bounded previews.
use crate::core::{self, Index, MAX_HTML, ProjectionProfile, blocks, source, table_query};
use kuchikiki::traits::TendrilSink;
use std::{
    fs,
    path::PathBuf,
    sync::{Arc, atomic::AtomicBool},
};

struct Fixture {
    _root: tempfile::TempDir,
    original: PathBuf,
    cache: PathBuf,
    index: Index,
    bytes: Vec<u8>,
}

#[test]
fn markup_breaks_are_bounded_after_plain_fallback_and_header_only_audits() {
    let header = "H<br>".repeat(10000);
    let value = format!("{}TAIL", "v<br>".repeat(10000));
    for profile in profiles() {
        let f = Fixture::new(&format!("| {header} |\n|---|\n| {value} |\n"), profile);
        f.audit(1, 1);
        for page in 0..f.index.pages.len() {
            let html = core::read_page(&f.cache, &f.index, page).unwrap();
            assert!(html.contains("data-cell-preview=\"true\""));
            let dom = kuchikiki::parse_html().one(html);
            assert!(dom.descendants().count() < super::MAX_EVENTS);
        }
        assert_eq!(
            values(f.query("SELECT length(c1), substr(c1,-4) FROM t").unwrap()),
            vec![vec![String::from("20004"), String::from("TAIL")]]
        );
        let empty = Fixture::new("| Header |\n|---|\n", profile);
        let path = empty.cache.join("pages.html");
        let html = fs::read_to_string(&path).unwrap();
        fs::write(&path, html.replace("Header", "Tamper")).unwrap();
        assert!(core::audit_tables(&empty.cache, &empty.index).is_err());
    }
}

#[test]
fn complete_row_sidecar_shares_the_existing_cache_budget_before_writing() {
    use std::io::Write;
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("limited");
    let mut file = std::io::BufWriter::new(fs::File::create(&path).unwrap());
    let mut count = super::MAX_CACHE - 10;
    let mut writer = super::DataWriter {
        file: &mut file,
        bytes: &mut count,
        html_bytes: 7,
    };
    assert!(
        writer
            .write_all(b"four")
            .unwrap_err()
            .to_string()
            .contains("cache budget")
    );
    writer.flush().unwrap();
    assert_eq!(count, super::MAX_CACHE - 10);
    assert_eq!(fs::metadata(path).unwrap().len(), 0);
}

#[test]
fn flattened_rows_that_fit_do_not_omit_a_long_column_for_short_peers() {
    let value = "x".repeat(50_000);
    for profile in profiles() {
        let f = Fixture::new(
            &format!("| Key | Value |\n|---|---|\n| a | {value}<br>TAIL |\n"),
            profile,
        );
        let window = core::read_table(&f.cache, &f.index, 0, 0, &[0, 1]).unwrap();
        assert!(!window.html.contains("data-cell-preview"));
        assert!(window.html.contains(&value));
        assert!(window.html.contains("<br>\nTAIL"));
        f.audit(1, 2);
    }
}
impl Fixture {
    fn new(text: &str, profile: ProjectionProfile) -> Self {
        let root = tempfile::tempdir().unwrap();
        let original = root.path().join("input.md");
        let cache = root.path().join("cache");
        fs::create_dir(&cache).unwrap();
        fs::write(&original, text).unwrap();
        let index = core::build_index_with_profile(&original, &cache, profile).unwrap();
        Self {
            _root: root,
            original,
            cache,
            index,
            bytes: text.as_bytes().to_vec(),
        }
    }
    fn query(&self, sql: &str) -> std::io::Result<table_query::Window> {
        table_query::Queries::default().run(
            &self.cache,
            &self.index,
            0,
            sql,
            Arc::new(AtomicBool::new(false)),
        )
    }
    fn source_unchanged(&self) {
        let mut bytes = Vec::new();
        let mut offset = 0;
        while offset < self.index.source_bytes {
            let window =
                source::read_source(&self.cache, &self.index, None, Some(offset), false).unwrap();
            assert_eq!(window.start, offset);
            assert!(window.end > offset);
            assert!(window.text.len() <= source::SOURCE_BYTES);
            assert_eq!(window.text.len(), window.end - window.start);
            bytes.extend_from_slice(window.text.as_bytes());
            offset = window.end;
        }
        assert_eq!(
            bytes, self.bytes,
            "bounded source windows must reconstruct every original byte"
        );
        assert_eq!(
            fs::read(&self.original).unwrap(),
            self.bytes,
            "index/query/audit must never rewrite the granted source"
        );
    }
    fn audit(&self, rows: usize, columns: usize) {
        let audit = core::audit_tables(&self.cache, &self.index).unwrap();
        assert_eq!(
            (audit.tables, audit.rows, audit.cells),
            (1, rows, rows * columns)
        );
        let metadata = &self.index.tables[0];
        let mut seen = 0;
        for page in metadata.first_page..metadata.end_page {
            let html = core::read_page(&self.cache, &self.index, page).unwrap();
            let count = html_shape(&html, columns);
            assert_eq!(self.index.pages[page].row_start, seen);
            assert_eq!(self.index.pages[page].rows, count);
            seen += count;
        }
        assert_eq!(seen, rows);
    }
}
fn profiles() -> [ProjectionProfile; 2] {
    [ProjectionProfile::Indexed, ProjectionProfile::Web]
}
fn html_shape(html: &str, columns: usize) -> usize {
    assert!(
        html.len() <= MAX_HTML,
        "projection HTML must remain transport-bounded"
    );
    let dom = kuchikiki::parse_html().one(html);
    for tag in ["table", "thead", "tbody"] {
        assert_eq!(dom.select(tag).unwrap().count(), 1, "{tag}");
        assert_eq!(html.matches(&format!("</{tag}>")).count(), 1);
    }
    assert_eq!(dom.select("thead tr").unwrap().count(), 1);
    assert_eq!(dom.select("thead th").unwrap().count(), columns);
    assert_eq!(html.matches("</th>").count(), columns);
    let rows = dom.select("tbody tr").unwrap().collect::<Vec<_>>();
    assert_eq!(html.matches("</tr>").count(), rows.len() + 1);
    assert_eq!(html.matches("</td>").count(), rows.len() * columns);
    for row in &rows {
        assert_eq!(row.as_node().select("td").unwrap().count(), columns);
    }
    rows.len()
}
fn values(window: table_query::Window) -> Vec<Vec<String>> {
    window
        .rows
        .into_iter()
        .map(|row| row.into_iter().map(|v| v.unwrap()).collect())
        .collect()
}

#[test]
fn oversized_cell_has_bounded_preview_complete_source_and_explicit_sql_error() {
    let value = format!("{}汉😀TAIL", "x".repeat(140_000));
    for profile in profiles() {
        let f = Fixture::new(
            &format!("| Value | ID |\n|---|---|\n| {value} | row000 |\n| after | row001 |\n"),
            profile,
        );
        f.audit(2, 2);
        let first = core::read_table(&f.cache, &f.index, 0, 0, &[0, 1]).unwrap();
        assert!(first.html.contains("data-cell-preview=\"true\""));
        assert!(
            !first.html.contains("TAIL"),
            "this case must exercise a genuinely clipped preview"
        );
        let record_file = f.index.tables[0].data_file.as_ref().unwrap();
        let records = fs::read_to_string(f.cache.join(record_file)).unwrap();
        let record: serde_json::Value =
            serde_json::from_str(records.lines().next().unwrap()).unwrap();
        assert_eq!(record["cells"][0], value);
        let failure = f
            .query("SELECT length(c1), substr(c1,-4) FROM t")
            .err()
            .expect("oversized SQL input must reject rather than query clipped text");
        assert!(
            failure.to_string().contains("SQL input text budget"),
            "{failure}"
        );
        f.source_unchanged();
    }
}

#[test]
fn six_thousand_emphasis_nodes_flatten_without_losing_sql_text_or_next_row() {
    let markdown = format!("{}TAIL", "*em* ".repeat(6000));
    let plain = format!("{}TAIL", "em ".repeat(6000));
    for profile in profiles() {
        let f = Fixture::new(
            &format!("| Value |\n|---|\n| {markdown} |\n| NEXT |\n"),
            profile,
        );
        f.audit(2, 1);
        let result = values(
            f.query("SELECT _row,_line,length(c1),substr(c1,-4) FROM t ORDER BY _row")
                .unwrap(),
        );
        assert_eq!(
            result,
            vec![
                vec![
                    "1".into(),
                    "3".into(),
                    plain.chars().count().to_string(),
                    "TAIL".into()
                ],
                vec!["2".into(), "4".into(), "4".into(), "NEXT".into()]
            ]
        );
        let html = core::read_table(&f.cache, &f.index, 0, 0, &[0])
            .unwrap()
            .html;
        assert_eq!(html_shape(&html, 1), 2);
        assert!(
            !html.contains("data-cell-preview=\"true\""),
            "markup pressure alone must not clip bounded plaintext"
        );
        f.source_unchanged();
    }
}

#[test]
fn sixteen_dense_rows_preserve_unique_rows_selected_columns_and_source_coordinates() {
    let mut text = String::from("\u{feff}# Table\r\n\r\n| ID | Rich | Tail |\r\n|---|---|---|\r\n");
    for row in 0..16 {
        text.push_str(&format!(
            "| row-{row:03} | {}END{row:03} | 汉😀-{row:03} |\r\n",
            "**abc** ".repeat(350)
        ));
    }
    for profile in profiles() {
        let f = Fixture::new(&text, profile);
        f.audit(16, 3);
        let table = &f.index.tables[0];
        let all_html = (table.first_page..table.end_page)
            .map(|p| core::read_page(&f.cache, &f.index, p).unwrap())
            .collect::<String>();
        for row in 0..16 {
            assert_eq!(all_html.matches(&format!("row-{row:03}")).count(), 1);
        }
        let mut next = 0;
        while next < 16 {
            let window = core::read_table(&f.cache, &f.index, 0, next, &[0, 2]).unwrap();
            assert_eq!(window.row_start, next);
            assert!(window.row_end > next);
            assert_eq!(html_shape(&window.html, 2), window.row_end - next);
            let dom = kuchikiki::parse_html().one(window.html);
            for (offset, row) in dom.select("tbody tr").unwrap().enumerate() {
                let number = next + offset;
                let attributes = row.attributes.borrow();
                assert_eq!(
                    attributes.get("data-source-line"),
                    Some((number + 5).to_string().as_str())
                );
                let start: usize = attributes
                    .get("data-source-start")
                    .unwrap()
                    .parse()
                    .unwrap();
                let end: usize = attributes.get("data-source-end").unwrap().parse().unwrap();
                assert!(text.is_char_boundary(start) && text.is_char_boundary(end));
                assert!(text[start..end].contains(&format!("row-{number:03}")));
                let cells = row
                    .as_node()
                    .select("td")
                    .unwrap()
                    .map(|cell| cell.text_contents())
                    .collect::<Vec<_>>();
                assert_eq!(
                    cells,
                    vec![format!("row-{number:03}"), format!("汉😀-{number:03}")]
                );
            }
            let (block, _) =
                blocks::read_table_blocks(&f.cache, &f.index, 0, next, &[0, 2]).unwrap();
            match block {
                blocks::Block::Table {
                    columns, row_lines, ..
                } => {
                    assert_eq!(columns, vec![0, 2]);
                    assert_eq!(
                        row_lines,
                        (next..window.row_end)
                            .map(|row| row + 5)
                            .collect::<Vec<_>>()
                    );
                }
                _ => panic!("expected structured table"),
            }
            next = window.row_end;
        }
        let result = values(
            f.query("SELECT _row,_line,length(c2),substr(c2,-6) FROM t ORDER BY _row")
                .unwrap(),
        );
        assert_eq!(result.len(), 16);
        for (row, values) in result.iter().enumerate() {
            assert_eq!(
                values,
                &vec![
                    (row + 1).to_string(),
                    (row + 5).to_string(),
                    (350 * 4 + 6).to_string(),
                    format!("END{row:03}")
                ]
            );
        }
        f.source_unchanged();
    }
}

#[test]
fn escaping_inflation_clips_only_preview_and_retains_unicode_crlf_bom_sql_value() {
    let plain = format!("{}汉😀TAIL", "&<>\"".repeat(9000));
    // Code spans keep punctuation literal, making escaped HTML much larger than
    // the original UTF-8 or SQLite value without creating raw-HTML structure.
    let text = format!("\u{feff}| Value |\r\n|---|\r\n| `{plain}` |\r\n");
    for profile in profiles() {
        let f = Fixture::new(&text, profile);
        f.audit(1, 1);
        let html = core::read_table(&f.cache, &f.index, 0, 0, &[0])
            .unwrap()
            .html;
        assert!(html.contains("data-cell-preview=\"true\""));
        assert_eq!(html_shape(&html, 1), 1);
        assert_eq!(
            values(
                f.query("SELECT length(c1),substr(c1,-6),_line FROM t")
                    .unwrap()
            ),
            vec![vec![
                plain.chars().count().to_string(),
                "汉😀TAIL".into(),
                "3".into()
            ]]
        );
        f.source_unchanged();
    }
}

#[test]
fn maximum_width_and_header_only_tables_remain_valid() {
    let header = format!(
        "|{}|\n|{}|\n",
        (0..256)
            .map(|c| format!("H{c}"))
            .collect::<Vec<_>>()
            .join("|"),
        vec!["---"; 256].join("|")
    );
    for profile in profiles() {
        let empty = Fixture::new(&header, profile);
        empty.audit(0, 256);
        let html = core::read_table(&empty.cache, &empty.index, 0, 0, &[0, 255])
            .unwrap()
            .html;
        assert_eq!(html_shape(&html, 2), 0);
        assert_eq!(
            values(empty.query("SELECT count(*) FROM t").unwrap()),
            vec![vec!["0".to_string()]]
        );
        let mut text = header.clone();
        for row in 0..5 {
            text.push_str(&format!(
                "|{}|\n",
                (0..256)
                    .map(|column| format!("r{row}c{column}"))
                    .collect::<Vec<_>>()
                    .join("|")
            ));
        }
        let f = Fixture::new(&text, profile);
        f.audit(5, 256);
        let columns = (0..256).collect::<Vec<_>>();
        let first = core::read_table(&f.cache, &f.index, 0, 0, &columns).unwrap();
        assert!(first.page_size <= 2);
        let last = core::read_table(&f.cache, &f.index, 0, 4, &[0, 255]).unwrap();
        assert_eq!(last.row_end, 5);
        assert!(last.row_start <= 4);
        assert_eq!(html_shape(&last.html, 2), last.row_end - last.row_start);
        assert_eq!(
            values(f.query("SELECT c1,c256 FROM t ORDER BY _row").unwrap()),
            (0..5)
                .map(|row| vec![format!("r{row}c0"), format!("r{row}c255")])
                .collect::<Vec<_>>()
        );
        f.source_unchanged();
    }
}

#[test]
fn ordinary_inline_plaintext_matches_cached_structured_cells() {
    let text = "| Value | Other |\n|---|---|\n| **bold** *em* `a < b` &amp; \\| | [link](https://example.com) |\n| ![alt](missing.png) <b>raw</b> | first<br>second |\n";
    for profile in profiles() {
        let f = Fixture::new(text, profile);
        f.audit(2, 2);
        let (block, _) = blocks::read_table_blocks(&f.cache, &f.index, 0, 0, &[0, 1]).unwrap();
        let expected = match block {
            blocks::Block::Table { rows, .. } => rows
                .into_iter()
                .map(|row| {
                    row.into_iter()
                        .map(|cell| cell.into_iter().map(|s| s.text).collect::<String>())
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>(),
            _ => panic!("expected structured table"),
        };
        assert_eq!(
            values(f.query("SELECT c1,c2 FROM t ORDER BY _row").unwrap()),
            expected,
            "the complete SQL sidecar must preserve existing ordinary cell plaintext semantics"
        );
        f.source_unchanged();
    }
}

#[test]
fn audit_rejects_same_size_input_sidecar_and_preview_tampering() {
    for profile in profiles() {
        let f = Fixture::new("| Header |\n|---|\n| original |\n", profile);
        f.audit(1, 1);
        let data = f.cache.join(f.index.tables[0].data_file.as_ref().unwrap());
        let bytes = fs::read_to_string(&data).unwrap();
        fs::write(&data, bytes.replace("original", "modified")).unwrap();
        assert!(core::audit_tables(&f.cache, &f.index).is_err());
        fs::write(&data, &bytes).unwrap();
        let pages = f.cache.join("pages.html");
        let html = fs::read_to_string(&pages).unwrap();
        for tampered in [
            html.replace("original", "modified"),
            html.replace("Header", "Broken"),
            html.replace("data-source-line=\"3\"", "data-source-line=\"9\""),
        ] {
            assert_ne!(tampered, html);
            assert_eq!(tampered.len(), html.len());
            fs::write(&pages, tampered).unwrap();
            assert!(core::audit_tables(&f.cache, &f.index).is_err());
        }
        fs::write(&pages, html).unwrap();
        f.audit(1, 1);
        f.source_unchanged();
    }
}

// A nested table cannot currently be paged independently of its container.
// Its budget fallback is an explicit whole-document source projection, never
// a partially published set of table cells or a silently truncated rich block.
fn assert_nested_source_fallback(text: &str, profile: ProjectionProfile) {
    let f = Fixture::new(text, profile);
    assert!(
        f.index.text_pages,
        "oversized nested table must open as explicit source preview"
    );
    assert_eq!(
        f.index.format,
        core::document_format::DocumentFormat::Markdown
    );
    assert!(
        f.index.tables.is_empty(),
        "whole-document fallback cannot retain SQL table identities"
    );
    assert!(
        f.index
            .pages
            .iter()
            .all(|page| page.table.is_none() && page.rows == 0 && page.columns == 0)
    );
    assert!(
        f.query("SELECT * FROM t").is_err(),
        "source fallback must not expose stale query input"
    );

    let note = core::documents::preview_note(&f.cache)
        .unwrap()
        .expect("fallback must explain the source preview");
    assert!(!note.trim().is_empty());
    assert!(note.len() <= 1024);
    let first = core::read_page(&f.cache, &f.index, 0).unwrap();
    let dom = kuchikiki::parse_html().one(first);
    let notice = dom
        .select(".document-preview-note[role=status]")
        .unwrap()
        .collect::<Vec<_>>();
    assert_eq!(notice.len(), 1, "fallback must have a visible status note");
    assert_eq!(notice[0].text_contents(), note);
    assert!(
        blocks::read_blocks(&f.cache, &f.index, 0)
            .unwrap()
            .iter()
            .any(|block| {
                matches!(block, blocks::Block::Paragraph { runs, .. }
            if runs.iter().map(|run| run.text.as_str()).collect::<String>() == note)
            }),
        "native projection must expose the same fallback explanation"
    );

    let mut raw = String::new();
    for page in 0..f.index.pages.len() {
        let html = core::read_page(&f.cache, &f.index, page).unwrap();
        assert!(
            html.len() <= MAX_HTML,
            "source fallback HTML must also respect the transport budget"
        );
        assert_eq!(
            kuchikiki::parse_html()
                .one(html)
                .select("table")
                .unwrap()
                .count(),
            0,
            "Markdown table source belongs inside code text, not live cell IDs"
        );
        raw.push_str(&core::read_cached_page(&f.cache, &f.index, page).unwrap());
    }
    assert_eq!(
        raw.as_bytes(),
        text.as_bytes(),
        "raw cached text must preserve BOM, CRLF and every cell tail"
    );
    let audit = core::audit_tables(&f.cache, &f.index).unwrap();
    assert_eq!((audit.tables, audit.rows, audit.cells), (0, 0, 0));
    f.source_unchanged();

    // The failed rich attempt may have already emitted a top-level table.
    // Its page file, sidecars and private staging directories must disappear.
    assert!(!f.cache.join("pages.html").exists());
    for entry in fs::read_dir(&f.cache).unwrap() {
        let entry = entry.unwrap();
        assert!(
            entry.file_type().unwrap().is_file(),
            "fallback left an owned staging directory"
        );
        assert!(
            !entry
                .file_name()
                .to_string_lossy()
                .starts_with("markdown-table-"),
            "failed table input sidecar survived fallback"
        );
    }
    let mut siblings = fs::read_dir(f._root.path())
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    siblings.sort();
    assert_eq!(
        siblings,
        vec!["cache", "input.md"],
        "owned staging must not outlive indexing"
    );
}

#[test]
fn quoted_oversized_cell_falls_back_to_complete_original_source() {
    let text = format!(
        "\u{feff}# Before\r\n\r\n> | Value |\r\n> |---|\r\n> | {}汉😀TAIL |\r\n\r\nAfter\r\n",
        "x".repeat(140_000)
    );
    for profile in profiles() {
        assert_nested_source_fallback(&text, profile);
    }
}

#[test]
fn list_nested_emphasis_event_overflow_has_explicit_source_fallback() {
    let text = format!(
        "- Item\n\n  | Value |\n  |---|\n  | {}TAIL |\n\nAfter\n",
        "*em* ".repeat(6000)
    );
    for profile in profiles() {
        assert_nested_source_fallback(&text, profile);
    }
}

#[test]
fn nested_cumulative_inline_budget_falls_back_without_losing_any_row() {
    let mut text = String::from("> | ID | Rich |\n> |---|---|\n");
    for row in 0..16 {
        text.push_str(&format!(
            "> | row-{row:03} | {}END{row:03} |\n",
            "**abc** ".repeat(350)
        ));
    }
    for profile in profiles() {
        assert_nested_source_fallback(&text, profile);
    }
}

#[test]
fn nested_html_escape_expansion_triggers_source_fallback_at_serialization() {
    // Raw source and event count are small, but escaping the code span alone
    // expands beyond128KiB. This exercises the final serialized HTML boundary.
    let text = format!(
        "> | Value |\n> |---|\n> | `{}汉😀TAIL` |\n",
        "&<>\"".repeat(10000)
    );
    assert!(text.len() < MAX_HTML);
    for profile in profiles() {
        assert_nested_source_fallback(&text, profile);
    }
}

#[test]
fn whole_document_fallback_discards_already_emitted_top_level_table_ids() {
    let text = format!(
        "| Earlier | Value |\n|---|---|\n| row-before | unchanged |\n\n> | Nested |\n> |---|\n> | {}TAIL |\n",
        "x".repeat(140_000)
    );
    for profile in profiles() {
        assert_nested_source_fallback(&text, profile);
    }
}

#[test]
fn ordinary_quoted_and_list_tables_remain_rich_with_source_anchors() {
    let text = "> | Quoted | Value |\r\n> |---|---|\r\n> | **中文😀** | first |\r\n\r\n- Item\r\n\r\n  | Nested | Value |\r\n  |---|---|\r\n  | NestedRow | *second* |\r\n";
    for profile in profiles() {
        let f = Fixture::new(text, profile);
        assert!(
            !f.index.text_pages,
            "ordinary nested tables must not lose rich projection"
        );
        assert!(core::documents::preview_note(&f.cache).unwrap().is_none());
        let html = (0..f.index.pages.len())
            .map(|page| core::read_page(&f.cache, &f.index, page).unwrap())
            .collect::<String>();
        let dom = kuchikiki::parse_html().one(html);
        assert!(dom.select("strong").unwrap().any(|node| node.text_contents() == "中文😀"));
        assert!(dom.select("em").unwrap().any(|node| node.text_contents() == "second"));
        assert_eq!(dom.select("blockquote table").unwrap().count(), 1);
        assert_eq!(dom.select("li table").unwrap().count(), 1);
        let lines = dom
            .select("tr")
            .unwrap()
            .map(|row| {
                row.attributes
                    .borrow()
                    .get("data-source-line")
                    .unwrap()
                    .parse::<usize>()
                    .unwrap()
            })
            .collect::<Vec<_>>();
        assert_eq!(lines, vec![1, 3, 7, 9]);
        core::audit_tables(&f.cache, &f.index).unwrap();
        f.source_unchanged();
    }
}
