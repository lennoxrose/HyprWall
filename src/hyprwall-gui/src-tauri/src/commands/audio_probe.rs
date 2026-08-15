//! Detects whether a file has an audio track, headlessly -- gates whether
//! the settings sidebar's volume slider is worth showing at all. Same
//! event-driven "wait for a terminal mpv event, don't poll" shape as
//! `thumbnails.rs`'s `wait_for_load`, but waiting for `FileLoaded` (when
//! track metadata becomes available) rather than `EndFile` (when playback
//! finishes) -- this probe never actually plays anything (`pause=yes`), so
//! it never reaches a natural `EndFile` on its own.

use libmpv2::Mpv;
use libmpv2::events::Event;
use libmpv2::mpv_end_file_reason;

#[tauri::command]
pub fn has_audio_track(path: String) -> Result<bool, String> {
    has_audio_track_impl(&path).map_err(|e| e.to_string())
}

fn has_audio_track_impl(path: &str) -> anyhow::Result<bool> {
    // See thumbnails.rs's identical comment: GTK's init resets LC_NUMERIC
    // away from "C", and mpv refuses to initialize outside it.
    unsafe { libc::setlocale(libc::LC_NUMERIC, c"C".as_ptr()) };

    let mpv = Mpv::with_initializer(|init| {
        init.set_option("vo", "null")?;
        init.set_option("ao", "null")?;
        init.set_option("pause", "yes")?;
        init.set_option("osc", "no")?;
        Ok(())
    })?;
    mpv.command("loadfile", &[path, "replace"])?;
    wait_for_metadata(&mpv, path)?;

    let count: i64 = mpv.get_property("track-list/count").unwrap_or(0);
    for i in 0..count {
        let track_type: String =
            mpv.get_property(&format!("track-list/{i}/type")).unwrap_or_default();
        if track_type == "audio" {
            return Ok(true);
        }
    }
    Ok(false)
}

fn wait_for_metadata(mpv: &Mpv, path: &str) -> anyhow::Result<()> {
    loop {
        match mpv.wait_event(5.0) {
            Some(Ok(Event::FileLoaded)) => return Ok(()),
            Some(Ok(Event::EndFile(reason))) => {
                if reason == mpv_end_file_reason::Error {
                    anyhow::bail!("mpv failed to decode {path}");
                }
                return Ok(());
            }
            Some(Ok(_)) => continue,
            Some(Err(e)) => anyhow::bail!("mpv event error while probing {path}: {e}"),
            None => anyhow::bail!("timed out probing {path} for an audio track"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> String {
        format!("{}/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))
    }

    #[test]
    fn detects_an_audio_track() {
        assert!(has_audio_track_impl(&fixture("sample_with_audio.mp4")).unwrap());
    }

    #[test]
    fn reports_false_for_a_silent_video() {
        assert!(!has_audio_track_impl(&fixture("sample.mp4")).unwrap());
    }

    #[test]
    fn reports_false_for_a_still_image() {
        assert!(!has_audio_track_impl(&fixture("sample.png")).unwrap());
    }
}
