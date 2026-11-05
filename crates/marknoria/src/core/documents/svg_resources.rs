// Adapted from Codex Thread Studio; Copyright (c) 2026 messon007.
// SPDX-License-Identifier: MIT. See third-party/codex-thread-studio-MIT.txt.
//! Embedded raster resources only: callers retain SVG parsing/output bounds.
//! Parsing and rendering must run on a bounded worker, never on the GTK thread.
use image::{ImageDecoder, ImageEncoder, ImageReader};
use resvg::usvg;
use std::{
    collections::HashMap,
    io::{self, Cursor, Write},
    sync::{Arc, Mutex},
};

const MAX_IMAGES: usize = 64;
const MAX_PIXELS: u64 = 16 * 1024 * 1024;
const MAX_BYTES: usize = 8 * 1024 * 1024;
const MAX_EDGE: u32 = 32768;

struct Raster {
    data: Arc<Vec<u8>>,
    pixels: u64,
}
#[derive(Default)]
struct State {
    images: usize,
    pixels: u64,
    bytes: usize,
    error: Option<String>,
    cache: HashMap<Arc<Vec<u8>>, Raster>,
}

/// Per-parse accounting, shared by every `<image>` and expanded `<use>`.
/// Original bytes are never modified. Only canonical, bounded PNGs reach resvg's
/// raster decoder; arbitrary strings and nested SVG resource trees stay inert.
#[derive(Default)]
pub struct Resources {
    state: Mutex<State>,
}
impl Resources {
    pub fn resolver(&self) -> usvg::ImageHrefResolver<'_> {
        usvg::ImageHrefResolver {
            resolve_data: Box::new(move |_, data, _| self.resolve(data)),
            resolve_string: Box::new(|_, _| None),
        }
    }
    /// Call after parsing, before rendering. A budget breach rejects the complete
    /// image instead of presenting a silently incomplete resource-limited frame.
    pub fn check(&self) -> Result<(), String> {
        let state = self
            .state
            .lock()
            .map_err(|_| "SVG resource worker stopped")?;
        match &state.error {
            Some(error) => Err(error.clone()),
            None => Ok(()),
        }
    }
    fn resolve(&self, data: Arc<Vec<u8>>) -> Option<usvg::ImageKind> {
        let mut state = self.state.lock().ok()?;
        if state.error.is_some() {
            return None;
        }
        state.images += 1;
        if state.images > MAX_IMAGES {
            state.error = Some("SVG exceeds 64 embedded image resources".into());
            return None;
        }
        if let Some(raster) = state.cache.get(&data) {
            let (pixels, encoded) = (raster.pixels, raster.data.clone());
            charge_pixels(&mut state, pixels)?;
            return Some(usvg::ImageKind::PNG(encoded));
        }
        let format = image::guess_format(&data).ok()?;
        if !matches!(
            format,
            image::ImageFormat::Png
                | image::ImageFormat::Jpeg
                | image::ImageFormat::Gif
                | image::ImageFormat::WebP
        ) {
            return None;
        }
        let mut reader = ImageReader::with_format(Cursor::new(data.as_slice()), format);
        let mut limits = image::Limits::default();
        limits.max_image_width = Some(MAX_EDGE);
        limits.max_image_height = Some(MAX_EDGE);
        limits.max_alloc = Some(128 * 1024 * 1024);
        reader.limits(limits);
        let mut decoder = match reader.into_decoder() {
            Ok(decoder) => decoder,
            Err(image::ImageError::Limits(_)) => {
                state.error =
                    Some("SVG bitmap exceeds its decoder allocation/dimension limits".into());
                return None;
            }
            Err(_) => return None,
        };
        let (width, height) = decoder.dimensions();
        if width == 0 || height == 0 || width > MAX_EDGE || height > MAX_EDGE {
            state.error = Some("SVG bitmap exceeds 32,768 pixels per side".into());
            return None;
        }
        let pixels = u64::from(width) * u64::from(height);
        charge_pixels(&mut state, pixels)?;
        let orientation = decoder.orientation().ok()?;
        let mut decoded = match image::DynamicImage::from_decoder(decoder) {
            Ok(decoded) => decoded,
            Err(image::ImageError::Limits(_)) => {
                state.error =
                    Some("SVG bitmap exceeds its decoder allocation/dimension limits".into());
                return None;
            }
            Err(_) => return None,
        };
        decoded.apply_orientation(orientation);
        let rgba = decoded.into_rgba8();
        // Decode before canonicalization: the original compressed format never
        // reaches an unbounded second decoder or carries metadata into resvg.
        let mut encoded = LimitedWriter {
            bytes: Vec::new(),
            limit: MAX_BYTES - state.bytes,
        };
        if image::codecs::png::PngEncoder::new_with_quality(
            &mut encoded,
            image::codecs::png::CompressionType::Fast,
            image::codecs::png::FilterType::Adaptive,
        )
        .write_image(
            &rgba,
            rgba.width(),
            rgba.height(),
            image::ExtendedColorType::Rgba8,
        )
        .is_err()
        {
            state.error = Some("SVG normalized bitmap resources exceed 8 MiB".into());
            return None;
        }
        state.bytes += encoded.bytes.len();
        let encoded = Arc::new(encoded.bytes);
        state.cache.insert(
            data,
            Raster {
                data: encoded.clone(),
                pixels,
            },
        );
        Some(usvg::ImageKind::PNG(encoded))
    }
}

fn charge_pixels(state: &mut State, pixels: u64) -> Option<()> {
    if pixels > MAX_PIXELS.saturating_sub(state.pixels) {
        state.error = Some("SVG embedded bitmaps exceed 16 megapixels in total".into());
        return None;
    }
    state.pixels += pixels;
    Some(())
}

struct LimitedWriter {
    bytes: Vec<u8>,
    limit: usize,
}
impl Write for LimitedWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > self.limit.saturating_sub(self.bytes.len()) {
            return Err(io::Error::other("SVG bitmap byte budget exceeded"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::{engine::general_purpose::STANDARD, Engine};

    fn raster(width: u32, height: u32, format: image::ImageFormat) -> Vec<u8> {
        let input = image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
            width,
            height,
            image::Rgba([220, 40, 10, 255]),
        ));
        let input = if format == image::ImageFormat::Jpeg {
            image::DynamicImage::ImageRgb8(input.to_rgb8())
        } else {
            input
        };
        let mut out = Cursor::new(Vec::new());
        input.write_to(&mut out, format).unwrap();
        out.into_inner()
    }
    fn render(svg: &str) -> Result<Vec<u8>, String> {
        let resources = Resources::default();
        let options = usvg::Options {
            image_href_resolver: resources.resolver(),
            ..Default::default()
        };
        let tree = usvg::Tree::from_str(svg, &options).unwrap();
        resources.check()?;
        let mut pixels = resvg::tiny_skia::Pixmap::new(4, 2).unwrap();
        resvg::render(
            &tree,
            resvg::tiny_skia::Transform::identity(),
            &mut pixels.as_mut(),
        );
        Ok(pixels.take())
    }
    #[test]
    fn embedded_formats_render_with_svg_geometry_and_alpha() {
        for format in [
            image::ImageFormat::Png,
            image::ImageFormat::Jpeg,
            image::ImageFormat::Gif,
            image::ImageFormat::WebP,
        ] {
            let bytes = raster(2, 1, format);
            let svg = format!(
                r#"<svg xmlns="http://www.w3.org/2000/svg" width="4" height="2"><image x="1" y="1" width="2" height="1" href="data:application/octet-stream;base64,{}"/></svg>"#,
                STANDARD.encode(bytes)
            );
            let pixels = render(&svg).unwrap();
            assert_eq!(&pixels[..4], &[0, 0, 0, 0]);
            let pixel = &pixels[(4 + 1) * 4..(4 + 2) * 4];
            assert!(
                pixel[0] > 180 && pixel[1] < 70 && pixel[2] < 40 && pixel[3] == 255,
                "{format:?}: {pixel:?}"
            );
        }
        let input = image::RgbaImage::from_pixel(2, 1, image::Rgba([200, 80, 40, 128]));
        let mut bytes = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(input)
            .write_to(&mut bytes, image::ImageFormat::Png)
            .unwrap();
        let svg = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="4" height="2"><image width="2" height="1" href="data:image/png;base64,{}"/></svg>"#,
            STANDARD.encode(bytes.into_inner())
        );
        assert_eq!(&render(&svg).unwrap()[..4], &[100, 40, 20, 128]);
    }
    #[test]
    fn orientation_is_applied_before_exposing_intrinsic_resource_size() {
        let input = raster(2, 3, image::ImageFormat::Jpeg);
        let exif = b"Exif\0\0II\x2a\0\x08\0\0\0\x01\0\x12\x01\x03\0\x01\0\0\0\x06\0\0\0\0\0\0\0";
        let mut rotated = vec![0xff, 0xd8, 0xff, 0xe1];
        rotated.extend_from_slice(&((exif.len() + 2) as u16).to_be_bytes());
        rotated.extend_from_slice(exif);
        rotated.extend_from_slice(&input[2..]);
        let resources = Resources::default();
        let Some(usvg::ImageKind::PNG(data)) = resources.resolve(Arc::new(rotated)) else {
            panic!("JPEG resource missing")
        };
        let image = image::load_from_memory(&data).unwrap();
        assert_eq!((image.width(), image.height()), (3, 2));
        resources.check().unwrap();
    }
    #[test]
    fn repeated_resources_share_normalized_bytes_but_charge_render_work() {
        let resources = Resources::default();
        let input = Arc::new(raster(4096, 2048, image::ImageFormat::Png));
        let Some(usvg::ImageKind::PNG(first)) = resources.resolve(input.clone()) else {
            panic!("First resource missing")
        };
        let Some(usvg::ImageKind::PNG(second)) = resources.resolve(input.clone()) else {
            panic!("Second resource missing")
        };
        assert!(Arc::ptr_eq(&first, &second));
        assert_eq!(resources.state.lock().unwrap().pixels, MAX_PIXELS);
        assert!(resources.resolve(input).is_none());
        assert!(resources.check().unwrap_err().contains("16 megapixels"));
    }
    #[test]
    fn count_and_streaming_byte_budgets_reject_the_whole_image() {
        let resources = Resources::default();
        let input = Arc::new(raster(2, 1, image::ImageFormat::Png));
        for _ in 0..64 {
            assert!(resources.resolve(input.clone()).is_some());
        }
        assert!(resources.resolve(input.clone()).is_none());
        assert!(resources.check().unwrap_err().contains("64 embedded"));
        // Simulate previously admitted PNGs leaving less room than a PNG header.
        let resources = Resources::default();
        resources.state.lock().unwrap().bytes = MAX_BYTES - 8;
        assert!(resources.resolve(input.clone()).is_none());
        assert!(resources.check().unwrap_err().contains("8 MiB"));
        assert_eq!(resources.state.lock().unwrap().bytes, MAX_BYTES - 8);
        let fresh = Resources::default();
        assert!(fresh.resolve(input).is_some());
        fresh.check().unwrap();
        let mut writer = LimitedWriter {
            bytes: Vec::new(),
            limit: 8,
        };
        writer.write_all(b"12345678").unwrap();
        assert!(writer.write_all(b"9").is_err());
        assert_eq!(writer.bytes, b"12345678");
    }
    #[test]
    fn unknown_and_nested_vector_resources_remain_inert() {
        let nested=STANDARD.encode(br#"<svg xmlns="http://www.w3.org/2000/svg" width="4" height="2"><rect width="4" height="2" fill="red"/></svg>"#);
        let svg = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="4" height="2"><rect width="4" height="2" fill="green"/><image width="4" height="2" href="data:image/svg+xml;base64,{nested}"/><image width="4" height="2" href="file:///etc/passwd"/><image width="4" height="2" href="http://127.0.0.1:9/unwanted.png"/><image width="4" height="2" href="data:image/png;base64,aW52YWxpZA=="/></svg>"#
        );
        assert!(render(&svg)
            .unwrap()
            .chunks_exact(4)
            .all(|pixel| pixel == [0, 128, 0, 255]));
    }
}
