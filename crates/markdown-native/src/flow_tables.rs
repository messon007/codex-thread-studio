//! Join indexed table continuations before toolkit anchors are created. Rows and
//! source objects move unchanged; unrelated tables never merge by appearance.
use crate::{BlockKind, DisplayBlock};
use std::collections::HashSet;

pub(super) const CELLS: usize = 65_536;
pub(super) const BYTES: usize = 4 * super::BUFFER_BYTES;
pub(super) fn coalesce(input: Vec<DisplayBlock>) -> Result<Vec<DisplayBlock>, String> {
    let mut output: Vec<DisplayBlock> = Vec::with_capacity(input.len());
    let mut seen = HashSet::new();
    let (mut cells, mut bytes) = (0usize, 0usize);
    for row in input {
        let BlockKind::Table {
            id,
            row_start,
            header,
            rows,
        } = &row.block.kind
        else {
            output.push(row);
            continue;
        };
        for cell in header.iter().chain(rows.iter().flat_map(|r| &r.cells)) {
            cells = cells.saturating_add(1);
            bytes = bytes
                .saturating_add(cell.content.plain.len())
                .saturating_add(1);
            if cells > CELLS || bytes > BYTES {
                return Err("Text-flow table context exceeds its work budget".into());
            }
        }
        let Some(id) = id else {
            output.push(row);
            continue;
        };
        row_start
            .checked_add(rows.len())
            .ok_or("Indexed table row range overflows")?;
        if let Some(previous) = output.last_mut() {
            if let BlockKind::Table {
                id: Some(old_id),
                row_start: old_start,
                header: old_header,
                rows: old_rows,
            } = &mut previous.block.kind
            {
                if old_id == id {
                    if previous.indent != row.indent
                        || previous.quote != row.quote
                        || row.marker.is_some()
                        || old_header != header
                    {
                        return Err(
                            "Indexed table continuation changes its header or structure".into()
                        );
                    }
                    // Idempotent adjacent replay is safe only with identical
                    // row values AND original source coordinates.
                    if old_start == row_start
                        && old_rows == rows
                        && previous.block.source == row.block.source
                    {
                        continue;
                    }
                    if old_start.checked_add(old_rows.len()) != Some(*row_start)
                        || old_rows.last().zip(rows.first()).is_some_and(|(a, b)| {
                            a.source.end > b.source.start || a.source.end_line > b.source.line
                        })
                        || previous.block.source.end > row.block.source.end
                    {
                        return Err(
                            "Indexed table continuation has a gap, overlap or reversed source"
                                .into(),
                        );
                    }
                    let BlockKind::Table { rows, .. } = row.block.kind else {
                        unreachable!()
                    };
                    old_rows.extend(rows);
                    previous.block.source.start =
                        previous.block.source.start.min(row.block.source.start);
                    previous.block.source.line =
                        previous.block.source.line.min(row.block.source.line);
                    previous.block.source.end = row.block.source.end;
                    previous.block.source.end_line = previous
                        .block
                        .source
                        .end_line
                        .max(row.block.source.end_line);
                    continue;
                }
            }
        }
        if !seen.insert(*id) {
            return Err("Indexed table repeats outside a continuous group".into());
        }
        output.push(row);
    }
    Ok(output)
}
