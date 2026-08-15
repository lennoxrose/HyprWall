import { useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { TitleBar } from "./components/TitleBar";
import { MonitorsDropdown } from "./components/MonitorsDropdown";
import { SettingsModal } from "./components/SettingsModal";
import { LibraryGrid } from "./components/LibraryGrid";
import { Sidebar } from "./components/Sidebar";
import { EmptyLibraryState } from "./components/EmptyLibraryState";
import { ErrorState } from "./components/ErrorState";
import { useSelection } from "./state/selection";
import {
  DaemonUnreachableError,
  getLibraryFolders,
  listMonitors,
  scanLibrary,
  setLibraryFolders,
  setWallpaper,
  unsetWallpaper,
  watchLibraryFolders,
} from "./lib/api";
import type { MonitorState, WallpaperEntry } from "./lib/types";

export default function App() {
  const [monitors, setMonitors] = useState<MonitorState[]>([]);
  const [wallpapers, setWallpapers] = useState<WallpaperEntry[]>([]);
  const [folders, setFolders] = useState<string[]>([]);
  const [daemonDown, setDaemonDown] = useState(false);
  const [actionError, setActionError] = useState<string | null>(null);
  const [monitorsOpen, setMonitorsOpen] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [groupMode, setGroupMode] = useState(false);
  const [settingsSidebarPath, setSettingsSidebarPath] = useState<string | null>(null);

  // Right-clicking the picture the sidebar is already open for closes it
  // again -- the same tile is both the open and the close affordance.
  const toggleSettingsSidebar = (path: string) =>
    setSettingsSidebarPath((current) => (current === path ? null : path));
  const { selectedMonitors, toggleMonitor, clearSelectedMonitors, selectedWallpaper, setSelectedWallpaper } =
    useSelection();

  const refresh = useCallback(async () => {
    try {
      const [mons, libFolders] = await Promise.all([listMonitors(), getLibraryFolders()]);
      setMonitors(mons);
      setFolders(libFolders);
      setWallpapers(await scanLibrary(libFolders));
      setDaemonDown(false);
    } catch (err) {
      if (err instanceof DaemonUnreachableError) {
        setDaemonDown(true);
      } else {
        console.error(err);
      }
    }
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  useEffect(() => {
    if (!daemonDown) return;
    const id = setInterval(refresh, 3000);
    return () => clearInterval(id);
  }, [daemonDown, refresh]);

  // Live library updates: watches `folders` on disk and, when something
  // changes there, re-scans just the library grid -- not `refresh()`, which
  // would also touch monitors/daemon state for no reason. Re-subscribes
  // whenever `folders` itself changes (e.g. the user edits it in Settings)
  // so the watcher always matches what's actually configured.
  useEffect(() => {
    if (folders.length === 0) return;
    watchLibraryFolders(folders).catch(() => {});
    const unlistenPromise = listen("library-changed", () => {
      scanLibrary(folders)
        .then(setWallpapers)
        .catch(() => {});
    });
    return () => {
      unlistenPromise.then((unlisten) => unlisten());
      watchLibraryFolders([]).catch(() => {});
    };
  }, [folders]);

  // Runs a single per-command action (assign/pause/play/folder edit).
  // `Response::Error` from a specific command (e.g. "unknown monitor") is
  // not the same failure as the daemon being unreachable -- it's shown
  // inline near the control that triggered it instead of the whole-page
  // ErrorState, per the spec's error-handling split. A DaemonUnreachableError
  // still flips daemonDown, since at that point every control is about to
  // be disabled anyway.
  const runAction = async (fn: () => Promise<void>) => {
    setActionError(null);
    try {
      await fn();
      await refresh();
    } catch (err) {
      if (err instanceof DaemonUnreachableError) {
        setDaemonDown(true);
      } else {
        setActionError(String(err instanceof Error ? err.message : err));
      }
    }
  };

  // Multiple library folders are scanned together (already true on the
  // backend -- `scan_library` always accepted a list); these just add/
  // remove one entry from that list rather than overwriting it outright.
  const addLibraryFolder = (path: string) =>
    runAction(async () => {
      const trimmed = path.trim();
      if (!trimmed || folders.includes(trimmed)) return;
      await setLibraryFolders([...folders, trimmed]);
    });

  const removeLibraryFolder = (path: string) =>
    runAction(async () => {
      await setLibraryFolders(folders.filter((f) => f !== path));
    });

  // Auto-saves as soon as a wallpaper is picked -- no separate "Assign"
  // confirmation step. If no monitor is selected yet, this just records
  // the wallpaper pick (via setSelectedWallpaper) without calling out;
  // picking a monitor afterward doesn't retroactively assign, so the
  // expected flow is monitor(s) first, then wallpaper.
  const selectWallpaper = (path: string) => {
    setSelectedWallpaper(path);
    if (selectedMonitors.size === 0) return;
    if (groupMode && selectedMonitors.size < 2) {
      setActionError("select at least 2 monitors to group");
      return;
    }
    runAction(() => setWallpaper(Array.from(selectedMonitors), path)).then(() => {
      if (groupMode) {
        setGroupMode(false);
        clearSelectedMonitors();
      }
    });
  };

  const toggleGroupMode = () => setGroupMode((g) => !g);

  // If every selected monitor is already playing the exact same path (e.g.
  // right after Ungroup, which re-`Set`s each member solo to the path they
  // shared), Confirm regroups them immediately with that shared path --
  // no reason to force picking a wallpaper again just to reassert what's
  // already showing. Otherwise it just drops the "must have 2+ selected"
  // gate so the next wallpaper pick assigns immediately.
  const confirmGroupSelection = () => {
    const paths = new Set(Array.from(selectedMonitors).map((name) => monitors.find((m) => m.name === name)?.current_path ?? null));
    const sharedPath = paths.size === 1 ? paths.values().next().value : null;
    if (sharedPath) {
      runAction(() => setWallpaper(Array.from(selectedMonitors), sharedPath)).then(() => {
        setGroupMode(false);
        clearSelectedMonitors();
      });
      return;
    }
    setGroupMode(false);
  };

  // Cancel abandons the in-progress selection entirely.
  const cancelGroupMode = () => {
    setGroupMode(false);
    clearSelectedMonitors();
  };

  // Splits every member of `names`' zone back out to its own solo zone,
  // each re-`Set` to the same path the group was already playing -- reuses
  // hyprwalld's existing split-on-resubmit behavior, no dedicated "ungroup"
  // wire command needed.
  const ungroup = (names: string[], path: string) =>
    runAction(async () => {
      for (const name of names) await setWallpaper([name], path);
    });

  const removeWallpaper = (names: string[]) =>
    runAction(async () => {
      for (const name of names) await unsetWallpaper(name);
    });

  return (
    <div
      style={{
        position: "relative",
        display: "flex",
        flexDirection: "column",
        height: "100vh",
        overflow: "hidden",
        fontFamily: "sans-serif",
        color: "#eee",
        background: "#0a0a0a",
      }}
    >
      <TitleBar
        monitorsOpen={monitorsOpen}
        onToggleMonitors={() => setMonitorsOpen((o) => !o)}
        onOpenSettings={() => setSettingsOpen(true)}
      />
      <SettingsModal
        open={settingsOpen}
        onClose={() => setSettingsOpen(false)}
        libraryFolders={folders}
        onAddLibraryFolder={addLibraryFolder}
        onRemoveLibraryFolder={removeLibraryFolder}
      />
      <MonitorsDropdown
        open={monitorsOpen}
        monitors={monitors}
        selected={selectedMonitors}
        onToggle={toggleMonitor}
        onClose={() => setMonitorsOpen(false)}
        groupMode={groupMode}
        onToggleGroupMode={toggleGroupMode}
        onConfirmGroupSelection={confirmGroupSelection}
        onCancelGroupMode={cancelGroupMode}
        onUngroup={ungroup}
        onRemoveWallpaper={removeWallpaper}
      />
      <Sidebar
        path={settingsSidebarPath}
        kind={wallpapers.find((w) => w.path === settingsSidebarPath)?.kind ?? null}
        onClose={() => setSettingsSidebarPath(null)}
      />
      <fieldset
        disabled={daemonDown}
        style={{ border: "none", padding: 16, margin: 0, flex: 1, overflow: "auto" }}
      >
        {daemonDown ? (
          <ErrorState message="hyprwalld is not running or unreachable. Start it — this will recover automatically." />
        ) : (
          <>
            {actionError && (
              <p style={{ color: "#f87171", fontSize: 13 }} role="alert">
                {actionError}
              </p>
            )}

            {folders.length === 0 ? (
              <EmptyLibraryState />
            ) : (
              <LibraryGrid
                wallpapers={wallpapers}
                selected={selectedWallpaper}
                onSelect={selectWallpaper}
                onOpenSettings={toggleSettingsSidebar}
              />
            )}
          </>
        )}
      </fieldset>
    </div>
  );
}
