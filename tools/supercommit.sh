#!/usr/bin/env bash
set -euo pipefail

repo_root="$(git rev-parse --show-toplevel 2>/dev/null || true)"
if [[ -z "${repo_root}" ]]; then
  echo "Error: this script must run inside a Git repository."
  exit 1
fi

cd "${repo_root}"

if [[ $# -gt 0 ]]; then
  commit_message="$*"
else
  read -r -p "Commit message: " commit_message
fi

if [[ -z "${commit_message// }" ]]; then
  echo "Error: commit message cannot be empty."
  exit 1
fi

submodule_entries="$(git config --file .gitmodules --get-regexp path 2>/dev/null || true)"

if [[ -n "${submodule_entries}" ]]; then
  while read -r _key submodule_path; do
    [[ -z "${submodule_path}" ]] && continue

    if [[ ! -d "${submodule_path}" ]]; then
      echo "Skipping missing submodule path: ${submodule_path}"
      continue
    fi

    if [[ ! -d "${submodule_path}/.git" && ! -f "${submodule_path}/.git" ]]; then
      echo "Skipping uninitialized submodule: ${submodule_path}"
      continue
    fi

    echo "Processing submodule: ${submodule_path}"

    branch_name="$(git -C "${submodule_path}" symbolic-ref --quiet --short HEAD || true)"
    if [[ -z "${branch_name}" ]]; then
      echo "Error: submodule '${submodule_path}' is in detached HEAD."
      echo "Checkout a branch there first, then run supercommit again."
      exit 1
    fi

    git -C "${submodule_path}" add -A

    if git -C "${submodule_path}" diff --cached --quiet; then
      echo "No staged changes in ${submodule_path}, skipping commit."
    else
      git -C "${submodule_path}" commit -m "${commit_message}"
    fi
  done <<< "${submodule_entries}"
fi

echo "Processing main repository..."
git add -A

if git diff --cached --quiet; then
  echo "No staged changes in main repository, skipping commit."
else
  git commit -m "${commit_message}"
fi

echo "Supercommit complete."
