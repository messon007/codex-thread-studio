//! Literal full-snapshot search, independent of GTK and bounded for worker use.
use regex::RegexBuilder;
use std::ops::Range;
use std::sync::atomic::{AtomicBool, Ordering};
use studio_markdown_native::text::TextDocument;
use studio_markdown_native::{text_coordinates::NormalizedCursor, text_edit::Page};

pub const MAX_HITS: usize = 1000;
pub const MAX_QUERY: usize = 4096;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hit {
    pub start: usize,
    pub end: usize,
    pub line: usize,
}
#[derive(Clone, Debug, Default)]
pub struct Matches {
    pub hits: Vec<Hit>,
    pub limited: bool,
}
#[derive(Clone, Debug)]
pub struct EditTarget {
    /// Unicode-scalar selection in the current unsaved GTK page.
    pub local: Option<Range<usize>>,
    /// Original immutable snapshot bytes, authoritative outside a dirty page.
    pub original: Option<Range<usize>>,
}
#[derive(Clone, Debug)]
pub struct EditMatches {
    /// Offsets refer to the whole LF-normalized in-memory document.
    pub matches: Matches,
    pub targets: Vec<EditTarget>,
}
pub fn find_edit(
    document: &TextDocument,
    page: &Page,
    edited: &str,
    query: &str,
    case: bool,
    cancelled: &AtomicBool,
) -> Result<EditMatches, String> {
    let window = &page.window;
    if document.as_str().get(window.start..window.end) != Some(window.text.as_str())
        || page.normalized != window.text.replace("\r\n", "\n")
    {
        return Err("The editor page does not match its snapshot".into());
    }
    if edited.len() > studio_markdown_native::edit_buffer::MAX_PAGE_BYTES || edited.contains('\0') {
        return Err("Editor search exceeds its page budget".into());
    }
    if cancelled.load(Ordering::Relaxed) {
        return Ok(EditMatches {
            matches: Matches::default(),
            targets: vec![],
        });
    }
    let prefix = &document.as_str()[..window.start];
    let suffix = &document.as_str()[window.end..];
    let mut text = prefix.replace("\r\n", "\n");
    let start = text.len();
    text.push_str(edited);
    let end = text.len();
    text.push_str(&suffix.replace("\r\n", "\n"));
    let working = TextDocument::new(text)?;
    let matches = find(&working, query, case, cancelled)?;
    let clean = edited == page.normalized;
    let mut whole = NormalizedCursor::new(document.as_str());
    let mut before = NormalizedCursor::new(prefix);
    let mut after = NormalizedCursor::new(suffix);
    let mut local = NormalizedCursor::new(edited);
    let mut targets = Vec::with_capacity(matches.hits.len());
    for hit in &matches.hits {
        if cancelled.load(Ordering::Relaxed) {
            return Ok(EditMatches {
                matches: Matches::default(),
                targets: vec![],
            });
        }
        let selection = if hit.start >= start && hit.end <= end {
            Some(local.advance(hit.start - start)?.scalars..local.advance(hit.end - start)?.scalars)
        } else {
            None
        };
        let original = if clean {
            Some(whole.advance(hit.start)?.raw..whole.advance(hit.end)?.raw)
        } else if hit.end <= start {
            Some(before.advance(hit.start)?.raw..before.advance(hit.end)?.raw)
        } else if hit.start >= end {
            Some(
                window.end + after.advance(hit.start - end)?.raw
                    ..window.end + after.advance(hit.end - end)?.raw,
            )
        } else {
            None
        };
        targets.push(EditTarget {
            local: selection,
            original,
        });
    }
    Ok(EditMatches { matches, targets })
}
pub fn find(
    document: &TextDocument,
    query: &str,
    case_sensitive: bool,
    cancelled: &AtomicBool,
) -> Result<Matches, String> {
    if query.len() > MAX_QUERY {
        return Err("Source search query exceeds 4 KiB".into());
    }
    let mut output = Matches::default();
    if query.is_empty() || cancelled.load(Ordering::Relaxed) {
        return Ok(output);
    }
    let pattern = RegexBuilder::new(&regex::escape(query))
        .case_insensitive(!case_sensitive)
        .size_limit(2 * 1024 * 1024)
        .dfa_size_limit(1024 * 1024)
        .build()
        .map_err(|e| e.to_string())?;
    for found in pattern.find_iter(document.as_str()) {
        if cancelled.load(Ordering::Relaxed) {
            return Ok(Matches::default());
        }
        if output.hits.len() == MAX_HITS {
            output.limited = true;
            break;
        }
        output.hits.push(Hit {
            start: found.start(),
            end: found.end(),
            line: document.line_at(found.start()),
        });
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn matches(text: &str, query: &str, case: bool) -> Matches {
        find(
            &TextDocument::new(text.into()).unwrap(),
            query,
            case,
            &AtomicBool::new(false),
        )
        .unwrap()
    }
    #[test]
    fn literal_unicode_whitespace_and_crlf_have_exact_offsets() {
        let text = "heading\r\n[中文😀].  \r\n[中文😀].\r\n";
        let found = matches(text, "[中文😀].  ", true);
        assert_eq!(
            found.hits,
            vec![Hit {
                start: 9,
                end: 24,
                line: 2
            }]
        );
        assert_eq!(&text[found.hits[0].start..found.hits[0].end], "[中文😀].  ");
        assert_eq!(matches(text, "  ", false).hits.len(), 1);
        assert_eq!(matches(text, ".*", false).hits.len(), 0);
        assert_eq!(matches("A\r\nB", "\r\nB", false).hits[0].line, 1);
    }
    #[test]
    fn unicode_case_and_nonoverlapping_occurrences_follow_source_search() {
        assert_eq!(matches("Été été ÉTÉ", "été", false).hits.len(), 3);
        assert_eq!(matches("Été été ÉTÉ", "été", true).hits.len(), 1);
        assert_eq!(matches("aaaa", "aa", true).hits.len(), 2);
    }
    #[test]
    fn long_lines_and_late_pages_use_snapshot_bytes() {
        let text = format!(
            "{}{}needle 中文😀  \r\n",
            "line\r\n".repeat(600),
            "x".repeat(300_000)
        );
        let found = matches(&text, "needle 中文😀  ", true);
        assert_eq!(found.hits[0].line, 601);
        assert_eq!(found.hits[0].start, 303_600);
        assert_eq!(
            &text[found.hits[0].start..found.hits[0].end],
            "needle 中文😀  "
        );
    }
    #[test]
    fn result_budget_cancellation_empty_and_query_budget_are_explicit() {
        let output = matches(&"a ".repeat(MAX_HITS + 1), "a", true);
        assert_eq!(output.hits.len(), MAX_HITS);
        assert!(output.limited);
        assert!(!matches(&"a ".repeat(MAX_HITS), "a", true).limited);
        assert!(matches("abc", "", true).hits.is_empty());
        let document = TextDocument::new("abc".into()).unwrap();
        assert!(find(&document, "a", true, &AtomicBool::new(true))
            .unwrap()
            .hits
            .is_empty());
        assert!(find(
            &document,
            &"a".repeat(MAX_QUERY + 1),
            true,
            &AtomicBool::new(false)
        )
        .is_err());
    }
    #[test]
    fn editor_search_includes_dirty_text_and_maps_untouched_snapshot_ranges() {
        let raw = "before needle\r\nold 😀\r\nafter needle  \r\n";
        let document = TextDocument::new(raw.into()).unwrap();
        let start = raw.find("old").unwrap();
        let end = raw.find("after").unwrap();
        let page = Page {
            window: studio_markdown_native::SourceWindow {
                projection: Vec::new(),
                references: Vec::new(),
                start,
                end,
                line: 2,
                end_line: 2,
                text: raw[start..end].into(),
            },
            normalized: "old 😀\n".into(),
        };
        let found = find_edit(
            &document,
            &page,
            "new 😀 needle  \n",
            "needle  ",
            true,
            &AtomicBool::new(false),
        )
        .unwrap();
        assert_eq!(found.matches.hits.len(), 2);
        assert_eq!(found.targets[0].local, Some(6..14));
        assert_eq!(found.targets[0].original, None);
        let range = found.targets[1].original.clone().unwrap();
        assert_eq!(&raw[range], "needle  ");
        assert_eq!(found.matches.hits[1].line, 3);
        assert_eq!(document.as_str(), raw);
    }
    #[test]
    fn clean_editor_search_normalizes_crlf_and_preserves_raw_boundaries() {
        let document = TextDocument::new("Été\r\n😀 été\r\nend\n".into()).unwrap();
        let page = studio_markdown_native::text_edit::page(&document, 1, None, false).unwrap();
        let found = find_edit(
            &document,
            &page,
            &page.normalized,
            "été",
            false,
            &AtomicBool::new(false),
        )
        .unwrap();
        assert_eq!(found.matches.hits.len(), 2);
        assert_eq!(found.targets[1].local, Some(6..9));
        assert_eq!(
            &document.as_str()[found.targets[1].original.clone().unwrap()],
            "été"
        );
        let found = find_edit(
            &document,
            &page,
            &page.normalized,
            "\n😀",
            true,
            &AtomicBool::new(false),
        )
        .unwrap();
        assert_eq!(
            &document.as_str()[found.targets[0].original.clone().unwrap()],
            "\r\n😀"
        );
    }
    #[test]
    fn matches_across_a_page_boundary_require_navigation_and_cancel_stays_bounded() {
        let raw = format!(
            "{}needle-tail",
            "x".repeat(studio_markdown_native::text::WINDOW_BYTES - 3)
        );
        let document = TextDocument::new(raw).unwrap();
        let page = studio_markdown_native::text_edit::page(&document, 1, None, false).unwrap();
        let found = find_edit(
            &document,
            &page,
            &page.normalized,
            "needle",
            true,
            &AtomicBool::new(false),
        )
        .unwrap();
        assert_eq!(found.targets[0].local, None);
        assert!(found.targets[0].original.is_some());
        let edited = format!("!{}", page.normalized);
        let found = find_edit(
            &document,
            &page,
            &edited,
            "needle",
            true,
            &AtomicBool::new(false),
        )
        .unwrap();
        assert_eq!(found.targets[0].original, None);
        assert!(
            find_edit(&document, &page, &edited, "x", true, &AtomicBool::new(true))
                .unwrap()
                .matches
                .hits
                .is_empty()
        );
    }
}
