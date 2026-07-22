#!/usr/bin/env sh
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repository_root=$(CDPATH= cd -- "$script_dir/../.." && pwd)
CARGO_TARGET_DIR="$repository_root/target"
export CARGO_TARGET_DIR
sh "$script_dir/verify-toolchain.sh" --check-installed
pwsh -NoProfile -File "$repository_root/tools/workspace-hygiene.ps1" -Action Audit

if [ "$#" -gt 0 ]; then
  exec "$@"
fi
