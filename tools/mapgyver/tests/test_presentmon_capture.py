import contextlib
import copy
import csv
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


def policy_correlation():
    return dict(roles=[
        dict(role="editor", begin_qpc=END, end_qpc=END + 10, raw_rows=2,
             addresses=dict(idxgi_swap_chain="0xaa", idxgi_swap_chain1="0xaa", idxgi_swap_chain2="0xaa")),
        dict(role="native_output", begin_qpc=END, end_qpc=END + 10, raw_rows=2,
             addresses=dict(idxgi_swap_chain="0xbb", idxgi_swap_chain1="0xbb", idxgi_swap_chain2="0xbb")),
    ])


def policy_rows(output_interval="0", editor_interval="0"):
    return [["0X0000BB", str(END + 1), output_interval], ["0xAA", str(END + 1), editor_interval],
            ["0xbb", str(END + 2), output_interval], ["0xaa", str(END + 2), editor_interval]]


def policy_csv(rows, headers=("SwapChainAddress", "QPCTime", "SyncInterval")):
    output = io.StringIO(newline="")
    writer = csv.writer(output)
    writer.writerow(headers)
    writer.writerows(rows)
    return output.getvalue()


class PresentMonPolicyTests(unittest.TestCase):
    def test_cli_policy_defaults_to_latest_and_accepts_only_explicit_choices(self):
        self.assertEqual(CAPTURE["parse_args"](["0", "q02"]).presentation_policy, "latest")
        for policy in ("latest", "output-vblank"):
            args = CAPTURE["parse_args"](["0", "q02", "--presentation-policy", policy])
            self.assertEqual(args.presentation_policy, policy)
        for policy in ("", "vblank", "output_vblank", "OutputVblank", "1"):
            with self.subTest(policy=policy), contextlib.redirect_stderr(io.StringIO()):
                with self.assertRaises(SystemExit) as stopped:
                    CAPTURE["parse_args"](["0", "q02", "--presentation-policy", policy])
                self.assertEqual(stopped.exception.code, 2)

    def test_exact_role_intervals_validate_both_policies_without_order_or_float_inference(self):
        for requested, native_policy, output_interval in (
            ("latest", "latest", "0"), ("output-vblank", "output_vblank", "1"),
        ):
            correlation = policy_correlation()
            correlation["roles"].reverse()
            native = dict(workload=dict(presentation_policy=native_policy))
            source = policy_csv(policy_rows(output_interval), headers=("SWAPCHAINADDRESS", "qpctime", "syncinterval"))
            with self.subTest(policy=requested):
                result = CAPTURE["validate_presentation_policy"](source, requested, native, correlation)
            self.assertEqual(result["native_policy"], native_policy)
            self.assertEqual(result["requested_policy"], requested)
            self.assertEqual(result["expected_sync_interval_by_role"],
                             dict(editor=0, native_output=int(output_interval)))
            self.assertEqual(result["rows_checked_by_role"], dict(editor=2, native_output=2))
            self.assertIn("no cadence or latency pass", result["scope"])

    def test_native_policy_mismatch_or_unknown_requested_policy_is_rejected(self):
        source = policy_csv(policy_rows())
        for requested, native_policy in (
            ("latest", "output_vblank"), ("output-vblank", "latest"),
            ("output_vblank", "output_vblank"), ("unknown", "latest"), ("latest", "unknown"),
        ):
            with self.subTest(requested=requested, native=native_policy), self.assertRaises(ValueError):
                CAPTURE["validate_presentation_policy"](
                    source, requested, dict(workload=dict(presentation_policy=native_policy)), policy_correlation())

    def test_wrong_editor_or_output_sync_interval_cannot_validate_a_requested_policy(self):
        for requested, native_policy, output_interval, editor_interval in (
            ("latest", "latest", "1", "0"),
            ("latest", "latest", "0", "1"),
            ("output-vblank", "output_vblank", "0", "0"),
            ("output-vblank", "output_vblank", "1", "1"),
            ("output-vblank", "output_vblank", "01", "0"),
            ("latest", "latest", "0.0", "0"),
            ("latest", "latest", "0", ""),
        ):
            with self.subTest(policy=requested, output=output_interval, editor=editor_interval):
                with self.assertRaises(ValueError):
                    CAPTURE["validate_presentation_policy"](
                        policy_csv(policy_rows(output_interval, editor_interval)), requested,
                        dict(workload=dict(presentation_policy=native_policy)), policy_correlation())

    def test_unknown_address_and_exact_large_qpc_lifetime_boundaries_are_enforced(self):
        for address, qpc in (("0xcc", END + 1), ("0xaa", END - 1), ("0xaa", END + 10)):
            rows = policy_rows()
            rows[1][:2] = [address, str(qpc)]
            with self.subTest(address=address, qpc=qpc), self.assertRaises(ValueError):
                CAPTURE["validate_presentation_policy"](
                    policy_csv(rows), "latest", dict(workload=dict(presentation_policy="latest")), policy_correlation())
        rows = policy_rows()
        rows[1][1] = str(END)
        result = CAPTURE["validate_presentation_policy"](
            policy_csv(rows), "latest", dict(workload=dict(presentation_policy="latest")), policy_correlation())
        self.assertEqual(result["rows_checked_by_role"], dict(editor=2, native_output=2))

    def test_ambiguous_role_identity_or_unknown_role_is_rejected(self):
        ambiguous = policy_correlation()
        ambiguous["roles"].append(copy.deepcopy(ambiguous["roles"][0]))
        unknown = policy_correlation()
        unknown["roles"][0]["role"] = "unlabeled"
        for correlation in (ambiguous, unknown):
            with self.subTest(correlation=correlation), self.assertRaises(ValueError):
                CAPTURE["validate_presentation_policy"](
                    policy_csv(policy_rows()), "latest", dict(workload=dict(presentation_policy="latest")), correlation)

    def test_required_columns_and_full_both_role_row_coverage_are_enforced(self):
        native = dict(workload=dict(presentation_policy="latest"))
        for column in range(3):
            headers = ["SwapChainAddress", "QPCTime", "SyncInterval"]
            headers[column] = "OtherColumn"
            with self.subTest(missing=column), self.assertRaises(ValueError):
                CAPTURE["validate_presentation_policy"](
                    policy_csv(policy_rows(), headers), "latest", native, policy_correlation())
        missing = policy_correlation()
        missing["roles"][0]["raw_rows"] = 3
        for rows, correlation in (
            ([], policy_correlation()), (policy_rows()[1::2], policy_correlation()),
            (policy_rows() + [policy_rows()[0]], policy_correlation()), (policy_rows(), missing),
        ):
            with self.subTest(rows=rows, correlation=correlation), self.assertRaises(ValueError):
                CAPTURE["validate_presentation_policy"](policy_csv(rows), "latest", native, correlation)


class PresentMonCaptureTests(unittest.TestCase):
    def test_gpu_stages_default_to_disabled_and_require_explicit_selection(self):
        self.assertIs(CAPTURE["parse_args"](["0", "q02"]).gpu_stages, False)
        self.assertIs(CAPTURE["parse_args"](["0", "q02", "--gpu-stages"]).gpu_stages, True)

    def test_gpu_stages_with_output_vblank_are_rejected_before_any_process(self):
        namespace = CAPTURE["main"].__globals__
        with mock.patch.object(namespace["sys"], "argv", [
                "capture-presentmon.py", "0", "q02", "--gpu-stages",
                "--presentation-policy", "output-vblank",
        ]), mock.patch.object(subprocess, "run") as run, \
                mock.patch.object(subprocess, "Popen") as launch, \
                contextlib.redirect_stderr(io.StringIO()):
            with self.assertRaises(SystemExit) as stopped:
                CAPTURE["main"]()
        self.assertEqual(stopped.exception.code, 2)
        run.assert_not_called()
        launch.assert_not_called()

    def test_native_gpu_stage_selection_and_report_presence_must_match(self):
        for requested in (False, True):
            valid = self._native_capture_report(requested)
            CAPTURE["validate_gpu_stages"](valid, requested)
            for value in (not requested, int(requested), str(requested), None):
                broken = copy.deepcopy(valid)
                broken["workload"]["gpu_stages"] = value
                with self.subTest(requested=requested, native=value), self.assertRaises(ValueError):
                    CAPTURE["validate_gpu_stages"](broken, requested)
            broken = copy.deepcopy(valid)
            broken["generations"][0]["timing"]["gpu_stages"] = None if requested else {}
            with self.subTest(requested=requested, wrong_presence=True), self.assertRaises(ValueError):
                CAPTURE["validate_gpu_stages"](broken, requested)
            broken = copy.deepcopy(valid)
            broken["generations"] = []
            with self.subTest(requested=requested, no_generations=True), self.assertRaises(ValueError):
                CAPTURE["validate_gpu_stages"](broken, requested)

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
                        presentation_policy_verified=True, live_etw_query_verified=True, native_ticks=END + 1)
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
                                      role_correlation_verified=True, presentation_policy_verified=True,
                                      live_etw_query_verified=True))
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
        self.assertFalse(receipt["presentation_policy_verified"])
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

    def test_collector_tail_defaults_and_bounded_range_are_explicit(self):
        self.assertEqual(CAPTURE["parse_args"](["0", "q02"]).collector_tail_seconds, 10)
        for tail in (2, 10, 37, 60):
            with self.subTest(tail=tail):
                args = CAPTURE["parse_args"]([
                    "0", "q01", "--sample-seconds", "3600", "--warmup-seconds", "0",
                    "--collector-tail-seconds", str(tail),
                ])
                self.assertEqual(args.collector_tail_seconds, tail)
                self.assertEqual((args.sample_seconds, args.warmup_seconds), (3600, 0))

    def test_invalid_duration_and_non_integer_arguments_are_rejected(self):
        for arguments in (
            ["--sample-seconds", "7"],
            ["--sample-seconds", "-1"],
            ["--warmup-seconds", "-1"],
            ["--sample-seconds", "8", "--warmup-seconds", "3593"],
            ["--sample-seconds", str(1 << 80)],
            ["--sample-seconds", "8.0"],
            ["--warmup-seconds", "1e2"],
            ["--collector-tail-seconds", "-1"],
            ["--collector-tail-seconds", "0"],
            ["--collector-tail-seconds", "1"],
            ["--collector-tail-seconds", "61"],
            ["--collector-tail-seconds", str(1 << 80)],
            ["--collector-tail-seconds", "10.0"],
            ["--collector-tail-seconds", "1e1"],
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

    @staticmethod
    def _native_capture_report(gpu_stages=False, policy="latest"):
        return dict(render_process_id=71, workload=dict(presentation_policy=policy, gpu_stages=gpu_stages),
                    generations=[dict(timing=dict(gpu_stages={} if gpu_stages else None))],
                    observer=dict(frame_age=dict(config={})))

    def _collect_query_launch_failure(self, arguments, native_fixture):
        namespace = CAPTURE["collect"].__globals__
        args = CAPTURE["parse_args"](arguments)
        native = SimpleNamespace(pid=71)
        collector = SimpleNamespace(pid=72)
        raw_csv = "unchanged collector CSV,including dropped rows\r\n"
        native_stdout = json.dumps(native_fixture)

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
        validate_policy = mock.Mock(return_value=dict(validated=True))
        with mock.patch.dict(namespace, communicate=completed,
                             fingerprints=lambda *_args: dict(presentmon_sha256=CAPTURE["PRESENTMON_SHA256"]),
                             BASE=dict(compact_resources=lambda _report: None),
                             validate_presentation_policy=validate_policy), \
                mock.patch.object(namespace["platform"], "platform", return_value="test-platform"), \
                mock.patch.object(namespace["platform"], "machine", return_value="test-machine"), \
                mock.patch.object(namespace["runpy"], "run_path", side_effect=parser), \
                mock.patch.object(subprocess, "run", side_effect=preparations), \
                mock.patch.object(subprocess, "Popen", side_effect=[native, collector, OSError("query unavailable")]) as launch:
            result = CAPTURE["collect"](args, evidence)
        self.assertIs(result, evidence)
        return evidence, validate_policy, launch

    def test_query_launch_failure_still_retains_collector_csv_and_native_result(self):
        native_fixture = self._native_capture_report(policy="output_vblank")
        evidence, validate_policy, launch = self._collect_query_launch_failure([
            "0", "q01", "--presentation-policy", "output-vblank", "--collector-tail-seconds", "37",
        ], native_fixture)
        raw_csv = "unchanged collector CSV,including dropped rows\r\n"
        self.assertEqual(evidence["native"]["stdout"], json.dumps(native_fixture))
        self.assertEqual(evidence["presentmon"]["stdout"], raw_csv)
        self.assertEqual(evidence["etw_query"], dict(failure="query unavailable"))
        self.assertEqual(evidence["native_command"][1:], ["qualify", "0", "q01", "8", "30", "output-vblank"])
        self.assertEqual(launch.call_args_list[0].args[0], evidence["native_command"])
        self.assertIs(evidence["requested_gpu_stages"], False)
        self.assertEqual(evidence["collector_tail_seconds"], 37)
        self.assertEqual(evidence["presentmon_timed_seconds"], 8 + 30 + 37)
        timed_index = evidence["capture_command"].index("--timed")
        self.assertEqual(evidence["capture_command"][timed_index + 1], str(8 + 30 + 37))
        validate_policy.assert_called_once_with(raw_csv, "output-vblank", native_fixture, dict(mapped=True))
        self.assertTrue(evidence["presentation_policy_verified"])
        self.assertFalse(evidence["route_verified"])
        self.assertFalse(evidence["live_etw_query_verified"])
        self.assertIn("validation_failure", evidence)

    def test_gpu_stages_select_the_exact_staged_native_command_and_record_the_flag(self):
        native_fixture = self._native_capture_report(True)
        evidence, validate_policy, launch = self._collect_query_launch_failure([
            "0", "q02", "--gpu-stages", "--sample-seconds", "120",
        ], native_fixture)
        self.assertEqual(evidence["native_command"][1:], ["qualify-staged", "0", "q02", "120", "30"])
        self.assertEqual(launch.call_args_list[0].args[0], evidence["native_command"])
        self.assertIs(evidence["requested_gpu_stages"], True)
        self.assertEqual(evidence["requested_presentation_policy"], "latest")
        self.assertTrue(evidence["presentation_policy_verified"])
        self.assertFalse(evidence["route_verified"])
        self.assertEqual(evidence["etw_query"], dict(failure="query unavailable"))
        validate_policy.assert_called_once_with(
            evidence["presentmon"]["stdout"], "latest", native_fixture, dict(mapped=True))

    def test_collection_rejects_mismatched_native_stage_selection_before_role_validation(self):
        for arguments, fixture in (
            (["0", "q02"], self._native_capture_report(True)),
            (["0", "q02", "--gpu-stages"], self._native_capture_report(False)),
        ):
            with self.subTest(arguments=arguments):
                evidence, validate_policy, _launch = self._collect_query_launch_failure(arguments, fixture)
                self.assertFalse(evidence["route_verified"])
                self.assertFalse(evidence["presentation_policy_verified"])
                self.assertEqual(evidence["validation_failure"], "requested and native GPU stage selections differ")
                self.assertEqual(evidence["native_report"], fixture)
                validate_policy.assert_not_called()
