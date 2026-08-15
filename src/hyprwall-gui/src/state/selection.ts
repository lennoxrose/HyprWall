import { useState } from "react";

export function useSelection() {
  const [selectedMonitors, setSelectedMonitors] = useState<Set<string>>(new Set());
  const [selectedWallpaper, setSelectedWallpaper] = useState<string | null>(null);

  const toggleMonitor = (name: string) => {
    setSelectedMonitors((prev) => {
      const next = new Set(prev);
      if (next.has(name)) next.delete(name);
      else next.add(name);
      return next;
    });
  };

  const clearSelectedMonitors = () => setSelectedMonitors(new Set());

  return { selectedMonitors, toggleMonitor, clearSelectedMonitors, selectedWallpaper, setSelectedWallpaper };
}
