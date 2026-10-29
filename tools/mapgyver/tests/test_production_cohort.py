import copy
from pathlib import Path
import tempfile
import unittest

from test_production_qualification import ANALYZER, BEGIN, END, FREQUENCY, token, wall, write_capture, write_rows


class ProductionCohortTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.directory = Path(temporary.name) / "capture"
        self.renderer, self.sources, self.consumer = write_capture(self.directory)

    def analyze(self):
        for name, rows in zip(("renderer", "source-1", "source-2", "consumer"),
                              (self.renderer, *self.sources, self.consumer)):
            write_rows(self.directory / (name + ".jsonl"), rows)
        return ANALYZER.analyze_capture(self.directory)

    def frames(self):
        return [row["frame"] for row in self.consumer if row["kind"] == "consumer_frame"]

    def outputs(self):
        return [row["event"]["Output"] for row in self.renderer
                if row.get("event", {}).get("Output", {}).get("output") == 2]

    def test_exact_cohort_joins_renderer_sources_and_canonical_gpu_pixels(self):
        result = self.analyze()
        cohort = result["spout_output_cohort"]
        self.assertTrue(cohort["coverage_complete"])
        self.assertEqual((cohort["expected_accepted_images"], cohort["captured_images"]), (60, 60))
        self.assertEqual(cohort["records"][0]["source_frames"], [1, 1])
        self.assertEqual(cohort["source_age"][0]["p95_ms"], 2)
        self.assertEqual(cohort["sender_api_to_capture_completion"]["lower_bound"]["p95_ms"], 1.25)
        self.assertEqual(cohort["sender_api_to_capture_completion"]["upper_bound"]["p95_ms"], 1.3)
        publication = cohort["source_publication_age"]
        self.assertTrue(publication["coverage_complete"])
        self.assertEqual(publication["outputs"][0]["sources"][0]["publication_to_api_end"]["p95_ms"], 1 / 3)
        self.assertTrue(result["frame_age"]["spout_output_cohort_coverage_complete"])
        self.assertFalse(result["frame_age"]["output_frame_cohort_coverage_complete"])
        self.assertEqual(result["criteria"]["source_to_consumer_age"]["status"], "UNAVAILABLE")

    def test_repeat_capture_cannot_replace_one_missing_output_image(self):
        first, second = self.frames()[:2]
        for name in ("spout_image", "raw_words", "left", "right"):
            second[name] = copy.deepcopy(first[name])
        result = self.analyze()
        cohort = result["spout_output_cohort"]
        self.assertTrue(result["frame_age"]["capture_count_at_least_nominal"])
        self.assertFalse(cohort["coverage_complete"])
        self.assertEqual((cohort["captured_images"], cohort["missing_images"], cohort["repeated_captures"]), (59, 1, 1))
        self.assertEqual(cohort["records"][0]["capture_row"], 0)
        self.assertIsNone(cohort["records"][1]["source_ages_ms"])
        self.assertIsNone(result["criteria"]["spout_capture_age"]["observed_within_target"])

    def test_a_later_capture_still_belongs_to_the_sample_output_cohort(self):
        last = self.frames()[-1]
        last.update(receive_begin_qpc=END + 80, completion_observed_qpc=END + 100)
        cpu = next(row["event"]["SourceCpu"] for row in self.consumer
                   if row.get("event", {}).get("SourceCpu", {}).get("token") == last["token"])
        cpu.update(wall=wall(END + 80, END + 90), receive_api=wall(END + 80, END + 90))
        result = self.analyze()
        self.assertEqual(result["frame_age"]["channels"][0]["valid"], 59)
        self.assertTrue(result["spout_output_cohort"]["coverage_complete"])
        self.assertEqual(result["spout_output_cohort"]["records"][-1]["capture_row"], 59)

    def test_handle_reuse_with_reset_counter_is_ambiguous_instead_of_guessed(self):
        outputs = self.outputs()
        outputs[1]["spout_image"] = copy.deepcopy(outputs[0]["spout_image"])
        cohort = self.analyze()["spout_output_cohort"]
        self.assertFalse(cohort["coverage_complete"])
        self.assertEqual(cohort["missing_images"], 2)
        self.assertIn("ambiguous", cohort["records"][0]["missing"])

    def test_equal_counter_on_another_handle_does_not_join(self):
        self.frames()[0]["spout_image"]["shared_handle"] += 1
        cohort = self.analyze()["spout_output_cohort"]
        self.assertFalse(cohort["coverage_complete"])
        self.assertEqual(cohort["missing_images"], 1)

    def test_copy_counters_are_independent_of_producer_and_renderer_batch_serials(self):
        for stream in self.sources:
            for row in stream:
                if row["kind"] == "source_frame":
                    row["submission"]["spout_image"]["frame"] += 100
        for row in self.renderer:
            use = row.get("event", {}).get("SourceUse")
            if use:
                use["publication"]["spout_image"]["frame"] += 100
        for output in self.outputs():
            output["spout_image"]["frame"] += 200
        for frame in self.frames():
            frame["spout_image"]["frame"] += 200
        cohort = self.analyze()["spout_output_cohort"]
        self.assertTrue(cohort["coverage_complete"])
        self.assertEqual(cohort["records"][0]["source_frames"], [1, 1])
        self.assertEqual(cohort["records"][0]["spout_image"]["frame"], 201)

    def test_valid_past_gpu_marker_must_match_the_exact_renderer_source_use(self):
        first, second = self.frames()[:2]
        second["left"] = copy.deepcopy(first["left"])
        second["raw_words"][:6] = first["raw_words"][:6]
        result = self.analyze()
        self.assertTrue(result["frame_age"]["complete"])
        cohort = result["spout_output_cohort"]
        self.assertFalse(cohort["coverage_complete"])
        self.assertIn("exact renderer source", cohort["invalid_captures"][0]["reason"])

    def test_unknown_source_versions_do_not_create_zero_age_or_complete_coverage(self):
        use = next(row["event"]["SourceUse"] for row in self.renderer if "SourceUse" in row.get("event", {}))
        use["publication"]["spout_image"] = None
        cohort = self.analyze()["spout_output_cohort"]
        self.assertFalse(cohort["coverage_complete"])
        self.assertEqual(cohort["missing_images"], 1)
        self.assertIsNone(cohort["records"][0]["source_ages_ms"])

    def test_missing_receive_interval_does_not_certify_a_capture(self):
        cpu = next(row["event"]["SourceCpu"] for row in self.consumer if "SourceCpu" in row.get("event", {}))
        cpu["receive_api"] = None
        cohort = self.analyze()["spout_output_cohort"]
        self.assertEqual(cohort["missing_images"], 1)
        self.assertIn("receive interval", cohort["invalid_captures"][0]["reason"])

    def test_unknown_publication_clock_is_missing_instead_of_a_zero_age(self):
        use = next(row["event"]["SourceUse"] for row in self.renderer if "SourceUse" in row.get("event", {}))
        use["publication"]["publication_qpc"] = None
        age = self.analyze()["spout_output_cohort"]["source_publication_age"]
        self.assertFalse(age["coverage_complete"])
        self.assertEqual(age["outputs"][0]["sources"][0]["missing"], 1)

    def test_output_api_cannot_precede_its_source_publication(self):
        use = next(row["event"]["SourceUse"] for row in self.renderer if "SourceUse" in row.get("event", {}))
        use["publication"]["publication_qpc"] = BEGIN + 100
        with self.assertRaisesRegex(ValueError, "precedes its retained source publication"):
            self.analyze()

    def test_api_end_after_capture_completion_produces_a_zero_lower_bound_not_negative_age(self):
        first = self.frames()[0]
        first.update(receive_begin_qpc=BEGIN + 23, completion_observed_qpc=BEGIN + 24)
        cpu = next(row["event"]["SourceCpu"] for row in self.consumer if "SourceCpu" in row.get("event", {}))
        cpu.update(wall=wall(BEGIN + 23, BEGIN + 24), receive_api=wall(BEGIN + 23, BEGIN + 24))
        cohort = self.analyze()["spout_output_cohort"]
        self.assertTrue(cohort["coverage_complete"])
        bounds = cohort["records"][0]["sender_api_capture_delay_ms"]
        self.assertEqual(bounds, {"lower_bound": 0, "upper_bound": 2 * 1000 / FREQUENCY})

    def test_interruption_cannot_pass_using_nominal_capture_counts_with_a_missing_image(self):
        first, second = self.frames()[:2]
        for name in ("spout_image", "raw_words", "left", "right"):
            second[name] = copy.deepcopy(first[name])
        # Reuse the synthetic one-second interval as each isolated function-level oracle.
        windows = [{"name": name, "begin_qpc": BEGIN, "end_qpc": END}
                   for name in ("editor_minimized", "editor_disconnected")]
        rows = [*self.renderer, *[{"kind": "phase_outputs", "name": window["name"],
                                 "before": [token("renderer", 1)] * 2,
                                 "after": [token("renderer", 60)] * 2} for window in windows]]
        result = ANALYZER.independence_metrics(
            {"independence_seconds": 1}, {"phase_windows": windows}, rows, self.sources,
            self.consumer, FREQUENCY, ANALYZER.events(rows), ANALYZER.events(self.consumer))
        for phase in result["phases"]:
            self.assertTrue(phase["main_outputs_advanced"])
            self.assertEqual(phase["frame_age"]["channels"][0]["valid"], 60)
            self.assertEqual(phase["spout_output_cohort"]["missing_images"], 1)
            self.assertFalse(phase["complete"])


if __name__ == "__main__":
    unittest.main()
