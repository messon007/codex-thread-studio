//! Experimental bounded, presentation-neutral text-flow projection.
//! A toolkit adapter can embed objects without losing their text or source
//! identity. Whole-fragment rendering remains experimental; bounded multiline
//! rich paragraphs share its native object/selection model in production.
use crate::{inline, BlockKind, Cell, Fragment, Row, SourceRange};
use std::ops::Range;

#[path = "flow_prefix.rs"]
mod prefix;
#[path = "flow_tables.rs"]
mod tables;
pub use prefix::{Prefix, PrefixResult, PrefixStep, TablePrefix};
pub const BUFFER_BYTES: usize = 512 * 1024;
pub const CONTEXT_FRAGMENTS: usize = 64;
pub const CONTEXT_BYTES: usize = 4 * BUFFER_BYTES;
pub const OBJECTS: usize = 256;
pub const SEGMENTS: usize = 2048;
pub const ANCHOR: char = '\u{fffc}';
pub const RICH_TEXT_BYTES: usize = crate::inline::MATH_SOURCE_BYTES + 4;

#[derive(Clone, Debug)]
pub enum ObjectKind {
    /// Native controls occupy one layout anchor but contribute no text to
    /// selection. The code itself stays selectable in the shared buffer.
    CodeHeader {
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
    Math {
        source: String,
        display: bool,
    },
    Image(crate::ImageReference),
}
#[derive(Clone, Debug)]
pub struct Object {
    /// A single toolkit anchor; alternative text stays in the worker model.
    pub characters: Range<usize>,
    pub alternative: String,
    pub kind: ObjectKind,
}
#[derive(Clone, Debug)]
pub enum Style {
    Inline(inline::Style),
    Heading(u8),
    Code(String),
    Paragraph { indent: u8, quote: bool },
}
#[derive(Clone, Debug)]
pub struct Span {
    pub characters: Range<usize>,
    pub style: Style,
}
#[derive(Clone, Debug)]
pub struct Link {
    pub characters: Range<usize>,
    pub destination: String,
}
#[derive(Clone, Debug)]
pub struct Segment {
    pub characters: Range<usize>,
    pub source: SourceRange,
}
#[derive(Clone, Debug, Default)]
pub struct Projection {
    pub buffer: String,
    pub characters: usize,
    pub objects: Vec<Object>,
    pub spans: Vec<Span>,
    pub links: Vec<Link>,
    pub segments: Vec<Segment>,
    /// Tiny rich paragraphs have a worker-prepared direct byte map, so native
    /// selection capture needs only a bounded substring copy on the UI thread.
    pub quick: Option<std::sync::Arc<QuickText>>,
}
#[derive(Clone, Debug)]
pub struct QuickText {
    text: String,
    bytes: Vec<usize>,
    source_bytes: Vec<usize>,
}
impl QuickText {
    pub fn source_range(&self, range: Range<usize>) -> Option<Range<usize>> {
        if range.start > range.end {
            return None;
        }
        let a = *self.bytes.get(range.start)?;
        let b = *self.bytes.get(range.end)?;
        Some(self.source_bytes.binary_search(&a).ok()?..self.source_bytes.binary_search(&b).ok()?)
    }
    pub fn projected_range(&self, range: Range<usize>) -> Option<Range<usize>> {
        if range.start > range.end {
            return None;
        }
        let a = *self.source_bytes.get(range.start)?;
        let b = *self.source_bytes.get(range.end)?;
        let start = self.bytes.partition_point(|p| *p <= a).saturating_sub(1);
        let end = self
            .bytes
            .partition_point(|p| *p < b)
            .min(self.bytes.len() - 1);
        Some(start..end.max(start))
    }
    pub fn select(&self, range: Range<usize>, limit: usize) -> Option<&str> {
        if range.start > range.end {
            return None;
        }
        let text = self
            .text
            .get(*self.bytes.get(range.start)?..*self.bytes.get(range.end)?)?;
        (text.len() <= limit).then_some(text)
    }
}
/// Consume already-parsed rich text without adding block separators or losing
/// original CRLF, styles, links or literal object characters. Source ownership
/// remains with the containing document/conversation block.
pub fn project_rich(content: crate::RichText) -> Result<Projection, String> {
    if content.plain.len() > RICH_TEXT_BYTES {
        return Err("Rich paragraph exceeds its bounded projection budget".into());
    }
    let original = content.plain.clone();
    let mut result = Projection::default();
    rich(&mut result, content)?;
    let mut objects = result.objects.iter().peekable();
    let mut cursor = 0;
    let mut bytes = vec![0];
    for (index, character) in result.buffer.chars().enumerate() {
        let mut encoded = [0; 4];
        let part = if objects.peek().is_some_and(|o| o.characters.start == index) {
            objects.next().unwrap().alternative.as_str()
        } else {
            character.encode_utf8(&mut encoded)
        };
        if original.get(cursor..cursor + part.len()) != Some(part) {
            return Err("Rich paragraph selection mapping differs from source".into());
        }
        cursor += part.len();
        bytes.push(cursor);
    }
    if cursor != original.len() {
        return Err("Incomplete rich paragraph selection mapping".into());
    }
    result.quick = Some(std::sync::Arc::new(QuickText {
        source_bytes: original
            .char_indices()
            .map(|(b, _)| b)
            .chain(std::iter::once(original.len()))
            .collect(),
        text: original,
        bytes,
    }));
    Ok(result)
}
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TableText {
    pub id: usize,
    pub text: String,
    pub source: SourceRange,
}
#[derive(Clone, Debug)]
pub struct SelectionRequest {
    pub tables: Vec<usize>,
    pub remaining: usize,
}
#[derive(Clone, Debug)]
pub struct Selection {
    pub characters: Range<usize>,
    pub text: String,
    pub source: Option<SourceRange>,
}

impl Projection {
    fn push(&mut self, text: &str) -> Result<(), String> {
        if self.buffer.len().saturating_add(text.len()) > BUFFER_BYTES {
            return Err("Text-flow buffer exceeds 512 KiB".into());
        }
        self.buffer.push_str(text);
        self.characters += text.chars().count();
        Ok(())
    }
    fn object(&mut self, kind: ObjectKind, alternative: String) -> Result<(), String> {
        if self.objects.len() >= OBJECTS {
            return Err("Text-flow object budget exceeded".into());
        }
        let start = self.characters;
        self.push("\u{fffc}")?;
        self.objects.push(Object {
            characters: start..self.characters,
            alternative,
            kind,
        });
        Ok(())
    }
    /// Toolkit offsets are Unicode scalars. Objects are atomic selections; only
    /// synthetic anchors are expanded, never a literal U+FFFC from the source.
    pub fn select(&self, range: Range<usize>, limit: usize) -> Result<Selection, String> {
        self.select_impl(range, limit, None)
    }
    /// Freeze table identities and the remaining byte allowance without copying
    /// partial indexed-table alternatives into a native clipboard offer.
    pub fn selection_request(
        &self,
        range: Range<usize>,
        limit: usize,
    ) -> Result<SelectionRequest, String> {
        if range.start > range.end || range.end > self.characters {
            return Err("Invalid text-flow selection".into());
        }
        let mut tables = Vec::new();
        let mut placeholders = Vec::new();
        if !range.is_empty() {
            for object in &self.objects {
                if range.start < object.characters.end && range.end > object.characters.start {
                    if let ObjectKind::Table { id: Some(id), .. } = object.kind {
                        if tables.contains(&id) {
                            return Err("Repeated indexed table selection".into());
                        }
                        tables.push(id);
                        placeholders.push(TableText {
                            id,
                            text: String::new(),
                            source: self
                                .source_for(&object.characters)
                                .ok_or("Missing table source range")?,
                        });
                    }
                }
            }
        }
        let base = self.select_impl(range, limit, Some(&placeholders))?;
        Ok(SelectionRequest {
            tables,
            remaining: limit - base.text.len(),
        })
    }
    pub fn select_resolved(
        &self,
        range: Range<usize>,
        limit: usize,
        tables: &[TableText],
    ) -> Result<Selection, String> {
        let request = self.selection_request(range.clone(), limit)?;
        if tables.len() != request.tables.len()
            || tables
                .iter()
                .enumerate()
                .any(|(i, t)| request.tables[i] != t.id)
        {
            return Err("Resolved table selection identities changed".into());
        }
        self.select_impl(range, limit, Some(tables))
    }
    fn select_impl(
        &self,
        mut range: Range<usize>,
        limit: usize,
        tables: Option<&[TableText]>,
    ) -> Result<Selection, String> {
        if range.start > range.end || range.end > self.characters {
            return Err("Invalid text-flow selection".into());
        }
        if range.is_empty() {
            return Ok(Selection {
                characters: range,
                text: String::new(),
                source: None,
            });
        }
        for object in &self.objects {
            if range.start < object.characters.end && range.end > object.characters.start {
                range.start = range.start.min(object.characters.start);
                range.end = range.end.max(object.characters.end);
            }
        }
        let mut source = self.source_for(&range);
        let mut objects = self.objects.iter().peekable();
        let mut text = String::new();
        for (index, c) in self
            .buffer
            .chars()
            .enumerate()
            .take(range.end)
            .skip(range.start)
        {
            while objects.peek().is_some_and(|o| o.characters.start < index) {
                objects.next();
            }
            let mut encoded = [0; 4];
            let part = match objects.peek().filter(|o| o.characters.start == index) {
                Some(object) => {
                    if let (ObjectKind::Table { id: Some(id), .. }, Some(tables)) =
                        (&object.kind, tables)
                    {
                        let table = tables
                            .iter()
                            .find(|t| t.id == *id)
                            .ok_or("Missing resolved table selection")?;
                        let original = self
                            .source_for(&object.characters)
                            .ok_or("Missing table source range")?;
                        if table.source.start > original.start
                            || table.source.end < original.end
                            || table.source.line > original.line
                            || table.source.end_line < original.end_line
                            || table.source.start > table.source.end
                            || table.source.line == 0
                        {
                            return Err(
                                "Resolved table does not contain the original source range".into(),
                            );
                        }
                        if let Some(source) = &mut source {
                            source.start = source.start.min(table.source.start);
                            source.end = source.end.max(table.source.end);
                            source.line = source.line.min(table.source.line);
                            source.end_line = source.end_line.max(table.source.end_line);
                        }
                        table.text.as_str()
                    } else {
                        object.alternative.as_str()
                    }
                }
                None => c.encode_utf8(&mut encoded),
            };
            if text.len().saturating_add(part.len()) > limit {
                return Err("Text-flow selection exceeds its byte budget".into());
            }
            text.push_str(part);
        }
        Ok(Selection {
            characters: range,
            text,
            source,
        })
    }
    fn source_for(&self, range: &Range<usize>) -> Option<SourceRange> {
        let first = self
            .segments
            .partition_point(|s| s.characters.end <= range.start);
        let mut source: Option<SourceRange> = None;
        for segment in self.segments[first..]
            .iter()
            .take_while(|s| s.characters.start < range.end)
        {
            let start = segment.characters.start.max(range.start);
            let end = segment.characters.end.min(range.end);
            // Textless controls are not quoted source. Keep full segment
            // coverage for layout/scanning, but don't let selecting a header
            // (especially a trailing empty fence) widen a comment's range.
            let object = self.objects.partition_point(|o| o.characters.end <= start);
            let control_only = self.objects.get(object).is_some_and(|o| {
                matches!(o.kind, ObjectKind::CodeHeader { .. })
                    && o.characters.start <= start
                    && end <= o.characters.end
            });
            if start >= end || control_only {
                continue;
            }
            match &mut source {
                None => source = Some(segment.source.clone()),
                Some(value) => {
                    value.start = value.start.min(segment.source.start);
                    value.end = value.end.max(segment.source.end);
                    value.line = value.line.min(segment.source.line);
                    value.end_line = value.end_line.max(segment.source.end_line);
                }
            }
        }
        source
    }
}

#[derive(Clone)]
struct QuotePiece {
    bytes: Range<usize>,
    source: Option<SourceRange>,
    ordinal: usize,
    atomic: bool,
}
/// Bounded rendered-text matching across successive indexed fragments. Keeps
/// only the quote-sized overlap and its source provenance, never old documents.
/// Large atomic alternatives cannot fit a comment and break the overlap.
pub struct QuoteScanner {
    quote: String,
    finder: memchr::memmem::Finder<'static>,
    checked: usize,
    text: String,
    pieces: Vec<QuotePiece>,
    base: usize,
    started: bool,
    stopped: bool,
}
impl QuoteScanner {
    /// A known atomic object too large for the quote cannot contain a valid
    /// partial match; prevent overlap from bridging across the skipped object.
    pub fn boundary(&mut self) {
        self.base = self.base.saturating_add(self.text.len()).saturating_add(1);
        self.checked = self.base;
        self.text.clear();
        self.pieces.clear();
        self.started = false;
    }
    pub fn new(quote: &str) -> Result<Self, String> {
        if quote.trim().is_empty() || quote.len() > crate::quote::MAX_QUOTE || quote.contains('\0')
        {
            return Err("Invalid text-flow quote".into());
        }
        Ok(Self {
            quote: quote.into(),
            finder: memchr::memmem::Finder::new(quote).into_owned(),
            checked: 0,
            text: String::new(),
            pieces: Vec::new(),
            base: 0,
            started: false,
            stopped: false,
        })
    }
    fn push(
        &mut self,
        part: &str,
        source: Option<&SourceRange>,
        ordinal: usize,
        atomic: bool,
        visit: &mut impl FnMut(SourceRange, usize) -> bool,
    ) {
        if self.stopped || part.is_empty() {
            return;
        }
        if atomic && part.len() > self.quote.len() {
            self.scan(visit);
            self.base += self.text.len() + part.len();
            self.text.clear();
            self.pieces.clear();
            self.checked = self.base;
            return;
        }
        let mut cursor = 0;
        while cursor < part.len() && !self.stopped {
            let mut end = if atomic {
                part.len()
            } else {
                let pending = self.base + self.text.len() - self.checked;
                (cursor + (64 * 1024usize).saturating_sub(pending).max(4)).min(part.len())
            };
            while !part.is_char_boundary(end) {
                end -= 1;
            }
            let start = self.text.len();
            self.text.push_str(&part[cursor..end]);
            self.pieces.push(QuotePiece {
                bytes: start..self.text.len(),
                source: source.cloned(),
                ordinal: ordinal + cursor,
                atomic,
            });
            if self.base + self.text.len() - self.checked >= 64 * 1024 {
                self.scan(visit);
            }
            cursor = end;
        }
    }
    fn scan(&mut self, visit: &mut impl FnMut(SourceRange, usize) -> bool) {
        let mut search = 0;
        while let Some(relative) = self.finder.find(self.text[search..].as_bytes()) {
            let start = search + relative;
            let end = start + self.quote.len();
            if self.base + end > self.checked {
                let first = self.pieces.partition_point(|p| p.bytes.end <= start);
                let last = self.pieces.partition_point(|p| p.bytes.start < end);
                let pieces = &self.pieces[first..last];
                let complete = pieces
                    .iter()
                    .all(|p| !p.atomic || (start <= p.bytes.start && end >= p.bytes.end));
                if complete {
                    let mut range: Option<SourceRange> = None;
                    let mut occurrence = 0;
                    for piece in pieces {
                        if let Some(source) = &piece.source {
                            match &mut range {
                                None => {
                                    range = Some(source.clone());
                                    occurrence = if start < piece.bytes.start {
                                        usize::MAX - (piece.bytes.start - start)
                                    } else {
                                        piece.ordinal + start - piece.bytes.start
                                    };
                                }
                                Some(range) => {
                                    range.start = range.start.min(source.start);
                                    range.end = range.end.max(source.end);
                                    range.line = range.line.min(source.line);
                                    range.end_line = range.end_line.max(source.end_line);
                                }
                            }
                        }
                    }
                    if let Some(range) = range {
                        if !visit(range, occurrence) {
                            self.stopped = true;
                            break;
                        }
                    }
                }
            }
            search = start + self.text[start..].chars().next().unwrap().len_utf8();
        }
        self.checked = self.base + self.text.len();
        let mut cut = self.text.len().saturating_sub(self.quote.len() + 3);
        while !self.text.is_char_boundary(cut) {
            cut -= 1;
        }
        if let Some(piece) = self
            .pieces
            .iter()
            .find(|p| p.atomic && p.bytes.start < cut && p.bytes.end > cut)
        {
            cut = piece.bytes.end;
        }
        self.pieces.retain_mut(|p| {
            if p.bytes.end <= cut {
                return false;
            }
            let dropped = cut.saturating_sub(p.bytes.start);
            p.ordinal += dropped;
            p.bytes.start = p.bytes.start.max(cut) - cut;
            p.bytes.end -= cut;
            true
        });
        self.text.drain(..cut);
        self.base += cut;
    }
    pub fn feed(
        &mut self,
        projection: &Projection,
        mut visit: impl FnMut(SourceRange, usize) -> bool,
    ) {
        if self.stopped || projection.buffer.is_empty() {
            return;
        }
        if self.started {
            self.push("\n\n", None, 0, false, &mut visit);
        }
        self.started = true;
        let offsets = projection
            .buffer
            .char_indices()
            .map(|(byte, _)| byte)
            .chain(std::iter::once(projection.buffer.len()))
            .collect::<Vec<_>>();
        let mut previous = 0;
        let mut objects = projection.objects.iter().peekable();
        for segment in &projection.segments {
            self.push(
                &projection.buffer[offsets[previous]..offsets[segment.characters.start]],
                None,
                0,
                false,
                &mut visit,
            );
            let mut cursor = segment.characters.start;
            let mut ordinal = 0;
            while let Some(object) = objects
                .peek()
                .filter(|o| o.characters.start < segment.characters.end)
            {
                let part = &projection.buffer[offsets[cursor]..offsets[object.characters.start]];
                self.push(part, Some(&segment.source), ordinal, false, &mut visit);
                ordinal += part.len();
                self.push(
                    &object.alternative,
                    Some(&segment.source),
                    ordinal,
                    true,
                    &mut visit,
                );
                ordinal += object.alternative.len();
                cursor = object.characters.end;
                objects.next();
            }
            self.push(
                &projection.buffer[offsets[cursor]..offsets[segment.characters.end]],
                Some(&segment.source),
                ordinal,
                false,
                &mut visit,
            );
            previous = segment.characters.end;
            if self.stopped {
                break;
            }
        }
        if !self.stopped {
            self.scan(&mut visit);
        }
    }
}

pub fn project(fragment: &Fragment) -> Result<Projection, String> {
    project_context(std::slice::from_ref(fragment))
}
/// Bounded ordered context from one immutable document snapshot. Parsing and
/// continuation joining belong on the worker that owns these source fragments.
pub fn project_context(fragments: &[Fragment]) -> Result<Projection, String> {
    if fragments.len() > CONTEXT_FRAGMENTS {
        return Err("Text-flow context exceeds its fragment budget".into());
    }
    if let Some(first) = fragments.first() {
        if fragments.len() > 1 && (first.document_id.is_empty() || first.revision.is_empty()) {
            return Err("Text-flow context requires a document snapshot identity".into());
        }
        let mut bytes = 0usize;
        for (index, fragment) in fragments.iter().enumerate() {
            if fragment.document_id != first.document_id || fragment.revision != first.revision {
                return Err("Text-flow context mixes document snapshots".into());
            }
            if index > 0
                && (fragments[index - 1].page >= fragment.page
                    || fragments[index - 1].source.end > fragment.source.start
                    || fragments[index - 1].source.end_line > fragment.source.line)
            {
                return Err("Text-flow context pages or source windows are not ordered".into());
            }
            bytes = bytes
                .saturating_add(fragment.source.text.len())
                .saturating_add(fragment.header.as_ref().map_or(0, |h| h.text.len()));
            if bytes > CONTEXT_BYTES {
                return Err("Text-flow context exceeds its source-text budget".into());
            }
        }
    }
    let mut rows = Vec::new();
    for fragment in fragments {
        let parsed = crate::display_rows(crate::parse_inline(fragment, true)?);
        if rows.len().saturating_add(parsed.len()) > SEGMENTS {
            return Err("Text-flow segment budget exceeded".into());
        }
        rows.extend(parsed);
    }
    project_rows(rows)
}
/// Reuse already parsed blocks from one snapshot, including source/image mappings.
pub fn project_blocks(blocks: Vec<crate::Block>) -> Result<Projection, String> {
    project_rows(crate::display_rows(blocks))
}
fn project_rows(rows: Vec<crate::DisplayBlock>) -> Result<Projection, String> {
    if rows.len() > SEGMENTS {
        return Err("Text-flow segment budget exceeded".into());
    }
    let rows = tables::coalesce(rows)?;
    let mut result = Projection::default();
    for row in rows {
        if !result.buffer.is_empty() {
            result.push("\n\n")?;
        }
        let start = result.characters;
        if let Some(marker) = row.marker {
            result.push(&format!("{marker} "))?;
        }
        match row.block.kind {
            BlockKind::Paragraph(content) => rich(&mut result, content)?,
            BlockKind::Heading { level, content } => {
                rich(&mut result, content)?;
                result.spans.push(Span {
                    characters: start..result.characters,
                    style: Style::Heading(level),
                });
            }
            BlockKind::Code { language, text } => {
                result.object(
                    ObjectKind::CodeHeader {
                        language: language.clone(),
                        text: text.clone(),
                    },
                    String::new(),
                )?;
                let code_start = result.characters;
                result.push(&text)?;
                result.spans.push(Span {
                    characters: code_start..result.characters,
                    style: Style::Code(language),
                });
            }
            BlockKind::Table {
                id,
                row_start,
                header,
                rows,
            } => {
                let mut alternative = header
                    .iter()
                    .map(|c| c.content.plain.as_str())
                    .collect::<Vec<_>>()
                    .join("\t");
                if alternative.len() > BUFFER_BYTES {
                    return Err("Text-flow table exceeds its byte budget".into());
                }
                for row in &rows {
                    if alternative
                        .len()
                        .saturating_add(
                            row.cells
                                .iter()
                                .map(|c| c.content.plain.len())
                                .sum::<usize>(),
                        )
                        .saturating_add(row.cells.len() + 1)
                        > BUFFER_BYTES
                    {
                        return Err("Text-flow table exceeds its byte budget".into());
                    }
                    alternative.push('\n');
                    alternative.push_str(
                        &row.cells
                            .iter()
                            .map(|c| c.content.plain.as_str())
                            .collect::<Vec<_>>()
                            .join("\t"),
                    );
                }
                result.object(
                    ObjectKind::Table {
                        id,
                        row_start,
                        header,
                        rows,
                    },
                    alternative,
                )?;
            }
            BlockKind::Diagram { kind, source } => {
                result.object(
                    ObjectKind::Diagram {
                        kind,
                        source: source.clone(),
                    },
                    source,
                )?;
            }
            BlockKind::Rule => result.push("—")?,
            _ => return Err("Unflattened text-flow block".into()),
        }
        result.spans.push(Span {
            characters: start..result.characters,
            style: Style::Paragraph {
                indent: row.indent,
                quote: row.quote,
            },
        });
        result.segments.push(Segment {
            characters: start..result.characters,
            source: row.block.source,
        });
    }
    Ok(result)
}

fn rich(output: &mut Projection, content: crate::RichText) -> Result<(), String> {
    let mut replacements = Vec::with_capacity(content.images.len() + content.math.len());
    for math in &content.math {
        let alternative = content
            .plain
            .get(math.range.clone())
            .ok_or("Invalid text-flow math range")?
            .to_owned();
        let source = content
            .plain
            .get(math.body.clone())
            .ok_or("Invalid text-flow formula body")?
            .to_owned();
        replacements.push((
            math.characters.clone(),
            alternative,
            ObjectKind::Math {
                source,
                display: math.display,
            },
        ));
    }
    let characters: Vec<_> = content.plain.chars().collect();
    for image in &content.images {
        let alternative = characters
            .get(image.characters.clone())
            .ok_or("Invalid text-flow image range")?
            .iter()
            .collect();
        replacements.push((
            image.characters.clone(),
            alternative,
            ObjectKind::Image(image.clone()),
        ));
    }
    // Stable sorting keeps consecutive images with empty alt text in order.
    replacements.sort_by_key(|(range, _, _)| range.start);
    let mut previous_end = 0;
    for (range, _, _) in &replacements {
        if range.start < previous_end || range.end < range.start || range.end > characters.len() {
            return Err("Overlapping or invalid text-flow objects".into());
        }
        previous_end = range.end;
    }
    let mut replacements = replacements.into_iter().peekable();
    let mut starts = Vec::with_capacity(characters.len() + 1);
    let mut ends = Vec::with_capacity(characters.len() + 1);
    let mut skip_until = 0;
    for index in 0..=characters.len() {
        ends.push(output.characters);
        let mut start = output.characters;
        while replacements
            .peek()
            .is_some_and(|(range, _, _)| range.start == index)
        {
            let (range, alternative, kind) = replacements.next().unwrap();
            skip_until = range.end;
            output.object(kind, alternative)?;
            if range.is_empty() {
                // A following word/link begins after an empty-alt image;
                // the preceding word ends before it. One boundary array
                // cannot represent both sides of this inserted anchor.
                start = output.characters;
            }
        }
        starts.push(start);
        if index >= skip_until {
            if let Some(c) = characters.get(index) {
                output.push(c.encode_utf8(&mut [0; 4]))?;
            }
        }
    }
    if let Some(attributes) = content.attributes {
        for span in &attributes.spans {
            let (Some(&start), Some(&end)) = (starts.get(span.start), ends.get(span.end)) else {
                return Err("Invalid text-flow style mapping".into());
            };
            if start < end {
                output.spans.push(Span {
                    characters: start..end,
                    style: Style::Inline(span.style),
                });
            }
        }
        for link in &attributes.links {
            let (Some(&start), Some(&end)) = (starts.get(link.start), ends.get(link.end)) else {
                return Err("Invalid text-flow link mapping".into());
            };
            if start < end {
                output.links.push(Link {
                    characters: start..end,
                    destination: link.destination.clone(),
                });
            }
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "flow_selection_tests.rs"]
mod selection_tests;

#[cfg(test)]
#[path = "flow_context_tests.rs"]
mod context_tests;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn quote_search_uses_copy_text_unicode_and_atomic_objects() {
        let input="# Heading\n\nBefore **中文😀** $x^2$.\n\n| A | B |\n|---|---|\n| one | two |\n\nAfter\n";
        let flow = project(&fragment(input)).unwrap();
        let quote = flow.select(0..flow.characters, BUFFER_BYTES).unwrap();
        let mut found = Vec::new();
        QuoteScanner::new(&quote.text)
            .unwrap()
            .feed(&flow, |source, _| {
                found.push(source);
                true
            });
        assert_eq!(found, vec![quote.source.unwrap()]);
        let mut partial = 0;
        QuoteScanner::new("x^2").unwrap().feed(&flow, |_, _| {
            partial += 1;
            true
        });
        assert_eq!(partial, 0);
        let table = flow
            .objects
            .iter()
            .find(|o| matches!(o.kind, ObjectKind::Table { .. }))
            .unwrap();
        let mut found = Vec::new();
        QuoteScanner::new(&table.alternative)
            .unwrap()
            .feed(&flow, |source, _| {
                found.push(source);
                true
            });
        assert_eq!(
            found,
            vec![flow
                .select(table.characters.clone(), BUFFER_BYTES)
                .unwrap()
                .source
                .unwrap()]
        );
    }
    #[test]
    fn flow_quote_relocation_is_scoped_deduplicated_and_keeps_legacy_preview() {
        use crate::quote::{Anchor, Kind, Locator, Query, Status};
        let flow = project(&fragment(
            "Same **中文😀**.\n\nSame **中文😀**.\n\nSame **中文😀**.\n",
        ))
        .unwrap();
        let mut query = Query {
            quote: "Same 中文😀.".into(),
            anchor: Anchor {
                base_hash: "rev".into(),
                start_line: Some(5),
                end_line: Some(5),
                text_kind: Kind::Flow,
            },
        };
        let mut locator = Locator::new(query.clone(), "rev").unwrap();
        locator.flow(&flow);
        locator.flow(&flow);
        let found = locator.finish();
        assert_eq!(found.status, Status::Unique);
        assert_eq!(found.position.unwrap().line, 5);
        assert_eq!(found.text_kind, Some(Kind::Flow));
        query.anchor.base_hash = "old".into();
        let mut locator = Locator::new(query, "new").unwrap();
        locator.flow(&flow);
        assert_eq!(locator.finish().status, Status::Ambiguous);
        let blocks = crate::parse(&fragment("**中文😀**\n")).unwrap();
        let mut legacy = Locator::new(
            Query {
                quote: "中文😀".into(),
                anchor: Anchor {
                    text_kind: Kind::Preview,
                    ..Anchor::default()
                },
            },
            "rev",
        )
        .unwrap();
        legacy.preview(&blocks);
        legacy.flow(&project_blocks(blocks).unwrap());
        assert_eq!(legacy.finish().status, Status::Unique);
    }
    #[test]
    fn flow_quotes_keep_generated_markers_literal_anchors_and_explicit_limits() {
        use crate::quote::{Anchor, Kind, Locator, Query, Status};
        let flow = project(&fragment("- item\n\nText \u{fffc} literal.\n\n---\n")).unwrap();
        let text = flow.select(0..flow.characters, BUFFER_BYTES).unwrap().text;
        assert!(text.contains('\u{fffc}'));
        let mut locator = Locator::new(
            Query {
                quote: text,
                anchor: Anchor {
                    text_kind: Kind::Flow,
                    ..Anchor::default()
                },
            },
            "rev",
        )
        .unwrap();
        locator.flow(&flow);
        assert_eq!(locator.finish().status, Status::Unique);
        assert!(QuoteScanner::new("").is_err());
        assert!(QuoteScanner::new(&"x".repeat(crate::quote::MAX_QUOTE + 1)).is_err());
        let repeated = project(&fragment("aaaaa")).unwrap();
        let mut hits = 0;
        QuoteScanner::new("aaa").unwrap().feed(&repeated, |_, _| {
            hits += 1;
            hits < 2
        });
        assert_eq!(hits, 2);
    }
    #[test]
    fn quotes_bridge_indexed_table_pages_without_retaining_documents() {
        use crate::quote::{Anchor, Kind, Locator, Query, Status};
        let parts = [
            "# H",
            "Before **中文😀**.",
            "| A | B |\n|---|---|\n| one | two |",
            "After.",
        ];
        let source = parts.join("\n\n");
        let complete = project(&fragment(&source)).unwrap();
        let quote = complete
            .select(0..complete.characters, BUFFER_BYTES)
            .unwrap();
        let mut locator = Locator::new(
            Query {
                quote: quote.text.clone(),
                anchor: Anchor {
                    text_kind: Kind::Flow,
                    ..Anchor::default()
                },
            },
            "rev",
        )
        .unwrap();
        let (mut offset, mut line) = (0, 1);
        for part in parts {
            let mut input = fragment(part);
            input.metadata.start = offset;
            input.metadata.end = offset + part.len();
            input.metadata.line = line;
            input.source.start = offset;
            input.source.end = offset + part.len();
            input.source.line = line;
            input.source.end_line = line + part.lines().count() - 1;
            locator.flow(&project_blocks(crate::parse(&input).unwrap()).unwrap());
            offset += part.len() + 2;
            line += part.bytes().filter(|c| *c == b'\n').count() + 2;
        }
        let found = locator.finish();
        assert_eq!(found.status, Status::Unique);
        assert_eq!(
            found.position.unwrap(),
            crate::quote::Position::from(&quote.source.unwrap())
        );
        let mut scanner = QuoteScanner::new("absent").unwrap();
        let plain = project(&fragment(&"中😀".repeat(50000))).unwrap();
        for _ in 0..12 {
            scanner.feed(&plain, |_, _| true);
            assert!(scanner.text.len() <= scanner.quote.len() + 6);
            assert!(scanner.pieces.len() < 10);
        }
        // Real matches straddle scan batches and UTF-8 boundaries. Neither
        // retained overlap nor the final flush may replay an earlier hit.
        let text = format!(
            "{}needle中文😀{}needle中文😀end",
            "x".repeat(65533),
            "y".repeat(65526)
        );
        let mut found = Vec::new();
        QuoteScanner::new("needle中文😀").unwrap().feed(
            &project(&fragment(&text)).unwrap(),
            |_, ordinal| {
                found.push(ordinal);
                true
            },
        );
        assert_eq!(
            found,
            text.match_indices("needle中文😀")
                .map(|(offset, _)| offset)
                .collect::<Vec<_>>()
        );
        let skipped = project(&fragment(
            "needle\n\n| A |\n|---|\n| needle inside a larger atomic object |\n\nneedle",
        ))
        .unwrap();
        let mut found = Vec::new();
        QuoteScanner::new("needle")
            .unwrap()
            .feed(&skipped, |source, _| {
                found.push(source.line);
                true
            });
        assert_eq!(found, vec![1, 7]);
    }
    fn fragment(text: &str) -> Fragment {
        Fragment {
            page: 0,
            document_id: "test".into(),
            revision: "revision".into(),
            metadata: crate::FragmentMetadata {
                start: 0,
                end: text.len(),
                line: 1,
                rows: 0,
                row_start: 0,
                table: None,
            },
            source: crate::SourceWindow {
                start: 0,
                end: text.len(),
                line: 1,
                end_line: text.lines().count(),
                text: text.into(),
                projection: vec![],
                references: vec![],
            },
            header: None,
        }
    }
    #[test]
    fn indexed_tables_keep_identity_row_offset_and_original_cells() {
        let mut input = fragment("| Name | Value |\n|---|---|\n| 中文😀 | $x^2$ |\n");
        input.metadata.table = Some(7);
        input.metadata.row_start = 150;
        input.metadata.rows = 1;
        let projected = project(&input).unwrap();
        let ObjectKind::Table {
            id,
            row_start,
            header,
            rows,
        } = &projected.objects[0].kind
        else {
            panic!("Missing indexed table")
        };
        assert_eq!(*id, Some(7));
        assert_eq!(*row_start, 150);
        assert_eq!(header.len(), 2);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].cells[0].content.plain, "中文😀");
        assert_eq!(rows[0].cells[0].source.line, 3);
        // Worker fragment extraction remains available to quote scanners. The
        // native view separately guards whole-table Copy until it can stream
        // across index pages rather than silently exporting a partial table.
        assert!(projected
            .select(0..projected.characters, BUFFER_BYTES)
            .unwrap()
            .text
            .contains("中文😀"));
    }
    #[test]
    fn code_controls_are_textless_and_bodies_keep_unicode_selection_offsets() {
        let flow = project(&fragment(
            "Before.\n\n```rust\nlet 中文 = \"😀￼\";\n```\n\nAfter.\n\n```\n```\n",
        ))
        .unwrap();
        let headers = flow
            .objects
            .iter()
            .filter(|o| matches!(o.kind, ObjectKind::CodeHeader { .. }))
            .collect::<Vec<_>>();
        assert_eq!(headers.len(), 2);
        for header in &headers {
            assert!(header.alternative.is_empty());
            assert!(flow
                .select(header.characters.clone(), BUFFER_BYTES)
                .unwrap()
                .text
                .is_empty());
            assert!(flow
                .select(header.characters.clone(), BUFFER_BYTES)
                .unwrap()
                .source
                .is_none());
        }
        assert!(
            matches!(&headers[0].kind,ObjectKind::CodeHeader{language,text} if language=="rust" && text=="let 中文 = \"😀￼\";\n")
        );
        assert!(
            matches!(&headers[1].kind,ObjectKind::CodeHeader{language,text} if language.is_empty() && text.is_empty())
        );
        let bodies = flow
            .spans
            .iter()
            .filter(|s| matches!(s.style, Style::Code(_)))
            .collect::<Vec<_>>();
        assert_eq!(bodies.len(), 2);
        assert_eq!(
            flow.select(bodies[0].characters.clone(), BUFFER_BYTES)
                .unwrap()
                .text,
            "let 中文 = \"😀￼\";\n"
        );
        assert!(bodies[1].characters.is_empty());
        let selected = flow.select(0..flow.characters, BUFFER_BYTES).unwrap();
        assert_eq!(
            selected.text,
            "Before.\n\nlet 中文 = \"😀￼\";\n\n\nAfter.\n\n"
        );
        assert_eq!(selected.text.matches(ANCHOR).count(), 1);
        let mut found = Vec::new();
        QuoteScanner::new(&selected.text)
            .unwrap()
            .feed(&flow, |source, _| {
                found.push(source);
                true
            });
        assert_eq!(found, vec![selected.source.clone().unwrap()]);
        assert_eq!(selected.source.unwrap().line, 1);
        // Synthetic header positions are outside syntax ranges; including them
        // in a selection must not include language labels or executable code twice.
        assert_eq!(bodies[0].characters.start, headers[0].characters.end);
    }
    #[test]
    fn unicode_cross_block_copy_preserves_styles_links_and_containing_lines() {
        let input="# Heading\r\n\r\n**中文😀** [link](https://example.invalid)  \r\nnext\r\n\r\n```rust\r\nlet x = 1;\r\n```\r\n";
        let flow = project(&fragment(input)).unwrap();
        let selection = flow.select(0..flow.characters, BUFFER_BYTES).unwrap();
        assert!(selection.text.contains("中文😀 link") && selection.text.contains("let x = 1;"));
        assert!(!selection.text.contains("**") && !selection.text.contains("```"));
        assert_eq!(selection.source.unwrap().line, 1);
        assert!(flow
            .links
            .iter()
            .any(|l| l.destination == "https://example.invalid"));
        assert!(flow
            .spans
            .iter()
            .any(|s| matches!(s.style, Style::Inline(inline::Style::Bold))));
        assert!(flow.select(0..flow.characters, 3).is_err());
    }
    #[test]
    fn synthetic_objects_are_atomic_and_literal_replacement_characters_survive() {
        let flow=project(&fragment("Before ￼ $x^2$ after.\n\n| A | B |\n|---|---|\n| 中文😀 | two |\n\n```mermaid\nflowchart LR\nA-->B\n```\n")).unwrap();
        assert_eq!(flow.objects.len(), 3);
        let copy = flow.select(0..flow.characters, BUFFER_BYTES).unwrap();
        assert_eq!(copy.text.matches(ANCHOR).count(), 1);
        assert!(
            copy.text.contains("$x^2$")
                && copy.text.contains("A\tB\n中文😀\ttwo")
                && copy.text.contains("A-->B")
        );
        for object in &flow.objects {
            let picked = flow
                .select(
                    object.characters.start..object.characters.start + 1,
                    BUFFER_BYTES,
                )
                .unwrap();
            assert_eq!(picked.text, object.alternative);
            assert_eq!(picked.characters, object.characters);
        }
    }
    #[test]
    fn explicit_selection_limits_do_not_silently_discard_content() {
        let flow = project(&fragment("plain")).unwrap();
        assert!(flow.select(2..1, 10).is_err());
        assert!(flow.select(0..100, 10).is_err());
        assert!(flow.select(0..0, 0).unwrap().text.is_empty());
    }
    #[test]
    fn images_preserve_unicode_empty_alts_links_math_and_source_lines() {
        let text =
            "中文😀 [![**图😀**](one.png)](next.md) ![](empty.svg)![](second.png) $x+1$ **tail**\n";
        let flow = project(&fragment(text)).unwrap();
        assert_eq!(flow.objects.len(), 4);
        let names: Vec<_> = flow.objects[..3]
            .iter()
            .map(|object| match &object.kind {
                ObjectKind::Image(image) => &image.source,
                _ => panic!("Missing image"),
            })
            .collect();
        assert_eq!(
            names,
            vec![
                &crate::ImageSource::Reference("one.png".into()),
                &crate::ImageSource::Reference("empty.svg".into()),
                &crate::ImageSource::Reference("second.png".into()),
            ]
        );
        let copy = flow.select(0..flow.characters, BUFFER_BYTES).unwrap();
        assert_eq!(copy.text, "中文😀 图😀  $x+1$ tail");
        assert_eq!(copy.source.unwrap().line, 1);
        let first = &flow.objects[0];
        assert_eq!(flow.links[0].characters, first.characters);
        assert_eq!(
            flow.select(first.characters.clone(), 100).unwrap().text,
            "图😀"
        );
        for object in &flow.objects[1..3] {
            assert_eq!(object.characters.len(), 1);
            assert_eq!(flow.select(object.characters.clone(), 0).unwrap().text, "");
        }
    }
    #[test]
    fn image_alt_copy_is_complete_and_nested_alt_math_is_not_a_second_object() {
        let alt = "中文😀".repeat(300);
        let text = format!("![{alt}](large.png)\n\n![outer ![inner](inner.png) $x$](outer.png)\n");
        let flow = project(&fragment(&text)).unwrap();
        assert_eq!(flow.objects.len(), 2);
        let ObjectKind::Image(image) = &flow.objects[0].kind else {
            panic!("Missing image")
        };
        assert_eq!(image.alternative.chars().count(), 512);
        assert_eq!(flow.objects[0].alternative, alt);
        assert!(flow
            .select(flow.objects[0].characters.clone(), 100)
            .is_err());
        assert_eq!(flow.objects[1].alternative, "outer inner $x$");
        assert_eq!(
            flow.select(flow.objects[1].characters.clone(), 100)
                .unwrap()
                .source
                .unwrap()
                .line,
            3
        );
    }
    #[test]
    fn empty_alt_anchors_do_not_capture_adjacent_links_or_styles() {
        let flow = project(&fragment(
            "[left](left.md)![](a.png)![](b.png)[**中文😀**](right.md)\n",
        ))
        .unwrap();
        assert_eq!(flow.links[0].characters, 0..4);
        assert_eq!(flow.objects[0].characters, 4..5);
        assert_eq!(flow.objects[1].characters, 5..6);
        assert_eq!(flow.links[1].characters, 6..9);
        let bold = flow
            .spans
            .iter()
            .find(|span| matches!(span.style, Style::Inline(inline::Style::Bold)))
            .unwrap();
        assert_eq!(bold.characters, 6..9);
        assert_eq!(
            flow.select(0..flow.characters, 100).unwrap().text,
            "left中文😀"
        );
        let empty_link = project(&fragment("[![](a.png)](next.md)\n")).unwrap();
        assert_eq!(empty_link.objects.len(), 1);
        assert!(empty_link.links.is_empty());
    }
    #[test]
    fn table_images_keep_full_alt_text_and_data_sources_without_transport_urls() {
        let text = "| ![Head](head.svg) | Image |\n|---|---|\n| 中文😀 | ![Pixel](data:image/png;base64,QUFB) |\n";
        let flow = project(&fragment(text)).unwrap();
        let object = &flow.objects[0];
        let ObjectKind::Table { header, rows, .. } = &object.kind else {
            panic!("Missing table")
        };
        assert!(matches!(
            &header[0].content.images[0].source,
            crate::ImageSource::Reference(_)
        ));
        assert!(matches!(
            &rows[0].cells[1].content.images[0].source,
            crate::ImageSource::Embedded(_)
        ));
        let selected = flow
            .select(object.characters.clone(), BUFFER_BYTES)
            .unwrap();
        assert_eq!(selected.text, "Head\tImage\n中文😀\tPixel");
        assert!(!format!("{:?}", rows[0].cells[1].content.images).contains("base64"));
    }
    #[test]
    fn snapshot_image_handles_preserve_original_source_coordinates() {
        let text = format!(
            "# Before\n\n![图😀](data:image/png;base64,{})\n\nAfter\n",
            "QUFB".repeat(4000)
        );
        let mut fragment = fragment(&text);
        fragment.source = crate::source_projection::project(fragment.source, 7).unwrap();
        assert!(fragment.source.text.len() < 100);
        let flow = project(&fragment).unwrap();
        let object = &flow.objects[0];
        let ObjectKind::Image(image) = &object.kind else {
            panic!("Missing snapshot image")
        };
        assert_eq!(
            image.source,
            crate::ImageSource::Document { page: 7, image: 0 }
        );
        let selection = flow.select(object.characters.clone(), 100).unwrap();
        let source = selection.source.unwrap();
        assert_eq!(selection.text, "图😀");
        assert_eq!(source.line, 3);
        assert!(source.end > 16000);
        assert!(!format!("{flow:?}").contains("QUFB"));
    }
    #[test]
    fn compact_objects_keep_full_cells_bounded_titles_and_shifted_unicode_links() {
        let long = format!("{} tail-cell", "中文😀 ".repeat(200));
        let input = format!("Before $x^2$ [中文😀](https://example.invalid) **last**.\n\n| Header |\n|---|\n| {long} |\n");
        let flow = project(&fragment(&input)).unwrap();
        assert!(flow.buffer.len() < 100);
        let table = flow.objects.last().unwrap();
        let ObjectKind::Table { header, rows, .. } = &table.kind else {
            panic!("Missing table")
        };
        assert!(header[0].content.attributes.is_none());
        assert!(rows[0].cells[0].content.attributes.is_none());
        assert_eq!(rows[0].cells[0].content.plain, long);
        assert!(rows[0].cells[0].title().len() <= crate::CELL_PREVIEW_BYTES + 3);
        let copied = flow.select(table.characters.clone(), BUFFER_BYTES).unwrap();
        assert!(copied.text.ends_with("tail-cell"));
        assert!(flow.select(table.characters.clone(), 100).is_err());
        let link = &flow.links[0];
        assert_eq!(
            flow.buffer
                .chars()
                .skip(link.characters.start)
                .take(link.characters.len())
                .collect::<String>(),
            "中文😀"
        );
        let bold = flow
            .spans
            .iter()
            .find(|s| matches!(s.style, Style::Inline(inline::Style::Bold)))
            .unwrap();
        assert_eq!(
            flow.buffer
                .chars()
                .skip(bold.characters.start)
                .take(bold.characters.len())
                .collect::<String>(),
            "last"
        );
    }
    #[test]
    fn rich_projection_quick_selection_matches_every_native_range() {
        let input =
            "Literal \u{fffc} 字😀 **before** $\\frac{\r\na}{b}$ [after](docs/a.md).\nOutside.";
        let content = crate::parse_text(input)
            .unwrap()
            .into_iter()
            .find_map(|b| match b.kind {
                crate::BlockKind::Paragraph(c) => Some(c),
                _ => None,
            })
            .unwrap();
        let original = content.plain.clone();
        assert!(content.attributes.as_ref().unwrap().multiline_math);
        let projected = project_rich(content).unwrap();
        let quick = projected.quick.as_ref().unwrap();
        assert_eq!(projected.objects.len(), 1);
        assert_eq!(
            quick.select(0..projected.characters, 8192),
            Some(original.as_str())
        );
        for start in 0..=projected.characters {
            for end in start..=projected.characters {
                let range = start..end;
                assert_eq!(
                    quick.select(range.clone(), 8192),
                    Some(projected.select(range.clone(), 8192).unwrap().text.as_str())
                );
                assert_eq!(
                    quick.projected_range(quick.source_range(range.clone()).unwrap()),
                    Some(range)
                );
            }
        }
        let link = &projected.links[0];
        assert_eq!(quick.select(link.characters.clone(), 8192), Some("after"));
        assert!(quick.select(0..projected.characters, 1).is_none());
        assert!(quick.select(2..1, 8192).is_none());
        assert!(quick.projected_range(usize::MAX..usize::MAX).is_none());
        assert!(quick.source_range(usize::MAX..usize::MAX).is_none());
    }

    #[test]
    fn rich_search_expands_partial_formula_and_rejects_large_input() {
        let content = crate::parse_text("字😀 before $\\frac{\na}{b}$ after.")
            .unwrap()
            .into_iter()
            .find_map(|b| match b.kind {
                crate::BlockKind::Paragraph(c) => Some(c),
                _ => None,
            })
            .unwrap();
        let original = content.plain.clone();
        let projected = project_rich(content.clone()).unwrap();
        let quick = projected.quick.as_ref().unwrap();
        let atom = &projected.objects[0];
        let source = quick.source_range(atom.characters.clone()).unwrap();
        for character in source.start..source.end {
            assert_eq!(
                quick.projected_range(character..character + 1),
                Some(atom.characters.clone())
            );
        }
        assert_eq!(
            quick.select(atom.characters.clone(), 8192),
            Some(atom.alternative.as_str())
        );
        assert_eq!(
            quick.select(0..projected.characters, 8192),
            Some(original.as_str())
        );
        let mut oversized = content;
        oversized.plain = "x".repeat(RICH_TEXT_BYTES + 1);
        assert!(project_rich(oversized)
            .unwrap_err()
            .contains("projection budget"));
    }
}
