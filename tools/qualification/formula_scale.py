#!/usr/bin/env python3
"""Source-fingerprinted qualification of direct product-Formula lane evaluation."""

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
from collections.abc import Sequence
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

if __package__:
    from .runtime_scale import command_output, sha256_bytes, utc_now, working_tree_sha
else:
    from runtime_scale import command_output, sha256_bytes, utc_now, working_tree_sha


EVIDENCE_ID = "formula.direct-100k.local"
FIXTURE = Path("apps/chataigne/test-samples/test_multiplex.noisette")
PARTITIONS = {(1_000, 100), (10_000, 10)}
WORKERS = (1, 2, 4, 8)
TEST_COUNT = 8
TEST_COMMAND = (
    "cargo",
    "test",
    "--locked",
    "-q",
    "-p",
    "Chataigne2",
    "--bin",
    "Chataigne2",
    "--features",
    "kernel-profiling",
    "--target-dir",
    "target/t16-app-default",
    "multiplex_formula_",
    "--",
    "--ignored",
    "--nocapture",
    "--test-threads=1",
)
PREFIXES = {
    "serial": "formula scale:",
    "workers": "formula workers:",
    "unchanged": "formula unchanged:",
}
REQUIRED_FIELDS = {
    "serial": {
        "processors", "lanes_per_processor", "formula", "exec_nodes", "build_ms",
        "tick_us", "kernel_ms", "kernel_evaluations", "intents", "diagnostics",
        "rss_before_mb", "rss_built_mb", "rss_evaluated_mb",
    },
    "workers": {
        "processors", "lanes_per_processor", "reorder_contexts", "workers", "tick_us",
        "process_cpu_ms", "kernel_thread_ms", "kernel_evaluations", "ordered_effects",
        "rss_before_mb", "rss_after_mb",
    },
    "unchanged": {
        "processors", "lanes_per_processor", "tick_us", "process_cpu_ms",
        "kernel_thread_ms", "kernel_evaluations", "intents_per_tick", "rss_mb",
    },
}
FIELD_PATTERN = re.compile(r"([a-z_]+)=(\[[^\]]*\]|[^\s]+)")
TEST_RESULT_PATTERN = re.compile(r"test result: ok\. (\d+) passed; 0 failed;")
LIST_FIELDS = {"tick_us", "process_cpu_ms", "intents_per_tick"}
TEXT_FIELDS = {"formula"}
BOOL_FIELDS = {"reorder_contexts"}


def parse_fields(kind: str, line: str) -> dict[str, Any]:
    pairs = FIELD_PATTERN.findall(line)
    fields: dict[str, Any] = {}
    for key, raw in pairs:
        if key in fields:
            raise ValueError(f"duplicate {kind} field: {key}")
        try:
            fields[key] = json.loads(raw)
        except json.JSONDecodeError:
            fields[key] = raw
    missing = REQUIRED_FIELDS[kind] - fields.keys()
    extra = fields.keys() - REQUIRED_FIELDS[kind]
    if missing or extra:
        raise ValueError(f"{kind} fields differ: missing={sorted(missing)}, extra={sorted(extra)}")
    for key, value in fields.items():
        if key in LIST_FIELDS or key in TEXT_FIELDS or key in BOOL_FIELDS:
            continue
        if type(value) is not int or value < 0:
            raise ValueError(f"{kind} field {key} must be a nonnegative integer")
    partition = (fields["processors"], fields["lanes_per_processor"])
    if partition not in PARTITIONS:
        raise ValueError(f"unexpected {kind} partition: {partition}")
    if not isinstance(fields["tick_us"], list) or len(fields["tick_us"]) != 3:
        raise ValueError(f"{kind} must contain exactly three warmed tick measurements")
    if any(type(value) is not int or value <= 0 for value in fields["tick_us"]):
        raise ValueError(f"{kind} tick measurements must be positive integers")
    if fields["kernel_evaluations"] != 300_000:
        raise ValueError(f"{kind} did not evaluate all 300,000 warmed lanes")
    if kind == "serial" and (
        fields["formula"] != "Action"
        or fields["diagnostics"] != 0
        or fields["exec_nodes"] != 3
        or fields["intents"] == 0
    ):
        raise ValueError("serial product Formula has diagnostics or an unexpected graph")
    if kind == "workers":
        if (
            type(fields["workers"]) is not int
            or fields["workers"] not in WORKERS
            or type(fields["reorder_contexts"]) is not bool
        ):
            raise ValueError("worker case has an invalid count or reorder mode")
        if not isinstance(fields["process_cpu_ms"], list) or len(fields["process_cpu_ms"]) != 3:
            raise ValueError("worker case must contain three CPU measurements")
        if fields["ordered_effects"] != 200_000:
            raise ValueError("worker case has an unexpected ordered effect count")
    if kind == "unchanged":
        if not isinstance(fields["process_cpu_ms"], list) or len(fields["process_cpu_ms"]) != 3:
            raise ValueError("unchanged case must contain three CPU measurements")
        if fields["intents_per_tick"] != [200_000, 0, 0, 0]:
            raise ValueError("unchanged case did not suppress post-initialization intents")
    if "process_cpu_ms" in fields and any(
        type(value) is not int or value < 0 for value in fields["process_cpu_ms"]
    ):
        raise ValueError(f"{kind} CPU measurements must be nonnegative integers")
    return fields


def parse_results(output: str) -> dict[str, list[dict[str, Any]]]:
    rows: dict[str, list[dict[str, Any]]] = {kind: [] for kind in PREFIXES}
    for line in output.splitlines():
        for kind, prefix in PREFIXES.items():
            if prefix in line:
                rows[kind].append(parse_fields(kind, line.split(prefix, 1)[1]))
    if not any(int(count) == TEST_COUNT for count in TEST_RESULT_PATTERN.findall(output)):
        raise ValueError(f"app test output must confirm exactly {TEST_COUNT} passing Formula tests")
    expected_serial = PARTITIONS
    actual_serial = [(row["processors"], row["lanes_per_processor"]) for row in rows["serial"]]
    if len(actual_serial) != len(expected_serial) or set(actual_serial) != expected_serial:
        raise ValueError(f"missing or duplicate serial Formula partitions: {actual_serial}")
    actual_unchanged = [
        (row["processors"], row["lanes_per_processor"])
        for row in rows["unchanged"]
    ]
    if len(actual_unchanged) != len(PARTITIONS) or set(actual_unchanged) != PARTITIONS:
        raise ValueError(f"missing or duplicate unchanged Formula partitions: {actual_unchanged}")
    expected_workers = {
        (processors, lanes, workers, reorder)
        for processors, lanes in PARTITIONS
        for workers in WORKERS
        for reorder in (False, True)
    }
    actual_workers = [
        (row["processors"], row["lanes_per_processor"], row["workers"], row["reorder_contexts"])
        for row in rows["workers"]
    ]
    if len(actual_workers) != len(expected_workers) or set(actual_workers) != expected_workers:
        raise ValueError(f"missing or duplicate Formula worker cases: {actual_workers}")
    for kind in rows:
        rows[kind].sort(key=lambda row: (
            row["processors"], row.get("reorder_contexts", False), row.get("workers", 0)
        ))
    return rows


def resolve_output_dir(root: Path, value: Path | None) -> Path:
    target_root = (root / "target").resolve()
    output_dir = (
        value.resolve() if value is not None and value.is_absolute()
        else (root / value).resolve() if value is not None
        else target_root / "qualification" / "formula-scale"
        / datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    )
    if not output_dir.is_relative_to(target_root) or output_dir == target_root:
        raise ValueError("formula scale output must be a child of the workspace target directory")
    if output_dir.exists() and any(output_dir.iterdir()):
        raise ValueError("formula scale output directory already contains artifacts")
    return output_dir


def cargo_environment(root: Path) -> dict[str, str]:
    environment = os.environ.copy()
    environment["GC_SKIP_UI_BUILD"] = "1"
    if os.name != "nt":
        return environment
    powershell = shutil.which("pwsh") or shutil.which("powershell")
    if powershell is None:
        raise ValueError("PowerShell is required to prepare the pinned Windows ASIO SDK")
    setup = subprocess.run(
        (powershell, "-NoProfile", "-File", str(root / "tools/asio.ps1"), "-SetupOnly"),
        cwd=root,
        capture_output=True,
        check=False,
        text=True,
        encoding="utf-8",
    )
    if setup.returncode != 0:
        raise ValueError(f"ASIO setup failed: {setup.stdout}{setup.stderr}")
    for key in ("CPAL_ASIO_DIR", "LIBCLANG_PATH"):
        values = [
            line.split("=", 1)[1].strip()
            for line in setup.stdout.splitlines()
            if line.startswith(f"{key}=")
        ]
        if len(values) != 1 or not values[0]:
            raise ValueError(f"ASIO setup did not report one {key} value")
        environment[key] = values[0]
    return environment


def build_report(root: Path, output_dir: Path) -> tuple[dict[str, Any], str]:
    started_at = utc_now()
    tested_tree_sha = working_tree_sha(root)
    commit_sha = command_output(root, ("git", "rev-parse", "HEAD"))
    rustc = command_output(root, ("rustc", "-Vv"))
    cargo = command_output(root, ("cargo", "-V"))
    manifest = tomllib.loads((root / "apps/chataigne/Cargo.toml").read_text(encoding="utf-8"))
    fixture_bytes = (root / FIXTURE).read_bytes()
    environment = cargo_environment(root)
    result = subprocess.run(
        TEST_COMMAND,
        cwd=root,
        env=environment,
        capture_output=True,
        check=False,
        text=True,
        encoding="utf-8",
    )
    output = result.stdout + result.stderr
    parse_error = None
    measured: dict[str, list[dict[str, Any]]] = {}
    try:
        measured = parse_results(output)
    except ValueError as error:
        parse_error = str(error)
    output_dir.mkdir(parents=True, exist_ok=True)
    log_path = output_dir / "cargo-test.log"
    log_bytes = output.encode("utf-8")
    log_path.write_bytes(log_bytes)
    report = {
        "schema_version": 1,
        "evidence_id": EVIDENCE_ID,
        "status": "PASS" if result.returncode == 0 and parse_error is None else "FAIL",
        "commit_sha": commit_sha,
        "tested_tree_sha": tested_tree_sha,
        "command": list(TEST_COMMAND),
        "features": {
            "default": manifest["features"]["default"],
            "additional": ["kernel-profiling"],
            "ui_assets_skipped": True,
        },
        "profile": "optimized-test",
        "fixture": {"path": FIXTURE.as_posix(), "sha256": sha256_bytes(fixture_bytes)},
        "toolchain_fingerprint": {"rustc": rustc, "cargo": cargo, "os": platform.platform()},
        "started_at": started_at,
        "finished_at": utc_now(),
        "exit_code": result.returncode,
        "artifact_id": log_path.relative_to(root).as_posix(),
        "artifact_hash": sha256_bytes(log_bytes),
        "measured_result": measured,
        "parse_error": parse_error,
        "scope": (
            "direct processor evaluation only; no engine tick, output dispatch, "
            "UI, or transport"
        ),
        "not_covered": [
            "100k-lane end-to-end product tick and latency",
            "sparse-dirty product crossover",
            "production worker generation/cancellation commit boundary",
        ],
    }
    return report, output


def main(arguments: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path.cwd())
    parser.add_argument("--output-dir", type=Path)
    options = parser.parse_args(arguments)
    root = options.root.resolve()
    try:
        output_dir = resolve_output_dir(root, options.output_dir)
        report, output = build_report(root, output_dir)
    except (OSError, ValueError) as error:
        print(f"Formula scale qualification error: {error}", file=sys.stderr)
        return 2
    report_path = output_dir / "formula-scale-report.json"
    report_path.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(output, end="" if output.endswith("\n") else "\n")
    print(f"Formula scale report: {report_path.relative_to(root).as_posix()}")
    return 0 if report["status"] == "PASS" else 1


if __name__ == "__main__":
    raise SystemExit(main())
