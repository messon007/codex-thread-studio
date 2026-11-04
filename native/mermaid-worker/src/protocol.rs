//! Shared with the Rust 1.92 parent. Keep this module GTK/engine-independent.
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const VERSION: u8 = 3;
pub const SOURCE_LIMIT: usize = 128 * 1024;
pub const INPUT_LIMIT: usize = 800 * 1024;
pub const SVG_LIMIT: usize = 4 * 1024 * 1024;
pub const FONT_LIMIT: usize = 24;
pub const FONT_BYTES: u64 = 64 * 1024 * 1024;

// Fixed worker-owned SVG header, not a parser for arbitrary authored XML. The
// count is taken from generated native math SVGs before glyph outlining.
pub const SVG_HEADER: &str = "<svg data-studio-math-labels=\"";
pub fn math_labels(svg: &str) -> Result<usize, String> {
    let count = svg
        .strip_prefix(SVG_HEADER)
        .and_then(|rest| rest.split_once("\" "))
        .map(|(count, _)| count)
        .filter(|count| {
            !count.is_empty() && count.len() <= 6 && count.bytes().all(|b| b.is_ascii_digit())
        })
        .and_then(|count| count.parse::<usize>().ok())
        .filter(|count| *count <= 131_072);
    count.ok_or_else(|| "Mermaid returned invalid SVG metadata".into())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Request {
    pub version: u8,
    pub source: String,
    pub dark: bool,
    #[serde(default)]
    pub style: Style,
    #[serde(default)]
    pub font_defaults: Option<FontDefaults>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FontDefaults {
    pub sans: String,
    pub mono: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FontPlan {
    pub version: u8,
    pub primary: String,
    pub mono: String,
    pub families: Vec<String>,
    pub characters: Vec<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields, default)]
pub struct Style {
    pub font_family: String,
    pub font_family_mono: String,
    pub font_size: u8,
    pub density: String,
    pub curve: String,
    pub style: String,
    pub layout: String,
}
impl Default for Style {
    fn default() -> Self {
        Self {
            font_family: "DejaVu Sans".into(),
            font_family_mono: "DejaVu Sans Mono".into(),
            font_size: 15,
            density: "standard".into(),
            curve: "rounded".into(),
            style: "auto".into(),
            layout: "auto".into(),
        }
    }
}
impl Request {
    pub fn validate(&self) -> Result<(), String> {
        if self.version != VERSION {
            return Err("Unsupported native Mermaid protocol version".into());
        }
        if self.source.is_empty() || self.source.len() > SOURCE_LIMIT || self.source.contains('\0')
        {
            return Err("Mermaid source is empty, invalid or exceeds 128 KiB".into());
        }
        let s = &self.style;
        if self.font_defaults.as_ref().is_some_and(|f| {
            [&f.sans, &f.mono]
                .iter()
                .any(|s| s.is_empty() || s.len() > 8192 || s.chars().any(char::is_control))
        }) {
            return Err("Invalid native Mermaid generic font defaults".into());
        }
        if !(12..=20).contains(&s.font_size)
            || !["compact", "standard", "loose"].contains(&s.density.as_str())
            || !["rounded", "linear", "step", "basis"].contains(&s.curve.as_str())
            || !["auto", "classic", "neo", "handDrawn", "document"].contains(&s.style.as_str())
            || !["auto", "dagre", "elk"].contains(&s.layout.as_str())
            || [&s.font_family, &s.font_family_mono]
                .iter()
                .any(|s| s.is_empty() || s.len() > 8192 || s.chars().any(char::is_control))
        {
            return Err("Invalid native Mermaid presentation options".into());
        }
        Ok(())
    }
    pub fn config(&self) -> Value {
        let s = &self.style;
        let (theme, look) = match s.style.as_str() {
            "classic" => (if self.dark { "dark" } else { "default" }, "classic"),
            "handDrawn" => (if self.dark { "dark" } else { "neutral" }, "handDrawn"),
            "document" => ("neutral", "classic"),
            _ => (if self.dark { "neo-dark" } else { "neo" }, "neo"),
        };
        let (nodes, ranks, outer, inner) = match s.density.as_str() {
            "compact" => (24, 32, 8, 10),
            "loose" => (56, 70, 18, 20),
            _ => (38, 48, 12, 15),
        };
        // Match Studio's web preferences, including neutral Document in dark UI.
        // Auto leaves layout to the engine, as the existing web configuration does.
        let mut config = serde_json::json!({"securityLevel":"strict","htmlLabels":true,"theme":theme,"look":look,
            "fontSize":s.font_size,"fontFamily":s.font_family,
            "flowchart":{"nodeSpacing":nodes,"rankSpacing":ranks,"diagramPadding":outer,
                "padding":inner,"curve":s.curve}});
        if s.layout != "auto" {
            config["layout"] = serde_json::json!(s.layout);
        }
        if s.style == "handDrawn" {
            config["handDrawnSeed"] = serde_json::json!(1);
        }
        config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn svg_metadata_is_fixed_bounded_and_not_inferred_from_source() {
        assert_eq!(
            math_labels("<svg data-studio-math-labels=\"2\" xmlns=\"x\"></svg>").unwrap(),
            2
        );
        for svg in [
            "<svg/>",
            "<svg data-studio-math-labels=\"-1\" />",
            "<svg data-studio-math-labels=\"999999\" />",
            "<svg data-studio-math-labels=\"1x\" />",
            "<svg data-studio-math-labels=\"\" />",
        ] {
            assert!(math_labels(svg).is_err());
        }
    }
    #[test]
    fn strict_envelope_rejects_unknown_and_oversized_content() {
        for source in [
            r#"{"version":1,"source":"flowchart LR","dark":false,"svgPath":"/tmp/no"}"#,
            r#"{"version":1,"source":"flowchart LR","dark":false,"style":{"fontFiles":[]}}"#,
        ] {
            assert!(serde_json::from_str::<Request>(source).is_err());
        }
        let mut r = Request {
            version: VERSION,
            source: "flowchart LR\nA-->B".into(),
            dark: false,
            style: Style::default(),
            font_defaults: None,
        };
        assert!(r.validate().is_ok());
        r.source = "x".repeat(SOURCE_LIMIT + 1);
        assert!(r.validate().is_err());
        r.source = "flowchart LR".into();
        r.version = VERSION + 1;
        assert!(r.validate().is_err());
        r.version = VERSION;
        r.style.font_family = "x\n".into();
        assert!(r.validate().is_err());
        r.style = Style::default();
        r.style.layout = "network".into();
        assert!(r.validate().is_err());
    }
    #[test]
    fn themes_and_layouts_match_existing_preferences() {
        let mut r = Request {
            version: VERSION,
            source: "flowchart LR\nA-->B".into(),
            dark: false,
            style: Style::default(),
            font_defaults: None,
        };
        for (style, dark, theme, look) in [
            ("auto", false, "neo", "neo"),
            ("neo", true, "neo-dark", "neo"),
            ("classic", true, "dark", "classic"),
            ("handDrawn", false, "neutral", "handDrawn"),
            ("document", true, "neutral", "classic"),
        ] {
            r.style.style = style.into();
            r.dark = dark;
            assert!(r.validate().is_ok());
            let c = r.config();
            assert_eq!(c["theme"], theme);
            assert_eq!(c["look"], look);
        }
        r.style.layout = "elk".into();
        assert_eq!(r.config()["layout"], "elk");
        r.style.layout = "auto".into();
        assert!(r.config().get("layout").is_none());
    }
}
