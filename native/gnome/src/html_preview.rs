//! Worker-owned, passive local HTML. The original snapshot is never serialized
//! from this view or used as a browser URL. The sanitizer owns HTML5 correction;
//! the tokenizer supplies bounded, verifiable source-heading positions only.
use html5gum::{emitters::default::DefaultEmitter, Token, Tokenizer};
use std::{
    collections::{HashMap, HashSet},
    io::{self, Write},
};
use studio_markdown_native::text::TextDocument;

pub const INPUT_BYTES: usize = 2 * 1024 * 1024;
const OUTPUT_BYTES: usize = 4 * 1024 * 1024;
const ELEMENTS: usize = 16_384;
const DEPTH: usize = 128;
const HEADINGS: usize = 2_000;
const FORBIDDEN: &[&str] = &[
    "head", "title", "script", "style", "iframe", "object", "embed", "form", "input", "button",
    "textarea", "select", "option", "link", "meta", "base", "img", "video", "audio", "source",
    "canvas", "template", "noscript",
];
#[derive(Clone, Debug)]
pub struct Heading {
    pub label: String,
    pub depth: usize,
    pub id: usize,
    /// Verified original opening-tag offset; ambiguous labels have no mapping.
    pub original: Option<usize>,
}
#[derive(Clone, Debug)]
pub struct Preview {
    pub body: Result<String, String>,
    pub headings: Vec<Heading>,
}
fn heading(name: &[u8]) -> Option<usize> {
    (name.len() == 2 && name[0] == b'h' && (b'1'..=b'6').contains(&name[1]))
        .then(|| usize::from(name[1] - b'1'))
}
fn void(name: &str) -> bool {
    matches!(
        name,
        "area"
            | "base"
            | "br"
            | "col"
            | "embed"
            | "hr"
            | "img"
            | "input"
            | "link"
            | "meta"
            | "param"
            | "source"
            | "track"
            | "wbr"
    )
}
#[derive(Debug)]
struct RawHeading {
    label: String,
    limited: bool,
    depth: usize,
    start: usize,
    end: usize,
}
fn scan(source: &str) -> Result<Vec<RawHeading>, String> {
    let mut emitter = DefaultEmitter::<usize>::new_with_span();
    emitter.naively_switch_states(true);
    let mut stack: Vec<String> = Vec::new();
    let mut result = Vec::new();
    let mut current: Option<RawHeading> = None;
    let mut elements = 0;
    for (tokens, token) in Tokenizer::new_with_emitter(source, emitter).enumerate() {
        if tokens >= ELEMENTS * 8 {
            return Err("HTML exceeds the token budget; use Source code.".into());
        }
        match token.map_err(|e| e.to_string())? {
            Token::StartTag(tag) => {
                elements += 1;
                if elements > ELEMENTS || tag.attributes.len() > 128 {
                    return Err(
                        "HTML exceeds the element/attribute budget; use Source code.".into(),
                    );
                }
                let name = String::from_utf8_lossy(&tag.name).to_string();
                let suppressed = stack.iter().any(|s| FORBIDDEN.contains(&s.as_str()));
                if !suppressed {
                    if let Some(depth) = heading(&tag.name) {
                        if let Some(old) = current.take() {
                            result.push(old);
                        }
                        current = Some(RawHeading {
                            label: String::new(),
                            limited: false,
                            depth,
                            start: tag.span.start,
                            end: tag.span.end,
                        });
                    }
                }
                if !void(&name) {
                    if stack.len() >= DEPTH {
                        return Err("HTML exceeds the nesting budget; use Source code.".into());
                    }
                    stack.push(name);
                }
            }
            Token::EndTag(tag) => {
                if heading(&tag.name).is_some() {
                    if let Some(old) = current.take() {
                        result.push(old);
                    }
                }
                let name = String::from_utf8_lossy(&tag.name);
                if let Some(i) = stack.iter().rposition(|s| *s == name) {
                    stack.truncate(i);
                }
            }
            Token::String(value)
                if current.is_some() && !stack.iter().any(|s| FORBIDDEN.contains(&s.as_str())) =>
            {
                let current = current.as_mut().unwrap();
                if !current.limited {
                    let text = String::from_utf8_lossy(&value);
                    let mut end = text
                        .len()
                        .min(4096_usize.saturating_sub(current.label.len()));
                    while !text.is_char_boundary(end) {
                        end -= 1;
                    }
                    current.label.push_str(&text[..end]);
                    current.limited = end < text.len();
                }
            }
            _ => {}
        }
    }
    if let Some(old) = current {
        result.push(old);
    }
    if result.len() > HEADINGS {
        return Err("HTML exceeds the heading budget; use Source code.".into());
    }
    for h in &mut result {
        h.label = h.label.split_whitespace().collect::<Vec<_>>().join(" ");
    }
    Ok(result)
}
struct Bounded(Vec<u8>);
impl Write for Bounded {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > OUTPUT_BYTES.saturating_sub(self.0.len()) {
            return Err(io::Error::other(
                "HTML exceeds the output budget; use Source code.",
            ));
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
impl Preview {
    pub fn prepare(path: &str, document: &TextDocument) -> Option<Self> {
        if !matches!(
            path.rsplit('.').next()?.to_ascii_lowercase().as_str(),
            "htm" | "html" | "xhtml"
        ) {
            return None;
        }
        let mut preview = Self {
            body: Ok(String::new()),
            headings: Vec::new(),
        };
        preview.body = preview.render(document.as_str());
        if preview.body.is_err() {
            preview.headings.clear();
        }
        Some(preview)
    }
    fn render(&mut self, source: &str) -> Result<String, String> {
        if source.len() > INPUT_BYTES {
            return Err("HTML exceeds the Preview size budget; use Source code.".into());
        }
        let original = scan(source)?;
        let mut sanitizer = ammonia::Builder::default();
        // Explicit allow-list: no URL-bearing, style, event, namespace or DOM
        // identity attributes survive. Original IDs/classes cannot target UI.
        let mut tags = sanitizer.clone_tags();
        for tag in FORBIDDEN {
            tags.remove(tag);
        }
        sanitizer
            .tags(tags)
            .tag_attributes(HashMap::new())
            .generic_attributes(HashSet::from(["title", "lang", "dir"]))
            .link_rel(None)
            .url_schemes(HashSet::new())
            .url_relative(ammonia::UrlRelative::Deny)
            .clean_content_tags(FORBIDDEN.iter().copied().collect());
        // Passive table/list semantics are safe; dimensions and resource URLs
        // are deliberately excluded, matching the existing static Preview.
        sanitizer
            .add_tag_attributes("td", ["colspan", "rowspan"])
            .add_tag_attributes("th", ["colspan", "rowspan", "scope"])
            .add_tag_attributes("ol", ["start", "reversed"])
            .add_tag_attributes("li", ["value"]);
        let mut writer = Bounded(Vec::new());
        sanitizer
            .clean(source)
            .write_to(&mut writer)
            .map_err(|e| e.to_string())?;
        let body = String::from_utf8(writer.0).map_err(|e| e.to_string())?;
        let rendered = scan(&body)?;
        let mut positions: HashMap<(usize, &str), Vec<usize>> = HashMap::new();
        for h in original.iter().filter(|h| !h.limited) {
            positions
                .entry((h.depth, &h.label))
                .or_default()
                .push(h.start);
        }
        let mut output = String::with_capacity(body.len() + rendered.len() * 40);
        let mut offset = 0;
        for (id, h) in rendered.into_iter().enumerate() {
            let at = h.end.checked_sub(1).ok_or("Invalid heading token")?;
            if at < offset || body.as_bytes().get(at) != Some(&b'>') || !body.is_char_boundary(at) {
                return Err("HTML heading position could not be verified; use Source code.".into());
            }
            output.push_str(&body[offset..at]);
            output.push_str(&format!(" id=\"studio-html-heading-{id}\""));
            offset = at;
            let original = (!h.limited)
                .then(|| {
                    positions
                        .get(&(h.depth, h.label.as_str()))
                        .filter(|p| p.len() == 1)
                        .map(|p| p[0])
                })
                .flatten();
            self.headings.push(Heading {
                label: if h.limited {
                    format!("{}…", h.label)
                } else {
                    h.label
                },
                depth: h.depth,
                id,
                original,
            });
        }
        output.push_str(&body[offset..]);
        if output.len() > OUTPUT_BYTES {
            return Err("HTML exceeds the output budget; use Source code.".into());
        }
        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn prepare(source: &str) -> Preview {
        Preview::prepare("file.HTML", &TextDocument::new(source.into()).unwrap()).unwrap()
    }
    #[test]
    fn passive_html_keeps_semantics_without_any_active_attribute_or_content() {
        let p = prepare(
            r#"<h1 id=x onclick=alert(1)>Title &amp; 中文😀</h1><script>secret</script><style>bad</style><p style="background:url(http://x)" src=secret><a href="javascript:alert(1)">link</a></p><img src=file:///secret><iframe src=https://x>hidden</iframe><form>credentials</form><table><tr><th scope=col>A</th><td colspan=2>value</td></tr></table>"#,
        );
        let body = p.body.unwrap();
        assert!(body.contains("Title &amp; 中文😀"));
        assert!(body.contains("<table>") && body.contains("colspan=\"2\""));
        for bad in [
            "secret",
            "credentials",
            "hidden",
            "onclick",
            "javascript:",
            "<script",
            "style=",
            "href=",
            "src=",
            "id=\"x\"",
        ] {
            assert!(!body.contains(bad), "{bad}: {body}");
        }
        assert_eq!(p.headings[0].original, Some(0));
        assert_eq!(p.headings[0].label, "Title & 中文😀");
    }
    #[test]
    fn headings_use_original_utf8_bytes_and_ambiguous_labels_are_unmapped() {
        let source =
            "中文😀\r\n<h2\n title='x'>One <em>two</em></h2>\r\n<h1>Same</h1><h1>Same</h1>";
        let p = prepare(source);
        let body = p.body.unwrap();
        assert_eq!(p.headings[0].original, Some(source.find("<h2").unwrap()));
        assert_eq!(p.headings[0].label, "One two");
        assert_eq!(p.headings[1].original, None);
        assert_eq!(p.headings[2].original, None);
        for i in 0..3 {
            assert!(body.contains(&format!("id=\"studio-html-heading-{i}\"")));
        }
    }
    #[test]
    fn malformed_namespace_and_entity_payloads_remain_passive() {
        for source in ["<svg><foreignObject><script>alert(1)</script><img src=x onerror=alert(2)></foreignObject></svg>", "<math><mtext><table><mglyph><style><!--</style><img title=\"--><img src=x onerror=alert(1)>\">", "<a href='&#106;avascript:alert(1)' style='color:red'>safe</a><meta http-equiv=refresh content='0;url=file:///etc/passwd'>", "<template><h1>Hidden</h1></template><h1>Shown</h1>"] {
            let p = prepare(source); let body = p.body.unwrap();
            for bad in ["<script", "<img", "<meta", "href=", "onerror=", "style=", "<svg", "<math"] { assert!(!body.contains(bad), "{bad}: {body}"); }
        }
    }
    #[test]
    fn a_single_large_text_token_does_not_escape_the_outline_label_budget() {
        let source = format!("<h1>{}</h1>", "中文😀".repeat(8192));
        let p = prepare(&source);
        assert!(p.body.as_ref().unwrap().contains(&"中文😀".repeat(8192)));
        assert!(p.headings[0].label.len() <= 4096 + '…'.len_utf8());
        assert!(
            p.headings[0].original.is_none(),
            "A truncated label cannot verify a source heading"
        );
    }
    #[test]
    fn size_depth_and_elements_are_explicit_errors_not_partial_html() {
        for source in [
            "x".repeat(INPUT_BYTES + 1),
            "<div>".repeat(DEPTH + 1),
            "<br>".repeat(ELEMENTS + 1),
        ] {
            assert!(prepare(&source).body.is_err());
        }
        assert!(Preview::prepare("file.rs", &TextDocument::new("text".into()).unwrap()).is_none());
    }
}
