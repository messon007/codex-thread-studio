//! Presentation-only Mermaid defaults, part of every native raster cache key.
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
#[derive(Clone, Debug, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Style {
    pub font_family: String,
    pub font_family_mono: String,
    pub font_size: u8,
    pub density: String,
    pub curve: String,
    #[serde(default = "auto")]
    pub style: String,
    #[serde(default = "auto")]
    pub layout: String,
}
fn auto() -> String {
    "auto".into()
}
impl Default for Style {
    fn default() -> Self {
        Self {
            font_family: "sans-serif".into(),
            font_family_mono: "monospace".into(),
            font_size: 15,
            density: "standard".into(),
            curve: "rounded".into(),
            style: auto(),
            layout: auto(),
        }
    }
}
impl Style {
    pub fn validate(&self) -> Result<(), String> {
        if !(12..=20).contains(&self.font_size)
            || !["compact", "standard", "loose"].contains(&self.density.as_str())
            || !["rounded", "linear", "step", "basis"].contains(&self.curve.as_str())
            || !["auto", "classic", "neo", "handDrawn", "document"].contains(&self.style.as_str())
            || !["auto", "dagre", "elk"].contains(&self.layout.as_str())
            || [&self.font_family, &self.font_family_mono]
                .iter()
                .any(|s| s.is_empty() || s.len() > 8192 || s.chars().any(char::is_control))
        {
            return Err("Invalid native Mermaid style".into());
        }
        Ok(())
    }
    pub fn options(&self) -> Value {
        let (nodes, ranks, outer, inner) = match self.density.as_str() {
            "compact" => (24, 32, 8, 10),
            "loose" => (56, 70, 18, 20),
            _ => (38, 48, 12, 15),
        };
        json!({"fontFamily":self.font_family,"fontFamilyMono":self.font_family_mono,"fontSize":self.font_size,"padding":outer,"config":{"flowchart":{"nodeSpacing":nodes,"rankSpacing":ranks,"diagramPadding":outer,"padding":inner,"curve":self.curve}}})
    }
    pub fn heap_bytes(&self) -> usize {
        self.font_family.capacity()
            + self.font_family_mono.capacity()
            + self.density.capacity()
            + self.curve.capacity()
            + self.style.capacity()
            + self.layout.capacity()
            + std::mem::size_of::<Self>()
    }
}
