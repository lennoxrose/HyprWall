//! Wayland connection bootstrap, output tracking, and per-output background
//! layer surfaces.
//!
//! Installed API note (SCTK 0.21.1, wayland-client 0.31.15): the version
//! targeted by the original task sketch (SCTK ~0.19) exposed a
//! `delegate_output!` macro. That macro no longer exists in 0.21; SCTK now
//! routes all its internal event handling through a `Dispatch2` trait and a
//! single blanket `delegate_dispatch2!(AppData)` macro that covers every
//! `Dispatch2` impl SCTK's modules register (including `OutputState`'s
//! handling of `wl_output` and `zxdg_output_v1`, and -- as of this task --
//! `CompositorState`'s handling of `wl_surface` and `LayerShell`'s handling
//! of `zwlr_layer_surface_v1`). `delegate_registry!` is unchanged. See
//! SCTK's own `examples/list_outputs.rs` for the same `delegate_registry!` +
//! `delegate_dispatch2!` pairing this file uses.
//!
//! Task 7 bound `zwlr_layer_shell_v1` directly (not via SCTK's
//! `shell::wlr_layer` wrapper) because `LayerShell::bind` requires the app
//! to already implement `LayerShellHandler`. This task adds that impl, so
//! `WaylandBackend::new` now binds the real `LayerShell` wrapper (which
//! subsumes the old presence check: `LayerShell::bind` itself fails if the
//! global is absent). Creating a layer surface also needs a `wl_surface`,
//! which needs `CompositorState` (and `AppData: CompositorHandler`) -- none
//! of that existed before Task 8.
//!
//! Event pump (changed in Task 10): `main.rs` no longer runs
//! `blocking_dispatch` on a background thread. It inserts `WaylandBackend`'s
//! connection + event queue into the single calloop event loop that also
//! drives IPC and per-zone render pings, via `calloop_wayland_source::
//! WaylandSource`, and calls `EventQueue::dispatch_pending(&mut AppData)`
//! from that source's callback. Everything here (including every EGL/GL
//! call) therefore runs on calloop's one thread -- necessary because
//! `MonitorSurface`/`EglCore` (see `render/egl_context.rs`) are `!Send` and
//! because EGL contexts are thread-affine anyway.

use std::collections::HashMap;
use std::rc::Rc;

use glow::HasContext;
use smithay_client_toolkit::compositor::{CompositorHandler, CompositorState};
use smithay_client_toolkit::output::{OutputHandler, OutputState};
use smithay_client_toolkit::registry::{ProvidesRegistryState, RegistryState};
use smithay_client_toolkit::shell::WaylandSurface;
use smithay_client_toolkit::shell::wlr_layer::{LayerShell, LayerShellHandler, LayerSurface, LayerSurfaceConfigure};
use smithay_client_toolkit::{delegate_dispatch2, delegate_registry, registry_handlers};
use wayland_client::protocol::wl_output::{self, WlOutput};
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_client::{Connection, QueueHandle};

use super::output::LayerSurfaces;
use crate::monitor::{Monitor, Rect};
use crate::monitor_registry::MonitorRegistry;
use crate::render::egl_context::{EglCore, MonitorSurface};

/// Dark blue-gray clear color shown for one frame when a monitor's surface
/// is first created, before any zone has been `set` on it (or if it never
/// is).
const CLEAR_COLOR: (f32, f32, f32, f32) = (0.05, 0.05, 0.08, 1.0);

pub struct WaylandBackend {
    pub conn: Connection,
    pub event_queue: wayland_client::EventQueue<AppData>,
}

pub struct AppData {
    pub registry_state: RegistryState,
    pub output_state: OutputState,
    pub compositor_state: CompositorState,
    pub layer_shell: LayerShell,
    pub layer_surfaces: LayerSurfaces,
    pub monitors: MonitorRegistry,
    /// Per-monitor EGL window surface, keyed by output name, all sharing
    /// `egl_core`'s single GL context/namespace (Task 10: a zone's video
    /// frame is rendered once into an offscreen texture and a different crop
    /// blitted into each member monitor's surface, which requires every
    /// monitor to see the same GL objects -- see `render/egl_context.rs`'s
    /// module docs). Wrapped in `Rc` so `render::RenderResources` can hold
    /// its own mirrored clone (`sync_monitor_surfaces`) without owning
    /// Wayland state; kept separate from `layer_surfaces` per the
    /// one-file-one-job convention. Populated lazily from the `configure`
    /// callback below.
    pub render_targets: HashMap<String, Rc<MonitorSurface>>,
    /// The shared EGL context/config, created alongside the first monitor's
    /// surface and reused for every subsequent one. `None` until then.
    pub egl_core: Option<Rc<EglCore>>,
    /// Output names whose layer surface still needs destroying, queued by
    /// `output_destroyed` instead of destroyed immediately. Destroying the
    /// `wl_surface` must not happen until every `Rc<MonitorSurface>`
    /// referencing it -- including any clone `render::RenderResources` is
    /// still holding from before this dispatch tick's `sync_monitor_
    /// surfaces` call -- has actually dropped (see `MonitorSurface`'s own
    /// drop-order requirement). `main.rs` drains this right after calling
    /// `sync_monitor_surfaces`, which is the point that stale clone is
    /// guaranteed gone.
    pub pending_layer_destroy: Vec<String>,
    /// Output names removed by `output_destroyed`, queued for `main.rs` to
    /// feed into `ZoneManager::remove_monitor` and (if that dissolves a
    /// zone) `RenderResources::teardown_zone`. `AppData` doesn't hold a
    /// `ZoneManager`/`RenderResources` itself (see `Daemon` in `main.rs`), so
    /// this is the same queue-and-drain hand-off pattern as
    /// `pending_layer_destroy`, just for a different downstream effect.
    /// Without this, a zone whose last monitor is unplugged never gets its
    /// `MpvInstance`/`ZoneTarget` torn down and leaks until process exit.
    pub pending_monitor_removals: Vec<String>,
}

impl WaylandBackend {
    pub fn new() -> anyhow::Result<(Self, AppData)> {
        let conn = Connection::connect_to_env()?;
        let (globals, event_queue) = wayland_client::globals::registry_queue_init(&conn)?;
        let qh = event_queue.handle();

        let compositor_state = CompositorState::bind(&globals, &qh)
            .map_err(|e| anyhow::anyhow!("compositor does not support wl_compositor: {e}"))?;

        // Fail fast: this daemon only targets wlroots compositors.
        // `LayerShell::bind` both confirms the global's presence and does
        // the real bind (Task 7 only did the presence check via a raw
        // bind, before `AppData` implemented `LayerShellHandler`).
        let layer_shell = LayerShell::bind(&globals, &qh)
            .map_err(|_| anyhow::anyhow!("compositor does not support zwlr_layer_shell_v1 (not wlroots-based?)"))?;

        let registry_state = RegistryState::new(&globals);
        let output_state = OutputState::new(&globals, &qh);

        let backend = WaylandBackend { conn, event_queue };
        let data = AppData {
            registry_state,
            output_state,
            compositor_state,
            layer_shell,
            layer_surfaces: LayerSurfaces::new(),
            monitors: MonitorRegistry::new(),
            render_targets: HashMap::new(),
            egl_core: None,
            pending_layer_destroy: Vec::new(),
            pending_monitor_removals: Vec::new(),
        };
        Ok((backend, data))
    }
}

impl OutputHandler for AppData {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }

    fn new_output(&mut self, _conn: &Connection, qh: &QueueHandle<Self>, output: WlOutput) {
        self.sync_output(output.clone());
        if let Some(info) = self.output_state.info(&output)
            && let Some(name) = info.name
        {
            self.layer_surfaces
                .create(&self.compositor_state, &self.layer_shell, qh, &output, name);
        }
    }

    fn update_output(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, output: WlOutput) {
        self.sync_output(output);
    }

    fn output_destroyed(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, output: WlOutput) {
        if let Some(info) = self.output_state.info(&output)
            && let Some(name) = info.name
        {
            self.monitors.remove(&name);
            // Drop this struct's own `Rc<MonitorSurface>` now, but do
            // NOT destroy the layer surface yet: `render::
            // RenderResources` may still be holding its own clone of the
            // same `Rc` from before this dispatch tick's `sync_monitor_
            // surfaces` call, and destroying the `wl_surface` while that
            // clone's EGL surface is still alive would violate
            // `MonitorSurface`'s drop-order requirement. Queue the name;
            // `main.rs` destroys it right after resyncing, once that
            // stale clone (if any) is guaranteed dropped.
            self.render_targets.remove(&name);
            // Also queue the name for `ZoneManager::remove_monitor` +
            // (if that dissolves a zone) `RenderResources::teardown_zone`
            // -- see `pending_monitor_removals`'s doc comment. Without
            // this, a zone whose last monitor is unplugged keeps its
            // `MpvInstance`/`ZoneTarget` running (and using CPU/GPU)
            // indefinitely, since nothing else ever calls `Set` again for
            // it.
            self.pending_monitor_removals.push(name.clone());
            self.pending_layer_destroy.push(name);
        }
    }
}

impl AppData {
    fn sync_output(&mut self, output: WlOutput) {
        let Some(info) = self.output_state.info(&output) else {
            return;
        };
        let Some(name) = info.name else { return };
        let Some((lw, lh)) = info.logical_size else { return };
        let (lx, ly) = info.logical_position.unwrap_or((0, 0));
        self.monitors.insert(Monitor {
            name,
            logical: Rect {
                x: lx,
                y: ly,
                w: lw,
                h: lh,
            },
        });
    }
}

impl ProvidesRegistryState for AppData {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }
    registry_handlers![OutputState];
}

// Required so `CompositorState::create_surface` can dispatch `wl_surface`
// events (`LayerSurfaces::create` creates one surface per output). None of
// these matter yet for a background layer with no buffer content; Task 9+
// may care about scale/transform changes when rendering.
impl CompositorHandler for AppData {
    fn scale_factor_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &WlSurface,
        _new_factor: i32,
    ) {
    }

    fn transform_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &WlSurface,
        _new_transform: wl_output::Transform,
    ) {
    }

    fn frame(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _surface: &WlSurface, _time: u32) {}

    fn surface_enter(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _surface: &WlSurface, _output: &WlOutput) {
    }

    fn surface_leave(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _surface: &WlSurface, _output: &WlOutput) {
    }
}

impl LayerShellHandler for AppData {
    fn closed(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _layer: &LayerSurface) {}

    fn configure(
        &mut self,
        conn: &Connection,
        _qh: &QueueHandle<Self>,
        layer: &LayerSurface,
        configure: LayerSurfaceConfigure,
        _serial: u32,
    ) {
        let Some(name) = self.layer_surfaces.name_for(layer).map(str::to_owned) else {
            return;
        };

        // Only create the render target once per output; resizing an
        // existing EGL surface on later configure events is out of scope
        // for this task (matches Task 8's create-once-on-new-output scope).
        if self.render_targets.contains_key(&name) {
            return;
        }

        // A `(0, 0)` size means "you choose" -- fall back to the monitor's
        // known logical size.
        let (width, height) = if configure.new_size.0 > 0 && configure.new_size.1 > 0 {
            (configure.new_size.0 as i32, configure.new_size.1 as i32)
        } else if let Some(monitor) = self.monitors.get(&name) {
            (monitor.logical.w, monitor.logical.h)
        } else {
            eprintln!("hyprwalld: configure for {name} has no usable size yet, skipping");
            return;
        };

        let (core, surface) = if let Some(core) = &self.egl_core {
            match core.create_surface(layer.wl_surface(), width, height) {
                Ok(surface) => (Rc::clone(core), surface),
                Err(e) => {
                    eprintln!("hyprwalld: failed to create EGL surface for {name}: {e}");
                    return;
                }
            }
        } else {
            let wl_display_ptr = conn.backend().display_ptr() as *mut std::ffi::c_void;
            match EglCore::new(wl_display_ptr, layer.wl_surface(), width, height) {
                Ok((core, surface)) => (core, surface),
                Err(e) => {
                    eprintln!("hyprwalld: failed to create EGL context for {name}: {e}");
                    return;
                }
            }
        };
        if self.egl_core.is_none() {
            self.egl_core = Some(Rc::clone(&core));
        }

        if let Err(e) = surface.make_current() {
            eprintln!("hyprwalld: eglMakeCurrent failed for {name}: {e}");
            return;
        }
        unsafe {
            surface
                .gl()
                .clear_color(CLEAR_COLOR.0, CLEAR_COLOR.1, CLEAR_COLOR.2, CLEAR_COLOR.3);
            surface.gl().clear(glow::COLOR_BUFFER_BIT);
        }
        if let Err(e) = surface.swap_buffers() {
            eprintln!("hyprwalld: eglSwapBuffers failed for {name}: {e}");
            return;
        }

        self.render_targets.insert(name, Rc::new(surface));
    }
}

delegate_registry!(AppData);
delegate_dispatch2!(AppData);
