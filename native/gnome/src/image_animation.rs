//! Portable validation for bounded, already-composited image timelines.
//! Called on Rust workers; GTK never interprets unvalidated packet metadata.
use std::{sync::Arc, time::Duration};
pub const BYTES: usize = 16 * 1024 * 1024;
#[derive(Debug)]
pub struct Frame {
    pub pixels: Arc<[u8]>,
    pub delay: Duration,
}
#[derive(Debug)]
pub struct Timeline {
    pub width: u32,
    pub height: u32,
    pub plays: u32,
    pub frames: Vec<Frame>,
}
impl Timeline {
    pub fn parse(bytes: &[u8]) -> Result<((u32, u32), Self), String> {
        if bytes.len() < 32 || &bytes[..8] != b"STIMA001" {
            return Err("Invalid image animation envelope".into());
        }
        let number = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        let original = (number(8), number(12));
        let width = number(16);
        let height = number(20);
        let count = number(24) as usize;
        let plays = number(28);
        if original.0 == 0
            || original.1 == 0
            || original.0 > 32768
            || original.1 > 32768
            || u64::from(original.0) * u64::from(original.1) > 16 * 1024 * 1024
            || width == 0
            || height == 0
            || width > 768
            || height > 768
            || !(2..=128).contains(&count)
        {
            return Err("Image animation dimensions or frame count exceed their budget".into());
        }
        let pixels = width as usize * height as usize * 4;
        let length = (pixels + 4)
            .checked_mul(count)
            .ok_or("Image animation length overflow")?;
        if length > BYTES || bytes.len() != 32 + length {
            return Err("Image animation bytes exceed their budget".into());
        }
        // Validate every delay before allocating immutable frame storage.
        if (0..count).any(|i| number(32 + i * (pixels + 4)) < 20) {
            return Err("Image animation delay is invalid".into());
        }
        let frames = (0..count)
            .map(|i| {
                let offset = 32 + i * (pixels + 4);
                Frame {
                    pixels: bytes[offset + 4..offset + 4 + pixels].into(),
                    delay: Duration::from_millis(u64::from(number(offset))),
                }
            })
            .collect();
        Ok((
            original,
            Self {
                width,
                height,
                plays,
                frames,
            },
        ))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn packet() -> Vec<u8> {
        let mut bytes = b"STIMA001".to_vec();
        for value in [5u32, 3, 5, 3, 2, 3] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        for (delay, color) in [(40u32, [100, 40, 20, 128]), (120000, [0, 0, 128, 128])] {
            bytes.extend_from_slice(&delay.to_le_bytes());
            for _ in 0..15 {
                bytes.extend_from_slice(&color);
            }
        }
        bytes
    }
    #[test]
    fn valid_packet_retains_dimensions_pixels_timing_and_plays() {
        let (original, timeline) = Timeline::parse(&packet()).unwrap();
        assert_eq!(original, (5, 3));
        assert_eq!(timeline.frames.len(), 2);
        assert_eq!(timeline.plays, 3);
        assert_eq!(timeline.frames[1].delay, Duration::from_secs(120));
        assert_eq!(&timeline.frames[0].pixels[..4], &[100, 40, 20, 128]);
        let mut bytes = packet();
        bytes[28..32].copy_from_slice(&0u32.to_le_bytes());
        assert_eq!(Timeline::parse(&bytes).unwrap().1.plays, 0);
    }
    #[test]
    fn invalid_headers_delays_lengths_and_extra_bytes_are_rejected() {
        for (offset, value) in [
            (8, 0u32),
            (12, 32769),
            (16, 769),
            (20, 0),
            (24, 1),
            (24, 129),
            (32, 0),
            (96, 19),
        ] {
            let mut bytes = packet();
            bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
            assert!(Timeline::parse(&bytes).is_err(), "{offset}: {value}");
        }
        let mut bytes = packet();
        bytes.pop();
        assert!(Timeline::parse(&bytes).is_err());
        let mut bytes = packet();
        bytes.push(0);
        assert!(Timeline::parse(&bytes).is_err());
        for count in 0..32 {
            assert!(Timeline::parse(&packet()[..count]).is_err());
        }
        let mut bytes = packet();
        bytes[16..20].copy_from_slice(&768u32.to_le_bytes());
        bytes[20..24].copy_from_slice(&768u32.to_le_bytes());
        bytes[24..28].copy_from_slice(&128u32.to_le_bytes());
        assert!(Timeline::parse(&bytes).unwrap_err().contains("bytes"));
    }
}
