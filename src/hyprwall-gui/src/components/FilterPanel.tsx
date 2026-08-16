import { COLOR_BUCKETS, DATE_BUCKETS, type DateBucket } from "../lib/colorBuckets";
import type { WallpaperKind } from "../lib/types";

const PANEL_BG = "var(--hw-bg)"; // matches the app's main background (App.tsx)
const PANEL_BORDER = "1px solid var(--hw-border)";

/** Thin funnel glyph -- same thin-stroke line style as the app's other
 * custom icons (cog, chevron, warning triangle). */
export function FilterIcon() {
  return (
    <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
      <path d="M3 4h18l-7 9v6l-4 2v-8z" strokeLinejoin="round" strokeLinecap="round" />
    </svg>
  );
}

function ChipToggle({ label, active, onClick }: { label: string; active: boolean; onClick: () => void }) {
  return (
    <button
      onClick={onClick}
      aria-pressed={active}
      style={{
        border: active ? "1px solid var(--hw-success)" : "1px solid var(--hw-border)",
        borderRadius: 6,
        background: active ? "rgba(74,222,128,0.14)" : "var(--hw-bg)",
        color: active ? "var(--hw-success)" : "var(--hw-text-muted)",
        fontSize: 12,
        padding: "5px 8px",
        cursor: "pointer",
        whiteSpace: "nowrap",
      }}
    >
      {label}
    </button>
  );
}

/** Centered facet title flanked by two short divider lines -- stops short
 * of the panel's own padded edge (extra inset margin), not a full-bleed
 * rule. */
function FacetLabel({ children }: { children: string }) {
  return (
    <div style={{ display: "flex", alignItems: "center", gap: 6, marginBottom: 6 }}>
      <div style={{ flex: 1, height: 1, background: "var(--hw-border)", marginLeft: 4 }} />
      <span style={{ fontSize: 10.5, color: "var(--hw-text-muted)", textTransform: "uppercase", letterSpacing: 0.3, whiteSpace: "nowrap" }}>
        {children}
      </span>
      <div style={{ flex: 1, height: 1, background: "var(--hw-border)", marginRight: 4 }} />
    </div>
  );
}

export interface FilterState {
  kinds: Set<WallpaperKind>;
  colors: Set<string>;
  dateBucket: DateBucket | null;
}

export function emptyFilterState(): FilterState {
  return { kinds: new Set(), colors: new Set(), dateBucket: null };
}

export function isFilterActive(filters: FilterState): boolean {
  return filters.kinds.size > 0 || filters.colors.size > 0 || filters.dateBucket !== null;
}

interface Props {
  open: boolean;
  onClose: () => void;
  filters: FilterState;
  onChange: (filters: FilterState) => void;
}

export function FilterPanel({ open, onClose, filters, onChange }: Props) {
  if (!open) return null;

  const toggleKind = (kind: WallpaperKind) => {
    const kinds = new Set(filters.kinds);
    kinds.has(kind) ? kinds.delete(kind) : kinds.add(kind);
    onChange({ ...filters, kinds });
  };

  const toggleColor = (name: string) => {
    const colors = new Set(filters.colors);
    colors.has(name) ? colors.delete(name) : colors.add(name);
    onChange({ ...filters, colors });
  };

  const selectDateBucket = (bucket: DateBucket) => {
    onChange({ ...filters, dateBucket: filters.dateBucket === bucket ? null : bucket });
  };

  return (
    <>
      {/* Invisible full-page backdrop, closes the panel on an outside click. */}
      <div style={{ position: "fixed", inset: 0, zIndex: 39 }} onClick={onClose} />
      <div
        style={{
          position: "absolute",
          top: "calc(100% + 6px)",
          right: 0,
          zIndex: 40,
          width: 200,
          background: PANEL_BG,
          border: PANEL_BORDER,
          borderRadius: 8,
          padding: 14,
          display: "flex",
          flexDirection: "column",
          gap: 14,
        }}
        onClick={(e) => e.stopPropagation()}
      >
        <div>
          <FacetLabel>Type</FacetLabel>
          <div style={{ display: "grid", gridTemplateColumns: "repeat(2, 1fr)", gap: 6 }}>
            <ChipToggle label="Video" active={filters.kinds.has("video")} onClick={() => toggleKind("video")} />
            <ChipToggle label="Image" active={filters.kinds.has("image")} onClick={() => toggleKind("image")} />
          </div>
        </div>

        <div>
          <FacetLabel>Color</FacetLabel>
          <div style={{ display: "grid", gridTemplateColumns: "repeat(3, 1fr)", gap: 8, justifyItems: "center" }}>
            {COLOR_BUCKETS.map((bucket) => {
              const active = filters.colors.has(bucket.name);
              return (
                <button
                  key={bucket.name}
                  onClick={() => toggleColor(bucket.name)}
                  aria-pressed={active}
                  aria-label={bucket.name}
                  title={bucket.name}
                  style={{
                    width: 22,
                    height: 22,
                    borderRadius: "50%",
                    background: bucket.swatch,
                    border: active ? "2px solid var(--hw-success)" : "1px solid var(--hw-border)",
                    cursor: "pointer",
                    padding: 0,
                  }}
                />
              );
            })}
          </div>
        </div>

        <div>
          <FacetLabel>Added</FacetLabel>
          <div style={{ display: "grid", gridTemplateColumns: "repeat(2, 1fr)", gap: 6 }}>
            {DATE_BUCKETS.map((bucket) => (
              <ChipToggle
                key={bucket.name}
                label={bucket.label}
                active={filters.dateBucket === bucket.name}
                onClick={() => selectDateBucket(bucket.name)}
              />
            ))}
          </div>
        </div>

        {isFilterActive(filters) && (
          <div style={{ textAlign: "center" }}>
            <button
              onClick={() => onChange(emptyFilterState())}
              style={{
                border: "none",
                background: "transparent",
                color: "var(--hw-text-muted)",
                fontSize: 10,
                cursor: "pointer",
                padding: 0,
              }}
            >
              Clear filters
            </button>
          </div>
        )}
      </div>
    </>
  );
}
