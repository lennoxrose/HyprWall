//! Per-output `zwlr_layer_shell_v1` background layer surfaces.
//!
//! Installed API note (SCTK 0.21.1): the brief's sketch assumed
//! `layer_shell.compositor_state()` would hand back a `CompositorState` to
//! create the underlying `wl_surface` from. No such method exists on
//! `LayerShell` in 0.21 -- callers own their own `CompositorState` and pass a
//! `WlSurface` (from `CompositorState::create_surface`) into
//! `LayerShell::create_layer_surface`, which accepts anything
//! `Into<Surface>` (and `WlSurface: Into<Surface>` is provided by SCTK).
//!
//! Creating a `wl_surface` requires the app state to implement
//! `CompositorHandler` (SCTK's `SurfaceData` dispatches through it), on top
//! of the `LayerShellHandler` the brief already calls out. Both route
//! through SCTK's `Dispatch2`/`delegate_dispatch2!` machinery that
//! `wayland/connection.rs` already uses for output tracking -- there is no
//! separate `delegate_layer!` macro in 0.21 (grepped the crate source;
//! doesn't exist).
//!
//! Since `create_layer_surface`'s generic dispatch bounds are only
//! satisfiable by the app's single concrete state type (the blanket
//! `Dispatch` impls `delegate_dispatch2!` generates are keyed to
//! `AppData` specifically, not to an arbitrary type parameter), this API is
//! written directly against `AppData` rather than the brief's generic `D`.

use std::collections::HashMap;

use smithay_client_toolkit::compositor::CompositorState;
use smithay_client_toolkit::shell::wlr_layer::{
    Anchor, KeyboardInteractivity, Layer, LayerShell, LayerSurface,
};
use smithay_client_toolkit::shell::WaylandSurface;
use wayland_client::protocol::wl_output::WlOutput;
use wayland_client::QueueHandle;

use super::connection::AppData;

/// Tracks one background `LayerSurface` per output, keyed by output name
/// (e.g. `"DP-1"`).
#[derive(Default)]
pub struct LayerSurfaces {
    surfaces: HashMap<String, LayerSurface>,
}

impl LayerSurfaces {
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a full-screen background layer surface anchored to all four
    /// edges, exclusive_zone -1, no keyboard interactivity, named
    /// "hyprwall", and commits it. Replaces any existing surface already
    /// tracked under `name`.
    pub fn create(
        &mut self,
        compositor_state: &CompositorState,
        layer_shell: &LayerShell,
        qh: &QueueHandle<AppData>,
        output: &WlOutput,
        name: String,
    ) {
        let wl_surface = compositor_state.create_surface(qh);
        let layer_surface = layer_shell.create_layer_surface(
            qh,
            wl_surface,
            Layer::Background,
            Some("hyprwall"),
            Some(output),
        );
        layer_surface.set_anchor(Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT);
        layer_surface.set_exclusive_zone(-1);
        layer_surface.set_keyboard_interactivity(KeyboardInteractivity::None);
        layer_surface.commit();
        self.surfaces.insert(name, layer_surface);
    }

    pub fn destroy(&mut self, name: &str) {
        self.surfaces.remove(name);
    }

    pub fn get(&self, name: &str) -> Option<&LayerSurface> {
        self.surfaces.get(name)
    }

    /// Finds the output name that owns `layer`, by identity (`LayerSurface`
    /// implements `PartialEq` by comparing the underlying `wl_surface`).
    /// Used by the `configure` callback (Task 9), which is only handed the
    /// `LayerSurface` itself, to know which monitor's render state to
    /// create/update.
    pub fn name_for(&self, layer: &LayerSurface) -> Option<&str> {
        self.surfaces.iter().find(|(_, v)| *v == layer).map(|(k, _)| k.as_str())
    }
}
