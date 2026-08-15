#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonitorInfo {
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    /// Sorted names of every monitor sharing this monitor's zone, including
    /// itself. Empty if this monitor has no wallpaper assigned at all. A
    /// single-element list (just itself) means a solo zone -- distinct from
    /// a real multi-monitor group, which has `group.len() > 1`.
    pub group: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Response {
    Ok,
    Path(String),
    MonitorList(Vec<MonitorInfo>),
    Error(String),
}

impl Response {
    pub fn to_wire(&self) -> String {
        match self {
            Response::Ok => "ok".to_string(),
            Response::Path(p) => p.clone(),
            Response::MonitorList(infos) => infos
                .iter()
                .map(|m| {
                    let group = if m.group.is_empty() { "-".to_string() } else { m.group.join(",") };
                    format!("{} {},{},{},{} {}", m.name, m.x, m.y, m.w, m.h, group)
                })
                .collect::<Vec<_>>()
                .join("\n"),
            Response::Error(msg) => format!("error: {msg}"),
        }
    }
}

/// Parses a response body back into a `Response`. `Path` and single-monitor
/// `MonitorList` replies are both plain text, but a `MonitorInfo` line has a
/// recognizable `name x,y,w,h` shape a filesystem path never does, so this
/// disambiguates directly rather than needing the caller to already know
/// which command it sent.
pub fn parse_response(text: &str) -> Response {
    if let Some(msg) = text.strip_prefix("error: ") {
        return Response::Error(msg.to_string());
    }
    if text == "ok" {
        return Response::Ok;
    }
    if text.contains('\n') {
        return Response::MonitorList(text.lines().filter_map(parse_monitor_info_line).collect());
    }
    if let Some(info) = parse_monitor_info_line(text) {
        return Response::MonitorList(vec![info]);
    }
    Response::Path(text.to_string())
}

fn parse_monitor_info_line(line: &str) -> Option<MonitorInfo> {
    let mut fields = line.splitn(3, ' ');
    let name = fields.next()?;
    let rect = fields.next()?;
    let group = fields.next()?;

    let mut parts = rect.split(',');
    let x = parts.next()?.parse().ok()?;
    let y = parts.next()?.parse().ok()?;
    let w = parts.next()?.parse().ok()?;
    let h = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }

    let group = if group == "-" { Vec::new() } else { group.split(',').map(str::to_string).collect() };

    Some(MonitorInfo { name: name.to_string(), x, y, w, h, group })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ok_round_trips() {
        assert_eq!(parse_response(&Response::Ok.to_wire()), Response::Ok);
    }

    #[test]
    fn error_round_trips() {
        let r = Response::Error("unknown monitor eDP-9".to_string());
        assert_eq!(parse_response(&r.to_wire()), r);
    }

    #[test]
    fn path_round_trips() {
        let r = Response::Path("/home/u/video.mp4".to_string());
        assert_eq!(parse_response(&r.to_wire()), r);
    }

    #[test]
    fn monitor_list_round_trips() {
        let r = Response::MonitorList(vec![
            MonitorInfo { name: "eDP-1".to_string(), x: 0, y: 0, w: 1920, h: 1080, group: vec!["eDP-1".to_string(), "HDMI-A-1".to_string()] },
            MonitorInfo { name: "HDMI-A-1".to_string(), x: 1920, y: 0, w: 1920, h: 1080, group: vec!["eDP-1".to_string(), "HDMI-A-1".to_string()] },
        ]);
        assert_eq!(parse_response(&r.to_wire()), r);
    }

    #[test]
    fn monitor_info_wire_format_is_name_space_rect_space_group() {
        let r = Response::MonitorList(vec![MonitorInfo {
            name: "eDP-1".to_string(),
            x: 0,
            y: 0,
            w: 1920,
            h: 1080,
            group: vec!["eDP-1".to_string()],
        }]);
        assert_eq!(r.to_wire(), "eDP-1 0,0,1920,1080 eDP-1");
    }

    #[test]
    fn monitor_info_with_empty_group_round_trips_via_placeholder() {
        let r = Response::MonitorList(vec![MonitorInfo {
            name: "eDP-1".to_string(),
            x: 0,
            y: 0,
            w: 1920,
            h: 1080,
            group: vec![],
        }]);
        assert_eq!(r.to_wire(), "eDP-1 0,0,1920,1080 -");
        assert_eq!(parse_response(&r.to_wire()), r);
    }
}
