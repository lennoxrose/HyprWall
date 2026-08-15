use hyprwall_ipc::{Command, Response};

use crate::app::AppState;
use hyprwall_config::model::{Config, ZoneConfig};
use hyprwall_config::store;
use crate::render::RenderResources;
use crate::zone_manager::ZoneError;

/// `render` is `&mut` (not pure logic, unlike the rest of this function)
/// because `Command::Set` may need to create/replace a zone's `MpvInstance`
/// + `ZoneTarget`, and `Command::Pause`/`Play` need to reach the zone's live
/// `MpvInstance` to actually toggle playback -- see `render::RenderResources`.
/// Tests that don't need real GL/mpv pass `RenderResources::
/// new_headless_for_test()`, under which the GL-touching parts of `Set`
/// silently no-op while the pure `ZoneManager` bookkeeping they assert on
/// still runs.
pub fn handle_command(state: &mut AppState, cmd: Command, render: &mut RenderResources) -> Response {
    match cmd {
        Command::MonitorList => Response::MonitorList(state.registry.names()),
        Command::Get { monitor } => match state.zones.path_for_monitor(&monitor) {
            Some(path) => Response::Path(path.to_string()),
            None => Response::Error(format!("no wallpaper set for {monitor}")),
        },
        Command::Set { monitors, path } => match apply_zone(state, render, &monitors, &path) {
            Ok(()) => {
                persist(state);
                Response::Ok
            }
            Err(ZoneError::UnknownMonitor(name)) => Response::Error(format!("unknown monitor {name}")),
        },
        Command::Pause { monitor } => set_paused(state, render, &monitor, true),
        Command::Play { monitor } => set_paused(state, render, &monitor, false),
    }
}

/// Applies `monitors`/`path` via `ZoneManager::apply_set`, then wires up real
/// playback via `RenderResources::apply_set_outcome` (a no-op beyond
/// `ZoneManager` bookkeeping if there's no GL context or the monitors'
/// surfaces aren't ready yet -- see that function's doc comment).
///
/// Deliberately does **not** persist to config -- that's `Command::Set`'s
/// job, done once by its caller right above after this returns `Ok`. Startup/
/// hotplug config restore (`main.rs`'s `restore_saved_zones`) calls this
/// directly instead of going through `handle_command`, and must NOT persist:
/// if only some of a saved config's zones have all their monitors present
/// yet, persisting mid-restore would rebuild `config.toml` from `ZoneManager`'s
/// current (partial) state and silently drop the not-yet-restored zones from
/// disk before their monitors ever get a chance to reappear.
pub fn apply_zone(
    state: &mut AppState,
    render: &mut RenderResources,
    monitors: &[String],
    path: &str,
) -> Result<(), ZoneError> {
    let outcome = state.zones.apply_set(monitors, path.to_string(), &state.registry)?;
    render.apply_set_outcome(&outcome, monitors, path);
    Ok(())
}

fn set_paused(state: &AppState, render: &RenderResources, monitor: &str, paused: bool) -> Response {
    let Some(zone) = state.zones.zone_for_monitor(monitor) else {
        return Response::Error(format!("no wallpaper set for {monitor}"));
    };
    let Some(zp) = render.zone_playback.get(&zone.id) else {
        return Response::Error(format!("no active playback for {monitor}"));
    };
    match zp.mpv.set_paused(paused) {
        Ok(()) => Response::Ok,
        Err(e) => Response::Error(format!("failed to set pause state: {e}")),
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
        let mut render = RenderResources::new_headless_for_test();
        let resp = handle_command(&mut state, Command::MonitorList, &mut render);
        assert_eq!(resp, Response::MonitorList(vec!["HDMI-A-1".to_string(), "eDP-1".to_string()]));
    }

    #[test]
    fn set_then_get_round_trips() {
        let mut state = state_with(&["eDP-1"]);
        let mut render = RenderResources::new_headless_for_test();
        let set_resp = handle_command(
            &mut state,
            Command::Set { monitors: vec!["eDP-1".to_string()], path: "/a.mp4".to_string() },
            &mut render,
        );
        assert_eq!(set_resp, Response::Ok);

        let get_resp =
            handle_command(&mut state, Command::Get { monitor: "eDP-1".to_string() }, &mut render);
        assert_eq!(get_resp, Response::Path("/a.mp4".to_string()));
    }

    #[test]
    fn set_unknown_monitor_returns_error() {
        let mut state = state_with(&["eDP-1"]);
        let mut render = RenderResources::new_headless_for_test();
        let resp = handle_command(
            &mut state,
            Command::Set { monitors: vec!["eDP-9".to_string()], path: "/a.mp4".to_string() },
            &mut render,
        );
        assert_eq!(resp, Response::Error("unknown monitor eDP-9".to_string()));
    }

    #[test]
    fn get_before_set_returns_error() {
        let mut state = state_with(&["eDP-1"]);
        let mut render = RenderResources::new_headless_for_test();
        let resp =
            handle_command(&mut state, Command::Get { monitor: "eDP-1".to_string() }, &mut render);
        assert_eq!(resp, Response::Error("no wallpaper set for eDP-1".to_string()));
    }

    #[test]
    fn set_persists_to_config_file() {
        let mut state = state_with(&["eDP-1", "HDMI-A-1"]);
        let mut render = RenderResources::new_headless_for_test();
        handle_command(
            &mut state,
            Command::Set {
                monitors: vec!["eDP-1".to_string(), "HDMI-A-1".to_string()],
                path: "/pano.mp4".to_string(),
            },
            &mut render,
        );
        let loaded = store::load(&state.config_path).unwrap();
        assert_eq!(loaded.zones.len(), 1);
        assert_eq!(loaded.zones[0].path, "/pano.mp4");
    }

    #[test]
    fn pause_without_set_returns_error() {
        let mut state = state_with(&["eDP-1"]);
        let mut render = RenderResources::new_headless_for_test();
        let resp =
            handle_command(&mut state, Command::Pause { monitor: "eDP-1".to_string() }, &mut render);
        assert_eq!(resp, Response::Error("no wallpaper set for eDP-1".to_string()));
    }

    #[test]
    fn apply_zone_does_not_persist_to_config_file() {
        // Unlike `Command::Set`, `apply_zone` (used directly by main.rs's
        // startup/hotplug config restore) must not write config.toml itself
        // -- see its doc comment on why persisting mid-restore could drop
        // not-yet-restored saved zones from disk.
        let mut state = state_with(&["eDP-1"]);
        let mut render = RenderResources::new_headless_for_test();
        apply_zone(&mut state, &mut render, &["eDP-1".to_string()], "/a.mp4").unwrap();

        assert!(!state.config_path.exists(), "apply_zone must not write config.toml");
        assert_eq!(state.zones.path_for_monitor("eDP-1"), Some("/a.mp4"));
    }

    #[test]
    fn pause_after_set_without_gpu_reports_no_active_playback() {
        // Headless `RenderResources` never creates a real `MpvInstance` (no
        // GL/EGL in a test environment), so `Set` succeeds (the pure
        // `ZoneManager` bookkeeping this module is responsible for) but
        // there is nothing live to pause -- that's a distinct, honest error
        // from "no wallpaper set at all".
        let mut state = state_with(&["eDP-1"]);
        let mut render = RenderResources::new_headless_for_test();
        handle_command(
            &mut state,
            Command::Set { monitors: vec!["eDP-1".to_string()], path: "/a.mp4".to_string() },
            &mut render,
        );
        let resp =
            handle_command(&mut state, Command::Pause { monitor: "eDP-1".to_string() }, &mut render);
        assert_eq!(resp, Response::Error("no active playback for eDP-1".to_string()));
    }
}
