#!/usr/bin/env python3
"""Enforce active monorepo membership, dependency direction, and absence of gitlinks."""

from __future__ import annotations

import json
import subprocess
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


def fail(message: str) -> None:
    raise SystemExit(message)


def command(*arguments: str) -> str:
    return subprocess.check_output(arguments, cwd=ROOT, text=True).strip()


def verify_no_submodules() -> None:
    if (ROOT / ".gitmodules").exists():
        fail(".gitmodules is forbidden in the Golden monorepo")
    gitlinks = [
        line for line in command("git", "ls-files", "--stage").splitlines() if line.startswith("160000 ")
    ]
    if gitlinks:
        fail("gitlinks are forbidden in the Golden monorepo:\n" + "\n".join(gitlinks))


def verify_cargo_dependencies() -> None:
    metadata = json.loads(command("cargo", "metadata", "--format-version", "1", "--no-deps"))
    rules = json.loads(
        (ROOT / "docs" / "architecture" / "dependency-rules.v1.json").read_text(encoding="utf-8")
    )
    allowed = {rule["from"]: set(rule["may_depend_on"]) for rule in rules["rules"]}
    workspace_ids = set(metadata["workspace_members"])
    workspace_packages = {package["id"]: package for package in metadata["packages"] if package["id"] in workspace_ids}
    workspace_names = {package["name"] for package in workspace_packages.values()}

    for package in workspace_packages.values():
        if package["name"] not in allowed:
            continue
        actual = {
            dependency["name"]
            for dependency in package["dependencies"]
            if dependency["name"] in workspace_names
        }
        unexpected = actual - allowed[package["name"]]
        if unexpected:
            fail(f"{package['name']} has forbidden workspace dependencies: {sorted(unexpected)}")

        manifest = Path(package["manifest_path"])
        if "legacy" in manifest.parts:
            fail(f"active workspace member is inside legacy sources: {manifest}")


def verify_javascript_workspace() -> None:
    lock = json.loads((ROOT / "package-lock.json").read_text(encoding="utf-8"))
    expected = {
        "apps/chataigne/ui",
        "packages/golden-alchemist-ui",
        "packages/golden-graph-ui",
        "packages/golden-runtime-client",
        "packages/golden-statechart-ui",
        "packages/golden-ui",
    }
    packages = set(lock.get("packages", {}))
    missing = expected - packages
    if missing:
        fail(f"root JavaScript lock is missing workspace packages: {sorted(missing)}")


def main() -> None:
    verify_no_submodules()
    verify_cargo_dependencies()
    verify_javascript_workspace()
    print("workspace architecture: valid")


if __name__ == "__main__":
    main()
