use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use similar::{Algorithm, DiffTag, capture_diff_slices_deadline};
use std::{
    fs::{self, File},
    io::{self, BufWriter, Read, Seek, SeekFrom, Write},
    path::Path,
    time::{Duration, Instant},
};

pub const MAX_COMBINED: u64 = 128 * 1024 * 1024;
const MAX_LINES: usize = 1_000_000;
const RECORD_BYTES: u64 = 56;
const PREVIEW_BYTES: usize = 512;
pub const LINE_BYTES: usize = 32 * 1024;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComparisonIndex {
    pub rows: usize,
    pub changes: Vec<usize>,
    pub left_lines: usize,
    pub right_lines: usize,
    pub left_hash: String,
    pub right_hash: String,
    pub added: usize,
    pub removed: usize,
    pub bounded_alignment: bool,
    pub elapsed_ms: f64,
    pub peak_rss_kib: Option<usize>,
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}
fn hash(text: &str) -> String {
    Sha256::digest(text.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn read_input(path: &Path, remaining: u64) -> io::Result<String> {
    let file = File::open(path)?;
    if !file.metadata()?.is_file() {
        return Err(invalid("Choose regular Markdown files"));
    }
    let mut bytes = Vec::new();
    file.take(remaining + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > remaining {
        return Err(invalid(
            "Comparison inputs exceed the combined 128 MiB limit",
        ));
    }
    let text = String::from_utf8(bytes).map_err(|_| invalid("Comparison requires UTF-8 text"))?;
    if text.contains('\0') {
        return Err(invalid("Comparison does not accept NUL bytes"));
    }
    Ok(text)
}

fn offsets(lines: &[&str]) -> Vec<usize> {
    let mut offset = 0;
    lines
        .iter()
        .map(|line| {
            let current = offset;
            offset += line.len();
            current
        })
        .collect()
}

pub fn build_comparison(
    left: &Path,
    right: &Path,
    directory: &Path,
) -> io::Result<ComparisonIndex> {
    let started = Instant::now();
    let left_text = read_input(left, MAX_COMBINED)?;
    let right_text = read_input(right, MAX_COMBINED - left_text.len() as u64)?;
    let left_lines: Vec<_> = left_text
        .split_inclusive('\n')
        .take(MAX_LINES + 1)
        .collect();
    let right_lines: Vec<_> = right_text
        .split_inclusive('\n')
        .take(MAX_LINES + 1)
        .collect();
    if left_lines.len() + right_lines.len() > MAX_LINES {
        return Err(invalid(
            "Comparison exceeds the combined one-million-line limit",
        ));
    }
    let left_offsets = offsets(&left_lines);
    let right_offsets = offsets(&right_lines);
    fs::write(directory.join("left.md"), &left_text)?;
    fs::write(directory.join("right.md"), &right_text)?;
    let deadline = Instant::now() + Duration::from_secs(5);
    let operations =
        capture_diff_slices_deadline(Algorithm::Myers, &left_lines, &right_lines, Some(deadline));
    let bounded_alignment = Instant::now() >= deadline;
    let mut records = BufWriter::new(File::create(directory.join("rows.bin"))?);
    let mut left_rows = BufWriter::new(File::create(directory.join("left-rows.bin"))?);
    let mut right_rows = BufWriter::new(File::create(directory.join("right-rows.bin"))?);
    let mut result = ComparisonIndex {
        rows: 0,
        changes: Vec::new(),
        left_lines: left_lines.len(),
        right_lines: right_lines.len(),
        left_hash: hash(&left_text),
        right_hash: hash(&right_text),
        added: 0,
        removed: 0,
        bounded_alignment,
        elapsed_ms: 0.0,
        peak_rss_kib: None,
    };
    let mut changed = false;
    for operation in operations {
        let left_range = operation.old_range();
        let right_range = operation.new_range();
        if operation.tag() != DiffTag::Equal {
            result.removed += left_range.len();
            result.added += right_range.len();
        }
        for position in 0..left_range.len().max(right_range.len()) {
            let old = (position < left_range.len()).then_some(left_range.start + position);
            let new = (position < right_range.len()).then_some(right_range.start + position);
            let kind = match (old, new) {
                (Some(old), Some(new)) if left_lines[old] == right_lines[new] => 0,
                (Some(_), Some(_)) => 1,
                (Some(_), None) => 2,
                (None, Some(_)) => 3,
                _ => unreachable!(),
            };
            if kind != 0 && !changed {
                result.changes.push(result.rows);
            }
            changed = kind != 0;
            let fields = [
                old.map(|line| left_offsets[line] as u64).unwrap_or(0),
                old.map(|line| left_lines[line].len() as u64).unwrap_or(0),
                old.map(|line| line as u64 + 1).unwrap_or(0),
                new.map(|line| right_offsets[line] as u64).unwrap_or(0),
                new.map(|line| right_lines[line].len() as u64).unwrap_or(0),
                new.map(|line| line as u64 + 1).unwrap_or(0),
                kind,
            ];
            for field in fields {
                records.write_all(&field.to_le_bytes())?;
            }
            if old.is_some() {
                left_rows.write_all(&(result.rows as u64).to_le_bytes())?;
            }
            if new.is_some() {
                right_rows.write_all(&(result.rows as u64).to_le_bytes())?;
            }
            result.rows += 1;
        }
    }
    records.flush()?;
    left_rows.flush()?;
    right_rows.flush()?;
    result.elapsed_ms = started.elapsed().as_secs_f64() * 1000.0;
    result.peak_rss_kib = crate::peak_rss_kib();
    fs::write(
        directory.join("comparison.json"),
        serde_json::to_vec(&result)?,
    )?;
    Ok(result)
}

pub fn row_for_line(
    directory: &Path,
    index: &ComparisonIndex,
    side: &str,
    line: usize,
) -> io::Result<usize> {
    let count = match side {
        "left" => index.left_lines,
        "right" => index.right_lines,
        _ => return Err(invalid("Invalid side")),
    };
    if count == 0 {
        return Ok(0);
    }
    let mut file = File::open(directory.join(format!("{side}-rows.bin")))?;
    file.seek(SeekFrom::Start((line.clamp(1, count) - 1) as u64 * 8))?;
    let mut bytes = [0; 8];
    file.read_exact(&mut bytes)?;
    Ok(u64::from_le_bytes(bytes) as usize)
}

pub fn hunk_lines(directory: &Path, index: &ComparisonIndex, row: usize) -> io::Result<[u64; 2]> {
    let current = record(directory, index, row)?;
    let previous = if row > 0 {
        record(directory, index, row - 1)?
    } else {
        [0; 7]
    };
    Ok([
        if current[2] > 0 {
            current[2]
        } else {
            previous[2] + 1
        },
        if current[5] > 0 {
            current[5]
        } else {
            previous[5] + 1
        },
    ])
}

fn record(directory: &Path, index: &ComparisonIndex, row: usize) -> io::Result<[u64; 7]> {
    if row >= index.rows {
        return Err(invalid("Comparison row out of range"));
    }
    let mut file = File::open(directory.join("rows.bin"))?;
    file.seek(SeekFrom::Start(row as u64 * RECORD_BYTES))?;
    let mut bytes = [0; RECORD_BYTES as usize];
    file.read_exact(&mut bytes)?;
    Ok(std::array::from_fn(|position| {
        u64::from_le_bytes(bytes[position * 8..position * 8 + 8].try_into().unwrap())
    }))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Cell {
    pub line: u64,
    pub text: String,
    pub bytes: u64,
    pub truncated: bool,
    pub ending: &'static str,
}

#[derive(Serialize)]
pub struct Row {
    pub row: usize,
    pub kind: &'static str,
    pub left: Option<Cell>,
    pub right: Option<Cell>,
}

fn preview(directory: &Path, side: &str, fields: &[u64]) -> io::Result<Option<Cell>> {
    if fields[2] == 0 {
        return Ok(None);
    }
    let mut file = File::open(directory.join(format!("{side}.md")))?;
    file.seek(SeekFrom::Start(fields[0]))?;
    let mut bytes = vec![0; fields[1].min(PREVIEW_BYTES as u64) as usize];
    file.read_exact(&mut bytes)?;
    if let Err(error) = std::str::from_utf8(&bytes) {
        bytes.truncate(error.valid_up_to());
    }
    let tail_bytes = fields[1].min(2);
    file.seek(SeekFrom::Start(fields[0] + fields[1] - tail_bytes))?;
    let mut tail = vec![0; tail_bytes as usize];
    file.read_exact(&mut tail)?;
    let ending = if tail.ends_with(b"\r\n") {
        "CRLF"
    } else if tail.ends_with(b"\n") {
        "LF"
    } else {
        "No newline"
    };
    Ok(Some(Cell {
        line: fields[2],
        truncated: bytes.len() as u64 != fields[1],
        bytes: fields[1],
        text: String::from_utf8(bytes).map_err(io::Error::other)?,
        ending,
    }))
}

pub fn read_rows(directory: &Path, index: &ComparisonIndex, start: usize) -> io::Result<Vec<Row>> {
    if start > index.rows || start == index.rows && index.rows != 0 {
        return Err(invalid("Comparison row out of range"));
    }
    let mut rows = Vec::new();
    let mut bytes = 0;
    for row in start..start.saturating_add(96).min(index.rows) {
        let fields = record(directory, index, row)?;
        let entry = Row {
            row,
            kind: match fields[6] {
                0 => "equal",
                1 => "changed",
                2 => "removed",
                _ => "added",
            },
            left: preview(directory, "left", &fields[..3])?,
            right: preview(directory, "right", &fields[3..6])?,
        };
        let size = serde_json::to_vec(&entry)?.len();
        if bytes + size > 96 * 1024 {
            break;
        }
        bytes += size;
        rows.push(entry);
    }
    Ok(rows)
}

#[derive(Serialize)]
pub struct LineWindow {
    pub line: u64,
    pub offset: usize,
    pub end: usize,
    pub total: usize,
    pub text: String,
}

pub fn read_line(
    directory: &Path,
    index: &ComparisonIndex,
    row: usize,
    side: &str,
    offset: usize,
    previous: bool,
) -> io::Result<LineWindow> {
    let record = record(directory, index, row)?;
    let fields = match side {
        "left" => &record[..3],
        "right" => &record[3..6],
        _ => return Err(invalid("Invalid comparison side")),
    };
    if fields[2] == 0 || offset > fields[1] as usize {
        return Err(invalid("Line position out of range"));
    }
    let mut file = File::open(directory.join(format!("{side}.md")))?;
    let mut start = if previous {
        offset.saturating_sub(LINE_BYTES)
    } else {
        offset
    };
    let end = if previous {
        offset
    } else {
        (fields[1] as usize).min(start.saturating_add(LINE_BYTES))
    };
    file.seek(SeekFrom::Start(fields[0] + start as u64))?;
    let mut bytes = vec![0; end - start];
    file.read_exact(&mut bytes)?;
    if previous {
        let skip = bytes
            .iter()
            .position(|byte| byte & 0xc0 != 0x80)
            .unwrap_or(bytes.len());
        bytes.drain(..skip);
        start += skip;
    } else if bytes.first().is_some_and(|byte| byte & 0xc0 == 0x80) {
        return Err(invalid("Position splits a UTF-8 character"));
    }
    if let Err(error) = std::str::from_utf8(&bytes) {
        bytes.truncate(error.valid_up_to());
    }
    Ok(LineWindow {
        line: fields[2],
        offset: start,
        end: start + bytes.len(),
        total: fields[1] as usize,
        text: String::from_utf8(bytes).map_err(io::Error::other)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn compare(left: &str, right: &str) -> (tempfile::TempDir, ComparisonIndex) {
        let directory = tempfile::tempdir().unwrap();
        fs::write(directory.path().join("a.md"), left).unwrap();
        fs::write(directory.path().join("b.md"), right).unwrap();
        let index = build_comparison(
            &directory.path().join("a.md"),
            &directory.path().join("b.md"),
            directory.path(),
        )
        .unwrap();
        (directory, index)
    }

    #[test]
    fn hunk_locations_preserve_context_for_insertions_and_deletions() {
        for (left, right, expected) in [
            ("before\nafter\n", "before\nnew\nafter\n", [2, 2]),
            ("before\nold\nafter\n", "before\nafter\n", [2, 2]),
            ("", "new\n", [1, 1]),
            ("old\n", "", [1, 1]),
        ] {
            let (directory, index) = compare(left, right);
            assert_eq!(
                hunk_lines(directory.path(), &index, index.changes[0]).unwrap(),
                expected
            );
        }
    }

    #[test]
    fn alignment_preserves_every_byte_line_and_newline_difference() {
        for (left, right) in [
            ("", ""),
            ("same\n", "same\n"),
            ("one\r\n中文 😀\nend", "one\n中文 😀\nnew\nend\n"),
            ("deleted\n", ""),
            ("", "inserted\n"),
            ("# A\n\n|x|y|\n", "# A\n\n|x|changed|\n"),
        ] {
            let (directory, index) = compare(left, right);
            for (side, expected) in [("left", left), ("right", right)] {
                let mut actual = String::new();
                let mut line = 0;
                for row in 0..index.rows {
                    if let Ok(window) = read_line(directory.path(), &index, row, side, 0, false) {
                        line += 1;
                        assert_eq!(window.line, line);
                        assert_eq!(
                            row_for_line(directory.path(), &index, side, line as usize).unwrap(),
                            row
                        );
                        actual.push_str(&window.text);
                    }
                }
                assert_eq!(actual, expected);
            }
            assert_eq!(index.changes.is_empty(), left == right);
        }
    }

    #[test]
    fn long_lines_have_explicit_bounded_previews_and_lossless_utf8_windows() {
        let text = "界😀\t ".repeat(30000) + "\r\n";
        let (directory, index) = compare(&text, "changed\n");
        let rows = read_rows(directory.path(), &index, 0).unwrap();
        assert!(rows[0].left.as_ref().unwrap().truncated);
        assert_eq!(rows[0].left.as_ref().unwrap().ending, "CRLF");
        let mut offset = 0;
        let mut actual = String::new();
        loop {
            let window = read_line(directory.path(), &index, 0, "left", offset, false).unwrap();
            assert!(window.text.len() <= LINE_BYTES);
            actual.push_str(&window.text);
            if window.end == window.total {
                break;
            }
            assert!(window.end > offset);
            let backwards =
                read_line(directory.path(), &index, 0, "left", window.end, true).unwrap();
            assert_eq!(&text[backwards.offset..backwards.end], backwards.text);
            offset = window.end;
        }
        assert_eq!(actual, text);
        assert!(read_line(directory.path(), &index, 0, "invalid", 0, false).is_err());
        assert!(read_rows(directory.path(), &index, index.rows + 1).is_err());
    }

    #[test]
    fn windows_are_bounded_and_source_is_never_interpreted_as_markup() {
        let left = "<script>unsafe</script>\t  \n".repeat(10000);
        let (directory, index) = compare(&left, &(left.clone() + "last\n"));
        let rows = read_rows(directory.path(), &index, 0).unwrap();
        assert!(rows.len() <= 96);
        assert!(serde_json::to_vec(&rows).unwrap().len() < 128 * 1024);
        assert!(rows[0].left.as_ref().unwrap().text.starts_with("<script>"));
        assert!(index.changes.last().is_some_and(|row| *row == 10000));
    }
}
