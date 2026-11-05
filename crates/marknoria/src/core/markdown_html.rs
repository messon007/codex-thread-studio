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
                            || (name == "a" && key == "href" && href(value))
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
pub(super) fn scopes(source: &str) -> Vec<(Scope, bool)> {
    tokenize(source).map(|sink| sink.scopes).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
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
