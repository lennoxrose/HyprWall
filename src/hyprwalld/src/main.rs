mod app;
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
    // libmpv refuses to initialize outside the "C" locale for LC_NUMERIC
    // (it parses numbers like frame timestamps with the C library's
    // locale-sensitive functions internally); the desktop's LANG/LC_*
    // being e.g. en_US.UTF-8 is enough to trip this, so every MpvInstance
    // this process ever creates depends on setting it back here first.
    // Only LC_NUMERIC is touched -- everything else (message locale,
    // collation, etc.) is left as the user configured it.
    unsafe { libc::setlocale(libc::LC_NUMERIC, c"C".as_ptr()) };

    let (backend, app_data) = WaylandBackend::new()?;
    let config_path = hyprwall_config::store::default_config_path();
    // One informative diagnostic if the file exists but fails to parse; the
    // `Config` value itself is discarded here -- `restore_saved_zones` below
    // re-reads the file fresh on every relevant Wayland event instead of
    // caching this snapshot (see its doc comment for why a one-time load
    // isn't enough), so this call exists only for the startup message. A
    // missing file already returns `Config::default()` from `load` itself
    // (not an error), so this only fires for a genuinely corrupt file.
    if let Err(e) = hyprwall_config::store::load(&config_path) {
        eprintln!("hyprwalld: failed to load {}: {e} (starting with no saved zones)", config_path.display());
    }

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
            // Reflect any monitor unplugs into `ZoneManager`'s bookkeeping,
            // and tear down playback for any zone that dissolved as a
            // result (its last member monitor just disappeared). A zone
            // that still has other monitors after this stays alive and
            // keeps playing to them -- see `ZoneManager::remove_monitor`.
            for name in daemon.app_data.pending_monitor_removals.drain(..).collect::<Vec<_>>() {
                if let Some(zone_id) = daemon.state.zones.remove_monitor(&name) {
                    daemon.render.teardown_zone(zone_id);
                }
            }
            for name in daemon.app_data.pending_layer_destroy.drain(..).collect::<Vec<_>>() {
                daemon.app_data.layer_surfaces.destroy(&name);
            }
            // Restore any saved zone whose monitors just became available --
            // covers both the initial startup burst of outputs (this closure
            // also runs for the very first dispatch, after the queue's
            // already-pending globals are processed above) and any later
            // hotplug replug. See `restore_saved_zones`.
            restore_saved_zones(daemon);
            result
        })
        .map_err(|e| anyhow::anyhow!("failed to insert wayland source into event loop: {}", e.error))?;

    let socket_path = hyprwall_ipc::default_socket_path();
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

/// Restores any zone from `config.toml` whose member monitors are all now
/// known, and that isn't already covered by a live zone. Called after every
/// Wayland dispatch tick, so it naturally covers both:
/// - **Startup**: at daemon launch every saved zone is unclaimed, so each one
///   restores as soon as its monitors' `wl_output`s have been processed.
/// - **Hotplug replug**: a monitor whose zone was fully torn down while
///   unplugged (its last/only member disappearing dissolves the zone --
///   see `ZoneManager::remove_monitor`) is "unclaimed" again, so replugging
///   it restores the same saved assignment without the user re-running `set`.
///
/// Re-reads `config.toml` from disk on every call rather than caching a
/// startup snapshot: config-on-disk isn't static across the daemon's
/// lifetime -- every successful `Command::Set` rewrites it (see `ipc::
/// handler::persist`) -- and a zone `set` live during this session (not
/// present in the file at startup) must be just as restorable on a later
/// replug as one that was already there when the daemon launched. The file
/// is small and this only runs on Wayland structural events (output
/// add/remove, layer surface configure), not per frame, so re-reading it
/// every time is cheap. A load failure (missing or corrupt file) is treated
/// as "no saved zones" silently -- `main()` already logged once at startup
/// for a corrupt file; repeating that on every dispatch tick would spam
/// stderr for as long as the file stays broken.
///
/// "Already covered" checks two things per candidate zone, so this never
/// clobbers a zone that's still alive: `ZoneManager::zone_for_monitor` (the
/// normal case) and `RenderResources::is_monitor_live` (the case where a
/// monitor was unplugged from a *surviving* multi-monitor zone --
/// `remove_monitor` strips it from `ZoneManager`'s bookkeeping even though
/// the zone and its `RenderResources` cache are still very much alive for the
/// other member(s); without this second check, replugging that monitor would
/// restore a brand new zone from disk on top of the live one instead of
/// silently resuming the existing zone's blit, contradicting the spec's "no
/// mpv/zone recreation needed" for that case).
///
/// Uses `ipc::handler::apply_zone`, the exact same "apply to `ZoneManager`,
/// then wire up real playback via `RenderResources`" logic `Command::Set`
/// uses -- deliberately without persisting (see `apply_zone`'s doc comment):
/// re-persisting what was just read back from the same file is redundant at
/// best, and if only some of the file's zones are restorable this tick
/// (others still waiting on a monitor), persisting now would rebuild the
/// file from `ZoneManager`'s current partial state and drop the rest.
fn restore_saved_zones(daemon: &mut Daemon) {
    let saved = hyprwall_config::store::load(&daemon.state.config_path).unwrap_or_default();
    if saved.zones.is_empty() {
        return;
    }
    for cfg in &saved.zones {
        let already_covered = cfg.monitors.iter().any(|m| {
            daemon.state.zones.zone_for_monitor(m).is_some() || daemon.render.is_monitor_live(m)
        });
        if already_covered {
            continue;
        }
        let ready = cfg.monitors.iter().all(|m| daemon.app_data.monitors.contains(m));
        if !ready {
            continue;
        }
        // Same refresh `handle_connection` does before every IPC command --
        // `ZoneManager::apply_set` needs an up-to-date registry to validate
        // monitor names and compute the zone's bounding box.
        daemon.state.registry = daemon.app_data.monitors.clone();
        if let Err(e) = ipc::handler::apply_zone(&mut daemon.state, &mut daemon.render, &cfg.monitors, &cfg.path) {
            eprintln!("hyprwalld: failed to restore saved zone {:?}: {e}", cfg.monitors);
        }
    }
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
