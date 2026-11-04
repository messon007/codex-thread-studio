//! Presentation-neutral inline attributes for scrollable native rich text.
//! Markdown parsing and Unicode offset preparation belong on Rust workers.
use pulldown_cmark::{Event, Tag, TagEnd};
use std::ops::Range;

pub const SCROLLABLE_TEXT_BYTES: usize = 8 * 1024;
pub const PAGE_BYTES: usize = 8 * 1024;
pub const MATH_SOURCE_BYTES: usize = 16 * 1024;
pub const MATH_SPANS: usize = 256;

/// Math candidates retain the original text; renderers must not replace source
/// characters with object placeholders or lose the original selection offsets.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Math {
    /// UTF-8 byte range in RichText::plain, including delimiters.
    pub range: Range<usize>,
    /// UTF-8 formula body in RichText::plain, excluding delimiters.
    pub body: Range<usize>,
    /// Unicode scalar range in RichText::plain, including delimiters.
    pub characters: Range<usize>,
    pub source: crate::SourceRange,
    pub display: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    Bold,
    Italic,
    Strike,
    Code,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub style: Style,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Link {
    pub start: usize,
    pub end: usize,
    pub destination: String,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Attributes {
    /// Prepared text contains literal carriage returns. Markup-based consumers
    /// must use an exact-text layout instead of normalizing them through XML.
    pub carriage_returns: bool,
    /// At least one retained formula spans a hard line break.
    pub multiline_math: bool,
    pub spans: Vec<Span>,
    pub links: Vec<Link>,
    pub text: String,
    pub pages: Vec<Page>,
    pub sources: Vec<SourceSpan>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceSpan {
    pub source: Range<usize>,
    pub rendered: Range<usize>,
    pub direct: bool,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Match {
    pub page: usize,
    pub characters: Range<usize>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Page {
    pub start: usize,
    pub end: usize,
    pub character: usize,
    pub characters: usize,
    pub spans: Vec<usize>,
    pub links: Vec<usize>,
    /// Physical source lines for literal pages; rich pages use source spans.
    pub lines: Option<(usize, usize)>,
}
impl Attributes {
    pub fn heap_bytes(&self) -> usize {
        self.spans.capacity() * std::mem::size_of::<Span>()
            + self.links.capacity() * std::mem::size_of::<Link>()
            + self
                .links
                .iter()
                .map(|link| link.destination.capacity())
                .sum::<usize>()
            + self.text.capacity()
            + self.pages.capacity() * std::mem::size_of::<Page>()
            + self.sources.capacity() * std::mem::size_of::<SourceSpan>()
            + self
                .pages
                .iter()
                .map(|p| (p.spans.capacity() + p.links.capacity()) * std::mem::size_of::<usize>())
                .sum::<usize>()
    }
    /// Resolve the exact original source occurrence, never another equal quote.
    /// Source byte offsets and rendered Unicode scalar offsets stay distinct.
    pub fn project(&self, source: &str, base: usize, range: Range<usize>) -> Option<Match> {
        if range.start >= range.end {
            return None;
        }
        let endpoint = |position: usize, end: bool| -> Option<usize> {
            let index = self
                .sources
                .partition_point(|span| span.source.start <= position)
                .checked_sub(1)?;
            let span = &self.sources[index];
            if position > span.source.end || !end && position == span.source.end {
                return None;
            }
            if span.direct {
                let prefix = source
                    .get(span.source.start.checked_sub(base)?..position.checked_sub(base)?)?;
                Some(span.rendered.start + prefix.chars().count())
            } else if position == span.source.start {
                Some(span.rendered.start)
            } else if position == span.source.end {
                Some(span.rendered.end)
            } else {
                None
            }
        };
        let start = endpoint(range.start, false)?;
        let end = endpoint(range.end, true)?;
        if start >= end {
            return None;
        }
        let byte = |offset: usize| {
            self.text
                .char_indices()
                .nth(offset)
                .map(|(index, _)| index)
                .or_else(|| (offset == self.text.chars().count()).then_some(self.text.len()))
        };
        let first = byte(start)?;
        let last = byte(end)?;
        if self.text.get(first..last)?
            != source.get(range.start.checked_sub(base)?..range.end.checked_sub(base)?)?
        {
            return None;
        }
        Some(Match {
            page: self
                .pages
                .partition_point(|page| page.start <= first)
                .saturating_sub(1),
            characters: start..end,
        })
    }
}
pub(crate) fn attributes<'a, 'b: 'a>(
    events: impl IntoIterator<Item = &'a Event<'b>>,
    text: &str,
) -> Attributes {
    attributes_with_math(events, text, &[])
}
pub(crate) fn attributes_with_math<'a, 'b: 'a>(
    events: impl IntoIterator<Item = &'a Event<'b>>,
    text: &str,
    math: &[Math],
) -> Attributes {
    let mut result = Attributes::default();
    result.carriage_returns = text.contains('\r');
    result.text = text.to_owned();
    let mut start = 0;
    let mut character = 0;
    while start < text.len() {
        let mut end = (start + PAGE_BYTES).min(text.len());
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        if end < text.len() {
            if let Some((position, c)) = text[start..end]
                .char_indices()
                .rev()
                .find(|(_, c)| c.is_whitespace())
            {
                if position >= PAGE_BYTES / 2 {
                    end = start + position + c.len_utf8();
                }
            }
            // Preserve a complete retained formula. Most pages stay at 8 KiB;
            // a single large expression can occupy its own bounded page.
            let i = math.partition_point(|m| m.range.start < end);
            if let Some(m) = i
                .checked_sub(1)
                .and_then(|i| math.get(i))
                .filter(|m| m.range.end > end)
            {
                end = if m.range.start > start {
                    m.range.start
                } else {
                    m.range.end
                };
            }
            if end < text.len()
                && text.as_bytes()[end - 1] == b'\r'
                && text.as_bytes()[end] == b'\n'
            {
                end -= 1;
            }
        }
        let characters = text[start..end].chars().count();
        result.pages.push(Page {
            start,
            end,
            character,
            characters,
            spans: Vec::new(),
            links: Vec::new(),
            lines: None,
        });
        start = end;
        character += characters;
    }
    if result.pages.is_empty() {
        result.pages.push(Page {
            start: 0,
            end: 0,
            character: 0,
            characters: 0,
            spans: Vec::new(),
            links: Vec::new(),
            lines: None,
        });
    }
    let mut offset = 0;
    let mut styles = Vec::new();
    let mut links: Vec<Option<(usize, String)>> = Vec::new();
    for event in events {
        match event {
            Event::Start(Tag::Strong) => styles.push((Style::Bold, offset)),
            Event::Start(Tag::Emphasis) => styles.push((Style::Italic, offset)),
            Event::Start(Tag::Strikethrough) => styles.push((Style::Strike, offset)),
            Event::End(TagEnd::Strong | TagEnd::Emphasis | TagEnd::Strikethrough) => {
                if let Some((style, start)) = styles.pop() {
                    if start < offset {
                        result.spans.push(Span {
                            start,
                            end: offset,
                            style,
                        });
                    }
                }
            }
            Event::Start(Tag::Link { dest_url, .. } | Tag::Image { dest_url, .. }) => {
                let safe = crate::markup::safe_link(dest_url, links.iter().any(Option::is_some));
                links.push(safe.then(|| (offset, dest_url.to_string())));
            }
            Event::End(TagEnd::Link | TagEnd::Image) => {
                if let Some(Some((start, destination))) = links.pop() {
                    if start < offset {
                        result.links.push(Link {
                            start,
                            end: offset,
                            destination,
                        });
                    }
                }
            }
            Event::Code(code) => {
                let start = offset;
                offset += code.chars().count();
                if start < offset {
                    result.spans.push(Span {
                        start,
                        end: offset,
                        style: Style::Code,
                    });
                }
            }
            Event::Text(text) | Event::Html(text) | Event::InlineHtml(text) => {
                offset += text.chars().count()
            }
            Event::SoftBreak | Event::HardBreak => offset += 1,
            Event::TaskListMarker(_) => offset += 2,
            _ => {}
        }
    }
    for page in &mut result.pages {
        let end = page.character + page.characters;
        page.spans = result
            .spans
            .iter()
            .enumerate()
            .filter(|(_, s)| s.start < end && s.end > page.character)
            .map(|(i, _)| i)
            .collect();
        page.links = result
            .links
            .iter()
            .enumerate()
            .filter(|(_, s)| s.start < end && s.end > page.character)
            .map(|(i, _)| i)
            .collect();
    }
    result
}

/// Reuse native text paging for literal Markdown after a formatting limit.
/// NUL is visible, while ordinary source occurrences retain exact byte/scalar
/// mapping. Extremely dense normalization uses a bounded unverified remainder.
pub(crate) fn literal(raw: &str, base: usize, line: usize) -> Attributes {
    let display = raw.replace('\0', "␀");
    let mut result = attributes(std::iter::empty(), &display);
    let mut next_line = line;
    for page in &mut result.pages {
        let text = &result.text[page.start..page.end];
        let newlines = text.bytes().filter(|b| *b == b'\n').count();
        page.lines = Some((
            next_line,
            next_line + newlines - usize::from(text.ends_with('\n')),
        ));
        next_line += newlines;
    }
    let (mut byte, mut character) = (0, 0);
    for part in raw.split_inclusive('\0') {
        if result.sources.len() >= 4094 {
            result.sources.push(SourceSpan {
                source: base + byte..base + raw.len(),
                rendered: character..raw.chars().count(),
                direct: false,
            });
            byte = raw.len();
            break;
        }
        let text = part.strip_suffix('\0').unwrap_or(part);
        let count = text.chars().count();
        if !text.is_empty() {
            result.sources.push(SourceSpan {
                source: base + byte..base + byte + text.len(),
                rendered: character..character + count,
                direct: true,
            });
        }
        byte += text.len();
        character += count;
        if part.ends_with('\0') {
            result.sources.push(SourceSpan {
                source: base + byte..base + byte + 1,
                rendered: character..character + 1,
                direct: false,
            });
            byte += 1;
            character += 1;
        }
    }
    debug_assert_eq!(byte, raw.len());
    result
}

/// Reuse the parser's own event offsets and borrowed text slices. No fuzzy
/// quote lookup, secondary Markdown parse, or source copy is needed.
pub(crate) fn source_map(
    events: &[(Event<'_>, Range<usize>)],
    input: &str,
    map: impl Fn(Range<usize>) -> Range<usize>,
) -> Vec<SourceSpan> {
    let mut output = Vec::new();
    let mut character = 0;
    for (event, source) in events {
        let (text, count) = match event {
            Event::Text(text) | Event::Code(text) | Event::Html(text) | Event::InlineHtml(text) => {
                (Some(text), text.chars().count())
            }
            Event::SoftBreak | Event::HardBreak => (None, 1),
            Event::TaskListMarker(_) => (None, 2),
            _ => continue,
        };
        if let Some(text) = text.filter(|text| !text.is_empty()) {
            let borrowed = if let pulldown_cmark::CowStr::Borrowed(value) = text {
                (value.as_ptr() as usize)
                    .checked_sub(input.as_ptr() as usize)
                    .and_then(|start| {
                        let end = start.checked_add(value.len())?;
                        (start >= source.start
                            && end <= source.end
                            && input.get(start..end) == Some(value))
                        .then_some(start..end)
                    })
            } else {
                None
            };
            let raw = input.get(source.clone()).unwrap_or("");
            let direct = borrowed.or_else(|| {
                let mut positions = raw.match_indices(text.as_ref());
                let (offset, _) = positions.next()?;
                positions
                    .next()
                    .is_none()
                    .then_some(source.start + offset..source.start + offset + text.len())
            });
            if let Some(source) = direct {
                output.push(SourceSpan {
                    source: map(source),
                    rendered: character..character + count,
                    direct: true,
                });
            } else if let Some(pieces) = matches!(event, Event::Code(_))
                .then(|| code_sources(raw, source.start, text.as_ref(), character, &map))
                .flatten()
            {
                output.extend(pieces);
            } else {
                output.push(SourceSpan {
                    source: map(source.clone()),
                    rendered: character..character + count,
                    direct: false,
                });
            }
        } else if count > 0 {
            let direct = input.get(source.clone()) == Some("\n") && count == 1;
            output.push(SourceSpan {
                source: map(source.clone()),
                rendered: character..character + count,
                direct,
            });
        }
        character += count;
    }
    output
}

/// Code spans normalize newlines and sometimes trim one surrounding space.
/// Verify that normalization against the parser's value before retaining ranges.
fn code_sources(
    raw: &str,
    base: usize,
    value: &str,
    character: usize,
    map: &impl Fn(Range<usize>) -> Range<usize>,
) -> Option<Vec<SourceSpan>> {
    code_source_lines(raw, base, value, character, map, true)
        .or_else(|| code_source_lines(raw, base, value, character, map, false))
}
fn code_source_lines(
    raw: &str,
    base: usize,
    value: &str,
    character: usize,
    map: &impl Fn(Range<usize>) -> Range<usize>,
    combine_crlf: bool,
) -> Option<Vec<SourceSpan>> {
    let ticks = raw.bytes().take_while(|b| *b == b'`').count();
    if ticks == 0
        || raw.bytes().rev().take_while(|b| *b == b'`').count() != ticks
        || raw.len() < ticks * 2
    {
        return None;
    }
    let body = &raw[ticks..raw.len() - ticks];
    let origin = base + ticks;
    let mut normalized = String::new();
    let mut pieces = Vec::new();
    let mut characters = 0;
    let mut start = 0;
    let mut positions = body.char_indices().peekable();
    while let Some((offset, c)) = positions.next() {
        if c != '\r' && c != '\n' {
            continue;
        }
        if start < offset {
            let text = &body[start..offset];
            let count = text.chars().count();
            pieces.push(SourceSpan {
                source: origin + start..origin + offset,
                rendered: characters..characters + count,
                direct: true,
            });
            characters += count;
            normalized.push_str(text);
        }
        let mut end = offset + c.len_utf8();
        if combine_crlf && c == '\r' && positions.peek().is_some_and(|(_, c)| *c == '\n') {
            let (next, c) = positions.next().unwrap();
            end = next + c.len_utf8();
        }
        pieces.push(SourceSpan {
            source: origin + offset..origin + end,
            rendered: characters..characters + 1,
            direct: false,
        });
        characters += 1;
        normalized.push(' ');
        start = end;
    }
    if start < body.len() {
        let text = &body[start..];
        let count = text.chars().count();
        pieces.push(SourceSpan {
            source: origin + start..origin + body.len(),
            rendered: characters..characters + count,
            direct: true,
        });
        characters += count;
        normalized.push_str(text);
    }
    let trim = if normalized == value {
        0
    } else if normalized.starts_with(' ')
        && normalized.ends_with(' ')
        && normalized.get(1..normalized.len().checked_sub(1)?) == Some(value)
    {
        1
    } else {
        return None;
    };
    let mut output = Vec::new();
    for mut piece in pieces {
        let start = piece.rendered.start.max(trim);
        let end = piece.rendered.end.min(characters - trim);
        if start >= end {
            continue;
        }
        if piece.direct {
            let text = &raw[piece.source.start - base..piece.source.end - base];
            let byte = |n: usize| text.char_indices().nth(n).map_or(text.len(), |(at, _)| at);
            let from = byte(start - piece.rendered.start);
            let to = byte(end - piece.rendered.start);
            piece.source = piece.source.start + from..piece.source.start + to;
        }
        piece.source = map(piece.source);
        piece.rendered = character + start - trim..character + end - trim;
        output.push(piece);
    }
    Some(output)
}

#[cfg(test)]
mod tests {
    #[test]
    fn literal_normalization_keeps_valid_source_projection_and_physical_lines() {
        let raw = "before\0 tail 中文😀\r\nnext";
        let attributes = super::literal(raw, 731, 40);
        assert!(!attributes.text.contains('\0'));
        assert!(attributes.text.contains('␀'));
        assert_eq!(attributes.pages[0].lines, Some((40, 41)));
        let start = raw.find("tail 中文😀").unwrap();
        let hit = attributes
            .project(raw, 731, 731 + start..731 + start + "tail 中文😀".len())
            .unwrap();
        assert_eq!(hit.characters.start, raw[..start].chars().count());
        assert!(attributes.project(raw, 731, 737..738).is_none());
        assert!(attributes.links.is_empty());
    }
    #[test]
    fn pathological_normalization_maps_are_bounded_without_false_highlights() {
        let raw = "x\0".repeat(20000);
        let attributes = super::literal(&raw, 0, 1);
        assert!(attributes.sources.len() <= 4096);
        assert_eq!(attributes.text.chars().count(), raw.chars().count());
        assert!(attributes
            .project(&raw, 0, raw.len() - 2..raw.len() - 1)
            .is_none());
        assert!(attributes
            .pages
            .iter()
            .all(|p| p.end - p.start <= super::PAGE_BYTES));
        let raw = format!("{}\r\nend", "x".repeat(super::PAGE_BYTES - 1));
        let attributes = super::literal(&raw, 0, 12);
        assert!(attributes
            .pages
            .windows(2)
            .all(|pages| !attributes.text[pages[0].start..pages[0].end].ends_with('\r')));
    }
    use super::*;
    use crate::{parse_text, BlockKind};
    fn selected(text: &str, start: usize, end: usize) -> String {
        text.chars().skip(start).take(end - start).collect()
    }
    #[test]
    fn source_occurrences_map_to_distinct_pages_without_quote_guessing() {
        let needle = "repeat中文😀";
        let source = format!(
            "**{needle}** {} [{needle}](docs/{needle}.md) {} `{needle}`",
            "context ".repeat(1400),
            "later ".repeat(1600)
        );
        let blocks = parse_text(&source).unwrap();
        let BlockKind::Paragraph(text) = &blocks[0].kind else {
            panic!("paragraph missing")
        };
        let layout = text.attributes.as_ref().unwrap();
        let matches = source
            .match_indices(needle)
            .map(|(at, _)| layout.project(&source, 0, at..at + needle.len()))
            .collect::<Vec<_>>();
        assert_eq!(matches.len(), 4);
        assert!(
            matches[2].is_none(),
            "Hidden link destinations must not target another equal visible quote"
        );
        let visible = matches.into_iter().flatten().collect::<Vec<_>>();
        assert_eq!(visible.len(), 3);
        assert!(visible.windows(2).all(|pair| pair[0].page < pair[1].page));
        for found in visible {
            assert_eq!(
                selected(&layout.text, found.characters.start, found.characters.end),
                needle
            );
        }
        assert_eq!(
            layout.project(&source, 0, source.len() + 1..source.len() + 2),
            None
        );
    }
    #[test]
    fn entities_escapes_and_code_keep_original_source_offsets() {
        let source = format!(
            "{}before &amp; **after😀** `code😀` \\*literal\\* end",
            "prefix ".repeat(1400)
        );
        let blocks = parse_text(&source).unwrap();
        let BlockKind::Paragraph(text) = &blocks[0].kind else {
            panic!("paragraph missing")
        };
        let layout = text.attributes.as_ref().unwrap();
        for needle in ["before", "after😀", "code😀", "literal"] {
            let start = source.find(needle).unwrap();
            let found = layout
                .project(&source, 0, start..start + needle.len())
                .unwrap();
            assert_eq!(
                selected(&layout.text, found.characters.start, found.characters.end),
                needle
            );
        }
        for needle in ["amp", "**after😀**", "`code😀`"] {
            let start = source.find(needle).unwrap();
            assert_eq!(
                layout.project(&source, 0, start..start + needle.len()),
                None
            );
        }
    }
    #[test]
    fn fragment_base_and_byte_preserving_diagrams_keep_projection_positions() {
        let source = format!(
            "@startuml\nA -> B\n@enduml\n\n{}**target中文😀**",
            "context ".repeat(1500)
        );
        let blocks = parse_text(&source).unwrap();
        let BlockKind::Paragraph(text) = &blocks.last().unwrap().kind else {
            panic!("paragraph missing")
        };
        let start = source.find("target中文😀").unwrap();
        let found = text
            .attributes
            .as_ref()
            .unwrap()
            .project(&source, 0, start..start + "target中文😀".len())
            .unwrap();
        assert!(found.page > 0);
        let fragment = crate::Fragment {
            page: 0,
            document_id: "fixture".into(),
            revision: "r".into(),
            metadata: crate::FragmentMetadata {
                start: 500,
                end: 500 + source.len(),
                line: 20,
                rows: 0,
                row_start: 0,
                table: None,
            },
            source: crate::SourceWindow {
                projection: Vec::new(),
                references: Vec::new(),
                start: 500,
                end: 500 + source.len(),
                line: 20,
                end_line: 24,
                text: source.clone(),
            },
            header: None,
        };
        let blocks = crate::parse(&fragment).unwrap();
        let BlockKind::Paragraph(text) = &blocks.last().unwrap().kind else {
            panic!("paragraph missing")
        };
        let shifted = text
            .attributes
            .as_ref()
            .unwrap()
            .project(
                &source,
                500,
                500 + start..500 + start + "target中文😀".len(),
            )
            .unwrap();
        assert_eq!(found, shifted);
    }
    #[test]
    fn normalized_multiline_code_maps_each_original_word_without_delimiters() {
        let source = format!(
            "{} `` first😀\r\nsecond second `` tail",
            "context ".repeat(1400)
        );
        let blocks = parse_text(&source).unwrap();
        let BlockKind::Paragraph(text) = &blocks[0].kind else {
            panic!("paragraph missing")
        };
        let layout = text.attributes.as_ref().unwrap();
        assert!(layout.text.contains("first😀  second second"));
        let mut previous = None;
        for (position, _) in source.match_indices("second") {
            let found = layout.project(&source, 0, position..position + 6).unwrap();
            assert_eq!(
                selected(&layout.text, found.characters.start, found.characters.end),
                "second"
            );
            if let Some(previous) = previous {
                assert_ne!(previous, found.characters.start);
            }
            previous = Some(found.characters.start);
        }
        let position = source.find("first😀").unwrap();
        assert!(layout
            .project(&source, 0, position..position + "first😀".len())
            .is_some());
        let tick = source.find("``").unwrap();
        assert!(layout.project(&source, 0, tick..tick + 2).is_none());
    }
    #[test]
    fn long_paragraph_keeps_unicode_styles_links_and_literal_html() {
        let source=format!("{}**中文😀 *nested*** ~~old~~ `<b>code</b>` [link😀](docs/a.md:3) [bad](javascript:alert) [data](data:text/html,test) <b>literal</b>","plain ".repeat(1600));
        let blocks = parse_text(&source).unwrap();
        let BlockKind::Paragraph(text) = &blocks[0].kind else {
            panic!("paragraph missing")
        };
        let attrs = text.attributes.as_ref().unwrap();
        assert!(attrs.heap_bytes() > 0);
        assert!(attrs
            .spans
            .iter()
            .any(|s| s.style == Style::Bold
                && selected(&text.plain, s.start, s.end) == "中文😀 nested"));
        assert!(
            attrs
                .spans
                .iter()
                .any(|s| s.style == Style::Italic
                    && selected(&text.plain, s.start, s.end) == "nested")
        );
        assert!(attrs
            .spans
            .iter()
            .any(|s| s.style == Style::Strike && selected(&text.plain, s.start, s.end) == "old"));
        assert!(attrs
            .spans
            .iter()
            .any(|s| s.style == Style::Code
                && selected(&text.plain, s.start, s.end) == "<b>code</b>"));
        assert_eq!(attrs.links.len(), 1);
        assert_eq!(attrs.links[0].destination, "docs/a.md:3");
        assert_eq!(
            selected(&text.plain, attrs.links[0].start, attrs.links[0].end),
            "link😀"
        );
        assert!(text.plain.contains("<b>literal</b>"));
        assert!(
            blocks[0].heap_bytes()
                >= text.plain.capacity() + text.markup.capacity() + attrs.heap_bytes()
        );
    }
    #[test]
    fn small_text_stays_lightweight_and_nested_image_keeps_outer_link() {
        let blocks = parse_text("**small**").unwrap();
        let BlockKind::Paragraph(text) = &blocks[0].kind else {
            panic!("paragraph missing")
        };
        assert!(text.attributes.is_none());
        let blocks = parse_text(&format!(
            "{}[![alt😀](image.png)](docs/next.md)\nnext  \nline",
            "abc ".repeat(2200)
        ))
        .unwrap();
        let BlockKind::Paragraph(text) = &blocks[0].kind else {
            panic!("paragraph missing")
        };
        let attrs = text.attributes.as_ref().unwrap();
        assert_eq!(attrs.links.len(), 1);
        assert_eq!(attrs.links[0].destination, "docs/next.md");
        assert_eq!(
            selected(&text.plain, attrs.links[0].start, attrs.links[0].end),
            "alt😀"
        );
        assert!(text.plain.ends_with("\nnext\nline"));
    }
    #[test]
    fn raw_html_and_code_controls_do_not_reach_native_c_strings() {
        let source = format!(
            "{}before\0after `<b>code\0</b>` <b>raw\0html</b> **end😀**",
            "word ".repeat(1800)
        );
        let blocks = parse_text(&source).unwrap();
        let BlockKind::Paragraph(text) = &blocks[0].kind else {
            panic!("paragraph missing")
        };
        assert!(!text.plain.contains('\0'));
        assert!(!text.markup.contains('\0'));
        let attrs = text.attributes.as_ref().unwrap();
        assert!(attrs
            .spans
            .iter()
            .any(|s| s.style == Style::Bold && selected(&text.plain, s.start, s.end) == "end😀"));
    }
    #[test]
    fn pages_round_trip_exact_unicode_text_with_styles_and_links_across_boundaries() {
        let source = format!(
            "**{}tail** [{}](docs/next.md)\r\nend",
            "字😀 ".repeat(2400),
            "linked中文😀".repeat(1600)
        );
        let blocks = parse_text(&source).unwrap();
        let BlockKind::Paragraph(text) = &blocks[0].kind else {
            panic!("paragraph missing")
        };
        let attrs = text.attributes.as_ref().unwrap();
        let mut rebuilt = String::new();
        let mut character = 0;
        for page in &attrs.pages {
            let slice = &attrs.text[page.start..page.end];
            assert!(slice.len() <= PAGE_BYTES);
            assert_eq!(page.character, character);
            assert_eq!(page.characters, slice.chars().count());
            character += page.characters;
            rebuilt.push_str(slice);
            for index in &page.spans {
                let span = &attrs.spans[*index];
                assert!(span.start < page.character + page.characters && span.end > page.character);
            }
            for index in &page.links {
                let link = &attrs.links[*index];
                assert!(link.start < page.character + page.characters && link.end > page.character);
            }
        }
        assert_eq!(rebuilt, text.plain);
        assert!(attrs.pages.iter().filter(|p| !p.spans.is_empty()).count() > 1);
        assert!(attrs.pages.iter().filter(|p| !p.links.is_empty()).count() > 1);
    }
}
