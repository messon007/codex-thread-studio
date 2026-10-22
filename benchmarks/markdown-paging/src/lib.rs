use comrak::{
    Arena, Options,
    nodes::{AstNode, NodeValue},
};
use serde::{Deserialize, Serialize};
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
};

pub fn options() -> Options<'static> {
    let mut options = Options::default();
    options.extension.table = true;
    options.extension.strikethrough = true;
    options.extension.tasklist = true;
    options.extension.autolink = true;
    options.render.hardbreaks = true;
    options.render.r#unsafe = true;
    options.render.sourcepos = true;
    options
}

pub struct Lines {
    starts: Vec<(usize, usize)>,
    bytes: usize,
}

impl Lines {
    pub fn new(source: &str) -> Self {
        let mut starts = vec![(0, 0)];
        let mut utf16 = 0;
        let mut previous_cr = false;
        for (byte, character) in source.char_indices() {
            utf16 += character.len_utf16();
            if character == '\n' && previous_cr {
                *starts.last_mut().unwrap() = (byte + 1, utf16);
            } else if matches!(character, '\n' | '\r') {
                starts.push((byte + 1, utf16));
            }
            previous_cr = character == '\r';
        }
        Self {
            starts,
            bytes: source.len(),
        }
    }

    fn start(&self, line: usize) -> (usize, usize) {
        self.starts
            .get(line.saturating_sub(1))
            .copied()
            .unwrap_or((self.bytes, 0))
    }

    fn span(&self, start: usize, end: usize) -> usize {
        self.starts
            .get(end)
            .map_or(self.bytes, |entry| entry.0)
            .saturating_sub(self.start(start).0)
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageMeta {
    pub id: usize,
    pub start_line: usize,
    pub end_line: usize,
    pub source_bytes: usize,
    pub table_rows: usize,
    pub html_offset: u64,
    pub html_bytes: usize,
}

pub struct Part<'a> {
    node: &'a AstNode<'a>,
    rows: Option<Vec<&'a AstNode<'a>>>,
    pub start_line: usize,
    pub end_line: usize,
    bytes: usize,
    table_rows: usize,
}

pub struct Page<'a> {
    pub meta: PageMeta,
    parts: Vec<Part<'a>>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Heading {
    pub id: usize,
    pub parent: Option<usize>,
    pub level: u8,
    pub label: String,
    pub line: usize,
    pub byte_offset: usize,
    pub utf16_offset: usize,
    pub page: usize,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageContent {
    #[serde(flatten)]
    pub meta: PageMeta,
    pub html: String,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Window {
    pub read_only: bool,
    pub target_page: usize,
    pub previous_page: Option<usize>,
    pub next_page: Option<usize>,
    pub total_pages: usize,
    pub toc: Vec<Heading>,
    pub pages: Vec<PageContent>,
}

pub fn window_range(target: usize, count: usize, context: usize) -> std::ops::Range<usize> {
    target.saturating_sub(context)..target.saturating_add(context).saturating_add(1).min(count)
}

pub fn paginate<'a>(
    root: &'a AstNode<'a>,
    lines: &Lines,
    target_bytes: usize,
    row_limit: usize,
) -> Vec<Page<'a>> {
    assert!(target_bytes > 0 && row_limit > 0);
    let mut parts = Vec::new();
    for node in root.children() {
        let position = node.data().sourcepos;
        let bytes = lines.span(position.start.line, position.end.line);
        let table = matches!(node.data().value, NodeValue::Table(_));
        let rows: Vec<_> = if table {
            node.children().collect()
        } else {
            Vec::new()
        };
        if table && (bytes > target_bytes || rows.len().saturating_sub(1) > row_limit) {
            let header = rows[0];
            let header_bytes = lines.span(
                header.data().sourcepos.start.line,
                header.data().sourcepos.end.line + 1,
            );
            let mut start = 1;
            while start < rows.len() {
                let mut end = start;
                let mut size = header_bytes;
                while end < rows.len() && end - start < row_limit {
                    let position = rows[end].data().sourcepos;
                    let row_bytes = lines.span(position.start.line, position.end.line);
                    if end > start && size + row_bytes > target_bytes {
                        break;
                    }
                    size += row_bytes;
                    end += 1;
                }
                parts.push(Part {
                    node,
                    rows: Some(rows[start..end].to_vec()),
                    start_line: if start == 1 {
                        position.start.line
                    } else {
                        rows[start].data().sourcepos.start.line
                    },
                    end_line: rows[end - 1].data().sourcepos.end.line,
                    bytes: size,
                    table_rows: end - start,
                });
                start = end;
            }
            if rows.len() > 1 {
                continue;
            }
        }
        parts.push(Part {
            node,
            rows: None,
            start_line: position.start.line,
            end_line: position.end.line,
            bytes,
            table_rows: rows.len().saturating_sub(1),
        });
    }
    let mut pages: Vec<Page<'a>> = Vec::new();
    for part in parts {
        let needs_page = pages.last().is_none_or(|page| {
            !page.parts.is_empty()
                && (page.meta.source_bytes + part.bytes > target_bytes
                    || page.meta.table_rows + part.table_rows > row_limit * 2)
        });
        if needs_page {
            pages.push(Page {
                meta: PageMeta {
                    id: pages.len(),
                    start_line: part.start_line,
                    end_line: part.end_line,
                    source_bytes: 0,
                    table_rows: 0,
                    html_offset: 0,
                    html_bytes: 0,
                },
                parts: Vec::new(),
            });
        }
        let page = pages.last_mut().unwrap();
        page.meta.end_line = part.end_line;
        page.meta.source_bytes += part.bytes;
        page.meta.table_rows += part.table_rows;
        page.parts.push(part);
    }
    pages
}

pub fn outline<'a>(root: &'a AstNode<'a>, lines: &Lines, pages: &[Page<'_>]) -> Vec<Heading> {
    let mut headings: Vec<Heading> = Vec::new();
    let mut parents: Vec<(u8, usize)> = Vec::new();
    for node in root.descendants() {
        let data = node.data();
        let NodeValue::Heading(heading) = &data.value else {
            continue;
        };
        let mut label = String::new();
        for child in node.descendants().skip(1) {
            match &child.data().value {
                NodeValue::Text(text) => label.push_str(text),
                NodeValue::HtmlInline(text) => label.push_str(text),
                NodeValue::Code(code) => label.push_str(&code.literal),
                NodeValue::SoftBreak | NodeValue::LineBreak => label.push(' '),
                _ => {}
            }
        }
        let line = data.sourcepos.start.line;
        let (byte_offset, utf16_offset) = lines.start(line);
        let page = pages
            .partition_point(|page| page.meta.start_line <= line)
            .saturating_sub(1);
        while parents
            .last()
            .is_some_and(|parent| parent.0 >= heading.level)
        {
            parents.pop();
        }
        let id = headings.len();
        let parent = parents.last().map(|entry| entry.1);
        parents.push((heading.level, id));
        headings.push(Heading {
            id,
            parent,
            level: heading.level,
            label: label.chars().take(512).collect(),
            line,
            byte_offset,
            utf16_offset,
            page,
        });
    }
    headings
}

pub fn render_page<'a>(page: &Page<'a>, scratch: &'a AstNode<'a>, options: &Options) -> String {
    let mut html = String::new();
    for part in &page.parts {
        if let Some(rows) = &part.rows {
            let header = part.node.first_child().unwrap();
            let following = rows.last().unwrap().next_sibling();
            scratch.data_mut().value = part.node.data().value.clone();
            scratch.data_mut().sourcepos = part.node.data().sourcepos;
            scratch.append(header);
            for row in rows {
                scratch.append(row);
            }
            let result = comrak::format_html(scratch, options, &mut html);
            part.node.prepend(header);
            for row in rows {
                if let Some(following) = following {
                    following.insert_before(row);
                } else {
                    part.node.append(row);
                }
            }
            result.unwrap();
        } else {
            comrak::format_html(part.node, options, &mut html).unwrap();
        }
    }
    html
}

pub fn render_window<'a>(
    pages: &[Page<'a>],
    scratch: &'a AstNode<'a>,
    options: &Options,
    toc: &[Heading],
    target: usize,
    context: usize,
    include_toc: bool,
) -> Window {
    let range = window_range(target, pages.len(), context);
    let start = range.start;
    let end = range.end;
    Window {
        read_only: true,
        target_page: target,
        previous_page: start.checked_sub(1),
        next_page: (end < pages.len()).then_some(end),
        total_pages: pages.len(),
        toc: if include_toc {
            toc.to_vec()
        } else {
            Vec::new()
        },
        pages: range
            .map(|index| {
                let html = render_page(&pages[index], scratch, options);
                let mut meta = pages[index].meta.clone();
                meta.html_bytes = html.len();
                PageContent { meta, html }
            })
            .collect(),
    }
}

pub fn cached_window(
    pages: &[PageMeta],
    file: &mut File,
    toc: &[Heading],
    target: usize,
    context: usize,
    include_toc: bool,
) -> Window {
    let range = window_range(target, pages.len(), context);
    let start = range.start;
    let end = range.end;
    Window {
        read_only: true,
        target_page: target,
        previous_page: start.checked_sub(1),
        next_page: (end < pages.len()).then_some(end),
        total_pages: pages.len(),
        toc: if include_toc {
            toc.to_vec()
        } else {
            Vec::new()
        },
        pages: range
            .map(|index| {
                let meta = pages[index].clone();
                file.seek(SeekFrom::Start(meta.html_offset)).unwrap();
                let mut bytes = vec![0; meta.html_bytes];
                file.read_exact(&mut bytes).unwrap();
                PageContent {
                    meta,
                    html: String::from_utf8(bytes).unwrap(),
                }
            })
            .collect(),
    }
}

pub fn scratch<'a>(arena: &'a Arena<'a>) -> &'a AstNode<'a> {
    arena.alloc(NodeValue::Document.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cached_windows_match_live_rendering() {
        use std::io::Write;
        let source = (0..20)
            .map(|index| format!("# Section {index}\n\nContent {index}.\n\n"))
            .collect::<String>();
        let arena = Arena::new();
        let options = options();
        let root = comrak::parse_document(&arena, &source, &options);
        let lines = Lines::new(&source);
        let mut pages = paginate(root, &lines, 50, 5);
        let toc = outline(root, &lines, &pages);
        let scratch = scratch(&arena);
        let path = std::env::temp_dir().join(format!(
            "studio-paging-{}-{}.html",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut file = File::options()
            .create_new(true)
            .read(true)
            .write(true)
            .open(&path)
            .unwrap();
        let mut offset = 0;
        for page in &mut pages {
            let html = render_page(page, scratch, &options);
            page.meta.html_offset = offset;
            page.meta.html_bytes = html.len();
            file.write_all(html.as_bytes()).unwrap();
            offset += html.len() as u64;
        }
        let metadata: Vec<_> = pages.iter().map(|page| page.meta.clone()).collect();
        for target in [0, pages.len() / 2, pages.len() - 1] {
            let live = render_window(&pages, scratch, &options, &toc, target, 2, target == 0);
            let cached = cached_window(&metadata, &mut file, &toc, target, 2, target == 0);
            assert_eq!(
                serde_json::to_value(live).unwrap(),
                serde_json::to_value(cached).unwrap()
            );
        }
        drop(file);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn outline_recognizes_setext_and_nested_headings_but_not_fenced_text() {
        let source = "Title\n=====\n\n```text\n# Not a heading\n```\n\n> ## Quoted\n\n- Item\n\n  ### Nested\n\n## Tail\n";
        let arena = Arena::new();
        let options = options();
        let root = comrak::parse_document(&arena, source, &options);
        let lines = Lines::new(source);
        let pages = paginate(root, &lines, 30, 5);
        let toc = outline(root, &lines, &pages);
        assert_eq!(
            toc.iter()
                .map(|entry| entry.label.as_str())
                .collect::<Vec<_>>(),
            vec!["Title", "Quoted", "Nested", "Tail"]
        );
        for heading in &toc {
            assert!(pages[heading.page].meta.start_line <= heading.line);
            assert!(pages[heading.page].meta.end_line >= heading.line);
        }
    }

    #[test]
    fn context_windows_are_clipped_and_have_continuation_pointers() {
        let source = (0..20)
            .map(|index| format!("# Section {index}\n\nParagraph with words {index}.\n\n"))
            .collect::<String>();
        let arena = Arena::new();
        let options = options();
        let root = comrak::parse_document(&arena, &source, &options);
        let pages = paginate(root, &Lines::new(&source), 80, 5);
        let toc = outline(root, &Lines::new(&source), &pages);
        let first = render_window(&pages, scratch(&arena), &options, &toc, 0, 2, true);
        assert_eq!(first.pages.len(), 3);
        assert_eq!(first.previous_page, None);
        assert_eq!(first.next_page, Some(3));
        assert_eq!(first.toc.len(), 20);
        let middle = render_window(&pages, scratch(&arena), &options, &toc, 5, 2, false);
        assert_eq!(
            middle
                .pages
                .iter()
                .map(|page| page.meta.id)
                .collect::<Vec<_>>(),
            vec![3, 4, 5, 6, 7]
        );
        let last = render_window(
            &pages,
            scratch(&arena),
            &options,
            &toc,
            pages.len() - 1,
            2,
            false,
        );
        assert_eq!(last.pages.len(), 3);
        assert_eq!(last.next_page, None);
    }

    #[test]
    fn split_tables_keep_headers_alignment_rows_and_original_ast() {
        let source = format!(
            "# Table\n\n| A | B |\n| :- | -: |\n{}\n",
            (0..25)
                .map(|index| format!("| row-{index} | value-{index} |\n"))
                .collect::<String>()
        );
        let arena = Arena::new();
        let options = options();
        let root = comrak::parse_document(&arena, &source, &options);
        let mut before = String::new();
        comrak::format_html(root, &options, &mut before).unwrap();
        let pages = paginate(root, &Lines::new(&source), 100, 3);
        let scratch = scratch(&arena);
        let output = pages
            .iter()
            .map(|page| render_page(page, scratch, &options))
            .collect::<String>();
        for index in 0..25 {
            assert_eq!(output.matches(&format!(">row-{index}</td>")).count(), 1);
        }
        assert_eq!(
            output.matches("<thead>").count(),
            output.matches("<tbody>").count()
        );
        assert_eq!(
            output.matches("<thead>").count(),
            output.matches("</tbody>").count()
        );
        assert!(output.contains("align=\"right\""));
        let mut after = String::new();
        comrak::format_html(root, &options, &mut after).unwrap();
        assert_eq!(before, after);
    }

    #[test]
    fn references_are_resolved_against_the_whole_document() {
        let source = "# First\n\n[Cross-page reference][target]\n\n# Second\n\ntext\n\n[target]: https://example.com\n";
        let arena = Arena::new();
        let options = options();
        let root = comrak::parse_document(&arena, source, &options);
        let pages = paginate(root, &Lines::new(source), 30, 5);
        let output = pages
            .iter()
            .map(|page| render_page(page, scratch(&arena), &options))
            .collect::<String>();
        assert!(output.contains("href=\"https://example.com\""));
    }

    #[test]
    fn outline_is_not_capped_at_two_thousand_and_offsets_keep_utf16() {
        let source = format!("# 中文 😀\r\n\r\n{}", "## Repeated\r\n\r\n".repeat(2_500));
        let arena = Arena::new();
        let root = comrak::parse_document(&arena, &source, &options());
        let lines = Lines::new(&source);
        let pages = paginate(root, &lines, 1024, 200);
        let toc = outline(root, &lines, &pages);
        assert_eq!(toc.len(), 2_501);
        assert_eq!(
            toc[1].utf16_offset,
            source[..source.find("##").unwrap()].encode_utf16().count()
        );
        assert_eq!(toc[1].parent, Some(0));
        assert!(toc.last().unwrap().page > 0);
    }

    #[test]
    fn atomic_blocks_are_never_cut_in_the_middle() {
        let source = format!(
            "# Before\n\n```text\n{}\n```\n\n# After\n",
            "long line\n".repeat(100)
        );
        let arena = Arena::new();
        let options = options();
        let root = comrak::parse_document(&arena, &source, &options);
        let pages = paginate(root, &Lines::new(&source), 100, 200);
        assert!(pages.iter().any(|page| page.meta.source_bytes > 100));
        let output = pages
            .iter()
            .map(|page| render_page(page, scratch(&arena), &options))
            .collect::<String>();
        assert_eq!(output.matches("<pre").count(), 1);
        assert!(output.contains("language-text"));
    }
}
