import { useEffect, useState } from "react";
import { DROPDOWN_ANIM_MS, DROPDOWN_EASING, NERD_FONT, TITLEBAR_BLUE, TITLEBAR_HEIGHT } from "./TitleBar";
import { getWallpaperSettings, hasAudioTrack, setWallpaperSettings } from "../lib/api";
import type { FitMode, WallpaperSettings } from "../lib/types";

interface Props {
  /** The picture the sidebar is configuring, or `null` when closed. */
  path: string | null;
  kind: "video" | "image" | null;
  onClose: () => void;
}

const SIDEBAR_WIDTH = 280;
const PANEL_RADIUS = 14; // matches MonitorsDropdown's PANEL_RADIUS
const OUTER_GAP = 10; // breathing room between the panel and the navbar/window bottom

const DEFAULT_SETTINGS: WallpaperSettings = {
  zoom: 1,
  pan_x: 0,
  pan_y: 0,
  fit: "cover",
  volume: 0,
  brightness: 0,
  contrast: 0,
  hue: 0,
  saturation: 0,
};

const FIT_MODES: FitMode[] = ["cover", "contain", "stretch"];

function Slider({
  label,
  value,
  min,
  max,
  step,
  onChange,
}: {
  label: string;
  value: number;
  min: number;
  max: number;
  step: number;
  onChange: (value: number) => void;
}) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(() => String(value));

  const commit = () => {
    const parsed = Number(draft);
    if (!Number.isNaN(parsed)) onChange(Math.min(max, Math.max(min, parsed)));
    setEditing(false);
  };

  return (
    <label style={{ display: "flex", flexDirection: "column", gap: 4, fontSize: 12, color: "rgba(255,255,255,0.85)" }}>
      <span style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
        <span>{label}</span>
        {editing ? (
          <input
            className="settings-input no-spinner"
            type="number"
            autoFocus
            value={draft}
            min={min}
            max={max}
            step={step}
            style={{ width: 64, padding: "2px 6px", fontSize: 11, border: "1px solid #fff" }}
            onChange={(e) => setDraft(e.target.value)}
            onBlur={commit}
            onKeyDown={(e) => {
              if (e.key === "Enter") commit();
              if (e.key === "Escape") setEditing(false);
            }}
            onClick={(e) => e.stopPropagation()}
          />
        ) : (
          <span
            style={{ opacity: 0.7, cursor: "text" }}
            onClick={() => {
              setDraft(String(value));
              setEditing(true);
            }}
          >
            {value.toFixed(2)}
          </span>
        )}
      </span>
      <input type="range" min={min} max={max} step={step} value={value} onChange={(e) => onChange(Number(e.target.value))} />
    </label>
  );
}

/** Right-side settings panel for one picture's zoom/pan/fit/color/volume,
 * opened by right-clicking a selected library tile. Visually reuses
 * `MonitorsDropdown`'s language (blue chrome, the same slide timing/easing,
 * a boxShadow'd panel) but slides in from the window's right edge instead
 * of down from the titlebar, and is only rounded on the left edge (the one
 * facing into the app) -- the right edge is flush against the window
 * boundary, same as the window itself, so rounding it would look like the
 * panel is "leaping up" off the edge instead of belonging to it. No
 * `CornerNotch` either: that piece fillets the titlebar-to-dropdown seam
 * specifically, which doesn't exist here. */
export function Sidebar({ path, kind, onClose }: Props) {
  const [settings, setSettings] = useState<WallpaperSettings>(DEFAULT_SETTINGS);
  const [showVolume, setShowVolume] = useState(false);
  const open = path !== null;

  useEffect(() => {
    if (!path) return;
    getWallpaperSettings(path)
      .then(setSettings)
      .catch(() => setSettings(DEFAULT_SETTINGS));
    if (kind === "video") {
      hasAudioTrack(path)
        .then(setShowVolume)
        .catch(() => setShowVolume(false));
    } else {
      setShowVolume(false);
    }
  }, [path, kind]);

  // Local state updates immediately (the slider itself stays responsive),
  // but the daemon call waits ~150ms after the last change so a drag
  // doesn't flood the IPC socket with one command per pixel moved.
  useEffect(() => {
    if (!path) return;
    const timer = setTimeout(() => {
      setWallpaperSettings(path, settings).catch(() => {});
    }, 150);
    return () => clearTimeout(timer);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [settings]);

  const update = (patch: Partial<WallpaperSettings>) => setSettings((s) => ({ ...s, ...patch }));

  return (
    <div
      style={{
        position: "absolute",
        top: TITLEBAR_HEIGHT + OUTER_GAP,
        right: 0,
        bottom: OUTER_GAP,
        zIndex: 20,
        width: SIDEBAR_WIDTH,
        willChange: "transform",
        transform: `translateX(${open ? "0" : "100%"})`,
        transition: `transform ${DROPDOWN_ANIM_MS}ms ${DROPDOWN_EASING}`,
      }}
    >
      <div
        onClick={(e) => e.stopPropagation()}
        style={{
          height: "100%",
          background: TITLEBAR_BLUE,
          borderTopLeftRadius: PANEL_RADIUS,
          borderBottomLeftRadius: PANEL_RADIUS,
          boxShadow: "0 8px 24px rgba(0,0,0,0.5)",
          padding: "22px 14px",
          display: "flex",
          flexDirection: "column",
          gap: 14,
          overflow: "auto",
          fontFamily: NERD_FONT,
        }}
      >
        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
          <span style={{ fontWeight: 700, fontSize: 14, color: "#fff" }}>Picture Settings</span>
          <button
            onClick={onClose}
            aria-label="Close picture settings"
            style={{ background: "transparent", border: "none", color: "#fff", fontSize: 16, cursor: "pointer" }}
          >
            ×
          </button>
        </div>

        <Slider label="Zoom" value={settings.zoom} min={1} max={3} step={0.01} onChange={(v) => update({ zoom: v })} />
        <Slider label="Position X" value={settings.pan_x} min={-0.5} max={0.5} step={0.01} onChange={(v) => update({ pan_x: v })} />
        <Slider label="Position Y" value={settings.pan_y} min={-0.5} max={0.5} step={0.01} onChange={(v) => update({ pan_y: v })} />

        <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
          <span style={{ fontSize: 12, color: "rgba(255,255,255,0.85)" }}>Fit</span>
          <div style={{ display: "flex", gap: 4 }}>
            {FIT_MODES.map((f) => (
              <button
                key={f}
                onClick={() => update({ fit: f })}
                style={{
                  flex: 1,
                  border: "none",
                  borderRadius: 6,
                  padding: "5px 0",
                  fontSize: 11,
                  fontWeight: 600,
                  cursor: "pointer",
                  background: settings.fit === f ? "#4ade80" : "rgba(255,255,255,0.14)",
                  color: settings.fit === f ? "#0a0a0a" : "#fff",
                  textTransform: "capitalize",
                }}
              >
                {f}
              </button>
            ))}
          </div>
        </div>

        {showVolume && (
          <Slider label="Volume" value={settings.volume} min={0} max={100} step={1} onChange={(v) => update({ volume: v })} />
        )}

        <Slider label="Brightness" value={settings.brightness} min={-100} max={100} step={1} onChange={(v) => update({ brightness: v })} />
        <Slider label="Contrast" value={settings.contrast} min={-100} max={100} step={1} onChange={(v) => update({ contrast: v })} />
        <Slider label="Hue" value={settings.hue} min={-100} max={100} step={1} onChange={(v) => update({ hue: v })} />
        <Slider label="Saturation" value={settings.saturation} min={-100} max={100} step={1} onChange={(v) => update({ saturation: v })} />
      </div>
    </div>
  );
}
