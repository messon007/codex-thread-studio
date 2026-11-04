use merman_render::text::{
    DeterministicTextMeasurer, TextStyle, WrapMode, measure_html_with_inline_styles,
};

fn measure(html: &str, width: Option<f64>, family: &str) -> merman_render::text::TextMetrics {
    let measurer = DeterministicTextMeasurer::default().with_width_callback(|text, style| {
        let unit = match style.font_family.as_deref() {
            Some("Wide") => 20.0,
            Some("Narrow") | Some("monospace") => 5.0,
            _ => 10.0,
        };
        text.chars().count() as f64 * unit
    });
    let style = TextStyle {
        font_family: Some(family.to_owned()),
        font_size: 16.0,
        ..Default::default()
    };
    measure_html_with_inline_styles(measurer.as_ref(), html, &style, width, WrapMode::HtmlLike)
}

#[test]
fn named_families_change_wrapping_geometry() {
    let plain = measure("xxxx WWWW", Some(100.0), "Body");
    let mixed = measure(
        "<span style='font-family:Narrow'>xxxx</span> <span style='font-family:Wide'>WWWW</span>",
        Some(100.0),
        "Body",
    );
    assert_eq!((plain.width, plain.line_count), (90.0, 1));
    assert_eq!(
        (mixed.width, mixed.height, mixed.line_count),
        (100.0, 48.0, 2)
    );
}

#[test]
fn nested_families_keep_their_actual_style_groups() {
    let metrics = measure(
        "<span style='font-family:Wide'>AA<span style='font-family:Narrow'>aa</span>BB</span>",
        Some(80.0),
        "Body",
    );
    assert_eq!(
        (metrics.width, metrics.height, metrics.line_count),
        (90.0, 24.0, 1)
    );
}

#[test]
fn explicit_family_inheritance_overrides_code_face_and_priority_is_retained() {
    assert_eq!(measure("<code>xxxx</code>", None, "Wide").width, 20.0);
    assert_eq!(
        measure(
            "<code style='font-family:inherit'>xxxx</code>",
            None,
            "Wide"
        )
        .width,
        80.0
    );
    assert_eq!(
        measure(
            "<code style='font-family:inherit /* retained */'>xxxx</code>",
            None,
            "Wide"
        )
        .width,
        80.0
    );
    assert_eq!(
        measure(
            "<span style='font-family:Wide!important;font-family:Narrow'>xxxx</span>",
            None,
            "Body"
        )
        .width,
        80.0
    );
    assert_eq!(
        measure(
            "<span style='font-family:Wide'><code style='font-family:unset'>xxxx</code></span>",
            None,
            "Body"
        )
        .width,
        80.0
    );
}

#[test]
fn equivalent_family_runs_keep_nonadditive_shaping_across_tags() {
    let measurer = DeterministicTextMeasurer::default().with_width_callback(|text, _| {
        if text == "AVA" {
            10.0
        } else {
            text.chars().count() as f64 * 20.0
        }
    });
    let style = TextStyle {
        font_family: Some("Body".into()),
        ..Default::default()
    };
    let metrics = measure_html_with_inline_styles(
        measurer.as_ref(),
        "<span style='font-family:Wide'>AV<code style='font-family:inherit'>A</code></span>",
        &style,
        None,
        WrapMode::HtmlLike,
    );
    assert_eq!(metrics.width, 10.0);
}

#[test]
fn admission_matches_the_supported_measurement_contract() {
    use merman_render::text::unsupported_html_label_style;
    let control = merman::OperationControl::new();
    let config = merman::MermaidConfig::from_value(
        serde_json::json!({"securityLevel":"strict","htmlLabels":true}),
    );
    let check = |label: &str| {
        unsupported_html_label_style(
            label,
            "text",
            &config,
            &["letter-spacing", "white-space"],
            &control,
        )
        .unwrap()
    };
    assert_eq!(
        check("<span style='font-family:DejaVu Serif'>Hello</span>"),
        None
    );
    assert_eq!(
        check("<code style='font-family:inherit'>Hello</code>"),
        None
    );
    assert_eq!(
        check("<code style='font-family:inherit /* retained */'>Hello</code>"),
        None
    );
    assert_eq!(
        check("<span style='font-size:inherit /* retained */!important'>Hello</span>"),
        None
    );
    assert_eq!(
        check("<span style='font-family:DejaVu Serif!important'>Hello</span>"),
        None
    );
    for label in [
        "<span style='font-family:var(--face)'>Hello</span>",
        "<span style='font-family:DejaVu Serif'>$$x^2$$</span>",
        "<span style='font-family:DejaVu Serif'>Hello<img src='image.png'/></span>",
        "<span style='font-family:DejaVu Serif'><i class='fas fa-check'></i>Hello</span>",
    ] {
        assert_eq!(check(label).as_deref(), Some("font-family"), "{label}");
    }
    assert_eq!(
        check("<span style='font-size:32px'>Hello</span>").as_deref(),
        Some("font-size")
    );
    assert_eq!(
        check("<b style='font-weight:normal'>Hello</b>").as_deref(),
        Some("font-weight")
    );
}
