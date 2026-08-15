import { invoke } from "@tauri-apps/api/core";
import type { MonitorState, WallpaperEntry } from "./types";

export class DaemonUnreachableError extends Error {}

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (err) {
    const message = String(err);
    if (message.includes("hyprwalld unreachable")) {
      throw new DaemonUnreachableError(message);
    }
    throw new Error(message);
  }
}

export const listMonitors = () => call<MonitorState[]>("list_monitors");

export const setWallpaper = (monitors: string[], path: string) =>
  call<void>("set_wallpaper", { monitors, path });

export const unsetWallpaper = (monitor: string) => call<void>("unset_wallpaper", { monitor });

export const pauseWallpaper = (monitor: string) => call<void>("pause_wallpaper", { monitor });

export const playWallpaper = (monitor: string) => call<void>("play_wallpaper", { monitor });

export const getLibraryFolders = () => call<string[]>("get_library_folders");

export const setLibraryFolders = (folders: string[]) => call<void>("set_library_folders", { folders });

export const scanLibrary = (folders: string[]) => call<WallpaperEntry[]>("scan_library", { folders });

export const watchLibraryFolders = (folders: string[]) => call<void>("watch_library_folders", { folders });

export const captureMonitorSnapshot = (monitorName: string) =>
  call<string>("capture_monitor_snapshot", { monitorName });
