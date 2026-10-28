import contextlib
import copy
import hashlib
import io
import json
from pathlib import Path
import runpy
import unittest
from unittest import mock

ANALYZER = runpy.run_path(str(Path(__file__).parents[1] / "analyze-presentation.py"))


def context():
    return dict(before_qpc=100, after_qpc=101, window_rect_physical=[1, 2, 100, 80],
                client_origin_physical=[2, 4], client_extent_physical=[96, 72], dpi=120,
                monitor=dict(device_name="DISPLAY1", rect_physical=[0, 0, 2560, 1600],
                             work_rect_physical=[0, 0, 2560, 1540], primary=True),
                current_mode={"Ok": dict(width_pixels=2560, height_pixels=1600,
                                         nominal_refresh_hz=240, refresh_scope="integer only")},
                coordinate_context="physical", scope="snapshot")


def record(policy, index=0):
    return dict(requested_policy=policy, session_name=f"capture-{index}",
                qpc_frequency=1000, native_present_interval_qpc=dict(begin=100 + index * 200, end=200 + index * 200),
                inputs=dict(executable_sha256="same", files={"source": "same"}),
                adapter=dict(index=0, luid="same", description="Intel", software=False),
                workload=dict(scenario="q02", exercise="timed", sample_seconds=120, warmup_seconds=30),
                window_context={"editor": ANALYZER["window_context"]({"window_at_end": {"Ok": context()}})})


class PresentationAnalysisTests(unittest.TestCase):
    def test_cohort_rejects_swapped_overlapping_or_incomparable_present_intervals(self):
        rows = [record(policy, index) for index, policy in enumerate(ANALYZER["SEQUENCE"])]
        swapped = [rows[2], rows[1], rows[0]]
        self.assertTrue(any("precedes or overlaps" in message for message in ANALYZER["verify_cohort"](swapped)))
        overlap = copy.deepcopy(rows)
        overlap[1]["native_present_interval_qpc"]["begin"] = 199
        self.assertTrue(any("precedes or overlaps" in message for message in ANALYZER["verify_cohort"](overlap)))
        different_clock = copy.deepcopy(rows)
        different_clock[1]["qpc_frequency"] = 1001
        self.assertTrue(any("QPC frequency" in message for message in ANALYZER["verify_cohort"](different_clock)))
        touching = copy.deepcopy(rows)
        touching[1]["native_present_interval_qpc"]["begin"] = 200
        self.assertEqual(ANALYZER["verify_cohort"](touching), [])

    def test_cli_returns_failure_for_incomplete_observations_with_verified_configuration(self):
        result = dict(cohort_configuration_verified=True, complete_capture_observations=False,
                      runs=[dict(native_attempts=7146)], verification_failures=["run 3: query unavailable"])
        namespace = ANALYZER["main"].__globals__
        output = io.StringIO()
        with mock.patch.dict(namespace, analyze=lambda _paths: result), \
                mock.patch("sys.argv", ["analyze-presentation.py", "a1", "b", "a2"]), \
                contextlib.redirect_stdout(output):
            status = ANALYZER["main"]()
        self.assertEqual(status, 1)
        self.assertEqual(json.loads(output.getvalue()), result)

    def test_absent_session_query_preserves_native_facts_without_inventing_zero_counters(self):
        session = "MapGyver-M01-Present-" + "a" * 32
        query = dict(session_name=session, before_qpc=200, after_qpc=201, status=4201,
                     status_message="not found", response_validation_error=None, statistics=None)
        native = dict(generations=[dict(swapchains=[dict(end_qpc=100), dict(end_qpc=101)])])
        evidence = dict(session_name=session, etw_query=dict(stdout=json.dumps(query)),
                        etw_query_report=query, presentmon=dict(stderr=""), route_verified=False,
                        live_etw_query_verified=False,
                        validation_failure="live ETW query failed or returned an invalid identity")
        live, warnings, source, failures = ANALYZER["loss_observations"](evidence, native, "CSV rows")
        self.assertFalse(live["available"])
        self.assertIsNone(live["counters"])
        self.assertIsNone(live["all_counters_zero"])
        self.assertFalse(live["final_capture_zero_loss_verified"])
        self.assertEqual(live["win32_status"], 4201)
        self.assertTrue(warnings["no_loss_reported"])
        self.assertEqual(source, "derived_from_raw_after_failed_query")
        self.assertTrue(failures)
        for key, value in (("route_verified", True), ("live_etw_query_verified", True),
                           ("validation_failure", "different failure")):
            with self.subTest(key=key), self.assertRaises(ValueError):
                ANALYZER["loss_observations"](dict(evidence, **{key: value}), native, "CSV rows")
        for status, statistics in ((5, None), (4201, {}), (True, None)):
            changed = copy.deepcopy(evidence)
            changed["etw_query_report"].update(status=status, statistics=statistics)
            changed["etw_query"]["stdout"] = json.dumps(changed["etw_query_report"])
            with self.subTest(status=status, statistics=statistics), self.assertRaises(ValueError):
                ANALYZER["loss_observations"](changed, native, "CSV rows")

    def test_incomplete_loss_query_does_not_discard_factual_cohort_runs(self):
        records = [record(policy, index) for index, policy in enumerate(ANALYZER["SEQUENCE"])]
        runs = [dict(native_attempts=7200, verification_failures=[], complete_capture_observations=True),
                dict(native_attempts=7200, verification_failures=[], complete_capture_observations=True),
                dict(native_attempts=7146, verification_failures=["query unavailable"],
                     complete_capture_observations=False)]
        namespace = ANALYZER["analyze"].__globals__
        with mock.patch.dict(namespace, analyze_capture=mock.Mock(side_effect=list(zip(records, runs)))):
            result = ANALYZER["analyze"](["a1", "b", "a2"])
        self.assertTrue(result["cohort_configuration_verified"])
        self.assertFalse(result["complete_capture_observations"])
        self.assertEqual([run["native_attempts"] for run in result["runs"]], [7200, 7200, 7146])
        self.assertEqual(result["verification_failures"], ["run 3: query unavailable"])

    def test_window_configuration_ignores_observation_time_but_retains_geometry_and_mode(self):
        first = context()
        second = dict(first, before_qpc=9007199254740999, after_qpc=9007199254741000, scope="later snapshot")
        second["current_mode"] = copy.deepcopy(first["current_mode"])
        second["current_mode"]["Ok"]["refresh_scope"] = "updated explanation"
        extracted = ANALYZER["window_context"]({"window_at_end": {"Ok": first}})
        self.assertEqual(extracted, ANALYZER["window_context"]({"window_at_end": {"Ok": second}}))
        for key, value in (("dpi", 96), ("client_origin_physical", [200, 4]),
                           ("client_extent_physical", [95, 72])):
            changed = copy.deepcopy(first)
            changed[key] = value
            self.assertNotEqual(extracted, ANALYZER["window_context"]({"window_at_end": {"Ok": changed}}))
        changed = copy.deepcopy(first)
        changed["current_mode"]["Ok"]["nominal_refresh_hz"] = 60
        self.assertNotEqual(extracted, ANALYZER["window_context"]({"window_at_end": {"Ok": changed}}))

    def test_unavailable_window_and_mode_do_not_become_comparable_configuration(self):
        for result in ({"Err": "missing"}, {}, {"Ok": dict(context(), current_mode={"Err": "unavailable"})}):
            with self.subTest(result=result), self.assertRaises(ValueError):
                ANALYZER["window_context"]({"window_at_end": result})

    def test_cohort_rejects_drift_and_wrong_sequence_without_discarding_factual_runs(self):
        rows = [record(policy, index) for index, policy in enumerate(ANALYZER["SEQUENCE"])]
        self.assertEqual(ANALYZER["verify_cohort"](rows), [])
        for key in ("inputs", "adapter", "workload", "window_context"):
            bad = copy.deepcopy(rows)
            bad[1][key] = {"changed": True} if key != "adapter" else dict(bad[1][key], luid="other")
            with self.subTest(key=key):
                self.assertTrue(ANALYZER["verify_cohort"](bad))
        bad = copy.deepcopy(rows)
        bad[1]["requested_policy"] = "latest"
        self.assertIn("latest/output-vblank/latest", ANALYZER["verify_cohort"](bad)[0])
        bad = copy.deepcopy(rows)
        bad[2]["session_name"] = bad[0]["session_name"]
        self.assertIn("reuses a capture session", ANALYZER["verify_cohort"](bad)[0])

    def test_nominal_slots_do_not_hide_missing_attempts_or_merge_overlapping_late_counts(self):
        path = dict(attempts=7197, accepted_calls=7190, busy=5, occluded=1, no_target=1, late_attempts=4)
        result = ANALYZER["submission_summary"](path, 120)
        self.assertEqual(result["nominal_frame_slots_at_60_hz"], 7200)
        self.assertEqual(result["actual_attempts_minus_nominal_slots"], -3)
        self.assertEqual((result["busy"], result["late_attempts"]), (5, 4))
        self.assertNotIn("total_misses", result)
        for invalid in (dict(path, attempts=True), dict(path, busy=-1),
                        dict(path, accepted_calls=7191), dict(path, late_attempts=7198)):
            with self.subTest(invalid=invalid), self.assertRaises(ValueError):
                ANALYZER["submission_summary"](invalid, 120)

    def test_stream_integrity_requires_exact_bytes_and_successful_complete_process(self):
        text = '{"ticks":9007199254740999}\n'
        stream = dict(exit_code=0, timed_out=False, valid_utf8=True, stdout=text, stderr="",
                      stdout_bytes=len(text.encode()), stdout_sha256=hashlib.sha256(text.encode()).hexdigest(),
                      stderr_bytes=0, stderr_sha256=hashlib.sha256(b"").hexdigest())
        ANALYZER["verify_stream"](stream, "native")
        for key, value in (("exit_code", True), ("timed_out", True), ("valid_utf8", False),
                           ("stdout_bytes", stream["stdout_bytes"] + 1), ("stdout_sha256", "wrong")):
            with self.subTest(key=key), self.assertRaises(ValueError):
                ANALYZER["verify_stream"](dict(stream, **{key: value}), "native")
