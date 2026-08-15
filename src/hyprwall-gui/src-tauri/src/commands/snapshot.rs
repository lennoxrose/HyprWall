use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// `~/.cache/hyprwall-gui/monitor-snapshots`.
fn snapshot_dir() -> Result<PathBuf, String> {
    let dir = dirs::cache_dir()
        .ok_or_else(|| "no cache dir available for this platform".to_string())?
        .join("hyprwall-gui")
        .join("monitor-snapshots");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

/// Removes any previously captured snapshot for `monitor_name` (any file
/// starting with `<monitor_name>-`), ignoring failures -- best-effort
/// cleanup, not load-bearing for the capture that follows.
fn remove_previous_snapshots(dir: &std::path::Path, monitor_name: &str) {
    let prefix = format!("{monitor_name}-");
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        if entry.file_name().to_string_lossy().starts_with(&prefix) {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

/// Captures a single still frame of `monitor_name`'s current output via
/// `grim` (the standard wlroots/Hyprland screenshot tool) and returns the
/// PNG's path. Deliberately shells out rather than talking to the
/// compositor's screenshot protocol directly -- `grim` already exists for
/// exactly this, so this is the same "reuse what's already there" call as
/// the rest of hyprwall-gui's backend.
///
/// The filename includes the current timestamp rather than being a fixed
/// `<monitor_name>.png` overwritten in place: the frontend's `<img>` src is
/// this exact path via `convertFileSrc`, and browsers cache a failed image
/// load by URL -- a fixed path meant one bad load (e.g. from a since-fixed
/// bug, or a transient grim hiccup) stayed permanently broken on every
/// later open, since the URL never changed for the webview to know to
/// retry. A fresh filename per capture makes every open a genuinely new
/// URL, so there's nothing stale to ever get stuck on.
#[tauri::command]
pub fn capture_monitor_snapshot(monitor_name: String) -> Result<String, String> {
    let dir = snapshot_dir()?;
    remove_previous_snapshots(&dir, &monitor_name);

    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_millis();
    let dest = dir.join(format!("{monitor_name}-{millis}.png"));
    let status = std::process::Command::new("grim")
        .arg("-o")
        .arg(&monitor_name)
        .arg(&dest)
        .status()
        .map_err(|e| format!("failed to run grim: {e}"))?;
    if !status.success() {
        return Err(format!("grim exited with {status} capturing {monitor_name}"));
    }
    dest.to_str()
        .map(str::to_string)
        .ok_or_else(|| "snapshot path is not valid UTF-8".to_string())
}
