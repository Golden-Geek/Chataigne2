from __future__ import annotations

import unittest

from tools.migration.materialize_phase9_parity import build_entry


class Phase9ParityMaterializationTests(unittest.TestCase):
    def test_submodule_provenance_is_scaffolding_with_removed_path(self) -> None:
        row = {
            "capability_id": "submodule_ref/submodules/golden_core",
            "baseline_sources": [{"path": "submodules/golden_core"}],
            "discovered_facts": {"inventory_kind": "submodule_ref", "inventory_name": "golden_core"},
        }
        wrapper = {
            "platform": "windows",
            "commit_sha": "a" * 40,
            "tested_tree_sha": "b" * 40,
            "started_at": "2026-01-01T00:00:00Z",
            "finished_at": "2026-01-01T00:01:00Z",
            "product_report": "target/report.json",
            "product_report_sha256": "c" * 64,
            "capability_count": 1,
            "capability_ids_sha256": "d" * 64,
            "toolchain": {"target_host": "x86_64-pc-windows-msvc"},
        }
        entry = build_entry(row, wrapper, False, None)
        self.assertEqual(entry["classification"], "baseline_scaffolding")
        self.assertEqual(entry["migration_state"], "old_path_removed")
        self.assertEqual(entry["final_owner"]["path"], "crates")


if __name__ == "__main__":
    unittest.main()
