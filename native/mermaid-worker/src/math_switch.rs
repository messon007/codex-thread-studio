//! Preserve generated sequence-actor math before Merman's native SVG lowering.
//!
//! Merman 0.9.3 emits a math HTML branch and a literal SVG text branch in a
//! switch. Its resvg preset deliberately prefers the existing SVG fallback,
//! discarding the generated formula. Only promote proven native math actors;
//! ordinary/unsupported labels retain the original upstream behavior.
use merman::{OperationControl, OperationPhase, svg};
use roxmltree::{Document, Node, ParsingOptions};
use std::{borrow::Cow, ops::Range};

const SVG: &str = "http://www.w3.org/2000/svg";
const LIMIT: usize = 4 * 1024 * 1024;

pub struct PreferMathSwitches(pub OperationControl);

fn class(node: Node<'_, '_>, value: &str) -> bool {
    node.attribute("class")
        .is_some_and(|classes| classes.split_ascii_whitespace().any(|c| c == value))
}

fn error(message: impl Into<String>) -> svg::RenderError {
    svg::RenderError::SvgPostprocess {
        pass: "prefer-native-math".into(),
        message: message.into(),
    }
}

impl PreferMathSwitches {
    fn checkpoint(&self) -> svg::RenderResult<()> {
        self.0.checkpoint_at(OperationPhase::Postprocess)?;
        Ok(())
    }

    fn rewrite<'a>(&self, source: Cow<'a, str>) -> svg::RenderResult<Cow<'a, str>> {
        self.checkpoint()?;
        if source.len() > LIMIT {
            return Err(error("SVG exceeds 4 MiB"));
        }
        if !source.contains("<switch") || !source.contains("merman-native-math-svg") {
            return Ok(source);
        }
        // No DTD/entity resolver. The parser is bounded independently of the
        // upstream structural validator; no diagram can request external input.
        let document = Document::parse_with_options(
            &source,
            ParsingOptions {
                nodes_limit: 131_072,
                ..Default::default()
            },
        )
        .map_err(|e| error(e.to_string()))?;
        self.checkpoint()?;
        // Derive subtree facts once in reverse document order. Repeated
        // descendant searches on nested SVG/switch elements can be quadratic.
        let mut facts = vec![[false; 3]; document.descendants().len()];
        for (index, node) in document.descendants().rev().enumerate() {
            if index % 128 == 0 {
                self.checkpoint()?;
            }
            let id = node.id().get_usize();
            facts[id][0] |= node.has_tag_name((SVG, "path"));
            facts[id][1] |= node.has_tag_name((SVG, "svg"))
                && class(node, "merman-native-math-svg")
                && node.attribute("viewBox").is_some()
                && facts[id][0];
            facts[id][2] |= node.is_text() && node.text().is_some_and(|s| s.contains("$$"));
            if let Some(parent) = node.parent() {
                let values = facts[id];
                let parent = parent.id().get_usize();
                for (slot, value) in facts[parent].iter_mut().zip(values) {
                    *slot |= value;
                }
            }
        }
        let mut edits: Vec<(Range<usize>, &str)> = Vec::new();
        for (index, node) in document.descendants().enumerate() {
            if index % 128 == 0 {
                self.checkpoint()?;
            }
            if !node.has_tag_name((SVG, "switch")) || node.attributes().len() != 0 {
                continue;
            }
            let mut children = node.children().filter(Node::is_element);
            let Some(html) = children.next() else {
                continue;
            };
            // Mermaid emits one SVG fallback text element per actor-label line.
            let fallbacks: Vec<_> = children.collect();
            if !html.has_tag_name((SVG, "foreignObject"))
                || fallbacks.is_empty()
                || !fallbacks
                    .iter()
                    .all(|n| n.has_tag_name((SVG, "text")) && class(*n, "actor"))
                || !fallbacks.iter().any(|n| facts[n.id().get_usize()][2])
            {
                continue;
            }
            if !facts[html.id().get_usize()][1] {
                continue;
            }
            // Neutralize switch selection, retain its complete HTML/math branch,
            // and let the original resvg preset perform conversion/validation.
            // No attributes are dropped or synthesized on authored elements.
            let range = node.range();
            let open_end = range.start
                + source[range.clone()]
                    .find('>')
                    .ok_or_else(|| error("Missing switch opening tag"))?
                + 1;
            let close_start = range.start
                + source[range.clone()]
                    .rfind("</")
                    .ok_or_else(|| error("Missing switch closing tag"))?;
            edits.push((range.start..open_end, "<g>"));
            edits.extend(fallbacks.into_iter().map(|node| (node.range(), "")));
            edits.push((close_start..range.end, "</g>"));
        }
        self.checkpoint()?;
        if edits.is_empty() {
            return Ok(source);
        }
        edits.sort_unstable_by_key(|edit| edit.0.start);
        let mut output = String::with_capacity(source.len());
        let mut cursor = 0;
        for (range, replacement) in edits {
            self.checkpoint()?;
            if range.start < cursor {
                return Err(error("Overlapping math switches"));
            }
            output.push_str(&source[cursor..range.start]);
            output.push_str(replacement);
            cursor = range.end;
        }
        output.push_str(&source[cursor..]);
        Ok(Cow::Owned(output))
    }
}

impl svg::SvgPostprocessor for PreferMathSwitches {
    fn name(&self) -> &'static str {
        "prefer-native-math"
    }
    fn process<'a>(
        &self,
        source: Cow<'a, str>,
        _context: &svg::SvgPostprocessContext<'_>,
    ) -> svg::RenderResult<Cow<'a, str>> {
        self.rewrite(source)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use merman::{RenderOutput, RenderRequest, Renderer, SvgEnvironment, SvgRequest};

    fn fixture(math: &str, text: &str, attributes: &str) -> String {
        format!(
            r#"<svg xmlns="{SVG}"><switch{attributes}><foreignObject><div xmlns="http://www.w3.org/1999/xhtml">{math}</div></foreignObject><text class="actor actor-box">{text}</text></switch></svg>"#
        )
    }
    const MATH: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" class="merman-native-math-svg" viewBox="0 0 1 1"><path d="M0 0H1V1Z"/></svg>"#;

    #[test]
    fn only_generated_math_actors_are_promoted_without_changing_other_fallbacks() {
        let adapter = PreferMathSwitches(OperationControl::new());
        let source = fixture(MATH, "$$x^2$$", "");
        let result = adapter.rewrite(Cow::Borrowed(&source)).unwrap();
        assert!(!result.contains("<switch") && !result.contains("<text"));
        assert!(result.contains(MATH));
        for source in [
            fixture("literal $$bad$$", "$$bad$$", ""),
            fixture(MATH, "literal", ""),
            fixture(MATH, "$$x$$", " systemLanguage=\"en\""),
            fixture(MATH, "$$x$$", "").replace("class=\"actor actor-box\"", "class=\"other\""),
            fixture(MATH, "$$x$$", "").replace(SVG, "urn:untrusted"),
        ] {
            assert!(matches!(
                adapter.rewrite(Cow::Borrowed(&source)).unwrap(),
                Cow::Borrowed(_)
            ));
        }
    }

    #[test]
    fn budgets_cancellation_and_entities_do_not_bypass_the_adapter() {
        let control = OperationControl::new();
        let adapter = PreferMathSwitches(control.clone());
        assert!(adapter.rewrite(Cow::Owned("x".repeat(LIMIT + 1))).is_err());
        let source = fixture(MATH, "$$x$$", "");
        let with_dtd = format!("<!DOCTYPE svg [<!ENTITY injected 'bad'>]>{source}");
        assert!(adapter.rewrite(Cow::Borrowed(&with_dtd)).is_err());
        let crowded = source.replace("<switch>", &format!("{}<switch>", "<g/>".repeat(131_072)));
        assert!(adapter.rewrite(Cow::Borrowed(&crowded)).is_err());
        control.cancel();
        assert!(matches!(
            adapter.rewrite(Cow::Borrowed(&source)),
            Err(svg::RenderError::Cancelled(_))
        ));
    }

    #[test]
    fn promotion_does_not_bypass_native_external_resource_validation() {
        for uri in [
            "file:///tmp/native-math-private.svg",
            "https://example.invalid/private.svg",
        ] {
            let control = OperationControl::new();
            let mut pipeline = svg::SvgPipeline::resvg_safe();
            pipeline.push_postprocessor(PreferMathSwitches(control));
            let session = merman_render::environment::RenderEnvironment::deterministic()
                .begin_session()
                .unwrap();
            let source = fixture(
                &MATH.replace("</svg>", &format!("<use href=\"{uri}\"/></svg>")),
                "$$x$$",
                "",
            );
            if let Ok(svg) = pipeline.process_to_string(&source, &session) {
                assert!(!svg.contains(uri), "Unsafe structural resource survived");
            }
        }
    }

    #[test]
    fn actual_sequence_actor_and_message_outlines_survive_native_lowering() {
        for (label, count) in [("$$x^2$$", 3), ("$$x^2$$<br/>$$y^2$$", 5)] {
            let control = OperationControl::new();
            let mut pipeline = svg::SvgPipeline::resvg_safe();
            pipeline.push_postprocessor(PreferMathSwitches(control.clone()));
            let source =
                format!("sequenceDiagram\nparticipant A as {label}\nA->>B: $$\\frac{{a}}{{b}}$$");
            let result = Renderer::new()
                .render(RenderRequest::svg(
                    &source,
                    control,
                    SvgRequest {
                        environment: SvgEnvironment::deterministic().with_compiled_math_renderer(),
                        pipeline: Some(pipeline),
                        ..Default::default()
                    },
                ))
                .unwrap();
            let RenderOutput::Svg(Some(output)) = result else {
                panic!("Missing SVG")
            };
            let svg = output.svg();
            assert_eq!(
                svg.matches("class=\"merman-native-math-svg\"").count(),
                count
            );
            assert!(!svg.contains("<foreignObject"));
            let tree = Document::parse(svg).unwrap();
            assert!(
                !tree
                    .descendants()
                    .filter(|n| n.has_tag_name((SVG, "text")))
                    .any(|n| n
                        .descendants()
                        .filter_map(|n| n.text())
                        .any(|s| s.contains("$$")))
            );
        }
    }
}
