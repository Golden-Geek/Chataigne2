import copy
import json
from pathlib import Path
import runpy
import unittest

ANALYZER = runpy.run_path(str(Path(__file__).parents[1] / "analyze-pipeline.py"))
RUNNER = runpy.run_path(str(Path(__file__).parents[1] / "run-qualification.py"))


def timing(rows):
    values = [(r[4] - r[3]) * 1000.0 / r[5] for r in rows if r[6]]
    return dict(gpu_execution=ANALYZER["distribution"](values), skipped_gpu_queries=0, invalid_gpu_queries=0,
                pending_gpu_queries=0, truncated_samples=0,
                gpu_trace=dict(columns=ANALYZER["GPU_COLUMNS"], rows=rows, skipped=0, invalid=0, pending=0, truncated=0))


def fixture():
    source_rows = [[0, 90, 95, 1_000_000, 1_002_000, 1_000_000, False],
                   [1, 100, 105, 5_000_000, 5_003_000, 1_000_000, True]]
    render_rows = [[10, 100, 110, 10, 11, 1000, True], [11, 120, 130, 20, 21, 1000, True]]
    return dict(timing=timing(render_rows), gpu_admission=dict(submitted=2, sampled=dict(admitted=2)),
                source_workers=[dict(receive_calls=2, timing=timing(copy.deepcopy(source_rows)),
                                     gpu_admission=dict(submitted=2, sampled=dict(admitted=1))) for _ in range(2)],
                dependency_trace=dict(qpc_frequency=1000, truncated=0,
                                      columns=["renderer_frame", "source_a_batch", "source_b_batch"],
                                      rows=[[10, 0, None], [11, 0, 1]]))


class PipelineTests(unittest.TestCase):
    def test_exact_batches_repetition_absence_and_warmup_join_independent_gpu_clocks(self):
        result = ANALYZER["analyze_generation"](fixture())
        self.assertTrue(result["complete_dependency_coverage"])
        self.assertEqual(result["matched_batches"], 2)
        self.assertEqual(result["warmup_source_references"], [2, 0])
        self.assertEqual(result["repeated_source_references"], [1, 0])
        self.assertEqual(result["absent_source_references"], [0, 1])
        self.assertEqual(result["correlated_serial_gpu_span_sum"]["maximum_ms"], 6)
        self.assertEqual(result["cpu_enclosed_pipeline_upper_bound"]["maximum_ms"], 40)
        # A conservative wall bound above budget cannot establish that GPU execution failed.
        self.assertFalse(result["upper_bound_sufficient_for_gpu_target"])

    def test_missing_source_query_stays_unknown_instead_of_becoming_zero(self):
        generation = fixture()
        trace = generation["source_workers"][0]["timing"]["gpu_trace"]
        trace["rows"].pop(0)
        trace["skipped"] = 1
        result = ANALYZER["analyze_generation"](generation)
        self.assertFalse(result["complete_dependency_coverage"])
        self.assertEqual(result["missing_source_spans"], [2, 0])
        self.assertIsNone(result["cpu_enclosed_pipeline_upper_bound"])

    def test_tampered_identity_clock_coverage_and_distribution_are_rejected(self):
        for mutation in (
            lambda g: g["dependency_trace"]["rows"].pop(),
            lambda g: g["dependency_trace"]["rows"][0].__setitem__(1, 2),
            lambda g: g["timing"]["gpu_trace"]["rows"][0].__setitem__(2, 99),
            lambda g: g["timing"]["gpu_trace"]["rows"][0].__setitem__(5, 2000),
            lambda g: g["timing"]["gpu_trace"]["rows"][1].__setitem__(0, 10),
            lambda g: g["source_workers"][0]["timing"]["gpu_trace"].__setitem__("pending", 1),
        ):
            generation = fixture()
            mutation(generation)
            with self.assertRaises(ValueError):
                ANALYZER["analyze_generation"](generation)

    def test_compaction_preserves_exact_integer_clock_words_and_warmup_flag(self):
        span = dict(frame=7, submitted_qpc=2**60 + 3, observed_completed_qpc=2**60 + 9,
                    start_ticks=2**63 + 11, end_ticks=2**63 + 13, frequency=1_000_000, collected=False)
        timing = dict(gpu_trace=dict(spans=[span], skipped=2, invalid=0, pending=1, truncated=0))
        RUNNER["compact_gpu_trace"](timing)
        trace = timing["gpu_trace"]
        self.assertEqual(dict(zip(trace["columns"], trace["rows"][0])), span)
        self.assertEqual(trace["pending"], 1)

    def test_preserved_stale_binary_attempt_cannot_validate_schema_13(self):
        path = Path(__file__).parents[3] / "docs/progress/evidence/mapgyver-m01-pipeline-stale-binary-2026-10-28.json"
        packet = json.loads(path.read_text(encoding="utf-8"))["packets"][0]
        evidence = RUNNER["decode_evidence"](packet)
        release = runpy.run_path(str(Path(__file__).parents[1] / "analyze-release.py"))
        with self.assertRaisesRegex(ValueError, "schema 13 pipeline dependencies missing"):
            release["analyze_sample"](evidence)

    def test_old_combined_query_end_cannot_validate_split_stage_schema(self):
        path = Path(__file__).parents[3] / "docs/progress/evidence/mapgyver-m01-pipeline-intel-controls-2026-10-28.json"
        evidence = RUNNER["decode_evidence"](json.loads(path.read_text(encoding="utf-8"))["packets"][0])
        evidence["schema_version"] = 14
        release = runpy.run_path(str(Path(__file__).parents[1] / "analyze-release.py"))
        with self.assertRaisesRegex(ValueError, "schema 14 split submission stages missing"):
            release["analyze_sample"](evidence)

    def test_split_query_signal_and_flush_preserve_distinct_nonoverlapping_boundaries(self):
        release = runpy.run_path(str(Path(__file__).parents[1] / "analyze-release.py"))
        path = Path(__file__).parents[3] / "docs/progress/evidence/mapgyver-m01-pipeline-intel-controls-2026-10-28.json"
        evidence = RUNNER["decode_evidence"](json.loads(path.read_text(encoding="utf-8"))["packets"][0])
        generation = evidence["run"]["report"]["generations"][0]
        trace = generation["operation_timing"]
        trace["operation_names"] += ["gpu_signal", "gpu_flush"]
        trace["sampled_by_operation"] += [None, None]
        for window in (trace["warmup"], trace["sampled"]):
            log = window["outliers"]
            for event in [*log["events"], *([log["maximum"]] if log["maximum"] else [])]:
                event["spans"] += [None, None]
        event = trace["warmup"]["outliers"]["events"][0]
        # Reattribute this real retained frame's existing terminal span without changing time.
        span = event["spans"][-3]
        event["spans"][-3] = dict(offset_ns=span["offset_ns"], elapsed_ns=1)
        event["spans"][-2] = dict(offset_ns=span["offset_ns"]+1, elapsed_ns=1)
        event["spans"][-1] = dict(offset_ns=span["offset_ns"]+2, elapsed_ns=span["elapsed_ns"]-2)
        analyzed = release["verify_operations"](generation)
        self.assertEqual(len(analyzed["sampled_by_operation"]), 19)
        self.assertEqual(analyzed["warmup_outliers"]["events"][0]["spans"][-1]["elapsed_ns"], span["elapsed_ns"]-2)
        event["spans"][-1]["offset_ns"] -= 1
        with self.assertRaisesRegex(ValueError, "operation spans overlap"):
            release["verify_operations"](generation)


if __name__ == "__main__":
    unittest.main()
