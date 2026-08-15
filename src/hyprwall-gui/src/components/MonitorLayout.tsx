import { useEffect, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { captureMonitorSnapshot } from "../lib/api";
import type { MonitorState } from "../lib/types";

interface Props {
  monitors: MonitorState[];
  selected: Set<string>;
  onToggle: (name: string) => void;
  /** The dropdown this layout lives in is always mounted (it slides open/
   * closed rather than mounting/unmounting), so "on mount" can't mean
   * "when opened" anymore -- snapshots are (re-)fetched each time this
   * flips from closed to open instead. */
  open: boolean;
}

const GAP = 6;
const RADIUS = 8;

export function MonitorLayout({ monitors, selected, onToggle, open }: Props) {
  const [snapshots, setSnapshots] = useState<Record<string, string>>({});

  useEffect(() => {
    if (!open) return;
    let cancelled = false;
    for (const m of monitors) {
      captureMonitorSnapshot(m.name)
        .then((path) => {
          if (cancelled) return;
          // No cache-busting query string: Tauri's asset protocol on this
          // platform doesn't resolve a path with `?...` appended (broke
          // image loading entirely). Each mount already re-invokes the
          // capture command, so the only staleness risk is the webview's
          // own image cache serving a previous open's bitmap for the exact
          // same path -- acceptable for a "quick snapshot", not a live feed.
          setSnapshots((prev) => ({ ...prev, [m.name]: convertFileSrc(path) }));
        })
        .catch(() => {});
    }
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open]);

  if (monitors.length === 0) {
    return <p>No monitors reported by hyprwalld.</p>;
  }

  const minX = Math.min(...monitors.map((m) => m.x));
  const minY = Math.min(...monitors.map((m) => m.y));
  const maxX = Math.max(...monitors.map((m) => m.x + m.w));
  const maxY = Math.max(...monitors.map((m) => m.y + m.h));
  const panelW = 480;
  const scale = panelW / (maxX - minX);
  const panelH = (maxY - minY) * scale;

  return (
    <div style={{ position: "relative", width: panelW, height: panelH }}>
      {monitors.map((m) => {
        const snapshot = snapshots[m.name];
        return (
          <button
            key={m.name}
            onClick={() => onToggle(m.name)}
            style={{
              position: "absolute",
              left: (m.x - minX) * scale + GAP / 2,
              top: (m.y - minY) * scale + GAP / 2,
              width: m.w * scale - GAP,
              height: m.h * scale - GAP,
              borderRadius: RADIUS,
              border: selected.has(m.name) ? "2px solid #4ade80" : "1px solid #555",
              background: snapshot ? "#000" : m.current_path ? "#1e3a2e" : "#222",
              color: "#eee",
              fontSize: 12,
              cursor: "pointer",
              overflow: "hidden",
              padding: 0,
            }}
          >
            {snapshot && (
              <img
                src={snapshot}
                alt=""
                style={{
                  position: "absolute",
                  inset: 0,
                  width: "100%",
                  height: "100%",
                  objectFit: "cover",
                  borderRadius: RADIUS - 1,
                }}
              />
            )}
            <div
              style={{
                position: "relative",
                textShadow: snapshot ? "0 1px 3px rgba(0,0,0,0.9)" : "none",
              }}
            >
              {m.name}
              {m.current_path && <div style={{ fontSize: 10, opacity: 0.85 }}>assigned</div>}
            </div>
          </button>
        );
      })}
    </div>
  );
}
