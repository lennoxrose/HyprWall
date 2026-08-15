import type { MonitorState } from "../lib/types";

interface Props {
  monitors: MonitorState[];
  selected: Set<string>;
  onToggle: (name: string) => void;
}

export function MonitorLayout({ monitors, selected, onToggle }: Props) {
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
    <div style={{ position: "relative", width: panelW, height: panelH, background: "#111" }}>
      {monitors.map((m) => (
        <button
          key={m.name}
          onClick={() => onToggle(m.name)}
          style={{
            position: "absolute",
            left: (m.x - minX) * scale,
            top: (m.y - minY) * scale,
            width: m.w * scale,
            height: m.h * scale,
            border: selected.has(m.name) ? "2px solid #4ade80" : "1px solid #555",
            background: m.current_path ? "#1e3a2e" : "#222",
            color: "#eee",
            fontSize: 12,
            cursor: "pointer",
          }}
        >
          {m.name}
          {m.current_path && <div style={{ fontSize: 10, opacity: 0.7 }}>assigned</div>}
        </button>
      ))}
    </div>
  );
}
