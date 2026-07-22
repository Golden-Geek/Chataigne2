from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
import sys
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any


PHASE0_INVENTORY_COMMIT = "82a72b3ef517aefe32e4a6907e6cba66aab52022"
LEDGER_PATH = "docs/product/manifests/functional-parity.v1.json"
DASHBOARD_PATH = "docs/product/manifests/phase9-qualification.v1.json"


def _capability_ids(document: Mapping[str, Any]) -> list[str]:
    rows = document.get("rows")
    if not isinstance(rows, list):
        return []
    return [
        row.get("capability_id")
        for row in rows
        if isinstance(row, Mapping) and isinstance(row.get("capability_id"), str)
    ]


def _ids_digest(capability_ids: Sequence[str]) -> str:
    payload = "".join(f"{capability_id}\n" for capability_id in sorted(capability_ids))
    return hashlib.sha256(payload.encode("utf-8")).hexdigest()


def compare_documents(
    baseline: Mapping[str, Any], current: Mapping[str, Any]
) -> dict[str, Any]:
    baseline_ids = _capability_ids(baseline)
    current_ids = _capability_ids(current)
    baseline_set = set(baseline_ids)
    current_set = set(current_ids)
    missing = sorted(baseline_set - current_set)
    added = sorted(current_set - baseline_set)
    blockers: list[str] = []
    if len(baseline_ids) != len(baseline_set):
        blockers.append("Phase 0 inventory contains duplicate capability IDs")
    if len(current_ids) != len(current_set):
        blockers.append("current inventory contains duplicate capability IDs")
    if missing:
        blockers.append(f"Phase 0 capability IDs disappeared: {missing[:5]}")
    return {
        "ready": not blockers,
        "metrics": {
            "baseline_rows": len(baseline_ids),
            "current_rows": len(current_ids),
            "preserved_baseline_rows": len(baseline_set & current_set),
            "new_rows": len(added),
            "new_capability_ids_sha256": _ids_digest(added),
        },
        "missing_capability_ids": missing,
        "new_capability_ids": added,
        "blockers": blockers,
    }


def _load_json(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ValueError(f"cannot load {path}: {error}") from error
    if not isinstance(value, dict):
        raise ValueError(f"{path} must contain a JSON object")
    return value


def _load_phase0_inventory(root: Path) -> dict[str, Any]:
    command = [
        "git",
        "show",
        f"{PHASE0_INVENTORY_COMMIT}:{LEDGER_PATH}",
    ]
    result = subprocess.run(
        command,
        cwd=root,
        capture_output=True,
        check=False,
        text=True,
        encoding="utf-8",
    )
    if result.returncode != 0:
        detail = result.stderr.strip() or "git show failed"
        raise ValueError(f"cannot load immutable Phase 0 inventory: {detail}")
    try:
        value = json.loads(result.stdout)
    except json.JSONDecodeError as error:
        raise ValueError(f"Phase 0 inventory is invalid JSON: {error}") from error
    if not isinstance(value, dict):
        raise ValueError("Phase 0 inventory must contain a JSON object")
    return value


def build_report(root: Path) -> dict[str, Any]:
    root = root.resolve()
    baseline = _load_phase0_inventory(root)
    current = _load_json(root / LEDGER_PATH)
    dashboard = _load_json(root / DASHBOARD_PATH)
    report = compare_documents(baseline, current)
    blockers = list(report["blockers"])
    parity = dashboard.get("parity_ledger")
    identity = parity.get("identity_evidence") if isinstance(parity, Mapping) else None
    if not isinstance(identity, Mapping):
        blockers.append("Phase 9 dashboard does not record capability identity evidence")
    else:
        expected = {
            "phase0_inventory_commit": PHASE0_INVENTORY_COMMIT,
            **report["metrics"],
        }
        for field, value in expected.items():
            if identity.get(field) != value:
                blockers.append(
                    f"Phase 9 identity evidence drifted for {field}: "
                    f"dashboard={identity.get(field)!r}, actual={value!r}"
                )
    if not isinstance(parity, Mapping) or parity.get("identity_state") != "stable":
        blockers.append("Phase 9 dashboard does not mark capability identity stable")
    report["ready"] = not blockers
    report["blockers"] = blockers
    report["phase0_inventory_commit"] = PHASE0_INVENTORY_COMMIT
    return report


def main(arguments: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Verify stable capability identity from Phase 0 to Phase 9."
    )
    parser.add_argument("--root", type=Path, default=Path.cwd())
    parser.add_argument("--json", action="store_true", dest="as_json")
    options = parser.parse_args(arguments)
    try:
        report = build_report(options.root)
    except ValueError as error:
        print(f"Phase 9 identity error: {error}", file=sys.stderr)
        return 2

    if options.as_json:
        print(json.dumps(report, indent=2, sort_keys=True))
    else:
        state = "STABLE" if report["ready"] else "DRIFTED"
        print(f"Phase 9 capability identity: {state}")
        for blocker in report["blockers"]:
            print(f"- {blocker}")
    return 0 if report["ready"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
