#!/usr/bin/env bash
set -euo pipefail

archive=$(realpath "$1")
image=$2
node=$(realpath "$(command -v node)")
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." && pwd)

docker run --rm -i \
    -v "$archive:/tmp/mapgyver.deb:ro" \
    -v "$node:/usr/local/bin/node:ro" \
    -v "$root:/workbench:ro" \
    "$image" bash -se <<'CONTAINER'
set -euo pipefail
export DEBIAN_FRONTEND=noninteractive

archive=/tmp/mapgyver.deb
architecture=$(dpkg-deb -f "$archive" Architecture)
graphics=(mesa-vulkan-drivers)
if [[ $architecture != "$(dpkg --print-architecture)" ]]; then
    dpkg --add-architecture "$architecture"
    graphics+=("mesa-vulkan-drivers:$architecture")
fi
apt-get -qq update
apt-get -qq install -y --no-install-recommends \
    "$archive" libatomic1 "${graphics[@]}" ffmpeg python3 dbus-daemon \
    xvfb xauth x11-utils xdotool xserver-xorg-core xserver-xorg-video-dummy x11-xserver-utils \
    weston sway grim

cd /workbench
export PLAYWRIGHT_BROWSERS_PATH=/opt/playwright
node node_modules/playwright-core/cli.js install --with-deps --only-shell chromium
useradd --create-home mapgyver-qa
# WebKit and the display servers run as an ordinary desktop user.
runuser -u mapgyver-qa -- env MAPGYVER_TEST_BINARY=/usr/bin/mapgyver2 bash -se <<'DESKTOP'
set -euo pipefail
printf '\nVerifying the native X11 workbench\n'
GDK_BACKEND=x11 WEBKIT_DISABLE_DMABUF_RENDERER=1 xvfb-run -a \
    node apps/mapgyver/tests/native_output/desktop-readiness.mjs

printf '\nVerifying media and Screen Warping\n'
node apps/mapgyver/tests/browser/portable-preview.mjs

printf '\nVerifying X11 Rectangle and Fullscreen output\n'
GDK_BACKEND=x11 WEBKIT_DISABLE_DMABUF_RENDERER=1 xvfb-run -a -s '-screen 0 1280x1024x24' \
    node apps/mapgyver/tests/browser/portable-rectangle.mjs

printf '\nVerifying XRandR output recovery\n'
bash apps/mapgyver/tests/native_output/check-x11-recovery.sh

for scales in 1,1 2,2 1,2 2,1; do
    printf '\nVerifying Wayland output at scales %s\n' "$scales"
    xvfb-run -a -s '-screen 0 2560x1200x24' \
        bash apps/mapgyver/tests/native_output/check-wayland-output.sh "${scales%,*}" "${scales#*,}"
done

printf '\nVerifying Wayland output recovery\n'
bash apps/mapgyver/tests/native_output/check-wayland-recovery.sh
DESKTOP
CONTAINER
