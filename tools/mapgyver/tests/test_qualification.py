"""Evidence compaction must retain sparse observations and unavailable counters."""

import copy
import contextlib
import io
import json
import runpy
from pathlib import Path
import unittest

RUNNER = Path(__file__).resolve().parents[1] / "run-qualification.py"
COMPACT = runpy.run_path(str(RUNNER))["compact_resources"]
COMPACT_AGE = runpy.run_path(str(RUNNER))["compact_frame_age"]
COMPACT_CHILD = runpy.run_path(str(RUNNER))["compact_child_resources"]
PARSE_ARGS = runpy.run_path(str(RUNNER))["parse_args"]
ENCODE = runpy.run_path(str(RUNNER))["encode_evidence"]
DECODE = runpy.run_path(str(RUNNER))["decode_evidence"]


def observation():
    processes = [
        {
            "process_id": index,
            "private_bytes": 100 + index,
            "working_set_bytes": 50 + index,
            "handle_count": 10 + index,
        }
        for index in range(4)
    ]
    return {
        "elapsed_seconds": 30.01,
        "in_sample_window": True,
        "stage": {"generation": 2, "settling": True, "gpu_pipeline": {"pending": 2, "sampled": {"busy": 7}}},
        "renderer": processes[0],
        "sources": processes[1:3],
        "observer": processes[3],
        "renderer_gpu_local_nonlocal": [
            {"current_usage_bytes": 1024, "budget_bytes": 4096},
            None,
        ],
        "renderer_allocations": {
            "live_bytes_by_owner": [8, 16, 8, 8],
            "retiring_bytes_by_owner": [0, 0, 0, 0],
        },
        "presentation_editor_output": [
            {"frame": {"Err": "0x887a000b"}},
            {"frame": {"Ok": {"present_count": 12}}},
        ],
    }


class EvidenceTests(unittest.TestCase):
    def test_large_lossless_capture_preserves_integer_clocks_nulls_and_native_failures(self):
        evidence = {
            "run": {"exit_code": 1, "stderr": "native failure", "report": None},
            "rows": [[9_007_199_254_740_999 + n, None, {"Err": "0x887a000b"}] for n in range(25_000)],
        }
        plain = ENCODE(evidence, "json")
        self.assertGreater(len(plain), 1_048_576)
        encoded = ENCODE(evidence, "gzip-base64")
        self.assertLess(len(encoded), 1_048_576)
        self.assertEqual(DECODE(json.loads(encoded)), evidence)
        self.assertEqual(json.loads(plain), evidence)

    def test_compressed_capture_rejects_changed_length_hash_and_encoding(self):
        packet = json.loads(ENCODE({"clock": 9_007_199_254_740_999}, "gzip-base64"))
        for key, value in [("decoded_bytes", 0), ("sha256", "0" * 64), ("wire_schema_version", 2)]:
            with self.subTest(key=key), self.assertRaises(ValueError):
                DECODE({**packet, key: value})

    def test_interactive_observations_cannot_be_mislabeled_as_a_qualification_series(self):
        args = PARSE_ARGS(["0", "diagnostic", "--underlay", "--warmup", "0", "--samples", "1"])
        self.assertTrue(args.underlay)
        self.assertFalse(PARSE_ARGS(["0", "q01"]).underlay)
        invalid = [
            ["0", "diagnostic"],
            ["0", "q01", "--underlay", "--warmup", "0", "--samples", "1"],
            ["0", "diagnostic", "--underlay", "--samples", "1"],
            ["0", "diagnostic", "--underlay", "--warmup", "0"],
            ["0", "diagnostic", "--underlay", "--recreate", "1"],
        ]
        for argv in invalid:
            with self.subTest(argv=argv), contextlib.redirect_stderr(io.StringIO()):
                with self.assertRaises(SystemExit):
                    PARSE_ARGS(argv)

    def test_child_resource_compaction_preserves_phase_ownership_and_unknown_segments(self):
        snapshot = {
            "live_bytes_by_owner": [8, 0, 0, 8], "retiring_bytes_by_owner": [4, 0, 0, 4],
            "live_textures": 2, "retiring_textures": 2,
            "peak_live_plus_retiring_bytes": 24, "peak_live_plus_retiring_textures": 4,
        }
        resources = {"sample_count": 1, "samples": [{
            "elapsed_seconds": 2.0, "source_phase": 1, "allocations": snapshot,
            "gpu_local_nonlocal": [None, {"current_usage_bytes": 900, "budget_bytes": 1000}],
        }]}
        nested = copy.deepcopy(resources)
        COMPACT_CHILD(resources)
        report = {"resources": {"samples": [observation()]},
                  "generations": [{"source_workers": [{"resources": nested}]}]}
        COMPACT(report)
        self.assertNotIn("samples", nested)
        self.assertEqual(nested["sample_columns"], resources["sample_columns"])
        self.assertEqual(nested["sample_rows"], resources["sample_rows"])
        row = dict(zip(resources["sample_columns"], resources["sample_rows"][0], strict=True))
        self.assertEqual(row["source_phase"], 1)
        self.assertEqual(row["live_bytes_by_owner"], [8, 0, 0, 8])
        self.assertIsNone(row["gpu_local.current_usage_bytes"])
        self.assertEqual(row["gpu_nonlocal.current_usage_bytes"], 900)
        with self.assertRaisesRegex(ValueError, "sample count"):
            COMPACT_CHILD({"sample_count": 1, "samples": []})

    def test_frame_age_compaction_preserves_high_clock_words_and_checks_count(self):
        sample = {
            "submitted_qpc": 9_007_199_254_740_999,
            "completed_qpc": 9_007_199_254_741_001,
            "words": [[1, 2, 4_294_967_295, 100, 5], [6, 7, 8, 9, 10]],
        }
        report = {"observer": {"frame_age": {"captured_samples": 1, "samples": [sample]}}}
        COMPACT_AGE(report)
        age = report["observer"]["frame_age"]
        self.assertEqual(len(age["sample_columns"]), 12)
        self.assertEqual(age["sample_rows"], [[
            sample["submitted_qpc"], sample["completed_qpc"], *sample["words"][0], *sample["words"][1],
        ]])
        with self.assertRaisesRegex(ValueError, "sample count"):
            COMPACT_AGE({"observer": {"frame_age": {"captured_samples": 2, "samples": [sample]}}})

    def test_all_observations_and_unknown_counters_survive(self):
        sample = observation()
        later = copy.deepcopy(sample)
        later["elapsed_seconds"] = 31.02
        later["renderer"]["private_bytes"] = 999
        report = {
            "resources": {
                "samples": [sample, later],
                "exclusions": ["unknown texture residency"],
            }
        }
        COMPACT(report)
        resources = report["resources"]
        self.assertEqual(resources["process_ids_renderer_sources_observer"], [0, 1, 2, 3])
        rows = [dict(zip(resources["columns"], row, strict=True)) for row in resources["rows"]]
        self.assertEqual([row["elapsed_seconds"] for row in rows], [30.01, 31.02])
        self.assertEqual([row["renderer.private_bytes"] for row in rows], [100, 999])
        self.assertEqual([row["generation"] for row in rows], [2, 2])
        self.assertEqual([row["settling"] for row in rows], [True, True])
        self.assertIsNone(rows[0]["renderer_gpu_nonlocal.current_usage_bytes"])
        self.assertEqual(resources["exclusions"], ["unknown texture residency"])
        self.assertEqual(len(resources["gpu_observations"]), 2)
        self.assertEqual(resources["gpu_observations"][0]["gpu_pipeline"], sample["stage"]["gpu_pipeline"])
        self.assertEqual(
            resources["gpu_observations"][0]["renderer_allocations"], sample["renderer_allocations"],
        )
        self.assertEqual(
            resources["gpu_observations"][0]["presentation_editor_output"], sample["presentation_editor_output"],
        )

    def test_process_replacement_cannot_be_silently_compacted(self):
        first, second = observation(), observation()
        second["sources"][1]["process_id"] = 50
        with self.assertRaisesRegex(ValueError, "identity changed"):
            COMPACT({"resources": {"samples": [first, second]}})


if __name__ == "__main__":
    unittest.main()
