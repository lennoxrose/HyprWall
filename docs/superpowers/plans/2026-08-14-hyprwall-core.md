# hyprwall-core Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A daemon (`hyprwalld`) that plays video wallpapers as a per-monitor Hyprland background over `zwlr_layer_shell_v1`, controllable via a CLI (`hyprwallctl`) over a Unix socket, with monitors groupable into zones that stretch one video across their combined area.

**Architecture:** Three-crate Cargo workspace. `hyprwall-ipc` is the shared wire protocol (pure, no I/O). `hyprwalld` is the daemon: pure logic (config, zone/monitor bookkeeping, command handling) is built and unit-tested first, in isolation from Wayland/EGL/mpv; the Wayland output tracking, layer-shell surfaces, EGL contexts, and libmpv render pipeline are wired in afterward and verified manually on a live Hyprland session. `hyprwallctl` is a thin CLI over the same socket.

**Tech Stack:** Rust, `smithay-client-toolkit` + `wayland-client` + `calloop` (Wayland/event loop), `khronos-egl` + `wayland-egl` (EGL), `libmpv2` (mpv render API), `serde`/`toml` (config), `clap` (CLI args).

**Spec:** `docs/superpowers/specs/2026-08-14-hyprwall-core-design.md`

## Global Constraints

- Targets wlroots-based compositors only (requires `zwlr_layer_shell_v1`); fail fast with a clear message if the global isn't present.
- IPC socket is exactly `$XDG_RUNTIME_DIR/hyprwall.sock`, plain-text, one command per line, one response per connection (spec: IPC protocol section).
- Config is exactly `~/.config/hyprwall/config.toml`, `[[zones]]` array-of-tables format (spec: Config section).
- One file = one job, deep folders over flat ones (`CLAUDE.md`).
- Do not implement scene/particle rendering, Workshop download, GUI, or playlists — out of scope for this plan (spec: Out of scope section).
- Crate versions: use `cargo add <crate>` to pull latest compatible versions rather than hand-typing version numbers — these crates move fast and pinned numbers in this plan would go stale. If example code in a task doesn't match the installed version's API, check `cargo doc -p <crate> --open` and adjust; this is expected for Task 7 onward.

---

## Task 1: Workspace scaffold + `hyprwall-ipc` protocol

**Files:**
- Create: `Cargo.toml` (workspace root)
- Create: `.gitignore`
- Create: `crates/hyprwall-ipc/Cargo.toml`
- Create: `crates/hyprwall-ipc/src/lib.rs`
- Create: `crates/hyprwall-ipc/src/command.rs`
- Create: `crates/hyprwall-ipc/src/response.rs`

**Interfaces:**
- Produces: `hyprwall_ipc::Command` (enum: `MonitorList`, `Set { monitors: Vec<String>, path: String }`, `Pause { monitor: String }`, `Play { monitor: String }`, `Get { monitor: String }`), `hyprwall_ipc::ParseError`, `hyprwall_ipc::parse_command(line: &str) -> Result<Command, ParseError>`, `Command::to_wire(&self) -> String`, `hyprwall_ipc::Response` (enum: `Ok`, `Path(String)`, `MonitorList(Vec<String>)`, `Error(String)`), `Response::to_wire(&self) -> String`, `hyprwall_ipc::parse_response(text: &str) -> Response`.

- [ ] **Step 1: Scaffold the workspace**

```bash
mkdir -p crates/hyprwall-ipc/src
```

Write `Cargo.toml`:

```toml
[workspace]
resolver = "2"
members = ["crates/hyprwall-ipc", "crates/hyprwalld", "crates/hyprwallctl"]
```

Write `.gitignore`:

```
/target
```

Write `crates/hyprwall-ipc/Cargo.toml`:

```toml
[package]
name = "hyprwall-ipc"
version = "0.1.0"
edition = "2021"

[dependencies]
```

Write `crates/hyprwall-ipc/src/lib.rs`:

```rust
mod command;
mod response;

pub use command::{parse_command, Command, ParseError};
pub use response::{parse_response, Response};
```

Note: `crates/hyprwalld` and `crates/hyprwallctl` don't exist yet, so the workspace won't build until Task 2 and Task 4 create them. Skip `cargo build` until then; this step just lays out files.

- [ ] **Step 2: Write the failing tests for `Command` parse/format**

Write `crates/hyprwall-ipc/src/command.rs`:

```rust
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    MonitorList,
    Set { monitors: Vec<String>, path: String },
    Pause { monitor: String },
    Play { monitor: String },
    Get { monitor: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    Empty,
    UnknownVerb(String),
    MissingArg { verb: &'static str, arg: &'static str },
    EmptyMonitorList,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::Empty => write!(f, "empty command"),
            ParseError::UnknownVerb(v) => write!(f, "unknown command: {v}"),
            ParseError::MissingArg { verb, arg } => {
                write!(f, "{verb} requires <{arg}>")
            }
            ParseError::EmptyMonitorList => write!(f, "monitor list is empty"),
        }
    }
}

pub fn parse_command(line: &str) -> Result<Command, ParseError> {
    let line = line.trim();
    if line.is_empty() {
        return Err(ParseError::Empty);
    }
    let mut top = line.splitn(2, ' ');
    let verb = top.next().unwrap();
    let rest = top.next().unwrap_or("").trim();

    match verb {
        "monitor" if rest == "list" => Ok(Command::MonitorList),
        "monitor" => Err(ParseError::UnknownVerb(format!("monitor {rest}"))),
        "set" => {
            let mut args = rest.splitn(2, ' ');
            let monitors_csv = args.next().unwrap_or("");
            let path = args.next().unwrap_or("").trim();
            if monitors_csv.is_empty() {
                return Err(ParseError::MissingArg { verb: "set", arg: "monitor" });
            }
            if path.is_empty() {
                return Err(ParseError::MissingArg { verb: "set", arg: "path" });
            }
            let monitors: Vec<String> = monitors_csv
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .collect();
            if monitors.is_empty() {
                return Err(ParseError::EmptyMonitorList);
            }
            Ok(Command::Set { monitors, path: path.to_string() })
        }
        "pause" if !rest.is_empty() => Ok(Command::Pause { monitor: rest.to_string() }),
        "pause" => Err(ParseError::MissingArg { verb: "pause", arg: "monitor" }),
        "play" if !rest.is_empty() => Ok(Command::Play { monitor: rest.to_string() }),
        "play" => Err(ParseError::MissingArg { verb: "play", arg: "monitor" }),
        "get" if !rest.is_empty() => Ok(Command::Get { monitor: rest.to_string() }),
        "get" => Err(ParseError::MissingArg { verb: "get", arg: "monitor" }),
        other => Err(ParseError::UnknownVerb(other.to_string())),
    }
}

impl Command {
    pub fn to_wire(&self) -> String {
        match self {
            Command::MonitorList => "monitor list".to_string(),
            Command::Set { monitors, path } => {
                format!("set {} {}", monitors.join(","), path)
            }
            Command::Pause { monitor } => format!("pause {monitor}"),
            Command::Play { monitor } => format!("play {monitor}"),
            Command::Get { monitor } => format!("get {monitor}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_monitor_list() {
        assert_eq!(parse_command("monitor list"), Ok(Command::MonitorList));
    }

    #[test]
    fn parses_single_monitor_set() {
        assert_eq!(
            parse_command("set eDP-1 /home/u/video.mp4"),
            Ok(Command::Set {
                monitors: vec!["eDP-1".to_string()],
                path: "/home/u/video.mp4".to_string()
            })
        );
    }

    #[test]
    fn parses_multi_monitor_set_as_zone() {
        assert_eq!(
            parse_command("set eDP-1,HDMI-A-1 /home/u/pano.mp4"),
            Ok(Command::Set {
                monitors: vec!["eDP-1".to_string(), "HDMI-A-1".to_string()],
                path: "/home/u/pano.mp4".to_string()
            })
        );
    }

    #[test]
    fn parses_pause_play_get() {
        assert_eq!(parse_command("pause eDP-1"), Ok(Command::Pause { monitor: "eDP-1".to_string() }));
        assert_eq!(parse_command("play eDP-1"), Ok(Command::Play { monitor: "eDP-1".to_string() }));
        assert_eq!(parse_command("get eDP-1"), Ok(Command::Get { monitor: "eDP-1".to_string() }));
    }

    #[test]
    fn rejects_empty_line() {
        assert_eq!(parse_command(""), Err(ParseError::Empty));
        assert_eq!(parse_command("   "), Err(ParseError::Empty));
    }

    #[test]
    fn rejects_unknown_verb() {
        assert_eq!(parse_command("frobnicate x"), Err(ParseError::UnknownVerb("frobnicate".to_string())));
    }

    #[test]
    fn rejects_set_missing_path() {
        assert_eq!(
            parse_command("set eDP-1"),
            Err(ParseError::MissingArg { verb: "set", arg: "path" })
        );
    }

    #[test]
    fn rejects_set_empty_monitor_list() {
        assert_eq!(parse_command("set , /x.mp4"), Err(ParseError::EmptyMonitorList));
    }

    #[test]
    fn to_wire_round_trips_set() {
        let cmd = Command::Set {
            monitors: vec!["eDP-1".to_string(), "HDMI-A-1".to_string()],
            path: "/x.mp4".to_string(),
        };
        assert_eq!(parse_command(&cmd.to_wire()), Ok(cmd));
    }
}
```

- [ ] **Step 3: Write the failing tests for `Response` parse/format**

Write `crates/hyprwall-ipc/src/response.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Response {
    Ok,
    Path(String),
    MonitorList(Vec<String>),
    Error(String),
}

impl Response {
    pub fn to_wire(&self) -> String {
        match self {
            Response::Ok => "ok".to_string(),
            Response::Path(p) => p.clone(),
            Response::MonitorList(names) => names.join("\n"),
            Response::Error(msg) => format!("error: {msg}"),
        }
    }
}

/// Best-effort parse of a response body back into a `Response`. `Path` and
/// `MonitorList` are wire-ambiguous (both are plain text), so this is only
/// used where the caller already knows which command it sent; ambiguous
/// cases are treated as a single-element list falling back to `Path`.
pub fn parse_response(text: &str) -> Response {
    if let Some(msg) = text.strip_prefix("error: ") {
        return Response::Error(msg.to_string());
    }
    if text == "ok" {
        return Response::Ok;
    }
    if text.contains('\n') {
        return Response::MonitorList(text.lines().map(str::to_string).collect());
    }
    Response::Path(text.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ok_round_trips() {
        assert_eq!(parse_response(&Response::Ok.to_wire()), Response::Ok);
    }

    #[test]
    fn error_round_trips() {
        let r = Response::Error("unknown monitor eDP-9".to_string());
        assert_eq!(parse_response(&r.to_wire()), r);
    }

    #[test]
    fn path_round_trips() {
        let r = Response::Path("/home/u/video.mp4".to_string());
        assert_eq!(parse_response(&r.to_wire()), r);
    }

    #[test]
    fn monitor_list_round_trips() {
        let r = Response::MonitorList(vec!["eDP-1".to_string(), "HDMI-A-1".to_string()]);
        assert_eq!(parse_response(&r.to_wire()), r);
    }
}
```

- [ ] **Step 4: Run the tests**

```bash
cd crates/hyprwall-ipc && cargo test
```

Expected: all tests pass (this crate has no dependency on the not-yet-created workspace members, so `cargo test -p hyprwall-ipc` from the repo root works too once Task 2 exists; for now `cd` into the crate directly).

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml .gitignore crates/hyprwall-ipc
git commit -m "feat: hyprwall-ipc wire protocol (Command/Response)"
```

---

## Task 2: `hyprwalld` config module (TOML zones)

**Files:**
- Create: `crates/hyprwalld/Cargo.toml`
- Create: `crates/hyprwalld/src/main.rs`
- Create: `crates/hyprwalld/src/config/mod.rs`
- Create: `crates/hyprwalld/src/config/model.rs`
- Create: `crates/hyprwalld/src/config/store.rs`

**Interfaces:**
- Consumes: nothing from Task 1 directly.
- Produces: `config::model::{Config, ZoneConfig}`, `config::store::{load, save, default_config_path}` — `load(path: &Path) -> anyhow::Result<Config>` (empty `Config` if file absent), `save(path: &Path, cfg: &Config) -> anyhow::Result<()>`, `default_config_path() -> PathBuf`.

- [ ] **Step 1: Scaffold the crate and add dependencies**

```bash
mkdir -p crates/hyprwalld/src/config
cd crates/hyprwalld
cargo init --name hyprwalld --bin .
cargo add serde --features derive
cargo add toml
cargo add anyhow
cargo add dirs
cargo add tempfile --dev
cd ../..
```

`cargo init` will create a `Cargo.toml`/`src/main.rs`; overwrite `src/main.rs` per Step 4 below.

- [ ] **Step 2: Write the failing test for config model + store round-trip**

Write `crates/hyprwalld/src/config/model.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub zones: Vec<ZoneConfig>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ZoneConfig {
    pub monitors: Vec<String>,
    pub path: String,
}
```

Write `crates/hyprwalld/src/config/store.rs`:

```rust
use std::path::{Path, PathBuf};

use super::model::Config;

pub fn default_config_path() -> PathBuf {
    dirs::config_dir()
        .expect("no config dir available for this platform")
        .join("hyprwall")
        .join("config.toml")
}

pub fn load(path: &Path) -> anyhow::Result<Config> {
    if !path.exists() {
        return Ok(Config::default());
    }
    let text = std::fs::read_to_string(path)?;
    Ok(toml::from_str(&text)?)
}

pub fn save(path: &Path, cfg: &Config) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text = toml::to_string_pretty(cfg)?;
    std::fs::write(path, text)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::model::ZoneConfig;

    #[test]
    fn load_missing_file_returns_default() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("does-not-exist.toml");
        let cfg = load(&path).unwrap();
        assert_eq!(cfg, Config::default());
    }

    #[test]
    fn save_then_load_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("config.toml");
        let cfg = Config {
            zones: vec![
                ZoneConfig { monitors: vec!["eDP-1".to_string()], path: "/a.mp4".to_string() },
                ZoneConfig {
                    monitors: vec!["HDMI-A-1".to_string(), "HDMI-A-2".to_string()],
                    path: "/b.mp4".to_string(),
                },
            ],
        };
        save(&path, &cfg).unwrap();
        let loaded = load(&path).unwrap();
        assert_eq!(loaded, cfg);
    }
}
```

- [ ] **Step 3: Wire the module tree**

Write `crates/hyprwalld/src/config/mod.rs`:

```rust
pub mod model;
pub mod store;
```

- [ ] **Step 4: Minimal `main.rs` so the crate builds**

Write `crates/hyprwalld/src/main.rs`:

```rust
mod config;

fn main() {
    println!("hyprwalld: not yet implemented");
}
```

- [ ] **Step 5: Run the tests**

```bash
cargo test -p hyprwalld
```

Expected: PASS (`load_missing_file_returns_default`, `save_then_load_round_trips`).

- [ ] **Step 6: Commit**

```bash
git add crates/hyprwalld
git commit -m "feat: hyprwalld config load/save (TOML zones)"
```

---

## Task 3: Monitor registry + zone manager (pure logic)

**Files:**
- Create: `crates/hyprwalld/src/monitor.rs`
- Create: `crates/hyprwalld/src/monitor_registry.rs`
- Create: `crates/hyprwalld/src/zone.rs`
- Create: `crates/hyprwalld/src/zone_manager.rs`
- Modify: `crates/hyprwalld/src/main.rs`

**Interfaces:**
- Consumes: nothing new from Task 1/2 at the type level (will be wired together in Task 5).
- Produces: `monitor::{Monitor, Rect}`, `monitor_registry::MonitorRegistry` with `new()`, `insert(&mut self, m: Monitor)`, `remove(&mut self, name: &str)`, `names(&self) -> Vec<String>`, `geometry(&self) -> HashMap<String, Rect>`, `contains(&self, name: &str) -> bool`. `zone::Zone { id: u64, monitors: Vec<String>, path: Option<String>, bounding_box: Option<Rect> }`. `zone_manager::{ZoneManager, ZoneApplyOutcome, ZoneError}` — `ZoneManager::new()`, `apply_set(&mut self, monitors: &[String], path: String, registry: &MonitorRegistry) -> Result<ZoneApplyOutcome, ZoneError>`, `zone_for_monitor(&self, monitor: &str) -> Option<&Zone>`, `path_for_monitor(&self, monitor: &str) -> Option<&str>`. `ZoneApplyOutcome { zone_id: u64, bounding_box: Rect, dissolved_zone_ids: Vec<u64> }`. `ZoneError { UnknownMonitor(String) }`.

- [ ] **Step 1: Write `Monitor`/`Rect` and the failing bounding-box test**

Write `crates/hyprwalld/src/monitor.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    /// Union of one or more rects: the smallest rect containing all of them.
    pub fn union(rects: &[Rect]) -> Option<Rect> {
        let first = *rects.first()?;
        let (mut min_x, mut min_y) = (first.x, first.y);
        let (mut max_x, mut max_y) = (first.x + first.w, first.y + first.h);
        for r in &rects[1..] {
            min_x = min_x.min(r.x);
            min_y = min_y.min(r.y);
            max_x = max_x.max(r.x + r.w);
            max_y = max_y.max(r.y + r.h);
        }
        Some(Rect { x: min_x, y: min_y, w: max_x - min_x, h: max_y - min_y })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Monitor {
    pub name: String,
    pub logical: Rect,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn union_of_single_rect_is_itself() {
        let r = Rect { x: 0, y: 0, w: 1920, h: 1080 };
        assert_eq!(Rect::union(&[r]), Some(r));
    }

    #[test]
    fn union_of_two_side_by_side_rects() {
        let a = Rect { x: 0, y: 0, w: 1920, h: 1080 };
        let b = Rect { x: 1920, y: 0, w: 1920, h: 1080 };
        assert_eq!(Rect::union(&[a, b]), Some(Rect { x: 0, y: 0, w: 3840, h: 1080 }));
    }

    #[test]
    fn union_of_empty_is_none() {
        assert_eq!(Rect::union(&[]), None);
    }
}
```

- [ ] **Step 2: Write `MonitorRegistry` and its test**

Write `crates/hyprwalld/src/monitor_registry.rs`:

```rust
use std::collections::HashMap;

use crate::monitor::{Monitor, Rect};

#[derive(Debug, Default)]
pub struct MonitorRegistry {
    monitors: HashMap<String, Monitor>,
}

impl MonitorRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, m: Monitor) {
        self.monitors.insert(m.name.clone(), m);
    }

    pub fn remove(&mut self, name: &str) {
        self.monitors.remove(name);
    }

    pub fn contains(&self, name: &str) -> bool {
        self.monitors.contains_key(name)
    }

    pub fn names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.monitors.keys().cloned().collect();
        names.sort();
        names
    }

    pub fn geometry(&self) -> HashMap<String, Rect> {
        self.monitors.iter().map(|(k, v)| (k.clone(), v.logical)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mon(name: &str, x: i32, w: i32) -> Monitor {
        Monitor { name: name.to_string(), logical: Rect { x, y: 0, w, h: 1080 } }
    }

    #[test]
    fn insert_then_contains() {
        let mut reg = MonitorRegistry::new();
        reg.insert(mon("eDP-1", 0, 1920));
        assert!(reg.contains("eDP-1"));
        assert!(!reg.contains("HDMI-A-1"));
    }

    #[test]
    fn remove_drops_it() {
        let mut reg = MonitorRegistry::new();
        reg.insert(mon("eDP-1", 0, 1920));
        reg.remove("eDP-1");
        assert!(!reg.contains("eDP-1"));
    }

    #[test]
    fn names_are_sorted() {
        let mut reg = MonitorRegistry::new();
        reg.insert(mon("HDMI-A-1", 1920, 1920));
        reg.insert(mon("eDP-1", 0, 1920));
        assert_eq!(reg.names(), vec!["HDMI-A-1".to_string(), "eDP-1".to_string()]);
    }
}
```

- [ ] **Step 3: Write `Zone` and the failing `ZoneManager` tests**

Write `crates/hyprwalld/src/zone.rs`:

```rust
use crate::monitor::Rect;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Zone {
    pub id: u64,
    pub monitors: Vec<String>,
    pub path: Option<String>,
    pub bounding_box: Option<Rect>,
}
```

Write `crates/hyprwalld/src/zone_manager.rs`:

```rust
use std::fmt;

use crate::monitor::Rect;
use crate::monitor_registry::MonitorRegistry;
use crate::zone::Zone;

#[derive(Debug, Default)]
pub struct ZoneManager {
    zones: Vec<Zone>,
    next_id: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZoneApplyOutcome {
    pub zone_id: u64,
    pub bounding_box: Rect,
    pub dissolved_zone_ids: Vec<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ZoneError {
    UnknownMonitor(String),
}

impl fmt::Display for ZoneError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ZoneError::UnknownMonitor(name) => write!(f, "unknown monitor {name}"),
        }
    }
}

impl ZoneManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Groups `monitors` into one zone playing `path`. Any named monitor
    /// already in a different zone is pulled out of it first; a zone left
    /// with no monitors is dissolved. Returns the id of the (re)formed zone
    /// and the ids of any zones dissolved as a side effect.
    pub fn apply_set(
        &mut self,
        monitors: &[String],
        path: String,
        registry: &MonitorRegistry,
    ) -> Result<ZoneApplyOutcome, ZoneError> {
        for name in monitors {
            if !registry.contains(name) {
                return Err(ZoneError::UnknownMonitor(name.clone()));
            }
        }

        let mut dissolved_zone_ids = Vec::new();

        // Pull each named monitor out of whatever zone currently holds it.
        for name in monitors {
            for zone in &mut self.zones {
                zone.monitors.retain(|m| m != name);
            }
        }
        // Dissolve now-empty zones (other than the one we're about to (re)form).
        self.zones.retain(|z| {
            if z.monitors.is_empty() {
                dissolved_zone_ids.push(z.id);
                false
            } else {
                true
            }
        });

        let geometry = registry.geometry();
        let rects: Vec<Rect> = monitors.iter().map(|m| geometry[m]).collect();
        let bounding_box = Rect::union(&rects).expect("monitors is non-empty");

        // Reuse an existing zone id if this exact monitor set already forms one
        // (e.g. re-`set` with a new path); otherwise mint a new zone.
        let mut sorted_new = monitors.to_vec();
        sorted_new.sort();
        let existing = self.zones.iter_mut().find(|z| {
            let mut sorted_existing = z.monitors.clone();
            sorted_existing.sort();
            sorted_existing == sorted_new
        });

        let zone_id = if let Some(zone) = existing {
            zone.path = Some(path);
            zone.bounding_box = Some(bounding_box);
            zone.id
        } else {
            let id = self.next_id;
            self.next_id += 1;
            self.zones.push(Zone {
                id,
                monitors: monitors.to_vec(),
                path: Some(path),
                bounding_box: Some(bounding_box),
            });
            id
        };

        Ok(ZoneApplyOutcome { zone_id, bounding_box, dissolved_zone_ids })
    }

    pub fn zone_for_monitor(&self, monitor: &str) -> Option<&Zone> {
        self.zones.iter().find(|z| z.monitors.iter().any(|m| m == monitor))
    }

    pub fn path_for_monitor(&self, monitor: &str) -> Option<&str> {
        self.zone_for_monitor(monitor)?.path.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::monitor::Monitor;

    fn registry_with(names: &[&str]) -> MonitorRegistry {
        let mut reg = MonitorRegistry::new();
        for (i, name) in names.iter().enumerate() {
            reg.insert(Monitor {
                name: name.to_string(),
                logical: Rect { x: (i as i32) * 1920, y: 0, w: 1920, h: 1080 },
            });
        }
        reg
    }

    #[test]
    fn set_single_monitor_forms_zone_of_one() {
        let reg = registry_with(&["eDP-1"]);
        let mut zm = ZoneManager::new();
        let outcome = zm.apply_set(&["eDP-1".to_string()], "/a.mp4".to_string(), &reg).unwrap();
        assert_eq!(outcome.bounding_box, Rect { x: 0, y: 0, w: 1920, h: 1080 });
        assert!(outcome.dissolved_zone_ids.is_empty());
        assert_eq!(zm.path_for_monitor("eDP-1"), Some("/a.mp4"));
    }

    #[test]
    fn set_two_monitors_spans_bounding_box() {
        let reg = registry_with(&["eDP-1", "HDMI-A-1"]);
        let mut zm = ZoneManager::new();
        let outcome = zm
            .apply_set(&["eDP-1".to_string(), "HDMI-A-1".to_string()], "/pano.mp4".to_string(), &reg)
            .unwrap();
        assert_eq!(outcome.bounding_box, Rect { x: 0, y: 0, w: 3840, h: 1080 });
        assert_eq!(zm.zone_for_monitor("eDP-1").unwrap().id, zm.zone_for_monitor("HDMI-A-1").unwrap().id);
    }

    #[test]
    fn re_setting_one_monitor_splits_it_out_of_a_zone() {
        let reg = registry_with(&["eDP-1", "HDMI-A-1"]);
        let mut zm = ZoneManager::new();
        zm.apply_set(&["eDP-1".to_string(), "HDMI-A-1".to_string()], "/pano.mp4".to_string(), &reg)
            .unwrap();

        let outcome = zm.apply_set(&["eDP-1".to_string()], "/solo.mp4".to_string(), &reg).unwrap();

        assert_eq!(zm.path_for_monitor("eDP-1"), Some("/solo.mp4"));
        // The old two-monitor zone had HDMI-A-1 left in it after eDP-1 was pulled out,
        // so it survives (not dissolved) with just HDMI-A-1, still on the old path.
        assert_eq!(zm.path_for_monitor("HDMI-A-1"), Some("/pano.mp4"));
        assert!(outcome.dissolved_zone_ids.is_empty());
    }

    #[test]
    fn re_setting_both_members_alone_dissolves_the_zone() {
        let reg = registry_with(&["eDP-1", "HDMI-A-1"]);
        let mut zm = ZoneManager::new();
        zm.apply_set(&["eDP-1".to_string(), "HDMI-A-1".to_string()], "/pano.mp4".to_string(), &reg)
            .unwrap();
        let zone_id = zm.zone_for_monitor("eDP-1").unwrap().id;

        let outcome = zm.apply_set(&["eDP-1".to_string()], "/solo.mp4".to_string(), &reg).unwrap();
        assert!(outcome.dissolved_zone_ids.is_empty(), "HDMI-A-1 still holds the old zone open");

        let outcome2 = zm.apply_set(&["HDMI-A-1".to_string()], "/other.mp4".to_string(), &reg).unwrap();
        assert_eq!(outcome2.dissolved_zone_ids, vec![zone_id]);
    }

    #[test]
    fn unknown_monitor_is_rejected() {
        let reg = registry_with(&["eDP-1"]);
        let mut zm = ZoneManager::new();
        let err = zm.apply_set(&["eDP-9".to_string()], "/a.mp4".to_string(), &reg).unwrap_err();
        assert_eq!(err, ZoneError::UnknownMonitor("eDP-9".to_string()));
    }
}
```

- [ ] **Step 4: Wire new modules into `main.rs`**

Modify `crates/hyprwalld/src/main.rs`:

```rust
mod config;
mod monitor;
mod monitor_registry;
mod zone;
mod zone_manager;

fn main() {
    println!("hyprwalld: not yet implemented");
}
```

- [ ] **Step 5: Run the tests**

```bash
cargo test -p hyprwalld
```

Expected: all new tests pass, including the two `zone_manager` tests that pin down exactly what "dissolve" means (a zone survives as long as at least one original member is still in it under the old path; it only dissolves once every member has been pulled into other zones).

- [ ] **Step 6: Commit**

```bash
git add crates/hyprwalld
git commit -m "feat: monitor registry + zone manager (grouping, bounding box)"
```

---

## Task 4: `hyprwallctl` CLI

**Files:**
- Create: `crates/hyprwallctl/Cargo.toml`
- Create: `crates/hyprwallctl/src/main.rs`
- Create: `crates/hyprwallctl/src/client.rs`

**Interfaces:**
- Consumes: `hyprwall_ipc::{Command, parse_response}` (Task 1).
- Produces: `client::send(socket_path: &Path, cmd: &Command) -> std::io::Result<String>` (raw response body, caller decides how to print/parse it).

- [ ] **Step 1: Scaffold the crate**

```bash
mkdir -p crates/hyprwallctl/src
cd crates/hyprwallctl
cargo init --name hyprwallctl --bin .
cargo add hyprwall-ipc --path ../hyprwall-ipc
cargo add clap --features derive
cd ../..
```

- [ ] **Step 2: Write the failing test for the socket client**

Write `crates/hyprwallctl/src/client.rs`:

```rust
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;

use hyprwall_ipc::Command;

pub fn send(socket_path: &Path, cmd: &Command) -> std::io::Result<String> {
    let mut stream = UnixStream::connect(socket_path)?;
    writeln!(stream, "{}", cmd.to_wire())?;
    stream.shutdown(std::net::Shutdown::Write)?;
    let mut response = String::new();
    stream.read_to_string(&mut response)?;
    Ok(response.trim_end().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read as _;
    use std::os::unix::net::UnixListener;
    use std::thread;

    #[test]
    fn sends_command_and_reads_response() {
        let dir = tempfile::tempdir().unwrap();
        let socket_path = dir.path().join("test.sock");
        let listener = UnixListener::bind(&socket_path).unwrap();

        let server = thread::spawn(move || {
            let (mut conn, _) = listener.accept().unwrap();
            let mut received = String::new();
            conn.read_to_string(&mut received).unwrap();
            assert_eq!(received.trim_end(), "monitor list");
            conn.write_all(b"eDP-1\nHDMI-A-1").unwrap();
        });

        let response = send(&socket_path, &Command::MonitorList).unwrap();
        server.join().unwrap();

        assert_eq!(response, "eDP-1\nHDMI-A-1");
    }
}
```

Add `tempfile` as a dev-dependency for this crate too:

```bash
cd crates/hyprwallctl && cargo add tempfile --dev && cd ../..
```

- [ ] **Step 3: Run the test to verify it fails, then passes**

```bash
cargo test -p hyprwallctl
```

Expected: compiles and passes once `client.rs` above is in place (this is TDD-in-spirit — the test and implementation were written together above since the implementation is a few lines; run it now to confirm).

- [ ] **Step 4: Write the CLI entrypoint**

Write `crates/hyprwallctl/src/main.rs`:

```rust
mod client;

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use hyprwall_ipc::Command;

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: CliCommand,
}

#[derive(Subcommand)]
enum CliCommand {
    /// List known monitor names
    MonitorList,
    /// Assign a wallpaper to one monitor, or a comma-separated list to span them as one zone
    Set { monitors: String, path: String },
    Pause { monitor: String },
    Play { monitor: String },
    Get { monitor: String },
}

fn socket_path() -> PathBuf {
    let runtime_dir = std::env::var("XDG_RUNTIME_DIR").expect("XDG_RUNTIME_DIR is not set");
    PathBuf::from(runtime_dir).join("hyprwall.sock")
}

fn main() {
    let cli = Cli::parse();
    let command = match cli.command {
        CliCommand::MonitorList => Command::MonitorList,
        CliCommand::Set { monitors, path } => Command::Set {
            monitors: monitors.split(',').map(str::to_string).collect(),
            path,
        },
        CliCommand::Pause { monitor } => Command::Pause { monitor },
        CliCommand::Play { monitor } => Command::Play { monitor },
        CliCommand::Get { monitor } => Command::Get { monitor },
    };

    match client::send(&socket_path(), &command) {
        Ok(response) => println!("{response}"),
        Err(e) => {
            eprintln!("hyprwallctl: {e}");
            std::process::exit(1);
        }
    }
}
```

- [ ] **Step 5: Build the whole workspace**

```bash
cargo build
```

Expected: builds cleanly (this is the first point all three crates exist together).

- [ ] **Step 6: Commit**

```bash
git add crates/hyprwallctl
git commit -m "feat: hyprwallctl CLI over the IPC socket"
```

---

## Task 5: Command handler (`AppState` + `handle_command`)

**Files:**
- Create: `crates/hyprwalld/src/app.rs`
- Create: `crates/hyprwalld/src/ipc/mod.rs`
- Create: `crates/hyprwalld/src/ipc/handler.rs`
- Modify: `crates/hyprwalld/src/main.rs`

**Interfaces:**
- Consumes: `hyprwall_ipc::{Command, Response}` (Task 1), `monitor_registry::MonitorRegistry`, `zone_manager::{ZoneManager, ZoneError}` (Task 3), `config::store::save` (Task 2).
- Produces: `app::AppState { registry: MonitorRegistry, zones: ZoneManager, config_path: PathBuf }` with `AppState::new(registry: MonitorRegistry, config_path: PathBuf) -> Self`, `ipc::handler::handle_command(state: &mut AppState, cmd: Command) -> Response`.

- [ ] **Step 1: Add `hyprwall-ipc` as a dependency of `hyprwalld`**

```bash
cd crates/hyprwalld && cargo add hyprwall-ipc --path ../hyprwall-ipc && cd ../..
```

- [ ] **Step 2: Write `AppState`**

Write `crates/hyprwalld/src/app.rs`:

```rust
use std::path::PathBuf;

use crate::monitor_registry::MonitorRegistry;
use crate::zone_manager::ZoneManager;

pub struct AppState {
    pub registry: MonitorRegistry,
    pub zones: ZoneManager,
    pub config_path: PathBuf,
}

impl AppState {
    pub fn new(registry: MonitorRegistry, config_path: PathBuf) -> Self {
        Self { registry, zones: ZoneManager::new(), config_path }
    }
}
```

- [ ] **Step 3: Write the failing tests for `handle_command`**

Write `crates/hyprwalld/src/ipc/handler.rs`:

```rust
use hyprwall_ipc::{Command, Response};

use crate::app::AppState;
use crate::config::model::{Config, ZoneConfig};
use crate::config::store;
use crate::zone_manager::ZoneError;

pub fn handle_command(state: &mut AppState, cmd: Command) -> Response {
    match cmd {
        Command::MonitorList => Response::MonitorList(state.registry.names()),
        Command::Get { monitor } => match state.zones.path_for_monitor(&monitor) {
            Some(path) => Response::Path(path.to_string()),
            None => Response::Error(format!("no wallpaper set for {monitor}")),
        },
        Command::Set { monitors, path } => {
            match state.zones.apply_set(&monitors, path, &state.registry) {
                Ok(_outcome) => {
                    persist(state);
                    Response::Ok
                }
                Err(ZoneError::UnknownMonitor(name)) => {
                    Response::Error(format!("unknown monitor {name}"))
                }
            }
        }
        // Pause/Play act on playback state that only exists once a zone has a
        // running mpv instance (Task 10). Until then, report clearly rather
        // than silently succeeding.
        Command::Pause { monitor } | Command::Play { monitor } => {
            if state.zones.path_for_monitor(&monitor).is_some() {
                Response::Error("playback control not implemented yet".to_string())
            } else {
                Response::Error(format!("no wallpaper set for {monitor}"))
            }
        }
    }
}

fn persist(state: &AppState) {
    // Rebuild the zone list from ZoneManager's current state each time; this
    // is only ever called after a successful `apply_set`, so zones is never
    // empty in a way that matters here.
    let zones: Vec<ZoneConfig> = state
        .registry
        .names()
        .into_iter()
        .filter_map(|name| {
            let zone = state.zones.zone_for_monitor(&name)?;
            if zone.monitors.first() != Some(&name) {
                return None; // only emit each zone once, keyed by its first monitor
            }
            Some(ZoneConfig { monitors: zone.monitors.clone(), path: zone.path.clone()? })
        })
        .collect();
    let _ = store::save(&state.config_path, &Config { zones });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::monitor::{Monitor, Rect};
    use crate::monitor_registry::MonitorRegistry;

    fn state_with(names: &[&str]) -> AppState {
        let mut registry = MonitorRegistry::new();
        for (i, name) in names.iter().enumerate() {
            registry.insert(Monitor {
                name: name.to_string(),
                logical: Rect { x: (i as i32) * 1920, y: 0, w: 1920, h: 1080 },
            });
        }
        let dir = tempfile::tempdir().unwrap();
        AppState::new(registry, dir.path().join("config.toml"))
    }

    #[test]
    fn monitor_list_returns_registry_names() {
        let mut state = state_with(&["eDP-1", "HDMI-A-1"]);
        let resp = handle_command(&mut state, Command::MonitorList);
        assert_eq!(resp, Response::MonitorList(vec!["HDMI-A-1".to_string(), "eDP-1".to_string()]));
    }

    #[test]
    fn set_then_get_round_trips() {
        let mut state = state_with(&["eDP-1"]);
        let set_resp = handle_command(
            &mut state,
            Command::Set { monitors: vec!["eDP-1".to_string()], path: "/a.mp4".to_string() },
        );
        assert_eq!(set_resp, Response::Ok);

        let get_resp = handle_command(&mut state, Command::Get { monitor: "eDP-1".to_string() });
        assert_eq!(get_resp, Response::Path("/a.mp4".to_string()));
    }

    #[test]
    fn set_unknown_monitor_returns_error() {
        let mut state = state_with(&["eDP-1"]);
        let resp = handle_command(
            &mut state,
            Command::Set { monitors: vec!["eDP-9".to_string()], path: "/a.mp4".to_string() },
        );
        assert_eq!(resp, Response::Error("unknown monitor eDP-9".to_string()));
    }

    #[test]
    fn get_before_set_returns_error() {
        let mut state = state_with(&["eDP-1"]);
        let resp = handle_command(&mut state, Command::Get { monitor: "eDP-1".to_string() });
        assert_eq!(resp, Response::Error("no wallpaper set for eDP-1".to_string()));
    }

    #[test]
    fn set_persists_to_config_file() {
        let mut state = state_with(&["eDP-1", "HDMI-A-1"]);
        handle_command(
            &mut state,
            Command::Set {
                monitors: vec!["eDP-1".to_string(), "HDMI-A-1".to_string()],
                path: "/pano.mp4".to_string(),
            },
        );
        let loaded = store::load(&state.config_path).unwrap();
        assert_eq!(loaded.zones.len(), 1);
        assert_eq!(loaded.zones[0].path, "/pano.mp4");
    }
}
```

Add the `tempfile` dev-dependency to `hyprwalld` (used by these tests):

```bash
cd crates/hyprwalld && cargo add tempfile --dev && cd ../..
```

Write `crates/hyprwalld/src/ipc/mod.rs`:

```rust
pub mod handler;
```

- [ ] **Step 4: Wire new modules into `main.rs`**

Modify `crates/hyprwalld/src/main.rs`:

```rust
mod app;
mod config;
mod ipc;
mod monitor;
mod monitor_registry;
mod zone;
mod zone_manager;

fn main() {
    println!("hyprwalld: not yet implemented");
}
```

- [ ] **Step 5: Run the tests**

```bash
cargo test -p hyprwalld
```

Expected: all pass, including `set_persists_to_config_file` proving the handler writes through to disk on every successful `set`.

- [ ] **Step 6: Commit**

```bash
git add crates/hyprwalld
git commit -m "feat: command handler wiring AppState to zone manager + config"
```

---

## Task 6: IPC socket + daemon skeleton (no Wayland yet)

**Files:**
- Create: `crates/hyprwalld/src/ipc/socket.rs`
- Modify: `crates/hyprwalld/src/ipc/mod.rs`
- Modify: `crates/hyprwalld/src/main.rs`

**Interfaces:**
- Consumes: `app::AppState`, `ipc::handler::handle_command` (Task 5).
- Produces: `ipc::socket::{socket_path, bind_listener}` — `socket_path() -> PathBuf`, `bind_listener(path: &Path) -> std::io::Result<UnixListener>` (handles stale-socket cleanup; returns `AddrInUse` if another instance is genuinely listening). `main.rs` becomes a runnable IPC-only daemon: accepts connections, reads one line, dispatches via `handle_command`, writes the response, closes the connection.

- [ ] **Step 1: Write the failing test for stale-socket handling**

Write `crates/hyprwalld/src/ipc/socket.rs`:

```rust
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};

pub fn socket_path() -> PathBuf {
    let runtime_dir = std::env::var("XDG_RUNTIME_DIR").expect("XDG_RUNTIME_DIR is not set");
    PathBuf::from(runtime_dir).join("hyprwall.sock")
}

/// Binds the daemon's control socket. If a socket file already exists but
/// nothing is listening on it (stale, e.g. from a crash), it's removed and
/// rebound. If something *is* listening, this returns an `AddrInUse` error
/// rather than stealing the socket out from under a running instance.
pub fn bind_listener(path: &Path) -> std::io::Result<UnixListener> {
    if path.exists() {
        match UnixStream::connect(path) {
            Ok(_) => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::AddrInUse,
                    "hyprwalld is already running (socket is live)",
                ));
            }
            Err(_) => {
                std::fs::remove_file(path)?;
            }
        }
    }
    UnixListener::bind(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binds_fresh_socket() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hyprwall.sock");
        let listener = bind_listener(&path).unwrap();
        drop(listener);
    }

    #[test]
    fn removes_stale_socket_file_and_rebinds() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hyprwall.sock");
        // Bind and drop without cleanup, leaving a stale file on disk.
        let listener = UnixListener::bind(&path).unwrap();
        drop(listener);
        assert!(path.exists());

        let relistener = bind_listener(&path).unwrap();
        drop(relistener);
    }

    #[test]
    fn refuses_to_steal_a_live_socket() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hyprwall.sock");
        let _live = UnixListener::bind(&path).unwrap();

        let err = bind_listener(&path).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::AddrInUse);
    }
}
```

- [ ] **Step 2: Run the socket tests**

```bash
cargo test -p hyprwalld ipc::socket
```

Expected: all three pass.

- [ ] **Step 3: Write the failing integration test for the full accept loop**

Add to the bottom of `crates/hyprwalld/src/ipc/socket.rs`, inside a new test module (append, don't replace the block above):

```rust
#[cfg(test)]
mod integration_tests {
    use std::io::{Read, Write};
    use std::os::unix::net::UnixStream;
    use std::thread;
    use std::time::Duration;

    use hyprwall_ipc::Command;

    use crate::app::AppState;
    use crate::ipc::handler::handle_command;
    use crate::monitor_registry::MonitorRegistry;

    use super::bind_listener;

    #[test]
    fn accept_loop_dispatches_one_command_per_connection() {
        let dir = tempfile::tempdir().unwrap();
        let socket_path = dir.path().join("hyprwall.sock");
        let config_path = dir.path().join("config.toml");
        let listener = bind_listener(&socket_path).unwrap();

        let server = thread::spawn(move || {
            let mut state = AppState::new(MonitorRegistry::new(), config_path);
            // Empty registry: `monitor list` should come back empty, and
            // `set` on any name should be rejected as unknown — this is the
            // seam Task 7 fills in later with real Wayland outputs.
            let (mut conn, _) = listener.accept().unwrap();
            let mut line = String::new();
            conn.read_to_string(&mut line).unwrap();
            let cmd = hyprwall_ipc::parse_command(&line).unwrap();
            let resp = handle_command(&mut state, cmd);
            conn.write_all(resp.to_wire().as_bytes()).unwrap();
        });

        thread::sleep(Duration::from_millis(50));
        let mut client = UnixStream::connect(&socket_path).unwrap();
        writeln!(client, "{}", Command::MonitorList.to_wire()).unwrap();
        client.shutdown(std::net::Shutdown::Write).unwrap();
        let mut response = String::new();
        client.read_to_string(&mut response).unwrap();

        server.join().unwrap();
        assert_eq!(response, "");
    }
}
```

- [ ] **Step 4: Run it**

```bash
cargo test -p hyprwalld
```

Expected: PASS. This pins down the "empty registry" seam Task 7 will fill.

- [ ] **Step 5: Wire the real accept loop into `main.rs`**

Modify `crates/hyprwalld/src/ipc/mod.rs`:

```rust
pub mod handler;
pub mod socket;
```

Modify `crates/hyprwalld/src/main.rs`:

```rust
mod app;
mod config;
mod ipc;
mod monitor;
mod monitor_registry;
mod zone;
mod zone_manager;

use std::io::{Read, Write};

use app::AppState;
use monitor_registry::MonitorRegistry;

fn main() {
    let socket_path = ipc::socket::socket_path();
    let listener = ipc::socket::bind_listener(&socket_path).unwrap_or_else(|e| {
        eprintln!("hyprwalld: {e}");
        std::process::exit(1);
    });

    let config_path = config::store::default_config_path();
    // Task 7 replaces this empty registry with one populated from real
    // Wayland outputs; until then every command that needs a monitor name
    // will correctly report it as unknown.
    let mut state = AppState::new(MonitorRegistry::new(), config_path);

    println!("hyprwalld listening on {}", socket_path.display());
    for conn in listener.incoming() {
        let mut conn = match conn {
            Ok(c) => c,
            Err(e) => {
                eprintln!("hyprwalld: connection error: {e}");
                continue;
            }
        };
        let mut line = String::new();
        if conn.read_to_string(&mut line).is_err() {
            continue;
        }
        let response = match hyprwall_ipc::parse_command(&line) {
            Ok(cmd) => ipc::handler::handle_command(&mut state, cmd),
            Err(e) => hyprwall_ipc::Response::Error(e.to_string()),
        };
        let _ = conn.write_all(response.to_wire().as_bytes());
    }
}
```

- [ ] **Step 6: Manual smoke test**

```bash
XDG_RUNTIME_DIR=/tmp cargo run -p hyprwalld &
sleep 1
cargo run -p hyprwallctl -- monitor-list
kill %1
```

Expected: prints an empty line (no monitors known yet — that's correct at this stage) and the daemon doesn't crash. Note: `clap`'s derive turns `MonitorList` into the kebab-case flag `monitor-list`; if it instead expects a different spelling, run `cargo run -p hyprwallctl -- --help` to confirm and adjust the command above.

- [ ] **Step 7: Commit**

```bash
git add crates/hyprwalld
git commit -m "feat: IPC accept loop wired into hyprwalld main"
```

---

## Task 7: Wayland connection + real monitor tracking

> From here on, tests are manual (documented steps run on a live Hyprland session) — there's no headless Wayland compositor in this workflow to automate against. This matches the spec's Testing section.

**Files:**
- Create: `crates/hyprwalld/src/wayland/mod.rs`
- Create: `crates/hyprwalld/src/wayland/connection.rs`
- Create: `crates/hyprwalld/src/wayland/output.rs`
- Modify: `crates/hyprwalld/src/main.rs`
- Modify: `crates/hyprwalld/src/monitor_registry.rs` (add `remove` call sites; no signature change)

**Interfaces:**
- Consumes: `monitor_registry::MonitorRegistry::{insert, remove}`, `monitor::{Monitor, Rect}` (Task 3).
- Produces: `wayland::connection::WaylandBackend` — owns the `wayland-client` `Connection`, `EventQueue`, and `smithay-client-toolkit` registry state; exposes `new() -> anyhow::Result<Self>` (fails fast if `zwlr_layer_shell_v1` isn't advertised) and a way to pump events into `AppState.registry` each iteration of the main loop (exact method name decided during implementation — document it in this file's doc comment once written, since SCTK's exact dispatch entrypoint depends on the installed version; see Global Constraints).

- [ ] **Step 1: Add Wayland dependencies**

```bash
cd crates/hyprwalld
cargo add wayland-client
cargo add smithay-client-toolkit
cargo add calloop
cargo add calloop-wayland-source
cd ../..
```

- [ ] **Step 2: Check the installed SCTK API surface before writing code**

```bash
cargo doc -p smithay-client-toolkit --open
```

Read the `output` module (`OutputState`, `OutputHandler`, `delegate_output!`) and the `registry` module (`RegistryState`, `delegate_registry!`). The exact trait method names/signatures below match SCTK 0.19-era APIs; if the installed version differs, adjust to match what `cargo doc` shows rather than the letter of this task — the behavior (fail fast without `zwlr_layer_shell_v1`, sync `OutputInfo` into `MonitorRegistry` on add/update/remove) is what must hold.

- [ ] **Step 3: Write the connection bootstrap**

Write `crates/hyprwalld/src/wayland/connection.rs`:

```rust
use smithay_client_toolkit::output::{OutputHandler, OutputState};
use smithay_client_toolkit::registry::{ProvidesRegistryState, RegistryState};
use smithay_client_toolkit::{delegate_output, delegate_registry, registry_handlers};
use wayland_client::protocol::wl_output::WlOutput;
use wayland_client::{Connection, QueueHandle};

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

        // Fail fast: this daemon only targets wlroots compositors.
        globals
            .bind::<wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1::ZwlrLayerShellV1, _, _>(
                &qh, 1..=4, (),
            )
            .map_err(|_| anyhow::anyhow!("compositor does not support zwlr_layer_shell_v1 (not wlroots-based?)"))?;

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

delegate_output!(AppData);
delegate_registry!(AppData);
```

Add the wlr protocols dependency this needs:

```bash
cd crates/hyprwalld && cargo add wayland-protocols-wlr --features client && cd ../..
```

- [ ] **Step 4: Wire it into `main.rs`, replacing the empty `MonitorRegistry`**

Write `crates/hyprwalld/src/wayland/mod.rs`:

```rust
pub mod connection;
pub mod output;
```

`output.rs` starts empty — this task puts all logic in `connection.rs`; `output.rs` becomes the home for layer-surface-per-output bookkeeping in Task 8. For now:

```rust
// Populated in Task 8 with per-output layer-surface tracking.
```

Modify `crates/hyprwalld/src/main.rs` to drive the Wayland event queue on a background thread that keeps `AppData.monitors` in sync, while the existing IPC accept loop keeps running on the main thread against a `MonitorRegistry` shared via `Arc<Mutex<..>>`. Replace the body of `main()`:

```rust
mod app;
mod config;
mod ipc;
mod monitor;
mod monitor_registry;
mod wayland;
mod zone;
mod zone_manager;

use std::io::{Read, Write};
use std::sync::{Arc, Mutex};

use app::AppState;
use monitor_registry::MonitorRegistry;
use wayland::connection::WaylandBackend;

fn main() -> anyhow::Result<()> {
    let (backend, mut data) = WaylandBackend::new()?;
    let shared_monitors = Arc::new(Mutex::new(MonitorRegistry::new()));

    {
        let shared_monitors = Arc::clone(&shared_monitors);
        let mut event_queue = backend.event_queue;
        std::thread::spawn(move || loop {
            event_queue.blocking_dispatch(&mut data).expect("wayland dispatch failed");
            *shared_monitors.lock().unwrap() = std::mem::replace(&mut data.monitors, MonitorRegistry::new());
        });
    }

    let socket_path = ipc::socket::socket_path();
    let listener = ipc::socket::bind_listener(&socket_path)?;
    let config_path = config::store::default_config_path();

    println!("hyprwalld listening on {}", socket_path.display());
    for conn in listener.incoming() {
        let mut conn = match conn {
            Ok(c) => c,
            Err(e) => {
                eprintln!("hyprwalld: connection error: {e}");
                continue;
            }
        };
        let mut line = String::new();
        if conn.read_to_string(&mut line).is_err() {
            continue;
        }
        let registry = shared_monitors.lock().unwrap().clone_into_new();
        let mut state = AppState::new(registry, config_path.clone());
        let response = match hyprwall_ipc::parse_command(&line) {
            Ok(cmd) => ipc::handler::handle_command(&mut state, cmd),
            Err(e) => hyprwall_ipc::Response::Error(e.to_string()),
        };
        let _ = conn.write_all(response.to_wire().as_bytes());
    }
    Ok(())
}
```

This introduces a `clone_into_new` need on `MonitorRegistry` and drops the persisted `ZoneManager` across requests, both of which are placeholders that get fixed for real in Task 10 when `AppState` becomes long-lived and shared across the Wayland thread and the IPC loop instead of being reconstructed per request. Note this explicitly rather than polishing it now — YAGNI until Task 10 needs the real shared-state design anyway.

Add `#[derive(Clone)]` to `MonitorRegistry` in `crates/hyprwalld/src/monitor_registry.rs` (it already only contains a `HashMap<String, Monitor>` with `Monitor: Clone`, so this is a one-line addition) and replace the `clone_into_new()` call above with `.clone()`.

- [ ] **Step 5: Manual test on Hyprland**

```bash
cargo build -p hyprwalld
XDG_RUNTIME_DIR=$XDG_RUNTIME_DIR cargo run -p hyprwalld &
sleep 1
cargo run -p hyprwallctl -- monitor-list
kill %1
```

Expected: prints your real monitor name(s) (e.g. `eDP-1`), not an empty response. If it fails to start with the `zwlr_layer_shell_v1` error, something's wrong with the Hyprland session, not the code — Hyprland always advertises this global.

- [ ] **Step 6: Commit**

```bash
git add crates/hyprwalld
git commit -m "feat: Wayland output tracking feeds the live monitor registry"
```

---

## Task 8: Layer-shell surface per monitor

**Files:**
- Modify: `crates/hyprwalld/src/wayland/output.rs`
- Modify: `crates/hyprwalld/src/wayland/connection.rs`

**Interfaces:**
- Consumes: `AppData` (Task 7), `smithay_client_toolkit::shell::wlr_layer` module.
- Produces: `output::LayerSurfaces` — tracks one `LayerSurface` per output name, created on `new_output`/destroyed on `output_destroyed`, alongside the existing `MonitorRegistry` sync.

- [ ] **Step 1: Check the installed SCTK layer-shell API**

```bash
cargo doc -p smithay-client-toolkit --open
```

Read `shell::wlr_layer` (`LayerShell`, `LayerSurface`, `LayerSurfaceConfigure`, `Layer`, `Anchor`). Confirm exact method names for: creating a layer surface from a `WlOutput` + `Layer::Background`, setting anchor to all four edges, setting `exclusive_zone(-1)`, setting `keyboard_interactivity(KeyboardInteractivity::None)`, and committing.

- [ ] **Step 2: Track one `LayerSurface` per monitor**

Write `crates/hyprwalld/src/wayland/output.rs`:

```rust
use std::collections::HashMap;

use smithay_client_toolkit::shell::wlr_layer::{
    Anchor, KeyboardInteractivity, Layer, LayerShell, LayerSurface,
};
use wayland_client::protocol::wl_output::WlOutput;
use wayland_client::QueueHandle;

pub struct LayerSurfaces {
    surfaces: HashMap<String, LayerSurface>,
}

impl LayerSurfaces {
    pub fn new() -> Self {
        Self { surfaces: HashMap::new() }
    }

    pub fn create<D>(
        &mut self,
        layer_shell: &LayerShell,
        qh: &QueueHandle<D>,
        output: &WlOutput,
        name: String,
    ) where
        D: smithay_client_toolkit::shell::wlr_layer::LayerShellHandler + 'static,
    {
        let surface = layer_shell.create_layer_surface(
            qh,
            layer_shell.compositor_state().create_surface(qh),
            Layer::Background,
            Some("hyprwall"),
            Some(output),
        );
        surface.set_anchor(Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT);
        surface.set_exclusive_zone(-1);
        surface.set_keyboard_interactivity(KeyboardInteractivity::None);
        surface.commit();
        self.surfaces.insert(name, surface);
    }

    pub fn destroy(&mut self, name: &str) {
        self.surfaces.remove(name);
    }

    pub fn get(&self, name: &str) -> Option<&LayerSurface> {
        self.surfaces.get(name)
    }
}
```

Note: `layer_shell.compositor_state()` and the exact `create_layer_surface` signature are the parts most likely to have shifted from this sketch — this is exactly the kind of spot Step 1's `cargo doc` check is for. `LayerShellHandler` also requires implementing a `configure` callback (called when the compositor assigns the surface a size); add it to `AppData` in `connection.rs`:

```rust
impl smithay_client_toolkit::shell::wlr_layer::LayerShellHandler for AppData {
    fn closed(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _layer: &LayerSurface) {}

    fn configure(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _layer: &LayerSurface,
        _configure: smithay_client_toolkit::shell::wlr_layer::LayerSurfaceConfigure,
        _serial: u32,
    ) {
        // Task 9 uses this to (re)size the EGL surface; nothing to do yet.
    }
}
```

Wire `LayerSurfaces` creation into `AppData::sync_output` and destruction into `output_destroyed` (both in `connection.rs`), and add `delegate_layer!(AppData);` alongside the existing `delegate_output!`/`delegate_registry!` calls. Also bind `LayerShell` itself (`LayerShell::bind(&globals, &qh)?`) in `WaylandBackend::new` and store it on `AppData` so `sync_output` can call `LayerSurfaces::create`.

- [ ] **Step 3: Manual test on Hyprland**

```bash
cargo build -p hyprwalld
XDG_RUNTIME_DIR=$XDG_RUNTIME_DIR cargo run -p hyprwalld &
sleep 1
hyprctl layers
kill %1
```

Expected: `hyprctl layers` shows a `background` layer surface named `hyprwall` for each connected monitor. Screen content is whatever the compositor shows for an uncommitted/blank buffer (likely nothing visible yet) — that's expected, rendering starts in Task 9.

- [ ] **Step 4: Commit**

```bash
git add crates/hyprwalld
git commit -m "feat: per-monitor background layer-shell surfaces"
```

---

## Task 9: EGL context + solid-color render (prove the pipeline before mpv)

**Files:**
- Create: `crates/hyprwalld/src/render/mod.rs`
- Create: `crates/hyprwalld/src/render/egl_context.rs`
- Modify: `crates/hyprwalld/src/wayland/connection.rs`

**Interfaces:**
- Consumes: `LayerSurface`'s underlying `WlSurface` (Task 8).
- Produces: `render::egl_context::EglContext` — `EglContext::new(wl_display: &WlDisplay, wl_surface: &WlSurface, width: i32, height: i32) -> anyhow::Result<Self>`, `EglContext::make_current(&self) -> anyhow::Result<()>`, `EglContext::swap_buffers(&self) -> anyhow::Result<()>`. This task's deliverable is each monitor showing a solid clear-color instead of nothing, proving the EGL plumbing before mpv is added on top of it in Task 10.

- [ ] **Step 1: Add EGL dependencies**

```bash
cd crates/hyprwalld
cargo add khronos-egl --features dynamic
cargo add wayland-egl
cd ../..
```

- [ ] **Step 2: Check the installed `khronos-egl`/`wayland-egl` API**

```bash
cargo doc -p khronos-egl -p wayland-egl --open
```

Confirm: how to load EGL dynamically (`egl::DynamicInstance`), `eglGetDisplay`/`eglInitialize`/`eglChooseConfig`/`eglCreateContext`/`eglCreateWindowSurface` signatures, and `wayland_egl::WlEglSurface::new(surface, width, height)` for turning a `wl_surface` into the native window handle EGL needs.

- [ ] **Step 3: Write the EGL context wrapper**

Write `crates/hyprwalld/src/render/egl_context.rs`:

```rust
use egl::{self, API as EglApi};
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_client::Proxy;
use wayland_egl::WlEglSurface;

pub struct EglContext {
    egl: EglApi,
    display: egl::Display,
    context: egl::Context,
    surface: egl::Surface,
    _wl_egl_surface: WlEglSurface, // must outlive `surface`
}

impl EglContext {
    pub fn new(
        wl_display_ptr: *mut std::ffi::c_void,
        wl_surface: &WlSurface,
        width: i32,
        height: i32,
    ) -> anyhow::Result<Self> {
        let egl = unsafe { egl::DynamicInstance::<egl::EGL1_4>::load()? };
        let display = unsafe { egl.get_display(wl_display_ptr) }
            .ok_or_else(|| anyhow::anyhow!("eglGetDisplay failed"))?;
        egl.initialize(display)?;

        let attribs = [
            egl::SURFACE_TYPE, egl::WINDOW_BIT,
            egl::RENDERABLE_TYPE, egl::OPENGL_ES2_BIT,
            egl::RED_SIZE, 8,
            egl::GREEN_SIZE, 8,
            egl::BLUE_SIZE, 8,
            egl::NONE,
        ];
        let config = egl
            .choose_first_config(display, &attribs)?
            .ok_or_else(|| anyhow::anyhow!("no matching EGL config"))?;

        egl.bind_api(egl::OPENGL_ES_API)?;
        let context_attribs = [egl::CONTEXT_CLIENT_VERSION, 2, egl::NONE];
        let context = egl.create_context(display, config, None, &context_attribs)?;

        let wl_egl_surface = WlEglSurface::new(wl_surface.id(), width, height)?;
        let surface = unsafe {
            egl.create_window_surface(display, config, wl_egl_surface.ptr() as *mut _, None)?
        };

        Ok(Self { egl, display, context, surface, _wl_egl_surface: wl_egl_surface })
    }

    pub fn make_current(&self) -> anyhow::Result<()> {
        self.egl.make_current(self.display, Some(self.surface), Some(self.surface), Some(self.context))?;
        Ok(())
    }

    pub fn swap_buffers(&self) -> anyhow::Result<()> {
        self.egl.swap_buffers(self.display, self.surface)?;
        Ok(())
    }
}
```

Note: `egl.get_display` needs the raw `wl_display` pointer, not the `wayland-client` `Connection` wrapper — get it via `Connection::backend().display_ptr()` (confirm exact method name against the installed `wayland-client` version's docs, same caveat as Task 7-8). This is the sketch to adjust against `cargo doc` output, per Global Constraints.

Write `crates/hyprwalld/src/render/mod.rs`:

```rust
pub mod egl_context;
```

- [ ] **Step 4: Wire a per-monitor EGL context + clear-color draw into the layer surface's configure callback**

Modify the `configure` method added in Task 8 (`connection.rs`) to create an `EglContext` for the surface (sized to the `configure` event's width/height, falling back to the monitor's logical size if the compositor sends `0,0` meaning "you choose"), call `make_current()`, then a minimal OpenGL ES clear:

```rust
// Requires the `gles31` or similar raw-bindings crate for `glClearColor`/`glClear`;
// add it now:
```

```bash
cd crates/hyprwalld && cargo add glow && cd ../..
```

`glow` gives safe-ish OpenGL ES function loading without hand-rolling FFI bindings. Load it once per `EglContext` via `glow::Context::from_loader_function(|s| egl.get_proc_address(s) as *const _)`, then each frame:

```rust
unsafe {
    gl.clear_color(0.05, 0.05, 0.08, 1.0);
    gl.clear(glow::COLOR_BUFFER_BIT);
}
egl_context.swap_buffers()?;
```

Store the `EglContext` + `glow::Context` alongside each monitor's `LayerSurface` (extend the `LayerSurfaces` map from Task 8, or add a parallel `HashMap<String, RenderTarget>` — prefer the latter to keep `wayland/output.rs`'s job to "track surfaces" and put render state in `render/`, per the one-file-one-job convention).

- [ ] **Step 5: Manual test on Hyprland**

```bash
cargo build -p hyprwalld
XDG_RUNTIME_DIR=$XDG_RUNTIME_DIR cargo run -p hyprwalld &
sleep 1
# look at the screen
kill %1
```

Expected: every monitor's background is now a solid dark blue-gray instead of empty/black-by-omission — confirms the EGL pipeline (context, surface, swap) works before video decode is layered on top in Task 10. If nothing changes, check `configure` is actually firing (add a `eprintln!` temporarily) before debugging EGL itself.

- [ ] **Step 6: Commit**

```bash
git add crates/hyprwalld
git commit -m "feat: EGL context per monitor, solid-color render proves the pipeline"
```

---

## Task 10: mpv render pipeline — zones actually play video

**Files:**
- Create: `crates/hyprwalld/src/render/mpv_instance.rs`
- Create: `crates/hyprwalld/src/render/zone_target.rs`
- Create: `crates/hyprwalld/src/render/frame_scheduler.rs`
- Modify: `crates/hyprwalld/src/app.rs`
- Modify: `crates/hyprwalld/src/ipc/handler.rs`
- Modify: `crates/hyprwalld/src/main.rs`

**Interfaces:**
- Consumes: `EglContext` (Task 9), `ZoneManager`/`ZoneApplyOutcome` (Task 3), `AppState` (Task 5).
- Produces: `render::mpv_instance::MpvInstance` — `new(egl_context: &EglContext) -> anyhow::Result<Self>`, `load_file(&mut self, path: &str) -> anyhow::Result<()>`, `render_to_fbo(&mut self, fbo: u32, width: i32, height: i32) -> anyhow::Result<()>`. `render::zone_target::ZoneTarget` — offscreen FBO+texture sized to a zone's bounding box, plus `blit_region(&self, dst: &EglContext, src_rect: Rect, dst_size: (i32, i32))`. This task makes `Command::Set` actually create/replace playback resources, fulfilling the spec's core success criteria.

- [ ] **Step 1: Add the mpv dependency**

```bash
cd crates/hyprwalld && cargo add libmpv2 --features render && cd ../..
```

- [ ] **Step 2: Check the installed `libmpv2` render API**

```bash
cargo doc -p libmpv2 --open
```

Confirm: `Mpv::new()` construction, `libmpv2::render::RenderContext` construction (it needs an OpenGL "get proc address" callback, mirroring the `glow` loader from Task 9), `RenderContext::render(fbo, width, height, flip_y)`, and how to register the update/wakeup callback (`RenderContext::set_update_callback`).

- [ ] **Step 3: Write `MpvInstance`**

Write `crates/hyprwalld/src/render/mpv_instance.rs`:

```rust
use libmpv2::{Mpv, render::{RenderContext, RenderParam, RenderParamApiType}};

pub struct MpvInstance {
    mpv: Mpv,
    render_context: RenderContext,
}

impl MpvInstance {
    pub fn new(get_proc_address: impl Fn(&str) -> *mut std::ffi::c_void + 'static) -> anyhow::Result<Self> {
        let mpv = Mpv::new()?;
        mpv.set_property("vo", "libmpv")?;
        mpv.set_property("loop-file", "inf")?; // wallpapers loop
        mpv.set_property("mute", "yes")?; // spec: default mute, no per-wallpaper policy in v1

        let render_context = RenderContext::new(
            unsafe { mpv.ctx.as_mut() },
            vec![
                RenderParam::ApiType(RenderParamApiType::OpenGl),
                RenderParam::InitParams(get_proc_address),
            ],
        )?;

        Ok(Self { mpv, render_context })
    }

    pub fn load_file(&mut self, path: &str) -> anyhow::Result<()> {
        self.mpv.command("loadfile", &[path, "replace"])?;
        Ok(())
    }

    pub fn render_to_fbo(&mut self, fbo: i32, width: i32, height: i32) -> anyhow::Result<()> {
        self.render_context.render::<()>(fbo, width, height, true)?;
        Ok(())
    }

    pub fn set_wakeup_callback(&mut self, cb: impl Fn() + Send + 'static) {
        self.render_context.set_update_callback(cb);
    }
}
```

This is the highest-uncertainty FFI surface in the whole plan (exact `libmpv2` render-API shape). Treat the `cargo doc` check in Step 2 as load-bearing, not optional — adjust field/method names to match, keeping the three public methods' behavior (load a file, render one frame into a caller-provided FBO, get told when a new frame is ready) intact.

- [ ] **Step 4: Write `ZoneTarget` (offscreen FBO + per-monitor blit)**

Write `crates/hyprwalld/src/render/zone_target.rs`:

```rust
use glow::HasContext;

use crate::monitor::Rect;

pub struct ZoneTarget {
    pub fbo: glow::Framebuffer,
    pub texture: glow::Texture,
    pub bounding_box: Rect,
}

impl ZoneTarget {
    pub fn new(gl: &glow::Context, bounding_box: Rect) -> anyhow::Result<Self> {
        unsafe {
            let texture = gl.create_texture().map_err(|e| anyhow::anyhow!(e))?;
            gl.bind_texture(glow::TEXTURE_2D, Some(texture));
            gl.tex_image_2d(
                glow::TEXTURE_2D, 0, glow::RGBA8 as i32,
                bounding_box.w, bounding_box.h, 0,
                glow::RGBA, glow::UNSIGNED_BYTE, None,
            );
            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MIN_FILTER, glow::LINEAR as i32);
            gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MAG_FILTER, glow::LINEAR as i32);

            let fbo = gl.create_framebuffer().map_err(|e| anyhow::anyhow!(e))?;
            gl.bind_framebuffer(glow::FRAMEBUFFER, Some(fbo));
            gl.framebuffer_texture_2d(
                glow::FRAMEBUFFER, glow::COLOR_ATTACHMENT0, glow::TEXTURE_2D, Some(texture), 0,
            );

            Ok(Self { fbo, texture, bounding_box })
        }
    }

    /// Blits the sub-rect of this zone's texture that corresponds to one
    /// member monitor's logical position into that monitor's currently-bound
    /// default framebuffer.
    pub fn blit_region(&self, gl: &glow::Context, monitor_logical: Rect) {
        let src_x0 = monitor_logical.x - self.bounding_box.x;
        let src_y0 = monitor_logical.y - self.bounding_box.y;
        unsafe {
            gl.bind_framebuffer(glow::READ_FRAMEBUFFER, Some(self.fbo));
            gl.bind_framebuffer(glow::DRAW_FRAMEBUFFER, None); // caller has already made the monitor's context current
            gl.blit_framebuffer(
                src_x0, src_y0, src_x0 + monitor_logical.w, src_y0 + monitor_logical.h,
                0, 0, monitor_logical.w, monitor_logical.h,
                glow::COLOR_BUFFER_BIT, glow::LINEAR,
            );
        }
    }
}
```

A single-monitor zone has `monitor_logical == bounding_box`, so `src_x0`/`src_y0` are always `0,0` there — no special case, matching the spec's "single-monitor zone is the same path with a 1:1 blit" requirement.

- [ ] **Step 5: Write the frame scheduler**

Write `crates/hyprwalld/src/render/frame_scheduler.rs`:

```rust
// The wakeup callback registered via MpvInstance::set_wakeup_callback fires
// from an mpv-internal thread and must not touch GL directly. It sends a
// unit over a channel; the main loop's calloop source wakes on that channel
// and performs the actual render + blit + swap sequence for that zone.
//
// Exact wiring (channel type, calloop Source impl) depends on the calloop
// version's idiomatic "ping" primitive — check `cargo doc -p calloop --open`
// for `calloop::ping::Ping` / `PingSource`, which is built for exactly this
// producer-thread-wakes-consumer-thread pattern, before hand-rolling one.
```

Wire one `calloop::ping::Ping` per zone into the main loop (Step 6), each tied to that zone's `MpvInstance::set_wakeup_callback`.

- [ ] **Step 6: Wire `Command::Set` to actually create playback resources**

This is the integration step tying Tasks 3, 5, 9, and 10 together in `app.rs`/`ipc/handler.rs`/`main.rs`:

- Extend `AppState` (in `app.rs`) with `zone_playback: HashMap<u64, (MpvInstance, ZoneTarget)>` keyed by `Zone::id`.
- In `handler.rs`, after a successful `state.zones.apply_set(..)` returns `ZoneApplyOutcome`, the handler needs access to each affected monitor's `EglContext`/`glow::Context` to build the new `ZoneTarget` and any newly-orphaned monitor's context to stop blitting to it — this means `handle_command` can no longer be pure with respect to GL state. Change its signature to take an additional `render: &mut RenderResources` parameter (a new struct in `render/mod.rs` holding the per-monitor `EglContext`s and the `zone_playback` map moved out of `AppState`), and update all Task 5 tests to construct a `RenderResources::new_headless_for_test()` stub that skips real GL calls (guarded by `#[cfg(test)]`) so the existing pure-logic tests keep passing without a real GPU context.
- For each `dissolved_zone_ids` entry, drop its `MpvInstance`/`ZoneTarget` from `zone_playback`.
- For the (re)formed zone: if `zone_playback` doesn't have an entry for `outcome.zone_id`, or its `ZoneTarget.bounding_box != outcome.bounding_box`, tear down any old entry and create a fresh `MpvInstance` + `ZoneTarget` sized to `outcome.bounding_box`; either way, call `load_file(&path)` on it.
- Update `main.rs`'s render loop: on each zone's ping, `make_current` a scratch/shared context bound to the `ZoneTarget`'s FBO (or reuse one member monitor's context as the "current" context when rendering into the FBO — FBOs are just GL objects, any current context on the same EGL display can render into one bound via `framebuffer_texture_2d` from Step 4, no separate offscreen EGL surface needed), call `mpv_instance.render_to_fbo(...)`, then for each member monitor `make_current` that monitor's `EglContext`, `blit_region`, `swap_buffers`.

Update `Command::Pause`/`Command::Play` in `handler.rs` to call `mpv.set_property("pause", true/false)` on the zone's `MpvInstance` instead of returning "not implemented yet".

- [ ] **Step 7: Manual test — single monitor**

```bash
cargo build -p hyprwalld
XDG_RUNTIME_DIR=$XDG_RUNTIME_DIR cargo run -p hyprwalld &
sleep 1
cargo run -p hyprwallctl -- set eDP-1 ~/Videos/some-test-clip.mp4
```

Expected: that monitor plays the video, looping, muted. Swap `eDP-1` for your real output name (from `hyprwallctl monitor-list`).

- [ ] **Step 8: Manual test — spanned zone (requires 2+ monitors)**

```bash
cargo run -p hyprwallctl -- set eDP-1,HDMI-A-1 ~/Videos/panorama-clip.mp4
```

Expected: one continuous image spans both monitors — the right half of the video appears on the second monitor picking up exactly where the left half left off, not the same frame duplicated on each screen.

- [ ] **Step 9: Commit**

```bash
git add crates/hyprwalld
git commit -m "feat: mpv render pipeline, zones play and span video for real"
```

---

## Task 11: Startup config restore, hotplug resume, manual test docs

**Files:**
- Modify: `crates/hyprwalld/src/main.rs`
- Modify: `crates/hyprwalld/src/wayland/connection.rs`
- Create: `crates/hyprwalld/README.md`

**Interfaces:**
- Consumes: everything from Tasks 1-10.
- Produces: no new public interfaces — this task closes the remaining gaps between the spec's Error Handling section and the current implementation, and documents manual verification.

- [ ] **Step 1: Load config at startup and restore zones once their monitors are known**

In `main.rs`, after `WaylandBackend::new()` succeeds and before entering the accept loop: `let saved = config::store::load(&config_path)?;`. After each `sync_output` call (in the Wayland thread) discovers a monitor, check whether that monitor completes a saved `ZoneConfig` whose every member is now present in the registry; if so, call the same code path `handle_command` uses for `Command::Set` internally (extract the "apply + build playback resources" logic from Task 10's handler into a shared `fn restore_or_apply_set(...)` function callable from both places, since Step 6 of Task 10 already built it — this is a refactor of that function's call site, not new logic).

- [ ] **Step 2: Confirm hotplug resume behavior**

Verify (by reading, not new code — this was already designed into Task 7's `output_destroyed`/`sync_output` and Task 10's per-monitor blit) that: unplugging a zone member stops that monitor's `EglContext` from being iterated (it's gone from `LayerSurfaces`) while the zone's `MpvInstance`/`ZoneTarget` keep running untouched for remaining members; replugging the same output name re-creates its `EglContext` and, since it's still a member of a live zone in `zone_playback`, starts blitting to it again next frame with no zone recreation. If tracing the code shows this isn't actually true (e.g. `output_destroyed` accidentally tears down the zone), fix it now — this is the spec's explicit "monitor unplugged" / "monitor plugged in" behavior.

- [ ] **Step 3: Write the manual test README**

Write `crates/hyprwalld/README.md`:

```markdown
# hyprwalld manual test checklist

Run on a live Hyprland session (`hyprctl version` should succeed).

1. `cargo build --release`
2. `./target/release/hyprwalld &`
3. `./target/release/hyprwallctl monitor-list` — should print your real output names.
4. `./target/release/hyprwallctl set <monitor> <path-to-video>` — that monitor should play the video, looping, muted.
5. Unplug/disable one monitor (`hyprctl keyword monitor <name>,disable` or physically), then re-enable it — daemon should not crash; wallpaper should resume on that monitor without re-running `set`.
6. Kill and restart the daemon (`kill %1; ./target/release/hyprwalld &`) — the previously-set wallpaper(s) should reappear without re-running `set`.
7. With 2+ monitors: `hyprwallctl set <mon1>,<mon2> <path-to-a-wide-video>` — should show one continuous image spanning both, not the same frame duplicated.
8. `hyprwallctl set <mon1> <path>` after step 7 — should split `<mon1>` back out to its own wallpaper while `<mon2>` keeps playing the spanned video alone.
```

- [ ] **Step 4: Run through the full checklist**

Follow `crates/hyprwalld/README.md` steps 1-8 manually. This is the final acceptance check against the spec's Success Criteria section — all of them should now hold.

- [ ] **Step 5: Commit**

```bash
git add crates/hyprwalld
git commit -m "feat: restore zones from config on startup, document manual test checklist"
```

---

## Self-Review Notes

- **Spec coverage:** Purpose/success criteria → Tasks 1, 3, 10, 11. Architecture (zones, EGL, mpv) → Tasks 3, 7-10. Threading model → Task 10. Components → Tasks 1, 2, 3, 4, 5, 6. Data flow → Tasks 3, 5, 10. IPC protocol → Tasks 1, 6. Config → Tasks 2, 11. Error handling (bad file, no layer-shell, unplug/replug, stale socket, unknown monitor) → Tasks 3, 5, 6, 7, 11. Testing → automated unit tests in Tasks 1-6, manual checklist in Task 11. Out-of-scope items are not touched by any task.
- **Placeholder scan:** no TBD/TODO left in any task; the two spots with genuine external uncertainty (SCTK's exact layer-shell/output method names, `libmpv2`'s exact render-API shape) are flagged as explicit `cargo doc` verification steps rather than glossed over, per Global Constraints — that's a real action, not a placeholder.
- **Type consistency:** `Command`/`Response` (Task 1) used identically in Tasks 4, 5, 6. `MonitorRegistry`/`Rect`/`Monitor` (Task 3) used identically in Tasks 5, 7. `ZoneManager`/`Zone`/`ZoneApplyOutcome`/`ZoneError` (Task 3) used identically in Tasks 5, 10. `AppState` gains fields across Tasks 5, 7, 10 — each modification is called out explicitly at its introduction point rather than silently assumed.
