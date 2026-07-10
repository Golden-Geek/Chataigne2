#!/usr/bin/env bash
set -euo pipefail

repo_root="$(git rev-parse --show-toplevel)"
cd "${repo_root}"

commit_message="${*:-}"
if [[ -z "${commit_message}" ]]; then
  read -r -p "Commit message: " commit_message
fi
if [[ -z "${commit_message// }" ]]; then
  echo "Commit message cannot be empty." >&2
  exit 1
fi

git add -A
if ! git diff --cached --quiet; then
  git commit -m "${commit_message}"
else
  echo "No staged changes, skipping commit."
fi

echo "Supercommit complete."
