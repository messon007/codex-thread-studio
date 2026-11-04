//! Streaming coordinates for LF-normalized views of original UTF-8 text.
//! Sorted search endpoints need one pass, not a per-character mapping table.
pub struct NormalizedCursor<'a> {
    raw: &'a str,
    position: usize,
    normalized: usize,
    scalars: usize,
    next_crlf: Option<usize>,
    scanned: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Position {
    pub raw: usize,
    pub scalars: usize,
}
impl<'a> NormalizedCursor<'a> {
    pub fn new(raw: &'a str) -> Self {
        Self {
            raw,
            position: 0,
            normalized: 0,
            scalars: 0,
            next_crlf: None,
            scanned: false,
        }
    }
    pub fn advance(&mut self, target: usize) -> Result<Position, String> {
        if target < self.normalized {
            return Err("Normalized coordinates must be ordered".into());
        }
        while self.normalized < target {
            if !self.scanned {
                self.next_crlf = self.raw[self.position..]
                    .find("\r\n")
                    .map(|offset| self.position + offset);
                self.scanned = true;
            }
            if self.next_crlf == Some(self.position) {
                self.position += 2;
                self.normalized += 1;
                self.scalars += 1;
                self.scanned = false;
            } else {
                // Rust's substring search and scalar count process unchanged
                // spans in bulk. Cache the next CRLF so many hits stay linear.
                let end = self
                    .position
                    .saturating_add(target - self.normalized)
                    .min(self.next_crlf.unwrap_or(self.raw.len()));
                let span = self
                    .raw
                    .get(self.position..end)
                    .filter(|span| !span.is_empty())
                    .ok_or("Normalized coordinate exceeds its text or splits UTF-8")?;
                self.scalars += span.chars().count();
                self.normalized += span.len();
                self.position = end;
            }
        }
        if self.normalized != target {
            return Err("Normalized coordinate splits a UTF-8 character".into());
        }
        Ok(Position {
            raw: self.position,
            scalars: self.scalars,
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn normalized_byte_endpoints_keep_crlf_unicode_and_scalar_positions() {
        let mut cursor = NormalizedCursor::new("A\r\n中文😀\nZ");
        assert_eq!(cursor.advance(2).unwrap(), Position { raw: 3, scalars: 2 });
        assert_eq!(
            cursor.advance(12).unwrap(),
            Position {
                raw: 13,
                scalars: 5
            }
        );
        assert_eq!(
            cursor.advance(14).unwrap(),
            Position {
                raw: 15,
                scalars: 7
            }
        );
    }
    #[test]
    fn standalone_cr_and_repeated_boundaries_are_not_collapsed() {
        let mut cursor = NormalizedCursor::new("a\rb\r\n");
        assert_eq!(cursor.advance(2).unwrap(), Position { raw: 2, scalars: 2 });
        assert_eq!(cursor.advance(2).unwrap(), Position { raw: 2, scalars: 2 });
        assert_eq!(cursor.advance(4).unwrap(), Position { raw: 5, scalars: 4 });
    }
    #[test]
    fn invalid_and_backward_endpoints_are_rejected() {
        assert!(NormalizedCursor::new("😀").advance(1).is_err());
        assert!(NormalizedCursor::new("x").advance(2).is_err());
        let mut cursor = NormalizedCursor::new("ab");
        cursor.advance(2).unwrap();
        assert!(cursor.advance(1).is_err());
    }
    #[test]
    fn large_spans_and_adjacent_crlf_endpoints_do_not_need_a_mapping_table() {
        let size = 4 * 1024 * 1024;
        let raw = format!("{}\r\n😀", "x".repeat(size));
        let mut cursor = NormalizedCursor::new(&raw);
        assert_eq!(
            cursor.advance(size).unwrap(),
            Position {
                raw: size,
                scalars: size
            }
        );
        assert_eq!(
            cursor.advance(size + 1).unwrap(),
            Position {
                raw: size + 2,
                scalars: size + 1
            }
        );
        assert_eq!(
            cursor.advance(size + 5).unwrap(),
            Position {
                raw: size + 6,
                scalars: size + 2
            }
        );
    }
}
