# hyprwall-core design

Date: 2026-08-14
Status: approved for planning
Sub-project 1 of 4 (see `CLAUDE.md` for full decomposition)

## Purpose

A daemon that renders video wallpapers as a per-monitor background on
Hyprland, controllable over IPC, plus a thin CLI to drive it. Monitors
can be assigned individually or grouped into a **zone**, where one
video is stretched across the group's combined screen area instead of
being repeated on each monitor. This is the minimum usable slice of
the larger Wallpaper Engine clone: no scene/particle rendering, no
Workshop download, no GUI yet — those are separate specs (sub-projects
2-4).

## Success criteria

- `hyprwalld` runs on Hyprland, creates a background surface on every
  connected monitor, and plays an assigned video file on each,
  independently by default.
- `hyprwallctl set <monitor> <path>` changes what's playing on that
  monitor without restarting the daemon.
- `hyprwallctl set <monitor1>,<monitor2> <path>` groups those monitors
  into one zone and stretches a single video across their combined
  logical area — one continuous image, not the same frame duplicated
  on each screen.
- Daemon restart restores the last-assigned wallpaper (and any zone
  groupings) per monitor from config, no CLI/GUI resend needed.
- Monitor unplug/replug doesn't crash the daemon; surfaces are torn
  down/recreated to match, and a zone missing a member falls back to
  rendering only on the monitors still present.

## Architecture

Single daemon process, single-threaded event loop (`calloop`), driving
both the Wayland connection (`smithay-client-toolkit`) and the IPC
socket. One `zwlr_layer_shell_v1` surface per output: layer=BACKGROUND,
anchored full-screen, `exclusive_zone -1`, no keyboard interactivity —
this is what makes it sit behind normal windows instead of being a
focusable layer.

A monitor belongs to exactly one **zone**. By default every monitor is
its own zone of one. A zone owns a single `libmpv` render-API instance
and one offscreen EGL render target (FBO + texture) sized to the
zone's bounding box — the union of its member monitors' logical
positions/sizes (from `zxdg_output_manager_v1`, which is why that
protocol is needed alongside plain `wl_output`). mpv decodes and
renders into that shared offscreen texture once per frame.

Each monitor still has its own `zwlr_layer_shell_v1` surface and its
own onscreen EGL context (surfaces are inherently per-output — the
compositor has no notion of a multi-output surface). To present, the
daemon blits the sub-rectangle of the zone's shared texture that
corresponds to that monitor's logical position within the zone's
bounding box, then calls `eglSwapBuffers` on that monitor's context. A
single-monitor zone is the same path with a 1:1 blit — no special case.
mpv remains responsible for decode, hardware acceleration, seeking,
and A/V timing; the daemon never touches codecs.

## Threading model

Everything runs on one thread. `libmpv`'s render-context wakeup
callback fires from an mpv-internal thread and must not call GL
directly from there — it just pings a `calloop` event source. The main
loop, on that ping, renders the owning zone's offscreen target once
(`eglMakeCurrent` on the zone's offscreen context, `mpv_render_context_
render`), then for each member monitor `eglMakeCurrent`s that
monitor's onscreen context, blits the zone texture's relevant
sub-rect, and swaps. With a handful of monitors/zones this round-robin
is cheap; there is no shared GL state fought over since each zone and
each monitor has its own context.

## Components

- **`hyprwall-ipc` (shared crate)** — wire protocol only. `Command`
  enum (`MonitorList`, `Set { monitors: Vec<String>, path }`,
  `Pause { monitor }`, `Play { monitor }`, `Get { monitor }`) with text
  parse/format — `Set` takes one or more monitor names so a single
  command can express both the per-monitor and zone cases. Used by
  both `hyprwalld` and `hyprwallctl` so the protocol can't drift
  between them.
- **`hyprwalld`** — the daemon binary. Internally split by
  responsibility (see `CLAUDE.md` for the exact file layout: Wayland
  connection/output-tracking/logical-geometry/layer-surface creation
  under `wayland/`, EGL+mpv+zone-render-target+frame-scheduling under
  `render/`, socket+command-handling under `ipc/`, TOML load/save
  under `config/`, a `Zone` struct owning the shared mpv instance +
  offscreen target + member list, and a `Monitor` struct tying one
  output to its onscreen surface/context and its owning zone).
- **`hyprwallctl`** — CLI binary. Parses argv into a `Command` (reusing
  `hyprwall-ipc`; a comma-separated monitor list becomes
  `Set { monitors, .. }` with multiple entries), connects to the
  socket, sends it, prints the response. No logic beyond that.

## Data flow

1. `hyprwallctl set eDP-1,HDMI-A-1 ~/video.mp4` → serializes
   `Command::Set { monitors: ["eDP-1","HDMI-A-1"], path }` → writes to
   `$XDG_RUNTIME_DIR/hyprwall.sock`.
2. Daemon's socket handler reads the line, parses it back via
   `hyprwall-ipc`. If any named monitor is currently in a different
   zone, that monitor is removed from its old zone (dissolving it if
   it's now empty) before the new grouping is formed.
3. Handler finds-or-creates the `Zone` for exactly this monitor set,
   recomputes its bounding box from member monitors' logical
   positions/sizes, tears down the zone's existing mpv instance/target
   (if any), creates a fresh one sized to the new bounding box, loads
   the file, and writes the zone (monitor set + path) through to the
   TOML config store.
4. mpv begins decoding into the zone's offscreen target; its wakeup
   callback starts pinging the frame scheduler, which renders the
   zone once and blits per member monitor.
5. Handler writes `ok` (or `error: <msg>`) back on the socket;
   `hyprwallctl` prints it and exits.

On startup, before touching Wayland: load `~/.config/hyprwall/
config.toml`. As outputs are bound, once all members of a saved zone
are present, issue the same "set" flow internally to restore it. A
zone with a missing member still renders on whichever members are
present, cropped to their own logical area within the original
bounding box, until the missing monitor reappears.

## IPC protocol

Unix socket at `$XDG_RUNTIME_DIR/hyprwall.sock`, plain text, one
command per line, one response per line — deliberately hyprctl-shaped
so it's scriptable with `socat`/`nc` without tooling:

```
monitor list                          -> eDP-1\nHDMI-A-1\n...
set <monitor>[,<monitor>...] <path>   -> ok | error: <message>
pause <monitor>                        -> ok | error: <message>
play <monitor>                          -> ok | error: <message>
get <monitor>                            -> <path> | error: <message>
```

`set` with one monitor name assigns that monitor its own zone (the
default). `set` with a comma-separated list groups those monitors into
one zone and stretches a single video across their combined area —
`monitor` here really means "monitor or comma-list", kept as one verb
rather than a separate `group` command so there's one code path for
both cases (a single-monitor zone is not a special case, see
Architecture). `get` and `pause`/`play` still take exactly one monitor
name and act on that monitor's zone.

Stale sockets from a crashed prior instance: on startup, attempt to
connect to an existing socket at the path; if that fails (nothing
listening), unlink and bind fresh; if it succeeds, another instance is
already running — log and exit rather than stealing the socket.

## Config

TOML at `~/.config/hyprwall/config.toml`, one entry per zone:

```toml
[[zones]]
monitors = ["eDP-1"]
path = "/home/user/Videos/wallpaper.mp4"

[[zones]]
monitors = ["HDMI-A-1", "HDMI-A-2"]
path = "/home/user/Videos/panorama.mp4"
```

Written whenever a `set` succeeds (rewriting any zone entries whose
monitor membership changed); read once at startup. No live watching of
the file — it's a write-through cache of IPC state, not a hand-edited
config in v1.

## Error handling

- Bad file / unsupported codec on `set`: mpv reports the load failure,
  handler responds `error: <mpv message>` over IPC, the monitor keeps
  showing whatever frame it last rendered (or stays blank if nothing
  ever loaded).
- Compositor without `zwlr_layer_shell_v1` support: fail fast at
  startup with a clear stderr message and non-zero exit — this daemon
  only targets wlroots-based compositors (Hyprland).
- Monitor unplugged: `wl_output` removal event tears down that
  monitor's layer surface and onscreen EGL context. Its zone keeps
  running (mpv instance and offscreen target untouched) and keeps
  rendering to any remaining member monitors at the zone's original
  bounding box; config entry is left in place in case it's
  reconnected.
- Monitor plugged in: new `wl_output` triggers surface/context
  creation. If it matches a member of a currently-running zone
  (missing since startup, or reconnected after unplug), the daemon
  starts blitting to it immediately — no mpv/zone recreation needed.
  Otherwise, if config has a saved single-monitor zone for that output
  name, restore it.
- `set` names a monitor that doesn't exist: `error: unknown monitor
  <name>`, no zone changes applied.

## Testing

- Unit tests (no Wayland needed): `hyprwall-ipc` command parse/format
  round-trips; `config::store` TOML load/save round-trips.
- Manual integration test on a live Hyprland session (documented in
  the crate README once it exists): start `hyprwalld`, run
  `hyprwallctl set <monitor> <path>` against a known-good video file,
  confirm it renders; unplug/replug a monitor and confirm no crash;
  kill and restart the daemon and confirm the wallpaper reappears
  without re-issuing `set`; on a multi-monitor setup, run
  `hyprwallctl set <mon1>,<mon2> <path>` and confirm the video spans
  continuously across both (not duplicated on each) and picks back up
  as one piece if a member is unplugged/replugged.
- A headless-wlroots CI smoke test (daemon starts, binds a layer
  surface, exits cleanly) is a nice-to-have, not required for this
  sub-project to be considered done.

## Out of scope (deferred to later specs)

- Scene/particle wallpapers (sub-project 2)
- Workshop download/steamcmd integration (sub-project 3)
- GUI (sub-project 4)
- Playlists / scheduled rotation
- Per-wallpaper audio muting policy beyond mpv's default (single global
  mute is fine for now; nothing to design here)
- Pixel-perfect zone stretching across monitors with mismatched scale
  factors or rotation — the bounding box is computed from logical
  (already scale/rotation-adjusted) positions and sizes, so it works,
  but no special-casing beyond that for v1
