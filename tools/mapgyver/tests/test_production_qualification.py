import copy
import contextlib
import importlib.util
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[3]
SPEC = importlib.util.spec_from_file_location("production_qualification", ROOT / "tools/mapgyver/production_qualification.py")
ANALYZER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(ANALYZER)
FREQUENCY = 60_000
BEGIN = 60_000
END = 120_000
ADAPTER = {"low": 123, "high": 0}
MIB = 1024 * 1024


def token(identity, serial):
    return {"device": {"runtime": identity, "generation": 0}, "serial": serial}


def wall(begin, end):
    return {"begin": begin, "end": end}


def event(kind, value):
    return {"kind": "diagnostic", "event": {kind: value}}


def gpu(identity, context):
    return event("Gpu", {"token": identity, "context": context, "status": "Valid",
                         "begin": 100, "end": 200, "frequency": 1_000_000})


def accounting(rows):
    count = sum(row["kind"] == "diagnostic" for row in rows)
    return {"enabled": True, "queued": 0, "written": count, "drained": count,
            "dropped_full": 0, "clock_failures": 0}


def write_rows(path, rows):
    path.write_text("".join(json.dumps(row) + "\n" for row in rows), encoding="utf-8")


def write_capture(directory):
    directory.mkdir()
    descriptor = lambda width, height: {"extent": {"width": width, "height": height}, "format": "Bgra8Unorm",
                                        "color_encoding": "Srgb", "alpha_mode": "Straight"}
    sources = [[{"kind": "source_config", "pid": 101 + index, "qpc_frequency": FREQUENCY,
                 "source": {"instance": f"producer-{index}", "adapter": ADAPTER,
                            "descriptor": descriptor(*( (1920, 1080) if index == 0 else (1280, 720)))}}]
               for index in range(2)]
    consumer = [{"kind": "consumer_config", "pid": 103, "qpc_frequency": FREQUENCY,
                 "consumer": {"instance": "consumer"}}]
    renderer = [{"kind": "capture", "process_id": 100, "adapter": ADAPTER, "scenario": "Q01",
                 "qpc_frequency": FREQUENCY, "requested_warmup_seconds": 0,
                 "requested_sample_seconds": 1, "requested_independence_seconds": 0}]
    for frame in range(1, 61):
        start = BEGIN + (frame - 1) * 1000
        native_token = token("renderer", frame)
        renderer.append(event("FrameCpu", {"token": native_token, "wall": wall(start + 10, start + 40),
                                            "thread_cpu": {"kernel_100ns": 10, "user_100ns": 30}}))
        renderer.append(gpu(native_token, "Renderer"))
        for output in (1, 2, 3):
            renderer.append(event("Output", {"token": native_token, "output": output, "outcome": "Accepted",
                                              "delivery": wall(start + 20, start + 30), "consumer_api": wall(start + 22, start + 25)}))
        stamps, words = [], []
        for index in range(2):
            source = index + 1
            stamp = {"source": source, "frame": frame, "qpc": start - 20}
            # The analyzer compares Rust-supplied canonical words. This synthetic oracle is
            # deliberately opaque; the Python test does not introduce a second checksum encoder.
            canonical = [source, frame, 0, start - 20, 0, 0x12345678]
            stamps.append(stamp)
            words.extend(canonical)
            producer = token(f"producer-{index}", frame)
            sources[index].append({"kind": "source_frame", "submission": {"token": producer, "stamp": stamp,
                                                                          "outcome": "Accepted"}, "marker_words": canonical})
            context = {"FixtureProducer": {"id": f"producer-{index}"}}
            sources[index].extend([event("SourceCpu", {"token": producer, "context": context, "wall": wall(start - 20, start),
                                                       "thread_cpu": None}), gpu(producer, context)])
            worker = token(f"source-worker-{index}", frame)
            source_context = {"Source": {"texture": source, "renderer_device": native_token["device"], "binding": str(index)}}
            renderer.extend([event("SourceCpu", {"token": worker, "context": source_context,
                                                   "wall": wall(start, start + 5), "thread_cpu": None}),
                             gpu(worker, source_context),
                             event("SourceUse", {"token": native_token, "texture": source, "sequence": frame,
                                                  "publication": {"source_token": worker}})])
        capture = token("consumer", frame)
        consumer.append({"kind": "consumer_frame", "frame": {"row": frame - 1, "token": capture,
                         "receive_begin_qpc": start + 80, "completion_observed_qpc": start + 100,
                         "raw_words": words, "left": {"Ok": stamps[0]}, "right": {"Ok": stamps[1]}}})
        context = {"FixtureConsumer": {"id": "consumer"}}
        consumer.extend([event("SourceCpu", {"token": capture, "context": context, "wall": wall(start + 80, start + 90),
                                               "thread_cpu": None}), gpu(capture, context)])
    for rows in sources:
        rows.append({"kind": "source_final", "diagnostics": accounting(rows)})
    consumer.append({"kind": "consumer_final", "diagnostics": accounting(consumer),
                     "consumer": {"final_readbacks": 1, "finished": True, "overflow": 0, "mismatch": 0,
                                  "failed_receives": 0, "device_removed": False}})
    renderer.append({"kind": "adapter_memory", "qpc": BEGIN + 500, "snapshot": {
        "adapter": ADAPTER, "process_id": 100, "qpc_frequency": FREQUENCY,
        "local": {"budget_bytes": 2048 * MIB, "current_usage_bytes": 10 * MIB}}})
    for coordinate in (BEGIN + 500, END + 800):
        renderer.append({"kind": "process_resources", "snapshot": {"process_id": 100, "observed_qpc": coordinate,
                         "memory": {"private_bytes": 150 * MIB}, "handle_count": 100,
                         "times": {"creation_filetime_100ns": 111}}})
    manifest = {"schema_version": 2, "run_id": "first", "scenario": "Q01", "pid": 100,
                "fixture_pids": [101, 102, 103], "adapter": ADAPTER, "warmup_seconds": 0, "sample_seconds": 1,
                "independence_seconds": 0, "debug_assertions": False, "arch": "x86_64", "qpc_frequency": FREQUENCY}
    summary = {"process_id": 100, "phase_windows": [{"name": "warmup", "begin_qpc": BEGIN, "end_qpc": BEGIN},
                                  {"name": "sample", "begin_qpc": BEGIN, "end_qpc": END}],
               "diagnostics": accounting(renderer), "fixture_report_loss": 0,
               "geometry": {"views": [{"physical_size": [640, 360]}, {"physical_size": [480, 270]}]},
               "window": {"backing": [1920, 1080], "client": [1920, 1080]},
               "active_runtime": {"state": "Running", "failure": "historical backend error"},
               "final_runtime": {"state": "Stopped", "resources": {"bytes_used": 0, "in_flight": 0,
                                  "peak_bytes": 128 * MIB, "byte_limit": 512 * MIB}},
               "retirement": {"begin_qpc": END + 100, "actual_retirement": True,
                              "both_hwnds_retained_until_retirement": True, "output_destroyed_after_retirement": True},
               "functional_completed": True}
    parent = {"schema_version": 2, "renderer_returned_ok": True, "qpc": END + 900,
              "children": [{"pid": pid, "success": True, "forced": False} for pid in (101, 102, 103)]}
    launcher = {"exit_code": 0, "timed_out": False, "forced_cleanup": False,
                "owned_job_active_before_close": 0, "exit_observed_qpc": END + 1000, "qpc_frequency": FREQUENCY}
    for name, value in [("manifest.json", manifest), ("renderer-summary.json", summary),
                        ("parent-summary.json", parent), ("launcher.json", launcher),
                        ("inputs.json", {"before": {"executable_sha256": "same"}, "after": {"executable_sha256": "same"}})]:
        (directory / name).write_text(json.dumps(value), encoding="utf-8")
    for name, rows in zip(("renderer", "source-1", "source-2", "consumer"), (renderer, *sources, consumer)):
        write_rows(directory / (name + ".jsonl"), rows)
    return renderer, sources, consumer


class ProductionQualificationTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.directory = Path(self.temporary.name) / "capture"
        self.renderer, self.sources, self.consumer = write_capture(self.directory)

    def analyze(self):
        return ANALYZER.analyze_capture(self.directory)

    def test_complete_short_capture_stays_diagnostic_and_contexts_never_become_critical_path(self):
        result = self.analyze()
        self.assertFalse(result["prescribed_sample"])
        self.assertTrue(result["renderer_gpu"]["complete"])
        self.assertTrue(result["dependencies"]["complete"])
        self.assertEqual(result["criteria"]["gpu_critical_path"]["status"], "UNAVAILABLE")
        self.assertEqual(result["frame_age"]["channels"][0]["frame_age"]["p95_ms"], 2)
        self.assertEqual(result["outputs"][0]["counts"]["Accepted"], 60)
        self.assertEqual(result["criteria"]["functional_completion"]["status"], "PASSED_NATIVE")
        self.assertFalse(ANALYZER.analyze_series([self.directory])["qualified"])

    def test_raw_marker_corruption_is_retained_even_if_rust_decoded_stamp_was_tampered(self):
        frame = next(row["frame"] for row in self.consumer if row["kind"] == "consumer_frame")
        frame["raw_words"][5] ^= 1
        write_rows(self.directory / "consumer.jsonl", self.consumer)
        invalid = self.analyze()["frame_age"]["channels"][0]["invalid"]
        self.assertEqual(len(invalid), 1)
        self.assertIn("canonical", invalid[0]["reason"])
        self.assertIn("raw_words", invalid[0])

    def test_missing_producer_frame_future_completion_and_rust_errors_are_not_zero_age(self):
        rows = [row["frame"] for row in self.consumer if row["kind"] == "consumer_frame"]
        rows[0]["left"] = {"Err": "Checksum"}
        rows[1]["left"]["Ok"]["frame"] = 999
        rows[2]["completion_observed_qpc"] = 1
        write_rows(self.directory / "consumer.jsonl", self.consumer)
        channel = self.analyze()["frame_age"]["channels"][0]
        self.assertEqual(len(channel["invalid"]), 3)
        self.assertEqual(channel["valid"], 57)

    def test_duplicate_diagnostic_identity_and_incomplete_stream_are_rejected(self):
        first = next(row for row in self.renderer if row["kind"] == "diagnostic")
        write_rows(self.directory / "renderer.jsonl", [*self.renderer, first])
        with self.assertRaises(ANALYZER.ArtifactError):
            self.analyze()
        write_rows(self.directory / "renderer.jsonl", self.renderer)
        path = self.directory / "consumer.jsonl"
        path.write_text(path.read_text().rstrip("\n"), encoding="utf-8")
        result = ANALYZER.analyze_series([self.directory])
        self.assertEqual(result["status"], "FAILED_NATIVE")
        self.assertIn("incomplete", result["captures"][0]["artifact_error"])

    def test_zero_local_budget_and_missing_process_counter_never_use_capacity_fallback(self):
        memory = next(row for row in self.renderer if row["kind"] == "adapter_memory")
        memory["snapshot"]["local"]["budget_bytes"] = 0
        memory["snapshot"]["shared_system_capacity_bytes"] = 1 << 60
        memory["snapshot"]["nonlocal"] = {"budget_bytes": 1 << 60}
        self.renderer.append({"kind": "process_resources", "snapshot": None, "error": {"Native": {"hresult": -1}}})
        write_rows(self.directory / "renderer.jsonl", self.renderer)
        resources = self.analyze()["resources"]
        self.assertIsNone(resources["admission_within_target"])
        self.assertIsNone(resources["renderer_private_within_target"])

    def test_missing_gpu_row_remains_missing_despite_valid_other_context_percentiles(self):
        self.renderer.remove(next(row for row in self.renderer if row.get("event", {}).get("Gpu", {}).get("context") == "Renderer"))
        write_rows(self.directory / "renderer.jsonl", self.renderer)
        result = self.analyze()
        self.assertEqual(result["renderer_gpu"]["missing_rows"], 1)
        self.assertFalse(result["renderer_gpu"]["complete"])
        self.assertFalse(result["criteria"]["telemetry_coverage"]["observed_within_target"])
        self.assertEqual(result["criteria"]["gpu_context_coverage"]["status"], "UNAVAILABLE")

    def test_retirement_and_input_changes_cannot_be_hidden_by_successful_native_exit(self):
        summary = ANALYZER.load_json(self.directory / "renderer-summary.json")
        summary["final_runtime"]["resources"]["bytes_used"] = 1
        (self.directory / "renderer-summary.json").write_text(json.dumps(summary), encoding="utf-8")
        (self.directory / "inputs.json").write_text(json.dumps({"before": {"exe": "a"}, "after": {"exe": "b"}}), encoding="utf-8")
        result = self.analyze()
        self.assertEqual(result["criteria"]["completion_retirement"]["status"], "FAILED_NATIVE")
        self.assertEqual(result["criteria"]["input_identity"]["status"], "FAILED_NATIVE")

    def test_identical_capture_repeated_three_times_is_not_an_independent_series(self):
        result = ANALYZER.analyze_series([self.directory] * 3)
        self.assertFalse(result["prescribed_three_sample_series"])
        self.assertFalse(result["qualified"])

    def classified_series(self, renderer_unavailable=None, later_status="UNAVAILABLE"):
        template = self.analyze()
        captures = []
        for index in range(3):
            capture = copy.deepcopy(template)
            capture["prescribed_sample"] = True
            capture["manifest"].update(run_id=f"independent-{index}", warmup_seconds=30, sample_seconds=120)
            for name, item in capture["criteria"].items():
                renderer = item["owner_phase"] == "M03" and item["owner_gate"] == 9
                item["status"] = "PASSED_NATIVE" if renderer else later_status
                if name == renderer_unavailable:
                    item["status"] = "UNAVAILABLE"
            captures.append(capture)
        with mock.patch.object(ANALYZER, "analyze_capture", side_effect=captures):
            return ANALYZER.analyze_series([self.directory] * 3)

    def test_later_physical_and_full_app_gates_cannot_block_verified_renderer_qualification(self):
        criteria = self.analyze()["criteria"]
        self.assertEqual(criteria["physical_display_dpi_input"]["owner_phase"], "M05")
        self.assertEqual(criteria["Q05_handles_growth"]["owner_phase"], "M09")
        self.assertEqual((criteria["renderer_handle_growth"]["owner_phase"],
                          criteria["renderer_handle_growth"]["owner_gate"]), ("M03", 9))
        for later in ("UNAVAILABLE", "FAILED_NATIVE"):
            with self.subTest(later=later):
                result = self.classified_series(later_status=later)
                self.assertTrue(result["renderer_qualification"]["qualified"])
                self.assertEqual(result["renderer_qualification"]["status"], "PASSED_NATIVE")
                self.assertFalse(result["qualified"])

    def test_renderer_critical_path_cpu_deadlines_and_handle_budgets_are_not_waived(self):
        for missing in ("gpu_critical_path", "cpu_submission", "deadline_misses", "renderer_handle_growth"):
            with self.subTest(missing=missing):
                result = self.classified_series(renderer_unavailable=missing)
                self.assertFalse(result["renderer_qualification"]["qualified"])
                self.assertIn(missing, result["renderer_qualification"]["required_criteria"])
                self.assertEqual(result["renderer_qualification"]["status"], "UNAVAILABLE")

    def test_a_prescribed_series_with_every_criterion_passed_has_consistent_aggregate_status(self):
        result = self.classified_series(later_status="PASSED_NATIVE")
        self.assertTrue(result["qualified"])
        self.assertEqual(result["status"], "PASSED_NATIVE")
        self.assertEqual(result["renderer_qualification"]["status"], "PASSED_NATIVE")

    def test_schema_one_captures_are_not_reinterpreted_using_new_clock_or_sink_semantics(self):
        manifest = ANALYZER.load_json(self.directory / "manifest.json")
        manifest["schema_version"] = 1
        (self.directory / "manifest.json").write_text(json.dumps(manifest), encoding="utf-8")
        result = ANALYZER.analyze_series([self.directory])
        self.assertEqual(result["status"], "FAILED_NATIVE")
        self.assertIn("unsupported production capture schema", result["captures"][0]["artifact_error"])

    def test_capture_age_observations_do_not_qualify_an_unjoined_output_frame_cohort(self):
        result = self.analyze()
        age = result["criteria"]["source_to_consumer_age"]
        self.assertEqual(age["status"], "UNAVAILABLE")
        self.assertTrue(age["observed_capture_p95_within_budget"])
        self.assertIn("exact output-frame cohort", age["reason"])

    def test_oversampled_valid_captures_do_not_establish_normative_output_frame_age(self):
        oversampled = []
        index = 0
        for row in self.consumer:
            oversampled.append(row)
            if row["kind"] == "consumer_frame":
                row["frame"]["row"] = index
                index += 1
                for _ in range(2):
                    repeated = copy.deepcopy(row)
                    repeated["frame"]["row"] = index
                    repeated["frame"]["token"]["serial"] = 1000 + index
                    index += 1
                    oversampled.append(repeated)
        write_rows(self.directory / "consumer.jsonl", oversampled)
        result = self.analyze()
        self.assertEqual(result["frame_age"]["channels"][0]["valid"], 180)
        self.assertEqual(result["frame_age"]["channels"][0]["repeated"], 120)
        self.assertTrue(result["frame_age"]["capture_count_at_least_nominal"])
        self.assertFalse(result["frame_age"]["output_frame_cohort_coverage_complete"])
        self.assertEqual(result["criteria"]["source_to_consumer_age"]["status"], "UNAVAILABLE")

    def test_full_duration_metadata_does_not_hide_incomplete_cadence_and_missing_observations(self):
        manifest = ANALYZER.load_json(self.directory / "manifest.json")
        manifest.update(warmup_seconds=30, sample_seconds=120)
        (self.directory / "manifest.json").write_text(json.dumps(manifest), encoding="utf-8")
        self.renderer[0].update(requested_warmup_seconds=30, requested_sample_seconds=120)
        write_rows(self.directory / "renderer.jsonl", self.renderer)
        summary = ANALYZER.load_json(self.directory / "renderer-summary.json")
        summary["phase_windows"][0].update(begin_qpc=1, end_qpc=1 + 30 * FREQUENCY)
        summary["phase_windows"][1].update(begin_qpc=2 + 30 * FREQUENCY, end_qpc=2 + 150 * FREQUENCY)
        (self.directory / "renderer-summary.json").write_text(json.dumps(summary), encoding="utf-8")
        result = self.analyze()
        self.assertTrue(result["prescribed_sample"])
        self.assertEqual(result["criteria"]["output_api_cadence"]["status"], "FAILED_NATIVE")
        self.assertEqual(result["criteria"]["gpu_critical_path"]["status"], "UNAVAILABLE")
        self.assertEqual(result["criteria"]["renderer_private_memory"]["status"], "UNAVAILABLE")
        self.assertEqual(result["criteria"]["deadline_misses"]["status"], "UNAVAILABLE")

    def test_launcher_clock_cannot_create_a_negative_passing_shutdown(self):
        launcher = ANALYZER.load_json(self.directory / "launcher.json")
        launcher["exit_observed_qpc"] = 1
        (self.directory / "launcher.json").write_text(json.dumps(launcher), encoding="utf-8")
        with self.assertRaises(ANALYZER.ArtifactError):
            self.analyze()

    def test_cli_requires_explicit_short_mode_and_preserves_native_argument_names(self):
        directory = str(ROOT / "target" / "production-qualification-unit-unused")
        arguments = ["run", "--directory", directory, "--samples", "1", "--warmup", "0", "--seconds", "2"]
        with contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit):
            ANALYZER.parse_args(arguments)
        args = ANALYZER.parse_args([*arguments, "--diagnostic", "--isolation-seconds", "60"])
        command = ANALYZER.capture_command(args, Path(directory) / "sample-001")
        self.assertEqual(command[1:3], ["--role", "renderer"])
        self.assertIn("--independence", command)
        self.assertEqual(command[command.index("--independence") + 1], "60")


if __name__ == "__main__":
    unittest.main()
