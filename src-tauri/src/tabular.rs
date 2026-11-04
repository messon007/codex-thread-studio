//! Bounded, presentation-neutral CSV/TSV/XLSX snapshots. No browser or GTK APIs.
use calamine::{DataRef, Reader, Xlsx};
use serde::Serialize;
use std::io::Cursor;

pub const MAX_ROWS: usize = 10_000;
pub const MAX_COLUMNS: usize = 200;
pub const MAX_CELL: usize = 256 * 1024;
pub const MAX_MEMORY: usize = 128 * 1024 * 1024;

#[derive(Debug)]
pub struct Sheet {
    pub name: String,
    pub rows: Vec<Vec<String>>,
    pub width: usize,
    pub truncated: bool,
}
#[derive(Debug)]
pub struct Workbook {
    pub sheets: Vec<Sheet>,
    pub delimiter: Option<String>,
    pub weight: usize,
}
fn account(value: &str, weight: &mut usize) -> Result<(), String> {
    if value.len() > MAX_CELL || value.contains('\0') {
        return Err("A cell exceeds 256 KiB or contains NUL text".into());
    }
    *weight = weight.saturating_add(value.len() + 2 * std::mem::size_of::<String>());
    if *weight > MAX_MEMORY {
        return Err(
            "Table preview exceeds its 128 MiB memory budget; split the source table".into(),
        );
    }
    Ok(())
}
fn account_row(weight: &mut usize) -> Result<(), String> {
    *weight = weight.saturating_add(2 * std::mem::size_of::<Vec<String>>());
    if *weight > MAX_MEMORY {
        return Err("Table preview exceeds its 128 MiB memory budget".into());
    }
    Ok(())
}
fn detect(bytes: &[u8], fallback: u8) -> u8 {
    let sample = &bytes[..bytes.len().min(64 * 1024)];
    let mut best = (0, 0, 0);
    let mut selected = fallback;
    for delimiter in [fallback, b',', b'\t', b';'] {
        let mut reader = csv::ReaderBuilder::new()
            .has_headers(false)
            .flexible(true)
            .delimiter(delimiter)
            .from_reader(sample);
        let mut widths = std::collections::BTreeMap::<usize, usize>::new();
        let mut count = 0;
        for row in reader.byte_records().take(64).flatten() {
            *widths.entry(row.len()).or_default() += 1;
            count += 1;
        }
        if let Some((width, frequency)) = widths
            .into_iter()
            .filter(|(width, _)| *width > 1)
            .max_by_key(|(width, n)| (*n, *width))
        {
            let score = (frequency * 1000 / count.max(1), frequency, width);
            if score > best {
                best = score;
                selected = delimiter;
            }
        }
    }
    selected
}
pub fn load(bytes: Vec<u8>, extension: &str, separator: Option<&str>) -> Result<Workbook, String> {
    if extension == "xlsx" {
        if separator.is_some_and(|value| value != "auto") {
            return Err("A delimiter only applies to CSV/TSV files".into());
        }
        return xlsx(bytes);
    }
    if !matches!(extension, "csv" | "tsv") {
        return Err("Choose a CSV, TSV or XLSX file".into());
    }
    let fallback = if extension == "tsv" { b'\t' } else { b',' };
    let delimiter = match separator.unwrap_or("auto") {
        "auto" => detect(&bytes, fallback),
        "," => b',',
        ";" => b';',
        "\t" => b'\t',
        _ => return Err("Choose Auto, comma, tab or semicolon".into()),
    };
    if bytes.contains(&0) {
        return Err("CSV/TSV must contain UTF-8 text without NUL bytes".into());
    }
    let mut input = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .delimiter(delimiter)
        .from_reader(bytes.as_slice());
    let mut sheet = Sheet {
        name: "Data".into(),
        rows: Vec::new(),
        width: 0,
        truncated: false,
    };
    let mut weight = 0;
    for row in input.records() {
        let row = row.map_err(|error| format!("Cannot parse UTF-8 delimited text: {error}"))?;
        if sheet.rows.len() >= MAX_ROWS {
            sheet.truncated = true;
            break;
        }
        sheet.truncated |= row.len() > MAX_COLUMNS;
        account_row(&mut weight)?;
        let mut output = Vec::new();
        for cell in row.iter().take(MAX_COLUMNS) {
            account(cell, &mut weight)?;
            output.push(cell.to_owned());
        }
        sheet.width = sheet.width.max(output.len());
        sheet.rows.push(output);
    }
    Ok(Workbook {
        sheets: vec![sheet],
        delimiter: Some((delimiter as char).to_string()),
        weight,
    })
}
fn cell_text(value: &DataRef<'_>) -> String {
    match value {
        DataRef::String(value) | DataRef::DateTimeIso(value) | DataRef::DurationIso(value) => {
            value.clone()
        }
        DataRef::SharedString(value) => (*value).to_owned(),
        DataRef::Int(value) => value.to_string(),
        DataRef::Float(value) => value.to_string(),
        DataRef::Bool(value) => value.to_string(),
        DataRef::Error(value) => value.to_string(),
        DataRef::DateTime(value) => {
            if value.is_duration() {
                value
                    .as_duration()
                    .map(|duration| duration.to_string())
                    .unwrap_or_else(|| value.to_string())
            } else {
                value
                    .as_datetime()
                    .map(|date| date.to_string())
                    .unwrap_or_else(|| value.to_string())
            }
        }
        DataRef::Empty => String::new(),
    }
}
fn xlsx(bytes: Vec<u8>) -> Result<Workbook, String> {
    crate::validate_spreadsheet_archive(&bytes)?;
    let mut input =
        Xlsx::new(Cursor::new(bytes)).map_err(|error| format!("Cannot read XLSX: {error}"))?;
    let names = input.sheet_names();
    if names.len() > 64 {
        return Err("Workbook exceeds 64 sheets".into());
    }
    let mut sheets = Vec::new();
    let mut weight = 0;
    for name in names {
        if name.len() > 1024 || name.contains('\0') {
            return Err("Invalid worksheet name".into());
        }
        let mut reader = input
            .worksheet_cells_reader(&name)
            .map_err(|error| error.to_string())?;
        let mut sheet = Sheet {
            name,
            rows: Vec::new(),
            width: 0,
            truncated: false,
        };
        // Never allocate a dense Range from the declared Excel dimensions.
        // Sparse sheets can declare billions of empty positions.
        while let Some(cell) = reader.next_cell().map_err(|error| error.to_string())? {
            let (row, column) = cell.get_position();
            let (row, column) = (row as usize, column as usize);
            if row >= MAX_ROWS {
                sheet.truncated = true;
                break;
            }
            if column >= MAX_COLUMNS {
                sheet.truncated = true;
                continue;
            }
            while sheet.rows.len() <= row {
                account_row(&mut weight)?;
                sheet.rows.push(Vec::new());
            }
            let values = &mut sheet.rows[row];
            while values.len() <= column {
                account("", &mut weight)?;
                values.push(String::new());
            }
            let text = cell_text(cell.get_value());
            weight =
                weight.saturating_sub(values[column].len() + 2 * std::mem::size_of::<String>());
            account(&text, &mut weight)?;
            values[column] = text;
            sheet.width = sheet.width.max(column + 1);
        }
        sheets.push(sheet);
    }
    Ok(Workbook {
        sheets,
        delimiter: None,
        weight,
    })
}
pub fn sql_columns(sheet: &Sheet) -> Vec<String> {
    let columns = (0..sheet.width.max(1))
        .map(|column| {
            sheet
                .rows
                .first()
                .and_then(|row| row.get(column))
                .cloned()
                .unwrap_or_default()
        })
        .collect::<Vec<_>>();
    crate::table_query::normalize_columns(&columns)
}
pub fn default_sql(columns: &[String], compact: bool) -> String {
    let columns = columns
        .iter()
        .map(|column| crate::table_query::quote_identifier(column))
        .collect::<Vec<_>>();
    let sql = if compact {
        format!("SELECT {}\nFROM data\nLIMIT 1000", columns.join(", "))
    } else {
        format!("SELECT\n  {}\nFROM data\nLIMIT 1000", columns.join(",\n  "))
    };
    if sql.len() > 64 * 1024 {
        "SELECT *\nFROM data\nLIMIT 1000".into()
    } else {
        sql
    }
}
pub fn column_name(mut index: usize) -> String {
    let mut out = String::new();
    loop {
        out.insert(0, (b'A' + (index % 26) as u8) as char);
        if index < 26 {
            break;
        }
        index = index / 26 - 1;
    }
    out
}
/// Reformat only an initial SELECT list; expressions and quoted literals stay
/// intact. Comments/ambiguous SQL are deliberately left exactly as entered.
pub fn format_sql(sql: &str, compact: bool) -> Result<String, String> {
    if sql.len() > 64 * 1024 || sql.contains('\0') {
        return Err("SQL exceeds 64 KiB or contains NUL text".into());
    }
    let start = sql.len() - sql.trim_start().len();
    let bytes = sql.as_bytes();
    if bytes
        .get(start..start + 6)
        .is_none_or(|word| !word.eq_ignore_ascii_case(b"SELECT"))
    {
        return Ok(sql.into());
    }
    let end = start + 6;
    if sql[end..]
        .chars()
        .next()
        .is_some_and(|ch| ch.is_alphanumeric() || ch == '_')
    {
        return Ok(sql.into());
    }
    let mut quote = 0u8;
    let mut depth = 0usize;
    let mut begin = end;
    let mut columns = Vec::new();
    let mut index = end;
    while index < bytes.len() {
        let ch = bytes[index];
        if quote != 0 {
            let close = if quote == b'[' { b']' } else { quote };
            if ch == close {
                if bytes.get(index + 1) == Some(&close) {
                    index += 1;
                } else {
                    quote = 0;
                }
            }
        } else if matches!(ch, b'\'' | b'"' | b'`' | b'[') {
            quote = ch;
        } else if bytes
            .get(index..index + 2)
            .is_some_and(|part| part == b"--" || part == b"/*")
        {
            return Ok(sql.into());
        } else if ch == b'(' {
            depth += 1;
        } else if ch == b')' {
            depth = depth.saturating_sub(1);
        } else if depth == 0 && ch == b',' {
            columns.push(sql[begin..index].trim());
            begin = index + 1;
        } else if depth == 0
            && bytes
                .get(index..index + 4)
                .is_some_and(|word| word.eq_ignore_ascii_case(b"FROM"))
            && !sql[..index]
                .chars()
                .next_back()
                .is_some_and(|ch| ch.is_alphanumeric() || ch == '_')
            && !sql[index + 4..]
                .chars()
                .next()
                .is_some_and(|ch| ch.is_alphanumeric() || ch == '_')
        {
            columns.push(sql[begin..index].trim());
            if columns.len() < 2 || columns.iter().any(|column| column.is_empty()) {
                return Ok(sql.into());
            }
            let columns = columns
                .into_iter()
                .map(compact_fragment)
                .collect::<Vec<_>>();
            let selection = if compact {
                format!(" {}\n", columns.join(", "))
            } else {
                format!("\n  {}\n", columns.join(",\n  "))
            };
            return Ok(format!("{}{selection}{}", &sql[..end], &sql[index..]));
        }
        index += 1;
    }
    Ok(sql.into())
}
fn compact_fragment(source: &str) -> String {
    let mut out = String::new();
    let mut quote = None;
    let mut space = false;
    let mut chars = source.chars().peekable();
    while let Some(ch) = chars.next() {
        if let Some(delimiter) = quote {
            out.push(ch);
            if ch == delimiter {
                if chars.peek() == Some(&ch) {
                    out.push(chars.next().unwrap());
                } else {
                    quote = None;
                }
            }
        } else if ch.is_whitespace() {
            space = true;
        } else {
            if space && !out.is_empty() {
                out.push(' ');
            }
            space = false;
            out.push(ch);
            if matches!(ch, '\'' | '"' | '`' | '[') {
                quote = Some(if ch == '[' { ']' } else { ch });
            }
        }
    }
    out
}
#[derive(Serialize)]
pub struct ChartPoint {
    pub label: String,
    pub value: f64,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Chart {
    pub column: String,
    pub max_abs: f64,
    pub points: Vec<ChartPoint>,
}
pub fn chart(columns: &[String], rows: &[Vec<String>]) -> Result<Chart, String> {
    let value_column = (0..columns.len())
        .find(|column| {
            rows.iter()
                .filter_map(|row| row.get(*column))
                .filter(|value| {
                    !value.trim().is_empty()
                        && value.trim().parse::<f64>().is_ok_and(f64::is_finite)
                })
                .take(2)
                .count()
                >= rows.len().min(2).max(1)
        })
        .ok_or("No numeric column is available for a chart")?;
    let points = rows
        .iter()
        .enumerate()
        .filter_map(|(index, row)| {
            let value = row
                .get(value_column)?
                .trim()
                .parse::<f64>()
                .ok()
                .filter(|value| value.is_finite())?;
            let label = if value_column > 0 {
                row.first().map(String::as_str).unwrap_or("")
            } else {
                ""
            };
            Some(ChartPoint {
                label: if label.is_empty() {
                    (index + 1).to_string()
                } else {
                    label.chars().take(160).collect()
                },
                value,
            })
        })
        .take(100)
        .collect::<Vec<_>>();
    let max_abs = points
        .iter()
        .map(|point| point.value.abs())
        .fold(0.0_f64, f64::max);
    let max_abs = if max_abs == 0.0 { 1.0 } else { max_abs };
    Ok(Chart {
        column: columns[value_column].clone(),
        max_abs,
        points,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn delimited_detection_preserves_quotes_newlines_bom_unicode_and_sparse_rows() {
        let input="\u{feff}name;note;score\r\n中文;\"quoted, comma\nsecond \"\"line\"\"\";2\r\nshort;row\r\n";
        let workbook = load(input.as_bytes().to_vec(), "csv", None).unwrap();
        assert_eq!(workbook.delimiter.as_deref(), Some(";"));
        assert_eq!(
            workbook.sheets[0].rows[1],
            vec!["中文", "quoted, comma\nsecond \"line\"", "2"]
        );
        assert_eq!(workbook.sheets[0].width, 3);
        assert_eq!(
            sql_columns(&workbook.sheets[0]),
            vec!["name", "note", "score"]
        );
        assert!(load(b"a\0b".to_vec(), "csv", None).is_err());
        assert!(load(vec![0xff], "csv", None).is_err());
        assert_eq!(
            load(b"a\tb\n1\t2".to_vec(), "tsv", None)
                .unwrap()
                .delimiter
                .as_deref(),
            Some("\t")
        );
        assert_eq!(
            load(b"a;b\n1;2".to_vec(), "csv", Some(",")).unwrap().sheets[0].width,
            1
        );
    }
    #[test]
    fn bounded_rows_and_columns_are_explicit_and_sql_uses_all_normalized_headers() {
        let source = "name,name,\n".to_owned() + &"x,y,z\n".repeat(MAX_ROWS + 1);
        let book = load(source.into_bytes(), "csv", None).unwrap();
        assert_eq!(book.sheets[0].rows.len(), MAX_ROWS);
        assert!(book.sheets[0].truncated);
        let columns = sql_columns(&book.sheets[0]);
        assert_eq!(columns, vec!["name", "name_2", "column_3"]);
        assert!(default_sql(&columns, false).contains("\"name\",\n  \"name_2\""));
        assert_eq!(column_name(26), "AA");
        let chart = chart(
            &["name".into(), "value".into()],
            &[
                vec!["one".into(), "-2".into()],
                vec!["two".into(), "3".into()],
            ],
        )
        .unwrap();
        assert_eq!(chart.max_abs, 3.0);
        assert_eq!(chart.points[0].value, -2.0);
    }
    #[test]
    fn compact_sql_retains_literals_expressions_and_filters() {
        let sql="SELECT\n  \"名,称\",\n  printf('%s, %s', name, 'two  spaces') AS label,\n  CASE WHEN x>1 THEN 'a\nb' ELSE 'z' END\nFROM data WHERE x>0 ORDER BY x";
        let compact = format_sql(sql, true).unwrap();
        assert!(compact.contains("\"名,称\", printf('%s, %s', name, 'two  spaces') AS label, CASE"));
        assert!(compact.contains("'a\nb'"));
        assert!(compact.ends_with("FROM data WHERE x>0 ORDER BY x"));
        assert_eq!(format_sql(&compact, false).unwrap(), sql);
        let commented = "SELECT a, -- comment\n b FROM data";
        assert_eq!(format_sql(commented, true).unwrap(), commented);
    }
    #[test]
    fn streamed_xlsx_reads_multiple_sheets_cached_formulas_and_sparse_dimensions() {
        use std::io::Write;
        let mut archive = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let files = [
            (
                "[Content_Types].xml",
                r#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/></Types>"#,
            ),
            (
                "_rels/.rels",
                r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>"#,
            ),
            (
                "xl/workbook.xml",
                r#"<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets><sheet name="First 中文" sheetId="1" r:id="rId1"/><sheet name="Empty" sheetId="2" r:id="rId2"/></sheets></workbook>"#,
            ),
            (
                "xl/_rels/workbook.xml.rels",
                r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet2.xml"/></Relationships>"#,
            ),
            (
                "xl/worksheets/sheet1.xml",
                r#"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><dimension ref="A1:XFD1048576"/><sheetData><row r="1"><c r="A1" t="inlineStr"><is><t>name</t></is></c><c r="B1" t="inlineStr"><is><t>value</t></is></c></row><row r="2"><c r="A2" t="inlineStr"><is><t>中文 &amp; 😀</t></is></c><c r="B2"><f>1+2</f><v>3</v></c></row><row r="1048576"><c r="XFD1048576"><v>1</v></c></row></sheetData></worksheet>"#,
            ),
            (
                "xl/worksheets/sheet2.xml",
                r#"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData/></worksheet>"#,
            ),
        ];
        for (name, xml) in files {
            archive
                .start_file(
                    name,
                    zip::write::SimpleFileOptions::default()
                        .compression_method(zip::CompressionMethod::Stored),
                )
                .unwrap();
            archive.write_all(xml.as_bytes()).unwrap();
        }
        let bytes = archive.finish().unwrap().into_inner();
        let book = load(bytes, "xlsx", None).unwrap();
        assert_eq!(book.sheets.len(), 2);
        assert_eq!(book.sheets[0].rows[1], vec!["中文 & 😀", "3"]);
        assert!(book.sheets[0].truncated);
        assert!(book.sheets[1].rows.is_empty());
        assert!(book.weight < 8192);
    }
}
