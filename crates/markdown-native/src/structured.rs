//! Read-only formatting with original token spelling and sparse byte mappings.
//! Never serialize a parsed value back over the source. Invalid/oversized input
//! retains its original immutable document and the same bounded page interface.
use crate::text::TextDocument;
use std::{ops::Range, sync::Arc};

pub const FORMAT_BYTES: usize = 256 * 1024;
pub const OUTPUT_BYTES: usize = 1024 * 1024;
const TOKENS: usize = 65_536;
const DEPTH: usize = 128;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Language {
    Json,
    Xml,
    Yaml,
    Toml,
    Config,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Format {
    pub language: Language,
    pub label: &'static str,
}
impl Format {
    pub fn for_path(path: &str) -> Option<Self> {
        let name = path
            .rsplit(['/', '\\'])
            .find(|s| !s.is_empty())?
            .to_ascii_lowercase();
        let (language, label) = if name == ".env" || name.starts_with(".env.") {
            (Language::Config, "ENV")
        } else {
            match name.as_str() {
                ".editorconfig" => (Language::Config, "EDITORCONFIG"),
                ".gitconfig" => (Language::Config, "GITCONFIG"),
                ".npmrc" => (Language::Config, "NPMRC"),
                ".pnpmrc" => (Language::Config, "PNPMRC"),
                ".yarnrc" => (Language::Config, "YARNRC"),
                "cargo.lock" | "poetry.lock" | "pdm.lock" | "pipfile" => (Language::Toml, "TOML"),
                _ => match name.rsplit_once('.')?.1 {
                    "json" => (Language::Json, "JSON"),
                    "json5" => (Language::Json, "JSON5"),
                    "jsonc" => (Language::Json, "JSONC"),
                    "jsonl" | "ndjson" => (Language::Json, "JSONL"),
                    "yaml" | "yml" => (Language::Yaml, "YAML"),
                    "xml" => (Language::Xml, "XML"),
                    "plist" => (Language::Xml, "PLIST"),
                    "rss" => (Language::Xml, "RSS"),
                    "atom" => (Language::Xml, "ATOM"),
                    "toml" => (Language::Toml, "TOML"),
                    "ini" => (Language::Config, "INI"),
                    "cfg" => (Language::Config, "CFG"),
                    "conf" => (Language::Config, "CONF"),
                    "properties" => (Language::Config, "PROPERTIES"),
                    _ => return None,
                },
            }
        };
        Some(Self { language, label })
    }
}
#[derive(Clone, Debug)]
struct Segment {
    display: Range<usize>,
    original: usize,
}
#[derive(Clone, Debug)]
pub struct Preview {
    pub format: Format,
    pub document: TextDocument,
    pub formatted: bool,
    /// Original input exceeds the initial formatting byte budget.
    pub limited: bool,
    original: TextDocument,
    map: Arc<[Segment]>,
}
impl Preview {
    pub fn prepare(path: &str, original: &TextDocument) -> Option<Self> {
        let format = Format::for_path(path)?;
        let source = original.as_str();
        let limited = source.len() > FORMAT_BYTES;
        let result = if limited || source.trim().is_empty() {
            None
        } else {
            match format.language {
                Language::Json => json(source),
                Language::Xml => xml(source),
                Language::Toml => assignments(source, true),
                Language::Config => assignments(source, false),
                Language::Yaml => None,
            }
        };
        let (document, map, formatted) = match result.filter(|b| b.text != source) {
            Some(builder) => (
                TextDocument::new(builder.text).ok()?,
                builder.map.into(),
                true,
            ),
            None => (original.clone(), Arc::from([]), false),
        };
        Some(Self {
            format,
            document,
            formatted,
            limited,
            original: original.clone(),
            map,
        })
    }
    /// Generated indentation has no original position. A mixed selection maps
    /// to the containing original passage, including its original whitespace.
    pub fn original_range(&self, range: Range<usize>) -> Option<Range<usize>> {
        if range.start >= range.end
            || range.end > self.document.bytes()
            || !self.document.as_str().is_char_boundary(range.start)
            || !self.document.as_str().is_char_boundary(range.end)
        {
            return None;
        }
        if !self.formatted {
            return Some(range);
        }
        let start = self.map.partition_point(|s| s.display.end <= range.start);
        let spans = &self.map[start..];
        let first = spans.first().filter(|s| s.display.start < range.end)?;
        let last = spans
            .iter()
            .take_while(|s| s.display.start < range.end)
            .last()?;
        Some(
            first.original + range.start.max(first.display.start) - first.display.start
                ..last.original + range.end.min(last.display.end) - last.display.start,
        )
    }
    pub fn original_at(&self, display: usize) -> usize {
        if !self.formatted {
            return display.min(self.original.bytes());
        }
        let i = self.map.partition_point(|s| s.display.end <= display);
        self.map.get(i).map_or(self.original.bytes(), |s| {
            s.original + display.saturating_sub(s.display.start).min(s.display.len())
        })
    }
    pub fn display_at(&self, original: usize) -> usize {
        if !self.formatted {
            return original.min(self.document.bytes());
        }
        let i = self
            .map
            .partition_point(|s| s.original + s.display.len() <= original);
        self.map.get(i).map_or(self.document.bytes(), |s| {
            s.display.start + original.saturating_sub(s.original).min(s.display.len())
        })
    }
    pub fn original(&self) -> &TextDocument {
        &self.original
    }
}
struct Builder<'a> {
    source: &'a str,
    text: String,
    map: Vec<Segment>,
}
impl<'a> Builder<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            source,
            text: String::new(),
            map: Vec::new(),
        }
    }
    fn generated(&mut self, text: &str) -> Option<()> {
        if self.text.len().checked_add(text.len())? > OUTPUT_BYTES {
            return None;
        }
        self.text.push_str(text);
        Some(())
    }
    fn copy(&mut self, range: Range<usize>) -> Option<()> {
        if range.is_empty() {
            return Some(());
        }
        let start = self.text.len();
        let text = self.source.get(range.clone())?;
        self.generated(text)?;
        if let Some(last) = self
            .map
            .last_mut()
            .filter(|s| s.display.end == start && s.original + s.display.len() == range.start)
        {
            last.display.end = self.text.len();
        } else {
            if self.map.len() >= TOKENS {
                return None;
            }
            self.map.push(Segment {
                display: start..self.text.len(),
                original: range.start,
            });
        }
        Some(())
    }
    fn newline(&mut self) -> Option<()> {
        if !self.text.is_empty() && !self.text.ends_with('\n') {
            self.generated("\n")?;
        }
        Some(())
    }
    fn indent(&mut self, depth: usize) -> Option<()> {
        if self.text.is_empty() || self.text.ends_with('\n') {
            self.generated(&"  ".repeat(depth.min(64)))?;
        }
        Some(())
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Open(u8),
    Close(u8),
    Comma,
    Colon,
    LineComment,
    BlockComment,
    Value,
}
fn json(source: &str) -> Option<Builder<'_>> {
    use jsonc_parser::{tokens::Token, Scanner};
    let mut scanner = Scanner::new(source, &Default::default());
    let mut tokens = Vec::new();
    while let Some(token) = scanner.scan().ok()? {
        if tokens.len() >= TOKENS {
            return None;
        }
        let kind = match token {
            Token::OpenBrace => Kind::Open(b'{'),
            Token::OpenBracket => Kind::Open(b'['),
            Token::CloseBrace => Kind::Close(b'{'),
            Token::CloseBracket => Kind::Close(b'['),
            Token::Comma => Kind::Comma,
            Token::Colon => Kind::Colon,
            Token::CommentLine(_) => Kind::LineComment,
            Token::CommentBlock(_) => Kind::BlockComment,
            _ => Kind::Value,
        };
        tokens.push((kind, scanner.token_start()..scanner.token_end()));
    }
    let mut out = Builder::new(source);
    let mut stack = Vec::new();
    let mut previous = None;
    for (i, (kind, range)) in tokens.iter().enumerate() {
        let next = tokens.get(i + 1).map(|t| t.0);
        match *kind {
            Kind::Open(c) => {
                if stack.is_empty() && previous.is_some() {
                    out.newline()?;
                }
                out.indent(stack.len())?;
                out.copy(range.clone())?;
                if stack.len() >= DEPTH {
                    return None;
                }
                stack.push(c);
                if next != Some(Kind::Close(c)) {
                    out.newline()?;
                }
            }
            Kind::Close(c) => {
                if stack.pop()? != c {
                    return None;
                }
                if previous != Some(Kind::Open(c)) {
                    out.newline()?;
                }
                out.indent(stack.len())?;
                out.copy(range.clone())?;
            }
            Kind::Comma => {
                out.copy(range.clone())?;
                out.newline()?;
            }
            Kind::Colon => {
                out.copy(range.clone())?;
                out.generated(" ")?;
            }
            Kind::LineComment | Kind::BlockComment => {
                if !out.text.is_empty() && !out.text.ends_with(['\n', ' ']) {
                    out.generated(" ")?;
                }
                out.indent(stack.len())?;
                out.copy(range.clone())?;
                if *kind == Kind::LineComment && next.is_some() {
                    out.newline()?;
                } else if *kind == Kind::BlockComment
                    && next
                        .is_some_and(|k| !matches!(k, Kind::Comma | Kind::Colon | Kind::Close(_)))
                {
                    out.generated(" ")?;
                }
            }
            Kind::Value => {
                if stack.is_empty() && previous.is_some() {
                    out.newline()?;
                }
                out.indent(stack.len())?;
                out.copy(range.clone())?;
            }
        }
        previous = Some(*kind);
    }
    if !stack.is_empty() {
        return None;
    }
    if source.ends_with('\n') {
        out.newline()?;
    }
    Some(out)
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum XmlKind {
    Start,
    End,
    Text,
    Cdata,
    Other,
}
fn xml(source: &str) -> Option<Builder<'_>> {
    use quick_xml::{events::Event, Reader};
    let mut reader = Reader::from_str(source);
    let mut stack = Vec::<(bool, bool)>::new();
    let mut tokens = Vec::new();
    loop {
        let start = reader.buffer_position() as usize;
        let event = reader.read_event().ok()?;
        let range = start..reader.buffer_position() as usize;
        if matches!(event, Event::Eof) {
            break;
        }
        if tokens.len() >= TOKENS {
            return None;
        }
        let kind = match event {
            Event::Start(ref element) => {
                for attribute in element.attributes().with_checks(false) {
                    let attribute = attribute.ok()?;
                    if attribute.key.as_ref() == b"xml:space"
                        && attribute.value.as_ref() == b"preserve"
                    {
                        return None;
                    }
                }
                if let Some(parent) = stack.last_mut() {
                    parent.0 = true;
                }
                if stack.len() >= DEPTH {
                    return None;
                }
                stack.push((false, false));
                XmlKind::Start
            }
            Event::End(_) => {
                let (child, text) = stack.pop()?;
                if child && text {
                    return None;
                }
                XmlKind::End
            }
            Event::Text(_) | Event::GeneralRef(_) | Event::CData(_) => {
                let significant = !source[range.clone()].trim().is_empty();
                if significant {
                    stack.last_mut()?.1 = true;
                }
                if !significant {
                    continue;
                }
                if matches!(event, Event::CData(_)) {
                    XmlKind::Cdata
                } else {
                    XmlKind::Text
                }
            }
            _ => {
                if let Some(parent) = stack.last_mut() {
                    parent.0 = true;
                }
                XmlKind::Other
            }
        };
        tokens.push((kind, range));
    }
    if !stack.is_empty() {
        return None;
    }
    let mut out = Builder::new(source);
    let mut depth = 0usize;
    for (i, (kind, range)) in tokens.iter().enumerate() {
        if *kind == XmlKind::End {
            depth = depth.checked_sub(1)?;
        }
        let previous = i.checked_sub(1).map(|n| tokens[n].0);
        let inline = matches!(kind, XmlKind::Text | XmlKind::Cdata)
            || (*kind == XmlKind::End && matches!(previous, Some(XmlKind::Text | XmlKind::Cdata)));
        if !inline {
            out.newline()?;
            out.indent(depth)?;
        }
        out.copy(range.clone())?;
        if *kind == XmlKind::Start {
            depth += 1;
        }
    }
    if source.ends_with('\n') {
        out.newline()?;
    }
    Some(out)
}
fn assignments(source: &str, toml: bool) -> Option<Builder<'_>> {
    let mut out = Builder::new(source);
    let mut offset = 0;
    let mut multiline: Option<&str> = None;
    let mut quote = None;
    for raw in source.split_inclusive('\n') {
        let line = raw.trim_end_matches(['\r', '\n']);
        let started_multiline = multiline.is_some() || quote.is_some();
        let mut escaped = false;
        let mut delimiter = None;
        let mut colon = None;
        let mut index = 0;
        while index < line.len() {
            if let Some(end) = multiline {
                let bytes = line.as_bytes();
                while index < bytes.len() {
                    if end == "\"\"\"" && bytes[index] == b'\\' {
                        index = (index + 2).min(bytes.len());
                        continue;
                    }
                    if bytes[index] == end.as_bytes()[0] && line[index..].starts_with(end) {
                        let quote = bytes[index];
                        index += 3;
                        while bytes.get(index) == Some(&quote) {
                            index += 1;
                        }
                        multiline = None;
                        break;
                    }
                    index += 1;
                }
                continue;
            }
            let c = line.as_bytes()[index];
            if escaped {
                escaped = false;
            } else if let Some(q) = quote {
                if c == b'\\' && q == b'"' {
                    escaped = true;
                } else if c == q {
                    quote = None;
                }
            } else if c == b'#' || c == b';' {
                break;
            } else if c == b'"' || c == b'\'' {
                let triple = if c == b'"' { "\"\"\"" } else { "'''" };
                if toml && line[index..].starts_with(triple) {
                    multiline = Some(triple);
                    index += 3;
                    continue;
                }
                quote = Some(c);
            } else if c == b'=' && delimiter.is_none() {
                delimiter = Some(index);
            } else if !toml && c == b':' && colon.is_none() {
                colon = Some(index);
            }
            index += 1;
        }
        let skip = started_multiline || line.trim_start().starts_with(['#', ';', '[']);
        if let Some(at) = delimiter.or(colon).filter(|_| !skip) {
            let left = line[..at].trim_end_matches([' ', '\t']).len();
            let right = at + 1 + line[at + 1..].len()
                - line[at + 1..].trim_start_matches([' ', '\t']).len();
            out.copy(offset..offset + left)?;
            out.generated(" ")?;
            out.copy(offset + at..offset + at + 1)?;
            out.generated(" ")?;
            out.copy(offset + right..offset + line.len())?;
        } else {
            out.copy(offset..offset + line.len())?;
        }
        out.copy(offset + line.len()..offset + raw.len())?;
        offset += raw.len();
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn preview(path: &str, source: &str) -> Preview {
        Preview::prepare(path, &TextDocument::new(source.into()).unwrap()).unwrap()
    }
    #[test]
    fn tokenizer_depth_and_output_limits_retain_complete_input() {
        for source in [
            format!("{}0{}", "[".repeat(DEPTH + 1), "]".repeat(DEPTH + 1)),
            format!("[{}]", "0,".repeat(TOKENS)),
            format!(
                "{}{}{}",
                "[".repeat(64),
                "0,".repeat(12_000),
                "]".repeat(64)
            ),
        ] {
            let p = preview("large.json5", &source);
            assert!(!p.formatted);
            assert_eq!(p.document.as_str(), source);
        }
        assert!(Format::for_path("ordinary.rs").is_none());
        assert_eq!(
            Format::for_path(r"C:\project\FILE.JSON5").unwrap().label,
            "JSON5"
        );
    }
    #[test]
    fn json5_preserves_comments_duplicate_keys_and_numeric_spelling() {
        let source="{/* 保留😀 */ unquoted:'值',\"n\":1e+04,\"n\":0xFF,// comment\r\n items:[true,null,],}\r\n";
        let p = preview("a.json5", source);
        assert!(p.formatted);
        assert!(p.document.as_str().contains("\"n\": 1e+04"));
        for token in [
            "/* 保留😀 */",
            "unquoted",
            "'值'",
            "1e+04",
            "0xFF",
            "// comment",
        ] {
            let start = p.document.as_str().find(token).unwrap();
            let original = p.original_range(start..start + token.len()).unwrap();
            assert_eq!(&source[original], token);
        }
        for segment in p.map.iter() {
            assert_eq!(
                &p.document.as_str()[segment.display.clone()],
                &source[segment.original..segment.original + segment.display.len()]
            );
        }
        assert_eq!(p.original.as_str(), source);
    }
    #[test]
    fn mappings_cover_original_passages_and_ignore_generated_indentation() {
        let p = preview("a.json", "{\"中文😀\": [1, 2]}\r\n");
        let a = p.document.as_str().find("\"中文😀\"").unwrap();
        let b = p.document.as_str().find('2').unwrap() + 1;
        let range = p.original_range(a..b).unwrap();
        assert_eq!(&p.original.as_str()[range], "\"中文😀\": [1, 2");
        let indent = p.document.as_str().find("    1").unwrap();
        assert!(p.original_range(indent..indent + 4).is_none());
        for (byte, c) in p
            .original
            .as_str()
            .char_indices()
            .filter(|(_, c)| !c.is_whitespace())
        {
            let display = p.display_at(byte);
            assert_eq!(p.document.as_str()[display..].chars().next(), Some(c));
            assert_eq!(p.original_at(display), byte);
        }
    }
    #[test]
    fn xml_preserves_entities_and_mixed_content_without_resolving_resources() {
        let p = preview(
            "a.xml",
            "<?xml version='1.0'?><root><a>中文&amp;😀</a><b/></root>",
        );
        assert!(p.formatted);
        assert!(p.document.as_str().contains("<a>中文&amp;😀</a>"));
        for source in [
            "<p>before <b>bold</b> after</p>",
            "<a><b></a>",
            "<a xml:space='preserve'>  <b/>  </a>",
        ] {
            let p = preview("a.xml", source);
            assert!(!p.formatted);
            assert_eq!(p.document.as_str(), source);
        }
    }
    #[test]
    fn yaml_assignments_multiline_and_large_inputs_preserve_source() {
        let yaml = "a: |\n  literal = value\n  中文😀\n";
        assert_eq!(preview("a.yaml", yaml).document.as_str(), yaml);
        let p = preview(
            ".env.local",
            "URL=https://host/x=a\r\n'key=x'=value\r\n# keep\r\n",
        );
        assert!(p
            .document
            .as_str()
            .starts_with("URL = https://host/x=a\r\n'key=x' = value"));
        let toml = "value=\"\"\"\ninside=unchanged\n\"\"\"\nnext=2\n";
        let p = preview("Cargo.lock", toml);
        assert!(p.document.as_str().contains("inside=unchanged"));
        assert!(p.document.as_str().ends_with("next = 2\n"));
        let escaped = "value=\"\"\"\n\\\"\"\" literal\ninside=unchanged\n\"\"\"\nnext=2\n";
        let p = preview("settings.toml", escaped);
        assert!(p.document.as_str().contains("inside=unchanged"));
        assert!(p.document.as_str().ends_with("next = 2\n"));
        let env = "VALUE=\"line one\ninside=unchanged\n\"\nNEXT=2\n";
        let p = preview(".env", env);
        assert!(p.document.as_str().contains("inside=unchanged"));
        assert!(p.document.as_str().ends_with("NEXT = 2\n"));
        let large = format!("[{}]", "0,".repeat(FORMAT_BYTES));
        let p = preview("a.json", &large);
        assert!(p.limited && !p.formatted);
        assert_eq!(p.document.as_str().as_ptr(), p.original.as_str().as_ptr());
    }
}
