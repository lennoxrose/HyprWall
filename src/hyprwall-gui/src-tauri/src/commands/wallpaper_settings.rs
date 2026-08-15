use std::path::Path;

use hyprwall_config::model::WallpaperSettings;
use hyprwall_ipc::{Command, Response, client, default_socket_path, parse_response};

#[tauri::command]
pub fn get_wallpaper_settings(path: String) -> Result<WallpaperSettings, String> {
    get_wallpaper_settings_at(&hyprwall_config::store::default_config_path(), &path)
}

fn get_wallpaper_settings_at(config_path: &Path, path: &str) -> Result<WallpaperSettings, String> {
    let cfg = hyprwall_config::store::load(config_path).map_err(|e| e.to_string())?;
    Ok(cfg.wallpaper_settings.get(path).copied().unwrap_or_default())
}

#[tauri::command]
pub fn set_wallpaper_settings(path: String, settings: WallpaperSettings) -> Result<(), String> {
    set_wallpaper_settings_at(&default_socket_path(), &path, settings)
}

fn set_wallpaper_settings_at(
    socket_path: &Path,
    path: &str,
    settings: WallpaperSettings,
) -> Result<(), String> {
    let cmd = Command::SetWallpaperSettings { path: path.to_string(), settings };
    let raw = client::send(socket_path, &cmd).map_err(|e| format!("hyprwalld unreachable: {e}"))?;
    match parse_response(&raw) {
        Response::Ok => Ok(()),
        Response::Error(msg) => Err(msg),
        other => Err(format!("unexpected response: {other:?}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_on_missing_config_returns_default() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        assert_eq!(
            get_wallpaper_settings_at(&path, "/a.jpg").unwrap(),
            WallpaperSettings::default()
        );
    }

    #[test]
    fn get_returns_a_saved_entry() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let mut cfg = hyprwall_config::store::load(&path).unwrap();
        let settings = WallpaperSettings { zoom: 1.4, ..WallpaperSettings::default() };
        cfg.wallpaper_settings.insert("/a.jpg".to_string(), settings);
        hyprwall_config::store::save(&path, &cfg).unwrap();

        assert_eq!(get_wallpaper_settings_at(&path, "/a.jpg").unwrap(), settings);
    }

    #[test]
    fn set_sends_the_command_and_reports_ok() {
        let dir = tempfile::tempdir().unwrap();
        let socket_path = dir.path().join("hyprwall.sock");
        let listener = std::os::unix::net::UnixListener::bind(&socket_path).unwrap();
        let server = std::thread::spawn(move || {
            use std::io::{Read, Write};
            let (mut conn, _) = listener.accept().unwrap();
            let mut received = String::new();
            conn.read_to_string(&mut received).unwrap();
            assert!(received.starts_with("wallpaper-settings "));
            conn.write_all(b"ok").unwrap();
        });

        let resp = set_wallpaper_settings_at(&socket_path, "/a.jpg", WallpaperSettings::default());
        server.join().unwrap();
        assert_eq!(resp, Ok(()));
    }

    #[test]
    fn set_surfaces_a_daemon_error() {
        let dir = tempfile::tempdir().unwrap();
        let socket_path = dir.path().join("hyprwall.sock");
        let listener = std::os::unix::net::UnixListener::bind(&socket_path).unwrap();
        let server = std::thread::spawn(move || {
            use std::io::{Read, Write};
            let (mut conn, _) = listener.accept().unwrap();
            let mut received = String::new();
            conn.read_to_string(&mut received).unwrap();
            conn.write_all(b"error: something broke").unwrap();
        });

        let resp = set_wallpaper_settings_at(&socket_path, "/a.jpg", WallpaperSettings::default());
        server.join().unwrap();
        assert_eq!(resp, Err("something broke".to_string()));
    }
}
