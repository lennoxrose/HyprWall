use std::fmt;

use hyprwall_config::model::{FitMode, WallpaperSettings};

#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    MonitorList,
    Set { monitors: Vec<String>, path: String },
    Unset { monitor: String },
    Pause { monitor: String },
    Play { monitor: String },
    Get { monitor: String },
    SetWallpaperSettings { path: String, settings: WallpaperSettings },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    Empty,
    UnknownVerb(String),
    MissingArg { verb: &'static str, arg: &'static str },
    EmptyMonitorList,
    InvalidWallpaperSettings,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::Empty => write!(f, "empty command"),
            ParseError::UnknownVerb(v) => write!(f, "unknown command: {v}"),
            ParseError::MissingArg { verb, arg } => {
                write!(f, "{verb} requires <{arg}>")
            }
            ParseError::EmptyMonitorList => write!(f, "monitor list is empty"),
            ParseError::InvalidWallpaperSettings => write!(f, "invalid wallpaper-settings value"),
        }
    }
}

fn parse_wallpaper_settings_blob(blob: &str) -> Option<WallpaperSettings> {
    let mut settings = WallpaperSettings::default();
    for pair in blob.split(',') {
        let mut kv = pair.splitn(2, ':');
        let key = kv.next()?;
        let value = kv.next()?;
        match key {
            "zoom" => settings.zoom = value.parse().ok()?,
            "pan_x" => settings.pan_x = value.parse().ok()?,
            "pan_y" => settings.pan_y = value.parse().ok()?,
            "fit" => {
                settings.fit = match value {
                    "cover" => FitMode::Cover,
                    "contain" => FitMode::Contain,
                    "stretch" => FitMode::Stretch,
                    _ => return None,
                }
            }
            "volume" => settings.volume = value.parse().ok()?,
            "brightness" => settings.brightness = value.parse().ok()?,
            "contrast" => settings.contrast = value.parse().ok()?,
            "hue" => settings.hue = value.parse().ok()?,
            "saturation" => settings.saturation = value.parse().ok()?,
            _ => return None,
        }
    }
    Some(settings)
}

fn format_wallpaper_settings_blob(s: &WallpaperSettings) -> String {
    let fit = match s.fit {
        FitMode::Cover => "cover",
        FitMode::Contain => "contain",
        FitMode::Stretch => "stretch",
    };
    format!(
        "zoom:{},pan_x:{},pan_y:{},fit:{fit},volume:{},brightness:{},contrast:{},hue:{},saturation:{}",
        s.zoom, s.pan_x, s.pan_y, s.volume, s.brightness, s.contrast, s.hue, s.saturation
    )
}

pub fn parse_command(line: &str) -> Result<Command, ParseError> {
    let line = line.trim();
    if line.is_empty() {
        return Err(ParseError::Empty);
    }
    let mut top = line.splitn(2, ' ');
    let verb = top.next().unwrap();
    let rest = top.next().unwrap_or("").trim();

    match verb {
        "monitor" if rest == "list" => Ok(Command::MonitorList),
        "monitor" => Err(ParseError::UnknownVerb(format!("monitor {rest}"))),
        "set" => {
            let mut args = rest.splitn(2, ' ');
            let monitors_csv = args.next().unwrap_or("");
            let path = args.next().unwrap_or("").trim();
            if monitors_csv.is_empty() {
                return Err(ParseError::MissingArg { verb: "set", arg: "monitor" });
            }
            if path.is_empty() {
                return Err(ParseError::MissingArg { verb: "set", arg: "path" });
            }
            let monitors: Vec<String> = monitors_csv
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .collect();
            if monitors.is_empty() {
                return Err(ParseError::EmptyMonitorList);
            }
            Ok(Command::Set { monitors, path: path.to_string() })
        }
        "unset" if !rest.is_empty() => Ok(Command::Unset { monitor: rest.to_string() }),
        "unset" => Err(ParseError::MissingArg { verb: "unset", arg: "monitor" }),
        "pause" if !rest.is_empty() => Ok(Command::Pause { monitor: rest.to_string() }),
        "pause" => Err(ParseError::MissingArg { verb: "pause", arg: "monitor" }),
        "play" if !rest.is_empty() => Ok(Command::Play { monitor: rest.to_string() }),
        "play" => Err(ParseError::MissingArg { verb: "play", arg: "monitor" }),
        "get" if !rest.is_empty() => Ok(Command::Get { monitor: rest.to_string() }),
        "get" => Err(ParseError::MissingArg { verb: "get", arg: "monitor" }),
        "wallpaper-settings" => {
            let mut args = rest.splitn(2, ' ');
            let blob = args.next().unwrap_or("");
            let path = args.next().unwrap_or("").trim();
            if path.is_empty() {
                return Err(ParseError::MissingArg { verb: "wallpaper-settings", arg: "path" });
            }
            let settings =
                parse_wallpaper_settings_blob(blob).ok_or(ParseError::InvalidWallpaperSettings)?;
            Ok(Command::SetWallpaperSettings { path: path.to_string(), settings })
        }
        other => Err(ParseError::UnknownVerb(other.to_string())),
    }
}

impl Command {
    pub fn to_wire(&self) -> String {
        match self {
            Command::MonitorList => "monitor list".to_string(),
            Command::Set { monitors, path } => {
                format!("set {} {}", monitors.join(","), path)
            }
            Command::Unset { monitor } => format!("unset {monitor}"),
            Command::Pause { monitor } => format!("pause {monitor}"),
            Command::Play { monitor } => format!("play {monitor}"),
            Command::Get { monitor } => format!("get {monitor}"),
            Command::SetWallpaperSettings { path, settings } => {
                format!("wallpaper-settings {} {}", format_wallpaper_settings_blob(settings), path)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hyprwall_config::model::{FitMode, WallpaperSettings};

    #[test]
    fn parses_wallpaper_settings() {
        let settings = WallpaperSettings {
            zoom: 1.2,
            pan_x: -0.1,
            pan_y: 0.0,
            fit: FitMode::Contain,
            volume: 40.0,
            brightness: 10.0,
            contrast: -5.0,
            hue: 0.0,
            saturation: 20.0,
        };
        let line = "wallpaper-settings zoom:1.2,pan_x:-0.1,pan_y:0,fit:contain,volume:40,brightness:10,contrast:-5,hue:0,saturation:20 /home/u/Pictures/a photo.jpg";
        assert_eq!(
            parse_command(line),
            Ok(Command::SetWallpaperSettings { path: "/home/u/Pictures/a photo.jpg".to_string(), settings })
        );
    }

    #[test]
    fn rejects_wallpaper_settings_with_unknown_key() {
        let line = "wallpaper-settings zoom:1.0,bogus:1 /a.jpg";
        assert_eq!(parse_command(line), Err(ParseError::InvalidWallpaperSettings));
    }

    #[test]
    fn rejects_wallpaper_settings_missing_path() {
        let line = "wallpaper-settings zoom:1.0";
        assert_eq!(
            parse_command(line),
            Err(ParseError::MissingArg { verb: "wallpaper-settings", arg: "path" })
        );
    }

    #[test]
    fn to_wire_round_trips_wallpaper_settings() {
        let cmd = Command::SetWallpaperSettings {
            path: "/a.jpg".to_string(),
            settings: WallpaperSettings::default(),
        };
        assert_eq!(parse_command(&cmd.to_wire()), Ok(cmd));
    }

    #[test]
    fn parses_monitor_list() {
        assert_eq!(parse_command("monitor list"), Ok(Command::MonitorList));
    }

    #[test]
    fn parses_single_monitor_set() {
        assert_eq!(
            parse_command("set eDP-1 /home/u/video.mp4"),
            Ok(Command::Set {
                monitors: vec!["eDP-1".to_string()],
                path: "/home/u/video.mp4".to_string()
            })
        );
    }

    #[test]
    fn parses_multi_monitor_set_as_zone() {
        assert_eq!(
            parse_command("set eDP-1,HDMI-A-1 /home/u/pano.mp4"),
            Ok(Command::Set {
                monitors: vec!["eDP-1".to_string(), "HDMI-A-1".to_string()],
                path: "/home/u/pano.mp4".to_string()
            })
        );
    }

    #[test]
    fn parses_pause_play_get() {
        assert_eq!(parse_command("pause eDP-1"), Ok(Command::Pause { monitor: "eDP-1".to_string() }));
        assert_eq!(parse_command("play eDP-1"), Ok(Command::Play { monitor: "eDP-1".to_string() }));
        assert_eq!(parse_command("get eDP-1"), Ok(Command::Get { monitor: "eDP-1".to_string() }));
    }

    #[test]
    fn parses_unset() {
        assert_eq!(parse_command("unset eDP-1"), Ok(Command::Unset { monitor: "eDP-1".to_string() }));
    }

    #[test]
    fn rejects_unset_missing_monitor() {
        assert_eq!(parse_command("unset"), Err(ParseError::MissingArg { verb: "unset", arg: "monitor" }));
    }

    #[test]
    fn to_wire_round_trips_unset() {
        let cmd = Command::Unset { monitor: "eDP-1".to_string() };
        assert_eq!(parse_command(&cmd.to_wire()), Ok(cmd));
    }

    #[test]
    fn rejects_empty_line() {
        assert_eq!(parse_command(""), Err(ParseError::Empty));
        assert_eq!(parse_command("   "), Err(ParseError::Empty));
    }

    #[test]
    fn rejects_unknown_verb() {
        assert_eq!(parse_command("frobnicate x"), Err(ParseError::UnknownVerb("frobnicate".to_string())));
    }

    #[test]
    fn rejects_set_missing_path() {
        assert_eq!(
            parse_command("set eDP-1"),
            Err(ParseError::MissingArg { verb: "set", arg: "path" })
        );
    }

    #[test]
    fn rejects_set_empty_monitor_list() {
        assert_eq!(parse_command("set , /x.mp4"), Err(ParseError::EmptyMonitorList));
    }

    #[test]
    fn to_wire_round_trips_set() {
        let cmd = Command::Set {
            monitors: vec!["eDP-1".to_string(), "HDMI-A-1".to_string()],
            path: "/x.mp4".to_string(),
        };
        assert_eq!(parse_command(&cmd.to_wire()), Ok(cmd));
    }
}
