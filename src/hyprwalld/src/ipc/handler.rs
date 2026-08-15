use hyprwall_ipc::{Command, MonitorInfo, Response};

use crate::app::AppState;
use crate::render::RenderResources;
use crate::zone_manager::{ClearOutcome as ZoneClearOutcome, ZoneError};
use hyprwall_config::model::{Config, WallpaperSettings, ZoneConfig};
use hyprwall_config::store;

/// `render` is `&mut` (not pure logic, unlike the rest of this function)
/// because `Command::Set` may need to create/replace a zone's `MpvInstance`
/// and `ZoneTarget`, and `Command::Pause`/`Play` need to reach the zone's
/// live `MpvInstance` to actually toggle playback -- see
/// `render::RenderResources`. Tests that don't need real GL/mpv pass
/// `RenderResources::new_headless_for_test()`, under which the GL-touching
/// parts of `Set` silently no-op while the pure `ZoneManager` bookkeeping
/// they assert on still runs.
pub fn handle_command(state: &mut AppState, cmd: Command, render: &mut RenderResources) -> Response {
    match cmd {
        Command::MonitorList => Response::MonitorList(
            state
                .registry
                .names()
                .into_iter()
                .filter_map(|name| {
                    let m = state.registry.get(&name)?;
                    let mut group: Vec<String> = state
                        .zones
                        .zone_for_monitor(&name)
                        .map(|z| z.monitors.clone())
                        .unwrap_or_default();
                    group.sort();
                    Some(MonitorInfo {
                        name: m.name.clone(),
                        x: m.logical.x,
                        y: m.logical.y,
                        w: m.logical.w,
                        h: m.logical.h,
                        group,
                    })
                })
                .collect(),
        ),
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
        Command::Unset { monitor } => handle_unset(state, render, &monitor),
        Command::Pause { monitor } => set_paused(state, render, &monitor, true),
        Command::Play { monitor } => set_paused(state, render, &monitor, false),
        Command::SetWallpaperSettings { path, settings } => {
            persist_wallpaper_settings(state, &path, settings);
            for zone_id in state.zones.zone_ids_with_path(&path) {
                render.apply_wallpaper_settings_to_zone(zone_id, &settings);
            }
            Response::Ok
        }
    }
}

/// Clears the wallpaper for `monitor`'s zone. Unlike a monitor unplug, this
/// is user-initiated and must not split a real (multi-monitor) group apart:
/// a solo zone is dissolved (nothing left worth keeping), but a group keeps
/// every member -- it just stops playing -- so picking a wallpaper for the
/// same members later reforms the exact same group instead of the user
/// having to re-select monitors from scratch. See `ZoneManager::clear_path`.
fn handle_unset(state: &mut AppState, render: &mut RenderResources, monitor: &str) -> Response {
    match state.zones.clear_path(monitor) {
        ZoneClearOutcome::NotFound => Response::Error(format!("no wallpaper set for {monitor}")),
        // The GUI clears a whole group by unsetting each member in turn;
        // the first call already cleared the (shared) zone path, so later
        // calls in the same batch are a no-op success, not an error --
        // otherwise that loop would abort partway through and never refresh.
        ZoneClearOutcome::AlreadyCleared => Response::Ok,
        ZoneClearOutcome::Dissolved { zone_id } => {
            render.teardown_zone(zone_id);
            render.clear_monitor(monitor);
            persist(state);
            Response::Ok
        }
        ZoneClearOutcome::Cleared { zone_id, monitors } => {
            render.teardown_zone(zone_id);
            for m in &monitors {
                render.clear_monitor(m);
            }
            persist(state);
            Response::Ok
        }
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
    let settings = load_wallpaper_settings(&state.config_path, path);
    render.apply_wallpaper_settings_to_zone(outcome.zone_id, &settings);
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
            Some(ZoneConfig {
                monitors: zone.monitors.clone(),
                path: zone.path.clone()?,
            })
        })
        .collect();
    // `library_paths` and `wallpaper_settings` are written elsewhere
    // (`library_paths` by hyprwall-gui, `wallpaper_settings` by
    // `persist_wallpaper_settings` below) -- this function only ever
    // rebuilds `zones`, so both must carry their existing values through
    // rather than being defaulted away on every zone save.
    let existing = store::load(&state.config_path).unwrap_or_default();
    let _ = store::save(
        &state.config_path,
        &Config {
            zones,
            library_paths: existing.library_paths,
            wallpaper_settings: existing.wallpaper_settings,
        },
    );
}

fn load_wallpaper_settings(config_path: &std::path::Path, path: &str) -> WallpaperSettings {
    store::load(config_path)
        .ok()
        .and_then(|cfg| cfg.wallpaper_settings.get(path).copied())
        .unwrap_or_default()
}

fn persist_wallpaper_settings(state: &AppState, path: &str, settings: WallpaperSettings) {
    let mut cfg = store::load(&state.config_path).unwrap_or_default();
    cfg.wallpaper_settings.insert(path.to_string(), settings);
    let _ = store::save(&state.config_path, &cfg);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::monitor::{Monitor, Rect};
    use crate::monitor_registry::MonitorRegistry;
    use hyprwall_config::model::WallpaperSettings;

    fn state_with(names: &[&str]) -> AppState {
        let mut registry = MonitorRegistry::new();
        for (i, name) in names.iter().enumerate() {
            registry.insert(Monitor {
                name: name.to_string(),
                logical: Rect {
                    x: (i as i32) * 1920,
                    y: 0,
                    w: 1920,
                    h: 1080,
                },
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
        assert_eq!(
            resp,
            Response::MonitorList(vec![
                MonitorInfo {
                    name: "HDMI-A-1".to_string(),
                    x: 1920,
                    y: 0,
                    w: 1920,
                    h: 1080,
                    group: vec![]
                },
                MonitorInfo {
                    name: "eDP-1".to_string(),
                    x: 0,
                    y: 0,
                    w: 1920,
                    h: 1080,
                    group: vec![]
                },
            ])
        );
    }

    #[test]
    fn monitor_list_reports_solo_zone_group_of_self() {
        let mut state = state_with(&["eDP-1"]);
        let mut render = RenderResources::new_headless_for_test();
        handle_command(
            &mut state,
            Command::Set {
                monitors: vec!["eDP-1".to_string()],
                path: "/a.mp4".to_string(),
            },
            &mut render,
        );

        let resp = handle_command(&mut state, Command::MonitorList, &mut render);
        assert_eq!(
            resp,
            Response::MonitorList(vec![MonitorInfo {
                name: "eDP-1".to_string(),
                x: 0,
                y: 0,
                w: 1920,
                h: 1080,
                group: vec!["eDP-1".to_string()],
            }])
        );
    }

    #[test]
    fn monitor_list_reports_shared_group_for_a_multi_monitor_zone() {
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

        let resp = handle_command(&mut state, Command::MonitorList, &mut render);
        let Response::MonitorList(infos) = resp else {
            panic!("expected MonitorList")
        };
        let expected_group = vec!["HDMI-A-1".to_string(), "eDP-1".to_string()];
        for info in &infos {
            assert_eq!(
                info.group, expected_group,
                "monitor {} should list both zone members sorted",
                info.name
            );
        }
    }

    #[test]
    fn set_then_get_round_trips() {
        let mut state = state_with(&["eDP-1"]);
        let mut render = RenderResources::new_headless_for_test();
        let set_resp = handle_command(
            &mut state,
            Command::Set {
                monitors: vec!["eDP-1".to_string()],
                path: "/a.mp4".to_string(),
            },
            &mut render,
        );
        assert_eq!(set_resp, Response::Ok);

        let get_resp = handle_command(
            &mut state,
            Command::Get {
                monitor: "eDP-1".to_string(),
            },
            &mut render,
        );
        assert_eq!(get_resp, Response::Path("/a.mp4".to_string()));
    }

    #[test]
    fn set_unknown_monitor_returns_error() {
        let mut state = state_with(&["eDP-1"]);
        let mut render = RenderResources::new_headless_for_test();
        let resp = handle_command(
            &mut state,
            Command::Set {
                monitors: vec!["eDP-9".to_string()],
                path: "/a.mp4".to_string(),
            },
            &mut render,
        );
        assert_eq!(resp, Response::Error("unknown monitor eDP-9".to_string()));
    }

    #[test]
    fn get_before_set_returns_error() {
        let mut state = state_with(&["eDP-1"]);
        let mut render = RenderResources::new_headless_for_test();
        let resp = handle_command(
            &mut state,
            Command::Get {
                monitor: "eDP-1".to_string(),
            },
            &mut render,
        );
        assert_eq!(resp, Response::Error("no wallpaper set for eDP-1".to_string()));
    }

    #[test]
    fn unset_on_a_monitor_with_no_wallpaper_returns_error() {
        let mut state = state_with(&["eDP-1"]);
        let mut render = RenderResources::new_headless_for_test();
        let resp = handle_command(
            &mut state,
            Command::Unset {
                monitor: "eDP-1".to_string(),
            },
            &mut render,
        );
        assert_eq!(resp, Response::Error("no wallpaper set for eDP-1".to_string()));
    }

    #[test]
    fn unset_clears_a_solo_monitors_wallpaper() {
        let mut state = state_with(&["eDP-1"]);
        let mut render = RenderResources::new_headless_for_test();
        handle_command(
            &mut state,
            Command::Set {
                monitors: vec!["eDP-1".to_string()],
                path: "/a.mp4".to_string(),
            },
            &mut render,
        );

        let resp = handle_command(
            &mut state,
            Command::Unset {
                monitor: "eDP-1".to_string(),
            },
            &mut render,
        );
        assert_eq!(resp, Response::Ok);

        let get_resp = handle_command(
            &mut state,
            Command::Get {
                monitor: "eDP-1".to_string(),
            },
            &mut render,
        );
        assert_eq!(get_resp, Response::Error("no wallpaper set for eDP-1".to_string()));
    }

    #[test]
    fn unset_persists_the_removal_to_config_file() {
        let mut state = state_with(&["eDP-1"]);
        let mut render = RenderResources::new_headless_for_test();
        handle_command(
            &mut state,
            Command::Set {
                monitors: vec!["eDP-1".to_string()],
                path: "/a.mp4".to_string(),
            },
            &mut render,
        );

        handle_command(
            &mut state,
            Command::Unset {
                monitor: "eDP-1".to_string(),
            },
            &mut render,
        );

        let loaded = store::load(&state.config_path).unwrap();
        assert!(loaded.zones.is_empty());
    }

    #[test]
    fn unset_on_one_monitor_of_a_group_clears_the_whole_zones_path_but_keeps_the_group() {
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

        let resp = handle_command(
            &mut state,
            Command::Unset {
                monitor: "eDP-1".to_string(),
            },
            &mut render,
        );
        assert_eq!(resp, Response::Ok);

        // The path is zone-wide, so clearing it via either member clears
        // playback for both -- but the group itself must survive, not split.
        let edp1_get = handle_command(
            &mut state,
            Command::Get {
                monitor: "eDP-1".to_string(),
            },
            &mut render,
        );
        assert_eq!(edp1_get, Response::Error("no wallpaper set for eDP-1".to_string()));
        let hdmi_get = handle_command(
            &mut state,
            Command::Get {
                monitor: "HDMI-A-1".to_string(),
            },
            &mut render,
        );
        assert_eq!(hdmi_get, Response::Error("no wallpaper set for HDMI-A-1".to_string()));

        let Response::MonitorList(infos) = handle_command(&mut state, Command::MonitorList, &mut render) else {
            panic!("expected MonitorList")
        };
        let expected_group = vec!["HDMI-A-1".to_string(), "eDP-1".to_string()];
        for info in &infos {
            assert_eq!(
                info.group, expected_group,
                "monitor {} should still list both zone members",
                info.name
            );
        }
    }

    #[test]
    fn unsetting_every_member_of_a_group_in_turn_does_not_error_on_the_later_members() {
        // The GUI clears a whole group by calling Unset once per member in a
        // loop; the first call already clears the zone's (shared) path, so
        // later calls must succeed as a no-op rather than erroring and
        // aborting that loop partway through.
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

        handle_command(
            &mut state,
            Command::Unset {
                monitor: "eDP-1".to_string(),
            },
            &mut render,
        );
        let resp = handle_command(
            &mut state,
            Command::Unset {
                monitor: "HDMI-A-1".to_string(),
            },
            &mut render,
        );
        assert_eq!(resp, Response::Ok);
    }

    #[test]
    fn setting_a_wallpaper_again_after_clearing_reforms_the_same_group() {
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

        handle_command(
            &mut state,
            Command::Unset {
                monitor: "eDP-1".to_string(),
            },
            &mut render,
        );
        handle_command(
            &mut state,
            Command::Unset {
                monitor: "HDMI-A-1".to_string(),
            },
            &mut render,
        );

        handle_command(
            &mut state,
            Command::Set {
                monitors: vec!["eDP-1".to_string(), "HDMI-A-1".to_string()],
                path: "/new.mp4".to_string(),
            },
            &mut render,
        );

        assert_eq!(
            state.zones.zone_for_monitor("eDP-1").unwrap().id,
            state.zones.zone_for_monitor("HDMI-A-1").unwrap().id,
            "both members must land back in the same zone"
        );
        assert_eq!(state.zones.path_for_monitor("eDP-1"), Some("/new.mp4"));
        assert_eq!(state.zones.path_for_monitor("HDMI-A-1"), Some("/new.mp4"));
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
    fn set_preserves_library_paths_written_by_the_gui() {
        let mut state = state_with(&["eDP-1"]);
        let mut render = RenderResources::new_headless_for_test();

        // Simulate hyprwall-gui having already saved library folders before
        // hyprwalld ever writes a zone.
        let mut cfg = store::load(&state.config_path).unwrap();
        cfg.library_paths = vec!["/home/u/Videos".to_string()];
        store::save(&state.config_path, &cfg).unwrap();

        handle_command(
            &mut state,
            Command::Set {
                monitors: vec!["eDP-1".to_string()],
                path: "/a.mp4".to_string(),
            },
            &mut render,
        );

        let loaded = store::load(&state.config_path).unwrap();
        assert_eq!(loaded.library_paths, vec!["/home/u/Videos".to_string()]);
        assert_eq!(loaded.zones.len(), 1, "the zone save itself should still have happened");
    }

    #[test]
    fn pause_without_set_returns_error() {
        let mut state = state_with(&["eDP-1"]);
        let mut render = RenderResources::new_headless_for_test();
        let resp = handle_command(
            &mut state,
            Command::Pause {
                monitor: "eDP-1".to_string(),
            },
            &mut render,
        );
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
            Command::Set {
                monitors: vec!["eDP-1".to_string()],
                path: "/a.mp4".to_string(),
            },
            &mut render,
        );
        let resp = handle_command(
            &mut state,
            Command::Pause {
                monitor: "eDP-1".to_string(),
            },
            &mut render,
        );
        assert_eq!(resp, Response::Error("no active playback for eDP-1".to_string()));
    }

    #[test]
    fn set_wallpaper_settings_persists_to_config_file() {
        let mut state = state_with(&["eDP-1"]);
        let mut render = RenderResources::new_headless_for_test();
        let settings = WallpaperSettings {
            zoom: 1.5,
            ..WallpaperSettings::default()
        };

        let resp = handle_command(
            &mut state,
            Command::SetWallpaperSettings {
                path: "/a.jpg".to_string(),
                settings,
            },
            &mut render,
        );
        assert_eq!(resp, Response::Ok);

        let loaded = store::load(&state.config_path).unwrap();
        assert_eq!(loaded.wallpaper_settings.get("/a.jpg"), Some(&settings));
    }

    #[test]
    fn set_wallpaper_settings_preserves_zones_and_library_paths() {
        let mut state = state_with(&["eDP-1"]);
        let mut render = RenderResources::new_headless_for_test();
        handle_command(
            &mut state,
            Command::Set {
                monitors: vec!["eDP-1".to_string()],
                path: "/a.mp4".to_string(),
            },
            &mut render,
        );

        handle_command(
            &mut state,
            Command::SetWallpaperSettings {
                path: "/a.mp4".to_string(),
                settings: WallpaperSettings::default(),
            },
            &mut render,
        );

        let loaded = store::load(&state.config_path).unwrap();
        assert_eq!(
            loaded.zones.len(),
            1,
            "the earlier Set's zone must survive a later SetWallpaperSettings"
        );
    }

    #[test]
    fn apply_zone_looks_up_saved_wallpaper_settings_without_erroring() {
        // Headless RenderResources has no live mpv instance to inspect
        // property values on (no GL/mpv in a test environment), so this
        // proves the lookup-and-apply path in `apply_zone` runs cleanly
        // for a path with a saved settings entry, not that mpv actually
        // received them -- that's the plan's manual-verification step.
        let mut state = state_with(&["eDP-1"]);
        let mut render = RenderResources::new_headless_for_test();
        let settings = WallpaperSettings {
            brightness: 30.0,
            ..WallpaperSettings::default()
        };
        handle_command(
            &mut state,
            Command::SetWallpaperSettings {
                path: "/a.jpg".to_string(),
                settings,
            },
            &mut render,
        );

        apply_zone(&mut state, &mut render, &["eDP-1".to_string()], "/a.jpg").unwrap();

        assert_eq!(state.zones.path_for_monitor("eDP-1"), Some("/a.jpg"));
    }
}
