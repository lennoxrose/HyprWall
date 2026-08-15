use std::process::Command;

const UNIT: &str = "hyprwalld.socket";

fn parse_is_enabled(stdout: &str) -> Result<bool, String> {
    match stdout.trim() {
        "enabled" => Ok(true),
        "disabled" => Ok(false),
        other => Err(format!("systemctl reports {UNIT} as {other:?} -- is hyprwall installed as a package?")),
    }
}

/// Whether the packaged `hyprwalld.socket` systemd user unit is enabled --
/// i.e. whether `hyprwalld` is reachable on demand (socket-activated) even
/// while not currently running.
#[tauri::command]
pub fn get_background_service_enabled() -> Result<bool, String> {
    let output = Command::new("systemctl")
        .args(["--user", "is-enabled", UNIT])
        .output()
        .map_err(|e| format!("failed to run systemctl: {e}"))?;
    parse_is_enabled(&String::from_utf8_lossy(&output.stdout))
}

#[tauri::command]
pub fn set_background_service_enabled(enabled: bool) -> Result<(), String> {
    let action = if enabled { "enable" } else { "disable" };
    let status = Command::new("systemctl")
        .args(["--user", action, "--now", UNIT])
        .status()
        .map_err(|e| format!("failed to run systemctl: {e}"))?;
    if !status.success() {
        return Err(format!("systemctl {action} {UNIT} exited with {status}"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_enabled() {
        assert_eq!(parse_is_enabled("enabled\n"), Ok(true));
    }

    #[test]
    fn parses_disabled() {
        assert_eq!(parse_is_enabled("disabled\n"), Ok(false));
    }

    #[test]
    fn errors_on_anything_else() {
        let err = parse_is_enabled("not-found\n").unwrap_err();
        assert!(err.contains("not-found"), "got: {err}");
    }
}
