// Adapted from Codex Thread Studio; Copyright (c) 2026 messon007.
// SPDX-License-Identifier: MIT. See third-party/codex-thread-studio-MIT.txt.
//! Bounded, worker-only GIF/WebP/APNG decoding into composited RGBA frames.
//! The opt-in wire format leaves the existing still-image protocol unchanged.
use image::{AnimationDecoder, ImageDecoder};
use std::{
    io::Cursor,
    time::{Duration, Instant},
};

const EDGE: u32 = 768;
const FRAME_LIMIT: usize = 128;
const PIXEL_BYTES: usize = 16 * 1024 * 1024;
const MAGIC: &[u8; 8] = b"MNIMA001";

fn limits() -> image::Limits {
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(32768);
    limits.max_image_height = Some(32768);
    limits.max_alloc = Some(128 * 1024 * 1024);
    limits
}
fn current(deadline: Instant) -> Result<(), String> {
    if Instant::now() >= deadline {
        Err("Animation preview exceeded its time budget".into())
    } else {
        Ok(())
    }
}
fn loops(count: image::metadata::LoopCount) -> u32 {
    match count {
        image::metadata::LoopCount::Infinite => 0,
        image::metadata::LoopCount::Finite(n) => n.get(),
    }
}
pub(crate) fn decode(bytes: &[u8], mime: &str) -> Result<Option<Vec<u8>>, String> {
    decode_resolution(bytes,mime,false)
}
pub(crate) fn decode_resolution(bytes: &[u8], mime: &str, source: bool) -> Result<Option<Vec<u8>>, String> {
    if bytes.is_empty() || bytes.len() > 25 * 1024 * 1024 {
        return Err("Animation source exceeds 25 MiB".into());
    }
    let deadline = Instant::now() + Duration::from_secs(3);
    match mime {
        "image/gif" => {
            // GIF stores repetitions after the first play. Read this metadata
            // with the codec itself, without confusing no loop extension with
            // indefinite playback or scanning compressed pixels for byte patterns.
            let mut meta = gif::DecodeOptions::new()
                .read_info(Cursor::new(bytes))
                .map_err(|e| e.to_string())?;
            meta.next_frame_info().map_err(|e| e.to_string())?;
            let plays = match meta.repeat() {
                gif::Repeat::Infinite => 0,
                gif::Repeat::Finite(n) => u32::from(n) + 1,
            };
            let mut decoder = image::codecs::gif::GifDecoder::new(Cursor::new(bytes))
                .map_err(|e| e.to_string())?;
            decoder.set_limits(limits()).map_err(|e| e.to_string())?;
            let original = decoder.dimensions();
            let orientation = decoder.orientation().map_err(|e| e.to_string())?;
            collect(
                decoder.into_frames(),
                original,
                plays,
                deadline,
                orientation,
                source,
            )
        }
        "image/webp" => {
            let mut decoder = image::codecs::webp::WebPDecoder::new(Cursor::new(bytes))
                .map_err(|e| e.to_string())?;
            decoder.set_limits(limits()).map_err(|e| e.to_string())?;
            if !decoder.has_animation() {
                return Ok(None);
            }
            let original = decoder.dimensions();
            let orientation = decoder.orientation().map_err(|e| e.to_string())?;
            let plays = loops(decoder.loop_count());
            collect(
                decoder.into_frames(),
                original,
                plays,
                deadline,
                orientation,
                source,
            )
        }
        "image/png" => {
            let mut decoder = image::codecs::png::PngDecoder::new(Cursor::new(bytes))
                .map_err(|e| e.to_string())?;
            decoder.set_limits(limits()).map_err(|e| e.to_string())?;
            if !decoder.is_apng().map_err(|e| e.to_string())? {
                return Ok(None);
            }
            let original = decoder.dimensions();
            let orientation = decoder.orientation().map_err(|e| e.to_string())?;
            let decoder = decoder.apng().map_err(|e| e.to_string())?;
            let plays = loops(decoder.loop_count());
            collect(
                decoder.into_frames(),
                original,
                plays,
                deadline,
                orientation,
                source,
            )
        }
        _ => Ok(None),
    }
}
fn collect(
    frames: image::Frames<'_>,
    original: (u32, u32),
    plays: u32,
    deadline: Instant,
    orientation: image::metadata::Orientation,
    source:bool,
) -> Result<Option<Vec<u8>>, String> {
    current(deadline)?;
    super::dimensions(original.0, original.1)?;
    let mut body = Vec::new();
    let mut count = 0usize;
    let mut size = None;
    let mut oriented_original = original;
    for item in frames {
        current(deadline)?;
        if count == FRAME_LIMIT {
            return Err("Animation preview exceeds 128 frames".into());
        }
        let frame = item.map_err(|e| e.to_string())?;
        let (numerator, denominator) = frame.delay().numer_denom_ms();
        let delay = ((u64::from(numerator) + u64::from(denominator.max(1)) - 1)
            / u64::from(denominator.max(1)))
        .clamp(20, u64::from(u32::MAX)) as u32;
        let buffer = frame.into_buffer();
        if buffer.dimensions() != original {
            return Err("Animation decoder returned a partial canvas".into());
        }
        let mut image = image::DynamicImage::ImageRgba8(buffer);
        image.apply_orientation(orientation);
        oriented_original = (image.width(), image.height());
        let preview = if !source && (oriented_original.0 > EDGE || oriented_original.1 > EDGE) {
            image
                .resize(EDGE, EDGE, image::imageops::FilterType::Triangle)
                .into_rgba8()
        } else {
            image.into_rgba8()
        };
        let dimensions = preview.dimensions();
        if size.is_some_and(|s| s != dimensions) {
            return Err("Animation preview changed canvas size".into());
        }
        size = Some(dimensions);
        let mut pixels = preview.into_raw();
        if body.len().saturating_add(pixels.len()).saturating_add(4) > PIXEL_BYTES {
            return Err("Animation preview exceeds 16 MiB".into());
        }
        for pixel in pixels.chunks_exact_mut(4) {
            let alpha = u16::from(pixel[3]);
            for channel in &mut pixel[..3] {
                *channel = ((u16::from(*channel) * alpha + 127) / 255) as u8;
            }
        }
        body.extend_from_slice(&delay.to_le_bytes());
        body.extend_from_slice(&pixels);
        count += 1;
        current(deadline)?;
    }
    if count < 2 {
        return Ok(None);
    }
    let (width, height) = size.unwrap();
    let mut output = Vec::with_capacity(32 + body.len());
    output.extend_from_slice(MAGIC);
    for value in [
        oriented_original.0,
        oriented_original.1,
        width,
        height,
        count as u32,
        plays,
    ] {
        output.extend_from_slice(&value.to_le_bytes());
    }
    output.extend_from_slice(&body);
    Ok(Some(output))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn u32_at(bytes: &[u8], at: usize) -> u32 {
        u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
    }
    fn gif(repeat: Option<image::codecs::gif::Repeat>, count: usize) -> Vec<u8> {
        let mut bytes = Vec::new();
        {
            let mut encoder = image::codecs::gif::GifEncoder::new(&mut bytes);
            if let Some(repeat) = repeat {
                encoder.set_repeat(repeat).unwrap();
            }
            for i in 0..count {
                encoder
                    .encode_frame(image::Frame::from_parts(
                        image::RgbaImage::from_pixel(
                            2,
                            1,
                            image::Rgba(if i % 2 == 0 {
                                [255, 0, 0, 255]
                            } else {
                                [0, 0, 255, 255]
                            }),
                        ),
                        0,
                        0,
                        image::Delay::from_numer_denom_ms(if i % 2 == 0 { 40 } else { 90 }, 1),
                    ))
                    .unwrap();
            }
        }
        bytes
    }
    #[test]
    fn gif_frames_delays_and_repeat_metadata_round_trip() {
        for (repeat, plays) in [
            (None, 1),
            (Some(image::codecs::gif::Repeat::Finite(2)), 3),
            (Some(image::codecs::gif::Repeat::Infinite), 0),
        ] {
            let output = decode(&gif(repeat, 2), "image/gif").unwrap().unwrap();
            assert_eq!(&output[..8], MAGIC);
            assert_eq!((u32_at(&output, 8), u32_at(&output, 12)), (2, 1));
            assert_eq!((u32_at(&output, 24), u32_at(&output, 28)), (2, plays));
            assert_eq!((u32_at(&output, 32), u32_at(&output, 44)), (40, 90));
            assert_eq!(&output[36..40], &[255, 0, 0, 255]);
            assert_eq!(&output[48..52], &[0, 0, 255, 255]);
        }
    }
    #[test]
    fn gif_cropped_frames_and_previous_disposal_are_composited() {
        let mut bytes = Vec::new();
        {
            let mut encoder =
                gif::Encoder::new(&mut bytes, 5, 3, &[255, 0, 0, 0, 0, 255, 0, 255, 0]).unwrap();
            for (width, height, left, top, color, dispose) in [
                (5, 3, 0, 0, 0, gif::DisposalMethod::Keep),
                (1, 1, 2, 1, 1, gif::DisposalMethod::Previous),
                (1, 1, 4, 2, 2, gif::DisposalMethod::Keep),
            ] {
                encoder
                    .write_frame(&gif::Frame {
                        width,
                        height,
                        left,
                        top,
                        dispose,
                        buffer: std::borrow::Cow::Owned(vec![
                            color;
                            width as usize * height as usize
                        ]),
                        ..Default::default()
                    })
                    .unwrap();
            }
        }
        let output = decode(&bytes, "image/gif").unwrap().unwrap();
        assert_eq!(u32_at(&output, 24), 3);
        assert_eq!(&output[128..132], &[0, 0, 255, 255]);
        assert_eq!(&output[192..196], &[255, 0, 0, 255]);
        assert_eq!(&output[220..224], &[0, 255, 0, 255]);
    }
    #[test]
    fn still_images_keep_the_legacy_response_and_corrupt_animation_is_rejected() {
        assert!(decode(&gif(None, 1), "image/gif").unwrap().is_none());
        let mut png = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(image::RgbaImage::new(2, 1))
            .write_to(&mut png, image::ImageFormat::Png)
            .unwrap();
        assert!(decode(png.get_ref(), "image/png").unwrap().is_none());
        assert!(decode(b"GIF89a", "image/gif").is_err());
    }
    #[test]
    fn apng_and_webp_use_composited_pixels_delays_and_finite_plays() {
        let colors=[[200,80,40,128],[0,0,255,128]];
        let mut apng=Vec::new();{
            let mut encoder=png::Encoder::new(&mut apng,5,3);encoder.set_color(png::ColorType::Rgba);encoder.set_depth(png::BitDepth::Eight);encoder.set_animated(2,2).unwrap();let mut writer=encoder.write_header().unwrap();for(i,color)in colors.iter().enumerate(){writer.set_frame_delay(if i==0{4}else{9},100).unwrap();writer.write_image_data(&color.repeat(15)).unwrap();}
        }
        fn chunk(out:&mut Vec<u8>,name:&[u8;4],bytes:&[u8]){out.extend_from_slice(name);out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());out.extend_from_slice(bytes);if bytes.len()%2!=0{out.push(0);}}
        let mut body=Vec::new();chunk(&mut body,b"VP8X",&[0x12,0,0,0,4,0,0,2,0,0]);chunk(&mut body,b"ANIM",&[0,0,0,0,2,0]);for(i,color)in colors.iter().enumerate(){let mut image=Cursor::new(Vec::new());image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(5,3,image::Rgba(*color))).write_to(&mut image,image::ImageFormat::WebP).unwrap();let mut frame=vec![0,0,0,0,0,0,4,0,0,2,0,0,if i==0{40}else{90},0,0,2];frame.extend_from_slice(&image.get_ref()[12..]);chunk(&mut body,b"ANMF",&frame);}let mut webp=b"RIFF".to_vec();webp.extend_from_slice(&((body.len()+4) as u32).to_le_bytes());webp.extend_from_slice(b"WEBP");webp.extend_from_slice(&body);
        for (bytes, mime) in [
            (apng.as_slice(),"image/png"),
            (webp.as_slice(),"image/webp"),
        ] {
            let output = decode(bytes, mime).unwrap().unwrap();
            assert_eq!((u32_at(&output, 8), u32_at(&output, 12)), (5, 3));
            assert_eq!((u32_at(&output, 24), u32_at(&output, 28)), (2, 2));
            assert_eq!((u32_at(&output, 32), u32_at(&output, 96)), (40, 90));
            assert_eq!(&output[36..40], &[100, 40, 20, 128], "{mime}");
            assert_eq!(&output[100..104], &[0, 0, 128, 128], "{mime}");
        }
    }
    #[test]
    fn frame_and_time_limits_never_publish_a_partial_timeline() {
        assert!(decode(&gif(None, FRAME_LIMIT + 1), "image/gif")
            .unwrap_err()
            .contains("128 frames"));
        let frames = image::Frames::new(Box::new(std::iter::once(Ok(image::Frame::new(
            image::RgbaImage::new(1, 1),
        )))));
        assert!(collect(
            frames,
            (1, 1),
            0,
            Instant::now(),
            image::metadata::Orientation::NoTransforms,
            false
        )
        .unwrap_err()
        .contains("time budget"));
    }
    #[test]
    fn orientation_long_delays_and_transparent_pixels_survive_the_transport() {
        let frames = image::Frames::new(Box::new((0..2).map(|_| {
            Ok(image::Frame::from_parts(
                image::RgbaImage::from_pixel(2, 1, image::Rgba([200, 80, 40, 128])),
                0,
                0,
                image::Delay::from_numer_denom_ms(120000, 1),
            ))
        })));
        let output = collect(
            frames,
            (2, 1),
            1,
            Instant::now() + Duration::from_secs(3),
            image::metadata::Orientation::Rotate90,
            false,
        )
        .unwrap()
        .unwrap();
        assert_eq!((u32_at(&output, 8), u32_at(&output, 12)), (1, 2));
        assert_eq!((u32_at(&output, 16), u32_at(&output, 20)), (1, 2));
        assert_eq!(u32_at(&output, 32), 120000);
        assert_eq!(&output[36..40], &[100, 40, 20, 128]);
    }
    #[test]
    fn resizing_keeps_original_dimensions_and_large_timelines_are_rejected() {
        let frames = image::Frames::new(Box::new(
            (0..2).map(|_| Ok(image::Frame::new(image::RgbaImage::new(2000, 10)))),
        ));
        let output = collect(
            frames,
            (2000, 10),
            1,
            Instant::now() + Duration::from_secs(3),
            image::metadata::Orientation::NoTransforms,
            false,
        )
        .unwrap()
        .unwrap();
        assert_eq!((u32_at(&output, 8), u32_at(&output, 12)), (2000, 10));
        assert_eq!(u32_at(&output, 16), 768);
        assert!(u32_at(&output, 20) <= 10);
        let source_frames=image::Frames::new(Box::new((0..2).map(|_|Ok(image::Frame::new(image::RgbaImage::new(2000,10))))));
        let source=collect(source_frames,(2000,10),1,Instant::now()+Duration::from_secs(3),image::metadata::Orientation::NoTransforms,true).unwrap().unwrap();
        assert_eq!((u32_at(&source,16),u32_at(&source,20)),(2000,10));
        let frames = image::Frames::new(Box::new(
            (0..8).map(|_| Ok(image::Frame::new(image::RgbaImage::new(768, 768)))),
        ));
        assert!(collect(
            frames,
            (768, 768),
            0,
            Instant::now() + Duration::from_secs(3),
            image::metadata::Orientation::NoTransforms,
            false
        )
        .unwrap_err()
        .contains("16 MiB"));
    }
}
