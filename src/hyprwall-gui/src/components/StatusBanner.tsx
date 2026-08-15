export function StatusBanner() {
  return (
    <div style={{ background: "#7f1d1d", color: "#fff", padding: 8, textAlign: "center" }}>
      hyprwalld is not running or unreachable at $XDG_RUNTIME_DIR/hyprwall.sock. Start it — this
      will recover automatically.
    </div>
  );
}
