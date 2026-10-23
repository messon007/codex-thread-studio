use comrak::{Arena, Options, nodes::NodeValue};
use serde::Serialize;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[derive(Serialize)]
struct Heading {
    raw: String,
    label: String,
    depth: u8,
    line: usize,
    offset: usize,
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub struct ParsedDocument {
    html: String,
    headings: Vec<Heading>,
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
impl ParsedDocument {
    pub fn take_html(&mut self) -> String {
        std::mem::take(&mut self.html)
    }

    pub fn headings_json(&self) -> String {
        serde_json::to_string(&self.headings).unwrap()
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub fn parse_document(source: &str) -> ParsedDocument {
    let mut options = Options::default();
    options.extension.table = true;
    options.extension.strikethrough = true;
    options.extension.tasklist = true;
    options.extension.autolink = true;
    options.render.hardbreaks = true;
    options.render.r#unsafe = true;
    let arena = Arena::new();
    let root = comrak::parse_document(&arena, source, &options);
    let mut lines = vec![(0, 0)];
    let mut utf16_offset = 0;
    let mut previous_cr = false;
    for (byte, character) in source.char_indices() {
        utf16_offset += character.len_utf16();
        if character == '\n' && previous_cr {
            *lines.last_mut().unwrap() = (byte + 1, utf16_offset);
        } else if matches!(character, '\n' | '\r') {
            lines.push((byte + 1, utf16_offset));
        }
        previous_cr = character == '\r';
    }
    let mut headings = Vec::new();
    for node in root.children() {
        let data = node.data();
        let NodeValue::Heading(heading) = &data.value else {
            continue;
        };
        let line = data.sourcepos.start.line;
        let (byte, offset) = lines[line - 1];
        let end = lines
            .get(data.sourcepos.end.line)
            .map_or(source.len(), |entry| entry.0);
        let mut label = String::new();
        for child in node.descendants().skip(1) {
            match &child.data().value {
                NodeValue::Text(text) => label.push_str(text),
                NodeValue::HtmlInline(text) => label.push_str(text),
                NodeValue::Code(code) => label.push_str(&code.literal),
                NodeValue::SoftBreak | NodeValue::LineBreak => label.push('\n'),
                _ => {}
            }
        }
        headings.push(Heading {
            raw: source[byte..end].to_owned(),
            label,
            depth: heading.level - 1,
            line,
            offset,
        });
        if headings.len() == 2_000 {
            break;
        }
    }
    let mut html = String::new();
    comrak::format_html(root, &options, &mut html).unwrap();
    ParsedDocument { html, headings }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headings_use_original_utf16_line_starts() {
        let source = "# 中文 😀\r\n\r\n   ## A &amp; **B**\r\n\r\nSetext\r\n===\r\n";
        let document = parse_document(source);
        assert_eq!(document.headings.len(), 3);
        assert_eq!(document.headings[1].line, 3);
        assert_eq!(
            document.headings[1].offset,
            source[..source.find("   ##").unwrap()]
                .encode_utf16()
                .count()
        );
        assert_eq!(document.headings[1].raw, "   ## A &amp; **B**\r\n");
        assert_eq!(document.headings[2].raw, "Setext\r\n===\r\n");
    }

    #[test]
    fn keeps_gfm_and_raw_html_for_the_existing_sanitizer() {
        let document = parse_document(
            "| A | B |\n| :- | -: |\n| x | y |\n\n- [x] Task\n\n~~deleted~~\n\n<script>alert(1)</script>\n",
        );
        assert!(document.html.contains("<table>"));
        assert!(document.html.contains("checkbox"));
        assert!(document.html.contains("<del>deleted</del>"));
        assert!(document.html.contains("<script>"));
    }

    #[test]
    fn legacy_cr_lines_keep_heading_positions() {
        let source = "# First\r\r   ## Second\r";
        let document = parse_document(source);
        assert_eq!(document.headings[1].line, 3);
        assert_eq!(document.headings[1].offset, source.find("   ##").unwrap());
        assert_eq!(document.headings[1].raw, "   ## Second\r");
    }

    #[test]
    fn ignores_nested_headings_and_bounds_the_outline() {
        let document = parse_document(&format!("> # Nested\n\n{}", "# Heading\n\n".repeat(2_100)));
        assert_eq!(document.headings.len(), 2_000);
        assert_eq!(document.headings[0].line, 3);
    }
}
