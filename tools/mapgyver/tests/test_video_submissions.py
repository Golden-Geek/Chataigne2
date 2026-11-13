"""Complete attribution needs independent counters, stable identities and healthy capture."""
from copy import deepcopy
from pathlib import Path
import runpy
import unittest

API = runpy.run_path(str(Path(__file__).parents[1] / "video_submissions.py"))
ANALYZE = API["analyze_source_submissions"]


def binding(stream="gpu"):
    return {"stream": stream, "owner": "worker-" + stream, "file": "clip.mp4",
            "reference": {"media": "video", "channel": stream}}


def submission(serial=1, stream="gpu", kind="software", pts=0):
    upload = {"stream": "upload-" + stream, "sequence": serial, "bytes": 8,
              "row_pitch": 8, "rows": 1, "cpu": {"begin": serial * 10, "end": serial * 10 + 1}}
    payload = {"Software": upload} if kind == "software" else {"Hap": {"bytes": 4}}
    if kind == "resident":
        payload = "Resident"
    return {"stream": stream, "token": {"device": {"runtime": "producer-" + stream, "generation": 1}, "serial": serial},
            "descriptor": {"extent": {"width": 2, "height": 1}, "format": "Rgba8Unorm",
                           "color_encoding": "Srgb", "alpha_mode": "Straight"},
            "payload": payload, "frame": {"epoch_authority": "clock", "epoch_generation": 0,
            "pts_nanos": pts, "start_nanos": pts, "end_nanos": pts + 10}, "observed_qpc": serial * 10 + 2}


def budget(software=0, hap=0, retained=0, name="budget"):
    return {"id": name, "retained_bytes": retained, "uploads": {
        "software": {"calls": software, "bytes": software * 8, "overflowed": False},
        "hap": {"calls": hap, "bytes": hap * 4, "overflowed": False}}}


def health(events):
    count = sum(map(len, events.values()))
    return {"summary": {"duration_limit": False, "events": count, "diagnostics": {
        "enabled": True, "queued": 0, "dropped_full": 0, "clock_failures": 0,
        "written": count, "drained": count}}, "observed_events": count}


def analyze(events=None, bindings=None, snapshots=None, captures=None):
    events = events if events is not None else {"SourceSubmission": [submission()]}
    return ANALYZE(events, [binding()] if bindings is None else bindings,
                   [budget(1)] if snapshots is None else snapshots,
                   [health(events)] if captures is None else captures)


class SourceAttributionTests(unittest.TestCase):
    def test_unsampled_repeated_pts_and_alias_captures_keep_actual_upload_counts(self):
        first, second = submission(), submission(2)
        witness = {"stream": "gpu", "sequence": 2, "software_upload": second["payload"]["Software"],
                   "frame": {"pts_nanos": 0}}
        events = {"SourceSubmission": [first, second, deepcopy(first)], "SourceUse": [witness, deepcopy(witness)]}
        result = analyze(events, snapshots=[budget(2), budget(1), budget(2)])
        totals, source = result["source_worker_cpu_uploads"], result["sources"][0]
        self.assertTrue(totals["complete_cpu_payload_pts_attribution"])
        self.assertEqual(totals["observed"]["software"], {"calls": 2, "bytes": 16})
        self.assertEqual((source["submissions"], source["distinct_observed_pts"], source["repeated_pts_submissions"]), (2, 1, 1))
        self.assertEqual((source["renderer_uses"], source["rendered_publication_versions"]), (2, 1))
        self.assertEqual(source["duplicate_submitted_windows"], 1)

    def test_loop_offsets_and_replacement_epochs_do_not_invent_duplicate_windows(self):
        rows = [submission(i) for i in (1, 2, 3)]
        rows[1]["frame"].update(start_nanos=100, end_nanos=110)
        rows[2]["frame"]["epoch_authority"] = "replacement-clock"
        result = analyze({"SourceSubmission": rows}, snapshots=[budget(3)])
        item = result["sources"][0]
        self.assertEqual((item["repeated_pts_submissions"], item["duplicate_submitted_windows"]), (2, 0))
        self.assertTrue(result["source_worker_cpu_uploads"]["complete_cpu_payload_pts_attribution"])

    def test_relink_stream_lifetimes_roll_up_to_the_same_authored_channel(self):
        replacement = binding("replacement")
        replacement.update(reference=binding()["reference"], file="other.mp4")
        result = analyze({"SourceSubmission": [submission(), submission(stream="replacement")]},
                         bindings=[binding(), replacement], snapshots=[budget(2)])
        self.assertEqual(len(result["channels"]), 1)
        channel = result["channels"][0]
        self.assertEqual(channel["software"], {"calls": 2, "bytes": 16})
        self.assertEqual(channel["streams"], ["gpu", "replacement"])
        self.assertEqual(channel["files"], ["clip.mp4", "other.mp4"])

    def test_equal_layout_channels_resident_hap_and_renderer_uploads_do_not_merge(self):
        image = {"token": {"device": "renderer", "serial": 1}, "texture": 7,
                 "stream": "cpu", "sequence": 1, "bytes": 8, "row_pitch": 8, "rows": 1}
        events = {"SourceSubmission": [submission(), submission(stream="hap", kind="hap"),
                  submission(stream="resident", kind="resident")], "ImageUpload": [image, deepcopy(image)],
                  "SourceUse": [{"stream": "cpu", "sequence": 1, "frame": {"pts_nanos": 0}}]}
        result = analyze(events, bindings=[binding(s) for s in ("gpu", "hap", "resident", "cpu")], snapshots=[budget(1, 1)])
        self.assertTrue(result["source_worker_cpu_uploads"]["complete_cpu_payload_attribution"])
        self.assertEqual(result["render_thread_uploads"], {"calls": 1, "bytes": 8, "complete_channel_attribution": True,
                         "unknown_pts_upload_calls": 0, "complete_pts_attribution": True})
        sources = {s["stream"]: s for s in result["sources"]}
        self.assertEqual(sources["resident"]["resident_submissions"], 1)
        self.assertEqual(sources["hap"]["hap"], {"calls": 1, "bytes": 4})
        self.assertEqual(sources["cpu"]["software"]["calls"], 0)

    def test_missing_bindings_rows_clocks_and_windows_stay_explicit(self):
        row = submission()
        row["frame"] = row["observed_qpc"] = None
        result = analyze({"SourceSubmission": [row]})
        self.assertTrue(result["source_worker_cpu_uploads"]["complete_cpu_payload_attribution"])
        self.assertFalse(result["source_worker_cpu_uploads"]["complete_cpu_payload_pts_attribution"])
        self.assertEqual(result["sources"][0]["unclocked_submissions"], 1)
        self.assertFalse(analyze(bindings=[])["source_worker_cpu_uploads"]["complete_cpu_payload_attribution"])
        result = analyze({"SourceSubmission": [submission(2)]}, snapshots=[budget(2)])
        self.assertEqual(result["source_worker_cpu_uploads"]["missing"]["software"], {"calls": 1, "bytes": 8})
        self.assertIn("missing_source_submission_serials", result["source_worker_cpu_uploads"]["unknown_reasons"])

    def test_legacy_renderer_witnesses_cannot_supply_unsampled_source_facts(self):
        events = {"SourceUse": [{"stream": "gpu", "sequence": 1, "software_upload": submission()["payload"]["Software"]}]}
        result = analyze(events, bindings=[], snapshots=[budget(3)])
        facts = result["source_worker_cpu_uploads"]
        self.assertFalse(facts["complete_cpu_payload_attribution"])
        self.assertEqual(facts["missing"]["software"]["calls"], 3)
        self.assertIn("renderer_witnessed_uploads_missing_source_submissions", facts["unknown_reasons"])
        with self.assertRaises(ValueError):
            API["require_complete_attribution"](result)

    def test_acceptance_requires_pts_and_renderer_channel_attribution(self):
        API["require_complete_attribution"](analyze())
        row = submission()
        row["frame"] = None
        with self.assertRaises(ValueError):
            API["require_complete_attribution"](analyze({"SourceSubmission": [row]}))
        image = {"token": {"device": "renderer", "serial": 1}, "texture": 7,
                 "stream": "gpu", "sequence": 1, "bytes": 8, "row_pitch": 8, "rows": 1}
        result = analyze({"ImageUpload": [image]}, snapshots=[budget()])
        self.assertTrue(result["render_thread_uploads"]["complete_channel_attribution"])
        self.assertFalse(result["render_thread_uploads"]["complete_pts_attribution"])
        with self.assertRaises(ValueError):
            API["require_complete_attribution"](result)

    def test_empty_or_legacy_resident_only_captures_cannot_prove_source_attribution(self):
        for events in ({}, {"SourceUse": [{"stream": "gpu", "sequence": 1, "frame": None}]}):
            with self.assertRaises(ValueError):
                API["require_complete_attribution"](analyze(events, snapshots=[budget()]))

    def test_loss_missing_footers_incomplete_drain_and_duration_limits_never_pass(self):
        events = {"SourceSubmission": [submission()]}
        for field in ("queued", "dropped_full", "clock_failures", "drained"):
            broken = health(events)
            broken["summary"]["diagnostics"][field] += 1
            self.assertFalse(analyze(events, captures=[broken])["source_worker_cpu_uploads"]["complete_cpu_payload_attribution"])
        for captures in ([], [None], [{"summary": {}}]):
            self.assertFalse(analyze(events, captures=captures)["source_worker_cpu_uploads"]["complete_cpu_payload_attribution"])
        broken = health(events)
        broken["summary"]["duration_limit"] = True
        self.assertFalse(analyze(events, captures=[broken])["source_worker_cpu_uploads"]["complete_cpu_payload_attribution"])

    def test_overflow_unmeasured_and_multiple_budgets_do_not_certify_per_source_coverage(self):
        overflow = budget(1)
        overflow["uploads"]["software"]["overflowed"] = True
        for snapshots in ([overflow], [None], [{"id": "budget", "uploads": None}], [budget(1), budget(name="other")]):
            self.assertFalse(analyze(snapshots=snapshots)["source_worker_cpu_uploads"]["complete_cpu_payload_attribution"])
        self.assertFalse(analyze(snapshots=[budget(1, retained=8)])["source_worker_cpu_uploads"]["complete_cpu_payload_attribution"])

    def test_live_latest_counters_do_not_inherit_an_older_retired_snapshot(self):
        result = analyze(snapshots=[budget(2, retained=8), budget(1, retained=0)])
        self.assertIn("source_storage_not_retired_at_latest_counters", result["source_worker_cpu_uploads"]["unknown_reasons"])

    def test_changed_submission_upload_binding_and_descriptor_identities_are_rejected(self):
        first = submission()
        for field, value in (("frame", None), ("observed_qpc", 999)):
            changed = deepcopy(first)
            changed[field] = value
            with self.assertRaises(ValueError):
                analyze({"SourceSubmission": [first, changed]})
        changed = deepcopy(binding())
        changed["file"] = "other.mp4"
        with self.assertRaises(ValueError):
            analyze(bindings=[binding(), changed])
        changed = submission(2)
        changed["descriptor"]["extent"]["width"] = 4
        with self.assertRaises(ValueError):
            analyze({"SourceSubmission": [first, changed]})
        changed = submission(stream="other")
        changed["payload"]["Software"] = first["payload"]["Software"]
        with self.assertRaises(ValueError):
            analyze({"SourceSubmission": [first, changed]})

    def test_counter_and_renderer_witness_contradictions_are_errors(self):
        with self.assertRaises(ValueError):
            analyze(snapshots=[budget(0)])
        bad = budget(1)
        bad["uploads"]["software"]["bytes"] += 1
        with self.assertRaises(ValueError):
            analyze(snapshots=[budget(1), bad])
        for change in ("pts", "upload"):
            witness = {"stream": "gpu", "sequence": 1, "software_upload": deepcopy(submission()["payload"]["Software"]),
                       "frame": {"pts_nanos": 0}}
            if change == "pts":
                witness["frame"]["pts_nanos"] = 1
            else:
                witness["software_upload"]["bytes"] += 1
            with self.assertRaises(ValueError):
                analyze({"SourceSubmission": [submission()], "SourceUse": [witness]})

    def test_invalid_scalar_and_window_metadata_are_rejected(self):
        for field, value in (("serial", True), ("serial", -1)):
            row = submission()
            row["token"][field] = value
            with self.assertRaises(ValueError):
                analyze({"SourceSubmission": [row]})
        row = submission()
        row["frame"]["end_nanos"] = 0
        with self.assertRaises(ValueError):
            analyze({"SourceSubmission": [row]})

    def test_binding_log_parser_keeps_duplicate_and_prefixed_facts_for_identity_validation(self):
        import json
        line = "[mapgyver-video-source] " + json.dumps(binding())
        self.assertEqual(API["bindings_from_log"]("noise\ntest prefix " + line + "\n" + line), [binding(), binding()])


if __name__ == "__main__":
    unittest.main()
