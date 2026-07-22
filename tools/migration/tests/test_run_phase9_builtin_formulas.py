from __future__ import annotations

import hashlib
import tempfile
import unittest
from pathlib import Path

from tools.migration import run_phase9_builtin_formulas as MODULE


def formula_row(capability_id: str, path: str, content: bytes) -> dict[str, object]:
    canonical_content = content.replace(b"\r\n", b"\n").replace(b"\r", b"\n")
    return {
        "capability_id": capability_id,
        "discovered_facts": {
            "file": path,
            "inventory_kind": "formula",
            "sha256": hashlib.sha256(canonical_content).hexdigest(),
        },
    }


class RunPhase9BuiltinFormulasTests(unittest.TestCase):
    def test_inventory_accepts_every_json_without_fixed_formula_names(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            formulas = root / "apps" / "chataigne" / "builtin_formulas"
            formulas.mkdir(parents=True)
            first = b'{"label":"First"}'
            added = b'{\r\n  "label":"Added Later"\r\n}'
            (formulas / "First.json").write_bytes(first)
            (formulas / "AddedLater.json").write_bytes(added)
            ledger = {
                "rows": [
                    formula_row(
                        "formula/first",
                        "apps/chataigne/builtin_formulas/First.json",
                        first,
                    ),
                    formula_row(
                        "formula/added-later",
                        "apps/chataigne/builtin_formulas/AddedLater.json",
                        added,
                    ),
                ]
            }

            records, errors = MODULE.formula_inventory(root, ledger)

        self.assertEqual(errors, [])
        self.assertEqual(
            {record["capability_id"] for record in records},
            {"formula/first", "formula/added-later"},
        )

    def test_inventory_rejects_untracked_or_digest_drifted_json(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            formulas = root / "apps" / "chataigne" / "builtin_formulas"
            formulas.mkdir(parents=True)
            content = b'{"label":"Current"}'
            (formulas / "Current.json").write_bytes(content)
            (formulas / "Untracked.json").write_text("{}", encoding="utf-8")
            ledger = {
                "rows": [
                    formula_row(
                        "formula/current",
                        "apps/chataigne/builtin_formulas/Current.json",
                        b"different",
                    )
                ]
            }

            _, errors = MODULE.formula_inventory(root, ledger)

        self.assertTrue(any("digest differs" in error for error in errors))
        self.assertTrue(any("no generated formula capability row" in error for error in errors))

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
