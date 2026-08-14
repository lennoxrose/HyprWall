use hyprwall_ipc::{Command, Response};

use crate::app::AppState;
use crate::config::model::{Config, ZoneConfig};
use crate::config::store;
use crate::zone_manager::ZoneError;

pub fn handle_command(state: &mut AppState, cmd: Command) -> Response {
    match cmd {
        Command::MonitorList => Response::MonitorList(state.registry.names()),
        Command::Get { monitor } => match state.zones.path_for_monitor(&monitor) {
            Some(path) => Response::Path(path.to_string()),
            None => Response::Error(format!("no wallpaper set for {monitor}")),
        },
        Command::Set { monitors, path } => {
            match state.zones.apply_set(&monitors, path, &state.registry) {
                Ok(_outcome) => {
                    persist(state);
                    Response::Ok
                }
                Err(ZoneError::UnknownMonitor(name)) => {
                    Response::Error(format!("unknown monitor {name}"))
                }
            }
        }
        // Pause/Play act on playback state that only exists once a zone has a
        // running mpv instance (Task 10). Until then, report clearly rather
        // than silently succeeding.
        Command::Pause { monitor } | Command::Play { monitor } => {
            if state.zones.path_for_monitor(&monitor).is_some() {
                Response::Error("playback control not implemented yet".to_string())
            } else {
                Response::Error(format!("no wallpaper set for {monitor}"))
            }
        }
    }
}

fn persist(state: &AppState) {
    // Rebuild the zone list from ZoneManager's current state each time; this
    // is only ever called after a successful `apply_set`, so zones is never
    // empty in a way that matters here.
    let zones: Vec<ZoneConfig> = state
        .registry
        .names()
        .into_iter()
        .filter_map(|name| {
            let zone = state.zones.zone_for_monitor(&name)?;
            if zone.monitors.first() != Some(&name) {
                return None; // only emit each zone once, keyed by its first monitor
            }
            Some(ZoneConfig { monitors: zone.monitors.clone(), path: zone.path.clone()? })
        })
        .collect();
    let _ = store::save(&state.config_path, &Config { zones });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::monitor::{Monitor, Rect};
    use crate::monitor_registry::MonitorRegistry;

    fn state_with(names: &[&str]) -> AppState {
        let mut registry = MonitorRegistry::new();
        for (i, name) in names.iter().enumerate() {
            registry.insert(Monitor {
                name: name.to_string(),
                logical: Rect { x: (i as i32) * 1920, y: 0, w: 1920, h: 1080 },
            });
        }
        let dir = tempfile::tempdir().unwrap();
        AppState::new(registry, dir.path().join("config.toml"))
    }

    #[test]
    fn monitor_list_returns_registry_names() {
        let mut state = state_with(&["eDP-1", "HDMI-A-1"]);
        let resp = handle_command(&mut state, Command::MonitorList);
        assert_eq!(resp, Response::MonitorList(vec!["HDMI-A-1".to_string(), "eDP-1".to_string()]));
    }

    #[test]
    fn set_then_get_round_trips() {
        let mut state = state_with(&["eDP-1"]);
        let set_resp = handle_command(
            &mut state,
            Command::Set { monitors: vec!["eDP-1".to_string()], path: "/a.mp4".to_string() },
        );
        assert_eq!(set_resp, Response::Ok);

        let get_resp = handle_command(&mut state, Command::Get { monitor: "eDP-1".to_string() });
        assert_eq!(get_resp, Response::Path("/a.mp4".to_string()));
    }

    #[test]
    fn set_unknown_monitor_returns_error() {
        let mut state = state_with(&["eDP-1"]);
        let resp = handle_command(
            &mut state,
            Command::Set { monitors: vec!["eDP-9".to_string()], path: "/a.mp4".to_string() },
        );
        assert_eq!(resp, Response::Error("unknown monitor eDP-9".to_string()));
    }

    #[test]
    fn get_before_set_returns_error() {
        let mut state = state_with(&["eDP-1"]);
        let resp = handle_command(&mut state, Command::Get { monitor: "eDP-1".to_string() });
        assert_eq!(resp, Response::Error("no wallpaper set for eDP-1".to_string()));
    }

    #[test]
    fn set_persists_to_config_file() {
        let mut state = state_with(&["eDP-1", "HDMI-A-1"]);
        handle_command(
            &mut state,
            Command::Set {
                monitors: vec!["eDP-1".to_string(), "HDMI-A-1".to_string()],
                path: "/pano.mp4".to_string(),
            },
        );
        let loaded = store::load(&state.config_path).unwrap();
        assert_eq!(loaded.zones.len(), 1);
        assert_eq!(loaded.zones[0].path, "/pano.mp4");
    }
}
