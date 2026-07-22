from __future__ import annotations

import argparse
import hashlib
import json
import platform
import shutil
import subprocess
import sys
from collections.abc import Mapping, Sequence
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

try:
    from .run_phase9_scale import command_output, working_tree_sha
except ImportError:
    from run_phase9_scale import command_output, working_tree_sha


EVIDENCE_ID = "phase9.product.functional-parity.local"


def utc_now() -> str:
    return datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")


def sha256_file(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def validate_product_report(report: Mapping[str, Any]) -> None:
    if report.get("schema_version") != 1 or report.get("gate") != "chataigne-product-gate":
        raise ValueError("product report has the wrong contract")
    if report.get("validation") != "RUNNABLE" or report.get("overall_status") != "PASS":
        raise ValueError("the complete product gate did not pass")
    results = report.get("results")
    if not isinstance(results, list):
        raise ValueError("product report contains no result list")
    incomplete = [
        item.get("id", "<missing>")
        for item in results
        if isinstance(item, Mapping) and item.get("required") and item.get("status") != "PASS"
    ]
    if incomplete:
        raise ValueError(f"required product checks did not pass: {incomplete}")
    required_ids = {
        "architecture.phase8_contracts",
        "architecture.phase9_deletion",
        "architecture.phase9_docs",
        "e2e.lan_non_loopback",
        "e2e.ui_workflow",
        "evidence.module_loopback",
        "product_manifest.drift",
        "product_manifest.schema",
        "rust.clippy",
        "rust.test",
        "smoke.cargo_run",
        "smoke.cargo_run_dev",
        "smoke.watch",
        "ui.build",
        "ui.check",
        "ui.lint",
        "ui.unit_tests",
    }
    passed_ids = {
        item.get("id")
        for item in results
        if isinstance(item, Mapping) and item.get("status") == "PASS"
    }
    missing = required_ids - passed_ids
    if missing:
        raise ValueError(f"product report is missing parity checks: {sorted(missing)}")


def resolve_output_dir(root: Path, value: Path | None) -> Path:
    target = (root / "target").resolve()
    output = (
        (root / value).resolve()
        if value is not None and not value.is_absolute()
        else value.resolve()
        if value is not None
        else target / "phase9" / "product-parity" / datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    )
    if output == target or not output.is_relative_to(target):
        raise ValueError("Phase 9 parity output must be below the workspace target directory")
    return output


def run(arguments: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Run and record complete Phase 9 product parity.")
    parser.add_argument("--output-dir", type=Path)
    parser.add_argument("--skip-ui-install", action="store_true")
    options = parser.parse_args(arguments)
    root = Path(__file__).resolve().parents[2]
    output = resolve_output_dir(root, options.output_dir)
    output.mkdir(parents=True, exist_ok=True)
    product_report_path = output / "product-gate" / "product-gate-report.json"
    product_report_path.parent.mkdir(parents=True, exist_ok=True)
    tested_tree_sha = working_tree_sha(root)
    started_at = utc_now()
    powershell = shutil.which("pwsh") or shutil.which("powershell.exe")
    if powershell is None:
        raise ValueError("PowerShell is required for the complete product gate")
    command = [
        powershell,
        "-NoLogo",
        "-NoProfile",
        "-NonInteractive",
        "-File",
        str(root / "tools" / "product-gate" / "product-gate.ps1"),
        "-ReportPath",
        str(product_report_path),
        "-RequiredPlatforms",
        native_platform_name(),
    ]
    if options.skip_ui_install:
        command.append("-SkipUiInstall")
    result = subprocess.run(command, cwd=root, check=False)
    if result.returncode != 0:
        raise ValueError(f"product gate failed with exit code {result.returncode}")
    product_report = json.loads(product_report_path.read_text(encoding="utf-8-sig"))
    validate_product_report(product_report)
    inventory = json.loads(
        (root / "docs" / "product" / "manifests" / "functional-parity.v1.json").read_text(encoding="utf-8")
    )
    capability_ids = sorted(row["capability_id"] for row in inventory["rows"])
    report = {
        "schema_version": 1,
        "contract": "chataigne-phase9-product-parity-report-v1",
        "evidence_id": EVIDENCE_ID,
        "status": "PASS",
        "commit_sha": command_output(root, ("git", "rev-parse", "HEAD")),
        "tested_tree_sha": tested_tree_sha,
        "started_at": started_at,
        "finished_at": utc_now(),
        "capability_count": len(capability_ids),
        "capability_ids_sha256": hashlib.sha256(("\n".join(capability_ids) + "\n").encode()).hexdigest(),
        "product_report": product_report_path.relative_to(root).as_posix(),
        "product_report_sha256": sha256_file(product_report_path),
        "toolchain": product_report.get("toolchain", {}),
        "platform": native_platform_name(),
    }
    report_path = output / "phase9-product-parity-report.json"
    report_path.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(f"Phase 9 product parity report: {report_path.relative_to(root).as_posix()}")
    return 0


def native_platform_name() -> str:
    if sys.platform == "win32":
        return "windows"
    if sys.platform == "darwin":
        return "macos"
    if sys.platform.startswith("linux"):
        return "linux"
    return platform.system().lower()


def main() -> int:
    try:
        return run()
    except (OSError, ValueError, json.JSONDecodeError) as error:
        print(f"Phase 9 product parity failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
