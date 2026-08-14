# hyprwall-core design

Date: 2026-08-14
Status: approved for planning
Sub-project 1 of 4 (see `CLAUDE.md` for full decomposition)

## Purpose

A daemon that renders video wallpapers as a per-monitor background on
Hyprland, controllable over IPC, plus a thin CLI to drive it. This is
the minimum usable slice of the larger Wallpaper Engine clone: no
scene/particle rendering, no Workshop download, no GUI yet — those are
separate specs (sub-projects 2-4).

## Success criteria

- `hyprwalld` runs on Hyprland, creates a background surface on every
  connected monitor, and plays an assigned video file on each,
  independently.
- `hyprwallctl set <monitor> <path>` changes what's playing on that
  monitor without restarting the daemon.
- Daemon restart restores the last-assigned wallpaper per monitor from
  config, no CLI/GUI resend needed.
- Monitor unplug/replug doesn't crash the daemon; surfaces are torn
  down/recreated to match.

## Architecture

Single daemon process, single-threaded event loop (`calloop`), driving
both the Wayland connection (`smithay-client-toolkit`) and the IPC
socket. One `zwlr_layer_shell_v1` surface per output: layer=BACKGROUND,
anchored full-screen, `exclusive_zone -1`, no keyboard interactivity —
this is what makes it sit behind normal windows instead of being a
focusable layer.

Each surface owns an EGL context bound to its `wl_egl_window`, and its
own `libmpv` render-API instance. mpv renders directly into that
context's framebuffer (the same approach `mpvpaper` uses); the daemon
then calls `eglSwapBuffers` to present. mpv is responsible for decode,
hardware acceleration, seeking, and A/V timing — the daemon does not
touch codecs.

## Threading model

Everything runs on one thread. `libmpv`'s render-context wakeup
callback fires from an mpv-internal thread and must not call GL
directly from there — it just pings a `calloop` event source. The main
loop, on that ping, `eglMakeCurrent`s the corresponding monitor's
context, calls `mpv_render_context_render`, and swaps. With a handful
of monitors this round-robin is cheap; there is no shared GL state
between monitors to fight over since each has its own EGL context.

## Components

- **`hyprwall-ipc` (shared crate)** — wire protocol only. `Command`
  enum (`MonitorList`, `Set { monitor, path }`, `Pause { monitor }`,
  `Play { monitor }`, `Get { monitor }`) with text parse/format. Used
  by both `hyprwalld` and `hyprwallctl` so the protocol can't drift
  between them.
- **`hyprwalld`** — the daemon binary. Internally split by
  responsibility (see `CLAUDE.md` for the exact file layout: Wayland
  connection/output-tracking/layer-surface creation under `wayland/`,
  EGL+mpv+frame-scheduling under `render/`, socket+command-handling
  under `ipc/`, TOML load/save under `config/`, plus a `Monitor` struct
  tying one output to its surface/mpv-instance/current-path).
- **`hyprwallctl`** — CLI binary. Parses argv into a `Command` (reusing
  `hyprwall-ipc`), connects to the socket, sends it, prints the
  response. No logic beyond that.

## Data flow

1. `hyprwallctl set eDP-1 ~/video.mp4` → serializes `Command::Set` →
   writes to `$XDG_RUNTIME_DIR/hyprwall.sock`.
2. Daemon's socket handler reads the line, parses it back into a
   `Command` via `hyprwall-ipc`, looks up the `Monitor` for `eDP-1`.
3. Handler tears down that monitor's existing mpv instance (if any),
   creates a new one, loads the file, updates `Monitor.current_path`,
   writes the change through to the TOML config store.
4. mpv begins decoding; its wakeup callback starts pinging the frame
   scheduler, frames start appearing via the EGL swap path.
5. Handler writes `ok` (or `error: <msg>`) back on the socket;
   `hyprwallctl` prints it and exits.

On startup, before touching Wayland: load `~/.config/hyprwall/
config.toml`. After each output is bound, if config has a saved path
for that output name, issue the same "set" flow internally to restore
it.

## IPC protocol

Unix socket at `$XDG_RUNTIME_DIR/hyprwall.sock`, plain text, one
command per line, one response per line — deliberately hyprctl-shaped
so it's scriptable with `socat`/`nc` without tooling:

```
monitor list                 -> eDP-1\nHDMI-A-1\n...
set <monitor> <path>         -> ok | error: <message>
pause <monitor>              -> ok | error: <message>
play <monitor>                -> ok | error: <message>
get <monitor>                  -> <path> | error: <message>
```

Stale sockets from a crashed prior instance: on startup, attempt to
connect to an existing socket at the path; if that fails (nothing
listening), unlink and bind fresh; if it succeeds, another instance is
already running — log and exit rather than stealing the socket.

## Config

TOML at `~/.config/hyprwall/config.toml`:

```toml
[monitors.eDP-1]
path = "/home/user/Videos/wallpaper.mp4"
```

Written whenever a `set` succeeds; read once at startup. No live
watching of the file — it's a write-through cache of IPC state, not a
hand-edited config in v1.

## Error handling

- Bad file / unsupported codec on `set`: mpv reports the load failure,
  handler responds `error: <mpv message>` over IPC, the monitor keeps
  showing whatever frame it last rendered (or stays blank if nothing
  ever loaded).
- Compositor without `zwlr_layer_shell_v1` support: fail fast at
  startup with a clear stderr message and non-zero exit — this daemon
  only targets wlroots-based compositors (Hyprland).
- Monitor unplugged: `wl_output` removal event tears down that
  monitor's layer surface, EGL context, and mpv instance; config entry
  is left in place (in case it's reconnected) but nothing renders to
  it.
- Monitor plugged in: new `wl_output` triggers surface/context/mpv
  creation; if config has a saved path for that output name, restore
  it immediately.

## Testing

- Unit tests (no Wayland needed): `hyprwall-ipc` command parse/format
  round-trips; `config::store` TOML load/save round-trips.
- Manual integration test on a live Hyprland session (documented in
  the crate README once it exists): start `hyprwalld`, run
  `hyprwallctl set <monitor> <path>` against a known-good video file,
  confirm it renders; unplug/replug a monitor and confirm no crash;
  kill and restart the daemon and confirm the wallpaper reappears
  without re-issuing `set`.
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
