//! Static HTML projection: no scripts, stylesheets, network or local file reads.
use super::*;
use kuchikiki::{NodeRef, traits::*};

fn tag(node: &NodeRef) -> &str {
    node.as_element()
        .map(|e| e.name.local.as_ref())
        .unwrap_or("")
}
pub(super) fn clean(
    node: &NodeRef,
    depth: usize,
    resolve: &mut dyn FnMut(&str, &str) -> io::Result<Option<String>>,
) -> io::Result<(String, String)> {
    if depth > 64 {
        return Err(error("HTML exceeds 64 levels"));
    }
    if let Some(text) = node.as_text() {
        return Ok((escaped(&text.borrow()), text.borrow().to_string()));
    }
    let name = tag(node);
    if matches!(
        name,
        "script"
            | "style"
            | "iframe"
            | "object"
            | "embed"
            | "form"
            | "input"
            | "button"
            | "link"
            | "meta"
            | "head"
            | "svg"
            | "math"
            | "template"
            | "noscript"
    ) {
        return Ok((String::new(), String::new()));
    }
    if name == "img" {
        let e = node.as_element().unwrap();
        let a = e.attributes.borrow();
        let alt = a.get("alt").unwrap_or("Image");
        let src = a.get("src").unwrap_or("");
        let image =
            resolve(src, alt)?.unwrap_or_else(|| format!("<span>[Image: {}]</span>", escaped(alt)));
        return Ok((image, format!("{alt}\n")));
    }
    let mut html = String::new();
    let mut text = String::new();
    for child in node.children() {
        let (h, t) = clean(&child, depth + 1, resolve)?;
        html.push_str(&h);
        text.push_str(&t);
        if html.len() > 4 * 1024 * 1024 {
            return Err(error("HTML segment exceeds 4 MiB"));
        }
    }
    let allowed = matches!(
        name,
        "p" | "div"
            | "section"
            | "article"
            | "main"
            | "header"
            | "footer"
            | "span"
            | "br"
            | "hr"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "strong"
            | "b"
            | "em"
            | "i"
            | "u"
            | "s"
            | "del"
            | "code"
            | "pre"
            | "ul"
            | "ol"
            | "li"
            | "blockquote"
            | "table"
            | "thead"
            | "tbody"
            | "tfoot"
            | "tr"
            | "th"
            | "td"
            | "a"
            | "details"
            | "summary"
            | "dl"
            | "dt"
            | "dd"
            | "sup"
            | "sub"
    );
    if allowed {
        let mut attrs = String::new();
        if name == "a" {
            if let Some(e) = node.as_element() {
                if let Some(href) = e.attributes.borrow().get("href") {
                    let value = href.trim();
                    if value.starts_with("https://")
                        || value.starts_with("http://")
                        || value.starts_with("mailto:")
                    {
                        attrs = format!(" href=\"{}\" rel=\"noopener noreferrer\"", escaped(value));
                    }
                }
            }
        }
        if name == "br" || name == "hr" {
            html = format!("<{name}>");
            text.push('\n');
        } else {
            html = format!("<{name}{attrs}>{html}</{name}>");
            if !matches!(
                name,
                "span"
                    | "strong"
                    | "b"
                    | "em"
                    | "i"
                    | "u"
                    | "s"
                    | "del"
                    | "code"
                    | "a"
                    | "sup"
                    | "sub"
            ) {
                text.push('\n');
            }
        }
    }
    Ok((html, text))
}
pub(super) fn document(source: &str) -> io::Result<NodeRef> {
    if source.len() > 16 * 1024 * 1024 {
        return Err(error("HTML chapter exceeds 16 MiB"));
    }
    let dom = kuchikiki::parse_html().one(source);
    if dom.descendants().take(100_001).count() > 100_000 {
        return Err(error("HTML exceeds 100,000 nodes"));
    }
    Ok(dom)
}
pub(super) fn standalone(
    path: &Path,
    directory: &Path,
    source: &str,
    starts: &[usize],
    started: Instant,
) -> io::Result<Index> {
    let dom = document(source)?;
    let (html, _) = clean(&dom, 0, &mut |_, _| Ok(None))?;
    if html.len() > crate::core::MAX_HTML {
        let mut index = text(
            path,
            directory,
            source,
            starts,
            DocumentFormat::Text,
            started,
        )?;
        index.format = DocumentFormat::Html;
        publish(directory, &index)?;
        return Ok(index);
    }
    fs::write(directory.join("pages.html"), &html)?;
    File::create(directory.join("diagrams.src"))?;
    let mut index = Index {
        version: 6,
        format: DocumentFormat::Html,
        name: name(path),
        revision: hash(source.as_bytes()),
        source_bytes: source.len(),
        source_lines: starts.len(),
        ..Default::default()
    };
    index.pages.push(Page {
        start: 0,
        end: source.len(),
        line: 1,
        offset: 0,
        bytes: html.len(),
        rows: 0,
        row_start: 0,
        columns: 0,
        table: None,
    });
    index.index_ms = started.elapsed().as_secs_f64() * 1000.0;
    index.peak_rss_kib = crate::core::peak_rss_kib();
    publish(directory, &index)?;
    Ok(index)
}
