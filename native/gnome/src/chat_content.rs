//! Worker-produced conversation blocks; GTK receives immutable presentations.
use std::sync::Arc;
use studio_markdown_native::{display_rows, parse_text_readable, DisplayBlock};

#[derive(Debug)]
pub struct Content {
    pub rows: Vec<Arc<DisplayBlock>>,
    pub bytes: usize,
    pub source_bytes: usize,
    previous: Option<std::sync::Weak<Content>>,
    compatible: Vec<bool>,
}
impl Content {
    pub fn parse(source: &str) -> Result<Arc<Self>, String> {
        Self::parse_reusing(source, None)
    }
    pub fn parse_reusing(source: &str, previous: Option<&Arc<Self>>) -> Result<Arc<Self>, String> {
        let rows = display_rows(parse_text_readable(source)?);
        let compatible=rows.iter().enumerate().map(|(index,row)| {
            let Some(old)=previous.and_then(|p|p.rows.get(index)) else{return false;};
            matches!((&old.block.kind,&row.block.kind),(studio_markdown_native::BlockKind::Table{header:a,..},studio_markdown_native::BlockKind::Table{header:b,..}) if old.block.source.start==row.block.source.start && a==b)
        }).collect::<Vec<_>>();
        let rows = rows
            .into_iter()
            .enumerate()
            .map(|(index, row)| {
                previous
                    .and_then(|p| p.rows.get(index))
                    .filter(|old| old.as_ref() == &row)
                    .cloned()
                    .unwrap_or_else(|| Arc::new(row))
            })
            .collect::<Vec<_>>();
        let bytes = rows.capacity() * std::mem::size_of::<Arc<DisplayBlock>>()
            + rows
                .iter()
                .map(|row| {
                    std::mem::size_of::<DisplayBlock>()
                        + row.block.heap_bytes()
                        + row.marker.as_ref().map_or(0, String::capacity)
                })
                .sum::<usize>()
            + compatible.capacity();
        Ok(Arc::new(Self {
            rows,
            bytes,
            source_bytes: source.len(),
            previous: previous.map(Arc::downgrade),
            compatible,
        }))
    }
    pub fn retain_state(&self, old: &Arc<Self>, index: usize) -> bool {
        self.rows
            .get(index)
            .zip(old.rows.get(index))
            .is_some_and(|(a, b)| Arc::ptr_eq(a, b))
            || self.compatible.get(index) == Some(&true)
                && self
                    .previous
                    .as_ref()
                    .and_then(std::sync::Weak::upgrade)
                    .is_some_and(|p| Arc::ptr_eq(&p, old))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use studio_markdown_native::BlockKind;
    #[test]
    fn streaming_keeps_unchanged_embedded_image_rows() {
        let source = "![Inline](data:image/png;base64,aGVsbG8=)\n\n";
        let before = Content::parse(source).unwrap();
        let after =
            Content::parse_reusing(&format!("{source}New response text"), Some(&before)).unwrap();
        assert!(Arc::ptr_eq(&before.rows[0], &after.rows[0]));
        assert!(after.retain_state(&before, 0));
        assert!(after.bytes > source.len());
    }
    #[test]
    fn conversation_parser_keeps_tables_code_source_anchors_and_escaped_links() {
        let source = "# 标题😀\r\n\r\n[unsafe](javascript:alert) <b>literal</b>\r\n\r\n```rust\r\nfn main() {}\r\n```\r\n\r\n| Name | Value |\r\n|---|---|\r\n| 中文 | **yes** |\r\n";
        let content = Content::parse(source).unwrap();
        assert_eq!(content.rows.len(), 4);
        assert!(content.bytes > source.len());
        let BlockKind::Paragraph(text) = &content.rows[1].block.kind else {
            panic!("paragraph missing");
        };
        assert!(!text.markup.contains("href=\"javascript:"));
        assert!(!text.markup.contains("<b>literal</b>"));
        assert_eq!(content.rows[2].block.source.line, 5);
        assert!(
            matches!(&content.rows[2].block.kind, BlockKind::Code { language, text } if language == "rust" && text.contains("fn main()"))
        );
        let BlockKind::Table { header, rows, .. } = &content.rows[3].block.kind else {
            panic!("table missing");
        };
        assert_eq!(header.len(), 2);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].cells[0].content.plain, "中文");
        assert!(rows[0].cells[1].content.markup.contains("<b>yes</b>"));
    }
}
