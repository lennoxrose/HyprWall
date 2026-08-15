import { TITLEBAR_BLUE, TITLEBAR_HEIGHT } from "./TitleBar";
import { MonitorLayout } from "./MonitorLayout";
import type { MonitorState } from "../lib/types";

interface Props {
  monitors: MonitorState[];
  selected: Set<string>;
  onToggle: (name: string) => void;
  onClose: () => void;
  onPause: (monitor: string) => void;
  onPlay: (monitor: string) => void;
}

/** Drops down directly beneath the titlebar's "Monitors" trigger, sharing
 * its blue as a top accent so it reads as part of the navbar rather than a
 * disconnected popup. Closes on outside click via the full-screen backdrop. */
export function MonitorsDropdown({ monitors, selected, onToggle, onClose, onPause, onPlay }: Props) {
  return (
    <>
      <div
        onClick={onClose}
        style={{ position: "fixed", inset: 0, top: TITLEBAR_HEIGHT, zIndex: 10 }}
      />
      <div
        onClick={(e) => e.stopPropagation()}
        style={{
          position: "absolute",
          top: TITLEBAR_HEIGHT,
          left: "50%",
          transform: "translateX(-50%)",
          zIndex: 20,
          background: "#111",
          borderTop: `2px solid ${TITLEBAR_BLUE}`,
          borderLeft: "1px solid #222",
          borderRight: "1px solid #222",
          borderBottom: "1px solid #222",
          boxShadow: "0 8px 24px rgba(0,0,0,0.5)",
          padding: 14,
          minWidth: 420,
        }}
      >
        <MonitorLayout monitors={monitors} selected={selected} onToggle={onToggle} />
        {Array.from(selected).map((name) => {
          const m = monitors.find((mon) => mon.name === name);
          if (!m?.current_path) return null;
          return (
            <div key={name} style={{ marginTop: 8, color: "#eee", fontSize: 13 }}>
              {name}: <button onClick={() => onPause(name)}>Pause</button>{" "}
              <button onClick={() => onPlay(name)}>Play</button>
            </div>
          );
        })}
      </div>
    </>
  );
}
