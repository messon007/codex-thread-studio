use crate::core::Index;
use serde::Serialize;
use std::{
    fs::File,
    io::{self, Read, Seek, SeekFrom},
    path::Path,
};

pub const SOURCE_BYTES: usize = 128 * 1024;
pub const SOURCE_LINES: usize = 200;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceWindow {
    pub start: usize,
    pub end: usize,
    pub line: usize,
    pub end_line: usize,
    pub starts_mid_line: bool,
    pub ends_mid_line: bool,
    pub text: String,
}

fn line_offset(file: &mut File, line: usize) -> io::Result<usize> {
    file.seek(SeekFrom::Start((line as u64 - 1) * 8))?;
    let mut bytes = [0; 8];
    file.read_exact(&mut bytes)?;
    Ok(u64::from_le_bytes(bytes) as usize)
}

fn line_at(file: &mut File, count: usize, offset: usize) -> io::Result<usize> {
    let mut lower = 1;
    let mut upper = count + 1;
    while lower < upper {
        let middle = lower + (upper - lower) / 2;
        if line_offset(file, middle)? <= offset {
            lower = middle + 1;
        } else {
            upper = middle;
        }
    }
    Ok(lower - 1)
}

pub fn read_range(directory: &Path, index: &Index, start: usize, end: usize) -> io::Result<SourceWindow> {
    if start > end || end > index.source_bytes || end - start > SOURCE_BYTES * 4 {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "Source range exceeds the page limit"));
    }
    let mut source = File::open(directory.join("source.md"))?;
    source.seek(SeekFrom::Start(start as u64))?;
    let mut bytes = vec![0; end - start];
    source.read_exact(&mut bytes)?;
    let text = String::from_utf8(bytes).map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    let mut lines = File::open(directory.join("lines.bin"))?;
    let line = line_at(&mut lines, index.source_lines, start)?;
    let end_line = line_at(&mut lines, index.source_lines, end.saturating_sub(1).max(start))?;
    Ok(SourceWindow { start, end, line, end_line,
        starts_mid_line: start != line_offset(&mut lines, line)?,
        ends_mid_line: end < index.source_bytes && !text.ends_with('\n'), text })
}

pub fn read_source(
    directory: &Path,
    index: &Index,
    line: Option<usize>,
    offset: Option<usize>,
    previous: bool,
) -> io::Result<SourceWindow> {
    let invalid = || io::Error::new(io::ErrorKind::InvalidInput, "Source position out of range");
    if line.is_some() && offset.is_some()
        || line.is_some_and(|line| line == 0 || line > index.source_lines)
        || offset.is_some_and(|offset| offset > index.source_bytes)
        || previous && offset.is_none()
    {
        return Err(invalid());
    }
    let mut lines = File::open(directory.join("lines.bin"))?;
    let mut source = File::open(directory.join("source.md"))?;
    let position = match offset {
        Some(offset) => offset,
        None => line_offset(&mut lines, line.unwrap_or(1))?,
    };
    if position < index.source_bytes {
        source.seek(SeekFrom::Start(position as u64))?;
        let mut byte = [0];
        source.read_exact(&mut byte)?;
        if byte[0] & 0xc0 == 0x80 {
            return Err(invalid());
        }
    }
    let position_line = line_at(&mut lines, index.source_lines, position)?;
    let mut start = if previous {
        line_offset(
            &mut lines,
            position_line.saturating_sub(SOURCE_LINES).max(1),
        )?
        .max(position.saturating_sub(SOURCE_BYTES))
    } else {
        position
    };
    let end = if previous {
        position
    } else {
        index.source_bytes.min(start.saturating_add(SOURCE_BYTES))
    };
    source.seek(SeekFrom::Start(start as u64))?;
    let mut bytes = vec![0; end - start];
    source.read_exact(&mut bytes)?;
    if previous {
        let boundary = bytes
            .iter()
            .position(|byte| byte & 0xc0 != 0x80)
            .unwrap_or(bytes.len());
        start += boundary;
        bytes.drain(..boundary);
        let start_line = line_at(&mut lines, index.source_lines, start)?;
        if start > 0 && start != line_offset(&mut lines, start_line)? {
            if let Some(newline) = bytes
                .iter()
                .position(|byte| *byte == b'\n')
                .filter(|newline| newline + 1 < bytes.len())
            {
                start += newline + 1;
                bytes.drain(..=newline);
            }
        }
    } else {
        let newlines: Vec<_> = bytes
            .iter()
            .enumerate()
            .filter_map(|(position, byte)| (*byte == b'\n').then_some(position))
            .collect();
        if let Some(newline) = newlines.get(SOURCE_LINES - 1) {
            bytes.truncate(newline + 1);
        } else if end < index.source_bytes {
            if let Some(newline) = newlines.last() {
                bytes.truncate(newline + 1);
            } else {
                if let Err(error) = std::str::from_utf8(&bytes) {
                    if error.error_len().is_some() {
                        return Err(io::Error::new(io::ErrorKind::InvalidData, error));
                    }
                    bytes.truncate(error.valid_up_to());
                }
            }
        }
    }
    let end = start + bytes.len();
    let line = line_at(&mut lines, index.source_lines, start)?;
    let starts_mid_line = start != line_offset(&mut lines, line)?;
    let end_line = line_at(
        &mut lines,
        index.source_lines,
        end.saturating_sub(1).max(start),
    )?;
    let ends_mid_line = end < index.source_bytes && bytes.last().is_some_and(|byte| *byte != b'\n');
    let text = String::from_utf8(bytes)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    Ok(SourceWindow {
        start,
        end,
        line,
        end_line,
        starts_mid_line,
        ends_mid_line,
        text,
    })
}

/// Open a bounded source window around an absolute line without splitting its
/// preceding context at an arbitrary byte offset.
pub(crate) fn read_source_with_context(
    directory: &Path,
    index: &Index,
    line: usize,
    before: usize,
) -> io::Result<SourceWindow> {
    if line == 0 || line > index.source_lines || before >= SOURCE_LINES {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "Source context out of range"));
    }
    let mut lines = File::open(directory.join("lines.bin"))?;
    let target = line_offset(&mut lines, line)?;
    // Reserve most of the byte budget for the target and following lines.
    let context_limit = target.saturating_sub(SOURCE_BYTES / 4);
    let earliest = line_at(&mut lines, index.source_lines, context_limit)?;
    let earliest = earliest + usize::from(line_offset(&mut lines, earliest)? < context_limit);
    let start = line.saturating_sub(before).max(earliest);
    let window = read_source(directory, index, Some(start), None, false)?;
    if window.end <= target && window.end < index.source_bytes {
        // An exceptionally long target line may consume the full byte window.
        return read_source(directory, index, Some(line), None, false);
    }
    Ok(window)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::build_index;

    fn fixture(text: &str) -> (tempfile::TempDir, Index) {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("input.md");
        std::fs::write(&path, text).unwrap();
        let index = build_index(&path, directory.path()).unwrap();
        (directory, index)
    }

    #[test]
    fn source_windows_preserve_bytes_lines_and_reverse_navigation() {
        let text = (0..750)
            .map(|line| format!("Line {line} 中文 😀\t  \r\n"))
            .collect::<String>();
        let (directory, index) = fixture(&text);
        let mut offset = 0;
        let mut combined = String::new();
        while offset < text.len() {
            let page = read_source(directory.path(), &index, None, Some(offset), false).unwrap();
            assert_eq!(page.text, text[page.start..page.end]);
            assert!(page.end > offset);
            assert!(page.text.len() <= SOURCE_BYTES);
            assert!(page.text.bytes().filter(|byte| *byte == b'\n').count() <= SOURCE_LINES);
            combined.push_str(&page.text);
            if offset > 0 {
                let previous =
                    read_source(directory.path(), &index, None, Some(offset), true).unwrap();
                assert_eq!(previous.end, offset);
                assert_eq!(previous.text, text[previous.start..offset]);
                assert!(previous.start < offset);
            }
            offset = page.end;
        }
        assert_eq!(combined, text);
        let target = read_source(directory.path(), &index, Some(400), None, false).unwrap();
        assert_eq!(target.line, 400);
        assert!(target.text.starts_with("Line 399"));
        let context = read_source_with_context(directory.path(), &index, 400, 24).unwrap();
        assert_eq!(context.line, 376);
        assert_eq!(context.end_line, 575);
        assert_eq!(context.text, text[context.start..context.end]);
        assert_eq!(read_source_with_context(directory.path(), &index, 3, 24).unwrap().line, 1);
        assert!(read_source_with_context(directory.path(), &index, 400, SOURCE_LINES).is_err());
        assert!(read_source(directory.path(), &index, Some(0), None, false).is_err());
        assert!(read_source(directory.path(), &index, None, Some(text.len() + 1), false).is_err());
        assert!(read_source(directory.path(), &index, Some(1), Some(0), false).is_err());
        assert!(
            read_source(
                directory.path(),
                &index,
                None,
                Some(text.find('中').unwrap() + 1),
                false
            )
            .is_err()
        );
    }

    #[test]
    fn source_long_lines_continue_without_losing_utf8_or_empty_files() {
        let text = format!("# Title\n\n[short]({})\n", "中".repeat(SOURCE_BYTES));
        let (directory, mut index) = fixture("");
        std::fs::write(directory.path().join("source.md"), &text).unwrap();
        let starts = std::iter::once(0usize)
            .chain(text.match_indices('\n').map(|(offset, _)| offset + 1))
            .collect::<Vec<_>>();
        std::fs::write(
            directory.path().join("lines.bin"),
            starts
                .iter()
                .flat_map(|offset| (*offset as u64).to_le_bytes())
                .collect::<Vec<_>>(),
        )
        .unwrap();
        index.source_bytes = text.len();
        index.source_lines = starts.len();
        let context = read_source_with_context(directory.path(), &index, 3, 24).unwrap();
        assert_eq!(context.line, 3);
        assert!(context.ends_mid_line);
        let end = read_source_with_context(directory.path(), &index, index.source_lines, 24).unwrap();
        assert_eq!(end.end, text.len());
        let mut offset = 0;
        let mut combined = String::new();
        let mut continued = false;
        while offset < text.len() {
            let page = read_source(directory.path(), &index, None, Some(offset), false).unwrap();
            assert!(page.end > offset);
            continued |= page.starts_mid_line || page.ends_mid_line;
            combined.push_str(&page.text);
            if page.starts_mid_line {
                let previous =
                    read_source(directory.path(), &index, None, Some(offset), true).unwrap();
                assert!(previous.start < offset);
                assert_eq!(previous.text, text[previous.start..offset]);
            }
            offset = page.end;
        }
        assert!(continued);
        assert_eq!(combined, text);
        let (empty, index) = fixture("");
        let page = read_source(empty.path(), &index, Some(1), None, false).unwrap();
        assert_eq!(page.text, "");
        assert_eq!(page.line, 1);
    }
}
