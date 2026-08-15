export const TITLEBAR_HEIGHT = 30;
export const TITLEBAR_BLUE = "#2563eb";
export const NERD_FONT = '"JetBrainsMono Nerd Font", "JetBrains Mono", monospace';
/** Shared pace for every Monitors-dropdown-related animation (panel slide,
 * corner notch fade, chevron rotation) -- opening and closing both use this
 * same duration, deliberately not asymmetric. */
export const DROPDOWN_ANIM_MS = 380;

function CogIcon() {
  return (
    <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
      <circle cx="12" cy="12" r="3" />
      <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z" />
    </svg>
  );
}

/** Sharp double-chevron, mitered corners -- deliberately not the rounded
 * pill style of a typical icon-font glyph. `direction="up"` is used while
 * the Monitors dropdown is open. */
function DoubleChevron({ direction = "down" }: { direction?: "down" | "up" }) {
  return (
    <svg
      width="13"
      height="13"
      viewBox="0 0 24 24"
      fill="none"
      style={{
        transform: direction === "up" ? "rotate(180deg)" : "rotate(0deg)",
        transition: `transform ${DROPDOWN_ANIM_MS}ms ease`,
      }}
    >
      <path d="M3 5 L12 12 L21 5" stroke="currentColor" strokeWidth="2.5" strokeLinecap="butt" strokeLinejoin="miter" />
      <path d="M3 13 L12 20 L21 13" stroke="currentColor" strokeWidth="2.5" strokeLinecap="butt" strokeLinejoin="miter" />
    </svg>
  );
}

interface Props {
  monitorsOpen: boolean;
  onToggleMonitors: () => void;
}

export function TitleBar({ monitorsOpen, onToggleMonitors }: Props) {
  return (
    <div
      data-tauri-drag-region
      style={{
        position: "relative",
        zIndex: 30,
        display: "flex",
        alignItems: "center",
        height: TITLEBAR_HEIGHT,
        background: TITLEBAR_BLUE,
        color: "#fff",
        fontFamily: NERD_FONT,
        fontSize: 13,
        userSelect: "none",
        flexShrink: 0,
      }}
    >
      <div style={{ display: "flex", alignItems: "baseline", gap: 7, paddingLeft: 10 }}>
        <span style={{ fontWeight: 700, letterSpacing: 0.3 }}>HyprWall</span>
        <span style={{ color: "rgba(255,255,255,0.6)", fontWeight: 400, fontSize: 12 }}>| v0.1.0</span>
      </div>

      <button
        onClick={onToggleMonitors}
        aria-expanded={monitorsOpen}
        style={{
          position: "absolute",
          left: "50%",
          top: 0,
          height: "100%",
          transform: "translateX(-50%)",
          display: "flex",
          alignItems: "center",
          gap: 7,
          border: "none",
          background: monitorsOpen ? "rgba(255,255,255,0.14)" : "transparent",
          color: "#fff",
          fontFamily: NERD_FONT,
          fontSize: 13,
          fontWeight: 600,
          padding: "0 12px",
          cursor: "pointer",
        }}
      >
        <DoubleChevron direction={monitorsOpen ? "up" : "down"} />
        Monitors
        <DoubleChevron direction={monitorsOpen ? "up" : "down"} />
      </button>

      <div style={{ marginLeft: "auto", display: "flex", alignItems: "center", paddingRight: 10 }}>
        <CogIcon />
      </div>
    </div>
  );
}
