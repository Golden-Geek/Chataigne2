from __future__ import annotations

import importlib.util
import json
import tempfile
import unittest
from pathlib import Path


SCRIPT = Path(__file__).resolve().parents[1] / "phase9_readiness.py"
SPEC = importlib.util.spec_from_file_location("phase9_readiness", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


def write_json(root: Path, relative: str, value: object) -> None:
    path = root / relative
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value), encoding="utf-8")


def passing_result() -> dict[str, object]:
    return {
        "status": "PASS",
        "commit_sha": "qualified",
        "tested_tree_sha": "qualified-tree",
        "toolchain_fingerprint": "test-toolchain",
        "started_at": "2026-07-18T00:00:00Z",
        "finished_at": "2026-07-18T00:00:01Z",
        "exit_code": 0,
        "ignored_or_skipped": [],
    }


def qualified_row() -> dict[str, object]:
    return {
        "capability_id": "test/capability",
        "product_area": "workbench",
        "classification": "operational_baseline",
        "title": "Test capability",
        "baseline_source": {
            "repository_ref": "baseline",
            "paths": ["baseline/path"],
        },
        "user_workflow": {
            "status": "complete",
            "steps": ["Perform the action"],
            "expected_feedback": ["Observe the result"],
        },
        "runtime_semantics": {
            "status": "complete",
            "inputs": ["intent"],
            "outputs": ["result"],
            "state": [],
            "ordering": "ordered",
            "timing": "bounded",
            "errors": [],
            "recovery": [],
        },
        "final_owner": {"status": "complete", "path": "crates/test"},
        "evidence": [
            {
                "evidence_id": "test-evidence",
                "kind": "integration",
                "command": "test command",
                "platforms": ["test"],
                "fixtures": [],
                "required": True,
            }
        ],
        "last_passing_result": passing_result(),
        "manual_evidence": [],
        "migration_state": "old_path_removed",
        "temporary_adapters": [],
        "approval": None,
        "dependencies": [],
    }


def ready_dashboard() -> dict[str, object]:
    return {
        "schema_version": 1,
        "phase": 9,
        "revision": 1,
        "validation_state": "CHECKPOINT_RUNNABLE",
        "last_runnable_checkpoint": {
            "phase": 8,
            "qualified_commit": MODULE.PHASE8_CHECKPOINT,
        },
        "construction_interval": {},
        "parity_ledger": {
            "evidence_manifest": (
                "docs/product/manifests/functional-parity-evidence.v1.json"
            ),
            "expected_rows": 1,
            "qualified_rows": 1,
            "schema_state": "generated_discovery_with_authored_evidence",
            "identity_state": "stable",
        },
        "subphases": [
            {"subphase_id": subphase_id, "state": "runnable"}
            for subphase_id in sorted(MODULE.EXPECTED_SUBPHASES)
        ],
        "evidence_records": [
            {
                "evidence_id": "test-result",
                "status": "PASS",
                "tested_tree_sha": "test-tree",
                "command": "test command",
                "artifact_id": "test-artifact",
                "artifact_hash": "test-hash",
            }
        ],
        "qualification_gates": [
            {"gate_id": gate_id, "state": "PASS", "evidence": "test-result"}
            for gate_id in sorted(MODULE.EXPECTED_GATES)
        ],
        "carried_temporary_adapters": [],
    }


class Phase9ReadinessTests(unittest.TestCase):
    def test_current_tree_reports_truthful_construction_blockers(self) -> None:
        root = Path(__file__).resolve().parents[3]
        report = MODULE.build_report(root)

        self.assertFalse(report["ready"])
        self.assertEqual(report["validation_state"], "CONSTRUCTION")
        self.assertEqual(report["metrics"]["parity_rows"], 622)
        self.assertEqual(report["metrics"]["qualified_parity_rows"], 622)
        self.assertTrue(
            any("subphases are not runnable" in blocker for blocker in report["blockers"])
        )
        self.assertTrue(
            any("qualification gates are incomplete" in blocker for blocker in report["blockers"])
        )

    def test_complete_evidence_matrix_is_ready(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            write_json(
                root,
                "docs/product/manifests/phase9-qualification.v1.json",
                ready_dashboard(),
            )
            write_json(
                root,
                "docs/product/manifests/functional-parity.v1.json",
                {"rows": [{"capability_id": "test/capability"}]},
            )
            write_json(
                root,
                "docs/product/manifests/functional-parity-evidence.v1.json",
                {
                    "manifest_kind": "functional_parity_evidence",
                    "inventory": "docs/product/manifests/functional-parity.v1.json",
                    "baseline_ref": MODULE.BASELINE_REF,
                    "entries": [qualified_row()],
                },
            )

            report = MODULE.build_report(root)

        self.assertTrue(report["ready"])
        self.assertEqual(report["blockers"], [])

    def test_checkpoint_claim_with_incomplete_gate_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            dashboard = ready_dashboard()
            dashboard["qualification_gates"][0]["state"] = "NOT_RUN"
            dashboard["qualification_gates"][0]["evidence"] = None
            write_json(
                root,
                "docs/product/manifests/phase9-qualification.v1.json",
                dashboard,
            )
            write_json(
                root,
                "docs/product/manifests/functional-parity.v1.json",
                {"rows": [{"capability_id": "test/capability"}]},
            )
            write_json(
                root,
                "docs/product/manifests/functional-parity-evidence.v1.json",
                {
                    "manifest_kind": "functional_parity_evidence",
                    "inventory": "docs/product/manifests/functional-parity.v1.json",
                    "baseline_ref": MODULE.BASELINE_REF,
                    "entries": [qualified_row()],
                },
            )

            report = MODULE.build_report(root)

        self.assertFalse(report["ready"])
        self.assertIn(
            "Phase 9 claims CHECKPOINT_RUNNABLE while acceptance blockers remain",
            report["blockers"],
        )

    def test_duplicate_capability_ids_are_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            dashboard = ready_dashboard()
            dashboard["parity_ledger"]["qualified_rows"] = 2
            row = qualified_row()
            write_json(
                root,
                "docs/product/manifests/phase9-qualification.v1.json",
                dashboard,
            )
            write_json(
                root,
                "docs/product/manifests/functional-parity.v1.json",
                {"rows": [{"capability_id": "test/capability"}]},
            )
            write_json(
                root,
                "docs/product/manifests/functional-parity-evidence.v1.json",
                {
                    "manifest_kind": "functional_parity_evidence",
                    "inventory": "docs/product/manifests/functional-parity.v1.json",
                    "baseline_ref": MODULE.BASELINE_REF,
                    "entries": [row, dict(row)],
                },
            )

            report = MODULE.build_report(root)

        self.assertFalse(report["ready"])
        self.assertTrue(
            any(
                "evidence capability IDs are not unique" in blocker
                for blocker in report["blockers"]
            )
        )


if __name__ == "__main__":
    unittest.main()
