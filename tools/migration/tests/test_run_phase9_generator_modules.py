from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from tools.migration import run_phase9_generator_modules as MODULE


def row(capability_id: str, kind: str, path: str) -> dict[str, object]:
    return {
        "capability_id": capability_id,
        "baseline_sources": [{"path": path, "line": 1}],
        "discovered_facts": {"inventory_kind": kind},
    }


class RunPhase9GeneratorModulesTests(unittest.TestCase):
    def test_inventory_is_source_and_kind_driven(self) -> None:
        ledger = {
            "rows": [
                row(
                    "module/signals-module",
                    "module",
                    "apps/chataigne/src/module/modules/generators/signals/mod.rs",
                ),
                row(
                    "script_template/signals",
                    "script_template",
                    "apps/chataigne/src/module/script_templates/signals_module.js",
                ),
                row(
                    "asset/signals",
                    "asset",
                    "apps/chataigne/src/module/modules/generators/signals/icon.svg",
                ),
                row(
                    "module/unrelated",
                    "module",
                    "apps/chataigne/src/module/modules/system/unrelated.rs",
                ),
            ]
        }

        records, errors = MODULE.generator_capability_inventory(ledger, expected_counts=None)

        self.assertTrue(any("missing required kinds" in error for error in errors))
        self.assertEqual(
            [record["capability_id"] for record in records],
            ["module/signals-module", "script_template/signals"],
        )

    def test_inventory_rejects_duplicate_ids_and_count_drift(self) -> None:
        duplicate = row(
            "module/signals-module",
            "module",
            "apps/chataigne/src/module/modules/generators/signals/mod.rs",
        )
        ledger = {"rows": [duplicate, duplicate]}

        _, errors = MODULE.generator_capability_inventory(
            ledger,
            expected_counts={"signals": 1, "metronomes": 0, "spatializer": 0},
        )

        self.assertTrue(any("duplicate capability IDs" in error for error in errors))
        self.assertTrue(any("signals generator inventory has 2 rows" in error for error in errors))

    def test_output_directory_must_stay_under_target(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "target").mkdir()

            with self.assertRaisesRegex(ValueError, "target directory"):
                MODULE.resolve_output_dir(root, root / "outside")

    def test_rust_host_uses_the_verbose_toolchain_target(self) -> None:
        self.assertEqual(
            MODULE.rust_host("rustc 1.97.0\nhost: x86_64-pc-windows-msvc\n"),
            "x86_64-pc-windows-msvc",
        )


if __name__ == "__main__":
    unittest.main()
