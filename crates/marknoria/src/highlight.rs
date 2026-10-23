use std::sync::LazyLock;
use syntect::{
    highlighting::ThemeSet,
    html::{ClassStyle, ClassedHTMLGenerator, css_for_theme_with_class_style},
    parsing::SyntaxSet,
    util::LinesWithEndings,
};

static SYNTAXES: LazyLock<SyntaxSet> = LazyLock::new(SyntaxSet::load_defaults_newlines);
static THEMES: LazyLock<ThemeSet> = LazyLock::new(ThemeSet::load_defaults);

fn style() -> ClassStyle {
    ClassStyle::SpacedPrefixed { prefix: "mn-" }
}

pub fn code(language: &str, source: &str) -> Option<String> {
    if source.len() > 64 * 1024 {
        return None;
    }
    let token = language.split_whitespace().next()?;
    let syntax = SYNTAXES.find_syntax_by_token(token)?;
    let mut generator = ClassedHTMLGenerator::new_with_class_style(syntax, &SYNTAXES, style());
    for line in LinesWithEndings::from(source) {
        generator
            .parse_html_for_line_which_includes_newline(line)
            .ok()?;
    }
    let html = generator.finalize();
    if html.len() > 2 * 1024 * 1024 {
        return None;
    }
    Some(format!(
        "<pre><code class=\"mn-code\">{html}</code></pre>\n"
    ))
}

pub fn css(dark: bool) -> String {
    let name = if dark {
        "base16-ocean.dark"
    } else {
        "InspiredGitHub"
    };
    css_for_theme_with_class_style(&THEMES.themes[name], style()).expect("bundled syntax theme")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn code_is_highlighted_without_executing_html_or_losing_text() {
        let html = code("rust", "fn main() { println!(\"<script>\"); }\n").unwrap();
        assert!(html.contains("<span class=\"mn-"), "{html}");
        assert!(html.contains("&lt;script&gt;"));
        assert!(!html.contains("<script>"));
        assert!(code("unknown-language", "text").is_none());
        assert!(code("rust", &"x".repeat(64 * 1024 + 1)).is_none());
        assert!(css(false).contains(".mn-"));
        assert!(css(true).contains(".mn-"));
    }
}
