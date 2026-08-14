# HyprWall

Wallpaper Engine clone for Hyprland, written in Rust. Plays video and
Workshop "scene" wallpapers as a layer-shell background, with a Tauri
GUI for browsing/assigning them.

## Sub-project decomposition (build order)

The full app is too large for one spec. Each of these is its own
design doc under `docs/superpowers/specs/` and its own implementation
plan. Do not start a later one until the current one is usable.

1. **hyprwall-core** — daemon: layer-shell surface per monitor, video
   playback via libmpv render API, Unix-socket IPC, `hyprwallctl` CLI.
   *Status: in design.*
2. **hyprwall-scene** — Wallpaper Engine scene format (layers,
   particles, effects) parser + renderer, bolted onto the core daemon's
   render loop.
3. **hyprwall-workshop** — steamcmd wrapper + local library scanner for
   downloading/indexing Workshop items (appid 431960). Blocked on
   confirming Steam account ownership of Wallpaper Engine; build/test
   against local fixtures until then.
4. **hyprwall-gui** — Tauri app (TypeScript/TSX frontend) for browsing
   the library, editing playlists/monitor assignment, talking to the
   daemon over its IPC socket.

## Architecture conventions (all sub-projects)

- **Deep folders, one file = one job.** Every file has a single,
  narrow responsibility — no god files, no "misc utils" dumping
  grounds. If a file is doing two unrelated things, split it. Prefer a
  deeper directory tree with small files over a flat one with large
  files.
- **Rust workspace**, one crate per sub-project/binary, shared logic
  factored into its own crate rather than duplicated (e.g. the IPC
  wire protocol is a crate both the daemon and the CLI depend on, not
  code copied into each).
- **Frontend is TypeScript/TSX** (React) inside Tauri — no plain JS, no
  other frontend framework.

### hyprwall-core layout (reference shape for later crates too)

```
crates/
  hyprwall-ipc/            # shared: wire protocol, used by daemon + ctl
    src/
      lib.rs
      command.rs           # Command enum + parse
      response.rs          # Response enum + format
  hyprwalld/                # daemon binary
    src/
      main.rs               # entrypoint only: build App, run event loop
      app.rs                # top-level orchestration
      wayland/
        connection.rs       # wl connection/registry bootstrap
        output.rs           # monitor add/remove tracking
        xdg_output.rs       # logical position/size (zxdg_output_manager_v1)
        layer_surface.rs    # per-output zwlr_layer_shell surface
      render/
        egl_context.rs      # EGL context creation/binding
        mpv_instance.rs     # one libmpv render-API instance
        zone_target.rs      # zone's offscreen FBO/texture + blit-to-monitor
        frame_scheduler.rs  # mpv wakeup -> calloop -> render dispatch
      ipc/
        socket.rs           # unix socket listen/accept, stale handling
        handler.rs          # parsed Command -> app action
      config/
        model.rs            # TOML config structs (zones)
        store.rs            # load/save
      zone.rs                # Zone: monitor set + bounding box + mpv + target
      monitor.rs              # Monitor: output + logical rect + surface + zone id
  hyprwallctl/               # thin CLI binary
    src/
      main.rs                # arg parse -> client -> print reply
      client.rs              # socket connect/send/receive
```

## Tech choices (hyprwall-core)

- Wayland client: `smithay-client-toolkit` + `calloop` event loop
- Layer surface: `zwlr_layer_shell_v1`, layer=BACKGROUND, full-screen
  anchor, exclusive_zone -1, no keyboard interactivity
- Video: `libmpv` render API into an EGL context per monitor (same
  pattern as `mpvpaper`)
- IPC: Unix socket at `$XDG_RUNTIME_DIR/hyprwall.sock`, plain-text
  hyprctl-style protocol
- Config: TOML at `~/.config/hyprwall/config.toml`
