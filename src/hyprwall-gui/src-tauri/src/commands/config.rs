use std::path::Path;

use hyprwall_config::store;

#[tauri::command]
pub fn get_library_folders() -> Result<Vec<String>, String> {
    get_library_folders_at(&store::default_config_path())
}

#[tauri::command]
pub fn set_library_folders(folders: Vec<String>) -> Result<(), String> {
    set_library_folders_at(&store::default_config_path(), folders)
}

fn get_library_folders_at(path: &Path) -> Result<Vec<String>, String> {
    store::load(path)
        .map(|cfg| cfg.library_paths)
        .map_err(|e| e.to_string())
}

fn set_library_folders_at(path: &Path, folders: Vec<String>) -> Result<(), String> {
    let mut cfg = store::load(path).map_err(|e| e.to_string())?;
    cfg.library_paths = folders;
    store::save(path, &cfg).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_on_missing_file_returns_empty() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        assert_eq!(get_library_folders_at(&path).unwrap(), Vec::<String>::new());
    }

    #[test]
    fn set_then_get_round_trips_without_touching_zones() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");

        // Simulate hyprwalld having already saved a zone -- set_library_folders
        // must not clobber it.
        let mut cfg = store::load(&path).unwrap();
        cfg.zones.push(hyprwall_config::model::ZoneConfig {
            monitors: vec!["eDP-1".to_string()],
            path: "/a.mp4".to_string(),
        });
        store::save(&path, &cfg).unwrap();

        set_library_folders_at(&path, vec!["/home/u/Videos".to_string()]).unwrap();

        let loaded = store::load(&path).unwrap();
        assert_eq!(loaded.library_paths, vec!["/home/u/Videos".to_string()]);
        assert_eq!(
            loaded.zones.len(),
            1,
            "zones set by hyprwalld must survive a GUI-side folder update"
        );
    }
}
