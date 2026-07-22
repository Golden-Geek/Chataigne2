#!/usr/bin/env python3
"""Run and record the Phase 9 ANode catalog qualification."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import subprocess
import sys
import tempfile
from collections.abc import Mapping, Sequence
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


EVIDENCE_ID = "phase9.formula.anode-catalog.local"
RESULT_PREFIX = "PHASE9_ANODE_CATALOG_RESULT="
CATALOG_COMMAND = (
    "cargo",
    "test",
    "-p",
    "Chataigne2",
    "app::state_machine_nodes_formula::formula_tests",
    "--",
    "--nocapture",
)
ALCHEMIST_COMMAND = ("cargo", "test", "-p", "chataigne_alchemist")
PROCESSOR_COMMAND = ("cargo", "test", "-p", "chataigne_processor")
IGNORED_PROCESSOR_FIXTURES = (
    {
        "test": "alchemist::tests::module_input_can_emit_state_transition_intent",
        "reason": (
            "obsolete pre-manager fixture; current managed-formula tests cover manager "
            "semantics"
        ),
    },
    {
        "test": "alchemist::tests::routing_node_passes_value_to_downstream_consumers",
        "reason": (
            "obsolete pre-manager fixture; the active typed-value routing test covers "
            "the current routing declaration"
        ),
    },
)


def utc_now() -> str:
    return datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


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
    with tempfile.TemporaryDirectory(prefix="chataigne-phase9-anode-index-") as directory:
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
        / "anode-catalog"
        / datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    )
    if not output_dir.is_relative_to(target_root) or output_dir == target_root:
        raise ValueError("Phase 9 ANode evidence must be a child of the workspace target directory")
    return output_dir


def parse_catalog_output(output: str) -> list[dict[str, Any]]:
    matches = [
        line.removeprefix(RESULT_PREFIX)
        for line in output.splitlines()
        if line.startswith(RESULT_PREFIX)
    ]
    if len(matches) != 1:
        raise ValueError(f"expected one {RESULT_PREFIX} record, found {len(matches)}")
    try:
        result = json.loads(matches[0])
    except json.JSONDecodeError as error:
        raise ValueError(f"invalid ANode catalog result: {error}") from error
    anodes = result.get("anodes") if isinstance(result, Mapping) else None
    if not isinstance(anodes, list):
        raise ValueError("ANode catalog result has no anodes array")
    if not all(isinstance(entry, dict) for entry in anodes):
        raise ValueError("ANode catalog result contains a non-object entry")
    return anodes


def anode_inventory(
    ledger: Mapping[str, Any], runtime_anodes: Sequence[Mapping[str, Any]]
) -> tuple[list[dict[str, Any]], list[str]]:
    rows = [
        row
        for row in ledger.get("rows", [])
        if isinstance(row, Mapping)
        and row.get("discovered_facts", {}).get("inventory_kind") == "anode"
    ]
    rows_by_type = {
        row.get("discovered_facts", {}).get("inventory_name"): row for row in rows
    }
    runtime_by_type = {entry.get("type_id"): entry for entry in runtime_anodes}
    errors: list[str] = []
    if len(rows_by_type) != len(rows):
        errors.append("generated ANode rows contain duplicate runtime type IDs")
    if len(runtime_by_type) != len(runtime_anodes):
        errors.append("runtime ANode catalog contains duplicate type IDs")

    missing_runtime = sorted(set(rows_by_type) - set(runtime_by_type))
    untracked_runtime = sorted(set(runtime_by_type) - set(rows_by_type))
    if missing_runtime:
        errors.append(f"generated ANode rows are absent from the runtime catalog: {missing_runtime}")
    if untracked_runtime:
        errors.append(f"runtime ANode types have no generated capability row: {untracked_runtime}")
    if not runtime_anodes:
        errors.append("runtime ANode catalog is empty")

    records: list[dict[str, Any]] = []
    required_fields = {
        "type_id",
        "label",
        "category",
        "execution_kind",
        "config_fields",
        "inputs",
        "outputs",
    }
    for entry in runtime_anodes:
        type_id = entry.get("type_id")
        row = rows_by_type.get(type_id)
        if row is None:
            continue
        missing_fields = sorted(required_fields - set(entry))
        if missing_fields:
            errors.append(f"{type_id} runtime result is missing fields: {missing_fields}")
            continue
        if any(not entry.get(field) for field in ("type_id", "label", "category", "execution_kind")):
            errors.append(f"{type_id} runtime result has empty identity metadata")
        capability_id = row.get("capability_id")
        if not capability_id:
            errors.append(f"{type_id} generated row has no capability ID")
        records.append({"capability_id": capability_id, **dict(entry)})
    return records, errors


def run_test(root: Path, output_dir: Path, name: str, command: Sequence[str]) -> dict[str, Any]:
    result = subprocess.run(
        command,
        cwd=root,
        capture_output=True,
        check=False,
        text=True,
        encoding="utf-8",
    )
    content = (result.stdout + result.stderr).encode("utf-8")
    path = output_dir / f"{name}.log"
    path.write_bytes(content)
    return {
        "name": name,
        "command": " ".join(command),
        "exit_code": result.returncode,
        "artifact_id": path.relative_to(root).as_posix(),
        "artifact_hash": sha256_bytes(content),
        "stdout": result.stdout,
    }


def build_report(root: Path, output_dir: Path) -> dict[str, Any]:
    started_at = utc_now()
    tested_tree_sha = working_tree_sha(root)
    commit_sha = command_output(root, ("git", "rev-parse", "HEAD"))
    rustc = command_output(root, ("rustc", "-Vv"))
    cargo = command_output(root, ("cargo", "-V"))
    ledger = json.loads(
        (root / "docs" / "product" / "manifests" / "functional-parity.v1.json").read_text(
            encoding="utf-8"
        )
    )

    output_dir.mkdir(parents=True, exist_ok=True)
    results = [
        run_test(root, output_dir, "catalog-test", CATALOG_COMMAND),
        run_test(root, output_dir, "alchemist-tests", ALCHEMIST_COMMAND),
        run_test(root, output_dir, "processor-tests", PROCESSOR_COMMAND),
    ]
    errors = [f"{result['name']} failed" for result in results if result["exit_code"] != 0]
    runtime_anodes: list[dict[str, Any]] = []
    if results[0]["exit_code"] == 0:
        try:
            runtime_anodes = parse_catalog_output(results[0]["stdout"])
        except ValueError as error:
            errors.append(str(error))
    anodes, inventory_errors = anode_inventory(ledger, runtime_anodes)
    errors.extend(inventory_errors)
    finished_at = utc_now()
    public_results = [{key: value for key, value in result.items() if key != "stdout"} for result in results]
    return {
        "schema_version": 1,
        "evidence_id": EVIDENCE_ID,
        "status": "PASS" if not errors else "FAIL",
        "commit_sha": commit_sha,
        "tested_tree_sha": tested_tree_sha,
        "command": " && ".join(result["command"] for result in public_results),
        "toolchain_fingerprint": {
            "rustc": rustc,
            "cargo": cargo,
            "target_host": rust_host(rustc),
            "os": platform.platform(),
            "features": [],
        },
        "started_at": started_at,
        "finished_at": finished_at,
        "exit_code": max(result["exit_code"] for result in public_results),
        "ignored_or_skipped": list(IGNORED_PROCESSOR_FIXTURES),
        "artifacts": public_results,
        "measured_result": {
            "anode_count": len(anodes),
            "anodes": anodes,
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
        print(f"Phase 9 ANode catalog qualification failed: {error}", file=sys.stderr)
        return 1
    report_path = output_dir / "phase9-anode-catalog-report.json"
    report_path.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(report, indent=2))
    print(f"Report: {report_path}")
    return 0 if report["status"] == "PASS" else 1


if __name__ == "__main__":
    raise SystemExit(main())
