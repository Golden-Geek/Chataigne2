import copy
from io import BytesIO
import importlib.util
import json
from pathlib import Path
import sys
import unittest
import zipfile

from PIL import Image

ROOT = Path(__file__).resolve().parents[5]
NATIVE = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(NATIVE))
SPEC = importlib.util.spec_from_file_location("pattern_pixels", NATIVE / "pattern_pixels.py")
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class NativePatternCaptures(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.samples = []
        archive = ROOT / "docs/progress/evidence/mapgyver-m05-preview-alpha-2026-11-06/pattern-captures.zip"
        with zipfile.ZipFile(archive) as source:
            for case in ("fixed-nvidia-100", "fixed-intel-125"):
                receipt = "pattern-result.json" if "nvidia" in case else "pattern-failure.json"
                result = json.loads(source.read(case + "/" + receipt))
                for stage in result["stages"]:
                    name = case + "/" + stage["stage"]
                    cls.samples.append((name, "nvidia" in case,
                        json.loads(source.read(name + "/projection.json")),
                        json.loads(source.read(name + "/capture.json")),
                        source.read(name + "/owned-window.png")))
            old = "nvidia-comparison/direct-source-selected"
            cls.old_alpha = json.loads(source.read(old + "/pixels.json"))

    def test_replay_preserves_the_actual_passes_and_unaccepted_intel_precision_failure(self):
        self.assertEqual(len(self.samples), 18)
        passed = 0
        for name, nvidia, metadata, capture, raw in self.samples:
            with self.subTest(name=name), Image.open(BytesIO(raw)) as image:
                result = MODULE.analyze(image, metadata, capture)
                expected = nvidia or name.endswith("screen-plain")
                self.assertEqual(result["passed"], expected)
                if expected:
                    passed += 1
                    self.assertLessEqual(result["maximum_rgb_error"], 2)
                else:
                    self.assertEqual(result["maximum_rgb_error"], 4)
        self.assertEqual(passed, 12)

    def test_every_sampled_zero_and_partial_alpha_tile_meets_the_original_limit(self):
        count = 0
        for name, _, metadata, capture, raw in self.samples:
            with self.subTest(name=name), Image.open(BytesIO(raw)) as image:
                for probe in MODULE.analyze(image, metadata, capture)["probes"]:
                    if probe["rgba"][3] < 255:
                        self.assertLessEqual(probe["maximum_error"], 2)
                        count += 9
        self.assertEqual(count, 504)

    def test_swapped_channels_reject_every_otherwise_passing_capture(self):
        for name, nvidia, metadata, capture, raw in self.samples:
            if not nvidia and not name.endswith("screen-plain"):
                continue
            with self.subTest(name=name), Image.open(BytesIO(raw)) as image:
                broken = copy.deepcopy(metadata)
                for rgba in broken["pattern"]["rgba"]:
                    rgba[0], rgba[2] = rgba[2], rgba[0]
                self.assertFalse(MODULE.analyze(image, broken, capture)["passed"])

    def test_old_transparent_tile_matches_two_selection_fills_instead_of_one(self):
        probe = next(probe for probe in self.old_alpha["probes"] if probe["rgba"][3] == 0)
        alpha = 24 / 255
        doubled = MODULE.expected_color([0, 0, 0, 0], alpha * (2 - alpha))
        self.assertGreater(probe["maximum_error"], 2)
        self.assertEqual(probe["expected"], [36, 55, 85])
        for actual in probe["actual"]:
            self.assertTrue(all(abs(a - b) <= 2 for a, b in zip(actual, doubled)))


if __name__ == "__main__":
    unittest.main()
