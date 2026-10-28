import contextlib
import copy
import hashlib
import io
import json
from pathlib import Path
import runpy
import subprocess
from types import SimpleNamespace
import unittest
from unittest import mock


CAPTURE = runpy.run_path(str(Path(__file__).parents[1] / "capture-presentmon.py"))
SESSION = "MapGyver-M01-Present-" + "a" * 32
END = (1 << 63) + 11


def native_report():
    return dict(generations=[
        dict(swapchains=[dict(end_qpc=END - 10), dict(end_qpc=END - 9)]),
        dict(swapchains=[dict(end_qpc=END - 1), dict(end_qpc=END)]),
    ])


def query_report():
    return dict(
        session_name=SESSION,
        before_qpc=END + 1,
        after_qpc=END + 2,
        status=0,
        response_validation_error=None,
        statistics=dict(
            returned_session_name=SESSION,
            events_lost=0,
            log_buffers_lost=0,
            realtime_buffers_lost=0,
        ),
    )


class PresentMonCaptureTests(unittest.TestCase):
    def test_generated_output_path_requires_target_descendant_new_file_and_existing_parent(self):
        namespace = CAPTURE["parse_args"].__globals__
        root = (Path.cwd() / "fixture-capture-root").resolve()
        destination = root / "target" / "run.json"
        with mock.patch.dict(namespace, ROOT=root), \
                mock.patch.object(Path, "is_dir", return_value=True), \
                mock.patch.object(Path, "exists", return_value=False):
            args = CAPTURE["parse_args"](["0", "q01", "--output", str(destination)])
        self.assertEqual(args.output, destination.resolve())
        for destination, parent_exists, file_exists in (
            (root / "outside.json", True, False),
            (root / "target-other" / "run.json", True, False),
            (root / "target" / ".." / "outside.json", True, False),
            (root / "target" / "missing" / "run.json", False, False),
            (root / "target" / "run.json", True, True),
        ):
            with self.subTest(destination=destination), \
                    mock.patch.dict(namespace, ROOT=root), \
                    mock.patch.object(Path, "is_dir", return_value=parent_exists), \
                    mock.patch.object(Path, "exists", return_value=file_exists), \
                    contextlib.redirect_stderr(io.StringIO()):
                with self.assertRaises(SystemExit) as stopped:
                    CAPTURE["parse_args"](["0", "q01", "--output", str(destination)])
                self.assertEqual(stopped.exception.code, 2)

    def test_exclusive_packet_output_receipt_matches_exact_saved_bytes_and_large_integers(self):
        namespace = CAPTURE["main"].__globals__
        destination = mock.MagicMock(spec=Path)
        destination.__str__.return_value = "target/capture.json"
        output = io.BytesIO()
        destination.open.return_value = contextlib.nullcontext(output)
        evidence = dict(route_verified=True, role_correlation_verified=True,
                        live_etw_query_verified=True, native_ticks=END + 1)
        stdout = io.StringIO()
        with mock.patch.dict(namespace, parse_args=lambda: SimpleNamespace(output=destination),
                             collect=lambda _args, target: target.update(copy.deepcopy(evidence))), \
                contextlib.redirect_stdout(stdout):
            exit_code = CAPTURE["main"]()
        destination.open.assert_called_once_with("xb")
        payload = output.getvalue()
        receipt = json.loads(stdout.getvalue())
        self.assertEqual(exit_code, 0)
        self.assertEqual(receipt, dict(output_file="target/capture.json", bytes=len(payload),
                                      sha256=hashlib.sha256(payload).hexdigest(), route_verified=True,
                                      role_correlation_verified=True, live_etw_query_verified=True))
        self.assertTrue(payload.endswith(b"\n"))
        self.assertEqual(CAPTURE["BASE"]["decode_evidence"](json.loads(payload)), evidence)
        self.assertNotIn("payload", receipt)
        self.assertNotIn(str(END + 1), stdout.getvalue())

    def test_exclusive_open_collision_cannot_overwrite_or_emit_success_receipt(self):
        namespace = CAPTURE["main"].__globals__
        destination = mock.MagicMock(spec=Path)
        destination.open.side_effect = FileExistsError("another capture already created this file")
        stdout = io.StringIO()
        with mock.patch.dict(namespace, parse_args=lambda: SimpleNamespace(output=destination),
                             collect=lambda _args, target: target.update(route_verified=True)), \
                contextlib.redirect_stdout(stdout):
            with self.assertRaises(FileExistsError):
                CAPTURE["main"]()
        destination.open.assert_called_once_with("xb")
        self.assertEqual(stdout.getvalue(), "")

    def test_generated_failure_packet_preserves_partial_evidence_and_returns_failed_receipt(self):
        namespace = CAPTURE["main"].__globals__
        destination = mock.MagicMock(spec=Path)
        output = io.BytesIO()
        destination.open.return_value = contextlib.nullcontext(output)

        def failed_collection(_args, evidence):
            evidence["native_ticks"] = END + 1
            raise OSError("collector unavailable")

        stdout = io.StringIO()
        with mock.patch.dict(namespace, parse_args=lambda: SimpleNamespace(output=destination),
                             collect=failed_collection), contextlib.redirect_stdout(stdout):
            exit_code = CAPTURE["main"]()
        payload = output.getvalue()
        evidence = CAPTURE["BASE"]["decode_evidence"](json.loads(payload))
        receipt = json.loads(stdout.getvalue())
        self.assertEqual(exit_code, 1)
        self.assertEqual(evidence["native_ticks"], END + 1)
        self.assertEqual(evidence["preparation_or_collection_failure"], "collector unavailable")
        self.assertFalse(evidence["route_verified"])
        self.assertFalse(receipt["route_verified"])
        self.assertFalse(receipt["role_correlation_verified"])
        self.assertFalse(receipt["live_etw_query_verified"])
        self.assertEqual(receipt["bytes"], len(payload))
        self.assertEqual(receipt["sha256"], hashlib.sha256(payload).hexdigest())

    def test_stdout_default_still_emits_the_lossless_packet(self):
        namespace = CAPTURE["main"].__globals__
        evidence = dict(route_verified=True, native_ticks=END + 1)
        stdout = io.StringIO()
        with mock.patch.dict(namespace, parse_args=lambda: SimpleNamespace(output=None),
                             collect=lambda _args, target: target.update(evidence)), \
                contextlib.redirect_stdout(stdout):
            exit_code = CAPTURE["main"]()
        self.assertEqual(exit_code, 0)
        packet = json.loads(stdout.getvalue())
        self.assertEqual(packet["encoding"], "gzip-base64")
        self.assertEqual(CAPTURE["BASE"]["decode_evidence"](packet), evidence)

    def test_duration_defaults_and_total_boundary_are_admitted_before_collection(self):
        args = CAPTURE["parse_args"](["0", "q02"])
        self.assertEqual((args.sample_seconds, args.warmup_seconds), (8, 30))
        for sample, warmup in ((8, 3592), (3600, 0), (120, 30)):
            with self.subTest(sample=sample, warmup=warmup):
                args = CAPTURE["parse_args"]([
                    "0", "q01", "--sample-seconds", str(sample), "--warmup-seconds", str(warmup),
                ])
                self.assertEqual((args.sample_seconds, args.warmup_seconds), (sample, warmup))

    def test_invalid_duration_and_non_integer_arguments_are_rejected(self):
        for arguments in (
            ["--sample-seconds", "7"],
            ["--sample-seconds", "-1"],
            ["--warmup-seconds", "-1"],
            ["--sample-seconds", "8", "--warmup-seconds", "3593"],
            ["--sample-seconds", str(1 << 80)],
            ["--sample-seconds", "8.0"],
            ["--warmup-seconds", "1e2"],
        ):
            with self.subTest(arguments=arguments), contextlib.redirect_stderr(io.StringIO()):
                with self.assertRaises(SystemExit) as stopped:
                    CAPTURE["parse_args"](["0", "q01", *arguments])
                self.assertEqual(stopped.exception.code, 2)

    def test_successful_live_query_preserves_exact_qpc_and_never_claims_final_zero(self):
        observation = CAPTURE["validate_etw_query"](query_report(), SESSION, native_report())
        self.assertEqual(observation["before_qpc"], END + 1)
        self.assertEqual(observation["after_qpc"], END + 2)
        self.assertEqual(observation["native_present_interval_end_qpc"], END)
        self.assertIs(type(observation["before_qpc"]), int)
        self.assertTrue(observation["queried_after_native_present_interval"])
        self.assertTrue(observation["all_counters_zero"])
        self.assertFalse(observation["final_capture_zero_loss_verified"])
        self.assertIn("live API query", observation["scope"])

    def test_separate_nonzero_loss_counters_remain_visible_in_valid_query(self):
        for key in ("events_lost", "log_buffers_lost", "realtime_buffers_lost"):
            query = query_report()
            query["statistics"][key] = (1 << 32) - 1
            with self.subTest(counter=key):
                observation = CAPTURE["validate_etw_query"](query, SESSION, native_report())
                self.assertFalse(observation["all_counters_zero"])
                self.assertEqual(observation["counters"], {
                    name: ((1 << 32) - 1 if name == key else 0)
                    for name in ("events_lost", "log_buffers_lost", "realtime_buffers_lost")
                })
                self.assertFalse(observation["final_capture_zero_loss_verified"])

    def test_failed_api_or_unavailable_statistics_cannot_become_zero_loss(self):
        for status in (5, 4201, -1, True, "0", None):
            query = query_report()
            query["status"] = status
            with self.subTest(status=status), self.assertRaises(ValueError):
                CAPTURE["validate_etw_query"](query, SESSION, native_report())
        for query in (None, {}, dict(query_report(), statistics=None),
                      dict(query_report(), statistics={}),
                      dict(query_report(), response_validation_error="name was truncated")):
            with self.subTest(query=query), self.assertRaises(ValueError):
                CAPTURE["validate_etw_query"](query, SESSION, native_report())

    def test_loss_counters_require_available_unsigned_32_bit_integers(self):
        for key in ("events_lost", "log_buffers_lost", "realtime_buffers_lost"):
            for invalid in (-1, 1 << 32, True, 0.0, "0", None):
                query = query_report()
                query["statistics"][key] = invalid
                with self.subTest(counter=key, value=invalid), self.assertRaises(ValueError):
                    CAPTURE["validate_etw_query"](query, SESSION, native_report())
            query = query_report()
            del query["statistics"][key]
            with self.subTest(missing=key), self.assertRaises(ValueError):
                CAPTURE["validate_etw_query"](query, SESSION, native_report())

    def test_identity_mismatch_stale_query_and_invalid_qpc_are_rejected(self):
        for field in ("session_name", "returned_session_name"):
            query = query_report()
            target = query if field == "session_name" else query["statistics"]
            target[field] = SESSION[:-1] + "b"
            with self.subTest(identity=field), self.assertRaises(ValueError):
                CAPTURE["validate_etw_query"](query, SESSION, native_report())
        for before, after in ((END - 1, END + 2), (END + 2, END + 1)):
            query = dict(query_report(), before_qpc=before, after_qpc=after)
            with self.subTest(before=before, after=after), self.assertRaises(ValueError):
                CAPTURE["validate_etw_query"](query, SESSION, native_report())
        for field in ("before_qpc", "after_qpc"):
            for invalid in (True, float(END), None, 0, -1, 1 << 64):
                query = query_report()
                query[field] = invalid
                with self.subTest(field=field, value=invalid), self.assertRaises(ValueError):
                    CAPTURE["validate_etw_query"](query, SESSION, native_report())
        malformed_native = copy.deepcopy(native_report())
        malformed_native["generations"][1]["swapchains"][1]["end_qpc"] = True
        for native in (dict(generations=[]), malformed_native):
            with self.subTest(native=native), self.assertRaises(ValueError):
                CAPTURE["validate_etw_query"](query_report(), SESSION, native)

    def test_query_launch_failure_still_retains_collector_csv_and_native_result(self):
        namespace = CAPTURE["collect"].__globals__
        args = CAPTURE["parse_args"](["0", "q01"])
        native = SimpleNamespace(pid=71)
        collector = SimpleNamespace(pid=72)
        raw_csv = "unchanged collector CSV,including dropped rows\r\n"
        native_stdout = json.dumps(dict(render_process_id=71, observer=dict(frame_age=dict(config={}))))

        def completed(process, *_args):
            return dict(exit_code=0, stdout=native_stdout if process is native else raw_csv,
                        stderr="", timed_out=False, valid_utf8=True)

        # Isolate orchestration: CSV/native semantics have separate parser tests. This test
        # makes query startup fail after both pipe-draining futures have been submitted.
        def parser(path):
            if Path(path).name == "presentmon_roles.py":
                return dict(correlate=lambda *_args: dict(mapped=True))
            return dict(analyze=lambda *_args: dict(parsed=True))

        help_text = " ".join((
            "--process_id", "--session_name", "--output_stdout", "--v1_metrics", "--qpc_time",
            "--no_console_stats", "--no_track_input", "--timed", "--terminate_after_timed",
        ))
        inventory = json.dumps([dict(index=0, software=False, description="Intel Graphics")])
        preparations = [subprocess.CompletedProcess([], 0, help_text.encode(), b""),
                        subprocess.CompletedProcess([], 0, inventory.encode(), b"")]
        evidence = {}
        with mock.patch.dict(namespace, communicate=completed,
                             fingerprints=lambda *_args: dict(presentmon_sha256=CAPTURE["PRESENTMON_SHA256"]),
                             BASE=dict(compact_resources=lambda _report: None)), \
                mock.patch.object(namespace["platform"], "platform", return_value="test-platform"), \
                mock.patch.object(namespace["platform"], "machine", return_value="test-machine"), \
                mock.patch.object(namespace["runpy"], "run_path", side_effect=parser), \
                mock.patch.object(subprocess, "run", side_effect=preparations), \
                mock.patch.object(subprocess, "Popen", side_effect=[native, collector, OSError("query unavailable")]):
            result = CAPTURE["collect"](args, evidence)
        self.assertIs(result, evidence)
        self.assertEqual(evidence["native"]["stdout"], native_stdout)
        self.assertEqual(evidence["presentmon"]["stdout"], raw_csv)
        self.assertEqual(evidence["etw_query"], dict(failure="query unavailable"))
        self.assertFalse(evidence["route_verified"])
        self.assertFalse(evidence["live_etw_query_verified"])
        self.assertIn("validation_failure", evidence)
