use hyprwall_ipc::{client, default_socket_path, parse_response, Command, MonitorInfo, Response};
use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct MonitorState {
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub current_path: Option<String>,
}

#[tauri::command]
pub fn list_monitors() -> Result<Vec<MonitorState>, String> {
    list_monitors_at(&default_socket_path())
}

fn list_monitors_at(socket_path: &std::path::Path) -> Result<Vec<MonitorState>, String> {
    let infos = match send_and_parse(socket_path, Command::MonitorList)? {
        Response::MonitorList(infos) => infos,
        other => return Err(format!("unexpected response to monitor list: {other:?}")),
    };

    let mut states = Vec::with_capacity(infos.len());
    for MonitorInfo { name, x, y, w, h } in infos {
        let current_path = match send_and_parse(socket_path, Command::Get { monitor: name.clone() })? {
            Response::Path(p) => Some(p),
            Response::Error(_) => None,
            other => return Err(format!("unexpected response to get: {other:?}")),
        };
        states.push(MonitorState { name, x, y, w, h, current_path });
    }
    Ok(states)
}

#[tauri::command]
pub fn set_wallpaper(monitors: Vec<String>, path: String) -> Result<(), String> {
    ok_or_error(send_and_parse(&default_socket_path(), Command::Set { monitors, path })?)
}

#[tauri::command]
pub fn unset_wallpaper(monitor: String) -> Result<(), String> {
    ok_or_error(send_and_parse(&default_socket_path(), Command::Unset { monitor })?)
}

#[tauri::command]
pub fn pause_wallpaper(monitor: String) -> Result<(), String> {
    ok_or_error(send_and_parse(&default_socket_path(), Command::Pause { monitor })?)
}

#[tauri::command]
pub fn play_wallpaper(monitor: String) -> Result<(), String> {
    ok_or_error(send_and_parse(&default_socket_path(), Command::Play { monitor })?)
}

fn ok_or_error(resp: Response) -> Result<(), String> {
    match resp {
        Response::Ok => Ok(()),
        Response::Error(msg) => Err(msg),
        other => Err(format!("unexpected response: {other:?}")),
    }
}

fn send_and_parse(socket_path: &std::path::Path, cmd: Command) -> Result<Response, String> {
    let raw = client::send(socket_path, &cmd).map_err(|e| format!("hyprwalld unreachable: {e}"))?;
    Ok(parse_response(&raw))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::os::unix::net::UnixListener;
    use std::thread;

    #[test]
    fn list_monitors_merges_geometry_and_current_path() {
        let dir = tempfile::tempdir().unwrap();
        let socket_path = dir.path().join("hyprwall.sock");

        let listener = UnixListener::bind(&socket_path).unwrap();
        let server = thread::spawn(move || {
            for reply in ["eDP-1 0,0,1920,1080", "/a.mp4"] {
                let (mut conn, _) = listener.accept().unwrap();
                let mut received = String::new();
                conn.read_to_string(&mut received).unwrap();
                conn.write_all(reply.as_bytes()).unwrap();
            }
        });

        let states = list_monitors_at(&socket_path).unwrap();
        server.join().unwrap();

        assert_eq!(
            states,
            vec![MonitorState {
                name: "eDP-1".to_string(),
                x: 0,
                y: 0,
                w: 1920,
                h: 1080,
                current_path: Some("/a.mp4".to_string()),
            }]
        );
    }

    #[test]
    fn list_monitors_reports_none_for_unassigned_monitor() {
        let dir = tempfile::tempdir().unwrap();
        let socket_path = dir.path().join("hyprwall.sock");
        let listener = UnixListener::bind(&socket_path).unwrap();
        let server = thread::spawn(move || {
            for reply in ["eDP-1 0,0,1920,1080", "error: no wallpaper set for eDP-1"] {
                let (mut conn, _) = listener.accept().unwrap();
                let mut received = String::new();
                conn.read_to_string(&mut received).unwrap();
                conn.write_all(reply.as_bytes()).unwrap();
            }
        });

        let states = list_monitors_at(&socket_path).unwrap();
        server.join().unwrap();
        assert_eq!(states[0].current_path, None);
    }

    #[test]
    fn unset_wallpaper_sends_the_unset_command() {
        let dir = tempfile::tempdir().unwrap();
        let socket_path = dir.path().join("hyprwall.sock");
        let listener = UnixListener::bind(&socket_path).unwrap();
        let server = thread::spawn(move || {
            let (mut conn, _) = listener.accept().unwrap();
            let mut received = String::new();
            conn.read_to_string(&mut received).unwrap();
            assert_eq!(received.trim_end(), "unset eDP-1");
            conn.write_all(b"ok").unwrap();
        });

        let resp = send_and_parse(&socket_path, Command::Unset { monitor: "eDP-1".to_string() }).unwrap();
        server.join().unwrap();
        assert_eq!(resp, Response::Ok);
    }

    #[test]
    fn unreachable_daemon_surfaces_a_recognizable_error() {
        let dir = tempfile::tempdir().unwrap();
        let socket_path = dir.path().join("nothing-listening.sock");
        let err = list_monitors_at(&socket_path).unwrap_err();
        assert!(err.contains("hyprwalld unreachable"), "got: {err}");
    }
}
