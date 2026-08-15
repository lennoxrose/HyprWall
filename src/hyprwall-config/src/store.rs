use std::path::{Path, PathBuf};

use crate::model::Config;

pub fn default_config_path() -> PathBuf {
    dirs::config_dir()
        .expect("no config dir available for this platform")
        .join("hyprwall")
        .join("config.toml")
}

pub fn load(path: &Path) -> anyhow::Result<Config> {
    if !path.exists() {
        return Ok(Config::default());
    }
    let text = std::fs::read_to_string(path)?;
    Ok(toml::from_str(&text)?)
}

pub fn save(path: &Path, cfg: &Config) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text = toml::to_string_pretty(cfg)?;
    std::fs::write(path, text)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ZoneConfig;

    #[test]
    fn load_missing_file_returns_default() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("does-not-exist.toml");
        let cfg = load(&path).unwrap();
        assert_eq!(cfg, Config::default());
    }

    #[test]
    fn save_then_load_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("config.toml");
        let cfg = Config {
            zones: vec![
                ZoneConfig { monitors: vec!["eDP-1".to_string()], path: "/a.mp4".to_string() },
                ZoneConfig {
                    monitors: vec!["HDMI-A-1".to_string(), "HDMI-A-2".to_string()],
                    path: "/b.mp4".to_string(),
                },
            ],
            library_paths: vec![],
        };
        save(&path, &cfg).unwrap();
        let loaded = load(&path).unwrap();
        assert_eq!(loaded, cfg);
    }

    #[test]
    fn library_paths_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let cfg = Config { zones: vec![], library_paths: vec!["/home/u/Videos/wallpapers".to_string()] };
        save(&path, &cfg).unwrap();
        let loaded = load(&path).unwrap();
        assert_eq!(loaded, cfg);
    }
}
