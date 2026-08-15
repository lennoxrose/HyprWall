# HyprWall

Local video and image wallpapers for [Hyprland](https://hyprland.org).
`hyprwalld` renders a video or still image as a layer-shell background per
monitor (same approach as `mpvpaper`, via libmpv's render API); `hyprwall-gui`
is a Tauri app for browsing your library and assigning wallpapers without
touching the CLI.

Fully offline, local-files-only. Not a Wallpaper Engine clone, and it will
never reverse-engineer Wallpaper Engine's Workshop or scene format -- that's
a third-party engine with its own name for a reason.

## Features

- Video (`mp4`, `webm`, `mkv`) or still-image wallpapers, per monitor
- Span one wallpaper across multiple monitors as a single zone
- Per-wallpaper zoom, pan, fit mode, brightness/contrast/hue/saturation, and
  volume
- Multiple library folders, watched live -- new files show up without a
  rescan
- Play/pause per monitor, persists across restarts
- Start-on-login and run-in-background toggles
- Socket-activated (`hyprwalld.socket`) or plain `exec-once` launch, your
  choice

## Requirements

- Hyprland (uses `zwlr_layer_shell_v1`)
- `mpv`, `wayland`, `mesa`, `gtk3`, `webkit2gtk-4.1`,
  `libappindicator-gtk3`, `librsvg` at runtime

## Installation

### Quick install (Arch and Arch-based, temporary)

hyprwall isn't on the AUR yet -- see [Future Plans](#future-plans) for why.
Until it is, this script runs the same build the eventual AUR package will
use, straight off GitHub:

```sh
curl -fsSL https://raw.githubusercontent.com/lennoxrose/HyprWall/master/install.sh | bash
```

It clones the repo, then runs `makepkg -si` against `packaging/PKGBUILD`
(the same recipe as the `hyprwall-git` package). Review `install.sh` before
piping it into a shell, as with any installer.

### From source (any distro)

```sh
git clone https://github.com/lennoxrose/HyprWall.git
cd HyprWall

cd src/hyprwall-gui && npm ci && npm run build && cd ../..
cargo build --release --workspace
```

Binaries land in `target/release/`: `hyprwalld`, `hyprwallctl`,
`hyprwall-gui`. Install them, `packaging/hyprwall-gui.desktop`, and
`packaging/systemd/*` wherever your distro expects them -- see
`packaging/PKGBUILD`'s `package()` step for the exact paths.

## Usage

Add to your Hyprland config:

```
exec-once = hyprwalld
```

(Or enable `hyprwalld.socket` via systemd for on-demand, socket-activated
startup instead.)

Then either use the GUI (`hyprwall-gui`) to browse your library and assign
wallpapers, or drive it from the CLI:

```sh
hyprwallctl monitor-list
hyprwallctl set eDP-1 ~/Videos/wallpapers/rain.mp4
hyprwallctl set DP-1,DP-2 ~/Pictures/wallpapers/mountains.png   # spans both as one zone
hyprwallctl pause eDP-1
hyprwallctl play eDP-1
hyprwallctl unset eDP-1
hyprwallctl get eDP-1
```

Config lives at `~/.config/hyprwall/config.toml` and is managed by the GUI
or `hyprwallctl` -- you shouldn't need to hand-edit it.

## Building the GUI's dev environment

```sh
cd src/hyprwall-gui
npm install
npm run tauri dev
```

Requires `hyprwalld` already running (the GUI talks to it over the IPC
socket, it doesn't start it).

## Future Plans

- **Publish on the AUR.** Blocked right now: AUR account registration has
  been closed since the June 2026 malware wave targeting the repository,
  with no announced reopen date. `install.sh` and the `hyprwall-git`
  PKGBUILD in `packaging/` are the bridge until that's viable again --
  same install recipe, just not distributed through AUR infrastructure.
- **A HyprWall-native way to share/discover wallpapers**, built from
  scratch -- not a Workshop clone, not reverse-engineered from anyone
  else's format.
- **A 3D scene creator**, in the spirit of what Wallpaper Engine offers for
  its own engine, but entirely HyprWall's own format and tooling.

Neither of the last two is in progress -- they're direction, not a
roadmap with dates.

## License

MIT, see [LICENSE](LICENSE).
