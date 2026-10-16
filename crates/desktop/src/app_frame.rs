//! Cache only the last successfully decoded frame; never reuse across window identities.
use eframe::egui;
use freelane_agent_protocol::WindowTarget;
use std::sync::Arc;
pub struct Key {
    target: WindowTarget,
    width: u32,
    height: u32,
    jpeg_hex: String,
}
pub struct Decoded {
    pub image: Option<egui::ColorImage>,
    pub key: Arc<Key>,
}
pub fn decode(
    target: WindowTarget,
    width: u32,
    height: u32,
    jpeg_hex: String,
    previous: Option<&Arc<Key>>,
) -> Result<Decoded, String> {
    if width == 0
        || height == 0
        || width > 640
        || height > 480
        || jpeg_hex.is_empty()
        || jpeg_hex.len() > 65536
        || !jpeg_hex.len().is_multiple_of(2)
    {
        return Err("Invalid window frame bounds".into());
    }
    if let Some(key) = previous {
        if key.target == target
            && key.width == width
            && key.height == height
            && key.jpeg_hex == jpeg_hex
        {
            return Ok(Decoded {
                image: None,
                key: Arc::clone(key),
            });
        }
    }
    let bytes: Result<Vec<u8>, _> = jpeg_hex
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|p| u8::from_str_radix(std::str::from_utf8(p).unwrap_or(""), 16))
        .collect();
    let bytes = bytes.map_err(|e| e.to_string())?;
    let mut reader =
        image::ImageReader::with_format(std::io::Cursor::new(bytes), image::ImageFormat::Jpeg);
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(4 * 1024 * 1024);
    limits.max_image_width = Some(640);
    limits.max_image_height = Some(480);
    reader.limits(limits);
    let rgb = reader.decode().map_err(|e| e.to_string())?.into_rgb8();
    if rgb.width() != width || rgb.height() != height {
        return Err("JPEG dimensions do not match the response".into());
    }
    Ok(Decoded {
        image: Some(egui::ColorImage::from_rgb(
            [width as usize, height as usize],
            rgb.as_raw(),
        )),
        key: Arc::new(Key {
            target,
            width,
            height,
            jpeg_hex,
        }),
    })
}
/// Recover a display cleared while an unchanged capture was in flight, without guest I/O.
pub fn restore(key: Arc<Key>) -> Result<Decoded, String> {
    decode(
        key.target.clone(),
        key.width,
        key.height,
        key.jpeg_hex.clone(),
        None,
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    fn target() -> WindowTarget {
        WindowTarget {
            handle: 1,
            pid: 2,
            created: 3,
            incarnation: 4,
        }
    }
    fn jpeg(value: u8) -> String {
        let mut data = Vec::new();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut data, 90)
            .encode(&[value, value, value], 1, 1, image::ExtendedColorType::Rgb8)
            .unwrap();
        data.iter().map(|b| format!("{b:02x}")).collect()
    }
    #[test]
    fn identical_validated_frame_skips_decode_and_retains_key() {
        let data = jpeg(20);
        let first = decode(target(), 1, 1, data.clone(), None).unwrap();
        assert!(first.image.is_some());
        let next = decode(target(), 1, 1, data, Some(&first.key)).unwrap();
        assert!(next.image.is_none());
        assert!(std::sync::Arc::ptr_eq(&first.key, &next.key));
    }
    #[test]
    fn changed_pixels_are_decoded() {
        let first = decode(target(), 1, 1, jpeg(20), None).unwrap();
        let next = decode(target(), 1, 1, jpeg(220), Some(&first.key)).unwrap();
        assert!(next.image.is_some());
        assert_ne!(first.image.unwrap().pixels, next.image.unwrap().pixels);
    }
    #[test]
    fn new_window_incarnation_never_reuses_previous_frame() {
        let data = jpeg(20);
        let first = decode(target(), 1, 1, data.clone(), None).unwrap();
        let next = decode(
            WindowTarget {
                incarnation: 5,
                ..target()
            },
            1,
            1,
            data,
            Some(&first.key),
        )
        .unwrap();
        assert!(next.image.is_some());
    }
    #[test]
    fn dimensions_are_checked_even_when_bytes_match() {
        let data = jpeg(20);
        let first = decode(target(), 1, 1, data.clone(), None).unwrap();
        assert!(decode(target(), 2, 1, data, Some(&first.key)).is_err());
    }
    #[test]
    fn invalid_frames_cannot_poison_or_match_a_valid_cache() {
        let first = decode(target(), 1, 1, jpeg(20), None).unwrap();
        for data in ["0".into(), "zz".into(), "00".into(), "0".repeat(65538)] {
            assert!(decode(target(), 1, 1, data, Some(&first.key)).is_err());
        }
        for (w, h) in [(0, 1), (641, 1), (1, 481)] {
            assert!(decode(target(), w, h, jpeg(20), Some(&first.key)).is_err());
        }
    }
}
