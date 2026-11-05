//! Bounded parser-owned Markdown references. Indexing records targets only; it
//! never reads a sibling file or fetches a network URL.
use pulldown_cmark::{Event, Tag};
use serde::{Deserialize, Serialize};
use std::{
    fs::File,
    io::{self, BufWriter, Read, Seek, SeekFrom, Write},
    ops::Range,
    path::Path,
};

pub const MAX_REFERENCES: usize = 4096;
pub const MAX_TARGET_BYTES: usize = 32 * 1024 * 1024;
pub const MAX_LINK_BYTES: usize = 8192;
pub const PREFIX: &str = "marknoria-reference:";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Kind {
    Image,
    Link,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Reference {
    pub kind: Kind,
    pub inline: bool,
    pub start_byte: usize,
    pub end_byte: usize,
    pub offset: u64,
    pub bytes: usize,
    #[serde(default)]
    pub occurrence: usize,
    #[serde(default)]
    pub approximate: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Anchor {
    pub name: String,
    pub start_byte: usize,
    pub end_byte: usize,
    pub page: usize,
}

pub(super) struct Collector {
    output: BufWriter<File>,
    pub references: Vec<Reference>,
    bytes: usize,
    anchors: Vec<Anchor>,
    table_depth: usize,
}
impl Collector {
    pub fn new(directory: &Path) -> io::Result<Self> {
        Ok(Self {
            output: BufWriter::new(File::create(directory.join("references.bin"))?),
            references: vec![],
            bytes: 0,
            anchors: vec![],
            table_depth: 0,
        })
    }
    pub fn capture<'a>(
        &mut self,
        mut event: Event<'a>,
        span: Range<usize>,
    ) -> io::Result<(Event<'a>, Range<usize>)> {
        track_table(&event, &mut self.table_depth);
        if let Event::Html(source) | Event::InlineHtml(source) = &event {
            event = match super::markdown_html::web_fragment(
                source,
                self.references.len(),
                self.table_depth > 0,
            )? {
                Some(fragment) => {
                    if self.anchors.len() + fragment.anchors.len() > MAX_REFERENCES {
                        return Err(super::invalid("HTML anchor budget exceeded"));
                    }
                    for (occurrence, reference) in fragment.references.into_iter().enumerate() {
                        self.capture_target(
                            reference.kind,
                            &reference.target,
                            &span,
                            occurrence,
                            true,
                        )?;
                    }
                    self.anchors
                        .extend(fragment.anchors.into_iter().map(|name| Anchor {
                            name,
                            start_byte: span.start,
                            end_byte: span.end,
                            page: 0,
                        }));
                    Event::Html(fragment.html.into())
                }
                None => literal(source),
            };
            return Ok((event, span));
        }
        let (kind, target) = match &mut event {
            Event::Start(Tag::Image { dest_url, .. }) => (Kind::Image, dest_url),
            Event::Start(Tag::Link { dest_url, .. }) => (Kind::Link, dest_url),
            _ => return Ok((event, span)),
        };
        let id = self.capture_target(kind, target, &span, 0, false)?;
        *target = format!("{PREFIX}{id}").into();
        Ok((event, span))
    }
    fn capture_target(
        &mut self,
        kind: Kind,
        target: &str,
        span: &Range<usize>,
        occurrence: usize,
        approximate: bool,
    ) -> io::Result<usize> {
        let inline = target
            .get(..5)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("data:"));
        if self.references.len() >= MAX_REFERENCES
            || target.len() > MAX_TARGET_BYTES
            || self.bytes.saturating_add(target.len()) > MAX_TARGET_BYTES
            || (!inline || kind == Kind::Link) && target.len() > MAX_LINK_BYTES
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Markdown references exceed their count/target budget",
            ));
        }
        let id = self.references.len();
        self.references.push(Reference {
            kind,
            inline,
            start_byte: span.start,
            end_byte: span.end,
            offset: self.bytes as u64,
            bytes: target.len(),
            occurrence,
            approximate,
        });
        self.output.write_all(target.as_bytes())?;
        self.bytes += target.len();
        Ok(id)
    }
    pub fn finish(mut self) -> io::Result<(Vec<Reference>, Vec<Anchor>)> {
        self.output.flush()?;
        Ok((self.references, self.anchors))
    }
}

fn track_table(event: &Event<'_>, depth: &mut usize) {
    match event {
        Event::Start(Tag::Table(_)) => *depth += 1,
        Event::End(pulldown_cmark::TagEnd::Table) => *depth = depth.saturating_sub(1),
        _ => {}
    }
}
fn literal<'a>(source: &str) -> Event<'a> {
    let mut html = String::new();
    pulldown_cmark::html::push_html(&mut html, std::iter::once(Event::Text(source.into())));
    Event::Html(html.into())
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceInfo {
    pub revision: String,
    pub id: usize,
    pub kind: Kind,
    pub inline: bool,
    pub start_byte: usize,
    pub end_byte: usize,
    pub target: Option<String>,
    pub occurrence: usize,
    pub precision: &'static str,
}

pub fn info(directory: &Path, index: &super::Index, id: usize) -> io::Result<ReferenceInfo> {
    let reference = index
        .references
        .get(id)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Unknown Markdown reference"))?;
    Ok(ReferenceInfo {
        revision: index.revision.clone(),
        id,
        kind: reference.kind,
        inline: reference.inline,
        start_byte: reference.start_byte,
        end_byte: reference.end_byte,
        occurrence: reference.occurrence,
        precision: if reference.approximate {
            "approximate"
        } else {
            "exact"
        },
        target: if reference.inline {
            None
        } else {
            Some(target(directory, index, id)?)
        },
    })
}

pub fn target(directory: &Path, index: &super::Index, id: usize) -> io::Result<String> {
    let reference = index
        .references
        .get(id)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Unknown Markdown reference"))?;
    if reference.bytes > MAX_TARGET_BYTES
        || reference.offset.saturating_add(reference.bytes as u64) > MAX_TARGET_BYTES as u64
        || reference.kind == Kind::Link && reference.bytes > MAX_LINK_BYTES
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Invalid Markdown reference budget",
        ));
    }
    let mut file = File::open(directory.join("references.bin"))?;
    file.seek(SeekFrom::Start(reference.offset))?;
    let mut bytes = vec![0; reference.bytes];
    file.read_exact(&mut bytes)?;
    String::from_utf8(bytes).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

pub(super) fn projected(event: Event<'_>) -> Event<'_> {
    match event {
        Event::Start(Tag::Image { dest_url, title, .. }) => Event::Html(format!("<span class=\"markdown-reference-image\" data-image-reference=\"{}\" title=\"{}\"><span class=\"markdown-reference-alt\">[image: ",dest_url.strip_prefix(PREFIX).unwrap_or(""),super::documents::escaped(&title)).into()),
        Event::End(pulldown_cmark::TagEnd::Image) => Event::Html("]</span></span>".into()),
        Event::Start(Tag::Link { dest_url, title, .. }) => Event::Html(format!("<a href=\"#\" data-link-reference=\"{}\" title=\"{}\">", dest_url.strip_prefix(PREFIX).unwrap_or(""),super::documents::escaped(&title)).into()),
        other => other,
    }
}

// URI payloads have already been replaced by bounded IDs. Count renderable
// event data instead of the discarded original URI's source span; final HTML
// and event limits remain authoritative and indexed/native limits are unchanged.
pub(super) fn event_weight(event: &Event<'_>) -> usize {
    match event {
        Event::Text(text)
        | Event::Code(text)
        | Event::Html(text)
        | Event::InlineHtml(text)
        | Event::FootnoteReference(text)
        | Event::InlineMath(text)
        | Event::DisplayMath(text) => text.len(),
        Event::Start(
            Tag::Image {
                dest_url, title, ..
            }
            | Tag::Link {
                dest_url, title, ..
            },
        ) => dest_url.len().saturating_add(title.len()).saturating_add(1),
        Event::Start(Tag::CodeBlock(pulldown_cmark::CodeBlockKind::Fenced(info))) => {
            info.len().saturating_add(1)
        }
        _ => 1,
    }
}

pub(super) struct Auditor<'a> {
    directory: &'a Path,
    index: &'a super::Index,
    ids: std::collections::HashMap<(usize, usize, Kind, usize), usize>,
    seen: Vec<bool>,
    anchors_seen: Vec<bool>,
    table_depth: usize,
}
impl<'a> Auditor<'a> {
    pub fn new(directory: &'a Path, index: &'a super::Index) -> Self {
        Self {
            directory,
            index,
            ids: index
                .references
                .iter()
                .enumerate()
                .map(|(id, reference)| {
                    (
                        (
                            reference.start_byte,
                            reference.end_byte,
                            reference.kind,
                            reference.occurrence,
                        ),
                        id,
                    )
                })
                .collect(),
            seen: vec![false; index.references.len()],
            anchors_seen: vec![false; index.anchors.len()],
            table_depth: 0,
        }
    }
    pub fn event<'b>(
        &mut self,
        mut event: Event<'b>,
        span: &Range<usize>,
    ) -> io::Result<Event<'b>> {
        track_table(&event, &mut self.table_depth);
        if let Event::Html(source) | Event::InlineHtml(source) = &event {
            let base = self
                .index
                .references
                .iter()
                .position(|reference| {
                    reference.approximate
                        && reference.start_byte == span.start
                        && reference.end_byte == span.end
                })
                .unwrap_or(0);
            return match super::markdown_html::web_fragment(source, base, self.table_depth > 0)? {
                Some(fragment) => {
                    for (occurrence, reference) in fragment.references.iter().enumerate() {
                        let id = *self
                            .ids
                            .get(&(span.start, span.end, reference.kind, occurrence))
                            .ok_or_else(|| {
                                super::invalid("Raw reference source mapping mismatch")
                            })?;
                        if id != base + occurrence
                            || !self.index.references[id].approximate
                            || target(self.directory, self.index, id)? != reference.target
                        {
                            return Err(super::invalid("Raw reference differs from source"));
                        }
                        self.seen[id] = true;
                    }
                    let anchors = self
                        .index
                        .anchors
                        .iter()
                        .enumerate()
                        .filter(|(_, anchor)| {
                            anchor.start_byte == span.start && anchor.end_byte == span.end
                        })
                        .collect::<Vec<_>>();
                    if anchors.len() != fragment.anchors.len()
                        || anchors
                            .iter()
                            .zip(&fragment.anchors)
                            .any(|((_, anchor), name)| anchor.name != *name)
                    {
                        return Err(super::invalid("HTML anchor differs from source"));
                    }
                    for (id, _) in anchors {
                        self.anchors_seen[id] = true;
                    }
                    Ok(Event::Html(fragment.html.into()))
                }
                None => Ok(literal(source)),
            };
        }
        let (kind, url) = match &mut event {
            Event::Start(Tag::Image { dest_url, .. }) => (Kind::Image, dest_url),
            Event::Start(Tag::Link { dest_url, .. }) => (Kind::Link, dest_url),
            _ => return Ok(event),
        };
        let id = *self
            .ids
            .get(&(span.start, span.end, kind, 0))
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Reference source mapping mismatch",
                )
            })?;
        if target(self.directory, self.index, id)? != url.as_ref() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Reference target differs from original source",
            ));
        }
        self.seen[id] = true;
        *url = format!("{PREFIX}{id}").into();
        Ok(event)
    }
    pub fn finish(self) -> io::Result<()> {
        if self.index.anchors.iter().any(|anchor| {
            anchor.page
                != self
                    .index
                    .pages
                    .partition_point(|page| page.start <= anchor.start_byte)
                    .saturating_sub(1)
        }) {
            return Err(super::invalid("HTML anchor page differs from source"));
        }
        if self.seen.iter().chain(&self.anchors_seen).all(|seen| *seen) {
            Ok(())
        } else {
            Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Unexpected reference metadata",
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    mod fixtures {
        include!("../../tests/support/references.rs");
    }
    use super::*;
    use crate::core::{ProjectionProfile, build_index, build_index_with_profile, read_page};
    #[test]
    fn a_web_table_with_no_uri_or_anchor_still_audits_its_projection_policy() {
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("empty-image.md");
        std::fs::write(&file,"| Picture |\n|---|\n| <img alt=\"missing\" srcset=\"https://example.invalid/never.png 2x\"> |\n").unwrap();
        let cache = root.path().join("web");
        let index = build_index_with_profile(&file, &cache, ProjectionProfile::Web).unwrap();
        assert!(index.references.is_empty() && index.anchors.is_empty() && index.web_markdown);
        crate::core::audit_tables(&cache, &index).unwrap();
    }
    #[test]
    fn web_image_profile_keeps_source_pixels_and_indexed_preview_stays_bounded() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("quality.png");
        fixtures::source_image().save(&path).unwrap();
        let cache = root.path().join("web");
        let web = build_index_with_profile(&path, &cache, ProjectionProfile::Web).unwrap();
        assert_eq!((web.assets[0].width, web.assets[0].height), (4096, 1024));
        let frame = crate::core::documents::asset(&cache, &web, 0, 0).unwrap();
        let pixels = image::open(frame).unwrap().to_rgba8();
        assert_eq!(pixels.get_pixel(2000, 500).0, [0, 0, 0, 255]);
        assert_eq!(pixels.get_pixel(2001, 500).0, [255, 255, 255, 255]);
        assert_eq!(pixels.get_pixel(21, 20).0, [0, 0, 0, 255]);
        let indexed = build_index(&path, &root.path().join("indexed")).unwrap();
        assert_eq!(
            (indexed.assets[0].width, indexed.assets[0].height),
            (2048, 512)
        );
        assert_eq!(indexed.revision, web.revision);
    }
    #[test]
    fn raw_html_resources_are_transactional_and_keep_fragment_precision_and_occurrences() {
        let root = tempfile::tempdir().unwrap();
        let source = fixtures::raw_html_document();
        let file = root.path().join("raw.md");
        std::fs::write(&file, &source).unwrap();
        let cache = root.path().join("web");
        let mut index = build_index_with_profile(&file, &cache, ProjectionProfile::Web).unwrap();
        assert_eq!(index.references.len(), 11);
        assert_eq!(info(&cache, &index, 2).unwrap().precision, "exact");
        let first = info(&cache, &index, 3).unwrap();
        let second = info(&cache, &index, 4).unwrap();
        assert_eq!(first.precision, "approximate");
        assert_eq!(second.precision, "approximate");
        assert_eq!(
            (first.start_byte, first.end_byte),
            (second.start_byte, second.end_byte)
        );
        assert_eq!((first.occurrence, second.occurrence), (0, 1));
        assert!(source[first.start_byte..first.end_byte].contains("<div><img"));
        assert!(
            index
                .anchors
                .iter()
                .any(|anchor| anchor.name == "custom-heading")
        );
        assert!(
            index
                .anchors
                .iter()
                .any(|anchor| anchor.name == "custom-anchor")
        );
        let html = (0..index.pages.len())
            .map(|id| read_page(&cache, &index, id).unwrap())
            .collect::<String>();
        assert!(
            html.contains("data-image-reference=\"3\"")
                && html.contains("data-image-reference=\"7\"")
        );
        assert!(html.contains("data-document-anchor=\"source-view\""));
        assert!(
            !html.contains(" id=\"source-view\"") && !html.contains("data-image-reference=\"999\"")
        );
        assert!(
            !html.contains(" onerror=")
                && !html.contains(" onclick=")
                && !html.contains(" srcset=")
        );
        assert!(!html.contains("href=\"javascript:") && !html.contains("<script>"));
        assert!(
            !index
                .references
                .iter()
                .enumerate()
                .any(|(id, _)| target(&cache, &index, id).unwrap() == "not-accepted.png")
        );
        crate::core::audit_tables(&cache, &index).unwrap();
        index.references[4].occurrence = 0;
        assert!(crate::core::audit_tables(&cache, &index).is_err());
        let legacy = build_index(&file, &root.path().join("native")).unwrap();
        assert!(legacy.references.is_empty() && legacy.anchors.is_empty());
        assert_eq!(std::fs::read(&file).unwrap(), source.as_bytes());
    }

    #[test]
    fn web_references_keep_bom_crlf_ranges_and_include_table_images_without_reading_targets() {
        let root = tempfile::tempdir().unwrap();
        let source = "\u{feff}# 中文😀\r\n\r\n![avatar](missing.png) [notes](notes%20file.md#heading-0)\r\n\r\n| Picture |\r\n|---|\r\n| ![table](other.svg) |\r\n\r\n<span data-image-reference=\"999\">forged</span>\r\n";
        let path = root.path().join("doc.md");
        std::fs::write(&path, source).unwrap();
        let web = root.path().join("web");
        let index = build_index_with_profile(&path, &web, ProjectionProfile::Web).unwrap();
        assert_eq!(index.references.len(), 3);
        assert_eq!(target(&web, &index, 0).unwrap(), "missing.png");
        assert_eq!(
            target(&web, &index, 1).unwrap(),
            "notes%20file.md#heading-0"
        );
        assert_eq!(target(&web, &index, 2).unwrap(), "other.svg");
        for reference in &index.references {
            assert!(
                source.is_char_boundary(reference.start_byte)
                    && source.is_char_boundary(reference.end_byte)
            );
            assert!(reference.start_byte >= 3);
        }
        assert_eq!(
            &source[index.references[0].start_byte..index.references[0].end_byte],
            "![avatar](missing.png)"
        );
        let html = (0..index.pages.len())
            .map(|page| read_page(&web, &index, page).unwrap())
            .collect::<String>();
        assert!(
            html.contains("data-image-reference=\"0\"")
                && html.contains("data-image-reference=\"2\"")
        );
        assert!(!html.contains("<span data-image-reference=\"999\""));
        assert!(!html.contains("missing.png"));
        crate::core::audit_tables(&web, &index).unwrap();
        let legacy = root.path().join("indexed");
        let old = build_index(&path, &legacy).unwrap();
        assert!(old.references.is_empty());
        assert!(
            read_page(&legacy, &old, 0)
                .unwrap()
                .contains("[image: avatar]")
        );
        assert_eq!(old.revision, index.revision);
        assert_eq!(std::fs::read(&path).unwrap(), source.as_bytes());
    }

    #[test]
    fn large_inline_targets_stay_out_of_projection_metadata_and_fail_explicit_budgets() {
        let root = tempfile::tempdir().unwrap();
        let uri = format!("data:image/png;base64,{}", "A".repeat(200_000));
        let path = root.path().join("inline.md");
        std::fs::write(&path, format!("![inline]({uri})\n")).unwrap();
        let cache = root.path().join("cache");
        let index = build_index_with_profile(&path, &cache, ProjectionProfile::Web).unwrap();
        let metadata = info(&cache, &index, 0).unwrap();
        assert!(metadata.inline && metadata.target.is_none());
        assert!(serde_json::to_vec(&metadata).unwrap().len() < 1024);
        assert!(serde_json::to_vec(&index).unwrap().len() < 4096);
        assert!(read_page(&cache, &index, 0).unwrap().len() < 1024);
        assert_eq!(target(&cache, &index, 0).unwrap(), uri);
        let too_many = root.path().join("many.md");
        std::fs::write(&too_many, "[x](file.md)\n\n".repeat(MAX_REFERENCES + 1)).unwrap();
        assert!(
            build_index_with_profile(&too_many, &root.path().join("many"), ProjectionProfile::Web)
                .err()
                .unwrap()
                .to_string()
                .contains("reference")
        );
        let long_link = root.path().join("link.md");
        std::fs::write(
            &long_link,
            format!("[link]({})", "x".repeat(MAX_LINK_BYTES + 1)),
        )
        .unwrap();
        assert!(
            build_index_with_profile(
                &long_link,
                &root.path().join("long"),
                ProjectionProfile::Web
            )
            .is_err()
        );
    }
}
