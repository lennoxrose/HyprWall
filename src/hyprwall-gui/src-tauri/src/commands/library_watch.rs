use std::path::Path;
use std::sync::Mutex;
use std::time::Duration;

use notify::RecursiveMode;
use notify_debouncer_mini::{new_debouncer, Debouncer};
use tauri::{AppHandle, Emitter, State};

/// The currently active library-folder watcher, if any. Held in Tauri
/// managed state so `watch_library_folders` can swap it out wholesale each
/// time the configured folders change, instead of trying to reconcile a
/// live watcher's folder set in place.
#[derive(Default)]
pub struct WatcherState(pub Mutex<Option<Debouncer<notify::RecommendedWatcher>>>);

/// (Re)starts watching `folders` for filesystem changes, replacing whatever
/// was watched before. Debounced (400ms) so a burst of events -- a video
/// mid-download landing as several writes, or an `rm -r` of many files --
/// coalesces into a single `library-changed` event instead of one per file.
/// The frontend reacts by re-running `scan_library` and updating just the
/// wallpaper grid, not a full page reload -- this only ever needs to tell it
/// "something changed," never what, so the event payload is empty.
#[tauri::command]
pub fn watch_library_folders(app: AppHandle, state: State<WatcherState>, folders: Vec<String>) -> Result<(), String> {
    let mut guard = state.0.lock().map_err(|e| e.to_string())?;
    *guard = None; // drop (and stop) whatever was watched before

    if folders.is_empty() {
        return Ok(());
    }

    let mut debouncer = new_debouncer(
        Duration::from_millis(400),
        move |result: notify_debouncer_mini::DebounceEventResult| {
            if result.is_ok() {
                let _ = app.emit("library-changed", ());
            }
        },
    )
    .map_err(|e| e.to_string())?;

    for folder in &folders {
        // Non-recursive: `scan_library` itself only scans one level deep, so
        // watching subdirectories too would just be events for files the
        // grid was never going to show anyway.
        if let Err(e) = debouncer
            .watcher()
            .watch(Path::new(folder), RecursiveMode::NonRecursive)
        {
            eprintln!("hyprwall-gui: failed to watch library folder {folder}: {e}");
        }
    }

    *guard = Some(debouncer);
    Ok(())
}
