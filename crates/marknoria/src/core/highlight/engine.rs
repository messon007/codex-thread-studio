//! Native Tree-sitter events over pinned, statically linked grammar/query crates.
//! No files, environment access, downloads or WASM runtime are involved.
use super::{Style, StyledRange};
use std::{cell::RefCell, sync::atomic::Ordering};
use tree_sitter_highlight::{HighlightEvent, Highlighter};
#[path = "budget.rs"]
mod budget;
use marknoria_syntax::{self as grammars, CAPTURES};
const MAX_EVENTS: usize = 262144;
thread_local! {static PARSER:RefCell<Highlighter>=RefCell::new(Highlighter::new());}

fn push(out: &mut Vec<StyledRange>, start: usize, end: usize, style: Style) {
    if start == end {
        return;
    }
    if let Some(last) = out
        .last_mut()
        .filter(|last| last.range.end == start && last.style == style)
    {
        last.range.end = end
    } else {
        out.push(StyledRange {
            range: start..end,
            style,
        })
    }
}
pub(super) fn ranges(language: &str, source: &str) -> Option<Vec<StyledRange>> {
    if language.len() > 64 || source.len() > 128 * 1024 {
        return None;
    }
    let name = language.to_ascii_lowercase();
    let name = grammars::alias(&name);
    let config = grammars::configuration(name)?;
    // The bundled INI grammar expects LF. Map every parsed byte boundary back
    // to the immutable original; output, selections and CRLF remain exact.
    if name == "ini" && source.contains("\r\n") {
        let mut bytes = Vec::new();
        let mut map = vec![0];
        let raw = source.as_bytes();
        let mut at = 0;
        while at < raw.len() {
            if raw[at] == b'\r' && raw.get(at + 1) == Some(&b'\n') {
                at += 1
            }
            bytes.push(raw[at]);
            at += 1;
            map.push(at)
        }
        let lf = String::from_utf8(bytes).ok()?;
        let mut out = ranges(name, &lf)?;
        for span in &mut out {
            span.range = map[span.range.start]..map[span.range.end]
        }
        return Some(out);
    }
    let lease = budget::Lease::new();
    PARSER.with(|parser| {
        let mut parser = parser.borrow_mut();
        let mut out = Vec::new();
        let mut stack = vec![Style::default()];
        let mut consumed = 0;
        let mut stopped = false;
        {
            let events = parser
                .highlight(config, source.as_bytes(), None, Some(&lease.flag), |name| {
                    grammars::configuration(&name.to_ascii_lowercase())
                })
                .ok()?;
            for (event_count, event) in events.enumerate() {
                if event_count >= MAX_EVENTS
                    || stack.len() > 128
                    || (event_count % 256 == 0 && lease.expired())
                {
                    stopped = true;
                    break;
                }
                match event {
                    Ok(HighlightEvent::HighlightStart(style)) => {
                        stack.push(Style::capture(CAPTURES[style.0]))
                    }
                    Ok(HighlightEvent::HighlightEnd) => {
                        if stack.len() > 1 {
                            stack.pop();
                        }
                    }
                    Ok(HighlightEvent::Source { start, end }) => {
                        if start != consumed
                            || end > source.len()
                            || !source.is_char_boundary(start)
                            || !source.is_char_boundary(end)
                        {
                            return None;
                        }
                        push(&mut out, start, end, *stack.last().unwrap());
                        consumed = end;
                    }
                    Err(_) => {
                        stopped = true;
                        break;
                    }
                }
            }
        }
        if stopped || lease.flag.load(Ordering::Relaxed) != 0 {
            parser.parser().reset()
        }
        push(&mut out, consumed, source.len(), Style::default());
        Some(out)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_bundled_grammars_compile() {
        for language in grammars::LANGUAGES {
            assert!(
                grammars::configuration(language).is_some(),
                "Invalid bundled grammar {language}"
            )
        }
    }
    #[test]
    fn deep_and_dense_input_preserves_complete_bounded_text() {
        for (language, text) in [
            (
                "json",
                format!("{}{{\"value\":42}}{}", "[".repeat(3000), "]".repeat(3000)),
            ),
            (
                "html",
                format!("{}text{}", "<div>".repeat(1000), "</div>".repeat(1000)),
            ),
        ] {
            let spans = ranges(language, &text).unwrap();
            assert!(spans.len() <= MAX_EVENTS);
            assert_eq!(
                spans
                    .iter()
                    .map(|s| &text[s.range.clone()])
                    .collect::<String>(),
                text
            );
        }
    }
}
