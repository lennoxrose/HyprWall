mod app;
mod config;
mod ipc;
mod monitor;
mod monitor_registry;
mod render;
mod wayland;
mod zone;
mod zone_manager;

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;

use calloop::generic::Generic;
use calloop::{EventLoop, Interest, LoopHandle, Mode, PostAction};
use calloop_wayland_source::WaylandSource;

use app::AppState;
use render::RenderResources;
use wayland::connection::{AppData, WaylandBackend};

/// Everything the single-threaded Wayland/IPC/render event loop shares.
/// This *is* calloop's `Data` type parameter, so every inserted source's
/// callback gets `&mut Daemon`. Kept as three separate structs rather than
/// one flat bag of fields on purpose: `app_data` is Wayland dispatch state
/// (owns the layer surfaces and, transitively, the canonical per-monitor EGL
/// surfaces), `state` is the pure IPC/zone bookkeeping Task 5 already tested
/// headless, and `render` is the GL/mpv state Task 10 adds. `render` mirrors
/// what it needs from `app_data` (see `RenderResources::sync_monitor_surfaces`)
/// rather than `app_data` and `render` both being handed to `handle_command`,
/// which keeps `handle_command`'s signature exactly the one extra parameter
/// the Task 10 brief calls for.
struct Daemon {
    app_data: AppData,
    state: AppState,
    render: RenderResources,
}

fn main() -> anyhow::Result<()> {
    let (backend, app_data) = WaylandBackend::new()?;
    let config_path = config::store::default_config_path();

    let mut event_loop: EventLoop<Daemon> = EventLoop::try_new()?;
    let loop_handle = event_loop.handle();

    let wayland_source = WaylandSource::new(backend.conn, backend.event_queue);
    // Not `?`: `InsertError<WaylandSource<AppData>>` isn't `Send` (`AppData`
    // holds `Rc`s), so it can't go through anyhow's blanket `From<E: Error +
    // Send + Sync>` conversion. Extract just the `Send + Sync` `calloop::
    // Error` and drop the (non-Send) returned source instead.
    loop_handle
        .insert_source(wayland_source, |_, queue, daemon: &mut Daemon| {
            let result = queue.dispatch_pending(&mut daemon.app_data);
            // Refresh `render`'s mirrored view of per-monitor surfaces/the
            // shared EGL core right after Wayland dispatch may have changed
            // them (new output configured, or one destroyed), and only then
            // drain any layer-surface destroys that were queued -- see
            // `AppData::pending_layer_destroy`'s doc comment for why this
            // must happen in this order.
            daemon.render.sync_monitor_surfaces(&daemon.app_data);
            for name in daemon.app_data.pending_layer_destroy.drain(..).collect::<Vec<_>>() {
                daemon.app_data.layer_surfaces.destroy(&name);
            }
            result
        })
        .map_err(|e| anyhow::anyhow!("failed to insert wayland source into event loop: {}", e.error))?;

    let socket_path = ipc::socket::socket_path();
    let listener = ipc::socket::bind_listener(&socket_path)?;
    listener.set_nonblocking(true)?;
    println!("hyprwalld listening on {}", socket_path.display());

    loop_handle
        .insert_source(
            Generic::new(listener, Interest::READ, Mode::Level),
            |_, listener, daemon: &mut Daemon| {
                loop {
                    match listener.accept() {
                        Ok((mut conn, _)) => handle_connection(&mut conn, daemon),
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                        Err(e) => eprintln!("hyprwalld: connection error: {e}"),
                    }
                }
                Ok(PostAction::Continue)
            },
        )
        .map_err(|e| anyhow::anyhow!("failed to insert IPC listener into event loop: {}", e.error))?;

    let mut daemon =
        Daemon { app_data, state: AppState::new(Default::default(), config_path), render: RenderResources::new() };

    loop {
        event_loop.dispatch(None, &mut daemon)?;
        wire_pending_zone_wakeups(&loop_handle, &mut daemon);
    }
}

/// Reads one command off `conn`, dispatches it, writes the response. The IPC
/// wire protocol (see `hyprwall-ipc`) is one command per connection, matching
/// `hyprwallctl`'s one-shot-process-per-invocation model.
fn handle_connection(conn: &mut UnixStream, daemon: &mut Daemon) {
    let mut line = String::new();
    if conn.read_to_string(&mut line).is_err() {
        return;
    }
    // `AppState::registry` is a snapshot; refresh it from the live Wayland
    // state before every command so `monitor-list`/`set` see current
    // monitors without `AppState` itself needing to borrow `AppData`.
    daemon.state.registry = daemon.app_data.monitors.clone();
    let response = match hyprwall_ipc::parse_command(&line) {
        Ok(cmd) => ipc::handler::handle_command(&mut daemon.state, cmd, &mut daemon.render),
        Err(e) => hyprwall_ipc::Response::Error(e.to_string()),
    };
    let _ = conn.write_all(response.to_wire().as_bytes());
}

/// Wires a fresh calloop ping for every zone `handle_command` just created a
/// new `MpvInstance` for, so its wakeup callback can reach the render loop.
/// See `RenderResources::needs_wakeup_wiring` and `render::frame_scheduler`.
fn wire_pending_zone_wakeups(loop_handle: &LoopHandle<'static, Daemon>, daemon: &mut Daemon) {
    let pending: Vec<u64> = daemon.render.needs_wakeup_wiring.drain(..).collect();
    for zone_id in pending {
        let result = render::frame_scheduler::register(loop_handle, move |daemon: &mut Daemon| {
            daemon.render.render_and_present(zone_id, &daemon.app_data.monitors);
        });
        match result {
            Ok(ping) => {
                if let Some(zp) = daemon.render.zone_playback.get_mut(&zone_id) {
                    zp.mpv.set_wakeup_callback(move || ping.ping());
                }
            }
            Err(e) => eprintln!("hyprwalld: failed to wire zone {zone_id} wakeup: {e}"),
        }
    }
}
