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


class FloatingNativePatternCaptures(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.samples = []
        archive = ROOT / "docs/progress/evidence/mapgyver-m05-preview-precision-2026-11-06/precision-captures.zip"
        with zipfile.ZipFile(archive) as source:
            for case in ("nvidia-100", "intel-125"):
                result = json.loads(source.read(case + "/pattern-result.json"))
                assert result["result"] == "pass"
                for stage in result["stages"]:
                    name = case + "/" + stage["stage"]
                    cls.samples.append((name,
                        json.loads(source.read(name + "/projection.json")),
                        json.loads(source.read(name + "/capture.json")),
                        source.read(name + "/owned-window.png")))

    def test_all_original_color_cases_pass_without_relaxing_the_oracle(self):
        self.assertEqual(len(self.samples), 18)
        pixels = 0
        for name, metadata, capture, raw in self.samples:
            with self.subTest(name=name), Image.open(BytesIO(raw)) as image:
                result = MODULE.analyze(image, metadata, capture)
                self.assertTrue(result["passed"])
                self.assertEqual(result["tolerance"], 2)
                self.assertLessEqual(result["maximum_rgb_error"], 1 if metadata["selected"] else 2)
                pixels += result["pixel_count"]
                mismatched = copy.deepcopy(capture)
                mismatched["window_dpi"] += 24
                with self.assertRaises(AssertionError):
                    MODULE.analyze(image, metadata, mismatched)
        self.assertEqual(pixels, 2592)

    def test_the_previous_intel_dark_gray_failure_is_now_correct_on_both_gpus(self):
        count = 0
        for name, metadata, capture, raw in self.samples:
            if not name.endswith("direct-source-selected"):
                continue
            with self.subTest(name=name), Image.open(BytesIO(raw)) as image:
                probe = next(probe for probe in MODULE.analyze(image, metadata, capture)["probes"]
                             if probe["rgba"] == [32, 32, 32, 255])
                self.assertEqual(probe["expected"], [49, 64, 90])
                self.assertLessEqual(probe["maximum_error"], 1)
                count += 1
        self.assertEqual(count, 2)

    def test_channel_and_alpha_negative_controls_still_reject_the_fixed_images(self):
        alpha_cases = 0
        for name, metadata, capture, raw in self.samples:
            with self.subTest(name=name), Image.open(BytesIO(raw)) as image:
                swapped = copy.deepcopy(metadata)
                for rgba in swapped["pattern"]["rgba"]:
                    rgba[0], rgba[2] = rgba[2], rgba[0]
                self.assertFalse(MODULE.analyze(image, swapped, capture)["passed"])
                if any(probe["rgba"][3] < 255 for probe in MODULE.analyze(image, metadata, capture)["probes"]):
                    ignored_alpha = copy.deepcopy(metadata)
                    for rgba in ignored_alpha["pattern"]["rgba"]:
                        rgba[3] = 255
                    self.assertFalse(MODULE.analyze(image, ignored_alpha, capture)["passed"])
                    alpha_cases += 1
        self.assertEqual(alpha_cases, 14)


if __name__ == "__main__":
    unittest.main()
