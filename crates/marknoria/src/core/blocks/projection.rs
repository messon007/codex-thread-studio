//! Bounded native projection of the shared parser's cached, inert HTML.
//! Parsing source slices loses reference definitions and container context.
//! Reusing parser output keeps all frontends on the same Markdown semantics.
use super::{Block, MAX_TABLE_ROWS, Span};
use kuchikiki::{NodeRef, traits::TendrilSink};

fn name(node: &NodeRef) -> &str {
    node.as_element()
        .map_or("", |element| element.name.local.as_ref())
}
fn attribute(node: &NodeRef, key: &str) -> Option<String> {
    node.as_element()?
        .attributes
        .borrow()
        .get(key)
        .map(str::to_owned)
}
fn number(node: &NodeRef, key: &str) -> Option<usize> {
    attribute(node, key)?.parse().ok()
}
fn push(runs: &mut Vec<Span>, span: Span) {
    if span.text.is_empty() {
        return;
    }
    if let Some(last) = runs.last_mut() {
        if last.bold == span.bold
            && last.italic == span.italic
            && last.code == span.code
            && last.underline == span.underline
            && last.link == span.link
        {
            last.text.push_str(&span.text);
            return;
        }
    }
    runs.push(span);
}
fn inline(node: &NodeRef, mut style: Span, output: &mut Vec<Span>, depth: usize) {
    if let Some(text) = node.as_text() {
        let text = text.borrow();
        // HTML serialization adds a formatting newline after <br>. Native
        // text widgets already received that explicit line break.
        style.text = if output.last().is_some_and(|span| span.text.ends_with('\n')) {
            text.strip_prefix("\r\n")
                .or_else(|| text.strip_prefix('\n'))
                .unwrap_or(&text)
                .to_string()
        } else {
            text.to_string()
        };
        push(output, style);
        return;
    }
    if depth >= 64 {
        style.text = node.text_contents();
        push(output, style);
        return;
    }
    match name(node) {
        "strong" | "b" => style.bold = true,
        "em" | "i" => style.italic = true,
        "code" => style.code = true,
        "del" | "s" => style.underline = true,
        "a" => {
            style.link = attribute(node, "href")
                .filter(|url| url.starts_with("https://") || url.starts_with("http://"))
        }
        "br" => {
            style.text = "\n".into();
            push(output, style);
            return;
        }
        "input" => {
            style.text = if attribute(node, "checked").is_some() {
                "[x] "
            } else {
                "[ ] "
            }
            .into();
            push(output, style);
            return;
        }
        "script" | "style" | "iframe" | "object" => return,
        _ => {}
    }
    for child in node.children() {
        inline(&child, style.clone(), output, depth + 1);
    }
}
fn runs(node: &NodeRef) -> Vec<Span> {
    let mut output = vec![];
    inline(node, Span::default(), &mut output, 0);
    output
}

pub(super) fn project(
    html: &str,
    index: &crate::core::Index,
    base_line: usize,
) -> std::io::Result<Vec<Block>> {
    if html.len() > crate::core::MAX_HTML {
        return Err(std::io::Error::other(
            "Native projection exceeds the shared page budget",
        ));
    }
    let document = kuchikiki::parse_html().one(html);
    let mut output = vec![];
    walk(&document, index, base_line, false, 0, &mut output);
    Ok(output)
}
fn walk(
    node: &NodeRef,
    index: &crate::core::Index,
    base_line: usize,
    quote: bool,
    depth: usize,
    output: &mut Vec<Block>,
) {
    if depth >= 64 {
        output.push(Block::Paragraph {
            quote,
            runs: vec![Span {
                text: node.text_contents(),
                ..Span::default()
            }],
        });
        return;
    }
    match name(node) {
        "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
            let line = attribute(node, "id")
                .and_then(|id| {
                    id.strip_prefix("heading-")
                        .and_then(|id| id.parse::<usize>().ok())
                })
                .and_then(|id| index.toc.get(id))
                .map_or(base_line, |heading| heading.line);
            output.push(Block::Heading {
                level: name(node).as_bytes()[1] - b'0',
                line,
                runs: runs(node),
            });
        }
        "p" => output.push(Block::Paragraph {
            quote,
            runs: runs(node),
        }),
        "pre" => {
            let code = node.children().find(|child| name(child) == "code");
            let language = code
                .as_ref()
                .and_then(|node| attribute(node, "data-language"))
                .or_else(|| {
                    code.as_ref()
                        .and_then(|node| attribute(node, "class"))
                        .and_then(|class| {
                            class.split_whitespace().find_map(|class| {
                                class.strip_prefix("language-").map(str::to_owned)
                            })
                        })
                });
            output.push(Block::Code {
                language,
                text: node.text_contents(),
                document: attribute(node, "data-document-code").is_some(),
                syntax: vec![],
            });
        }
        "figure" if attribute(node,"data-asset-id").is_some()=>{if let Some(id)=number(node,"data-asset-id"){if let Some(asset)=index.assets.get(id){output.push(Block::Image{id,alt:asset.alt.clone(),width:asset.width,height:asset.height,frames:asset.frames.clone(),plays:asset.plays});}}},
        "details"=>{let label=node.children().find(|c|name(c)=="summary").map(|c|c.text_contents()).unwrap_or_else(||"Details".into());let text=node.children().filter(|c|name(c)!="summary").map(|c|c.text_contents()).collect::<String>();output.push(Block::Details{label,text});},
        "figure" if attribute(node, "data-diagram-id").is_some() => output.push(Block::Diagram {
            id: number(node, "data-diagram-id"),
            language: attribute(node, "data-diagram-kind").unwrap_or_default(),
        }),
        "ul" | "ol" => list(node, index, base_line, quote, depth + 1, output),
        "table" => table(node, output),
        "hr" => output.push(Block::Rule),
        "blockquote" => {
            for child in node.children() {
                walk(&child, index, base_line, true, depth + 1, output);
            }
        }
        "script" | "style" | "iframe" | "object" | "head" => {}
        "" if node.as_text().is_some() => {
            if !node.text_contents().trim().is_empty() {
                output.push(Block::Paragraph {
                    quote,
                    runs: runs(node),
                });
            }
        }
        _ => {
            for child in node.children() {
                walk(&child, index, base_line, quote, depth + 1, output);
            }
        }
    }
}
fn list(
    node: &NodeRef,
    index: &crate::core::Index,
    base_line: usize,
    quote: bool,
    depth: usize,
    output: &mut Vec<Block>,
) {
    let ordered = name(node) == "ol";
    let mut number = number(node, "start").unwrap_or(1) as u64;
    let indent = node
        .ancestors()
        .filter(|ancestor| matches!(name(ancestor), "ul" | "ol"))
        .count()
        .saturating_sub(1);
    for item in node.children().filter(|child| name(child) == "li") {
        let mut current = vec![Span {
            text: "  ".repeat(indent),
            ..Span::default()
        }];
        let mut emitted = false;
        for child in item.children() {
            if matches!(
                name(&child),
                "ul" | "ol"
                    | "pre"
                    | "figure"
                    | "table"
                    | "blockquote"
                    | "h1"
                    | "h2"
                    | "h3"
                    | "h4"
                    | "h5"
                    | "h6"
                    | "hr"
            ) {
                emit_item(&mut current, &mut emitted, ordered, number, quote, output);
                walk(&child, index, base_line, quote, depth + 1, output);
            } else {
                if name(&child) == "p" && current.iter().any(|run| !run.text.trim().is_empty()) {
                    push(
                        &mut current,
                        Span {
                            text: "\n".into(),
                            ..Span::default()
                        },
                    );
                }
                inline(&child, Span::default(), &mut current, 0);
            }
        }
        emit_item(&mut current, &mut emitted, ordered, number, quote, output);
        number = number.saturating_add(1);
    }
}
fn emit_item(
    current: &mut Vec<Span>,
    emitted: &mut bool,
    ordered: bool,
    number: u64,
    quote: bool,
    output: &mut Vec<Block>,
) {
    if current.iter().all(|span| span.text.trim().is_empty()) {
        current.clear();
        return;
    }
    let runs = std::mem::take(current);
    if *emitted {
        output.push(Block::Paragraph { quote, runs });
        return;
    }
    if let Some(Block::List {
        ordered: previous,
        start,
        items,
    }) = output.last_mut()
    {
        if *previous == ordered && *start + items.len() as u64 == number {
            items.push(runs);
            *emitted = true;
            return;
        }
    }
    output.push(Block::List {
        ordered,
        start: number,
        items: vec![runs],
    });
    *emitted = true;
}
fn table(node: &NodeRef, output: &mut Vec<Block>) {
    let mut header = vec![];
    let mut rows = vec![];
    let mut lines = vec![];
    for row in node.descendants().filter(|node| name(node) == "tr") {
        let cells: Vec<_> = row
            .children()
            .filter(|node| matches!(name(node), "th" | "td"))
            .collect();
        if cells.first().is_some_and(|cell| name(cell) == "th") {
            header = cells.iter().map(runs).collect();
        } else {
            lines.push(number(&row, "data-source-line").unwrap_or(0));
            rows.push(cells.iter().map(runs).collect::<Vec<_>>());
        }
    }
    let count = rows.len();
    let names: Vec<String> = header
        .iter()
        .map(|cell| cell.iter().map(|span| span.text.as_str()).collect())
        .collect();
    if rows.is_empty() {
        rows.push(vec![]);
    }
    // Nested tables have no paged inventory entry. Keep every row/column in
    // successive bounded grids, including headers wider than the indexed-table
    // limit. Native table headers must not create thousands of widgets at once.
    for column_start in (0..header.len().max(1)).step_by(256) {
        let column_end = (column_start + 256).min(header.len());
        let page_size = MAX_TABLE_ROWS
            .min(512 / (column_end - column_start).max(1))
            .max(1);
        for (page, chunk) in rows.chunks(page_size).enumerate() {
            let start = page * page_size;
            output.push(Block::Table {
                id: None,
                row_start: start,
                total_rows: count,
                page_size,
                columns: (column_start..column_end).collect(),
                column_names: names[column_start..column_end].to_vec(),
                row_lines: lines
                    .get(start..(start + chunk.len()).min(lines.len()))
                    .unwrap_or_default()
                    .to_vec(),
                header: header[column_start..column_end].to_vec(),
                rows: if count == 0 {
                    vec![]
                } else {
                    chunk
                        .iter()
                        .map(|row| {
                            row.get(column_start..column_end)
                                .unwrap_or_default()
                                .to_vec()
                        })
                        .collect()
                },
                truncated: start + chunk.len() < count,
            });
        }
    }
}
