from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

from tools.migration import run_phase9_anode_catalog as MODULE


def anode_row(capability_id: str, type_id: str) -> dict[str, object]:
    return {
        "capability_id": capability_id,
        "discovered_facts": {
            "inventory_kind": "anode",
            "inventory_name": type_id,
        },
    }


def runtime_anode(type_id: str, label: str) -> dict[str, object]:
    return {
        "type_id": type_id,
        "label": label,
        "category": "Test",
        "execution_kind": "Pure",
        "config_fields": 0,
        "inputs": 1,
        "outputs": 1,
    }


class RunPhase9ANodeCatalogTests(unittest.TestCase):
    def test_inventory_accepts_arbitrary_exact_catalog(self) -> None:
        ledger = {
            "rows": [
                anode_row("anode/first", "first"),
                anode_row("anode/added-later", "vendor.added_later"),
            ]
        }

        records, errors = MODULE.anode_inventory(
            ledger,
            [runtime_anode("first", "First"), runtime_anode("vendor.added_later", "Added")],
        )

        self.assertEqual(errors, [])
        self.assertEqual(
            [record["capability_id"] for record in records],
            ["anode/first", "anode/added-later"],
        )

    def test_inventory_rejects_missing_untracked_and_duplicate_types(self) -> None:
        ledger = {
            "rows": [
                anode_row("anode/expected", "expected"),
                anode_row("anode/duplicate", "expected"),
            ]
        }

        _, errors = MODULE.anode_inventory(
            ledger,
            [runtime_anode("untracked", "Untracked"), runtime_anode("untracked", "Again")],
        )

        self.assertTrue(any("generated ANode rows contain duplicate" in error for error in errors))
        self.assertTrue(any("runtime ANode catalog contains duplicate" in error for error in errors))
        self.assertTrue(any("absent from the runtime catalog" in error for error in errors))
        self.assertTrue(any("no generated capability row" in error for error in errors))

    def test_parser_requires_one_structured_catalog_record(self) -> None:
        payload = {"anodes": [runtime_anode("any", "Any")]}
        output = f"noise\n{MODULE.RESULT_PREFIX}{json.dumps(payload)}\n"

        self.assertEqual(MODULE.parse_catalog_output(output), payload["anodes"])
        with self.assertRaisesRegex(ValueError, "expected one"):
            MODULE.parse_catalog_output("no structured result")
        with self.assertRaisesRegex(ValueError, "expected one"):
            MODULE.parse_catalog_output(output + output)

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
