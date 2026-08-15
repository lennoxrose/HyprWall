use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub zones: Vec<ZoneConfig>,
    #[serde(default)]
    pub library_paths: Vec<String>,
    #[serde(default)]
    pub wallpaper_settings: HashMap<String, WallpaperSettings>,
    /// What `fit` a picture gets the first time it's assigned, before
    /// anyone's opened its sidebar and saved a `WallpaperSettings` entry
    /// for it. Doesn't touch already-configured pictures.
    #[serde(default)]
    pub default_fit: FitMode,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ZoneConfig {
    pub monitors: Vec<String>,
    pub path: String,
}

fn default_zoom() -> f64 {
    1.0
}

/// Per-picture display tuning: zoom/pan/fit control how the image or video
/// frame is cropped and positioned within its zone, volume/brightness/
/// contrast/hue/saturation map directly onto the mpv properties of the same
/// name. Keyed by file path in `Config.wallpaper_settings` -- these follow
/// the picture, not any particular monitor assignment.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WallpaperSettings {
    /// Linear zoom factor (1.0 = no zoom), not mpv's own log2 `video-zoom`
    /// space -- converted at the point mpv is actually told about it.
    #[serde(default = "default_zoom")]
    pub zoom: f64,
    #[serde(default)]
    pub pan_x: f64,
    #[serde(default)]
    pub pan_y: f64,
    #[serde(default)]
    pub fit: FitMode,
    /// 0-100. Default 0 (muted) -- a wallpaper stays silent until this is
    /// deliberately raised.
    #[serde(default)]
    pub volume: f64,
    /// -100 to 100, mpv's own native range for all four of these.
    #[serde(default)]
    pub brightness: f64,
    #[serde(default)]
    pub contrast: f64,
    #[serde(default)]
    pub hue: f64,
    #[serde(default)]
    pub saturation: f64,
}

impl Default for WallpaperSettings {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            pan_x: 0.0,
            pan_y: 0.0,
            fit: FitMode::default(),
            volume: 0.0,
            brightness: 0.0,
            contrast: 0.0,
            hue: 0.0,
            saturation: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FitMode {
    #[default]
    Cover,
    Contain,
    Stretch,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wallpaper_settings_default_matches_mpv_neutral_state() {
        let s = WallpaperSettings::default();
        assert_eq!(s.zoom, 1.0);
        assert_eq!(s.pan_x, 0.0);
        assert_eq!(s.pan_y, 0.0);
        assert_eq!(s.fit, FitMode::Cover);
        assert_eq!(s.volume, 0.0);
        assert_eq!(s.brightness, 0.0);
        assert_eq!(s.contrast, 0.0);
        assert_eq!(s.hue, 0.0);
        assert_eq!(s.saturation, 0.0);
    }

    #[test]
    fn wallpaper_settings_partial_toml_fills_missing_fields_from_defaults() {
        let parsed: WallpaperSettings = toml::from_str("brightness = 20.0").unwrap();
        assert_eq!(parsed.zoom, 1.0, "zoom's default is 1.0, not f64's own 0.0 default");
        assert_eq!(parsed.brightness, 20.0);
        assert_eq!(parsed.fit, FitMode::Cover);
    }

    #[test]
    fn config_default_fit_defaults_to_cover_and_round_trips() {
        let mut cfg = Config::default();
        assert_eq!(cfg.default_fit, FitMode::Cover);

        cfg.default_fit = FitMode::Contain;
        let text = toml::to_string_pretty(&cfg).unwrap();
        let round_tripped: Config = toml::from_str(&text).unwrap();
        assert_eq!(round_tripped.default_fit, FitMode::Contain);
    }

    #[test]
    fn config_round_trips_with_wallpaper_settings() {
        let mut cfg = Config::default();
        cfg.wallpaper_settings.insert(
            "/a.jpg".to_string(),
            WallpaperSettings { zoom: 1.5, ..WallpaperSettings::default() },
        );
        let text = toml::to_string_pretty(&cfg).unwrap();
        let round_tripped: Config = toml::from_str(&text).unwrap();
        assert_eq!(round_tripped, cfg);
    }
}
