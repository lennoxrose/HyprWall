use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};

use crate::Command;

/// Resolves the daemon's control socket path: `$XDG_RUNTIME_DIR/hyprwall.sock`.
/// Shared by `hyprwalld` (binds it), `hyprwallctl`, and `hyprwall-gui`
/// (both connect to it) so the path is defined in exactly one place.
pub fn default_socket_path() -> PathBuf {
    let runtime_dir = std::env::var("XDG_RUNTIME_DIR").expect("XDG_RUNTIME_DIR is not set");
    PathBuf::from(runtime_dir).join("hyprwall.sock")
}

pub fn send(socket_path: &Path, cmd: &Command) -> std::io::Result<String> {
    let mut stream = UnixStream::connect(socket_path)?;
    writeln!(stream, "{}", cmd.to_wire())?;
    stream.shutdown(std::net::Shutdown::Write)?;
    let mut response = String::new();
    stream.read_to_string(&mut response)?;
    Ok(response.trim_end().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::net::UnixListener;
    use std::thread;

    #[test]
    fn sends_command_and_reads_response() {
        let dir = tempfile::tempdir().unwrap();
        let socket_path = dir.path().join("test.sock");
        let listener = UnixListener::bind(&socket_path).unwrap();

        let server = thread::spawn(move || {
            let (mut conn, _) = listener.accept().unwrap();
            let mut received = String::new();
            conn.read_to_string(&mut received).unwrap();
            assert_eq!(received.trim_end(), "monitor list");
            conn.write_all(b"eDP-1\nHDMI-A-1").unwrap();
        });

        let response = send(&socket_path, &Command::MonitorList).unwrap();
        server.join().unwrap();

        assert_eq!(response, "eDP-1\nHDMI-A-1");
    }
}
