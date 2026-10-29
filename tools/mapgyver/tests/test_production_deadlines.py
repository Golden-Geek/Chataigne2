import copy
import importlib.util
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[3]
SPEC = importlib.util.spec_from_file_location("production_deadlines", ROOT / "tools/mapgyver/production_deadlines.py")
DEADLINES = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(DEADLINES)
FREQUENCY = 1_000_000_000
ORIGIN = 1000
DEVICE = {"runtime": "renderer-device", "generation": 1}
WINDOW = {"begin_qpc": ORIGIN + FREQUENCY, "end_qpc": ORIGIN + 2 * FREQUENCY}


def schedule(index, disposition="Submitted", observed=None, frequency=FREQUENCY):
    ceil = lambda value, divisor: (value + divisor - 1) // divisor
    start, end = ceil(index * FREQUENCY, 60), ceil((index + 1) * FREQUENCY, 60)
    if observed is None:
        # Match the actual scheduler's floor conversion of an observed integral QPC tick.
        observed = ceil((start + 1000) * frequency, FREQUENCY) * FREQUENCY // frequency
    counter = lambda nanos: ORIGIN + ceil(nanos * frequency, FREQUENCY)
    return {"device": DEVICE, "token": {"device": DEVICE, "serial": index + 1} if disposition == "Submitted" else None,
            "cadence": "cadence-one", "frame_index": index, "anchor_monotonic_ns": 0,
            "slot_start_monotonic_ns": start, "slot_end_monotonic_ns": end,
            "observed_monotonic_ns": observed, "rate_numerator": 60, "rate_denominator": 1,
            "coalesced_slots": 0, "skipped_slots": 0, "epoch_generation": 0, "disposition": disposition,
            "clock": {"Qpc": {"origin_qpc": ORIGIN, "frequency": frequency, "anchor_qpc": ORIGIN,
                              "slot_start_qpc": counter(start), "slot_end_qpc": counter(end),
                              "observed_qpc_lower": counter(observed), "observed_qpc_upper": counter(observed + 1) - 1}},
            "recorded_qpc": counter(observed) + 50_000}


def fixture(frequency=FREQUENCY):
    rows = [schedule(index, frequency=frequency) for index in range(120)]
    rows.append(schedule(120, "Fence", observed=2_100_000_000, frequency=frequency))
    outputs = []
    for row in rows[:-1]:
        begin = row["clock"]["Qpc"]["slot_start_qpc"] + max(1, frequency // 100_000)
        for output in (1, 2, 3):
            outputs.append({"token": row["token"], "output": output, "outcome": "Accepted",
                            "consumer_api": {"begin": begin, "end": begin + max(1, frequency // 100_000)}})
    return rows, outputs


class SubmissionDeadlineTests(unittest.TestCase):
    def analyze(self, rows, outputs, window=WINDOW, frequency=FREQUENCY):
        return DEADLINES.analyze_deadlines(rows, outputs, window, frequency)

    def test_exact_half_open_deadline_cohort_and_api_upper_bound_do_not_prove_display(self):
        result = self.analyze(*fixture())
        self.assertTrue(result["coverage_complete"])
        self.assertEqual((result["first_frame_index"], result["last_frame_index"], result["expected_slots"]), (59, 118, 60))
        self.assertTrue(result["submission_verdict"])
        self.assertFalse(result["definite_failure_lower_bound_exceeds_budget"])
        self.assertIn("not GPU/display completion", result["scope"])

    def test_next_slot_boundary_allows_normal_start_jitter(self):
        rows, outputs = fixture()
        result = self.analyze(rows, outputs)
        self.assertEqual(result["outputs"][0]["accepted_observed_on_time"], 60)
        self.assertEqual(result["outputs"][0]["observed_miss_fraction"], 0)

    def test_late_end_only_is_not_a_failed_lower_bound(self):
        rows, outputs = fixture()
        row = rows[70]
        trace = next(trace for trace in outputs if trace["token"] == row["token"] and trace["output"] == 1)
        trace["consumer_api"]["end"] = row["clock"]["Qpc"]["slot_end_qpc"] + 100
        result = self.analyze(rows, outputs)
        self.assertGreater(result["outputs"][0]["observed_miss_fraction"], 0.001)
        self.assertEqual(result["outputs"][0]["definite_miss_fraction_lower_bound"], 0)
        self.assertIsNone(result["submission_verdict"])
        self.assertFalse(result["definite_failure_lower_bound_exceeds_budget"])

    def test_late_api_begin_exposes_a_per_output_failed_lower_bound(self):
        rows, outputs = fixture()
        row = rows[70]
        trace = next(trace for trace in outputs if trace["token"] == row["token"] and trace["output"] == 2)
        deadline = row["clock"]["Qpc"]["slot_end_qpc"]
        trace["consumer_api"] = {"begin": deadline + 1, "end": deadline + 100}
        result = self.analyze(rows, outputs)
        self.assertFalse(result["submission_verdict"])
        self.assertTrue(result["definite_failure_lower_bound_exceeds_budget"])
        self.assertEqual(result["outputs"][1]["definite_misses"], 1)
        self.assertEqual(result["outputs"][0]["definite_miss_fraction_lower_bound"], 0)

    def test_explicit_skip_ranges_and_busy_opportunities_do_not_double_count(self):
        rows, outputs = fixture()
        rows[70] = schedule(70, "Busy")
        rows[71]["skipped_slots"] = 1
        result = self.analyze(rows, outputs)
        self.assertTrue(result["coverage_complete"])
        self.assertEqual(result["outputs"][0]["unsubmitted"], 1)
        self.assertTrue(result["definite_failure_lower_bound_exceeds_budget"])

    def test_unrecorded_slots_are_unknown_not_automatically_misses(self):
        rows, outputs = fixture()
        del rows[70]
        result = self.analyze(rows, outputs)
        self.assertFalse(result["coverage_complete"])
        self.assertEqual(result["missing_schedule_slots"], [70])
        self.assertIsNone(result["submission_verdict"])
        self.assertFalse(result["definite_failure_lower_bound_exceeds_budget"])

    def test_fence_next_index_proves_unconsumed_tail_only_until_its_lower_bound(self):
        rows, outputs = fixture()
        rows = rows[:118] + [schedule(118, "Fence", observed=2_100_000_000)]
        result = self.analyze(rows, outputs)
        self.assertTrue(result["coverage_complete"])
        self.assertEqual(result["outputs"][0]["unsubmitted"], 1)
        rows[-1] = schedule(118, "Fence", observed=1_999_999_999)
        result = self.analyze(rows, outputs)
        self.assertFalse(result["mapping_available"])
        self.assertIsNone(result["submission_verdict"])

    def test_missing_output_api_interval_and_authority_change_remain_unavailable(self):
        rows, outputs = fixture()
        missing = next(trace for trace in outputs if trace["token"] == rows[70]["token"] and trace["output"] == 3)
        missing["consumer_api"] = None
        result = self.analyze(rows, outputs)
        self.assertFalse(result["coverage_complete"])
        self.assertIsNone(result["submission_verdict"])
        rows[70]["clock"] = {"Unavailable": {"reason": "AuthorityChanged"}}
        result = self.analyze(rows, outputs)
        self.assertFalse(result["mapping_available"])
        self.assertIn("unavailable", result["reason"])

    def test_changed_show_epoch_keeps_mapping_but_a_new_cadence_does_not(self):
        rows, outputs = fixture()
        rows[70]["epoch_generation"] = 2
        self.assertTrue(self.analyze(rows, outputs)["coverage_complete"])
        rows[70]["cadence"] = "another-cadence"
        self.assertFalse(self.analyze(rows, outputs)["mapping_available"])

    def test_rounding_uses_exact_nested_ceil_and_rejects_false_zero_timestamps(self):
        frequency = 10_000_000
        rows, outputs = fixture(frequency)
        window = {"begin_qpc": ORIGIN + frequency, "end_qpc": ORIGIN + 2 * frequency}
        self.assertTrue(self.analyze(rows, outputs, window, frequency)["coverage_complete"])
        damaged = copy.deepcopy(rows)
        damaged[70]["clock"]["Qpc"]["slot_end_qpc"] += 1
        self.assertFalse(self.analyze(damaged, outputs, window, frequency)["mapping_available"])
        damaged[70]["clock"] = {"Unavailable": {"reason": "InitializationUnavailable"}}
        self.assertFalse(self.analyze(damaged, outputs, window, frequency)["mapping_available"])


if __name__ == "__main__":
    unittest.main()
