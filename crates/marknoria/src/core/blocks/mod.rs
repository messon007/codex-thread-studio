//! Structured page projection for native preview rendering.
//!
//! Native widgets consume bounded projections of the shared parser's cached
//! output, preserving reference definitions and nested container context.
//! Diagram fences resolve to their indexed ids (the backend Chromium renderer
//! owns actual diagram output). Tables are bounded per projection.

use serde::Serialize;
use std::{io, path::Path};

/// Each projected grid is bounded; nested tables continue in additional grids.
const MAX_TABLE_ROWS: usize = 50;
mod projection;

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Span {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
    pub code: bool,
    pub underline: bool,
    pub link: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Block {
    Heading {
        level: u8,
        line: usize,
        runs: Vec<Span>,
    },
    Paragraph {
        quote: bool,
        runs: Vec<Span>,
    },
    Code {
        language: Option<String>,
        text: String,
        /// An internal slice of a text document, not a Markdown code fence.
        document: bool,
        #[serde(skip_serializing_if = "Vec::is_empty")]
        syntax: Vec<crate::core::highlight::NativeRun>,
    },
    List {
        ordered: bool,
        start: u64,
        /// Flattened items; nested lists appear as indented items.
        items: Vec<Vec<Span>>,
    },
    Table {
        id: Option<usize>,
        #[serde(rename = "rowStart")]
        row_start: usize,
        #[serde(rename = "totalRows")]
        total_rows: usize,
        #[serde(rename = "pageSize")]
        page_size: usize,
        columns: Vec<usize>,
        #[serde(rename = "columnNames")]
        column_names: Vec<String>,
        #[serde(rename = "rowLines")]
        row_lines: Vec<usize>,
        header: Vec<Vec<Span>>,
        rows: Vec<Vec<Vec<Span>>>,
        truncated: bool,
    },
    Diagram {
        id: Option<usize>,
        language: String,
    },
    Image {id:usize,alt:String,width:u32,height:u32,frames:Vec<crate::core::documents::AssetFrame>,plays:u32},
    Details {label:String,text:String},
    Rule,
}

/// Projects one page of an indexed document. `page` is 0-based like
/// `read_page`.
pub fn read_blocks(directory: &Path, index: &crate::core::Index, page: usize) -> io::Result<Vec<Block>> {
    let metadata = index
        .pages
        .get(page)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Page out of range"))?;
    if index.text_pages {
        let mut blocks = Vec::new();
        if page == 0 {
            if let Some(note) = crate::core::documents::preview_note(directory)? {
                blocks.push(Block::Paragraph { quote: false, runs: vec![Span { text: note, ..Default::default() }] });
            }
        }
        blocks.push(Block::Code {
            language: Some(crate::core::highlight::language_for_name(&index.name)),
            text: crate::core::read_cached_page(directory, index, page)?,
            document: true,
            syntax: vec![],
        });
        return Ok(blocks);
    }
    if metadata.table.is_some() {
        let id = metadata.table.unwrap();
        let table = &index.tables[id];
        let mut blocks = projection::project(
            &crate::core::read_page(directory, index, page)?,
            index,
            metadata.line,
        )?;
        for block in &mut blocks {
            if let Block::Table {
                id: table_id,
                row_start,
                total_rows,
                page_size,
                columns,
                column_names,
                ..
            } = block
            {
                *table_id = Some(id);
                *row_start = metadata.row_start;
                *total_rows = table.rows;
                *page_size = metadata.rows;
                *columns = (0..table.columns.len()).collect();
                *column_names = table.columns.clone();
            }
        }
        return Ok(blocks);
    }
    let mut blocks = projection::project(
        &crate::core::read_page(directory, index, page)?,
        index,
        metadata.line,
    )?;
    for block in &mut blocks { if let Block::Image{id,frames,width,height,..}=block {let path=crate::core::documents::asset(directory,index,*id,0)?;if index.assets[*id].pdf_page.is_some(){let(w,h)=image::image_dimensions(path).map_err(io::Error::other)?;*width=w;*height=h;}for frame in frames{frame.file=directory.join(&frame.file).to_string_lossy().into_owned();}}}
    Ok(blocks)
}

/// Native equivalent of the shared selected-column table projection.
pub fn table_row_for_line(
    directory: &Path,
    index: &crate::core::Index,
    page: usize,
    line: usize,
) -> io::Result<usize> {
    let metadata = index
        .pages
        .get(page)
        .ok_or_else(|| io::Error::other("Page out of range"))?;
    for block in read_blocks(directory, index, page)? {
        if let Block::Table { row_lines, .. } = block {
            return Ok(metadata.row_start
                + row_lines
                    .partition_point(|start| *start <= line)
                    .saturating_sub(1));
        }
    }
    Ok(metadata.row_start)
}

/// Native equivalent of the shared selected-column table projection.
/// This single-table API does not return surrounding generated Details notices.
/// Hosts needing omission guidance should retain the notices from `read_blocks`
/// and provide bounded `source::read_source` navigation for original row text.
pub fn read_table_blocks(
    directory: &Path,
    index: &crate::core::Index,
    id: usize,
    row: usize,
    columns: &[usize],
) -> io::Result<(Block, usize)> {
    let window = crate::core::read_table(directory, index, id, row, columns)?;
    let mut blocks = projection::project(
        &window.html,
        index,
        index.pages[index.tables[id].first_page].line,
    )?;
    let mut block = blocks
        .drain(..)
        .find(|block| matches!(block, Block::Table { .. }))
        .ok_or_else(|| io::Error::other("Shared table projection is missing"))?;
    if let Block::Table {
        id: table_id,
        row_start,
        total_rows,
        page_size,
        columns: selected,
        column_names,
        truncated,
        ..
    } = &mut block
    {
        *table_id = Some(id);
        *row_start = window.row_start;
        *total_rows = window.total_rows;
        *page_size = window.page_size;
        *selected = columns.to_vec();
        *column_names = index.tables[id].columns.clone();
        *truncated = window.row_end < window.total_rows;
    }
    Ok((block, window.row_start))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::build_index;
    use tempfile::TempDir;

    fn page_blocks(source: &str) -> Vec<Block> {
        let directory = TempDir::new().unwrap();
        let path = directory.path().join("fixture.md");
        std::fs::write(&path, source).unwrap();
        let index = build_index(&path, directory.path()).unwrap();
        read_blocks(directory.path(), &index, 0).unwrap()
    }

    #[test]
    fn shortened_table_guidance_is_separate_from_literal_cells() {
        let root = TempDir::new().unwrap();
        let path = root.path().join("large.md");
        std::fs::write(&path, format!("| Key | Value |\n| --- | --- |\n| original-key | {}TAIL |\n", "x".repeat(140_000))).unwrap();
        let index = build_index(&path, root.path()).unwrap();
        let check = |blocks: &[Block]| {
            let notice = blocks.iter().position(|block| matches!(block, Block::Details { label, text }
                if label == "Shortened table preview" && text.contains("source line 3") && text.contains("Source view"))).unwrap();
            let table = blocks.iter().position(|block| matches!(block, Block::Table { .. })).unwrap();
            assert!(notice < table);
            if let Block::Table { header, rows, row_lines, .. } = &blocks[table] {
                assert_eq!(row_lines, &[3]);
                assert_eq!(rows[0][0].iter().map(|span| span.text.as_str()).collect::<String>(), "original-key");
                for cell in header.iter().chain(rows.iter().flatten()) {
                    let text = cell.iter().map(|span| span.text.as_str()).collect::<String>();
                    assert!(!text.contains("Shortened table preview") && !text.contains("Source view"));
                }
            }
        };
        check(&read_blocks(root.path(), &index, 0).unwrap());
        // The omitted column is not mounted, but the row-level marker retains
        // the warning for hosts projecting the selected-column HTML to blocks.
        let window = crate::core::read_table(root.path(), &index, 0, 0, &[0]).unwrap();
        assert!(!window.html.contains("data-cell-preview"));
        assert!(window.html.contains("data-row-preview"));
        check(&projection::project(&window.html, &index, 1).unwrap());
        assert!(!page_blocks("| Key |\n| --- |\n| ordinary |\n").iter().any(|block| matches!(block, Block::Details { .. })));
    }

    #[test]
    fn projects_headings_paragraphs_and_inline_styles() {
        let blocks =
            page_blocks("# Title\n\nPlain **bold** *italic* `code` [link](https://example.com)\n");
        assert!(matches!(
            blocks.first(),
            Some(Block::Heading {
                level: 1,
                line: 1,
                ..
            })
        ));
        let Some(Block::Paragraph { quote: false, runs }) = blocks.get(1) else {
            panic!("expected paragraph, got {blocks:?}");
        };
        assert!(
            runs.iter()
                .any(|span| span.bold && span.text.contains("bold"))
        );
        assert!(runs.iter().any(|span| span.code && span.text == "code"));
        assert!(
            runs.iter()
                .any(|span| span.link.as_deref() == Some("https://example.com"))
        );
    }

    #[test]
    fn projects_lists_quotes_code_and_rules() {
        let blocks = page_blocks("- one\n- two\n\n> quoted\n\n```rust\nfn main() {}\n```\n\n---\n");
        assert!(matches!(
            blocks.first(),
            Some(Block::List { ordered: false, items, .. }) if items.len() == 2
        ));
        assert!(matches!(
            blocks.get(1),
            Some(Block::Paragraph { quote: true, .. })
        ));
        assert!(matches!(
            blocks.get(2),
            Some(Block::Code { language: Some(lang), text, .. }) if lang == "rust" && text.contains("fn main")
        ));
        assert!(matches!(blocks.get(3), Some(Block::Rule)));
    }

    #[test]
    fn projects_tables_across_paged_rows() {
        // The indexer itself pages tables, so a small table stays whole.
        let source = format!(
            "| a | b |\n| - | - |\n{}\n",
            (0..8)
                .map(|i| format!("| {i} | x |"))
                .collect::<Vec<_>>()
                .join("\n")
        );
        let blocks = page_blocks(&source);
        let Some(Block::Table {
            header,
            rows,
            truncated,
            ..
        }) = blocks.first()
        else {
            panic!("expected table, got {blocks:?}");
        };
        assert_eq!(header.len(), 2);
        assert_eq!(rows.len(), 8);
        assert!(!truncated);
    }

    #[test]
    fn bounds_table_rows_in_one_projection() {
        let source = format!(
            "| a | b |\n| - | - |\n{}\n",
            (0..80)
                .map(|i| format!("| {i} | x |"))
                .collect::<Vec<_>>()
                .join("\n")
        );
        let directory = crate::runtime::cache::temporary_directory("mn-projection-test-").unwrap();
        let path = directory.path().join("table.md");
        std::fs::write(&path, &source).unwrap();
        let index = build_index(&path, directory.path()).unwrap();
        let (block, _) = read_table_blocks(directory.path(), &index, 0, 0, &[0, 1]).unwrap();
        let blocks = vec![block];
        let Some(Block::Table {
            header,
            rows,
            truncated,
            ..
        }) = blocks.first()
        else {
            panic!("expected table, got {blocks:?}");
        };
        assert_eq!(header.len(), 2);
        assert_eq!(rows.len(), MAX_TABLE_ROWS);
        assert!(truncated);
    }

    #[test]
    fn projects_diagram_fences_to_indexed_ids() {
        let blocks = page_blocks("```mermaid\ngraph TD\nA-->B\n```\n");
        assert!(matches!(
            blocks.first(),
            Some(Block::Diagram { id: Some(0), language }) if language == "mermaid"
        ));
    }

    #[test]
    fn preserves_nested_loose_list_code_and_following_items_in_source_order() {
        let source = "7. parent\n\n   paragraph\n\n   - child\n\n     ```rust\n     let child = 1;\n     ```\n\n     after code\n\n   parent continuation\n\n8. sibling\n";
        let blocks = page_blocks(source);
        let text: String = blocks
            .iter()
            .map(|block| match block {
                Block::List { items, .. } => items
                    .iter()
                    .flatten()
                    .map(|run| run.text.as_str())
                    .collect(),
                Block::Paragraph { runs, .. } => runs.iter().map(|run| run.text.as_str()).collect(),
                Block::Code { text, .. } => text.clone(),
                _ => String::new(),
            })
            .collect();
        let mut offset = 0;
        for expected in [
            "parent",
            "paragraph",
            "child",
            "let child = 1;",
            "after code",
            "parent continuation",
            "sibling",
        ] {
            offset += text[offset..]
                .find(expected)
                .unwrap_or_else(|| panic!("Missing {expected}: {blocks:?}"))
                + expected.len();
        }
        assert!(matches!(blocks.first(), Some(Block::List { start: 7, .. })));
        assert!(blocks.iter().any(|block| matches!(block, Block::List { start: 8, items, .. } if items.iter().flatten().any(|span| span.text.contains("sibling")))));
        assert_eq!(
            blocks
                .iter()
                .filter_map(|block| match block {
                    Block::List { items, .. } => Some(items.len()),
                    _ => None,
                })
                .sum::<usize>(),
            3
        );
    }

    #[test]
    fn preserves_breaks_and_image_alt_text_without_loading_resources() {
        let blocks =
            page_blocks("first\nsecond<br>third ![description](https://example.com/image.png)\n");
        let Block::Paragraph { runs, .. } = &blocks[0] else {
            panic!("{blocks:?}")
        };
        let text: String = runs.iter().map(|span| span.text.as_str()).collect();
        assert_eq!(text, "first\nsecond\nthird [image: description]");
    }

    #[test]
    fn rejects_pages_out_of_range() {
        let directory = TempDir::new().unwrap();
        let path = directory.path().join("empty.md");
        std::fs::write(&path, "# hi\n").unwrap();
        let index = build_index(&path, directory.path()).unwrap();
        assert!(read_blocks(directory.path(), &index, index.pages.len()).is_err());
    }

    #[test]
    fn shared_projection_preserves_document_wide_reference_definitions() {
        let directory = crate::runtime::cache::temporary_directory("mn-projection-test-").unwrap();
        let path = directory.path().join("references.md");
        let source = format!(
            "# First\n\n[linked label][remote]\n\n{}\n# End\n\n[remote]: https://example.com/reference\n",
            "paragraph\n\n".repeat(4000)
        );
        std::fs::write(&path, source).unwrap();
        let index = build_index(&path, directory.path()).unwrap();
        let blocks = read_blocks(directory.path(), &index, 0).unwrap();
        assert!(blocks.iter().any(|block| matches!(block,Block::Paragraph { runs,.. } if runs.iter().any(|run| run.text=="linked label" && run.link.as_deref()==Some("https://example.com/reference")))));
    }

    #[test]
    fn shared_projection_keeps_diagrams_inside_lists_and_quotes() {
        let blocks = page_blocks(
            "- nested diagram\n\n  ```mermaid\n  graph TD\n  A-->B\n  ```\n\n> ```d2\n> a -> b\n> ```\n",
        );
        let diagrams: Vec<_> = blocks
            .iter()
            .filter_map(|block| match block {
                Block::Diagram { id, language } => Some((*id, language.as_str())),
                _ => None,
            })
            .collect();
        assert_eq!(diagrams, vec![(Some(0), "mermaid"), (Some(1), "d2")]);
    }

    #[test]
    fn nested_table_headers_are_bounded_without_dropping_columns() {
        let header = (0..300)
            .map(|column| format!("column-{column}"))
            .collect::<Vec<_>>()
            .join(" | ");
        let separator = vec!["---"; 300].join(" | ");
        let row = (0..300)
            .map(|column| format!("cell-{column}"))
            .collect::<Vec<_>>()
            .join(" | ");
        let blocks = page_blocks(&format!(
            "- nested\n\n  | {header} |\n  | {separator} |\n  | {row} |\n"
        ));
        let tables: Vec<_> = blocks
            .iter()
            .filter_map(|block| match block {
                Block::Table { header, rows, .. } => Some((header, rows)),
                _ => None,
            })
            .collect();
        assert_eq!(tables.len(), 2);
        assert_eq!(
            tables.iter().map(|(header, _)| header.len()).sum::<usize>(),
            300
        );
        assert!(tables.iter().all(|(header, _)| header.len() <= 256));
        assert_eq!(
            tables
                .iter()
                .flat_map(|(_, rows)| rows.iter())
                .map(|row| row.len())
                .sum::<usize>(),
            300
        );
        assert!(serde_json::to_string(&blocks).unwrap().contains("cell-299"));
    }

    #[test]
    fn shared_projection_keeps_nested_headings_code_and_every_nested_table_row() {
        let directory = crate::runtime::cache::temporary_directory("mn-projection-test-").unwrap();
        let path = directory.path().join("nested.md");
        let source = format!(
            "- parent text\n\n  ### Nested heading\n\n  ```rust\n  fn nested() {{}}\n  ```\n\n  | Name | Value |\n  |---|---|\n{}\n  parent continuation\n\n- sibling text\n",
            (0..149)
                .map(|row| format!("  | nested-{row} | **value-{row}** |\n"))
                .collect::<String>()
        );
        std::fs::write(&path, source).unwrap();
        let index = build_index(&path, directory.path()).unwrap();
        let blocks: Vec<_> = (0..index.pages.len())
            .flat_map(|page| read_blocks(directory.path(), &index, page).unwrap())
            .collect();
        assert!(blocks.iter().any(|block| matches!(
            block,
            Block::Heading {
                level: 3,
                line: 3,
                ..
            }
        )));
        assert!(
            blocks.iter().any(
                |block| matches!(block,Block::Code { text,.. } if text.contains("fn nested()"))
            )
        );
        let tables: Vec<_> = blocks
            .iter()
            .filter_map(|block| match block {
                Block::Table {
                    rows, row_lines, ..
                } => Some((rows, row_lines)),
                _ => None,
            })
            .collect();
        assert_eq!(
            tables.iter().map(|(rows, _)| rows.len()).sum::<usize>(),
            149
        );
        assert_eq!(tables[0].1[0], 11);
        assert_eq!(*tables.last().unwrap().1.last().unwrap(), 159);
        let projected = serde_json::to_string(&blocks).unwrap();
        for expected in [
            "parent text",
            "parent continuation",
            "sibling text",
            "nested-148",
            "value-148",
        ] {
            assert!(projected.contains(expected), "Missing {expected}");
        }
    }
}
