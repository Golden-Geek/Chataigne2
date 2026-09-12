#!/usr/bin/env python3
"""Qualify persisted full-workbench graph projects at three live-node thresholds."""

from __future__ import annotations

import argparse
import json
import os
import platform
import re
import subprocess
import sys
import tomllib
from collections.abc import Sequence
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

if __package__:
    from .formula_scale import cargo_environment
    from .graph_fixture import DEFAULT_SOURCE, write_fixture
    from .runtime_scale import command_output, sha256_bytes, utc_now, working_tree_sha
else:
    from formula_scale import cargo_environment
    from graph_fixture import DEFAULT_SOURCE, write_fixture
    from runtime_scale import command_output, sha256_bytes, utc_now, working_tree_sha


EVIDENCE_ID = "product.authored-graph-scale.local"
TARGETS = (1_000, 10_000, 100_000)
RESULT_PREFIX = "AUTHORED_SCALE_RESULT="
RESULT_PATTERN = re.compile(r"test result: ok\. 1 passed; 0 failed;")
TEST_COMMAND = (
    "cargo", "test", "--locked", "-q", "-p", "Chataigne2", "--bin", "Chataigne2",
    "--target-dir", "target/t16-app-default",
    "authored_graph_project_loads_ticks_and_round_trips", "--", "--ignored", "--nocapture",
    "--test-threads=1",
)
RESULT_FIELDS = {
    "authored_nodes", "graph_roots", "minimum_live_nodes", "prepared_nodes",
    "reloaded_nodes", "load_ms", "prepare_ms", "tick_us", "tick_callbacks",
    "tick_snapshot_builds", "tick_snapshot_nodes_cloned", "tick_edits_applied",
    "save_ms", "saved_bytes", "reload_ms", "load_rss_mb", "prepare_rss_mb",
    "reload_rss_mb",
}


TICK_FIELDS = {
    "tick_us", "tick_callbacks", "tick_snapshot_builds", "tick_snapshot_nodes_cloned",
    "tick_edits_applied",
}


def parse_result(output: str, target: int, graph_roots: int) -> dict[str, Any]:
    rows = []
    for line in output.splitlines():
        if RESULT_PREFIX in line:
            try:
                rows.append(json.loads(line.split(RESULT_PREFIX, 1)[1]))
            except json.JSONDecodeError as error:
                raise ValueError(f"invalid authored graph result: {error}") from error
    if len(rows) != 1 or not RESULT_PATTERN.search(output):
        raise ValueError(f"expected one passing authored graph result, found {len(rows)}")
    row = rows[0]
    if not isinstance(row, dict) or row.keys() != RESULT_FIELDS:
        raise ValueError("authored graph result fields differ from the qualification contract")
    if any(type(value) is not int or value < 0 for key, value in row.items() if key not in TICK_FIELDS):
        raise ValueError("authored graph measurements must be nonnegative integers")
    for field in TICK_FIELDS:
        values = row[field]
        if (
            not isinstance(values, list)
            or len(values) != 5
            or any(type(value) is not int or value < 0 for value in values)
        ):
            raise ValueError(f"authored graph {field} must contain five nonnegative measurements")
    if row["minimum_live_nodes"] != target or row["graph_roots"] != graph_roots:
        raise ValueError("authored graph result does not match the generated fixture")
    if row["authored_nodes"] < target or row["reloaded_nodes"] < target:
        raise ValueError("authored graph load or reload missed the live-node target")
    if row["prepared_nodes"] < row["authored_nodes"] or row["saved_bytes"] == 0:
        raise ValueError("authored graph prepare or save produced invalid counts")
    return row


def resolve_output_dir(root: Path, value: Path | None) -> Path:
    target_root = (root / "target").resolve()
    output_dir = (
        value.resolve() if value is not None and value.is_absolute()
        else (root / value).resolve() if value is not None
        else target_root / "qualification" / "authored-graph-scale"
        / datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    )
    if not output_dir.is_relative_to(target_root) or output_dir == target_root:
        raise ValueError("authored graph output must be a child of the workspace target directory")
    if output_dir.exists() and any(output_dir.iterdir()):
        raise ValueError("authored graph output directory already contains artifacts")
    return output_dir


def build_report(root: Path, output_dir: Path) -> dict[str, Any]:
    started_at = utc_now()
    tested_tree_sha = working_tree_sha(root)
    commit_sha = command_output(root, ("git", "rev-parse", "HEAD"))
    toolchain = {
        "rustc": command_output(root, ("rustc", "-Vv")),
        "cargo": command_output(root, ("cargo", "-V")),
        "os": platform.platform(),
    }
    manifest = tomllib.loads((root / "apps/chataigne/Cargo.toml").read_text(encoding="utf-8"))
    source_path = root / DEFAULT_SOURCE
    source_sha = sha256_bytes(source_path.read_bytes())
    environment = cargo_environment(root)
    output_dir.mkdir(parents=True, exist_ok=True)
    scenarios = []
    for target in TARGETS:
        fixture_path = output_dir / f"authored-{target}.noisette"
        metadata = write_fixture(source_path, fixture_path, minimum_live_nodes=target)
        fixture_sha = sha256_bytes(fixture_path.read_bytes())
        scenario_env = environment.copy()
        scenario_env.update({
            "CHATAIGNE_AUTHORED_SCALE_FIXTURE": str(fixture_path),
            "CHATAIGNE_AUTHORED_SCALE_MIN_NODES": str(target),
            "CHATAIGNE_AUTHORED_SCALE_GRAPH_ROOTS": str(metadata["graphNodeCount"]),
        })
        result = subprocess.run(
            TEST_COMMAND, cwd=root, env=scenario_env, capture_output=True,
            check=False, text=True, encoding="utf-8",
        )
        output = result.stdout + result.stderr
        log_path = output_dir / f"authored-{target}.log"
        log_bytes = output.encode("utf-8")
        log_path.write_bytes(log_bytes)
        parse_error = None
        measured = None
        try:
            measured = parse_result(output, target, metadata["graphNodeCount"])
        except ValueError as error:
            parse_error = str(error)
        scenarios.append({
            "target": target,
            "status": "PASS" if result.returncode == 0 and parse_error is None else "FAIL",
            "exit_code": result.returncode,
            "fixture": {
                **metadata,
                "path": fixture_path.relative_to(root).as_posix(),
                "sha256": fixture_sha,
            },
            "log": {
                "path": log_path.relative_to(root).as_posix(),
                "sha256": sha256_bytes(log_bytes),
            },
            "measured_result": measured,
            "parse_error": parse_error,
            "startup_tick_deadline_exceeded": measured is not None and measured["tick_us"][0] > 8_000,
            "warmed_tick_deadline_exceeded": measured is not None
            and any(value > 8_000 for value in measured["tick_us"][1:]),
        })
        print(f"authored graph {target}: {scenarios[-1]['status']} ({log_path})", flush=True)
    if working_tree_sha(root) != tested_tree_sha:
        raise ValueError("source tree changed during authored graph qualification")
    return {
        "schema_version": 2,
        "evidence_id": EVIDENCE_ID,
        "status": "PASS" if all(row["status"] == "PASS" for row in scenarios) else "FAIL",
        "commit_sha": commit_sha,
        "tested_tree_sha": tested_tree_sha,
        "command": list(TEST_COMMAND),
        "features": {"default": manifest["features"]["default"], "ui_assets_skipped": True},
        "profile": "optimized-test",
        "source_fixture": {"path": DEFAULT_SOURCE.as_posix(), "sha256": source_sha},
        "toolchain_fingerprint": toolchain,
        "started_at": started_at,
        "finished_at": utc_now(),
        "scenarios": scenarios,
        "scope": "persisted Chataigne full-workbench load, five engine ticks, sparse save and reload",
        "not_covered": [
            "tick tail distribution, interaction, UI paint, transport and multi-client behavior",
            "graph compilation and evaluation across all cloned Constant ANodes",
            "edit and undo/redo at these scales",
        ],
    }


def main(arguments: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path.cwd())
    parser.add_argument("--output-dir", type=Path)
    options = parser.parse_args(arguments)
    root = options.root.resolve()
    try:
        output_dir = resolve_output_dir(root, options.output_dir)
        report = build_report(root, output_dir)
    except (OSError, ValueError) as error:
        print(f"Authored graph qualification error: {error}", file=sys.stderr)
        return 2
    report_path = output_dir / "authored-graph-scale-report.json"
    report_path.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(f"Authored graph report: {report_path.relative_to(root).as_posix()}")
    return 0 if report["status"] == "PASS" else 1


if __name__ == "__main__":
    raise SystemExit(main())
