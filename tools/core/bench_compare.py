#!/usr/bin/env python3
"""Compare Criterion bencher-format results against a stored baseline.

Usage:
    python3 tools/core/bench_compare.py \
        --results /tmp/bench_results.txt \
        --baseline crates/golden_core/engine/benches/baseline.json \
        --output /tmp/bench_summary.md

Regression thresholds (configurable via --threshold-* flags):
    tick_20k_passive              p50: 10%
    tick_20k_sparse_active/*      p50: 10%
    dispatch_10k_with_1k_listeners p50: 15%

Exit code 0 = no regressions, 1 = at least one regression.
"""

import argparse
import json
import re
import sys
from pathlib import Path

# Scenario name  → max allowed regression fraction (0.10 = 10%)
THRESHOLDS: dict[str, float] = {
    "tick_20k_passive": 0.10,
    "tick_20k_sparse_active": 0.10,
    "dispatch_10k_with_1k_listeners": 0.15,
}
DEFAULT_THRESHOLD = 0.10


def parse_bencher_output(text: str) -> dict[str, float]:
    """Parse `cargo bench --output-format bencher` stdout.

    Each line looks like:
        test tick_20k_passive ... bench:   1 234 ns/iter (+/- 56)
    Returns {name: ns_per_iter}.
    """
    results: dict[str, float] = {}
    # Strip commas from numbers (e.g. "1,234" → "1234")
    for line in text.splitlines():
        m = re.match(r"test (.+?) \.\.\. bench:\s+([\d,]+)\s+ns/iter", line)
        if m:
            name = m.group(1).strip()
            ns = float(m.group(2).replace(",", ""))
            results[name] = ns
    return results


def load_baseline(path: Path) -> tuple[dict[str, float], dict[str, float]]:
    """Load baseline ns values and per-scenario thresholds from baseline.json.

    Returns ({name: p50_ns}, {name: regression_fraction}).
    """
    data = json.loads(path.read_text())
    baseline: dict[str, float] = {}
    thresholds: dict[str, float] = {}
    for name, entry in data.get("scenarios", {}).items():
        p50 = entry.get("p50_ns")
        if p50 is not None:
            baseline[name] = float(p50)
        t = entry.get("thresholds", {}).get("regression_pct")
        if t is not None:
            thresholds[name] = float(t) / 100.0
    return baseline, thresholds


def threshold_for(name: str) -> float:
    for key, t in THRESHOLDS.items():
        if name.startswith(key):
            return t
    return DEFAULT_THRESHOLD


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--results", required=True)
    parser.add_argument("--baseline", required=True)
    parser.add_argument("--output", required=True)
    args = parser.parse_args()

    results = parse_bencher_output(Path(args.results).read_text())
    baseline, file_thresholds = load_baseline(Path(args.baseline))

    lines: list[str] = ["## Benchmark results\n"]
    lines.append("| Scenario | Baseline (ns) | Current (ns) | Delta | Status |")
    lines.append("|---|---|---|---|---|")

    regressions: list[str] = []

    # Report on all results; flag those with a baseline to compare.
    for name, current_ns in sorted(results.items()):
        if name in baseline:
            base_ns = baseline[name]
            delta_frac = (current_ns - base_ns) / base_ns
            delta_str = f"{delta_frac:+.1%}"
            threshold = file_thresholds.get(name) or threshold_for(name)
            if delta_frac > threshold:
                status = f"🔴 REGRESSION (>{threshold:.0%} threshold)"
                regressions.append(f"  - `{name}`: {delta_frac:+.1%} (threshold {threshold:.0%})")
            elif delta_frac < -0.05:
                status = "🟢 improvement"
            else:
                status = "✅ ok"
            lines.append(
                f"| `{name}` | {base_ns:,.0f} | {current_ns:,.0f} | {delta_str} | {status} |"
            )
        else:
            lines.append(f"| `{name}` | — | {current_ns:,.0f} | — | ℹ️ no baseline |")

    if regressions:
        lines.append("\n### ❌ Regressions detected\n")
        lines.extend(regressions)
        lines.append(
            "\nTo update the baseline after an intentional perf change, "
            "run `cargo bench` on `main` and update `crates/golden_core/engine/benches/baseline.json`."
        )
    elif baseline:
        lines.append("\n✅ No regressions detected.")
    else:
        lines.append("\n⚠️ No baseline found — commit `crates/golden_core/engine/benches/baseline.json` to enable regression checks.")

    summary = "\n".join(lines) + "\n"
    Path(args.output).write_text(summary)
    print(summary)

    if regressions:
        print(f"FAIL: {len(regressions)} regression(s) detected.", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
