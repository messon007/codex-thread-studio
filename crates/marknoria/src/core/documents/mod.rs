//! Format-specific, toolkit-free readers over the shared immutable page cache.
use crate::core::{Heading, Index, Page, Table, document_format::DocumentFormat};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::{self, BufRead, BufReader, BufWriter, Read, Write},
    path::{Path, PathBuf},
    sync::Arc,
    time::Instant,
};
mod archive;
mod html;
mod image_decode;
mod pdf;
pub mod structured;
mod json_stream;
pub(crate) mod projection;
pub(crate) const TEXT_PAGE_BYTES: usize = 16 * 1024;
mod svg_resources;
#[cfg(test)]
mod tests;

#[derive(Clone, Debug)]
pub struct TextDocument(Arc<str>);
impl TextDocument {
    pub fn new(text: String) -> Result<Self, String> {
        if text.len() > crate::core::MAX_SOURCE as usize {
            return Err("Text exceeds 128 MiB".into());
        }
        Ok(Self(text.into()))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
    pub fn bytes(&self) -> usize {
        self.0.len()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetFrame {
    pub file: String,
    pub delay_ms: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Asset {
    pub alt: String,
    pub width: u32,
    pub height: u32,
    pub frames: Vec<AssetFrame>,
    pub plays: u32,
    pub pdf_page: Option<usize>,
}
#[derive(Serialize, Deserialize)]
pub struct TableRecord {
    pub line: usize,
    pub cells: Vec<String>,
}
pub(super) fn error(e: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, e.to_string())
}
pub fn escaped(text: &str) -> String {
    text.chars()
        .map(|c| {
            if c < ' ' && !matches!(c, '\r' | '\n' | '\t') {
                char::from_u32(0x2400 + c as u32).unwrap()
            } else {
                c
            }
        })
        .collect::<String>()
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
fn name(path: &Path) -> String {
    path.file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .chars()
        .take(240)
        .collect()
}
fn hash(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn publish(directory: &Path, index: &Index) -> io::Result<()> {
    fs::write(directory.join("index.pending"), serde_json::to_vec(index)?)?;
    fs::rename(
        directory.join("index.pending"),
        directory.join("index.json"),
    )
}
pub fn text(
    path: &Path,
    directory: &Path,
    source: &str,
    starts: &[usize],
    format: DocumentFormat,
    started: Instant,
) -> io::Result<Index> {
    if format == DocumentFormat::Html && source.len() <= 64 * 1024 {
        return html::standalone(path, directory, source, starts, started);
    }
    let mut index = Index {
        version: 7,
        text_pages: true,
        format,
        name: name(path),
        revision: hash(source.as_bytes()),
        source_bytes: source.len(),
        source_lines: starts.len(),
        ..Default::default()
    };
    File::create(directory.join("diagrams.src"))?;
    let path_name = path.to_string_lossy();
    let structured_format = structured::Format::for_path(&path_name);
    let jsonl = structured_format.is_some_and(|f| f.label == "JSONL");
    let json = structured_format.filter(|f| f.language == structured::Language::Json && !jsonl);
    let mut writer = projection::Writer::new(directory, source, starts, source.len().saturating_mul(4).max(1024))?;
    if let Some(json) = json {
        if let Err(reason) = json_stream::format(source, json.label, &mut writer)? {
            // Throw away a failed projection once; serve the immutable source.
            // Readers never rescan the original document to retry formatting.
            drop(writer);
            writer = projection::Writer::new(directory, source, starts, source.len())?;
            if !writer.copy(0..source.len())? { return Err(error("Document exceeds the bounded cache budget")); }
            fs::write(directory.join("preview-note.txt"), format!("{} preview shows original source: {}. Source and Edit retain the original bytes.", json.label, reason))?;
        }
    } else {
        let records: Box<dyn Iterator<Item = &str>> = if jsonl && !source.is_empty() {
            Box::new(source.split_inclusive('\n'))
        } else { Box::new(std::iter::once(source)) };
        // JSONL retains its per-record expansion limit. The page writer also
        // bounds aggregate cache bytes and DOM/source anchors per page.
        writer = projection::Writer::new(directory, source, starts, if jsonl { source.len().saturating_mul(4) } else { source.len().max(structured::OUTPUT_BYTES) })?;
        let mut source_at = 0;
        for record in records {
            let preview = if record.len() <= structured::FORMAT_BYTES {
                structured::Preview::prepare_bounded(&path_name,
                    &TextDocument::new(record.into()).map_err(error)?,
                    if jsonl { record.len().saturating_mul(4) } else { structured::OUTPUT_BYTES })
            } else { None };
            if let Some(p) = preview.as_ref().filter(|p| p.formatted) {
                let mut display_at = 0;
                for segment in p.map.iter() {
                    if !writer.generated(&p.document.as_str()[display_at..segment.display.start])?
                        || !writer.copy(source_at + segment.original..source_at + segment.original + segment.display.len())? {
                        return Err(error("Document exceeds the bounded projection cache budget"));
                    }
                    display_at = segment.display.end;
                }
                if !writer.generated(&p.document.as_str()[display_at..])? { return Err(error("Document exceeds the bounded projection cache budget")); }
            } else if !writer.copy(source_at..source_at + record.len())? { return Err(error("Document exceeds the bounded projection cache budget")); }
            source_at += record.len();
        }
    }
    index.pages = writer.finish()?;
    index.index_ms = started.elapsed().as_secs_f64() * 1000.0;
    index.peak_rss_kib = crate::core::peak_rss_kib();
    publish(directory, &index)?;
    Ok(index)
}

pub(crate) fn text_page_html(directory: &Path, index: &Index, page: usize, chunk: &str) -> io::Result<String> {
    let language = crate::core::highlight::language_for_name(&index.name);
    let anchors = projection::read(directory, index, page, chunk)?;
    let mut html = crate::core::highlight::code_mapped(&language, chunk, &anchors);
    if page == 0 {
        if let Some(note) = preview_note(directory)? {
            html.insert_str(0, &format!("<p class=\"document-preview-note\" role=\"status\">{}</p>", escaped(&note)));
        }
    }
    Ok(html)
}

pub(crate) fn preview_note(directory: &Path) -> io::Result<Option<String>> {
    match File::open(directory.join("preview-note.txt")) {
        Ok(file) => {
            let mut note = String::new();
            file.take(1024).read_to_string(&mut note)?;
            Ok(Some(note))
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

pub(super) struct Builder<'a> {
    pub directory: &'a Path,
    pub index: Index,
    html: BufWriter<File>,
    source: BufWriter<File>,
    starts: Vec<usize>,
    offset: u64,
    text_bytes: usize,
    asset_bytes: u64,
    started: Instant,
    source_images:bool,
}
impl<'a> Builder<'a> {
    fn new(
        path: &Path,
        directory: &'a Path,
        format: DocumentFormat,
        bytes: &[u8],
        started: Instant,
    ) -> io::Result<Self> {
        fs::create_dir_all(directory)?;
        fs::write(directory.join("original.bin"), bytes)?;
        File::create(directory.join("diagrams.src"))?;
        Ok(Self {
            directory,
            index: Index {
                version: 6,
                format,
                name: name(path),
                revision: hash(bytes),
                original_bytes: Some(bytes.len()),
                ..Default::default()
            },
            html: BufWriter::new(File::create(directory.join("pages.html"))?),
            source: BufWriter::new(File::create(directory.join("source.md"))?),
            starts: vec![0],
            offset: 0,
            text_bytes: 0,
            asset_bytes: 0,
            started,
            source_images:false,
        })
    }
    fn source(&mut self, text: &str) -> io::Result<(usize, usize)> {
        let start = self.text_bytes;
        if start + text.len() > crate::core::MAX_SOURCE as usize {
            return Err(error("Extracted text exceeds 128 MiB"));
        }
        self.starts
            .extend(text.match_indices('\n').map(|(at, _)| start + at + 1));
        self.source.write_all(text.as_bytes())?;
        self.text_bytes += text.len();
        Ok((start, self.text_bytes))
    }
    fn page(
        &mut self,
        html: &str,
        start: usize,
        end: usize,
        table: Option<usize>,
        rows: usize,
        row_start: usize,
        columns: usize,
    ) -> io::Result<usize> {
        if html.len() > crate::core::MAX_HTML
            || self.offset + html.len() as u64 > 512 * 1024 * 1024
            || self.index.pages.len() >= 100_000
        {
            return Err(error("Document exceeds the bounded page/cache budget"));
        }
        let id = self.index.pages.len();
        self.index.pages.push(Page {
            start,
            end,
            line: self.starts.partition_point(|n| *n <= start).max(1),
            offset: self.offset,
            bytes: html.len(),
            rows,
            row_start,
            columns,
            table,
        });
        self.html.write_all(html.as_bytes())?;
        self.offset += html.len() as u64;
        Ok(id)
    }
    pub fn rich(&mut self, html: &str, text: &str) -> io::Result<usize> {
        let (start, end) = self.source(text)?;
        self.page(html, start, end, None, 0, 0, 0)
    }
    pub fn heading(&mut self, label: &str, level: u8) -> io::Result<()> {
        let line = self.starts.len();
        let id = self.index.toc.len();
        let page = self.rich(
            &format!(
                "<h{level} id=\"heading-{id}\" data-source-line=\"{line}\">{}</h{level}>",
                escaped(label)
            ),
            &format!("{label}\n"),
        )?;
        self.index.toc.push(Heading {
            id,
            level,
            label: label.chars().take(240).collect(),
            line,
            page,
        });
        Ok(())
    }
    pub fn paragraphs(&mut self, text: &str) -> io::Result<()> {
        let mut at = 0;
        while at < text.len() {
            let mut end = (at + 8192).min(text.len());
            while !text.is_char_boundary(end) {
                end -= 1;
            }
            let chunk = &text[at..end];
            self.rich(
                &format!("<pre><code>{}</code></pre>", escaped(chunk)),
                chunk,
            )?;
            at = end;
        }
        Ok(())
    }
    pub fn table(
        &mut self,
        label: &str,
        headers: Vec<String>,
        data_file: String,
    ) -> io::Result<()> {
        self.heading(label, 2)?;
        let id = self.index.tables.len();
        let columns = headers.len().max(1);
        if columns > 256 {
            return Err(error("Table exceeds 256 columns"));
        }
        let headers = if headers.is_empty() {
            vec!["Column 1".into()]
        } else {
            headers
        };
        let line = self.starts.len();
        let (start, end) = self.source(&format!("{}\n", serde_json::to_string(&headers)?))?;
        let header = format!(
            "<tr data-source-start=\"{start}\" data-source-end=\"{}\" data-source-line=\"{line}\" data-source-end-line=\"{line}\">{}</tr>",
            end.saturating_sub(1),
            headers
                .iter()
                .map(|c| format!("<th>{}</th>", crate::core::csv_document::cell(c)))
                .collect::<String>()
        );
        let prefix = format!("<table><thead>{header}</thead><tbody>");
        let suffix = "</tbody></table>";
        let mut meta = Table {
            label: Some(label.into()),
            data_file: Some(data_file.clone()),
            first_page: self.index.pages.len(),
            end_page: 0,
            rows: 0,
            columns: headers,
            header_bytes: header.len(),
            max_row_bytes: 0,
        };
        let mut body = String::new();
        let mut count = 0;
        let mut page_start = start;
        let mut page_end = end;
        for record in BufReader::new(File::open(self.directory.join(&data_file))?).lines() {
            let record = record?;
            let parsed: TableRecord = serde_json::from_str(&record)?;
            if parsed.cells.len() != columns {
                return Err(error("Worksheet width changed"));
            }
            let line = self.starts.len();
            let (s, e) = self.source(&format!("{record}\n"))?;
            let row = format!(
                "<tr data-source-start=\"{s}\" data-source-end=\"{}\" data-source-line=\"{line}\" data-source-end-line=\"{line}\">{}</tr>",
                e.saturating_sub(1),
                parsed
                    .cells
                    .iter()
                    .map(|c| format!("<td>{}</td>", crate::core::csv_document::cell(c)))
                    .collect::<String>()
            );
            if prefix.len() + row.len() + suffix.len() > crate::core::MAX_HTML {
                return Err(error("A worksheet row exceeds the 128 KiB display budget"));
            }
            if count > 0
                && (count >= (512 / columns).clamp(1, 16)
                    || body.len() + row.len() > 16 * 1024
                    || prefix.len() + body.len() + row.len() + suffix.len() > crate::core::MAX_HTML)
            {
                self.page(
                    &format!("{prefix}{body}{suffix}"),
                    page_start,
                    page_end,
                    Some(id),
                    count,
                    meta.rows,
                    columns,
                )?;
                meta.rows += count;
                body.clear();
                count = 0;
                page_start = s;
            }
            meta.max_row_bytes = meta.max_row_bytes.max(row.len());
            body.push_str(&row);
            count += 1;
            page_end = e;
        }
        self.page(
            &format!("{prefix}{body}{suffix}"),
            page_start,
            page_end,
            Some(id),
            count,
            meta.rows,
            columns,
        )?;
        meta.rows += count;
        meta.end_page = self.index.pages.len();
        self.index.tables.push(meta);
        Ok(())
    }
    pub fn image(&mut self, bytes: &[u8], alt: &str) -> io::Result<usize> {
        if self.index.assets.len() >= 2048 || bytes.len() > 25 * 1024 * 1024 {
            return Err(error("Image exceeds the resource budget"));
        }
        let id = self.index.assets.len();
        let mime = if bytes.starts_with(b"\x89PNG") {
            "image/png"
        } else if bytes.starts_with(b"GIF8") {
            "image/gif"
        } else if bytes.starts_with(b"RIFF") {
            "image/webp"
        } else if bytes.starts_with(b"\xff\xd8") {
            "image/jpeg"
        } else {
            "image/svg+xml"
        };
        let animation = if self.source_images { image_decode::animation::decode_resolution(bytes,mime,true) } else { image_decode::animation::decode(bytes,mime) }.map_err(error)?;
        let raw = if let Some(animation) = animation {
            animation
        } else {
            if self.source_images { image_decode::decode_resolution(bytes,mime,true) } else { image_decode::decode(bytes,mime) }.map_err(error)?
        };
        let value = |at: usize| u32::from_le_bytes(raw[at..at + 4].try_into().unwrap());
        let (width, height) = (value(16), value(20));
        let animated = raw.starts_with(b"MNIMA001");
        let count = if animated { value(24) as usize } else { 1 };
        let plays = if animated { value(28) } else { 1 };
        let size = width as usize * height as usize * 4;
        let mut at = if animated { 32 } else { 24 };
        let mut frames = vec![];
        for frame in 0..count {
            let delay = if animated {
                let d = value(at);
                at += 4;
                d
            } else {
                100
            };
            let mut rgba = raw[at..at + size].to_vec();
            at += size;
            for pixel in rgba.chunks_exact_mut(4) {
                let alpha = u32::from(pixel[3]);
                if alpha > 0 {
                    for channel in &mut pixel[..3] {
                        *channel = ((u32::from(*channel) * 255 + alpha / 2) / alpha).min(255) as u8;
                    }
                }
            }
            let encoded = encode_png(width, height, rgba)?;
            if self.source_images && encoded.len()>20*1024*1024 { return Err(error("Full-resolution image frame exceeds the 20 MiB transport budget")); }
            self.asset_bytes += encoded.len() as u64;
            if self.asset_bytes > 128 * 1024 * 1024 {
                return Err(error("Image cache exceeds 128 MiB"));
            }
            let file = format!("asset-{id}-{frame}.png");
            fs::write(self.directory.join(&file), encoded)?;
            frames.push(AssetFrame {
                file,
                delay_ms: delay,
            });
        }
        self.index.assets.push(Asset {
            alt: alt.chars().take(256).collect(),
            width,
            height,
            frames,
            plays,
            pdf_page: None,
        });
        Ok(id)
    }
    pub fn figure(&self, id: usize) -> String {
        let a = &self.index.assets[id];
        format!(
            "<figure data-asset-id=\"{id}\" data-asset-width=\"{}\" data-asset-height=\"{}\"><div class=\"document-image\"></div><figcaption>{}</figcaption></figure>",
            a.width,
            a.height,
            escaped(&a.alt)
        )
    }
    fn finish(mut self) -> io::Result<Index> {
        if self.index.pages.is_empty() {
            self.rich("<p>Empty document</p>", "")?;
        }
        self.html.flush()?;
        self.source.flush()?;
        let mut lines = BufWriter::new(File::create(self.directory.join("lines.bin"))?);
        for at in &self.starts {
            lines.write_all(&(*at as u64).to_le_bytes())?;
        }
        lines.flush()?;
        self.index.source_bytes = self.text_bytes;
        self.index.source_lines = self.starts.len();
        self.index.index_ms = self.started.elapsed().as_secs_f64() * 1000.0;
        self.index.peak_rss_kib = crate::core::peak_rss_kib();
        publish(self.directory, &self.index)?;
        Ok(self.index)
    }
}
fn encode_png(width: u32, height: u32, rgba: Vec<u8>) -> io::Result<Vec<u8>> {
    let image = image::RgbaImage::from_raw(width, height, rgba)
        .ok_or_else(|| error("Invalid image frame"))?;
    let mut data = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(image)
        .write_to(&mut data, image::ImageFormat::Png)
        .map_err(error)?;
    Ok(data.into_inner())
}
pub fn build(
    path: &Path,
    directory: &Path,
    format: DocumentFormat,
    started: Instant,
) -> io::Result<Index> {
    build_resolution(path,directory,format,started,false)
}
pub(super) fn build_resolution(path:&Path,directory:&Path,format:DocumentFormat,started:Instant,source_images:bool) -> io::Result<Index> {
    let file = File::open(path)?;
    if !file.metadata()?.is_file() || file.metadata()?.len() > crate::core::MAX_SOURCE {
        return Err(error("Expected a regular document of at most 128 MiB"));
    }
    let mut bytes = vec![];
    file.take(crate::core::MAX_SOURCE + 1).read_to_end(&mut bytes)?;
    if bytes.len() > crate::core::MAX_SOURCE as usize {
        return Err(error("Document exceeds 128 MiB"));
    }
    let mut builder = Builder::new(path, directory, format, &bytes, started)?;
    builder.source_images=source_images;
    match format {
        DocumentFormat::Image => {
            let id = builder.image(&bytes, &name(path))?;
            builder.rich(&builder.figure(id), &format!("{}\n", name(path)))?;
        }
        DocumentFormat::Xlsx => archive::workbook(&mut builder)?,
        DocumentFormat::Epub => archive::epub(&mut builder)?,
        DocumentFormat::Pdf => pdf::index(&mut builder)?,
        _ => return Err(error("Invalid binary document format")),
    }
    builder.finish()
}

/// Immutable original-container snapshot. PDF.js owns page discovery, text and
/// rendering in the Web worker; native/indexed readers keep the Poppler path.
pub(crate) fn web_pdf(path: &Path, directory: &Path, started: Instant) -> io::Result<Index> {
    let file = File::open(path)?;
    if !file.metadata()?.is_file() || file.metadata()?.len() > crate::core::MAX_SOURCE {
        return Err(error("Expected a regular PDF of at most 128 MiB"));
    }
    let mut bytes = Vec::new();
    file.take(crate::core::MAX_SOURCE + 1).read_to_end(&mut bytes)?;
    if bytes.len() > crate::core::MAX_SOURCE as usize || !bytes.iter().take(1024).copied().collect::<Vec<_>>().windows(5).any(|s| s == b"%PDF-") {
        return Err(error("Expected a PDF header within the first 1,024 bytes and at most 128 MiB"));
    }
    let mut builder = Builder::new(path, directory, DocumentFormat::Pdf, &bytes, started)?;
    builder.index.renderer = Some(crate::core::DocumentRenderer::Pdfjs);
    builder.rich("<p>Loading PDF pages…</p>", "")?;
    builder.finish()
}

pub(crate) fn web_epub(path: &Path, directory: &Path, started: Instant) -> io::Result<Index> {
    let file = File::open(path)?;
    if !file.metadata()?.is_file() || file.metadata()?.len() > crate::core::MAX_SOURCE { return Err(error("Expected a regular EPUB of at most 128 MiB")); }
    let mut bytes = Vec::new(); file.take(crate::core::MAX_SOURCE + 1).read_to_end(&mut bytes)?;
    if bytes.len() > crate::core::MAX_SOURCE as usize { return Err(error("EPUB exceeds 128 MiB")); }
    let mut builder = Builder::new(path, directory, DocumentFormat::Epub, &bytes, started)?;
    builder.index.epub_package = Some(archive::web_package(directory)?);
    builder.index.renderer = Some(crate::core::DocumentRenderer::Epubjs);
    builder.rich("<p>Loading EPUB chapters…</p>", "")?;
    builder.finish()
}

/// Reads only a confined member of this immutable EPUB snapshot. No member is
/// extracted as a filesystem pathname and no adjacent-file grant is implied.
pub fn epub_resource(directory: &Path, index: &Index, name: &str) -> io::Result<(Vec<u8>, &'static str)> {
    if index.format != DocumentFormat::Epub { return Err(error("EPUB resources require an EPUB snapshot")); }
    archive::web_resource(directory, name)
}
pub fn asset(directory: &Path, index: &Index, id: usize, frame: usize) -> io::Result<PathBuf> {
    let item = index
        .assets
        .get(id)
        .ok_or_else(|| error("Image out of range"))?;
    let image = item
        .frames
        .get(frame)
        .ok_or_else(|| error("Image frame out of range"))?;
    let path = directory.join(&image.file);
    if let Some(page) = item.pdf_page {
        if !path.is_file() {
            pdf::render(directory, page, &path)?;
        }
    }
    if path.metadata()?.len() > 20 * 1024 * 1024 {
        return Err(error("Image output exceeds 20 MiB"));
    }
    Ok(path)
}
pub fn audit(directory: &Path, index: &Index) -> io::Result<crate::core::TableAudit> {
    let mut report = crate::core::TableAudit {
        tables: index.tables.len(),
        rows: 0,
        cells: 0,
        fragments: 0,
    };
    for (id, table) in index.tables.iter().enumerate() {
        let file = table
            .data_file
            .as_ref()
            .ok_or_else(|| error("Missing worksheet source cache"))?;
        let mut expected = Sha256::new();
        let mut rows = 0;
        for record in BufReader::new(File::open(directory.join(file))?).lines() {
            let record: TableRecord = serde_json::from_str(&record?)?;
            expected.update(b"<tr>");
            for cell in record.cells {
                expected.update(format!("<td>{}</td>", crate::core::csv_document::cell(&cell)));
            }
            expected.update(b"</tr>");
            rows += 1;
        }
        if rows != table.rows {
            return Err(error("Worksheet row count mismatch"));
        }
        let mut actual = Sha256::new();
        for page in table.first_page..table.end_page {
            let html = crate::core::read_page(directory, index, page)?;
            let (r, c) = crate::core::audit_body(&mut actual, &html)?;
            if index.pages[page].table != Some(id)
                || r != index.pages[page].rows
                || c != r * table.columns.len()
            {
                return Err(error("Worksheet page metadata mismatch"));
            }
            report.fragments += 1;
            report.rows += r;
            report.cells += c;
        }
        if expected.finalize() != actual.finalize() {
            return Err(error("Worksheet cell content differs from its snapshot"));
        }
    }
    Ok(report)
}
