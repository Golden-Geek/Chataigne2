from __future__ import annotations

import json
import tempfile
import unittest
from contextlib import redirect_stderr
from io import StringIO
from pathlib import Path
from subprocess import CompletedProcess
from unittest.mock import patch

from tools.qualification import authored_graph_scale


def complete_output(target: int = 1_000, graph_roots: int = 72) -> str:
    row = {
        "authored_nodes": 1_088,
        "graph_roots": graph_roots,
        "minimum_live_nodes": target,
        "prepared_nodes": 1_245,
        "reloaded_nodes": 1_088,
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


def live_output(case: str, target: int = 1_000, roots: int = 10) -> str:
    prefix = authored_graph_scale.LIVE_EDIT_CASES[case][1]
    action = "duplicate" if case == "duplicate" else "remove"
    row = {
        "base_nodes": target + 100,
        f"{action}_ms": 71,
        f"{action}_tick_ms": 89,
        "undo_ms": 37,
        "undo_tick_ms": 43,
        "redo_ms": 35,
        "redo_tick_ms": 46,
    }
    if case == "duplicate":
        row.update({
            "duplicate_roots": roots,
            "inserted_nodes": roots * 14,
            **{field: [0, 1, 2] for field in authored_graph_scale.LIVE_EDIT_PHASE_FIELDS},
        })
    else:
        row.update({
            "removed_roots": roots + (case == "mixed_remove"),
            "removed_nodes": roots * 14 + (case == "mixed_remove"),
        })
    return f"{prefix}{json.dumps(row)}\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured"


class AuthoredGraphScaleTests(unittest.TestCase):
    def test_rejects_live_root_count_without_live_edit_mode(self) -> None:
        errors = StringIO()
        with redirect_stderr(errors), self.assertRaises(SystemExit) as exit_error:
            authored_graph_scale.main(["--live-edit-roots", "43"])
        self.assertEqual(exit_error.exception.code, 2)
        self.assertIn("requires --live-edits", errors.getvalue())

    def test_parses_all_live_edit_cases(self) -> None:
        for case in authored_graph_scale.LIVE_EDIT_CASES:
            with self.subTest(case=case):
                row = authored_graph_scale.parse_live_edit_result(live_output(case), case, 1_000)
                self.assertEqual(row["base_nodes"], 1_100)

    def test_requires_600_record_edit_when_requested(self) -> None:
        for case in authored_graph_scale.LIVE_EDIT_CASES:
            with self.subTest(case=case):
                output = live_output(case, target=10_000, roots=43)
                expected = 43 * 14 + (case == "mixed_remove")
                row = authored_graph_scale.parse_live_edit_result(output, case, 10_000, 43, expected)
                self.assertGreaterEqual(row["inserted_nodes" if case == "duplicate" else "removed_nodes"], 600)
                with self.assertRaisesRegex(ValueError, "expected roots"):
                    authored_graph_scale.parse_live_edit_result(output, case, 10_000, 43, expected + 1)
                with self.assertRaisesRegex(ValueError, "expected roots"):
                    authored_graph_scale.parse_live_edit_result(output, case, 10_000, 43, expected - 1)

    def test_rejects_missing_or_unverified_live_edit_evidence(self) -> None:
        for case in authored_graph_scale.LIVE_EDIT_CASES:
            output = live_output(case)
            with self.subTest(case=case):
                with self.assertRaisesRegex(ValueError, "one passing"):
                    authored_graph_scale.parse_live_edit_result(output + "\n" + output, case, 1_000)
                with self.assertRaisesRegex(ValueError, "one passing"):
                    authored_graph_scale.parse_live_edit_result(output.replace("1 passed", "0 passed"), case, 1_000)
                with self.assertRaisesRegex(ValueError, "fields differ"):
                    authored_graph_scale.parse_live_edit_result(output.replace('"undo_ms": 37, ', ""), case, 1_000)
                with self.assertRaisesRegex(ValueError, "missed the live-node target"):
                    authored_graph_scale.parse_live_edit_result(output, case, 10_000)

    def test_rejects_invalid_live_edit_counts_and_phase_series(self) -> None:
        output = live_output("duplicate")
        with self.assertRaisesRegex(ValueError, "expected roots"):
            invalid = output.replace('"duplicate_roots": 10', '"duplicate_roots": 9')
            authored_graph_scale.parse_live_edit_result(invalid, "duplicate", 1_000)
        with self.assertRaisesRegex(ValueError, "nonnegative integers"):
            invalid = output.replace('"undo_ms": 37', '"undo_ms": -1')
            authored_graph_scale.parse_live_edit_result(invalid, "duplicate", 1_000)
        with self.assertRaisesRegex(ValueError, "phase counters"):
            authored_graph_scale.parse_live_edit_result(output.replace("[0, 1, 2]", "[0, 1]", 1), "duplicate", 1_000)

    def test_live_edit_runner_marks_missing_result_as_failure(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            output_dir = root / "target" / "live"
            output_dir.mkdir(parents=True)
            completed = CompletedProcess(
                args=[], returncode=0, stdout="test result: ok. 1 passed; 0 failed;", stderr="",
            )
            with patch.object(authored_graph_scale.subprocess, "run", return_value=completed):
                row = authored_graph_scale.run_live_edit_case(root, output_dir, 1_000, {}, "duplicate", 43, 602)
            self.assertEqual(row["status"], "FAIL")
            self.assertIn("one passing", row["parse_error"])
            self.assertEqual(row["requested_roots"], 43)
            self.assertEqual(row["expected_edited_nodes"], 602)
            self.assertTrue((output_dir / "authored-1000-duplicate.log").exists())

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

    def test_rejects_live_node_count_drift(self) -> None:
        output = complete_output().replace('"reloaded_nodes": 1088', '"reloaded_nodes": 1089')
        with self.assertRaisesRegex(ValueError, "changed the live-node count"):
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
