import json
import tempfile
import unittest
from pathlib import Path

from tools.migration.check_phase2_contracts import dashboard_violations, runtime_boundary_violations


class Phase2ContractTests(unittest.TestCase):
    def test_raw_host_engine_mutex_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "crates/transport_server/src/bad.rs"
            source.parent.mkdir(parents=True)
            source.write_text("type Bad<T> = Arc<Mutex<Engine<T>>>;", encoding="utf-8")

            self.assertEqual(len(runtime_boundary_violations(root)), 1)

    def test_complete_dashboard_is_accepted(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            path = root / "docs/product/manifests/phase2-seams.v1.json"
            path.parent.mkdir(parents=True)
            rows = [
                {
                    "seam_id": seam,
                    "migration_state": "adapted",
                    "authoritative_path": "authoritative",
                    "facade_path": "facade",
                    "evidence": ["test"],
                }
                for seam in (
                    "project_transactions",
                    "graph_editing",
                    "runtime_values",
                    "observation",
                    "module_io",
                    "persistence",
                    "host_lifecycle",
                    "shadow_execution",
                    "io_recording",
                )
            ]
            adapter = {
                "adapter_id": "adapter",
                "owner": "owner",
                "scope": "scope",
                "authoritative_path": "path",
                "introduced_phase": "2",
                "expiry_phase": "6",
                "deletion_criteria": "criteria",
                "deletion_issue": "issue",
                "tests": ["test"],
                "side_effect_policy": "policy",
                "current_state": "active",
                "removed_in": None,
            }
            path.write_text(
                json.dumps(
                    {
                        "schema_version": 1,
                        "manifest_kind": "phase2_application_seams",
                        "rows": rows,
                        "temporary_adapters": [adapter],
                    }
                ),
                encoding="utf-8",
            )

            self.assertEqual(dashboard_violations(root), [])


if __name__ == "__main__":
    unittest.main()

