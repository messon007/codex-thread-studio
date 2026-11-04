//! Markdown to the small, escaped Pango markup subset understood by GtkLabel.
//! Runs on Rust workers, never the GTK main loop. HTML stays literal text.
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

pub(crate) fn safe_link(destination: &str, nested: bool) -> bool {
    !nested
        && !["javascript:", "data:", "studio-doc-image:"]
            .iter()
            .any(|scheme| {
                destination
                    .trim_start()
                    .get(..scheme.len())
                    .is_some_and(|prefix| prefix.eq_ignore_ascii_case(scheme))
            })
        && !destination.chars().any(char::is_control)
}

pub(crate) fn escape(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    for character in input.chars() {
        match character {
            '&' => output.push_str("&amp;"),
            '<' => output.push_str("&lt;"),
            '>' => output.push_str("&gt;"),
            '"' => output.push_str("&quot;"),
            '\'' => output.push_str("&apos;"),
            '\0' => output.push('␀'),
            value => output.push(value),
        }
    }
    output
}

pub fn markup(source: &str) -> String {
    let options =
        Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS;
    markup_events(Parser::new_ext(source, options))
}

pub fn markup_events<'a>(events: impl IntoIterator<Item = Event<'a>>) -> String {
    let mut output = String::new();
    let mut links = Vec::new();
    for event in events {
        match event {
            Event::Start(tag) => match tag {
                Tag::Heading { .. } => output.push_str("<big><b>"),
                Tag::Strong => output.push_str("<b>"),
                Tag::Emphasis => output.push_str("<i>"),
                Tag::Strikethrough => output.push_str("<s>"),
                Tag::CodeBlock(_) => output.push_str("<tt>"),
                Tag::Item => output.push_str("• "),
                Tag::BlockQuote(_) => output.push_str("┃ "),
                Tag::Table(_) => output.push_str("<tt>"),
                Tag::TableCell => output.push_str("  "),
                Tag::Link { dest_url, .. } | Tag::Image { dest_url, .. } => {
                    let safe = safe_link(&dest_url, links.iter().any(|safe| *safe));
                    links.push(safe);
                    if safe {
                        output.push_str("<a href=\"");
                        output.push_str(&escape(&dest_url));
                        output.push_str("\">");
                    }
                }
                _ => {}
            },
            Event::End(tag) => match tag {
                TagEnd::Heading(_) => output.push_str("</b></big>\n\n"),
                TagEnd::Strong => output.push_str("</b>"),
                TagEnd::Emphasis => output.push_str("</i>"),
                TagEnd::Strikethrough => output.push_str("</s>"),
                TagEnd::CodeBlock => output.push_str("</tt>\n"),
                TagEnd::Paragraph => output.push_str("\n\n"),
                TagEnd::Item | TagEnd::TableHead | TagEnd::TableRow => output.push('\n'),
                TagEnd::Table => output.push_str("</tt>\n"),
                TagEnd::Link | TagEnd::Image => {
                    if links.pop().unwrap_or(false) {
                        output.push_str("</a>");
                    }
                }
                _ => {}
            },
            Event::Text(text) | Event::Html(text) | Event::InlineHtml(text) => {
                output.push_str(&escape(&text))
            }
            Event::Code(code) => {
                output.push_str("<tt>");
                output.push_str(&escape(&code));
                output.push_str("</tt>");
            }
            Event::SoftBreak | Event::HardBreak => output.push('\n'),
            Event::Rule => output.push_str("────────────\n"),
            Event::TaskListMarker(checked) => {
                output.push_str(if checked { "☑ " } else { "☐ " })
            }
            _ => {}
        }
    }
    output.trim_end().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preserves_unicode_styles_and_escapes_markup_injection() {
        let rendered = markup("# 中文\n\n**bold** and `<b>code</b>`\n\n<script>alert(1)</script>");
        assert!(rendered.contains("<big><b>中文</b></big>"));
        assert!(rendered.contains("<b>bold</b>"));
        assert!(rendered.contains("&lt;b&gt;code&lt;/b&gt;"));
        assert!(!rendered.contains("<script>"));
    }
    #[test]
    fn file_links_survive_but_executable_schemes_stay_inert() {
        let rendered =
            markup("[Read](docs/README.md:1) [bad](javascript:alert) [data](data:text/html,test)");
        assert!(rendered.contains("<a href=\"docs/README.md:1\">Read</a>"));
        assert!(!rendered.contains("href=\"javascript:") && !rendered.contains("href=\"data:"));
    }
}
