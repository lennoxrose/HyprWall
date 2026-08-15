use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use libmpv2::Mpv;

/// `~/.cache/hyprwall-gui/thumbnails` (or the platform equivalent of
/// `dirs::cache_dir()`).
pub fn thumbnail_cache_dir() -> anyhow::Result<PathBuf> {
    let dir = dirs::cache_dir()
        .ok_or_else(|| anyhow::anyhow!("no cache dir available for this platform"))?
        .join("hyprwall-gui")
        .join("thumbnails");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn cache_key(video_path: &str) -> String {
    let mut hasher = DefaultHasher::new();
    video_path.hash(&mut hasher);
    format!("{:016x}.png", hasher.finish())
}

/// Returns the cached thumbnail path for `video_path`, generating it first
/// (via a one-shot headless mpv frame grab) if it isn't already cached.
pub fn ensure_thumbnail(video_path: &str) -> anyhow::Result<PathBuf> {
    let dest = thumbnail_cache_dir()?.join(cache_key(video_path));
    if dest.exists() {
        return Ok(dest);
    }
    generate_thumbnail(video_path, &dest)?;
    Ok(dest)
}

/// Uses mpv's built-in `image` video output (not the render API `hyprwalld`
/// uses -- that needs a live Wayland/EGL surface this process doesn't have)
/// to decode one frame at 1 second in and write it straight to a PNG file,
/// with no window, GL context, or `screenshot` command involved. Verified
/// against the installed mpv (0.41.0):
/// `mpv --vo=image --vo-image-outdir=<dir> --vo-image-format=png --frames=1
/// --start=1 --ao=null <file>` writes `<dir>/00000001.png`.
fn generate_thumbnail(video_path: &str, dest: &Path) -> anyhow::Result<()> {
    let work_dir = tempfile::tempdir()?;
    let outdir = work_dir.path().to_string_lossy().into_owned();

    let mpv = Mpv::with_initializer(move |init| {
        init.set_option("vo", "image")?;
        init.set_option("vo-image-outdir", outdir.as_str())?;
        init.set_option("vo-image-format", "png")?;
        init.set_option("frames", "1")?;
        init.set_option("start", "1")?;
        init.set_option("ao", "null")?;
        init.set_option("osc", "no")?;
        Ok(())
    })?;
    mpv.command("loadfile", &[video_path, "replace"])?;

    let produced = work_dir.path().join("00000001.png");
    let deadline = Instant::now() + Duration::from_secs(5);
    while !produced.exists() {
        if Instant::now() > deadline {
            anyhow::bail!("timed out waiting for a thumbnail frame from {video_path}");
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    std::fs::rename(&produced, dest)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_path() -> String {
        concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/sample.mp4").to_string()
    }

    #[test]
    fn generates_and_caches_a_thumbnail() {
        // Isolate this test's cache dir via $XDG_CACHE_HOME so it doesn't
        // collide with a real user cache or other tests.
        let cache_root = tempfile::tempdir().unwrap();
        std::env::set_var("XDG_CACHE_HOME", cache_root.path());

        let path = fixture_path();
        let thumb = ensure_thumbnail(&path).unwrap();
        assert!(thumb.exists());
        assert!(thumb.starts_with(cache_root.path()));

        // Second call hits the cache instead of regenerating: assert by
        // checking the file's mtime doesn't change.
        let mtime_before = std::fs::metadata(&thumb).unwrap().modified().unwrap();
        std::thread::sleep(Duration::from_millis(20));
        let thumb_again = ensure_thumbnail(&path).unwrap();
        let mtime_after = std::fs::metadata(&thumb_again).unwrap().modified().unwrap();
        assert_eq!(thumb, thumb_again);
        assert_eq!(mtime_before, mtime_after, "second call should not have regenerated the file");
    }
}
