import copy
from pathlib import Path
import runpy
import unittest

ANALYZER = runpy.run_path(str(Path(__file__).parents[1] / "gpu_stages.py"))
analyze = ANALYZER["analyze"]
PIPELINE = runpy.run_path(str(Path(__file__).parents[1] / "analyze-pipeline.py"))


def fixture(count=3):
    rows = []
    spans = []
    for index in range(count):
        start = (1 << 63) + index * 1000
        boundaries = [start, start + 1, start + 3, start + 6, start + 10, start + 15, start + 21]
        row = dict(frame=index * 2, submitted_qpc=(1 << 60) + index * 10 + 1,
                   observed_completed_qpc=(1 << 60) + index * 10 + 8,
                   frequency=1000, collected=index != 0, boundaries=boundaries)
        rows.append(row)
        spans.append([row["frame"], row["submitted_qpc"], row["observed_completed_qpc"],
                      boundaries[0], boundaries[-1], row["frequency"], row["collected"]])
    timing = dict(gpu_stages=dict(stage_names=list(ANALYZER["STAGES"]), rows=rows,
                                  skipped=0, invalid=0, pending=0, truncated=0),
                  gpu_trace=dict(columns=PIPELINE["GPU_COLUMNS"], rows=spans,
                                 skipped=0, invalid=0, pending=0, truncated=0),
                  gpu_execution=PIPELINE["distribution"]([21.0 for row in rows if row["collected"]]),
                  skipped_gpu_queries=0, invalid_gpu_queries=0, pending_gpu_queries=0, truncated_samples=0)
    return dict(timing=timing, gpu_admission=dict(submitted=count, sampled=dict(admitted=max(0, count - 1))))


def refresh_span(generation, index):
    row = generation["timing"]["gpu_stages"]["rows"][index]
    generation["timing"]["gpu_trace"]["rows"][index] = [
        row["frame"], row["submitted_qpc"], row["observed_completed_qpc"],
        row["boundaries"][0], row["boundaries"][-1], row["frequency"], row["collected"],
    ]
    spans = generation["timing"]["gpu_trace"]["rows"]
    generation["timing"]["gpu_execution"] = PIPELINE["distribution"]([
        (span[4] - span[3]) * 1000.0 / span[5] for span in spans if span[6]
    ])


class GpuStagesTests(unittest.TestCase):
    def test_exact_stage_analysis_preserves_input_and_warmup(self):
        generation = fixture()
        before = copy.deepcopy(generation)
        result = analyze(generation, True)
        self.assertEqual(generation, before)
        self.assertEqual((result["frames"], result["sampled_frames"]), (3, 2))
        self.assertEqual([stage["stage"] for stage in result["stages"]], list(ANALYZER["STAGES"]))
        for index, stage in enumerate(result["stages"]):
            self.assertEqual(stage["all"]["samples"], 3)
            self.assertEqual(stage["sampled"]["samples"], 2)
            self.assertEqual(stage["all"]["maximum_ms"], index + 1)
        longest = result["top_sampled_frames"]
        self.assertEqual([row["frame"] for row in longest], [2, 4])
        self.assertEqual(longest[0]["total_ticks"], 21)
        self.assertEqual(sum(longest[0]["stage_ticks"].values()), 21)
        self.assertEqual(longest[0]["dominant_stage"], "tail")
        self.assertGreater(longest[0]["boundaries"][0], 1 << 63)
        self.assertIn("not GPU busy time", result["scope"])

    def test_disabled_requires_explicit_null_and_exact_boolean_mode(self):
        generation = fixture()
        generation["timing"]["gpu_stages"] = None
        self.assertEqual(analyze(generation, False), {"enabled": False})
        for mutation in (lambda g: g["timing"].pop("gpu_stages"),
                         lambda g: g["timing"].update(gpu_stages={})):
            broken = copy.deepcopy(generation)
            mutation(broken)
            with self.assertRaises(ValueError):
                analyze(broken, False)
        for enabled in (0, 1, None):
            with self.subTest(enabled=enabled), self.assertRaises(ValueError):
                analyze(generation, enabled)
        with self.assertRaises(ValueError):
            analyze(generation, True)

    def test_exact_stage_schema_and_order(self):
        for mutation in (lambda t: t.update(extra=0), lambda t: t.pop("pending"),
                         lambda t: t["stage_names"].reverse(),
                         lambda t: t["stage_names"].__setitem__(0, "source_ready"),
                         lambda t: t["rows"][0].update(extra=0),
                         lambda t: t["rows"][0].pop("boundaries")):
            broken = fixture()
            mutation(broken["timing"]["gpu_stages"])
            with self.assertRaises(ValueError):
                analyze(broken, True)

    def test_rejects_nonzero_or_inexact_loss_counters_in_both_traces(self):
        for trace in ("gpu_stages", "gpu_trace"):
            for name in ("skipped", "invalid", "pending", "truncated"):
                for value in (1, -1, True, 0.0, 1 << 64):
                    broken = fixture()
                    broken["timing"][trace][name] = value
                    with self.subTest(trace=trace, name=name, value=value), self.assertRaises(ValueError):
                        analyze(broken, True)
        for name in ("skipped_gpu_queries", "invalid_gpu_queries", "pending_gpu_queries", "truncated_samples"):
            broken = fixture()
            broken["timing"][name] = 1
            with self.subTest(name=name), self.assertRaises(ValueError):
                analyze(broken, True)

    def test_rejects_identity_endpoint_frequency_and_sample_flag_mismatches(self):
        for name in ("frame", "submitted_qpc", "observed_completed_qpc", "frequency"):
            broken = fixture()
            broken["timing"]["gpu_stages"]["rows"][1][name] += 1
            with self.subTest(field=name), self.assertRaises(ValueError):
                analyze(broken, True)
        for index in (0, -1):
            broken = fixture()
            broken["timing"]["gpu_stages"]["rows"][1]["boundaries"][index] += 1
            with self.subTest(endpoint=index), self.assertRaises(ValueError):
                analyze(broken, True)
        broken = fixture()
        broken["timing"]["gpu_stages"]["rows"][1]["collected"] = False
        with self.assertRaises(ValueError):
            analyze(broken, True)

    def test_all_identity_and_tick_words_are_exact_u64_not_boolean(self):
        for name in ("frame", "submitted_qpc", "observed_completed_qpc", "frequency", "boundaries"):
            for value in (-1, True, 1.0, 1 << 64):
                broken = fixture()
                row = broken["timing"]["gpu_stages"]["rows"][1]
                if name == "boundaries":
                    row[name][2] = value
                else:
                    row[name] = value
                with self.subTest(name=name, value=value), self.assertRaises(ValueError):
                    analyze(broken, True)
        for index in range(6):
            broken = fixture()
            broken["timing"]["gpu_trace"]["rows"][0][index] = True
            with self.subTest(context_column=index), self.assertRaises(ValueError):
                analyze(broken, True)
        for trace in ("gpu_stages", "gpu_trace"):
            broken = fixture()
            if trace == "gpu_stages":
                broken["timing"][trace]["rows"][0]["collected"] = 0
            else:
                broken["timing"][trace]["rows"][0][6] = 0
            with self.assertRaises(ValueError):
                analyze(broken, True)

    def test_rejects_clock_regressions_zero_frequency_and_bad_boundary_width(self):
        for mutation in (lambda r: r.update(frequency=0), lambda r: r.update(submitted_qpc=0),
                         lambda r: r.update(observed_completed_qpc=r["submitted_qpc"] - 1),
                         lambda r: r["boundaries"].__setitem__(3, r["boundaries"][2] - 1),
                         lambda r: r["boundaries"].pop()):
            broken = fixture()
            mutation(broken["timing"]["gpu_stages"]["rows"][1])
            with self.assertRaises(ValueError):
                analyze(broken, True)

    def test_rejects_duplicate_missing_and_extra_frames_or_sampled_coverage(self):
        for mutation in (
            lambda g: g["timing"]["gpu_stages"]["rows"].__setitem__(1,
                       copy.deepcopy(g["timing"]["gpu_stages"]["rows"][0])),
            lambda g: g["timing"]["gpu_stages"]["rows"].pop(),
            lambda g: g["timing"]["gpu_trace"]["rows"].pop(),
            lambda g: g["gpu_admission"].update(submitted=True),
            lambda g: g["gpu_admission"]["sampled"].update(admitted=3),
        ):
            broken = fixture()
            mutation(broken)
            with self.assertRaises(ValueError):
                analyze(broken, True)
        broken = fixture()
        broken["timing"]["gpu_stages"]["rows"][0]["collected"] = True
        refresh_span(broken, 0)
        with self.assertRaises(ValueError):
            analyze(broken, True)

    def test_context_rows_must_remain_compact_and_have_matching_distribution(self):
        for mutation in (lambda t: t.update(columns=[]),
                         lambda t: t.update(rows=[{}]),
                         lambda t: t.update(spans=t["rows"])):
            broken = fixture()
            mutation(broken["timing"]["gpu_trace"])
            with self.assertRaises(ValueError):
                analyze(broken, True)
        broken = fixture()
        broken["timing"]["gpu_execution"]["maximum_ms"] += 1
        with self.assertRaisesRegex(ValueError, "distribution mismatch"):
            analyze(broken, True)

    def test_empty_trace_and_bound_are_explicit(self):
        empty = analyze(fixture(0), True)
        self.assertEqual(empty["frames"], 0)
        self.assertEqual(empty["top_sampled_frames"], [])
        self.assertTrue(all(stage["all"] is None and stage["sampled"] is None for stage in empty["stages"]))
        globals_ = analyze.__globals__
        previous = globals_["CAPACITY"]
        try:
            globals_["CAPACITY"] = 0
            analyze(fixture(0), True)
            with self.assertRaisesRegex(ValueError, "bound"):
                analyze(fixture(1), True)
        finally:
            globals_["CAPACITY"] = previous

    def test_zero_length_stages_and_exact_fraction_ordering(self):
        generation = fixture()
        row = generation["timing"]["gpu_stages"]["rows"][1]
        start = row["boundaries"][0]
        row.update(boundaries=[start, start, start, start, start, start, start + 1], frequency=3)
        refresh_span(generation, 1)
        result = analyze(generation, True)
        self.assertEqual(result["stages"][0]["sampled"]["median_ms"], 0)
        self.assertEqual(result["top_sampled_frames"][0]["frame"], row["frame"])
        self.assertEqual(result["top_sampled_frames"][0]["total_ms"], 1000 / 3)
        self.assertEqual(result["top_sampled_frames"][0]["dominant_stage"], "tail")

    def test_top_frames_are_ranked_before_float_rounding(self):
        generation = fixture()
        for index in (1, 2):
            row = generation["timing"]["gpu_stages"]["rows"][index]
            start = row["boundaries"][0]
            end = start + (1 << 53) + index - 1
            row["boundaries"] = [start, end, end, end, end, end, end]
            refresh_span(generation, index)
        longest = analyze(generation, True)["top_sampled_frames"]
        self.assertEqual(longest[0]["total_ms"], longest[1]["total_ms"])
        self.assertEqual([row["frame"] for row in longest], [4, 2])
        self.assertEqual(longest[0]["total_ticks"], (1 << 53) + 1)

    def test_span_join_uses_frame_identity_instead_of_row_position(self):
        generation = fixture()
        expected = analyze(generation, True)
        generation["timing"]["gpu_trace"]["rows"].reverse()
        generation["timing"]["gpu_stages"]["rows"] = [
            generation["timing"]["gpu_stages"]["rows"][index] for index in (1, 0, 2)
        ]
        self.assertEqual(analyze(generation, True), expected)

    def test_top_ten_and_nearest_rank_summaries(self):
        generation = fixture(13)
        for index, row in enumerate(generation["timing"]["gpu_stages"]["rows"]):
            start = row["boundaries"][0]
            row["boundaries"] = [start, start + index, start + index, start + index,
                                 start + index, start + index, start + index]
            refresh_span(generation, index)
        result = analyze(generation, True)
        self.assertEqual([r["frame"] for r in result["top_sampled_frames"]], list(range(24, 4, -2)))
        values = result["stages"][0]["sampled"]
        self.assertEqual(values, dict(samples=12, mean_ms=6.5, median_ms=6.0,
                                      p95_ms=12.0, p99_ms=12.0, maximum_ms=12.0))


if __name__ == "__main__":
    unittest.main()
