//! Validated shared appearance model and rendering plan, prepared on a Rust worker.
use serde::Deserialize;
use serde_json::Value;
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Typography {
    pub ui_font_family: String,
    #[serde(default = "default_size")]
    pub ui_font_size: f64,
    pub ui_font_weight: u16,
    #[serde(default)]
    pub content_font_family: Option<String>,
    #[serde(default)]
    pub content_font_size: Option<f64>,
    #[serde(default)]
    pub content_font_weight: Option<u16>,
    #[serde(default = "default_workspace_family")]
    pub workspace_font_family: String,
    #[serde(default = "default_size")]
    pub workspace_font_size: f64,
    pub code_font_family: String,
    pub code_font_size: f64,
    pub code_font_weight: u16,
    pub high_contrast: bool,
}
fn default_size() -> f64 {
    14.0
}
fn font_stack(families: &[String]) -> Result<String, String> {
    if families.is_empty()
        || families.len() > 16
        || families
            .iter()
            .any(|f| f.is_empty() || f.len() > 512 || f.chars().any(char::is_control))
    {
        return Err("Invalid native font stack".into());
    }
    Ok(families
        .iter()
        .map(|family| {
            if ["sans-serif", "serif", "monospace", "cursive", "fantasy"].contains(&family.as_str())
            {
                family.clone()
            } else {
                format!("\"{}\"", family.replace('\\', "\\\\").replace('"', "\\\""))
            }
        })
        .collect::<Vec<_>>()
        .join(","))
}
fn default_workspace_family() -> String {
    r#""Noto Sans CJK SC", "Noto Sans SC", "Microsoft YaHei", system-ui, sans-serif"#.into()
}
#[derive(Clone, Debug, Deserialize)]
pub struct Fonts {
    pub ui: Vec<String>,
    pub content: Vec<String>,
    pub code: Vec<String>,
    pub workspace: Vec<String>,
}
#[derive(Clone, Debug, Deserialize)]
pub struct Snapshot {
    pub revision: String,
    pub values: Value,
    pub fonts: Fonts,
}
#[derive(Clone, Debug)]
pub struct Plan {
    pub css: String,
    pub contrast: bool,
    pub maximum: i32,
    pub threshold: i32,
    pub code: Option<(String, f64, u16)>,
    pub markdown: MarkdownMode,
    pub diagram: crate::diagram_style::Style,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarkdownMode {
    Reading,
    Technical,
    Compact,
}
impl MarkdownMode {
    pub fn line_height(self) -> f32 {
        match self {
            Self::Reading => 1.86,
            Self::Technical => 1.72,
            Self::Compact => 1.55,
        }
    }
    pub fn code_line_height(self) -> f32 {
        if self == Self::Compact {
            1.48
        } else {
            1.65
        }
    }
    pub fn spacing(self) -> i32 {
        match self {
            Self::Reading => 4,
            Self::Technical => 0,
            Self::Compact => -4,
        }
    }
    fn css(self, base: Option<f64>) -> String {
        let delta = match self {
            Self::Reading => 1.0,
            Self::Technical => 0.0,
            Self::Compact => -1.0,
        };
        let (height, block_height) = (self.line_height(), self.code_line_height());
        let mut css=format!(".studio-markdown {{line-height:{height};}}\n.studio-markdown-block .studio-code {{line-height:{block_height};}}\n");
        for (class, scale) in [
            ("", 1.0),
            (".title-1", 2.0),
            (".title-2", 1.5),
            (".title-3", 1.25),
        ] {
            let size = match base {
                Some(base) => format!("{}px", (base + delta).max(11.0) * scale),
                None if self != Self::Technical => format!(
                    "max({}px,calc({scale}em {} {}px))",
                    11.0 * scale,
                    if delta > 0.0 { "+" } else { "-" },
                    delta.abs() * scale
                ),
                None => continue,
            };
            css.push_str(&format!(".studio-markdown{class} {{font-size:{size};}}\n"));
        }
        css
    }
}
impl Plan {
    pub fn code_font(&self) -> Option<gtk::pango::FontDescription> {
        self.code.as_ref().map(|(family, size, weight)| {
            let mut font = gtk::pango::FontDescription::new();
            font.set_family(family);
            font.set_absolute_size(size * gtk::pango::SCALE as f64);
            font.set_weight(match weight {
                500 => gtk::pango::Weight::Medium,
                600 => gtk::pango::Weight::Semibold,
                _ => gtk::pango::Weight::Normal,
            });
            font
        })
    }
    /// Present the worker's escaped Pango subset with code-role fonts. This is
    /// not a Markdown/HTML parser: source HTML is already escaped by the worker.
    /// Pango still owns parsing, UTF-8 ranges, links, shaping and selection.
    pub fn inline_markup(&self, original: &str) -> String {
        let Some(_) = &self.code else {
            return original.to_owned();
        };
        if !original.contains("<tt>") {
            return original.to_owned();
        }
        let base = self.code_font().unwrap();
        let tags: Vec<String> = (0..4)
            .map(|index| {
                let mut font = base.clone();
                font.set_style(if index & 2 != 0 {
                    gtk::pango::Style::Italic
                } else {
                    gtk::pango::Style::Normal
                });
                if index & 1 != 0 {
                    font.set_weight(gtk::pango::Weight::Bold);
                }
                let font = font
                    .to_string()
                    .replace('&', "&amp;")
                    .replace('<', "&lt;")
                    .replace('>', "&gt;")
                    .replace('"', "&quot;")
                    .replace('\'', "&apos;");
                format!("<span font_desc=\"{font}\">")
            })
            .collect();
        let mut output = String::with_capacity(original.len());
        let (mut bold, mut italic) = (0usize, 0usize);
        let mut rest = original;
        while let Some(start) = rest.find('<') {
            output.push_str(&rest[..start]);
            rest = &rest[start..];
            let Some(end) = rest.find('>') else {
                break;
            };
            let tag = &rest[..=end];
            match tag {
                "<b>" => {
                    bold += 1;
                    output.push_str(tag);
                }
                "</b>" => {
                    bold = bold.saturating_sub(1);
                    output.push_str(tag);
                }
                "<i>" => {
                    italic += 1;
                    output.push_str(tag);
                }
                "</i>" => {
                    italic = italic.saturating_sub(1);
                    output.push_str(tag);
                }
                "<tt>" => {
                    output.push_str(&tags[usize::from(bold > 0) + 2 * usize::from(italic > 0)])
                }
                "</tt>" => output.push_str("</span>"),
                _ => output.push_str(tag),
            }
            rest = &rest[end + 1..];
        }
        output.push_str(rest);
        output
    }
}
impl Snapshot {
    pub fn typography(&self) -> Result<Option<Typography>, String> {
        if self.values["typography"].is_null() {
            Ok(None)
        } else {
            serde_json::from_value(self.values["typography"].clone())
                .map(Some)
                .map_err(|_| "Invalid typography response".into())
        }
    }
    pub fn plan(&self) -> Result<Plan, String> {
        if self.revision.len() != 64 || self.revision.bytes().any(|b| !b.is_ascii_hexdigit()) {
            return Err("Invalid appearance revision".into());
        }
        let (maximum, threshold) = match self.values["contentWidth"].as_str() {
            None | Some("comfortable") => (960, 960),
            Some("wide") => (1280, 1280),
            Some("full") => (i32::MAX, i32::MAX),
            _ => return Err("Invalid reading width".into()),
        };
        let markdown = match self.values.get("markdownMode").filter(|v| !v.is_null()) {
            None => MarkdownMode::Technical,
            Some(v) if v == "technical" => MarkdownMode::Technical,
            Some(v) if v == "reading" => MarkdownMode::Reading,
            Some(v) if v == "compact" => MarkdownMode::Compact,
            _ => return Err("Invalid Markdown layout mode".into()),
        };
        let typography = self.typography()?;
        let content_size = typography
            .as_ref()
            .map(|t| t.content_font_size.unwrap_or(t.ui_font_size));
        let mut plan = Plan {
            css: String::new(),
            contrast: false,
            maximum,
            threshold,
            code: None,
            markdown,
            diagram: crate::diagram_style::Style::default(),
        };
        if let Some(t) = typography {
            for family in [
                &t.ui_font_family,
                &t.workspace_font_family,
                &t.code_font_family,
            ]
            .into_iter()
            .chain(t.content_font_family.iter())
            {
                if family.is_empty() || family.len() > 512 || family.chars().any(char::is_control) {
                    return Err("Invalid typography family".into());
                }
            }
            let content_size = t.content_font_size.unwrap_or(t.ui_font_size);
            let content_weight = t.content_font_weight.unwrap_or(t.ui_font_weight);
            if ![
                t.ui_font_size,
                t.workspace_font_size,
                t.code_font_size,
                content_size,
            ]
            .iter()
            .all(|v| v.is_finite() && (11.0..=24.0).contains(v))
                || ![t.ui_font_weight, t.code_font_weight, content_weight]
                    .iter()
                    .all(|v| [400, 500, 600].contains(v))
            {
                return Err("Typography exceeds its bounds".into());
            }
            for (selector, families, size, weight) in [
                (
                    "window, popover",
                    &self.fonts.ui,
                    t.ui_font_size,
                    t.ui_font_weight,
                ),
                (
                    ".studio-content",
                    &self.fonts.content,
                    content_size,
                    content_weight,
                ),
                (
                    ".studio-workspace",
                    &self.fonts.workspace,
                    t.workspace_font_size,
                    t.ui_font_weight,
                ),
                (
                    ".studio-code",
                    &self.fonts.code,
                    t.code_font_size,
                    t.code_font_weight,
                ),
            ] {
                let names = font_stack(families)?;
                plan.css.push_str(&format!("{selector} {{ font-family: {names}; font-size: {size}px; font-weight: {weight}; }}\n"));
            }
            // Role typography sets the base size; retain Markdown heading hierarchy.
            for (class, scale) in [("title-1", 2.0), ("title-2", 1.5), ("title-3", 1.25)] {
                plan.css.push_str(&format!(
                    ".studio-content.{class} {{ font-size: {}px; font-weight: 700; }}\n",
                    content_size * scale
                ));
            }
            plan.contrast = t.high_contrast;
            plan.code = Some((
                self.fonts.code.join(","),
                t.code_font_size,
                t.code_font_weight,
            ));
        }
        for (field, target) in [
            ("mermaidDensity", &mut plan.diagram.density),
            ("mermaidCurve", &mut plan.diagram.curve),
            ("mermaidStyle", &mut plan.diagram.style),
            ("mermaidLayout", &mut plan.diagram.layout),
        ] {
            if let Some(value) = self.values.get(field).filter(|v| !v.is_null()) {
                *target = value
                    .as_str()
                    .ok_or("Invalid Mermaid preference response")?
                    .into();
            }
        }
        if let Some(value) = self.values.get("mermaidFontSize").filter(|v| !v.is_null()) {
            plan.diagram.font_size =
                u8::try_from(value.as_u64().ok_or("Invalid Mermaid font size")?)
                    .map_err(|_| "Invalid Mermaid font size")?;
        }
        if plan.code.is_some() {
            plan.diagram.font_family = font_stack(&self.fonts.content)?;
            plan.diagram.font_family_mono = font_stack(&self.fonts.code)?;
        }
        plan.diagram.validate()?;
        plan.css.push_str(&markdown.css(content_size));
        Ok(plan)
    }
}
#[derive(Debug)]
pub enum Request {
    Read,
    Save { revision: String, values: Value },
}
#[derive(Clone, Debug)]
pub struct Reply {
    pub snapshot: Snapshot,
    pub plan: Plan,
}
pub async fn execute(api: &crate::gateway::Api, request: Request) -> Result<Reply, String> {
    let (method, body) = match request {
        Request::Read => (reqwest::Method::GET, None),
        Request::Save { revision, values } => (
            reqwest::Method::PATCH,
            Some(serde_json::json!({"expectedRevision":revision,"values":values})),
        ),
    };
    let value = api
        .json(method, "/studio/native/appearance", body.as_ref())
        .await?;
    crate::worker::compute(move || {
        let snapshot: Snapshot =
            serde_json::from_value(value).map_err(|_| "Invalid appearance response")?;
        let plan = snapshot.plan()?;
        Ok(Reply { snapshot, plan })
    })
    .await
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn markdown_mode_validation_keeps_font_defaults_and_bounds() {
        let mut snapshot:Snapshot=serde_json::from_value(serde_json::json!({"revision":"a".repeat(64),"values":{"typography":null},"fonts":{"ui":[],"content":[],"code":[],"workspace":[]}})).unwrap();
        for (value, expected) in [
            (serde_json::Value::Null, MarkdownMode::Technical),
            (serde_json::json!("reading"), MarkdownMode::Reading),
            (serde_json::json!("compact"), MarkdownMode::Compact),
        ] {
            snapshot.values["markdownMode"] = value;
            let plan = snapshot.plan().unwrap();
            assert_eq!(plan.markdown, expected);
            assert!(plan.code.is_none());
            assert!(!plan.css.contains("font-family"));
        }
        for value in [
            serde_json::json!("unknown"),
            serde_json::json!(42),
            serde_json::json!(false),
        ] {
            snapshot.values["markdownMode"] = value;
            assert!(snapshot.plan().is_err());
        }
    }
    #[test]
    fn inline_code_profile_round_trips_pango_text_and_nested_emphasis() {
        let plan = Plan {
            css: String::new(),
            contrast: false,
            maximum: 960,
            threshold: 960,
            code: Some(("Font \"quote\" & <literal>,monospace".into(), 15.0, 600)),
            markdown: MarkdownMode::Technical,
            diagram: Default::default(),
        };
        let original = studio_markdown_native::markup::markup(
            "中文😀 `code <b>&` **`strong`** *`italic`* literal <tt> &",
        );
        let styled = plan.inline_markup(&original);
        let (_, before, _) = gtk::pango::parse_markup(&original, '\0').unwrap();
        let (attributes, after, _) = gtk::pango::parse_markup(&styled, '\0').unwrap();
        assert_eq!(before, after);
        let mut iterator = attributes.iterator();
        let mut found = 0;
        loop {
            let font = iterator.font().0;
            if font
                .family()
                .is_some_and(|f| f.contains("Font \"quote\" & <literal>"))
            {
                found += 1;
                assert_eq!(font.size(), 15 * gtk::pango::SCALE);
                assert!(font.is_size_absolute());
                let (start, end) = iterator.range();
                let text = &after.as_str()[start as usize..end as usize];
                if text == "strong" {
                    assert_eq!(font.weight(), gtk::pango::Weight::Bold);
                } else if text == "italic" {
                    assert_eq!(font.style(), gtk::pango::Style::Italic);
                } else {
                    assert_eq!(font.weight(), gtk::pango::Weight::Semibold);
                }
            }
            if !iterator.next_style_change() {
                break;
            }
        }
        assert_eq!(found, 3);
        let mut defaults = plan;
        defaults.code = None;
        assert_eq!(defaults.inline_markup(&original), original);
    }
    #[test]
    fn default_plan_and_family_escaping_are_bounded() {
        let base:Snapshot=serde_json::from_value(serde_json::json!({"revision":"a".repeat(64),"values":{"contentWidth":"wide","typography":null},"fonts":{"ui":[],"content":[],"code":[],"workspace":[]}})).unwrap();
        assert_eq!(base.plan().unwrap().maximum, 1280);
        assert!(!base.plan().unwrap().css.contains("font-family"));
        let mut invalid = base;
        invalid.values["contentWidth"] = serde_json::json!("unbounded");
        assert!(invalid.plan().is_err());
    }
    #[test]
    fn legacy_defaults_and_literal_font_names_cannot_escape_css() {
        let mut snapshot:Snapshot=serde_json::from_value(serde_json::json!({"revision":"b".repeat(64),"values":{"typography":{"uiFontFamily":"sans-serif","uiFontWeight":400,"codeFontFamily":"monospace","codeFontSize":15,"codeFontWeight":600,"highContrast":true}},"fonts":{"ui":["sans-serif"],"content":["odd\";color:red;/*"],"code":["monospace"],"workspace":["Noto Sans CJK SC","sans-serif"]}})).unwrap();
        let plan = snapshot.plan().unwrap();
        assert!(plan.css.contains("font-size: 14px"));
        assert!(plan.css.contains("font-family: sans-serif;"));
        assert!(plan.css.contains("odd\\\";color:red;/*\";"));
        assert!(plan.contrast);
        snapshot.fonts.content = vec!["bad\nfont".into()];
        assert!(snapshot.plan().is_err());
    }
}
