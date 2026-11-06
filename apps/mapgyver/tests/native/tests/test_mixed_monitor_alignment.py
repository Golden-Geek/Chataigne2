"""A settled pixel match cannot substitute for a genuine same-HWND DPI round trip."""
import copy
import json
from pathlib import Path
import unittest
import zipfile


ROOT = Path(__file__).resolve().parents[5]
ARCHIVE = ROOT / "docs/progress/evidence/mapgyver-m05-mixed-move-2026-11-06/mixed-move.zip"
NEGATIVE_ARCHIVE = ROOT / "docs/progress/evidence/mapgyver-m05-negative-origin-2026-11-06/negative-origin.zip"


class MixedMonitorAlignment(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.cases = []
        for path, receipt in ((ARCHIVE, "mixed-move.json"), (NEGATIVE_ARCHIVE, "negative-move.json")):
            with zipfile.ZipFile(path) as archive:
                cls.cases.extend(json.loads(archive.read(name + "/" + receipt)) for name in
                                 ("nv-screen", "nv-source", "intel-screen", "intel-source"))

    def assert_round_trip(self, case):
        self.assertEqual(case["result"], "pass")
        self.assertEqual(case["normal_exit"], 0)
        stages = case["stages"]
        negative = case.get("negative_origin", False)
        self.assertEqual([s["stage"] for s in stages],
                         ["laptop-before", "negative-after" if negative else "external-after", "laptop-return"])
        self.assertGreater(case["pid"], 0)
        self.assertGreater(case["hwnd"], 0)
        for stage in stages:
            self.assertEqual(stage["pid"], case["pid"])
            self.assertEqual(stage["hwnd"], case["hwnd"])
            capture = stage["capture"]
            self.assertEqual(capture["capture_mode"], "visible owned foreground window")
            self.assertFalse(capture["window_maximized"])
            left, top, right, bottom = capture["desktop_bounds"]
            ml, mt, mr, mb = capture["monitor_bounds"]
            self.assertTrue(ml <= left < right <= mr and mt <= top < bottom <= mb)
            self.assertEqual(capture["window_dpi"], 96 * stage["geometry"]["viewport"]["dpr"])
        self.assertEqual([s["capture"]["window_dpi"] for s in stages], [120, 96, 120])
        monitors = [s["capture"]["monitor_bounds"] for s in stages]
        self.assertEqual(monitors[0], monitors[2])
        self.assertNotEqual(monitors[0], monitors[1])
        if negative:
            self.assertLess(monitors[1][0], 0)
            self.assertLessEqual(monitors[1][2], 0)
        else:
            self.assertGreaterEqual(monitors[1][0], 0)

    def test_both_editors_on_both_adapters_complete_real_round_trips(self):
        expected = {(editor, adapter) for editor in ("screen", "source") for adapter in
                    ("dxgi:00000000:00016022", "dxgi:00000000:00015b69")}
        self.assertEqual(len(self.cases), 8)
        for negative in (False, True):
            cases = [c for c in self.cases if c.get("negative_origin", False) == negative]
            self.assertEqual(len(cases), 4)
            self.assertEqual({(c["editor"], c["adapter"]["key"]) for c in cases}, expected)
        for case in self.cases:
            with self.subTest(pid=case["pid"]):
                self.assert_round_trip(case)

    def test_replaced_process_or_window_cannot_count_as_movement(self):
        for case in self.cases:
            for key in ("pid", "hwnd"):
                value = copy.deepcopy(case)
                value["stages"][1][key] += 1
                with self.subTest(pid=case["pid"], key=key), self.assertRaises(AssertionError):
                    self.assert_round_trip(value)

    def test_copying_initial_placement_cannot_count_as_an_actual_crossing(self):
        for case in self.cases:
            value = copy.deepcopy(case)
            value["stages"][1] = copy.deepcopy(value["stages"][0])
            value["stages"][1]["stage"] = case["stages"][1]["stage"]
            with self.subTest(pid=case["pid"]), self.assertRaises(AssertionError):
                self.assert_round_trip(value)

    def test_partial_monitor_placement_or_css_scaling_cannot_qualify(self):
        for case in self.cases:
            partial = copy.deepcopy(case)
            capture = partial["stages"][1]["capture"]
            capture["desktop_bounds"][0] = capture["monitor_bounds"][0] - 1
            scaled = copy.deepcopy(case)
            scaled["stages"][1]["geometry"]["viewport"]["dpr"] = 1.25
            for label, value in (("partial", partial), ("CSS only", scaled)):
                with self.subTest(pid=case["pid"], kind=label), self.assertRaises(AssertionError):
                    self.assert_round_trip(value)

    def test_positive_origin_captures_cannot_claim_negative_monitor_coverage(self):
        for case in self.cases:
            if case.get("negative_origin", False):
                continue
            value = copy.deepcopy(case)
            value["negative_origin"] = True
            value["stages"][1]["stage"] = "negative-after"
            with self.subTest(pid=case["pid"]), self.assertRaises(AssertionError):
                self.assert_round_trip(value)


if __name__ == "__main__":
    unittest.main()
