from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import subprocess
import sys
import tempfile
from collections.abc import Sequence
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


EVIDENCE_ID = "runtime.scalar-100k.local"
RESULT_PREFIX = "RUNTIME_SCALE_RESULT="
EXPECTED_PARTITIONS = {"p1000-l100", "p10000-l10"}
TEST_COMMAND = (
    "cargo",
    "test",
    "-p",
    "golden_runtime",
    "--release",
    "--test",
    "scalar_scale",
    "runtime_100k_scalar_dense_sparse_idle_qualification",
    "--",
    "--ignored",
    "--nocapture",
)


def utc_now() -> str:
    return datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def parse_results(output: str) -> list[dict[str, Any]]:
    results: list[dict[str, Any]] = []
    for line in output.splitlines():
        if RESULT_PREFIX not in line:
            continue
        payload = line.split(RESULT_PREFIX, 1)[1].strip()
        try:
            result = json.loads(payload)
        except json.JSONDecodeError as error:
            raise ValueError(f"invalid runtime scale result JSON: {error}") from error
        if not isinstance(result, dict):
            raise ValueError("runtime scale result must be a JSON object")
        results.append(result)
    partitions = [result.get("partition") for result in results]
    if set(partitions) != EXPECTED_PARTITIONS or len(partitions) != len(
        EXPECTED_PARTITIONS
    ):
        raise ValueError(
            "runtime scale output must contain exactly one result for each partition; "
            f"found={partitions}"
        )
    return sorted(results, key=lambda result: str(result["partition"]))


def command_output(root: Path, command: Sequence[str]) -> str:
    result = subprocess.run(
        command,
        cwd=root,
        capture_output=True,
        check=False,
        text=True,
        encoding="utf-8",
    )
    if result.returncode != 0:
        detail = result.stderr.strip() or result.stdout.strip() or "command failed"
        raise ValueError(f"{' '.join(command)}: {detail}")
    return result.stdout.strip()


def working_tree_sha(root: Path) -> str:
    with tempfile.TemporaryDirectory(prefix="chataigne-scale-index-") as directory:
        index_path = Path(directory) / "index"
        environment = os.environ.copy()
        environment["GIT_INDEX_FILE"] = str(index_path)
        for command in (("git", "read-tree", "HEAD"), ("git", "add", "-A", "--", ".")):
            result = subprocess.run(
                command,
                cwd=root,
                env=environment,
                capture_output=True,
                check=False,
                text=True,
                encoding="utf-8",
            )
            if result.returncode != 0:
                detail = result.stderr.strip() or "git working-tree capture failed"
                raise ValueError(f"{' '.join(command)}: {detail}")
        result = subprocess.run(
            ("git", "write-tree"),
            cwd=root,
            env=environment,
            capture_output=True,
            check=False,
            text=True,
            encoding="utf-8",
        )
        if result.returncode != 0:
            detail = result.stderr.strip() or "git write-tree failed"
            raise ValueError(f"git write-tree: {detail}")
        return result.stdout.strip()


def resolve_output_dir(root: Path, value: Path | None) -> Path:
    target_root = (root / "target").resolve()
    output_dir = (
        value.resolve()
        if value is not None and value.is_absolute()
        else (root / value).resolve()
        if value is not None
        else target_root / "qualification" / "runtime-scale" / datetime.now(timezone.utc).strftime(
            "%Y%m%dT%H%M%SZ"
        )
    )
    if not output_dir.is_relative_to(target_root) or output_dir == target_root:
        raise ValueError("runtime scale output must be a child of the workspace target directory")
    return output_dir


def build_report(root: Path, output_dir: Path) -> tuple[dict[str, Any], str]:
    started_at = utc_now()
    tested_tree_sha = working_tree_sha(root)
    commit_sha = command_output(root, ("git", "rev-parse", "HEAD"))
    rustc = command_output(root, ("rustc", "-Vv"))
    cargo = command_output(root, ("cargo", "-V"))
    result = subprocess.run(
        TEST_COMMAND,
        cwd=root,
        capture_output=True,
        check=False,
        text=True,
        encoding="utf-8",
    )
    combined_output = result.stdout + result.stderr
    finished_at = utc_now()
    parse_error = None
    measured_result: list[dict[str, Any]] = []
    try:
        measured_result = parse_results(combined_output)
    except ValueError as error:
        parse_error = str(error)

    output_dir.mkdir(parents=True, exist_ok=True)
    log_path = output_dir / "cargo-test.log"
    log_bytes = combined_output.encode("utf-8")
    log_path.write_bytes(log_bytes)
    log_relative = log_path.relative_to(root).as_posix()
    status = "PASS" if result.returncode == 0 and parse_error is None else "FAIL"
    report = {
        "schema_version": 1,
        "evidence_id": EVIDENCE_ID,
        "status": status,
        "commit_sha": commit_sha,
        "tested_tree_sha": tested_tree_sha,
        "command": " ".join(TEST_COMMAND),
        "toolchain_fingerprint": {
            "rustc": rustc,
            "cargo": cargo,
            "os": platform.platform(),
        },
        "started_at": started_at,
        "finished_at": finished_at,
        "exit_code": result.returncode,
        "ignored_or_skipped": [],
        "artifact_id": log_relative,
        "artifact_hash": sha256_bytes(log_bytes),
        "measured_result": {
            "partitions": measured_result,
            "thresholds_us": {
                "dense_p95": 8_000,
                "dense_p99": 12_000,
                "sparse_p95": 2_000,
                "idle_p95": 500,
                "deadline_us": 16_670,
                "deadline_miss_rate_strictly_below": 0.001,
            },
            "determinism_workers": [1, 2, 4, 8],
            "bounded_output_reuse": True,
            "parse_error": parse_error,
        },
    }
    return report, combined_output


def main(arguments: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Run and record the 100,000-scalar runtime qualification."
    )
    parser.add_argument("--root", type=Path, default=Path.cwd())
    parser.add_argument("--output-dir", type=Path)
    options = parser.parse_args(arguments)
    root = options.root.resolve()
    try:
        output_dir = resolve_output_dir(root, options.output_dir)
        report, combined_output = build_report(root, output_dir)
    except ValueError as error:
        print(f"Runtime scale qualification error: {error}", file=sys.stderr)
        return 2

    report_path = output_dir / "runtime-scale-report.json"
    report_path.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(combined_output, end="" if combined_output.endswith("\n") else "\n")
    print(f"Runtime scale report: {report_path.relative_to(root).as_posix()}")
    return 0 if report["status"] == "PASS" else 1


if __name__ == "__main__":
    raise SystemExit(main())
