//! Bounded, presentation-independent full-table text for native selection.
use super::*;
use studio_markdown_native::{flow::TableText, BlockKind, Fragment, SourceRange};
#[derive(Default)]
pub(crate) struct Budget {
    pub(super) pages: usize,
    pub(super) bytes: usize,
}
pub(crate) fn read(
    snapshot: &marknoria::server::ReaderSnapshot,
    id: usize,
    limit: usize,
    budget: &mut Budget,
) -> Result<Option<TableText>, String> {
    let table = snapshot.index.tables.get(id).ok_or("Table out of range")?;
    // Every body row contributes at least a newline, even with empty cells.
    if table.rows > limit {
        return Ok(None);
    }
    let mut text = String::new();
    let source = visit(snapshot, id, budget, |part| {
        if part.len() > limit.saturating_sub(text.len()) {
            return false;
        }
        text.push_str(part);
        true
    })?;
    Ok(source.map(|source| TableText { id, text, source }))
}
pub(crate) fn prefix(
    snapshot: &marknoria::server::ReaderSnapshot,
    id: usize,
    characters: usize,
    budget: &mut Budget,
) -> Result<studio_markdown_native::flow::TablePrefix, String> {
    if characters > studio_markdown_native::flow::BUFFER_BYTES / 4 {
        return Err("Table prefix exceeds its character budget".into());
    }
    let mut remaining = characters;
    let mut text = String::new();
    let source = visit(snapshot, id, budget, |part| {
        for character in part.chars() {
            if remaining == 0 {
                return false;
            }
            text.push(character);
            remaining -= 1;
        }
        true
    })?;
    Ok(studio_markdown_native::flow::TablePrefix {
        id,
        text,
        complete: source.is_some(),
    })
}
/// A single bounded parser traversal serves exact Copy and prefix consumers.
/// Returning false stops before reading another source page; no partial exact
/// selection is ever presented as complete.
fn visit(
    snapshot: &marknoria::server::ReaderSnapshot,
    id: usize,
    budget: &mut Budget,
    mut push: impl FnMut(&str) -> bool,
) -> Result<Option<SourceRange>, String> {
    let table = snapshot.index.tables.get(id).ok_or("Table out of range")?;
    let mut headers = HashMap::new();
    let mut expected_header = None;
    let mut source: Option<SourceRange> = None;
    let mut count = 0usize;
    for page in table.first_page..table.end_page {
        budget.pages += 1;
        if budget.pages > 4096 {
            return Err("Table selection exceeds its source-page work budget".into());
        }
        let fragment: Fragment = serde_json::from_value(fragment(snapshot, page, &mut headers)?)
            .map_err(|e| e.to_string())?;
        budget.bytes = budget
            .bytes
            .saturating_add(fragment.source.text.len())
            .saturating_add(fragment.header.as_ref().map_or(0, |h| h.text.len()));
        if budget.bytes > 16 * 1024 * 1024 {
            return Err("Table selection exceeds its source-text work budget".into());
        }
        let blocks = studio_markdown_native::parse(&fragment)?;
        if blocks.len() != 1 {
            return Err("Indexed table selection contains another block".into());
        }
        for block in blocks {
            let BlockKind::Table {
                id: Some(found),
                row_start,
                header,
                rows,
            } = block.kind
            else {
                return Err("Indexed table selection could not be parsed".into());
            };
            if found != id || row_start != count {
                return Err("Indexed table selection row range changed".into());
            }
            if let Some(old) = &expected_header {
                if old != &header {
                    return Err("Indexed table selection header changed".into());
                }
            } else {
                for (index, cell) in header.iter().enumerate() {
                    if (index > 0 && !push("\t")) || !push(&cell.content.plain) {
                        return Ok(None);
                    }
                }
                expected_header = Some(header);
            }
            for row in rows {
                if !push("\n") {
                    return Ok(None);
                }
                for (index, cell) in row.cells.iter().enumerate() {
                    if (index > 0 && !push("\t")) || !push(&cell.content.plain) {
                        return Ok(None);
                    }
                }
                count += 1;
            }
            if let Some(range) = &mut source {
                range.start = range.start.min(block.source.start);
                range.end = range.end.max(block.source.end);
                range.line = range.line.min(block.source.line);
                range.end_line = range.end_line.max(block.source.end_line);
            } else {
                source = Some(block.source);
            }
        }
    }
    if count != table.rows {
        return Err("Indexed table selection is missing rows".into());
    }
    Ok(Some(source.ok_or("Indexed table selection has no source")?))
}
