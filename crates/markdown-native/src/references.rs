//! CommonMark reference resolution shared by backend projection and native parsing.
use pulldown_cmark::{BrokenLink, CowStr, Event, LinkType, Options, Parser, Tag};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, ops::Range, sync::Arc};

pub type Key = unicase::UniCase<String>;
pub type Lookup<'a> = dyn Fn(&str) -> Option<Arc<Definition>> + 'a;
pub type Events<'a> = Box<dyn Iterator<Item = (Event<'a>, Range<usize>)> + 'a>;
pub const MAX_CONTEXT_BYTES: usize = 128 * 1024;
pub const MAX_DEFINITIONS: usize = 256;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Definition {
    pub label: String,
    pub destination: String,
    pub title: String,
}
impl Definition {
    pub fn bytes(&self) -> usize {
        self.label.len() + self.destination.len() + self.title.len()
    }
    fn pair(&self) -> (CowStr<'static>, CowStr<'static>) {
        (self.destination.clone().into(), self.title.clone().into())
    }
}

pub fn context<'a>(
    items: impl Iterator<Item = &'a Definition>,
) -> Result<HashMap<Key, Arc<Definition>>, String> {
    let mut result: HashMap<Key, Arc<Definition>> = HashMap::new();
    let mut bytes = 0usize;
    for item in items {
        if item.label.is_empty()
            || item.label.len() > 4096
            || item.destination.len() > 8192
            || item.title.len() > 8192
        {
            return Err("Invalid reference definition context".into());
        }
        let key = Key::new(item.label.clone());
        if let Some(existing) = result.get(&key) {
            if existing.destination != item.destination || existing.title != item.title {
                return Err("Conflicting reference definitions".into());
            }
            continue;
        }
        bytes = bytes.saturating_add(item.bytes());
        if bytes > MAX_CONTEXT_BYTES || result.len() >= MAX_DEFINITIONS {
            return Err("Reference definition context exceeds its budget".into());
        }
        result.insert(key, Arc::new(item.clone()));
    }
    Ok(result)
}

pub fn events<'a>(text: &'a str, options: Options, lookup: Option<&'a Lookup<'a>>) -> Events<'a> {
    let Some(lookup) = lookup else {
        return Box::new(Parser::new_ext(text, options).into_offset_iter());
    };
    // A page is much shorter than the full document. Feeding a large external
    // URL to the parser's broken-link callback would exhaust its per-input
    // expansion guard and silently suppress all later references. Resolve only
    // identity here; attach destinations to emitted events under our own bounded
    // context/image budgets, without expanding the parser's input.
    let callback = move |link: BrokenLink<'a>| {
        lookup(&link.reference).map(|_| (CowStr::Borrowed(""), CowStr::Borrowed("")))
    };
    let parser = Parser::new_with_broken_link_callback(text, options, Some(callback));
    Box::new(parser.into_offset_iter().map(move |(mut event, span)| {
        // Local duplicate definitions must not override the first definition in
        // the complete document. The callback alone only handles missing labels.
        match &mut event {
            Event::Start(Tag::Link {
                link_type,
                dest_url,
                title,
                id,
            })
            | Event::Start(Tag::Image {
                link_type,
                dest_url,
                title,
                id,
            }) if matches!(
                link_type,
                LinkType::Reference
                    | LinkType::Collapsed
                    | LinkType::Shortcut
                    | LinkType::ReferenceUnknown
                    | LinkType::CollapsedUnknown
                    | LinkType::ShortcutUnknown
            ) =>
            {
                if let Some(definition) = lookup(id) {
                    (*dest_url, *title) = definition.pair();
                }
            }
            _ => {}
        }
        (event, span)
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BlockKind, Fragment, FragmentMetadata, ImageSource, SourceWindow};
    fn definitions(text: &str) -> HashMap<Key, Arc<Definition>> {
        Parser::new_ext(text, Options::all())
            .reference_definitions()
            .iter()
            .map(|(label, d)| {
                (
                    Key::new(label.into()),
                    Arc::new(Definition {
                        label: label.into(),
                        destination: d.dest.to_string(),
                        title: d.title.as_deref().unwrap_or("").into(),
                    }),
                )
            })
            .collect()
    }
    fn window(text: &str, start: usize, line: usize) -> SourceWindow {
        SourceWindow {
            start,
            end: start + text.len(),
            line,
            end_line: line + text.matches('\n').count() - usize::from(text.ends_with('\n')),
            text: text.into(),
            projection: Vec::new(),
            references: Vec::new(),
        }
    }
    fn fragment(source: SourceWindow) -> Fragment {
        Fragment {
            page: 0,
            document_id: "doc".into(),
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
    fn full_collapsed_shortcut_unicode_duplicates_and_nested_images_match_whole_document() {
        let prefix="[key]: original.md \"First title\"\n[collapsed]: collapsed.md\n[shortcut]: shortcut.md\n[Straße]: unicode.md\n[multi word]: whitespace.md\n[image]: sample.png\n\n";
        let body="A [full][KEY], [collapsed][], [shortcut], [unicode][STRASSE], [space][multi\n word], [![nested][image]][key], and [inline](inline.md).\n\n[key]: wrong.md\n";
        let text = format!("{prefix}{body}");
        let defs = definitions(&text);
        let lookup = |label: &str| defs.get(&Key::new(label.into())).cloned();
        let source = crate::source_projection::project_with_references(
            window(body, prefix.len(), prefix.matches('\n').count() + 1),
            0,
            Some(&lookup),
        )
        .unwrap();
        assert!(source.projection.is_empty());
        assert_eq!(source.references.len(), 6);
        let blocks = crate::parse(&fragment(source)).unwrap();
        let full = crate::parse_text(&text).unwrap();
        assert_eq!(blocks, full);
        let BlockKind::Paragraph(p) = &blocks[0].kind else {
            panic!("paragraph")
        };
        assert!(p.markup.contains("original.md") && !p.markup.contains("wrong.md"));
        assert!(matches!(&p.images[0].source, ImageSource::Reference(path) if path=="sample.png"));
    }
    #[test]
    fn huge_off_page_reference_image_is_deferred_with_exact_original_coordinates() {
        let uri = format!("data:image/png;base64,{}", "QUFB".repeat(40000));
        let prefix = "## Before\n\n";
        let body =
            "![**caption** 中文][picture]\n\nAfter [download][picture] and [Guide][guide].\n";
        let full = format!("{prefix}{body}\n[picture]: {uri}\n[guide]: guide.md\n");
        let defs = definitions(&full);
        let lookup = |label: &str| defs.get(&Key::new(label.into())).cloned();
        let source = crate::source_projection::project_with_references(
            window(body, prefix.len(), 3),
            7,
            Some(&lookup),
        )
        .unwrap();
        assert_eq!(source.references.len(), 2);
        assert_eq!(
            source
                .references
                .iter()
                .find(|d| d.label == "picture")
                .unwrap()
                .destination,
            "data:,"
        );
        assert_eq!(source.projection.len(), 1);
        assert!(source.text.len() < 128 && !source.text.contains("data:"));
        let blocks = crate::parse(&fragment(source)).unwrap();
        let original = crate::parse_text(&full).unwrap();
        assert_eq!(blocks[0].source, original[1].source);
        assert_eq!(blocks[1].source, original[2].source);
        let BlockKind::Paragraph(after) = &blocks[1].kind else {
            panic!("following links")
        };
        // The whole-source parser can itself exhaust its expansion allowance
        // after the repeated large URL. A bounded page context must still retain
        // the following ordinary link and the inert data-link caption.
        assert_eq!(after.plain, "After download and Guide.");
        assert!(after.markup.contains("href=\"guide.md\""));
        assert!(!after.markup.contains("data:"));
        let BlockKind::Paragraph(caption) = &blocks[0].kind else {
            panic!("caption")
        };
        assert_eq!(caption.plain, "caption 中文");
        assert_eq!(
            caption.images[0].source,
            ImageSource::Document { page: 7, image: 0 }
        );
        assert_eq!(
            crate::source_projection::image_with_references(body, 0, Some(&lookup))
                .unwrap()
                .bytes
                .len(),
            120000
        );
    }
    #[test]
    fn continuation_table_header_and_rows_resolve_definitions_at_original_lines() {
        let header = "| [Guide][target] | Evidence |\n|---|---|\n";
        let rows = (0..200)
            .map(|i| format!("| Row {i} | [details][target] |\n"))
            .collect::<Vec<_>>();
        let full = format!("{header}{}\n[target]: guide.md:9\n", rows.concat());
        let defs = definitions(&full);
        let lookup = |label: &str| defs.get(&Key::new(label.into())).cloned();
        let body = rows[150..152].concat();
        let start = header.len() + rows[..150].concat().len();
        let source = crate::source_projection::project_with_references(
            window(&body, start, 153),
            9,
            Some(&lookup),
        )
        .unwrap();
        let mut input = fragment(source);
        input.header = Some(
            crate::source_projection::project_with_references(
                window(header, 0, 1),
                0,
                Some(&lookup),
            )
            .unwrap(),
        );
        input.metadata.table = Some(0);
        input.metadata.row_start = 150;
        input.metadata.rows = 2;
        let blocks = crate::parse(&input).unwrap();
        let complete = crate::parse_text(&full).unwrap();
        let (
            BlockKind::Table { header, rows, .. },
            BlockKind::Table {
                header: all_header,
                rows: all_rows,
                ..
            },
        ) = (&blocks[0].kind, &complete[0].kind)
        else {
            panic!("table")
        };
        assert_eq!(header, all_header);
        assert_eq!(rows, &all_rows[150..152]);
        assert!(header[0].content.markup.contains("guide.md:9"));
        assert_eq!(header[0].source.line, 1);
        assert_eq!(rows[0].cells[1].source.line, 153);
    }
    #[test]
    fn context_budgets_conflicts_and_dangerous_reference_links_remain_inert() {
        let mut d = Definition {
            label: "key".into(),
            destination: "javascript:alert(1)".into(),
            title: String::new(),
        };
        let mut source = window("[safe label][key]", 0, 1);
        source.references.push(d.clone());
        let blocks = crate::parse(&fragment(source)).unwrap();
        let BlockKind::Paragraph(p) = &blocks[0].kind else {
            panic!("paragraph")
        };
        assert_eq!(p.plain, "safe label");
        assert!(!p.markup.contains("href="));
        let first = d.clone();
        d.destination = "other".into();
        assert!(context([&first, &d].into_iter()).is_err());
        d.destination = "x".repeat(8193);
        assert!(context([&d].into_iter()).is_err());
    }
}
