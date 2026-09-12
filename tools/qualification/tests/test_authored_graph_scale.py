from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

from tools.qualification import authored_graph_scale


def complete_output(target: int = 1_000, graph_roots: int = 72) -> str:
    row = {
        "authored_nodes": 1_088,
        "graph_roots": graph_roots,
        "minimum_live_nodes": target,
        "prepared_nodes": 1_245,
        "reloaded_nodes": 1_089,
        "load_ms": 34,
        "prepare_ms": 44,
        "tick_us": [6_418, 180, 170, 175, 172],
        "tick_callbacks": [1, 0, 0, 0, 0],
        "tick_snapshot_builds": [2, 0, 0, 0, 0],
        "tick_snapshot_nodes_cloned": [2_490, 0, 0, 0, 0],
        "tick_edits_applied": [9, 0, 0, 0, 0],
        "save_ms": 13,
        "saved_bytes": 778_107,
        "reload_ms": 22,
        "load_rss_mb": 21,
        "prepare_rss_mb": 25,
        "reload_rss_mb": 26,
    }
    return (
        f"AUTHORED_SCALE_RESULT={json.dumps(row)}\n"
        "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 513 filtered out"
    )


class AuthoredGraphScaleTests(unittest.TestCase):
    def test_parses_one_complete_product_result(self) -> None:
        row = authored_graph_scale.parse_result(complete_output(), 1_000, 72)
        self.assertEqual(row["authored_nodes"], 1_088)

    def test_rejects_duplicate_or_unverified_result(self) -> None:
        output = complete_output()
        with self.assertRaisesRegex(ValueError, "one passing"):
            authored_graph_scale.parse_result(output + "\n" + output, 1_000, 72)
        with self.assertRaisesRegex(ValueError, "one passing"):
            authored_graph_scale.parse_result(output.replace("1 passed", "0 passed"), 1_000, 72)

    def test_rejects_missing_measurement_and_missed_threshold(self) -> None:
        output = complete_output()
        with self.assertRaisesRegex(ValueError, "fields differ"):
            authored_graph_scale.parse_result(output.replace('"tick_us": [6418, 180, 170, 175, 172], ', ""), 1_000, 72)
        with self.assertRaisesRegex(ValueError, "missed the live-node target"):
            authored_graph_scale.parse_result(complete_output(target=10_000), 10_000, 72)

    def test_rejects_incomplete_tick_series(self) -> None:
        output = complete_output().replace("[6418, 180, 170, 175, 172]", "[6418, 180]")
        with self.assertRaisesRegex(ValueError, "five nonnegative"):
            authored_graph_scale.parse_result(output, 1_000, 72)

    def test_output_directory_must_be_empty_under_target(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "target").mkdir()
            with self.assertRaisesRegex(ValueError, "child of the workspace target"):
                authored_graph_scale.resolve_output_dir(root, Path("outside"))
            output_dir = root / "target" / "existing"
            output_dir.mkdir()
            (output_dir / "keep.txt").write_text("user data", encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "already contains artifacts"):
                authored_graph_scale.resolve_output_dir(root, output_dir)


if __name__ == "__main__":
    unittest.main()
