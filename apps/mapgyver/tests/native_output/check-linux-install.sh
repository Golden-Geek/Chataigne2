#!/usr/bin/env bash
set -euo pipefail

archive=$(realpath "$1")
image=$2
node=$(realpath "$(command -v node)")
tests=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)

docker run --rm -i \
    -v "$archive:/tmp/mapgyver.deb:ro" \
    -v "$node:/usr/local/bin/node:ro" \
    -v "$tests:/tests:ro" \
    "$image" bash -se <<'CONTAINER'
set -euo pipefail
export DEBIAN_FRONTEND=noninteractive

archive=/tmp/mapgyver.deb
architecture=$(dpkg-deb -f "$archive" Architecture)
emulator=()
if [[ $architecture != "$(dpkg --print-architecture)" ]]; then
    test "$architecture" = armhf
    dpkg --add-architecture armhf
    emulator=(qemu-user libatomic1)
fi
apt-get -qq update
apt-get -qq install -y --no-install-recommends "$archive" "${emulator[@]}"

package=$(dpkg-deb -f "$archive" Package)
test "$(dpkg-query -W -f '${db:Status-Status}' "$package")" = installed
binary=/usr/bin/mapgyver2
test -x "$binary"
test -f "/usr/share/doc/$package/copyright"
desktop=$(dpkg-query -L "$package" | awk '/^\/usr\/share\/applications\/.*\.desktop$/')
grep -Fx 'Exec=mapgyver2' "$desktop"

test_binary=$binary
if [[ ${#emulator[@]} -ne 0 ]]; then
    # Ask the armhf linker to trace dependencies and resolve relocations under QEMU.
    linkage=$(qemu-arm -E LD_TRACE_LOADED_OBJECTS=1 -E LD_WARN=1 -E LD_BIND_NOW=1 "$binary" 2>&1)
    test_binary=/tmp/mapgyver2-armhf
    printf '#!/bin/sh\nexec qemu-arm /usr/bin/mapgyver2 "$@"\n' > "$test_binary"
    chmod +x "$test_binary"
else
    linkage=$(ldd -r "$binary" 2>&1)
fi
if grep -E 'not found|undefined symbol' <<< "$linkage"; then
    exit 1
fi
MAPGYVER_TEST_BINARY="$test_binary" node /tests/headless/media.mjs
CONTAINER
