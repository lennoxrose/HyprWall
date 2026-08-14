use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};

pub fn socket_path() -> PathBuf {
    let runtime_dir = std::env::var("XDG_RUNTIME_DIR").expect("XDG_RUNTIME_DIR is not set");
    PathBuf::from(runtime_dir).join("hyprwall.sock")
}

/// Binds the daemon's control socket. If a socket file already exists but
/// nothing is listening on it (stale, e.g. from a crash), it's removed and
/// rebound. If something *is* listening, this returns an `AddrInUse` error
/// rather than stealing the socket out from under a running instance.
pub fn bind_listener(path: &Path) -> std::io::Result<UnixListener> {
    if path.exists() {
        match UnixStream::connect(path) {
            Ok(_) => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::AddrInUse,
                    "hyprwalld is already running (socket is live)",
                ));
            }
            Err(_) => {
                std::fs::remove_file(path)?;
            }
        }
    }
    UnixListener::bind(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binds_fresh_socket() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hyprwall.sock");
        let listener = bind_listener(&path).unwrap();
        drop(listener);
    }

    #[test]
    fn removes_stale_socket_file_and_rebinds() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hyprwall.sock");
        // Bind and drop without cleanup, leaving a stale file on disk.
        let listener = UnixListener::bind(&path).unwrap();
        drop(listener);
        assert!(path.exists());

        let relistener = bind_listener(&path).unwrap();
        drop(relistener);
    }

    #[test]
    fn refuses_to_steal_a_live_socket() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hyprwall.sock");
        let _live = UnixListener::bind(&path).unwrap();

        let err = bind_listener(&path).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::AddrInUse);
    }
}

#[cfg(test)]
mod integration_tests {
    use std::io::{Read, Write};
    use std::os::unix::net::UnixStream;
    use std::thread;
    use std::time::Duration;

    use hyprwall_ipc::Command;

    use crate::app::AppState;
    use crate::ipc::handler::handle_command;
    use crate::monitor_registry::MonitorRegistry;

    use super::bind_listener;

    #[test]
    fn accept_loop_dispatches_one_command_per_connection() {
        let dir = tempfile::tempdir().unwrap();
        let socket_path = dir.path().join("hyprwall.sock");
        let config_path = dir.path().join("config.toml");
        let listener = bind_listener(&socket_path).unwrap();

        let server = thread::spawn(move || {
            let mut state = AppState::new(MonitorRegistry::new(), config_path);
            // Empty registry: `monitor list` should come back empty, and
            // `set` on any name should be rejected as unknown — this is the
            // seam Task 7 fills in later with real Wayland outputs.
            let (mut conn, _) = listener.accept().unwrap();
            let mut line = String::new();
            conn.read_to_string(&mut line).unwrap();
            let cmd = hyprwall_ipc::parse_command(&line).unwrap();
            let resp = handle_command(&mut state, cmd);
            conn.write_all(resp.to_wire().as_bytes()).unwrap();
        });

        thread::sleep(Duration::from_millis(50));
        let mut client = UnixStream::connect(&socket_path).unwrap();
        writeln!(client, "{}", Command::MonitorList.to_wire()).unwrap();
        client.shutdown(std::net::Shutdown::Write).unwrap();
        let mut response = String::new();
        client.read_to_string(&mut response).unwrap();

        server.join().unwrap();
        assert_eq!(response, "");
    }
}
