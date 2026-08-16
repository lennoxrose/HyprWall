function SearchIcon() {
  return (
    <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="var(--hw-text-muted)" strokeWidth="2">
      <circle cx="11" cy="11" r="7" />
      <path d="M21 21 L16.65 16.65" strokeLinecap="round" />
    </svg>
  );
}

interface Props {
  value: string;
  onChange: (value: string) => void;
}

export function SearchBar({ value, onChange }: Props) {
  return (
    <div style={{ position: "relative", flex: 3 }}>
      <div style={{ position: "absolute", left: 8, top: "50%", transform: "translateY(-50%)", display: "flex" }}>
        <SearchIcon />
      </div>
      <input
        className="settings-input"
        style={{ width: "100%", paddingLeft: 26 }}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder="Search wallpapers..."
        aria-label="Search wallpapers"
      />
    </div>
  );
}
