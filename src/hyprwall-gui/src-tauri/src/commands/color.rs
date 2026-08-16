use std::path::Path;

/// Average color of an already-generated thumbnail PNG, as `#rrggbb`.
/// Thumbnails are already downscaled to 320px wide (see `thumbnails.rs`),
/// so decoding one here is cheap enough to do on every scan.
pub fn dominant_color(thumbnail_path: &Path) -> Option<String> {
    let img = image::open(thumbnail_path).ok()?.into_rgb8();
    let pixels = img.pixels();
    let count = img.width() as u64 * img.height() as u64;
    if count == 0 {
        return None;
    }
    let (mut r, mut g, mut b) = (0u64, 0u64, 0u64);
    for p in pixels {
        r += p[0] as u64;
        g += p[1] as u64;
        b += p[2] as u64;
    }
    Some(format!(
        "#{:02x}{:02x}{:02x}",
        (r / count) as u8,
        (g / count) as u8,
        (b / count) as u8
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn averages_a_known_thumbnail() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/sample.png");
        let color = dominant_color(Path::new(path)).unwrap();
        assert_eq!(color.len(), 7);
        assert!(color.starts_with('#'));
    }

    #[test]
    fn missing_file_returns_none() {
        assert!(dominant_color(Path::new("/definitely/does/not/exist.png")).is_none());
    }
}
