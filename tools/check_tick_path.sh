#!/usr/bin/env bash
# R1 enforcement: no full-graph iteration (self.nodes.iter/values/keys) in tick-path files.
#
# Usage: bash tools/check_tick_path.sh [--root <crate-root>]
# Run from the repo root or pass --root to point at crates/core.
# Exits 1 when a violation is found.

set -euo pipefail

ROOT="${1:-crates/core/src/engine}"
if [[ "$1" == "--root" ]]; then ROOT="$2"; fi

FORBIDDEN='self\.nodes\.(iter|values|keys)\('
TICK_PATH_FILES=(
    "runtime/tick.rs"
    "runtime/scheduled_updates.rs"
    "runtime/stabilization.rs"
    "dispatch.rs"
)

FAILED=0
for f in "${TICK_PATH_FILES[@]}"; do
    FULL="$ROOT/$f"
    if [[ ! -f "$FULL" ]]; then
        echo "SKIP (not found): $FULL"
        continue
    fi
    # Lines with a PERF-EXCEPTION comment are explicitly approved; skip them.
    if grep -En "$FORBIDDEN" "$FULL" | grep -v "PERF-EXCEPTION"; then
        echo "FAIL: full-graph iteration in tick-path file: $f"
        echo "  Add a '// PERF-EXCEPTION: <reason>' comment and a benchmark showing the cost is bounded."
        FAILED=1
    fi
done

if [[ $FAILED -eq 0 ]]; then
    echo "OK: no full-graph iteration in tick-path files."
fi
exit $FAILED
