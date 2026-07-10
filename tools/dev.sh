#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

if [[ "${1:-}" != "--skip-install" ]]; then
  npm ci
fi

cargo check --workspace --all-targets --all-features
npm run check

echo "Golden monorepo dependencies and workspace checks are ready."
