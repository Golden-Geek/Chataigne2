import copy
from pathlib import Path
import runpy
import unittest


OBSERVATION = runpy.run_path(str(Path(__file__).parents[1] / "consumer_observation.py"))
validate = OBSERVATION["validate"]
START = (1 << 63) + 13
FREQUENCY = 10_000_000


def report(mode="active"):
    window = dict(start_qpc=START, frequency=FREQUENCY, warmup_seconds=30, maximum_seconds=150)
    operations = {name: index + 1 for index, name in enumerate(OBSERVATION["OPERATIONS"])}
    final = dict(operations)
    if mode == "active":
        final["sdk_receive_calls"] += 120
    return dict(
        sample_window=window,
        workload=dict(consumer_mode=mode, gpu_stages=True, scenario="q02",
                      presentation_policy="latest", warmup_seconds=30, sample_seconds=120),
        observer=dict(
            consumer_mode=mode,
            observation_ready=dict(window=dict(window), consumer_mode=mode,
                                   completed_qpc=START + 20 * FREQUENCY,
                                   operations_at_fence=operations,
                                   completion_markers=19, skipped_completion_markers=1),
            observation_ready_received_qpc=START + 21 * FREQUENCY,
            observation_finish_qpc=START + 151 * FREQUENCY,
            checkpoints=[dict(phase=phase) for phase in
                         ("initial", "resized", "source_absent", "recovered")],
            frame_age=(dict(config=dict(window), captured_samples=120, sample_columns=[], sample_rows=[])
                       if mode == "active" else None),
            resources=dict(operations=final, completion_markers=139 if mode == "active" else 19,
                           skipped_completion_markers=3 if mode == "active" else 1),
        ),
    )


class ConsumerObservationTests(unittest.TestCase):
    def test_active_uses_the_exact_run_clock_and_preserves_input(self):
        native = report()
        before = copy.deepcopy(native)
        result = validate(native, "active")
        self.assertEqual(result, dict(clock=native["sample_window"], consumer_mode="active",
                                      quiescence_verified=True, frame_age_available=True,
                                      paused_work_proof=None))
        self.assertEqual(native, before)
        self.assertIs(type(result["clock"]["start_qpc"]), int)
        self.assertEqual(result["clock"]["start_qpc"], START)
        result["clock"]["start_qpc"] += 1
        self.assertEqual(native, before)

    def test_paused_requires_unchanged_nonzero_fence_counters_and_no_age_report(self):
        native = report("paused")
        before = copy.deepcopy(native)
        result = validate(native, "paused")
        self.assertFalse(result["frame_age_available"])
        self.assertTrue(result["quiescence_verified"])
        proof = result["paused_work_proof"]
        self.assertTrue(proof["unchanged"])
        self.assertEqual(proof["operations_at_fence"], proof["final_operations"])
        self.assertEqual(proof["final_operations"], native["observer"]["resources"]["operations"])
        self.assertEqual(proof["markers_at_fence"], proof["final_markers"])
        self.assertEqual(proof["markers_at_fence"], dict(completion_markers=19, skipped_completion_markers=1))
        self.assertEqual(proof["observation_finish_qpc"], START + 151 * FREQUENCY)
        self.assertEqual(proof["sample_window_end_qpc"], START + 150 * FREQUENCY)
        proof["final_operations"]["sdk_receive_calls"] += 1
        proof["final_markers"]["completion_markers"] += 1
        self.assertEqual(native, before)

    def test_requires_explicit_matching_modes_without_a_fallback(self):
        for mode in (None, False, 0, "", "unknown", "Paused"):
            with self.subTest(requested=mode), self.assertRaises(ValueError):
                validate(report(), mode)
        for path in (("workload",), ("observer",), ("observer", "observation_ready")):
            for value in (None, False, "paused", "unknown"):
                broken = report()
                target = broken
                for name in path:
                    target = target[name]
                target["consumer_mode"] = value
                with self.subTest(path=path, value=value), self.assertRaises(ValueError):
                    validate(broken, "active")
        broken = report()
        broken["workload"].pop("consumer_mode")
        with self.assertRaises(ValueError):
            validate(broken, "active")

    def test_paused_cannot_fabricate_an_empty_or_zero_age_report(self):
        for age in ({}, dict(config=report()["sample_window"], captured_samples=0), report()["observer"]["frame_age"]):
            broken = report("paused")
            broken["observer"]["frame_age"] = age
            with self.subTest(age=age), self.assertRaises(ValueError):
                validate(broken, "paused")
        for mode in ("active", "paused"):
            broken = report(mode)
            broken["observer"].pop("frame_age")
            with self.subTest(missing_age=mode), self.assertRaises(ValueError):
                validate(broken, mode)

    def test_active_requires_an_available_nonempty_age_report(self):
        for value in (None, {}, False):
            broken = report()
            broken["observer"]["frame_age"] = value
            with self.subTest(age=value), self.assertRaises(ValueError):
                validate(broken, "active")
        for value in (None, False, 0, -1, 1.0, 1 << 64):
            broken = report()
            broken["observer"]["frame_age"]["captured_samples"] = value
            with self.subTest(captured=value), self.assertRaises(ValueError):
                validate(broken, "active")

    def test_clock_fields_require_exact_native_unsigned_integers(self):
        for field in ("start_qpc", "frequency", "warmup_seconds", "maximum_seconds"):
            for value in (None, False, -1, 1.0, "30", 1 << 64):
                broken = report()
                broken["sample_window"][field] = value
                with self.subTest(field=field, value=value), self.assertRaises(ValueError):
                    validate(broken, "active")
        for field in ("start_qpc", "frequency"):
            broken = report()
            broken["sample_window"][field] = 0
            with self.subTest(zero=field), self.assertRaises(ValueError):
                validate(broken, "active")

    def test_window_schema_and_qpc_overflow_are_rejected(self):
        for window in (None, {}, dict(report()["sample_window"], unknown=0),
                       dict(report()["sample_window"], start_qpc=(1 << 64) - 1),
                       dict(report()["sample_window"], frequency=(1 << 64) - 1),
                       dict(report()["sample_window"], maximum_seconds=3601)):
            broken = report()
            broken["sample_window"] = window
            with self.subTest(window=window), self.assertRaises(ValueError):
                validate(broken, "active")

    def test_workload_duration_and_control_selection_must_match_the_clock(self):
        for key, value in (("warmup_seconds", 29), ("warmup_seconds", True),
                           ("warmup_seconds", 1 << 32), ("sample_seconds", 7),
                           ("sample_seconds", 119), ("sample_seconds", False),
                           ("sample_seconds", 120.0), ("sample_seconds", 1 << 32),
                           ("scenario", "diagnostic"), ("gpu_stages", 1)):
            broken = report()
            broken["workload"][key] = value
            with self.subTest(key=key, value=value), self.assertRaises(ValueError):
                validate(broken, "active")
        for key, value in (("gpu_stages", False), ("presentation_policy", "output_vblank")):
            broken = report("paused")
            broken["workload"][key] = value
            with self.subTest(paused_key=key), self.assertRaises(ValueError):
                validate(broken, "paused")
        active = report()
        active["workload"]["gpu_stages"] = False
        validate(active, "active")

    def test_acknowledgement_and_age_clocks_must_equal_the_root_clock(self):
        for owner in ("acknowledgement", "frame_age"):
            for key in ("start_qpc", "frequency", "warmup_seconds", "maximum_seconds"):
                broken = report()
                window = (broken["observer"]["observation_ready"]["window"] if owner == "acknowledgement"
                          else broken["observer"]["frame_age"]["config"])
                window[key] += 1
                with self.subTest(owner=owner, key=key), self.assertRaises(ValueError):
                    validate(broken, "active")

    def test_acknowledgement_completion_and_parent_receipt_precede_sampling(self):
        for completed, received in ((START - 1, START), (START + 2, START + 1),
                                    (START + 29 * FREQUENCY, START + 30 * FREQUENCY),
                                    (START + 30 * FREQUENCY, START + 31 * FREQUENCY)):
            broken = report()
            broken["observer"]["observation_ready"]["completed_qpc"] = completed
            broken["observer"]["observation_ready_received_qpc"] = received
            with self.subTest(completed=completed, received=received), self.assertRaises(ValueError):
                validate(broken, "active")
        for field in ("completed_qpc", "observation_ready_received_qpc"):
            for value in (None, False, -1, float(START), "123", 1 << 64):
                broken = report()
                target = broken["observer"]["observation_ready"] if field == "completed_qpc" else broken["observer"]
                target[field] = value
                with self.subTest(field=field, value=value), self.assertRaises(ValueError):
                    validate(broken, "active")
        valid = report()
        valid["observer"]["observation_ready"]["completed_qpc"] = START
        valid["observer"]["observation_ready_received_qpc"] = START
        validate(valid, "active")

    def test_readiness_acknowledgement_cannot_be_missing_or_change_schema(self):
        for value in (None, {}, dict(report()["observer"]["observation_ready"], unknown=0)):
            broken = report()
            broken["observer"]["observation_ready"] = value
            with self.subTest(ready=value), self.assertRaises(ValueError):
                validate(broken, "active")

    def test_observation_finishes_only_after_the_complete_run_window(self):
        for mode in ("active", "paused"):
            for value in (START - 1, START + 20 * FREQUENCY, START + 150 * FREQUENCY - 1,
                          None, False, -1, float(START), "123", 1 << 64):
                broken = report(mode)
                broken["observer"]["observation_finish_qpc"] = value
                with self.subTest(mode=mode, finished=value), self.assertRaises(ValueError):
                    validate(broken, mode)
            missing = report(mode)
            missing["observer"].pop("observation_finish_qpc")
            with self.subTest(mode=mode, missing=True), self.assertRaises(ValueError):
                validate(missing, mode)
            valid = report(mode)
            valid["observer"]["observation_finish_qpc"] = START + 150 * FREQUENCY
            validate(valid, mode)

    def test_resource_markers_require_exact_u32_counters_at_fence_and_finish(self):
        for mode in ("active", "paused"):
            for owner in ("observation_ready", "resources"):
                for name in ("completion_markers", "skipped_completion_markers"):
                    for value in (None, False, -1, 1.0, "1", 1 << 32):
                        broken = report(mode)
                        broken["observer"][owner][name] = value
                        with self.subTest(mode=mode, owner=owner, name=name, value=value), \
                                self.assertRaises(ValueError):
                            validate(broken, mode)
                    broken = report(mode)
                    broken["observer"][owner].pop(name)
                    with self.subTest(mode=mode, owner=owner, missing=name), self.assertRaises(ValueError):
                        validate(broken, mode)

    def test_paused_rejects_resource_marker_progress_or_regression_after_the_fence(self):
        for name in ("completion_markers", "skipped_completion_markers"):
            for change in (-1, 1):
                broken = report("paused")
                broken["observer"]["resources"][name] += change
                with self.subTest(name=name, change=change), self.assertRaises(ValueError):
                    validate(broken, "paused")
        active = report()
        validate(active, "active")
        active["observer"]["resources"]["completion_markers"] = 18
        with self.assertRaisesRegex(ValueError, "regressed"):
            validate(active, "active")

    def test_paused_rejects_any_operation_added_after_the_fence(self):
        for name in OBSERVATION["OPERATIONS"]:
            broken = report("paused")
            broken["observer"]["resources"]["operations"][name] += 1
            with self.subTest(operation=name), self.assertRaises(ValueError):
                validate(broken, "paused")

    def test_operation_fields_are_complete_exact_u64_on_both_sides(self):
        for owner in ("fence", "final"):
            for name in OBSERVATION["OPERATIONS"]:
                for value in (None, False, -1, 1.0, 1 << 64):
                    broken = report("paused")
                    operations = (broken["observer"]["observation_ready"]["operations_at_fence"] if owner == "fence"
                                  else broken["observer"]["resources"]["operations"])
                    operations[name] = value
                    with self.subTest(owner=owner, operation=name, value=value), self.assertRaises(ValueError):
                        validate(broken, "paused")
            for mutation in (lambda value: value.pop("clears"), lambda value: value.update(unknown=0)):
                broken = report("paused")
                operations = (broken["observer"]["observation_ready"]["operations_at_fence"] if owner == "fence"
                              else broken["observer"]["resources"]["operations"])
                mutation(operations)
                with self.subTest(owner=owner, schema=True), self.assertRaises(ValueError):
                    validate(broken, "paused")

    def test_requires_exactly_four_ordered_content_checkpoints(self):
        valid = report()["observer"]["checkpoints"]
        for value in (None, [], valid[:3], valid + [valid[-1]], list(reversed(valid)),
                      [None, *valid[1:]], [{}, *valid[1:]]):
            broken = report()
            broken["observer"]["checkpoints"] = value
            with self.subTest(checkpoints=value), self.assertRaises(ValueError):
                validate(broken, "active")


if __name__ == "__main__":
    unittest.main()
