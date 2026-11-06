//! Public behavior stays lossless when a build excludes optional grammars.
use marknoria::{
    core::highlight,
    reader::{Location, Options, Reader},
};
const EXCLUDED: &[(&str, &str)] = &[
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
];
#[test]
fn excluded_source_files_and_markdown_keep_text_escaping_and_original_anchors() {
    let root = tempfile::tempdir().unwrap();
    let source = "\u{feff}// first 中文\r\nvalue = \"😀 <script>alert(1)</script>&\";\r\n";
    for &(language, extension) in EXCLUDED {
        if cfg!(feature = "syntax-reduced") {
            assert!(marknoria_syntax::configuration(language).is_none());
            assert!(highlight::native(language, source, false).is_empty());
            assert!(highlight::code(language, source).is_none());
        }
        let reader = Reader::from_bytes(
            &format!("source.{extension}"),
            source.as_bytes(),
            Options::default().cache_root(root.path().join("cache")),
        )
        .unwrap();
        let owner = reader.descriptor();
        assert_eq!(
            reader
                .original_range(&owner.revision, 0, source.len())
                .unwrap(),
            source.as_bytes()
        );
        assert_eq!(
            reader
                .source(&owner.revision, Some(1), None, false)
                .unwrap()
                .text,
            source
        );
        let normalized = source.replace("\r\n", "\n");
        let from = normalized[..normalized.find('😀').unwrap()]
            .encode_utf16()
            .count();
        let selection = reader
            .source_selection(&owner.revision, 0, source.len(), from, from + 2)
            .unwrap();
        assert_eq!(selection.quote, "😀");
        assert!(
            matches!(selection.location,Location::Source{start_byte,end_byte} if start_byte==source.find('😀').unwrap()&&end_byte==start_byte+4)
        );
        let projected = reader.window(&owner.revision, 0, 1).unwrap();
        assert!(projected.pages[0].html.contains("😀"));
        assert!(!projected.pages[0].html.contains("<script>"));
        if cfg!(feature = "syntax-reduced") {
            assert!(
                !projected.pages[0].html.contains("mn-h"),
                "Wrong grammar used for {language}"
            );
        }
        reader.close().unwrap();
        let markdown = format!("# Exact source\r\n\r\n```{language}\r\n{source}```\r\n");
        let reader = Reader::from_bytes(
            "fences.md",
            markdown.as_bytes(),
            Options::default().cache_root(root.path().join("cache")),
        )
        .unwrap();
        let owner = reader.descriptor();
        let page = reader.window(&owner.revision, 0, 1).unwrap();
        assert!(page.pages[0].html.contains("😀") && !page.pages[0].html.contains("<script>"));
        if cfg!(feature = "syntax-reduced") {
            assert!(
                !page.pages[0].html.contains("mn-h"),
                "Disabled fence {language} highlighted"
            );
        }
        assert_eq!(
            reader
                .original_range(&owner.revision, 0, markdown.len())
                .unwrap(),
            markdown.as_bytes()
        );
        reader.close().unwrap();
    }
}
