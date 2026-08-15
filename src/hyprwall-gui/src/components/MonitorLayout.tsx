import { useEffect, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { captureMonitorSnapshot } from "../lib/api";
import { DROPDOWN_ANIM_MS } from "./TitleBar";
import { ErrorState } from "./ErrorState";
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
const SEAM_THICKNESS = 2;
// Kept off the box edges so the seam reads as an inner divider, not
// something drawn on top of the outer border/corners.
const SEAM_INSET = 10;
// Dashes running along the seam itself -- "to bottom" for a vertical seam
// (side-by-side monitors), "to right" for a horizontal one (stacked), so
// the dashes always read as "- - - -" along the boundary, not across it.
const SEAM_DASH_VERTICAL = "repeating-linear-gradient(to bottom, #9ca3af 0 8px, transparent 8px 16px)";
const SEAM_DASH_HORIZONTAL = "repeating-linear-gradient(to right, #9ca3af 0 8px, transparent 8px 16px)";

/** True if `a` sits directly against `b` on the given side, sharing at
 * least one pixel of the perpendicular edge (logical monitor coordinates,
 * not yet scaled to panel px). */
function touches(a: MonitorState, b: MonitorState, side: "left" | "right" | "top" | "bottom"): boolean {
  if (side === "left") return a.x === b.x + b.w && Math.max(a.y, b.y) < Math.min(a.y + a.h, b.y + b.h);
  if (side === "right") return a.x + a.w === b.x && Math.max(a.y, b.y) < Math.min(a.y + a.h, b.y + b.h);
  if (side === "top") return a.y === b.y + b.h && Math.max(a.x, b.x) < Math.min(a.x + a.w, b.x + b.w);
  return a.y + a.h === b.y && Math.max(a.x, b.x) < Math.min(a.x + a.w, b.x + b.w);
}

/** Every other monitor sharing a real (>1-member) zone with `m`. */
function groupmatesOf(m: MonitorState, monitors: MonitorState[]): MonitorState[] {
  if (m.group.length <= 1) return [];
  return monitors.filter((n) => n.name !== m.name && m.group.includes(n.name));
}

function uniqueGroups(monitors: MonitorState[]): string[][] {
  const seen = new Set<string>();
  const groups: string[][] = [];
  for (const m of monitors) {
    if (m.group.length <= 1) continue;
    const key = m.group.join(",");
    if (seen.has(key)) continue;
    seen.add(key);
    groups.push(m.group);
  }
  return groups;
}

export function MonitorLayout({ monitors, selected, onToggle, open }: Props) {
  const [snapshots, setSnapshots] = useState<Record<string, string>>({});

  // Waits for the slide-open transition to fully settle before firing any
  // capture_monitor_snapshot calls. Each capture shells out to grim (real
  // wall-clock time), and the resulting setSnapshots + <img> mount was
  // landing mid-transition -- a repaint colliding with an in-flight CSS
  // animation is exactly what reads as a jump/stutter partway through.
  // Closing never touches this at all, which is why only opening jumped.
  useEffect(() => {
    if (!open) return;
    let cancelled = false;
    const timer = setTimeout(() => {
      for (const m of monitors) {
        captureMonitorSnapshot(m.name)
          .then((path) => {
            if (cancelled) return;
            // No cache-busting query string: Tauri's asset protocol on this
            // platform doesn't resolve a path with `?...` appended (broke
            // image loading entirely). Each open already re-invokes the
            // capture command, so the only staleness risk is the webview's
            // own image cache serving a previous open's bitmap for the
            // exact same path -- acceptable for a "quick snapshot".
            setSnapshots((prev) => ({ ...prev, [m.name]: convertFileSrc(path) }));
          })
          .catch(() => {});
      }
    }, DROPDOWN_ANIM_MS);
    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open]);

  if (monitors.length === 0) {
    return <ErrorState message="no monitors were reported by hyprwalld." />;
  }

  const minX = Math.min(...monitors.map((m) => m.x));
  const minY = Math.min(...monitors.map((m) => m.y));
  const maxX = Math.max(...monitors.map((m) => m.x + m.w));
  const maxY = Math.max(...monitors.map((m) => m.y + m.h));
  const panelW = 480;
  const scale = panelW / (maxX - minX);
  const panelH = (maxY - minY) * scale;

  // Per-monitor box geometry: a side touching a groupmate loses its GAP
  // inset (and that corner's rounding) so the two boxes read as one
  // merged shape instead of two separate rounded rectangles.
  const boxes = new Map(
    monitors.map((m) => {
      const mates = groupmatesOf(m, monitors);
      const mergedLeft = mates.some((n) => touches(m, n, "left"));
      const mergedRight = mates.some((n) => touches(m, n, "right"));
      const mergedTop = mates.some((n) => touches(m, n, "top"));
      const mergedBottom = mates.some((n) => touches(m, n, "bottom"));

      const insetLeft = mergedLeft ? 0 : GAP / 2;
      const insetRight = mergedRight ? 0 : GAP / 2;
      const insetTop = mergedTop ? 0 : GAP / 2;
      const insetBottom = mergedBottom ? 0 : GAP / 2;

      const left = (m.x - minX) * scale + insetLeft;
      const top = (m.y - minY) * scale + insetTop;
      const width = m.w * scale - insetLeft - insetRight;
      const height = m.h * scale - insetTop - insetBottom;

      return [
        m.name,
        {
          left,
          top,
          width,
          height,
          borderTopLeftRadius: mergedTop || mergedLeft ? 0 : RADIUS,
          borderTopRightRadius: mergedTop || mergedRight ? 0 : RADIUS,
          borderBottomLeftRadius: mergedBottom || mergedLeft ? 0 : RADIUS,
          borderBottomRightRadius: mergedBottom || mergedRight ? 0 : RADIUS,
          // No border drawn on a side touching a groupmate -- that's an
          // internal seam (the dashed divider), not the shape's own edge.
          mergedLeft,
          mergedRight,
          mergedTop,
          mergedBottom,
        },
      ] as const;
    }),
  );

  // One seam per grouped pair: a straight hazard-stripe band on the shared
  // border if they're physically touching, or a thinner stripe connector
  // between their centers if the group spans a gap.
  const seams = uniqueGroups(monitors).flatMap((group) => {
    const members = group.map((name) => monitors.find((m) => m.name === name)!).filter(Boolean);
    const segments: { key: string; style: import("react").CSSProperties }[] = [];
    for (let i = 0; i < members.length; i++) {
      for (let j = i + 1; j < members.length; j++) {
        const a = members[i];
        const b = members[j];
        const key = `${a.name}|${b.name}`;
        if (touches(a, b, "right")) {
          const x = (a.x + a.w - minX) * scale;
          const y1 = (Math.max(a.y, b.y) - minY) * scale + SEAM_INSET;
          const y2 = (Math.min(a.y + a.h, b.y + b.h) - minY) * scale - SEAM_INSET;
          segments.push({
            key,
            style: {
              left: x - SEAM_THICKNESS / 2,
              top: y1,
              width: SEAM_THICKNESS,
              height: Math.max(0, y2 - y1),
              background: SEAM_DASH_VERTICAL,
            },
          });
        } else if (touches(a, b, "bottom")) {
          const y = (a.y + a.h - minY) * scale;
          const x1 = (Math.max(a.x, b.x) - minX) * scale + SEAM_INSET;
          const x2 = (Math.min(a.x + a.w, b.x + b.w) - minX) * scale - SEAM_INSET;
          segments.push({
            key,
            style: {
              left: x1,
              top: y - SEAM_THICKNESS / 2,
              width: Math.max(0, x2 - x1),
              height: SEAM_THICKNESS,
              background: SEAM_DASH_HORIZONTAL,
            },
          });
        } else {
          const ax = (a.x - minX + a.w / 2) * scale;
          const ay = (a.y - minY + a.h / 2) * scale;
          const bx = (b.x - minX + b.w / 2) * scale;
          const by = (b.y - minY + b.h / 2) * scale;
          const dx = bx - ax;
          const dy = by - ay;
          const fullLength = Math.hypot(dx, dy);
          const length = Math.max(0, fullLength - SEAM_INSET * 2);
          const angle = (Math.atan2(dy, dx) * 180) / Math.PI;
          const ux = fullLength === 0 ? 0 : dx / fullLength;
          const uy = fullLength === 0 ? 0 : dy / fullLength;
          segments.push({
            key,
            style: {
              left: ax + ux * SEAM_INSET,
              top: ay + uy * SEAM_INSET - SEAM_THICKNESS / 2,
              width: length,
              height: SEAM_THICKNESS,
              background: SEAM_DASH_HORIZONTAL,
              transform: `rotate(${angle}deg)`,
              transformOrigin: "0 50%",
              opacity: 0.85,
            },
          });
        }
      }
    }
    return segments;
  });

  // Grouped monitors act as one clickable unit -- toggling any member
  // selects/deselects the whole group together, matching the merged visual.
  const handleClick = (m: MonitorState) => {
    const mates = groupmatesOf(m, monitors);
    if (mates.length === 0) {
      onToggle(m.name);
      return;
    }
    const group = [m, ...mates];
    const target = !group.every((n) => selected.has(n.name));
    for (const n of group) {
      if (selected.has(n.name) !== target) onToggle(n.name);
    }
  };

  return (
    <div style={{ position: "relative", width: panelW, height: panelH }}>
      {monitors.map((m) => {
        const snapshot = snapshots[m.name];
        const box = boxes.get(m.name)!;
        return (
          <button
            key={m.name}
            onClick={() => handleClick(m)}
            style={{
              position: "absolute",
              left: box.left,
              top: box.top,
              width: box.width,
              height: box.height,
              borderTopLeftRadius: box.borderTopLeftRadius,
              borderTopRightRadius: box.borderTopRightRadius,
              borderBottomLeftRadius: box.borderBottomLeftRadius,
              borderBottomRightRadius: box.borderBottomRightRadius,
              borderTop: box.mergedTop ? "none" : selected.has(m.name) ? "2px solid #4ade80" : "1px solid #555",
              borderBottom: box.mergedBottom ? "none" : selected.has(m.name) ? "2px solid #4ade80" : "1px solid #555",
              borderLeft: box.mergedLeft ? "none" : selected.has(m.name) ? "2px solid #4ade80" : "1px solid #555",
              borderRight: box.mergedRight ? "none" : selected.has(m.name) ? "2px solid #4ade80" : "1px solid #555",
              background: "#3a3a3a",
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
      {seams.map((seam) => (
        <div key={seam.key} style={{ position: "absolute", pointerEvents: "none", ...seam.style }} />
      ))}
    </div>
  );
}
