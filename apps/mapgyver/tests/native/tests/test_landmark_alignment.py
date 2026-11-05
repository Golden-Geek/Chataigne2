"""Replay actual owned-window captures; shifted DOM/DPI metadata must fail closed."""
import copy
import hashlib
import json
from pathlib import Path
import sys
import unittest

from PIL import Image

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from visible_zoom import independent_landmark_alignment

ROOT = Path(__file__).resolve().parents[5]
RECEIPT = ROOT / "docs/progress/evidence/mapgyver-m05-display-alignment-2026-11-05.json"


class NativeLandmarkAlignment(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.cases = json.loads(RECEIPT.read_text(encoding="utf-8"))["cases"]

    def analyze(self, case, geometry=None, capture=None):
        path = ROOT / case["image"]
        self.assertEqual(hashlib.sha256(path.read_bytes()).hexdigest(), case["image_sha256"])
        with Image.open(path) as image:
            return independent_landmark_alignment(
                image, geometry or case["geometry"], capture or case["capture"], case["editor"]
            )

    def test_four_observed_native_configurations_align(self):
        self.assertEqual({case["name"] for case in self.cases}, {
            "screen-100", "screen-125", "source-100", "source-125"
        })
        for case in self.cases:
            with self.subTest(case=case["name"]):
                measured = self.analyze(case)
                self.assertTrue(measured["settled_alignment_verified"])
                self.assertFalse(measured["input_latency_verified"])

    def test_three_physical_pixel_projection_error_rejects_every_case(self):
        for case in self.cases:
            with self.subTest(case=case["name"]):
                geometry = copy.deepcopy(case["geometry"])
                geometry["active"]["x"] += 3 / geometry["viewport"]["dpr"]
                self.assertFalse(self.analyze(case, geometry)["settled_alignment_verified"])

    def test_mismatched_native_dpi_or_background_capture_is_not_accepted(self):
        for case in self.cases:
            with self.subTest(case=case["name"]):
                capture = copy.deepcopy(case["capture"])
                capture["window_dpi"] += 24
                with self.assertRaises(AssertionError):
                    self.analyze(case, capture=capture)
                capture = copy.deepcopy(case["capture"])
                capture["capture_mode"] = "PrintWindow"
                with self.assertRaises(AssertionError):
                    self.analyze(case, capture=capture)


if __name__ == "__main__":
    unittest.main()
