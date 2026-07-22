#!/usr/bin/env python3
"""Reject unreviewed changes to the workspace's duplicate Cargo versions."""

from __future__ import annotations

import argparse
import json
import subprocess
from collections import defaultdict
from pathlib import Path
from typing import Any


REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
BASELINE_PATH = Path(__file__).with_name("duplicate-versions.json")


def collect_duplicates(metadata: dict[str, Any]) -> dict[str, list[str]]:
    versions: dict[str, set[str]] = defaultdict(set)
    for package in metadata["packages"]:
        versions[package["name"]].add(package["version"])
    return {
        name: sorted(found)
        for name, found in sorted(versions.items())
        if len(found) > 1
    }


def load_metadata() -> dict[str, Any]:
    completed = subprocess.run(
        ["cargo", "metadata", "--format-version", "1", "--locked", "--all-features"],
        cwd=REPOSITORY_ROOT,
        check=True,
        capture_output=True,
        text=True,
        encoding="utf-8",
    )
    return json.loads(completed.stdout)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--print-baseline",
        action="store_true",
        help="Print the current duplicate set for reviewed baseline updates.",
    )
    args = parser.parse_args()

    actual = collect_duplicates(load_metadata())
    if args.print_baseline:
        print(json.dumps(actual, indent=2, sort_keys=True))
        return 0

    expected = json.loads(BASELINE_PATH.read_text(encoding="utf-8"))
    if actual == expected:
        print(f"duplicate-version baseline PASS ({len(actual)} reviewed crate names)")
        return 0

    print("Cargo duplicate-version set changed; review and regenerate with --print-baseline.")
    print("Expected:")
    print(json.dumps(expected, indent=2, sort_keys=True))
    print("Actual:")
    print(json.dumps(actual, indent=2, sort_keys=True))
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
