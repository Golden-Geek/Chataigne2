#!/usr/bin/env bash
set -euo pipefail

primary_scale="${1:-1}"
secondary_scale="${2:-$primary_scale}"
if [[ ! "$primary_scale" =~ ^[12]$ || ! "$secondary_scale" =~ ^[12]$ ]]; then
    echo 'Select output scales of 1 or 2.' >&2
    exit 2
fi

runtime=$(mktemp -d)
export XDG_RUNTIME_DIR="$runtime"
export WAYLAND_DISPLAY=mapgyver-test
export GDK_BACKEND=wayland
export WEBKIT_DISABLE_DMABUF_RENDERER=1
export MAPGYVER_TEST_WAYLAND_SCALES="$primary_scale,$secondary_scale"
cat > "$runtime/weston.ini" <<CONFIG
[output]
name=screen0
mode=640x480
scale=$primary_scale

[output]
name=screen1
mode=640x480
scale=$secondary_scale
CONFIG
weston --backend=x11-backend.so --use-pixman --output-count=2 --config="$runtime/weston.ini" \
    --no-input --socket="$WAYLAND_DISPLAY" --idle-time=0 --log="$runtime/weston.log" &
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
