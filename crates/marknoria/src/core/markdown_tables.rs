//! Bounded Markdown table projections. Complete cell values are a separate,
//! streamed input store: neither SQL nor source access consumes a preview.
use super::*;
use std::io::BufReader;

const RICH_EVENTS: usize = MAX_EVENTS / 4;
const HEADER_HTML: usize = MAX_HTML / 4;
const ROW_HTML: usize = MAX_HTML / 2;

#[derive(Debug)]
pub(super) struct NestedBudget;
impl std::fmt::Display for NestedBudget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("A nested Markdown table exceeds the bounded rich-preview budget")
    }
}
impl std::error::Error for NestedBudget {}
pub(super) fn nested_budget() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, NestedBudget)
}

#[cfg(test)]
#[path = "../../tests/support/markdown_tables.rs"]
mod tests;

fn safe(event: Event<'_>, web: bool) -> Event<'_> {
    if web
        && (matches!(
            event,
            Event::Html(_)
                | Event::InlineHtml(_)
                | Event::Start(Tag::Image { .. })
                | Event::End(TagEnd::Image)
        ) || matches!(&event, Event::Start(Tag::Link { dest_url, .. }) if dest_url.starts_with(references::PREFIX)))
    {
        references::projected(event)
    } else {
        inert_cell_event(event)
    }
}

struct Row<'a> {
    rich: Option<Vec<Event<'a>>>,
    cells: Vec<String>,
    span: Range<usize>,
    weight: usize,
    header: bool,
}

impl<'a> Row<'a> {
    fn read(
        first: (Event<'a>, Range<usize>),
        events: &mut impl Iterator<Item = io::Result<(Event<'a>, Range<usize>)>>,
        web: bool,
    ) -> io::Result<Self> {
        let header = matches!(first.0, Event::Start(Tag::TableHead));
        let mut row = Self {
            rich: Some(Vec::new()),
            cells: Vec::new(),
            span: first.1.clone(),
            weight: 0,
            header,
        };
        let mut cell: Option<markdown_html::CellText> = None;
        let mut current = Some(first);
        loop {
            let (event, span) = current
                .take()
                .map(Ok)
                .or_else(|| events.next())
                .ok_or_else(|| invalid("Unterminated Markdown table row"))??;
            row.span.end = row.span.end.max(span.end);
            let finished = matches!(event, Event::End(TagEnd::TableHead | TagEnd::TableRow));
            let event = safe(event, web);
            match &event {
                Event::Start(Tag::TableCell) => cell = Some(Default::default()),
                Event::End(TagEnd::TableCell) => row.cells.push(
                    cell.take()
                        .ok_or_else(|| invalid("Missing Markdown table cell"))?
                        .finish(),
                ),
                _ => {
                    if let Some(cell) = &mut cell {
                        match &event {
                            Event::Text(value) => cell.push_text(value),
                            Event::Code(value) => cell.push_code(value),
                            Event::SoftBreak | Event::HardBreak => cell.push_break(),
                            Event::Html(value) | Event::InlineHtml(value) => cell.push_html(value),
                            Event::Start(_) | Event::End(_) => cell.boundary(),
                            _ => {}
                        }
                    }
                }
            }
            row.weight = row.weight.saturating_add(references::event_weight(&event));
            if let Some(rich) = &mut row.rich {
                if rich.len() >= RICH_EVENTS || row.weight > ROW_HTML / 2 {
                    row.rich = None;
                } else {
                    rich.push(event);
                }
            }
            if finished {
                return Ok(row);
            }
        }
    }

    fn render(&self, alignments: &[Alignment], starts: &[usize]) -> io::Result<(String, usize)> {
        let limit = if self.header { HEADER_HTML } else { ROW_HTML };
        if let Some(rich) = &self.rich {
            let mut events = vec![Event::Start(Tag::Table(alignments.to_vec()))];
            if !self.header {
                events.extend([Event::Start(Tag::TableHead), Event::End(TagEnd::TableHead)]);
            }
            events.extend(rich.iter().cloned());
            events.push(Event::End(TagEnd::Table));
            if let Ok(events) = markdown::autolinks(events) {
                let event_count = events.len();
                let mut html = String::new();
                push_markdown_html(&mut html, events);
                let (open, close) = if self.header {
                    ("<thead>", "</thead>")
                } else {
                    ("<tbody>", "</tbody>")
                };
                let body = html
                    .split_once(open)
                    .and_then(|(_, rest)| rest.split_once(close))
                    .ok_or_else(|| invalid("Invalid rendered Markdown table row"))?
                    .0;
                let body = annotate_rows(body, [self.span.clone()], starts)?;
                if body.len() <= limit && event_count <= RICH_EVENTS {
                    return Ok((body, event_count));
                }
            }
        }
        // Excessive inline markup is flattened before considering omission.
        // Allocate a per-cell escaped-byte budget, leaving space for tags and
        // immutable source coordinates even at the maximum 256 columns.
        let available = limit.saturating_sub(512 + self.cells.len() * 64);
        let base_events = self.cells.len() * 3 + 2;
        let mut full_bytes = 0usize;
        let mut full_breaks = 0usize;
        'measure: for value in &self.cells {
            for c in value.chars() {
                full_bytes = full_bytes.saturating_add(escaped_size(c));
                full_breaks += usize::from(c == '\n');
                if full_bytes > available || base_events + 2 * full_breaks > RICH_EVENTS {
                    break 'measure;
                }
            }
        }
        let complete = full_bytes <= available && base_events + 2 * full_breaks <= RICH_EVENTS;
        let budget = if complete {
            available
        } else {
            available / self.cells.len().max(1)
        };
        let break_limit = RICH_EVENTS.saturating_sub(base_events)
            / (2 * if complete { 1 } else { self.cells.len().max(1) });
        let mut breaks = 0;
        let mut abridged = false;
        let mut body = String::from(OWNED_ROW);
        let tag = if self.header { "th" } else { "td" };
        for (column, value) in self.cells.iter().enumerate() {
            let (text, clipped, count) = escaped_prefix(value, budget, break_limit);
            breaks += count;
            body.push_str(&format!("<{tag}"));
            if clipped {
                abridged = true;
                body.push_str(" data-cell-preview=\"true\"");
            }
            match alignments.get(column) {
                Some(Alignment::Left) => body.push_str(" style=\"text-align: left\""),
                Some(Alignment::Center) => body.push_str(" style=\"text-align: center\""),
                Some(Alignment::Right) => body.push_str(" style=\"text-align: right\""),
                _ => {}
            }
            body.push('>');
            body.push_str(&text);
            body.push_str(&format!("</{tag}>"));
        }
        body.push_str("</tr>\n");
        let mut body = annotate_rows(&body, [self.span.clone()], starts)?;
        if abridged {
            // Survives column projection: a hidden clipped column still needs
            // original-source search activation rather than an invented match.
            body = body.replacen("<tr ", "<tr data-row-preview=\"true\" ", 1);
        }
        if body.len() > limit {
            return Err(invalid("Table preview exceeds its bounded row allocation"));
        }
        Ok((body, base_events + 2 * breaks))
    }
}

fn escaped_prefix(value: &str, budget: usize, break_limit: usize) -> (String, bool, usize) {
    let mut out = String::new();
    let mut consumed = 0;
    let mut breaks = 0;
    for c in value.chars() {
        if c == '\n' {
            if breaks >= break_limit {
                break;
            }
            breaks += 1;
        }
        let escaped = match c {
            '&' => "&amp;".into(),
            '<' => "&lt;".into(),
            '>' => "&gt;".into(),
            '"' => "&quot;".into(),
            '\n' => "<br>\n".into(),
            '\r' => "&#13;".into(),
            c if c < ' ' && !matches!(c, '\r' | '\t') => {
                char::from_u32(0x2400 + c as u32).unwrap().to_string()
            }
            c => c.to_string(),
        };
        if out.len() + escaped.len() > budget {
            break;
        }
        out.push_str(&escaped);
        consumed += c.len_utf8();
    }
    (out, consumed < value.len(), breaks)
}

fn escaped_size(c: char) -> usize {
    match c {
        '&' => 5,
        '<' | '>' => 4,
        '"' => 6,
        '\n' => 5,
        '\r' => 5,
        c if c < ' ' && c != '\t' => 3,
        c => c.len_utf8(),
    }
}

struct DataWriter<'a> {
    file: &'a mut BufWriter<File>,
    bytes: &'a mut u64,
    html_bytes: u64,
}
impl Write for DataWriter<'_> {
    fn write(&mut self, value: &[u8]) -> io::Result<usize> {
        if self.html_bytes + *self.bytes + value.len() as u64 > MAX_CACHE {
            return Err(invalid(
                "Complete table input exceeds the shared cache budget",
            ));
        }
        self.file.write_all(value)?;
        *self.bytes += value.len() as u64;
        Ok(value.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}

fn record(file: &mut BufWriter<File>, writer: &mut Writer<'_>, row: &Row<'_>) -> io::Result<()> {
    #[derive(Serialize)]
    struct Record<'a> {
        line: usize,
        cells: &'a [String],
    }
    let value = Record {
        line: source_range(&writer.starts, row.span.clone()).line,
        cells: &row.cells,
    };
    let mut bounded = DataWriter {
        file,
        bytes: &mut writer.table_data_bytes,
        html_bytes: writer.offset,
    };
    serde_json::to_writer(&mut bounded, &value)?;
    bounded.write_all(b"\n")
}

pub(super) fn build<'a>(
    writer: &mut Writer<'_>,
    events: &mut impl Iterator<Item = io::Result<(Event<'a>, Range<usize>)>>,
    alignments: &[Alignment],
    table_span: Range<usize>,
    directory: &Path,
    table_id: usize,
) -> io::Result<()> {
    if alignments.is_empty() || alignments.len() > 256 {
        return Err(invalid("Tables support between 1 and 256 columns"));
    }
    let first = events
        .next()
        .ok_or_else(|| invalid("Missing Markdown table header"))??;
    if !matches!(first.0, Event::Start(Tag::TableHead)) {
        return Err(invalid("Missing Markdown table header"));
    }
    let mut header = Row::read(first, events, writer.web_references)?;
    header.span = table_span.start
        ..writer.source[table_span.start..]
            .find('\n')
            .map_or(table_span.end, |end| table_span.start + end);
    let (header_html, header_events) = header.render(alignments, &writer.starts)?;
    let prefix = format!("<table><thead>{header_html}</thead><tbody>");
    let suffix = "</tbody></table>";
    let data_file = format!("markdown-table-{table_id}.jsonl");
    let mut full = BufWriter::new(File::create(directory.join(&data_file))?);
    let mut labels = header
        .cells
        .iter()
        .map(|value| value.replace('\n', " ").chars().take(160).collect())
        .collect::<Vec<String>>();
    labels.resize(alignments.len(), String::new());
    writer.tables.push(Table {
        label: None,
        data_file: Some(data_file),
        first_page: writer.pages.len(),
        end_page: writer.pages.len(),
        rows: 0,
        columns: labels,
        max_row_bytes: 0,
        header_bytes: 0,
    });
    let mut body = String::new();
    let mut rows = 0usize;
    let mut event_count = header_events + 2;
    let mut span = header.span.clone();
    let row_limit = (512 / alignments.len()).clamp(1, 16);
    let mut emitted = false;
    for_next(events, |first, events| {
        if matches!(first.0, Event::End(TagEnd::Table)) {
            return Ok(false);
        }
        if !matches!(first.0, Event::Start(Tag::TableRow)) {
            return Err(invalid("Unexpected Markdown table event"));
        }
        let row = Row::read(first, events, writer.web_references)?;
        if row.cells.len() != alignments.len() {
            return Err(invalid("Markdown table column count changed"));
        }
        let (html, row_events) = row.render(alignments, &writer.starts)?;
        if rows > 0
            && (rows >= row_limit
                || event_count + row_events > MAX_EVENTS
                || prefix.len() + body.len() + html.len() + suffix.len() > MAX_HTML
                || body.len() + html.len() >= PAGE_BYTES)
        {
            writer.commit_page(
                format!("{prefix}{body}{suffix}"),
                span.clone(),
                rows,
                alignments.len(),
                Some(table_id),
            )?;
            body.clear();
            rows = 0;
            event_count = header_events + 2;
            emitted = true;
        }
        if rows == 0 {
            span.start = if emitted {
                row.span.start
            } else {
                table_span.start
            };
        }
        record(&mut full, writer, &row)?;
        body.push_str(&html);
        rows += 1;
        event_count += row_events;
        span.end = row.span.end;
        Ok(true)
    })?;
    if rows > 0 || !emitted {
        writer.commit_page(
            format!("{prefix}{body}{suffix}"),
            span,
            rows,
            alignments.len(),
            Some(table_id),
        )?;
    }
    full.flush()
}

// A small streaming driver permits row consumption inside the callback without
// buffering the rest of a table or fighting a by_ref() iterator borrow.
fn for_next<'a, I: Iterator<Item = io::Result<(Event<'a>, Range<usize>)>>>(
    events: &mut I,
    mut row: impl FnMut((Event<'a>, Range<usize>), &mut I) -> io::Result<bool>,
) -> io::Result<()> {
    while let Some(first) = events.next() {
        if !row(first?, events)? {
            return Ok(());
        }
    }
    Err(invalid("Unterminated Markdown table"))
}

pub(super) fn audit(directory: &Path, index: &Index) -> io::Result<TableAudit> {
    let source = String::from_utf8(audit_snapshot(directory, index)?)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let starts = std::iter::once(0)
        .chain(source.match_indices('\n').map(|(at, _)| at + 1))
        .collect::<Vec<_>>();
    let options =
        Options::ENABLE_TABLES | Options::ENABLE_TASKLISTS | Options::ENABLE_STRIKETHROUGH;
    let bom = usize::from(source.starts_with('\u{feff}')) * 3;
    let (parse_source, _) = bare_plantuml(&source[bom..], options);
    let web = index.web_markdown || !index.references.is_empty() || !index.anchors.is_empty();
    let mut references = web.then(|| references::Auditor::new(directory, index));
    let mut events = Parser::new_ext(&parse_source, options)
        .into_offset_iter()
        .map(|(event, span)| {
            let span = span.start + bom..span.end + bom;
            let event = match &mut references {
                Some(audit) => audit.event(event, &span)?,
                None => event,
            };
            Ok((event, span))
        });
    let mut result = TableAudit {
        tables: 0,
        rows: 0,
        cells: 0,
        fragments: 0,
    };
    let mut depth = 0usize;
    while let Some(first) = events.next() {
        let (event, span) = first?;
        if depth == 0
            && let Event::Start(Tag::Table(alignments)) = &event
        {
            let id = result.tables;
            let metadata = index
                .tables
                .get(id)
                .ok_or_else(|| invalid("Missing Markdown table inventory"))?;
            let data_file = format!("markdown-table-{id}.jsonl");
            if metadata.data_file.as_deref() != Some(&data_file) {
                return Err(invalid("Invalid complete Markdown table input"));
            }
            let mut records = serde_json::Deserializer::from_reader(BufReader::new(File::open(
                directory.join(data_file),
            )?))
            .into_iter::<documents::TableRecord>();
            let first = events
                .next()
                .ok_or_else(|| invalid("Missing audit table header"))??;
            let mut header = Row::read(first, &mut events, web)?;
            header.span = span.start
                ..source[span.start..]
                    .find('\n')
                    .map_or(span.end, |end| span.start + end);
            let (header_html, _) = header.render(alignments, &starts)?;
            let mut cached = VecDeque::new();
            let mut page = metadata.first_page;
            let mut number = 0;
            let mut cached_count = 0;
            for_next(&mut events, |first, events| {
                if matches!(first.0, Event::End(TagEnd::Table)) {
                    return Ok(false);
                }
                let row = Row::read(first, events, web)?;
                let record = records
                    .next()
                    .ok_or_else(|| invalid("Missing complete table record"))??;
                if record.line != source_range(&starts, row.span.clone()).line
                    || record.cells != row.cells
                {
                    return Err(invalid(
                        "Complete table input differs from original Markdown",
                    ));
                }
                while cached.is_empty() && page < metadata.end_page {
                    let info = &index.pages[page];
                    if info.table != Some(id)
                        || info.row_start != cached_count
                        || info.columns != alignments.len()
                    {
                        return Err(invalid("Table fragment inventory mismatch"));
                    }
                    let html = read_page(directory, index, page)?;
                    let actual_header = html
                        .split_once("<thead>")
                        .and_then(|(_, rest)| rest.split_once("</thead>"))
                        .ok_or_else(|| invalid("Missing repeated header"))?
                        .0;
                    if actual_header != header_html {
                        return Err(invalid(
                            "Repeated table header differs from source projection",
                        ));
                    }
                    cached.extend(
                        table_body(&html)?
                            .split_inclusive("</tr>")
                            .filter(|row| row.contains("<tr"))
                            .map(str::to_owned),
                    );
                    if cached.len() != info.rows {
                        return Err(invalid("Table fragment row count mismatch"));
                    }
                    cached_count += info.rows;
                    page += 1;
                    result.fragments += 1;
                }
                let actual = cached
                    .pop_front()
                    .ok_or_else(|| invalid("Missing projected table row"))?;
                if actual.trim() != row.render(alignments, &starts)?.0.trim() {
                    return Err(invalid("Projected table row differs from original source"));
                }
                number += 1;
                result.rows += 1;
                result.cells += row.cells.len();
                Ok(true)
            })?;
            if number == 0 {
                let html = read_page(directory, index, page)?;
                let info = &index.pages[page];
                let actual_header = html
                    .split_once("<thead>")
                    .and_then(|(_, rest)| rest.split_once("</thead>"))
                    .ok_or_else(|| invalid("Missing header-only table header"))?
                    .0;
                if actual_header != header_html
                    || info.table != Some(id)
                    || info.row_start != 0
                    || info.columns != alignments.len()
                    || !table_body(&html)?.trim().is_empty()
                    || info.rows != 0
                {
                    return Err(invalid("Header-only table contains unexpected rows"));
                }
                page += 1;
                result.fragments += 1;
            }
            if number != metadata.rows
                || !cached.is_empty()
                || page != metadata.end_page
                || records.next().is_some()
            {
                return Err(invalid(
                    "Markdown table input/projection row count mismatch",
                ));
            }
            // Audit the separately requested selected-column/page path too.
            let columns = (0..alignments.len()).collect::<Vec<_>>();
            let mut next = 0;
            loop {
                let window = read_table(directory, index, id, next, &columns)?;
                if window.row_end < metadata.rows && window.row_end <= next {
                    return Err(invalid("Table window made no progress"));
                }
                if window.row_end >= metadata.rows {
                    break;
                }
                next = window.row_end;
            }
            result.tables += 1;
        } else {
            match event {
                Event::Start(_) => depth += 1,
                Event::End(_) => depth = depth.saturating_sub(1),
                _ => {}
            }
        }
    }
    drop(events);
    if let Some(references) = references {
        references.finish()?;
    }
    if result.tables != index.tables.len() {
        return Err(invalid("Unexpected table inventory"));
    }
    Ok(result)
}

pub(super) fn audit_source(directory: &Path, index: &Index) -> io::Result<TableAudit> {
    if !index.tables.is_empty() || documents::preview_note(directory)?.is_none() {
        return Err(invalid(
            "Source-only Markdown must declare its fallback and omit table capabilities",
        ));
    }
    let source = audit_snapshot(directory, index)?;
    let mut next = 0;
    for (id, page) in index.pages.iter().enumerate() {
        let text = read_cached_page(directory, index, id)?;
        if page.table.is_some()
            || page.start != next
            || page.end > source.len()
            || source.get(page.start..page.end) != Some(text.as_bytes())
        {
            return Err(invalid(
                "Source-only Markdown projection lost original bytes",
            ));
        }
        next = page.end;
    }
    if next != source.len() {
        return Err(invalid("Source-only Markdown projection is incomplete"));
    }
    Ok(TableAudit {
        tables: 0,
        rows: 0,
        cells: 0,
        fragments: 0,
    })
}

fn audit_snapshot(directory: &Path, index: &Index) -> io::Result<Vec<u8>> {
    let file = File::open(directory.join("source.md"))?;
    if file.metadata()?.len() > MAX_SOURCE {
        return Err(invalid("Markdown exceeds the source budget"));
    }
    let mut source = Vec::new();
    file.take(MAX_SOURCE + 1).read_to_end(&mut source)?;
    if source.len() != index.source_bytes
        || source.len() as u64 > MAX_SOURCE
        || Sha256::digest(&source)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
            != index.revision
    {
        return Err(invalid("Markdown snapshot changed"));
    }
    Ok(source)
}
