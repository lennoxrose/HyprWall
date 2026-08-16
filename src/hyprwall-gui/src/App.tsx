import { useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { TitleBar } from "./components/TitleBar";
import { MonitorsDropdown } from "./components/MonitorsDropdown";
import { SettingsModal } from "./components/SettingsModal";
import { LibraryGrid } from "./components/LibraryGrid";
import { SearchBar } from "./components/SearchBar";
import { FilterIcon, FilterPanel, emptyFilterState, isFilterActive, type FilterState } from "./components/FilterPanel";
import { dateBucket, nearestColorBucket } from "./lib/colorBuckets";
import { Sidebar } from "./components/Sidebar";
import { EmptyLibraryState } from "./components/EmptyLibraryState";
import { ErrorState } from "./components/ErrorState";
import { useSelection } from "./state/selection";
import { useTheme } from "./state/theme";
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
  const [searchQuery, setSearchQuery] = useState("");
  const [filters, setFilters] = useState<FilterState>(emptyFilterState());
  const [filterOpen, setFilterOpen] = useState(false);
  const { theme, setMode, setColor, resetToDefaults } = useTheme();

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

  const filteredWallpapers = wallpapers.filter((w) => {
    if (!(w.path.split("/").pop() ?? "").toLowerCase().includes(searchQuery.trim().toLowerCase())) return false;
    if (filters.kinds.size > 0 && !filters.kinds.has(w.kind)) return false;
    if (filters.colors.size > 0 && (!w.dominant_color || !filters.colors.has(nearestColorBucket(w.dominant_color))))
      return false;
    if (filters.dateBucket !== null && (w.added_ts === null || dateBucket(w.added_ts) !== filters.dateBucket))
      return false;
    return true;
  });

  // Distinguishes *why* the grid is empty -- a genuinely empty library reads
  // very differently from "your search/filters excluded everything", and
  // conflating them produced a broken-looking `no wallpapers match "".`
  // when a filter (not the search box) was what excluded every entry.
  const emptyResultsMessage = () => {
    if (wallpapers.length === 0) return "no wallpaper files were found in the configured library folders.";
    const reasons: string[] = [];
    if (searchQuery.trim()) reasons.push(`"${searchQuery.trim()}"`);
    if (isFilterActive(filters)) reasons.push("the selected filters");
    return reasons.length > 0 ? `no wallpapers match ${reasons.join(" and ")}.` : "no wallpapers match.";
  };

  return (
    <div
      style={{
        position: "relative",
        display: "flex",
        flexDirection: "column",
        height: "100vh",
        overflow: "hidden",
        fontFamily: "sans-serif",
        color: "var(--hw-text)",
        background: "var(--hw-bg)",
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
        theme={theme}
        onSetThemeMode={setMode}
        onSetThemeColor={setColor}
        onResetTheme={resetToDefaults}
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
              <p style={{ color: "var(--hw-danger)", fontSize: 13 }} role="alert">
                {actionError}
              </p>
            )}

            {folders.length === 0 ? (
              <EmptyLibraryState />
            ) : (
              <>
                <div style={{ display: "flex", gap: 8, marginBottom: 12 }}>
                  <SearchBar value={searchQuery} onChange={setSearchQuery} />
                  <div style={{ position: "relative", flex: 0.5 }}>
                    <button
                      onClick={() => setFilterOpen((o) => !o)}
                      aria-expanded={filterOpen}
                      style={{
                        width: "100%",
                        height: "100%",
                        display: "flex",
                        alignItems: "center",
                        justifyContent: "center",
                        gap: 6,
                        border: isFilterActive(filters)
                          ? "1px solid var(--hw-success)"
                          : "1px solid var(--hw-border)",
                        borderRadius: 4,
                        background: "transparent",
                        color: isFilterActive(filters) ? "var(--hw-success)" : "var(--hw-text)",
                        fontSize: 13,
                        cursor: "pointer",
                      }}
                    >
                      <FilterIcon />
                      Filter{isFilterActive(filters) ? ` (${filters.kinds.size + filters.colors.size + (filters.dateBucket ? 1 : 0)})` : ""}
                    </button>
                    <FilterPanel
                      open={filterOpen}
                      onClose={() => setFilterOpen(false)}
                      filters={filters}
                      onChange={setFilters}
                    />
                  </div>
                </div>
                {filteredWallpapers.length === 0 ? (
                  <ErrorState message={emptyResultsMessage()} />
                ) : (
                  <LibraryGrid
                    wallpapers={filteredWallpapers}
                    selected={selectedWallpaper}
                    onSelect={selectWallpaper}
                    onOpenSettings={toggleSettingsSidebar}
                  />
                )}
              </>
            )}
          </>
        )}
      </fieldset>
    </div>
  );
}
