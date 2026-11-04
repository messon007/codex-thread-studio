//! Bounded, immutable EPUB archives. Only individual resources reach the reader.
use axum::{
    body::{Body, Bytes},
    extract::{rejection::JsonRejection, State},
    http::{Response, StatusCode},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{HashMap, VecDeque},
    io::{Cursor, Read},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use zip::ZipArchive;
type Error = (StatusCode, String);
type Archive = ZipArchive<Cursor<Arc<[u8]>>>;
#[path = "native_epub_search.rs"]
mod search;
const MAX_RESOURCE: usize = 16 * 1024 * 1024;
const MAX_XML: usize = 2 * 1024 * 1024;
fn bad(error: impl ToString) -> Error {
    (StatusCode::BAD_REQUEST, error.to_string())
}
fn gone() -> Error {
    (
        StatusCode::GONE,
        "EPUB snapshot closed or expired; reopen the book".into(),
    )
}
fn valid_id(id: &str) -> bool {
    uuid::Uuid::parse_str(id).is_ok_and(|value| value.to_string() == id)
}

#[derive(Deserialize)]
#[serde(tag = "operation", rename_all = "camelCase", deny_unknown_fields)]
pub enum Request {
    Open {
        id: String,
        root: String,
        path: String,
    },
    Asset {
        id: String,
        path: String,
    },
    Search {
        id: String,
        query: String,
        #[serde(default)]
        #[serde(rename = "caseSensitive")]
        case_sensitive: bool,
        #[serde(default)]
        start: usize,
    },
    Save {
        id: String,
        update: ReadingUpdate,
    },
    Close {
        id: String,
        #[serde(default)]
        update: Option<ReadingUpdate>,
    },
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReadingUpdate {
    revision: u64,
    reading: crate::epub_reader::ReadingState,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Chapter {
    id: String,
    href: String,
    label: String,
    linear: bool,
}
#[derive(Clone, Serialize)]
struct Heading {
    label: String,
    href: String,
    depth: usize,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Manifest {
    document_id: String,
    root: String,
    path: String,
    hash: String,
    title: String,
    author: String,
    language: String,
    direction: String,
    layout: String,
    spine_node_index: usize,
    spine: Vec<Chapter>,
    toc: Vec<Heading>,
    toc_limited: bool,
}
struct Book {
    archive: Archive,
    entries: HashMap<String, usize>,
    resources: HashMap<String, String>,
    manifest: Manifest,
    saved_revision: Mutex<u64>,
}
#[derive(Default)]
struct Cache {
    books: HashMap<String, (Arc<Book>, Instant)>,
    pages: VecDeque<((String, String), Arc<[u8]>)>,
    weight: usize,
    closed: VecDeque<(String, Instant)>,
}
#[derive(Clone)]
pub struct Books {
    cache: Arc<Mutex<Cache>>,
    workers: Arc<tokio::sync::Semaphore>,
    closing: Arc<tokio::sync::Semaphore>,
    searching: Arc<tokio::sync::Semaphore>,
}
impl Default for Books {
    fn default() -> Self {
        Self {
            cache: Arc::default(),
            workers: Arc::new(tokio::sync::Semaphore::new(2)),
            closing: Arc::new(tokio::sync::Semaphore::new(1)),
            searching: Arc::new(tokio::sync::Semaphore::new(1)),
        }
    }
}
fn bounded(value: &str, max: usize) -> String {
    value
        .chars()
        .filter(|c| !c.is_control() || *c == ' ')
        .take(max)
        .collect()
}
fn xml(value: &str) -> Result<roxmltree::Document<'_>, Error> {
    roxmltree::Document::parse_with_options(
        value,
        roxmltree::ParsingOptions {
            allow_dtd: false,
            nodes_limit: 100_000,
            ..Default::default()
        },
    )
    .map_err(bad)
}
/// URI paths and ZIP paths share canonical names, without touching the filesystem.
fn resource_path(base: &str, href: &str) -> Result<String, Error> {
    let (href, fragment) = href.split_once('#').unwrap_or((href, ""));
    if href.contains('?') || href.contains(':') || href.contains('\\') || href.starts_with("//") {
        return Err(bad("EPUB resource must be a local archive path"));
    }
    let href = percent_encoding_decode(href)?;
    let base = percent_encoding_decode(base)?;
    let mut parts = if href.starts_with('/') {
        Vec::new()
    } else {
        base.split('/')
            .filter(|v| !v.is_empty())
            .map(str::to_owned)
            .collect()
    };
    if !href.is_empty() {
        for piece in href.split('/') {
            match piece {
                "" | "." => {}
                ".." => {
                    if parts.pop().is_none() {
                        return Err(bad("EPUB resource escapes its archive"));
                    }
                }
                value => parts.push(value.to_owned()),
            }
        }
    }
    let mut result = encode_path(&parts.join("/"));
    if result.len() > 4096 || result.is_empty() {
        return Err(bad("Invalid EPUB resource path"));
    }
    if !fragment.is_empty() {
        if fragment.len() > 2048 || fragment.chars().any(char::is_control) {
            return Err(bad("Invalid EPUB fragment"));
        }
        result.push('#');
        result.push_str(fragment);
    }
    Ok(result)
}
fn encode_path(value: &str) -> String {
    const PATH: &percent_encoding::AsciiSet = &percent_encoding::NON_ALPHANUMERIC
        .remove(b'/')
        .remove(b'-')
        .remove(b'_')
        .remove(b'.')
        .remove(b'~');
    percent_encoding::utf8_percent_encode(value, PATH).to_string()
}
fn percent_encoding_decode(value: &str) -> Result<String, Error> {
    for (index, byte) in value.bytes().enumerate() {
        if byte == b'%'
            && !value
                .as_bytes()
                .get(index + 1..index + 3)
                .is_some_and(|bytes| bytes.iter().all(u8::is_ascii_hexdigit))
        {
            return Err(bad("Invalid EPUB path encoding"));
        }
    }
    let value = percent_encoding::percent_decode_str(value)
        .decode_utf8()
        .map_err(bad)?
        .into_owned();
    if value.chars().any(char::is_control) || value.contains('\\') || value.contains(':') {
        return Err(bad("Invalid EPUB resource path"));
    }
    Ok(value)
}
fn read_entry(
    archive: &mut Archive,
    entries: &HashMap<String, usize>,
    name: &str,
    limit: usize,
) -> Result<Vec<u8>, Error> {
    let index = *entries
        .get(name)
        .ok_or_else(|| bad(format!("EPUB resource not found: {}", bounded(name, 256))))?;
    let mut file = archive.by_index(index).map_err(bad)?;
    if file.size() > limit as u64 {
        return Err((
            StatusCode::PAYLOAD_TOO_LARGE,
            "EPUB resource exceeds its reading budget".into(),
        ));
    }
    let mut bytes = Vec::new();
    (&mut file)
        .take((limit + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(bad)?;
    if bytes.len() > limit {
        return Err((
            StatusCode::PAYLOAD_TOO_LARGE,
            "EPUB resource grew beyond its reading budget".into(),
        ));
    }
    Ok(bytes)
}
fn read_xml(
    archive: &mut Archive,
    entries: &HashMap<String, usize>,
    name: &str,
) -> Result<String, Error> {
    String::from_utf8(read_entry(archive, entries, name, MAX_XML)?).map_err(bad)
}
fn name(node: roxmltree::Node<'_, '_>, target: &str) -> bool {
    node.is_element() && node.tag_name().name() == target
}
fn text(node: roxmltree::Node<'_, '_>) -> String {
    bounded(
        &node
            .descendants()
            .filter_map(|n| n.text().filter(|_| n.is_text()))
            .collect::<String>(),
        512,
    )
}
fn parse_book(bytes: Arc<[u8]>, id: String, root: String, path: String) -> Result<Book, Error> {
    let hash = crate::stable_content_hash(&bytes);
    let mut archive = ZipArchive::new(Cursor::new(bytes)).map_err(bad)?;
    if archive.is_empty() || archive.len() > 10_000 {
        return Err(bad("EPUB must contain at most 10,000 archive entries"));
    }
    let mut entries = HashMap::new();
    let mut expanded = 0u64;
    for index in 0..archive.len() {
        let file = archive.by_index_raw(index).map_err(bad)?;
        let raw = file.name();
        if file.encrypted() || raw.eq_ignore_ascii_case("META-INF/encryption.xml") {
            return Err(bad("Encrypted/DRM EPUB files are not supported"));
        }
        if file.enclosed_name().is_none()
            || raw.contains('\\')
            || raw.contains(':')
            || raw.chars().any(char::is_control)
            || raw.len() > 4096
            || file.unix_mode().is_some_and(|v| v & 0o170000 == 0o120000)
        {
            return Err(bad("EPUB contains an unsafe archive path"));
        }
        expanded = expanded
            .checked_add(file.size())
            .ok_or_else(|| bad("EPUB expansion overflow"))?;
        if file.size() > 64 * 1024 * 1024 || expanded > 512 * 1024 * 1024 {
            return Err(bad("EPUB expanded resources exceed their budget"));
        }
        if file.is_dir() {
            continue;
        }
        // ZIP names are already decoded names; unlike URI references, '%' is literal.
        let canonical = raw
            .split('/')
            .filter(|s| !s.is_empty() && *s != ".")
            .collect::<Vec<_>>()
            .join("/");
        if canonical.split('/').any(|s| s == "..")
            || entries.insert(encode_path(&canonical), index).is_some()
        {
            return Err(bad("EPUB contains duplicate or unsafe archive paths"));
        }
    }
    let mime = read_entry(&mut archive, &entries, "mimetype", 128)?;
    if mime != b"application/epub+zip" {
        return Err(bad("Invalid EPUB media type"));
    }
    let container = read_xml(&mut archive, &entries, "META-INF/container.xml")?;
    let container = xml(&container)?;
    let package = container
        .descendants()
        .find(|n| name(*n, "rootfile"))
        .and_then(|n| n.attribute("full-path"))
        .ok_or_else(|| bad("Missing EPUB package path"))?;
    let package = resource_path("", package)?;
    let opf = read_xml(&mut archive, &entries, &package)?;
    let opf = xml(&opf)?;
    let package_node = opf.root_element();
    let base = package.rsplit_once('/').map(|(a, _)| a).unwrap_or("");
    let metadata = package_node
        .children()
        .find(|n| name(*n, "metadata"))
        .ok_or_else(|| bad("Missing EPUB metadata"))?;
    let get = |label| {
        metadata
            .children()
            .find(|n| name(*n, label))
            .map(text)
            .unwrap_or_default()
    };
    let title = get("title");
    let author = get("creator");
    let language = get("language");
    let layout = metadata
        .children()
        .find(|n| name(*n, "meta") && n.attribute("property") == Some("rendition:layout"))
        .map(text)
        .unwrap_or_else(|| "reflowable".into());
    let items = package_node
        .children()
        .find(|n| name(*n, "manifest"))
        .ok_or_else(|| bad("Missing EPUB manifest"))?;
    let mut resources = HashMap::new();
    let mut item_ids = HashMap::new();
    let mut nav = None;
    let mut ncx = None;
    for item in items.children().filter(|n| name(*n, "item")) {
        let id = item
            .attribute("id")
            .ok_or_else(|| bad("Missing EPUB item ID"))?;
        let href = resource_path(
            base,
            item.attribute("href")
                .ok_or_else(|| bad("Missing EPUB item href"))?,
        )?;
        if id.len() > 512 || id.is_empty() || id.chars().any(char::is_control) || href.contains('#')
        {
            return Err(bad("Invalid EPUB manifest item"));
        }
        let mime = item
            .attribute("media-type")
            .unwrap_or("application/octet-stream");
        if mime.len() > 128 || !mime.is_ascii() {
            return Err(bad("Invalid EPUB resource media type"));
        }
        if item_ids.insert(id.to_owned(), href.clone()).is_some() {
            return Err(bad("Duplicate EPUB item ID"));
        }
        resources.insert(href.clone(), mime.to_owned());
        if item
            .attribute("properties")
            .is_some_and(|v| v.split_whitespace().any(|v| v == "nav"))
        {
            nav = Some(href.clone());
        }
        if mime == "application/x-dtbncx+xml" {
            ncx = Some(href);
        }
    }
    let spine_node_index = package_node
        .children()
        .filter(|n| n.is_element())
        .position(|n| name(n, "spine"))
        .ok_or_else(|| bad("Missing EPUB spine"))?;
    let spine_node = package_node.children().find(|n| name(*n, "spine")).unwrap();
    let mut spine = Vec::new();
    for item in spine_node.children().filter(|n| name(*n, "itemref")) {
        if spine.len() >= 5000 {
            return Err(bad("EPUB spine exceeds 5,000 chapters"));
        }
        let id = item
            .attribute("idref")
            .ok_or_else(|| bad("Missing EPUB chapter ID"))?;
        let href = item_ids
            .get(id)
            .ok_or_else(|| bad("EPUB spine references an unknown item"))?
            .clone();
        if !entries.contains_key(&href) {
            return Err(bad("EPUB chapter is missing from archive"));
        }
        if !matches!(
            resources[&href].as_str(),
            "application/xhtml+xml" | "text/html" | "image/svg+xml"
        ) {
            return Err(bad("Unsupported EPUB spine media type"));
        }
        spine.push(Chapter {
            id: id.into(),
            label: format!("Section {}", spine.len() + 1),
            href,
            linear: item.attribute("linear") != Some("no"),
        });
    }
    if spine.is_empty() {
        return Err(bad("EPUB contains no reading chapters"));
    }
    let mut toc = Vec::new();
    let mut toc_limited = false;
    if let Some(nav_path) = nav.or(ncx) {
        let value = read_xml(&mut archive, &entries, &nav_path)?;
        let nav_xml = xml(&value)?;
        let nav_base = nav_path.rsplit_once('/').map(|(a, _)| a).unwrap_or("");
        let html_nav = nav_xml.descendants().find(|n| {
            name(*n, "nav")
                && n.attributes()
                    .any(|a| a.name() == "type" && a.value().split_whitespace().any(|v| v == "toc"))
        });
        let section = html_nav.or_else(|| nav_xml.descendants().find(|n| name(*n, "navMap")));
        if let Some(section) = section {
            for node in section
                .descendants()
                .filter(|n| name(*n, "a") || name(*n, "navPoint"))
            {
                if toc.len() >= 2000 {
                    toc_limited = true;
                    break;
                }
                let (label, href, depth) = if name(node, "a") {
                    (
                        text(node),
                        node.attribute("href"),
                        node.ancestors()
                            .take_while(|n| *n != section)
                            .filter(|n| name(*n, "ol"))
                            .count()
                            .saturating_sub(1),
                    )
                } else {
                    (
                        node.children()
                            .find(|n| name(*n, "navLabel"))
                            .map(text)
                            .unwrap_or_default(),
                        node.children()
                            .find(|n| name(*n, "content"))
                            .and_then(|n| n.attribute("src")),
                        node.ancestors()
                            .skip(1)
                            .filter(|n| name(*n, "navPoint"))
                            .count(),
                    )
                };
                if let Some(href) = href {
                    if let Ok(href) = resource_path(nav_base, href) {
                        let resource = href.split('#').next().unwrap();
                        if resources.contains_key(resource) {
                            toc.push(Heading {
                                label,
                                href,
                                depth: depth.min(16),
                            });
                        }
                    }
                }
            }
        }
    }
    for chapter in &mut spine {
        if let Some(h) = toc
            .iter()
            .find(|h| h.href.split('#').next() == Some(&chapter.href))
        {
            chapter.label = h.label.clone();
        }
    }
    let manifest = Manifest {
        document_id: id,
        root,
        path,
        hash,
        title,
        author,
        language,
        direction: match spine_node.attribute("page-progression-direction") {
            Some("rtl") => "rtl",
            Some("ltr") => "ltr",
            _ => "default",
        }
        .into(),
        layout: if layout == "pre-paginated" {
            layout
        } else {
            "reflowable".into()
        },
        spine_node_index,
        spine,
        toc,
        toc_limited,
    };
    if serde_json::to_vec(&manifest).map_err(bad)?.len() > 6 * 1024 * 1024 {
        return Err(bad("EPUB metadata exceeds its presentation budget"));
    }
    Ok(Book {
        archive,
        entries,
        resources,
        manifest,
        saved_revision: Mutex::new(0),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    fn fixture(extra: Option<(&str, &[u8])>) -> Arc<[u8]> {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        let entries = [
            ("mimetype", "application/epub+zip"),
            (
                "META-INF/container.xml",
                r#"<container><rootfiles><rootfile full-path="OPS/package.opf"/></rootfiles></container>"#,
            ),
            (
                "OPS/package.opf",
                r#"<package><metadata><title>中文测试</title></metadata><manifest><item id="chapter" href="Text/%E4%BD%A0%E5%A5%BD%20%23.xhtml" media-type="application/xhtml+xml"/><item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/></manifest><spine><itemref idref="chapter"/></spine></package>"#,
            ),
            (
                "OPS/nav.xhtml",
                r#"<html xmlns:epub="http://www.idpf.org/2007/ops"><body><nav epub:type="toc"><ol><li><a href="Text/%E4%BD%A0%E5%A5%BD%20%23.xhtml#section">第一章</a></li></ol></nav></body></html>"#,
            ),
            (
                "OPS/Text/你好 #.xhtml",
                "<html><body><h1 id='section'>第一章</h1><p>Contents</p></body></html>",
            ),
        ];
        for (path, data) in entries {
            writer.start_file(path, options).unwrap();
            writer.write_all(data.as_bytes()).unwrap();
        }
        if let Some((path, data)) = extra {
            writer.start_file(path, options).unwrap();
            writer.write_all(data).unwrap();
        }
        writer.finish().unwrap().into_inner().into()
    }
    #[test]
    fn parses_unicode_archive_navigation_without_extracting_and_clones_only_shared_bytes() {
        let mut book = parse_book(
            fixture(None),
            uuid::Uuid::new_v4().to_string(),
            "/private".into(),
            "/private/book.epub".into(),
        )
        .unwrap();
        assert_eq!(book.manifest.title, "中文测试");
        assert_eq!(book.manifest.spine_node_index, 2);
        assert_eq!(book.manifest.toc[0].label, "第一章");
        assert!(book.manifest.toc[0].href.ends_with("%23.xhtml#section"));
        let href = book.manifest.spine[0].href.clone();
        let mut clone = book.archive.clone();
        let first = read_entry(&mut book.archive, &book.entries, &href, 1024).unwrap();
        assert_eq!(
            first,
            read_entry(&mut clone, &book.entries, &href, 1024).unwrap()
        );
        assert!(read_entry(&mut clone, &book.entries, &href, 8).is_err());
    }
    #[test]
    fn rejects_archive_escapes_encryption_and_entity_expansion() {
        for path in ["../outside", "META-INF/encryption.xml"] {
            assert!(parse_book(
                fixture(Some((path, b"bad"))),
                uuid::Uuid::new_v4().to_string(),
                "/".into(),
                "/b.epub".into()
            )
            .is_err());
        }
        assert!(xml("<!DOCTYPE a [<!ENTITY x SYSTEM 'file:///etc/passwd'>]><a>&x;</a>").is_err());
        for href in [
            "../../escape",
            "file:///etc/passwd",
            "https://example.com/a",
            "%你好",
            "%2f%2e%2e/escape",
            "bad%00file",
        ] {
            assert!(resource_path("", href).is_err(), "accepted {href}");
        }
        assert_eq!(
            resource_path("OPS/Text", "../Images/cover%20image.png").unwrap(),
            "OPS/Images/cover%20image.png"
        );
    }
}
enum Reply {
    Json(Value),
    Asset(Arc<[u8]>),
}
impl Books {
    fn book(&self, id: &str) -> Result<Arc<Book>, Error> {
        let mut cache = self.cache.lock().map_err(bad)?;
        let (book, touched) = cache.books.get_mut(id).ok_or_else(gone)?;
        *touched = Instant::now();
        Ok(book.clone())
    }
    fn save(
        book: &Book,
        state: &crate::GatewayState,
        update: ReadingUpdate,
    ) -> Result<Value, Error> {
        let mut revision = book.saved_revision.lock().map_err(bad)?;
        if update.revision == 0 {
            return Err(bad("Invalid EPUB reading revision"));
        }
        if update.revision <= *revision {
            return Ok(json!({"stale":true}));
        }
        let mut reading = update.reading;
        if reading.chapter_label.len() > 1024 {
            let mut end = 1024;
            while !reading.chapter_label.is_char_boundary(end) {
                end -= 1;
            }
            reading.chapter_label.truncate(end);
        }
        if reading.book_hash != book.manifest.hash {
            return Err(bad("EPUB reading state belongs to another revision"));
        }
        reading.root = book.manifest.root.clone();
        reading.path = book.manifest.path.clone();
        let _guard = state.epub_reading_lock.lock().map_err(bad)?;
        let saved = crate::epub_reader::save_state(
            &state.epub_reading_path,
            std::path::Path::new(&reading.path),
            reading.clone(),
        )
        .and_then(|v| serde_json::to_value(v).map_err(|e| e.to_string()))
        .map_err(bad)?;
        *revision = update.revision;
        Ok(saved)
    }
    fn process(&self, request: Request, state: &crate::GatewayState) -> Result<Reply, Error> {
        match request {
            Request::Search {
                id,
                query,
                case_sensitive,
                start,
            } => search::run(self, &id, &query, case_sensitive, start).map(Reply::Json),
            Request::Open { id, root, path } => {
                if !valid_id(&id) || root.len() > 4096 || path.len() > 8192 {
                    return Err(bad("Invalid EPUB open request"));
                }
                {
                    let cache = self.cache.lock().map_err(bad)?;
                    if cache
                        .closed
                        .iter()
                        .any(|(v, t)| v == &id && t.elapsed() < Duration::from_secs(60))
                    {
                        return Err(gone());
                    }
                    if cache.books.contains_key(&id) {
                        return Err((StatusCode::CONFLICT, "EPUB snapshot already exists".into()));
                    }
                }
                let shared = crate::shared_document_directories(state)?;
                let (root, path, _, _) = crate::resolve_epub_path_with_shared(
                    &crate::ReviewFileRequest { root, path },
                    &shared,
                )?;
                let mut bytes = Vec::new();
                std::fs::File::open(&path)
                    .map_err(bad)?
                    .take(crate::epub_reader::MAX_EPUB_BYTES + 1)
                    .read_to_end(&mut bytes)
                    .map_err(bad)?;
                if bytes.len() as u64 > crate::epub_reader::MAX_EPUB_BYTES {
                    return Err((StatusCode::PAYLOAD_TOO_LARGE, "EPUB exceeds 128 MiB".into()));
                }
                let book = Arc::new(parse_book(
                    bytes.into(),
                    id.clone(),
                    root.to_string_lossy().into(),
                    path.to_string_lossy().into(),
                )?);
                let saved = {
                    let _guard = state.epub_reading_lock.lock().map_err(bad)?;
                    crate::epub_reader::find_state(
                        &state.epub_reading_path,
                        &path,
                        &book.manifest.hash,
                    )
                    .map_err(bad)?
                };
                let mut result = serde_json::to_value(&book.manifest).map_err(bad)?;
                result["saved"] = serde_json::to_value(saved).map_err(bad)?;
                let mut cache = self.cache.lock().map_err(bad)?;
                if cache
                    .closed
                    .iter()
                    .any(|(v, t)| v == &id && t.elapsed() < Duration::from_secs(60))
                {
                    return Err(gone());
                }
                if cache.books.contains_key(&id) {
                    return Err((StatusCode::CONFLICT, "EPUB snapshot already exists".into()));
                }
                // Reclaim outside the cache lock, on this worker.
                let expired = cache
                    .books
                    .iter()
                    .filter(|(_, (_, t))| t.elapsed() > Duration::from_secs(600))
                    .map(|(id, _)| id.clone())
                    .collect::<Vec<_>>();
                let old = expired
                    .into_iter()
                    .filter_map(|id| cache.books.remove(&id))
                    .collect::<Vec<_>>();
                if cache.books.len() >= 2 {
                    drop(cache);
                    drop(old);
                    return Err((
                        StatusCode::TOO_MANY_REQUESTS,
                        "Close an existing EPUB before opening another".into(),
                    ));
                }
                cache.books.insert(id, (book, Instant::now()));
                drop(cache);
                drop(old);
                Ok(Reply::Json(result))
            }
            Request::Asset { id, path } => {
                if path.len() > 4096 || path.contains('#') || path.contains('?') {
                    return Err(bad("Invalid EPUB asset path"));
                }
                let path = resource_path("", &path)?;
                let book = self.book(&id)?;
                let mime = book
                    .resources
                    .get(&path)
                    .ok_or_else(|| bad("Resource is outside the EPUB manifest"))?;
                if !matches!(
                    mime.as_str(),
                    "application/xhtml+xml"
                        | "text/html"
                        | "text/css"
                        | "image/svg+xml"
                        | "image/png"
                        | "image/jpeg"
                        | "image/gif"
                        | "image/webp"
                        | "font/otf"
                        | "font/ttf"
                        | "font/woff"
                        | "font/woff2"
                        | "application/font-sfnt"
                        | "application/font-woff"
                        | "application/vnd.ms-opentype"
                        | "application/x-font-ttf"
                        | "application/x-font-opentype"
                ) {
                    return Err((
                        StatusCode::UNSUPPORTED_MEDIA_TYPE,
                        "EPUB resource type is not enabled for reading".into(),
                    ));
                }
                let key = (id.clone(), path.clone());
                {
                    let mut cache = self.cache.lock().map_err(bad)?;
                    if let Some(position) = cache.pages.iter().position(|(k, _)| k == &key) {
                        let entry = cache.pages.remove(position).unwrap();
                        let value = entry.1.clone();
                        cache.pages.push_back(entry);
                        return Ok(Reply::Asset(value));
                    }
                }
                let mut archive = book.archive.clone();
                let limit = if matches!(
                    mime.as_str(),
                    "application/xhtml+xml" | "text/html" | "text/css" | "image/svg+xml"
                ) {
                    8 * 1024 * 1024
                } else {
                    MAX_RESOURCE
                };
                let bytes = read_entry(&mut archive, &book.entries, &path, limit)?;
                if matches!(
                    mime.as_str(),
                    "application/xhtml+xml" | "text/html" | "image/svg+xml"
                ) && bytes
                    .iter()
                    .filter(|byte| **byte == b'<')
                    .take(100_001)
                    .count()
                    > 100_000
                {
                    return Err(bad("EPUB chapter exceeds its markup budget"));
                }
                let mut output = Vec::with_capacity(12 + mime.len() + bytes.len());
                output.extend_from_slice(b"STEPUB01");
                output.extend_from_slice(&(mime.len() as u32).to_le_bytes());
                output.extend_from_slice(mime.as_bytes());
                output.extend_from_slice(&bytes);
                let output: Arc<[u8]> = output.into();
                let mut cache = self.cache.lock().map_err(bad)?;
                if !cache.books.contains_key(&id) {
                    return Err(gone());
                }
                let mut old = Vec::new();
                while cache.pages.len() >= 16 || cache.weight + output.len() > 32 * 1024 * 1024 {
                    let Some(entry) = cache.pages.pop_front() else {
                        break;
                    };
                    cache.weight -= entry.1.len();
                    old.push(entry);
                }
                cache.weight += output.len();
                cache.pages.push_back((key, output.clone()));
                drop(cache);
                drop(old);
                Ok(Reply::Asset(output))
            }
            Request::Save { id, update } => {
                let book = self.book(&id)?;
                Ok(Reply::Json(Self::save(&book, state, update)?))
            }
            Request::Close { id, update } => {
                if !valid_id(&id) {
                    return Err(bad("Invalid EPUB snapshot ID"));
                }
                // A final reading update precedes removal in the same operation.
                let saved = if let Some(update) = update {
                    self.book(&id)
                        .and_then(|book| Self::save(&book, state, update))
                        .map(Some)
                } else {
                    Ok(None)
                };
                let mut cache = self.cache.lock().map_err(bad)?;
                let book = cache.books.remove(&id);
                cache
                    .closed
                    .retain(|(v, t)| v != &id && t.elapsed() < Duration::from_secs(60));
                while cache.closed.len() >= 128 {
                    cache.closed.pop_front();
                }
                cache.closed.push_back((id.clone(), Instant::now()));
                let mut old = Vec::new();
                let mut keep = VecDeque::new();
                while let Some(entry) = cache.pages.pop_front() {
                    if entry.0 .0 == id {
                        cache.weight -= entry.1.len();
                        old.push(entry);
                    } else {
                        keep.push_back(entry);
                    }
                }
                cache.pages = keep;
                drop(cache);
                drop((book, old));
                Ok(Reply::Json(
                    json!({"closed":true,"saveError":saved.err().map(|(_,e)|e)}),
                ))
            }
        }
    }
}
pub async fn handle(
    State(state): State<crate::GatewayState>,
    body: Result<Json<Request>, JsonRejection>,
) -> Response<Body> {
    let request = match body {
        Ok(Json(request)) => request,
        Err(_) => return crate::json_error(StatusCode::BAD_REQUEST, "Invalid native EPUB request"),
    };
    let lane = if matches!(request, Request::Close { .. }) {
        state.epubs.closing.clone()
    } else if matches!(request, Request::Search { .. }) {
        state.epubs.searching.clone()
    } else {
        state.epubs.workers.clone()
    };
    let _permit = match lane.try_acquire_owned() {
        Ok(permit) => permit,
        Err(_) => {
            return crate::json_error(
                StatusCode::TOO_MANY_REQUESTS,
                "EPUB workers are busy; try again",
            )
        }
    };
    // Metadata can be several MiB; serialization and destruction stay on a worker.
    match tokio::task::spawn_blocking(move || {
        state
            .epubs
            .clone()
            .process(request, &state)
            .and_then(|reply| match reply {
                Reply::Json(value) => serde_json::to_vec(&value)
                    .map(|v| ("application/json", Arc::<[u8]>::from(v)))
                    .map_err(bad),
                Reply::Asset(bytes) => Ok(("application/x-studio-epub-resource", bytes)),
            })
    })
    .await
    {
        Ok(Ok((mime, bytes))) => Response::builder()
            .header("content-type", mime)
            .header("cache-control", "no-store")
            .header("x-content-type-options", "nosniff")
            .body(Body::from(Bytes::from_owner(bytes)))
            .unwrap(),
        Ok(Err((status, error))) => crate::json_error(status, &error),
        Err(_) => crate::json_error(StatusCode::INTERNAL_SERVER_ERROR, "EPUB worker stopped"),
    }
}
