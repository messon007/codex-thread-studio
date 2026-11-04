//! Worker-side, bounded image decoding. The native UI receives pixels, not codecs.
use axum::{
    body::{Body, Bytes},
    extract::{rejection::JsonRejection, State},
    http::{HeaderMap, Response, StatusCode},
    Json,
};
use image::{ImageDecoder, ImageReader};
use resvg::{tiny_skia, usvg};
use serde::Deserialize;
use sha2::Digest;
use std::{
    collections::VecDeque,
    io::{Cursor, Read},
    sync::{Arc, Mutex, OnceLock},
};
#[path = "native_image_animation.rs"]
mod animation;

const MAX_FILE: u64 = 25 * 1024 * 1024;
const MAX_SOURCE_PIXELS: u64 = 16 * 1024 * 1024;
const EDGE: u32 = 2048;
const CACHE_BYTES: usize = 32 * 1024 * 1024;
const MAGIC: &[u8; 8] = b"STIMG001";
type Error = (StatusCode, String);
fn bad(error: impl ToString) -> Error {
    (StatusCode::BAD_REQUEST, error.to_string())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub root: String,
    pub path: String,
    #[serde(default)]
    pub animation: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OriginalRequest {
    pub root: String,
    pub path: String,
    pub sha256: String,
}
#[derive(Debug)]
struct Preview {
    pixels: Arc<[u8]>,
    sha256: String,
    mime: &'static str,
    limited: bool,
}
#[derive(Clone)]
struct Cached {
    pixels: Arc<[u8]>,
    limited: bool,
}
#[derive(Default)]
struct Cache {
    entries: VecDeque<(String, Cached)>,
    weight: usize,
}
#[derive(Clone)]
pub struct Images {
    cache: Arc<Mutex<Cache>>,
    workers: Arc<tokio::sync::Semaphore>,
}
impl Default for Images {
    fn default() -> Self {
        Self {
            cache: Arc::default(),
            workers: Arc::new(tokio::sync::Semaphore::new(2)),
        }
    }
}
impl Images {
    pub(crate) fn admit(&self) -> Result<tokio::sync::OwnedSemaphorePermit, Error> {
        self.workers.clone().try_acquire_owned().map_err(|_| {
            (
                StatusCode::TOO_MANY_REQUESTS,
                "Image workers are busy; try again".into(),
            )
        })
    }
    pub(crate) fn document_response(
        &self,
        bytes: Arc<[u8]>,
        original: bool,
        animation: bool,
    ) -> Result<Response<Body>, Error> {
        if bytes.is_empty() || bytes.len() > 10 * 1024 * 1024 {
            return Err(bad("Invalid embedded image size"));
        }
        let mime = inline_mime(&bytes)?;
        let frame = self.decoded_mode(&bytes, mime, animation)?;
        let body = if original {
            let mut body = Vec::with_capacity(12 + bytes.len() + frame.pixels.len());
            body.extend_from_slice(b"STIMS001");
            body.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
            body.extend_from_slice(&bytes);
            body.extend_from_slice(&frame.pixels);
            Body::from(body)
        } else {
            Body::from(Bytes::from_owner(frame.pixels))
        };
        Response::builder()
            .status(StatusCode::OK)
            .header("content-type", "application/x-studio-rgba")
            .header("cache-control", "no-store")
            .header("x-content-type-options", "nosniff")
            .header("x-studio-source-sha256", frame.sha256)
            .header("x-studio-source-mime", mime)
            .header(
                "x-studio-animation-limited",
                if frame.limited { "1" } else { "0" },
            )
            .body(body)
            .map_err(bad)
    }
    fn read(
        request: Request,
        shared: &[std::path::PathBuf],
    ) -> Result<(Vec<u8>, &'static str), Error> {
        if request.root.len() > 4096 || request.path.len() > 8192 {
            return Err(bad("Image path exceeds its budget"));
        }
        let (_, path, _, _) = crate::resolve_review_path_with_shared(
            &crate::ReviewFileRequest {
                root: request.root,
                path: request.path,
            },
            MAX_FILE,
            "25 MiB image",
            shared,
        )?;
        let mut bytes = Vec::new();
        std::fs::File::open(&path)
            .map_err(bad)?
            .take(MAX_FILE + 1)
            .read_to_end(&mut bytes)
            .map_err(bad)?;
        if bytes.len() as u64 > MAX_FILE {
            return Err((
                StatusCode::PAYLOAD_TOO_LARGE,
                "Image grew beyond 25 MiB while opening".into(),
            ));
        }
        let mime = crate::review_image_mime(&path, &bytes).ok_or_else(|| {
            (
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                "Choose a PNG, JPEG, GIF, WebP or SVG image".into(),
            )
        })?;
        if mime == "image/svg+xml" && bytes.len() > 2 * 1024 * 1024 {
            return Err((
                StatusCode::PAYLOAD_TOO_LARGE,
                "SVG source exceeds 2 MiB".into(),
            ));
        }
        Ok((bytes, mime))
    }
    fn load(&self, request: Request, shared: &[std::path::PathBuf]) -> Result<Preview, Error> {
        let animation = request.animation;
        let (bytes, mime) = Self::read(request, shared)?;
        self.decoded_mode(&bytes, mime, animation)
    }
    fn original(request: OriginalRequest, shared: &[std::path::PathBuf]) -> Result<Vec<u8>, Error> {
        if request.sha256.len() != 64 || !request.sha256.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(bad("Invalid original image identity"));
        }
        let (bytes, _) = Self::read(
            Request {
                root: request.root,
                path: request.path,
                animation: false,
            },
            shared,
        )?;
        let hash = sha2::Sha256::digest(&bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        if !hash.eq_ignore_ascii_case(&request.sha256) {
            return Err((
                StatusCode::CONFLICT,
                "Image changed since it was opened. Reload the image before saving its original."
                    .into(),
            ));
        }
        Ok(bytes)
    }
    #[cfg(test)]
    fn decoded(&self, bytes: &[u8], mime: &'static str) -> Result<Preview, Error> {
        self.decoded_mode(bytes, mime, false)
    }
    fn decoded_mode(
        &self,
        bytes: &[u8],
        mime: &'static str,
        animation: bool,
    ) -> Result<Preview, Error> {
        let key = sha2::Sha256::digest(bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        let cache_key = if animation {
            format!("animation:{key}")
        } else {
            key.clone()
        };
        {
            let mut cache = self.cache.lock().map_err(bad)?;
            if let Some(position) = cache
                .entries
                .iter()
                .position(|(identity, _)| identity == &cache_key)
            {
                let entry = cache.entries.remove(position).unwrap();
                let cached = entry.1.clone();
                cache.entries.push_back(entry);
                return Ok(Preview {
                    pixels: cached.pixels,
                    sha256: key,
                    mime,
                    limited: cached.limited,
                });
            }
        }
        let (animated, limited) = if animation {
            match animation::decode(bytes, mime) {
                Ok(timeline) => (timeline, false),
                Err(_) => (None, true),
            }
        } else {
            (None, false)
        };
        let frame: Arc<[u8]> = animated
            .map_or_else(|| decode(bytes, mime), Ok)
            .map_err(bad)?
            .into();
        let mut cache = self.cache.lock().map_err(bad)?;
        // Concurrent identical reads may have decoded the same content. Retain
        // one cache copy; releasing the duplicate happens on this worker.
        if let Some((_, cached)) = cache
            .entries
            .iter()
            .find(|(identity, _)| identity == &cache_key)
        {
            return Ok(Preview {
                pixels: cached.pixels.clone(),
                sha256: key,
                mime,
                limited: cached.limited,
            });
        }
        if limited {
            return Ok(Preview {
                pixels: frame,
                sha256: key,
                mime,
                limited,
            });
        }
        while cache.entries.len() >= 8 || cache.weight + frame.len() > CACHE_BYTES {
            let Some((_, old)) = cache.entries.pop_front() else {
                break;
            };
            cache.weight = cache.weight.saturating_sub(old.pixels.len());
            drop(cache);
            drop(old);
            cache = self.cache.lock().map_err(bad)?;
        }
        cache.weight += frame.len();
        cache.entries.push_back((
            cache_key,
            Cached {
                pixels: frame.clone(),
                limited,
            },
        ));
        Ok(Preview {
            pixels: frame,
            sha256: key,
            mime,
            limited,
        })
    }
}
fn dimensions(width: u32, height: u32) -> Result<(), String> {
    if width == 0
        || height == 0
        || width > 32768
        || height > 32768
        || u64::from(width) * u64::from(height) > MAX_SOURCE_PIXELS
    {
        Err("Image exceeds 32,768 pixels per side or 16 megapixels".into())
    } else {
        Ok(())
    }
}
fn frame(original: (u32, u32), width: u32, height: u32, pixels: Vec<u8>) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(24 + pixels.len());
    bytes.extend_from_slice(MAGIC);
    for value in [original.0, original.1, width, height] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes.extend_from_slice(&pixels);
    bytes
}
fn decode(bytes: &[u8], mime: &str) -> Result<Vec<u8>, String> {
    if mime == "image/svg+xml" {
        return svg(bytes);
    }
    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|error| error.to_string())?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(32768);
    limits.max_image_height = Some(32768);
    limits.max_alloc = Some(128 * 1024 * 1024);
    reader.limits(limits);
    let mut decoder = reader.into_decoder().map_err(|error| error.to_string())?;
    let (width, height) = decoder.dimensions();
    dimensions(width, height)?;
    let orientation = decoder.orientation().map_err(|error| error.to_string())?;
    let mut decoded =
        image::DynamicImage::from_decoder(decoder).map_err(|error| error.to_string())?;
    decoded.apply_orientation(orientation);
    let original = (decoded.width(), decoded.height());
    let preview = if original.0 > EDGE || original.1 > EDGE {
        decoded.resize(EDGE, EDGE, image::imageops::FilterType::Triangle)
    } else {
        decoded
    };
    let (width, height) = (preview.width(), preview.height());
    let mut pixels = preview.into_rgba8().into_raw();
    // A single premultiplied RGBA wire format also covers resvg/tiny-skia.
    for pixel in pixels.chunks_exact_mut(4) {
        let alpha = u16::from(pixel[3]);
        for channel in &mut pixel[..3] {
            *channel = ((u16::from(*channel) * alpha + 127) / 255) as u8;
        }
    }
    Ok(frame(original, width, height, pixels))
}
fn svg(bytes: &[u8]) -> Result<Vec<u8>, String> {
    if bytes.len() > 2 * 1024 * 1024 || bytes.iter().filter(|byte| **byte == b'<').count() > 16384 {
        return Err("SVG exceeds its source/element budget".into());
    }
    static FONTS: OnceLock<Arc<usvg::fontdb::Database>> = OnceLock::new();
    let fonts = FONTS
        .get_or_init(|| {
            let mut fonts = usvg::fontdb::Database::new();
            fonts.load_system_fonts();
            fonts.set_sans_serif_family("DejaVu Sans");
            Arc::new(fonts)
        })
        .clone();
    let resources = studio_svg_resources::Resources::default();
    let options = usvg::Options {
        fontdb: fonts,
        font_family: "sans-serif".into(),
        image_href_resolver: resources.resolver(),
        ..Default::default()
    };
    let tree = usvg::Tree::from_data(bytes, &options).map_err(|error| format!("SVG: {error}"))?;
    resources.check()?;
    let size = tree.size();
    let original = (size.width().ceil() as u32, size.height().ceil() as u32);
    // Vector documents may have large intrinsic dimensions; only the output
    // raster is allocated, and is always bounded independently of the SVG size.
    if original.0 == 0 || original.1 == 0 || original.0 > 1_000_000 || original.1 > 1_000_000 {
        return Err("Invalid SVG dimensions".into());
    }
    let scale = (EDGE as f32 / size.width()).min(EDGE as f32 / size.height());
    let width = ((size.width() * scale).ceil().max(1.0) as u32).min(EDGE);
    let height = ((size.height() * scale).ceil().max(1.0) as u32).min(EDGE);
    let mut pixels =
        tiny_skia::Pixmap::new(width, height).ok_or("Cannot allocate image preview")?;
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(scale, scale),
        &mut pixels.as_mut(),
    );
    Ok(frame(original, width, height, pixels.take()))
}
pub async fn handle(
    State(state): State<crate::GatewayState>,
    body: Result<Json<Request>, JsonRejection>,
) -> Response<Body> {
    let request = match body {
        Ok(Json(request)) => request,
        Err(_) => {
            return crate::json_error(StatusCode::BAD_REQUEST, "Invalid native image request")
        }
    };
    let permit = match state.images.workers.clone().try_acquire_owned() {
        Ok(permit) => permit,
        Err(_) => {
            return crate::json_error(
                StatusCode::TOO_MANY_REQUESTS,
                "Image workers are busy; try again",
            )
        }
    };
    match tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let shared = crate::shared_document_directories(&state)?;
        state.images.load(request, &shared)
    })
    .await
    {
        Ok(Ok(frame)) => Response::builder()
            .status(StatusCode::OK)
            .header("content-type", "application/x-studio-rgba")
            .header("cache-control", "no-store")
            .header("x-content-type-options", "nosniff")
            .header("x-studio-source-sha256", frame.sha256)
            .header("x-studio-source-mime", frame.mime)
            .header(
                "x-studio-animation-limited",
                if frame.limited { "1" } else { "0" },
            )
            .body(Body::from(Bytes::from_owner(frame.pixels)))
            .expect("valid image response"),
        Ok(Err((status, error))) => crate::json_error(status, &error),
        Err(_) => crate::json_error(StatusCode::INTERNAL_SERVER_ERROR, "Image worker stopped"),
    }
}

fn inline_mime(bytes: &[u8]) -> Result<&'static str, Error> {
    match image::guess_format(bytes) {
        Ok(image::ImageFormat::Png) => Ok("image/png"),
        Ok(image::ImageFormat::Jpeg) => Ok("image/jpeg"),
        Ok(image::ImageFormat::Gif) => Ok("image/gif"),
        Ok(image::ImageFormat::WebP) => Ok("image/webp"),
        _ if crate::review_image_mime(std::path::Path::new("inline.svg"), bytes)
            == Some("image/svg+xml") =>
        {
            Ok("image/svg+xml")
        }
        _ => Err(bad(
            "Only inline PNG, JPEG, GIF, WebP and SVG images are supported",
        )),
    }
}
/// Raw inline data from native workers; no paths or remote URLs. SVG uses the
/// same bounded rasterizer and embedded-only resource resolver as local SVG files.
pub async fn inline(
    State(state): State<crate::GatewayState>,
    headers: HeaderMap,
    body: Result<Bytes, axum::extract::rejection::BytesRejection>,
) -> Response<Body> {
    let animation = match headers.get("x-studio-animation") {
        None => false,
        Some(value) if value == "1" => true,
        _ => return crate::json_error(StatusCode::BAD_REQUEST, "Invalid animation preview option"),
    };
    let bytes = match body {
        Ok(b) if !b.is_empty() && b.len() <= 10 * 1024 * 1024 => b,
        _ => {
            return crate::json_error(
                StatusCode::PAYLOAD_TOO_LARGE,
                "Inline images must contain 1 byte–10 MiB",
            )
        }
    };
    let permit = match state.images.workers.clone().try_acquire_owned() {
        Ok(p) => p,
        Err(_) => {
            return crate::json_error(
                StatusCode::TOO_MANY_REQUESTS,
                "Image workers are busy; try again",
            )
        }
    };
    let result = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let mime = inline_mime(&bytes)?;
        state.images.decoded_mode(&bytes, mime, animation)
    })
    .await;
    match result {
        Ok(Ok(frame)) => Response::builder()
            .status(StatusCode::OK)
            .header("content-type", "application/x-studio-rgba")
            .header("cache-control", "no-store")
            .header("x-content-type-options", "nosniff")
            .header("x-studio-source-sha256", frame.sha256)
            .header("x-studio-source-mime", frame.mime)
            .header(
                "x-studio-animation-limited",
                if frame.limited { "1" } else { "0" },
            )
            .body(Body::from(Bytes::from_owner(frame.pixels)))
            .expect("image response"),
        Ok(Err((status, error))) => crate::json_error(status, &error),
        Err(_) => crate::json_error(StatusCode::INTERNAL_SERVER_ERROR, "Image worker stopped"),
    }
}

/// Revalidate access/content without decoding or substituting the scaled preview.
pub async fn original(
    State(state): State<crate::GatewayState>,
    body: Result<Json<OriginalRequest>, JsonRejection>,
) -> Response<Body> {
    let request = match body {
        Ok(Json(request)) => request,
        Err(_) => {
            return crate::json_error(StatusCode::BAD_REQUEST, "Invalid original image request")
        }
    };
    let permit = match state.images.workers.clone().try_acquire_owned() {
        Ok(permit) => permit,
        Err(_) => {
            return crate::json_error(
                StatusCode::TOO_MANY_REQUESTS,
                "Image workers are busy; try again",
            )
        }
    };
    match tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let shared = crate::shared_document_directories(&state)?;
        Images::original(request, &shared)
    })
    .await
    {
        Ok(Ok(bytes)) => Response::builder()
            .status(StatusCode::OK)
            .header("content-type", "application/octet-stream")
            .header("content-disposition", "attachment")
            .header("cache-control", "no-store")
            .header("x-content-type-options", "nosniff")
            .body(Body::from(bytes))
            .expect("original image response"),
        Ok(Err((status, error))) => crate::json_error(status, &error),
        Err(_) => crate::json_error(StatusCode::INTERNAL_SERVER_ERROR, "Image worker stopped"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn animation_opt_in_has_a_separate_cache_entry_and_the_same_original_identity() {
        let bytes = include_bytes!("../tests/fixtures/native-images/two-frame.png");
        let images = Images::default();
        let still = images.decoded(bytes, "image/png").unwrap();
        let animation = images.decoded_mode(bytes, "image/png", true).unwrap();
        assert_eq!(&still.pixels[..8], b"STIMG001");
        assert_eq!(&animation.pixels[..8], b"STIMA001");
        assert_eq!(still.sha256, animation.sha256);
        assert!(Arc::ptr_eq(
            &still.pixels,
            &images.decoded(bytes, "image/png").unwrap().pixels
        ));
        assert!(Arc::ptr_eq(
            &animation.pixels,
            &images
                .decoded_mode(bytes, "image/png", true)
                .unwrap()
                .pixels
        ));
        let legacy: Request =
            serde_json::from_value(serde_json::json!({"root":"/private","path":"image.png"}))
                .unwrap();
        assert!(!legacy.animation);
        assert_eq!(images.cache.lock().unwrap().entries.len(), 2);
    }
    #[test]
    fn excessive_animation_keeps_a_still_preview_without_caching_failure() {
        let mut bytes = Vec::new();
        {
            let mut encoder = image::codecs::gif::GifEncoder::new(&mut bytes);
            for _ in 0..129 {
                encoder
                    .encode_frame(image::Frame::new(image::RgbaImage::from_pixel(
                        2,
                        1,
                        image::Rgba([255, 0, 0, 255]),
                    )))
                    .unwrap();
            }
        }
        let images = Images::default();
        let still = images.decoded(&bytes, "image/gif").unwrap();
        let limited = images.decoded_mode(&bytes, "image/gif", true).unwrap();
        assert!(limited.limited);
        assert_eq!(limited.pixels, still.pixels);
        assert_eq!(limited.sha256, still.sha256);
        assert_eq!(images.cache.lock().unwrap().entries.len(), 1);
        assert!(images.decoded_mode(b"GIF89a", "image/gif", true).is_err());
    }
    fn u32_at(bytes: &[u8], offset: usize) -> u32 {
        u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
    }
    #[test]
    fn raster_preview_premultiplies_and_preserves_original_dimensions() {
        let image = image::RgbaImage::from_pixel(2100, 2, image::Rgba([200, 80, 40, 128]));
        let mut bytes = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(image)
            .write_to(&mut bytes, image::ImageFormat::Png)
            .unwrap();
        let pixels = decode(bytes.get_ref(), "image/png").unwrap();
        assert_eq!(&pixels[..8], MAGIC);
        assert_eq!(u32_at(&pixels, 8), 2100);
        assert_eq!(u32_at(&pixels, 16), EDGE);
        assert_eq!(&pixels[24..28], &[100, 40, 20, 128]);
        assert_eq!(
            pixels.len(),
            24 + (u32_at(&pixels, 16) * u32_at(&pixels, 20) * 4) as usize
        );
        assert!(dimensions(32768, 32768).is_err());
        assert!(decode(b"not PNG", "image/png").is_err());
    }
    #[test]
    fn svg_is_bounded_and_cannot_resolve_private_images() {
        let bytes=br##"<svg xmlns="http://www.w3.org/2000/svg" width="100000" height="50000"><rect width="100000" height="50000" fill="#00ff00"/><image href="file:///etc/passwd" width="20" height="20"/></svg>"##;
        let pixels = svg(bytes).unwrap();
        assert_eq!(u32_at(&pixels, 16), 2048);
        assert_eq!(u32_at(&pixels, 20), 1024);
        assert_eq!(&pixels[24..28], &[0, 255, 0, 255]);
        assert!(svg(&vec![b'<'; 16385]).is_err());
    }
    #[test]
    fn svg_bitmap_preview_keeps_the_original_source_identity() {
        let project = tempfile::tempdir().unwrap();
        let path = project.path().join("bitmap.svg");
        let bytes = br#"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><image width="20" height="10" href="data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAIAAAABCAYAAAD0In+KAAAADklEQVR4nGO4o8H1H4QBEjMEGyP2UiEAAAAASUVORK5CYII="/></svg>"#;
        std::fs::write(&path, bytes).unwrap();
        let service = Images::default();
        let frame = service
            .load(
                Request {
                    root: project.path().to_string_lossy().into(),
                    path: "bitmap.svg".into(),
                    animation: false,
                },
                &[],
            )
            .unwrap();
        let width = u32_at(&frame.pixels, 16);
        let height = u32_at(&frame.pixels, 20);
        let center = 24 + ((height / 2 * width + width / 2) * 4) as usize;
        assert_eq!(&frame.pixels[center..center + 4], &[220, 40, 10, 255]);
        assert_eq!(frame.mime, "image/svg+xml");
        assert_eq!(
            Images::original(
                OriginalRequest {
                    root: project.path().to_string_lossy().into(),
                    path: "bitmap.svg".into(),
                    sha256: frame.sha256
                },
                &[]
            )
            .unwrap(),
            bytes
        );
    }
    #[test]
    fn supported_raster_formats_and_jpeg_orientation_are_decoded_in_rust() {
        let input = image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
            2,
            3,
            image::Rgb([220, 40, 10]),
        ));
        for format in [
            image::ImageFormat::Jpeg,
            image::ImageFormat::WebP,
            image::ImageFormat::Gif,
        ] {
            let mut bytes = Cursor::new(Vec::new());
            input.write_to(&mut bytes, format).unwrap();
            let pixels = decode(bytes.get_ref(), "image/raster").unwrap();
            assert_eq!(u32_at(&pixels, 8), 2);
            assert_eq!(u32_at(&pixels, 12), 3);
            if format == image::ImageFormat::Jpeg {
                // Little-endian EXIF orientation 6, without relying on a test
                // image from outside the repository or an external converter.
                let exif =
                    b"Exif\0\0II\x2a\0\x08\0\0\0\x01\0\x12\x01\x03\0\x01\0\0\0\x06\0\0\0\0\0\0\0";
                let mut rotated = vec![0xff, 0xd8, 0xff, 0xe1];
                rotated.extend_from_slice(&((exif.len() + 2) as u16).to_be_bytes());
                rotated.extend_from_slice(exif);
                rotated.extend_from_slice(&bytes.get_ref()[2..]);
                let pixels = decode(&rotated, "image/jpeg").unwrap();
                assert_eq!(u32_at(&pixels, 8), 3);
                assert_eq!(u32_at(&pixels, 12), 2);
            }
        }
    }
    #[test]
    fn cache_reuses_content_but_revalidates_changed_files_and_access() {
        let project = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let path = project.path().join("image.png");
        let write = |color| {
            let image = image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
                2,
                1,
                image::Rgba(color),
            ));
            let mut bytes = Cursor::new(Vec::new());
            image.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
            std::fs::write(&path, bytes.get_ref()).unwrap();
        };
        let service = Images::default();
        let request = || Request {
            root: project.path().to_string_lossy().into(),
            path: "image.png".into(),
            animation: false,
        };
        write([255, 0, 0, 255]);
        let first = service.load(request(), &[]).unwrap();
        let same = service.load(request(), &[]).unwrap();
        assert!(Arc::ptr_eq(&first.pixels, &same.pixels));
        let original = |hash: &str| OriginalRequest {
            root: project.path().to_string_lossy().into(),
            path: "image.png".into(),
            sha256: hash.into(),
        };
        assert_eq!(
            Images::original(original(&first.sha256), &[]).unwrap(),
            std::fs::read(&path).unwrap()
        );
        write([0, 0, 255, 255]);
        let changed = service.load(request(), &[]).unwrap();
        assert!(!Arc::ptr_eq(&first.pixels, &changed.pixels));
        assert_ne!(first.sha256, changed.sha256);
        assert_eq!(&changed.pixels[24..28], &[0, 0, 255, 255]);
        assert_eq!(
            Images::original(original(&first.sha256), &[])
                .unwrap_err()
                .0,
            StatusCode::CONFLICT
        );
        assert_eq!(
            Images::original(original(&changed.sha256.to_uppercase()), &[]).unwrap(),
            std::fs::read(&path).unwrap()
        );
        assert_eq!(
            Images::original(original("invalid"), &[]).unwrap_err().0,
            StatusCode::BAD_REQUEST
        );
        #[cfg(unix)]
        {
            let forbidden = outside.path().join("image.png");
            std::fs::copy(&path, &forbidden).unwrap();
            std::fs::remove_file(&path).unwrap();
            std::os::unix::fs::symlink(&forbidden, &path).unwrap();
            assert_eq!(
                service.load(request(), &[]).unwrap_err().0,
                StatusCode::FORBIDDEN
            );
            assert_eq!(
                Images::original(original(&changed.sha256), &[])
                    .unwrap_err()
                    .0,
                StatusCode::FORBIDDEN
            );
        }
    }
}
