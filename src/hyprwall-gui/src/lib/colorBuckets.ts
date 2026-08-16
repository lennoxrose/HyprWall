export interface ColorBucket {
  name: string;
  swatch: string;
}

export const COLOR_BUCKETS: ColorBucket[] = [
  { name: "red", swatch: "#e53935" },
  { name: "orange", swatch: "#fb8c00" },
  { name: "yellow", swatch: "#fdd835" },
  { name: "green", swatch: "#43a047" },
  { name: "blue", swatch: "#1e88e5" },
  { name: "teal", swatch: "#00897b" },
  { name: "purple", swatch: "#8e24aa" },
  { name: "pink", swatch: "#ec407a" },
  { name: "brown", swatch: "#6d4c41" },
  { name: "gray", swatch: "#9e9e9e" },
  { name: "black", swatch: "#1a1a1a" },
  { name: "white", swatch: "#f5f5f5" },
];

function hexToRgb(hex: string): [number, number, number] {
  const n = parseInt(hex.slice(1), 16);
  return [(n >> 16) & 0xff, (n >> 8) & 0xff, n & 0xff];
}

/** Nearest bucket to `hex` by plain euclidean RGB distance -- good enough
 * for a coarse filter, no need for perceptual (Lab) distance here. */
export function nearestColorBucket(hex: string): string {
  const [r, g, b] = hexToRgb(hex);
  let best = COLOR_BUCKETS[0];
  let bestDist = Infinity;
  for (const bucket of COLOR_BUCKETS) {
    const [br, bg, bb] = hexToRgb(bucket.swatch);
    const dist = (r - br) ** 2 + (g - bg) ** 2 + (b - bb) ** 2;
    if (dist < bestDist) {
      bestDist = dist;
      best = bucket;
    }
  }
  return best.name;
}

export type DateBucket = "today" | "week" | "month" | "older";

export const DATE_BUCKETS: { name: DateBucket; label: string }[] = [
  { name: "today", label: "Today" },
  { name: "week", label: "This week" },
  { name: "month", label: "This month" },
  { name: "older", label: "Older" },
];

const DAY_SECONDS = 86400;

/** Bucketed relative to `now` (seconds since epoch, defaults to current
 * time -- exposed as a param so tests don't depend on wall-clock time). */
export function dateBucket(addedTs: number, now: number = Date.now() / 1000): DateBucket {
  const ageDays = (now - addedTs) / DAY_SECONDS;
  if (ageDays < 1) return "today";
  if (ageDays < 7) return "week";
  if (ageDays < 30) return "month";
  return "older";
}
