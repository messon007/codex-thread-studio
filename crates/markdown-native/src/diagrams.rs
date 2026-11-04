//! Byte-preserving bare PlantUML normalization around the CommonMark parser.
//! Code spans/blocks and HTML stay protected by pulldown-cmark's own ranges.
use pulldown_cmark::{Event, Options, Parser, Tag};
use std::{borrow::Cow, ops::Range};

fn marker(line: &str, prefix: &str) -> Option<String> {
    let content = line.trim_start_matches([' ', '\t', '>']);
    let kind = content.split_whitespace().next()?.strip_prefix(prefix)?;
    matches!(
        kind,
        "uml"
            | "mindmap"
            | "wbs"
            | "gantt"
            | "json"
            | "yaml"
            | "salt"
            | "ditaa"
            | "math"
            | "latex"
            | "chen"
            | "board"
            | "ebnf"
            | "regex"
            | "wire"
    )
    .then(|| kind.into())
}

pub fn normalize(source: &str, options: Options) -> (Cow<'_, str>, Vec<Range<usize>>) {
    if !source.contains("@start") {
        return (Cow::Borrowed(source), Vec::new());
    }
    let protected = Parser::new_ext(source, options)
        .into_offset_iter()
        .filter_map(|(event, range)| {
            matches!(
                event,
                Event::Start(Tag::CodeBlock(_))
                    | Event::Code(_)
                    | Event::Html(_)
                    | Event::InlineMath(_)
                    | Event::DisplayMath(_)
            )
            .then_some(range)
        })
        .collect::<Vec<_>>();
    let mut active: Option<(usize, String)> = None;
    let mut changes = Vec::new();
    let mut ranges = Vec::new();
    let mut offset = 0;
    for line in source.split_inclusive('\n') {
        let index = protected.partition_point(|range| range.start <= offset);
        if index == 0 || !protected[index - 1].contains(&offset) {
            if active.is_none() {
                if let Some(kind) = marker(line, "@start") {
                    active = Some((offset, kind));
                }
            } else if let Some(kind) = marker(line, "@end") {
                if let Some((start, expected)) = &active {
                    if kind == *expected {
                        let first_end = source[*start..]
                            .find('\n')
                            .map_or(source.len(), |length| *start + length);
                        let first_marker = *start + source[*start..first_end].find('@').unwrap();
                        let last_marker = offset + line.find('@').unwrap();
                        changes.push((first_marker..first_end, "```uml"));
                        changes.push((
                            last_marker..offset + line.trim_end_matches(['\r', '\n']).len(),
                            "```",
                        ));
                        ranges.push(first_marker..offset + line.len());
                        active = None;
                    }
                }
            }
        }
        offset += line.len();
    }
    if ranges.is_empty() {
        return (Cow::Borrowed(source), ranges);
    }
    let mut bytes = source.as_bytes().to_vec();
    for (range, replacement) in changes {
        bytes[range.clone()].fill(b' ');
        bytes[range.start..range.start + replacement.len()].copy_from_slice(replacement.as_bytes());
    }
    (Cow::Owned(String::from_utf8(bytes).unwrap()), ranges)
}
