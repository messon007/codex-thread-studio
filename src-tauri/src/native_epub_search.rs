//! Paged full-book text search. No renderer, archive extraction or whole-book DOM.
use super::*;
use html5ever::{
    tendril::StrTendril,
    tokenizer::{
        states::RawKind, BufferQueue, StartTag, Token, TokenSink, TokenSinkResult, Tokenizer,
        TokenizerOpts,
    },
};
const CHAPTER_BYTES: usize = 8 * 1024 * 1024;
const PAGE_BYTES: usize = 16 * 1024 * 1024;
const PAGE_CHAPTERS: usize = 16;
const MAX_COUNT: usize = 10_000;
#[derive(Default)]
struct Text {
    xml: bool,
    value: String,
    stack: Vec<(String, bool)>,
    suppressed: bool,
    space: bool,
    exceeded: bool,
}
fn block(tag: &str) -> bool {
    matches!(
        tag,
        "address"
            | "article"
            | "aside"
            | "blockquote"
            | "br"
            | "div"
            | "dl"
            | "dt"
            | "dd"
            | "figcaption"
            | "figure"
            | "footer"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "header"
            | "hr"
            | "li"
            | "main"
            | "nav"
            | "ol"
            | "p"
            | "pre"
            | "section"
            | "table"
            | "td"
            | "th"
            | "tr"
            | "ul"
    )
}
impl TokenSink for Text {
    type Handle = ();
    fn adjusted_current_node_present_but_not_in_html_namespace(&self) -> bool {
        self.xml
            || self
                .stack
                .iter()
                .any(|(name, _)| matches!(name.as_str(), "svg" | "math"))
    }
    fn process_token(&mut self, token: Token, _: u64) -> TokenSinkResult<()> {
        if self.exceeded {
            return TokenSinkResult::Continue;
        }
        match token {
            Token::CharacterTokens(value) if !self.suppressed => {
                for ch in value.chars() {
                    if ch.is_ascii_whitespace() {
                        self.space = !self.value.is_empty();
                    } else {
                        if self.space {
                            self.value.push(' ');
                            self.space = false;
                        }
                        self.value.push(ch);
                    }
                    if self.value.len() > CHAPTER_BYTES {
                        self.exceeded = true;
                        break;
                    }
                }
            }
            Token::TagToken(tag) => {
                let name = tag.name.as_ref();
                if block(name) && !self.suppressed {
                    self.space = !self.value.is_empty();
                }
                if tag.kind == StartTag {
                    let skip = self.suppressed
                        || matches!(
                            name,
                            "head"
                                | "title"
                                | "script"
                                | "style"
                                | "template"
                                | "noscript"
                                | "iframe"
                                | "object"
                                | "form"
                                | "button"
                                | "textarea"
                                | "select"
                        )
                        || tag.attrs.iter().any(|a| a.name.local.as_ref() == "hidden");
                    if !tag.self_closing
                        && !matches!(
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
                    {
                        if self.stack.len() >= 256 {
                            self.exceeded = true;
                        } else {
                            self.stack.push((name.to_owned(), self.suppressed));
                            self.suppressed = skip;
                        }
                    }
                    if !tag.self_closing
                        && matches!(name, "script" | "style" | "iframe" | "noscript")
                    {
                        return TokenSinkResult::RawData(if name == "script" {
                            RawKind::ScriptData
                        } else {
                            RawKind::Rawtext
                        });
                    }
                    if !tag.self_closing && matches!(name, "textarea" | "title") {
                        return TokenSinkResult::RawData(RawKind::Rcdata);
                    }
                } else if let Some(index) = self.stack.iter().rposition(|(n, _)| n == name) {
                    self.suppressed = self.stack[index].1;
                    self.stack.truncate(index);
                }
            }
            _ => {}
        }
        TokenSinkResult::Continue
    }
}
fn extract(bytes: &[u8], xml: bool, alive: impl Fn() -> bool) -> Result<String, Error> {
    let source = std::str::from_utf8(bytes).map_err(|_| bad("Section text is not UTF-8"))?;
    if bytes.len() > CHAPTER_BYTES
        || source.bytes().filter(|b| *b == b'<').take(100_001).count() > 100_000
    {
        return Err(bad("Section exceeds the search markup budget"));
    }
    let mut tokenizer = Tokenizer::new(
        Text {
            xml,
            ..Text::default()
        },
        TokenizerOpts::default(),
    );
    let mut start = 0;
    while start < source.len() {
        if !alive() {
            return Err(gone());
        }
        let mut end = (start + 16384).min(source.len());
        while !source.is_char_boundary(end) {
            end -= 1;
        }
        let mut input = BufferQueue::new();
        input.push_back(StrTendril::from(&source[start..end]));
        let _ = tokenizer.feed(&mut input);
        if tokenizer.sink.exceeded {
            return Err(bad("Section exceeds the search text/depth budget"));
        }
        start = end;
    }
    tokenizer.end();
    if tokenizer.sink.exceeded {
        return Err(bad("Section exceeds the search text/depth budget"));
    }
    Ok(tokenizer.sink.value)
}
fn snippet(text: &str, start: usize, end: usize) -> String {
    let mut first = start.saturating_sub(96);
    while !text.is_char_boundary(first) {
        first += 1;
    }
    // Unicode simple-case folding can match more UTF-8 bytes than the query
    // (for example ASCII k versus Kelvin signs). Bound the excerpt itself.
    let mut last = (end + 192).min(text.len()).min(first + 800);
    while !text.is_char_boundary(last) {
        last -= 1;
    }
    format!(
        "{}{}{}",
        if first > 0 { "…" } else { "" },
        &text[first..last],
        if last < text.len() { "…" } else { "" }
    )
}
pub(super) fn run(
    books: &Books,
    id: &str,
    query: &str,
    case_sensitive: bool,
    start: usize,
) -> Result<Value, Error> {
    if query.is_empty()
        || query.len() > 512
        || query.chars().all(char::is_whitespace)
        || query.contains('\0')
    {
        return Err(bad("Enter 1–512 bytes of literal search text"));
    }
    let book = books.book(id)?;
    if start >= book.manifest.spine.len() {
        return Err(bad("Invalid EPUB search section"));
    }
    let expression = regex::RegexBuilder::new(&regex::escape(query))
        .case_insensitive(!case_sensitive)
        .size_limit(2 * 1024 * 1024)
        .build()
        .map_err(bad)?;
    let mut archive = book.archive.clone();
    let mut sections = Vec::new();
    let mut issues = Vec::new();
    let mut expanded = 0;
    let mut next = start;
    while next < book.manifest.spine.len() && next - start < PAGE_CHAPTERS && expanded < PAGE_BYTES
    {
        books.book(id)?;
        let index = next;
        let chapter = &book.manifest.spine[index];
        let declared = book
            .entries
            .get(&chapter.href)
            .and_then(|index| {
                archive
                    .by_index(*index)
                    .ok()
                    .map(|entry| entry.size().min(CHAPTER_BYTES as u64) as usize)
            })
            .unwrap_or(0);
        if next > start && expanded.saturating_add(declared) > PAGE_BYTES {
            break;
        }
        expanded += declared;
        next += 1;
        let result = read_entry(&mut archive, &book.entries, &chapter.href, CHAPTER_BYTES)
            .and_then(|bytes| {
                extract(
                    &bytes,
                    book.resources
                        .get(&chapter.href)
                        .is_some_and(|mime| mime != "text/html"),
                    || books.cache.lock().is_ok_and(|c| c.books.contains_key(id)),
                )
            });
        let text = match result {
            Ok(text) => text,
            Err((status, error)) => {
                if status == StatusCode::GONE {
                    return Err(gone());
                }
                issues.push(json!({"chapter":index,"label":chapter.label,"error":error}));
                continue;
            }
        };
        let mut found = expression.find_iter(&text);
        let Some(first) = found.next() else {
            continue;
        };
        let count = 1 + found.by_ref().take(MAX_COUNT - 1).count();
        let limited = found.next().is_some();
        sections.push(json!({"chapter":index,"href":chapter.href,"label":chapter.label,"count":count,"limited":limited,"snippet":snippet(&text,first.start(),first.end())}));
    }
    books.book(id)?;
    Ok(
        json!({"bookHash":book.manifest.hash,"query":query,"caseSensitive":case_sensitive,"start":start,"scanned":next-start,"total":book.manifest.spine.len(),"next":if next<book.manifest.spine.len(){Some(next)}else{None},"sections":sections,"issues":issues}),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn html_text_decodes_entities_joins_inline_and_excludes_nonreading_content() {
        let source=br#"<!doctype html><html><head><title>secret</title><style>secret</style></head><body><p>Needle <em>and</em> native &amp; &#x4E2D;&#x6587;.</p><script>if (a < b) secret</script><p hidden>secret</p><template><p>secret</p></template><p>next</p></body></html>"#;
        assert_eq!(
            extract(source, false, || true).unwrap(),
            "Needle and native & 中文. next"
        );
        assert!(extract(source, false, || false).is_err());
        assert!(extract(&[255], true, || true).is_err());
        assert!(extract("<div>".repeat(257).as_bytes(), false, || true).is_err());
        assert_eq!(
            extract(
                b"<svg><text><![CDATA[Needle & literal]]></text></svg>",
                true,
                || true
            )
            .unwrap(),
            "Needle & literal"
        );
        assert_eq!(
            extract(
                b"<html><body><script/><p>Needle</p></body></html>",
                true,
                || true
            )
            .unwrap(),
            "Needle"
        );
    }
    #[test]
    fn snippets_preserve_unicode_and_trailing_spaces() {
        let value = format!("{}needle 末尾😀{}", "字😀".repeat(100), "字😀".repeat(100));
        let at = value.find("needle ").unwrap();
        let snippet = snippet(&value, at, at + 7);
        assert!(snippet.contains("needle 末尾😀"));
        assert!(snippet.len() < 800);
        let folded = "K".repeat(512);
        let clipped = super::snippet(&folded, 0, folded.len());
        assert!(clipped.len() <= 806);
        assert!(clipped.ends_with('…'));
    }
}
