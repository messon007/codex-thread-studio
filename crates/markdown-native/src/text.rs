//! Reusable bounded text windows for native source, diff and code widgets.
use crate::SourceWindow;
use std::sync::Arc;

pub const HIGHLIGHT_BYTES: usize = 64 * 1024;
pub const HIGHLIGHT_LINE_BYTES: usize = 4 * 1024;
/// A single long wrapped paragraph can dominate toolkit layout even without
/// syntax parsing. Keep ordinary-file reading/editing windows small in bytes
/// as well as lines; offsets still refer to the complete immutable document.
pub const WINDOW_BYTES: usize = 16 * 1024;
/// Admission for toolkit regex highlighters; checking a bounded byte slice does
/// not parse a language or allocate. Long source stays readable without colors.
pub fn highlight_safe(text: &str) -> bool {
    text.len() <= HIGHLIGHT_BYTES
        && text
            .split('\n')
            .all(|line| line.len() <= HIGHLIGHT_LINE_BYTES)
}

#[derive(Clone, Debug)]
pub struct TextDocument {
    text: Arc<str>,
    lines: Arc<[usize]>,
}

impl TextDocument {
    pub fn new(text: String) -> Result<Self, String> {
        if text.len() > 8 * 1024 * 1024 {
            return Err("Text exceeds the native small-file budget".into());
        }
        let mut lines = vec![0];
        lines.extend(text.match_indices('\n').map(|(offset, _)| offset + 1));
        Ok(Self {
            text: text.into(),
            lines: lines.into(),
        })
    }
    pub fn bytes(&self) -> usize {
        self.text.len()
    }
    /// Borrow the indexed immutable snapshot on a worker without copying it.
    pub fn as_str(&self) -> &str {
        &self.text
    }
    pub fn lines(&self) -> usize {
        self.lines.len()
    }
    pub fn line_at(&self, offset: usize) -> usize {
        self.lines
            .partition_point(|line| *line <= offset.min(self.bytes()))
            .max(1)
    }
    pub fn window(
        &self,
        line: usize,
        offset: Option<usize>,
        previous: bool,
    ) -> Result<SourceWindow, String> {
        if line == 0
            || line > self.lines.len()
            || offset
                .is_some_and(|offset| offset > self.bytes() || !self.text.is_char_boundary(offset))
        {
            return Err("Source position is out of range".into());
        }
        let position = offset.unwrap_or(self.lines[line - 1]);
        let position_line = self.line_at(position);
        let mut start = if previous {
            self.lines[position_line.saturating_sub(201)].max(position.saturating_sub(WINDOW_BYTES))
        } else {
            position
        };
        while !self.text.is_char_boundary(start) {
            start += 1;
        }
        let mut end = if previous {
            position
        } else {
            self.lines
                .get(position_line + 199)
                .copied()
                .unwrap_or(self.bytes())
                .min(start.saturating_add(WINDOW_BYTES))
                .min(self.bytes())
        };
        while !self.text.is_char_boundary(end) {
            end -= 1;
        }
        // Keep one physical newline together in both reading and editing.
        // The byte budget may grow by at most two boundary bytes.
        let raw = self.text.as_bytes();
        if start > 0 && raw.get(start) == Some(&b'\n') && raw[start - 1] == b'\r' {
            start -= 1;
        }
        if end > 0 && raw[end - 1] == b'\r' && raw.get(end) == Some(&b'\n') {
            end += 1;
        }
        Ok(SourceWindow {
            projection: Vec::new(),
            references: Vec::new(),
            start,
            end,
            line: self.line_at(start),
            end_line: self.line_at(end.saturating_sub(1).max(start)),
            text: self.text[start..end].into(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn toolkit_highlight_admission_keeps_long_lines_and_pages_plain() {
        assert!(highlight_safe("let name = \"中文😀\";\r\n"));
        assert!(!highlight_safe(&"x".repeat(HIGHLIGHT_LINE_BYTES + 1)));
        assert!(!highlight_safe(&"x\n".repeat(HIGHLIGHT_BYTES)));
    }
    #[test]
    fn long_unicode_lines_and_next_previous_windows_keep_exact_bytes() {
        let original = format!("# Code\r\n{}\r\nlast\n", "中文😀".repeat(30_000));
        let document = TextDocument::new(original.clone()).unwrap();
        let first = document.window(1, None, false).unwrap();
        assert_eq!(first.text, &original[first.start..first.end]);
        assert!(first.text.len() <= WINDOW_BYTES);
        let second = document.window(1, Some(first.end), false).unwrap();
        assert_eq!(second.start, first.end);
        assert_eq!(second.line, 2);
        assert_eq!(second.text, &original[second.start..second.end]);
        let previous = document.window(1, Some(second.start), true).unwrap();
        assert_eq!(previous.end, second.start);
        assert_eq!(previous.text, &original[previous.start..previous.end]);
        assert_eq!(document.window(3, None, false).unwrap().text, "last\n");
    }
}
