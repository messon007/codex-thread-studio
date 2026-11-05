//! GFM text extensions over the existing source-offset-preserving parser.
use linkify::{LinkFinder, LinkKind};
use pulldown_cmark::{Event, LinkType, Tag, TagEnd, TextMergeStream};
use std::io;

/// Linkify scans linearly. Work is limited by the existing indivisible page and
/// event budgets; source ranges are deliberately not rewritten by this renderer.
pub(super) fn autolinks<'a>(events: Vec<Event<'a>>) -> io::Result<Vec<Event<'a>>> {
    let mut finder = LinkFinder::new();
    finder.url_must_have_scheme(false);
    let mut output = Vec::with_capacity(events.len());
    let mut protected = 0usize;
    let mut raw_scopes = Vec::new();
    for event in TextMergeStream::new(events.into_iter()) {
        if let Event::Html(value) | Event::InlineHtml(value) = &event {
            for (scope, open) in super::markdown_html::scopes(value) {
                if open {
                    if raw_scopes.len() >= super::MAX_EVENTS {
                        return Err(super::invalid("Raw HTML scope budget exceeded"));
                    }
                    raw_scopes.push(scope);
                } else if let Some(index) = raw_scopes.iter().rposition(|item| *item == scope) {
                    raw_scopes.truncate(index);
                }
            }
        }
        match &event {
            Event::Start(Tag::Link { .. } | Tag::Image { .. } | Tag::CodeBlock(_)) => {
                protected += 1
            }
            Event::End(TagEnd::Link | TagEnd::Image | TagEnd::CodeBlock) => {
                protected = protected.saturating_sub(1)
            }
            _ => {}
        }
        if let Event::Text(text) = &event {
            if protected == 0 && raw_scopes.is_empty() {
                let mut end = 0;
                for link in finder.links(text) {
                    let value = link.as_str();
                    let lower = value.to_ascii_lowercase();
                    let (kind, href) = match link.kind() {
                        LinkKind::Email => (LinkType::Email, value.to_owned()),
                        LinkKind::Url
                            if lower.starts_with("http://") || lower.starts_with("https://") =>
                        {
                            (LinkType::Autolink, value.to_owned())
                        }
                        LinkKind::Url if lower.starts_with("www.") => {
                            (LinkType::Autolink, format!("http://{value}"))
                        }
                        _ => continue,
                    };
                    if link.start() > end {
                        output.push(Event::Text(text[end..link.start()].to_owned().into()));
                    }
                    output.push(Event::Start(Tag::Link {
                        link_type: kind,
                        dest_url: href.into(),
                        title: "".into(),
                        id: "".into(),
                    }));
                    output.push(Event::Text(value.to_owned().into()));
                    output.push(Event::End(TagEnd::Link));
                    end = link.end();
                    if output.len() > super::MAX_EVENTS {
                        return Err(super::invalid("Autolinks exceed the page event budget"));
                    }
                }
                if end > 0 {
                    if end < text.len() {
                        output.push(Event::Text(text[end..].to_owned().into()));
                    }
                    continue;
                }
            }
        }
        output.push(event);
        if output.len() > super::MAX_EVENTS {
            return Err(super::invalid("Markdown exceeds the page event budget"));
        }
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pulldown_cmark::{Options, Parser, html};
    #[test]
    fn extended_links_preserve_literals_and_never_nest() {
        let source = "URL https://example.org/a_(b). www.example.org/x, me@example.org.\n\n`www.code.org` [www.label.org](https://target.org) ![www.alt.org](image.png)\n\n```txt\nhttps://code.example.org\n```\n\nexample.org javascript:alert(1) <span>www.raw.org</span>";
        let events = Parser::new_ext(source, Options::ENABLE_TABLES).collect();
        let mut output = String::new();
        html::push_html(&mut output, autolinks(events).unwrap().into_iter());
        assert!(output.contains("href=\"https://example.org/a_(b)\""));
        assert!(output.contains("href=\"http://www.example.org/x\""));
        assert!(output.contains("href=\"mailto:me@example.org\""));
        assert!(
            !output.contains("href=\"http://www.code.org")
                && !output.contains("href=\"http://www.label.org")
        );
        assert!(
            !output.contains("href=\"http://www.alt.org")
                && !output.contains("href=\"https://code.example.org")
        );
        assert!(
            !output.contains("href=\"http://example.org") && !output.contains("href=\"javascript:")
        );
        assert!(!output.contains("<a href=\"https://target.org\"><a"));
    }
    #[test]
    fn raw_anchor_and_code_text_are_not_autolinked_again() {
        let source = "Inline <a href='https://target.example'>www.raw-label.example</a> and <code>www.raw-code.example</code> then www.allowed.example.";
        let events = Parser::new_ext(source, Options::empty()).collect();
        let mut html = String::new();
        pulldown_cmark::html::push_html(
            &mut html,
            autolinks(events)
                .unwrap()
                .into_iter()
                .map(super::super::inert_event),
        );
        assert!(html.contains(">www.raw-label.example</a>"));
        assert!(html.contains("<code>www.raw-code.example</code>"));
        assert!(
            !html.contains("http://www.raw-label.example")
                && !html.contains("http://www.raw-code.example")
        );
        assert!(html.contains("http://www.allowed.example"));
    }
    #[test]
    fn event_expansion_is_bounded() {
        let text = "https://e.example/ ".repeat(super::super::MAX_EVENTS);
        assert!(autolinks(vec![Event::Text(text.into())]).is_err());
    }
}
