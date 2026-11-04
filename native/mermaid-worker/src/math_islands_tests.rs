use merman::{OperationControl, RenderOutput, RenderRequest, Renderer, SvgEnvironment, SvgRequest};
use merman_render::math::{MathRenderer, RatexMathRenderer};

fn native_count(html: &str) -> usize {
    let source = format!("<div>{html}</div>");
    let doc = roxmltree::Document::parse(&source).unwrap();
    doc.descendants()
        .filter(|node| {
            node.has_tag_name(("http://www.w3.org/2000/svg", "svg"))
                && node.attribute("class").is_some_and(|c| {
                    c.split_ascii_whitespace()
                        .any(|c| c == "merman-native-math-svg")
                })
        })
        .count()
}

#[test]
fn unsupported_islands_do_not_discard_supported_siblings() {
    let config = merman::MermaidConfig::default();
    for label in [
        r"Before $$x^2$$ and $$\unknownNativeMacro{x}$$ after",
        r"$$x^2$$<br>$$\unknownNativeMacro{x}$$",
    ] {
        let html = RatexMathRenderer
            .render_html_label(label, &config)
            .expect("supported math island was discarded");
        assert_eq!(native_count(&html), 1);
        assert!(html.contains(r"$$\unknownNativeMacro{x}$$"));
    }
}

#[test]
fn unsupported_literal_is_xml_escaped_and_fully_declined_labels_stay_declined() {
    let config = merman::MermaidConfig::default();
    let html = RatexMathRenderer
        .render_html_label(
            r#"$$x^2$$ and $$\unknownNativeMacro{<script>&'"}$$"#,
            &config,
        )
        .expect("mixed math label was discarded");
    assert_eq!(native_count(&html), 1);
    assert!(!html.contains("<script>"));
    assert!(html.contains("&lt;script&gt;"));
    for label in [
        r"$$\unknownNativeMacro{x}$$",
        r"$$\frac{1}{$$",
        "unclosed $$x",
    ] {
        assert!(
            RatexMathRenderer
                .render_html_label(label, &config)
                .is_none()
        );
    }
}

#[test]
fn actual_flow_and_sequence_keep_native_formula_counts_with_literal_neighbors() {
    for (source, count) in [
        (
            r#"flowchart LR
A["Native $$\frac{a}{b}$$"] -->|"$$\sqrt{x+1}$$"| B["Selectable $$x^2$$ and $$\unknownNativeMacro{x}$$"]"#,
            3,
        ),
        (
            r#"sequenceDiagram
participant A as Input $$x^2$$ and $$\unknownNativeMacro{x}$$
A->>B: $$y^2$$"#,
            3,
        ),
    ] {
        let control = OperationControl::new();
        let mut pipeline = merman::svg::SvgPipeline::resvg_safe();
        pipeline.push_postprocessor(crate::math_switch::PreferMathSwitches(control.clone()));
        let output = Renderer::new()
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
        assert_eq!(native_count(output.svg()), count, "{source}");
        assert!(output.svg().contains("unknownNativeMacro"));
    }
}
