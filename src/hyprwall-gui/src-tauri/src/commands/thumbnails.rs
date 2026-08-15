use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use libmpv2::events::Event;
use libmpv2::{Mpv, mpv_end_file_reason};

use crate::commands::library::WallpaperKind;

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
pub fn ensure_thumbnail(path: &str, kind: WallpaperKind) -> anyhow::Result<PathBuf> {
    ensure_thumbnail_in(&thumbnail_cache_dir()?, path, kind)
}

/// Same as `ensure_thumbnail`, but against an explicit cache dir instead of
/// the real `dirs::cache_dir()`-derived one. Split out so tests can pass an
/// isolated `tempfile::tempdir()` directly rather than mutating the
/// process-global `$XDG_CACHE_HOME` env var, which `dirs::cache_dir()` reads
/// -- mutating it from multiple tests running in parallel threads in the
/// same process is a real race (each test's call can see another test's
/// value mid-run), not just a hypothetical one.
fn ensure_thumbnail_in(cache_dir: &Path, path: &str, kind: WallpaperKind) -> anyhow::Result<PathBuf> {
    let dest = cache_dir.join(cache_key(path));
    if dest.exists() {
        return Ok(dest);
    }
    generate_thumbnail(path, &dest, kind)?;
    Ok(dest)
}

/// Uses mpv's built-in `image` video output (not the render API `hyprwalld`
/// uses -- that needs a live Wayland/EGL surface this process doesn't have)
/// to decode one frame at 1 second in and write it straight to a PNG file,
/// with no window, GL context, or `screenshot` command involved. Verified
/// against the installed mpv (0.41.0):
/// `mpv --vo=image --vo-image-outdir=<dir> --vo-image-format=png --frames=1
/// --start=1 --vf=scale=320:-2 --ao=null <file>` writes `<dir>/00000001.png`.
///
/// The `scale` filter is load-bearing, not cosmetic: without it, a
/// full-resolution source (an 8K wallpaper photo, easily 7952x5304) makes
/// mpv encode and write a 60+MB PNG, which took ~7.7s on real hardware --
/// past `wait_for_stable_file`'s 5s deadline, so every large image in a
/// real photo library timed out. Downscaling first also produces a file
/// actually sized for the ~140px-wide grid cell this is rendered into
/// (`LibraryGrid.tsx`).
fn generate_thumbnail(path: &str, dest: &Path, kind: WallpaperKind) -> anyhow::Result<()> {
    let work_dir = tempfile::tempdir()?;
    let outdir = work_dir.path().to_string_lossy().into_owned();

    // GTK's init (pulled in by Tauri's webview) resets LC_NUMERIC back to
    // the process locale, undoing main.rs's startup setlocale call -- so
    // this has to happen again right here, immediately before mpv reads
    // it, not just once at process start.
    unsafe { libc::setlocale(libc::LC_NUMERIC, c"C".as_ptr()) };

    let mpv = Mpv::with_initializer(move |init| {
        init.set_option("vo", "image")?;
        init.set_option("vo-image-outdir", outdir.as_str())?;
        init.set_option("vo-image-format", "png")?;
        init.set_option("frames", "1")?;
        init.set_option("vf", "scale=320:-2")?;
        if kind == WallpaperKind::Video {
            // Skip a video's black intro frame. A still image has no
            // guaranteed second of content to seek into, so this is
            // video-only.
            init.set_option("start", "1")?;
        }
        init.set_option("ao", "null")?;
        init.set_option("osc", "no")?;
        Ok(())
    })?;
    mpv.command("loadfile", &[path, "replace"])?;
    wait_for_load(&mpv, path)?;

    let produced = work_dir.path().join("00000001.png");
    wait_for_stable_file(&produced, path)?;
    // Not `std::fs::rename`: the mpv work dir (under `$TMPDIR`, often tmpfs)
    // and the thumbnail cache dir (`dirs::cache_dir()`) are not guaranteed
    // to be on the same filesystem, and `rename(2)` fails with `EXDEV`
    // across a filesystem boundary. `copy` works regardless; `work_dir`'s
    // `Drop` cleans up `produced` along with the rest of the temp dir.
    std::fs::copy(&produced, dest)?;
    Ok(())
}

/// Blocks until mpv finishes loading `path` (successfully or not), instead
/// of relying purely on `wait_for_stable_file`'s filesystem polling. A file
/// mpv can't decode at all -- a broken download saved with an image
/// extension but actually containing an HTML error page, which really
/// happened in a real wallpaper folder -- never produces an output file, so
/// blind polling burned its *entire* deadline on every such file; with a
/// handful of them in a library, and the library watcher re-triggering the
/// scan, that compounded into an unresponsive GUI. mpv reports the failure
/// via an `EndFile` event within milliseconds; this surfaces it immediately
/// instead of waiting it out.
fn wait_for_load(mpv: &Mpv, path: &str) -> anyhow::Result<()> {
    loop {
        match mpv.wait_event(5.0) {
            Some(Ok(Event::EndFile(reason))) => {
                if reason == mpv_end_file_reason::Error {
                    anyhow::bail!("mpv failed to decode {path}");
                }
                return Ok(());
            }
            // StartFile/FileLoaded/VideoReconfig/etc -- not the terminal
            // event, keep waiting for EndFile.
            Some(Ok(_)) => continue,
            Some(Err(e)) => anyhow::bail!("mpv event error while loading {path}: {e}"),
            None => anyhow::bail!("timed out waiting for mpv to finish loading {path}"),
        }
    }
}

/// mpv's `vo=image` creates the output file before it finishes writing the
/// PNG's contents to it -- `path.exists()` alone becomes true well before
/// the encoded bytes are actually there, which showed up in practice as a
/// real thumbnail cached as a 0-byte file. Waits for the file to exist AND
/// hold a nonzero, unchanging size across two consecutive polls, which is
/// as close to "the write finished" as polling the filesystem gets without
/// an mpv-side completion signal for this VO. By the time this runs,
/// `wait_for_load` has already confirmed mpv finished decoding, so the
/// deadline here only needs to cover the tail of a disk write, not a full
/// decode -- 2s, not 5s.
fn wait_for_stable_file(path: &Path, video_path: &str) -> anyhow::Result<()> {
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut last_len: Option<u64> = None;
    loop {
        if let Ok(metadata) = std::fs::metadata(path) {
            let len = metadata.len();
            if len > 0 && last_len == Some(len) {
                return Ok(());
            }
            last_len = Some(len);
        }
        if Instant::now() > deadline {
            anyhow::bail!("timed out waiting for a thumbnail frame from {video_path}");
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wait_for_stable_file_does_not_return_on_an_empty_file() {
        // Reproduces the real bug: mpv creates the output file before it has
        // written any content to it. If wait_for_stable_file returned as
        // soon as the path merely existed, this test's later write would
        // race a copy that already happened -- the caller must see the
        // *final* nonzero size, not the initial empty one.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("00000001.png");
        std::fs::write(&path, b"").unwrap(); // file exists, zero bytes, like mpv's initial create

        let path_for_writer = path.clone();
        let writer = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(150));
            std::fs::write(&path_for_writer, b"not a real png but nonzero").unwrap();
        });

        wait_for_stable_file(&path, "irrelevant").unwrap();
        writer.join().unwrap();

        assert!(std::fs::metadata(&path).unwrap().len() > 0);
    }

    #[test]
    fn undecodable_file_fails_fast_instead_of_hanging_out_the_deadline() {
        // Reproduces the real bug: a real wallpaper library had a file
        // named like a `.jpg` that was actually an HTML error page (a
        // broken download). mpv can't decode it and says so within
        // milliseconds, but the old code only polled the filesystem for an
        // output file that would never appear, burning the full 5s
        // deadline on every such file before this fix.
        let cache_root = tempfile::tempdir().unwrap();
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/corrupt.jpg");

        let start = Instant::now();
        let result = ensure_thumbnail_in(cache_root.path(), path, WallpaperKind::Image);
        let elapsed = start.elapsed();

        assert!(result.is_err(), "expected an error decoding an HTML file as an image");
        assert!(elapsed < Duration::from_secs(1), "should fail fast, took {elapsed:?}");
    }

    fn fixture_path() -> String {
        concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/sample.mp4").to_string()
    }

    fn fixture_image_path() -> String {
        concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/sample.png").to_string()
    }

    #[test]
    fn generates_a_thumbnail_from_a_still_image() {
        let cache_root = tempfile::tempdir().unwrap();
        let path = fixture_image_path();

        let thumb = ensure_thumbnail_in(cache_root.path(), &path, WallpaperKind::Image).unwrap();
        assert!(thumb.exists());
        assert!(std::fs::metadata(&thumb).unwrap().len() > 0);
    }

    #[test]
    fn generates_and_caches_a_thumbnail() {
        let cache_root = tempfile::tempdir().unwrap();
        let path = fixture_path();

        let thumb = ensure_thumbnail_in(cache_root.path(), &path, WallpaperKind::Video).unwrap();
        assert!(thumb.exists());
        assert!(thumb.starts_with(cache_root.path()));

        // Second call hits the cache instead of regenerating: assert by
        // checking the file's mtime doesn't change.
        let mtime_before = std::fs::metadata(&thumb).unwrap().modified().unwrap();
        std::thread::sleep(Duration::from_millis(20));
        let thumb_again = ensure_thumbnail_in(cache_root.path(), &path, WallpaperKind::Video).unwrap();
        let mtime_after = std::fs::metadata(&thumb_again).unwrap().modified().unwrap();
        assert_eq!(thumb, thumb_again);
        assert_eq!(mtime_before, mtime_after, "second call should not have regenerated the file");
    }

    #[test]
    fn works_when_cache_dir_is_on_a_different_filesystem_than_tmp() {
        // Reproduces the real bug: mpv's work dir always lands under
        // `$TMPDIR` (tmpfs on most Linux setups), but the thumbnail cache
        // dir is wherever `dirs::cache_dir()` points (typically on the root
        // filesystem) -- a plain `rename(2)` fails with EXDEV across that
        // boundary. Forcing the fake cache dir onto the crate's own
        // filesystem (never tmpfs, unlike the default temp dir) reproduces
        // that boundary instead of coincidentally landing both dirs on the
        // same tmpfs a bare `tempfile::tempdir()` would.
        let cache_root = tempfile::Builder::new().tempdir_in(env!("CARGO_MANIFEST_DIR")).unwrap();

        let thumb = ensure_thumbnail_in(cache_root.path(), &fixture_path(), WallpaperKind::Video).unwrap();
        assert!(thumb.exists());
        assert!(thumb.starts_with(cache_root.path()));
    }

    #[test]
    fn generates_a_thumbnail_even_when_lc_numeric_is_not_c() {
        // Reproduces the real bug: GTK (pulled in by Tauri's webview) resets
        // LC_NUMERIC away from "C" at some point before a thumbnail is ever
        // requested. mpv refuses to initialize outside "C" for LC_NUMERIC,
        // so generate_thumbnail must re-set it itself rather than relying on
        // a one-time setlocale call at process start.
        //
        // setlocale's target is process-global, same as the env-var problem
        // ensure_thumbnail_in's split avoids -- but there is no per-call
        // parameter for it (it's glibc/mpv's own global state, not this
        // crate's), so this test truly cannot run concurrently with the
        // others in this file without a locale race. `#[test]` alone can't
        // express that; run this file with `--test-threads=1` if this test
        // is ever seen flaking.
        let cache_root = tempfile::tempdir().unwrap();

        unsafe { libc::setlocale(libc::LC_NUMERIC, c"en_US.UTF-8".as_ptr()) };
        let thumb = ensure_thumbnail_in(cache_root.path(), &fixture_path(), WallpaperKind::Video).unwrap();
        assert!(thumb.exists());

        // Restore, so this test doesn't leak process-global locale state
        // into whichever test runs after it in the same test binary.
        unsafe { libc::setlocale(libc::LC_NUMERIC, c"C".as_ptr()) };
    }
}
