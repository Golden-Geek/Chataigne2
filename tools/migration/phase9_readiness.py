from __future__ import annotations

import argparse
import json
import re
import sys
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any


PHASE8_CHECKPOINT = "b45a9b0a7a01ebee386e24a91daa42f897054bc6"
BASELINE_REF = "fb0f3a58f3593df8994bf8bd46f88ddd7612f41d"
COMMIT_SHA = re.compile(r"^[0-9a-f]{40}$")
EXPECTED_SUBPHASES = {"9A", "9B", "9C", "9D"}
EXPECTED_GATES = {
    "cross-platform-product",
    "final-documentation",
    "functional-parity-ledger",
    "governed-deletion",
    "graph-full-workbench-scale",
    "hardware-soak",
    "linux-clean-package",
    "macos-clean-package",
    "module-network-multiclient-soak",
    "product-ux-parity",
    "runtime-100k-dense",
    "runtime-100k-idle",
    "runtime-100k-sparse",
    "windows-clean-package",
}
PRODUCT_AREAS = {
    "dashboard",
    "diagnostics",
    "formula",
    "graph",
    "host",
    "module",
    "networking",
    "persistence",
    "script",
    "spatializer",
    "state_machine",
    "workbench",
}
NORMATIVE_ROW_FIELDS = {
    "approval",
    "baseline_source",
    "capability_id",
    "classification",
    "dependencies",
    "evidence",
    "final_owner",
    "last_passing_result",
    "manual_evidence",
    "migration_state",
    "product_area",
    "runtime_semantics",
    "temporary_adapters",
    "title",
    "user_workflow",
}


def _load_json(root: Path, relative: str) -> dict[str, Any]:
    path = root / relative
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ValueError(f"cannot load {relative}: {error}") from error
    if not isinstance(value, dict):
        raise ValueError(f"{relative} must contain a JSON object")
    return value


def _result_passed(result: Any) -> bool:
    if not isinstance(result, Mapping) or result.get("status") != "PASS":
        return False
    required = {
        "commit_sha",
        "exit_code",
        "finished_at",
        "ignored_or_skipped",
        "started_at",
        "tested_tree_sha",
        "toolchain_fingerprint",
    }
    return required.issubset(result)


def _workflow_characterized(value: Any) -> bool:
    if not isinstance(value, Mapping):
        return False
    status = value.get("status")
    return status in {"characterized", "complete"} and bool(value.get("steps"))


def _runtime_characterized(value: Any) -> bool:
    if not isinstance(value, Mapping):
        return False
    return value.get("status") in {"characterized", "complete"}


def _owner_assigned(value: Any) -> bool:
    if isinstance(value, str):
        return bool(value.strip())
    return (
        isinstance(value, Mapping)
        and value.get("status") in {"assigned", "complete"}
        and isinstance(value.get("path"), str)
        and bool(value["path"].strip())
    )


def _required_evidence_present(value: Any) -> bool:
    if not isinstance(value, list) or not value:
        return False
    for entry in value:
        if not isinstance(entry, Mapping):
            return False
        if not isinstance(entry.get("required"), bool):
            return False
        if not entry.get("evidence_id") or not entry.get("kind"):
            return False
        if entry["required"] and not entry.get("command"):
            return False
        if entry["required"] and not entry.get("platforms"):
            return False
        if not isinstance(entry.get("fixtures"), list):
            return False
    return True


def _manual_evidence_complete(value: Any) -> bool:
    if isinstance(value, list):
        return all(
            isinstance(entry, Mapping) and entry.get("status") == "PASS"
            for entry in value
        )
    return isinstance(value, Mapping) and value.get("status") in {
        "not_required",
        "PASS",
    }


def _passing_evidence_record(value: Any) -> bool:
    if not isinstance(value, Mapping) or value.get("status") != "PASS":
        return False
    required = {
        "artifact_hash",
        "artifact_id",
        "command",
        "evidence_id",
        "tested_tree_sha",
    }
    return required.issubset(value) and all(value.get(field) for field in required)


def _valid_commit(value: Any) -> bool:
    return isinstance(value, str) and COMMIT_SHA.fullmatch(value) is not None


def row_is_qualified(row: Mapping[str, Any]) -> bool:
    if not NORMATIVE_ROW_FIELDS.issubset(row):
        return False
    if row.get("classification") not in {
        "operational_baseline",
        "baseline_scaffolding",
        "planned_functionality",
    }:
        return False
    if row.get("product_area") not in PRODUCT_AREAS:
        return False
    if not isinstance(row.get("title"), str) or not row["title"].strip():
        return False
    baseline_source = row.get("baseline_source")
    if not isinstance(baseline_source, Mapping):
        return False
    if not baseline_source.get("repository_ref") or not baseline_source.get("paths"):
        return False
    if not _workflow_characterized(row.get("user_workflow")):
        return False
    if not _runtime_characterized(row.get("runtime_semantics")):
        return False
    if not _owner_assigned(row.get("final_owner")):
        return False
    if not _required_evidence_present(row.get("evidence")):
        return False
    if not _result_passed(row.get("last_passing_result")):
        return False
    if not _manual_evidence_complete(row.get("manual_evidence")):
        return False
    if row.get("migration_state") not in {"cut_over", "old_path_removed"}:
        return False
    if row.get("temporary_adapters") != []:
        return False
    if not isinstance(row.get("dependencies"), list):
        return False
    if row.get("classification") == "planned_functionality" and not row.get("approval"):
        return False
    return True


def build_report(root: Path) -> dict[str, Any]:
    root = root.resolve()
    dashboard = _load_json(
        root, "docs/product/manifests/phase9-qualification.v1.json"
    )
    ledger = _load_json(
        root, "docs/product/manifests/functional-parity.v1.json"
    )
    evidence = _load_json(
        root, "docs/product/manifests/functional-parity-evidence.v1.json"
    )
    blockers: list[str] = []

    if dashboard.get("phase") != 9:
        blockers.append("Phase 9 dashboard has the wrong phase number")
    validation_state = dashboard.get("validation_state")
    if validation_state not in {"CONSTRUCTION", "CHECKPOINT_RUNNABLE"}:
        blockers.append("Phase 9 dashboard has an invalid validation state")

    last_checkpoint = dashboard.get("last_runnable_checkpoint")
    if validation_state == "CONSTRUCTION":
        if (
            not isinstance(last_checkpoint, Mapping)
            or last_checkpoint.get("phase") != 8
            or last_checkpoint.get("qualified_commit") != PHASE8_CHECKPOINT
        ):
            blockers.append("Phase 9 construction does not preserve the immutable Phase 8 checkpoint")
    elif validation_state == "CHECKPOINT_RUNNABLE":
        if (
            not isinstance(last_checkpoint, Mapping)
            or last_checkpoint.get("phase") != 9
            or not _valid_commit(last_checkpoint.get("qualified_commit"))
            or not _valid_commit(last_checkpoint.get("record_commit"))
            or not last_checkpoint.get("cross_platform_run")
            or not last_checkpoint.get("package_run")
        ):
            blockers.append("Phase 9 runnable state does not record its exact qualified checkpoint")

    subphases = {
        item.get("subphase_id"): item
        for item in dashboard.get("subphases", [])
        if isinstance(item, Mapping)
    }
    missing_subphases = sorted(EXPECTED_SUBPHASES - set(subphases))
    if missing_subphases:
        blockers.append(f"Phase 9 dashboard is missing subphases: {missing_subphases}")
    incomplete_subphases = sorted(
        subphase_id
        for subphase_id in EXPECTED_SUBPHASES
        if subphases.get(subphase_id, {}).get("state") != "runnable"
    )
    if incomplete_subphases:
        blockers.append(f"Phase 9 subphases are not runnable: {incomplete_subphases}")

    evidence_records = {
        item.get("evidence_id"): item
        for item in dashboard.get("evidence_records", [])
        if isinstance(item, Mapping) and item.get("evidence_id")
    }
    if len(evidence_records) != len(dashboard.get("evidence_records", [])):
        blockers.append("Phase 9 evidence record IDs are missing or duplicated")

    gates = {
        item.get("gate_id"): item
        for item in dashboard.get("qualification_gates", [])
        if isinstance(item, Mapping)
    }
    missing_gates = sorted(EXPECTED_GATES - set(gates))
    if missing_gates:
        blockers.append(f"Phase 9 dashboard is missing gates: {missing_gates}")
    incomplete_gates = sorted(
        gate_id
        for gate_id in EXPECTED_GATES
        if gates.get(gate_id, {}).get("state") != "PASS"
        or not _passing_evidence_record(
            evidence_records.get(gates.get(gate_id, {}).get("evidence"))
        )
    )
    if incomplete_gates:
        blockers.append(f"Phase 9 qualification gates are incomplete: {incomplete_gates}")

    rows = ledger.get("rows")
    if not isinstance(rows, list):
        rows = []
        blockers.append("functional parity ledger does not contain a rows array")
    row_ids = [
        row.get("capability_id") for row in rows if isinstance(row, Mapping)
    ]
    if len(row_ids) != len(set(row_ids)):
        blockers.append("functional parity capability IDs are not unique")

    parity_state = dashboard.get("parity_ledger")
    expected_rows = (
        parity_state.get("expected_rows")
        if isinstance(parity_state, Mapping)
        else None
    )
    if expected_rows != len(rows):
        blockers.append(
            f"functional parity row count drifted: dashboard={expected_rows}, actual={len(rows)}"
        )

    if evidence.get("manifest_kind") != "functional_parity_evidence":
        blockers.append("functional parity evidence has the wrong manifest kind")
    if evidence.get("inventory") != "docs/product/manifests/functional-parity.v1.json":
        blockers.append("functional parity evidence names the wrong discovery inventory")
    if evidence.get("baseline_ref") != BASELINE_REF:
        blockers.append("functional parity evidence does not preserve the Phase 0 baseline ref")
    if isinstance(parity_state, Mapping):
        if parity_state.get("evidence_manifest") != (
            "docs/product/manifests/functional-parity-evidence.v1.json"
        ):
            blockers.append("Phase 9 dashboard names the wrong parity evidence input")
        if parity_state.get("schema_state") != (
            "generated_discovery_with_authored_evidence"
        ):
            blockers.append("Phase 9 parity schema boundary is not recorded")
        if parity_state.get("identity_state") != "stable":
            blockers.append("functional parity capability identity audit is incomplete")

    evidence_entries = evidence.get("entries")
    if not isinstance(evidence_entries, list):
        evidence_entries = []
        blockers.append("functional parity evidence does not contain an entries array")
    evidence_ids = [
        entry.get("capability_id")
        for entry in evidence_entries
        if isinstance(entry, Mapping)
    ]
    if len(evidence_ids) != len(set(evidence_ids)):
        blockers.append("functional parity evidence capability IDs are not unique")
    stale_evidence = sorted(set(evidence_ids) - set(row_ids))
    if stale_evidence:
        blockers.append(
            f"functional parity evidence contains stale capability IDs: {stale_evidence[:5]}"
        )
    invalid_evidence = sorted(
        entry.get("capability_id", "<missing>")
        if isinstance(entry, Mapping)
        else "<invalid-entry>"
        for entry in evidence_entries
        if not isinstance(entry, Mapping) or not row_is_qualified(entry)
    )
    if invalid_evidence:
        blockers.append(
            "functional parity evidence entries are not qualified: "
            f"{invalid_evidence[:5]}"
        )

    qualified_rows = sum(
        row_is_qualified(entry)
        and entry.get("capability_id") in set(row_ids)
        for entry in evidence_entries
        if isinstance(entry, Mapping)
    )
    if isinstance(parity_state, Mapping) and parity_state.get(
        "qualified_rows"
    ) != qualified_rows:
        blockers.append(
            "functional parity qualified row count does not match the dashboard"
        )
    if qualified_rows != len(rows):
        blockers.append(
            "functional parity evidence is incomplete: "
            f"{qualified_rows}/{len(rows)} rows qualified"
        )

    carried_adapters = dashboard.get("carried_temporary_adapters")
    if carried_adapters:
        blockers.append(
            f"temporary migration adapters remain: {sorted(carried_adapters)}"
        )

    if validation_state == "CONSTRUCTION":
        blockers.append("Phase 9 remains in CONSTRUCTION")

    if validation_state == "CHECKPOINT_RUNNABLE" and blockers:
        blockers.insert(
            0, "Phase 9 claims CHECKPOINT_RUNNABLE while acceptance blockers remain"
        )

    return {
        "phase": 9,
        "validation_state": validation_state,
        "ready": not blockers,
        "metrics": {
            "parity_rows": len(rows),
            "qualified_parity_rows": qualified_rows,
            "phase9_gates": len(EXPECTED_GATES),
            "passing_phase9_gates": len(EXPECTED_GATES) - len(incomplete_gates),
            "carried_temporary_adapters": len(carried_adapters or []),
        },
        "blockers": blockers,
    }


def main(arguments: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Report Phase 9 acceptance readiness.")
    parser.add_argument("--root", type=Path, default=Path.cwd())
    parser.add_argument("--json", action="store_true", dest="as_json")
    options = parser.parse_args(arguments)
    try:
        report = build_report(options.root)
    except ValueError as error:
        print(f"Phase 9 readiness error: {error}", file=sys.stderr)
        return 2

    if options.as_json:
        print(json.dumps(report, indent=2, sort_keys=True))
    else:
        state = "READY" if report["ready"] else "BLOCKED"
        print(f"Phase 9 readiness: {state}")
        for blocker in report["blockers"]:
            print(f"- {blocker}")
    return 0 if report["ready"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
