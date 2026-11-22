#!/usr/bin/env bash
set -euo pipefail

runtime=$(mktemp -d)
export XDG_RUNTIME_DIR="$runtime"
export WAYLAND_DISPLAY=mapgyver-test
export GDK_BACKEND=wayland
export WEBKIT_DISABLE_DMABUF_RENDERER=1
weston --backend=x11-backend.so --use-pixman --output-count=2 --width=640 --height=480 --scale="${1:-1}" \
    --no-config --no-input --socket="$WAYLAND_DISPLAY" --idle-time=0 --log="$runtime/weston.log" &
compositor=$!
cleanup() {
    kill "$compositor" 2>/dev/null || true
    wait "$compositor" 2>/dev/null || true
    rm -rf -- "$runtime"
}
trap cleanup EXIT

for (( attempt=0; attempt<100; attempt++ )); do
    if [[ -S "$runtime/$WAYLAND_DISPLAY" ]]; then break; fi
    if ! kill -0 "$compositor" 2>/dev/null; then break; fi
    sleep 0.05
done

if [[ ! -S "$runtime/$WAYLAND_DISPLAY" ]] ||
    ! node apps/mapgyver/tests/browser/portable-rectangle.mjs --wayland; then
    cat "$runtime/weston.log" >&2
    exit 1
fi
