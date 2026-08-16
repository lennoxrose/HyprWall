import { THEME_TOKENS } from "../lib/theme";
import type { ThemeColors, ThemeMode, ThemeState } from "../lib/types";

interface Props {
  theme: ThemeState;
  onSetMode: (mode: ThemeMode) => void;
  onSetColor: (key: keyof ThemeColors, value: string) => void;
  onReset: () => void;
}

function ModeButton({ label, active, onClick }: { label: string; active: boolean; onClick: () => void }) {
  return (
    <button
      onClick={onClick}
      aria-pressed={active}
      style={{
        flex: 1,
        border: active ? "1px solid var(--hw-success)" : "1px solid var(--hw-border)",
        borderRadius: 6,
        background: active ? "rgba(74,222,128,0.14)" : "transparent",
        color: active ? "var(--hw-success)" : "var(--hw-text-muted)",
        fontSize: 12,
        fontWeight: 600,
        padding: "6px 0",
        cursor: "pointer",
      }}
    >
      {label}
    </button>
  );
}

export function StyleSettings({ theme, onSetMode, onSetColor, onReset }: Props) {
  return (
    <div>
      <div style={{ fontSize: 13, fontWeight: 500, color: "var(--hw-text-muted)", marginBottom: 8 }}>Mode</div>
      <div style={{ display: "flex", gap: 6, marginBottom: 16 }}>
        <ModeButton label="Dark" active={theme.mode === "dark"} onClick={() => onSetMode("dark")} />
        <ModeButton label="Light" active={theme.mode === "light"} onClick={() => onSetMode("light")} />
      </div>

      <div style={{ fontSize: 13, fontWeight: 500, color: "var(--hw-text-muted)", marginBottom: 8 }}>Colors</div>
      <div style={{ display: "flex", flexDirection: "column", gap: 4, marginBottom: 12 }}>
        {THEME_TOKENS.map(({ key, label }) => (
          <div
            key={key}
            style={{ display: "flex", alignItems: "center", justifyContent: "space-between", padding: "4px 2px" }}
          >
            <span style={{ fontSize: 12, color: "var(--hw-text-muted)" }}>{label}</span>
            <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
              <span style={{ fontSize: 11, color: "var(--hw-text-muted)", fontFamily: "monospace" }}>{theme.colors[key]}</span>
              <input
                type="color"
                aria-label={label}
                value={theme.colors[key]}
                onChange={(e) => onSetColor(key, e.target.value)}
                style={{
                  width: 28,
                  height: 20,
                  padding: 0,
                  border: "1px solid var(--hw-border)",
                  borderRadius: 4,
                  background: "transparent",
                  cursor: "pointer",
                }}
              />
            </div>
          </div>
        ))}
      </div>

      <button
        onClick={onReset}
        style={{ border: "none", background: "transparent", color: "var(--hw-text-muted)", fontSize: 12, cursor: "pointer", padding: 0 }}
      >
        Reset to {theme.mode} defaults
      </button>
    </div>
  );
}
