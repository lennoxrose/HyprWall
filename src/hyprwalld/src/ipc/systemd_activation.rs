//! Accepts a listening socket handed off by systemd's socket-activation
//! protocol (`sd_listen_fds`), so `hyprwalld` can be started on-demand by
//! `hyprwalld.socket` instead of always self-binding its own socket. See
//! `packaging/systemd/hyprwalld.socket` -- it listens on the exact same
//! `$XDG_RUNTIME_DIR/hyprwall.sock` path `hyprwall_ipc::default_socket_path()`
//! already uses, so nothing downstream (`hyprwallctl`, the GUI) needs to
//! know or care whether this path was taken.
//!
//! Implemented by hand rather than via a crate: the handoff is exactly two
//! environment variables and one fixed file descriptor number, per
//! systemd's `sd_listen_fds(3)` contract.

use std::os::unix::io::FromRawFd;
use std::os::unix::net::UnixListener;

/// The first (and, for `hyprwalld`, only) fd systemd hands off starts here.
const SD_LISTEN_FDS_START: i32 = 3;

/// `true` if this process was launched via systemd socket activation with
/// at least one fd waiting, per the values `LISTEN_PID`/`LISTEN_FDS` would
/// hold in that case. Takes them as parameters (rather than reading
/// `std::env::var` itself) so this decision -- the actual logic worth
/// testing -- doesn't need a real process environment to exercise.
fn should_take_fd(listen_pid: Option<&str>, listen_fds: Option<&str>, our_pid: u32) -> bool {
    let pid_matches = listen_pid.and_then(|p| p.parse::<u32>().ok()) == Some(our_pid);
    let has_fds = listen_fds.and_then(|f| f.parse::<u32>().ok()).is_some_and(|n| n >= 1);
    pid_matches && has_fds
}

/// Returns the systemd-provided listening socket if `hyprwalld` was
/// launched via `hyprwalld.socket`, or `None` if it was launched normally
/// (by hand, or from a Hyprland `exec-once`) -- the caller falls back to
/// `ipc::socket::bind_listener` in that case, so both launch styles work
/// identically from here on.
pub fn take_listener() -> Option<UnixListener> {
    let pid = std::env::var("LISTEN_PID").ok();
    let fds = std::env::var("LISTEN_FDS").ok();
    if !should_take_fd(pid.as_deref(), fds.as_deref(), std::process::id()) {
        return None;
    }
    // SAFETY: systemd's contract guarantees fd `SD_LISTEN_FDS_START` is a
    // valid, already-listening socket fd handed off to this exact process
    // (just confirmed via `LISTEN_PID` above) for the lifetime of this
    // process -- nothing else in `hyprwalld` opens or closes it first.
    Some(unsafe { UnixListener::from_raw_fd(SD_LISTEN_FDS_START) })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn takes_fd_when_pid_matches_and_fds_available() {
        assert!(should_take_fd(Some("42"), Some("1"), 42));
    }

    #[test]
    fn refuses_when_pid_does_not_match() {
        assert!(!should_take_fd(Some("1"), Some("1"), 42));
    }

    #[test]
    fn refuses_when_fds_is_zero() {
        assert!(!should_take_fd(Some("42"), Some("0"), 42));
    }

    #[test]
    fn refuses_when_either_var_is_missing() {
        assert!(!should_take_fd(None, Some("1"), 42));
        assert!(!should_take_fd(Some("42"), None, 42));
    }
}
