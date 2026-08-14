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
//! Event pump: `WaylandBackend::event_queue.blocking_dispatch(&mut AppData)`
//! is the dispatch entrypoint. `main.rs` calls it in a loop on a background
//! thread; each successful dispatch may have run `AppData`'s `OutputHandler`
//! callbacks, which synchronously update `AppData.monitors` in place.

use smithay_client_toolkit::compositor::{CompositorHandler, CompositorState};
use smithay_client_toolkit::output::{OutputHandler, OutputState};
use smithay_client_toolkit::registry::{ProvidesRegistryState, RegistryState};
use smithay_client_toolkit::shell::wlr_layer::{
    LayerShell, LayerShellHandler, LayerSurface, LayerSurfaceConfigure,
};
use smithay_client_toolkit::{delegate_dispatch2, delegate_registry, registry_handlers};
use wayland_client::protocol::wl_output::{self, WlOutput};
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_client::{Connection, QueueHandle};

use super::output::LayerSurfaces;
use crate::monitor::{Monitor, Rect};
use crate::monitor_registry::MonitorRegistry;

pub struct WaylandBackend {
    pub conn: Connection,
    pub event_queue: wayland_client::EventQueue<AppData>,
    pub qh: QueueHandle<AppData>,
}

pub struct AppData {
    pub registry_state: RegistryState,
    pub output_state: OutputState,
    pub compositor_state: CompositorState,
    pub layer_shell: LayerShell,
    pub layer_surfaces: LayerSurfaces,
    pub monitors: MonitorRegistry,
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
        let layer_shell = LayerShell::bind(&globals, &qh).map_err(|_| {
            anyhow::anyhow!("compositor does not support zwlr_layer_shell_v1 (not wlroots-based?)")
        })?;

        let registry_state = RegistryState::new(&globals);
        let output_state = OutputState::new(&globals, &qh);

        let backend = WaylandBackend { conn, event_queue, qh };
        let data = AppData {
            registry_state,
            output_state,
            compositor_state,
            layer_shell,
            layer_surfaces: LayerSurfaces::new(),
            monitors: MonitorRegistry::new(),
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
        if let Some(info) = self.output_state.info(&output) {
            if let Some(name) = info.name {
                self.layer_surfaces.create(
                    &self.compositor_state,
                    &self.layer_shell,
                    qh,
                    &output,
                    name,
                );
            }
        }
    }

    fn update_output(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, output: WlOutput) {
        self.sync_output(output);
    }

    fn output_destroyed(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, output: WlOutput) {
        if let Some(info) = self.output_state.info(&output) {
            if let Some(name) = info.name {
                self.monitors.remove(&name);
                self.layer_surfaces.destroy(&name);
            }
        }
    }
}

impl AppData {
    fn sync_output(&mut self, output: WlOutput) {
        let Some(info) = self.output_state.info(&output) else { return };
        let Some(name) = info.name else { return };
        let Some((lw, lh)) = info.logical_size else { return };
        let (lx, ly) = info.logical_position.unwrap_or((0, 0));
        self.monitors.insert(Monitor { name, logical: Rect { x: lx, y: ly, w: lw, h: lh } });
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

    fn surface_enter(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &WlSurface,
        _output: &WlOutput,
    ) {
    }

    fn surface_leave(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &WlSurface,
        _output: &WlOutput,
    ) {
    }
}

impl LayerShellHandler for AppData {
    fn closed(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _layer: &LayerSurface) {}

    fn configure(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _layer: &LayerSurface,
        _configure: LayerSurfaceConfigure,
        _serial: u32,
    ) {
        // Task 9 uses this to (re)size the EGL surface; nothing to do yet.
    }
}

delegate_registry!(AppData);
delegate_dispatch2!(AppData);
