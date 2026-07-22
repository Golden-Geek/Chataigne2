from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
import sys
from collections.abc import Sequence
from datetime import datetime, timezone
from pathlib import Path

try:
    from .run_phase9_scale import command_output, working_tree_sha
except ImportError:
    from run_phase9_scale import command_output, working_tree_sha


COMMANDS = (
    ("python", "tools/migration/check_phase9_deletion.py"),
    ("python", "tools/migration/check_phase9_docs.py"),
    ("python", "tools/migration/product_manifest.py", "check"),
    ("python", "tools/migration/product_manifest.py", "validate"),
    ("python", "tools/migration/check_phase2_contracts.py"),
    ("python", "tools/migration/check_phase3_contracts.py"),
    ("python", "tools/migration/check_phase4_contracts.py"),
    ("python", "tools/migration/check_phase5_contracts.py"),
    ("python", "tools/migration/check_phase6_contracts.py"),
    ("python", "tools/migration/check_phase7_contracts.py"),
    ("python", "tools/migration/check_phase8_contracts.py"),
)


def utc_now() -> str:
    return datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")


def run(arguments: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Record Phase 9 deletion and documentation evidence.")
    parser.add_argument("--output-dir", type=Path)
    options = parser.parse_args(arguments)
    root = Path(__file__).resolve().parents[2]
    target = (root / "target").resolve()
    output = (
        (root / options.output_dir).resolve()
        if options.output_dir is not None and not options.output_dir.is_absolute()
        else options.output_dir.resolve()
        if options.output_dir is not None
        else target / "phase9" / "finalization" / datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    )
    if output == target or not output.is_relative_to(target):
        raise ValueError("Phase 9 finalization evidence must be below the workspace target directory")
    output.mkdir(parents=True, exist_ok=True)
    started_at = utc_now()
    results = []
    for index, command in enumerate(COMMANDS):
        result = subprocess.run(command, cwd=root, capture_output=True, text=True, encoding="utf-8", check=False)
        log_path = output / f"{index:02d}-{Path(command[1]).stem}.log"
        log_path.write_text(result.stdout + result.stderr, encoding="utf-8")
        results.append(
            {
                "command": " ".join(command),
                "status": "PASS" if result.returncode == 0 else "FAIL",
                "exit_code": result.returncode,
                "log": log_path.relative_to(root).as_posix(),
                "log_sha256": hashlib.sha256(log_path.read_bytes()).hexdigest(),
            }
        )
        if result.returncode != 0:
            raise ValueError(f"finalization command failed: {' '.join(command)}")
    report = {
        "schema_version": 1,
        "contract": "chataigne-phase9-finalization-report-v1",
        "evidence_id": "phase9.finalization.local",
        "status": "PASS",
        "commit_sha": command_output(root, ("git", "rev-parse", "HEAD")),
        "tested_tree_sha": working_tree_sha(root),
        "started_at": started_at,
        "finished_at": utc_now(),
        "results": results,
    }
    report_path = output / "phase9-finalization-report.json"
    report_path.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(f"Phase 9 finalization report: {report_path.relative_to(root).as_posix()}")
    return 0


def main() -> int:
    try:
        return run()
    except (OSError, ValueError) as error:
        print(f"Phase 9 finalization failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
