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
apt-get -qq update
apt-get -qq install -y --no-install-recommends /tmp/mapgyver.deb

package=$(dpkg-deb -f /tmp/mapgyver.deb Package)
test "$(dpkg-query -W -f '${db:Status-Status}' "$package")" = installed
binary=/usr/bin/mapgyver2
test -x "$binary"
test -f "/usr/share/doc/$package/copyright"
desktop=$(dpkg-query -L "$package" | awk '/^\/usr\/share\/applications\/.*\.desktop$/')
grep -Fx 'Exec=mapgyver2' "$desktop"

linkage=$(ldd -r "$binary" 2>&1)
if grep -E 'not found|undefined symbol' <<< "$linkage"; then
    exit 1
fi
MAPGYVER_TEST_BINARY="$binary" node /tests/headless/media.mjs
CONTAINER
