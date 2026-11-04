//! Full-cell literal search on workers; returned offsets are original scalars.
use std::{
    ops::Range,
    sync::atomic::{AtomicBool, Ordering},
};
use studio_markdown_native::text::TextDocument;

#[derive(Default)]
pub struct Matches {
    pub ranges: Vec<Range<usize>>,
    pub limited: bool,
}
pub fn find(
    text: &str,
    query: &str,
    case: bool,
    cancelled: &AtomicBool,
) -> Result<Matches, String> {
    if query.len() > crate::source_search::MAX_QUERY {
        return Err("Source search query exceeds 4 KiB".into());
    }
    if text.len() > 8 * 1024 * 1024 {
        return Err("Cell search exceeds the 8 MiB text budget".into());
    }
    if cancelled.load(Ordering::Relaxed) {
        return Ok(Matches::default());
    }
    let document = TextDocument::new(text.into())?;
    let found = crate::source_search::find(&document, query, case, cancelled)?;
    let mut result = Matches {
        ranges: Vec::with_capacity(found.hits.len()),
        limited: found.limited,
    };
    let (mut byte, mut character) = (0, 0);
    for hit in found.hits {
        if cancelled.load(Ordering::Relaxed) {
            return Ok(Matches::default());
        }
        character += text[byte..hit.start].chars().count();
        let end = character + text[hit.start..hit.end].chars().count();
        result.ranges.push(character..end);
        byte = hit.end;
        character = end;
    }
    Ok(result)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn literal_space_unicode_case_and_crlf_keep_original_offsets() {
        let text = "first\r\nÉté 中文😀  \r\nété 中文😀 \n";
        let insensitive = find(text, "été 中文😀 ", false, &AtomicBool::new(false)).unwrap();
        assert_eq!(insensitive.ranges, [7..15, 18..26]);
        assert_eq!(
            find(text, "été 中文😀 ", true, &AtomicBool::new(false))
                .unwrap()
                .ranges,
            [18..26]
        );
        assert_eq!(
            find(text, "中文😀  ", true, &AtomicBool::new(false))
                .unwrap()
                .ranges,
            [11..16]
        );
        assert!(find(text, ".*", false, &AtomicBool::new(false))
            .unwrap()
            .ranges
            .is_empty());
    }
    #[test]
    fn late_and_boundary_matches_are_not_limited_to_the_visible_page() {
        let text = format!(
            "{}boundary needle{} needle",
            "字😀".repeat(3000),
            "x".repeat(20000)
        );
        let found = find(&text, "needle", true, &AtomicBool::new(false)).unwrap();
        assert_eq!(found.ranges, [6009..6015, 26016..26022]);
        assert_eq!(
            find(
                "a".repeat(2000).as_str(),
                "a",
                true,
                &AtomicBool::new(false)
            )
            .unwrap()
            .ranges
            .len(),
            1000
        );
        assert!(
            find(&"a".repeat(2000), "a", true, &AtomicBool::new(false))
                .unwrap()
                .limited
        );
    }
    #[test]
    fn cancellation_and_query_budget_are_shared_with_source_find() {
        assert!(find("text", "text", true, &AtomicBool::new(true))
            .unwrap()
            .ranges
            .is_empty());
        assert!(find("text", &"x".repeat(4097), true, &AtomicBool::new(false)).is_err());
    }
}
