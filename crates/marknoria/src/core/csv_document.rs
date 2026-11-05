//! CSV records become the same bounded, source-anchored table cache as Markdown.
use crate::core::{Index, Page, Table};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::{self, BufWriter, Write},
    path::Path,
    time::Instant,
};

pub fn is_csv(path: &Path) -> bool {
    crate::core::document_format::DocumentFormat::for_path(path)
        == Some(crate::core::document_format::DocumentFormat::Csv)
}
fn error(e: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, e.to_string())
}
fn reader(source: &str, delimiter: u8) -> csv::Reader<&[u8]> {
    csv::ReaderBuilder::new()
        .delimiter(delimiter)
        .flexible(true)
        .from_reader(source.as_bytes())
}
pub(crate) fn cell(value: &str) -> String {
    let visible: String = value
        .chars()
        .map(|c| {
            if c < ' ' && !matches!(c, '\r' | '\n' | '\t') {
                char::from_u32(0x2400 + c as u32).unwrap()
            } else {
                c
            }
        })
        .collect();
    visible
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\n', "<br>")
}
pub(crate) fn header_start(source: &str, end: usize) -> usize {
    let mut start = if source.starts_with('\u{feff}') { 3 } else { 0 };
    let beginning = start;
    while start < end && matches!(source.as_bytes()[start], b'\r' | b'\n') {
        start += 1;
    }
    if start == beginning || end == 0 {
        0
    } else {
        start
    }
}
fn row_tag(source: &str, starts: &[usize], start: usize, mut end: usize) -> String {
    while end > start && matches!(source.as_bytes()[end - 1], b'\r' | b'\n') {
        end -= 1;
    }
    let range = crate::core::source_range(starts, start..end);
    format!(
        "<tr data-source-start=\"{start}\" data-source-end=\"{end}\" data-source-line=\"{}\" data-source-end-line=\"{}\">",
        range.line, range.end_line
    )
}
pub(crate) fn build(
    path: &Path,
    directory: &Path,
    source: &str,
    starts: &[usize],
    started: Instant,
) -> io::Result<Index> {
    if source.contains('\0') {
        return Err(error("CSV requires UTF-8 text without NUL bytes"));
    }
    let format = crate::core::document_format::DocumentFormat::for_path(path).unwrap();
    let delimiter = format.delimiter().unwrap();
    let mut scan = reader(source, delimiter);
    let mut names: Vec<String> = scan
        .headers()
        .map_err(error)?
        .iter()
        .map(str::to_owned)
        .collect();
    let header_end = if names.is_empty() {
        0
    } else {
        scan.position().byte() as usize
    };
    let header_start = header_start(source, header_end);
    let mut count = names.len();
    let mut record = csv::StringRecord::new();
    while scan.read_record(&mut record).map_err(error)? {
        count = count.max(record.len());
        if count > 256 {
            return Err(error("CSV tables support at most 256 columns"));
        }
    }
    if count > 256 {
        return Err(error("CSV tables support at most 256 columns"));
    }
    count = count.max(1);
    while names.len() < count {
        names.push(format!("Column {}", names.len() + 1));
    }
    let mut header = row_tag(source, starts, header_start, header_end);
    for name in &names {
        header.push_str(&format!("<th>{}</th>", cell(name)));
    }
    header.push_str("</tr>");
    if header.len() > crate::core::MAX_HTML / 2 {
        return Err(error("CSV header exceeds the 64 KiB display budget"));
    }
    let prefix = format!("<table><thead>{header}</thead><tbody>");
    let suffix = "</tbody></table>";
    let mut output = BufWriter::new(File::create(directory.join("pages.html"))?);
    File::create(directory.join("diagrams.src"))?;
    let original_name = path.file_name().unwrap_or_default().to_string_lossy();
    // The bounded display name also identifies the table's CSV format in
    // older consumers. Never truncate its extension away.
    let name = if original_name.chars().count() > 240 {
        format!(
            "{}….{}",
            original_name.chars().take(235).collect::<String>(),
            if format == crate::core::document_format::DocumentFormat::Tsv {
                "tsv"
            } else {
                "csv"
            }
        )
    } else {
        original_name.into_owned()
    };
    let mut index = Index {
        format,
        version: 5,
        name,
        revision: Sha256::digest(source.as_bytes())
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect(),
        source_bytes: source.len(),
        source_lines: starts.len(),
        ..Default::default()
    };
    let mut table = Table {
        label: None,
        data_file: None,
        first_page: 0,
        end_page: 0,
        rows: 0,
        columns: names,
        max_row_bytes: 0,
        header_bytes: header.len(),
    };
    let mut offset = 0u64;
    let mut body = String::new();
    let mut row_count = 0usize;
    let mut page_start = 0usize;
    let mut page_end = header_end;
    let limit = (512 / count).clamp(1, 16);
    let flush = |output: &mut BufWriter<File>,
                 index: &mut Index,
                 table: &mut Table,
                 body: &mut String,
                 rows: usize,
                 start: usize,
                 end: usize,
                 offset: &mut u64|
     -> io::Result<()> {
        let html = format!("{prefix}{body}{suffix}");
        if html.len() > crate::core::MAX_HTML
            || *offset + html.len() as u64 > 512 * 1024 * 1024
            || index.pages.len() >= 100_000
        {
            return Err(error("CSV exceeds the bounded table-cache budget"));
        }
        index.pages.push(Page {
            start,
            end,
            line: starts.partition_point(|n| *n <= start).max(1),
            offset: *offset,
            bytes: html.len(),
            rows,
            row_start: table.rows,
            columns: count,
            table: Some(0),
        });
        output.write_all(html.as_bytes())?;
        *offset += html.len() as u64;
        table.rows += rows;
        body.clear();
        Ok(())
    };
    let mut records = reader(source, delimiter);
    let mut record_lines = BufWriter::new(File::create(directory.join("csv-lines.bin"))?);
    records.headers().map_err(error)?;
    while records.read_record(&mut record).map_err(error)? {
        let mut start = record
            .position()
            .map(|p| p.byte() as usize)
            .unwrap_or(page_end);
        // csv-core can leave the LF of a CRLF separator pending in the next
        // record's byte position. Anchors must point at the actual record.
        while start < source.len() && matches!(source.as_bytes()[start], b'\r' | b'\n') {
            start += 1;
        }
        let end = records.position().byte() as usize;
        record_lines.write_all(&(starts.partition_point(|n| *n <= start) as u64).to_le_bytes())?;
        let mut row = row_tag(source, starts, start, end);
        for column in 0..count {
            row.push_str(&format!(
                "<td>{}</td>",
                cell(record.get(column).unwrap_or(""))
            ));
        }
        row.push_str("</tr>");
        if prefix.len() + row.len() + suffix.len() > crate::core::MAX_HTML {
            return Err(error(
                "An indivisible CSV record exceeds the 128 KiB display budget",
            ));
        }
        if row_count > 0
            && (row_count >= limit
                || body.len() + row.len() > 16 * 1024
                || prefix.len() + body.len() + row.len() + suffix.len() > crate::core::MAX_HTML)
        {
            flush(
                &mut output,
                &mut index,
                &mut table,
                &mut body,
                row_count,
                page_start,
                page_end,
                &mut offset,
            )?;
            row_count = 0;
            page_start = start;
        }
        table.max_row_bytes = table.max_row_bytes.max(row.len());
        body.push_str(&row);
        row_count += 1;
        page_end = end;
    }
    if row_count > 0 || index.pages.is_empty() {
        flush(
            &mut output,
            &mut index,
            &mut table,
            &mut body,
            row_count,
            page_start,
            page_end,
            &mut offset,
        )?;
    }
    output.flush()?;
    record_lines.flush()?;
    table.end_page = index.pages.len();
    index.tables.push(table);
    index.index_ms = started.elapsed().as_secs_f64() * 1000.0;
    index.peak_rss_kib = crate::core::peak_rss_kib();
    fs::write(directory.join("index.pending"), serde_json::to_vec(&index)?)?;
    fs::rename(
        directory.join("index.pending"),
        directory.join("index.json"),
    )?;
    Ok(index)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn long_csv_display_names_keep_format_identity_and_raw_query_values() {
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join(format!("{}.CSV", "a".repeat(241)));
        fs::write(&path, "Name,Notes\r\n001,\"a\r\nb\"\r\n").unwrap();
        let index = crate::core::build_index(&path, d.path()).unwrap();
        assert!(index.name.chars().count() <= 240);
        assert!(is_csv(Path::new(&index.name)));
        crate::core::audit_tables(d.path(), &index).unwrap();
        let result = crate::core::table_query::Queries::default()
            .run(
                d.path(),
                &index,
                0,
                "SELECT c1,c2 FROM t",
                std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            )
            .unwrap();
        assert_eq!(
            result.rows[0],
            vec![Some("001".into()), Some("a\r\nb".into())]
        );
    }
    #[test]
    fn csv_blank_prefix_retains_physical_header_and_record_lines() {
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("blank.csv");
        fs::write(&path, "\u{feff}\r\n\r\nA,B\r\n\r\n1,2\r\n").unwrap();
        let index = crate::core::build_index(&path, d.path()).unwrap();
        assert_eq!(index.tables[0].rows, 1);
        let page = crate::core::read_table(d.path(), &index, 0, 0, &[0, 1]).unwrap();
        assert!(
            page.html.contains("data-source-line=\"3\""),
            "{}",
            page.html
        );
        assert!(
            page.html.contains("data-source-line=\"5\""),
            "{}",
            page.html
        );
    }
    #[test]
    fn csv_preserves_records_multiline_unicode_html_and_original_source_ranges() {
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("中文.CSV");
        let source = "\u{feff}ID,描述,单位\r\n001,\"逗号,与\"\"引号\"\"\",V\r\n002,\"第一行\r\n第二行😀\",A\r\n003,<script>unsafe</script>,\r\n";
        fs::write(&path, source).unwrap();
        let index = crate::core::build_index(&path, d.path()).unwrap();
        assert_eq!(index.tables[0].rows, 3);
        assert_eq!(index.tables[0].columns, vec!["ID", "描述", "单位"]);
        let page = crate::core::read_table(d.path(), &index, 0, 0, &[0, 1, 2]).unwrap();
        assert!(page.html.contains("<td>001</td>"));
        assert!(page.html.contains("逗号,与&quot;引号&quot;"));
        assert!(page.html.contains("第二行😀"));
        assert!(!page.html.contains("<script>"));
        assert!(
            page.html
                .contains("data-source-line=\"3\" data-source-end-line=\"4\""),
            "{}",
            page.html
        );
        assert_eq!(
            crate::core::search(d.path(), &index, "第二行").unwrap()[0].table_row,
            Some(1)
        );
        assert_eq!(
            fs::read_to_string(d.path().join("source.md")).unwrap(),
            source
        );
    }
    #[test]
    fn csv_paging_ragged_empty_and_header_only_are_bounded_and_complete() {
        for source in [
            String::new(),
            "a,b\n".into(),
            "a\n1,extra\n2\n".into(),
            format!(
                "a,b\n{}",
                (0..1234).map(|r| format!("{r},x\n")).collect::<String>()
            ),
        ] {
            let d = tempfile::tempdir().unwrap();
            let path = d.path().join("table.csv");
            fs::write(&path, &source).unwrap();
            let index = crate::core::build_index(&path, d.path()).unwrap();
            crate::core::audit_tables(d.path(), &index).unwrap();
            let mut row = 0;
            loop {
                let page = crate::core::read_table(
                    d.path(),
                    &index,
                    0,
                    row,
                    &(0..index.tables[0].columns.len()).collect::<Vec<_>>(),
                )
                .unwrap();
                assert!(page.html.len() <= crate::core::MAX_HTML);
                assert!(page.row_end - page.row_start <= 50);
                row = page.row_end;
                if row == page.total_rows {
                    break;
                }
            }
            assert_eq!(row, index.tables[0].rows);
        }
    }
}
