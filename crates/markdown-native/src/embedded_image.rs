//! Decode image data URLs on Rust workers, retaining bytes rather than a large
//! encoded URI in widget metadata. Pixel codecs still run in the image service.
use base64::Engine;
use std::{fmt, sync::Arc};

pub const MAX_BYTES: usize = 10 * 1024 * 1024;
pub const MAX_URL_BYTES: usize = 3 * MAX_BYTES + 1024;

#[derive(Clone, PartialEq, Eq)]
pub struct Image {
    pub mime: &'static str,
    pub bytes: Arc<[u8]>,
}
impl fmt::Debug for Image {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EmbeddedImage")
            .field("mime", &self.mime)
            .field("bytes", &self.bytes.len())
            .finish()
    }
}

pub fn decode(source: &str, budget: usize) -> Result<Image, &'static str> {
    if source.len() > MAX_URL_BYTES || source.find(',').is_none_or(|i| i > 1024) {
        return Err("Embedded image URL exceeds its budget");
    }
    let url = data_url::DataUrl::process(source).map_err(|_| "Invalid embedded image URL")?;
    let mime = ["png", "jpeg", "gif", "webp", "svg+xml"]
        .iter()
        .find(|kind| url.mime_type().matches("image", kind))
        .map(|kind| match *kind {
            "png" => "image/png",
            "jpeg" => "image/jpeg",
            "gif" => "image/gif",
            "webp" => "image/webp",
            _ => "image/svg+xml",
        })
        .ok_or("Embedded image must be PNG, JPEG, GIF, WebP or SVG")?;
    let limit = budget.min(if mime == "image/svg+xml" {
        2 * 1024 * 1024
    } else {
        MAX_BYTES
    });
    // Common history/Markdown payloads use canonical padded base64. Keep the
    // existing block decoder for these; percent escapes, whitespace, parameters,
    // missing padding and fragments continue through the standards decoder.
    let (header, encoded) = source.split_once(',').unwrap();
    if header.eq_ignore_ascii_case(&format!("data:{mime};base64"))
        && base64::decoded_len_estimate(encoded.len()) <= limit.saturating_add(2)
    {
        if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(encoded) {
            if bytes.is_empty() {
                return Err("Embedded image is empty");
            }
            if bytes.len() > limit {
                return Err("Embedded image exceeds its decoded byte budget");
            }
            return Ok(Image {
                mime,
                bytes: bytes.into(),
            });
        }
    }
    let mut bytes = Vec::with_capacity(source.len().min(limit));
    url.decode(|chunk| {
        if chunk.len() > limit.saturating_sub(bytes.len()) {
            return Err("Embedded image exceeds its decoded byte budget");
        }
        bytes.extend_from_slice(chunk);
        Ok(())
    })
    .map_err(|error| match error {
        data_url::forgiving_base64::DecodeError::WriteError(error) => error,
        _ => "Invalid embedded image encoding",
    })?;
    if bytes.is_empty() {
        return Err("Embedded image is empty");
    }
    Ok(Image {
        mime,
        bytes: bytes.into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn common_base64_and_standards_forms_produce_identical_bytes() {
        for size in [1, 2, 3, 255, 256, 4097, 65_536] {
            let bytes = (0..size).map(|i| (i % 256) as u8).collect::<Vec<_>>();
            let encoded = base64::engine::general_purpose::STANDARD.encode(&bytes);
            for source in [
                format!("data:image/png;base64,{encoded}"),
                format!("DATA:IMAGE/PNG;BASE64,{encoded}"),
                format!("data:image/png;charset=utf-8;base64,{encoded}"),
                format!("data:image/png;base64,{}", encoded.trim_end_matches('=')),
                format!(
                    "data:image/png;base64,{}#fragment",
                    encoded.replace('=', "%3D")
                ),
            ] {
                let expected = data_url::DataUrl::process(&source)
                    .unwrap()
                    .decode_to_vec()
                    .unwrap()
                    .0;
                assert_eq!(expected, bytes);
                assert_eq!(decode(&source, size).unwrap().bytes.as_ref(), expected);
                assert!(decode(&source, size - 1).is_err());
            }
        }
    }
    #[test]
    fn standards_decoding_is_bounded_and_debug_does_not_print_content() {
        for (source, expected) in [
            ("data:image/png;base64,aGVsbG8=", b"hello".as_slice()),
            ("DATA:IMAGE/JPEG;BASE64,aGVsbG8", b"hello"),
            (
                "data:image/svg+xml;charset=utf-8,%3Csvg%3E%E4%B8%AD%3C/svg%3E",
                "<svg>中</svg>".as_bytes(),
            ),
            ("data:image/gif,%47IF89a", b"GIF89a"),
            ("data:image/webp;base64,UklGRg==#ignored", b"RIFF"),
        ] {
            let image = decode(source, 1024).unwrap();
            assert_eq!(&*image.bytes, expected);
            assert!(!format!("{image:?}").contains("hello"));
            assert!(decode(source, expected.len() - 1).is_err());
        }
        for source in [
            "data:text/html,%3Csvg%3E",
            "data:,text",
            "data:image/png;base64,@@@",
            "data:image/png,",
            "https://example.com/image.png",
        ] {
            assert!(decode(source, MAX_BYTES).is_err());
        }
        assert!(decode(
            &format!("data:image/png;{},AA", "a".repeat(1024)),
            MAX_BYTES
        )
        .is_err());
    }
}
