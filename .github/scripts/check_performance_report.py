#!/usr/bin/env python3
"""Validates a staging-bound PR's "## Performance Report" markdown table
(produced by scripts/stage-perf-test.sh, pasted into the PR description by
hand -- the measurement itself needs a real Hyprland session, which can't
run on a hosted CI runner) against the fixed low-end-hardware budget.

Limits are re-checked here from the reported *Result* numbers, independent
of whatever the pasted table's own Limit/Pass columns say -- those are just
as easy to paste stale or wrong.
"""

import os
import re
import sys

LIMITS = {
    "hyprwalld startup": ("s", 2.0),
    "hyprwall-gui startup": ("s", 3.0),
    "memory": ("mb", 300.0),
    "cpu": ("pct", 100.0),
}

KEYWORDS = {
    "hyprwalld startup": ("hyprwalld", "startup"),
    "hyprwall-gui startup": ("gui", "startup"),
    "memory": ("memory",),
    "cpu": ("cpu",),
}

VALUE_RE = re.compile(r"(-?\d+(?:\.\d+)?)\s*(ms|mb|gb|s|%)?", re.IGNORECASE)


def parse_value(cell: str):
    match = VALUE_RE.search(cell)
    if not match:
        return None
    number = float(match.group(1))
    unit = (match.group(2) or "").lower()
    if unit == "ms":
        number /= 1000.0
    elif unit == "gb":
        number *= 1024.0
    return number


def find_metric(label: str):
    lowered = label.lower()
    for key, words in KEYWORDS.items():
        if all(word in lowered for word in words):
            return key
    return None


def main() -> int:
    body = os.environ.get("PR_BODY") or ""
    heading = "## Performance Report"
    if heading not in body:
        print(
            "::error::PR is missing a '## Performance Report' section. "
            "Run scripts/stage-perf-test.sh on a real Hyprland session and "
            "paste its table into the PR description."
        )
        return 1

    section = body.split(heading, 1)[1]
    found: dict[str, float] = {}
    for line in section.splitlines():
        stripped = line.strip()
        if not stripped.startswith("|"):
            if found and stripped.startswith("#"):
                break  # next heading -- report section ended
            continue
        cells = [c.strip() for c in stripped.strip("|").split("|")]
        if len(cells) < 2:
            continue
        metric = find_metric(cells[0])
        if metric is None:
            continue
        value = parse_value(cells[1])
        if value is None:
            continue
        found[metric] = value

    missing = [key for key in LIMITS if key not in found]
    if missing:
        print(
            f"::error::Performance Report is missing: {', '.join(missing)}. "
            "Run scripts/stage-perf-test.sh and paste its full table."
        )
        return 1

    failed = []
    for key, (unit, limit) in LIMITS.items():
        value = found[key]
        mark = "PASS" if value <= limit else "FAIL"
        print(f"[{mark}] {key}: {value}{unit} (limit {limit}{unit})")
        if value > limit:
            failed.append(f"{key} is {value}{unit}, over the {limit}{unit} limit")

    print(f"Performance Report: {len(LIMITS) - len(failed)}/{len(LIMITS)} passed")

    if failed:
        print("::error::Performance Report over budget -- " + "; ".join(failed))
        return 1

    return 0


if __name__ == "__main__":
    sys.exit(main())
