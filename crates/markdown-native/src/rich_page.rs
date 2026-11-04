//! One render-ready rich page from already indexed attributes. No Markdown or
//! markup parse, whole-paragraph copy, or source-coordinate rebasing is needed.
use crate::{
    inline::{Attributes, Math, Page},
    RichText,
};
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct Source {
    pub attributes: Arc<Attributes>,
    pub math: Arc<[Math]>,
}
impl Source {
    /// Cheap bounded metadata handoff; actual page work belongs on a worker.
    pub fn new(content: &RichText) -> Option<Self> {
        if content.source_only {
            return None;
        }
        Some(Self {
            attributes: content.attributes.clone()?,
            math: Arc::from(content.math.clone()),
        })
    }
    pub fn page(&self, index: usize) -> Result<RichText, String> {
        let part = self
            .attributes
            .pages
            .get(index)
            .ok_or("Missing rich text page")?;
        let text = self
            .attributes
            .text
            .get(part.start..part.end)
            .ok_or("Invalid rich page boundaries")?;
        if text.len() > crate::flow::RICH_TEXT_BYTES {
            return Err("Rich page exceeds the text budget".into());
        }
        if text.chars().count() != part.characters {
            return Err("Invalid rich page character count".into());
        }
        let mut attrs = Attributes::default();
        attrs.text = text.to_owned();
        attrs.carriage_returns = text.contains('\r');
        for &id in &part.spans {
            let mut span = self
                .attributes
                .spans
                .get(id)
                .ok_or("Invalid rich page style")?
                .clone();
            span.start = span
                .start
                .saturating_sub(part.character)
                .min(part.characters);
            span.end = span.end.saturating_sub(part.character).min(part.characters);
            if span.start < span.end {
                attrs.spans.push(span);
            }
        }
        for &id in &part.links {
            let mut link = self
                .attributes
                .links
                .get(id)
                .ok_or("Invalid rich page link")?
                .clone();
            link.start = link
                .start
                .saturating_sub(part.character)
                .min(part.characters);
            link.end = link.end.saturating_sub(part.character).min(part.characters);
            if link.start < link.end {
                attrs.links.push(link);
            }
        }
        let mut math = Vec::new();
        let start = self.math.partition_point(|m| m.range.end <= part.start);
        for m in self.math[start..]
            .iter()
            .take_while(|m| m.range.start < part.end)
        {
            if m.range.start < part.start
                || m.range.end > part.end
                || m.body.start < m.range.start
                || m.body.end > m.range.end
                || m.characters.start < part.character
                || m.characters.end > part.character + part.characters
            {
                return Err("Rich page splits a formula".into());
            }
            let mut m = m.clone();
            m.range = m.range.start - part.start..m.range.end - part.start;
            m.body = m.body.start - part.start..m.body.end - part.start;
            m.characters = m.characters.start - part.character..m.characters.end - part.character;
            attrs.multiline_math |= text
                .get(m.body.clone())
                .ok_or("Invalid rich page formula")?
                .contains(['\r', '\n', '\u{2028}', '\u{2029}']);
            math.push(m);
        }
        attrs.pages.push(Page {
            start: 0,
            end: text.len(),
            character: 0,
            characters: part.characters,
            spans: (0..attrs.spans.len()).collect(),
            links: (0..attrs.links.len()).collect(),
            lines: part.lines,
        });
        Ok(RichText {
            source_only: false,
            plain: text.to_owned(),
            markup: String::new(),
            images: Vec::new(),
            math,
            attributes: Some(Arc::new(attrs)),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn paragraph(input: &str) -> RichText {
        crate::parse_text(input)
            .unwrap()
            .into_iter()
            .find_map(|b| match b.kind {
                crate::BlockKind::Paragraph(c) => Some(c),
                _ => None,
            })
            .unwrap()
    }
    #[test]
    fn atomic_pages_keep_crlf_unicode_styles_links_and_original_math_sources() {
        let input = format!(
            "**{}字😀 $\\frac{{\r\na}}{{b}}$ [after](docs/a.md) {} $x^2$ tail**",
            "word ".repeat(1636),
            "context ".repeat(2500)
        );
        let rich = paragraph(&input);
        let source = Source::new(&rich).unwrap();
        assert!(source.attributes.pages.len() > 3);
        let mut joined = String::new();
        let mut formulas = Vec::new();
        let mut links = 0;
        for (i, part) in source.attributes.pages.iter().enumerate() {
            let page = source.page(i).unwrap();
            assert!(page.plain.len() <= crate::inline::PAGE_BYTES);
            assert_eq!(page.plain, rich.plain[part.start..part.end]);
            assert!(page
                .attributes
                .as_ref()
                .unwrap()
                .spans
                .iter()
                .any(|s| s.style == crate::inline::Style::Bold));
            links += page.attributes.as_ref().unwrap().links.len();
            for m in &page.math {
                assert_eq!(
                    page.plain[..m.range.start].chars().count(),
                    m.characters.start
                );
                formulas.push(m.source.clone());
            }
            let projected = crate::flow::project_rich(page.clone()).unwrap();
            assert_eq!(
                projected
                    .quick
                    .as_ref()
                    .unwrap()
                    .select(0..projected.characters, crate::flow::RICH_TEXT_BYTES),
                Some(page.plain.as_str())
            );
            joined.push_str(&page.plain);
        }
        assert_eq!(joined, rich.plain);
        assert_eq!(links, 1);
        assert_eq!(
            formulas,
            rich.math
                .iter()
                .map(|m| m.source.clone())
                .collect::<Vec<_>>()
        );
        let at = input.find("after").unwrap();
        let found = source.attributes.project(&input, 0, at..at + 5).unwrap();
        let part = &source.attributes.pages[found.page];
        assert!(
            part.character <= found.characters.start
                && found.characters.end <= part.character + part.characters
        );
        assert_eq!(
            source
                .page(found.page)
                .unwrap()
                .plain
                .chars()
                .skip(found.characters.start - part.character)
                .take(5)
                .collect::<String>(),
            "after"
        );
    }
    #[test]
    fn one_large_formula_gets_its_own_bounded_page_without_splitting() {
        let rich = paragraph(&format!("prefix ${}$ tail", "x+".repeat(6000)));
        let source = Source::new(&rich).unwrap();
        assert_eq!(source.attributes.pages.len(), 3);
        let page = source.page(1).unwrap();
        assert_eq!(page.math.len(), 1);
        assert!(
            page.plain.len() > crate::inline::PAGE_BYTES
                && page.plain.len() <= crate::flow::RICH_TEXT_BYTES
        );
        assert_eq!(page.math[0].range, 0..page.plain.len());
        assert_eq!(
            source
                .attributes
                .pages
                .iter()
                .enumerate()
                .map(|(i, _)| source.page(i).unwrap().plain)
                .collect::<String>(),
            rich.plain
        );
        assert!(source.page(3).is_err());
    }
}
