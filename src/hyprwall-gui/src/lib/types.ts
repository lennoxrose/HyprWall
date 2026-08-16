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
  dominant_color: string | null;
  added_ts: number | null;
}

export type ThemeMode = "dark" | "light";

export interface ThemeColors {
  bg: string;
  bgElevated: string;
  border: string;
  borderHover: string;
  text: string;
  textMuted: string;
  accent: string;
  accentText: string;
  success: string;
  danger: string;
}

export interface ThemeState {
  mode: ThemeMode;
  colors: ThemeColors;
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
