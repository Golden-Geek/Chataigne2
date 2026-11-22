#!/usr/bin/env bash
set -euo pipefail

if [[ $(uname -s) != Linux ]] || ! command -v dpkg-shlibdeps >/dev/null; then
    echo 'Build this package on a Debian-based Linux host with dpkg-dev installed.' >&2
    exit 1
fi

profile=release
mode=()
if [[ $# -eq 1 && $1 = --debug ]]; then
    profile=debug
    mode=(--debug)
elif [[ $# -ne 0 ]]; then
    echo 'Usage: npm run package:mapgyver:linux -- [--debug]' >&2
    exit 2
fi

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
cd "$root"
npm run build:mapgyver
cd apps/mapgyver
npm exec -- tauri build --no-bundle "${mode[@]}" -- --locked

target=$(cargo metadata --locked --no-deps --format-version 1 |
    node -p "JSON.parse(require('node:fs').readFileSync(0, 'utf8')).target_directory")
binary="$target/$profile/mapgyver2"

work=$(mktemp -d)
trap 'rm -rf -- "$work"' EXIT
mkdir -p "$work/debian/map-gyver-2/usr/bin"
ln "$binary" "$work/debian/map-gyver-2/usr/bin/mapgyver2" 2>/dev/null ||
    cp "$binary" "$work/debian/map-gyver-2/usr/bin/mapgyver2"
cat > "$work/debian/control" <<'CONTROL'
Source: map-gyver-2
Section: graphics
Priority: optional

Package: map-gyver-2
Architecture: any
Description: Native video composition and projection mapping
CONTROL

dependencies=$(cd "$work" && dpkg-shlibdeps -O -e debian/map-gyver-2/usr/bin/mapgyver2)
config=$(node --input-type=module - "$dependencies" <<'JS'
const key = 'shlibs:Depends=';
const dependencyLine = process.argv[2].split('\n').find((line) => line.startsWith(key));
if (!dependencyLine?.slice(key.length)) throw new Error('No Debian runtime dependencies listed');
const depends = dependencyLine.slice(key.length).split(', ');
// wgpu loads the Vulkan loader at runtime.
depends.push('libvulkan1');
console.log(JSON.stringify({ bundle: { linux: { deb: { depends } } } }));
JS
)
npm exec -- tauri bundle --bundles deb "${mode[@]}" --config "$config"
