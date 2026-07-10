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


def verify_no_obsolete_sources() -> None:
    forbidden = (
        "legacy",
        "src",
        "src-ui",
        "submodules",
        "builtin_formulas",
        "capabilities",
        "gen",
        "build.rs",
        "tauri.conf.json",
    )
    tracked = [
        path
        for path in command("git", "ls-files", "--", *forbidden).splitlines()
        if (ROOT / path).exists()
    ]
    if tracked:
        fail("obsolete architecture sources are forbidden:\n" + "\n".join(tracked))


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
        legacy_features = [name for name in package.get("features", {}) if "legacy" in name.lower()]
        if legacy_features:
            fail(f"{package['name']} exposes forbidden legacy features: {legacy_features}")
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

    manifests = {
        path: json.loads((ROOT / path / "package.json").read_text(encoding="utf-8"))
        for path in expected
    }
    package_layers = {
        "@golden/alchemist-ui": "golden-alchemist-ui",
        "@golden/chataigne-ui": "apps/chataigne",
        "@golden/graph-ui": "golden-graph-ui",
        "@golden/runtime-client": "golden-runtime-client",
        "@golden/statechart-ui": "golden-statechart-ui",
        "@golden/ui": "golden-ui",
    }
    rules = json.loads(
        (ROOT / "docs" / "architecture" / "dependency-rules.v1.json").read_text(encoding="utf-8")
    )
    forbidden = {rule["layer"]: set(rule["imports"]) for rule in rules["forbidden"]}
    for path, manifest in manifests.items():
        source = package_layers[manifest["name"]]
        dependencies = {
            **manifest.get("dependencies", {}),
            **manifest.get("peerDependencies", {}),
        }
        for name, requirement in dependencies.items():
            if "legacy" in requirement:
                fail(f"{source} depends on legacy source through {name}: {requirement}")
            target = package_layers.get(name)
            if target in forbidden.get(source, set()):
                fail(f"{source} has forbidden package dependency {target}")


def main() -> None:
    verify_no_submodules()
    verify_no_obsolete_sources()
    verify_cargo_dependencies()
    verify_javascript_workspace()
    print("workspace architecture: valid")


if __name__ == "__main__":
    main()
