// Execute the exact public SDK fixtures in Studio's normal platform test matrix.
// Keep the reviewed vendor sources byte-for-byte; they own their formatting.
#[rustfmt::skip]
#[path = "../../crates/marknoria/tests/reader_api.rs"]
mod public_api;
#[rustfmt::skip]
#[path = "../../crates/marknoria/tests/markdown_profile.rs"]
mod markdown_profile;

#[test]
fn studio_reduced_profile_keeps_excluded_source_text_and_exact_ranges() {
    use marknoria::{
        core::highlight,
        reader::{Location, Options, Reader},
    };
    let cache = tempfile::tempdir().unwrap();
    let text = "\u{feff}// 中文\r\nvalue = \"😀 <script>unsafe()</script>&\";\r\n";
    for (language, extension) in [
        ("cobol", "cob"),
        ("d", "d"),
        ("hlsl", "hlsl"),
        ("fsharp", "fs"),
        ("ocaml", "ml"),
        ("idris", "idr"),
        ("odin", "odin"),
        ("c-sharp", "cs"),
        ("perl", "pl"),
        ("ruby", "rb"),
        ("elixir", "ex"),
        ("vb", "vb"),
        ("php", "php"),
    ] {
        assert!(highlight::code(language, text).is_none(), "{language}");
        let reader = Reader::from_bytes(
            &format!("source.{extension}"),
            text.as_bytes(),
            Options::default().cache_root(cache.path().join(language)),
        )
        .unwrap();
        let revision = reader.descriptor().revision;
        assert_eq!(
            reader.original_range(&revision, 0, text.len()).unwrap(),
            text.as_bytes()
        );
        let page = reader.window(&revision, 0, 1).unwrap();
        assert!(page.pages[0].html.contains("😀"));
        assert!(!page.pages[0].html.contains("<script>"));
        assert!(!page.pages[0].html.contains("mn-h"), "{language}");
        let normalized = text.replace("\r\n", "\n");
        let from = normalized[..normalized.find('😀').unwrap()]
            .encode_utf16()
            .count();
        let selected = reader
            .source_selection(&revision, 0, text.len(), from, from + 2)
            .unwrap();
        assert_eq!(selected.quote, "😀");
        assert!(
            matches!(selected.location, Location::Source {start_byte, end_byte}
            if start_byte == text.find('😀').unwrap() && end_byte == start_byte + 4)
        );
        reader.close().unwrap();
    }
    assert!(highlight::code("rust", "fn retained() -> bool { true }").is_some());
}
