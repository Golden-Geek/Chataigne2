from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from tools.qualification import formula_scale


def serial_line(processors: int, lanes: int) -> str:
    return (
        f"formula scale: processors={processors} lanes_per_processor={lanes} formula=Action "
        "exec_nodes=3 build_ms=10 tick_us=[100, 101, 102] kernel_ms=200 "
        "kernel_evaluations=300000 intents=800000 diagnostics=0 "
        "rss_before_mb=20 rss_built_mb=100 rss_evaluated_mb=200"
    )


def worker_line(processors: int, lanes: int, workers: int, reorder: bool) -> str:
    mode = str(reorder).lower()
    return (
        f"formula workers: processors={processors} lanes_per_processor={lanes} "
        f"reorder_contexts={mode} workers={workers} tick_us=[100, 101, 102] "
        "process_cpu_ms=[1, 2, 3] kernel_thread_ms=200 kernel_evaluations=300000 "
        "ordered_effects=200000 rss_before_mb=20 rss_after_mb=200"
    )


def unchanged_line(processors: int, lanes: int) -> str:
    return (
        f"formula unchanged: processors={processors} lanes_per_processor={lanes} "
        "tick_us=[100, 101, 102] process_cpu_ms=[1, 2, 3] "
        "kernel_thread_ms=200 kernel_evaluations=300000 "
        "intents_per_tick=[200000, 0, 0, 0] rss_mb=200"
    )


def complete_output() -> str:
    lines = []
    for processors, lanes in sorted(formula_scale.PARTITIONS):
        lines.append(serial_line(processors, lanes))
        lines.append(unchanged_line(processors, lanes))
        for reorder in (False, True):
            for workers in formula_scale.WORKERS:
                lines.append(worker_line(processors, lanes, workers, reorder))
    lines.append("test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 513 filtered out")
    return "\n".join(lines)


class FormulaScaleTests(unittest.TestCase):
    def test_parse_results_requires_every_partition_and_mode(self) -> None:
        result = formula_scale.parse_results(complete_output())

        self.assertEqual(len(result["serial"]), 2)
        self.assertEqual(len(result["workers"]), 16)
        self.assertEqual(len(result["unchanged"]), 2)
        self.assertEqual(result["workers"][0]["reorder_contexts"], False)

    def test_parse_results_rejects_missing_worker_case(self) -> None:
        output = complete_output().replace(worker_line(1_000, 100, 8, True) + "\n", "")

        with self.assertRaisesRegex(ValueError, "missing or duplicate Formula worker cases"):
            formula_scale.parse_results(output)

    def test_parse_results_rejects_duplicate_partition(self) -> None:
        output = complete_output() + "\n" + serial_line(1_000, 100)

        with self.assertRaisesRegex(ValueError, "missing or duplicate serial Formula partitions"):
            formula_scale.parse_results(output)

    def test_parse_results_rejects_missing_metric(self) -> None:
        output = complete_output().replace(" kernel_evaluations=300000", "", 1)

        with self.assertRaisesRegex(ValueError, "fields differ"):
            formula_scale.parse_results(output)

    def test_parse_results_rejects_unverified_test_count(self) -> None:
        output = complete_output().replace("8 passed", "7 passed")

        with self.assertRaisesRegex(ValueError, "exactly 8 passing"):
            formula_scale.parse_results(output)

    def test_parse_results_rejects_replayed_intents(self) -> None:
        output = complete_output().replace("[200000, 0, 0, 0]", "[200000, 1, 0, 0]", 1)

        with self.assertRaisesRegex(ValueError, "did not suppress"):
            formula_scale.parse_results(output)

    def test_parse_results_rejects_wrong_product_formula(self) -> None:
        output = complete_output().replace("formula=Action", "formula=Other", 1)

        with self.assertRaisesRegex(ValueError, "unexpected graph"):
            formula_scale.parse_results(output)

    def test_parse_results_rejects_wrong_ordered_effect_count(self) -> None:
        output = complete_output().replace("ordered_effects=200000", "ordered_effects=1", 1)

        with self.assertRaisesRegex(ValueError, "ordered effect count"):
            formula_scale.parse_results(output)

    def test_output_directory_must_be_empty_under_target(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "target").mkdir()
            with self.assertRaisesRegex(ValueError, "child of the workspace target"):
                formula_scale.resolve_output_dir(root, Path("outside"))
            output_dir = root / "target" / "existing"
            output_dir.mkdir()
            (output_dir / "keep.txt").write_text("user data", encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "already contains artifacts"):
                formula_scale.resolve_output_dir(root, output_dir)


if __name__ == "__main__":
    unittest.main()
