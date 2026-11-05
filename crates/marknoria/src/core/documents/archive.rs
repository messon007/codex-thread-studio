//! Archive readers reuse bounded parsing patterns from the MIT-licensed reader.
//! See third-party/codex-thread-studio-MIT.txt; all storage is MarkNoria-owned.
use super::*;
use calamine::{DataRef, Reader, Xlsx};
use std::collections::HashMap;

// ZIP paths are virtual POSIX names on every host, never extraction targets.
// Do not call ZIP 9.2's Windows `enclosed_name`: its DOS-name helper slices
// Unicode at byte 3 and can panic. Validate the same strict namespace ourselves.
fn safe_archive_name(name: &str) -> bool {
    if name.is_empty() || name.len() > 4096 || name.starts_with('/') || name.contains(['\\', '\0']) { return false; }
    name.strip_suffix('/').unwrap_or(name).split('/').all(|part| {
        if part.is_empty() || matches!(part, "." | "..") || part.encode_utf16().count() > 255
            || part.ends_with(['.', ' ']) || part.chars().any(|c| c.is_control() || matches!(c, '<' | '>' | ':' | '"' | '|' | '?' | '*')) { return false; }
        let stem = part.split('.').next().unwrap_or(part);
        if ["CON", "PRN", "AUX", "NUL", "CONIN$", "CONOUT$", "CLOCK$"].iter().any(|n| stem.eq_ignore_ascii_case(n)) { return false; }
        let device = stem.get(..3).is_some_and(|s| s.eq_ignore_ascii_case("COM") || s.eq_ignore_ascii_case("LPT"))
            && stem.get(3..).is_some_and(|rest| matches!(rest, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"));
        !device
    })
}

fn archive(path: &Path) -> io::Result<zip::ZipArchive<File>> {
    let mut archive = zip::ZipArchive::new(File::open(path)?).map_err(error)?;
    if archive.len() > 10_000 {
        return Err(error("Archive exceeds 10,000 entries"));
    }
    let mut total = 0u64;
    let mut names = std::collections::HashSet::new();
    for i in 0..archive.len() {
        let entry = archive.by_index_raw(i).map_err(error)?;
        let name = entry.name().map_err(|e| error(format!("Invalid archive file name: {e}")))?;
        if !safe_archive_name(&name)
            || entry.unix_mode().is_some_and(|m| m & 0o170000 == 0o120000)
            || !names.insert(name.into_owned())
        {
            return Err(error("Unsafe or duplicate archive path"));
        }
        total = total
            .checked_add(entry.size())
            .ok_or_else(|| error("Archive size overflow"))?;
        if total > 200 * 1024 * 1024 || entry.size() > 32 * 1024 * 1024 {
            return Err(error(
                "Archive exceeds 200 MiB expanded or 32 MiB per entry",
            ));
        }
    }
    Ok(archive)
}

#[cfg(test)]
mod path_tests {
    use super::*;
    #[test]
    fn virtual_zip_names_are_unicode_safe_and_platform_independent() {
        for name in ["OPS/é😀.xhtml", "中文/image.png", "META-INF/"] { assert!(safe_archive_name(name), "{name}"); }
        for name in ["../escape", "/absolute", "x/../y", "C:evil", "x\\y", "NUL.txt", "x/LPT¹", "trailing. ", "x//y"] { assert!(!safe_archive_name(name), "{name}"); }
    }
}
fn resource(archive: &mut zip::ZipArchive<File>, path: &str, limit: usize) -> io::Result<Vec<u8>> {
    let entry = archive.by_name(path).map_err(error)?;
    if entry.size() > limit as u64 {
        return Err(error("Archive resource exceeds its byte budget"));
    }
    let mut bytes = vec![];
    entry.take(limit as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > limit {
        return Err(error("Expanded resource exceeds its byte budget"));
    }
    Ok(bytes)
}

pub(super) fn web_package(directory: &Path) -> io::Result<String> {
    let mut archive = archive(&directory.join("original.bin"))?;
    let container = String::from_utf8(resource(&mut archive, "META-INF/container.xml", 2 * 1024 * 1024)?).map_err(error)?;
    let container = xml(&container)?;
    let package = container.descendants().find(|node| node.tag_name().name() == "rootfile")
        .and_then(|node| node.attribute("full-path")).ok_or_else(|| error("EPUB package missing"))?;
    let package = resolve("", package)?;
    let source = String::from_utf8(resource(&mut archive, &package, 2 * 1024 * 1024)?).map_err(error)?;
    let document = xml(&source)?;
    let chapters = document.descendants().filter(|node| node.tag_name().name() == "itemref").count();
    if chapters == 0 || chapters > 2048 { return Err(error("EPUB must contain 1–2048 spine chapters")); }
    Ok(package)
}

pub(super) fn web_resource(directory: &Path, name: &str) -> io::Result<(Vec<u8>, &'static str)> {
    if !safe_archive_name(name) { return Err(error("Invalid EPUB archive resource path")); }
    let extension = name.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    let (mime, limit) = match extension.as_str() {
        "opf" | "ncx" | "xml" => ("application/xml", 2 * 1024 * 1024),
        "html" | "htm" | "xhtml" => ("application/xhtml+xml", 2 * 1024 * 1024),
        "css" => ("text/css", 512 * 1024),
        "svg" => ("image/svg+xml", 2 * 1024 * 1024),
        "png" => ("image/png", 10 * 1024 * 1024),
        "jpg" | "jpeg" => ("image/jpeg", 10 * 1024 * 1024),
        "gif" => ("image/gif", 10 * 1024 * 1024),
        "webp" => ("image/webp", 10 * 1024 * 1024),
        "woff" => ("font/woff", 8 * 1024 * 1024), "woff2" => ("font/woff2", 8 * 1024 * 1024),
        "ttf" => ("font/ttf", 8 * 1024 * 1024), "otf" => ("font/otf", 8 * 1024 * 1024),
        _ => return Err(error("Unsupported EPUB resource type")),
    };
    let mut archive = archive(&directory.join("original.bin"))?;
    let bytes = resource(&mut archive, name, limit)?;
    if matches!(extension.as_str(), "opf" | "ncx" | "xml" | "html" | "htm" | "xhtml" | "svg") {
        let source = std::str::from_utf8(&bytes).map_err(error)?;
        let mut reader = quick_xml::Reader::from_str(source);
        let mut nodes = 0usize; let mut depth = 0usize;
        loop {
            use quick_xml::events::Event;
            match reader.read_event().map_err(error)? {
                Event::Eof => break,
                Event::Start(_) => { nodes += 1; depth += 1; },
                Event::Empty(_) | Event::Text(_) => nodes += 1,
                Event::End(_) => depth = depth.saturating_sub(1),
                Event::DocType(value) if value.as_ref().contains('[') => return Err(error("EPUB internal DTDs are not supported")),
                _ => (),
            }
            if nodes > 10000 || depth > 64 { return Err(error("EPUB resource exceeds 10,000 nodes or 64 levels")); }
        }
    }
    if matches!(extension.as_str(), "png" | "jpg" | "jpeg" | "gif" | "webp") {
        let (width, height) = image::ImageReader::new(std::io::Cursor::new(&bytes)).with_guessed_format().map_err(error)?.into_dimensions().map_err(error)?;
        if width as u64 * height as u64 > 16 * 1024 * 1024 { return Err(error("EPUB image exceeds 16 megapixels")); }
    }
    Ok((bytes, mime))
}
fn xml(source: &str) -> io::Result<roxmltree::Document<'_>> {
    roxmltree::Document::parse_with_options(
        source,
        roxmltree::ParsingOptions {
            allow_dtd: false,
            nodes_limit: 100_000,
            ..Default::default()
        },
    )
    .map_err(error)
}
// Retains the streaming-cell approach and exact scalar spellings of the reused
// table reader; it never constructs a dense Range from untrusted dimensions.
fn cell_text(value: &DataRef<'_>) -> String {
    match value {
        DataRef::String(s) | DataRef::DateTimeIso(s) | DataRef::DurationIso(s) => s.clone(),
        DataRef::SharedString(s) => (*s).to_owned(),
        DataRef::Int(n) => n.to_string(),
        DataRef::Float(n) => n.to_string(),
        DataRef::Bool(v) => v.to_string(),
        DataRef::Error(e) => e.to_string(),
        DataRef::DateTime(v) => {
            if v.is_duration() {
                v.as_duration()
                    .map(|d| d.to_string())
                    .unwrap_or_else(|| v.to_string())
            } else {
                v.as_datetime()
                    .map(|d| d.to_string())
                    .unwrap_or_else(|| v.to_string())
            }
        }
        DataRef::Empty => String::new(),
    }
}
pub(super) fn workbook(builder: &mut Builder<'_>) -> io::Result<()> {
    let path = builder.directory.join("original.bin");
    let checked = archive(&path)?;
    if checked.index_for_name("xl/workbook.xml").is_none()
        || checked.index_for_name("[Content_Types].xml").is_none()
    {
        return Err(error("XLSX package metadata missing"));
    }
    drop(checked);
    let mut book = Xlsx::new(BufReader::new(File::open(path)?)).map_err(error)?;
    let names = book.sheet_names().to_vec();
    if names.len() > 64 {
        return Err(error("Workbook exceeds 64 sheets"));
    }
    let mut projected = 0usize;
    for (id, label) in names.iter().enumerate() {
        if label.len() > 1024 || label.contains('\0') {
            return Err(error("Invalid worksheet name"));
        }
        let mut width = 0usize;
        let mut height = 0usize;
        let mut reader = book.worksheet_cells_reader(label).map_err(error)?;
        while let Some(cell) = reader.next_cell().map_err(error)? {
            let (r, c) = cell.get_position();
            height = height.max(r as usize + 1);
            width = width.max(c as usize + 1);
            if width > 256 || height > 1_000_000 {
                return Err(error("Worksheet exceeds 256 columns or 1,000,000 rows"));
            }
        }
        drop(reader);
        width = width.max(1);
        projected = projected
            .checked_add(height.max(1) * width)
            .ok_or_else(|| error("Worksheet size overflow"))?;
        if projected > 2_000_000 {
            return Err(error(
                "Workbook projection exceeds 2,000,000 cells; split sparse or large sheets",
            ));
        }
        let mut reader = book.worksheet_cells_reader(label).map_err(error)?;
        let data_file = format!("table-{id}.jsonl");
        let mut output = BufWriter::new(File::create(builder.directory.join(&data_file))?);
        let mut headers = vec![String::new(); width];
        let mut row = vec![String::new(); width];
        let mut current = 0usize;
        let emit = |number: usize,
                    row: &Vec<String>,
                    out: &mut BufWriter<File>,
                    headers: &mut Vec<String>|
         -> io::Result<()> {
            if number == 0 {
                *headers = row.clone();
                return Ok(());
            }
            let record = TableRecord {
                line: number + 1,
                cells: row.clone(),
            };
            let bytes = serde_json::to_vec(&record)?;
            if bytes.len() > 128 * 1024 {
                return Err(error("Worksheet row exceeds 128 KiB"));
            }
            out.write_all(&bytes)?;
            out.write_all(b"\n")?;
            Ok(())
        };
        while let Some(cell) = reader.next_cell().map_err(error)? {
            let (r, c) = cell.get_position();
            let r = r as usize;
            if r < current {
                return Err(error("Worksheet cells are out of order"));
            }
            while current < r {
                emit(current, &row, &mut output, &mut headers)?;
                row.fill(String::new());
                current += 1;
            }
            let text = cell_text(cell.get_value());
            if text.len() > 64 * 1024 || text.contains('\0') {
                return Err(error("Worksheet cell exceeds 64 KiB or contains NUL"));
            }
            row[c as usize] = text;
        }
        if height > 0 {
            emit(current, &row, &mut output, &mut headers)?;
        }
        output.flush()?;
        for (i, name) in headers.iter_mut().enumerate() {
            if name.is_empty() {
                *name = format!("Column {}", i + 1);
            }
        }
        builder.table(label, headers, data_file)?;
    }
    Ok(())
}
// Adapted from the reusable EPUB path resolver, retaining percent-decoding and
// archive-only path confinement; no ZIP member becomes a filesystem pathname.
fn decode(value: &str) -> io::Result<String> {
    let mut out = vec![];
    let mut i = 0;
    let bytes = value.as_bytes();
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let h = bytes
                .get(i + 1..i + 3)
                .ok_or_else(|| error("Invalid EPUB path encoding"))?;
            let h = std::str::from_utf8(h).map_err(error)?;
            out.push(u8::from_str_radix(h, 16).map_err(error)?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    let out = String::from_utf8(out).map_err(error)?;
    if out.chars().any(char::is_control) || out.contains(['\\', ':']) {
        return Err(error("Invalid EPUB resource path"));
    }
    Ok(out)
}
fn resolve(base: &str, href: &str) -> io::Result<String> {
    let href = href.split('#').next().unwrap_or("");
    if href.contains(['?', ':', '\\']) || href.starts_with("//") {
        return Err(error("EPUB resource must be a local archive path"));
    }
    let href = decode(href)?;
    let mut parts = if href.starts_with('/') {
        vec![]
    } else {
        base.split('/')
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .collect::<Vec<_>>()
    };
    for part in href.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                if parts.pop().is_none() {
                    return Err(error("EPUB resource escapes archive"));
                }
            }
            p => parts.push(p.into()),
        }
    }
    let path = parts.join("/");
    if path.is_empty() || path.len() > 4096 {
        return Err(error("Invalid EPUB path"));
    }
    Ok(path)
}
pub(super) fn epub(builder: &mut Builder<'_>) -> io::Result<()> {
    let mut archive = archive(&builder.directory.join("original.bin"))?;
    let container = String::from_utf8(resource(
        &mut archive,
        "META-INF/container.xml",
        2 * 1024 * 1024,
    )?)
    .map_err(error)?;
    let doc = xml(&container)?;
    let package = doc
        .descendants()
        .find(|n| n.tag_name().name() == "rootfile")
        .and_then(|n| n.attribute("full-path"))
        .ok_or_else(|| error("EPUB package missing"))?;
    let package = resolve("", package)?;
    let base = package
        .rsplit_once('/')
        .map_or("", |(base, _)| base)
        .to_owned();
    let opf =
        String::from_utf8(resource(&mut archive, &package, 2 * 1024 * 1024)?).map_err(error)?;
    let doc = xml(&opf)?;
    let mut manifest = HashMap::new();
    let mut spine = vec![];
    for node in doc.descendants() {
        match node.tag_name().name() {
            "item" => {
                if let (Some(id), Some(href)) = (node.attribute("id"), node.attribute("href")) {
                    manifest.insert(id.to_owned(), resolve(&base, href)?);
                }
            }
            "itemref" => {
                if let Some(id) = node.attribute("idref") {
                    spine.push(id.to_owned());
                }
            }
            _ => {}
        }
    }
    if spine.is_empty() || spine.len() > 2048 {
        return Err(error("EPUB must contain 1–2048 spine chapters"));
    }
    let mut image_ids = HashMap::<String, usize>::new();
    for (chapter, id) in spine.iter().enumerate() {
        let path = manifest
            .get(id)
            .ok_or_else(|| error("EPUB spine references missing chapter"))?;
        let source =
            String::from_utf8(resource(&mut archive, path, 16 * 1024 * 1024)?).map_err(error)?;
        let dom = html::document(&source)?;
        let label = dom
            .select_first("h1,h2,title")
            .ok()
            .map(|n| n.text_contents())
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| format!("Chapter {}", chapter + 1));
        builder.heading(label.trim(), 1)?;
        let body = dom
            .select_first("body")
            .map(|n| n.as_node().clone())
            .unwrap_or_else(|_| dom.clone());
        for child in body.children() {
            let (h, t) = html::clean(&child, 0, &mut |src, alt| {
                let base = path.rsplit_once('/').map_or("", |(base, _)| base);
                let path = match resolve(base, src) {
                    Ok(path) => path,
                    Err(_) => return Ok(None),
                };
                let id = if let Some(id) = image_ids.get(&path) {
                    *id
                } else {
                    if matches!(
                        archive.by_name(&path),
                        Err(zip::result::ZipError::FileNotFound)
                    ) {
                        return Ok(None);
                    }
                    let bytes = resource(&mut archive, &path, 10 * 1024 * 1024)?;
                    let id = builder.image(&bytes, alt)?;
                    image_ids.insert(path, id);
                    id
                };
                Ok(Some(builder.figure(id)))
            })?;
            if h.is_empty() && t.is_empty() {
                continue;
            }
            if h.len() > crate::core::MAX_HTML {
                builder.paragraphs(&t)?;
            } else {
                builder.rich(&h, &t)?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn resources_are_confined_even_after_decoding() {
        assert_eq!(
            resolve("OPS/text", "../images/a%20b.png").unwrap(),
            "OPS/images/a b.png"
        );
        for p in [
            "../../../escape",
            "https://host/a",
            "%2e%2e/%2e%2e/a",
            "%5cetc",
            "%00file",
            "%3afoo",
            "bad%GG",
        ] {
            assert!(resolve("OPS", p).is_err(), "{p}");
        }
    }
}
