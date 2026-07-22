from __future__ import annotations

import importlib.util
import tempfile
import unittest
from pathlib import Path


SCRIPT = Path(__file__).resolve().parents[1] / "runtime_scale.py"
SPEC = importlib.util.spec_from_file_location("runtime_scale", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


def result_line(partition: str) -> str:
    return (
        'RUNTIME_SCALE_RESULT={"partition":"'
        + partition
        + '","dense_p95_us":1.0}\n'
    )


class RuntimeScaleTests(unittest.TestCase):
    def test_parse_results_requires_both_unique_partitions(self) -> None:
        output = result_line("p10000-l10") + result_line("p1000-l100")

        results = MODULE.parse_results(output)

        self.assertEqual(
            [result["partition"] for result in results],
            ["p1000-l100", "p10000-l10"],
        )

    def test_parse_results_rejects_missing_partition(self) -> None:
        with self.assertRaisesRegex(ValueError, "exactly one result"):
            MODULE.parse_results(result_line("p1000-l100"))

    def test_parse_results_rejects_duplicate_partition(self) -> None:
        output = result_line("p1000-l100") * 2 + result_line("p10000-l10")
        with self.assertRaisesRegex(ValueError, "exactly one result"):
            MODULE.parse_results(output)

    def test_output_directory_must_stay_under_target(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "target").mkdir()

            with self.assertRaisesRegex(ValueError, "child of the workspace target"):
                MODULE.resolve_output_dir(root, Path("outside"))


if __name__ == "__main__":
    unittest.main()
