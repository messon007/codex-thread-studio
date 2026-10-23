use pulldown_cmark::{Alignment, CodeBlockKind, Event, Options, Parser, Tag, TagEnd, html};
pub mod compare;
pub mod edit;
pub mod highlight;
pub mod server;
pub mod source;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    borrow::Cow,
    collections::VecDeque,
    fs::{self, File},
    io::{self, BufWriter, Read, Seek, SeekFrom, Write},
    ops::Range,
    path::Path,
    time::Instant,
};

pub const MAX_SOURCE: u64 = 128 * 1024 * 1024;
pub const MAX_HTML: usize = 128 * 1024;
const PAGE_BYTES: usize = 16 * 1024;
const MAX_EVENTS: usize = 16_384;
const MAX_CACHE: u64 = 512 * 1024 * 1024;
const MAX_PAGES: usize = 100_000;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Page {
    pub start: usize,
    pub end: usize,
    pub line: usize,
    pub offset: u64,
    pub bytes: usize,
    pub rows: usize,
    pub row_start: usize,
    pub columns: usize,
    pub table: Option<usize>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Heading {
    pub id: usize,
    pub level: u8,
    pub label: String,
    pub line: usize,
    pub page: usize,
}

#[derive(Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Index {
    pub version: u8,
    pub name: String,
    pub revision: String,
    pub source_bytes: usize,
    pub source_lines: usize,
    pub pages: Vec<Page>,
    pub toc: Vec<Heading>,
    pub diagrams: Vec<Diagram>,
    pub tables: Vec<Table>,
    pub index_ms: f64,
    pub peak_rss_kib: Option<usize>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Table {
    pub first_page: usize,
    pub end_page: usize,
    pub rows: usize,
    pub columns: Vec<String>,
    pub max_row_bytes: usize,
    pub header_bytes: usize,
}

#[derive(Serialize, Deserialize)]
pub struct Diagram {
    pub id: usize,
    pub kind: String,
    pub page: usize,
    pub start: usize,
    pub end: usize,
    pub line: usize,
    pub offset: u64,
    pub bytes: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceRange {
    pub start: usize,
    pub end: usize,
    pub line: usize,
    pub end_line: usize,
}

fn source_range(starts: &[usize], span: Range<usize>) -> SourceRange {
    SourceRange {
        start: span.start,
        end: span.end,
        line: starts.partition_point(|start| *start <= span.start),
        end_line: starts
            .partition_point(|start| *start <= span.end.saturating_sub(1).max(span.start)),
    }
}

fn plantuml_marker(line: &str, prefix: &str) -> Option<String> {
    let content = line.trim_start_matches([' ', '\t', '>']);
    let marker = content.split_whitespace().next()?.strip_prefix(prefix)?;
    matches!(
        marker,
        "uml"
            | "mindmap"
            | "wbs"
            | "gantt"
            | "json"
            | "yaml"
            | "salt"
            | "ditaa"
            | "math"
            | "latex"
            | "chen"
            | "board"
            | "ebnf"
            | "regex"
            | "wire"
    )
    .then(|| marker.to_owned())
}

fn bare_plantuml(source: &str, options: Options) -> (Cow<'_, str>, Vec<Range<usize>>) {
    if !source.contains("@start") {
        return (Cow::Borrowed(source), Vec::new());
    }
    let protected: Vec<_> = Parser::new_ext(source, options)
        .into_offset_iter()
        .filter_map(|(event, span)| {
            matches!(
                event,
                Event::Start(Tag::CodeBlock(_)) | Event::Code(_) | Event::Html(_)
            )
            .then_some(span)
        })
        .collect();
    let mut ranges = Vec::new();
    let mut replacements = Vec::new();
    let mut active: Option<(usize, String)> = None;
    let mut offset = 0;
    for line in source.split_inclusive('\n') {
        let protected_index = protected.partition_point(|span| span.start <= offset);
        let inside_code = protected_index > 0 && protected[protected_index - 1].contains(&offset);
        if !inside_code {
            if active.is_none() {
                if let Some(kind) = plantuml_marker(line, "@start") {
                    active = Some((offset, kind));
                }
            } else if let Some(kind) = plantuml_marker(line, "@end")
                && let Some((start, expected)) = &active
                && kind == *expected
            {
                let first_end = source[*start..]
                    .find('\n')
                    .map_or(source.len(), |length| *start + length);
                let first_marker = *start + source[*start..first_end].find('@').unwrap();
                let last_marker = offset + line.find('@').unwrap();
                replacements.push((first_marker..first_end, "```uml"));
                replacements.push((
                    last_marker..offset + line.trim_end_matches(['\r', '\n']).len(),
                    "```",
                ));
                ranges.push(first_marker..offset + line.len());
                active = None;
            }
        }
        offset += line.len();
    }
    if ranges.is_empty() {
        return (Cow::Borrowed(source), ranges);
    }
    let mut bytes = source.as_bytes().to_vec();
    for (range, replacement) in replacements {
        bytes[range.clone()].fill(b' ');
        bytes[range.start..range.start + replacement.len()].copy_from_slice(replacement.as_bytes());
    }
    (Cow::Owned(String::from_utf8(bytes).unwrap()), ranges)
}

pub fn peak_rss_kib() -> Option<usize> {
    fs::read_to_string("/proc/self/status")
        .ok()?
        .lines()
        .find_map(|line| {
            line.strip_prefix("VmHWM:")?
                .split_whitespace()
                .next()?
                .parse()
                .ok()
        })
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn inert_event(event: Event<'_>) -> Event<'_> {
    match event {
        Event::InlineHtml(value) if inline_break(&value) => Event::HardBreak,
        Event::Html(value) | Event::InlineHtml(value) => Event::Text(value),
        Event::Start(Tag::Image { .. }) => Event::Text("[image: ".into()),
        Event::End(TagEnd::Image) => Event::Text("]".into()),
        Event::SoftBreak => Event::HardBreak,
        other => other,
    }
}

fn inline_break(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "<br>" | "<br/>" | "<br />"
    )
}

fn diagram_kind(info: &str) -> Option<&'static str> {
    match info
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "mermaid" => Some("mermaid"),
        "plantuml" | "puml" | "uml" => Some("plantuml"),
        "d2" => Some("d2"),
        _ => None,
    }
}

fn annotate_rows(
    html: &str,
    anchors: impl IntoIterator<Item = Range<usize>>,
    starts: &[usize],
) -> io::Result<String> {
    let anchors = anchors.into_iter();
    let mut annotated = String::with_capacity(html.len() + anchors.size_hint().0 * 144);
    let mut remaining = html;
    for span in anchors {
        let (before, after) = remaining
            .split_once("<tr>")
            .ok_or_else(|| invalid("Missing table row"))?;
        let range = source_range(starts, span);
        annotated.push_str(before);
        annotated.push_str(&format!("<tr data-source-start=\"{}\" data-source-end=\"{}\" data-source-line=\"{}\" data-source-end-line=\"{}\">", range.start, range.end, range.line, range.end_line));
        remaining = after;
    }
    annotated.push_str(remaining);
    Ok(annotated)
}

struct Writer<'a> {
    source: &'a str,
    starts: Vec<usize>,
    file: BufWriter<File>,
    pages: Vec<Page>,
    offset: u64,
    diagrams: Vec<Diagram>,
    diagram_file: BufWriter<File>,
    diagram_offset: u64,
    code_spans: VecDeque<Range<usize>>,
    nested_row_spans: VecDeque<Range<usize>>,
    tables: Vec<Table>,
}

impl Writer<'_> {
    fn page(
        &mut self,
        events: Vec<Event<'_>>,
        span: Range<usize>,
        rows: usize,
        columns: usize,
        table: Option<usize>,
        row_spans: &[Range<usize>],
        header_span: Option<Range<usize>>,
    ) -> io::Result<()> {
        if events.len() > MAX_EVENTS {
            return Err(invalid(
                "An indivisible block exceeds the prototype event budget",
            ));
        }
        let mut highlight_budget = 0;
        if columns == 0
            && events
                .iter()
                .any(|event| matches!(event, Event::Start(Tag::CodeBlock(_))))
        {
            let mut plain = String::new();
            html::push_html(&mut plain, events.iter().cloned().map(inert_event));
            let anchors = events
                .iter()
                .filter(|event| matches!(event, Event::Start(Tag::TableHead | Tag::TableRow)))
                .count();
            let figures = events.iter().filter(|event| matches!(event, Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(info))) if diagram_kind(info).is_some())).count();
            highlight_budget = MAX_HTML.saturating_sub(plain.len() + anchors * 256 + figures * 512);
        }
        let mut rendered = String::new();
        let safe_events = events.into_iter().map(inert_event);
        if columns > 0 {
            html::push_html(&mut rendered, safe_events);
            let anchors = header_span.into_iter().chain(row_spans.iter().cloned());
            rendered = annotate_rows(&rendered, anchors, &self.starts)?;
        } else {
            let mut events = safe_events.peekable();
            let mut ordinary = Vec::new();
            let mut anchors = Vec::new();
            while let Some(event) = events.next() {
                if matches!(event, Event::Start(Tag::TableHead | Tag::TableRow)) {
                    anchors.push(
                        self.nested_row_spans
                            .pop_front()
                            .ok_or_else(|| invalid("Missing nested row source range"))?,
                    );
                }
                let code_span = if matches!(event, Event::Start(Tag::CodeBlock(_))) {
                    self.code_spans.pop_front()
                } else {
                    None
                };
                if let Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(info))) = &event {
                    if let Some(kind) = diagram_kind(info) {
                        html::push_html(&mut rendered, ordinary.drain(..));
                        let id = self.diagrams.len();
                        let code_span =
                            code_span.ok_or_else(|| invalid("Missing diagram source range"))?;
                        let range = source_range(&self.starts, code_span);
                        rendered.push_str(&format!(
                            "<figure data-diagram-kind=\"{kind}\" data-diagram-id=\"{id}\" data-source-start=\"{}\" data-source-end=\"{}\" data-source-line=\"{}\" data-source-end-line=\"{}\">", range.start, range.end, range.line, range.end_line
                        ));
                        let mut code = vec![event];
                        let mut source = String::new();
                        for event in events.by_ref() {
                            if let Event::Text(text) = &event {
                                source.push_str(text);
                            }
                            let finished = matches!(event, Event::End(TagEnd::CodeBlock));
                            code.push(event);
                            if finished {
                                break;
                            }
                        }
                        if source.len() > 400_000
                            || self.diagram_offset + source.len() as u64 > 64 * 1024 * 1024
                        {
                            return Err(invalid("Diagram sources exceed the prototype budget"));
                        }
                        self.diagram_file.write_all(source.as_bytes())?;
                        self.diagrams.push(Diagram {
                            id,
                            kind: kind.into(),
                            page: self.pages.len(),
                            start: range.start,
                            end: range.end,
                            line: range.line,
                            offset: self.diagram_offset,
                            bytes: source.len(),
                        });
                        self.diagram_offset += source.len() as u64;
                        html::push_html(&mut rendered, code.into_iter());
                        rendered.push_str("</figure>");
                        continue;
                    }
                }
                if let Event::Start(Tag::CodeBlock(kind)) = &event {
                    let language = match kind {
                        CodeBlockKind::Fenced(info) => info.as_ref(),
                        CodeBlockKind::Indented => "",
                    };
                    let mut code = Vec::new();
                    let mut source = String::new();
                    for event in events.by_ref() {
                        if let Event::Text(text) = &event {
                            source.push_str(text);
                        }
                        let finished = matches!(event, Event::End(TagEnd::CodeBlock));
                        code.push(event);
                        if finished {
                            break;
                        }
                    }
                    let highlighted = highlight::code(language, &source).filter(|html| {
                        let mut plain = String::new();
                        html::push_html(
                            &mut plain,
                            std::iter::once(event.clone()).chain(code.iter().cloned()),
                        );
                        let extra = html.len().saturating_sub(plain.len());
                        if extra > highlight_budget {
                            return false;
                        }
                        highlight_budget -= extra;
                        true
                    });
                    if let Some(highlighted) = highlighted {
                        html::push_html(&mut rendered, ordinary.drain(..));
                        rendered.push_str(&highlighted);
                    } else {
                        ordinary.push(event);
                        ordinary.extend(code);
                    }
                    continue;
                }
                ordinary.push(event);
            }
            html::push_html(&mut rendered, ordinary.into_iter());
            if !anchors.is_empty() {
                rendered = annotate_rows(&rendered, anchors, &self.starts)?;
            }
        }
        if rendered.len() > MAX_HTML {
            return Err(invalid(
                "An indivisible block exceeds 128 KiB of HTML; this prototype cannot display it safely",
            ));
        }
        let row_start = table.map_or(0, |id| self.tables[id].rows);
        if let Some(id) = table {
            let metadata = &mut self.tables[id];
            metadata.end_page = self.pages.len() + 1;
            metadata.rows += rows;
            metadata.header_bytes = metadata.header_bytes.max(table_header(&rendered)?.len());
            if let Some((_, body)) = rendered.split_once("<tbody>") {
                metadata.max_row_bytes = metadata.max_row_bytes.max(
                    body.split_inclusive("</tr>")
                        .filter(|row| row.contains("<tr"))
                        .map(str::len)
                        .max()
                        .unwrap_or(0),
                );
            }
        }
        if self.offset + rendered.len() as u64 > MAX_CACHE || self.pages.len() >= MAX_PAGES {
            return Err(invalid("Document exceeds the prototype cache/index budget"));
        }
        let line = self.starts.partition_point(|start| *start <= span.start);
        self.pages.push(Page {
            start: span.start,
            end: span.end.min(self.source.len()),
            line,
            offset: self.offset,
            bytes: rendered.len(),
            rows,
            row_start,
            columns,
            table,
        });
        self.file.write_all(rendered.as_bytes())?;
        self.offset += rendered.len() as u64;
        Ok(())
    }
}

fn table_page<'a>(
    alignments: &[Alignment],
    header: &[Event<'a>],
    rows: &mut Vec<Event<'a>>,
) -> Vec<Event<'a>> {
    let mut events = vec![Event::Start(Tag::Table(alignments.to_vec()))];
    events.extend(header.iter().cloned());
    events.append(rows);
    events.push(Event::End(TagEnd::Table));
    events
}

pub fn build_index(path: &Path, directory: &Path) -> io::Result<Index> {
    let started = Instant::now();
    let file = File::open(path)?;
    if !file.metadata()?.is_file() || file.metadata()?.len() > MAX_SOURCE {
        return Err(invalid("Expected a regular UTF-8 file of at most 128 MiB"));
    }
    let mut source = String::new();
    file.take(MAX_SOURCE + 1).read_to_string(&mut source)?;
    if source.len() as u64 > MAX_SOURCE {
        return Err(invalid("File grew beyond the 128 MiB prototype limit"));
    }
    fs::create_dir_all(directory)?;
    fs::write(directory.join("source.md"), &source)?;
    let mut starts = vec![0];
    starts.extend(source.match_indices('\n').map(|(offset, _)| offset + 1));
    let mut lines = BufWriter::new(File::create(directory.join("lines.bin"))?);
    for offset in &starts {
        lines.write_all(&(*offset as u64).to_le_bytes())?;
    }
    lines.flush()?;
    let mut writer = Writer {
        source: &source,
        starts,
        file: BufWriter::new(File::create(directory.join("pages.html"))?),
        pages: Vec::new(),
        offset: 0,
        diagrams: Vec::new(),
        diagram_file: BufWriter::new(File::create(directory.join("diagrams.src"))?),
        diagram_offset: 0,
        code_spans: VecDeque::new(),
        nested_row_spans: VecDeque::new(),
        tables: Vec::new(),
    };
    let options =
        Options::ENABLE_TABLES | Options::ENABLE_TASKLISTS | Options::ENABLE_STRIKETHROUGH;
    let (parse_source, bare) = bare_plantuml(&source, options);
    let mut events = Parser::new_ext(&parse_source, options)
        .into_offset_iter()
        .peekable();
    let mut pending = Vec::new();
    let mut pending_span = 0..0;
    let mut depth = 0usize;
    let mut toc = Vec::<Heading>::new();
    let mut active_heading = None;
    let mut heading_offsets = Vec::new();
    let mut table_id = 0;
    let mut active_diagram = false;
    while let Some((mut event, span)) = events.next() {
        if let Event::Start(Tag::TableHead) = &event {
            let end = source[span.start..]
                .find('\n')
                .map_or(span.end, |length| span.start + length);
            writer.nested_row_spans.push_back(span.start..end);
        } else if matches!(event, Event::Start(Tag::TableRow)) {
            writer.nested_row_spans.push_back(span.clone());
        }
        if matches!(event, Event::Start(Tag::CodeBlock(_))) {
            writer.code_spans.push_back(span.clone());
        }
        let bare_source = if matches!(event, Event::Start(Tag::CodeBlock(_)))
            && bare
                .binary_search_by_key(&span.start, |range| range.start)
                .is_ok()
        {
            for (event, _) in events.by_ref() {
                if matches!(event, Event::End(TagEnd::CodeBlock)) {
                    break;
                }
            }
            Some(&source[span.clone()])
        } else {
            None
        };
        if depth == 0
            && let Event::Start(Tag::Table(alignments)) = &event
        {
            if !pending.is_empty() {
                writer.page(
                    std::mem::take(&mut pending),
                    pending_span.clone(),
                    0,
                    0,
                    None,
                    &[],
                    None,
                )?;
            }
            let alignments = alignments.clone();
            if alignments.len() > 256 {
                return Err(invalid(
                    "Tables above 256 columns are not supported by this prototype",
                ));
            }
            let mut header = Vec::new();
            for (event, header_span) in events.by_ref() {
                if header_span.end.saturating_sub(span.start) > MAX_HTML {
                    return Err(invalid("Table header exceeds the prototype byte budget"));
                }
                let finished = matches!(event, Event::End(TagEnd::TableHead));
                header.push(event);
                if header.len() > MAX_EVENTS {
                    return Err(invalid("Table header exceeds the prototype budget"));
                }
                if finished {
                    break;
                }
            }
            let row_limit = (512 / alignments.len().max(1)).clamp(1, 16);
            let mut labels = Vec::new();
            for event in &header {
                match event {
                    Event::Start(Tag::TableCell) => labels.push(String::new()),
                    Event::Text(text) | Event::Code(text) => {
                        if let Some(label) = labels.last_mut() {
                            let remaining = 160usize.saturating_sub(label.chars().count());
                            label.extend(text.chars().take(remaining));
                        }
                    }
                    Event::InlineHtml(value) if inline_break(value) => {
                        if let Some(label) = labels.last_mut() {
                            if label.chars().count() < 160 {
                                label.push(' ');
                            }
                        }
                    }
                    _ => {}
                }
            }
            labels.resize(alignments.len(), String::new());
            writer.tables.push(Table {
                first_page: writer.pages.len(),
                end_page: writer.pages.len(),
                rows: 0,
                columns: labels,
                max_row_bytes: 0,
                header_bytes: 0,
            });
            let mut rows = Vec::new();
            let header_span = span.start
                ..source[span.start..]
                    .find('\n')
                    .map_or(span.end, |length| span.start + length);
            let mut row_spans = Vec::new();
            let mut row_count = 0;
            let mut chunk_span = span.start..span.end;
            let mut emitted = false;
            for (event, row_span) in events.by_ref() {
                if matches!(event, Event::End(TagEnd::Table)) {
                    if row_count > 0 || !emitted {
                        writer.page(
                            table_page(&alignments, &header, &mut rows),
                            chunk_span.clone(),
                            row_count,
                            alignments.len(),
                            Some(table_id),
                            &row_spans,
                            Some(header_span.clone()),
                        )?;
                    }
                    break;
                }
                if matches!(event, Event::Start(Tag::TableRow)) && row_count == 0 {
                    chunk_span.start = if emitted { row_span.start } else { span.start };
                }
                if matches!(event, Event::Start(Tag::TableRow)) {
                    row_spans.push(row_span.clone());
                }
                let end_row = matches!(event, Event::End(TagEnd::TableRow));
                chunk_span.end = row_span.end;
                rows.push(event);
                if rows.len() + header.len() > MAX_EVENTS
                    || chunk_span.end.saturating_sub(chunk_span.start) > MAX_HTML
                {
                    return Err(invalid("A table row exceeds the prototype event budget"));
                }
                if end_row {
                    row_count += 1;
                    if row_count >= row_limit
                        || chunk_span.end.saturating_sub(chunk_span.start) >= PAGE_BYTES
                    {
                        writer.page(
                            table_page(&alignments, &header, &mut rows),
                            chunk_span.clone(),
                            row_count,
                            alignments.len(),
                            Some(table_id),
                            &row_spans,
                            Some(header_span.clone()),
                        )?;
                        row_count = 0;
                        row_spans.clear();
                        emitted = true;
                    }
                }
            }
            table_id += 1;
            continue;
        }
        if depth == 0 {
            let heading = matches!(event, Event::Start(Tag::Heading { .. }));
            let diagram = matches!(&event, Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(info))) if diagram_kind(info).is_some());
            if !pending.is_empty()
                && (heading
                    || diagram
                    || pending_span.end.saturating_sub(pending_span.start) >= PAGE_BYTES)
            {
                writer.page(
                    std::mem::take(&mut pending),
                    pending_span.clone(),
                    0,
                    0,
                    None,
                    &[],
                    None,
                )?;
            }
            active_diagram = diagram;
            if pending.is_empty() {
                pending_span.start = span.start;
            }
        }
        if let Event::Start(Tag::Heading { level, id, .. }) = &mut event {
            let heading_id = toc.len();
            *id = Some(format!("heading-{heading_id}").into());
            active_heading = Some(heading_id);
            toc.push(Heading {
                id: heading_id,
                level: *level as u8,
                label: String::new(),
                line: writer.starts.partition_point(|start| *start <= span.start),
                page: 0,
            });
            heading_offsets.push(span.start);
        }
        if let Some(index) = active_heading {
            match &event {
                Event::Text(text) | Event::Code(text) => {
                    let remaining = 512usize.saturating_sub(toc[index].label.chars().count());
                    toc[index].label.extend(text.chars().take(remaining));
                }
                Event::SoftBreak | Event::HardBreak => toc[index].label.push(' '),
                Event::End(TagEnd::Heading(_)) => active_heading = None,
                _ => {}
            }
        }
        match &event {
            Event::Start(_) => depth += 1,
            Event::End(_) => depth = depth.saturating_sub(1),
            _ => {}
        }
        pending_span.end = span.end;
        pending.push(event);
        if let Some(source) = bare_source {
            pending.push(Event::Text(source.into()));
            pending.push(Event::End(TagEnd::CodeBlock));
            depth = depth.saturating_sub(1);
        }
        if pending.len() > MAX_EVENTS
            || pending_span.end.saturating_sub(pending_span.start) > MAX_HTML
        {
            return Err(invalid("An indivisible block exceeds the prototype budget"));
        }
        if active_diagram && depth == 0 {
            writer.page(
                std::mem::take(&mut pending),
                pending_span.clone(),
                0,
                0,
                None,
                &[],
                None,
            )?;
            active_diagram = false;
        }
    }
    if !pending.is_empty() {
        writer.page(pending, pending_span, 0, 0, None, &[], None)?;
    }
    writer.file.flush()?;
    writer.diagram_file.flush()?;
    for (heading, offset) in toc.iter_mut().zip(heading_offsets) {
        heading.page = writer
            .pages
            .partition_point(|page| page.start <= offset)
            .saturating_sub(1);
    }
    let index = Index {
        version: 5,
        name: path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .chars()
            .take(240)
            .collect(),
        revision: Sha256::digest(source.as_bytes())
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
        source_bytes: source.len(),
        source_lines: writer.starts.len(),
        pages: writer.pages,
        toc,
        diagrams: writer.diagrams,
        tables: writer.tables,
        index_ms: started.elapsed().as_secs_f64() * 1000.0,
        peak_rss_kib: peak_rss_kib(),
    };
    fs::write(directory.join("index.pending"), serde_json::to_vec(&index)?)?;
    fs::rename(
        directory.join("index.pending"),
        directory.join("index.json"),
    )?;
    Ok(index)
}

pub fn read_page(directory: &Path, index: &Index, page: usize) -> io::Result<String> {
    let metadata = index
        .pages
        .get(page)
        .ok_or_else(|| invalid("Page out of range"))?;
    if metadata.bytes > MAX_HTML {
        return Err(invalid("Invalid page budget"));
    }
    let mut file = File::open(directory.join("pages.html"))?;
    file.seek(SeekFrom::Start(metadata.offset))?;
    let mut bytes = vec![0; metadata.bytes];
    file.read_exact(&mut bytes)?;
    String::from_utf8(bytes).map_err(|_| invalid("Invalid cached UTF-8"))
}

pub fn read_diagram(directory: &Path, index: &Index, id: usize) -> io::Result<String> {
    let metadata = index
        .diagrams
        .get(id)
        .ok_or_else(|| invalid("Diagram out of range"))?;
    if metadata.bytes > 400_000 {
        return Err(invalid("Diagram source exceeds budget"));
    }
    let mut file = File::open(directory.join("diagrams.src"))?;
    file.seek(SeekFrom::Start(metadata.offset))?;
    let mut source = String::new();
    file.take(metadata.bytes as u64)
        .read_to_string(&mut source)?;
    if source.len() != metadata.bytes {
        return Err(invalid("Incomplete diagram source cache"));
    }
    Ok(source)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TableWindow {
    pub html: String,
    pub row_start: usize,
    pub row_end: usize,
    pub page_size: usize,
    pub total_rows: usize,
    pub columns: Vec<usize>,
}

fn project_row(row: &str, cell: &str, columns: &[usize]) -> io::Result<String> {
    let start = row
        .find("<tr")
        .ok_or_else(|| invalid("Missing cached row"))?;
    let end = start
        + row[start..]
            .find('>')
            .ok_or_else(|| invalid("Invalid cached row"))?
        + 1;
    let mut output = row[..end].to_owned();
    let mut remaining = &row[end..];
    let mut selected = columns.iter().copied().peekable();
    let mut column = 0;
    let opening = format!("<{cell}");
    let closing = format!("</{cell}>");
    while let Some(start) = remaining.find(&opening) {
        let end = start
            + remaining[start..]
                .find(&closing)
                .ok_or_else(|| invalid("Invalid cached cell"))?
            + closing.len();
        if selected.peek() == Some(&column) {
            output.push_str(&remaining[..end]);
            selected.next();
        }
        remaining = &remaining[end..];
        column += 1;
    }
    if selected.peek().is_some() {
        return Err(invalid("Missing selected column"));
    }
    output.push_str(remaining);
    Ok(output)
}

pub fn read_table(
    directory: &Path,
    index: &Index,
    id: usize,
    row: usize,
    columns: &[usize],
) -> io::Result<TableWindow> {
    let table = index
        .tables
        .get(id)
        .ok_or_else(|| invalid("Table out of range"))?;
    if columns.is_empty()
        || columns.len() > table.columns.len()
        || columns.windows(2).any(|pair| pair[0] >= pair[1])
        || columns
            .last()
            .is_some_and(|column| *column >= table.columns.len())
        || row >= table.rows.max(1)
    {
        return Err(invalid("Invalid table row or columns"));
    }
    let byte_limit =
        MAX_HTML.saturating_sub(table.header_bytes + 1024) / table.max_row_bytes.max(1);
    let page_size = 50.min(512 / columns.len()).min(byte_limit).max(1);
    let row_start = row / page_size * page_size;
    let row_end = (row_start + page_size).min(table.rows);
    let first = read_page(directory, index, table.first_page)?;
    let header = first
        .split_once("<thead>")
        .and_then(|(_, rest)| rest.split_once("</thead>"))
        .ok_or_else(|| invalid("Invalid cached header"))?
        .0;
    let mut html = format!(
        "<table><thead>{}</thead><tbody>",
        project_row(header, "th", columns)?
    );
    let pages = &index.pages[table.first_page..table.end_page];
    let mut position = pages.partition_point(|page| page.row_start + page.rows <= row_start);
    while let Some(metadata) = pages.get(position) {
        if metadata.row_start >= row_end {
            break;
        }
        let content = read_page(directory, index, table.first_page + position)?;
        for (offset, source) in table_body(&content)?
            .split_inclusive("</tr>")
            .filter(|source| source.contains("<tr"))
            .enumerate()
        {
            let current = metadata.row_start + offset;
            if current >= row_start && current < row_end {
                html.push_str(&project_row(source, "td", columns)?);
            }
        }
        position += 1;
    }
    html.push_str("</tbody></table>");
    if html.len() > MAX_HTML {
        return Err(invalid("Table page exceeds HTML budget"));
    }
    Ok(TableWindow {
        html,
        row_start,
        row_end,
        page_size,
        total_rows: table.rows,
        columns: columns.to_vec(),
    })
}

#[derive(Serialize)]
pub struct TableAudit {
    pub tables: usize,
    pub rows: usize,
    pub cells: usize,
    pub fragments: usize,
}

fn table_body(html: &str) -> io::Result<&str> {
    if !html.contains("<tbody>") {
        return Ok("");
    }
    html.split_once("<tbody>")
        .and_then(|(_, body)| body.split_once("</tbody>").map(|(body, _)| body))
        .ok_or_else(|| invalid("Invalid table body"))
}

fn table_header(html: &str) -> io::Result<String> {
    let header = html
        .split_once("<thead>")
        .and_then(|(_, header)| header.split_once("</thead>").map(|(header, _)| header))
        .ok_or_else(|| invalid("Invalid table header"))?;
    let (prefix, row) = header
        .split_once("<tr")
        .ok_or_else(|| invalid("Invalid table header row"))?;
    let (_, content) = row
        .split_once('>')
        .ok_or_else(|| invalid("Invalid table header row"))?;
    Ok(format!("{prefix}<tr>{content}"))
}

fn audit_body(digest: &mut Sha256, html: &str) -> io::Result<(usize, usize)> {
    let mut rows = 0;
    let mut cells = 0;
    for row in table_body(html)?.split("</tr>") {
        let row = row.trim();
        if row.is_empty() {
            continue;
        }
        if !row.starts_with("<tr") {
            return Err(invalid("Invalid cached table row"));
        }
        let (_, content) = row
            .split_once('>')
            .ok_or_else(|| invalid("Invalid cached table row"))?;
        digest.update(b"<tr>");
        digest.update(content.as_bytes());
        digest.update(b"</tr>");
        rows += 1;
        cells += row.matches("<td").count();
    }
    Ok((rows, cells))
}

fn row_anchor(row: &str) -> io::Result<SourceRange> {
    let tag = row
        .split_once('>')
        .ok_or_else(|| invalid("Invalid source row"))?
        .0;
    let attribute = |name: &str| -> io::Result<usize> {
        tag.split_once(&format!(" data-source-{name}=\""))
            .and_then(|(_, value)| value.split_once('"'))
            .and_then(|(value, _)| value.parse().ok())
            .ok_or_else(|| invalid("Missing source row attribute"))
    };
    Ok(SourceRange {
        start: attribute("start")?,
        end: attribute("end")?,
        line: attribute("line")?,
        end_line: attribute("end-line")?,
    })
}

pub fn audit_tables(directory: &Path, index: &Index) -> io::Result<TableAudit> {
    let source = fs::read_to_string(directory.join("source.md"))?;
    let options =
        Options::ENABLE_TABLES | Options::ENABLE_TASKLISTS | Options::ENABLE_STRIKETHROUGH;
    let mut depth = 0usize;
    let mut table = Vec::new();
    let mut expected = Vec::new();
    let mut active = false;
    let mut rows = 0;
    let mut cells = 0;
    let mut starts = vec![0];
    starts.extend(source.match_indices('\n').map(|(offset, _)| offset + 1));
    let mut anchors = Vec::<Vec<SourceRange>>::new();
    let (parse_source, _) = bare_plantuml(&source, options);
    for (event, span) in Parser::new_ext(&parse_source, options).into_offset_iter() {
        if depth == 0 && matches!(event, Event::Start(Tag::Table(_))) {
            active = true;
            let end = source[span.start..]
                .find('\n')
                .map_or(span.end, |length| span.start + length);
            anchors.push(vec![source_range(&starts, span.start..end)]);
        }
        if active && matches!(event, Event::Start(Tag::TableRow)) {
            anchors
                .last_mut()
                .unwrap()
                .push(source_range(&starts, span));
        }
        match &event {
            Event::Start(_) => depth += 1,
            Event::End(_) => depth = depth.saturating_sub(1),
            _ => {}
        }
        if active {
            table.push(inert_event(event));
            if depth == 0 {
                let mut html = String::new();
                html::push_html(&mut html, table.drain(..));
                let mut digest = Sha256::new();
                let (table_rows, table_cells) = audit_body(&mut digest, &html)?;
                rows += table_rows;
                cells += table_cells;
                expected.push((digest.finalize(), table_header(&html)?.to_owned()));
                active = false;
            }
        }
    }
    let mut actual: Vec<_> = (0..expected.len()).map(|_| Sha256::new()).collect();
    let mut fragments = 0;
    let mut actual_rows = 0;
    let mut actual_cells = 0;
    let mut row_positions = vec![1usize; expected.len()];
    for (page_id, metadata) in index.pages.iter().enumerate() {
        if let Some(table_id) = metadata.table {
            let digest = actual
                .get_mut(table_id)
                .ok_or_else(|| invalid("Unexpected table fragment"))?;
            let html = read_page(directory, index, page_id)?;
            for (position, row) in html.split("<tr").skip(1).enumerate() {
                let anchor = row_anchor(row)?;
                let row_id = if position == 0 {
                    0
                } else {
                    let row_id = row_positions[table_id];
                    row_positions[table_id] += 1;
                    row_id
                };
                if Some(&anchor) != anchors[table_id].get(row_id) {
                    return Err(invalid("Table row source mapping mismatch"));
                }
            }
            if table_header(&html)? != expected[table_id].1 {
                return Err(invalid(
                    "Repeated table header differs from full-document rendering",
                ));
            }
            let (rows, cells) = audit_body(digest, &html)?;
            if rows != metadata.rows || cells != rows * metadata.columns {
                return Err(invalid("Table fragment metadata mismatch"));
            }
            actual_rows += rows;
            actual_cells += cells;
            fragments += 1;
        }
    }
    if rows != actual_rows
        || cells != actual_cells
        || actual
            .into_iter()
            .zip(&expected)
            .any(|(digest, (expected, _))| digest.finalize() != *expected)
    {
        return Err(invalid(
            "Fragmented tables differ from full-document rendering",
        ));
    }
    for (id, table) in index.tables.iter().enumerate() {
        let columns = (0..table.columns.len()).collect::<Vec<_>>();
        let mut digest = Sha256::new();
        let mut row = 0;
        loop {
            let window = read_table(directory, index, id, row, &columns)?;
            if table_header(&window.html)? != expected[id].1 {
                return Err(invalid("Paged table header differs from original"));
            }
            let (count, _) = audit_body(&mut digest, &window.html)?;
            if count != window.row_end - window.row_start {
                return Err(invalid("Paged table row count mismatch"));
            }
            for (position, rendered) in window.html.split("<tr").skip(1).enumerate() {
                let original = if position == 0 {
                    0
                } else {
                    window.row_start + position
                };
                if Some(&row_anchor(rendered)?) != anchors[id].get(original) {
                    return Err(invalid("Paged table source anchor mismatch"));
                }
            }
            row = window.row_end;
            if row >= table.rows {
                break;
            }
        }
        if digest.finalize() != expected[id].0 {
            return Err(invalid("Paged table cells differ from original"));
        }
    }
    Ok(TableAudit {
        tables: expected.len(),
        rows,
        cells,
        fragments,
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Match {
    pub page: usize,
    pub line: usize,
    pub snippet: String,
    pub table_row: Option<usize>,
}

pub fn search(directory: &Path, index: &Index, query: &str) -> io::Result<Vec<Match>> {
    if query.is_empty() || query.len() > 256 {
        return Err(invalid("Search requires 1–256 UTF-8 bytes"));
    }
    let source = fs::read_to_string(directory.join("source.md"))?;
    let mut matches = Vec::new();
    let mut offset = 0;
    for (line_index, line) in source.split_inclusive('\n').enumerate() {
        if let Some(column) = line.find(query) {
            let page = index
                .pages
                .partition_point(|page| page.end <= offset + column)
                .min(index.pages.len().saturating_sub(1));
            let table_row = if index
                .pages
                .get(page)
                .is_some_and(|page| page.table.is_some())
            {
                let content = read_page(directory, index, page)?;
                let position =
                    content
                        .split("<tr")
                        .skip(1)
                        .enumerate()
                        .find_map(|(position, row)| {
                            let anchor = row_anchor(row).ok()?;
                            (anchor.line <= line_index + 1 && anchor.end_line >= line_index + 1)
                                .then_some(position)
                        });
                position.map(|position| {
                    if position == 0 {
                        0
                    } else {
                        index.pages[page].row_start + position - 1
                    }
                })
            } else {
                None
            };
            matches.push(Match {
                page,
                line: line_index + 1,
                snippet: line.chars().take(160).collect(),
                table_row,
            });
            if matches.len() == 100 {
                break;
            }
        }
        offset += line.len();
    }
    Ok(matches)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(source: &str) -> (tempfile::TempDir, Index) {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("input.md");
        fs::write(&path, source).unwrap();
        let index = build_index(&path, directory.path()).unwrap();
        (directory, index)
    }

    #[test]
    fn inline_breaks_render_without_enabling_arbitrary_html() {
        let (directory, index) = fixture(
            "| Header<br>detail | Other |\n|---|---|\n| first<BR/>second | <br onclick=alert(1)> &lt;br&gt; |\n",
        );
        let page = read_table(directory.path(), &index, 0, 0, &[0, 1]).unwrap();
        assert_eq!(index.tables[0].columns[0], "Header detail");
        assert!(page.html.contains("first<br />\nsecond"));
        assert!(page.html.contains("&lt;br onclick=alert(1)&gt;"));
        assert!(page.html.contains("&lt;br&gt;"));
        assert!(!page.html.contains("<br onclick="));
        audit_tables(directory.path(), &index).unwrap();
    }

    #[test]
    fn table_projection_preserves_selected_cells_anchors_and_pagination() {
        let mut source = String::from("# 表格😀\r\n\r\n| 重复 | 重复 | |\r\n|:---|---:|---|\r\n");
        for row in 0..123 {
            source.push_str(&format!(
                "| **row-{row}** | [link](https://example.com) | 中文\\|{row} |\r\n"
            ));
        }
        let (directory, index) = fixture(&source);
        assert_eq!(index.tables[0].columns, ["重复", "重复", ""]);
        let mut rows = 0;
        for offset in [0, 50, 100] {
            let page = read_table(directory.path(), &index, 0, offset, &[0, 2]).unwrap();
            assert_eq!(page.page_size, 50);
            assert_eq!(page.row_start, offset);
            assert_eq!(page.html.matches("<th ").count(), 1);
            assert!(!page.html.contains("href="));
            assert!(page.html.contains("<strong>"));
            for (position, row) in page.html.split("<tr").skip(1).enumerate() {
                let range = row_anchor(row).unwrap();
                assert_eq!(
                    range.line,
                    if position == 0 {
                        3
                    } else {
                        offset + position + 4
                    }
                );
            }
            rows += page.row_end - page.row_start;
        }
        assert_eq!(rows, 123);
        let found = search(directory.path(), &index, "row-54").unwrap();
        assert_eq!(found[0].table_row, Some(54));
        let selected = read_table(
            directory.path(),
            &index,
            0,
            found[0].table_row.unwrap(),
            &[0],
        )
        .unwrap();
        assert!(selected.html.contains("row-54"));
        assert_eq!(
            fs::read_to_string(directory.path().join("source.md")).unwrap(),
            source
        );
    }

    #[test]
    fn table_projection_validates_columns_and_supports_header_only_tables() {
        let (directory, index) = fixture("| A | B |\n|---|---|\n");
        let page = read_table(directory.path(), &index, 0, 0, &[1]).unwrap();
        assert_eq!(page.total_rows, 0);
        assert!(page.html.contains(">B</th>"));
        for columns in [vec![], vec![2], vec![1, 0], vec![0, 0], vec![usize::MAX]] {
            assert!(read_table(directory.path(), &index, 0, 0, &columns).is_err());
        }
        assert!(read_table(directory.path(), &index, 1, 0, &[0]).is_err());
        assert!(read_table(directory.path(), &index, 0, 1, &[0]).is_err());
    }

    #[test]
    fn wide_table_pages_bound_cells_and_expand_when_columns_are_hidden() {
        let mut source = format!(
            "| {} |\n| {} |\n",
            vec!["Header"; 54].join(" | "),
            vec!["---"; 54].join(" | ")
        );
        for row in 0..119 {
            source.push_str(&format!(
                "| {} |\n",
                (0..54)
                    .map(|column| format!("r{row}c{column}"))
                    .collect::<Vec<_>>()
                    .join(" | ")
            ));
        }
        let (directory, index) = fixture(&source);
        let all = (0..54).collect::<Vec<_>>();
        let wide = read_table(directory.path(), &index, 0, 118, &all).unwrap();
        assert_eq!(wide.page_size, 9);
        assert!(wide.html.contains("r118c53"));
        assert!(wide.html.matches("<td>").count() <= 512);
        let narrow = read_table(directory.path(), &index, 0, 118, &[0, 53]).unwrap();
        assert_eq!(narrow.page_size, 50);
        assert_eq!(narrow.row_start, 100);
        assert!(narrow.html.contains("r118c53"));
        assert!(!narrow.html.contains("r118c1<"));
        audit_tables(directory.path(), &index).unwrap();
    }

    #[test]
    fn diagram_inventory_preserves_bare_sources_and_literal_code() {
        let source = "# Diagrams\r\n\r\n```mermaid\nflowchart LR\n A --> B\n```\n\n@startuml\r\nAlice -> Bob\r\n\r\nBob --> Alice\r\n@enduml\r\n\r\n@startmindmap\n* Root\n** Child\n@endmindmap\n\n```text\n@startuml\nLiteral -> Code\n@enduml\n```\n\n```d2\na -> b\n```\n\n```puml\nA -> B\n```\n";
        let (directory, index) = fixture(source);
        assert_eq!(
            index
                .diagrams
                .iter()
                .map(|diagram| diagram.kind.as_str())
                .collect::<Vec<_>>(),
            ["mermaid", "plantuml", "plantuml", "d2", "plantuml"]
        );
        let html = (0..index.pages.len())
            .map(|page| read_page(directory.path(), &index, page).unwrap())
            .collect::<String>();
        assert_eq!(html.matches("data-diagram-kind=").count(), 5);
        assert!(html.contains("@startuml\r\nAlice"));
        assert!(html.contains("@startmindmap\n* Root"));
        assert!(html.contains("class=\"language-text\""));
        assert_eq!(
            index.revision,
            Sha256::digest(source.as_bytes())
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        );
        assert_eq!(index.source_bytes, source.len());
        for diagram in &index.diagrams {
            assert_eq!(
                diagram.line,
                source[..diagram.start]
                    .bytes()
                    .filter(|byte| *byte == b'\n')
                    .count()
                    + 1
            );
            let stored = read_diagram(directory.path(), &index, diagram.id).unwrap();
            assert_eq!(stored.len(), diagram.bytes);
            assert!(!stored.is_empty());
            assert!(
                read_page(directory.path(), &index, diagram.page)
                    .unwrap()
                    .contains(&format!("data-diagram-id=\"{}\"", diagram.id))
            );
        }
        assert!(
            read_diagram(directory.path(), &index, 1)
                .unwrap()
                .contains("@startuml\r\nAlice")
        );
        assert!(
            read_diagram(directory.path(), &index, 2)
                .unwrap()
                .contains("@startmindmap\n* Root")
        );
    }

    #[test]
    fn wide_tables_split_rows_without_dropping_or_duplicating_body_cells() {
        let mut source = String::from("# Table\n\n| A | B |\n|:---|---:|\n");
        for row in 0..73 {
            source.push_str(&format!("| row-{row:04} | **value-{row:04}** |\n"));
        }
        source.push_str("\n## End\n\n[last][ref]\n\n[ref]: https://example.com\n");
        let (directory, index) = fixture(&source);
        assert!(index.pages.len() > 5);
        let mut html = String::new();
        for page in 0..index.pages.len() {
            let content = read_page(directory.path(), &index, page).unwrap();
            assert!(content.len() <= MAX_HTML);
            html.push_str(&content);
        }
        for row in 0..73 {
            assert_eq!(html.matches(&format!("row-{row:04}")).count(), 1);
        }
        assert_eq!(index.pages.iter().map(|page| page.rows).sum::<usize>(), 73);
        assert!(html.contains("text-align: right"));
        assert!(html.contains("href=\"https://example.com\""));
        assert!(html.contains("<strong>value-0000</strong>"));
        assert_eq!(index.toc.len(), 2);
        let audit = audit_tables(directory.path(), &index).unwrap();
        assert_eq!((audit.tables, audit.rows, audit.cells), (1, 73, 146));
        assert!(
            read_page(directory.path(), &index, index.toc[1].page)
                .unwrap()
                .contains("heading-1")
        );
    }

    #[test]
    fn table_audit_detects_content_corruption_even_when_sizes_and_counts_match() {
        let (directory, index) = fixture("| Header |\n|---|\n| original |\n");
        audit_tables(directory.path(), &index).unwrap();
        let path = directory.path().join("pages.html");
        let html = fs::read_to_string(&path).unwrap();
        fs::write(&path, html.replace("original", "modified")).unwrap();
        assert!(audit_tables(directory.path(), &index).is_err());
        fs::write(&path, html.replace("Header", "Broken")).unwrap();
        assert!(audit_tables(directory.path(), &index).is_err());
        fs::write(
            &path,
            html.replace("data-source-line=\"3\"", "data-source-line=\"9\""),
        )
        .unwrap();
        assert!(audit_tables(directory.path(), &index).is_err());
    }

    #[test]
    fn table_row_lines_remain_original_across_fragments_unicode_crlf_and_repeated_headers() {
        for newline in ["\n", "\r\n"] {
            let columns = (0..54)
                .map(|column| format!("Column {column}"))
                .collect::<Vec<_>>();
            let mut source = format!(
                "# 中文😀{newline}{newline}| {} |{newline}| {} |{newline}",
                columns.join(" | "),
                vec!["---"; 54].join(" | ")
            );
            for row in 0..40 {
                source.push_str(&format!(
                    "| {} |{newline}",
                    (0..54)
                        .map(|column| format!("行{row}-列{column}"))
                        .collect::<Vec<_>>()
                        .join(" | ")
                ));
            }
            source.push_str(&format!("{newline}## End{newline}"));
            let (directory, index) = fixture(&source);
            let audit = audit_tables(directory.path(), &index).unwrap();
            assert_eq!(audit.rows, 40);
            let mut next_row = 0;
            for (page_id, page) in index.pages.iter().enumerate() {
                assert_eq!(
                    page.line,
                    source[..page.start]
                        .bytes()
                        .filter(|byte| *byte == b'\n')
                        .count()
                        + 1
                );
                if page.table.is_none() {
                    continue;
                }
                let html = read_page(directory.path(), &index, page_id).unwrap();
                for (position, row) in html.split("<tr").skip(1).enumerate() {
                    let anchor = row_anchor(row).unwrap();
                    assert_eq!(
                        anchor.line,
                        source[..anchor.start]
                            .bytes()
                            .filter(|byte| *byte == b'\n')
                            .count()
                            + 1
                    );
                    if position == 0 {
                        assert_eq!((anchor.line, anchor.end_line), (3, 3));
                    } else {
                        assert_eq!((anchor.line, anchor.end_line), (5 + next_row, 5 + next_row));
                        assert!(
                            source[anchor.start..anchor.end].contains(&format!("行{next_row}-列0"))
                        );
                        next_row += 1;
                    }
                }
            }
            assert_eq!(next_row, 40);
            assert_eq!(index.toc[1].line, 46);
            let found = search(directory.path(), &index, "行25-列0").unwrap();
            assert_eq!(found[0].line, 30);
            assert!(
                read_page(directory.path(), &index, found[0].page)
                    .unwrap()
                    .contains("行25-列0")
            );
        }
    }

    #[test]
    fn nested_tables_keep_source_lines_and_top_level_diagrams_get_separate_fragments() {
        let source = "> | Quoted | Value |\r\n> |---|---|\r\n> | 中文😀 | first |\r\n\r\n- Item\r\n\r\n  | Nested | Value |\r\n  |---|---|\r\n  | NestedRow | second |\r\n\r\n```mermaid\nflowchart LR\n A --> B\n```\n\nAfter\n\n@startmindmap\n* Root\n@endmindmap\n";
        let (directory, index) = fixture(source);
        let html = (0..index.pages.len())
            .map(|page| read_page(directory.path(), &index, page).unwrap())
            .collect::<String>();
        let lines = html
            .split("<tr")
            .skip(1)
            .map(|row| row_anchor(row).unwrap().line)
            .collect::<Vec<_>>();
        assert_eq!(lines, [1, 3, 7, 9]);
        assert_eq!(index.diagrams.len(), 2);
        assert_ne!(index.diagrams[0].page, index.diagrams[1].page);
        for diagram in &index.diagrams {
            let page = &index.pages[diagram.page];
            assert_eq!(
                (page.start, page.end, page.line),
                (diagram.start, diagram.end, diagram.line)
            );
            let html = read_page(directory.path(), &index, diagram.page).unwrap();
            assert_eq!(html.matches("data-diagram-id=").count(), 1);
            assert!(!html.contains("After"));
        }
    }

    #[test]
    fn indented_source_matches_resolve_to_their_actual_fragment() {
        let mut source = String::from(
            "# First\n\nBefore\n\n  ## Indented Heading\n\n  | A | B |\n  |---|---|\n",
        );
        for row in 0..40 {
            source.push_str(&format!("  | row-{row:02} | 中文😀 |\n"));
        }
        let (directory, index) = fixture(&source);
        for query in ["Indented Heading", "  | row-16", "row-32"] {
            let found = search(directory.path(), &index, query).unwrap();
            assert_eq!(found.len(), 1);
            let expected_line = source
                .lines()
                .position(|line| line.contains(query))
                .unwrap()
                + 1;
            assert_eq!(found[0].line, expected_line);
            let page = read_page(directory.path(), &index, found[0].page).unwrap();
            assert!(
                page.contains(query.trim_start_matches([' ', '|'])),
                "{query} must be in its actual fragment"
            );
        }
    }

    #[test]
    fn complete_toc_recognizes_setext_nested_and_unicode_headings() {
        let source = format!(
            "中文😀\r\n=====\r\n\r\n```md\n# Not a heading\n```\n\n> ## Quoted\n\n{}",
            (0..2100)
                .map(|index| format!("\n## Heading {index}\n\nParagraph.\n"))
                .collect::<String>()
        );
        let (directory, index) = fixture(&source);
        assert_eq!(index.toc.len(), 2102);
        assert_eq!(index.toc[0].label, "中文😀");
        assert_eq!(index.toc[0].line, 1);
        for heading in &index.toc {
            let html = read_page(directory.path(), &index, heading.page).unwrap();
            assert!(html.contains(&format!("id=\"heading-{}\"", heading.id)));
        }
    }

    #[test]
    fn raw_html_is_inert_and_search_covers_unmounted_pages_and_keeps_spaces() {
        let source =
            "# Start\n\n<script>alert(1)</script>\n\n## End\n\nUniqueWord end\n\nNoSpace\n";
        let (directory, index) = fixture(source);
        let html = (0..index.pages.len())
            .map(|page| read_page(directory.path(), &index, page).unwrap())
            .collect::<String>();
        assert!(!html.contains("<script>"));
        assert!(html.contains("&lt;script&gt;"));
        let matches = search(directory.path(), &index, "UniqueWord ").unwrap();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].line, 7);
        assert!(
            search(directory.path(), &index, "NoSpace ")
                .unwrap()
                .is_empty()
        );
        assert!(read_page(directory.path(), &index, usize::MAX).is_err());
    }

    #[test]
    fn oversized_atomic_blocks_fail_explicitly_and_do_not_publish_partial_index() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("input.md");
        fs::write(&path, format!("# Start\n\n{}", "x".repeat(MAX_HTML + 1))).unwrap();
        assert!(build_index(&path, directory.path()).is_err());
        assert!(!directory.path().join("index.json").exists());
    }

    #[test]
    fn syntax_highlighting_does_not_make_valid_code_exceed_fragment_budget() {
        let code = "fn main() { let x = 123; }\n".repeat(1800);
        let source = format!("# Large code\n\n```rust\n{code}```\n");
        let (directory, index) = fixture(&source);
        let html = (0..index.pages.len())
            .map(|page| read_page(directory.path(), &index, page).unwrap())
            .collect::<String>();
        assert!(html.contains(&code));
        assert!(html.contains("language-rust"));
        assert!(index.pages.iter().all(|page| page.bytes <= MAX_HTML));
    }

    #[test]
    fn empty_documents_are_valid_and_revision_is_content_bound() {
        let (_directory, index) = fixture("");
        assert!(index.pages.is_empty());
        assert!(index.toc.is_empty());
        assert_eq!(index.revision.len(), 64);
    }

    #[test]
    fn image_placeholders_and_read_only_tasks_keep_visible_information() {
        let (directory, index) =
            fixture("- [x] Complete\n\n![An image](https://example.com/private.png)\n");
        let html = (0..index.pages.len())
            .map(|page| read_page(directory.path(), &index, page).unwrap())
            .collect::<String>();
        assert!(html.contains("[image: An image]"));
        assert!(!html.contains("<img"));
        assert!(html.contains("disabled"));
        assert!(html.contains("checked"));
    }

    #[test]
    fn exact_row_chunks_do_not_add_an_empty_header_only_page() {
        let source = format!(
            "| A |\n|---|\n{}",
            (0..32)
                .map(|row| format!("| Row {row} |\n"))
                .collect::<String>()
        );
        let (_directory, index) = fixture(&source);
        assert_eq!(index.pages.len(), 2);
        assert!(index.pages.iter().all(|page| page.rows == 16));
    }
}
