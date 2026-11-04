//! Page edits for ordinary text snapshots; no filesystem or widget dependency.
use crate::{edit_buffer::EditBuffer, text::TextDocument, SourceWindow};

pub const MAX_FILE_BYTES: usize = 5 * 1024 * 1024;
pub const MAX_OPERATIONS: usize = 8192;
pub const MAX_OPERATION_BYTES: usize = 2 * 1024 * 1024;
#[derive(Clone, Debug)]
pub enum Mutation {
    Insert { offset: usize, text: String },
    Delete { from: usize, to: usize },
}
#[derive(Clone, Debug)]
pub struct Page {
    pub window: SourceWindow,
    pub normalized: String,
}
pub fn page(
    document: &TextDocument,
    line: usize,
    offset: Option<usize>,
    previous: bool,
) -> Result<Page, String> {
    let mut window = document.window(line, offset, previous)?;
    let raw = document.as_str().as_bytes();
    if window.start > 0 && raw.get(window.start) == Some(&b'\n') && raw[window.start - 1] == b'\r' {
        window.start -= 1;
    }
    if window.end > 0 && raw[window.end - 1] == b'\r' && raw.get(window.end) == Some(&b'\n') {
        window.end += 1;
    }
    window.text = document.as_str()[window.start..window.end].into();
    window.line = document.line_at(window.start);
    window.end_line = document.line_at(window.end.saturating_sub(1).max(window.start));
    Ok(Page {
        normalized: window.text.replace("\r\n", "\n"),
        window,
    })
}
pub fn compose(
    document: &TextDocument,
    page: &Page,
    operations: &[Mutation],
    edited: &str,
) -> Result<(TextDocument, Page), String> {
    let window = &page.window;
    if document.as_str().get(window.start..window.end) != Some(window.text.as_str())
        || page.normalized != window.text.replace("\r\n", "\n")
    {
        return Err("The editor page does not match its snapshot".into());
    }
    if operations.len() > MAX_OPERATIONS || edited.len() > crate::edit_buffer::MAX_PAGE_BYTES {
        return Err("Save the current edit page before making more changes".into());
    }
    let mut buffer = EditBuffer::new(&page.normalized)?;
    let mut inserted = 0usize;
    for operation in operations {
        match operation {
            Mutation::Insert { offset, text } => {
                inserted = inserted.saturating_add(text.len());
                if inserted > MAX_OPERATION_BYTES {
                    return Err("Save the current edit page before making more changes".into());
                }
                buffer.insert(*offset, text)?;
            }
            Mutation::Delete { from, to } => buffer.delete(*from, *to)?,
        }
    }
    if buffer.text() != edited {
        return Err("The editor tracking differs from its text; changes remain in memory".into());
    }
    let changes = buffer.changes()?;
    // Convert normalized UTF-16 identities to original byte boundaries in one
    // pass, using the same CRLF/surrogate semantics as MarkNoria page editing.
    let mut chars = window.text.char_indices().peekable();
    let mut units = 0;
    let mut position = 0;
    let mut ranges = Vec::with_capacity(changes.len());
    for change in &changes {
        let mut offsets = [0; 2];
        for (slot, target) in offsets.iter_mut().zip([change.from, change.to]) {
            while units < target {
                let Some((offset, character)) = chars.next() else {
                    return Err("Editor position exceeds its page".into());
                };
                position = offset + character.len_utf8();
                if character == '\r' && chars.peek().is_some_and(|(_, c)| *c == '\n') {
                    position = chars.next().unwrap().0 + 1;
                }
                units += character.len_utf16();
            }
            if units != target {
                return Err("Editor position splits a Unicode character".into());
            }
            *slot = position;
        }
        ranges.push(offsets);
    }
    let newline = |text: &str| {
        text.find('\n').map(|p| {
            if p > 0 && text.as_bytes()[p - 1] == b'\r' {
                "\r\n"
            } else {
                "\n"
            }
        })
    };
    let newline = newline(&window.text)
        .or_else(|| newline(document.as_str()))
        .unwrap_or("\n");
    let mut raw = String::new();
    let mut previous = 0;
    for (change, [start, end]) in changes.iter().zip(ranges) {
        raw.push_str(&window.text[previous..start]);
        raw.push_str(&change.insert.replace('\n', newline));
        previous = end;
    }
    raw.push_str(&window.text[previous..]);
    let size = document.bytes() - (window.end - window.start) + raw.len();
    if size > MAX_FILE_BYTES {
        return Err("Edited file exceeds the 5 MiB limit".into());
    }
    let mut content = String::with_capacity(size);
    content.push_str(&document.as_str()[..window.start]);
    content.push_str(&raw);
    content.push_str(&document.as_str()[window.end..]);
    let document = TextDocument::new(content)?;
    let end = window.start + raw.len();
    let saved = Page {
        normalized: raw.replace("\r\n", "\n"),
        window: SourceWindow {
            projection: Vec::new(),
            references: Vec::new(),
            start: window.start,
            end,
            line: document.line_at(window.start),
            end_line: document.line_at(end.saturating_sub(1).max(window.start)),
            text: raw,
        },
    };
    Ok((document, saved))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn page_edits_preserve_untouched_mixed_newlines_and_unicode() {
        let source = "before\r\nA😀中文\r\nLF\nafter\r\n";
        let document = TextDocument::new(source.into()).unwrap();
        let page = page(&document, 2, None, false).unwrap();
        let operations = [
            Mutation::Delete { from: 1, to: 2 },
            Mutation::Insert {
                offset: 1,
                text: "🙂!\nnew".into(),
            },
        ];
        let (saved, rebased) =
            compose(&document, &page, &operations, "A🙂!\nnew中文\nLF\nafter\n").unwrap();
        assert_eq!(saved.as_str(), "before\r\nA🙂!\r\nnew中文\r\nLF\nafter\r\n");
        // Undo after saving is applied against the new baseline, not old offsets.
        let (undone, _) = compose(
            &saved,
            &rebased,
            &[
                Mutation::Delete { from: 1, to: 7 },
                Mutation::Insert {
                    offset: 1,
                    text: "😀".into(),
                },
            ],
            "A😀中文\nLF\nafter\n",
        )
        .unwrap();
        assert_eq!(undone.as_str(), source);
    }
    #[test]
    fn long_line_pages_do_not_split_crlf_and_only_replace_their_own_bytes() {
        let source = format!("{}\r\nlast\r\n", "x".repeat(crate::text::WINDOW_BYTES - 1));
        let document = TextDocument::new(source.clone()).unwrap();
        let page = page(&document, 1, None, false).unwrap();
        assert!(page.window.text.ends_with("\r\n"));
        let edited = format!("!{}", page.normalized);
        let (saved, _) = compose(
            &document,
            &page,
            &[Mutation::Insert {
                offset: 0,
                text: "!".into(),
            }],
            &edited,
        )
        .unwrap();
        assert_eq!(saved.as_str(), format!("!{source}"));
    }
    #[test]
    fn inconsistent_tracking_and_over_budget_edits_do_not_produce_a_snapshot() {
        let document = TextDocument::new("old\r\n".into()).unwrap();
        let mut page = page(&document, 1, None, false).unwrap();
        assert!(compose(&document, &page, &[], "new\n").is_err());
        assert!(compose(
            &document,
            &page,
            &[Mutation::Insert {
                offset: 99,
                text: "x".into()
            }],
            "old\n"
        )
        .is_err());
        page.window.start = 999;
        assert!(compose(&document, &page, &[], "old\n").is_err());
        assert_eq!(document.as_str(), "old\r\n");
    }
    #[test]
    fn file_and_operation_budgets_are_checked_before_a_save_can_be_requested() {
        let document = TextDocument::new("x".repeat(MAX_FILE_BYTES)).unwrap();
        let page = page(&document, 1, None, false).unwrap();
        let operation = Mutation::Insert {
            offset: 0,
            text: "x".into(),
        };
        assert!(compose(
            &document,
            &page,
            &[operation],
            &format!("x{}", page.normalized)
        )
        .unwrap_err()
        .contains("5 MiB"));
        let operations = vec![Mutation::Delete { from: 0, to: 0 }; MAX_OPERATIONS + 1];
        assert!(compose(&document, &page, &operations, &page.normalized).is_err());
        assert_eq!(document.bytes(), MAX_FILE_BYTES);
    }
}
