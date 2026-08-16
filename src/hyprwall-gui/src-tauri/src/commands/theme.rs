use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// The app's full set of user-recolorable UI chrome tokens -- backgrounds,
/// borders, text, and the few semantic accent colors. Deliberately excludes
/// content colors like the Filter panel's wallpaper-color swatches; those
/// represent real wallpaper colors being filtered, not app styling.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ThemeColors {
    pub bg: String,
    pub bg_elevated: String,
    pub border: String,
    pub border_hover: String,
    pub text: String,
    pub text_muted: String,
    pub accent: String,
    pub accent_text: String,
    pub success: String,
    pub danger: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ThemeMode {
    Dark,
    Light,
}

impl ThemeMode {
    pub fn defaults(self) -> ThemeColors {
        match self {
            ThemeMode::Dark => ThemeColors {
                bg: "#0a0a0a".to_string(),
                bg_elevated: "#141414".to_string(),
                border: "#333333".to_string(),
                border_hover: "#555555".to_string(),
                text: "#eeeeee".to_string(),
                text_muted: "#888888".to_string(),
                accent: "#2563eb".to_string(),
                accent_text: "#ffffff".to_string(),
                success: "#4ade80".to_string(),
                danger: "#f87171".to_string(),
            },
            ThemeMode::Light => ThemeColors {
                bg: "#f5f5f5".to_string(),
                bg_elevated: "#ffffff".to_string(),
                border: "#dddddd".to_string(),
                border_hover: "#bbbbbb".to_string(),
                text: "#1a1a1a".to_string(),
                text_muted: "#666666".to_string(),
                accent: "#2563eb".to_string(),
                accent_text: "#ffffff".to_string(),
                success: "#16a34a".to_string(),
                danger: "#dc2626".to_string(),
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ThemeState {
    pub mode: ThemeMode,
    pub colors: ThemeColors,
}

impl Default for ThemeState {
    fn default() -> Self {
        Self {
            mode: ThemeMode::Dark,
            colors: ThemeMode::Dark.defaults(),
        }
    }
}

fn theme_path() -> Result<PathBuf, String> {
    let dir = dirs::config_dir()
        .ok_or_else(|| "no config dir available for this platform".to_string())?
        .join("hyprwall-gui");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("theme.json"))
}

#[tauri::command]
pub fn get_theme() -> Result<ThemeState, String> {
    get_theme_at(&theme_path()?)
}

fn get_theme_at(path: &Path) -> Result<ThemeState, String> {
    if !path.exists() {
        return Ok(ThemeState::default());
    }
    let data = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str(&data).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_theme(theme: ThemeState) -> Result<(), String> {
    set_theme_at(&theme_path()?, &theme)
}

fn set_theme_at(path: &Path, theme: &ThemeState) -> Result<(), String> {
    let data = serde_json::to_string_pretty(theme).map_err(|e| e.to_string())?;
    std::fs::write(path, data).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_on_missing_file_returns_dark_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("theme.json");
        assert_eq!(get_theme_at(&path).unwrap(), ThemeState::default());
    }

    #[test]
    fn set_then_get_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("theme.json");

        let theme = ThemeState {
            mode: ThemeMode::Light,
            colors: ThemeColors {
                accent: "#ff00ff".to_string(),
                ..ThemeMode::Light.defaults()
            },
        };
        set_theme_at(&path, &theme).unwrap();

        assert_eq!(get_theme_at(&path).unwrap(), theme);
    }
}
