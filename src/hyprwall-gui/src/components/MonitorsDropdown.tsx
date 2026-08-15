import { DROPDOWN_ANIM_MS, TITLEBAR_BLUE, TITLEBAR_HEIGHT } from "./TitleBar";
import { MonitorLayout } from "./MonitorLayout";
import type { MonitorState } from "../lib/types";

interface Props {
  open: boolean;
  monitors: MonitorState[];
  selected: Set<string>;
  onToggle: (name: string) => void;
  onClose: () => void;
  onPause: (monitor: string) => void;
  onPlay: (monitor: string) => void;
}

const PAGE_BG = "#0a0a0a";
const NOTCH = 14;
const PANEL_RADIUS = 14;

/** A quarter-circle "fillet" so the navbar's straight bottom edge curves
 * smoothly into the dropdown's straight side edge instead of meeting it at
 * a sharp right angle. `corner` is the notch's own outer corner (the point
 * *away* from both the navbar and the dropdown) -- that's where the page
 * background shows through; the rest of the box is titlebar blue. Fades in
 * with the slide rather than sitting there permanently -- it's only a
 * meaningful shape while the panel is actually connected to the navbar. */
function CornerNotch({ side, open }: { side: "left" | "right"; open: boolean }) {
  const corner = side === "left" ? "bottom left" : "bottom right";
  return (
    <div
      style={{
        position: "absolute",
        top: 0,
        [side]: -NOTCH,
        width: NOTCH,
        height: NOTCH,
        opacity: open ? 1 : 0,
        transition: `opacity ${DROPDOWN_ANIM_MS}ms ease`,
        background: `radial-gradient(circle at ${corner}, ${PAGE_BG} ${NOTCH}px, ${TITLEBAR_BLUE} ${NOTCH}px)`,
      }}
    />
  );
}

/** Always mounted (not conditionally rendered) so it can genuinely slide
 * out from *behind* the titlebar rather than just appearing: it sits at
 * the same blue as the navbar, one z-index behind it (TitleBar is 30, this
 * is 20), clipped to zero height by default. Opening it grows
 * `grid-template-rows` from `0fr` to `1fr` -- an auto-height-friendly CSS
 * animation, no measured pixel height needed -- so the panel reads as
 * unrolling downward from under the navbar's bottom edge, not popping up
 * as a separate surface next to it. */
export function MonitorsDropdown({ open, monitors, selected, onToggle, onClose, onPause, onPlay }: Props) {
  return (
    <>
      {open && (
        <div onClick={onClose} style={{ position: "fixed", inset: 0, top: TITLEBAR_HEIGHT, zIndex: 10 }} />
      )}
      <div
        style={{
          position: "absolute",
          top: TITLEBAR_HEIGHT,
          left: "50%",
          transform: "translateX(-50%)",
          zIndex: 20,
          display: "grid",
          gridTemplateRows: open ? "1fr" : "0fr",
          transition: `grid-template-rows ${DROPDOWN_ANIM_MS}ms ease`,
        }}
      >
        {/* Siblings of the overflow:hidden row below, not children of it --
            that div must clip vertically for the grid-row animation to
            work, which would also clip these horizontally if they lived
            inside it. Absolute positioning anchors them to this outer,
            unclipped container instead. */}
        <CornerNotch side="left" open={open} />
        <CornerNotch side="right" open={open} />
        <div style={{ overflow: "hidden", minWidth: 420 }}>
          <div
            onClick={(e) => e.stopPropagation()}
            style={{
              background: TITLEBAR_BLUE,
              borderBottomLeftRadius: PANEL_RADIUS,
              borderBottomRightRadius: PANEL_RADIUS,
              boxShadow: "0 8px 24px rgba(0,0,0,0.5)",
              padding: 14,
            }}
          >
            <MonitorLayout monitors={monitors} selected={selected} onToggle={onToggle} open={open} />
            {Array.from(selected).map((name) => {
              const m = monitors.find((mon) => mon.name === name);
              if (!m?.current_path) return null;
              return (
                <div key={name} style={{ marginTop: 8, color: "#fff", fontSize: 13 }}>
                  {name}: <button onClick={() => onPause(name)}>Pause</button>{" "}
                  <button onClick={() => onPlay(name)}>Play</button>
                </div>
              );
            })}
          </div>
        </div>
      </div>
    </>
  );
}
