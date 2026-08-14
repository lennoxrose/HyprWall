# hyprwalld manual test checklist

Run on a live Hyprland session (`hyprctl version` should succeed).

1. `cargo build --release`
2. `./target/release/hyprwalld &`
3. `./target/release/hyprwallctl monitor-list` — should print your real output names.
4. `./target/release/hyprwallctl set <monitor> <path-to-video>` — that monitor should play the video, looping, muted.
5. Unplug/disable one monitor (`hyprctl keyword monitor <name>,disable` or physically), then re-enable it — daemon should not crash; wallpaper should resume on that monitor without re-running `set`.

   **Safety note:** disabling/re-enabling a monitor via `hyprctl keyword monitor` triggers a real DRM-level mode change on the physical display, which blanks/flashes that monitor — this is an unavoidable hardware side effect of the test, not something hyprwalld causes or can suppress. Before disabling, run `hyprctl monitors -j` and note the exact `x`, `y`, `refreshRate`, and mode for the monitor you're about to touch. When re-enabling, pass that *exact* mode/position string back (e.g. `hyprctl keyword monitor "DP-1,2560x1440@164.83,0x0,1"`) — do **not** use `preferred`/`auto`, since Hyprland may resolve those to a different refresh rate or position than the monitor already had, which both scrambles the desktop layout and forces an *extra* corrective modeset (an extra blank) once you fix it. Do this step only once per test run, and be mindful that repeated display blanking can be uncomfortable or unsafe for photosensitive individuals nearby.
6. Kill and restart the daemon (`kill %1; ./target/release/hyprwalld &`) — the previously-set wallpaper(s) should reappear without re-running `set`.
7. With 2+ monitors: `hyprwallctl set <mon1>,<mon2> <path-to-a-wide-video>` — should show one continuous image spanning both, not the same frame duplicated.
8. `hyprwallctl set <mon1> <path>` after step 7 — should split `<mon1>` back out to its own wallpaper while `<mon2>` keeps playing the spanned video alone.
