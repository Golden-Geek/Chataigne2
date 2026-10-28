"""Saved-evidence verification must reject corruption and preserve incomplete/failed scope."""

import copy
import json
from pathlib import Path
import runpy
import unittest

TOOLS = Path(__file__).resolve().parents[1]
ANALYZER = runpy.run_path(str(TOOLS / "analyze-release.py"))
ENCODE = runpy.run_path(str(TOOLS / "run-qualification.py"))["encode_evidence"]


def age_fixture():
    qpc = 9_007_199_254_740_999
    words = []
    for index in range(2):
        magic, low, high = 0xa671_c300 | index, qpc & 0xffff_ffff, qpc >> 32
        checksum = ANALYZER["rotate"](12, 7) ^ low ^ ANALYZER["rotate"](high, 13) ^ magic ^ 0x9e37_79b9
        words.extend([12, magic, low, high, checksum])
    distribution = {key: 2.0 for key in ("mean_ms", "median_ms", "p95_ms", "p99_ms", "maximum_ms")}
    channel = {
        "valid": 1, "absent": 0, "invalid": 0, "repeated": 0, "unique_frames": 1,
        "skipped_source_frame_ids": 0, "frame_age": {"samples": 1, **distribution},
    }
    return {
        "config": {"start_qpc": qpc - 1, "warmup_seconds": 0, "maximum_seconds": 1, "frequency": 1000},
        "captured_samples": 1, "sample_rows": [[qpc + 1, qpc + 2, *words]],
        "channels": [copy.deepcopy(channel), copy.deepcopy(channel)],
    }


class ReleaseAnalysisTests(unittest.TestCase):
    def test_source_workers_cannot_hide_renderer_sdk_work_bridge_copies_or_retained_allocations(self):
        zero = {"live_bytes_by_owner": [0]*4, "retiring_bytes_by_owner": [0]*4,
                "live_textures": 0, "retiring_textures": 0}
        worker = {
            "adapter_luid": "same", "maximum_surfaces": 3, "peak_surfaces": 3,
            "receive_calls": 5, "publications": 4, "truncated_receive_samples": 0,
            "receive_wall_time_all_windows": {"samples": 5},
            "gpu_admission": {"maximum_in_flight": 2, "peak_in_flight": 2, "pending": 0,
                              "reserved": False, "submitted": 5, "completed": 5,
                              "warmup": {"attempts": 3, "admitted": 3, "busy": 0},
                              "sampled": {"attempts": 2, "admitted": 2, "busy": 0}},
            "timing": {"cpu_submission_work": {"samples": 2}, "gpu_execution": {"samples": 2},
                       "skipped_gpu_queries": 0, "invalid_gpu_queries": 0, "pending_gpu_queries": 0,
                       "gpu_outliers": {"events": [], "omitted": 0, "total": 0, "maximum": None}},
            "resources": {"sample_count": 0, "sample_rows": [], "after_fence": zero,
                          "operations": {"sdk_receive_calls": 5, "explicit_gpu_copy_calls": 4,
                                         "sdk_texture_observations": 4, "diagnostic_readback_pixels": 0,
                                         "uniform_upload_bytes": 0, "sdk_send_calls": 0}},
        }
        generation = {"adapter_luid": "same", "operations": {"sdk_receive_calls": 0, "explicit_gpu_copy_calls": 0},
                      "source_workers": [copy.deepcopy(worker), copy.deepcopy(worker)]}
        self.assertEqual(len(ANALYZER["verify_source_workers"](generation)), 2)
        for mutation in ("renderer", "renderer_copy", "retained", "capacity", "copies", "adapter", "worker_gpu", "timing"):
            bad = copy.deepcopy(generation)
            first = bad["source_workers"][0]
            if mutation == "renderer": bad["operations"]["sdk_receive_calls"] = 1
            elif mutation == "renderer_copy": bad["operations"]["explicit_gpu_copy_calls"] = 1
            elif mutation == "retained": first["resources"]["after_fence"]["live_textures"] = 1
            elif mutation == "capacity": first["peak_surfaces"] = 4
            elif mutation == "copies": first["resources"]["operations"]["explicit_gpu_copy_calls"] = 0
            elif mutation == "adapter": first["adapter_luid"] = "other"
            elif mutation == "worker_gpu": first["gpu_admission"]["peak_in_flight"] = 3
            else: first["timing"]["cpu_submission_work"]["samples"] = 1
            with self.subTest(mutation=mutation), self.assertRaises(ValueError):
                ANALYZER["verify_source_workers"](bad)

    def test_gpu_coalescing_preserves_busy_denominators_and_rejects_unbounded_or_retained_work(self):
        admission = {
            "maximum_in_flight": 2, "peak_in_flight": 2, "pending": 0, "reserved": False,
            "submitted": 3, "completed": 3, "pending_at_final_poll": 1,
            "warmup": {"attempts": 2, "admitted": 2, "busy": 0},
            "sampled": {"attempts": 3, "admitted": 1, "busy": 2},
        }
        generation = {
            "gpu_admission": admission, "spout_sends": 3,
            "timing": {"cpu_submission_work": {"samples": 1}},
            "submission_editor_output_spout": [{"attempts": 3, "busy": 2} for _ in range(3)],
        }
        resources = {"gpu_observations": [{"gpu_pipeline": {
            **copy.deepcopy(admission), "submitted": 2, "completed": 0, "pending": 2,
        }}]}
        self.assertEqual(ANALYZER["verify_gpu_admission"](generation, resources)["sampled"]["busy"], 2)
        bad = copy.deepcopy(generation)
        bad["submission_editor_output_spout"][0]["busy"] = 0
        with self.assertRaisesRegex(ValueError, "coalescing missing"):
            ANALYZER["verify_gpu_admission"](bad, resources)
        for field in ("pending", "peak_in_flight"):
            bad = copy.deepcopy(generation)
            bad["gpu_admission"][field] = 3
            with self.subTest(field=field), self.assertRaises(ValueError):
                ANALYZER["verify_gpu_admission"](bad, resources)

    def test_new_schema_cannot_silently_use_a_stale_binary_without_operation_data(self):
        evidence = {"schema_version": 8, "run": {"exit_code": 0, "report": {
            "generations": [{"end": "deadline_reached"}],
        }}}
        with self.assertRaisesRegex(ValueError, "operation trace missing"):
            ANALYZER["analyze_sample"](evidence)
        evidence["schema_version"] = 9
        evidence["run"]["report"]["generations"][0]["operation_timing"] = {}
        with self.assertRaisesRegex(ValueError, "GPU admission missing"):
            ANALYZER["analyze_sample"](evidence)
        evidence["schema_version"] = 10
        evidence["run"]["report"]["generations"][0]["gpu_admission"] = {}
        with self.assertRaisesRegex(ValueError, "source workers missing"):
            ANALYZER["analyze_sample"](evidence)
        evidence["schema_version"] = 11
        evidence["run"]["report"]["generations"][0]["source_workers"] = [{}]
        with self.assertRaisesRegex(ValueError, "source GPU admission missing"):
            ANALYZER["analyze_sample"](evidence)
        evidence["schema_version"] = 12
        evidence["run"]["report"]["generations"][0]["source_workers"] = [{"gpu_admission": {}, "timing": {}}]
        with self.assertRaisesRegex(ValueError, "timeline admission missing"):
            ANALYZER["analyze_sample"](evidence)

    def test_operation_trace_preserves_high_stamps_and_rejects_overlapping_or_omitted_spans(self):
        event = {
            "stamp": {"frame": 0, "qpc": 9_007_199_254_740_999},
            "scheduled_delay_ns": 0, "elapsed_ns": 67_000_000, "gpu_query_skipped": False,
            "spans": [{"offset_ns": 0, "elapsed_ns": 67_000_000}, *([None] * 15)],
        }
        log = {"events": [event], "maximum": event, "maximum_score": 67_000_000, "total": 1, "omitted": 0}
        empty = {"events": [], "maximum": None, "maximum_score": 0, "total": 0, "omitted": 0}
        generation = {
            "spout_sends": 1,
            "timing": {"cpu_submission_work": {"samples": 1}, "gpu_outliers": empty},
            "operation_timing": {
                "operation_names": [f"operation_{index}" for index in range(16)],
                "sampled_by_operation": [{"samples": 1}, *([None] * 15)],
                "qpc_frequency": 10_000_000, "frame_period_ns": 16_666_667,
                "operation_threshold_ns": 4_000_000, "outlier_capacity_per_window": 128,
                "truncated_operation_samples": 0,
                "warmup": {"frames": 0, "outliers": copy.deepcopy(empty)},
                "sampled": {"frames": 1, "outliers": log},
            },
        }
        result = ANALYZER["verify_operations"](generation)
        self.assertEqual(result["sampled_outliers"]["maximum"]["stamp"]["qpc"], 9_007_199_254_740_999)
        for mutation, message in [("span", "exceeds frame"), ("overlap", "overlap"), ("count", "count mismatch")]:
            with self.subTest(mutation=mutation):
                bad = copy.deepcopy(generation)
                outliers = bad["operation_timing"]["sampled"]["outliers"]
                if mutation == "span":
                    outliers["events"][0]["spans"][0]["offset_ns"] = 1
                elif mutation == "overlap":
                    outliers["events"][0]["spans"][1] = {"offset_ns": 1, "elapsed_ns": 1}
                else:
                    outliers["omitted"] = 1
                with self.assertRaisesRegex(ValueError, message):
                    ANALYZER["verify_operations"](bad)

    def test_favorable_gpu_percentiles_cannot_qualify_incomplete_query_accounting(self):
        timing = {
            "cpu_submission_work": {"samples": 100},
            "gpu_execution": {"samples": 100, "p95_ms": 1.0, "p99_ms": 2.0},
            "skipped_gpu_queries": 0, "invalid_gpu_queries": 0,
            "pending_gpu_queries": 0, "truncated_samples": 0,
        }
        self.assertTrue(ANALYZER["gpu_timing_checks"](timing)["gpu_budget_met"])
        timing["gpu_execution"]["samples"] = 84
        timing["skipped_gpu_queries"] = 16
        result = ANALYZER["gpu_timing_checks"](timing)
        self.assertTrue(result["recorded_gpu_percentiles_within_target"])
        self.assertFalse(result["gpu_query_accounting_complete"])
        self.assertFalse(result["gpu_budget_met"])
        self.assertEqual(result["gpu_query_accounting"]["skipped_gpu_queries"], 16)

    def test_raw_integer_marker_oracle_rejects_corruption_and_wrong_distribution(self):
        age = age_fixture()
        self.assertTrue(ANALYZER["verify_age"](age))
        damaged = copy.deepcopy(age)
        damaged["sample_rows"][0][6] ^= 1
        with self.assertRaisesRegex(ValueError, "channel count"):
            ANALYZER["verify_age"](damaged)
        damaged = copy.deepcopy(age)
        damaged["channels"][1]["frame_age"]["p95_ms"] = 0
        with self.assertRaisesRegex(ValueError, "distribution"):
            ANALYZER["verify_age"](damaged)

    def test_failed_sample_remains_failed_and_cannot_close_a_release_series(self):
        evidence = {
            "schema_version": 7,
            "sample_index": 1, "profile": "release", "requested_series_samples": 3,
            "inputs_unchanged_after_run": True, "inputs": {}, "command": ["qualify"], "adapters": [],
            "run": {"exit_code": 1, "stderr": "cancelled", "report": None},
        }
        packet = json.loads(ENCODE(evidence, "gzip-base64"))
        result = ANALYZER["analyze_series"]([packet])
        self.assertFalse(result["three_immutable_release_samples"])
        self.assertFalse(result["samples"][0]["native_success"])
        self.assertEqual(result["samples"][0]["failure"], evidence["run"])
        self.assertTrue(result["still_unqualified"])
        changed = copy.deepcopy(evidence)
        changed["inputs"] = {"changed": True}
        with self.assertRaisesRegex(ValueError, "inputs changed"):
            ANALYZER["analyze_series"]([packet, json.loads(ENCODE(changed, "gzip-base64"))])


if __name__ == "__main__":
    unittest.main()
