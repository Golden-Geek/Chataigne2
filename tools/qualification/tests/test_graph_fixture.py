from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

from tools.qualification.graph_fixture import FORMULA_LABEL, build_fixture, write_fixture


REPOSITORY_ROOT = Path(__file__).resolve().parents[3]
SOURCE_FIXTURE = REPOSITORY_ROOT / "apps/chataigne/tests/samples/test_simple_load.noisette"


class GraphFixtureTests(unittest.TestCase):
    def test_builds_deterministic_unique_graph_nodes_and_promotes_editor(self) -> None:
        source = json.loads(SOURCE_FIXTURE.read_text(encoding="utf-8"))
        first, metadata = build_fixture(source, 12)
        second, _ = build_fixture(source, 12)

        self.assertEqual(first, second)
        self.assertEqual(metadata["graphNodeCount"], 12)
        library = next(child for child in first["root"]["children"] if child["type"] == "alchemist_formula_library")
        formula = next(child for child in library["children"] if child.get("meta", {}).get("label") == FORMULA_LABEL)
        anodes = [child for child in formula["children"] if child["type"] == "alchemist_anode"]
        self.assertEqual(len(anodes), 12)
        self.assertEqual(len({node["uuid"] for node in anodes}), 12)
        self.assertEqual(anodes[0]["meta"]["label"], "Scale Constant 00001")
        self.assertEqual(anodes[-1]["meta"]["label"], "Scale Constant 00012")
        self.assertEqual(anodes[0]["meta"]["decl_id"], "scale_constant_00001")
        self.assertEqual(anodes[-1]["meta"]["decl_id"], "scale_constant_00012")
        self.assertNotIn("chataigne.formula.external.file", formula["meta"]["tags"])

        dock_layout = first["ui_state"]["dock_layout"]
        self.assertEqual(
            dock_layout["panels"]["alchemistEditor-1"]["title"],
            f"Alchemist: {FORMULA_LABEL}",
        )
        main_leaf = dock_layout["grid"]["root"]["data"][1]["data"][0]
        self.assertEqual(main_leaf["data"]["activeView"], "alchemistEditor-1")

    def test_write_fixture_reports_compact_artifact(self) -> None:
        with tempfile.TemporaryDirectory() as temporary_directory:
            output = Path(temporary_directory) / "graph-scale.noisette"
            metadata = write_fixture(SOURCE_FIXTURE, output, 3)
            self.assertEqual(metadata["graphNodeCount"], 3)
            self.assertEqual(metadata["bytes"], output.stat().st_size)
            self.assertGreater(metadata["bytes"], 0)


if __name__ == "__main__":
    unittest.main()
