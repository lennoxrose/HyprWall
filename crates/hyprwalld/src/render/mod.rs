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
    pub fn sync_monitor_surfaces(&mut self, app_data: &AppData) {
        self.monitor_surfaces = app_data.render_targets.clone();
        self.core = app_data.egl_core.clone();
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
    /// the actual pixels are best-effort.
    pub fn apply_set_outcome(&mut self, outcome: &ZoneApplyOutcome, monitors: &[String], path: &str) {
        for id in &outcome.dissolved_zone_ids {
            if let Some(zp) = self.zone_playback.remove(id)
                && let Some(core) = &self.core
            {
                zp.target.destroy(&core.gl);
            }
        }

        let Some(core) = self.core.clone() else { return };
        let Some(any_surface) = monitors.iter().find_map(|m| self.monitor_surfaces.get(m).cloned())
        else {
            return;
        };
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
            self.zone_playback
                .insert(outcome.zone_id, ZonePlayback { mpv, target, monitors: monitors.to_vec() });
            self.needs_wakeup_wiring.push(outcome.zone_id);
        } else if let Some(zp) = self.zone_playback.get_mut(&outcome.zone_id) {
            zp.monitors = monitors.to_vec();
        }

        if let Some(zp) = self.zone_playback.get_mut(&outcome.zone_id)
            && let Err(e) = zp.mpv.load_file(path)
        {
            eprintln!("hyprwalld: mpv load_file({path}) failed for zone {}: {e}", outcome.zone_id);
        }
    }

    /// Renders one zone's current frame into its `ZoneTarget`, then blits
    /// the relevant crop into every member monitor's surface and presents
    /// it. Called from the zone's ping source (see `frame_scheduler`); a
    /// no-op if the zone or any of its monitor surfaces has since gone away.
    pub fn render_and_present(&mut self, zone_id: u64, monitors: &MonitorRegistry) {
        let Some(zp) = self.zone_playback.get_mut(&zone_id) else { return };
        if !zp.mpv.wants_redraw() {
            return;
        }
        let Some(any_surface) = zp.monitors.iter().find_map(|m| self.monitor_surfaces.get(m).cloned())
        else {
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
            let Some(surface) = self.monitor_surfaces.get(name) else { continue };
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
