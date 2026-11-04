//! Page-local edit composition without whole-document diffs.
//!
//! GTK supplies Unicode-scalar offsets. MarkNoria accepts UTF-16 offsets in an
//! LF-normalized page. Original token identities let us compose edits precisely,
//! including undo after a save, without rewriting unrelated newline bytes.
use serde::Serialize;

pub const MAX_PAGE_BYTES: usize = 512 * 1024;
const INSERTED: u32 = u32::MAX;

#[derive(Clone, Copy)]
struct Token {
    value: char,
    original: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Change {
    pub from: usize,
    pub to: usize,
    pub insert: String,
}

pub struct EditBuffer {
    baseline: String,
    tokens: Vec<Token>,
    original_units: u32,
    bytes: usize,
}

impl EditBuffer {
    pub fn new(text: &str) -> Result<Self, String> {
        if text.len() > MAX_PAGE_BYTES {
            return Err("Editor page exceeds 512 KiB".into());
        }
        if text.contains("\r\n") {
            return Err("Editor baseline must use normalized line breaks".into());
        }
        let mut units = 0;
        let mut tokens = Vec::with_capacity(text.chars().count());
        for value in text.chars() {
            tokens.push(Token {
                value,
                original: units,
            });
            units += value.len_utf16() as u32;
        }
        Ok(Self {
            baseline: text.into(),
            tokens,
            original_units: units,
            bytes: text.len(),
        })
    }

    pub fn insert(&mut self, offset: usize, text: &str) -> Result<(), String> {
        if offset > self.tokens.len() {
            return Err("Editor position is outside the current page".into());
        }
        if text.contains(['\r', '\0']) {
            return Err("Inserted text must use normalized line breaks".into());
        }
        if self.bytes.saturating_add(text.len()) > MAX_PAGE_BYTES {
            return Err("The current edit page would exceed 512 KiB".into());
        }
        self.tokens.splice(
            offset..offset,
            text.chars().map(|value| Token {
                value,
                original: INSERTED,
            }),
        );
        self.bytes += text.len();
        Ok(())
    }

    pub fn delete(&mut self, from: usize, to: usize) -> Result<(), String> {
        if from > to || to > self.tokens.len() {
            return Err("Editor deletion is outside the current page".into());
        }
        self.bytes -= self.tokens[from..to]
            .iter()
            .map(|token| token.value.len_utf8())
            .sum::<usize>();
        self.tokens.drain(from..to);
        Ok(())
    }

    pub fn text(&self) -> String {
        self.tokens.iter().map(|token| token.value).collect()
    }

    pub fn changes(&self) -> Result<Vec<Change>, String> {
        if self
            .tokens
            .iter()
            .map(|token| token.value)
            .eq(self.baseline.chars())
        {
            return Ok(Vec::new());
        }
        let mut changes = Vec::new();
        let mut cursor = 0;
        let mut inserted = String::new();
        for token in &self.tokens {
            if token.original == INSERTED {
                inserted.push(token.value);
                continue;
            }
            if token.original < cursor {
                return Err("Editor source identities are out of order".into());
            }
            if token.original > cursor || !inserted.is_empty() {
                changes.push(Change {
                    from: cursor as usize,
                    to: token.original as usize,
                    insert: std::mem::take(&mut inserted),
                });
            }
            cursor = token.original + token.value.len_utf16() as u32;
        }
        if cursor < self.original_units || !inserted.is_empty() {
            changes.push(Change {
                from: cursor as usize,
                to: self.original_units as usize,
                insert: inserted,
            });
        }
        if changes.len() > 2048 {
            return Err("This page has too many separate edits to save at once".into());
        }
        Ok(changes)
    }

    /// Call only after a successful server save; GTK retains its own undo stack.
    pub fn rebase(&mut self, saved: &str) -> Result<(), String> {
        if !self
            .tokens
            .iter()
            .map(|token| token.value)
            .eq(saved.chars())
        {
            return Err("The saved page differs from the current editor buffer".into());
        }
        *self = Self::new(saved)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scalar_edits_compose_to_normalized_utf16_positions() {
        let mut buffer = EditBuffer::new("A😀中文\nleft\nright\n").unwrap();
        buffer.delete(1, 2).unwrap();
        buffer.insert(1, "🙂!").unwrap();
        let last = buffer.text().chars().count() - 1;
        buffer.insert(last, "!").unwrap();
        let changes = buffer.changes().unwrap();
        assert_eq!(
            changes[0],
            Change {
                from: 1,
                to: 3,
                insert: "🙂!".into()
            }
        );
        assert_eq!(changes.len(), 2);
        assert_eq!(changes[1].insert, "!");
    }
    #[test]
    fn undo_and_redo_can_cross_a_successful_save() {
        let mut buffer = EditBuffer::new("before\n").unwrap();
        buffer.delete(0, 6).unwrap();
        buffer.insert(0, "after").unwrap();
        buffer.rebase("after\n").unwrap();
        assert!(buffer.changes().unwrap().is_empty());
        buffer.delete(0, 5).unwrap();
        buffer.insert(0, "before").unwrap();
        assert_eq!(
            buffer.changes().unwrap(),
            vec![Change {
                from: 0,
                to: 5,
                insert: "before".into()
            }]
        );
        buffer.delete(0, 6).unwrap();
        buffer.insert(0, "after").unwrap();
        assert!(buffer.changes().unwrap().is_empty());
    }
    #[test]
    fn rejected_edits_leave_the_page_unchanged() {
        let mut buffer = EditBuffer::new("中文").unwrap();
        assert!(buffer.insert(99, "x").is_err());
        assert!(buffer.delete(0, 99).is_err());
        assert!(buffer.insert(0, "\r\n").is_err());
        assert!(buffer.insert(0, &"x".repeat(MAX_PAGE_BYTES)).is_err());
        assert_eq!(buffer.text(), "中文");
        assert!(buffer.changes().unwrap().is_empty());
    }
}
