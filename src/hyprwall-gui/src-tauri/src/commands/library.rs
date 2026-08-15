use std::path::Path;

use serde::Serialize;

use crate::commands::thumbnails;

const VIDEO_EXTENSIONS: &[&str] = &["mp4", "webm", "mkv"];

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct WallpaperEntry {
    pub path: String,
    pub thumbnail_path: Option<String>,
}

#[tauri::command]
pub fn scan_library(folders: Vec<String>) -> Vec<WallpaperEntry> {
    let mut entries = Vec::new();
    for folder in &folders {
        let read_dir = match std::fs::read_dir(folder) {
            Ok(rd) => rd,
            Err(e) => {
                eprintln!("hyprwall-gui: failed to read library folder {folder}: {e}");
                continue;
            }
        };
        for entry in read_dir.flatten() {
            let path = entry.path();
            if !is_video_file(&path) {
                continue;
            }
            let Some(path_str) = path.to_str() else { continue };
            let thumbnail_path =
                thumbnails::ensure_thumbnail(path_str).ok().and_then(|p| p.to_str().map(str::to_string));
            entries.push(WallpaperEntry { path: path_str.to_string(), thumbnail_path });
        }
    }
    entries.sort_by(|a, b| a.path.cmp(&b.path));
    entries
}

fn is_video_file(path: &Path) -> bool {
    path.is_file()
        && path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| VIDEO_EXTENSIONS.contains(&e.to_lowercase().as_str()))
            .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_video_files_ignores_others_and_sorts_by_path() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("b.mp4"), b"").unwrap();
        std::fs::write(dir.path().join("a.webm"), b"").unwrap();
        std::fs::write(dir.path().join("notes.txt"), b"").unwrap();
        std::fs::create_dir(dir.path().join("subdir")).unwrap(); // non-recursive: ignored

        let entries = scan_library(vec![dir.path().to_str().unwrap().to_string()]);
        let paths: Vec<&str> = entries.iter().map(|e| e.path.as_str()).collect();

        assert_eq!(paths.len(), 2, "expected only the two video files, got: {paths:?}");
        assert!(paths[0].ends_with("a.webm"));
        assert!(paths[1].ends_with("b.mp4"));
    }

    #[test]
    fn unreadable_folder_is_skipped_not_fatal() {
        let entries = scan_library(vec!["/definitely/does/not/exist".to_string()]);
        assert!(entries.is_empty());
    }

    #[test]
    fn multiple_folders_are_all_scanned() {
        let dir_a = tempfile::tempdir().unwrap();
        let dir_b = tempfile::tempdir().unwrap();
        std::fs::write(dir_a.path().join("one.mkv"), b"").unwrap();
        std::fs::write(dir_b.path().join("two.mp4"), b"").unwrap();

        let entries = scan_library(vec![
            dir_a.path().to_str().unwrap().to_string(),
            dir_b.path().to_str().unwrap().to_string(),
        ]);
        assert_eq!(entries.len(), 2);
    }
}
