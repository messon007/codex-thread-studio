//! Validated image pixels received from the Rust rendering service. No codecs in GTK.
use std::{path::Path, sync::Arc};
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Source {
    Remote {
        url: String,
    },
    File {
        root: String,
        path: String,
    },
    Inline {
        token: String,
    },
    Embedded(studio_markdown_native::embedded_image::Image),
    Document {
        reader: String,
        revision: String,
        page: usize,
        image: usize,
        original: bool,
    },
}
#[derive(Debug)]
pub struct Frame {
    pub original: (u32, u32),
    pub width: u32,
    pub height: u32,
    pub pixels: Arc<[u8]>,
    pub identity: Option<Identity>,
    pub source: Option<studio_markdown_native::embedded_image::Image>,
    pub animation: Option<Arc<crate::image_animation::Timeline>>,
    pub animation_limited: bool,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Identity {
    sha256: String,
    extension: &'static str,
}
impl Identity {
    pub fn extension(&self) -> &'static str {
        self.extension
    }
    fn parse(headers: &reqwest::header::HeaderMap) -> Result<Self, String> {
        let sha256 = headers
            .get("x-studio-source-sha256")
            .and_then(|h| h.to_str().ok())
            .filter(|s| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit()))
            .ok_or("Image source identity is missing or invalid; rebuild the gateway")?;
        let extension = match headers
            .get("x-studio-source-mime")
            .and_then(|h| h.to_str().ok())
        {
            Some("image/png") => "png",
            Some("image/jpeg") => "jpg",
            Some("image/gif") => "gif",
            Some("image/webp") => "webp",
            Some("image/svg+xml") => "svg",
            _ => return Err("Unsupported original image format".into()),
        };
        Ok(Self {
            sha256: sha256.to_ascii_lowercase(),
            extension,
        })
    }
}
pub fn supports(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| {
            matches!(
                value.to_ascii_lowercase().as_str(),
                "png" | "jpg" | "jpeg" | "webp" | "gif" | "svg"
            )
        })
}
impl Frame {
    pub(crate) fn parse_image(
        bytes: Vec<u8>,
        headers: reqwest::header::HeaderMap,
    ) -> Result<Self, String> {
        let identity = Identity::parse(&headers)?;
        let mut frame = Self::parse(bytes)?;
        frame.animation_limited = headers
            .get("x-studio-animation-limited")
            .is_some_and(|h| h == "1");
        frame.identity = Some(identity);
        Ok(frame)
    }
    fn parse_document(
        bytes: Vec<u8>,
        headers: reqwest::header::HeaderMap,
        original: bool,
    ) -> Result<Self, String> {
        if !original {
            return Self::parse_image(bytes, headers);
        }
        if bytes.len() < 12 || &bytes[..8] != b"STIMS001" {
            return Err("Invalid document image source envelope".into());
        }
        let count = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
        if count == 0 || count > 10 * 1024 * 1024 || 12 + count > bytes.len() {
            return Err("Document image source exceeds its budget".into());
        }
        use sha2::Digest;
        let identity = Identity::parse(&headers)?;
        let hash = sha2::Sha256::digest(&bytes[12..12 + count])
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        if hash != identity.sha256 {
            return Err("Document image source identity mismatch".into());
        }
        let mime = match identity.extension {
            "png" => "image/png",
            "jpg" => "image/jpeg",
            "gif" => "image/gif",
            "webp" => "image/webp",
            _ => "image/svg+xml",
        };
        let source = studio_markdown_native::embedded_image::Image {
            mime,
            bytes: bytes[12..12 + count].into(),
        };
        let mut frame = Self::parse_image(bytes[12 + count..].to_vec(), headers)?;
        frame.source = Some(source);
        Ok(frame)
    }
    pub(crate) fn parse(bytes: Vec<u8>) -> Result<Self, String> {
        if bytes.starts_with(b"STIMA001") {
            let (original, timeline) = crate::image_animation::Timeline::parse(&bytes)?;
            return Ok(Self {
                original,
                width: timeline.width,
                height: timeline.height,
                pixels: timeline.frames[0].pixels.clone(),
                identity: None,
                source: None,
                animation: Some(Arc::new(timeline)),
                animation_limited: false,
            });
        }
        if bytes.len() < 24 || &bytes[..8] != b"STIMG001" {
            return Err("Invalid image preview response".into());
        }
        let number = |offset| u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap());
        let (original, width, height) = ((number(8), number(12)), number(16), number(20));
        if original.0 == 0
            || original.1 == 0
            || original.0 > 1_000_000
            || original.1 > 1_000_000
            || width == 0
            || height == 0
            || width > 2048
            || height > 2048
            || bytes.len() != 24 + width as usize * height as usize * 4
        {
            return Err("Image pixels exceed their presentation budget".into());
        }
        Ok(Self {
            original,
            width,
            height,
            pixels: bytes[24..].into(),
            identity: None,
            source: None,
            animation: None,
            animation_limited: false,
        })
    }
}
pub async fn load(api: &crate::gateway::Api, root: String, path: String) -> Result<Frame, String> {
    let (bytes, headers) = api
        .binary_with_headers(
            "/studio/native/image",
            &serde_json::json!({"root":root,"path":path,"animation":true}),
        )
        .await?;
    crate::worker::compute(move || Frame::parse_image(bytes, headers)).await
}
pub async fn load_source(api: &crate::gateway::Api, source: Source) -> Result<Frame, String> {
    match source {
        Source::Remote { url } => {
            let source = crate::remote_images::load(&url).await?;
            let original = source.clone();
            let bytes = crate::worker::compute(move || Ok(source.to_vec())).await?;
            let (pixels, headers) = api.inline_animation(bytes).await?;
            crate::worker::compute(move || {
                let mut frame = Frame::parse_image(pixels, headers)?;
                let mime = match frame.identity.as_ref().map(|i| i.extension) {
                    Some("png") => "image/png",
                    Some("jpg") => "image/jpeg",
                    Some("gif") => "image/gif",
                    Some("webp") => "image/webp",
                    _ => "image/svg+xml",
                };
                frame.source = Some(studio_markdown_native::embedded_image::Image {
                    mime,
                    bytes: original,
                });
                Ok(frame)
            })
            .await
        }
        Source::File { root, path } => load(api, root, path).await,
        Source::Inline { token } => {
            let bytes = crate::worker::compute(move || {
                crate::conversation_images::bytes(&token).map(|b| b.to_vec())
            })
            .await?;
            let (pixels, headers) = api.inline_animation(bytes).await?;
            crate::worker::compute(move || Frame::parse_image(pixels, headers)).await
        }
        Source::Document {
            reader,
            revision,
            page,
            image,
            original,
        } => {
            if reader.is_empty()
                || reader.len() > 128
                || !reader
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-')
                || revision.len() != 64
                || !revision.bytes().all(|b| b.is_ascii_hexdigit())
            {
                return Err("Invalid document image reference".into());
            }
            let (bytes,headers)=api.get_binary_with_headers(&format!("/studio/native/markdown/{reader}/image?page={page}&image={image}&revision={revision}&original={original}&animation=true")).await?;
            crate::worker::compute(move || Frame::parse_document(bytes, headers, original)).await
        }
        Source::Embedded(image) => {
            let bytes = crate::worker::compute(move || Ok(image.bytes.to_vec())).await?;
            let (pixels, headers) = api.inline_animation(bytes).await?;
            crate::worker::compute(move || Frame::parse_image(pixels, headers)).await
        }
    }
}

enum Original {
    File {
        root: String,
        path: String,
        sha256: String,
    },
    Inline(Arc<[u8]>),
}
pub struct Export {
    source: Original,
    path: std::path::PathBuf,
    sha256: String,
}
/// Capture immutable inline bytes before an admitted write can outlive its view.
pub fn prepare_export(
    source: Source,
    identity: Identity,
    path: std::path::PathBuf,
) -> Result<Export, String> {
    if !path.is_absolute() || path.parent().is_none() {
        return Err("Choose an absolute local destination".into());
    }
    let source = match source {
        Source::File { root, path } => Original::File {
            root,
            path,
            sha256: identity.sha256.clone(),
        },
        Source::Inline { token } => Original::Inline(crate::conversation_images::bytes(&token)?),
        Source::Embedded(image) => Original::Inline(image.bytes),
        Source::Document { .. } | Source::Remote { .. } => {
            return Err("Wait for the image's immutable source before saving".into())
        }
    };
    Ok(Export {
        source,
        path,
        sha256: identity.sha256,
    })
}
pub async fn export(api: &crate::gateway::Api, plan: Export) -> Result<(), String> {
    let bytes = match plan.source {
        Original::File { root, path, sha256 } => api
            .binary(
                "/studio/native/image/original",
                &serde_json::json!({"root":root,"path":path,"sha256":sha256}),
            )
            .await?
            .into(),
        Original::Inline(bytes) => bytes,
    };
    crate::worker::compute(move || write_original(&bytes, &plan.path, &plan.sha256)).await
}
fn write_original(bytes: &[u8], path: &Path, expected: &str) -> Result<(), String> {
    use sha2::Digest;
    use std::io::Write;
    #[cfg(debug_assertions)]
    if let Some(root) = std::env::var_os("STUDIO_GNOME_SMOKE_DIR").map(std::path::PathBuf::from) {
        if std::env::var_os("STUDIO_GNOME_SMOKE_IMAGE_SAVE").is_some()
            && std::env::var_os("CODEX_THREAD_STUDIO_CODEX_BIN").map(std::path::PathBuf::from)
                == Some(root.join("fake-codex.py"))
            && std::env::var_os("XDG_CONFIG_HOME").map(std::path::PathBuf::from)
                == Some(root.join("config"))
        {
            if let Some(name) = [
                "admitted-original.png",
                "inline-original.gif",
                "failed-original.png",
            ]
            .into_iter()
            .find(|name| path == root.join(name))
            {
                std::fs::write(
                    root.join(format!("{name}-started")),
                    bytes.len().to_string(),
                )
                .map_err(|e| e.to_string())?;
                let at = std::time::Instant::now();
                while !root.join(format!("{name}-release")).is_file() {
                    if at.elapsed() > std::time::Duration::from_secs(15) {
                        return Err("Private image save fixture did not release its worker".into());
                    }
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }
            }
        }
    }
    if bytes.is_empty() || bytes.len() > 25 * 1024 * 1024 {
        return Err("Original image exceeds its export budget".into());
    }
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .ok_or("Choose an absolute local destination")?;
    let mut file = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    let mut hash = sha2::Sha256::new();
    for chunk in bytes.chunks(64 * 1024) {
        hash.update(chunk);
        file.write_all(chunk).map_err(|e| e.to_string())?;
    }
    let hash = hash
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    if hash != expected {
        return Err(
            "Original image identity no longer matches the displayed image. Reload before saving."
                .into(),
        );
    }
    file.as_file().sync_all().map_err(|e| e.to_string())?;
    file.persist(path).map_err(|e| e.to_string())?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn document_source_lease_requires_a_complete_matching_envelope() {
        use sha2::Digest;
        let original = b"immutable source";
        let digest = sha2::Sha256::digest(original)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("x-studio-source-sha256", digest.parse().unwrap());
        headers.insert("x-studio-source-mime", "image/png".parse().unwrap());
        let mut pixels = b"STIMG001".to_vec();
        for value in [1u32, 1, 1, 1] {
            pixels.extend_from_slice(&value.to_le_bytes());
        }
        pixels.extend_from_slice(&[0, 1, 2, 255]);
        let mut envelope = b"STIMS001".to_vec();
        envelope.extend_from_slice(&(original.len() as u32).to_le_bytes());
        envelope.extend_from_slice(original);
        envelope.extend_from_slice(&pixels);
        let frame = Frame::parse_document(envelope.clone(), headers.clone(), true).unwrap();
        assert_eq!(frame.source.unwrap().bytes.as_ref(), original);
        assert_eq!(frame.original, (1, 1));
        let mut changed = envelope.clone();
        changed[12] ^= 1;
        assert!(Frame::parse_document(changed, headers.clone(), true).is_err());
        let mut changed = envelope.clone();
        changed[8..12].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(Frame::parse_document(changed, headers.clone(), true).is_err());
        assert!(Frame::parse_document(pixels, headers.clone(), true).is_err());
        assert!(Frame::parse_document(envelope, headers, false).is_err());
    }
    #[test]
    fn wire_requires_matching_dimensions_and_complete_pixels() {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("x-studio-source-sha256", "0".repeat(64).parse().unwrap());
        headers.insert("x-studio-source-mime", "image/png".parse().unwrap());
        let mut bytes = b"STIMG001".to_vec();
        for value in [2u32, 1, 2, 1] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        bytes.extend_from_slice(&[255, 0, 0, 255, 0, 0, 0, 0]);
        let frame = Frame::parse_image(bytes.clone(), headers.clone()).unwrap();
        assert_eq!(frame.original, (2, 1));
        assert_eq!(frame.pixels.len(), 8);
        bytes.pop();
        assert!(Frame::parse_image(bytes, headers.clone()).is_err());
        assert!(Frame::parse_image(Vec::new(), headers).is_err());
    }
    #[test]
    fn original_write_checks_identity_before_replacing_the_destination() {
        use sha2::Digest;
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("original 中文.png");
        std::fs::write(&path, b"old destination").unwrap();
        let bytes = b"Original bytes\0\xff\x80";
        assert!(write_original(bytes, &path, &"0".repeat(64)).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"old destination");
        let hash = sha2::Sha256::digest(bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        write_original(bytes, &path, &hash).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        assert_eq!(std::fs::read_dir(folder.path()).unwrap().count(), 1);
    }
    #[test]
    fn original_metadata_requires_a_digest_and_supported_type() {
        let mut headers = reqwest::header::HeaderMap::new();
        assert!(Identity::parse(&headers).is_err());
        headers.insert("x-studio-source-sha256", "A".repeat(64).parse().unwrap());
        headers.insert("x-studio-source-mime", "image/jpeg".parse().unwrap());
        let identity = Identity::parse(&headers).unwrap();
        assert_eq!(identity.sha256, "a".repeat(64));
        assert_eq!(identity.extension(), "jpg");
        headers.insert("x-studio-source-mime", "text/html".parse().unwrap());
        assert!(Identity::parse(&headers).is_err());
    }
}
