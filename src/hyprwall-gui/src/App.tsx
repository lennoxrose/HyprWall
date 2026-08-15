import { useCallback, useEffect, useState } from "react";
import { TitleBar } from "./components/TitleBar";
import { MonitorsDropdown } from "./components/MonitorsDropdown";
import { SettingsModal } from "./components/SettingsModal";
import { LibraryGrid } from "./components/LibraryGrid";
import { EmptyLibraryState } from "./components/EmptyLibraryState";
import { ErrorState } from "./components/ErrorState";
import { useSelection } from "./state/selection";
import {
  DaemonUnreachableError,
  getLibraryFolders,
  listMonitors,
  pauseWallpaper,
  playWallpaper,
  scanLibrary,
  setLibraryFolders,
  setWallpaper,
} from "./lib/api";
import type { MonitorState, WallpaperEntry } from "./lib/types";

export default function App() {
  const [monitors, setMonitors] = useState<MonitorState[]>([]);
  const [wallpapers, setWallpapers] = useState<WallpaperEntry[]>([]);
  const [folders, setFolders] = useState<string[]>([]);
  const [newFolder, setNewFolder] = useState("");
  const [daemonDown, setDaemonDown] = useState(false);
  const [actionError, setActionError] = useState<string | null>(null);
  const [monitorsOpen, setMonitorsOpen] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const { selectedMonitors, toggleMonitor, selectedWallpaper, setSelectedWallpaper } = useSelection();

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

  const addFolder = () =>
    runAction(async () => {
      if (!newFolder.trim()) return;
      await setLibraryFolders([...folders, newFolder.trim()]);
      setNewFolder("");
    });

  const removeFolder = (folder: string) =>
    runAction(() => setLibraryFolders(folders.filter((f) => f !== folder)));

  // Auto-saves as soon as a wallpaper is picked -- no separate "Assign"
  // confirmation step. If no monitor is selected yet, this just records
  // the wallpaper pick (via setSelectedWallpaper) without calling out;
  // picking a monitor afterward doesn't retroactively assign, so the
  // expected flow is monitor(s) first, then wallpaper.
  const selectWallpaper = (path: string) => {
    setSelectedWallpaper(path);
    if (selectedMonitors.size === 0) return;
    runAction(() => setWallpaper(Array.from(selectedMonitors), path));
  };

  const pause = (monitor: string) => runAction(() => pauseWallpaper(monitor));
  const play = (monitor: string) => runAction(() => playWallpaper(monitor));

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
        folders={folders}
        newFolder={newFolder}
        onNewFolderChange={setNewFolder}
        onAddFolder={addFolder}
        onRemoveFolder={removeFolder}
      />
      <MonitorsDropdown
        open={monitorsOpen}
        monitors={monitors}
        selected={selectedMonitors}
        onToggle={toggleMonitor}
        onClose={() => setMonitorsOpen(false)}
        onPause={pause}
        onPlay={play}
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
              <LibraryGrid wallpapers={wallpapers} selected={selectedWallpaper} onSelect={selectWallpaper} />
            )}
          </>
        )}
      </fieldset>
    </div>
  );
}
