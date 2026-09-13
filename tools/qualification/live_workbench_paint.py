"""Measure authored graph batch edits through the bundled headless product and browser."""

from __future__ import annotations

import argparse
import json
import math
import os
import platform
import shutil
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


CONTRACT = "chataigne-live-workbench-paint-v1"
TARGETS = (10_000, 100_000)
BUILD_COMMAND = (
    "cargo", "build", "--locked", "-p", "Chataigne2", "--bin", "Chataigne2",
    "--target-dir", "target/t16-app-default",
)
PROBE_SCRIPT = Path("tools/qualification/live_workbench_paint.mjs")


def percentile(values: list[float], quantile: float) -> float:
    ordered = sorted(values)
    return ordered[min(len(ordered) - 1, math.ceil(len(ordered) * quantile) - 1)]


def parse_probe_report(
    report: dict[str, Any], target: int, samples: int, binary_sha: str, fixture_sha: str,
) -> str:
    if report.get("contract") != CONTRACT or report.get("minimum_nodes") != target:
        raise ValueError("live workbench report contract or fixture target differs")
    if report.get("samples") != samples or report.get("binary_sha256") != binary_sha \
            or report.get("fixture_sha256") != fixture_sha:
        raise ValueError("live workbench report fingerprint or sample count differs")
    if type(report.get("base_nodes")) is not int or report["base_nodes"] < target:
        raise ValueError("live workbench never reached the authored node target")
    if type(report.get("duplicated_roots_per_sample")) is not int \
            or report["duplicated_roots_per_sample"] < 1 \
            or type(report.get("inserted_nodes_per_sample")) is not int \
            or report["inserted_nodes_per_sample"] < 600:
        raise ValueError("live workbench did not perform a 600-node batch")
    for field in ("latencies_ms", "http_ack_ms", "mutation_ms"):
        values = report.get(field)
        if not isinstance(values, list) or len(values) != samples \
                or any(type(value) not in (float, int) or not math.isfinite(value) or value < 0
                       for value in values):
            raise ValueError(f"live workbench has incomplete {field}")
    latencies = report["latencies_ms"]
    for field, quantile in (("p50_ms", 0.5), ("p95_ms", 0.95), ("p99_ms", 0.99)):
        if report.get(field) != percentile(latencies, quantile):
            raise ValueError(f"live workbench {field} does not match raw samples")
    if report.get("max_ms") != max(latencies):
        raise ValueError("live workbench max does not match raw samples")
    target_ms = 250 if target == 10_000 else 500
    if report.get("p95_target_ms") != target_ms:
        raise ValueError("live workbench silently changed the provisional p95 budget")
    if not isinstance(report.get("long_tasks"), list) or not isinstance(report.get("browser_errors"), list) \
            or not isinstance(report.get("browser_perf_logs"), list):
        raise ValueError("live workbench browser diagnostics are incomplete")
    if report["browser_errors"]:
        raise ValueError("live workbench recorded browser errors")
    transport = report.get("transport")
    if not isinstance(transport, dict) or set(transport) != {
        "slow_client_disconnects", "overflow_resyncs", "ws_snapshots",
    } or any(type(value) is not int or value < 0 for value in transport.values()):
        raise ValueError("live workbench transport recovery counters are incomplete")
    if transport["slow_client_disconnects"] != 0:
        raise ValueError("live workbench still disconnected a slow browser client")
    budget_passed = report["p95_ms"] <= target_ms
    if budget_passed and report.get("status") == "PASS" and report.get("error") is None:
        return "PASS"
    if not budget_passed and report.get("status") == "FAIL" \
            and "p95 exceeded the provisional workbench budget" in str(report.get("error")):
        return "BUDGET_FAIL"
    raise ValueError("live workbench report status does not match its measurements")


def resolve_output_dir(root: Path, value: Path | None) -> Path:
    target_root = (root / "target").resolve()
    output = (
        value.resolve() if value is not None and value.is_absolute()
        else (root / value).resolve() if value is not None
        else target_root / "qualification" / "live-workbench-paint"
        / datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    )
    if not output.is_relative_to(target_root) or output == target_root:
        raise ValueError("live workbench output must be a child of the workspace target directory")
    if output.exists() and any(output.iterdir()):
        raise ValueError("live workbench output directory already contains artifacts")
    return output


def build_report(root: Path, output_dir: Path, samples: int) -> dict[str, Any]:
    started_at = utc_now()
    tested_tree_sha = working_tree_sha(root)
    commit_sha = command_output(root, ("git", "rev-parse", "HEAD"))
    node = shutil.which("node")
    manifest = tomllib.loads((root / "apps/chataigne/Cargo.toml").read_text(encoding="utf-8"))
    source_sha = sha256_bytes((root / DEFAULT_SOURCE).read_bytes())
    environment = cargo_environment(root)
    environment.pop("GC_SKIP_UI_BUILD", None)
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
        fixture = output_dir / f"authored-{target}.noisette"
        metadata = write_fixture(root / DEFAULT_SOURCE, fixture, minimum_live_nodes=target)
        fixture_sha = sha256_bytes(fixture.read_bytes())
        probe_dir = output_dir / f"workbench-{target}"
        command = (node or "node", str(root / PROBE_SCRIPT), str(binary), str(fixture), str(target),
                   str(probe_dir), str(samples))
        process = None
        if binary_sha is not None and node is not None:
            process = subprocess.run(
                command, cwd=root, env=environment, capture_output=True, check=False,
                text=True, encoding="utf-8",
            )
        output = "" if process is None else process.stdout + process.stderr
        log = output_dir / f"workbench-{target}.log"
        log.write_text(output, encoding="utf-8")
        probe_report_path = probe_dir / "workbench-paint-report.json"
        measured = None
        parse_error = None
        status = "MISSING"
        if probe_report_path.is_file() and binary_sha is not None:
            try:
                measured = json.loads(probe_report_path.read_text(encoding="utf-8"))
                status = parse_probe_report(measured, target, samples, binary_sha, fixture_sha)
            except (ValueError, json.JSONDecodeError) as error:
                parse_error = str(error)
        else:
            parse_error = "probe report missing or product build failed"
        if process is not None and ((status == "PASS" and process.returncode != 0)
                                    or (status == "BUDGET_FAIL" and process.returncode == 0)):
            status = "MISSING"
            parse_error = "probe exit code disagrees with validated report"
        scenarios.append({
            "target": target, "status": status, "command": list(command),
            "exit_code": None if process is None else process.returncode,
            "fixture": {**metadata, "sha256": fixture_sha},
            "log": {"path": log.relative_to(root).as_posix(), "sha256": sha256_bytes(output.encode())},
            "probe_report": probe_report_path.relative_to(root).as_posix() if probe_report_path.is_file() else None,
            "measured_result": measured, "parse_error": parse_error,
        })
        print(f"live workbench {target}: {status} ({log})", flush=True)

    if working_tree_sha(root) != tested_tree_sha:
        raise ValueError("source tree changed during live workbench qualification")
    return {
        "schema_version": 1,
        "evidence_id": "product.live-workbench-paint.local",
        "status": "PASS" if build.returncode == 0 and all(row["status"] == "PASS" for row in scenarios) else "FAIL",
        "product_qualification": "OPEN",
        "commit_sha": commit_sha, "tested_tree_sha": tested_tree_sha,
        "artifact": {"path": binary.relative_to(root).as_posix(), "sha256": binary_sha},
        "build": {"command": list(BUILD_COMMAND), "exit_code": build.returncode,
                  "log": build_log.relative_to(root).as_posix()},
        "features": {"default": manifest["features"]["default"], "ui_assets_skipped": False},
        "toolchain_fingerprint": {
            "rustc": command_output(root, ("rustc", "-Vv")),
            "cargo": command_output(root, ("cargo", "-V")),
            "node": command_output(root, ("node", "--version")) if node else None,
            "os": platform.platform(),
        },
        "source_fixture": {"path": DEFAULT_SOURCE.as_posix(), "sha256": source_sha},
        "started_at": started_at, "finished_at": utc_now(),
        "scenarios": scenarios,
        "scope": "one real HTTP batch duplicate of at least 600 nodes per sample; bundled-UI headless app, "
                 "workbench WebSocket projection, DOM mutation, and two browser frame callbacks",
        "not_covered": ["direct pixel-difference proof", "native webview and physical devices"],
    }


def main(arguments: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path.cwd())
    parser.add_argument("--output-dir", type=Path)
    parser.add_argument("--samples", type=int, default=20)
    args = parser.parse_args(arguments)
    if args.samples < 1:
        parser.error("--samples must be positive")
    root = args.root.resolve()
    output_dir = resolve_output_dir(root, args.output_dir)
    report = build_report(root, output_dir, args.samples)
    report_path = output_dir / "live-workbench-paint-report.json"
    report_path.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(f"Live workbench report: {report_path.relative_to(root)}", flush=True)
    return 0 if report["status"] == "PASS" else 1


if __name__ == "__main__":
    sys.exit(main())
