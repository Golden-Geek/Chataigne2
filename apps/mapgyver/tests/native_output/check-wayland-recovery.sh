#!/usr/bin/env bash
set -euo pipefail

runtime=$(mktemp -d)
export XDG_RUNTIME_DIR="$runtime"
export WLR_BACKENDS=headless WLR_HEADLESS_OUTPUTS=2 WLR_RENDERER=pixman
export GDK_BACKEND=wayland WEBKIT_DISABLE_DMABUF_RENDERER=1
unset WAYLAND_DISPLAY WAYLAND_SOCKET SWAYSOCK
cat > "$runtime/sway.conf" <<'CONFIG'
xwayland disable
output * mode 640x480
CONFIG
sway -c "$runtime/sway.conf" > "$runtime/sway.log" 2>&1 &
compositor=$!
cleanup() {
    kill "$compositor" 2>/dev/null || true
    wait "$compositor" 2>/dev/null || true
    rm -rf -- "$runtime"
}
trap cleanup EXIT

for (( attempt=0; attempt<100; attempt++ )); do
    SWAYSOCK=$(find "$runtime" -maxdepth 1 -name 'sway-ipc.*.sock' -type s -print -quit)
    WAYLAND_DISPLAY=$(find "$runtime" -maxdepth 1 -name 'wayland-*' -type s -printf '%f\n' -quit)
    if [[ -n "$SWAYSOCK" && -n "$WAYLAND_DISPLAY" ]]; then break; fi
    if ! kill -0 "$compositor" 2>/dev/null; then break; fi
    sleep 0.05
done
export SWAYSOCK WAYLAND_DISPLAY

if [[ -z "$SWAYSOCK" || -z "$WAYLAND_DISPLAY" ]] ||
    ! node apps/mapgyver/tests/browser/portable-rectangle.mjs --wayland-recovery; then
    cat "$runtime/sway.log" >&2
    exit 1
fi
