//! User-selected local images only. File I/O and encoding run on Rust workers;
//! GTK holds shared immutable inputs, never copies base64 into widget state.
use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::{json, Value};
use std::{collections::HashMap, io::Read, path::PathBuf, sync::Arc};

pub const MAX_IMAGES: usize = 4;
pub const MAX_FILE: usize = 10 * 1024 * 1024;
pub const MAX_TOTAL: usize = 20 * 1024 * 1024;
const CACHE_BYTES: usize = 64 * 1024 * 1024;
pub const MIMES: &[&str] = &["image/png", "image/jpeg", "image/webp", "image/gif"];

pub struct Image {
    pub id: String,
    pub name: String,
    pub size: usize,
    pub input: Value,
    pub weight: usize,
}
impl std::fmt::Debug for Image {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Image")
            .field("id", &self.id)
            .field("size", &self.size)
            .finish_non_exhaustive()
    }
}
#[derive(Debug)]
pub enum Source {
    Files(Vec<PathBuf>),
    Clipboard(Vec<u8>),
}
pub fn mime(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some(MIMES[0])
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        Some(MIMES[1])
    } else if bytes.get(..4) == Some(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        Some(MIMES[2])
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some(MIMES[3])
    } else {
        None
    }
}
fn image(bytes: Vec<u8>, name: String, id: String) -> Result<Arc<Image>, String> {
    if bytes.is_empty() || bytes.len() > MAX_FILE {
        return Err("Each image must be non-empty and 10 MiB or smaller".into());
    }
    let mime = mime(&bytes).ok_or("Only PNG, JPEG, WebP and GIF images can be attached")?;
    let url = format!("data:{mime};base64,{}", STANDARD.encode(&bytes));
    let weight = url.len() + name.len() + id.len() + 128;
    Ok(Arc::new(Image {
        id,
        name,
        size: bytes.len(),
        input: json!({"type":"image","url":url}),
        weight,
    }))
}
pub fn prepare(source: Source, serial: u64) -> Result<Vec<Arc<Image>>, String> {
    let sources = match source {
        Source::Files(files) => {
            if files.is_empty() || files.len() > MAX_IMAGES {
                return Err("Choose at most four images".into());
            }
            let mut values = Vec::new();
            let mut total = 0;
            for path in files {
                let mut options = std::fs::OpenOptions::new();
                options.read(true);
                #[cfg(unix)]
                {
                    use std::os::unix::fs::OpenOptionsExt;
                    options.custom_flags(libc::O_NONBLOCK | libc::O_CLOEXEC);
                }
                let file = options
                    .open(&path)
                    .map_err(|error| format!("Cannot read selected image: {error}"))?;
                let metadata = file.metadata().map_err(|error| error.to_string())?;
                if !metadata.is_file() || metadata.len() > MAX_FILE as u64 {
                    return Err("Choose ordinary image files of 10 MiB or smaller".into());
                }
                let mut bytes = Vec::new();
                file.take((MAX_FILE + 1) as u64)
                    .read_to_end(&mut bytes)
                    .map_err(|error| error.to_string())?;
                total += bytes.len();
                if total > MAX_TOTAL {
                    return Err("Images in one message must total 20 MiB or less".into());
                }
                let name = path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .chars()
                    .filter(|ch| !ch.is_control())
                    .take(180)
                    .collect();
                values.push((bytes, name));
            }
            values
        }
        Source::Clipboard(bytes) => vec![(bytes, "Clipboard image".into())],
    };
    sources
        .into_iter()
        .enumerate()
        .map(|(index, (bytes, name))| image(bytes, name, format!("image-{serial}-{index}")))
        .collect()
}

#[derive(Default)]
pub struct Book {
    pub sessions: HashMap<String, Vec<Arc<Image>>>,
    flights: HashMap<String, Vec<Arc<Image>>>,
    pub pending: HashMap<String, u64>,
    pub revision: u64,
    sequence: u64,
}
impl Book {
    pub fn images(&self, key: &str) -> Vec<Arc<Image>> {
        self.sessions.get(key).cloned().unwrap_or_default()
    }
    pub fn begin(&mut self, key: &str) -> Result<u64, String> {
        if self.pending.contains_key(key) {
            return Err("Wait for the selected images to load".into());
        }
        if self.pending.len() >= 2 || self.sessions.len() + self.pending.len() >= 128 {
            return Err("Finish loading or remove pending attachments first".into());
        }
        self.sequence += 1;
        self.pending.insert(key.into(), self.sequence);
        self.revision += 1;
        Ok(self.sequence)
    }
    pub fn finish(
        &mut self,
        key: &str,
        serial: u64,
        result: Result<Vec<Arc<Image>>, String>,
    ) -> Result<(), String> {
        if self.pending.get(key) != Some(&serial) {
            return Ok(());
        }
        self.pending.remove(key);
        self.revision += 1;
        let images = result?;
        let current = self.images(key);
        if current.len() + images.len() > MAX_IMAGES
            || current
                .iter()
                .chain(&images)
                .map(|image| image.size)
                .sum::<usize>()
                > MAX_TOTAL
        {
            return Err("One message supports four images and 20 MiB total".into());
        }
        let mut retained = std::collections::HashSet::new();
        let weight = self
            .sessions
            .values()
            .flatten()
            .chain(self.flights.values().flatten())
            .chain(&images)
            .filter(|image| retained.insert(image.id.as_str()))
            .map(|image| image.weight)
            .sum::<usize>();
        if weight > CACHE_BYTES {
            return Err(
                "Pending images use the 64 MiB memory budget; send or remove some first".into(),
            );
        }
        self.sessions.entry(key.into()).or_default().extend(images);
        Ok(())
    }
    pub fn remove(&mut self, key: &str, ids: &[String]) {
        if let Some(images) = self.sessions.get_mut(key) {
            images.retain(|image| !ids.contains(&image.id));
            if images.is_empty() {
                self.sessions.remove(key);
            }
        }
        self.revision += 1;
    }
    pub fn sending(&mut self, key: &str, ids: &[String]) {
        let images = self
            .images(key)
            .into_iter()
            .filter(|image| ids.contains(&image.id))
            .collect::<Vec<_>>();
        if !images.is_empty() {
            self.flights.insert(key.into(), images);
        }
    }
    pub fn acknowledged(&mut self, key: &str, ids: &[String], accepted: bool) {
        self.flights.remove(key);
        if accepted {
            self.remove(key, ids);
        }
    }
    pub fn disconnected(&mut self, backend: Option<&str>) {
        self.flights.retain(|key, _| {
            backend.is_some_and(|backend| !key.starts_with(&format!("{backend}:")))
        });
    }
    pub fn forget(&mut self, key: &str) {
        self.pending.remove(key);
        self.sessions.remove(key);
        self.revision += 1;
    }
}

pub fn append(params: &mut Value, images: Vec<Arc<Image>>) -> Result<(), String> {
    if images.len() > MAX_IMAGES || images.iter().map(|image| image.size).sum::<usize>() > MAX_TOTAL
    {
        return Err("Attachment limit exceeded".into());
    }
    let input = params["input"]
        .as_array_mut()
        .ok_or("Invalid message input")?;
    input.extend(images.iter().map(|image| image.input.clone()));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reads_signatures_without_trusting_extensions_and_rejects_special_or_oversized_files() {
        let dir = tempfile::tempdir().unwrap();
        let image_path = dir.path().join("图像.txt");
        let bytes = b"\x89PNG\r\n\x1a\nprivate fixture";
        std::fs::write(&image_path, bytes).unwrap();
        let image = prepare(Source::Files(vec![image_path]), 1)
            .unwrap()
            .remove(0);
        assert_eq!(image.name, "图像.txt");
        assert_eq!(
            STANDARD
                .decode(
                    image.input["url"]
                        .as_str()
                        .unwrap()
                        .split_once(',')
                        .unwrap()
                        .1
                )
                .unwrap(),
            bytes
        );
        assert!(!format!("{image:?}").contains("base64"));
        assert!(prepare(Source::Files(vec![dir.path().into()]), 2).is_err());
        assert!(prepare(Source::Clipboard(b"<svg>not supported</svg>".to_vec()), 3).is_err());
        assert!(prepare(Source::Clipboard(vec![0; MAX_FILE + 1]), 4).is_err());
        #[cfg(unix)]
        assert!(prepare(Source::Files(vec!["/dev/zero".into()]), 5).is_err());
    }
    #[test]
    fn acknowledgements_and_late_preparation_preserve_other_sessions_and_newer_images() {
        let mut book = Book::default();
        let one = book.begin("codex:one").unwrap();
        let two = book.begin("opencode:two").unwrap();
        let result = prepare(Source::Clipboard(b"GIF89atest".to_vec()), one);
        book.finish("codex:one", one, result).unwrap();
        let original = book.images("codex:one")[0].id.clone();
        let next = book.begin("codex:one").unwrap();
        book.finish(
            "codex:one",
            next,
            prepare(Source::Clipboard(b"GIF89anewer".to_vec()), next),
        )
        .unwrap();
        book.remove("codex:one", &[original]);
        assert_eq!(book.images("codex:one").len(), 1);
        book.forget("opencode:two");
        book.finish(
            "opencode:two",
            two,
            prepare(Source::Clipboard(b"GIF89astale".to_vec()), two),
        )
        .unwrap();
        assert!(book.images("opencode:two").is_empty());
        let failed = book.begin("codex:one").unwrap();
        assert!(book
            .finish("codex:one", failed, Err("unreadable".into()))
            .is_err());
        assert_eq!(book.images("codex:one").len(), 1);
    }
    #[test]
    fn removing_inflight_images_does_not_release_their_memory_budget_until_acknowledged() {
        let mut book = Book::default();
        let serial = book.begin("codex:one").unwrap();
        let mut images = prepare(Source::Clipboard(b"GIF89atest".to_vec()), serial).unwrap();
        Arc::get_mut(&mut images[0]).unwrap().weight = CACHE_BYTES;
        let id = images[0].id.clone();
        book.finish("codex:one", serial, Ok(images)).unwrap();
        book.sending("codex:one", &[id.clone()]);
        book.remove("codex:one", &[id]);
        let next = book.begin("opencode:two").unwrap();
        assert!(book
            .finish(
                "opencode:two",
                next,
                prepare(Source::Clipboard(b"GIF89anew".to_vec()), next)
            )
            .is_err());
        book.acknowledged("codex:one", &[], false);
        let next = book.begin("opencode:two").unwrap();
        book.finish(
            "opencode:two",
            next,
            prepare(Source::Clipboard(b"GIF89anew".to_vec()), next),
        )
        .unwrap();
        assert_eq!(book.images("opencode:two").len(), 1);
    }
}
