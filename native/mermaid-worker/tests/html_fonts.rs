use merman_render::{svg::foreign_object_label_fallback_svg_text, text::DeterministicTextMeasurer};

fn family(sheet: &str, inner: &str) -> String {
    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" style="font-family:DejaVu Sans"><style>{sheet}</style><foreignObject width="500" height="30"><div xmlns="http://www.w3.org/1999/xhtml">{inner}</div></foreignObject></svg>"#
    );
    let output =
        foreign_object_label_fallback_svg_text(&svg, &DeterministicTextMeasurer::default());
    let doc = roxmltree::Document::parse(&output).unwrap();
    let text = doc
        .descendants()
        .find(|node| {
            node.has_tag_name("text")
                && node.attribute("class") == Some("merman-foreignobject-fallback-text")
        })
        .unwrap_or_else(|| panic!("native fallback text missing: {output}"));
    let style = text.attribute("style").unwrap();
    simplecss::DeclarationTokenizer::from(style)
        .find(|declaration| declaration.name == "font-family")
        .unwrap()
        .value
        .to_owned()
}

#[test]
fn intrinsic_code_face_survives_descendant_inheritance() {
    for inner in [
        "<code>iiii WWW 0123</code>",
        "<code><span>iiii WWW 0123</span></code>",
        "<code style='font-family:;'>iiii WWW 0123</code>",
    ] {
        assert_eq!(family("", inner), "monospace", "{inner}");
    }
    assert_eq!(family("", "<span>iiii WWW 0123</span>"), "DejaVu Sans");
}

#[test]
fn author_code_font_declarations_override_intrinsic_face() {
    for (sheet, inner, expected) in [
        (
            "code{font-family:Inter}",
            "<code>iiii WWW 0123</code>",
            "Inter",
        ),
        (
            "code{font-family:Inter!important}",
            "<code style='font-family:monospace'>iiii WWW 0123</code>",
            "Inter",
        ),
        (
            "",
            "<code style='font-family:inherit'>iiii WWW 0123</code>",
            "DejaVu Sans",
        ),
        (
            "",
            "<code style='font-family:inherit /* kept */'>iiii WWW 0123</code>",
            "DejaVu Sans",
        ),
        (
            "",
            "<code style='font-family:initial'>iiii WWW 0123</code>",
            "trebuchet ms,verdana,arial,sans-serif",
        ),
    ] {
        assert_eq!(family(sheet, inner), expected, "{sheet}: {inner}");
    }
}

#[test]
fn wrapped_repeated_words_and_entities_keep_their_own_fonts() {
    for inner in [
        "same <code>same</code> same",
        "same <code>sa&#109;e</code> same",
    ] {
        let svg = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" style="font-family:DejaVu Sans"><foreignObject width="90" height="100"><div xmlns="http://www.w3.org/1999/xhtml" style="width:90px;white-space:normal">{inner}</div></foreignObject></svg>"#
        );
        let measurer = DeterministicTextMeasurer::default().with_width_callback(|text, style| {
            let unit = if style.font_family.as_deref() == Some("monospace") {
                20.0
            } else {
                10.0
            };
            text.chars().count() as f64 * unit
        });
        let output = foreign_object_label_fallback_svg_text(&svg, measurer.as_ref());
        let doc = roxmltree::Document::parse(&output).unwrap();
        let runs: Vec<_> = doc
            .descendants()
            .filter(|n| {
                n.has_tag_name("text")
                    && n.attribute("class") == Some("merman-foreignobject-fallback-text")
            })
            .flat_map(|n| n.children().filter(|n| n.has_tag_name("tspan")))
            .map(|n| {
                let family = simplecss::DeclarationTokenizer::from(n.attribute("style").unwrap())
                    .find(|d| d.name == "font-family")
                    .unwrap()
                    .value;
                (n.text().unwrap_or(""), family)
            })
            .collect();
        assert_eq!(
            runs,
            [
                ("same", "DejaVu Sans"),
                ("same", "monospace"),
                ("same", "DejaVu Sans")
            ],
            "{output}"
        );
    }
}
