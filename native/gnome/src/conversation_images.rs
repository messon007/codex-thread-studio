//! Process-private image source cache. Call only on Rust workers; GTK sees references.
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::hash::BuildHasher;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, OnceLock},
};
pub const MAX_IMAGE: usize = studio_markdown_native::embedded_image::MAX_BYTES;
const CACHE: usize = 64 * 1024 * 1024;
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum Source {
    Remote { url: String },
    Inline { token: String },
    Local { path: String },
    Unavailable { reason: String },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Image {
    pub name: String,
    pub source: Source,
}
struct Stored {
    hash: u64,
    bytes: Arc<[u8]>,
}
struct Store {
    entries: lru::LruCache<String, Stored>,
    index: HashMap<u64, Vec<String>>,
    bytes: usize,
}
impl Default for Store {
    fn default() -> Self {
        Self {
            entries: lru::LruCache::new(std::num::NonZeroUsize::new(8192).unwrap()),
            index: HashMap::new(),
            bytes: 0,
        }
    }
}
fn store() -> &'static Mutex<Store> {
    static STORE: OnceLock<Mutex<Store>> = OnceLock::new();
    STORE.get_or_init(Default::default)
}
impl Store {
    fn retain(&mut self, bytes: impl Into<Arc<[u8]>>, budget: usize) -> Result<String, String> {
        let bytes = bytes.into();
        if bytes.is_empty() || bytes.len() > MAX_IMAGE || bytes.len() > budget {
            return Err("Image source exceeds its cache budget".into());
        }
        let hash = self.index.hasher().hash_one(bytes.as_ref());
        if let Some(keys) = self.index.get(&hash).cloned() {
            for key in keys {
                if self
                    .entries
                    .peek(&key)
                    .is_some_and(|s| s.bytes.as_ref() == bytes.as_ref())
                {
                    self.entries.get(&key);
                    return Ok(key);
                }
            }
        }
        let mut random = [0u8; 16];
        getrandom::fill(&mut random).map_err(|_| "Image identifier unavailable")?;
        let key = random
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        while self.bytes + bytes.len() > budget || self.entries.len() >= 8192 {
            let Some((key, old)) = self.entries.pop_lru() else {
                break;
            };
            self.bytes -= old.bytes.len();
            if let Some(keys) = self.index.get_mut(&old.hash) {
                keys.retain(|k| k != &key);
                if keys.is_empty() {
                    self.index.remove(&old.hash);
                }
            }
        }
        self.bytes += bytes.len();
        self.entries.put(key.clone(), Stored { hash, bytes });
        self.index.entry(hash).or_default().push(key.clone());
        Ok(key)
    }
}
fn retain(bytes: Arc<[u8]>) -> Result<String, String> {
    store()
        .lock()
        .map_err(|_| "Image source cache unavailable")?
        .retain(bytes, CACHE)
}
pub fn bytes(token: &str) -> Result<Arc<[u8]>, String> {
    if token.len() != 32 || !token.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("Invalid image reference".into());
    }
    store()
        .lock()
        .map_err(|_| "Image source cache unavailable")?
        .entries
        .get(token)
        .map(|e| e.bytes.clone())
        .ok_or_else(|| "Image preview expired. Reload the conversation to restore it.".into())
}
#[cfg(debug_assertions)]
pub fn smoke_evict(token: &str) -> Result<(), String> {
    let mut store = store()
        .lock()
        .map_err(|_| "Image source cache unavailable")?;
    let old = store
        .entries
        .pop(token)
        .ok_or("Image source was already absent before eviction")?;
    store.bytes -= old.bytes.len();
    if let Some(keys) = store.index.get_mut(&old.hash) {
        keys.retain(|k| k != token);
        if keys.is_empty() {
            store.index.remove(&old.hash);
        }
    }
    Ok(())
}
impl Image {
    fn unavailable(name: String, reason: impl Into<String>) -> Self {
        Self {
            name,
            source: Source::Unavailable {
                reason: reason.into(),
            },
        }
    }
    pub fn from_part(part: &Value) -> Option<Self> {
        let kind = part["type"].as_str()?;
        if kind == "nativeImage" {
            let image: Self = serde_json::from_value(part["preview"].clone()).ok()?;
            return (image.name.len() <= 512
                && match &image.source {
                    Source::Inline { token } => {
                        token.len() == 32 && token.bytes().all(|b| b.is_ascii_hexdigit())
                    }
                    Source::Local { path } => {
                        path.len() <= 8192 && !path.chars().any(char::is_control)
                    }
                    Source::Remote { url } => crate::remote_images::reference(url).is_ok(),
                    Source::Unavailable { reason } => reason.len() <= 512,
                })
            .then_some(image);
        }
        if !matches!(kind, "image" | "localImage" | "file") {
            return None;
        }
        if kind == "file"
            && !part["mime"]
                .as_str()
                .is_some_and(|m| m.get(..6).is_some_and(|p| p.eq_ignore_ascii_case("image/")))
        {
            return None;
        }
        let name = part["filename"]
            .as_str()
            .filter(|s| !s.is_empty())
            .unwrap_or("Attached image")
            .chars()
            .filter(|c| !c.is_control())
            .take(128)
            .collect::<String>();
        let url = part["url"]
            .as_str()
            .or_else(|| part["image_url"]["url"].as_str())
            .unwrap_or("");
        if kind == "localImage" {
            let path = part["path"].as_str().unwrap_or("");
            return Some(
                if path.is_empty() || path.len() > 8192 || path.chars().any(char::is_control) {
                    Self::unavailable(name, "Invalid local image path")
                } else {
                    Self {
                        name,
                        source: Source::Local { path: path.into() },
                    }
                },
            );
        }
        if url
            .get(..5)
            .is_some_and(|p| p.eq_ignore_ascii_case("file:"))
        {
            return Some(
                match url::Url::parse(url)
                    .ok()
                    .and_then(|u| {
                        if u.host_str().is_none() || u.host_str() == Some("localhost") {
                            u.to_file_path().ok()
                        } else {
                            None
                        }
                    })
                    .and_then(|p| {
                        p.to_str()
                            .filter(|p| p.len() <= 8192 && !p.chars().any(char::is_control))
                            .map(str::to_owned)
                    }) {
                    Some(path) => Self {
                        name,
                        source: Source::Local { path },
                    },
                    None => Self::unavailable(name, "Invalid local image URL"),
                },
            );
        }
        if url::Url::parse(url).is_ok_and(|u| matches!(u.scheme(), "http" | "https")) {
            return Some(match crate::remote_images::reference(url) {
                Ok(url) => Self {
                    name,
                    source: Source::Remote { url },
                },
                Err(reason) => Self::unavailable(name, reason),
            });
        }
        let source = studio_markdown_native::embedded_image::decode(url, MAX_IMAGE)
            .map_err(str::to_owned)
            .and_then(|image| retain(image.bytes));
        Some(match source {
            Ok(token) => Self {
                name,
                source: Source::Inline { token },
            },
            Err(reason) => Self::unavailable(name, reason),
        })
    }
    pub fn part(&self) -> Value {
        json!({"type":"nativeImage","preview":self})
    }
    pub fn weight(&self) -> usize {
        self.name.len()
            + match &self.source {
                Source::Inline { token } => token.len(),
                Source::Local { path } => path.len(),
                Source::Remote { url } => url.len(),
                Source::Unavailable { reason } => reason.len(),
            }
            + std::mem::size_of::<Self>()
    }
}
pub fn project(item: &Value) -> Vec<Image> {
    item["content"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Image::from_part)
        .take(16)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sources_are_private_deduplicated_bounded_and_literal() {
        let mut cache = Store::default();
        let a = cache.retain(vec![1; 64], 128).unwrap();
        assert_eq!(a, cache.retain(vec![1; 64], 128).unwrap());
        let b = cache.retain(vec![2; 64], 128).unwrap();
        cache.retain(vec![3; 64], 128).unwrap();
        assert!(cache.entries.peek(&a).is_none());
        assert!(cache.entries.peek(&b).is_some());
        assert!(cache.bytes <= 128);
        let source =
            Image::from_part(&json!({"type":"image","url":"data:image/png;base64,aGVsbG8="}))
                .unwrap();
        let Source::Inline { token } = &source.source else {
            panic!("inline source")
        };
        assert_eq!(bytes(token).unwrap().as_ref(), b"hello");
        assert_eq!(source, Image::from_part(&source.part()).unwrap());
        assert_eq!(
            source,
            Image::from_part(&json!({"type":"image","url":"DATA:IMAGE/PNG;BASE64,aGVsbG8="}))
                .unwrap()
        );
        assert!(!format!("{source:?}").contains("aGVsbG8"));
        let svg = Image::from_part(&json!({"type":"file","mime":"image/svg+xml","url":"data:image/svg+xml,%3Csvg%3E%3C/svg%3E"})).unwrap();
        let Source::Inline { token } = svg.source else {
            panic!("SVG image should use the shared inline cache")
        };
        assert_eq!(bytes(&token).unwrap().as_ref(), b"<svg></svg>");

        for uri in [
            "http://127.0.0.1/private.png",
            "data:text/html;base64,PHN2Zz4=",
            "data:image/png;base64,!!!!",
        ] {
            let image = Image::from_part(&json!({"type":"image","url":uri})).unwrap();
            assert!(matches!(image.source, Source::Unavailable { .. }));
        }
        assert!(Image::from_part(
            &json!({"type":"file","mime":"text/plain","url":"file:///private.txt"})
        )
        .is_none());
        assert!(
            matches!(Image::from_part(&json!({"type":"file","mime":"IMAGE/PNG","url":"FILE:///project/a%20b.png"})).unwrap().source,Source::Local{path}if path=="/project/a b.png")
        );
        assert!(bytes("not-a-token").is_err());
        let remote =
            Image::from_part(&json!({"type":"image","url":"https://example.org/image.png"}))
                .unwrap();
        assert!(matches!(remote.source, Source::Remote { .. }));
        assert_eq!(remote, Image::from_part(&remote.part()).unwrap());
    }
    #[test]
    fn codex_projection_strips_payloads_and_preserves_attachment_rows() {
        let mut payload = json!({"result":{"thread":{"id":"t","cwd":"/project","turns":[{"id":"turn","status":"completed","items":[{"id":"user","type":"userMessage","content":[{"type":"text","text":"Question"},{"type":"image","url":"data:image/png;base64,aGVsbG8="},{"type":"localImage","path":"/project/a.png"}]}]}]}}});
        let result = crate::protocol::prepare_presentation(&mut payload);
        let item = &result.history.unwrap()[0];
        assert_eq!(item.images.len(), 2);
        assert!(!item.body.contains("base64"));
        assert!(payload.pointer("/result/thread/turns").is_none());
        #[cfg(target_os = "linux")]
        assert_eq!(
            crate::chat_ui::parts(item)
                .filter(|p| matches!(p, crate::chat_ui::Part::Attachment(_)))
                .count(),
            2
        );
    }
}
