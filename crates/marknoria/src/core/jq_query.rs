//! Local jq-compatible queries. Evaluation happens only in a supervised worker;
//! no filesystem loader, environment access, shell, or network functions exist.
use crate::core::Index;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    io::{self, Read, Seek, Write},
    path::Path,
    process::{Command, Stdio},
    sync::atomic::AtomicBool,
    time::{Duration, Instant},
};

const MIN_OUTPUT: u64 = 8 * 1024 * 1024;
const MAX_OUTPUT: u64 = 256 * 1024 * 1024;
const PAGE_BYTES: usize = 48 * 1024;
pub(crate) fn output_budget(source_bytes: u64) -> usize {
    source_bytes.saturating_mul(4).clamp(MIN_OUTPUT, MAX_OUTPUT) as usize
}
fn bad(message: impl std::fmt::Display) -> io::Error {
    io::Error::other(message.to_string())
}

pub fn supported(name: &str) -> bool {
    name.rsplit_once('.').is_some_and(|(_, ext)| {
        matches!(
            ext.to_ascii_lowercase().as_str(),
            "json" | "jsonl" | "ndjson"
        )
    })
}
#[derive(Serialize, Deserialize)]
struct Job {
    filter: String,
    slurp: bool,
}
#[derive(Serialize, Deserialize)]
struct Metadata {
    pages: Vec<(u64, usize)>,
    values: usize,
    elapsed_ms: f64,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Page {
    pub kind: &'static str,
    pub id: String,
    pub revision: String,
    pub page: usize,
    pub pages: usize,
    pub total_values: usize,
    pub text: String,
    pub elapsed_ms: f64,
}
struct ResultSet {
    directory: tempfile::TempDir,
    revision: String,
    meta: Metadata,
}
#[derive(Default)]
pub struct Queries {
    results: HashMap<String, ResultSet>,
}
impl Queries {
    pub fn run(
        &mut self,
        directory: &Path,
        index: &Index,
        filter: &str,
        slurp: bool,
        worker: &Path,
        cancelled: &AtomicBool,
    ) -> io::Result<Page> {
        if !supported(&index.name) {
            return Err(bad("jq queries require JSON, JSONL or NDJSON"));
        }
        if filter.trim().is_empty() || filter.len() > 16384 {
            return Err(bad("Enter a jq filter of at most 16 KiB"));
        }
        if index.source_bytes as u64 > crate::core::MAX_SOURCE {
            return Err(bad("jq input exceeds the reader's 128 MiB file budget"));
        }
        let result = tempfile::Builder::new()
            .prefix("jq-")
            .tempdir_in(directory)?;
        fs::write(
            result.path().join("job.json"),
            serde_json::to_vec(&Job {
                filter: filter.into(),
                slurp,
            })?,
        )?;
        let mut command = Command::new(worker);
        command
            .args([
                "--marknoria-jq".as_ref(),
                directory.join("source.md").as_os_str(),
                result.path().as_os_str(),
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            unsafe {
                command.pre_exec(|| {
                    #[cfg(target_os = "linux")]
                    let memory = libc::rlimit {
                        rlim_cur: 768 * 1024 * 1024,
                        rlim_max: 768 * 1024 * 1024,
                    };
                    let cpu = libc::rlimit {
                        rlim_cur: 6,
                        rlim_max: 6,
                    };
                    // macOS does not implement Linux's address-space limit;
                    // supervisor RSS checks enforce the memory budget there.
                    #[cfg(target_os = "linux")]
                    if libc::setrlimit(libc::RLIMIT_AS, &memory) != 0 {
                        return Err(io::Error::last_os_error());
                    }
                    if libc::setrlimit(libc::RLIMIT_CPU, &cpu) != 0 {
                        return Err(io::Error::last_os_error());
                    }
                    Ok(())
                });
            }
        }
        let mut command = process_wrap::std::CommandWrap::from(command);
        #[cfg(unix)]
        command.wrap(process_wrap::std::ProcessGroup::leader());
        #[cfg(windows)]
        {
            let mut flags = process_wrap::std::CreationFlags(Default::default());
            flags.0.0 = 0x08000000; // CREATE_NO_WINDOW for supervised CLI helpers.
            command.wrap(flags).wrap(process_wrap::std::JobObject);
        }
        let mut child = command.spawn()?;
        crate::runtime::process::supervise_budget(
            child.as_mut(),
            cancelled,
            Duration::from_secs(5),
            512 * 1024,
        )
        .map_err(bad)?;
        let meta: Metadata = serde_json::from_slice(&fs::read(result.path().join("meta.json"))?)?;
        if self.results.len() >= 2 {
            if let Some(id) = self.results.keys().next().cloned() {
                self.close(&id);
            }
        }
        let id = uuid::Uuid::new_v4().to_string();
        self.results.insert(
            id.clone(),
            ResultSet {
                directory: result,
                revision: index.revision.clone(),
                meta,
            },
        );
        self.page(index, &id, 0)
    }
    pub fn page(&self, index: &Index, id: &str, page: usize) -> io::Result<Page> {
        let result = self
            .results
            .get(id)
            .filter(|r| r.revision == index.revision)
            .ok_or_else(|| bad("Query result expired; run the filter again"))?;
        let &(offset, bytes) = result
            .meta
            .pages
            .get(page)
            .ok_or_else(|| bad("Query page out of range"))?;
        let mut file = fs::File::open(result.directory.path().join("result.txt"))?;
        file.seek(io::SeekFrom::Start(offset))?;
        let mut text = vec![0; bytes];
        file.read_exact(&mut text)?;
        Ok(Page {
            kind: "jqQuery",
            id: id.into(),
            revision: index.revision.clone(),
            page,
            pages: result.meta.pages.len(),
            total_values: result.meta.values,
            text: String::from_utf8(text).map_err(bad)?,
            elapsed_ms: result.meta.elapsed_ms,
        })
    }
    pub fn close(&mut self, id: &str) {
        self.results.remove(id);
    }
}

struct Output {
    file: io::BufWriter<fs::File>,
    bytes: usize,
    limit: usize,
}
impl Write for Output {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.bytes.saturating_add(bytes.len()) > self.limit {
            return Err(bad(format!(
                "jq results exceed the {} MiB output budget; narrow the filter",
                self.limit / (1024 * 1024)
            )));
        }
        self.file.write_all(bytes)?;
        self.bytes += bytes.len();
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}
// Index bounded UTF-8 pages on disk without loading the whole result into RAM.
fn result_pages(path: &Path) -> io::Result<Vec<(u64, usize)>> {
    let mut file = fs::File::open(path)?;
    let total = file.metadata()?.len();
    let mut pages = Vec::new();
    let mut at = 0;
    let mut buffer = vec![0; PAGE_BYTES];
    while at < total {
        let bytes = (total - at).min(PAGE_BYTES as u64) as usize;
        file.seek(io::SeekFrom::Start(at))?;
        file.read_exact(&mut buffer[..bytes])?;
        let mut end = match std::str::from_utf8(&buffer[..bytes]) {
            Ok(_) => bytes,
            Err(e) if e.error_len().is_none() && at + (bytes as u64) < total => e.valid_up_to(),
            Err(e) => return Err(bad(e)),
        };
        if at + (end as u64) < total {
            if let Some(newline) = buffer[..end].iter().rposition(|&b| b == b'\n') {
                end = newline + 1;
            }
        }
        pages.push((at, end));
        at += end as u64;
    }
    if pages.is_empty() {
        pages.push((0, 0));
    }
    Ok(pages)
}
pub fn worker(source: &Path, directory: &Path) -> io::Result<()> {
    use jaq_core::{
        Ctx, Vars, data,
        load::{Arena, File, Loader},
        unwrap_valr,
    };
    use jaq_json::Val;
    let started = Instant::now();
    let job: Job = serde_json::from_slice(&fs::read(directory.join("job.json"))?)?;
    let source_bytes = fs::metadata(source)?.len();
    if source_bytes > crate::core::MAX_SOURCE || job.filter.len() > 16384 {
        return Err(bad("jq input budget exceeded"));
    }
    let defs = jaq_core::defs()
        .chain(jaq_std::defs())
        .chain(jaq_json::defs());
    // No environment function or I/O extension. Loader<()> rejects imports
    // rather than looking for modules on the host filesystem.
    let funs = jaq_core::funs()
        .chain(jaq_std::funs().filter(|(name, _, _)| *name != "env"))
        .chain(jaq_json::funs());
    let arena = Arena::default();
    let loader = Loader::new(defs);
    let modules = loader
        .load(
            &arena,
            File {
                code: job.filter.as_str(),
                path: (),
            },
        )
        .map_err(|e| bad(format!("Invalid jq filter: {e:?}")))?;
    let filter = jaq_core::Compiler::default()
        .with_funs(funs)
        .compile(modules)
        .map_err(|e| bad(format!("Invalid jq filter: {e:?}")))?;
    let mut source_file = fs::File::open(source)?;
    let mut prefix = [0; 3];
    let length = source_file.read(&mut prefix)?;
    if length != 3 || prefix != [0xef, 0xbb, 0xbf] {
        source_file.seek(io::SeekFrom::Start(0))?;
    }
    let input = io::BufReader::new(source_file);
    let values = jaq_json::read::read_many(input);
    let input: Box<dyn Iterator<Item = io::Result<Val>>> = if job.slurp {
        Box::new(std::iter::once(values.collect::<io::Result<Val>>()))
    } else {
        Box::new(values)
    };
    let mut output = Output {
        file: io::BufWriter::with_capacity(
            64 * 1024,
            fs::File::create(directory.join("result.txt"))?,
        ),
        bytes: 0,
        limit: output_budget(source_bytes),
    };
    let mut count = 0;
    for value in input {
        let value = value.map_err(|e| bad(format!("Invalid JSON: {e}")))?;
        let ctx = Ctx::<data::JustLut<Val>>::new(&filter.lut, Vars::new([]));
        for value in filter.id.run((ctx, value)).map(unwrap_valr) {
            let value = value.map_err(|e| bad(format!("jq evaluation failed: {e}")))?;
            count += 1;
            let pp = jaq_json::write::Pp {
                indent: Some("  ".into()),
                sep_space: true,
                ..Default::default()
            };
            jaq_json::write::write(&mut output, &pp, 0, &value)?;
            output.write_all(b"\n\n")?;
        }
    }
    output.flush()?;
    let pages = result_pages(&directory.join("result.txt"))?;
    fs::write(
        directory.join("meta.json"),
        serde_json::to_vec(&Metadata {
            pages,
            values: count,
            elapsed_ms: started.elapsed().as_secs_f64() * 1000.,
        })?,
    )?;
    Ok(())
}

pub fn worker_entry() -> Option<io::Result<()>> {
    let args = std::env::args_os().collect::<Vec<_>>();
    if args.get(1).is_none_or(|s| s != "--marknoria-jq") {
        return None;
    }
    Some(if args.len() == 4 {
        worker(Path::new(&args[2]), Path::new(&args[3]))
    } else {
        Err(bad("Invalid jq worker arguments"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn disk_pages_preserve_utf8_long_lines_and_empty_results() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("result.txt");
        for text in [
            String::new(),
            format!(
                "{}😀中文\n{}\n",
                "x".repeat(PAGE_BYTES - 2),
                "中😀".repeat(PAGE_BYTES)
            ),
        ] {
            fs::write(&path, &text).unwrap();
            let pages = result_pages(&path).unwrap();
            let mut restored = String::new();
            let mut file = fs::File::open(&path).unwrap();
            for (offset, bytes) in pages {
                assert!(bytes <= PAGE_BYTES);
                file.seek(io::SeekFrom::Start(offset)).unwrap();
                let mut slice = vec![0; bytes];
                file.read_exact(&mut slice).unwrap();
                restored.push_str(std::str::from_utf8(&slice).unwrap());
            }
            assert_eq!(restored, text);
        }
    }
    #[test]
    fn buffered_output_enforces_source_scaled_disk_budget() {
        assert_eq!(output_budget(0), MIN_OUTPUT as usize);
        assert_eq!(output_budget(40 * 1024 * 1024), 160 * 1024 * 1024);
        assert_eq!(output_budget(crate::core::MAX_SOURCE), MAX_OUTPUT as usize);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("result.txt");
        let mut output = Output {
            file: io::BufWriter::new(fs::File::create(&path).unwrap()),
            bytes: 0,
            limit: 4,
        };
        output.write_all(b"1234").unwrap();
        assert!(output.write_all(b"5").is_err());
        output.flush().unwrap();
        assert_eq!(fs::read(path).unwrap(), b"1234");
    }
}
