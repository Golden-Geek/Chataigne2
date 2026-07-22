#!/usr/bin/env python3
"""Run and record the Phase 9 built-in formula catalog qualification."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import shutil
import subprocess
import sys
import tempfile
from collections.abc import Mapping, Sequence
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


EVIDENCE_ID = "phase9.formula.builtin-catalog.local"
TEST_COMMAND = (
    "cargo",
    "test",
    "-p",
    "Chataigne2",
    "app::state_machine_nodes_processor",
)


def utc_now() -> str:
    return datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def canonical_file_sha256(path: Path) -> str:
    content = path.read_bytes()
    try:
        text = content.decode("utf-8")
    except UnicodeDecodeError:
        return sha256_bytes(content)
    canonical = text.replace("\r\n", "\n").replace("\r", "\n").encode("utf-8")
    return sha256_bytes(canonical)


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


def rust_host(rustc: str) -> str:
    return next(
        (line.removeprefix("host: ") for line in rustc.splitlines() if line.startswith("host: ")),
        platform.machine(),
    )


def working_tree_sha(root: Path) -> str:
    with tempfile.TemporaryDirectory(prefix="chataigne-phase9-formula-index-") as directory:
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
        else target_root
        / "phase9"
        / "builtin-formulas"
        / datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    )
    if not output_dir.is_relative_to(target_root) or output_dir == target_root:
        raise ValueError("Phase 9 formula evidence must be a child of the workspace target directory")
    return output_dir


def formula_inventory(root: Path, ledger: Mapping[str, Any]) -> tuple[list[dict[str, str]], list[str]]:
    formulas_dir = root / "apps" / "chataigne" / "builtin_formulas"
    paths = sorted(path for path in formulas_dir.iterdir() if path.is_file() and path.suffix == ".json")
    formula_rows = {
        row.get("discovered_facts", {}).get("file"): row
        for row in ledger.get("rows", [])
        if isinstance(row, Mapping)
        and row.get("discovered_facts", {}).get("inventory_kind") == "formula"
    }
    records: list[dict[str, str]] = []
    errors: list[str] = []
    discovered_paths: set[str] = set()
    for path in paths:
        relative = path.relative_to(root).as_posix()
        discovered_paths.add(relative)
        digest = canonical_file_sha256(path)
        row = formula_rows.get(relative)
        if not isinstance(row, Mapping):
            errors.append(f"{relative} has no generated formula capability row")
            continue
        expected_digest = row.get("discovered_facts", {}).get("sha256")
        if expected_digest != digest:
            errors.append(f"{relative} digest differs from generated discovery")
        records.append(
            {
                "capability_id": str(row.get("capability_id", "")),
                "path": relative,
                "sha256": digest,
            }
        )
    stale_paths = sorted(set(formula_rows) - discovered_paths)
    if stale_paths:
        errors.append(f"generated formula rows have no bundled JSON: {stale_paths}")
    if not records:
        errors.append("no built-in formula JSON files were discovered")
    if any(not record["capability_id"] for record in records):
        errors.append("a generated formula row has no capability ID")
    return records, errors


def build_report(root: Path, output_dir: Path) -> dict[str, Any]:
    started_at = utc_now()
    tested_tree_sha = working_tree_sha(root)
    commit_sha = command_output(root, ("git", "rev-parse", "HEAD"))
    rustc = command_output(root, ("rustc", "-Vv"))
    cargo = command_output(root, ("cargo", "-V"))
    node = command_output(root, ("node", "--version"))
    npm_executable = shutil.which("npm.cmd") or shutil.which("npm")
    if npm_executable is None:
        raise ValueError("npm was not found on PATH")
    npm = command_output(root, (npm_executable, "--version"))
    ledger = json.loads(
        (root / "docs" / "product" / "manifests" / "functional-parity.v1.json").read_text(
            encoding="utf-8"
        )
    )
    formulas, errors = formula_inventory(root, ledger)

    output_dir.mkdir(parents=True, exist_ok=True)
    result = subprocess.run(
        TEST_COMMAND,
        cwd=root,
        capture_output=True,
        check=False,
        text=True,
        encoding="utf-8",
    )
    log_bytes = (result.stdout + result.stderr).encode("utf-8")
    log_path = output_dir / "cargo-test.log"
    log_path.write_bytes(log_bytes)
    if result.returncode != 0:
        errors.append("built-in formula catalog tests failed")
    finished_at = utc_now()
    return {
        "schema_version": 1,
        "evidence_id": EVIDENCE_ID,
        "status": "PASS" if not errors else "FAIL",
        "commit_sha": commit_sha,
        "tested_tree_sha": tested_tree_sha,
        "command": " ".join(TEST_COMMAND),
        "toolchain_fingerprint": {
            "rustc": rustc,
            "cargo": cargo,
            "target_host": rust_host(rustc),
            "node": node,
            "package_manager": npm,
            "os": platform.platform(),
            "features": [],
        },
        "started_at": started_at,
        "finished_at": finished_at,
        "exit_code": result.returncode,
        "ignored_or_skipped": [],
        "artifact_id": log_path.relative_to(root).as_posix(),
        "artifact_hash": sha256_bytes(log_bytes),
        "measured_result": {
            "formula_count": len(formulas),
            "formulas": formulas,
            "validation_errors": errors,
        },
    }


def parse_args(argv: Sequence[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output-dir", type=Path)
    return parser.parse_args(argv)


def main(argv: Sequence[str] | None = None) -> int:
    args = parse_args(sys.argv[1:] if argv is None else argv)
    root = Path(__file__).resolve().parents[2]
    try:
        output_dir = resolve_output_dir(root, args.output_dir)
        report = build_report(root, output_dir)
    except (OSError, ValueError, json.JSONDecodeError) as error:
        print(f"Phase 9 built-in formula qualification failed: {error}", file=sys.stderr)
        return 1
    report_path = output_dir / "phase9-builtin-formulas-report.json"
    report_path.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(report, indent=2))
    print(f"Report: {report_path}")
    return 0 if report["status"] == "PASS" else 1


if __name__ == "__main__":
    raise SystemExit(main())
