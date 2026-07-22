#!/usr/bin/env python3
"""Run and record the Phase 9 generator-module family qualification."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import subprocess
import sys
import tempfile
from collections import Counter
from collections.abc import Mapping, Sequence
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


EVIDENCE_ID = "phase9.module.generator-families.local"
GENERATOR_COMMAND = (
    "cargo",
    "test",
    "-p",
    "Chataigne2",
    "app::module_modules_generators_",
    "--",
    "--nocapture",
)
SCRIPT_COMMAND = (
    "cargo",
    "test",
    "-p",
    "Chataigne2",
    "app::module::script_api::tests",
    "--",
    "--nocapture",
)
CONTRACT_COMMAND = ("python", "tools/migration/check_phase8_contracts.py")

QUALIFIED_KINDS = {
    "module",
    "node_type",
    "script_callback",
    "script_method",
    "script_snippet",
    "script_template",
}
FAMILY_SOURCE_PREFIXES = {
    "signals": (
        "apps/chataigne/src/module/modules/generators/signals/",
        "apps/chataigne/src/module/script_templates/signals_module.js",
        "apps/chataigne/src/module/script_templates/snippets/signals_",
    ),
    "metronomes": (
        "apps/chataigne/src/module/modules/generators/metronomes/",
        "apps/chataigne/src/module/script_templates/metronomes_module.js",
        "apps/chataigne/src/module/script_templates/snippets/metronomes_",
    ),
    "spatializer": (
        "apps/chataigne/src/module/modules/generators/spatializer",
        "apps/chataigne/src/module/script_templates/spatializer_module.js",
        "apps/chataigne/src/module/script_templates/snippets/spatializer_",
    ),
}
EXPECTED_FAMILY_COUNTS = {"signals": 9, "metronomes": 9, "spatializer": 8}
REQUIRED_KINDS = {
    "signals": {
        "module",
        "node_type",
        "script_callback",
        "script_method",
        "script_snippet",
        "script_template",
    },
    "metronomes": {
        "module",
        "node_type",
        "script_callback",
        "script_method",
        "script_snippet",
        "script_template",
    },
    "spatializer": {"module", "node_type", "script_snippet", "script_template"},
}


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
        (
            line.removeprefix("host: ")
            for line in rustc.splitlines()
            if line.startswith("host: ")
        ),
        platform.machine(),
    )


def working_tree_sha(root: Path) -> str:
    with tempfile.TemporaryDirectory(prefix="chataigne-phase9-generator-index-") as directory:
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
        / "generator-modules"
        / datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    )
    if not output_dir.is_relative_to(target_root) or output_dir == target_root:
        raise ValueError("Phase 9 generator evidence must be a child of the workspace target directory")
    return output_dir


def row_source_paths(row: Mapping[str, Any]) -> list[str]:
    sources = row.get("baseline_sources", [])
    if not isinstance(sources, list):
        return []
    return [
        source["path"].replace("\\", "/").lower()
        for source in sources
        if isinstance(source, Mapping) and isinstance(source.get("path"), str)
    ]


def generator_family(row: Mapping[str, Any]) -> str | None:
    facts = row.get("discovered_facts", {})
    if not isinstance(facts, Mapping) or facts.get("inventory_kind") not in QUALIFIED_KINDS:
        return None
    paths = row_source_paths(row)
    for family, prefixes in FAMILY_SOURCE_PREFIXES.items():
        if any(path.startswith(prefix) for path in paths for prefix in prefixes):
            return family
    return None


def generator_capability_inventory(
    ledger: Mapping[str, Any],
    expected_counts: Mapping[str, int] | None = EXPECTED_FAMILY_COUNTS,
) -> tuple[list[dict[str, str]], list[str]]:
    records: list[dict[str, str]] = []
    errors: list[str] = []
    for row in ledger.get("rows", []):
        if not isinstance(row, Mapping):
            continue
        family = generator_family(row)
        if family is None:
            continue
        capability_id = row.get("capability_id")
        facts = row.get("discovered_facts", {})
        kind = facts.get("inventory_kind") if isinstance(facts, Mapping) else None
        if not isinstance(capability_id, str) or not capability_id:
            errors.append(f"{family} generator row has no capability ID")
            continue
        if not isinstance(kind, str):
            errors.append(f"{capability_id} has no inventory kind")
            continue
        records.append({"capability_id": capability_id, "family": family, "kind": kind})

    ids = [record["capability_id"] for record in records]
    duplicate_ids = sorted(capability_id for capability_id, count in Counter(ids).items() if count > 1)
    if duplicate_ids:
        errors.append(f"generator qualification contains duplicate capability IDs: {duplicate_ids}")

    for family, required_kinds in REQUIRED_KINDS.items():
        actual_kinds = {record["kind"] for record in records if record["family"] == family}
        missing = sorted(required_kinds - actual_kinds)
        if missing:
            errors.append(f"{family} generator inventory is missing required kinds: {missing}")

    if expected_counts is not None:
        actual_counts = Counter(record["family"] for record in records)
        for family, expected in expected_counts.items():
            actual = actual_counts[family]
            if actual != expected:
                errors.append(f"{family} generator inventory has {actual} rows; expected {expected}")

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
    capabilities, inventory_errors = generator_capability_inventory(ledger)

    output_dir.mkdir(parents=True, exist_ok=True)
    results = [
        run_test(root, output_dir, "generator-tests", GENERATOR_COMMAND),
        run_test(root, output_dir, "script-template-tests", SCRIPT_COMMAND),
        run_test(root, output_dir, "phase8-contract", CONTRACT_COMMAND),
    ]
    errors = [f"{result['name']} failed" for result in results if result["exit_code"] != 0]
    errors.extend(inventory_errors)
    family_counts = Counter(record["family"] for record in capabilities)
    kind_counts = Counter(record["kind"] for record in capabilities)

    return {
        "schema_version": 1,
        "evidence_id": EVIDENCE_ID,
        "status": "PASS" if not errors else "FAIL",
        "commit_sha": commit_sha,
        "tested_tree_sha": tested_tree_sha,
        "command": " && ".join(result["command"] for result in results),
        "toolchain_fingerprint": {
            "rustc": rustc,
            "cargo": cargo,
            "target_host": rust_host(rustc),
            "os": platform.platform(),
            "features": [],
        },
        "started_at": started_at,
        "finished_at": utc_now(),
        "exit_code": max(result["exit_code"] for result in results),
        "ignored_or_skipped": [],
        "artifacts": results,
        "measured_result": {
            "capability_count": len(capabilities),
            "family_counts": dict(sorted(family_counts.items())),
            "kind_counts": dict(sorted(kind_counts.items())),
            "capabilities": capabilities,
            "excluded_pending_evidence": [
                "three generator SVG assets require rendered visual evidence",
                "SpatializerEditor and View Home/View Frame require UI interaction evidence",
            ],
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
        print(f"Phase 9 generator-module qualification failed: {error}", file=sys.stderr)
        return 1
    report_path = output_dir / "phase9-generator-modules-report.json"
    report_path.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(report, indent=2))
    print(f"Report: {report_path}")
    return 0 if report["status"] == "PASS" else 1


if __name__ == "__main__":
    raise SystemExit(main())
