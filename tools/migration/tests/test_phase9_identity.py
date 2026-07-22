from __future__ import annotations

import importlib.util
import unittest
from pathlib import Path


SCRIPT = Path(__file__).resolve().parents[1] / "phase9_identity.py"
SPEC = importlib.util.spec_from_file_location("phase9_identity", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class Phase9IdentityTests(unittest.TestCase):
    def test_current_tree_preserves_every_phase0_capability_id(self) -> None:
        root = Path(__file__).resolve().parents[3]
        report = MODULE.build_report(root)

        self.assertTrue(report["ready"])
        self.assertEqual(report["missing_capability_ids"], [])
        self.assertEqual(report["metrics"]["baseline_rows"], 583)
        self.assertEqual(report["metrics"]["current_rows"], 622)
        self.assertEqual(report["metrics"]["preserved_baseline_rows"], 583)
        self.assertEqual(report["metrics"]["new_rows"], 39)

    def test_disappearing_baseline_capability_is_rejected(self) -> None:
        baseline = {
            "rows": [
                {"capability_id": "baseline/kept"},
                {"capability_id": "baseline/missing"},
            ]
        }
        current = {
            "rows": [
                {"capability_id": "baseline/kept"},
                {"capability_id": "planned/new"},
            ]
        }

        report = MODULE.compare_documents(baseline, current)

        self.assertFalse(report["ready"])
        self.assertEqual(report["missing_capability_ids"], ["baseline/missing"])

    def test_new_capability_digest_is_order_independent(self) -> None:
        baseline = {"rows": [{"capability_id": "baseline/kept"}]}
        first = {
            "rows": [
                {"capability_id": "planned/b"},
                {"capability_id": "baseline/kept"},
                {"capability_id": "planned/a"},
            ]
        }
        second = {"rows": list(reversed(first["rows"]))}

        first_report = MODULE.compare_documents(baseline, first)
        second_report = MODULE.compare_documents(baseline, second)

        self.assertEqual(
            first_report["metrics"]["new_capability_ids_sha256"],
            second_report["metrics"]["new_capability_ids_sha256"],
        )


if __name__ == "__main__":
    unittest.main()
