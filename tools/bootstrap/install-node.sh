#!/usr/bin/env sh
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repository_root=$(CDPATH= cd -- "$script_dir/../.." && pwd)
manifest="$script_dir/toolchain.json"

case "$(uname -s):$(uname -m)" in
  Darwin:x86_64) distribution_key=macos_x64 ;;
  Darwin:arm64 | Darwin:aarch64) distribution_key=macos_arm64 ;;
  Linux:x86_64 | Linux:amd64) distribution_key=linux_x64 ;;
  Linux:arm64 | Linux:aarch64) distribution_key=linux_arm64 ;;
  *)
    echo "Unsupported Node host: $(uname -s) $(uname -m)" >&2
    exit 1
    ;;
esac

eval "$(python3 - "$manifest" "$distribution_key" <<'PY'
import json
import shlex
import sys

with open(sys.argv[1], encoding="utf-8") as stream:
    manifest = json.load(stream)
distribution = manifest["node"]["distributions"][sys.argv[2]]
values = {
    "version": manifest["node"]["version"],
    "base_url": manifest["node"]["base_url"],
    "archive_name": distribution["file"],
    "expected_hash": distribution["sha256"],
}
for key, value in values.items():
    print(f"{key}={shlex.quote(value)}")
PY
)"

cache_root="$repository_root/target/toolchains"
download_directory="$cache_root/downloads"
install_directory="$cache_root/node-v$version-$distribution_key"
node_executable="$install_directory/bin/node"
archive_path="$download_directory/$archive_name"

sha256_file() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  else
    shasum -a 256 "$1" | awk '{print $1}'
  fi
}

if [ ! -x "$node_executable" ]; then
  mkdir -p "$download_directory"
  actual_hash=""
  if [ -f "$archive_path" ]; then
    actual_hash=$(sha256_file "$archive_path")
  fi
  if [ "$actual_hash" != "$expected_hash" ]; then
    url="$base_url/v$version/$archive_name"
    echo "Downloading pinned Node $version from $url" >&2
    if command -v curl >/dev/null 2>&1; then
      curl --fail --location --silent --show-error "$url" --output "$archive_path"
    else
      wget -q "$url" -O "$archive_path"
    fi
  fi
  actual_hash=$(sha256_file "$archive_path")
  if [ "$actual_hash" != "$expected_hash" ]; then
    rm -f "$archive_path"
    echo "Node archive SHA-256 mismatch: expected $expected_hash, got $actual_hash." >&2
    exit 1
  fi

  temporary_directory="$install_directory.extracting-$$"
  rm -rf "$temporary_directory"
  mkdir -p "$temporary_directory"
  tar -xJf "$archive_path" -C "$temporary_directory"
  set -- "$temporary_directory"/*
  if [ "$#" -ne 1 ] || [ ! -d "$1" ]; then
    rm -rf "$temporary_directory"
    echo "Pinned Node archive must contain exactly one root directory." >&2
    exit 1
  fi
  rm -rf "$install_directory"
  mv "$1" "$install_directory"
  rm -rf "$temporary_directory"
fi

if [ ! -x "$node_executable" ]; then
  echo "Pinned Node executable was not installed at $node_executable." >&2
  exit 1
fi
printf '%s\n' "$install_directory/bin"
