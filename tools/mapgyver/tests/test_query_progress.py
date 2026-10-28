import copy
import json
from pathlib import Path
import runpy
import unittest

ROOT = Path(__file__).resolve().parents[3]
ANALYZE = runpy.run_path(str(ROOT / "tools/mapgyver/analyze-query-progress.py"))["analyze"]


class QueryProgressTests(unittest.TestCase):
    def test_retained_native_negative_controls_do_not_become_a_pass_after_drain(self):
        path = ROOT / "docs/progress/evidence/mapgyver-m01-query-progress-nvidia-2026-10-28.json"
        artifact = json.loads(path.read_text())
        result = ANALYZE(artifact)
        self.assertEqual([(r["admitted"], r["busy"]) for r in result["runs"]],
                         [(4, 476), (4, 476), (480, 0), (4, 476), (480, 0), (4, 476)])
        self.assertTrue(all(r["zero_after_fence"] for r in result["runs"]))

    def test_missing_ticks_hidden_padding_and_false_retirement_are_rejected(self):
        path = ROOT / "docs/progress/evidence/mapgyver-m01-query-progress-nvidia-2026-10-28.json"
        artifact = json.loads(path.read_text())
        for mutation in ("missing", "padding", "retained", "clock"):
            damaged = copy.deepcopy(artifact)
            report = damaged["runs"][0]["report"]
            if mutation == "missing": report["busy_ticks"].pop()
            elif mutation == "padding": report["operations_including_seed"]["clears"] += 1
            elif mutation == "retained": report["after_final_fence"]["pending"] = 1
            else: report["busy_ticks"][0]["qpc"] = 0
            with self.subTest(mutation=mutation), self.assertRaises(ValueError):
                ANALYZE(damaged)


if __name__ == "__main__":
    unittest.main()
