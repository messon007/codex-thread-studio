//! Small presentation intent; a fresh immutable snapshot validates its context.
use studio_markdown_native::text::TextDocument;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Source,
    Preview,
    Edit,
}
#[derive(Clone, Debug)]
pub struct Context {
    pub mode: Mode,
    pub line: usize,
    pub offset: Option<usize>,
    pub revision: String,
}
impl Context {
    pub fn position(&self, document: &TextDocument, revision: &str) -> (usize, Option<usize>) {
        // A terminal LF indexes an empty final line. Reading a shortened file
        // should reveal its last real content, rather than an empty EOF page.
        let last = document.line_at(document.bytes().saturating_sub(1));
        let line = self.line.max(1).min(last);
        let offset = self.offset.filter(|offset| {
            !self.revision.is_empty()
                && self.revision == revision
                && (*offset < document.bytes() || document.bytes() == 0)
                && document.as_str().is_char_boundary(*offset)
        });
        (line, offset)
    }
}
#[derive(Clone, Debug)]
pub struct Target {
    pub root: String,
    pub path: String,
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn changed_or_shortened_files_do_not_reuse_stale_bytes_or_empty_eof() {
        let context = Context {
            mode: Mode::Edit,
            line: 201,
            offset: Some(16384),
            revision: "old".into(),
        };
        let doc = TextDocument::new("中文😀\r\n".into()).unwrap();
        assert_eq!(context.position(&doc, "new"), (1, None));
        let doc = TextDocument::new(String::new()).unwrap();
        assert_eq!(context.position(&doc, "new"), (1, None));
    }
    #[test]
    fn unchanged_long_lines_retain_their_page_but_invalid_offsets_are_rejected() {
        let doc = TextDocument::new("中文😀".repeat(10000)).unwrap();
        let mut context = Context {
            mode: Mode::Source,
            line: 1,
            offset: Some(16380),
            revision: "same".into(),
        };
        assert_eq!(context.position(&doc, "same"), (1, Some(16380)));
        context.offset = Some(1);
        assert_eq!(context.position(&doc, "same"), (1, None));
        context.offset = Some(doc.bytes());
        assert_eq!(context.position(&doc, "same"), (1, None));
        context.offset = Some(0);
        context.revision.clear();
        assert_eq!(context.position(&doc, ""), (1, None));
    }
}
