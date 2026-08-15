import { useState } from "react";
import { NERD_FONT, TITLEBAR_BLUE } from "./TitleBar";

interface Props {
  open: boolean;
  onClose: () => void;
  libraryPath: string;
  onLibraryPathChange: (value: string) => void;
  onSaveLibraryPath: () => void;
}

interface Category {
  id: string;
  label: string;
}

// "Startup" has no content yet -- it's a placeholder category for a later
// prompt, not wired to anything.
const CATEGORIES: Category[] = [
  { id: "system", label: "System" },
  { id: "startup", label: "Startup" },
];

const BORDER = "1px solid #333";

export function SettingsModal({
  open,
  onClose,
  libraryPath,
  onLibraryPathChange,
  onSaveLibraryPath,
}: Props) {
  const [category, setCategory] = useState(CATEGORIES[0].id);

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
          background: "#0a0a0a",
          border: BORDER,
          borderRadius: 12,
          display: "flex",
          flexDirection: "column",
          overflow: "hidden",
          boxShadow: "0 20px 60px rgba(0,0,0,0.6)",
        }}
      >
        <div
          style={{
            display: "flex",
            alignItems: "center",
            justifyContent: "space-between",
            padding: "10px 14px",
            fontFamily: NERD_FONT,
            color: "#fff",
          }}
        >
          <span style={{ fontWeight: 700, fontSize: 16 }}>Settings</span>
          <button
            onClick={onClose}
            aria-label="Close settings"
            style={{ background: "transparent", border: "none", color: "#999", fontSize: 16, cursor: "pointer" }}
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
                color: category === cat.id ? "#fff" : "#555",
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

        <div style={{ flex: 1, padding: 16, color: "#eee", overflow: "auto" }}>
          {category === "system" && (
            <>
              <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", padding: 5 }}>
                <span style={{ fontSize: 13, fontWeight: 500, color: "#bbb" }}>Image Library Folder</span>
                <input
                  className="settings-input"
                  style={{ width: 500 }}
                  value={libraryPath}
                  onChange={(e) => onLibraryPathChange(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") onSaveLibraryPath();
                  }}
                  onBlur={onSaveLibraryPath}
                  placeholder="/absolute/path"
                />
              </div>
            </>
          )}
        </div>
      </div>
    </div>
  );
}
