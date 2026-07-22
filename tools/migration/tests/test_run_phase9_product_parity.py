from __future__ import annotations

import unittest

from tools.migration.run_phase9_product_parity import validate_product_report


REQUIRED_IDS = {
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


class Phase9ProductParityTests(unittest.TestCase):
    def test_accepts_complete_product_report(self) -> None:
        validate_product_report(
            {
                "schema_version": 1,
                "gate": "chataigne-product-gate",
                "validation": "RUNNABLE",
                "overall_status": "PASS",
                "results": [
                    {"id": result_id, "status": "PASS", "required": True}
                    for result_id in REQUIRED_IDS
                ],
            }
        )

    def test_rejects_missing_real_ui_workflow(self) -> None:
        with self.assertRaisesRegex(ValueError, "missing parity checks"):
            validate_product_report(
                {
                    "schema_version": 1,
                    "gate": "chataigne-product-gate",
                    "validation": "RUNNABLE",
                    "overall_status": "PASS",
                    "results": [
                        {"id": result_id, "status": "PASS", "required": True}
                        for result_id in REQUIRED_IDS - {"e2e.ui_workflow"}
                    ],
                }
            )


if __name__ == "__main__":
    unittest.main()
