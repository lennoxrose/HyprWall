import { convertFileSrc } from "@tauri-apps/api/core";
import { ErrorState } from "./ErrorState";
import type { WallpaperEntry } from "../lib/types";

interface Props {
  wallpapers: WallpaperEntry[];
  selected: string | null;
  onSelect: (path: string) => void;
}

export function LibraryGrid({ wallpapers, selected, onSelect }: Props) {
  if (wallpapers.length === 0) {
    return <ErrorState message="no wallpaper files were found in the configured library folders." />;
  }

  return (
    <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fill, 140px)", gap: 8 }}>
      {wallpapers.map((w) => (
        <button
          key={w.path}
          onClick={() => onSelect(w.path)}
          style={{
            border: selected === w.path ? "2px solid #4ade80" : "1px solid #444",
            padding: 0,
            background: "#181818",
            cursor: "pointer",
          }}
        >
          {w.thumbnail_path ? (
            <img
              src={convertFileSrc(w.thumbnail_path)}
              alt={w.path}
              style={{ width: "100%", height: 90, objectFit: "cover" }}
            />
          ) : (
            <div
              style={{
                width: "100%",
                height: 90,
                display: "flex",
                alignItems: "center",
                justifyContent: "center",
                color: "#888",
                fontSize: 10,
              }}
            >
              no preview
            </div>
          )}
          <div style={{ fontSize: 10, color: "#ccc", padding: 4, wordBreak: "break-all" }}>
            {w.path.split("/").pop()}
          </div>
        </button>
      ))}
    </div>
  );
}
