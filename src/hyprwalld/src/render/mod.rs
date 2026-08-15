//! Everything Task 10 needs that touches GL or mpv: the shared EGL context
//! and per-monitor window surfaces (mirrored from `wayland::connection::
//! AppData`, the Wayland dispatch state that actually creates/destroys
//! them -- see `sync_monitor_surfaces`), and each live zone's playback
//! resources (`ZonePlayback`: an `MpvInstance` rendering into a
//! `ZoneTarget`).
//!
//! Kept out of `AppState` (Task 5) specifically so Task 5's pure zone-
//! bookkeeping tests never need a real GPU or Wayland connection:
//! `RenderResources::new_headless_for_test` builds one with no EGL core, and
//! every method here that would touch GL/mpv checks for that and becomes a
//! documented no-op (zone bookkeeping in `ZoneManager`/`AppState` -- the part
//! those tests actually assert on -- is unaffected either way).

pub mod egl_context;
pub mod frame_scheduler;
pub mod mpv_instance;
pub mod zone_target;

use std::collections::HashMap;
use std::ffi::c_void;
use std::rc::Rc;

use egl_context::{EglCore, MonitorSurface};
use glow::HasContext;
use hyprwall_config::model::WallpaperSettings;
use mpv_instance::MpvInstance;
use zone_target::ZoneTarget;

use crate::monitor_registry::MonitorRegistry;
use crate::wayland::connection::AppData;
use crate::zone_manager::ZoneApplyOutcome;

/// One zone's live playback resources: the mpv player rendering into an
/// offscreen `ZoneTarget`, plus which monitors currently blit from it.
pub struct ZonePlayback {
    pub mpv: MpvInstance,
    pub target: ZoneTarget,
    pub monitors: Vec<String>,
}

/// A `ZoneApplyOutcome` that `apply_set_outcome` couldn't fully realize yet
/// because no EGL core exists, or none of the target monitors' surfaces have
/// been configured by Wayland yet (a real race at daemon startup: a startup
/// script's first `hyprwallctl set` can easily arrive before every output has
/// finished its first `configure`). Retried by `retry_pending_sets` once
/// `sync_monitor_surfaces` refreshes the mirrored surfaces.
#[derive(Clone)]
struct PendingSet {
    outcome: ZoneApplyOutcome,
    monitors: Vec<String>,
    path: String,
}

#[derive(Default)]
pub struct RenderResources {
    core: Option<Rc<EglCore>>,
    monitor_surfaces: HashMap<String, Rc<MonitorSurface>>,
    pub zone_playback: HashMap<u64, ZonePlayback>,
    /// Zone ids whose `MpvInstance` was just (re)created by
    /// `apply_set_outcome` and still needs its wakeup callback wired to a
    /// calloop ping. `handle_command`'s signature only carries `&mut
    /// RenderResources` (see `ipc::handler`), not a `LoopHandle` -- that
    /// only exists in `main.rs`, so this is the hand-off point: main.rs
    /// drains this after every command and calls `frame_scheduler::register`
    /// for each id.
    pub needs_wakeup_wiring: Vec<u64>,
    /// `Set`s that raced monitor configuration, keyed by the zone id they
    /// would have (re)formed. See `PendingSet`.
    pending_sets: HashMap<u64, PendingSet>,
}

impl RenderResources {
    pub fn new() -> Self {
        Self::default()
    }

    /// Test-only stub. Identical to `new()` -- there is no GL to set up
    /// either way until a real monitor surface exists -- but named
    /// separately so call sites document that they're deliberately running
    /// headless, per the Task 10 dispatch's ruling on Task 5/6 test fallout.
    #[cfg(test)]
    pub fn new_headless_for_test() -> Self {
        Self::default()
    }

    /// Refreshes this struct's view of live per-monitor surfaces and the
    /// shared EGL core from `AppData`. Cheap: only clones `Rc`s. Call after
    /// every Wayland dispatch tick, before handling any IPC command, so
    /// `Command::Set` always sees up-to-date surfaces.
    ///
    /// Also retries any `Set` that previously raced monitor configuration
    /// (see `PendingSet`) now that the surfaces just got refreshed -- this is
    /// the only place new `MonitorSurface`s become visible to this struct, so
    /// it's the right point to check whether a pending zone's monitor(s) are
    /// ready yet.
    pub fn sync_monitor_surfaces(&mut self, app_data: &AppData) {
        self.monitor_surfaces = app_data.render_targets.clone();
        self.core = app_data.egl_core.clone();
        self.retry_pending_sets();
    }

    /// Retries every `Set` recorded as pending. A no-op if still not ready
    /// (re-records itself via `apply_set_outcome`); harmless to call when
    /// `pending_sets` is empty.
    fn retry_pending_sets(&mut self) {
        if self.pending_sets.is_empty() {
            return;
        }
        let pending: Vec<PendingSet> = self.pending_sets.values().cloned().collect();
        for p in pending {
            self.apply_set_outcome(&p.outcome, &p.monitors, &p.path);
        }
    }

    /// Removes and (if a GL context is available) destroys a zone's playback
    /// resources, without any `ZoneManager` bookkeeping -- the caller is
    /// responsible for that side (either `apply_set_outcome`'s
    /// `dissolved_zone_ids` handling, or `teardown_zone` below for a monitor
    /// unplug). A no-op if the zone has no live playback (headless test, or a
    /// zone that raced monitor configuration and never got resources in the
    /// first place).
    fn drop_zone_playback(&mut self, zone_id: u64) {
        if let Some(zp) = self.zone_playback.remove(&zone_id)
            && let Some(core) = &self.core
        {
            zp.target.destroy(&core.gl);
        }
        self.pending_sets.remove(&zone_id);
    }

    /// Tears down a zone's playback resources (`MpvInstance` + `ZoneTarget`).
    /// Called from `main.rs` after `ZoneManager::remove_monitor` reports a
    /// zone was fully dissolved by a monitor unplug -- unlike a `Set`-
    /// triggered dissolve, there is no replacement zone to also set up here.
    pub fn teardown_zone(&mut self, zone_id: u64) {
        self.drop_zone_playback(zone_id);
    }

    /// Applies `settings` to `zone_id`'s live `MpvInstance`, if it has one
    /// -- a no-op (besides the log) for a headless test or a zone whose
    /// `Set` is still pending monitor configuration, same as every other
    /// GL/mpv call in this file.
    pub fn apply_wallpaper_settings_to_zone(&mut self, zone_id: u64, settings: &WallpaperSettings) {
        if let Some(zp) = self.zone_playback.get_mut(&zone_id)
            && let Err(e) = zp.mpv.apply_wallpaper_settings(settings)
        {
            eprintln!("hyprwalld: failed to apply wallpaper settings to zone {zone_id}: {e}");
        }
    }

    /// Clears one monitor's surface to black. Called after `Command::Unset`
    /// removes a monitor from a zone (whether or not the zone itself
    /// dissolved): without this, the monitor's layer-shell surface simply
    /// keeps showing whatever frame was blitted into it last, since nothing
    /// draws to it again once it's no longer any zone's member -- "removed
    /// the wallpaper" would otherwise look like nothing happened. A no-op if
    /// there's no GL context yet or this monitor's surface isn't tracked
    /// (headless tests, or a monitor whose first Wayland `configure` hasn't
    /// landed yet).
    pub fn clear_monitor(&mut self, name: &str) {
        let Some(core) = self.core.clone() else { return };
        let Some(surface) = self.monitor_surfaces.get(name).cloned() else {
            return;
        };
        if let Err(e) = surface.make_current() {
            eprintln!("hyprwalld: eglMakeCurrent failed clearing {name}: {e}");
            return;
        }
        unsafe {
            core.gl.clear_color(0.0, 0.0, 0.0, 1.0);
            core.gl.clear(glow::COLOR_BUFFER_BIT);
        }
        if let Err(e) = surface.swap_buffers() {
            eprintln!("hyprwalld: eglSwapBuffers failed clearing {name}: {e}");
        }
    }

    /// Records (or overwrites) a pending `Set` for `outcome.zone_id`, so it
    /// gets retried by `sync_monitor_surfaces` instead of being dropped.
    fn record_pending(&mut self, outcome: &ZoneApplyOutcome, monitors: &[String], path: &str) {
        self.pending_sets.insert(
            outcome.zone_id,
            PendingSet {
                outcome: outcome.clone(),
                monitors: monitors.to_vec(),
                path: path.to_string(),
            },
        );
    }

    /// Test-only accessor so `pending_sets` can stay private while still
    /// being assertable from `#[cfg(test)]` code in this module.
    #[cfg(test)]
    fn pending_set_count(&self) -> usize {
        self.pending_sets.len()
    }

    /// Applies a successful `ZoneManager::apply_set` outcome: drops
    /// dissolved zones' playback, (re)creates the (re)formed zone's
    /// `MpvInstance` + `ZoneTarget` if it doesn't exist yet or its bounding
    /// box changed, and (re)loads `path` either way.
    ///
    /// A no-op beyond the dissolved-zone/bookkeeping cleanup when there is
    /// no EGL core yet (headless tests, or a `Set` that races a
    /// not-yet-configured monitor's first Wayland `configure`) or when none
    /// of `monitors`' surfaces exist yet -- `ZoneManager`'s bookkeeping (and
    /// therefore `Get`/config persistence) still succeeds either way; only
    /// the actual pixels are best-effort. In that case the outcome is
    /// recorded in `pending_sets` and retried by `sync_monitor_surfaces`
    /// once a relevant monitor surface becomes available, instead of being
    /// silently dropped forever.
    pub fn apply_set_outcome(&mut self, outcome: &ZoneApplyOutcome, monitors: &[String], path: &str) {
        for id in &outcome.dissolved_zone_ids {
            self.drop_zone_playback(*id);
        }

        let Some(core) = self.core.clone() else {
            self.record_pending(outcome, monitors, path);
            return;
        };
        let Some(any_surface) = monitors.iter().find_map(|m| self.monitor_surfaces.get(m).cloned()) else {
            self.record_pending(outcome, monitors, path);
            return;
        };
        // Ready as of this call -- clear any earlier pending entry for this
        // zone so a later, unrelated failure below (a real GL/mpv error, not
        // "not ready yet") doesn't get silently retried forever.
        self.pending_sets.remove(&outcome.zone_id);

        if let Err(e) = any_surface.make_current() {
            eprintln!(
                "hyprwalld: eglMakeCurrent failed while setting up zone {}: {e}",
                outcome.zone_id
            );
            return;
        }

        let needs_new = self
            .zone_playback
            .get(&outcome.zone_id)
            .map(|zp| zp.target.bounding_box != outcome.bounding_box)
            .unwrap_or(true);

        if needs_new {
            if let Some(old) = self.zone_playback.remove(&outcome.zone_id) {
                old.target.destroy(&core.gl);
            }
            let target = match ZoneTarget::new(&core.gl, outcome.bounding_box) {
                Ok(t) => t,
                Err(e) => {
                    eprintln!(
                        "hyprwalld: failed to create render target for zone {}: {e}",
                        outcome.zone_id
                    );
                    return;
                }
            };
            let proc_core = Rc::clone(&core);
            let get_proc_address: Rc<dyn Fn(&str) -> *mut c_void> =
                Rc::new(move |name: &str| proc_core.get_proc_address(name));
            let mpv = match MpvInstance::new(get_proc_address) {
                Ok(m) => m,
                Err(e) => {
                    eprintln!(
                        "hyprwalld: failed to create mpv instance for zone {}: {e}",
                        outcome.zone_id
                    );
                    return;
                }
            };
            self.zone_playback.insert(
                outcome.zone_id,
                ZonePlayback {
                    mpv,
                    target,
                    monitors: monitors.to_vec(),
                },
            );
            self.needs_wakeup_wiring.push(outcome.zone_id);
        } else if let Some(zp) = self.zone_playback.get_mut(&outcome.zone_id) {
            zp.monitors = monitors.to_vec();
        }

        if let Some(zp) = self.zone_playback.get_mut(&outcome.zone_id)
            && let Err(e) = zp.mpv.load_file(path)
        {
            eprintln!(
                "hyprwalld: mpv load_file({path}) failed for zone {}: {e}",
                outcome.zone_id
            );
        }
    }

    /// True if `name` is currently a member of some live zone's playback --
    /// i.e. `render_and_present` would still (attempt to) blit to it once its
    /// surface exists. Checked against this struct's own per-zone `monitors`
    /// cache, not `ZoneManager`: `ZoneManager::remove_monitor` strips an
    /// unplugged monitor from a *surviving* multi-monitor zone's own
    /// bookkeeping (so `zone_for_monitor` correctly stops reporting it while
    /// disconnected), but deliberately leaves this struct's cache untouched so
    /// a later replug resumes blitting without recreating the zone -- see
    /// that function's doc comment. `main.rs`'s config-restore logic uses this
    /// to avoid restoring a fresh zone from disk on top of one that's already
    /// live and just waiting for its surface to reappear.
    pub fn is_monitor_live(&self, name: &str) -> bool {
        self.zone_playback
            .values()
            .any(|zp| zp.monitors.iter().any(|m| m == name))
    }

    /// Renders one zone's current frame into its `ZoneTarget`, then blits
    /// the relevant crop into every member monitor's surface and presents
    /// it. Called from the zone's ping source (see `frame_scheduler`); a
    /// no-op if the zone or any of its monitor surfaces has since gone away.
    pub fn render_and_present(&mut self, zone_id: u64, monitors: &MonitorRegistry) {
        let Some(zp) = self.zone_playback.get_mut(&zone_id) else {
            return;
        };
        if !zp.mpv.wants_redraw() {
            return;
        }
        let Some(any_surface) = zp.monitors.iter().find_map(|m| self.monitor_surfaces.get(m).cloned()) else {
            return;
        };
        if let Err(e) = any_surface.make_current() {
            eprintln!("hyprwalld: eglMakeCurrent failed rendering zone {zone_id}: {e}");
            return;
        }
        let bb = zp.target.bounding_box;
        if let Err(e) = zp.mpv.render_to_fbo(zp.target.fbo_raw(), bb.w, bb.h) {
            eprintln!("hyprwalld: mpv render_to_fbo failed for zone {zone_id}: {e}");
            return;
        }

        for name in &zp.monitors {
            let Some(surface) = self.monitor_surfaces.get(name) else {
                continue;
            };
            let Some(monitor) = monitors.get(name) else { continue };
            if let Err(e) = surface.make_current() {
                eprintln!("hyprwalld: eglMakeCurrent failed blitting zone {zone_id} to {name}: {e}");
                continue;
            }
            zp.target.blit_region(surface.gl(), monitor.logical);
            if let Err(e) = surface.swap_buffers() {
                eprintln!("hyprwalld: eglSwapBuffers failed for {name}: {e}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::monitor::Rect;

    fn outcome(zone_id: u64) -> ZoneApplyOutcome {
        ZoneApplyOutcome {
            zone_id,
            bounding_box: Rect {
                x: 0,
                y: 0,
                w: 1920,
                h: 1080,
            },
            dissolved_zone_ids: Vec::new(),
        }
    }

    // Full end-to-end "surface becomes ready, pending Set completes" can't
    // be exercised headlessly -- it needs a real EGLCore/MonitorSurface,
    // which needs a live Wayland connection (see the report for what a live
    // test would look like). These cover the part that *can* run headless:
    // a Set that races monitor configuration is recorded instead of
    // silently dropped, and retrying it while still not ready is a stable
    // no-op rather than a panic or a leak.

    #[test]
    fn set_with_no_core_is_recorded_as_pending_not_dropped() {
        let mut render = RenderResources::new_headless_for_test();
        render.apply_set_outcome(&outcome(1), &["eDP-1".to_string()], "/a.mp4");
        assert_eq!(render.pending_set_count(), 1);
        // The GL-free part still didn't blow up and there's still no live
        // playback to accidentally report as active.
        assert!(render.zone_playback.is_empty());
    }

    #[test]
    fn retrying_while_still_not_ready_is_idempotent() {
        let mut render = RenderResources::new_headless_for_test();
        render.apply_set_outcome(&outcome(1), &["eDP-1".to_string()], "/a.mp4");
        assert_eq!(render.pending_set_count(), 1);

        // sync_monitor_surfaces can't be called without a real AppData, but
        // it's a thin wrapper around exactly this retry call -- exercise it
        // directly to confirm repeated retries neither panic nor duplicate
        // the pending entry while nothing has become ready.
        render.retry_pending_sets();
        render.retry_pending_sets();
        assert_eq!(render.pending_set_count(), 1);
        assert!(render.zone_playback.is_empty());
    }

    #[test]
    fn dissolving_a_zone_clears_its_pending_set() {
        let mut render = RenderResources::new_headless_for_test();
        render.apply_set_outcome(&outcome(1), &["eDP-1".to_string()], "/a.mp4");
        assert_eq!(render.pending_set_count(), 1);

        // A later Set that dissolves zone 1 (e.g. its only monitor got
        // reassigned elsewhere) should give up on the stale pending retry,
        // not keep trying to realize a zone that no longer exists.
        let mut later = outcome(2);
        later.dissolved_zone_ids.push(1);
        render.apply_set_outcome(&later, &["HDMI-A-1".to_string()], "/b.mp4");

        assert_eq!(
            render.pending_set_count(),
            1,
            "zone 2's own Set is now pending in zone 1's place"
        );
        assert!(render.zone_playback.is_empty());
    }

    #[test]
    fn teardown_zone_clears_a_pending_set_too() {
        let mut render = RenderResources::new_headless_for_test();
        render.apply_set_outcome(&outcome(1), &["eDP-1".to_string()], "/a.mp4");
        assert_eq!(render.pending_set_count(), 1);

        render.teardown_zone(1);
        assert_eq!(render.pending_set_count(), 0);
    }

    // `is_monitor_live` is used by main.rs's config-restore logic to avoid
    // restoring a fresh zone on top of one that's already live. It reports
    // membership from `zone_playback`'s own cache, not from `pending_sets` --
    // a zone that's still waiting on its first surface isn't "live" yet in
    // the sense that matters here (there's nothing to avoid clobbering).

    #[test]
    fn is_monitor_live_is_false_with_no_zones() {
        let render = RenderResources::new_headless_for_test();
        assert!(!render.is_monitor_live("eDP-1"));
    }

    #[test]
    fn is_monitor_live_is_false_for_a_merely_pending_set() {
        let mut render = RenderResources::new_headless_for_test();
        render.apply_set_outcome(&outcome(1), &["eDP-1".to_string()], "/a.mp4");
        assert_eq!(
            render.pending_set_count(),
            1,
            "no GL core, so this only records a pending set"
        );
        assert!(
            !render.is_monitor_live("eDP-1"),
            "never got real playback resources, so not live"
        );
    }
}
