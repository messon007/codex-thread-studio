//! Private page-local projection mappings. Never part of the wire format.
use super::structured::Segment;
use super::{Index, Page, TEXT_PAGE_BYTES, error};
use std::{
    fs::File,
    io::{self, BufWriter, Read, Seek, SeekFrom, Write},
    ops::Range,
    path::Path,
};

const MAPS_PER_PAGE: usize = 384;
const PAGE_TEXT_BYTES: usize = TEXT_PAGE_BYTES / 2;
const ENTRY_BYTES: usize = 16;

pub(super) struct Writer<'a> {
    source: &'a str,
    starts: &'a [usize],
    output: BufWriter<File>,
    maps: BufWriter<File>,
    offsets: BufWriter<File>,
    text: String,
    segments: Vec<Segment>,
    pub pages: Vec<Page>,
    offset: u64,
    map_offset: u64,
    output_budget: usize,
    emitted: usize,
}
impl<'a> Writer<'a> {
    pub fn new(
        directory: &Path,
        source: &'a str,
        starts: &'a [usize],
        output_budget: usize,
    ) -> io::Result<Self> {
        Ok(Self {
            source,
            starts,
            output: BufWriter::new(File::create(directory.join("pages.txt"))?),
            maps: BufWriter::new(File::create(directory.join("pages.map"))?),
            offsets: BufWriter::new(File::create(directory.join("pages.map-index"))?),
            text: String::with_capacity(TEXT_PAGE_BYTES),
            segments: Vec::with_capacity(MAPS_PER_PAGE),
            pages: Vec::new(),
            offset: 0,
            map_offset: 0,
            output_budget,
            emitted: 0,
        })
    }
    pub fn generated(&mut self, text: &str) -> io::Result<bool> {
        self.append(text, None)
    }
    pub fn copy(&mut self, range: Range<usize>) -> io::Result<bool> {
        let source = self.source;
        self.append(&source[range.clone()], Some(range.start))
    }
    pub fn append(&mut self, mut text: &str, mut original: Option<usize>) -> io::Result<bool> {
        if self.emitted.saturating_add(text.len()) > self.output_budget {
            return Ok(false);
        }
        while !text.is_empty() {
            let contiguous = original.is_some_and(|at| {
                self.segments.last().is_some_and(|last| {
                    last.display.end == self.text.len() && last.original + last.display.len() == at
                })
            });
            if self.text.len() == PAGE_TEXT_BYTES
                || (self.segments.len() == MAPS_PER_PAGE && !contiguous)
            {
                self.flush()?;
            }
            let mut end = text.len().min(PAGE_TEXT_BYTES - self.text.len());
            while !text.is_char_boundary(end) {
                end -= 1;
            }
            if end == 0 {
                self.flush()?;
                continue;
            }
            // Split literal passages at their original newlines too, so plain
            // text and multiline strings have useful line anchors.
            if original.is_some() {
                if let Some(at) = text[..end].find('\n') {
                    end = at + 1;
                }
            }
            let start = self.text.len();
            self.text.push_str(&text[..end]);
            if let Some(at) = original {
                let full = self.segments.len() == MAPS_PER_PAGE;
                if let Some(last) = self.segments.last_mut().filter(|last| {
                    last.display.end == start
                        && last.original + last.display.len() == at
                        && (!self.text[..start].ends_with('\n') || full)
                }) {
                    last.display.end += end;
                } else {
                    self.segments.push(Segment {
                        display: start..start + end,
                        original: at,
                    });
                }
                original = Some(at + end);
            }
            self.emitted += end;
            text = &text[end..];
            let cache = self.offset
                + self.text.len() as u64
                + self.map_offset
                + (self.segments.len() * ENTRY_BYTES + (self.pages.len() + 1) * 12) as u64;
            if cache > crate::core::MAX_CACHE || self.pages.len() >= crate::core::MAX_PAGES {
                return Ok(false);
            }
        }
        Ok(true)
    }
    fn flush(&mut self) -> io::Result<()> {
        if self.text.is_empty() {
            return Ok(());
        }
        if self.pages.len() >= crate::core::MAX_PAGES {
            return Err(error("Document exceeds the bounded page budget"));
        }
        let start = self
            .segments
            .first()
            .map_or_else(|| self.pages.last().map_or(0, |p| p.end), |s| s.original);
        let end = self
            .segments
            .last()
            .map_or(start, |s| s.original + s.display.len());
        self.pages.push(Page {
            start,
            end,
            line: self.starts.partition_point(|n| *n <= start).max(1),
            offset: self.offset,
            bytes: self.text.len(),
            rows: 0,
            row_start: 0,
            columns: 0,
            table: None,
        });
        self.output.write_all(self.text.as_bytes())?;
        self.offsets.write_all(&self.map_offset.to_le_bytes())?;
        self.offsets
            .write_all(&(self.segments.len() as u32).to_le_bytes())?;
        for s in &self.segments {
            for n in [
                s.display.start,
                s.display.end,
                s.original,
                self.starts.partition_point(|n| *n <= s.original).max(1),
            ] {
                self.maps.write_all(&(n as u32).to_le_bytes())?;
            }
        }
        self.offset += self.text.len() as u64;
        self.map_offset += (self.segments.len() * ENTRY_BYTES) as u64;
        self.text.clear();
        self.segments.clear();
        Ok(())
    }
    pub fn finish(mut self) -> io::Result<Vec<Page>> {
        self.flush()?;
        if self.pages.is_empty() {
            self.pages.push(Page {
                start: 0,
                end: 0,
                line: 1,
                offset: 0,
                bytes: 0,
                rows: 0,
                row_start: 0,
                columns: 0,
                table: None,
            });
            self.offsets.write_all(&[0; 12])?;
        }
        self.output.flush()?;
        self.maps.flush()?;
        self.offsets.flush()?;
        Ok(self.pages)
    }
}

pub(crate) struct Anchor {
    pub display: Range<usize>,
    pub original: usize,
    pub line: usize,
}
pub(super) fn read(
    directory: &Path,
    index: &Index,
    page: usize,
    text: &str,
) -> io::Result<Vec<Anchor>> {
    let mut offsets = match File::open(directory.join("pages.map-index")) {
        Ok(file) => file,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            return Ok(vec![Anchor {
                display: 0..text.len(),
                original: index.pages[page].start,
                line: index.pages[page].line,
            }]);
        }
        Err(e) => return Err(e),
    };
    offsets.seek(SeekFrom::Start((page * 12) as u64))?;
    let mut entry = [0; 12];
    offsets.read_exact(&mut entry)?;
    let offset = u64::from_le_bytes(entry[..8].try_into().unwrap());
    let count = u32::from_le_bytes(entry[8..].try_into().unwrap()) as usize;
    if count > MAPS_PER_PAGE || offset > crate::core::MAX_CACHE {
        return Err(error("Invalid projection map budget"));
    }
    let mut maps = File::open(directory.join("pages.map"))?;
    maps.seek(SeekFrom::Start(offset))?;
    let mut result = Vec::with_capacity(count);
    let mut previous = 0;
    for _ in 0..count {
        let mut bytes = [0; ENTRY_BYTES];
        maps.read_exact(&mut bytes)?;
        let values: Vec<usize> = bytes
            .chunks_exact(4)
            .map(|b| u32::from_le_bytes(b.try_into().unwrap()) as usize)
            .collect();
        let range = values[0]..values[1];
        if range.start < previous
            || range.end < range.start
            || range.end > text.len()
            || !text.is_char_boundary(range.start)
            || !text.is_char_boundary(range.end)
            || values[2].saturating_add(range.len()) > index.source_bytes
        {
            return Err(error("Invalid projection map"));
        }
        previous = range.end;
        result.push(Anchor {
            display: range,
            original: values[2],
            line: values[3],
        });
    }
    Ok(result)
}
