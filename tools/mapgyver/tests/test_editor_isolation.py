import copy
import contextlib
import io
from pathlib import Path
import runpy
import unittest

CHECK = runpy.run_path(str(Path(__file__).parents[1] / "analyze-editor-isolation.py"))
RUNNER = runpy.run_path(str(Path(__file__).parents[1] / "run-qualification.py"))


def proof():
    windows = []
    rows = []
    for index, start in enumerate((3000, 73000)):
        snapshots = [dict(qpc=start+i*1000, editor_requests=10+index, layout_session=index+1,
                          layout_revision=1 if index == 0 else 0, rendered_revision=1 if index == 0 else 0,
                          preview_regions=2 if index == 0 else 0, output_presents=i*60, spout_sends=i*60)
                     for i in range(62)]
        windows.append(dict(kind=("javascript_busy", "blank_document")[index], snapshots=snapshots))
        for s in snapshots[:-1]:
            for offset in (10, 20):
                frame = s["qpc"] + offset
                words = []
                for channel in range(2):
                    magic = 0xa671_c300 | channel
                    checksum = CHECK["RELEASE"]["rotate"](frame, 7) ^ frame ^ magic ^ 0x9e37_79b9
                    words += [frame, magic, frame, 0, checksum]
                rows.append([frame, frame+1, *words])
    return dict(qpc_frequency=1000, frontend_busy_elapsed_ms=65000, busy_started_qpc=1000,
                busy_finished_qpc=66000, windows=windows,
                reconnected=dict(qpc=136000, layout_session=3, preview_regions=2, layout_revision=1, rendered_revision=1)), dict(sample_rows=rows)


class EditorIsolationTests(unittest.TestCase):
    def test_every_interval_has_output_and_fresh_independent_pixels(self):
        p, age = proof()
        # Session epochs need only increase: navigation callbacks can skip numbers.
        for window, session in zip(p['windows'], (5, 9)):
            for snapshot in window['snapshots']:
                snapshot['layout_session'] = session
        p['reconnected']['layout_session'] = 11
        windows = CHECK["verify_windows"](p, age)
        self.assertEqual([w["observed_seconds"] for w in windows], [61, 61])
        self.assertEqual(windows[0]["native_output_calls"], 3660)
        self.assertEqual(windows[1]["unique_observed_fixture_frames"], [122, 122])

    def test_missing_intervals_live_frontend_stale_layout_and_failed_reconnect_are_rejected(self):
        for mutation in (
            lambda p,a: p["windows"][0]["snapshots"].pop(),
            lambda p,a: p["windows"][0]["snapshots"][1].__setitem__("editor_requests", 11),
            lambda p,a: p["windows"][0]["snapshots"][1].__setitem__("layout_session", 2),
            lambda p,a: p["windows"][1]["snapshots"][1].__setitem__("layout_session", 1),
            lambda p,a: p["windows"][1]["snapshots"][3].__setitem__("preview_regions", 2),
            lambda p,a: p["windows"][1]["snapshots"][3].__setitem__("rendered_revision", 1),
            lambda p,a: p["windows"][0]["snapshots"][3].__setitem__("output_presents", 120),
            lambda p,a: p["reconnected"].__setitem__("layout_session", 1),
            lambda p,a: p.__setitem__("busy_finished_qpc", 62000),
            lambda p,a: a["sample_rows"].__setitem__(slice(0,2), []),
            lambda p,a: a["sample_rows"][0].__setitem__(3, 0),
            lambda p,a: a["sample_rows"][0].__setitem__(6, 0),
        ):
            p, age = copy.deepcopy(proof())
            mutation(p, age)
            with self.assertRaises(ValueError):
                CHECK["verify_windows"](p, age)

    def test_runner_keeps_lifecycle_exercise_distinct_from_timed_qualification(self):
        args = RUNNER["parse_args"](["0", "q01", "--editor-isolation", "--seconds", "185", "--warmup", "0", "--samples", "1"])
        self.assertTrue(args.editor_isolation)
        for argv in (["0", "q01", "--editor-isolation"], ["0", "q01", "--editor-isolation", "--recreate", "1"]):
            with contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit):
                RUNNER["parse_args"](argv)


if __name__ == "__main__":
    unittest.main()
