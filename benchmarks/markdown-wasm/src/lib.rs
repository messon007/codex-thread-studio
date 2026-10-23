use comrak::{
    Arena, Options,
    nodes::{NodeCodeBlock, NodeValue},
};
use serde::Serialize;
use std::{borrow::Cow, collections::HashMap};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[derive(Serialize)]
pub struct Heading {
    label: String,
    depth: u8,
    line: usize,
    offset: usize,
}

#[derive(Serialize)]
pub struct Document {
    html: String,
    headings: Vec<Heading>,
    plantuml_blocks: usize,
}

fn options() -> Options<'static> {
    let mut options = Options::default();
    options.extension.table = true;
    options.extension.strikethrough = true;
    options.extension.tasklist = true;
    options.extension.autolink = true;
    options.render.hardbreaks = true;
    options.render.r#unsafe = true;
    options
}

fn marker(line: &str, name: &str) -> bool {
    line.get(..name.len())
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case(name))
        && line
            .as_bytes()
            .get(name.len())
            .is_none_or(|byte| !byte.is_ascii_alphanumeric() && *byte != b'_')
}

fn fence(line: &str) -> Option<(u8, usize)> {
    let trimmed = line.trim_end_matches(['\r', '\n']);
    let indent = trimmed.bytes().take_while(|byte| *byte == b' ').count();
    if indent > 3 {
        return None;
    }
    let body = &trimmed[indent..];
    let character = *body.as_bytes().first()?;
    if !matches!(character, b'`' | b'~') {
        return None;
    }
    let length = body.bytes().take_while(|byte| *byte == character).count();
    (length >= 3).then_some((character, length))
}

fn plantuml_view(source: &str) -> (Cow<'_, str>, HashMap<usize, String>) {
    if !source.as_bytes().contains(&b'@') {
        return (Cow::Borrowed(source), HashMap::new());
    }
    let lines: Vec<&str> = source.split_inclusive('\n').collect();
    let mut replacements = HashMap::new();
    let mut blocks = HashMap::new();
    let mut active_fence = None;
    let mut cursor = 0;
    while cursor < lines.len() {
        let line = lines[cursor];
        if let Some((character, length)) = active_fence {
            if let Some((next_character, next_length)) = fence(line) {
                let tail = line.trim_start_matches(' ').trim_end_matches(['\r', '\n']);
                if next_character == character
                    && next_length >= length
                    && tail[next_length..].trim().is_empty()
                {
                    active_fence = None;
                }
            }
            cursor += 1;
            continue;
        }
        if let Some(opening) = fence(line) {
            active_fence = Some(opening);
            cursor += 1;
            continue;
        }
        if !marker(line, "@startuml") {
            cursor += 1;
            continue;
        }
        let start = cursor;
        cursor += 1;
        let mut fence_length = 3;
        while cursor < lines.len() && !marker(lines[cursor], "@enduml") {
            let run = lines[cursor]
                .trim_start_matches(' ')
                .bytes()
                .take_while(|byte| *byte == b'~')
                .count();
            fence_length = fence_length.max(run + 1);
            cursor += 1;
        }
        if cursor == lines.len() {
            break;
        }
        let delimiter = "~".repeat(fence_length);
        replacements.insert(start, format!("{delimiter}studio-plantuml-bench\n"));
        replacements.insert(cursor, format!("{delimiter}\n"));
        blocks.insert(
            start + 1,
            lines[start..=cursor].concat().trim_end().to_owned(),
        );
        cursor += 1;
    }
    if blocks.is_empty() {
        return (Cow::Borrowed(source), blocks);
    }
    let mut parsed = String::with_capacity(source.len());
    for (index, line) in lines.iter().enumerate() {
        parsed.push_str(replacements.get(&index).map_or(*line, String::as_str));
    }
    (Cow::Owned(parsed), blocks)
}

fn parse(source: &str, include_headings: bool) -> Document {
    let (view, blocks) = plantuml_view(source);
    let options = options();
    let arena = Arena::new();
    let root = comrak::parse_document(&arena, &view, &options);
    for node in root.descendants() {
        let mut data = node.data_mut();
        if let NodeValue::CodeBlock(block) = &data.value {
            if block.info == "studio-plantuml-bench" {
                if let Some(literal) = blocks.get(&data.sourcepos.start.line) {
                    data.value = NodeValue::CodeBlock(Box::new(NodeCodeBlock {
                        fenced: true,
                        info: "plantuml".into(),
                        literal: literal.clone(),
                        ..NodeCodeBlock::default()
                    }));
                }
            }
        }
    }
    let mut headings = Vec::new();
    if include_headings {
        let mut offsets = vec![0];
        let mut utf16_offset = 0;
        for character in source.chars() {
            utf16_offset += character.len_utf16();
            if character == '\n' {
                offsets.push(utf16_offset);
            }
        }
        for node in root.children() {
            let data = node.data();
            let NodeValue::Heading(heading) = &data.value else {
                continue;
            };
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
            let line = data.sourcepos.start.line;
            headings.push(Heading {
                label,
                depth: heading.level - 1,
                line,
                offset: offsets.get(line.saturating_sub(1)).copied().unwrap_or(0)
                    + data.sourcepos.start.column.saturating_sub(1),
            });
            if headings.len() == 2_000 {
                break;
            }
        }
    }
    let mut html = String::new();
    comrak::format_html(root, &options, &mut html).unwrap();
    Document {
        html,
        headings,
        plantuml_blocks: blocks.len(),
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub fn render_html(source: &str) -> String {
    parse(source, false).html
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub fn render_document_json(source: &str) -> String {
    serde_json::to_string(&parse(source, true)).unwrap()
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub struct ParsedDocument {
    document: Document,
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
impl ParsedDocument {
    pub fn take_html(&mut self) -> String {
        std::mem::take(&mut self.document.html)
    }

    pub fn headings_json(&self) -> String {
        serde_json::to_string(&self.document.headings).unwrap()
    }
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn parse_document(source: &str) -> ParsedDocument {
    ParsedDocument {
        document: parse(source, true),
    }
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn wasm_memory() -> JsValue {
    wasm_bindgen::memory()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_plantuml_preserves_content_and_heading_positions() {
        let source = "# 中文 😀\r\n\r\n@STARTUML\r\n# not a heading\r\nAlice -> Bob: <hello>\r\n@ENDUML\r\n\r\n## Tail\r\n";
        let document = parse(source, true);
        assert_eq!(document.plantuml_blocks, 1);
        assert_eq!(document.headings.len(), 2);
        assert_eq!(document.headings[1].line, 8);
        assert_eq!(
            document.headings[1].offset,
            source[..source.find("## Tail").unwrap()]
                .encode_utf16()
                .count()
        );
        assert!(document.html.contains("language-plantuml"));
        assert!(document.html.contains("@STARTUML"));
        assert!(document.html.contains("&lt;hello&gt;"));
    }

    #[test]
    fn fences_and_unfinished_blocks_are_not_rewritten() {
        for source in [
            "```text\n@startuml\nAlice -> Bob\n@enduml\n```\n",
            "@startuml\nAlice -> Bob",
            "> @startuml\n> A -> B\n> @enduml\n",
        ] {
            assert_eq!(parse(source, true).plantuml_blocks, 0);
        }
    }

    #[test]
    fn diagram_fence_delimiters_do_not_escape_the_adapter() {
        let document = parse("@startuml\n~~~\n# hidden\n@enduml\n\n## Tail", true);
        assert_eq!(document.headings.len(), 1);
        assert!(document.html.contains("~~~"));
        assert!(document.html.contains("# hidden"));
    }
}
