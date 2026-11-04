//! Process-private immutable text snapshots. Only bounded pages cross into GTK.
use std::{
    collections::HashMap,
    hash::BuildHasher,
    num::NonZeroUsize,
    ops::Range,
    sync::{Arc, Mutex, OnceLock},
};

pub const MAX_TEXT: usize = 16 * 1024 * 1024;
pub const CACHE_BYTES: usize = 64 * 1024 * 1024;
pub const PAGE_BYTES: usize = 64 * 1024;
pub const PAGE_LINES: usize = 256;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reference {
    pub token: String,
    pub bytes: usize,
    pub unavailable: Option<String>,
}
impl Reference {
    pub fn weight(&self) -> usize {
        self.token.capacity() + self.unavailable.as_ref().map_or(0, String::capacity)
    }
}
#[derive(Debug)]
struct Segment {
    start: usize,
    line: usize,
    continued: bool,
}
struct Document {
    text: String,
    pages: Vec<Segment>,
    lines: usize,
}
impl Document {
    fn new(text: String) -> Result<Self, String> {
        if text.len() > MAX_TEXT {
            return Err("Full output exceeds the 16 MiB snapshot limit".into());
        }
        let mut pages = Vec::new();
        let mut start = 0;
        let mut line = 1;
        while start < text.len() {
            pages.push(Segment {
                start,
                line,
                continued: start > 0 && text.as_bytes()[start - 1] != b'\n',
            });
            let mut end = (start + PAGE_BYTES).min(text.len());
            while !text.is_char_boundary(end) {
                end -= 1;
            }
            let mut newlines = 0;
            let mut last = None;
            for (offset, _) in text[start..end].match_indices('\n').take(PAGE_LINES) {
                newlines += 1;
                last = Some(start + offset + 1);
            }
            if newlines == PAGE_LINES
                || end < text.len() && last.is_some_and(|n| n - start >= PAGE_BYTES / 2)
            {
                end = last.unwrap();
            }
            // CRLF remains one intact source newline even on a very long line.
            if end < text.len()
                && text.as_bytes()[end - 1] == b'\r'
                && text.as_bytes()[end] == b'\n'
            {
                end -= 1;
            }
            line += newlines;
            start = end;
        }
        if pages.is_empty() {
            pages.push(Segment {
                start: 0,
                line: 1,
                continued: false,
            });
        }
        Ok(Self {
            text,
            pages,
            lines: line,
        })
    }
    fn weight(&self) -> usize {
        self.text.capacity()
            + self.pages.capacity() * std::mem::size_of::<Segment>()
            + std::mem::size_of::<Self>()
    }
    fn page(&self, index: usize, selection: Option<Range<usize>>) -> Result<Page, String> {
        let part = self
            .pages
            .get(index)
            .ok_or("Output page is outside this snapshot")?;
        let end = self
            .pages
            .get(index + 1)
            .map_or(self.text.len(), |s| s.start);
        let text = self.text[part.start..end].to_owned();
        let next_match = selection.as_ref().map(|range| range.end);
        let selection = selection.map(|range| {
            let start = range.start.saturating_sub(part.start).min(text.len());
            let finish = range.end.saturating_sub(part.start).min(text.len());
            Selection {
                start: text[..start].chars().count() as i32,
                end: text[..finish].chars().count() as i32,
                continues: range.end > end,
            }
        });
        let mut display = String::with_capacity(text.len());
        let mut characters = text.chars().peekable();
        let mut controls = false;
        while let Some(character) = characters.next() {
            let safe = match character {
                '\0' => '␀',
                '\r' if characters.peek() != Some(&'\n') => '␍',
                '\u{2028}' => '↵',
                '\u{2029}' => '¶',
                c => c,
            };
            controls |= safe != character;
            display.push(safe);
        }
        Ok(Page {
            index,
            pages: self.pages.len(),
            first_line: part.line,
            last_line: part.line + text.matches('\n').count(),
            total_lines: self.lines,
            offset: part.start,
            total_bytes: self.text.len(),
            continued: part.continued,
            text,
            display,
            selection,
            next_match,
            controls,
        })
    }
    fn find(&self, query: &str, from: usize) -> Result<Page, String> {
        if query.is_empty() || query.len() > 4096 {
            return Err("Find text must contain 1–4096 bytes".into());
        }
        if !self.text.is_char_boundary(from) {
            return Err("Find position is outside this snapshot".into());
        }
        let position = self.text[from..]
            .find(query)
            .map(|n| from + n)
            .or_else(|| self.text[..from].find(query))
            .ok_or("No matching text in this snapshot")?;
        let index = self
            .pages
            .partition_point(|p| p.start <= position)
            .saturating_sub(1);
        self.page(index, Some(position..position + query.len()))
    }
}
struct Stored {
    hash: u64,
    document: Arc<Document>,
}
struct Store {
    entries: lru::LruCache<String, Stored>,
    index: HashMap<u64, Vec<String>>,
    bytes: usize,
    budget: usize,
    active: Option<(String, Arc<Document>)>,
}
impl Store {
    fn new(budget: usize) -> Self {
        Self {
            entries: lru::LruCache::new(NonZeroUsize::new(256).unwrap()),
            index: HashMap::new(),
            bytes: 0,
            budget,
            active: None,
        }
    }
    fn existing(&mut self, hash: u64, text: &str) -> Option<String> {
        for key in self.index.get(&hash).cloned().unwrap_or_default() {
            if self
                .entries
                .peek(&key)
                .is_some_and(|s| s.document.text == text)
            {
                self.entries.get(&key);
                return Some(key);
            }
        }
        None
    }
    fn source(&mut self, reference: &Reference) -> Result<Arc<Document>, String> {
        if let Some(error) = &reference.unavailable {
            return Err(error.clone());
        }
        if reference.token.len() != 32 || !reference.token.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("Invalid output snapshot identity".into());
        }
        let source = if let Some((_, source)) = self
            .active
            .as_ref()
            .filter(|(token, _)| *token == reference.token)
        {
            source.clone()
        } else {
            self.entries.get(&reference.token).ok_or("This output snapshot expired. Reload the conversation or history page to retrieve it again.")?.document.clone()
        };
        if source.text.len() != reference.bytes {
            return Err("Output snapshot identity changed".into());
        }
        Ok(source)
    }
    fn remove_oldest(&mut self) {
        if let Some((key, old)) = self.entries.pop_lru() {
            self.bytes -= old.document.weight() + key.capacity() + 128;
            if let Some(keys) = self.index.get_mut(&old.hash) {
                keys.retain(|k| k != &key);
                if keys.is_empty() {
                    self.index.remove(&old.hash);
                }
            }
        }
    }
    fn insert(&mut self, hash: u64, document: Document) -> Result<String, String> {
        if let Some(key) = self.existing(hash, &document.text) {
            return Ok(key);
        }
        let mut random = [0u8; 16];
        getrandom::fill(&mut random).map_err(|_| "Output identity unavailable")?;
        let key = random
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        if self.entries.contains(&key) {
            return Err("Output identity collision; reload the source".into());
        }
        let weight = document.weight() + key.capacity() + 128;
        if weight > self.budget {
            return Err("Full output exceeds the snapshot cache budget".into());
        }
        while self.bytes + weight > self.budget || self.entries.len() >= 256 {
            self.remove_oldest();
        }
        self.bytes += weight;
        self.entries.put(
            key.clone(),
            Stored {
                hash,
                document: Arc::new(document),
            },
        );
        self.index.entry(hash).or_default().push(key.clone());
        Ok(key)
    }
}
fn store() -> &'static Mutex<Store> {
    static STORE: OnceLock<Mutex<Store>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(Store::new(CACHE_BYTES)))
}
/// Called from model projection workers, never GTK. No disk or provider request.
pub fn retain(mut text: String) -> Reference {
    let bytes = text.len();
    let result = (|| {
        if bytes > MAX_TEXT {
            return Err("Full output exceeds the 16 MiB snapshot limit".to_owned());
        }
        text.shrink_to_fit();
        let hasher = store()
            .lock()
            .map_err(|_| "Output cache unavailable")?
            .index
            .hasher()
            .clone();
        let hash = hasher.hash_one(text.as_bytes());
        if let Some(key) = store()
            .lock()
            .map_err(|_| "Output cache unavailable")?
            .existing(hash, &text)
        {
            return Ok(key);
        }
        let document = Document::new(text)?;
        store()
            .lock()
            .map_err(|_| "Output cache unavailable")?
            .insert(hash, document)
    })();
    match result {
        Ok(token) => Reference {
            token,
            bytes,
            unavailable: None,
        },
        Err(reason) => Reference {
            token: String::new(),
            bytes,
            unavailable: Some(reason),
        },
    }
}
/// Serialize on the projection worker without allocating an oversized JSON
/// string first. The immutable text uses the same byte-accounted snapshot cache.
pub fn retain_json(value: &impl serde::Serialize) -> Reference {
    struct Bounded(Vec<u8>);
    impl std::io::Write for Bounded {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            if bytes.len() > MAX_TEXT.saturating_sub(self.0.len()) {
                return Err(std::io::Error::other(
                    "JSON snapshot exceeds the 16 MiB limit",
                ));
            }
            self.0.extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut output = Bounded(Vec::new());
    match serde_json::to_writer_pretty(&mut output, value) {
        Ok(()) => retain(String::from_utf8(output.0).expect("JSON serialization emits UTF-8")),
        Err(error) => Reference {
            token: String::new(),
            bytes: MAX_TEXT + 1,
            unavailable: Some(error.to_string()),
        },
    }
}
fn lookup(reference: &Reference) -> Result<Arc<Document>, String> {
    store()
        .lock()
        .map_err(|_| "Output cache unavailable")?
        .source(reference)
}
fn pin(reference: Option<&Reference>) -> Result<(), String> {
    let mut store = store().lock().map_err(|_| "Output cache unavailable")?;
    let result = reference
        .map(|r| store.source(r).map(|source| (r.token.clone(), source)))
        .transpose();
    match result {
        Ok(active) => {
            store.active = active;
            Ok(())
        }
        Err(error) => {
            store.active = None;
            Err(error)
        }
    }
}
#[derive(Clone, Debug)]
pub enum Action {
    Page(usize),
    Find { query: String, from: usize },
}
#[derive(Clone, Debug)]
pub struct Request {
    pub dialog: u64,
    pub serial: u64,
    pub source: Reference,
    pub action: Action,
}
#[derive(Debug)]
pub struct Selection {
    pub start: i32,
    pub end: i32,
    pub continues: bool,
}
#[derive(Debug)]
pub struct Page {
    pub index: usize,
    pub pages: usize,
    pub first_line: usize,
    pub last_line: usize,
    pub total_lines: usize,
    pub offset: usize,
    pub total_bytes: usize,
    pub continued: bool,
    pub text: String,
    pub display: String,
    pub selection: Option<Selection>,
    pub next_match: Option<usize>,
    pub controls: bool,
}
#[derive(Debug)]
pub struct Response {
    pub dialog: u64,
    pub serial: u64,
    pub result: Result<Page, String>,
}
pub fn execute(request: Request) -> Result<Page, String> {
    let source = lookup(&request.source)?;
    match request.action {
        Action::Page(index) => source.page(index, None),
        Action::Find { query, from } => source.find(&query, from),
    }
}
/// Worker-only bounded source for derived indexes; never expands the GTK preview.
pub fn prefix(reference: &Reference, limit: usize) -> Result<(String, bool), String> {
    let source = lookup(reference)?;
    let mut end = source.text.len().min(limit);
    while !source.text.is_char_boundary(end) {
        end -= 1;
    }
    Ok((source.text[..end].to_owned(), end < source.text.len()))
}
#[cfg(test)]
pub fn export(source: Reference, path: std::path::PathBuf) -> Result<(), String> {
    prepare_export(source, path)?.write()
}
pub struct Export {
    source: Arc<Document>,
    path: std::path::PathBuf,
}
/// Capture a lease before queueing disk I/O, so closing the viewer cannot turn
/// a user-confirmed export into an expired-cache failure.
pub fn prepare_export(source: Reference, path: std::path::PathBuf) -> Result<Export, String> {
    Ok(Export {
        source: lookup(&source)?,
        path,
    })
}
impl Export {
    pub fn write(self) -> Result<(), String> {
        use std::io::Write;
        let Self { source, path } = self;
        #[cfg(debug_assertions)]
        if let Some(root) = std::env::var_os("STUDIO_GNOME_SMOKE_DIR").map(std::path::PathBuf::from)
        {
            if std::env::var_os("CODEX_THREAD_STUDIO_CODEX_BIN").map(std::path::PathBuf::from)
                == Some(root.join("fake-codex.py"))
                && std::env::var_os("XDG_CONFIG_HOME").map(std::path::PathBuf::from)
                    == Some(root.join("config"))
            {
                let marker = if std::env::var_os("STUDIO_GNOME_SMOKE_SAVE_PORTAL").is_some()
                    && path == root.join("confirmed-output.txt")
                {
                    Some("export-worker".to_string())
                } else if std::env::var_os("STUDIO_GNOME_SMOKE_WRITE_FAILURES").is_some() {
                    [
                        "failed-output-closed",
                        "failed-output-replaced",
                        "failed-output-queue-a",
                        "failed-output-queue-b",
                    ]
                    .into_iter()
                    .find(|name| path == root.join(format!("{name}.txt")))
                    .map(str::to_string)
                } else {
                    None
                };
                if let Some(marker) = marker {
                    // Pause an admitted lease before I/O, only in a validated
                    // private debug fixture. Never run on the GTK thread.
                    std::fs::write(
                        root.join(format!("{marker}-started")),
                        source.text.len().to_string(),
                    )
                    .map_err(|e| e.to_string())?;
                    let at = std::time::Instant::now();
                    while !root.join(format!("{marker}-release")).is_file() {
                        if at.elapsed() > std::time::Duration::from_secs(15) {
                            return Err("Private export fixture did not release its worker".into());
                        }
                        std::thread::sleep(std::time::Duration::from_millis(5));
                    }
                }
            }
        }
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .ok_or("Choose an absolute export path")?;
        if !path.is_absolute() {
            return Err("Choose an absolute export path".into());
        }
        let mut file = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
        file.write_all(source.text.as_bytes())
            .map_err(|e| e.to_string())?;
        file.as_file().sync_all().map_err(|e| e.to_string())?;
        file.persist(path).map_err(|e| e.to_string())?;
        Ok(())
    }
}
pub async fn run(
    mut input: tokio::sync::watch::Receiver<Option<Request>>,
    events: async_channel::Sender<crate::protocol::Event>,
) {
    let mut ready = false;
    loop {
        if !ready && input.changed().await.is_err() {
            break;
        }
        let Some(request) = input.borrow_and_update().clone() else {
            let _ = pin(None);
            ready = false;
            continue;
        };
        let dialog = request.dialog;
        let serial = request.serial;
        if let Err(error) = pin(Some(&request.source)) {
            ready = false;
            if events
                .send(crate::protocol::Event::ConversationOutput(Response {
                    dialog,
                    serial,
                    result: Err(error),
                }))
                .await
                .is_err()
            {
                break;
            }
            continue;
        }
        tokio::select! {
            changed=input.changed()=>{if changed.is_err(){break;} ready=true;},
            result=crate::worker::compute(move||execute(request))=>{
                ready=false;
                if events.send(crate::protocol::Event::ConversationOutput(Response{dialog,serial,result})).await.is_err(){break;}
            }
        }
    }
    let _ = pin(None);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pages_round_trip_unicode_crlf_long_lines_and_exact_global_positions() {
        for text in [
            format!(
                "{}\r\n{}",
                "字😀".repeat(40_000),
                "line 中文😀\r\n".repeat(2000)
            ),
            "\n".repeat(100_000),
            String::new(),
            format!("{}\r\nend", "x".repeat(PAGE_BYTES - 1)),
        ] {
            let source = Document::new(text.clone()).unwrap();
            let mut rebuilt = String::new();
            for index in 0..source.pages.len() {
                let p = source.page(index, None).unwrap();
                assert!(p.text.len() <= PAGE_BYTES);
                assert!(p.last_line - p.first_line <= PAGE_LINES);
                assert_eq!(p.first_line, 1 + text[..p.offset].matches('\n').count());
                assert_eq!(
                    p.continued,
                    p.offset > 0 && text.as_bytes()[p.offset - 1] != b'\n'
                );
                rebuilt.push_str(&p.text);
            }
            assert_eq!(rebuilt, text);
            assert_eq!(source.lines, 1 + text.matches('\n').count());
        }
    }
    #[test]
    fn search_is_literal_and_wraps_without_dropping_trailing_spaces() {
        let text = format!(
            "{}中文😀 needle \r\n{}needleX",
            "x".repeat(PAGE_BYTES - 6),
            "next\n".repeat(900)
        );
        let source = Document::new(text.clone()).unwrap();
        let p = source.find("needle ", 0).unwrap();
        let s = p.selection.unwrap();
        assert!(
            s.continues
                || p.text
                    .chars()
                    .skip(s.start as usize)
                    .take((s.end - s.start) as usize)
                    .collect::<String>()
                    == "needle "
        );
        assert!(source.find("needle  ", 0).is_err());
        assert!(source.find("needle ", text.len()).is_ok());
        assert!(source.find("x", text.len() + 1).is_err());
    }
    #[test]
    fn control_display_preserves_offsets_and_lf_line_numbering() {
        let source =
            Document::new("head\r\nprogress\rnext\u{2028}soft\u{2029}paragraph\0tail\n".into())
                .unwrap();
        let page = source.find("tail", 0).unwrap();
        let selected = page.selection.unwrap();
        assert!(page.controls);
        assert_eq!(page.display.chars().count(), page.text.chars().count());
        assert!(!page.display.contains('\0'));
        assert!(page.display.contains("progress␍next↵soft¶paragraph␀tail"));
        assert_eq!(
            page.display
                .chars()
                .skip(selected.start as usize)
                .take((selected.end - selected.start) as usize)
                .collect::<String>(),
            "tail"
        );
        assert_eq!(page.last_line, 3);
    }
    #[test]
    fn cache_bounds_collisions_eviction_expiry_and_atomic_export() {
        let mut cache = Store::new(10000);
        let a = cache
            .insert(1, Document::new("a".repeat(6000)).unwrap())
            .unwrap();
        let reference = Reference {
            token: a.clone(),
            bytes: 6000,
            unavailable: None,
        };
        let pinned = cache.source(&reference).unwrap();
        cache.active = Some((a.clone(), pinned));
        assert_eq!(cache.existing(1, &"a".repeat(6000)), Some(a.clone()));
        assert!(cache.existing(1, "different").is_none());
        cache
            .insert(1, Document::new("b".repeat(6000)).unwrap())
            .unwrap();
        assert!(cache.entries.peek(&a).is_none());
        assert_eq!(cache.source(&reference).unwrap().text.len(), 6000);
        cache.active = None;
        assert!(cache.source(&reference).err().unwrap().contains("expired"));
        assert!(cache.bytes <= cache.budget);
        let reference = retain("Full 中文😀\r\nnot a displayed excerpt\n".into());
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("output.txt");
        export(reference.clone(), path.clone()).unwrap();
        assert_eq!(
            std::fs::read_to_string(path).unwrap(),
            "Full 中文😀\r\nnot a displayed excerpt\n"
        );
        let mut fake = reference.clone();
        fake.token = "0".repeat(32);
        assert!(lookup(&fake).err().unwrap().contains("expired"));
        fake = reference;
        fake.bytes += 1;
        assert!(lookup(&fake).is_err());
        assert!(retain("x".repeat(MAX_TEXT + 1)).unavailable.is_some());
    }
}
