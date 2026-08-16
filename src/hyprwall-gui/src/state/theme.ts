import { useEffect, useState } from "react";
import { getTheme, setTheme as saveTheme } from "../lib/api";
import { applyThemeColors, defaultsForMode } from "../lib/theme";
import type { ThemeColors, ThemeMode, ThemeState } from "../lib/types";

const FALLBACK: ThemeState = { mode: "dark", colors: defaultsForMode("dark") };

/** Loads the persisted theme once on mount (falling back to the dark
 * defaults if the daemon-less GUI-local theme file doesn't exist yet or the
 * backend call fails -- there's no daemon involved here, so unlike the rest
 * of the app this has nothing to retry against), applies it as CSS custom
 * properties immediately, and persists every change back out. */
export function useTheme() {
  const [theme, setThemeState] = useState<ThemeState>(FALLBACK);

  useEffect(() => {
    applyThemeColors(FALLBACK.colors);
    getTheme()
      .then((loaded) => {
        setThemeState(loaded);
        applyThemeColors(loaded.colors);
      })
      .catch(() => {});
  }, []);

  const persist = (next: ThemeState) => {
    setThemeState(next);
    applyThemeColors(next.colors);
    saveTheme(next).catch(() => {});
  };

  const setMode = (mode: ThemeMode) => persist({ mode, colors: defaultsForMode(mode) });

  const setColor = (key: keyof ThemeColors, value: string) =>
    persist({ ...theme, colors: { ...theme.colors, [key]: value } });

  const resetToDefaults = () => persist({ mode: theme.mode, colors: defaultsForMode(theme.mode) });

  return { theme, setMode, setColor, resetToDefaults };
}
