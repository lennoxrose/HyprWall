import { useCallback, useEffect, useState } from "react";
import { MonitorLayout } from "./components/MonitorLayout";
import { LibraryGrid } from "./components/LibraryGrid";
import { AssignButton } from "./components/AssignButton";
import { StatusBanner } from "./components/StatusBanner";
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
  // inline near the control that triggered it instead of the persistent
  // StatusBanner, per the spec's error-handling split. A DaemonUnreachableError
  // still flips the banner, since at that point every control is about to be
  // disabled anyway.
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

  const assign = () =>
    runAction(() => {
      if (selectedMonitors.size === 0 || !selectedWallpaper) return Promise.resolve();
      return setWallpaper(Array.from(selectedMonitors), selectedWallpaper);
    });

  const pause = (monitor: string) => runAction(() => pauseWallpaper(monitor));
  const play = (monitor: string) => runAction(() => playWallpaper(monitor));

  return (
    <div style={{ fontFamily: "sans-serif", color: "#eee", background: "#0a0a0a", minHeight: "100vh" }}>
      {daemonDown && <StatusBanner />}
      <fieldset disabled={daemonDown} style={{ border: "none", padding: 16 }}>
        <h1>HyprWall</h1>
        {actionError && (
          <p style={{ color: "#f87171", fontSize: 13 }} role="alert">
            {actionError}
          </p>
        )}

        <section>
          <h2>Monitors</h2>
          <MonitorLayout monitors={monitors} selected={selectedMonitors} onToggle={toggleMonitor} />
          {Array.from(selectedMonitors).map((name) => {
            const m = monitors.find((mon) => mon.name === name);
            if (!m?.current_path) return null;
            return (
              <div key={name}>
                {name}: <button onClick={() => pause(name)}>Pause</button>
                <button onClick={() => play(name)}>Play</button>
              </div>
            );
          })}
        </section>

        <section>
          <h2>Library folders</h2>
          <ul>
            {folders.map((f) => (
              <li key={f}>
                {f} <button onClick={() => removeFolder(f)}>Remove</button>
              </li>
            ))}
          </ul>
          <input
            value={newFolder}
            onChange={(e) => setNewFolder(e.target.value)}
            placeholder="/absolute/path"
          />
          <button onClick={addFolder}>Add folder</button>
        </section>

        <section>
          <h2>Wallpapers</h2>
          <LibraryGrid wallpapers={wallpapers} selected={selectedWallpaper} onSelect={setSelectedWallpaper} />
        </section>

        <AssignButton disabled={selectedMonitors.size === 0 || !selectedWallpaper} onClick={assign} />
      </fieldset>
    </div>
  );
}
