import type { ThemeColors, ThemeMode } from "./types";

/** Order + label the Style tab renders each token row in. Kept as one list
 * so the tab doesn't need its own separate enumeration of `ThemeColors`. */
export const THEME_TOKENS: { key: keyof ThemeColors; label: string }[] = [
  { key: "bg", label: "Background" },
  { key: "bgElevated", label: "Panels & cards" },
  { key: "border", label: "Border" },
  { key: "borderHover", label: "Border (hover)" },
  { key: "text", label: "Text" },
  { key: "textMuted", label: "Muted text" },
  { key: "accent", label: "Accent" },
  { key: "accentText", label: "Text on accent" },
  { key: "success", label: "Success" },
  { key: "danger", label: "Danger" },
];

// Mirrors `ThemeMode::defaults` in `src-tauri/src/commands/theme.rs` -- kept
// duplicated (not fetched) so switching Dark/Light in the Style tab is
// instant and doesn't need a round trip before it can show the new palette.
const DARK_DEFAULTS: ThemeColors = {
  bg: "#0a0a0a",
  bgElevated: "#141414",
  border: "#333333",
  borderHover: "#555555",
  text: "#eeeeee",
  textMuted: "#888888",
  accent: "#2563eb",
  accentText: "#ffffff",
  success: "#4ade80",
  danger: "#f87171",
};

const LIGHT_DEFAULTS: ThemeColors = {
  bg: "#f5f5f5",
  bgElevated: "#ffffff",
  border: "#dddddd",
  borderHover: "#bbbbbb",
  text: "#1a1a1a",
  textMuted: "#666666",
  accent: "#2563eb",
  accentText: "#ffffff",
  success: "#16a34a",
  danger: "#dc2626",
};

export function defaultsForMode(mode: ThemeMode): ThemeColors {
  return mode === "dark" ? DARK_DEFAULTS : LIGHT_DEFAULTS;
}

const CSS_VAR_PREFIX = "--hw-";

function cssVarName(key: keyof ThemeColors): string {
  return CSS_VAR_PREFIX + key.replace(/[A-Z]/g, (c) => "-" + c.toLowerCase());
}

/** Every themed component reads its color via `var(--hw-<token>)` in an
 * inline style -- CSS custom properties resolve there same as in a
 * stylesheet, so applying a theme is just setting these on the root
 * element, no re-render of every component required. */
export function applyThemeColors(colors: ThemeColors) {
  const root = document.documentElement.style;
  for (const { key } of THEME_TOKENS) {
    root.setProperty(cssVarName(key), colors[key]);
  }
}

export function cssVar(key: keyof ThemeColors): string {
  return `var(${cssVarName(key)})`;
}
