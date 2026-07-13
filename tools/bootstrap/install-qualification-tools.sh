#!/usr/bin/env sh
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
manifest="$script_dir/toolchain.json"

eval "$(python3 - "$manifest" <<'PY'
import json
import shlex
import sys

with open(sys.argv[1], encoding="utf-8") as stream:
    manifest = json.load(stream)
tools = manifest["qualification_tools"]
print(f"cargo_deny_version={shlex.quote(tools['cargo_deny'])}")
print(f"cargo_machete_version={shlex.quote(tools['cargo_machete'])}")
PY
)"

ensure_tool() {
  package="$1"
  executable="$2"
  version="$3"
  version_prefix="$4"
  installed=""
  if command -v "$executable" >/dev/null 2>&1; then
    installed=$($executable --version 2>&1 || true)
  fi
  case "$installed" in
    "$version_prefix$version"*) return ;;
  esac
  cargo install --locked --force "$package" --version "$version"
}

ensure_tool cargo-deny cargo-deny "$cargo_deny_version" "cargo-deny "
ensure_tool cargo-machete cargo-machete "$cargo_machete_version" ""
sh "$script_dir/verify-toolchain.sh" --check-qualification-tools
