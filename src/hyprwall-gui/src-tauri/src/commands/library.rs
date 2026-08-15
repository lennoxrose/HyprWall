use std::path::Path;

use serde::Serialize;

use crate::commands::thumbnails;

const VIDEO_EXTENSIONS: &[&str] = &["mp4", "webm", "mkv"];
const IMAGE_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "jfif", "gif", "webp", "bmp", "tif", "tiff", "tga", "ppm", "pgm", "pbm",
    "pnm", "sgi", "dpx", "exr", "jp2", "j2k", "psd", "xpm", "pcx", "qoi", "heic", "heif", "avif",
    "jxl",
];

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum WallpaperKind {
    Video,
    Image,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct WallpaperEntry {
    pub path: String,
    pub thumbnail_path: Option<String>,
    pub kind: WallpaperKind,
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
            let Some(kind) = classify(&path) else { continue };
            let Some(path_str) = path.to_str() else { continue };
            let thumbnail_path = match thumbnails::ensure_thumbnail(path_str) {
                Ok(p) => p.to_str().map(str::to_string),
                Err(e) => {
                    eprintln!("hyprwall-gui: thumbnail generation failed for {path_str}: {e:#}");
                    None
                }
            };
            entries.push(WallpaperEntry { path: path_str.to_string(), thumbnail_path, kind });
        }
    }
    entries.sort_by(|a, b| a.path.cmp(&b.path));
    entries
}

fn classify(path: &Path) -> Option<WallpaperKind> {
    if !path.is_file() {
        return None;
    }
    let ext = path.extension()?.to_str()?.to_lowercase();
    if VIDEO_EXTENSIONS.contains(&ext.as_str()) {
        Some(WallpaperKind::Video)
    } else if IMAGE_EXTENSIONS.contains(&ext.as_str()) {
        Some(WallpaperKind::Image)
    } else {
        None
    }
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

    #[test]
    fn finds_image_files_and_tags_kind() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("wall.png"), b"").unwrap();
        std::fs::write(dir.path().join("clip.mp4"), b"").unwrap();
        std::fs::write(dir.path().join("notes.txt"), b"").unwrap();

        let entries = scan_library(vec![dir.path().to_str().unwrap().to_string()]);
        let kinds: Vec<(String, WallpaperKind)> = entries
            .iter()
            .map(|e| (e.path.rsplit('/').next().unwrap().to_string(), e.kind))
            .collect();

        assert_eq!(
            kinds,
            vec![
                ("clip.mp4".to_string(), WallpaperKind::Video),
                ("wall.png".to_string(), WallpaperKind::Image),
            ]
        );
    }
}
