use std::process::Command;

const BACKGROUND_UNIT: &str = "hyprwalld.socket";
const STARTUP_UNIT: &str = "hyprwalld.service";

fn parse_is_enabled(unit: &str, stdout: &str) -> Result<bool, String> {
    match stdout.trim() {
        "enabled" => Ok(true),
        "disabled" => Ok(false),
        other => Err(format!("systemctl reports {unit} as {other:?} -- is hyprwall installed as a package?")),
    }
}

fn is_enabled(unit: &str) -> Result<bool, String> {
    let output = Command::new("systemctl")
        .args(["--user", "is-enabled", unit])
        .output()
        .map_err(|e| format!("failed to run systemctl: {e}"))?;
    parse_is_enabled(unit, &String::from_utf8_lossy(&output.stdout))
}

fn set_enabled(unit: &str, enabled: bool) -> Result<(), String> {
    let action = if enabled { "enable" } else { "disable" };
    let status = Command::new("systemctl")
        .args(["--user", action, "--now", unit])
        .status()
        .map_err(|e| format!("failed to run systemctl: {e}"))?;
    if !status.success() {
        return Err(format!("systemctl {action} {unit} exited with {status}"));
    }
    Ok(())
}

/// Whether the packaged `hyprwalld.socket` systemd user unit is enabled --
/// i.e. whether `hyprwalld` is reachable on demand (socket-activated) even
/// while not currently running.
#[tauri::command]
pub fn get_background_service_enabled() -> Result<bool, String> {
    is_enabled(BACKGROUND_UNIT)
}

#[tauri::command]
pub fn set_background_service_enabled(enabled: bool) -> Result<(), String> {
    set_enabled(BACKGROUND_UNIT, enabled)
}

/// Whether the packaged `hyprwalld.service` systemd user unit is enabled --
/// i.e. whether `hyprwalld` starts immediately at login, rather than lazily
/// on the first connection the way `hyprwalld.socket` alone provides.
#[tauri::command]
pub fn get_start_on_login_enabled() -> Result<bool, String> {
    is_enabled(STARTUP_UNIT)
}

#[tauri::command]
pub fn set_start_on_login_enabled(enabled: bool) -> Result<(), String> {
    set_enabled(STARTUP_UNIT, enabled)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_enabled() {
        assert_eq!(parse_is_enabled(BACKGROUND_UNIT, "enabled\n"), Ok(true));
    }

    #[test]
    fn parses_disabled() {
        assert_eq!(parse_is_enabled(BACKGROUND_UNIT, "disabled\n"), Ok(false));
    }

    #[test]
    fn errors_on_anything_else() {
        let err = parse_is_enabled(BACKGROUND_UNIT, "not-found\n").unwrap_err();
        assert!(err.contains("not-found"), "got: {err}");
    }

    #[test]
    fn error_names_the_unit_it_was_checking() {
        let err = parse_is_enabled(STARTUP_UNIT, "not-found\n").unwrap_err();
        assert!(err.contains(STARTUP_UNIT), "got: {err}");
    }
}
