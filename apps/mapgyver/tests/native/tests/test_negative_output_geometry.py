"""Replay actual owned Rectangle/Fullscreen windows on a negative-origin display."""
import copy
import json
from pathlib import Path
import unittest
import zipfile


ARCHIVE = Path(__file__).resolve().parents[5] / "docs/progress/evidence/mapgyver-m05-negative-origin-2026-11-06/negative-origin.zip"


class NegativeOutputGeometry(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        with zipfile.ZipFile(ARCHIVE) as archive:
            cls.cases = [json.loads(archive.read(name + "/bounds.json")) for name in
                         ("output-nv", "output-intel")]

    def assert_output_geometry(self, case):
        self.assertEqual(case["result"], "pass")
        self.assertTrue(case["negativeOriginObserved"])
        self.assertTrue(any(d["bounds"]["left"] < 0 for d in case["displays"]))
        self.assertTrue(case["disabledWindowRetired"])
        self.assertEqual(case["pageErrors"], [])
        rows = case["observations"]
        self.assertEqual([r["phase"] for r in rows], ["initial", "position-only", "resize",
                         "undo-resize", "fullscreen-display-0", "fullscreen-display-1", "return-rectangle"])
        for row in rows:
            self.assertEqual(row["hwnd"], case["identity"])
            self.assertEqual(row["creations"], 1)
            self.assertTrue(row["visible"])
            self.assertFalse(row["minimized"])
            self.assertTrue(row["center_rgb"])
            self.assertTrue(all(r >= 250 and g <= 2 and b <= 2 for r, g, b in row["center_rgb"]))
        self.assertEqual(rows[0]["retirements"], rows[1]["retirements"])
        self.assertGreater(rows[2]["retirements"], rows[1]["retirements"])
        self.assertEqual(rows[1]["bounds"], rows[3]["bounds"])
        self.assertLess(rows[0]["bounds"]["left"], 0)
        for index in (0, 1):
            self.assertEqual(rows[4 + index]["bounds"], case["displays"][index]["bounds"])

    def test_both_gpus_preserve_output_identity_bounds_pixels_and_retirement(self):
        self.assertEqual({c["adapter"]["key"] for c in self.cases},
                         {"dxgi:00000000:00016022", "dxgi:00000000:00015b69"})
        for case in self.cases:
            with self.subTest(pid=case["pid"]):
                self.assert_output_geometry(case)

    def test_negative_window_position_without_a_negative_monitor_does_not_qualify(self):
        for case in self.cases:
            value = copy.deepcopy(case)
            for display in value["displays"]:
                display["bounds"]["left"] = abs(display["bounds"]["left"])
            with self.subTest(pid=case["pid"]), self.assertRaises(AssertionError):
                self.assert_output_geometry(value)

    def test_fullscreen_must_match_the_actual_display_exactly(self):
        for case in self.cases:
            value = copy.deepcopy(case)
            value["observations"][4]["bounds"]["left"] += 1
            with self.subTest(pid=case["pid"]), self.assertRaises(AssertionError):
                self.assert_output_geometry(value)

    def test_replaced_output_window_cannot_count_as_in_place_reassignment(self):
        for case in self.cases:
            value = copy.deepcopy(case)
            value["observations"][5]["hwnd"] += 1
            with self.subTest(pid=case["pid"]), self.assertRaises(AssertionError):
                self.assert_output_geometry(value)


if __name__ == "__main__":
    unittest.main()
