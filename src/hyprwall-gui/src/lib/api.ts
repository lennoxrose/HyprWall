import { invoke } from "@tauri-apps/api/core";
import type { FitMode, MonitorState, WallpaperEntry, WallpaperSettings } from "./types";

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

export const getDefaultFitMode = () => call<FitMode>("get_default_fit_mode");

export const setDefaultFitMode = (fit: FitMode) => call<void>("set_default_fit_mode", { fit });

export const scanLibrary = (folders: string[]) => call<WallpaperEntry[]>("scan_library", { folders });

export const watchLibraryFolders = (folders: string[]) => call<void>("watch_library_folders", { folders });

export const captureMonitorSnapshot = (monitorName: string) =>
  call<string>("capture_monitor_snapshot", { monitorName });

export const getBackgroundServiceEnabled = () => call<boolean>("get_background_service_enabled");

export const setBackgroundServiceEnabled = (enabled: boolean) =>
  call<void>("set_background_service_enabled", { enabled });

export const getStartOnLoginEnabled = () => call<boolean>("get_start_on_login_enabled");

export const setStartOnLoginEnabled = (enabled: boolean) =>
  call<void>("set_start_on_login_enabled", { enabled });

export const getWallpaperSettings = (path: string) =>
  call<WallpaperSettings>("get_wallpaper_settings", { path });

export const setWallpaperSettings = (path: string, settings: WallpaperSettings) =>
  call<void>("set_wallpaper_settings", { path, settings });

export const hasAudioTrack = (path: string) => call<boolean>("has_audio_track", { path });
