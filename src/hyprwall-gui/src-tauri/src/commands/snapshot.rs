use std::path::PathBuf;

/// `~/.cache/hyprwall-gui/monitor-snapshots` -- overwritten per monitor on
/// every capture (unlike thumbnails, a monitor's content is live, so there's
/// no point keying this off a content hash).
fn snapshot_dir() -> Result<PathBuf, String> {
    let dir = dirs::cache_dir()
        .ok_or_else(|| "no cache dir available for this platform".to_string())?
        .join("hyprwall-gui")
        .join("monitor-snapshots");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

/// Captures a single still frame of `monitor_name`'s current output via
/// `grim` (the standard wlroots/Hyprland screenshot tool) and returns the
/// PNG's path. Deliberately shells out rather than talking to the
/// compositor's screenshot protocol directly -- `grim` already exists for
/// exactly this, so this is the same "reuse what's already there" call as
/// the rest of hyprwall-gui's backend.
#[tauri::command]
pub fn capture_monitor_snapshot(monitor_name: String) -> Result<String, String> {
    let dest = snapshot_dir()?.join(format!("{monitor_name}.png"));
    let status = std::process::Command::new("grim")
        .arg("-o")
        .arg(&monitor_name)
        .arg(&dest)
        .status()
        .map_err(|e| format!("failed to run grim: {e}"))?;
    if !status.success() {
        return Err(format!("grim exited with {status} capturing {monitor_name}"));
    }
    dest.to_str().map(str::to_string).ok_or_else(|| "snapshot path is not valid UTF-8".to_string())
}
