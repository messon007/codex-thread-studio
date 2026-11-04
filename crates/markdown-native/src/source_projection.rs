//! Compact transport for data-image spans, with original source coordinates.
use crate::SourceWindow;
use pulldown_cmark::{Event, Options, Tag, TagEnd};
use serde::{Deserialize, Serialize};
use std::ops::Range;

pub const MAX_SOURCE: usize = 128 * 1024 * 1024;
pub const MAX_IMAGES: usize = 128;
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Replacement {
    pub start: usize,
    pub end: usize,
    pub source_start: usize,
    pub source_end: usize,
    pub line: usize,
    pub end_line: usize,
    pub page: usize,
    pub image: usize,
    pub newlines: usize,
}
fn options() -> Options {
    Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_MATH
}
fn eligible(uri: &str) -> bool {
    uri.len() > 8192
        && uri
            .get(..11)
            .is_some_and(|s| s.eq_ignore_ascii_case("data:image/"))
}
fn alt_markup<'a>(
    events: &mut impl Iterator<Item = (Event<'a>, Range<usize>)>,
    original: &str,
    span: &Range<usize>,
) -> Result<String, String> {
    if original.get(span.start..span.start + 2) != Some("![") {
        return Err("Unsupported image source spelling".into());
    }
    let start = span.start + 2;
    let mut end = start;
    let mut depth = 1usize;
    for (event, range) in events {
        match event {
            Event::Start(Tag::Image { .. }) => depth += 1,
            Event::End(TagEnd::Image) => {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            _ => {}
        }
        end = end.max(range.end);
    }
    if end > span.end || end.saturating_sub(start) > 128 * 1024 {
        return Err("Image caption exceeds the projection budget".into());
    }
    original
        .get(start..end)
        .map(str::to_owned)
        .ok_or_else(|| "Invalid image caption source range".into())
}
pub fn project(source: SourceWindow, page: usize) -> Result<SourceWindow, String> {
    project_with_references(source, page, None)
}
pub fn project_with_references(
    mut source: SourceWindow,
    page: usize,
    lookup: Option<&crate::references::Lookup<'_>>,
) -> Result<SourceWindow, String> {
    if source.text.len() > MAX_SOURCE || !source.projection.is_empty() {
        return Err("Invalid source projection input".into());
    }
    // Ordinary fragments need no second Markdown parse. Use the existing
    // SIMD-capable byte search before admitting source projection work.
    if lookup.is_none()
        && (source.text.len() <= 8192
            || !memchr::memchr2_iter(b'd', b'D', source.text.as_bytes()).any(|i| {
                source
                    .text
                    .get(i..i + 11)
                    .is_some_and(|s| s.eq_ignore_ascii_case("data:image/"))
            }))
    {
        return Ok(source);
    }
    let (normalized, _) = crate::diagrams::normalize(&source.text, options());
    let definitions = std::cell::RefCell::new(std::collections::HashMap::new());
    let definition_bytes = std::cell::Cell::new(0usize);
    let context_exhausted = std::cell::Cell::new(false);
    let record = |label: &str| {
        let definition = lookup.and_then(|lookup| lookup(label))?;
        let key = crate::references::Key::new(label.to_owned());
        if !definitions.borrow().contains_key(&key) {
            let bytes = definition.label.len()
                + definition.title.len()
                + if eligible(&definition.destination) {
                    6
                } else {
                    definition.destination.len()
                };
            if definition.title.len() > 8192
                || definition.label.len() > 4096
                || (definition.destination.len() > 8192 && !eligible(&definition.destination))
                || definitions.borrow().len() >= crate::references::MAX_DEFINITIONS
                || definition_bytes.get().saturating_add(bytes)
                    > crate::references::MAX_CONTEXT_BYTES
            {
                context_exhausted.set(true);
                return None;
            }
            definition_bytes.set(definition_bytes.get() + bytes);
            definitions.borrow_mut().insert(key, definition.clone());
        }
        Some(definition)
    };
    let mut events = crate::references::events(
        &normalized,
        options(),
        lookup.map(|_| &record as &crate::references::Lookup<'_>),
    );
    let mut replacements = Vec::new();
    let mut output = String::new();
    let mut previous = 0;
    let mut line = source.line;
    while let Some((event, span)) = events.next() {
        let Event::Start(Tag::Image { dest_url, .. }) = event else {
            continue;
        };
        if !eligible(&dest_url) {
            continue;
        }
        if replacements.len() >= MAX_IMAGES {
            return Err("Too many embedded images in one fragment".into());
        }
        let alt = alt_markup(&mut events, &source.text, &span)?;
        if span.start < previous || span.end > source.text.len() {
            return Err("Overlapping image source spans".into());
        }
        let gap = &source.text[previous..span.start];
        output.push_str(gap);
        line += gap.match_indices('\n').count();
        let image_text = &source.text[span.clone()];
        let newlines = image_text.match_indices('\n').count();
        let start = output.len();
        output.push_str("![");
        output.push_str(&alt);
        output.push_str(&format!(
            "](studio-doc-image:{page}:{})",
            replacements.len()
        ));
        replacements.push(Replacement {
            start,
            end: output.len(),
            source_start: source.start + span.start,
            source_end: source.start + span.end,
            line,
            end_line: line + newlines - usize::from(image_text.ends_with('\n')),
            page,
            image: replacements.len(),
            newlines: output[start..].match_indices('\n').count(),
        });
        line += newlines;
        previous = span.end;
    }
    drop(events);
    drop(normalized);
    // Large image definitions have become private snapshot references. Only
    // ordinary, bounded definitions need to cross the native transport.
    if context_exhausted.get() {
        return Err("Reference definition context exceeds its budget".into());
    }
    source.references = definitions
        .into_inner()
        .into_values()
        .map(|d| crate::references::Definition {
            label: d.label.clone(),
            // Other link occurrences of the image definition remain inert.
            destination: if eligible(&d.destination) {
                "data:,".into()
            } else {
                d.destination.clone()
            },
            title: d.title.clone(),
        })
        .collect();
    source.references.sort_by(|a, b| a.label.cmp(&b.label));
    crate::references::context(source.references.iter())?;
    if replacements.is_empty() {
        return Ok(source);
    }
    output.push_str(&source.text[previous..]);
    if output.len() > 512 * 1024 {
        return Err("Projected source exceeds the fragment budget".into());
    }
    source.text = output;
    source.projection = replacements;
    validate(&source)?;
    Ok(source)
}
pub fn image(source: &str, ordinal: usize) -> Result<crate::embedded_image::Image, String> {
    image_with_references(source, ordinal, None)
}
pub fn image_with_references(
    source: &str,
    ordinal: usize,
    lookup: Option<&crate::references::Lookup<'_>>,
) -> Result<crate::embedded_image::Image, String> {
    if source.len() > MAX_SOURCE || ordinal >= MAX_IMAGES {
        return Err("Invalid embedded image reference".into());
    }
    let (normalized, _) = crate::diagrams::normalize(source, options());
    let mut index = 0;
    let mut events = crate::references::events(&normalized, options(), lookup);
    while let Some((event, _)) = events.next() {
        if let Event::Start(Tag::Image { dest_url, .. }) = event {
            if eligible(&dest_url) {
                if index == ordinal {
                    return crate::embedded_image::decode(
                        &dest_url,
                        crate::embedded_image::MAX_BYTES,
                    )
                    .map_err(str::to_owned);
                }
                index += 1;
                // Projection replaces this entire image, including its caption.
                // Nested image syntax in that caption has no independent ordinal.
                let mut depth = 1usize;
                for (event, _) in events.by_ref() {
                    match event {
                        Event::Start(Tag::Image { .. }) => depth += 1,
                        Event::End(TagEnd::Image) => {
                            depth -= 1;
                            if depth == 0 {
                                break;
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }
    Err("Embedded image is absent from this snapshot".into())
}
pub fn validate(source: &SourceWindow) -> Result<(), String> {
    if source.start > source.end
        || source.end > MAX_SOURCE
        || source.line == 0
        || source.line > MAX_SOURCE + 1
        || source.projection.len() > MAX_IMAGES
    {
        return Err("Invalid projected source range".into());
    }
    let mut last = 0;
    let mut delta = 0i128;
    let mut line_delta = 0;
    for p in &source.projection {
        if p.start < last
            || p.start >= p.end
            || p.end > source.text.len()
            || !source.text.is_char_boundary(p.start)
            || !source.text.is_char_boundary(p.end)
            || p.source_start as i128 != source.start as i128 + p.start as i128 + delta
            || p.source_start >= p.source_end
            || p.source_end > source.end
            || p.line
                != source.line + source.text[..p.start].match_indices('\n').count() + line_delta
            || p.end_line < p.line
            || p.end_line > MAX_SOURCE + 1
            || p.image >= MAX_IMAGES
            || p.page > 100_000
            || source.text[p.start..p.end].match_indices('\n').count() != p.newlines
            || p.end_line - p.line < p.newlines
            || p.end_line - p.line > p.source_end - p.source_start
        {
            return Err("Invalid projected source mapping".into());
        }
        delta += (p.source_end - p.source_start) as i128 - (p.end - p.start) as i128;
        line_delta += p.end_line - p.line - p.newlines;
        last = p.end;
    }
    if !source.projection.is_empty()
        && source.end_line
            != source.line + source.text.match_indices('\n').count() + line_delta
                - usize::from(source.text.ends_with('\n'))
    {
        return Err("Projected source line count mismatch".into());
    }
    if source.start as i128 + source.text.len() as i128 + delta != source.end as i128 {
        return Err("Projected source length mismatch".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BlockKind, Fragment, FragmentMetadata, ImageSource};
    fn window(text: &str) -> SourceWindow {
        SourceWindow {
            start: 0,
            end: text.len(),
            line: 1,
            end_line: text.lines().count(),
            text: text.into(),
            projection: Vec::new(),
            references: Vec::new(),
        }
    }
    fn fragment(source: SourceWindow) -> Fragment {
        Fragment {
            page: 7,
            document_id: "snapshot".into(),
            revision: "revision".into(),
            metadata: FragmentMetadata {
                start: source.start,
                end: source.end,
                line: source.line,
                rows: 0,
                row_start: 0,
                table: None,
            },
            source,
            header: None,
        }
    }
    #[test]
    fn projection_preserves_rendered_text_and_original_ranges_after_large_images() {
        let uri = format!("data:image/png;base64,{}", "QUFB".repeat(4000));
        let text=format!("# Heading\n\nText **before** ![**bold** $x$ `code` a \\[b\\] 中文]({uri}) and after.\n\n## Next\n\nvalue\n");
        let original = crate::parse(&fragment(window(&text))).unwrap();
        let projected = project(window(&text), 7).unwrap();
        assert!(projected.text.len() < 256);
        assert_eq!(projected.projection.len(), 1);
        assert_eq!(projected.end, text.len());
        let blocks = crate::parse(&fragment(projected.clone())).unwrap();
        assert_eq!(blocks.len(), original.len());
        for (a, b) in blocks.iter().zip(&original) {
            assert_eq!(a.source, b.source);
            match (&a.kind, &b.kind) {
                (BlockKind::Paragraph(a), BlockKind::Paragraph(b)) => {
                    assert_eq!(a.plain, b.plain);
                    assert_eq!(a.markup, b.markup);
                }
                _ => {}
            }
        }
        let BlockKind::Paragraph(content) = &blocks[1].kind else {
            panic!("Image paragraph missing")
        };
        assert_eq!(
            content.images[0].source,
            ImageSource::Document { page: 7, image: 0 }
        );
        assert_eq!(image(&text, 0).unwrap().bytes.len(), 12000);
        assert!(image(&text, 1).is_err());
        let mut invalid = projected.clone();
        invalid.projection[0].source_end += 1;
        assert!(crate::parse(&fragment(invalid)).is_err());
        let mut invalid = projected;
        invalid.projection[0].line += 1;
        assert!(crate::parse(&fragment(invalid)).is_err());
    }
    #[test]
    fn multiline_alt_and_code_fences_keep_source_positions_and_are_not_fetched() {
        let uri = format!("data:image/png;base64,{}", "QUFB".repeat(4000));
        let text = format!("![first\nsecond]({uri})\n\n# After\n\n```text\n![code]({uri})\n```\n");
        let raw = crate::parse(&fragment(window(&text))).unwrap();
        let projected = project(window(&text), 2).unwrap();
        assert_eq!(projected.projection.len(), 1);
        assert_eq!(projected.projection[0].newlines, 1);
        let blocks = crate::parse(&fragment(projected)).unwrap();
        for (a, b) in blocks.iter().zip(&raw) {
            assert_eq!(a.source, b.source);
        }
        assert!(matches!(blocks[2].kind, BlockKind::Code { .. }));
    }
    #[test]
    fn candidate_scan_preserves_mixed_case_schemes_and_ordinary_source() {
        let ordinary = "ordinary text 中文\n".repeat(1000);
        assert_eq!(project(window(&ordinary), 0).unwrap().text, ordinary);
        for prefix in [
            "data:image/png;base64,",
            "DATA:IMAGE/PNG;BASE64,",
            "DaTa:ImAgE/PnG;BaSe64,",
        ] {
            let source = format!("![Image]({prefix}{})", "QUFB".repeat(4000));
            let compact = project(window(&source), 0).unwrap();
            assert_eq!(compact.projection.len(), 1);
            assert_eq!(image(&source, 0).unwrap().bytes.len(), 12000);
        }
    }
    #[test]
    fn nested_image_captions_have_no_independent_preview_or_original_ordinal() {
        let uri = |bytes: &str| format!("data:image/png;base64,{}", bytes.repeat(4000));
        let text = format!(
            "![Outer ![Inner]({}) tail]({})\n\n![Next]({})\n",
            uri("QkJC"),
            uri("QUFB"),
            uri("Q0ND")
        );
        let compact = project(window(&text), 7).unwrap();
        assert_eq!(compact.projection.len(), 2);
        let blocks = crate::parse(&fragment(compact)).unwrap();
        let BlockKind::Paragraph(outer) = &blocks[0].kind else {
            panic!("outer")
        };
        assert_eq!(outer.plain, "Outer Inner tail");
        assert_eq!(outer.images.len(), 1);
        assert_eq!(outer.images[0].alternative, "Outer Inner tail");
        assert_eq!(
            outer.images[0].source,
            ImageSource::Document { page: 7, image: 0 }
        );
        let BlockKind::Paragraph(next) = &blocks[1].kind else {
            panic!("next")
        };
        assert_eq!(
            next.images[0].source,
            ImageSource::Document { page: 7, image: 1 }
        );
        assert!(image(&text, 0).unwrap().bytes.iter().all(|b| *b == b'A'));
        assert!(image(&text, 1).unwrap().bytes.iter().all(|b| *b == b'C'));
        assert!(image(&text, 2).is_err());
        let direct = crate::parse(&fragment(window(&text))).unwrap();
        let BlockKind::Paragraph(outer) = &direct[0].kind else {
            panic!("direct outer")
        };
        assert_eq!(outer.images.len(), 1);
        assert_eq!(outer.images[0].alternative, "Outer Inner tail");
    }
    #[test]
    fn literal_internal_references_do_not_acquire_document_image_authority() {
        let parsed = crate::parse(&fragment(window("![literal](studio-doc-image:7:0)"))).unwrap();
        let BlockKind::Paragraph(text) = &parsed[0].kind else {
            panic!("Paragraph missing")
        };
        assert!(matches!(text.images[0].source, ImageSource::Reference(_)));
        assert!(!text.markup.contains("href="));
    }
}
