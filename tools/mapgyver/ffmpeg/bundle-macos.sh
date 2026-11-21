#!/usr/bin/env bash
set -euo pipefail

if [[ $(uname -s) != Darwin ]]; then
    echo "Build the macOS app bundle on a Mac." >&2
    exit 1
fi

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
root=$(cd "$here/../../.." && pwd)
minimum_macos=13.0
work=$(mktemp -d "${TMPDIR:-/tmp}/mapgyver-ffmpeg.XXXXXX")
trap 'rm -rf -- "$work"' EXIT

manifest="$here/source-build.json"
pin() {
    node -p 'require(process.argv[1]).ffmpeg[process.argv[2]]' "$manifest" "$1"
}

archive="$work/ffmpeg.tar.gz"
curl --fail --location --retry 3 --max-time 120 --output "$archive" "$(pin url)"
printf '%s  %s\n' "$(pin sha256)" "$archive" | shasum -a 256 -c -
tar -xzf "$archive" -C "$work"
source="$work/$(pin root)"
stage="$work/sdk"
mkdir "$work/build"
(
    cd "$work/build"
    export MACOSX_DEPLOYMENT_TARGET="$minimum_macos"
    "$source/configure" \
        --prefix="$stage" \
        --install-name-dir=@rpath \
        --extra-version="mapgyver-$(pin commit | cut -c1-10)" \
        --disable-autodetect --disable-gpl --disable-nonfree --enable-version3 \
        --enable-shared --disable-static --disable-debug --disable-doc \
        --disable-programs --disable-encoders --disable-avdevice --disable-avfilter \
        --disable-network
    make -s -j "$(sysctl -n hw.logicalcpu)"
    make -s install
)

# Resolve each dylib's own install name so the filenames match the Mach-O imports.
mkdir "$work/frameworks"
for component in avcodec avformat avutil swresample swscale; do
    library="$stage/lib/lib$component.dylib"
    name=$(otool -D "$library" | sed -n 's|^@rpath/\(lib[^/]*\.dylib\)$|\1|p')
    [[ "$name" = "lib$component."*.dylib ]]
    cp -L "$library" "$work/frameworks/$name"
done

config=$(node --input-type=module - "$work/frameworks" "$source" "$here" "$minimum_macos" <<'JS'
import { readdirSync } from 'node:fs';
import { join } from 'node:path';

const [frameworks, source, tools, minimumSystemVersion] = process.argv.slice(2);
const libraries = readdirSync(frameworks).map((library) => join(frameworks, library));
const files = {
    'Resources/licenses/video/FFmpeg-LGPL-3.0.txt': join(source, 'COPYING.LGPLv3'),
    'Resources/licenses/video/source-build.json': join(tools, 'source-build.json'),
    'Resources/licenses/video/bundle-macos.sh': join(tools, 'bundle-macos.sh')
};
console.log(JSON.stringify({ bundle: { macOS: { minimumSystemVersion, frameworks: libraries, files } } }));
JS
)

cd "$root"
npm run build:mapgyver
cd "$root/apps/mapgyver"
export FFMPEG_DIR="$stage"
export APPLE_SIGNING_IDENTITY="${APPLE_SIGNING_IDENTITY:--}"
npm exec -- tauri build --bundles app --config "$config" "$@" -- --locked
