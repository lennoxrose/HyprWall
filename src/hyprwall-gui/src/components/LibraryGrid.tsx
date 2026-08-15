import { useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { ErrorState } from "./ErrorState";
import type { WallpaperEntry, WallpaperKind } from "../lib/types";

const MUTED = "#555";
const RADIUS = 8; // matches MonitorLayout's monitor-tile RADIUS
const BORDER_REST = "1px solid #333"; // matches SettingsModal's BORDER
const BORDER_HOVER = "1px solid #555"; // matches MonitorLayout's unselected-tile border
const BORDER_SELECTED = "2px solid #4ade80"; // matches MonitorLayout's selected-tile border
const TRANSITION = "border-color 150ms cubic-bezier(0.4, 0, 0.2, 1), transform 150ms cubic-bezier(0.4, 0, 0.2, 1), box-shadow 150ms cubic-bezier(0.4, 0, 0.2, 1)";

/** A small play-triangle-in-a-frame, distinguishing video entries from
 * stills at a glance -- same thin-stroke line style as the app's other
 * custom icons (cog, chevron, warning triangle). */
function VideoKindIcon() {
  return (
    <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="#ccc" strokeWidth="1.8">
      <rect x="2" y="4" width="20" height="16" rx="2" />
      <path d="M10 8.5 L16 12 L10 15.5 Z" fill="#ccc" stroke="none" />
    </svg>
  );
}

/** A small mountain-and-sun picture glyph for still/animated image entries. */
function ImageKindIcon() {
  return (
    <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="#ccc" strokeWidth="1.8">
      <rect x="2" y="4" width="20" height="16" rx="2" />
      <circle cx="8" cy="9.5" r="1.4" fill="#ccc" stroke="none" />
      <path d="M2 17 L9 11 L14 15 L18 11.5 L22 15.5" strokeLinejoin="round" strokeLinecap="round" />
    </svg>
  );
}

/** Same picture glyph as `ImageKindIcon`, struck through -- the grid cell's
 * "nothing to show here" state, scaled down from the app's full-page
 * icon-then-message idiom (`CenteredMessage`) to fit a thumbnail cell. */
function NoPreviewIcon() {
  return (
    <svg width="28" height="28" viewBox="0 0 24 24" fill="none" stroke={MUTED} strokeWidth="1.5">
      <rect x="2" y="4" width="20" height="16" rx="2" />
      <circle cx="8" cy="9.5" r="1.4" fill={MUTED} stroke="none" />
      <path d="M2 17 L9 11 L14 15 L18 11.5 L22 15.5" strokeLinejoin="round" strokeLinecap="round" />
      <line x1="1" y1="21" x2="23" y2="3" strokeLinecap="round" />
    </svg>
  );
}

function KindBadge({ kind }: { kind: WallpaperKind }) {
  return (
    <div
      style={{
        position: "absolute",
        left: 6,
        bottom: 6,
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        width: 20,
        height: 20,
        borderRadius: 5,
        background: "rgba(0,0,0,0.6)",
        backdropFilter: "blur(4px)",
        WebkitBackdropFilter: "blur(4px)",
      }}
    >
      {kind === "video" ? <VideoKindIcon /> : <ImageKindIcon />}
    </div>
  );
}

interface Props {
  wallpapers: WallpaperEntry[];
  selected: string | null;
  onSelect: (path: string) => void;
}

export function LibraryGrid({ wallpapers, selected, onSelect }: Props) {
  const [hovered, setHovered] = useState<string | null>(null);

  if (wallpapers.length === 0) {
    return <ErrorState message="no wallpaper files were found in the configured library folders." />;
  }

  return (
    <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fill, 140px)", gap: 10 }}>
      {wallpapers.map((w) => {
        const isSelected = selected === w.path;
        const isHovered = hovered === w.path;
        return (
          <button
            key={w.path}
            onClick={() => onSelect(w.path)}
            onMouseEnter={() => setHovered(w.path)}
            onMouseLeave={() => setHovered((h) => (h === w.path ? null : h))}
            title={w.path.split("/").pop()}
            style={{
              position: "relative",
              border: isSelected ? BORDER_SELECTED : isHovered ? BORDER_HOVER : BORDER_REST,
              borderRadius: RADIUS,
              padding: 0,
              overflow: "hidden",
              background: "#141414",
              cursor: "pointer",
              transform: isHovered && !isSelected ? "translateY(-2px)" : "none",
              boxShadow: isHovered && !isSelected ? "0 6px 16px rgba(0,0,0,0.5)" : "0 1px 3px rgba(0,0,0,0.4)",
              transition: TRANSITION,
            }}
          >
            <div style={{ position: "relative", width: "100%", height: 90 }}>
              {w.thumbnail_path ? (
                <img
                  src={convertFileSrc(w.thumbnail_path)}
                  alt={w.path}
                  style={{ width: "100%", height: "100%", objectFit: "cover", display: "block" }}
                />
              ) : (
                <div
                  style={{
                    width: "100%",
                    height: "100%",
                    display: "flex",
                    flexDirection: "column",
                    alignItems: "center",
                    justifyContent: "center",
                    gap: 6,
                    color: MUTED,
                  }}
                >
                  <NoPreviewIcon />
                  <span style={{ fontSize: 10 }}>no preview</span>
                </div>
              )}
              <KindBadge kind={w.kind} />
            </div>
            <div
              style={{
                fontSize: 10,
                color: "#ccc",
                padding: "5px 6px",
                overflow: "hidden",
                textOverflow: "ellipsis",
                whiteSpace: "nowrap",
                textAlign: "left",
              }}
            >
              {w.path.split("/").pop()}
            </div>
          </button>
        );
      })}
    </div>
  );
}
