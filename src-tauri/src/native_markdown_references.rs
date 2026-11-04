//! Snapshot-private reference metadata. Large destinations are read only on demand.
use marknoria::server::ReaderSnapshot;
use std::{
    cell::RefCell,
    collections::HashMap,
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::PathBuf,
    sync::{Arc, Mutex, OnceLock},
};
use studio_markdown_native::references::{Definition, Key};

const MAX_METADATA: u64 = 8 * 1024 * 1024;
type Record = (String, usize, usize, usize);
struct Index {
    records: HashMap<Key, Record>,
    bytes: u64,
}
type Cached = Option<(PathBuf, String, Arc<Index>)>;
static CACHE: OnceLock<Mutex<Cached>> = OnceLock::new();

pub(super) struct Resolver {
    index: Arc<Index>,
    data: RefCell<File>,
    loaded: RefCell<HashMap<Key, Arc<Definition>>>,
    error: RefCell<Option<String>>,
}
impl Resolver {
    pub(super) fn open(snapshot: &ReaderSnapshot) -> Result<Self, String> {
        let directory = snapshot
            .source
            .parent()
            .ok_or("Missing reference directory")?;
        let metadata = directory.join("references.json");
        let cache = CACHE.get_or_init(|| Mutex::new(None));
        let retained = cache
            .lock()
            .map_err(|e| e.to_string())?
            .as_ref()
            .filter(|(path, revision, _)| path == &metadata && revision == &snapshot.revision)
            .map(|(_, _, index)| index.clone());
        let index = if let Some(index) = retained {
            index
        } else {
            let file = File::open(&metadata).map_err(|e| e.to_string())?;
            if file.metadata().map_err(|e| e.to_string())?.len() > MAX_METADATA {
                return Err("Native reference metadata exceeds its budget".into());
            }
            let entries: Vec<Record> =
                serde_json::from_reader(file.take(MAX_METADATA + 1)).map_err(|e| e.to_string())?;
            if entries.len() > 32_768 {
                return Err("Too many native reference definitions".into());
            }
            let mut records = HashMap::new();
            let mut end = 0usize;
            for record in entries {
                if record.0.is_empty() || record.0.len() > 4096 || record.1 != end {
                    return Err("Invalid native reference index".into());
                }
                end = end
                    .checked_add(record.2)
                    .and_then(|n| n.checked_add(record.3))
                    .filter(|n| *n <= marknoria::MAX_SOURCE as usize)
                    .ok_or("Invalid reference byte budget")?;
                if records.insert(Key::new(record.0.clone()), record).is_some() {
                    return Err("Duplicate native reference definition".into());
                }
            }
            let index = Arc::new(Index {
                records,
                bytes: end as u64,
            });
            *cache.lock().map_err(|e| e.to_string())? =
                Some((metadata, snapshot.revision.clone(), index.clone()));
            index
        };
        let data = File::open(directory.join("references.bin")).map_err(|e| e.to_string())?;
        if data.metadata().map_err(|e| e.to_string())?.len() != index.bytes {
            return Err("Native reference snapshot changed".into());
        }
        Ok(Self {
            index,
            data: RefCell::new(data),
            loaded: RefCell::new(HashMap::new()),
            error: RefCell::new(None),
        })
    }
    pub(super) fn is_empty(&self) -> bool {
        self.index.records.is_empty()
    }
    pub(super) fn check(&self) -> Result<(), String> {
        self.error
            .borrow()
            .as_ref()
            .map_or(Ok(()), |e| Err(e.clone()))
    }
    pub(super) fn lookup(&self, label: &str) -> Option<Arc<Definition>> {
        let key = Key::new(label.to_owned());
        if let Some(value) = self.loaded.borrow().get(&key) {
            return Some(value.clone());
        }
        let (label, offset, destination, title) = self.index.records.get(&key)?;
        let result = (|| -> Result<_, String> {
            let mut bytes = vec![0; destination + title];
            let mut data = self.data.borrow_mut();
            data.seek(SeekFrom::Start(*offset as u64))
                .map_err(|e| e.to_string())?;
            data.read_exact(&mut bytes).map_err(|e| e.to_string())?;
            let text = String::from_utf8(bytes).map_err(|_| "Reference is not UTF-8")?;
            if !text.is_char_boundary(*destination) {
                return Err("Invalid reference boundary".into());
            }
            Ok(Arc::new(Definition {
                label: label.clone(),
                destination: text[..*destination].to_owned(),
                title: text[*destination..].to_owned(),
            }))
        })();
        match result {
            Ok(value) => {
                self.loaded.borrow_mut().insert(key, value.clone());
                Some(value)
            }
            Err(error) => {
                *self.error.borrow_mut() = Some(error);
                None
            }
        }
    }
}
