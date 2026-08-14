use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    MonitorList,
    Set { monitors: Vec<String>, path: String },
    Pause { monitor: String },
    Play { monitor: String },
    Get { monitor: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    Empty,
    UnknownVerb(String),
    MissingArg { verb: &'static str, arg: &'static str },
    EmptyMonitorList,
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
        }
    }
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
        "pause" if !rest.is_empty() => Ok(Command::Pause { monitor: rest.to_string() }),
        "pause" => Err(ParseError::MissingArg { verb: "pause", arg: "monitor" }),
        "play" if !rest.is_empty() => Ok(Command::Play { monitor: rest.to_string() }),
        "play" => Err(ParseError::MissingArg { verb: "play", arg: "monitor" }),
        "get" if !rest.is_empty() => Ok(Command::Get { monitor: rest.to_string() }),
        "get" => Err(ParseError::MissingArg { verb: "get", arg: "monitor" }),
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
            Command::Pause { monitor } => format!("pause {monitor}"),
            Command::Play { monitor } => format!("play {monitor}"),
            Command::Get { monitor } => format!("get {monitor}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
