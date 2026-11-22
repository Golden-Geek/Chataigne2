#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 1 || $(uname -s) != Linux || $(uname -m) != x86_64 ]]; then
    echo 'Usage on x86_64 Linux: build-windows-arm64.sh <new SDK directory>' >&2
    exit 2
fi

scripts=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
root=$(cd "$scripts/../../.." && pwd)
output=$(realpath -m -- "$1")
cache=${MAPGYVER_FFMPEG_DOWNLOADS:-"$root/target/ffmpeg-windows-arm64/downloads"}
work=$(mktemp -d)
trap 'rm -rf -- "$work"' EXIT
mkdir -p "$cache" "$(dirname "$output")"
test ! -e "$output"

# Keep the existing FFmpeg source pin as the source of truth for both Windows builds.
python3 - "$scripts" "$work/source-build.json" <<'PYTHON'
import json
from pathlib import Path
import sys

scripts = Path(sys.argv[1])
common = json.loads((scripts / "source-build.json").read_text())
pin = json.loads((scripts / "windows-arm64.json").read_text())
pin["ffmpeg"] = common["ffmpeg"]
pin["source_date_epoch"] = common["source_date_epoch"]
Path(sys.argv[2]).write_text(json.dumps(pin, indent=2) + "\n")
PYTHON

mapfile -t pin < <(python3 - "$work/source-build.json" <<'PYTHON'
import json
import sys

with open(sys.argv[1]) as stream:
    pin = json.load(stream)
ffmpeg, toolchain = pin["ffmpeg"], pin["toolchain"]
print(ffmpeg["url"], ffmpeg["sha256"], ffmpeg["root"], ffmpeg["version"], ffmpeg["commit"],
      toolchain["url"], toolchain["sha256"], toolchain["root"], pin["source_date_epoch"],
      *pin["configure"], sep="\n")
PYTHON
)

fetch() {
    local url=$1 digest=$2 destination=$3
    if [[ ! -f $destination ]] || ! printf '%s  %s\n' "$digest" "$destination" | sha256sum --status --check; then
        curl --fail --location --retry 2 --output "$work/download" "$url"
        printf '%s  %s\n' "$digest" "$work/download" | sha256sum --check
        mv -- "$work/download" "$destination"
    fi
}

source_archive="$cache/ffmpeg-source.tar.gz"
compiler_archive="$cache/llvm-mingw-20261117.tar.xz"
fetch "${pin[0]}" "${pin[1]}" "$source_archive"
fetch "${pin[5]}" "${pin[6]}" "$compiler_archive"
tar -xf "$source_archive" -C "$work"
tar -xf "$compiler_archive" -C "$work"
compiler="$work/${pin[7]}"
export PATH="$compiler/bin:$PATH"
export SOURCE_DATE_EPOCH="${pin[8]}"

cd "$work/${pin[2]}"
test "$(cat RELEASE)" = "${pin[3]}"
# Prevent FFmpeg's version detection from picking up the surrounding repository.
printf '%s-%s\n' "${pin[3]}" "${pin[4]:0:12}" > VERSION
./configure "${pin[@]:9}"
make -j "$(nproc)"
make install DESTDIR="$work/stage"

sdk="$work/stage/mapgyver-video-sdk"
shopt -s nullglob
for library in avcodec avformat avutil swresample swscale; do
    definitions=("$sdk/lib/$library-"*.def)
    test "${#definitions[@]}" -eq 1
    base=$(basename "${definitions[0]}" .def)
    llvm-dlltool -m arm64 -d "${definitions[0]}" -D "$base.dll" -l "$sdk/lib/$library.lib"
    llvm-readobj --file-headers "$sdk/bin/$base.dll" | grep -q 'Machine: IMAGE_FILE_MACHINE_ARM64 '
done
aarch64-w64-mingw32-clang -I"$sdk/include" "$scripts/tests/windows-arm64-imports.c" \
    "$sdk/lib/"*.lib -o "$work/import-test.exe"
llvm-readobj --file-headers --coff-imports "$work/import-test.exe" > "$work/imports.txt"
grep -q 'Machine: IMAGE_FILE_MACHINE_ARM64 ' "$work/imports.txt"
for library in "$sdk/bin/"*.dll; do
    grep -Fqi "Name: $(basename "$library")" "$work/imports.txt"
done

cp COPYING.LGPLv3 "$sdk/LICENSE.txt"
cp COPYING.GPLv3 "$sdk/GPL-3.0.txt"
cp "$work/source-build.json" "$sdk/source-build.json"
mkdir -p "$sdk/compiler-notices"
for notice in "$compiler"/LICENSE* "$compiler"/COPYING*; do
    if [[ -f $notice ]]; then cp "$notice" "$sdk/compiler-notices/"; fi
done
mv --no-target-directory -- "$sdk" "$output"
printf 'Windows ARM64 FFmpeg SDK: %s\n' "$output"
