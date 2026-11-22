#!/usr/bin/env bash
set -euo pipefail

root=$PWD
runtime=$(mktemp -d)
export GDK_BACKEND=x11 WEBKIT_DISABLE_DMABUF_RENDERER=1
export XAUTHORITY="$runtime/Xauthority"
cookie=$(mcookie)
touch "$XAUTHORITY"
xauth -q add :0 MIT-MAGIC-COOKIE-1 "$cookie"
server_options=()
if [[ -n "${MAPGYVER_TEST_XORG_MODULE_PATH:-}" ]]; then
    server_options=(-modulepath "$MAPGYVER_TEST_XORG_MODULE_PATH")
fi
cat > "$runtime/xorg.conf" <<'CONFIG'
Section "ServerFlags"
    Option "AutoAddDevices" "false"
    Option "AutoAddGPU" "false"
EndSection
Section "Device"
    Identifier "Dummy"
    Driver "dummy"
    VideoRam 256000
EndSection
Section "Monitor"
    Identifier "DummyMonitor"
    HorizSync 30-100
    VertRefresh 50-80
    Modeline "640x480-fixture" 25.175 640 656 752 800 480 490 492 525 -HSync -VSync
    Option "PreferredMode" "640x480-fixture"
EndSection
Section "Screen"
    Identifier "DummyScreen"
    Device "Dummy"
    Monitor "DummyMonitor"
    DefaultDepth 24
    SubSection "Display"
        Depth 24
        Virtual 1280 480
        Modes "640x480-fixture"
    EndSubSection
EndSection
CONFIG
cd "$runtime"
Xorg -novtswitch -sharevts -noreset -pn -nolisten tcp -nolisten unix -listen local -displayfd 3 \
    -config xorg.conf -logfile "$runtime/Xorg.log" -auth "$XAUTHORITY" "${server_options[@]}" \
    3>"$runtime/display" >"$runtime/console" 2>&1 &
server=$!
cleanup() {
    kill "$server" 2>/dev/null || true
    wait "$server" 2>/dev/null || true
    rm -rf -- "$runtime"
}
trap cleanup EXIT

for (( attempt=0; attempt<100; attempt++ )); do
    if [[ -s "$runtime/display" ]] || ! kill -0 "$server" 2>/dev/null; then break; fi
    sleep 0.05
done
if [[ ! -s "$runtime/display" ]]; then
    tail -35 "$runtime/console" >&2
    exit 1
fi
export DISPLAY=":$(cat "$runtime/display")"
xauth -q add "$DISPLAY" MIT-MAGIC-COOKIE-1 "$cookie"
xrandr --addmode DUMMY1 640x480-fixture
xrandr --output DUMMY0 --mode 640x480-fixture --pos 0x0 --primary \
    --output DUMMY1 --mode 640x480-fixture --pos 640x0

cd "$root"
node apps/mapgyver/tests/browser/portable-rectangle.mjs --x11-recovery
