from __future__ import annotations

import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[3]
COMPARATOR = ROOT / "tools" / "core" / "bench_compare.py"
FINGERPRINT = {
    "host": "test-host/x86_64/cpu-a",
    "toolchain": "rustc 1.97.0 (test)",
    "profile": "bench",
    "features": ["golden_engine/default"],
}
SCENARIOS = ("alpha", "beta")


def result_line(name: str, value: str = "100", unit: str = "ns/iter") -> str:
    return f"test {name} ... bench: {value} {unit} (+/- 5)\n"


def baseline_data() -> dict[str, Any]:
    return {
        "schema_version": 1,
        "comparison_status": "qualified",
        "qualification_note": "complete synthetic test evidence",
        "measurement_unit": "ns/iter",
        "fingerprint": FINGERPRINT,
        "expected_scenarios": list(SCENARIOS),
        "scenarios": {
            name: {
                "description": f"synthetic {name}",
                "unit": "ns/iter",
                "p50_ns": 100,
                "p95_ns": 110,
                "thresholds": {"warning_pct": 5, "failure_pct": 10},
            }
            for name in SCENARIOS
        },
    }


class BenchmarkComparisonTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.directory = Path(self.temporary.name)
        self.baseline = self.directory / "baseline.json"
        self.fingerprint = self.directory / "fingerprint.json"
        self.results = self.directory / "results.txt"
        self.output = self.directory / "summary.md"
        self.write_json(self.baseline, baseline_data())
        self.write_json(
            self.fingerprint,
            {"schema_version": 1, "fingerprint": FINGERPRINT},
        )

    def tearDown(self) -> None:
        self.temporary.cleanup()

    @staticmethod
    def write_json(path: Path, value: Any) -> None:
        path.write_text(json.dumps(value) + "\n", encoding="utf-8")

    def invoke(self) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [
                sys.executable,
                str(COMPARATOR),
                "--results",
                str(self.results),
                "--baseline",
                str(self.baseline),
                "--fingerprint",
                str(self.fingerprint),
                "--output",
                str(self.output),
            ],
            cwd=ROOT,
            capture_output=True,
            text=True,
            check=False,
        )

    def assert_invalid(self, completed: subprocess.CompletedProcess[str], phrase: str) -> None:
        self.assertEqual(completed.returncode, 2, completed.stdout + completed.stderr)
        self.assertTrue(self.output.exists())
        summary = self.output.read_text(encoding="utf-8")
        self.assertIn("Benchmark evidence invalid", summary)
        self.assertIn(phrase, summary)
        self.assertNotIn("No failure-threshold regressions detected", summary)

    def test_complete_matching_evidence_passes(self) -> None:
        self.results.write_text(
            result_line("alpha", "104") + result_line("beta", "94"),
            encoding="utf-8",
        )

        completed = self.invoke()

        self.assertEqual(completed.returncode, 0, completed.stdout + completed.stderr)
        summary = self.output.read_text(encoding="utf-8")
        self.assertIn("No failure-threshold regressions detected", summary)
        self.assertIn("improvement", summary)

    def test_real_regression_fails(self) -> None:
        self.results.write_text(
            result_line("alpha", "111") + result_line("beta"),
            encoding="utf-8",
        )

        completed = self.invoke()

        self.assertEqual(completed.returncode, 1, completed.stdout + completed.stderr)
        self.assertIn("FAIL (>10% failure threshold)", self.output.read_text(encoding="utf-8"))

    def test_missing_and_empty_files_fail(self) -> None:
        completed = self.invoke()
        self.assert_invalid(completed, "cannot read benchmark results")

        self.results.write_text("", encoding="utf-8")
        self.baseline.write_text("", encoding="utf-8")
        completed = self.invoke()
        self.assert_invalid(completed, "baseline")
        self.assertIn("empty", self.output.read_text(encoding="utf-8"))

    def test_missing_truncated_duplicate_and_extra_scenarios_fail(self) -> None:
        cases = {
            "missing": result_line("alpha"),
            "truncated": result_line("alpha") + "test beta ... bench: 10",
            "duplicate": result_line("alpha") + result_line("alpha") + result_line("beta"),
            "extra": result_line("alpha") + result_line("beta") + result_line("gamma"),
        }
        phrases = {
            "missing": "missing=['beta']",
            "truncated": "malformed benchmark measurement",
            "duplicate": "duplicate benchmark scenario",
            "extra": "extra=['gamma']",
        }
        for name, contents in cases.items():
            with self.subTest(name=name):
                self.results.write_text(contents, encoding="utf-8")
                completed = self.invoke()
                self.assert_invalid(completed, phrases[name])

    def test_malformed_values_and_units_fail(self) -> None:
        for name, line, phrase in (
            ("nonfinite", result_line("alpha", "NaN"), "malformed benchmark measurement"),
            ("zero", result_line("alpha", "0"), "positive finite"),
            ("unit", result_line("alpha", "100", "us/iter"), "expected 'ns/iter'"),
        ):
            with self.subTest(name=name):
                self.results.write_text(line + result_line("beta"), encoding="utf-8")
                completed = self.invoke()
                self.assert_invalid(completed, phrase)

    def test_baseline_schema_and_sample_contracts_fail_closed(self) -> None:
        mutations = {
            "schema": lambda data: data.update(schema_version=2),
            "duplicate_expected": lambda data: data["expected_scenarios"].append("alpha"),
            "missing_case": lambda data: data["scenarios"].pop("beta"),
            "missing_p50": lambda data: data["scenarios"]["alpha"].pop("p50_ns"),
            "nonfinite_p95": lambda data: data["scenarios"]["alpha"].update(p95_ns=float("inf")),
            "bad_thresholds": lambda data: data["scenarios"]["alpha"].update(
                thresholds={"warning_pct": 20, "failure_pct": 10}
            ),
        }
        self.results.write_text(result_line("alpha") + result_line("beta"), encoding="utf-8")
        for name, mutate in mutations.items():
            with self.subTest(name=name):
                data = baseline_data()
                mutate(data)
                self.write_json(self.baseline, data)
                completed = self.invoke()
                self.assertEqual(completed.returncode, 2, completed.stdout + completed.stderr)

    def test_every_fingerprint_dimension_must_match(self) -> None:
        self.results.write_text(result_line("alpha") + result_line("beta"), encoding="utf-8")
        replacements: dict[str, Any] = {
            "host": "other-host",
            "toolchain": "rustc other",
            "profile": "release",
            "features": ["golden_engine/other"],
        }
        for field, replacement in replacements.items():
            with self.subTest(field=field):
                fingerprint = dict(FINGERPRINT)
                fingerprint[field] = replacement
                self.write_json(
                    self.fingerprint,
                    {"schema_version": 1, "fingerprint": fingerprint},
                )
                completed = self.invoke()
                self.assert_invalid(completed, "incomparable")
                self.assertIn(field, self.output.read_text(encoding="utf-8"))

    def test_unqualified_historical_baseline_cannot_pass(self) -> None:
        data = baseline_data()
        data["comparison_status"] = "unqualified"
        data["qualification_note"] = "raw samples and exact host were not retained"
        self.write_json(self.baseline, data)
        self.results.write_text(result_line("alpha") + result_line("beta"), encoding="utf-8")

        completed = self.invoke()

        self.assert_invalid(completed, "baseline is unqualified")


if __name__ == "__main__":
    unittest.main()
