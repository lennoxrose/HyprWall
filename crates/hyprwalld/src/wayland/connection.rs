//! Wayland connection bootstrap and output tracking.
//!
//! Installed API note (SCTK 0.21.1, wayland-client 0.31.15): the version
//! targeted by the original task sketch (SCTK ~0.19) exposed a
//! `delegate_output!` macro. That macro no longer exists in 0.21; SCTK now
//! routes all its internal event handling through a `Dispatch2` trait and a
//! single blanket `delegate_dispatch2!(AppData)` macro that covers every
//! `Dispatch2` impl SCTK's modules register (including `OutputState`'s
//! handling of `wl_output` and `zxdg_output_v1`). `delegate_registry!` is
//! unchanged. See SCTK's own `examples/list_outputs.rs` for the same
//! `delegate_registry!` + `delegate_dispatch2!` pairing this file uses.
//!
//! `zwlr_layer_shell_v1` is bound directly (not via SCTK's `shell::wlr_layer`
//! wrapper) because that wrapper's `LayerShell::bind` requires the app to
//! already implement `LayerShellHandler`, which is machinery this task does
//! not need yet -- Task 7 only has to confirm the global exists. Task 8 can
//! switch to the `LayerShell` wrapper once it actually creates surfaces.
//!
//! Event pump: `WaylandBackend::event_queue.blocking_dispatch(&mut AppData)`
//! is the dispatch entrypoint. `main.rs` calls it in a loop on a background
//! thread; each successful dispatch may have run `AppData`'s `OutputHandler`
//! callbacks, which synchronously update `AppData.monitors` in place.

use smithay_client_toolkit::output::{OutputHandler, OutputState};
use smithay_client_toolkit::registry::{ProvidesRegistryState, RegistryState};
use smithay_client_toolkit::{delegate_dispatch2, delegate_registry, registry_handlers};
use wayland_client::protocol::wl_output::WlOutput;
use wayland_client::{delegate_noop, Connection, QueueHandle};
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1::ZwlrLayerShellV1;

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
    pub monitors: MonitorRegistry,
}

impl WaylandBackend {
    pub fn new() -> anyhow::Result<(Self, AppData)> {
        let conn = Connection::connect_to_env()?;
        let (globals, event_queue) = wayland_client::globals::registry_queue_init(&conn)?;
        let qh = event_queue.handle();

        // Fail fast: this daemon only targets wlroots compositors. We only
        // need to confirm the global is present here; Task 8 does the real
        // bind (via SCTK's `LayerShell` wrapper) when it creates surfaces.
        globals
            .bind::<ZwlrLayerShellV1, AppData, ()>(&qh, 1..=4, ())
            .map_err(|_| {
                anyhow::anyhow!(
                    "compositor does not support zwlr_layer_shell_v1 (not wlroots-based?)"
                )
            })?;

        let registry_state = RegistryState::new(&globals);
        let output_state = OutputState::new(&globals, &qh);

        let backend = WaylandBackend { conn, event_queue, qh };
        let data = AppData { registry_state, output_state, monitors: MonitorRegistry::new() };
        Ok((backend, data))
    }
}

impl OutputHandler for AppData {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }

    fn new_output(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, output: WlOutput) {
        self.sync_output(output);
    }

    fn update_output(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, output: WlOutput) {
        self.sync_output(output);
    }

    fn output_destroyed(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, output: WlOutput) {
        if let Some(info) = self.output_state.info(&output) {
            if let Some(name) = info.name {
                self.monitors.remove(&name);
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

// zwlr_layer_shell_v1 has no events; we only bind it to check for its
// presence in `WaylandBackend::new`, so ignore anything it might send.
delegate_noop!(AppData: ignore ZwlrLayerShellV1);

delegate_registry!(AppData);
delegate_dispatch2!(AppData);
