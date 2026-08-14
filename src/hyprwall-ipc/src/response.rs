#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Response {
    Ok,
    Path(String),
    MonitorList(Vec<String>),
    Error(String),
}

impl Response {
    pub fn to_wire(&self) -> String {
        match self {
            Response::Ok => "ok".to_string(),
            Response::Path(p) => p.clone(),
            Response::MonitorList(names) => names.join("\n"),
            Response::Error(msg) => format!("error: {msg}"),
        }
    }
}

/// Best-effort parse of a response body back into a `Response`. `Path` and
/// `MonitorList` are wire-ambiguous (both are plain text), so this is only
/// used where the caller already knows which command it sent; ambiguous
/// cases are treated as a single-element list falling back to `Path`.
pub fn parse_response(text: &str) -> Response {
    if let Some(msg) = text.strip_prefix("error: ") {
        return Response::Error(msg.to_string());
    }
    if text == "ok" {
        return Response::Ok;
    }
    if text.contains('\n') {
        return Response::MonitorList(text.lines().map(str::to_string).collect());
    }
    Response::Path(text.to_string())
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
        let r = Response::MonitorList(vec!["eDP-1".to_string(), "HDMI-A-1".to_string()]);
        assert_eq!(parse_response(&r.to_wire()), r);
    }
}
