import { DROPDOWN_ANIM_MS, DROPDOWN_EASING, TITLEBAR_BLUE, TITLEBAR_HEIGHT } from "./TitleBar";
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
 * background shows through; the rest of the box is titlebar blue. A real
 * child of the sliding panel (not an independently faded overlay), so it
 * physically travels with the slide instead of fading in place. */
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
        background: `radial-gradient(circle at ${corner}, ${PAGE_BG} ${NOTCH}px, ${TITLEBAR_BLUE} ${NOTCH}px)`,
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
    </>
  );
}
