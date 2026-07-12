#!/usr/bin/env sh
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
sh "$script_dir/install-rust-toolchain.sh"
node_bin=$(sh "$script_dir/install-node.sh")
PATH="$node_bin:$PATH"
export PATH
sh "$script_dir/verify-toolchain.sh" --check-installed

if [ "$#" -gt 0 ]; then
  exec "$@"
fi
