//! Inspect actual native SVG output, including colors authored inside TeX.
#[path = "../../../../native/mermaid-worker/src/math_switch.rs"]
mod math_switch;

use merman::{OperationControl, RenderOutput, RenderRequest, Renderer, SvgEnvironment, SvgRequest};
use roxmltree::{Document, Node};
use serde_json::{Value, json};
use std::collections::BTreeSet;

fn render(source: &str, theme: Value) -> String {
    let control = OperationControl::new();
    let mut pipeline = merman::svg::SvgPipeline::resvg_safe();
    pipeline.push_postprocessor(math_switch::PreferMathSwitches(control.clone()));
    let renderer = Renderer::new().with_engine(
        merman::Engine::new().with_site_config(merman::MermaidConfig::from_value(theme)),
    );
    let output = renderer
        .render(RenderRequest::svg(
            source,
            control,
            SvgRequest {
                environment: SvgEnvironment::deterministic().with_compiled_math_renderer(),
                pipeline: Some(pipeline),
                ..Default::default()
            },
        ))
        .unwrap();
    let RenderOutput::Svg(Some(output)) = output else {
        panic!("Missing SVG")
    };
    output.svg().to_owned()
}

fn color(value: &str) -> (u8, u8, u8) {
    let c = merman::theme_color::ThemeColor::parse(value)
        .unwrap()
        .rgba_channels();
    (
        c.red.round() as u8,
        c.green.round() as u8,
        c.blue.round() as u8,
    )
}

fn math(node: Node<'_, '_>) -> bool {
    node.tag_name().name() == "svg"
        && node.attribute("class").is_some_and(|c| {
            c.split_ascii_whitespace()
                .any(|c| c == "merman-native-math-svg")
        })
}

fn paints(svg: &str) -> Vec<BTreeSet<(u8, u8, u8)>> {
    let doc = Document::parse(svg).unwrap();
    doc.descendants()
        .filter(|n| n.is_element() && math(*n))
        .map(|n| {
            n.descendants()
                .filter(|n| n.has_tag_name("path"))
                .filter_map(|n| n.attribute("fill"))
                .filter(|c| *c != "none")
                .map(color)
                .collect()
        })
        .collect()
}

#[test]
fn mixed_actor_prose_has_the_same_foreground_as_the_formula() {
    let svg = render(
        "sequenceDiagram\nparticipant A as Input $$x^2$$\nparticipant B as Output $$y^2$$\nA->>B: done",
        json!({"theme":"default","look":"classic","htmlLabels":true}),
    );
    let doc = Document::parse(&svg).unwrap();
    let prose: Vec<_> = doc
        .descendants()
        .filter(|n| n.has_tag_name("text"))
        .filter(|n| matches!(n.text().map(str::trim), Some("Input" | "Output")))
        .collect();
    assert_eq!(prose.len(), 4);
    for n in prose {
        assert_eq!(color(n.attribute("fill").unwrap()), (0, 0, 0));
    }
    let formulas = paints(&svg);
    assert_eq!(formulas.len(), 4);
    assert!(
        formulas
            .iter()
            .all(|colors| colors == &BTreeSet::from([(0, 0, 0)]))
    );
}

#[test]
fn dark_defaults_are_visible_and_explicit_tex_black_and_red_are_preserved() {
    let svg = render(
        r"sequenceDiagram
participant A as $$x+\color{red}{y}+\color{black}{z}$$
A->>B: $$x^2$$",
        json!({"theme":"dark","look":"classic","htmlLabels":true}),
    );
    let formulas = paints(&svg);
    assert_eq!(formulas.len(), 3);
    assert_eq!(
        formulas
            .iter()
            .filter(|c| c.contains(&(255, 0, 0))
                && c.contains(&(0, 0, 0))
                && c.contains(&(211, 211, 211)))
            .count(),
        2
    );
    assert_eq!(
        formulas
            .iter()
            .filter(|c| c == &&BTreeSet::from([(211, 211, 211)]))
            .count(),
        1
    );
}

#[test]
fn actor_note_loop_and_signal_use_their_own_authored_role_colors() {
    let svg = render(
        r"sequenceDiagram
participant A as $$x^2$$
note over A: $$x^2$$
loop $$x^2$$
A->>B: $$x^2$$
end",
        json!({"theme":"base","look":"classic","htmlLabels":true,"themeVariables":{
            "actorTextColor":"#00ff00","noteTextColor":"#ff00ff","loopTextColor":"#00ffff","signalTextColor":"#0000ff"
        }}),
    );
    let formulas = paints(&svg);
    assert_eq!(formulas.len(), 5);
    for (value, count) in [
        ((0, 255, 0), 2),
        ((255, 0, 255), 1),
        ((0, 255, 255), 1),
        ((0, 0, 255), 1),
    ] {
        assert_eq!(
            formulas
                .iter()
                .filter(|c| c == &&BTreeSet::from([value]))
                .count(),
            count,
            "{value:?}"
        );
    }
}

#[test]
fn flowchart_dark_theme_and_node_class_color_reach_the_layout_engine() {
    let svg = render(
        r#"flowchart LR
A["$$x^2$$"]:::custom --> B["$$y^2$$"]
classDef custom color:#00ff00;"#,
        json!({"theme":"dark","look":"classic","htmlLabels":true}),
    );
    let formulas = paints(&svg);
    assert_eq!(formulas.len(), 2);
    assert!(formulas.iter().any(|c| c == &BTreeSet::from([(0, 255, 0)])));
    assert!(
        formulas
            .iter()
            .any(|c| c.iter().all(|(r, g, b)| *r > 128 && *g > 128 && *b > 128))
    );
}

#[test]
fn edge_label_style_changes_default_ink_without_overriding_tex_color() {
    for (formula, expected) in [("x^2", (0, 0, 255)), (r"\color{black}{x}", (0, 0, 0))] {
        // Upstream Mermaid preprocessing treats a linkStyle hex followed by ';'
        // as an entity. Newline-terminated hex and named CSS colors avoid that.
        for style in ["color:#0000ff", "color:blue;", "color:blue !important;"] {
            let source = format!("flowchart LR\nA -->|\"$${formula}$$\"| B\nlinkStyle 0 {style}");
            let svg = render(
                &source,
                json!({"theme":"dark","look":"classic","htmlLabels":true}),
            );
            let formulas = paints(&svg);
            assert_eq!(formulas.len(), 1, "{style}");
            assert_eq!(formulas[0], BTreeSet::from([expected]), "{style}");
        }
    }
}

#[test]
fn invalid_edge_color_keeps_math_with_visible_theme_ink() {
    let svg = render(
        "flowchart LR\nA -->|\"$$x^2$$\"| B\nlinkStyle 0 color:notAColor;",
        json!({"theme":"dark","look":"classic","htmlLabels":true}),
    );
    let formulas = paints(&svg);
    assert_eq!(formulas.len(), 1);
    assert!(
        formulas[0]
            .iter()
            .all(|(r, g, b)| *r > 128 && *g > 128 && *b > 128)
    );
}

#[test]
fn translucent_default_math_ink_retains_its_alpha() {
    let svg = render(
        "sequenceDiagram\nparticipant A as $$x^2$$\nA->>B: done",
        json!({"theme":"base","look":"classic","themeVariables":{"actorTextColor":"rgba(0,255,0,0.5)"}}),
    );
    let doc = Document::parse(&svg).unwrap();
    let mut paths = 0;
    for node in doc.descendants().filter(|n| n.is_element() && math(*n)) {
        for path in node.descendants().filter(|n| n.has_tag_name("path")) {
            if let Some(fill) = path.attribute("fill").filter(|c| *c != "none") {
                let c = merman::theme_color::ThemeColor::parse(fill)
                    .unwrap()
                    .rgba_channels();
                assert!((c.alpha - 0.5).abs() < 0.0001);
                paths += 1;
            }
        }
    }
    assert!(paths > 0);
}
