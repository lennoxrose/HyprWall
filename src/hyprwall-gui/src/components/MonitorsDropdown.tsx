import type { CSSProperties } from "react";
import { DROPDOWN_ANIM_MS, DROPDOWN_EASING, TITLEBAR_BLUE, TITLEBAR_HEIGHT } from "./TitleBar";
import { MonitorLayout } from "./MonitorLayout";
import type { MonitorState } from "../lib/types";

interface Props {
  open: boolean;
  monitors: MonitorState[];
  selected: Set<string>;
  onToggle: (name: string) => void;
  onClose: () => void;
  groupMode: boolean;
  onToggleGroupMode: () => void;
  onConfirmGroupSelection: () => void;
  onCancelGroupMode: () => void;
  onUngroup: (names: string[], path: string) => void;
  onRemoveWallpaper: (names: string[]) => void;
}

const TOOLBAR_BUTTON_STYLE: CSSProperties = {
  border: "none",
  borderRadius: 6,
  background: "#4b5563",
  color: "var(--hw-accent-text)",
  fontSize: 12,
  padding: "6px 10px",
  cursor: "pointer",
};

const NOTCH = 14;
const PANEL_RADIUS = 14;

/** A quarter-circle "fillet" so the navbar's straight bottom edge curves
 * smoothly into the dropdown's straight side edge instead of meeting it at
 * a sharp right angle. `corner` is the notch's own outer corner (the point
 * *away* from both the navbar and the dropdown). Uses `transparent`, not a
 * hardcoded page-background color -- an earlier version blended into a
 * fixed hex assumed to match the page, which broke as soon as the page
 * background became themeable (rendered as a solid square in light mode
 * instead of a curve). Transparent always reveals whatever's actually
 * behind it, correct in any theme. A real child of the sliding panel (not
 * an independently faded overlay), so it physically travels with the slide
 * instead of fading in place. */
function CornerNotch({ side }: { side: "left" | "right" }) {
  const corner = side === "left" ? "bottom left" : "bottom right";
  return (
    <div
      style={{
        position: "absolute",
        top: 0,
        [side]: -NOTCH,
        width: NOTCH,
        height: NOTCH,
        background: `radial-gradient(circle at ${corner}, transparent ${NOTCH}px, ${TITLEBAR_BLUE} ${NOTCH}px)`,
      }}
    />
  );
}

/** Always fully rendered at its real size -- never clipped to zero height --
 * and permanently positioned so it sits directly behind the navbar
 * (TitleBar's z-index is 30, this is 20). Its resting position already
 * places it at `top: TITLEBAR_HEIGHT`; "closed" is `translateY(-100%)`,
 * which (since percentage translateY is relative to the element's own
 * height) tucks the whole thing up behind the bar regardless of how tall
 * its content is, without ever measuring pixels. Opening it is just
 * `translateY(0)` -- the panel literally slides down from behind the
 * navbar into view, rather than a clip/height trick faking the same look. */
export function MonitorsDropdown({
  open,
  monitors,
  selected,
  onToggle,
  onClose,
  groupMode,
  onToggleGroupMode,
  onConfirmGroupSelection,
  onCancelGroupMode,
  onUngroup,
  onRemoveWallpaper,
}: Props) {
  // A monitor in a real (>1-member) zone -- distinct from a solo zone,
  // whose `group` is just `[itself]`.
  const groupOf = (name: string) => {
    const m = monitors.find((mon) => mon.name === name);
    return m && m.group.length > 1 ? m.group : null;
  };

  const canUngroup = Array.from(selected).some((name) => groupOf(name) !== null);
  const canRemoveWallpaper = selected.size > 0;

  const handleUngroup = () => {
    const seen = new Set<string>();
    for (const name of selected) {
      const group = groupOf(name);
      if (!group || seen.has(group.join(","))) continue;
      seen.add(group.join(","));
      const path = monitors.find((m) => m.name === group[0])?.current_path;
      if (path) onUngroup(group, path);
    }
  };

  const handleRemoveWallpaper = () => onRemoveWallpaper(Array.from(selected));

  return (
    <>
      <div
        style={{
          position: "absolute",
          top: TITLEBAR_HEIGHT,
          left: "50%",
          zIndex: 20,
          minWidth: 420,
          // Pre-promotes this element to its own compositing layer before
          // any transition ever runs. Without it, WebKit only promotes a
          // transformed element to a layer the first time a transform
          // transition actually starts -- that promotion happens mid-
          // animation, which is what caused the very first open to visibly
          // jank through most of its duration while every animation after
          // (including every close) was already on a layer and ran smooth.
          willChange: "transform",
          transform: `translateX(-50%) translateY(${open ? "0" : "-100%"})`,
          transition: `transform ${DROPDOWN_ANIM_MS}ms ${DROPDOWN_EASING}`,
        }}
      >
        <CornerNotch side="left" />
        <CornerNotch side="right" />
        <div
          onClick={(e) => e.stopPropagation()}
          style={{
            background: TITLEBAR_BLUE,
            borderBottomLeftRadius: PANEL_RADIUS,
            borderBottomRightRadius: PANEL_RADIUS,
            padding: 14,
          }}
        >
          <div style={{ display: "flex", flexDirection: "column", gap: 8, marginBottom: 10 }}>
            <div style={{ display: "flex", gap: 8 }}>
              {groupMode && selected.size >= 2 ? (
                <div
                  style={{
                    ...TOOLBAR_BUTTON_STYLE,
                    flex: 1,
                    display: "flex",
                    padding: 0,
                    overflow: "hidden",
                  }}
                >
                  <button
                    onClick={onConfirmGroupSelection}
                    style={{ flex: 1, border: "none", background: "transparent", color: "var(--hw-accent-text)", fontSize: 12, padding: "6px 10px", cursor: "pointer" }}
                  >
                    Confirm
                  </button>
                  <div style={{ width: 1, background: "rgba(255,255,255,0.3)" }} />
                  <button
                    onClick={onCancelGroupMode}
                    style={{ flex: 1, border: "none", background: "transparent", color: "var(--hw-accent-text)", fontSize: 12, padding: "6px 10px", cursor: "pointer" }}
                  >
                    Cancel
                  </button>
                </div>
              ) : (
                <button
                  onClick={onToggleGroupMode}
                  style={{
                    ...TOOLBAR_BUTTON_STYLE,
                    flex: 1,
                    background: groupMode ? "#9ca3af" : TOOLBAR_BUTTON_STYLE.background,
                  }}
                >
                  Group
                </button>
              )}
              <button onClick={handleUngroup} disabled={!canUngroup} style={{ ...TOOLBAR_BUTTON_STYLE, flex: 1 }}>
                Ungroup
              </button>
            </div>
            <button onClick={handleRemoveWallpaper} disabled={!canRemoveWallpaper} style={{ ...TOOLBAR_BUTTON_STYLE, width: "100%" }}>
              Remove Wallpaper
            </button>
            {groupMode && (
              <span style={{ color: "rgba(255,255,255,0.75)", fontSize: 12, textAlign: "center" }}>
                {selected.size >= 2
                  ? `${selected.size} selected -- confirm to pick a wallpaper, or cancel`
                  : `select 2+ monitors, then pick a wallpaper (${selected.size} selected)`}
              </span>
            )}
          </div>
          <MonitorLayout monitors={monitors} selected={selected} onToggle={onToggle} open={open} />
        </div>
      </div>
    </>
  );
}
