//! Native math -> self-contained glyph-outline SVG, on an isolated Rust worker.
use std::{
    io::{Read, Write},
    process::Command,
    sync::{atomic::AtomicBool, Arc},
    time::Duration,
};
pub const SOURCE_LIMIT: usize = studio_markdown_native::inline::MATH_SOURCE_BYTES;
const SVG_LIMIT: usize = 2 * 1024 * 1024;
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Metrics {
    /// SVG viewBox units at the nominal em size, independent of raster scale.
    pub width: f64,
    pub height: f64,
    pub baseline: f64,
    pub em: f64,
}
impl Metrics {
    pub fn from_svg(svg: &str) -> Result<Self, String> {
        let raw = svg
            .strip_prefix("<svg data-studio-math=\"")
            .and_then(|s| s.split_once('"').map(|(head, _)| head))
            .filter(|s| s.len() <= 160)
            .ok_or("Inline math metrics are missing")?;
        let numbers = raw
            .split_whitespace()
            .map(str::parse::<f64>)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| "Invalid inline math metrics")?;
        let [width, height, baseline, em] = numbers.as_slice() else {
            return Err("Invalid inline math metric count".into());
        };
        if numbers.iter().any(|n| !n.is_finite() || n.abs() > 32768.0)
            || *width <= 0.0
            || *height <= 0.0
            || *em <= 0.0
            || *em > 1024.0
        {
            return Err("Inline math metrics exceed their bounds".into());
        }
        Ok(Self {
            width: *width,
            height: *height,
            baseline: *baseline,
            em: *em,
        })
    }
}
pub fn svg(source: &str, dark: bool) -> Result<String, String> {
    svg_mode(source, dark, false, true)
}
fn svg_mode(source: &str, dark: bool, inline: bool, display_style: bool) -> Result<String, String> {
    if source.is_empty() || source.len() > SOURCE_LIMIT || source.contains('\0') {
        return Err("Math source is empty, invalid or exceeds 16 KiB".into());
    }
    let nodes = ratex_parser::parse(source).map_err(|e| format!("Math: {e}"))?;
    let color = if dark {
        ratex_types::color::Color::WHITE
    } else {
        ratex_types::color::Color::BLACK
    };
    let layout = ratex_layout::layout(
        &nodes,
        &ratex_layout::LayoutOptions::default()
            .with_color(color)
            .with_style(if display_style {
                ratex_types::math_style::MathStyle::Display
            } else {
                ratex_types::math_style::MathStyle::Text
            }),
    );
    ratex_layout::validate_layout(&layout)?;
    let display = ratex_layout::to_display_list(&layout);
    let padding = if inline { 2.0 } else { 8.0 };
    let width = display.width * 24.0 + padding * 2.0;
    let height = (display.height + display.depth) * 24.0 + padding * 2.0;
    let baseline = display.height * 24.0 + padding;
    if !width.is_finite()
        || !height.is_finite()
        || width <= 0.0
        || height <= 0.0
        || width > 32768.0
        || height > 32768.0
        || !baseline.is_finite()
        || baseline.abs() > 32768.0
    {
        return Err("Math dimensions exceed the render budget".into());
    }
    let mut svg = ratex_svg::render_to_svg_with_color_syntax(
        &display,
        &ratex_svg::SvgOptions {
            font_size: 24.0,
            padding,
            embed_glyphs: true,
            ..Default::default()
        },
        ratex_svg::SvgColorSyntax::Rgb,
    );
    if inline {
        if !svg.starts_with("<svg ") {
            return Err("Unexpected inline math SVG envelope".into());
        }
        svg.insert_str(
            4,
            &format!(" data-studio-math=\"{width:.8} {height:.8} {baseline:.8} 24\""),
        );
    }
    if svg.len() > SVG_LIMIT {
        return Err("Math SVG exceeds 2 MiB".into());
    }
    Ok(svg)
}
pub fn render(source: &str, dark: bool, cancel: Arc<AtomicBool>) -> Result<String, String> {
    render_mode(source, dark, false, true, cancel)
}
pub fn render_inline(
    source: &str,
    dark: bool,
    display: bool,
    cancel: Arc<AtomicBool>,
) -> Result<String, String> {
    render_mode(source, dark, true, display, cancel)
}
fn render_mode(
    source: &str,
    dark: bool,
    inline: bool,
    display: bool,
    cancel: Arc<AtomicBool>,
) -> Result<String, String> {
    render_mode_timeout(
        source,
        dark,
        inline,
        display,
        cancel,
        Duration::from_secs(3),
    )
}
pub(crate) fn render_inline_until(
    source: &str,
    dark: bool,
    cancel: Arc<AtomicBool>,
    deadline: std::time::Instant,
) -> Result<String, String> {
    let remaining = deadline
        .checked_duration_since(std::time::Instant::now())
        .ok_or("Mermaid math deadline exceeded")?;
    render_mode_timeout(
        source,
        dark,
        true,
        true,
        cancel,
        remaining.min(Duration::from_secs(3)),
    )
}
fn render_mode_timeout(
    source: &str,
    dark: bool,
    inline: bool,
    display: bool,
    cancel: Arc<AtomicBool>,
    timeout: Duration,
) -> Result<String, String> {
    if source.len() > SOURCE_LIMIT {
        return Err("Math source exceeds 16 KiB".into());
    }
    let mut command = Command::new(std::env::current_exe().map_err(|e| e.to_string())?);
    command
        .arg("--native-math-worker")
        .env_clear()
        .env("LANG", "C.UTF-8")
        .env("HOME", "/nonexistent");
    let input = serde_json::to_vec(
        &serde_json::json!({"source":source,"dark":dark,"inline":inline,"displayStyle":display}),
    )
    .map_err(|e| e.to_string())?;
    crate::engine_process::svg(&mut command, input, cancel, timeout, SVG_LIMIT)
}
pub fn worker_stdio() -> i32 {
    let result = (|| -> Result<(), String> {
        #[cfg(unix)]
        unsafe {
            for (kind, value) in [
                (libc::RLIMIT_DATA, 256 * 1024 * 1024),
                (libc::RLIMIT_CPU, 2),
                (libc::RLIMIT_CORE, 0),
            ] {
                let limit = libc::rlimit {
                    rlim_cur: value,
                    rlim_max: value,
                };
                if libc::setrlimit(kind, &limit) != 0 {
                    return Err("Cannot apply math worker resource limits".into());
                }
            }
        }
        let mut input = Vec::new();
        std::io::stdin()
            .take(65537)
            .read_to_end(&mut input)
            .map_err(|e| e.to_string())?;
        if input.len() > 65536 {
            return Err("Math worker input exceeds its budget".into());
        }
        let v: serde_json::Value =
            serde_json::from_slice(&input).map_err(|_| "Invalid math worker input")?;
        let source = v["source"]
            .as_str()
            .ok_or("Math worker needs source text")?;
        let dark = v["dark"].as_bool().ok_or("Math worker needs a theme")?;
        let boolean = |key: &str, default: bool| -> Result<bool, String> {
            match v.get(key) {
                None => Ok(default),
                Some(serde_json::Value::Bool(b)) => Ok(*b),
                _ => Err(format!("Math worker needs a boolean {key}")),
            }
        };
        let inline = boolean("inline", false)?;
        let display = boolean("displayStyle", !inline)?;
        let output = if !inline && display {
            svg(source, dark)?
        } else {
            svg_mode(source, dark, inline, display)?
        };
        std::io::stdout()
            .write_all(output.as_bytes())
            .map_err(|e| e.to_string())?;
        Ok(())
    })();
    match result {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("{e}");
            1
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn expired_parent_deadline_rejects_before_spawning_math_worker() {
        let result = render_inline_until(
            "x",
            false,
            Arc::new(AtomicBool::new(false)),
            std::time::Instant::now(),
        );
        assert!(result.unwrap_err().contains("deadline"));
    }
    #[test]
    fn inline_styles_preserve_baselines_and_use_text_sized_operators() {
        let text = svg_mode(r"\sum_{i=0}^n x_i", false, true, false).unwrap();
        let display = svg_mode(r"\sum_{i=0}^n x_i", false, true, true).unwrap();
        let text_metrics = Metrics::from_svg(&text).unwrap();
        let display_metrics = Metrics::from_svg(&display).unwrap();
        assert!(display_metrics.height > text_metrics.height);
        for source in [r"x^2", r"\frac{a}{b}", r"\int_0^1 x\,dx"] {
            let inline = svg_mode(source, false, true, false).unwrap();
            let metrics = Metrics::from_svg(&inline).unwrap();
            assert_eq!(metrics.em, 24.0);
            assert!(metrics.baseline > 0.0 && metrics.baseline < metrics.height);
            assert!(inline.contains("<path") && !inline.contains("<text"));
            assert_ne!(inline, svg_mode(source, true, true, false).unwrap());
        }
        for raw in [
            "<svg/>",
            "<svg data-studio-math=\"NaN 1 1 24\">",
            "<svg data-studio-math=\"1 inf 1 24\">",
            "<svg data-studio-math=\"1 1 1 0\">",
            "<svg data-studio-math=\"1 1 1 24 0\">",
        ] {
            assert!(Metrics::from_svg(raw).is_err());
        }
    }
    #[test]
    fn embedded_fonts_render_standard_math_without_dom_or_font_files() {
        for source in [
            r"\frac{1}{2}",
            r"x=\frac{-b\pm\sqrt{b^2-4ac}}{2a}",
            r"\int_0^\infty e^{-x^2}\,dx=\frac{\sqrt{\pi}}{2}",
            r"\begin{pmatrix}a&b\\c&d\end{pmatrix}",
            r"\ce{2H2 + O2 -> 2H2O}",
        ] {
            let s = svg(source, false).unwrap();
            assert!(s.contains("<path"));
            assert!(!s.contains("<text"));
            assert!(!s.contains("<foreignObject"));
        }
        assert_ne!(svg("x^2", false).unwrap(), svg("x^2", true).unwrap());
    }
    #[test]
    fn invalid_input_and_unbounded_expansion_are_errors() {
        for source in [
            r"\input{/etc/passwd}",
            r"\write18{echo forbidden}",
            r"\unsupportedNativeMacro{x}",
            r"\def\a{\a}\a",
        ] {
            assert!(svg(source, false).is_err());
        }
        assert!(svg(&format!("{}x{}", "{".repeat(1000), "}".repeat(1000)), false).is_err());
        assert!(svg(&"x".repeat(SOURCE_LIMIT + 1), false).is_err());
    }
}
