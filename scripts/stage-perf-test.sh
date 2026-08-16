#!/usr/bin/env bash
# Run manually, on a real Hyprland session, before opening or updating a
# staging-bound PR. Builds release binaries, starts hyprwalld + hyprwall-gui,
# times each one's startup, samples combined memory/CPU for a short window,
# and prints a markdown table to paste into the PR description under a
# "## Performance Report" heading -- .github/workflows/ci.yml's
# performance-report job rejects any staging PR missing that table or over
# budget on any one metric.
#
# Needs: a running Hyprland session (hyprctl), jq, and a monitor to render
# to -- this can't run on a headless CI runner, which is why it's a local
# tool instead of an automated check.
set -euo pipefail

MEM_LIMIT_MB=300
CPU_LIMIT_PCT=100
DAEMON_LIMIT_S=2
GUI_LIMIT_S=3
SAMPLE_SECONDS=20

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

for bin in hyprctl jq; do
  if ! command -v "$bin" >/dev/null 2>&1; then
    echo "error: $bin not found -- this must run inside a real Hyprland session." >&2
    exit 1
  fi
done
if [ -z "${HYPRLAND_INSTANCE_SIGNATURE:-}" ]; then
  echo "error: not running inside Hyprland (HYPRLAND_INSTANCE_SIGNATURE unset)." >&2
  exit 1
fi

echo "==> building release binaries" >&2
(cd src/hyprwall-gui && npm ci && npm run build) >&2
cargo build --release --workspace >&2

DAEMON_BIN="$REPO_ROOT/target/release/hyprwalld"
CTL_BIN="$REPO_ROOT/target/release/hyprwallctl"
GUI_BIN="$REPO_ROOT/target/release/hyprwall-gui"

daemon_pid=""
gui_pid=""
cleanup() {
  [ -n "$daemon_pid" ] && kill "$daemon_pid" 2>/dev/null || true
  [ -n "$gui_pid" ] && kill "$gui_pid" 2>/dev/null || true
}
trap cleanup EXIT

now_s() { date +%s.%N; }

echo "==> starting hyprwalld" >&2
start=$(now_s)
"$DAEMON_BIN" &
daemon_pid=$!
until "$CTL_BIN" monitor-list >/dev/null 2>&1; do
  if ! kill -0 "$daemon_pid" 2>/dev/null; then
    echo "error: hyprwalld exited before becoming ready" >&2
    exit 1
  fi
  sleep 0.05
done
daemon_startup=$(awk -v a="$start" -v b="$(now_s)" 'BEGIN { printf "%.2f", b - a }')

echo "==> starting hyprwall-gui" >&2
start=$(now_s)
"$GUI_BIN" &
gui_pid=$!
until hyprctl clients -j 2>/dev/null | jq -e '.[] | select(.title == "HyprWall")' >/dev/null 2>&1; do
  if ! kill -0 "$gui_pid" 2>/dev/null; then
    echo "error: hyprwall-gui exited before showing a window" >&2
    exit 1
  fi
  sleep 0.05
done
gui_startup=$(awk -v a="$start" -v b="$(now_s)" 'BEGIN { printf "%.2f", b - a }')

echo "==> sampling memory/CPU for ${SAMPLE_SECONDS}s" >&2
mem_peak_kb=0
cpu_sum=0
for _ in $(seq 1 "$SAMPLE_SECONDS"); do
  d_mem_kb=$(awk '/VmRSS/{print $2}' "/proc/$daemon_pid/status" 2>/dev/null || echo 0)
  g_mem_kb=$(awk '/VmRSS/{print $2}' "/proc/$gui_pid/status" 2>/dev/null || echo 0)
  total_mem_kb=$((d_mem_kb + g_mem_kb))
  [ "$total_mem_kb" -gt "$mem_peak_kb" ] && mem_peak_kb=$total_mem_kb

  d_cpu=$(ps -p "$daemon_pid" -o %cpu= 2>/dev/null | tr -d ' ' || echo 0)
  g_cpu=$(ps -p "$gui_pid" -o %cpu= 2>/dev/null | tr -d ' ' || echo 0)
  cpu_sum=$(awk -v s="$cpu_sum" -v d="${d_cpu:-0}" -v g="${g_cpu:-0}" 'BEGIN { printf "%.2f", s + d + g }')
  sleep 1
done
mem_peak_mb=$(awk -v kb="$mem_peak_kb" 'BEGIN { printf "%.1f", kb / 1024 }')
cpu_avg_pct=$(awk -v sum="$cpu_sum" -v n="$SAMPLE_SECONDS" 'BEGIN { printf "%.1f", sum / n }')

pass() { awk -v v="$1" -v l="$2" 'BEGIN { exit !(v <= l) }' && echo "✅" || echo "❌"; }

total=4
passed=0
awk -v v="$daemon_startup" -v l="$DAEMON_LIMIT_S" 'BEGIN { exit !(v <= l) }' && passed=$((passed + 1))
awk -v v="$gui_startup" -v l="$GUI_LIMIT_S" 'BEGIN { exit !(v <= l) }' && passed=$((passed + 1))
awk -v v="$mem_peak_mb" -v l="$MEM_LIMIT_MB" 'BEGIN { exit !(v <= l) }' && passed=$((passed + 1))
awk -v v="$cpu_avg_pct" -v l="$CPU_LIMIT_PCT" 'BEGIN { exit !(v <= l) }' && passed=$((passed + 1))

cat <<EOF

## Performance Report

| Metric | Result | Limit | Pass |
|---|---|---|---|
| hyprwalld startup | ${daemon_startup}s | ${DAEMON_LIMIT_S}s | $(pass "$daemon_startup" "$DAEMON_LIMIT_S") |
| hyprwall-gui startup | ${gui_startup}s | ${GUI_LIMIT_S}s | $(pass "$gui_startup" "$GUI_LIMIT_S") |
| Combined memory | ${mem_peak_mb}MB | ${MEM_LIMIT_MB}MB | $(pass "$mem_peak_mb" "$MEM_LIMIT_MB") |
| Combined CPU | ${cpu_avg_pct}% | ${CPU_LIMIT_PCT}% | $(pass "$cpu_avg_pct" "$CPU_LIMIT_PCT") |

**Total: ${passed}/${total} passed**
EOF
