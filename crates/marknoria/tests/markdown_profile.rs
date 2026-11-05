//! Same synthetic bytes as examples/reader-corpus; public core only.
#[path = "support/markdown.rs"]
mod fixtures;
use marknoria::reader::{Location, Options, Precision, Reader};
#[test]
fn gfm_profile_and_source_ranges_survive_link_rendering() {
    let root = marknoria::runtime::cache::temporary_directory("mn-markdown-corpus-").unwrap();
    let original = fixtures::gfm();
    let reader = Reader::from_bytes(
        "public.md",
        original.as_bytes(),
        Options::default().cache_root(root.path().join("cache")),
    )
    .unwrap();
    let descriptor = reader.descriptor();
    let mut html = String::new();
    for page in 0..descriptor.projection_pages {
        html.push_str(&reader.window(&descriptor.revision, page, 1).unwrap().pages[0].html);
    }
    for expected in [
        "<strong>bold</strong>",
        "<em>emphasis</em>",
        "<del>deleted</del>",
        "type=\"checkbox\"",
        "href=\"http://www.example.org/path\"",
        "href=\"mailto:me@example.org\"",
        "href=\"https://example.org/a_(b)\"",
        "href=\"https://table.example.org\"",
        "href=\"https://target.example.org\"",
        "href=\"https://reference.example.org\"",
        "&lt;script&gt;",
        "<kbd>Ctrl</kbd>",
        "<strong title=\"literal &amp; title\">HTML bold</strong>",
        "<details open=\"\"><summary>HTML details</summary>",
        "First line<br",
    ] {
        assert!(html.contains(expected), "Missing {expected} in {html}");
    }
    assert!(!html.contains("href=\"http://www.code.example.org"));
    assert!(!html.contains("<script>"));
    assert!(!html.contains("id=\"source-view\"") && !html.contains("data-source-line=\"1\">Safe raw link"));
    assert!(!html.contains("onclick="));
    assert_eq!(descriptor.tables, 1);
    let table = reader.table(&descriptor.revision, 0, 0, &[0, 1]).unwrap();
    assert!(table.html.contains("中文|😀") && table.html.contains("001"));
    let excerpt = "EOF sentinel 中文😀.";
    let start = original.find(excerpt).unwrap();
    let selection = reader
        .source_selection(
            &descriptor.revision,
            start,
            start + excerpt.len(),
            0,
            excerpt.encode_utf16().count(),
        )
        .unwrap();
    assert_eq!(selection.quote, excerpt);
    assert_eq!(selection.precision, Precision::Exact);
    assert_eq!(
        selection.location,
        Location::Source {
            start_byte: start,
            end_byte: start + excerpt.len()
        }
    );
    reader.validate_selection(&selection).unwrap();
    assert_eq!(
        reader
            .original_range(&descriptor.revision, 0, original.len())
            .unwrap(),
        original.as_bytes()
    );
}
