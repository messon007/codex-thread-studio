//! Bounded, toolkit-independent formatted pages for native full-cell readers.
//! Consume the parser's existing styles/math; never parse Markdown a second time.
use crate::{inline::Style, RichText};
use std::collections::{BTreeMap, BTreeSet};

pub const PAGE_BYTES: usize = 4096;
pub const TEXT_BYTES: usize = 512 * 1024;
const MARKUP_BYTES: usize = 4 * 1024 * 1024;
const PAGE_MARKUP_BYTES: usize = 64 * 1024;

#[derive(Debug)]
pub struct Pages {
    pub pages: Vec<RichText>,
    /// Original Unicode-scalar ranges, prepared alongside the bounded labels.
    pub characters: Vec<std::ops::Range<usize>>,
}
#[derive(Clone, Copy)]
enum Change {
    Style(usize, i32),
    Link(usize, bool),
}

pub fn prepare(content: &RichText) -> Result<Pages, String> {
    if content.source_only || content.plain.len() > TEXT_BYTES {
        return Err("Formatted cell exceeds the paged label budget".into());
    }
    if content.plain.contains('\r') {
        return Err("Formatted cell requires source-preserving text layout".into());
    }
    let attributes = content
        .attributes
        .as_ref()
        .ok_or("Formatted cell has no prepared attributes")?;
    if attributes.text != content.plain {
        return Err("Formatted cell attributes do not match its text".into());
    }
    for math in &content.math {
        if math.range.start >= math.range.end
            || content.plain.get(math.range.clone()).is_none()
            || math.body.start < math.range.start
            || math.body.end > math.range.end
            || content.plain.get(math.body.clone()).is_none()
        {
            return Err("Formatted cell has invalid formula offsets".into());
        }
    }
    let mut pages = Vec::new();
    let mut ranges = Vec::new();
    let mut start = 0;
    let mut character = 0;
    let mut markup_bytes = 0usize;
    while start < content.plain.len() {
        let mut end = (start + PAGE_BYTES).min(content.plain.len());
        while !content.plain.is_char_boundary(end) {
            end -= 1;
        }
        if end < content.plain.len() {
            if let Some((offset, c)) = content.plain[start..end]
                .char_indices()
                .rev()
                .find(|(_, c)| c.is_whitespace())
                .filter(|(offset, _)| *offset >= PAGE_BYTES / 2)
            {
                end = start + offset + c.len_utf8();
            }
        }
        // A formula is atomic. Move it to the next page or dedicate one bounded
        // page to it, instead of rendering a truncated TeX expression.
        for math in &content.math {
            if math.range.start < end && math.range.end > end {
                end = if math.range.start > start {
                    math.range.start
                } else {
                    math.range.end
                };
                break;
            }
        }
        if end <= start
            || end > content.plain.len()
            || !content.plain.is_char_boundary(end)
            || end - start > crate::inline::MATH_SOURCE_BYTES + 4
        {
            return Err("Formatted cell formula exceeds the page budget".into());
        }
        if end < content.plain.len()
            && content.plain.as_bytes()[end - 1] == b'\r'
            && content.plain.as_bytes()[end] == b'\n'
        {
            end -= 1;
        }
        let text = &content.plain[start..end];
        let mut offsets = text.char_indices().map(|(b, _)| b).collect::<Vec<_>>();
        offsets.push(text.len());
        let characters = offsets.len() - 1;
        let mut changes: BTreeMap<usize, Vec<Change>> = BTreeMap::new();
        changes.entry(0).or_default();
        changes.entry(text.len()).or_default();
        // Reuse the existing page index to avoid scanning every style/link for
        // each short label page. Adjacent index pages can share a span.
        let mut spans = BTreeSet::new();
        let mut links = BTreeSet::new();
        for part in attributes
            .pages
            .iter()
            .filter(|p| p.start < end && p.end > start)
        {
            spans.extend(part.spans.iter().copied());
            links.extend(part.links.iter().copied());
        }
        for i in spans {
            let span = &attributes.spans[i];
            let a = span.start.saturating_sub(character).min(characters);
            let b = span.end.saturating_sub(character).min(characters);
            if a == b {
                continue;
            }
            let id = match span.style {
                Style::Bold => 0,
                Style::Italic => 1,
                Style::Strike => 2,
                Style::Code => 3,
            };
            changes
                .entry(offsets[a])
                .or_default()
                .push(Change::Style(id, 1));
            changes
                .entry(offsets[b])
                .or_default()
                .push(Change::Style(id, -1));
        }
        for i in links {
            let link = &attributes.links[i];
            if !crate::markup::safe_link(&link.destination, false) {
                continue;
            }
            let a = link.start.saturating_sub(character).min(characters);
            let b = link.end.saturating_sub(character).min(characters);
            if a == b {
                continue;
            }
            changes
                .entry(offsets[a])
                .or_default()
                .push(Change::Link(i, true));
            changes
                .entry(offsets[b])
                .or_default()
                .push(Change::Link(i, false));
        }
        let boundaries = changes.into_iter().collect::<Vec<_>>();
        let mut styles = [0i32; 4];
        let mut links = BTreeSet::new();
        let mut markup = String::new();
        for pair in boundaries.windows(2) {
            for change in &pair[0].1 {
                match *change {
                    Change::Style(i, n) => styles[i] += n,
                    Change::Link(i, true) => {
                        links.insert(i);
                    }
                    Change::Link(i, false) => {
                        links.remove(&i);
                    }
                }
            }
            if let Some(i) = links.first() {
                markup.push_str("<a href=\"");
                markup.push_str(&crate::markup::escape(&attributes.links[*i].destination));
                markup.push_str("\">");
            }
            let tags = [
                ("<b>", "</b>"),
                ("<i>", "</i>"),
                ("<s>", "</s>"),
                ("<tt>", "</tt>"),
            ];
            for (i, (open, _)) in tags.iter().enumerate() {
                if styles[i] > 0 {
                    markup.push_str(open);
                }
            }
            markup.push_str(&crate::markup::escape(&text[pair[0].0..pair[1].0]));
            for (i, (_, close)) in tags.iter().enumerate().rev() {
                if styles[i] > 0 {
                    markup.push_str(close);
                }
            }
            if !links.is_empty() {
                markup.push_str("</a>");
            }
            if markup.len() > PAGE_MARKUP_BYTES
                || markup.len().saturating_add(markup_bytes) > MARKUP_BYTES
            {
                return Err("Formatted cell markup exceeds the page budget".into());
            }
        }
        let math = content
            .math
            .iter()
            .filter(|m| m.range.start >= start && m.range.end <= end)
            .map(|m| {
                let mut m = m.clone();
                m.range = m.range.start - start..m.range.end - start;
                m.body = m.body.start - start..m.body.end - start;
                m.characters = m.characters.start - character..m.characters.end - character;
                m
            })
            .collect();
        markup_bytes += markup.len();
        ranges.push(character..character + characters);
        pages.push(RichText {
            source_only: false,
            plain: text.to_owned(),
            markup,
            images: Vec::new(),
            math,
            attributes: None,
        });
        character += characters;
        start = end;
    }
    Ok(Pages {
        pages,
        characters: ranges,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn cell(source: &str) -> RichText {
        let blocks = crate::parse_text(&format!("| Cell |\n|---|\n| {source} |\n")).unwrap();
        let crate::BlockKind::Table { rows, .. } = &blocks[0].kind else {
            panic!("No table")
        };
        rows[0].cells[0].content.as_ref().clone()
    }
    #[test]
    fn unicode_styles_links_math_and_exact_text_survive_pages() {
        let text = cell(&format!(
            "**{} 中文😀 $x^2$ [safe](docs/a.md?x=1&y=2) & <b>literal</b>** tail",
            "word ".repeat(2000)
        ));
        let pages = prepare(&text).unwrap();
        assert!(pages.pages.len() > 2);
        let original: Vec<_> = text.plain.chars().collect();
        assert_eq!(pages.characters.len(), pages.pages.len());
        for (page, range) in pages.pages.iter().zip(&pages.characters) {
            assert_eq!(
                original[range.clone()].iter().collect::<String>(),
                page.plain
            );
        }
        assert_eq!(
            pages
                .pages
                .iter()
                .map(|p| p.plain.as_str())
                .collect::<String>(),
            text.plain
        );
        assert!(pages
            .pages
            .iter()
            .any(|p| p.markup.contains("href=\"docs/a.md?x=1&amp;y=2\"")));
        assert!(pages
            .pages
            .iter()
            .any(|p| p.markup.contains("&lt;b&gt;literal&lt;/b&gt;")));
        assert_eq!(pages.pages.iter().map(|p| p.math.len()).sum::<usize>(), 1);
        for page in &pages.pages {
            assert!(page.plain.len() <= PAGE_BYTES);
            assert!(page.markup.contains("<b>"));
            for math in &page.math {
                assert_eq!(&page.plain[math.body.clone()], "x^2");
                assert_eq!(
                    page.plain[..math.range.start].chars().count(),
                    math.characters.start
                );
                assert_eq!(math.source, text.math[0].source);
            }
        }
    }
    #[test]
    fn formula_crossing_a_page_is_never_split_and_byte_zero_is_valid() {
        let text = cell(&format!(
            "{} $\\frac{{a}}{{b}}$ {}",
            "a".repeat(PAGE_BYTES - 8),
            "tail ".repeat(2000)
        ));
        let pages = prepare(&text).unwrap();
        assert_eq!(pages.pages[0].plain.len(), PAGE_BYTES - 7);
        assert!(pages.pages[1].plain.starts_with("$\\frac{a}{b}$"));
        assert_eq!(pages.pages[1].math[0].range.start, 0);
        assert_eq!(
            pages
                .pages
                .iter()
                .map(|p| p.plain.as_str())
                .collect::<String>(),
            text.plain
        );
    }
    #[test]
    fn formatting_budget_and_source_only_are_explicit() {
        let mut text = cell(&format!("$x$ {}", "a".repeat(9000)));
        text.plain.push('\r');
        assert!(prepare(&text).unwrap_err().contains("source-preserving"));
        text.plain.pop();
        text.source_only = true;
        assert!(prepare(&text).is_err());
        text.source_only = false;
        text.plain = "x".repeat(TEXT_BYTES + 1);
        assert!(prepare(&text).is_err());
    }
    #[test]
    fn long_formulas_get_an_atomic_page_and_bad_offsets_are_rejected() {
        let mut text = cell(&format!(
            "before ${}$ {}",
            "z".repeat(5000),
            "tail ".repeat(1700)
        ));
        let pages = prepare(&text).unwrap();
        let math_page = pages.pages.iter().find(|p| !p.math.is_empty()).unwrap();
        assert_eq!(math_page.math[0].range, 0..5002);
        assert_eq!(math_page.plain.len(), 5002);
        assert_eq!(
            pages
                .pages
                .iter()
                .map(|p| p.plain.as_str())
                .collect::<String>(),
            text.plain
        );
        text.math[0].body.end = text.plain.len() + 1;
        assert!(prepare(&text).is_err());
    }
    #[test]
    fn repeated_long_links_cannot_build_a_large_gtk_markup_page() {
        let source = format!("[x](docs/{}.md)", "a".repeat(4000)).repeat(20);
        let text = cell(&source);
        assert_eq!(text.plain.len(), 20);
        assert!(text.attributes.is_some());
        assert!(prepare(&text).unwrap_err().contains("markup"));
    }
}
