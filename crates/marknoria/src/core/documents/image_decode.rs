// Adapted from Codex Thread Studio; Copyright (c) 2026 messon007.
// SPDX-License-Identifier: MIT. See third-party/codex-thread-studio-MIT.txt.
use image::{ImageDecoder, ImageReader};
use resvg::{tiny_skia,usvg};
use std::{io::Cursor,sync::{Arc,OnceLock}};
#[path="image_animation.rs"]
pub(super) mod animation;
const MAX_SOURCE_PIXELS:u64=16*1024*1024;
const EDGE:u32=2048;
const MAGIC:&[u8;8]=b"MNIMG001";
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
pub(super) fn decode(bytes: &[u8], mime: &str) -> Result<Vec<u8>, String> {
    decode_resolution(bytes,mime,false)
}
pub(super) fn decode_resolution(bytes: &[u8], mime: &str, source: bool) -> Result<Vec<u8>, String> {
    if mime == "image/svg+xml" {
        return svg(bytes,source);
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
    let preview = if !source && (original.0 > EDGE || original.1 > EDGE) {
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
fn svg(bytes: &[u8], source:bool) -> Result<Vec<u8>, String> {
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
    let resources = super::svg_resources::Resources::default();
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
    if source { dimensions(original.0,original.1)?; }
    let scale = if source {1.0} else {(EDGE as f32 / size.width()).min(EDGE as f32 / size.height()).min(1.0)};
    let width = (size.width() * scale).ceil().max(1.0) as u32;
    let height = (size.height() * scale).ceil().max(1.0) as u32;
    let (width,height)=if source {(width,height)} else {(width.min(EDGE),height.min(EDGE))};
    let mut pixels =
        tiny_skia::Pixmap::new(width, height).ok_or("Cannot allocate image preview")?;
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(scale, scale),
        &mut pixels.as_mut(),
    );
    Ok(frame(original, width, height, pixels.take()))
}
