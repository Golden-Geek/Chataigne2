#!/usr/bin/env bash
set -euo pipefail

if [[ $(uname -s) != Linux ]] || ! command -v dpkg-shlibdeps >/dev/null; then
    echo 'Build this package on a Debian-based Linux host with dpkg-dev installed.' >&2
    exit 1
fi

profile=release
mode=()
target_args=()
target_subdirectory=
architecture=$(dpkg --print-architecture)
for argument in "$@"; do
    case "$argument" in
        --debug) profile=debug; mode=(--debug) ;;
        --armhf)
            rust_target=armv7-unknown-linux-gnueabihf
            target_args=(--target "$rust_target")
            target_subdirectory="$rust_target/"
            architecture=armhf
            ;;
        *)
            echo 'Usage: npm run package:mapgyver:linux -- [--debug] [--armhf]' >&2
            exit 2
            ;;
    esac
done

work=$(mktemp -d)
trap 'rm -rf -- "$work"' EXIT
if [[ ${#target_args[@]} -ne 0 ]]; then
    export CARGO_TARGET_ARMV7_UNKNOWN_LINUX_GNUEABIHF_LINKER=arm-linux-gnueabihf-gcc
    # Scope pkg-config to the target so Cargo's host-side generators use host libraries.
    export PKG_CONFIG_LIBDIR_armv7_unknown_linux_gnueabihf=/usr/lib/arm-linux-gnueabihf/pkgconfig:/usr/share/pkgconfig
    export PKG_CONFIG_SYSROOT_DIR_armv7_unknown_linux_gnueabihf=/
    # ffmpeg-sys-next 9.0.1's fallback Vulkan header assumes a 64-bit layout.
    # Pre-include Vulkan's public header so bindgen sees the target ABI.
    clang_args=${BINDGEN_EXTRA_CLANG_ARGS_armv7_unknown_linux_gnueabihf:-${BINDGEN_EXTRA_CLANG_ARGS:-}}
    export BINDGEN_EXTRA_CLANG_ARGS_armv7_unknown_linux_gnueabihf="$clang_args -include /usr/include/vulkan/vulkan.h"
    if [[ -z ${FFMPEG_DIR:-} ]]; then
        # ffmpeg-sys compiles a version probe on the host using these headers.
        # Expose the target FFmpeg API without shadowing the host's libc headers.
        export FFMPEG_DIR="$work/ffmpeg"
        mkdir -p "$FFMPEG_DIR/include"
        for library in avcodec avformat avutil swresample swscale; do
            ln -s "/usr/include/arm-linux-gnueabihf/lib$library" "$FFMPEG_DIR/include/"
        done
        ln -s /usr/lib/arm-linux-gnueabihf "$FFMPEG_DIR/lib"
    fi
fi

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
cd "$root"
npm run build:mapgyver
cd apps/mapgyver
GC_UI_ASSUME_BUILT=1 npm exec -- tauri build --no-bundle "${mode[@]}" "${target_args[@]}" -- --locked

target=$(cargo metadata --locked --no-deps --format-version 1 |
    node -p "JSON.parse(require('node:fs').readFileSync(0, 'utf8')).target_directory")
binary="$target/$target_subdirectory$profile/mapgyver2"

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

depends=$(cd "$work" && DEB_HOST_ARCH="$architecture" dpkg-shlibdeps -O -e debian/map-gyver-2/usr/bin/mapgyver2 |
    sed -n 's/^shlibs:Depends=//p')
if [[ -z $depends ]]; then
    echo 'No Debian runtime dependencies listed.' >&2
    exit 1
fi
# wgpu loads the Vulkan loader at runtime.
depends+=', libvulkan1'

npm exec -- tauri bundle --bundles deb "${mode[@]}" "${target_args[@]}"
archive_name=$(node -p 'const { productName, version } = require("./tauri.conf.json");
    `${productName}_${version}_${process.argv[1]}.deb`' "$architecture")
archive="$target/$target_subdirectory$profile/bundle/deb/$archive_name"

# Tauri adds distro-independent dependency names, including the pre-time64 GTK
# package. Keep its bundle contents and use the target distribution's ELF dependencies.
dpkg-deb --raw-extract "$archive" "$work/package"
node --input-type=module - "$work/package/DEBIAN/control" "$depends" <<'JS'
import { readFileSync, writeFileSync } from 'node:fs';

const [, , path, dependencies] = process.argv;
const control = readFileSync(path, 'utf8');
const field = /^Depends:[^\n]*(?:\n[ \t][^\n]*)*/m;
if (!field.test(control)) throw new Error('The Debian bundle has no Depends field');
writeFileSync(path, control.replace(field, `Depends: ${dependencies}`));
JS
dpkg-deb --build --root-owner-group "$work/package" "$work/repacked.deb"
mv -- "$work/repacked.deb" "$archive"
