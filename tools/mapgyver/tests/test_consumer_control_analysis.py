import copy
from pathlib import Path
import runpy
import unittest

ANALYZER = runpy.run_path(str(Path(__file__).parents[1] / "analyze-consumer-control.py"))


def cohort():
    records = []
    summaries = []
    frequency = 10_000_000
    origin = (1 << 53) + 1
    for index, (mode, seconds) in enumerate((
        ("paused", 8), ("active", 120), ("paused", 120), ("active", 120),
    )):
        begin = origin + index * 200 * frequency
        records.append(dict(
            consumer_mode=mode,
            session_name=f"MapGyver-M01-Present-{index + 1:032x}",
            qpc_frequency=frequency,
            native_present_interval_qpc=dict(begin=begin, end=begin + (30 + seconds) * frequency),
            inputs=dict(executable_sha256="a" * 64, files={"native-source": "b" * 64}),
            adapter=dict(index=0, luid="test-intel-adapter", description="Intel", software=False),
            window_context=dict(
                editor=dict(dpi=120, client_origin_physical=[192, 192], client_extent_physical=[1375, 950]),
                native_output=dict(dpi=96, client_origin_physical=[2568, 31], client_extent_physical=[3840, 2160])),
            workload=dict(scenario="q02", exercise="timed", sample_seconds=seconds,
                          warmup_seconds=30, presentation_policy="latest", gpu_stages=True,
                          consumer_mode=mode),
        ))
        # Smoke and sustained captures retain different log sizes. The three sustained
        # controls must hold the same logical allocations despite their consumer modes.
        summaries.append(dict(consumer_held_allocations_before_drop=dict(
            live_textures=2, live_bytes_by_owner=[0, 0, 1000 if index == 0 else 2000, 0])))
    return records, summaries


class ConsumerControlAnalysisTests(unittest.TestCase):
    def test_accepts_the_predeclared_smoke_and_active_paused_active_sequence(self):
        records, summaries = cohort()
        before = copy.deepcopy((records, summaries))
        self.assertIsNone(ANALYZER["verify_cohort"](records, summaries))
        self.assertEqual((records, summaries), before)
        self.assertGreater(records[0]["native_present_interval_qpc"]["begin"], 1 << 53)
        self.assertNotEqual(records[0]["native_present_interval_qpc"],
                            records[1]["native_present_interval_qpc"])

    def test_smoke_log_size_can_differ_from_the_sustained_controls(self):
        records, summaries = cohort()
        summaries[0]["consumer_held_allocations_before_drop"]["live_bytes_by_owner"] = [0, 0, 999, 0]
        self.assertIsNone(ANALYZER["verify_cohort"](records, summaries))

    def test_requires_all_four_records_and_matching_summary_coverage(self):
        records, summaries = cohort()
        for count_records, count_summaries in ((0, 0), (3, 3), (4, 3), (3, 4)):
            with self.subTest(records=count_records, summaries=count_summaries), self.assertRaises(ValueError):
                ANALYZER["verify_cohort"](records[:count_records], summaries[:count_summaries])

    def test_swapping_controls_cannot_preserve_a_valid_cohort(self):
        for left, right in ((0, 2), (1, 2), (1, 3)):
            records, summaries = cohort()
            records[left], records[right] = records[right], records[left]
            summaries[left], summaries[right] = summaries[right], summaries[left]
            with self.subTest(left=left, right=right), self.assertRaises(ValueError):
                ANALYZER["verify_cohort"](records, summaries)

    def test_consumer_modes_must_follow_the_predeclared_order(self):
        for index in range(4):
            records, summaries = cohort()
            wrong_mode = "active" if records[index]["consumer_mode"] == "paused" else "paused"
            records[index]["consumer_mode"] = wrong_mode
            records[index]["workload"]["consumer_mode"] = wrong_mode
            with self.subTest(index=index), self.assertRaises(ValueError):
                ANALYZER["verify_cohort"](records, summaries)

    def test_smoke_and_sustained_durations_cannot_be_substituted(self):
        for index in range(4):
            for seconds in ((120, 9) if index == 0 else (8, 119)):
                records, summaries = cohort()
                records[index]["workload"]["sample_seconds"] = seconds
                with self.subTest(index=index, seconds=seconds), self.assertRaises(ValueError):
                    ANALYZER["verify_cohort"](records, summaries)

    def test_source_binary_or_adapter_drift_breaks_the_frozen_cohort(self):
        for key, replacement in (
            ("inputs", dict(executable_sha256="c" * 64, files={"native-source": "b" * 64})),
            ("inputs", dict(executable_sha256="a" * 64, files={"native-source": "c" * 64})),
            ("adapter", dict(index=1, luid="different-adapter", description="Other GPU", software=False)),
        ):
            records, summaries = cohort()
            records[2][key] = replacement
            with self.subTest(key=key, replacement=replacement), self.assertRaises(ValueError):
                ANALYZER["verify_cohort"](records, summaries)

    def test_window_context_and_qpc_frequency_must_remain_comparable(self):
        for owner, key, replacement in (
            ("editor", "dpi", 96),
            ("editor", "client_origin_physical", [200, 192]),
            ("native_output", "client_extent_physical", [1920, 1080]),
        ):
            records, summaries = cohort()
            records[3]["window_context"][owner][key] = replacement
            with self.subTest(owner=owner, key=key), self.assertRaises(ValueError):
                ANALYZER["verify_cohort"](records, summaries)
        records, summaries = cohort()
        records[1]["qpc_frequency"] += 1
        with self.assertRaises(ValueError):
            ANALYZER["verify_cohort"](records, summaries)

    def test_workload_changes_beyond_mode_and_duration_are_rejected(self):
        for name, value in (("scenario", "q01"), ("warmup_seconds", 31),
                            ("gpu_stages", False), ("presentation_policy", "output_vblank"),
                            ("exercise", "editor_isolation"), ("extra_control", True)):
            records, summaries = cohort()
            records[2]["workload"][name] = value
            with self.subTest(name=name, value=value), self.assertRaises(ValueError):
                ANALYZER["verify_cohort"](records, summaries)

    def test_intervals_must_be_strictly_ordered_without_overlap_or_touching(self):
        for index in (1, 2, 3):
            for offset in (-1, 0):
                records, summaries = cohort()
                records[index]["native_present_interval_qpc"]["begin"] = (
                    records[index - 1]["native_present_interval_qpc"]["end"] + offset)
                with self.subTest(index=index, offset=offset), self.assertRaises(ValueError):
                    ANALYZER["verify_cohort"](records, summaries)

    def test_a_capture_session_cannot_be_reused_in_another_position(self):
        for index in (1, 2, 3):
            records, summaries = cohort()
            records[index]["session_name"] = records[0]["session_name"]
            with self.subTest(index=index), self.assertRaises(ValueError):
                ANALYZER["verify_cohort"](records, summaries)

    def test_all_sustained_controls_must_retain_equal_logical_allocations(self):
        for index in (1, 2, 3):
            for name, value in (("live_textures", 3), ("live_bytes_by_owner", [0, 0, 2001, 0])):
                records, summaries = cohort()
                summaries[index]["consumer_held_allocations_before_drop"][name] = value
                with self.subTest(index=index, name=name), self.assertRaises(ValueError):
                    ANALYZER["verify_cohort"](records, summaries)


if __name__ == "__main__":
    unittest.main()
