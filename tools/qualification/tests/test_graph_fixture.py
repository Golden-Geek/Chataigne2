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
        original_source = json.dumps(source, sort_keys=True)
        first, metadata = build_fixture(source, 12)
        second, _ = build_fixture(source, 12)

        self.assertEqual(first, second)
        self.assertEqual(json.dumps(source, sort_keys=True), original_source)
        self.assertEqual(metadata["graphNodeCount"], 12)
        self.assertEqual(metadata["serializedRecordCount"], self._count_nodes(first["root"]))
        self.assertGreater(metadata["graphNodeSubtreeRecordCount"], 1)
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
            source = json.loads(SOURCE_FIXTURE.read_text(encoding="utf-8"))
            expected, expected_metadata = build_fixture(source, 3)
            self.assertEqual(metadata["graphNodeCount"], 3)
            self.assertEqual(metadata["serializedRecordCount"], expected_metadata["serializedRecordCount"])
            self.assertEqual(metadata["bytes"], output.stat().st_size)
            self.assertGreater(metadata["bytes"], 0)
            self.assertEqual(output.read_text(encoding="utf-8"), json.dumps(expected, separators=(",", ":")))

    def test_write_fixture_refuses_existing_output(self) -> None:
        with tempfile.TemporaryDirectory() as temporary_directory:
            output = Path(temporary_directory) / "existing.noisette"
            output.write_text("user data", encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "refusing to replace"):
                write_fixture(SOURCE_FIXTURE, output, 3)
            self.assertEqual(output.read_text(encoding="utf-8"), "user data")

    def test_minimum_live_node_hint_requires_product_verification(self) -> None:
        with tempfile.TemporaryDirectory() as temporary_directory:
            for target in (1_000, 10_000, 100_000):
                output = Path(temporary_directory) / f"authored-{target}.noisette"
                metadata = write_fixture(
                    SOURCE_FIXTURE,
                    output,
                    minimum_live_nodes=target,
                )
                self.assertEqual(metadata["minimumLiveNodeTarget"], target)
                self.assertGreaterEqual(metadata["graphNodeCount"] * metadata["graphNodeSubtreeRecordCount"], target)
                self.assertLess(metadata["graphNodeCount"], target)
                self.assertGreater(metadata["bytes"], 0)

    def test_write_fixture_requires_one_count_mode(self) -> None:
        with tempfile.TemporaryDirectory() as temporary_directory:
            output = Path(temporary_directory) / "unused.noisette"
            with self.assertRaisesRegex(ValueError, "choose exactly one"):
                write_fixture(SOURCE_FIXTURE, output)
            with self.assertRaisesRegex(ValueError, "choose exactly one"):
                write_fixture(SOURCE_FIXTURE, output, 3, minimum_live_nodes=1_000)

    @staticmethod
    def _count_nodes(node: dict) -> int:
        return 1 + sum(GraphFixtureTests._count_nodes(child) for child in node.get("children", []))


if __name__ == "__main__":
    unittest.main()
