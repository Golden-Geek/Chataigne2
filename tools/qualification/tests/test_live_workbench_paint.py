from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from tools.qualification.live_workbench_paint import parse_probe_report, resolve_output_dir


def measured_report(latencies: list[float] | None = None) -> dict:
    latencies = latencies or [100.0, 200.0]
    return {
        "contract": "chataigne-live-workbench-paint-v1",
        "status": "PASS",
        "minimum_nodes": 10_000,
        "samples": 2,
        "binary_sha256": "a" * 64,
        "fixture_sha256": "b" * 64,
        "base_nodes": 10_255,
        "duplicated_roots_per_sample": 43,
        "inserted_nodes_per_sample": 602,
        "latencies_ms": latencies,
        "http_ack_ms": [60.0, 70.0],
        "mutation_ms": [75.0, 175.0],
        "p50_ms": min(latencies),
        "p95_ms": max(latencies),
        "p99_ms": max(latencies),
        "max_ms": max(latencies),
        "p95_target_ms": 250,
        "long_tasks": [],
        "browser_errors": [],
        "browser_perf_logs": [],
        "transport": {"slow_client_disconnects": 0, "overflow_resyncs": 1, "ws_snapshots": 2},
        "error": None,
    }


class LiveWorkbenchPaintTests(unittest.TestCase):
    def test_accepts_complete_result_and_explicit_budget_failure(self) -> None:
        report = measured_report()
        self.assertEqual(parse_probe_report(report, 10_000, 2, "a" * 64, "b" * 64), "PASS")
        report = measured_report([100.0, 325.0])
        report["status"] = "FAIL"
        report["error"] = "Error: p95 exceeded the provisional workbench budget"
        self.assertEqual(parse_probe_report(report, 10_000, 2, "a" * 64, "b" * 64), "BUDGET_FAIL")

    def test_rejects_wrong_fingerprint_or_missing_work(self) -> None:
        report = measured_report()
        with self.assertRaisesRegex(ValueError, "fingerprint"):
            parse_probe_report(report, 10_000, 2, "c" * 64, "b" * 64)
        report["inserted_nodes_per_sample"] = 599
        with self.assertRaisesRegex(ValueError, "600-node batch"):
            parse_probe_report(report, 10_000, 2, "a" * 64, "b" * 64)
        report["inserted_nodes_per_sample"] = 602
        report["latencies_ms"].pop()
        with self.assertRaisesRegex(ValueError, "incomplete latencies"):
            parse_probe_report(report, 10_000, 2, "a" * 64, "b" * 64)

    def test_rejects_changed_budget_or_unreported_recovery_failure(self) -> None:
        report = measured_report()
        report["p95_target_ms"] = 500
        with self.assertRaisesRegex(ValueError, "silently changed"):
            parse_probe_report(report, 10_000, 2, "a" * 64, "b" * 64)
        report["p95_target_ms"] = 250
        report["transport"]["slow_client_disconnects"] = 1
        with self.assertRaisesRegex(ValueError, "disconnected a slow browser"):
            parse_probe_report(report, 10_000, 2, "a" * 64, "b" * 64)

    def test_output_stays_under_target_and_is_new(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.assertEqual(resolve_output_dir(root, Path("target/paint")), root / "target" / "paint")
            with self.assertRaisesRegex(ValueError, "child of the workspace target"):
                resolve_output_dir(root, root)
            existing = root / "target" / "existing"
            existing.mkdir(parents=True)
            (existing / "report.json").write_text("{}", encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "already contains artifacts"):
                resolve_output_dir(root, existing)


if __name__ == "__main__":
    unittest.main()
