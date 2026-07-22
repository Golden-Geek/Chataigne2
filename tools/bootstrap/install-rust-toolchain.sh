#!/usr/bin/env sh
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
manifest="$script_dir/toolchain.json"

case "$(uname -s):$(uname -m)" in
  Darwin:x86_64) host_key=macos_x64 ;;
  Darwin:arm64 | Darwin:aarch64) host_key=macos_arm64 ;;
  Linux:x86_64 | Linux:amd64) host_key=linux_x64 ;;
  Linux:arm64 | Linux:aarch64) host_key=linux_arm64 ;;
  Linux:armv7l | Linux:armv7) host_key=linux_armv7 ;;
  *)
    echo "Unsupported Rust host: $(uname -s) $(uname -m)" >&2
    exit 1
    ;;
esac

eval "$(python3 - "$manifest" "$host_key" <<'PY'
import json
import shlex
import sys

with open(sys.argv[1], encoding="utf-8") as stream:
    manifest = json.load(stream)
rust = manifest["rust"]
values = {
    "channel": rust["channel"],
    "host": rust["hosts"][sys.argv[2]],
    "profile": rust["profile"],
    "components": " ".join(rust["components"]),
}
for key, value in values.items():
    print(f"{key}={shlex.quote(value)}")
PY
)"

toolchain="$channel-$host"
set -- toolchain install "$toolchain" --profile "$profile"
for component in $components; do
  set -- "$@" --component "$component"
done
rustup "$@"
rustup override set "$toolchain"
echo "Activated repository-pinned Rust toolchain $toolchain."
