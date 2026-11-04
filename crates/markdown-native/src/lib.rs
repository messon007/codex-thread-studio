//! Presentation-neutral Markdown blocks for native reader and conversation widgets.
//! Parse only bounded cached fragments on Rust workers; source ranges stay exact.
pub mod diagrams;
pub mod edit_buffer;
pub mod embedded_image;
pub mod flow;
pub mod inline;
pub mod markup;
pub mod quote;
pub mod references;
pub mod rich_page;
pub mod rich_pages;
pub mod source_projection;
#[cfg(feature = "structured-preview")]
pub mod structured;
#[cfg(feature = "syntax-highlight")]
pub mod syntax;
pub mod text;
pub mod text_coordinates;
pub mod text_edit;
pub mod unified_diff;
use std::{iter::Peekable, ops::Range};

use pulldown_cmark::{CodeBlockKind, Event, Options, Tag, TagEnd};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceWindow {
    pub start: usize,
    pub end: usize,
    pub line: usize,
    #[serde(default)]
    pub end_line: usize,
    pub text: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub projection: Vec<source_projection::Replacement>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub references: Vec<references::Definition>,
}

impl SourceWindow {
    /// A text caret can occupy the empty line after a terminal newline. That
    /// line belongs to this context only at document EOF, not at a page boundary.
    pub fn contains_caret_line(&self, line: usize, source_bytes: usize) -> bool {
        self.start <= self.end
            && self.end <= source_bytes
            && line >= self.line
            && (line <= self.end_line
                || (self.end == source_bytes
                    && self.text.ends_with('\n')
                    && self.end_line.checked_add(1) == Some(line)))
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FragmentMetadata {
    pub start: usize,
    pub end: usize,
    pub line: usize,
    pub rows: usize,
    pub row_start: usize,
    pub table: Option<usize>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Fragment {
    pub page: usize,
    pub document_id: String,
    pub revision: String,
    pub metadata: FragmentMetadata,
    pub source: SourceWindow,
    pub header: Option<SourceWindow>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceRange {
    pub start: usize,
    pub end: usize,
    pub line: usize,
    pub end_line: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RichText {
    /// A validated source window shown literally after a formatting limit.
    pub source_only: bool,
    pub plain: String,
    pub markup: String,
    pub images: Vec<ImageReference>,
    /// Worker-prepared candidates; plain text and markup retain exact TeX.
    pub math: Vec<inline::Math>,
    /// Prepared for long text, or explicitly requested by a text-flow adapter.
    /// Offsets are Unicode scalars, as in GTK.
    pub attributes: Option<std::sync::Arc<inline::Attributes>>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImageReference {
    pub source: ImageSource,
    pub alternative: String,
    /// Alt text in `RichText::plain`, in Unicode scalars. Empty alt text is
    /// an insertion point; nested images belong to their outer image.
    pub characters: Range<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImageSource {
    Reference(String),
    Embedded(embedded_image::Image),
    Document { page: usize, image: usize },
    Unavailable(&'static str),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cell {
    pub content: std::sync::Arc<RichText>,
    /// Worker-prepared literal excerpt for expensive table labels. Full styled
    /// content stays shared and is opened explicitly, never truncated in storage.
    pub preview: Option<String>,
    /// Complete math spans in the bounded prefix, indexed into shared content.
    /// A clipped formula remains literal; original offsets are never rebased.
    pub preview_math: Vec<usize>,
    pub source: SourceRange,
}

pub const CELL_PREVIEW_BYTES: usize = 256;
impl Cell {
    fn new(content: RichText, source: SourceRange) -> Self {
        let mut preview_math = Vec::new();
        let preview = (content.plain.len() > CELL_PREVIEW_BYTES
            || content.markup.len() > inline::SCROLLABLE_TEXT_BYTES)
            .then(|| {
                let mut end = content.plain.len().min(CELL_PREVIEW_BYTES);
                while !content.plain.is_char_boundary(end) {
                    end -= 1;
                }
                if !content.source_only {
                    preview_math = content
                        .math
                        .iter()
                        .enumerate()
                        .filter_map(|(index, span)| (span.range.end <= end).then_some(index))
                        .collect();
                }
                format!("{}…", &content.plain[..end])
            });
        Self {
            content: std::sync::Arc::new(content),
            preview,
            preview_math,
            source,
        }
    }

    pub fn title(&self) -> &str {
        self.preview.as_deref().unwrap_or(&self.content.plain)
    }
}

/// Bound labels in column menus without changing table identities or source.
pub fn column_label(name: &str) -> String {
    let mut end = name.len().min(CELL_PREVIEW_BYTES);
    while !name.is_char_boundary(end) {
        end -= 1;
    }
    if end < name.len() {
        format!("{}…", &name[..end])
    } else {
        name.into()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub cells: Vec<Cell>,
    pub source: SourceRange,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BlockKind {
    Paragraph(RichText),
    Heading {
        level: u8,
        content: RichText,
    },
    Code {
        language: String,
        text: String,
    },
    Table {
        id: Option<usize>,
        row_start: usize,
        header: Vec<Cell>,
        rows: Vec<Row>,
    },
    Diagram {
        kind: String,
        source: String,
    },
    List {
        start: Option<u64>,
        children: Vec<Block>,
    },
    Item(Vec<Block>),
    Quote(Vec<Block>),
    Group(Vec<Block>),
    Rule,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Block {
    pub source: SourceRange,
    pub kind: BlockKind,
}

/// Flat display rows keep long lists and quotes virtualizable. Source anchors
/// still refer to each original leaf block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DisplayBlock {
    pub block: Block,
    pub indent: u8,
    pub marker: Option<String>,
    pub quote: bool,
}

pub fn display_rows(blocks: Vec<Block>) -> Vec<DisplayBlock> {
    fn visit(blocks: Vec<Block>, depth: u8, quote: bool, output: &mut Vec<DisplayBlock>) {
        for block in blocks {
            match block.kind {
                BlockKind::List { start, children } => {
                    for (index, child) in children.into_iter().enumerate() {
                        let first = output.len();
                        visit(vec![child], depth.saturating_add(1), quote, output);
                        if let Some(row) = output.get_mut(first) {
                            row.marker = Some(start.map_or_else(
                                || "•".into(),
                                |start| format!("{}.", start + index as u64),
                            ));
                        }
                    }
                }
                BlockKind::Item(children) | BlockKind::Group(children) => {
                    visit(children, depth, quote, output)
                }
                BlockKind::Quote(children) => {
                    visit(children, depth.saturating_add(1), true, output)
                }
                kind => output.push(DisplayBlock {
                    block: Block {
                        source: block.source,
                        kind,
                    },
                    indent: depth.min(12),
                    marker: None,
                    quote,
                }),
            }
        }
    }
    let mut rows = Vec::new();
    visit(blocks, 0, false, &mut rows);
    rows
}

struct LineMap {
    base: usize,
    line: usize,
    starts: Vec<usize>,
    projection: Vec<source_projection::Replacement>,
}

impl LineMap {
    fn new(
        text: &str,
        base: usize,
        line: usize,
        projection: &[source_projection::Replacement],
    ) -> Self {
        let mut starts = vec![0];
        starts.extend(text.match_indices('\n').map(|(offset, _)| offset + 1));
        Self {
            base,
            line,
            starts,
            projection: projection.to_vec(),
        }
    }
    fn offset_at(&self, offset: usize) -> usize {
        let mut delta = 0i128;
        for p in &self.projection {
            if offset >= p.end {
                delta += (p.source_end - p.source_start) as i128 - (p.end - p.start) as i128;
            } else if offset > p.start {
                return p.source_start;
            } else {
                break;
            }
        }
        (self.base as i128 + offset as i128 + delta) as usize
    }
    fn line_at(&self, offset: usize, end: bool) -> usize {
        let mut extra = 0;
        for p in &self.projection {
            if offset >= p.end {
                extra += p.end_line - p.line - p.newlines;
            } else if offset >= p.start {
                return if end { p.end_line } else { p.line };
            } else {
                break;
            }
        }
        self.line
            + self
                .starts
                .partition_point(|start| *start <= offset)
                .saturating_sub(1)
            + extra
    }
}

struct Maps {
    body: LineMap,
    header: Option<LineMap>,
    prefix: usize,
    retain_inline: bool,
}

impl Maps {
    fn deferred(&self, span: Range<usize>, uri: &str) -> Option<(usize, usize)> {
        let range = self.range(span);
        self.body
            .projection
            .iter()
            .chain(self.header.iter().flat_map(|h| h.projection.iter()))
            .find(|p| {
                p.source_start == range.start
                    && p.source_end == range.end
                    && uri == format!("studio-doc-image:{}:{}", p.page, p.image)
            })
            .map(|p| (p.page, p.image))
    }
    fn range(&self, span: Range<usize>) -> SourceRange {
        let map_position = |position: usize, end: bool| {
            let (map, offset) = if position < self.prefix {
                (self.header.as_ref().unwrap(), position)
            } else {
                (&self.body, position - self.prefix)
            };
            (map.offset_at(offset), map.line_at(offset, end))
        };
        let (start, line) = map_position(span.start, false);
        let (end, _) = if span.start < self.prefix && span.end == self.prefix {
            let header = self.header.as_ref().unwrap();
            (header.offset_at(span.end), header.line_at(span.end, false))
        } else {
            map_position(span.end, false)
        };
        let (_, end_line) = map_position(span.end.saturating_sub(1).max(span.start), true);
        SourceRange {
            start,
            end,
            line,
            end_line,
        }
    }
}

type Tokens<'a> = Peekable<references::Events<'a>>;

/// Parse an in-memory Markdown message with the same syntax, source anchors and
/// safety budgets as a reader fragment. Call on a bounded CPU worker.
pub fn parse_text(text: &str) -> Result<Vec<Block>, String> {
    parse(&Fragment {
        page: 0,
        document_id: String::new(),
        revision: String::new(),
        metadata: FragmentMetadata {
            start: 0,
            end: text.len(),
            line: 1,
            rows: 0,
            row_start: 0,
            table: None,
        },
        source: SourceWindow {
            projection: Vec::new(),
            references: Vec::new(),
            start: 0,
            end: text.len(),
            line: 1,
            end_line: text.bytes().filter(|byte| *byte == b'\n').count() + 1,
            text: text.into(),
        },
        header: None,
    })
}

pub fn parse_text_readable(text: &str) -> Result<Vec<Block>, String> {
    let window = SourceWindow {
        projection: Vec::new(),
        references: Vec::new(),
        start: 0,
        end: text.len(),
        line: 1,
        end_line: 0,
        text: text.into(),
    };
    parse_readable(&Fragment {
        page: 0,
        document_id: String::new(),
        revision: String::new(),
        metadata: FragmentMetadata {
            start: 0,
            end: text.len(),
            line: 1,
            rows: 0,
            row_start: 0,
            table: None,
        },
        source: window,
        header: None,
    })
}
/// A formatting budget must not discard a valid, bounded source window.
/// Invalid/oversized ranges remain errors; only validated text becomes literal.
pub fn parse_readable(fragment: &Fragment) -> Result<Vec<Block>, String> {
    validate(fragment)?;
    match parse(fragment) {
        Ok(blocks) => Ok(blocks),
        Err(error)
            if !fragment.source.projection.is_empty()
                || fragment
                    .header
                    .as_ref()
                    .is_some_and(|h| !h.projection.is_empty()) =>
        {
            Err(error)
        }
        Err(_) => {
            let mut blocks = Vec::new();
            for source in fragment
                .header
                .iter()
                .chain(std::iter::once(&fragment.source))
            {
                let attributes = inline::literal(&source.text, source.start, source.line);
                let end_line = attributes
                    .pages
                    .last()
                    .and_then(|p| p.lines)
                    .map_or(source.line, |(_, end)| end);
                blocks.push(Block {
                    source: SourceRange {
                        start: source.start,
                        end: source.end,
                        line: source.line,
                        end_line,
                    },
                    kind: BlockKind::Paragraph(RichText {
                        source_only: true,
                        plain: attributes.text.clone(),
                        markup: String::new(),
                        images: Vec::new(),
                        math: Vec::new(),
                        attributes: Some(std::sync::Arc::new(attributes)),
                    }),
                });
            }
            Ok(blocks)
        }
    }
}

impl Block {
    /// Heap capacity owned by this block, including nested blocks and tables.
    pub fn heap_bytes(&self) -> usize {
        let rich = |text: &RichText| {
            text.plain.capacity()
                + text.markup.capacity()
                + text
                    .attributes
                    .as_ref()
                    .map_or(0, |attributes| attributes.heap_bytes())
                + text.images.capacity() * std::mem::size_of::<ImageReference>()
                + text.math.capacity() * std::mem::size_of::<inline::Math>()
                + text
                    .images
                    .iter()
                    .map(|image| {
                        image.alternative.capacity()
                            + match &image.source {
                                ImageSource::Reference(uri) => uri.capacity(),
                                ImageSource::Embedded(image) => image.bytes.len(),
                                ImageSource::Unavailable(_) | ImageSource::Document { .. } => 0,
                            }
                    })
                    .sum::<usize>()
        };
        let cells = |cells: &Vec<Cell>| {
            cells.capacity() * std::mem::size_of::<Cell>()
                + cells
                    .iter()
                    .map(|cell| {
                        rich(&cell.content)
                            + std::mem::size_of::<RichText>()
                            + cell.preview.as_ref().map_or(0, String::capacity)
                            + cell.preview_math.capacity() * std::mem::size_of::<usize>()
                    })
                    .sum::<usize>()
        };
        match &self.kind {
            BlockKind::Paragraph(text) | BlockKind::Heading { content: text, .. } => rich(text),
            BlockKind::Code { language, text } => language.capacity() + text.capacity(),
            BlockKind::Diagram { kind, source } => kind.capacity() + source.capacity(),
            BlockKind::Table { header, rows, .. } => {
                cells(header)
                    + rows.capacity() * std::mem::size_of::<Row>()
                    + rows.iter().map(|row| cells(&row.cells)).sum::<usize>()
            }
            BlockKind::List { children, .. }
            | BlockKind::Item(children)
            | BlockKind::Quote(children)
            | BlockKind::Group(children) => {
                children.capacity() * std::mem::size_of::<Block>()
                    + children.iter().map(Block::heap_bytes).sum::<usize>()
            }
            BlockKind::Rule => 0,
        }
    }
}

fn validate(fragment: &Fragment) -> Result<(), String> {
    if fragment.source.text.len() > 512 * 1024
        || fragment
            .header
            .as_ref()
            .is_some_and(|header| header.text.len() > 128 * 1024)
    {
        return Err("Native Markdown fragment exceeds its page budget".into());
    }
    if fragment.source.start > fragment.source.end
        || (fragment.source.projection.is_empty()
            && fragment.source.end - fragment.source.start != fragment.source.text.len())
        || fragment.source.line == 0
        || fragment
            .source
            .line
            .checked_add(fragment.source.text.len())
            .is_none()
        || fragment.header.as_ref().is_some_and(|header| {
            header.start > header.end
                || (header.projection.is_empty() && header.end - header.start != header.text.len())
                || header.line == 0
                || header.line.checked_add(header.text.len()).is_none()
        })
    {
        return Err("Native Markdown source range is invalid".into());
    }
    source_projection::validate(&fragment.source)?;
    if let Some(header) = &fragment.header {
        source_projection::validate(header)?;
    }
    Ok(())
}

pub fn parse(fragment: &Fragment) -> Result<Vec<Block>, String> {
    parse_inline(fragment, false)
}

/// Opt-in projection for native text-buffer experiments. The normal block
/// renderer keeps its existing short-label memory policy.
pub(crate) fn parse_inline(fragment: &Fragment, retain_inline: bool) -> Result<Vec<Block>, String> {
    validate(fragment)?;
    let mut text = String::new();
    if let Some(header) = &fragment.header {
        text.push_str(&header.text);
    }
    let prefix = text.len();
    text.push_str(&fragment.source.text);
    let maps = Maps {
        body: LineMap::new(
            &fragment.source.text,
            fragment.source.start,
            fragment.source.line,
            &fragment.source.projection,
        ),
        header: fragment
            .header
            .as_ref()
            .map(|h| LineMap::new(&h.text, h.start, h.line, &h.projection)),
        prefix,
        retain_inline,
    };
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_MATH;
    let (normalized, bare) = diagrams::normalize(&text, options);
    let definitions = references::context(
        fragment.source.references.iter().chain(
            fragment
                .header
                .iter()
                .flat_map(|header| header.references.iter()),
        ),
    )?;
    let lookup = |label: &str| {
        definitions
            .get(&references::Key::new(label.to_owned()))
            .cloned()
    };
    let mut tokens = references::events(
        &normalized,
        options,
        (!definitions.is_empty()).then_some(&lookup as &references::Lookup<'_>),
    )
    .peekable();
    let mut budget = 16_384;
    let mut blocks = nodes(
        &mut tokens,
        None,
        0,
        &maps,
        fragment,
        &mut budget,
        &normalized,
    )?;
    fn restore(blocks: &mut [Block], bare: &[Range<usize>], text: &str, maps: &Maps) {
        for block in blocks {
            match &mut block.kind {
                BlockKind::Diagram { kind, source } if kind == "plantuml" => {
                    if let Some(range) = bare.iter().find(|range| {
                        let original = maps.range((*range).clone());
                        original.start >= block.source.start && original.start < block.source.end
                    }) {
                        *source = text[range.clone()].to_owned();
                    }
                }
                BlockKind::List { children, .. }
                | BlockKind::Item(children)
                | BlockKind::Quote(children)
                | BlockKind::Group(children) => restore(children, bare, text, maps),
                _ => {}
            }
        }
    }
    restore(&mut blocks, &bare, &text, &maps);
    Ok(blocks)
}

fn rich(
    events: &[(Event<'_>, Range<usize>)],
    input: &str,
    maps: &Maps,
    retain_short: bool,
) -> RichText {
    // Inline math remains exact readable source until native inline image
    // layout is available. Enabling display math must never drop inline TeX,
    // currency-like text, or its selection/source anchors.
    let original = events;
    let literal;
    let has_math = events
        .iter()
        .any(|(e, _)| matches!(e, Event::InlineMath(_) | Event::DisplayMath(_)));
    let events = if has_math {
        literal = events
            .iter()
            .map(|(e, r)| {
                let e = if matches!(e, Event::InlineMath(_) | Event::DisplayMath(_)) {
                    Event::Text(input[r.clone()].to_owned().into())
                } else {
                    e.clone()
                };
                (e, r.clone())
            })
            .collect::<Vec<_>>();
        literal.as_slice()
    } else {
        events
    };
    let mut plain = String::new();
    let mut math = Vec::new();
    let mut characters = 0;
    let mut images = Vec::new();
    let mut image_budget = embedded_image::MAX_BYTES;
    let mut image_depth = 0usize;
    for (index, (event, image_span)) in events.iter().enumerate() {
        if matches!(event, Event::End(TagEnd::Image)) {
            image_depth = image_depth.saturating_sub(1);
        }
        if let Event::Start(Tag::Image { dest_url, .. }) = event {
            image_depth += 1;
            if image_depth == 1 && images.len() < 4 {
                let mut alternative = String::new();
                let mut depth = 1usize;
                for (event, _) in &events[index + 1..] {
                    match event {
                        Event::Start(Tag::Image { .. }) => depth += 1,
                        Event::End(TagEnd::Image) => {
                            depth -= 1;
                            if depth == 0 {
                                break;
                            }
                        }
                        Event::Text(text) | Event::Code(text) => alternative.push_str(text),
                        Event::SoftBreak | Event::HardBreak => alternative.push(' '),
                        _ => {}
                    }
                }
                let source =
                    if let Some((page, image)) = maps.deferred(image_span.clone(), dest_url) {
                        ImageSource::Document { page, image }
                    } else if dest_url
                        .get(..5)
                        .is_some_and(|p| p.eq_ignore_ascii_case("data:"))
                    {
                        match embedded_image::decode(dest_url, image_budget) {
                            Ok(image) => {
                                image_budget -= image.bytes.len();
                                ImageSource::Embedded(image)
                            }
                            Err(error) => ImageSource::Unavailable(error),
                        }
                    } else if dest_url.len() <= 8192 {
                        ImageSource::Reference(dest_url.to_string())
                    } else {
                        ImageSource::Unavailable("Image reference exceeds its budget")
                    };
                images.push(ImageReference {
                    source,
                    alternative: alternative.chars().take(512).collect(),
                    characters: 0..0,
                });
            }
        }
    }
    let mut image_depth = 0usize;
    let mut image_index = 0usize;
    for (index, (event, _)) in events.iter().enumerate() {
        match event {
            Event::Start(Tag::Image { .. }) => {
                if image_depth == 0 {
                    if let Some(image) = images.get_mut(image_index) {
                        image.characters.start = characters;
                    }
                }
                image_depth += 1;
            }
            Event::End(TagEnd::Image) => {
                image_depth = image_depth.saturating_sub(1);
                if image_depth == 0 {
                    if let Some(image) = images.get_mut(image_index) {
                        image.characters.end = characters;
                    }
                    image_index += 1;
                }
            }
            Event::Text(value)
            | Event::Code(value)
            | Event::Html(value)
            | Event::InlineHtml(value) => {
                let count = if has_math || !images.is_empty() {
                    value.chars().count()
                } else {
                    0
                };
                let (original_event, range) = &original[index];
                let display = matches!(original_event, Event::DisplayMath(_));
                if matches!(original_event, Event::InlineMath(_) | Event::DisplayMath(_))
                    && image_depth == 0
                    && math.len() < inline::MATH_SPANS
                    && !value.contains('\0')
                    // A closer immediately followed by a digit is ambiguous
                    // currency ("$5 and $10"), so retain its literal spelling.
                    && (display || !input.as_bytes().get(range.end).is_some_and(u8::is_ascii_digit))
                {
                    let delimiters = if display { 2 } else { 1 };
                    let body = delimiters..value.len().saturating_sub(delimiters);
                    if body.start < body.end
                        && body.len() <= inline::MATH_SOURCE_BYTES
                        && value.get(body.clone()).is_some()
                    {
                        math.push(inline::Math {
                            range: plain.len()..plain.len() + value.len(),
                            body: plain.len() + body.start..plain.len() + body.end,
                            characters: characters..characters + count,
                            source: maps.range(range.clone()),
                            display,
                        });
                    }
                }
                characters += count;
                if value.contains('\0') {
                    plain.extend(value.chars().map(|c| if c == '\0' { '␀' } else { c }));
                } else {
                    plain.push_str(value);
                }
            }
            Event::SoftBreak | Event::HardBreak => {
                plain.push('\n');
                characters += 1;
            }
            Event::TaskListMarker(checked) => {
                plain.push_str(if *checked { "☑ " } else { "☐ " });
                characters += 2;
            }
            _ => {}
        }
    }
    let markup = crate::markup::markup_events(events.iter().map(|(event, _)| event.clone()));
    let multiline_math = math
        .iter()
        .any(|m| plain[m.body.clone()].contains(['\n', '\r', '\u{2028}', '\u{2029}']));
    let attributes = (!plain.is_empty()
        && ((maps.retain_inline && retain_short)
            // Markup labels normalize CRLF, losing source bytes and shifting
            // math/link offsets. Prepared native text ranges avoid that path.
            || plain.contains('\r')
            || multiline_math
            || plain.len() > inline::SCROLLABLE_TEXT_BYTES
            || markup.len() > inline::SCROLLABLE_TEXT_BYTES))
        .then(|| {
            let mut attributes =
                inline::attributes_with_math(events.iter().map(|(event, _)| event), &plain, &math);
            attributes.multiline_math = multiline_math;
            attributes.sources = inline::source_map(events, input, |range| {
                let source = maps.range(range);
                source.start..source.end
            });
            std::sync::Arc::new(attributes)
        });
    RichText {
        source_only: false,
        plain,
        markup,
        images,
        math,
        attributes,
    }
}

fn inline<'a>(tokens: &mut Tokens<'a>, end: TagEnd) -> Vec<(Event<'a>, Range<usize>)> {
    let mut events = Vec::new();
    while let Some((event, span)) = tokens.next() {
        if event == Event::End(end) {
            break;
        }
        events.push((event, span));
    }
    events
}

fn nodes(
    tokens: &mut Tokens<'_>,
    end: Option<TagEnd>,
    depth: usize,
    maps: &Maps,
    fragment: &Fragment,
    budget: &mut usize,
    input: &str,
) -> Result<Vec<Block>, String> {
    if depth > 64 {
        return Err("Markdown nesting exceeds the native view budget".into());
    }
    let mut result = Vec::new();
    while let Some((event, span)) = tokens.next() {
        if matches!(&event,Event::End(tag) if Some(*tag)==end) {
            break;
        }
        if *budget == 0 {
            return Err("Markdown nodes exceed the native view budget".into());
        }
        *budget -= 1;
        let source = maps.range(span.clone());
        let kind = match event {
            Event::Start(Tag::Paragraph) => {
                let events = inline(tokens, TagEnd::Paragraph);
                let mut meaningful = events.iter().filter(|(e, _)| {
                    !matches!(e, Event::SoftBreak | Event::HardBreak)
                        && !matches!(e,Event::Text(s) if s.trim().is_empty())
                });
                match (meaningful.next(), meaningful.next()) {
                    (Some((Event::DisplayMath(body), _)), None) => BlockKind::Diagram {
                        kind: "math".into(),
                        source: body.to_string(),
                    },
                    _ => BlockKind::Paragraph(rich(&events, input, maps, true)),
                }
            }
            Event::Start(Tag::Heading { level, .. }) => BlockKind::Heading {
                level: level as u8,
                content: rich(&inline(tokens, TagEnd::Heading(level)), input, maps, true),
            },
            Event::Start(Tag::CodeBlock(kind)) => {
                let events = inline(tokens, TagEnd::CodeBlock);
                let text = events
                    .iter()
                    .filter_map(|(event, _)| {
                        if let Event::Text(value) = event {
                            Some(value.as_ref())
                        } else {
                            None
                        }
                    })
                    .collect::<String>();
                let language = if let CodeBlockKind::Fenced(info) = kind {
                    info.split_whitespace().next().unwrap_or("").to_owned()
                } else {
                    String::new()
                };
                let diagram = match language.to_ascii_lowercase().as_str() {
                    "mermaid" => Some("mermaid"),
                    "d2" => Some("d2"),
                    "plantuml" | "puml" | "uml" => Some("plantuml"),
                    "math" | "latex" | "tex" => Some("math"),
                    _ => None,
                };
                if let Some(kind) = diagram {
                    BlockKind::Diagram {
                        kind: kind.into(),
                        source: text,
                    }
                } else {
                    BlockKind::Code { language, text }
                }
            }
            Event::Start(Tag::Table(_)) => table(tokens, maps, fragment, budget, input)?,
            Event::DisplayMath(body) => BlockKind::Diagram {
                kind: "math".into(),
                source: body.to_string(),
            },
            Event::Start(Tag::List(start)) => BlockKind::List {
                start,
                children: nodes(
                    tokens,
                    Some(TagEnd::List(start.is_some())),
                    depth + 1,
                    maps,
                    fragment,
                    budget,
                    input,
                )?,
            },
            Event::Start(Tag::Item) => BlockKind::Item(nodes(
                tokens,
                Some(TagEnd::Item),
                depth + 1,
                maps,
                fragment,
                budget,
                input,
            )?),
            Event::Start(Tag::BlockQuote(kind)) => BlockKind::Quote(nodes(
                tokens,
                Some(TagEnd::BlockQuote(kind)),
                depth + 1,
                maps,
                fragment,
                budget,
                input,
            )?),
            Event::Start(tag) => BlockKind::Group(nodes(
                tokens,
                Some(tag.to_end()),
                depth + 1,
                maps,
                fragment,
                budget,
                input,
            )?),
            Event::Rule => BlockKind::Rule,
            Event::End(_) => continue,
            other => BlockKind::Paragraph(rich(&[(other, span)], input, maps, true)),
        };
        result.push(Block { source, kind });
    }
    Ok(result)
}

fn table(
    tokens: &mut Tokens<'_>,
    maps: &Maps,
    fragment: &Fragment,
    budget: &mut usize,
    input: &str,
) -> Result<BlockKind, String> {
    let mut header = Vec::new();
    let mut rows = Vec::new();
    let mut cells = Vec::new();
    let mut row_source = None;
    let mut in_header = false;
    while let Some((event, span)) = tokens.next() {
        match event {
            Event::Start(Tag::TableHead) => {
                in_header = true;
            }
            Event::End(TagEnd::TableHead) => {
                header = std::mem::take(&mut cells);
                in_header = false;
            }
            Event::Start(Tag::TableRow) => {
                row_source = Some(maps.range(span));
            }
            Event::End(TagEnd::TableRow) => {
                if let Some(source) = row_source.take() {
                    rows.push(Row {
                        source,
                        cells: std::mem::take(&mut cells),
                    });
                }
            }
            Event::Start(Tag::TableCell) => {
                if *budget == 0 {
                    return Err("Table cells exceed the native view budget".into());
                }
                *budget -= 1;
                cells.push(Cell::new(
                    rich(&inline(tokens, TagEnd::TableCell), input, maps, false),
                    maps.range(span),
                ));
            }
            Event::End(TagEnd::Table) => break,
            _ => {}
        }
    }
    if in_header {
        return Err("Incomplete native table header".into());
    }
    Ok(BlockKind::Table {
        id: fragment.metadata.table,
        row_start: fragment.metadata.row_start,
        header,
        rows,
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn source_context_includes_only_its_own_caret_lines() {
        let mut window = super::SourceWindow {
            start: 10,
            end: 21,
            line: 4,
            end_line: 5,
            text: "saved\ntext\n".into(),
            projection: Vec::new(),
            references: Vec::new(),
        };
        for line in [4, 5, 6] {
            assert!(window.contains_caret_line(line, 21));
        }
        assert!(!window.contains_caret_line(3, 21));
        assert!(!window.contains_caret_line(7, 21));
        // A page ending in a newline does not own the following page's line.
        assert!(!window.contains_caret_line(6, 100));
        assert!(!window.contains_caret_line(4, 20));
        window.text.pop();
        assert!(!window.contains_caret_line(6, 21));
        window.start = 22;
        assert!(!window.contains_caret_line(4, 21));
    }
    #[test]
    fn readable_limit_fallback_keeps_all_literal_text_without_active_markup() {
        let text=format!("literal-begin **raw 中文😀** <b>literal</b> [unsafe](javascript:alert)\r\n\r\n{}literal-tail ","# h\r\n".repeat(17000));
        assert!(super::parse_text(&text).unwrap_err().contains("nodes"));
        let blocks = super::parse_text_readable(&text).unwrap();
        assert_eq!(blocks.len(), 1);
        let super::BlockKind::Paragraph(content) = &blocks[0].kind else {
            panic!("Source paragraph missing")
        };
        assert!(content.source_only);
        assert_eq!(content.plain, text);
        assert!(content.markup.is_empty());
        assert!(content.images.is_empty());
        let attributes = content.attributes.as_ref().unwrap();
        assert!(attributes.links.is_empty());
        assert!(attributes.spans.is_empty());
        assert!(attributes
            .pages
            .iter()
            .all(|p| p.end - p.start <= super::inline::PAGE_BYTES));
        assert_eq!(
            attributes
                .pages
                .iter()
                .map(|p| &attributes.text[p.start..p.end])
                .collect::<String>(),
            text
        );
        let at = text.find("literal-tail ").unwrap();
        let found = attributes
            .project(&text, 0, at..at + "literal-tail ".len())
            .unwrap();
        assert!(found.page > 0);
        assert_eq!(blocks[0].source.end, text.len());
        assert_eq!(attributes.pages.last().unwrap().lines.unwrap().1, 17003);
    }
    #[test]
    fn literal_continuations_keep_separate_original_header_and_body_coordinates() {
        let text = "# h\n".repeat(17000);
        let mut fragment = super::Fragment {
            page: 3,
            document_id: "doc".into(),
            revision: "r".into(),
            metadata: super::FragmentMetadata {
                start: 50000,
                end: 50000 + text.len(),
                line: 900,
                rows: 0,
                row_start: 0,
                table: None,
            },
            source: super::SourceWindow {
                projection: Vec::new(),
                references: Vec::new(),
                start: 50000,
                end: 50000 + text.len(),
                line: 900,
                end_line: 0,
                text,
            },
            header: Some(super::SourceWindow {
                projection: Vec::new(),
                references: Vec::new(),
                start: 10,
                end: 18,
                line: 2,
                end_line: 2,
                text: "# Title\n".into(),
            }),
        };
        let blocks = super::parse_readable(&fragment).unwrap();
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].source.start, 10);
        assert_eq!(blocks[0].source.line, 2);
        assert_eq!(blocks[1].source.start, 50000);
        assert_eq!(blocks[1].source.line, 900);
        let super::BlockKind::Paragraph(text) = &blocks[1].kind else {
            panic!("Body missing")
        };
        assert_eq!(
            text.attributes.as_ref().unwrap().pages[0].lines.unwrap().0,
            900
        );
        fragment.header.as_mut().unwrap().end -= 1;
        assert!(super::parse_readable(&fragment).is_err());
        fragment.header = None;
        fragment.source.line = usize::MAX;
        assert!(super::parse_readable(&fragment).is_err());
        assert!(super::parse_text_readable(&"x".repeat(512 * 1024 + 1)).is_err());
    }
    #[test]
    fn table_excerpts_bound_unicode_labels_and_keep_full_styled_shared_source() {
        let body = format!("**bold 中文😀** {} tail-cell", "字😀abcdef".repeat(9000));
        let source = format!("| Name | Value |\r\n|---|---|\r\n| normal | {body} |\r\n");
        let blocks = super::parse_text(&source).unwrap();
        let super::BlockKind::Table { header, rows, .. } = &blocks[0].kind else {
            panic!("table missing")
        };
        assert!(header[0].preview.is_none());
        assert!(rows[0].cells[0].preview.is_none());
        let cell = &rows[0].cells[1];
        let preview = cell.preview.as_ref().unwrap();
        assert!(preview.len() <= super::CELL_PREVIEW_BYTES + '…'.len_utf8());
        assert!(preview.starts_with("bold 中文😀"));
        assert!(!preview.contains("tail-cell"));
        assert!(cell.content.plain.ends_with("tail-cell"));
        assert!(cell.content.markup.contains("<b>bold 中文😀</b>"));
        let attributes = cell.content.attributes.as_ref().unwrap();
        assert_eq!(attributes.text, cell.content.plain);
        assert!(attributes.pages.len() > 1);
        assert!(attributes
            .pages
            .iter()
            .all(|p| p.end - p.start <= super::inline::PAGE_BYTES));
        assert_eq!(cell.source.line, 3);
        assert!(source[cell.source.start..cell.source.end].contains("tail-cell"));
        assert!(std::sync::Arc::ptr_eq(&cell.content, &cell.clone().content));
        assert!(super::column_label(&"字😀".repeat(1000)).len() <= super::CELL_PREVIEW_BYTES + 3);
    }
    #[test]
    fn table_math_excerpts_keep_only_whole_formulas_and_original_offsets() {
        fn cell(body: &str) -> super::Cell {
            let blocks = super::parse_text(&format!("| H |\r\n|---|\r\n| {body} |\r\n")).unwrap();
            let super::BlockKind::Table { rows, .. } = &blocks[0].kind else {
                panic!("table missing")
            };
            rows[0].cells[0].clone()
        }
        let rich = cell(&format!("中文😀 **$x^2$** {} tail", "abc ".repeat(9000)));
        assert_eq!(rich.preview_math, vec![0]);
        let formula = &rich.content.math[rich.preview_math[0]];
        assert_eq!(&rich.content.plain[formula.range.clone()], "$x^2$");
        assert_eq!(&rich.content.plain[formula.body.clone()], "x^2");
        assert_eq!(formula.source.line, 3);
        let preview = rich.preview.as_deref().unwrap().strip_suffix('…').unwrap();
        assert_eq!(preview, &rich.content.plain[..preview.len()]);
        assert_eq!(
            preview[formula.range.clone()].chars().count(),
            formula.characters.len()
        );
        assert!(rich.content.plain.ends_with("tail"));
        assert!(std::sync::Arc::ptr_eq(&rich.content, &rich.clone().content));

        let boundary = cell(&format!("{}$xyz^2$ tail", "x".repeat(249)));
        assert_eq!(
            boundary.content.math[0].range.end,
            super::CELL_PREVIEW_BYTES
        );
        assert_eq!(boundary.preview_math, vec![0]);
        let clipped = cell(&format!("{}$xyz^2$ tail", "x".repeat(250)));
        assert_eq!(clipped.content.math.len(), 1);
        assert!(clipped.preview_math.is_empty());
        assert_eq!(
            clipped.content.math[0].range.end,
            super::CELL_PREVIEW_BYTES + 1
        );
        let unicode = cell(&format!("{}中$x$ tail", "x".repeat(254)));
        assert_eq!(
            unicode.preview.as_ref().unwrap().len(),
            254 + '…'.len_utf8()
        );
        assert!(unicode.preview_math.is_empty());
        let short = cell("$x$");
        assert!(short.preview.is_none() && short.preview_math.is_empty());
        let mut literal = (*rich.content).clone();
        literal.source_only = true;
        assert!(super::Cell::new(literal, rich.source)
            .preview_math
            .is_empty());
    }
    #[test]
    fn markup_heavy_short_cells_use_prepared_attributes_in_the_full_view() {
        let source = format!("| Cell |\n|---|\n| {} |\n", "**x** ".repeat(1500));
        let blocks = super::parse_text(&source).unwrap();
        let super::BlockKind::Table { rows, .. } = &blocks[0].kind else {
            panic!("table missing")
        };
        let cell = &rows[0].cells[0];
        assert!(cell.content.plain.len() < super::inline::SCROLLABLE_TEXT_BYTES);
        assert!(cell.content.markup.len() > super::inline::SCROLLABLE_TEXT_BYTES);
        assert!(cell.content.attributes.is_some());
        assert!(cell.preview.is_some());
    }
    #[test]
    fn math_blocks_keep_exact_source_ranges_and_inline_tex_remains_readable() {
        let input="# Math 中文\r\n\r\n$$\r\n\\frac{1}{2}\r\n$$\r\n\r\nInline $x^2$ and currency $5 and $10 remain visible.\r\n\r\n```latex\r\nE=mc^2\r\n```\r\n";
        let blocks = super::parse_text(input).unwrap();
        assert_eq!(blocks.len(), 4);
        assert!(
            matches!(&blocks[1].kind,super::BlockKind::Diagram{kind,source} if kind=="math"&&source.contains("\\frac{1}{2}"))
        );
        assert_eq!(blocks[1].source.line, 3);
        assert!(input[blocks[1].source.start..blocks[1].source.end].starts_with("$$"));
        let super::BlockKind::Paragraph(text) = &blocks[2].kind else {
            panic!("inline paragraph missing")
        };
        assert_eq!(
            text.plain,
            "Inline $x^2$ and currency $5 and $10 remain visible."
        );
        assert!(text.markup.contains("$x^2$"));
        assert_eq!(text.math.len(), 1);
        assert_eq!(blocks[3].source.line, 9);
        assert!(
            matches!(&blocks[3].kind,super::BlockKind::Diagram{kind,source} if kind=="math"&&source=="E=mc^2\n")
        );
    }
    #[test]
    fn math_does_not_activate_inside_protected_code_or_remove_mixed_content() {
        let input="`$x$` and \\$not_math\n\n```rust\nlet value = \"$$ x $$\";\n```\n\nBefore $$x^2$$ after.\n\n| Value |\n|---|\n| $x+y$ |\n";
        let blocks = super::parse_text(input).unwrap();
        assert!(matches!(&blocks[1].kind,super::BlockKind::Code{language,..} if language=="rust"));
        let super::BlockKind::Paragraph(text) = &blocks[2].kind else {
            panic!("mixed paragraph missing")
        };
        assert_eq!(text.plain, "Before $$x^2$$ after.");
        let super::BlockKind::Table { rows, .. } = &blocks[3].kind else {
            panic!("table missing")
        };
        assert_eq!(rows[0].cells[0].content.plain, "$x+y$");
        assert_eq!(rows[0].cells[0].content.math.len(), 1);
        assert!(
            matches!(&blocks[0].kind,super::BlockKind::Paragraph(text) if text.math.is_empty())
        );
    }
    #[test]
    fn inline_math_candidates_keep_unicode_and_original_occurrences() {
        let input = "# Header\r\n\r\n中文😀 &amp; **$x^2$** and [$x^2$](https://example.invalid) before $$\\frac{1}{2}$$ after.\r\n";
        let blocks = super::parse_text(input).unwrap();
        let super::BlockKind::Paragraph(text) = &blocks[1].kind else {
            panic!("paragraph missing")
        };
        assert_eq!(text.math.len(), 3);
        let positions = input
            .match_indices("$x^2$")
            .map(|(i, _)| i)
            .collect::<Vec<_>>();
        for (index, span) in text.math.iter().enumerate() {
            let literal = &text.plain[span.range.clone()];
            assert_eq!(literal, &input[span.source.start..span.source.end]);
            assert_eq!(
                span.characters.start,
                text.plain[..span.range.start].chars().count()
            );
            assert_eq!(
                span.characters.end,
                text.plain[..span.range.end].chars().count()
            );
            assert_eq!(span.source.line, 3);
            if index < 2 {
                assert_eq!(span.source.start, positions[index]);
                assert_eq!(&text.plain[span.body.clone()], "x^2");
                assert!(!span.display);
            } else {
                assert_eq!(&text.plain[span.body.clone()], "\\frac{1}{2}");
                assert!(span.display);
            }
        }
        assert!(text.markup.contains("&amp;"));
        assert!(!text.plain.contains("&amp;"));
    }
    #[test]
    fn math_metadata_budget_never_removes_literal_source() {
        let input = "$x$ ".repeat(super::inline::MATH_SPANS + 20);
        let blocks = super::parse_text(&input).unwrap();
        let super::BlockKind::Paragraph(text) = &blocks[0].kind else {
            panic!("paragraph missing")
        };
        assert_eq!(text.math.len(), super::inline::MATH_SPANS);
        assert_eq!(text.plain, input.trim_end());
        assert!(
            blocks[0].heap_bytes()
                >= text.math.capacity() * std::mem::size_of::<super::inline::Math>()
        );
        let large = format!(
            "Before ${}$ after",
            "x".repeat(super::inline::MATH_SOURCE_BYTES + 1)
        );
        let blocks = super::parse_text(&large).unwrap();
        let super::BlockKind::Paragraph(text) = &blocks[0].kind else {
            panic!("paragraph missing")
        };
        assert!(text.math.is_empty());
        assert_eq!(text.plain, large);
    }
    #[test]
    fn crlf_math_prepares_short_native_text_without_losing_styles_or_offsets() {
        let input = "Before **字😀** $\\frac{\r\na}{b}$ [after](docs/a.md).";
        let blocks = super::parse_text(input).unwrap();
        let super::BlockKind::Paragraph(text) = &blocks[0].kind else {
            panic!("No paragraph")
        };
        assert!(text.plain.contains("\r\n"));
        assert!(text.plain.len() < super::inline::SCROLLABLE_TEXT_BYTES);
        let attributes = text.attributes.as_ref().unwrap();
        assert_eq!(attributes.text, text.plain);
        assert!(attributes.carriage_returns);
        let formula = &text.math[0];
        let expected = "$\\frac{\r\na}{b}$";
        assert_eq!(&text.plain[formula.range.clone()], expected);
        assert_eq!(
            text.plain
                .chars()
                .skip(formula.characters.start)
                .take(formula.characters.len())
                .collect::<String>(),
            expected
        );
        let hit = attributes
            .project(input, 0, formula.source.start..formula.source.end)
            .unwrap();
        assert_eq!(hit.characters, formula.characters);
        let link = &attributes.links[0];
        assert_eq!(
            text.plain
                .chars()
                .skip(link.start)
                .take(link.end - link.start)
                .collect::<String>(),
            "after"
        );
        assert_eq!(link.destination, "docs/a.md");
        assert!(attributes
            .spans
            .iter()
            .any(|s| s.style == super::inline::Style::Bold));
        assert_eq!((formula.source.line, formula.source.end_line), (1, 2));
    }
    #[test]
    fn inline_math_fragment_offsets_survive_entity_and_nul_normalization() {
        let raw = "字\0😀 &amp; $x_1$ after";
        let fragment = Fragment {
            page: 0,
            document_id: "math".into(),
            revision: "test".into(),
            header: None,
            metadata: FragmentMetadata {
                start: 1000,
                end: 1000 + raw.len(),
                line: 50,
                rows: 1,
                row_start: 0,
                table: None,
            },
            source: SourceWindow {
                projection: Vec::new(),
                references: Vec::new(),
                start: 1000,
                end: 1000 + raw.len(),
                line: 50,
                end_line: 50,
                text: raw.into(),
            },
        };
        let blocks = parse(&fragment).unwrap();
        let BlockKind::Paragraph(text) = &blocks[0].kind else {
            panic!("paragraph missing")
        };
        let span = &text.math[0];
        assert_eq!(span.source.start, 1000 + raw.find('$').unwrap());
        assert_eq!(span.source.end, 1000 + raw.rfind('$').unwrap() + 1);
        assert_eq!(span.source.line, 50);
        assert_eq!(text.plain, "字␀😀 & $x_1$ after");
        assert_eq!(&text.plain[span.range.clone()], "$x_1$");
        assert_eq!(span.characters, 6..11);
    }
    use super::*;
    #[test]
    fn long_lists_become_individually_virtualizable_rows() {
        let text = (0..2048)
            .map(|index| format!("- Item {index}\n"))
            .collect::<String>();
        let rows = display_rows(parse(&fragment(&text)).unwrap());
        assert_eq!(rows.len(), 2048);
        assert!(rows
            .iter()
            .all(|row| row.indent == 1 && row.marker.as_deref() == Some("•")));
        assert_eq!(rows[2047].block.source.line, 2048);
        assert!(rows[2047].block.source.start > rows[0].block.source.start);
    }
    fn fragment(text: &str) -> Fragment {
        Fragment {
            page: 0,
            document_id: "test".into(),
            revision: "revision".into(),
            metadata: FragmentMetadata {
                start: 0,
                end: text.len(),
                line: 1,
                rows: 0,
                row_start: 0,
                table: None,
            },
            source: SourceWindow {
                projection: Vec::new(),
                references: Vec::new(),
                start: 0,
                end: text.len(),
                line: 1,
                end_line: 1,
                text: text.into(),
            },
            header: None,
        }
    }
    #[test]
    fn native_blocks_keep_unicode_crlf_source_offsets_and_styles() {
        let text = "# 中文\r\n\r\nHello **world**.\r\n\r\n```rust\r\nfn main() {}\r\n```\r\n";
        let blocks = parse(&fragment(text)).unwrap();
        assert!(matches!(
            &blocks[0].kind,
            BlockKind::Heading { level: 1, .. }
        ));
        assert_eq!(blocks[1].source.start, text.find("Hello").unwrap());
        assert_eq!(blocks[1].source.line, 3);
        assert!(
            matches!(&blocks[1].kind,BlockKind::Paragraph(content) if content.markup.contains("<b>world</b>"))
        );
        assert!(
            matches!(&blocks[2].kind,BlockKind::Code {language,text} if language=="rust" && text.contains("fn main()"))
        );
    }
    #[test]
    fn bare_diagrams_preserve_original_markers_and_protect_code_blocks() {
        let source="# Diagrams\r\n\r\n@startmindmap\r\n* 中文\r\n@endmindmap\r\n\r\n```text\r\n@startuml\r\nAlice -> Bob\r\n@enduml\r\n```\r\n";
        let blocks = parse(&fragment(source)).unwrap();
        assert!(
            matches!(&blocks[1].kind,BlockKind::Diagram{kind,source} if kind=="plantuml" && source.starts_with("@startmindmap") && source.contains("@endmindmap"))
        );
        assert!(matches!(&blocks[2].kind,BlockKind::Code{language,..} if language=="text"));
        assert_eq!(blocks[1].source.line, 3);
    }
    #[test]
    fn continuation_table_maps_header_and_body_to_their_original_ranges() {
        let header = "| 名称 | Value |\r\n|---|---|\r\n";
        let body = "| 中文😀 | **value** |\r\n";
        let mut page = fragment(body);
        page.source.start = 1000;
        page.source.end = 1000 + body.len();
        page.source.line = 200;
        page.header = Some(SourceWindow {
            projection: Vec::new(),
            references: Vec::new(),
            start: 100,
            end: 100 + header.len(),
            line: 10,
            end_line: 11,
            text: header.into(),
        });
        page.metadata.table = Some(2);
        page.metadata.row_start = 50;
        let blocks = parse(&page).unwrap();
        let BlockKind::Table {
            id,
            row_start,
            header,
            rows,
        } = &blocks[0].kind
        else {
            panic!("Missing native table")
        };
        assert_eq!(*id, Some(2));
        assert_eq!(*row_start, 50);
        assert_eq!(header[0].content.plain, "名称");
        assert!(header[0].source.start >= 100 && header[0].source.start < 100 + 100);
        assert_eq!(rows[0].source.start, 1000);
        assert_eq!(rows[0].source.line, 200);
        assert_eq!(rows[0].cells[0].content.plain, "中文😀");
        assert!(rows[0].cells[1].content.markup.contains("<b>value</b>"));
    }
    #[test]
    fn diagram_fences_remain_native_nodes_not_html() {
        let blocks = parse(&fragment(
            "```mermaid\ngraph TD\nA-->B\n```\n\n```d2\na -> b\n```\n",
        ))
        .unwrap();
        assert!(
            matches!(&blocks[0].kind,BlockKind::Diagram {kind,source} if kind=="mermaid" && source.contains("graph TD"))
        );
        assert!(matches!(&blocks[1].kind,BlockKind::Diagram {kind,..} if kind=="d2"));
    }
    #[test]
    fn embedded_image_metadata_retains_only_decoded_bytes_and_source_lines() {
        let source = format!("# Image\n\n[![中文😀](data:image/png;base64,{})](next.md)\n\n![Invalid](data:text/html,%3Cscript%3E)", "QUFB".repeat(4000));
        let blocks = parse(&fragment(&source)).unwrap();
        let BlockKind::Paragraph(text) = &blocks[1].kind else {
            panic!("Image paragraph missing")
        };
        assert_eq!(blocks[1].source.line, 3);
        assert_eq!(text.images[0].alternative, "中文😀");
        let super::ImageSource::Embedded(image) = &text.images[0].source else {
            panic!("Embedded image missing")
        };
        assert_eq!(image.bytes.len(), 12000);
        assert_eq!(image.bytes.as_ref(), vec![b'A'; 12000]);
        assert!(text.markup.contains("href=\"next.md\""));
        assert!(!text.markup.contains("data:"));
        assert!(text.markup.len() < 100);
        assert!(blocks[1].heap_bytes() >= 12000);
        let BlockKind::Paragraph(invalid) = &blocks[2].kind else {
            panic!("Invalid image paragraph missing")
        };
        assert!(matches!(
            invalid.images[0].source,
            super::ImageSource::Unavailable(_)
        ));
        assert!(!format!("{:?}", text.images[0]).contains("QUFB"));
    }
    #[test]
    fn images_keep_alt_links_and_source_anchors_with_bounded_preview_metadata() {
        let text="Text [![**中文**](images/a%20b.png)](docs/next.md)\n\n![one](one.png) ![two](two.svg) ![three](three.gif) ![four](four.webp) ![five](five.jpg)\n";
        let blocks = parse(&fragment(text)).unwrap();
        let BlockKind::Paragraph(first) = &blocks[0].kind else {
            panic!("Missing image paragraph")
        };
        assert_eq!(
            first.images[0].source,
            super::ImageSource::Reference("images/a%20b.png".into())
        );
        assert_eq!(first.images[0].alternative, "中文");
        assert_eq!(first.markup.matches("<a ").count(), 1);
        assert!(first.markup.contains("href=\"docs/next.md\""));
        let BlockKind::Paragraph(second) = &blocks[1].kind else {
            panic!("Missing image group")
        };
        assert_eq!(second.images.len(), 4);
        assert!(second.markup.contains("href=\"five.jpg\""));
        assert_eq!(blocks[1].source.line, 3);
        assert!(blocks[1].heap_bytes() > second.markup.len());
    }
}
