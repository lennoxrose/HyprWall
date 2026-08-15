export interface MonitorState {
  name: string;
  x: number;
  y: number;
  w: number;
  h: number;
  current_path: string | null;
  group: string[];
}

export type WallpaperKind = "video" | "image";

export interface WallpaperEntry {
  path: string;
  thumbnail_path: string | null;
  kind: WallpaperKind;
}

export type FitMode = "cover" | "contain" | "stretch";

export interface WallpaperSettings {
  zoom: number;
  pan_x: number;
  pan_y: number;
  fit: FitMode;
  volume: number;
  brightness: number;
  contrast: number;
  hue: number;
  saturation: number;
}
