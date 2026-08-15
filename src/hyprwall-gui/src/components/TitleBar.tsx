import { getCurrentWindow } from "@tauri-apps/api/window";

const NERD_FONT = '"JetBrainsMono Nerd Font", "JetBrains Mono", monospace';
const TITLEBAR_BLUE = "#2563eb";

function CogIcon() {
  return (
    <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
      <circle cx="12" cy="12" r="3" />
      <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z" />
    </svg>
  );
}

/** Sharp double-chevron, mitered corners -- deliberately not the rounded
 * pill style of a typical icon-font glyph. */
function DoubleChevronDown({ style }: { style?: React.CSSProperties }) {
  return (
    <svg width="10" height="10" viewBox="0 0 24 24" fill="none" style={style}>
      <path d="M3 5 L12 12 L21 5" stroke="currentColor" strokeWidth="2.5" strokeLinecap="butt" strokeLinejoin="miter" />
      <path d="M3 13 L12 20 L21 13" stroke="currentColor" strokeWidth="2.5" strokeLinecap="butt" strokeLinejoin="miter" />
    </svg>
  );
}

export function TitleBar() {
  return (
    <div
      data-tauri-drag-region
      style={{
        position: "relative",
        display: "flex",
        alignItems: "center",
        height: 20,
        background: TITLEBAR_BLUE,
        color: "#fff",
        fontFamily: NERD_FONT,
        fontSize: 11,
        userSelect: "none",
        flexShrink: 0,
      }}
    >
      <div style={{ display: "flex", alignItems: "center", gap: 6, paddingLeft: 8 }}>
        <CogIcon />
        <span style={{ fontWeight: 700, letterSpacing: 0.2 }}>HyprWall</span>
        <span style={{ color: "rgba(255,255,255,0.55)", fontWeight: 400 }}>| v0.1.0</span>
      </div>

      <div
        style={{
          position: "absolute",
          left: "50%",
          top: "50%",
          transform: "translate(-50%, -50%)",
          display: "flex",
          alignItems: "center",
          gap: 6,
        }}
      >
        <DoubleChevronDown />
        <span style={{ fontWeight: 600 }}>Monitors</span>
        <DoubleChevronDown />
      </div>

      <button
        onClick={() => getCurrentWindow().close()}
        aria-label="Close"
        style={{
          marginLeft: "auto",
          width: 20,
          height: 20,
          border: "none",
          background: "transparent",
          color: "#fff",
          fontFamily: NERD_FONT,
          fontSize: 12,
          lineHeight: "20px",
          cursor: "pointer",
        }}
      >
        ×
      </button>
    </div>
  );
}
