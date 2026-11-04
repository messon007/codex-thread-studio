//! Quote verification against immutable Markdown snapshots. No GUI or file paths.
//! Literal scans retain one bounded overlap; rendered scans reuse reader blocks.
use crate::{Block, BlockKind, SourceRange};
use serde::{Deserialize, Serialize};
use std::io::{self, Read};

pub const MAX_QUOTE: usize = 16 * 1024;
const CHUNK: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Kind {
    #[default]
    Auto,
    Source,
    Preview,
    /// Native continuous text, including atomic table/diagram alternatives.
    Flow,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct Anchor {
    pub base_hash: String,
    pub start_line: Option<usize>,
    pub end_line: Option<usize>,
    pub text_kind: Kind,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Query {
    pub quote: String,
    pub anchor: Anchor,
}
impl Query {
    pub fn validate(&self) -> Result<(), String> {
        if self.quote.trim().is_empty() || self.quote.len() > MAX_QUOTE || self.quote.contains('\0')
        {
            return Err("Comment quote requires 1–16384 UTF-8 bytes without NUL".into());
        }
        if self.anchor.base_hash.len() > 128
            || self.anchor.base_hash.contains('\0')
            || self.anchor.start_line.is_some_and(|n| n == 0)
            || self.anchor.end_line.is_some_and(|n| n == 0)
            || self
                .anchor
                .start_line
                .zip(self.anchor.end_line)
                .is_some_and(|(a, b)| a > b)
        {
            return Err("Invalid comment quote anchor".into());
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Status {
    Unique,
    Missing,
    Ambiguous,
    Limited,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Position {
    pub start: usize,
    pub end: usize,
    pub line: usize,
    pub end_line: usize,
}
impl From<&SourceRange> for Position {
    fn from(r: &SourceRange) -> Self {
        Self {
            start: r.start,
            end: r.end,
            line: r.line,
            end_line: r.end_line,
        }
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Located {
    pub status: Status,
    pub position: Option<Position>,
    pub text_kind: Option<Kind>,
}
#[derive(Default)]
struct Candidates {
    first: Option<(Position, usize)>,
    ambiguous: bool,
}
impl Candidates {
    fn add(&mut self, position: Position, ordinal: usize) {
        match &self.first {
            None => self.first = Some((position, ordinal)),
            Some(first) if first != &(position, ordinal) => self.ambiguous = true,
            _ => {}
        }
    }
}
pub struct Locator {
    query: Query,
    unchanged: bool,
    literal: Candidates,
    rendered: Candidates,
    flow_scan: Option<crate::flow::QuoteScanner>,
    limited: bool,
}
impl Locator {
    pub fn new(query: Query, revision: &str) -> Result<Self, String> {
        query.validate()?;
        let unchanged = !query.anchor.base_hash.is_empty() && query.anchor.base_hash == revision;
        let flow_scan = (query.anchor.text_kind == Kind::Flow)
            .then(|| crate::flow::QuoteScanner::new(&query.quote))
            .transpose()?;
        Ok(Self {
            query,
            unchanged,
            literal: Candidates::default(),
            rendered: Candidates::default(),
            flow_scan,
            limited: false,
        })
    }
    fn in_scope(&self, p: &Position) -> bool {
        if !self.unchanged {
            return true;
        }
        self.query.anchor.start_line.is_none_or(|start| {
            p.line >= start && p.end_line <= self.query.anchor.end_line.unwrap_or(start)
        })
    }
    pub fn needs_preview(&self) -> bool {
        self.query.anchor.text_kind != Kind::Source && !self.literal.ambiguous
    }
    pub fn preview_ambiguous(&self) -> bool {
        self.rendered.ambiguous
    }
    pub fn limited(&mut self) {
        self.limited = true;
        self.flow_boundary();
    }
    pub fn flow_boundary(&mut self) {
        if let Some(scanner) = &mut self.flow_scan {
            scanner.boundary();
        }
    }
    /// Full indexed-table text is one atomic selection, regardless of paging.
    pub fn flow_table(&mut self, table: crate::flow::TableText) {
        use crate::flow::{Object, ObjectKind, Projection, Segment, ANCHOR};
        let projection = Projection {
            buffer: ANCHOR.to_string(),
            characters: 1,
            objects: vec![Object {
                characters: 0..1,
                alternative: table.text,
                kind: ObjectKind::Table {
                    id: Some(table.id),
                    row_start: 0,
                    header: vec![],
                    rows: vec![],
                },
            }],
            segments: vec![Segment {
                characters: 0..1,
                source: table.source,
            }],
            ..Projection::default()
        };
        self.flow(&projection);
    }
    /// Count overlapping UTF-8 matches, including matches across read boundaries.
    /// `maximum` is the snapshot size bound, never a client-controlled file path.
    pub fn scan_literal(&mut self, mut input: impl Read, maximum: usize) -> io::Result<()> {
        if matches!(self.query.anchor.text_kind, Kind::Preview | Kind::Flow) {
            return Ok(());
        }
        let mut bytes = Vec::with_capacity(CHUNK + MAX_QUOTE + 4);
        let mut chunk = [0; CHUNK];
        let (mut base, mut base_line, mut read_bytes, mut last) = (0usize, 1usize, 0usize, None);
        loop {
            let n = input.read(&mut chunk)?;
            read_bytes = read_bytes
                .checked_add(n)
                .ok_or_else(|| io::Error::other("Quote scan size overflow"))?;
            if read_bytes > maximum {
                return Err(io::Error::other("Quote snapshot grew beyond its limit"));
            }
            bytes.extend_from_slice(&chunk[..n]);
            let valid = match std::str::from_utf8(&bytes) {
                Ok(s) => s.len(),
                Err(e) if e.error_len().is_none() && n != 0 => e.valid_up_to(),
                Err(e) => return Err(io::Error::new(io::ErrorKind::InvalidData, e)),
            };
            let source = std::str::from_utf8(&bytes[..valid]).unwrap();
            let (mut cursor, mut line) = (0, base_line);
            while let Some(relative) = source[cursor..].find(&self.query.quote) {
                let start = cursor + relative;
                line += source[cursor..start]
                    .bytes()
                    .filter(|b| *b == b'\n')
                    .count();
                if last.is_none_or(|previous| base + start > previous) {
                    last = Some(base + start);
                    let p = Position {
                        start: base + start,
                        end: base + start + self.query.quote.len(),
                        line,
                        end_line: line
                            + self.query.quote.as_bytes()[..self.query.quote.len() - 1]
                                .iter()
                                .filter(|b| **b == b'\n')
                                .count(),
                    };
                    if self.in_scope(&p) {
                        self.literal.add(p, 0);
                    }
                    if self.literal.ambiguous {
                        return Ok(());
                    }
                }
                cursor = start + source[start..].chars().next().unwrap().len_utf8();
                line += source[start..cursor]
                    .bytes()
                    .filter(|b| *b == b'\n')
                    .count();
            }
            if n == 0 {
                return Ok(());
            }
            let mut discard = valid.saturating_sub(self.query.quote.len() + 4);
            while !source.is_char_boundary(discard) {
                discard -= 1;
            }
            base_line += source[..discard].bytes().filter(|b| *b == b'\n').count();
            base += discard;
            bytes.drain(..discard);
        }
    }
    fn text(&mut self, text: &str, range: &SourceRange) {
        let position = Position::from(range);
        if !self.in_scope(&position) {
            return;
        }
        let mut cursor = 0;
        while let Some(relative) = text[cursor..].find(&self.query.quote) {
            let start = cursor + relative;
            self.rendered.add(position.clone(), start);
            if self.rendered.ambiguous {
                return;
            }
            cursor = start + text[start..].chars().next().unwrap().len_utf8();
        }
    }
    /// Header copies across table pages have the same source/occurrence identity.
    pub fn preview(&mut self, blocks: &[Block]) {
        if self.needs_flow() {
            return;
        }
        for block in blocks {
            if self.rendered.ambiguous {
                break;
            }
            match &block.kind {
                BlockKind::Paragraph(t) | BlockKind::Heading { content: t, .. } => {
                    self.text(&t.plain, &block.source)
                }
                BlockKind::Code { text, .. } => self.text(text, &block.source),
                BlockKind::Table { header, rows, .. } => {
                    for cell in header
                        .iter()
                        .chain(rows.iter().flat_map(|row| row.cells.iter()))
                    {
                        self.text(&cell.content.plain, &cell.source);
                        if self.rendered.ambiguous {
                            break;
                        }
                    }
                }
                BlockKind::List { children, .. }
                | BlockKind::Item(children)
                | BlockKind::Quote(children)
                | BlockKind::Group(children) => self.preview(children),
                _ => {}
            }
        }
    }
    pub fn needs_flow(&self) -> bool {
        self.query.anchor.text_kind == Kind::Flow
    }
    pub fn flow(&mut self, projection: &crate::flow::Projection) {
        if !self.needs_flow() {
            return;
        }
        let (unchanged, start, end) = (
            self.unchanged,
            self.query.anchor.start_line,
            self.query.anchor.end_line,
        );
        let rendered = &mut self.rendered;
        self.flow_scan
            .as_mut()
            .unwrap()
            .feed(projection, |source, ordinal| {
                let position = Position::from(&source);
                if !unchanged
                    || start.is_none_or(|start| {
                        position.line >= start && position.end_line <= end.unwrap_or(start)
                    })
                {
                    rendered.add(position, ordinal);
                }
                !rendered.ambiguous
            });
    }
    pub fn finish(self) -> Located {
        let fail = |status| Located {
            status,
            position: None,
            text_kind: None,
        };
        if self.literal.ambiguous || self.rendered.ambiguous {
            return fail(Status::Ambiguous);
        }
        if self.limited {
            return fail(Status::Limited);
        }
        let rendered_kind = if self.needs_flow() {
            Kind::Flow
        } else {
            Kind::Preview
        };
        let literal = self.literal.first.map(|p| p.0);
        let rendered = self.rendered.first.map(|p| p.0);
        if let (Some(a), Some(b)) = (&literal, &rendered) {
            if a.start < b.start || a.end > b.end {
                return fail(Status::Ambiguous);
            }
        }
        match literal
            .map(|p| (p, Kind::Source))
            .or_else(|| rendered.map(|p| (p, rendered_kind)))
        {
            Some((position, kind)) => Located {
                status: Status::Unique,
                position: Some(position),
                text_kind: Some(kind),
            },
            None => fail(Status::Missing),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn query(text: &str, kind: Kind) -> Query {
        Query {
            quote: text.into(),
            anchor: Anchor {
                text_kind: kind,
                ..Anchor::default()
            },
        }
    }
    #[test]
    fn streaming_matches_cross_chunks_and_keep_unicode_crlf_lines_and_spaces() {
        let source = format!("{}\r\n中😀 trailing \r\nnext\r\n", "x".repeat(CHUNK - 5));
        let mut scan = Locator::new(query("中😀 trailing \r\nnext", Kind::Source), "new").unwrap();
        scan.scan_literal(source.as_bytes(), source.len()).unwrap();
        let found = scan.finish();
        assert_eq!(found.status, Status::Unique);
        assert_eq!(
            found.position,
            Some(Position {
                start: CHUNK - 3,
                end: source.len() - 2,
                line: 2,
                end_line: 3
            })
        );
        let mut scan = Locator::new(query("trailing  ", Kind::Source), "new").unwrap();
        scan.scan_literal(source.as_bytes(), source.len()).unwrap();
        assert_eq!(scan.finish().status, Status::Missing);
    }
    #[test]
    fn multiline_formula_preview_quotes_keep_exact_crlf_and_source_lines() {
        let formula = "$\\frac{\r\na}{b}$";
        let source = format!("# Math\r\n\r\nBefore **字😀** {formula} after.\r\n");
        let blocks = crate::parse_text(&source).unwrap();
        for (quote, expected) in [
            (formula.to_owned(), Status::Unique),
            (formula.replace("\r\n", "\n"), Status::Missing),
        ] {
            let mut request = query(&quote, Kind::Preview);
            request.anchor.base_hash = "same".into();
            request.anchor.start_line = Some(3);
            request.anchor.end_line = Some(4);
            let mut locator = Locator::new(request, "same").unwrap();
            locator.preview(&blocks);
            let located = locator.finish();
            assert_eq!(located.status, expected);
            if expected == Status::Unique {
                let position = located.position.unwrap();
                assert_eq!((position.line, position.end_line), (3, 4));
                assert!(source[position.start..position.end].contains(formula));
            }
        }
    }
    #[test]
    fn unicode_read_boundaries_and_invalid_snapshot_bytes_are_checked() {
        for split in 1..=3 {
            let prefix = CHUNK - split;
            let text = format!("{}😀 tail", "x".repeat(prefix));
            let mut scan = Locator::new(query("😀 tail", Kind::Source), "new").unwrap();
            scan.scan_literal(text.as_bytes(), text.len()).unwrap();
            assert_eq!(scan.finish().position.unwrap().start, prefix);
        }
        let mut scan = Locator::new(query("needle", Kind::Source), "new").unwrap();
        assert!(scan.scan_literal(b"needle\xff".as_slice(), 7).is_err());
        let mut scan = Locator::new(query("needle", Kind::Source), "new").unwrap();
        assert!(scan.scan_literal(b"needle".as_slice(), 5).is_err());
    }
    #[test]
    fn overlapping_duplicates_and_changed_revision_never_guess_the_nearest() {
        let mut scan = Locator::new(query("aaa", Kind::Source), "new").unwrap();
        scan.scan_literal("aaaa".as_bytes(), 4).unwrap();
        assert_eq!(scan.finish().status, Status::Ambiguous);
        let mut q = query("same", Kind::Source);
        q.anchor.base_hash = "old".into();
        q.anchor.start_line = Some(3);
        q.anchor.end_line = Some(3);
        let mut scan = Locator::new(q.clone(), "old").unwrap();
        scan.scan_literal("same\n\nsame".as_bytes(), 10).unwrap();
        assert_eq!(scan.finish().position.unwrap().line, 3);
        let mut scan = Locator::new(q, "changed").unwrap();
        scan.scan_literal("same\n\nsame".as_bytes(), 10).unwrap();
        assert_eq!(scan.finish().status, Status::Ambiguous);
    }
    #[test]
    fn preview_uses_the_readers_exact_plain_text_and_rejects_duplicates() {
        let blocks =
            crate::parse_text("# Heading\n\n**Bold 中文** and &amp; `code` trailing  spaces\n")
                .unwrap();
        let mut scan = Locator::new(
            query("Bold 中文 and & code trailing  spaces", Kind::Preview),
            "new",
        )
        .unwrap();
        scan.preview(&blocks);
        assert_eq!(scan.finish().position.unwrap().line, 3);
        let blocks = crate::parse_text("**same**\n\nsame\n").unwrap();
        let mut scan = Locator::new(query("same", Kind::Preview), "new").unwrap();
        scan.preview(&blocks);
        assert_eq!(scan.finish().status, Status::Ambiguous);
    }
    #[test]
    fn repeated_page_headers_deduplicate_but_repeated_occurrences_do_not() {
        let blocks = crate::parse_text("| **header** |\n|---|\n| body |\n").unwrap();
        let mut scan = Locator::new(query("header", Kind::Preview), "new").unwrap();
        scan.preview(&blocks);
        scan.preview(&blocks);
        assert_eq!(scan.finish().status, Status::Unique);
        let blocks = crate::parse_text("same same\n").unwrap();
        let mut scan = Locator::new(query("same", Kind::Preview), "new").unwrap();
        scan.preview(&blocks);
        assert_eq!(scan.finish().status, Status::Ambiguous);
    }
    #[test]
    fn legacy_auto_and_limited_results_remain_conservative() {
        let text = "**word** elsewhere\n\n`**word**`\n";
        let mut scan = Locator::new(query("**word**", Kind::Auto), "new").unwrap();
        scan.scan_literal(text.as_bytes(), text.len()).unwrap();
        scan.preview(&crate::parse_text(text).unwrap());
        assert_eq!(scan.finish().status, Status::Ambiguous);
        let mut scan = Locator::new(query("unique", Kind::Source), "new").unwrap();
        scan.scan_literal(b"unique".as_slice(), 6).unwrap();
        scan.limited();
        assert_eq!(scan.finish().status, Status::Limited);
        assert!(Locator::new(query(" ", Kind::Auto), "new").is_err());
        assert!(Locator::new(query(&"x".repeat(MAX_QUOTE + 1), Kind::Source), "new").is_err());
    }
}
