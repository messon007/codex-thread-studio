//! Bounded raw-HTML token projection. No tree building, CSS, resource reads or JS.
//! The existing html5ever tokenizer preserves open inline tags across Markdown
//! events without fragment-parser auto-closing. Unsupported fragments remain
//! literal, matching the reader's existing inert fallback.
use html5ever::{
    tendril::StrTendril,
    tokenizer::{BufferQueue, TagKind, Token, TokenSink, TokenSinkResult, Tokenizer},
};

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
fn allowed(name: &str) -> bool {
    matches!(
        name,
        "p" | "div"
            | "span"
            | "section"
            | "article"
            | "main"
            | "header"
            | "footer"
            | "br"
            | "hr"
            | "wbr"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "strong"
            | "b"
            | "em"
            | "i"
            | "u"
            | "s"
            | "del"
            | "code"
            | "pre"
            | "ul"
            | "ol"
            | "li"
            | "blockquote"
            | "table"
            | "thead"
            | "tbody"
            | "tfoot"
            | "tr"
            | "th"
            | "td"
            | "a"
            | "details"
            | "summary"
            | "dl"
            | "dt"
            | "dd"
            | "sup"
            | "sub"
            | "kbd"
            | "samp"
            | "var"
            | "abbr"
            | "cite"
            | "time"
            | "mark"
            | "small"
            | "ruby"
            | "rt"
            | "rp"
            | "caption"
            | "colgroup"
            | "col"
    )
}
fn href(value: &str) -> bool {
    let value = value.trim();
    let lower = value.to_ascii_lowercase();
    !value.chars().any(char::is_control)
        && (lower.starts_with("https://")
            || lower.starts_with("http://")
            || lower.starts_with("mailto:")
            || value.starts_with('#'))
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Scope {
    Anchor,
    Code,
    Pre,
}
#[derive(Default)]
struct Projection {
    output: String,
    unsupported: bool,
    tokens: usize,
    table_structure: bool,
    scopes: Vec<(Scope, bool)>,
    web_base: Option<usize>,
    references: Vec<RawReference>,
    anchors: Vec<String>,
    target_bytes: usize,
    resource_budget: bool,
}
pub(super) struct RawReference {
    pub kind: super::references::Kind,
    pub target: String,
}
pub(super) struct WebFragment {
    pub html: String,
    pub references: Vec<RawReference>,
    pub anchors: Vec<String>,
}
impl Projection {
    fn reference(&mut self, kind: super::references::Kind, target: &str) -> Option<usize> {
        let base = self.web_base?;
        let inline = target
            .get(..5)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("data:"));
        if base + self.references.len() >= super::references::MAX_REFERENCES
            || self.target_bytes.saturating_add(target.len()) > super::references::MAX_TARGET_BYTES
            || (!inline || kind == super::references::Kind::Link)
                && target.len() > super::references::MAX_LINK_BYTES
        {
            self.resource_budget = true;
            return None;
        }
        let id = base + self.references.len();
        self.target_bytes += target.len();
        self.references.push(RawReference {
            kind,
            target: target.into(),
        });
        Some(id)
    }
}
impl TokenSink for Projection {
    type Handle = ();
    fn process_token(&mut self, token: Token, _line: u64) -> TokenSinkResult<()> {
        self.tokens += 1;
        if self.tokens > super::MAX_EVENTS || self.output.len() > super::MAX_HTML {
            self.unsupported = true;
        }
        if self.unsupported {
            return TokenSinkResult::Continue;
        }
        match token {
            Token::TagToken(tag) => {
                let name = tag.name.as_ref();
                let attribute = |key: &str| {
                    tag.attrs
                        .iter()
                        .find(|attr| attr.name.ns.is_empty() && attr.name.local.as_ref() == key)
                        .map(|attr| attr.value.as_ref())
                };
                if self.web_base.is_some() && name == "img" {
                    if tag.kind == TagKind::StartTag {
                        let alt: String =
                            attribute("alt").unwrap_or("").chars().take(256).collect();
                        if let Some(target) = attribute("src") {
                            if let Some(id) = self.reference(super::references::Kind::Image, target)
                            {
                                let title: String =
                                    attribute("title").unwrap_or("").chars().take(256).collect();
                                self.output.push_str(&format!("<span class=\"markdown-reference-image\" data-image-reference=\"{id}\" title=\"{}\"><span class=\"markdown-reference-alt\">[image: {}]</span></span>",escape(&title),escape(&alt)));
                            }
                        } else {
                            self.output.push_str(&format!("[image: {}]", escape(&alt)));
                        }
                    }
                    return TokenSinkResult::Continue;
                }
                if !allowed(name) {
                    self.unsupported = true;
                    return TokenSinkResult::Continue;
                }
                self.table_structure |= matches!(
                    name,
                    "table"
                        | "thead"
                        | "tbody"
                        | "tfoot"
                        | "tr"
                        | "th"
                        | "td"
                        | "caption"
                        | "colgroup"
                        | "col"
                );
                if let Some(scope) = match name {
                    "a" => Some(Scope::Anchor),
                    "code" => Some(Scope::Code),
                    "pre" => Some(Scope::Pre),
                    _ => None,
                } {
                    self.scopes.push((scope, tag.kind == TagKind::StartTag));
                }
                if tag.kind == TagKind::EndTag {
                    self.output.push_str(&format!("</{name}>"));
                } else {
                    self.output.push('<');
                    self.output.push_str(name);
                    if self.web_base.is_some() {
                        if let Some(anchor) = attribute("id")
                            .or_else(|| if name == "a" { attribute("name") } else { None })
                        {
                            if !anchor.is_empty()
                                && anchor.len() <= 256
                                && !anchor.chars().any(char::is_control)
                            {
                                self.anchors.push(anchor.to_owned());
                                self.output.push_str(&format!(
                                    " data-document-anchor=\"{}\"",
                                    escape(anchor)
                                ));
                                if self.anchors.len() > super::references::MAX_REFERENCES {
                                    self.resource_budget = true;
                                }
                            }
                        }
                        if name == "a" {
                            if let Some(target) = attribute("href") {
                                if let Some(id) =
                                    self.reference(super::references::Kind::Link, target)
                                {
                                    self.output.push_str(&format!(
                                        " href=\"#\" data-link-reference=\"{id}\""
                                    ));
                                }
                            }
                        }
                    }
                    for attr in tag.attrs {
                        if !attr.name.ns.is_empty() {
                            continue;
                        }
                        let key = attr.name.local.as_ref();
                        let value = attr.value.as_ref();
                        if value.len() > 2048 {
                            continue;
                        }
                        let keep = key == "title"
                            || (self.web_base.is_none()
                                && name == "a"
                                && key == "href"
                                && href(value))
                            || (matches!(name, "th" | "td")
                                && matches!(key, "rowspan" | "colspan")
                                && value.parse::<usize>().is_ok_and(|n| (1..=512).contains(&n)))
                            || (matches!(name, "th" | "td")
                                && key == "align"
                                && matches!(value, "left" | "center" | "right"))
                            || (name == "ol" && key == "start" && value.parse::<i32>().is_ok())
                            || (name == "time" && key == "datetime")
                            || (name == "details" && key == "open");
                        if keep {
                            self.output
                                .push_str(&format!(" {key}=\"{}\"", escape(value)));
                        }
                    }
                    if name == "a" {
                        self.output.push_str(" rel=\"noopener noreferrer\"");
                    }
                    self.output.push('>');
                }
            }
            Token::CharacterTokens(value) => self.output.push_str(&escape(&value)),
            Token::NullCharacterToken => self.unsupported = true,
            // Browser comments/doctype do not become document UI or source IDs.
            Token::CommentToken(_)
            | Token::DoctypeToken(_)
            | Token::EOFToken
            | Token::ParseError(_) => {}
        }
        TokenSinkResult::Continue
    }
}
fn tokenize(source: &str) -> Option<Projection> {
    if source.len() > super::MAX_HTML {
        return None;
    }
    let mut tokenizer = Tokenizer::new(Projection::default(), Default::default());
    let mut input = BufferQueue::new();
    input.push_back(StrTendril::from_slice(source));
    let _ = tokenizer.feed(&mut input);
    tokenizer.end();
    let sink = tokenizer.sink;
    (!sink.unsupported && sink.output.len() <= super::MAX_HTML).then_some(sink)
}

pub(super) fn fragment(source: &str) -> Option<String> {
    tokenize(source).map(|sink| sink.output)
}
pub(super) fn cell_fragment(source: &str) -> Option<String> {
    tokenize(source)
        .filter(|sink| !sink.table_structure)
        .map(|sink| sink.output)
}

/// Complete cell text, following the existing cached-HTML projection without a
/// DOM. Fragment boundaries are not text-node boundaries: raw tags and decoded
/// Markdown text can belong to the same node.
#[derive(Default)]
pub(super) struct CellText {
    output: String,
    text: String,
    pre_newline: bool,
    literal_cr: bool,
}
impl CellText {
    fn flush(&mut self) {
        if self.text.is_empty() {
            return;
        }
        // The HTML tree builder removes the first LF after <pre>; the existing
        // native projection separately removes one formatting LF after a break.
        let text = if self.pre_newline {
            self.text.strip_prefix('\n').unwrap_or(&self.text)
        } else {
            &self.text
        };
        let text = if self.output.ends_with('\n') {
            text.strip_prefix("\r\n")
                .or_else(|| text.strip_prefix('\n'))
                .unwrap_or(text)
        } else {
            text
        };
        self.output.push_str(text);
        self.text.clear();
        self.pre_newline = false;
    }
    /// A real HTML/formatting boundary, not merely another parser event.
    pub(super) fn boundary(&mut self) {
        self.flush();
        self.pre_newline = false;
        self.literal_cr = false;
    }
    /// Parser-decoded literal text. Never parse angle brackets or entities here.
    pub(super) fn push_text(&mut self, text: &str) {
        // HTML serialization preserves raw CR; HTML input preprocessing turns
        // CR/CRLF into LF, including a CRLF split across adjacent Text events.
        for ch in text.chars() {
            if ch == '\n' && self.literal_cr {
                self.literal_cr = false;
                continue;
            }
            self.literal_cr = ch == '\r';
            self.text.push(if self.literal_cr { '\n' } else { ch });
        }
    }
    pub(super) fn push_code(&mut self, text: &str) {
        self.boundary();
        self.push_text(text);
        self.boundary();
    }
    /// Markdown HardBreak is serialized as <br /> followed by a formatting LF.
    pub(super) fn push_break(&mut self) {
        self.boundary();
        self.output.push('\n');
        self.push_text("\n");
    }
    /// Already sanitized/projected inline HTML only; this is not a sanitizer.
    pub(super) fn push_html(&mut self, source: &str) {
        struct Sink<'a>(&'a mut CellText);
        impl TokenSink for Sink<'_> {
            type Handle = ();
            fn process_token(&mut self, token: Token, _line: u64) -> TokenSinkResult<()> {
                match token {
                    Token::CharacterTokens(text) => {
                        // Entity CRs have already passed input preprocessing and
                        // must not be normalized a second time.
                        self.0.text.push_str(&text);
                        self.0.literal_cr = false;
                    }
                    Token::ParseError(_) | Token::EOFToken => {}
                    token => {
                        self.0.boundary();
                        if let Token::TagToken(tag) = token {
                            match tag.name.as_ref() {
                                "br" => self.0.output.push('\n'),
                                "pre" if tag.kind == TagKind::StartTag => self.0.pre_newline = true,
                                "input" if tag.kind == TagKind::StartTag => {
                                    self.0.output.push_str(
                                        if tag
                                            .attrs
                                            .iter()
                                            .any(|a| a.name.local.as_ref() == "checked")
                                        {
                                            "[x] "
                                        } else {
                                            "[ ] "
                                        },
                                    );
                                }
                                _ => {}
                            }
                        }
                    }
                }
                TokenSinkResult::Continue
            }
        }
        let mut tokenizer = Tokenizer::new(Sink(self), Default::default());
        let mut input = BufferQueue::new();
        input.push_back(StrTendril::from_slice(source));
        let _ = tokenizer.feed(&mut input);
        tokenizer.end();
    }
    pub(super) fn finish(mut self) -> String {
        self.flush();
        self.output
    }
}
#[cfg(test)]
fn plain_fragment(source: &str) -> String {
    let mut text = CellText::default();
    text.push_html(source);
    text.finish()
}
pub(super) fn scopes(source: &str) -> Vec<(Scope, bool)> {
    tokenize(source).map(|sink| sink.scopes).unwrap_or_default()
}

// The tokenizer does not expose attribute byte offsets. Callers retain the
// enclosing parser span and a per-fragment occurrence, with approximate precision.
// References are returned only after the entire fragment has been accepted.
pub(super) fn web_fragment(
    source: &str,
    base: usize,
    cell: bool,
) -> std::io::Result<Option<WebFragment>> {
    let mut tokenizer = Tokenizer::new(
        Projection {
            web_base: Some(base),
            ..Default::default()
        },
        Default::default(),
    );
    let mut input = BufferQueue::new();
    input.push_back(StrTendril::from_slice(source));
    let _ = tokenizer.feed(&mut input);
    tokenizer.end();
    let sink = tokenizer.sink;
    if sink.resource_budget {
        return Err(super::invalid(
            "HTML references exceed their count/target budget",
        ));
    }
    Ok((!sink.unsupported
        && sink.output.len() <= super::MAX_HTML
        && (!cell || !sink.table_structure))
        .then_some(WebFragment {
            html: sink.output,
            references: sink.references,
            anchors: sink.anchors,
        }))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn complete_cell_plain_text_decodes_entities_and_owned_breaks() {
        assert_eq!(
            plain_fragment("<strong>汉字 &amp; 😀</strong><br />\n<code>x &lt; y</code>"),
            "汉字 & 😀\nx < y"
        );
        assert_eq!(plain_fragment("<br>&#10;&amp;tail"), "\n&tail");
        assert_eq!(
            plain_fragment("<input checked disabled><input disabled>"),
            "[x] [ ] "
        );
        assert_eq!(
            plain_fragment("<a href=\"#\" data-link-reference=\"4\">visible</a><!-- hidden -->"),
            "visible"
        );
        assert_eq!(
            plain_fragment("<span>[image: <em>alt</em>]</span>"),
            "[image: alt]"
        );
        assert_eq!(plain_fragment("<pre>\nbody</pre>"), "body");
        assert_eq!(
            plain_fragment(&"<em>v</em> ".repeat(6000)),
            "v ".repeat(6000)
        );
    }
    #[test]
    fn cell_text_keeps_dom_text_nodes_across_parser_fragments() {
        let mut text = CellText::default();
        text.push_html("<br>");
        text.push_text("\n");
        text.push_text("\ntail");
        assert_eq!(text.finish(), "\n\ntail");

        let mut text = CellText::default();
        text.push_html("<pre>");
        text.push_text("\nbody");
        text.push_html("</pre>");
        text.push_code("<b>&amp;</b>");
        assert_eq!(text.finish(), "body<b>&amp;</b>");

        let mut text = CellText::default();
        text.push_break();
        text.push_text("\ntail");
        assert_eq!(text.finish(), "\n\ntail", "only the writer's LF is removed");

        let mut text = CellText::default();
        text.push_text("one\r");
        text.push_text("\ntwo\rthree");
        assert_eq!(text.finish(), "one\ntwo\nthree");
    }

    #[test]
    fn complete_cell_text_matches_existing_cached_html_projection() {
        use crate::core::{self, blocks};
        use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
        // Independent oracle: the existing HTML tree + native Span projection,
        // not the new full-value sidecar or the bounded fallback renderer.
        for body in [
            "<br>&#10;tail",
            "<br>&#10;&#10;tail",
            "<pre>&#10;tail</pre>",
            "<br><pre>&#10;&#10;tail</pre>",
            "<br>*&#10;*&#10;tail",
            "<br>`<b>&amp;</b>`",
            "a &amp; 汉😀 <strong>b</strong><br>tail",
            "<br>&#10;<!-- boundary -->&#10;tail",
        ] {
            let source = format!("| Header |\n| --- |\n| {body} |\n");
            let root = tempfile::tempdir().unwrap();
            let path = root.path().join("input.md");
            let cache = root.path().join("cache");
            std::fs::create_dir(&cache).unwrap();
            std::fs::write(&path, &source).unwrap();
            let index = core::build_index(&path, &cache).unwrap();
            let expected = blocks::read_blocks(&cache, &index, 0)
                .unwrap()
                .into_iter()
                .find_map(|block| match block {
                    blocks::Block::Table { rows, .. } => Some(
                        rows[0][0]
                            .iter()
                            .map(|span| span.text.as_str())
                            .collect::<String>(),
                    ),
                    _ => None,
                })
                .unwrap();
            let mut text = CellText::default();
            let mut cell = 0;
            let mut inside = false;
            for event in Parser::new_ext(
                &source,
                Options::ENABLE_TABLES | Options::ENABLE_TASKLISTS | Options::ENABLE_STRIKETHROUGH,
            ) {
                let event = core::inert_cell_event(event);
                match event {
                    Event::Start(Tag::TableCell) => {
                        cell += 1;
                        inside = cell == 2;
                    }
                    Event::End(TagEnd::TableCell) => inside = false,
                    event if inside => match event {
                        Event::Text(value) => text.push_text(&value),
                        Event::Code(value) => text.push_code(&value),
                        Event::Html(value) | Event::InlineHtml(value) => text.push_html(&value),
                        Event::SoftBreak | Event::HardBreak => text.push_break(),
                        Event::Start(_) | Event::End(_) => text.boundary(),
                        _ => {}
                    },
                    _ => {}
                }
            }
            assert_eq!(text.finish(), expected, "{body}");
        }
    }
    #[test]
    fn inline_boundaries_are_not_closed_by_fragment_parsing() {
        assert_eq!(fragment("<strong>"), Some("<strong>".into()));
        assert_eq!(fragment("</strong>"), Some("</strong>".into()));
        assert_eq!(
            fragment("<div><em>中文 &amp; 😀</em></div>"),
            Some("<div><em>中文 &amp; 😀</em></div>".into())
        );
    }
    #[test]
    fn untrusted_attributes_cannot_claim_source_or_host_identity() {
        let html = fragment("<span id=source-view class=reader-shell data-source-line=1 onclick=bad() style='position:fixed' title='a &quot;b&quot;'>safe</span>").unwrap();
        assert_eq!(html, "<span title=\"a &quot;b&quot;\">safe</span>");
        assert_eq!(
            fragment("<a href='javascript&colon;alert(1)' target=_blank>bad</a>").unwrap(),
            "<a rel=\"noopener noreferrer\">bad</a>"
        );
        assert!(
            fragment("<a href='HTTPS://example.org/?a=1&amp;b=2'>link</a>")
                .unwrap()
                .contains("href=\"HTTPS://example.org/?a=1&amp;b=2\"")
        );
    }
    #[test]
    fn unsafe_markup_and_work_limits_retain_the_inert_fallback() {
        for html in [
            "<script>alert(1)</script>",
            "<img src=file:///secret>",
            "<iframe></iframe>",
            "<svg><a></a></svg>",
        ] {
            assert!(fragment(html).is_none());
        }
        assert!(fragment(&"x".repeat(super::super::MAX_HTML + 1)).is_none());
        assert!(fragment(&"<b></b>".repeat(super::super::MAX_EVENTS)).is_none());
        assert_eq!(
            fragment("<details open><summary>More</summary><p>body</p></details>"),
            Some("<details open=\"\"><summary>More</summary><p>body</p></details>".into())
        );
    }
}
