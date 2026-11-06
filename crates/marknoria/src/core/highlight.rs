//! Shared bounded syntax projections for web and native document readers.
mod engine;
use std::ops::Range;

/// Shared filename mapping for preview and source/edit; never reads the file.
pub fn language_for_name(name: &str) -> String {
    use crate::core::documents::structured::{Format, Language};
    if let Some(format) = Format::for_path(name) {
        return match format.language {
            Language::Json => "json",
            Language::Xml => "xml",
            Language::Yaml => "yaml",
            Language::Toml => "toml",
            Language::Config => "ini",
        }
        .into();
    }
    let name = name
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(name)
        .to_ascii_lowercase();
    let extension = name.rsplit_once('.').map_or(name.as_str(), |(_, ext)| ext);
    match extension {
        "md" | "markdown" | "mdown" | "mkd" | "mdx" => "markdown",
        "py" | "pyi" => "python",
        "rs" => "rust",
        "cc" | "cpp" | "cxx" | "c++" | "hpp" | "hxx" => "cpp",
        "h" => "c",
        "js" | "jsx" | "mjs" | "cjs" => "javascript",
        "ts" | "tsx" => "typescript",
        other => other,
    }
    .into()
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct NativeRun {
    pub text: String,
    pub color: String,
    pub bold: bool,
    pub italic: bool,
}

const MAX_STYLE_BYTES: usize = 128 * 1024;
const MAX_RUNS: usize = 2048;
const MAX_CODE_BYTES: usize = 64 * 1024;
const MAX_CODE_HTML: usize = 2 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Style {
    kind: u8,
    bold: bool,
    italic: bool,
}
#[derive(Clone, Debug)]
struct StyledRange {
    range: Range<usize>,
    style: Style,
}
impl Style {
    fn capture(name: &str) -> Self {
        let head = name.split('.').next().unwrap_or("");
        let kind = match head {
            "comment" => 1,
            "keyword" | "conditional" | "repeat" | "include" | "exception" | "storageclass" => 2,
            "string" | "character" => 3,
            "number" | "float" | "boolean" | "constant" => 4,
            "function" | "method" | "constructor" => 5,
            "type" | "namespace" | "module" => 6,
            "property" | "field" | "attribute" | "tag" => 7,
            "operator" | "punctuation" => 0,
            "markup" if name.contains("heading") => 5,
            "text" => match name {
                "text.title" => 5,
                "text.literal" => 3,
                "text.uri" | "text.reference" => 7,
                _ => 0,
            },
            _ => 0,
        };
        Self {
            kind,
            bold: name.contains("bold")
                || name.contains("strong")
                || name.contains("heading")
                || name == "text.title",
            italic: name.contains("italic") || name.contains("emphasis"),
        }
    }
    fn color(self, dark: bool) -> &'static str {
        let colors = if dark {
            [
                "#c0c5ce", "#65737e", "#b48ead", "#a3be8c", "#d08770", "#8fa1b3", "#ebcb8b",
                "#96b5b4", "#c0c5ce",
            ]
        } else {
            [
                "#323232", "#969896", "#a71d5d", "#183691", "#0086b3", "#795da3", "#0086b3",
                "#63a35c", "#323232",
            ]
        };
        colors[self.kind as usize]
    }
}
fn boundary(source: &str, limit: usize) -> usize {
    let mut end = source.len().min(limit);
    while !source.is_char_boundary(end) {
        end -= 1
    }
    end
}

fn coalesce_native(ranges: Vec<StyledRange>, dark: bool) -> Vec<StyledRange> {
    let mut merged: Vec<StyledRange> = Vec::new();
    for range in ranges {
        if let Some(last) = merged.last_mut().filter(|last| {
            last.range.end == range.range.start
                && last.style.color(dark) == range.style.color(dark)
                && last.style.bold == range.style.bold
                && last.style.italic == range.style.italic
        }) {
            last.range.end = range.range.end
        } else {
            merged.push(range)
        }
    }
    merged
}

/// Bound styling separately from text. Grow the parsed prefix only while it
/// contains fewer than the available style runs; dense editor windows do not
/// parse a large tail that cannot be displayed with syntax tags anyway.
pub fn native(language: &str, source: &str, dark: bool) -> Vec<NativeRun> {
    if source.len() > crate::core::edit::MAX_PAGE || source.is_empty() {
        return vec![];
    }
    let limit = boundary(source, MAX_STYLE_BYTES);
    let mut end = boundary(source, 16 * 1024);
    let ranges = loop {
        let Some(ranges) = engine::ranges(language, &source[..end]) else {
            return vec![];
        };
        let ranges = coalesce_native(ranges, dark);
        if ranges.len() >= MAX_RUNS - 1 || end >= limit {
            break ranges;
        }
        end = boundary(source, (end * 2).min(limit));
    };
    let mut runs = Vec::new();
    let mut consumed = 0;
    for range in ranges.into_iter().take(MAX_RUNS - 1) {
        if range.range.is_empty() {
            continue;
        }
        consumed = range.range.end;
        runs.push(NativeRun {
            text: source[range.range].into(),
            color: range.style.color(dark).into(),
            bold: range.style.bold,
            italic: range.style.italic,
        });
    }
    if consumed < source.len() {
        runs.push(NativeRun {
            text: source[consumed..].into(),
            color: Style::default().color(dark).into(),
            bold: false,
            italic: false,
        })
    }
    runs
}

pub fn code(language: &str, source: &str) -> Option<String> {
    if source.len() > MAX_CODE_BYTES {
        return None;
    }
    let token = language.split_whitespace().next()?;
    let ranges = engine::ranges(token, source)?;
    let mut html = format!(
        "<pre><code class=\"mn-code\" data-language=\"{}\">",
        crate::core::documents::escaped(token)
    );
    for range in ranges {
        let style = range.style;
        if style != Style::default() {
            html.push_str(&format!(
                "<span class=\"mn-h{}{}{}\">",
                style.kind,
                if style.bold { " mn-bold" } else { "" },
                if style.italic { " mn-italic" } else { "" }
            ));
        }
        html.push_str(
            &crate::core::documents::escaped(&source[range.range]).replace('\r', "&#13;"),
        );
        if style != Style::default() {
            html.push_str("</span>")
        }
        if html.len() + 14 > MAX_CODE_HTML {
            return None;
        }
    }
    html.push_str("</code></pre>\n");
    Some(html)
}

/// Retain a styled prefix and every source character when rich markup would
/// exceed a page budget. This keeps the existing four-pass fallback contract.
pub fn code_bounded(language: &str, source: &str, budget: usize) -> Option<String> {
    let mut end = source.len();
    for _ in 0..4 {
        let mut html = code(language, &source[..end])?;
        let tail = crate::core::documents::escaped(&source[end..]).replace('\r', "&#13;");
        if html.len().saturating_add(tail.len()) <= budget {
            if !tail.is_empty() {
                html.truncate(html.len().checked_sub("</code></pre>\n".len())?);
                html.push_str(&tail);
                html.push_str("</code></pre>\n")
            }
            return Some(html);
        }
        end = boundary(source, end / 4);
    }
    None
}

/// Keep original-byte anchors separate from syntax styles. Both inputs are a
/// bounded cached page; rendering never reparses the full structured document.
pub(crate) fn code_mapped(language: &str, source: &str, anchors: &[crate::core::documents::projection::Anchor]) -> String {
    let ranges = engine::ranges(language, source).unwrap_or_default();
    let render = |styled: bool| {
        let mut html = format!("<pre data-document-code=\"true\"><code class=\"mn-code\" data-language=\"{}\">", crate::core::documents::escaped(language));
        let mut cursor = 0;
        let mut style_index = 0;
        for anchor in anchors {
            html.push_str(&crate::core::documents::escaped(&source[cursor..anchor.display.start]).replace('\r', "&#13;"));
            let text = &source[anchor.display.clone()];
            let end_line = anchor.line + text.bytes().filter(|b| *b == b'\n').count() - usize::from(text.ends_with('\n'));
            html.push_str(&format!("<span data-source-start=\"{}\" data-source-end=\"{}\" data-source-line=\"{}\" data-source-end-line=\"{}\">", anchor.original, anchor.original + anchor.display.len(), anchor.line, end_line));
            let mut at = anchor.display.start;
            while styled && at < anchor.display.end {
                while style_index < ranges.len() && ranges[style_index].range.end <= at { style_index += 1; }
                let Some(range) = ranges.get(style_index).filter(|r| r.range.start < anchor.display.end) else { break; };
                let unstyled_end = range.range.start.min(anchor.display.end).max(at);
                html.push_str(&crate::core::documents::escaped(&source[at..unstyled_end]).replace('\r', "&#13;"));
                at = unstyled_end;
                let end = range.range.end.min(anchor.display.end);
                if end <= at { break; }
                let style = range.style;
                if style != Style::default() { html.push_str(&format!("<span class=\"mn-h{}{}{}\">", style.kind, if style.bold { " mn-bold" } else { "" }, if style.italic { " mn-italic" } else { "" })); }
                html.push_str(&crate::core::documents::escaped(&source[at..end]).replace('\r', "&#13;"));
                if style != Style::default() { html.push_str("</span>"); }
                at = end;
            }
            html.push_str(&crate::core::documents::escaped(&source[at..anchor.display.end]).replace('\r', "&#13;"));
            html.push_str("</span>");
            cursor = anchor.display.end;
        }
        html.push_str(&crate::core::documents::escaped(&source[cursor..]).replace('\r', "&#13;"));
        html.push_str("</code></pre>\n");
        html
    };
    let rich = render(true);
    if rich.len() + 1024 <= crate::core::MAX_HTML { rich } else { render(false) }
}

pub fn css(dark: bool) -> String {
    let mut css = format!(".mn-code{{color:{}}}\n", Style::default().color(dark));
    for kind in 0..=8 {
        css.push_str(&format!(
            ".mn-h{kind}{{color:{}}}\n",
            Style {
                kind,
                ..Default::default()
            }
            .color(dark)
        ))
    }
    css.push_str(".mn-bold{font-weight:bold}.mn-italic{font-style:italic}\n");
    css
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn document_languages_have_distinct_syntax_colors() {
        for (language, text) in [
            ("toml", "[reader]\nname = \"中文😀\"\nenabled = true\n"),
            ("yaml", "name: \"中文😀\"\nenabled: true\n"),
            ("json", "{\"name\": \"中文😀\", \"enabled\": true}\n"),
            ("py", "def main():\n    return \"中文😀\" # comment\n"),
            ("c", "int main(void) { return 42; } // comment\n"),
            ("cpp", "int main() { auto s = \"中文😀\"; return 42; }\n"),
            ("rs", "fn main() { let s = \"中文😀\"; } // comment\n"),
        ] {
            let runs = native(language, text, false);
            assert!(
                runs.iter()
                    .map(|r| &r.color)
                    .collect::<std::collections::HashSet<_>>()
                    .len()
                    > 1,
                "Missing colors for {language}: {runs:?}"
            );
            assert!(
                code(language, text).is_some(),
                "Missing HTML syntax for {language}"
            );
        }
    }
    #[test]
    fn native_syntax_preserves_exact_unicode_text_and_has_a_bounded_fallback() {
        let text = "fn main() { let 中文 = \"😀 <script>\"; }\n";
        for dark in [false, true] {
            let runs = native("rust", text, dark);
            assert!(!runs.is_empty());
            assert_eq!(
                runs.iter().map(|run| run.text.as_str()).collect::<String>(),
                text
            );
        }
        assert!(native("unknown-language", text, false).is_empty());
        assert!(native("rust", &"x".repeat(crate::core::edit::MAX_PAGE + 1), false).is_empty());
    }
    #[test]
    fn dense_and_large_windows_keep_colors_and_exact_complete_text() {
        for text in [
            "{\"id\":1,\"name\":\"中文😀\",\"active\":true}\n".repeat(300),
            "{\"name\":\"中文😀\"}\n".repeat(10000),
        ] {
            assert!(text.len() <= crate::core::edit::MAX_PAGE);
            let runs = native("json", &text, false);
            assert!(runs.len() <= 2048);
            assert!(
                runs.iter()
                    .map(|r| &r.color)
                    .collect::<std::collections::HashSet<_>>()
                    .len()
                    > 1
            );
            assert_eq!(
                runs.iter().map(|r| r.text.as_str()).collect::<String>(),
                text
            );
        }
    }
    #[test]
    fn dense_html_retains_a_styled_prefix_within_the_page_budget() {
        use kuchikiki::traits::TendrilSink;
        let text = "{\"id\":1,\"name\":\"中文😀 <script>\",\"active\":true}\r\n".repeat(120);
        let html = code_bounded("json", &text, 32 * 1024).unwrap();
        assert!(html.len() <= 32 * 1024);
        assert!(html.contains("<span class=\"mn-"));
        let document = kuchikiki::parse_html().one(html);
        assert_eq!(document.select_first("code").unwrap().text_contents(), text);
        assert_eq!(document.select("script").unwrap().count(), 0);
    }
    #[test]
    fn code_is_highlighted_without_executing_html_or_losing_text() {
        let html = code("rust", "fn main() { println!(\"<script>\"); }\n").unwrap();
        assert!(html.contains("<span class=\"mn-"), "{html}");
        assert!(html.contains("&lt;script&gt;"));
        assert!(!html.contains("<script>"));
        assert!(code("unknown-language", "text").is_none());
        assert!(code("rust", &"x".repeat(64 * 1024 + 1)).is_none());
        assert!(css(false).contains(".mn-"));
        assert!(css(true).contains(".mn-"));
    }
}

#[cfg(test)]
mod compatibility_tests {
    use super::*;
    fn color_at(runs: &[NativeRun], at: usize) -> &str {
        let mut offset = 0;
        for run in runs {
            let end = offset + run.text.len();
            if at < end {
                return &run.color;
            }
            offset = end
        }
        panic!("missing source byte {at}")
    }
    #[test]
    fn legacy_query_captures_keep_sql_fields_and_markdown_titles() {
        for dark in [false, true] {
            let text = "SELECT name FROM data WHERE value > 1.5;";
            let runs = native("sql", text, dark);
            for needle in ["name", "1.5"] {
                assert_ne!(
                    color_at(&runs, text.find(needle).unwrap()),
                    Style::default().color(dark)
                );
            }
            let runs = native("markdown", "# Reader title\n", dark);
            assert!(
                runs.iter()
                    .any(|run| run.bold && run.text.contains("Reader title"))
            );
        }
    }
    #[test]
    fn comments_literals_unicode_and_crlf_retain_text_and_colors() {
        for (language, text, comment, literal) in [
            (
                "rust",
                "// 中文😀\r\nfn main(){let n=42;let s=\"中文😀 <script>\";}\r\n",
                "//",
                "42",
            ),
            (
                "python",
                "# 中文😀\r\ndef main():\r\n    return 42\r\n",
                "#",
                "42",
            ),
            (
                "c",
                "// 中文😀\r\nint main(void){return 42;}\r\n",
                "//",
                "42",
            ),
            ("cpp", "// 中文😀\r\nint main(){auto n=42;}\r\n", "//", "42"),
            ("javascript", "// 中文😀\r\nconst n = 42;\r\n", "//", "42"),
            (
                "typescript",
                "// 中文😀\r\nconst n: number = 42;\r\n",
                "//",
                "42",
            ),
            ("yaml", "# 中文😀\r\nvalue: 42\r\n", "#", "42"),
            ("toml", "# 中文😀\r\nvalue = 42\r\n", "#", "42"),
            (
                "bash",
                "# 中文😀\r\nfor item in a b; do echo \"中文😀\"; done\r\n",
                "#",
                "for",
            ),
            (
                "sql",
                "-- 中文😀\r\nSELECT * FROM data WHERE value > 42;\r\n",
                "--",
                "SELECT",
            ),
            (
                "css",
                "/* 中文😀 */\r\nbody { padding: 42px; }\r\n",
                "/*",
                "42",
            ),
            (
                "xml",
                "<!-- 中文😀 -->\r\n<root value=\"42\">中文😀</root>\r\n",
                "<!--",
                "root",
            ),
            (
                "ini",
                "; 中文😀\r\n[reader]\r\nvalue = 42\r\n",
                ";",
                "reader",
            ),
            (
                "fsharp",
                "// 中文😀\r\nlet answer: int = 42\r\n",
                "//",
                "42",
            ),
            (
                "perl",
                "# 中文😀\r\nmy $ref = [1, 42]; my @items = $ref->@*;\r\n",
                "#",
                "42",
            ),
            (
                "solidity",
                "// 中文😀\r\ncontract C { struct S { uint value; } function f() public { S memory s = S({value:42}); } }\r\n",
                "//",
                "42",
            ),
        ] {
            for dark in [false, true] {
                let runs = native(language, text, dark);
                assert_eq!(
                    runs.iter().map(|r| r.text.as_str()).collect::<String>(),
                    text,
                    "{language}"
                );
                assert_ne!(
                    color_at(&runs, text.find(comment).unwrap()),
                    Style::default().color(dark),
                    "Lost {language} comment"
                );
                assert_ne!(
                    color_at(&runs, text.find(literal).unwrap()),
                    Style::default().color(dark),
                    "Lost {language} literal"
                );
            }
        }
    }
    #[test]
    fn embedded_css_and_javascript_are_highlighted_and_html_is_inert() {
        use kuchikiki::traits::TendrilSink;
        let text = "<style>body { padding: 42px; }</style><script>const value = 42;</script><p>中文😀 &amp;</p>\r\n";
        for dark in [false, true] {
            let runs = native("html", text, dark);
            assert_eq!(
                runs.iter().map(|r| r.text.as_str()).collect::<String>(),
                text
            );
            for needle in ["42px", "const"] {
                assert_ne!(
                    color_at(&runs, text.find(needle).unwrap()),
                    Style::default().color(dark),
                    "Missing injection {needle}"
                )
            }
        }
        let html = code("html", text).unwrap();
        let doc = kuchikiki::parse_html().one(html);
        assert_eq!(doc.select("script").unwrap().count(), 0);
        assert_eq!(doc.select_first("code").unwrap().text_contents(), text);
    }
}
