"""Qualify three real UI WebSocket clients against authored Chataigne projects."""

from __future__ import annotations

import argparse
import json
import os
import platform
import re
import shutil
import subprocess
import sys
import tomllib
import uuid
from collections.abc import Sequence
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

if __package__:
    from .authored_graph_scale import TARGETS
    from .formula_scale import cargo_environment
    from .graph_fixture import DEFAULT_SOURCE, write_fixture
    from .runtime_scale import command_output, sha256_bytes, utc_now, working_tree_sha
else:
    from authored_graph_scale import TARGETS
    from formula_scale import cargo_environment
    from graph_fixture import DEFAULT_SOURCE, write_fixture
    from runtime_scale import command_output, sha256_bytes, utc_now, working_tree_sha


RESULT_PREFIX = "PRODUCT_TRANSPORT_RESULT="
CONTRACT = "chataigne-product-transport-probe-v2"
BUILD_COMMAND = (
    "cargo", "build", "--locked", "-q", "-p", "Chataigne2", "--bin", "Chataigne2",
    "--target-dir", "target/t16-app-default",
)
PROBE_SCRIPT = Path("tools/qualification/transport_probe.mjs")
RESULT_FIELDS = {
    "contract", "status", "minimum_live_nodes", "graph_roots", "load_ms",
    "client_snapshots", "resync_reasons", "reconnect_snapshot",
    "subscribed_clients_after_reconnect", "session_consistent",
    "edited_param_uuid", "edited_value_delta_clients", "edited_value_snapshot_clients",
    "intent_applied", "reconnect_edited_value",
}
SNAPSHOT_FIELDS = {"nodes", "roots", "node_identity_sha256"}


def parse_probe_result(output: str, exit_code: int, target: int, graph_roots: int) -> dict[str, Any]:
    rows = []
    for line in output.splitlines():
        if RESULT_PREFIX in line:
            try:
                rows.append(json.loads(line.split(RESULT_PREFIX, 1)[1]))
            except json.JSONDecodeError as error:
                raise ValueError(f"invalid product transport result: {error}") from error
    if exit_code != 0 or len(rows) != 1:
        raise ValueError(f"expected one passing product transport result, found {len(rows)} with exit {exit_code}")
    row = rows[0]
    if not isinstance(row, dict) or row.keys() != RESULT_FIELDS:
        raise ValueError("product transport result fields differ from the qualification contract")
    if row["contract"] != CONTRACT or row["status"] != "PASS":
        raise ValueError("product transport probe did not pass its declared contract")
    if row["session_consistent"] is not True:
        raise ValueError("product transport clients did not retain one runtime session")
    if row["intent_applied"] is not True or row["reconnect_edited_value"] is not True:
        raise ValueError("product transport edit did not apply and survive reconnect")
    try:
        if not isinstance(row["edited_param_uuid"], str):
            raise ValueError("edited parameter UUID is not a string")
        uuid.UUID(row["edited_param_uuid"])
    except ValueError as error:
        raise ValueError("product transport edited parameter UUID is invalid") from error
    for field in (
        "minimum_live_nodes", "graph_roots", "load_ms", "subscribed_clients_after_reconnect",
        "edited_value_delta_clients", "edited_value_snapshot_clients",
    ):
        if type(row[field]) is not int or row[field] < 0:
            raise ValueError(f"product transport {field} must be a nonnegative integer")
    if row["minimum_live_nodes"] != target or row["graph_roots"] != graph_roots:
        raise ValueError("product transport result does not match the generated fixture")
    if row["subscribed_clients_after_reconnect"] != 3:
        raise ValueError("product transport did not recover three subscribed clients")
    if row["edited_value_delta_clients"] != 3 or row["edited_value_snapshot_clients"] != 3:
        raise ValueError("product transport edit did not reach all three clients")
    snapshots = row["client_snapshots"]
    if not isinstance(snapshots, list) or len(snapshots) != 3:
        raise ValueError("product transport requires three client snapshots")
    for snapshot in [*snapshots, row["reconnect_snapshot"]]:
        if not isinstance(snapshot, dict) or snapshot.keys() != SNAPSHOT_FIELDS:
            raise ValueError("product transport snapshot fields differ from the qualification contract")
        if type(snapshot["nodes"]) is not int or snapshot["nodes"] < target:
            raise ValueError("product transport snapshot missed the authored-node target")
        if type(snapshot["roots"]) is not int or snapshot["roots"] != graph_roots:
            raise ValueError("product transport snapshot missed the authored graph roots")
        if not isinstance(snapshot["node_identity_sha256"], str) or re.fullmatch(
            r"[0-9a-f]{64}", snapshot["node_identity_sha256"]
        ) is None:
            raise ValueError("product transport snapshot has no valid node-identity digest")
    if len({snapshot["node_identity_sha256"] for snapshot in [*snapshots, row["reconnect_snapshot"]]}) != 1:
        raise ValueError("product transport client snapshots contain different node identities")
    if row["resync_reasons"] != ["project_loaded"] * 3:
        raise ValueError("product transport did not deliver project replacement resync to every client")
    return row


def resolve_output_dir(root: Path, value: Path | None) -> Path:
    target_root = (root / "target").resolve()
    output = (
        value.resolve() if value is not None and value.is_absolute()
        else (root / value).resolve() if value is not None
        else target_root / "qualification" / "transport-scale"
        / datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    )
    if not output.is_relative_to(target_root) or output == target_root:
        raise ValueError("transport scale output must be a child of the workspace target directory")
    if output.exists() and any(output.iterdir()):
        raise ValueError("transport scale output directory already contains artifacts")
    return output


def build_report(root: Path, output_dir: Path) -> dict[str, Any]:
    started_at = utc_now()
    tested_tree_sha = working_tree_sha(root)
    commit_sha = command_output(root, ("git", "rev-parse", "HEAD"))
    node = shutil.which("node")
    toolchain = {
        "rustc": command_output(root, ("rustc", "-Vv")),
        "cargo": command_output(root, ("cargo", "-V")),
        "node": command_output(root, ("node", "--version")) if node is not None else None,
        "os": platform.platform(),
    }
    manifest = tomllib.loads((root / "apps/chataigne/Cargo.toml").read_text(encoding="utf-8"))
    source_path = root / DEFAULT_SOURCE
    source_sha = sha256_bytes(source_path.read_bytes())
    environment = cargo_environment(root)
    output_dir.mkdir(parents=True, exist_ok=True)

    build = subprocess.run(
        BUILD_COMMAND, cwd=root, env=environment, capture_output=True,
        check=False, text=True, encoding="utf-8",
    )
    build_log = output_dir / "cargo-build.log"
    build_log.write_text(build.stdout + build.stderr, encoding="utf-8")
    binary = root / "target/t16-app-default/debug" / ("Chataigne2.exe" if os.name == "nt" else "Chataigne2")
    binary_sha = sha256_bytes(binary.read_bytes()) if build.returncode == 0 and binary.is_file() else None

    scenarios = []
    for target in TARGETS:
        fixture_path = output_dir / f"authored-{target}.noisette"
        metadata = write_fixture(source_path, fixture_path, minimum_live_nodes=target)
        fixture_sha = sha256_bytes(fixture_path.read_bytes())
        probe_dir = output_dir / f"transport-{target}"
        command = (
            node or "node", str(root / PROBE_SCRIPT), str(binary), str(fixture_path),
            str(target), str(metadata["graphNodeCount"]), str(probe_dir),
        )
        result = None
        if build.returncode == 0 and binary_sha is not None and node is not None:
            result = subprocess.run(
                command, cwd=root, env=environment, capture_output=True,
                check=False, text=True, encoding="utf-8",
            )
        output = "" if result is None else result.stdout + result.stderr
        log_path = output_dir / f"transport-{target}.log"
        log_path.write_text(output, encoding="utf-8")
        measured = None
        parse_error = None
        if result is None:
            parse_error = (
                "product binary build failed" if build.returncode != 0
                else "product binary is missing" if binary_sha is None
                else "Node.js is missing"
            )
        else:
            try:
                measured = parse_probe_result(output, result.returncode, target, metadata["graphNodeCount"])
            except ValueError as error:
                parse_error = str(error)
        scenarios.append({
            "target": target,
            "status": "PASS" if measured is not None else "FAIL",
            "command": list(command),
            "exit_code": None if result is None else result.returncode,
            "fixture": {**metadata, "sha256": fixture_sha},
            "log": {"path": log_path.relative_to(root).as_posix(), "sha256": sha256_bytes(output.encode())},
            "server_log": (
                {"path": (probe_dir / "headless-server.log").relative_to(root).as_posix(),
                 "sha256": sha256_bytes((probe_dir / "headless-server.log").read_bytes())}
                if (probe_dir / "headless-server.log").is_file() else None
            ),
            "measured_result": measured,
            "parse_error": parse_error,
        })
        print(f"product transport {target}: {scenarios[-1]['status']} ({log_path})", flush=True)

    if working_tree_sha(root) != tested_tree_sha:
        raise ValueError("source tree changed during product transport qualification")
    return {
        "schema_version": 2,
        "evidence_id": "product.transport-scale.local",
        "status": "PASS" if build.returncode == 0 and all(row["status"] == "PASS" for row in scenarios) else "FAIL",
        "product_qualification": "OPEN",
        "commit_sha": commit_sha,
        "tested_tree_sha": tested_tree_sha,
        "artifact": {"path": binary.relative_to(root).as_posix(), "sha256": binary_sha},
        "build": {"command": list(BUILD_COMMAND), "exit_code": build.returncode,
                  "log": build_log.relative_to(root).as_posix()},
        "features": {"default": manifest["features"]["default"], "ui_assets_skipped": True},
        "toolchain_fingerprint": toolchain,
        "source_fixture": {"path": DEFAULT_SOURCE.as_posix(), "sha256": source_sha},
        "started_at": started_at,
        "finished_at": utc_now(),
        "scenarios": scenarios,
        "scope": (
            "headless Chataigne product project load, three live workbench WebSockets, "
            "project-replacement resync, concurrent full snapshots, one client intent edit "
            "delivered to all clients, and one reconnect preserving the edit"
        ),
        "not_covered": [
            "browser rendering, action-to-paint, and UI long tasks",
            "edits during saves, slow-client recovery, and multi-client endurance",
            "desktop native surfaces, physical devices, and cross-platform packaged artifacts",
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
        print(f"Product transport qualification error: {error}", file=sys.stderr)
        return 2
    report_path = output_dir / "transport-scale-report.json"
    report_path.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(f"Product transport report: {report_path.relative_to(root).as_posix()}")
    return 0 if report["status"] == "PASS" else 1


if __name__ == "__main__":
    raise SystemExit(main())
