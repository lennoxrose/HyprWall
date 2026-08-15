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
