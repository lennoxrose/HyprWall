import { useEffect, useState } from "react";
import { NERD_FONT, TITLEBAR_BLUE } from "./TitleBar";
import { CreditsPanel } from "./CreditsPanel";
import { StyleSettings } from "./StyleSettings";
import {
  getBackgroundServiceEnabled,
  getStartOnLoginEnabled,
  setBackgroundServiceEnabled,
  setStartOnLoginEnabled,
} from "../lib/api";
import type { ThemeColors, ThemeMode, ThemeState } from "../lib/types";

interface Props {
  open: boolean;
  onClose: () => void;
  libraryFolders: string[];
  onAddLibraryFolder: (path: string) => void;
  onRemoveLibraryFolder: (path: string) => void;
  theme: ThemeState;
  onSetThemeMode: (mode: ThemeMode) => void;
  onSetThemeColor: (key: keyof ThemeColors, value: string) => void;
  onResetTheme: () => void;
}

interface Category {
  id: string;
  label: string;
}

const CATEGORIES: Category[] = [
  { id: "system", label: "System" },
  { id: "startup", label: "Startup" },
  { id: "style", label: "Style" },
  { id: "credits", label: "Credits" },
];

const BORDER = "1px solid var(--hw-border)";

/** A labeled on/off pill, used for every systemd-backed toggle on the
 * Startup tab -- green "Enabled" when on, matching the app's selected-state
 * color elsewhere (`LibraryGrid`'s selected border, `MonitorLayout`'s
 * selected tile). */
function ToggleRow({
  label,
  enabled,
  onToggle,
  error,
}: {
  label: string;
  enabled: boolean | null;
  onToggle: () => void;
  error: string | null;
}) {
  return (
    <div style={{ marginTop: 6 }}>
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", padding: 5 }}>
        <span style={{ fontSize: 13, fontWeight: 500, color: "var(--hw-text-muted)" }}>{label}</span>
        <button
          onClick={onToggle}
          disabled={enabled === null}
          style={{
            border: "none",
            borderRadius: 6,
            padding: "5px 12px",
            fontSize: 12,
            fontWeight: 600,
            cursor: enabled === null ? "default" : "pointer",
            background: enabled ? "var(--hw-success)" : "var(--hw-bg-elevated)",
            color: enabled ? "var(--hw-bg)" : "var(--hw-text-muted)",
          }}
        >
          {enabled === null ? "..." : enabled ? "Enabled" : "Disabled"}
        </button>
      </div>
      {error && (
        <p style={{ color: "var(--hw-danger)", fontSize: 12, margin: "4px 0 0" }} role="alert">
          {error}
        </p>
      )}
    </div>
  );
}

export function SettingsModal({
  open,
  onClose,
  libraryFolders,
  onAddLibraryFolder,
  onRemoveLibraryFolder,
  theme,
  onSetThemeMode,
  onSetThemeColor,
  onResetTheme,
}: Props) {
  const [category, setCategory] = useState(CATEGORIES[0].id);
  const [newFolder, setNewFolder] = useState("");
  const [backgroundEnabled, setBackgroundEnabledState] = useState<boolean | null>(null);
  const [backgroundError, setBackgroundError] = useState<string | null>(null);
  const [startOnLoginEnabled, setStartOnLoginEnabledState] = useState<boolean | null>(null);
  const [startOnLoginError, setStartOnLoginError] = useState<string | null>(null);

  useEffect(() => {
    if (!open) return;
    getBackgroundServiceEnabled()
      .then((enabled) => {
        setBackgroundEnabledState(enabled);
        setBackgroundError(null);
      })
      .catch((err) => setBackgroundError(String(err instanceof Error ? err.message : err)));
    getStartOnLoginEnabled()
      .then((enabled) => {
        setStartOnLoginEnabledState(enabled);
        setStartOnLoginError(null);
      })
      .catch((err) => setStartOnLoginError(String(err instanceof Error ? err.message : err)));
  }, [open]);

  const addFolder = () => {
    if (!newFolder.trim()) return;
    onAddLibraryFolder(newFolder);
    setNewFolder("");
  };

  const toggleBackgroundService = () => {
    if (backgroundEnabled === null) return;
    const next = !backgroundEnabled;
    setBackgroundServiceEnabled(next)
      .then(() => {
        setBackgroundEnabledState(next);
        setBackgroundError(null);
      })
      .catch((err) => setBackgroundError(String(err instanceof Error ? err.message : err)));
  };

  const toggleStartOnLogin = () => {
    if (startOnLoginEnabled === null) return;
    const next = !startOnLoginEnabled;
    setStartOnLoginEnabled(next)
      .then(() => {
        setStartOnLoginEnabledState(next);
        setStartOnLoginError(null);
      })
      .catch((err) => setStartOnLoginError(String(err instanceof Error ? err.message : err)));
  };

  if (!open) return null;

  return (
    <div
      onClick={onClose}
      style={{
        position: "fixed",
        inset: 0,
        zIndex: 100,
        background: "rgba(0,0,0,0.35)",
        backdropFilter: "blur(10px)",
        WebkitBackdropFilter: "blur(10px)",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
      }}
    >
      <div
        onClick={(e) => e.stopPropagation()}
        style={{
          width: "70%",
          height: "70%",
          background: "var(--hw-bg)",
          border: BORDER,
          borderRadius: 12,
          display: "flex",
          flexDirection: "column",
          overflow: "hidden",
        }}
      >
        <div
          style={{
            display: "flex",
            alignItems: "center",
            justifyContent: "space-between",
            padding: "10px 14px",
            fontFamily: NERD_FONT,
            color: "var(--hw-text)",
          }}
        >
          <span style={{ fontWeight: 700, fontSize: 16 }}>Settings</span>
          <button
            onClick={onClose}
            aria-label="Close settings"
            style={{
              background: "transparent",
              border: "none",
              color: "var(--hw-text-muted)",
              fontSize: 16,
              cursor: "pointer",
            }}
          >
            ×
          </button>
        </div>

        <div style={{ display: "flex", gap: 4, padding: "0 14px" }}>
          {CATEGORIES.map((cat) => (
            <button
              key={cat.id}
              onClick={() => setCategory(cat.id)}
              style={{
                background: "transparent",
                border: "none",
                borderBottom: `2px solid ${category === cat.id ? TITLEBAR_BLUE : "transparent"}`,
                color: category === cat.id ? "var(--hw-text)" : "var(--hw-text-muted)",
                padding: "8px 4px",
                marginBottom: -1,
                fontSize: 13,
                fontWeight: 600,
                cursor: "pointer",
              }}
            >
              {cat.label}
            </button>
          ))}
        </div>

        <div style={{ flex: 1, padding: 16, color: "var(--hw-text)", overflow: "auto" }}>
          {category === "system" && (
            <div style={{ padding: 5 }}>
              <div style={{ fontSize: 13, fontWeight: 500, color: "var(--hw-text-muted)", marginBottom: 8 }}>
                Media Library Folders
              </div>
              <div style={{ display: "flex", flexDirection: "column", gap: 6, marginBottom: 8 }}>
                {libraryFolders.map((folder) => (
                  <div
                    key={folder}
                    style={{
                      display: "flex",
                      alignItems: "center",
                      justifyContent: "space-between",
                      gap: 8,
                      border: "1px solid var(--hw-border)",
                      borderRadius: 4,
                      padding: "5px 8px",
                    }}
                  >
                    <span
                      style={{
                        fontSize: 12,
                        color: "var(--hw-text-muted)",
                        overflow: "hidden",
                        textOverflow: "ellipsis",
                        whiteSpace: "nowrap",
                      }}
                    >
                      {folder}
                    </span>
                    <button
                      onClick={() => onRemoveLibraryFolder(folder)}
                      aria-label={`Remove ${folder}`}
                      style={{
                        background: "transparent",
                        border: "none",
                        color: "var(--hw-text-muted)",
                        fontSize: 14,
                        cursor: "pointer",
                      }}
                    >
                      ×
                    </button>
                  </div>
                ))}
              </div>
              <div style={{ display: "flex", gap: 6 }}>
                <input
                  className="settings-input"
                  style={{ flex: 1 }}
                  value={newFolder}
                  onChange={(e) => setNewFolder(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") addFolder();
                  }}
                  placeholder="/absolute/path"
                />
                <button
                  onClick={addFolder}
                  style={{
                    border: "none",
                    borderRadius: 6,
                    padding: "5px 12px",
                    fontSize: 12,
                    fontWeight: 600,
                    cursor: "pointer",
                    background: "var(--hw-success)",
                    color: "var(--hw-bg)",
                  }}
                >
                  Add
                </button>
              </div>
            </div>
          )}

          {category === "startup" && (
            <>
              <ToggleRow
                label="Run in Background"
                enabled={backgroundEnabled}
                onToggle={toggleBackgroundService}
                error={backgroundError}
              />
              <ToggleRow
                label="Start on Login"
                enabled={startOnLoginEnabled}
                onToggle={toggleStartOnLogin}
                error={startOnLoginError}
              />
            </>
          )}

          {category === "style" && (
            <StyleSettings
              theme={theme}
              onSetMode={onSetThemeMode}
              onSetColor={onSetThemeColor}
              onReset={onResetTheme}
            />
          )}

          {category === "credits" && <CreditsPanel />}
        </div>
      </div>
    </div>
  );
}
