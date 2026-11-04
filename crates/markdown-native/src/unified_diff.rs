//! Bounded old/new line origins for unified-diff presentation, not patch application.
//! Build beside the text index on a worker; rendering only performs indexed reads.
use std::sync::Arc;

pub const MAX_LINES: usize = 262_144;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Line {
    /// Zero denotes a header, note, unknown origin or a line absent on this side.
    pub old: u32,
    pub new: u32,
}
#[derive(Clone, Debug)]
pub struct Index {
    lines: Arc<[Line]>,
    pub limited: bool,
}
#[derive(Clone, Copy)]
struct Side {
    next: u32,
    remaining: u32,
}
impl Side {
    fn parse(value: &str) -> Option<Self> {
        let (start, count) = value.split_once(',').unwrap_or((value, "1"));
        if start.is_empty()
            || count.is_empty()
            || !start.bytes().all(|b| b.is_ascii_digit())
            || !count.bytes().all(|b| b.is_ascii_digit())
        {
            return None;
        }
        let next = start.parse::<u32>().ok()?;
        let remaining = count.parse::<u32>().ok()?;
        if (next == 0 && remaining != 0) || next.checked_add(remaining).is_none() {
            return None;
        }
        Some(Self { next, remaining })
    }
    fn take(&mut self) -> Option<u32> {
        if self.remaining == 0 {
            return None;
        }
        let line = self.next;
        self.next += 1;
        self.remaining -= 1;
        Some(line)
    }
}
fn hunk(line: &str) -> Option<(Side, Side)> {
    let fields = line.strip_prefix("@@ -")?;
    let (old, fields) = fields.split_once(" +")?;
    let (new, suffix) = fields.split_once(" @@")?;
    if !suffix.is_empty() && !suffix.starts_with(' ') {
        return None;
    }
    Some((Side::parse(old)?, Side::parse(new)?))
}
impl Index {
    pub fn new(source: &str) -> Self {
        let mut lines = Vec::new();
        let mut active = None::<(Side, Side)>;
        let mut limited = false;
        for raw in source.split('\n') {
            if lines.len() == MAX_LINES {
                limited = true;
                break;
            }
            let text = raw.strip_suffix('\r').unwrap_or(raw);
            let origin = if text.starts_with("@@") {
                // A malformed or combined hunk must not inherit the prior origin.
                active = hunk(text);
                Line::default()
            } else if let Some((old, new)) = &mut active {
                let result = match text.as_bytes().first() {
                    Some(b' ') if old.remaining != 0 && new.remaining != 0 => Some(Line {
                        old: old.take().unwrap(),
                        new: new.take().unwrap(),
                    }),
                    Some(b'-') => old.take().map(|old| Line { old, new: 0 }),
                    Some(b'+') => new.take().map(|new| Line { old: 0, new }),
                    Some(b'\\') => Some(Line::default()),
                    _ => None,
                };
                if result.is_none() {
                    active = None;
                }
                result.unwrap_or_default()
            } else {
                Line::default()
            };
            lines.push(origin);
        }
        Self {
            lines: lines.into(),
            limited,
        }
    }
    /// One-based physical diff line, independent of pagination or UTF-8 offsets.
    pub fn line(&self, physical: usize) -> Line {
        physical
            .checked_sub(1)
            .and_then(|index| self.lines.get(index))
            .copied()
            .unwrap_or_default()
    }
    pub fn bytes(&self) -> usize {
        self.lines.len() * std::mem::size_of::<Line>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn headers_hunks_zero_ranges_and_no_newline_notes_keep_exact_origins() {
        let source = "diff --git a/a b/a\r\n--- a/a\r\n+++ b/a\r\n@@ -2,2 +4,2 @@ 中文\r\n context\r\n--- old text\r\n\\ No newline at end of file\r\n+++ new text\r\n@@ -0,0 +1 @@\r\n+😀\r\n@@ -8 +0,0 @@\r\n-gone\r\n";
        let index = Index::new(source);
        assert_eq!(index.line(1), Line::default());
        assert_eq!(index.line(5), Line { old: 2, new: 4 });
        assert_eq!(index.line(6), Line { old: 3, new: 0 });
        assert_eq!(index.line(7), Line::default());
        assert_eq!(index.line(8), Line { old: 0, new: 5 });
        assert_eq!(index.line(10), Line { old: 0, new: 1 });
        assert_eq!(index.line(12), Line { old: 8, new: 0 });
        assert_eq!(index.line(0), Line::default());
        assert!(!index.limited);
    }
    #[test]
    fn malformed_or_exhausted_ranges_never_label_a_later_file_or_combined_diff() {
        for header in [
            "@@ -0 +1 @@",
            "@@ -1,-2 +1 @@",
            "@@ -4294967295 +1 @@",
            "@@@ -1 +1 @@@",
            "@@ -1 +1 @@bad",
        ] {
            let index = Index::new(&format!("@@ -1 +1 @@\n-old\n{header}\n+must not inherit\n"));
            assert_eq!(index.line(4), Line::default(), "{header}");
        }
        let index =
            Index::new("@@ -1 +1 @@\n-old\n+new\n+++ b/next\n--- a/next\n@@ -9 +12 @@\n next\n");
        assert_eq!(index.line(4), Line::default());
        assert_eq!(index.line(5), Line::default());
        assert_eq!(index.line(7), Line { old: 9, new: 12 });
    }
    #[test]
    fn mapping_storage_is_bounded_even_for_millions_of_empty_diff_lines() {
        let index = Index::new(&"\n".repeat(MAX_LINES + 100));
        assert!(index.limited);
        assert_eq!(index.bytes(), 2 * 1024 * 1024);
        assert_eq!(index.line(MAX_LINES + 1), Line::default());
    }
    #[test]
    fn paged_utf8_fragments_keep_the_origin_even_when_one_diff_line_crosses_pages() {
        let source = format!(
            "@@ -0,0 +1,3 @@\r\n+first\r\n+{}\r\n+last\r\n",
            "中文😀".repeat(40_000)
        );
        let index = Index::new(&source);
        let document = crate::text::TextDocument::new(source.clone()).unwrap();
        let mut offset = 0;
        let mut pages = 0;
        while offset < document.bytes() {
            let page = document.window(1, Some(offset), false).unwrap();
            assert_eq!(page.text, source[page.start..page.end]);
            let expected = match page.line {
                1 => 0,
                3 => 2,
                _ => unreachable!(),
            };
            assert_eq!(index.line(page.line).new, expected);
            assert!(page.end > offset);
            offset = page.end;
            pages += 1;
        }
        assert!(pages >= 3);
        assert_eq!(index.line(4), Line { old: 0, new: 3 });
    }
}
