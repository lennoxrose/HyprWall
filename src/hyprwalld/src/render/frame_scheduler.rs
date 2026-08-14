//! Wires an mpv render context's wakeup callback to the main calloop loop.
//!
//! The wakeup callback registered via `MpvInstance::set_wakeup_callback`
//! fires from an mpv-internal thread and must not touch GL or call back into
//! mpv. It can, however, safely call `calloop::ping::Ping::ping`, which is
//! exactly the "producer-thread-wakes-consumer-thread" primitive calloop
//! ships for this (`calloop::ping::make_ping`, checked against the installed
//! calloop 0.14.4 source rather than assumed): a `Ping` is a cheap `Clone`
//! handle usable from any thread, and its paired `PingSource`, once inserted
//! into the loop, produces one coalesced `()` event per dispatch no matter
//! how many times `ping()` was called in between -- "redraw with the latest
//! frame," never "redraw once per frame that was ever decoded," which is the
//! right semantics for a video wallpaper.
//!
//! `PingSource` also removes itself from the loop automatically once every
//! clone of its `Ping` has been dropped. A zone's `Ping` clone lives only
//! inside its `MpvInstance`'s wakeup-callback closure (via `set_update_callback`,
//! which owns the closure), so tearing down a zone (dropping its
//! `MpvInstance`, which drops its `RenderContext`, which drops that closure)
//! is enough to clean the ping source up -- nothing here needs to track or
//! hand back a `RegistrationToken` to remove by hand.

use calloop::LoopHandle;
use calloop::ping::{Ping, make_ping};

/// Registers a fresh ping source on `loop_handle`. Returns the `Ping` half:
/// hand it to `MpvInstance::set_wakeup_callback` (wrapped in a closure that
/// just calls `ping.ping()`) so mpv's wakeup fires it. `on_ping` runs on the
/// loop's own thread every time the source fires -- this is where the actual
/// render + blit + swap sequence for one zone belongs.
pub fn register<'l, Data: 'l>(
    loop_handle: &LoopHandle<'l, Data>,
    mut on_ping: impl FnMut(&mut Data) + 'l,
) -> anyhow::Result<Ping> {
    let (ping, source) = make_ping()?;
    loop_handle
        .insert_source(source, move |(), &mut (), data| on_ping(data))
        .map_err(|e| anyhow::anyhow!("failed to register zone ping source: {e}"))?;
    Ok(ping)
}
