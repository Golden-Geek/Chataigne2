import importlib.util
import copy
from pathlib import Path
import tempfile
import sys
import unittest

from PIL import Image

NATIVE = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(NATIVE))
SPEC = importlib.util.spec_from_file_location("pattern_pixels", NATIVE / "pattern_pixels.py")
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class PatternPixels(unittest.TestCase):
    def test_png_has_exact_authored_straight_rgba_including_hidden_rgb(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            manifest = MODULE.create(root)
            with Image.open(root / "rgba-pattern.png") as image:
                self.assertEqual(image.mode, "RGBA")
                for index, rgba in enumerate(manifest["rgba"]):
                    self.assertEqual(image.getpixel((index % 4 * 200 + 100, index // 4 * 150 + 75)), tuple(rgba))

    def test_alpha_uses_linear_light_and_retains_zero_coverage(self):
        self.assertEqual(MODULE.expected_color([255, 0, 0, 128]), [188, 0, 0])
        self.assertEqual(MODULE.expected_color([255, 0, 0, 0]), [0, 0, 0])
        self.assertEqual(MODULE.expected_color([64, 128, 192, 255]), [64, 128, 192])
        self.assertNotEqual(MODULE.expected_color([255, 0, 0, 128]), [128, 0, 0])

    def test_independent_homography_has_correct_corners_and_non_affine_center(self):
        quad = [[0.12, 0.22], [0.82, 0.12], [0.90, 0.82], [0.18, 0.90]]
        for point, expected in zip(((0, 0), (1, 0), (1, 1), (0, 1)), quad):
            for actual, value in zip(MODULE.project(quad, *point), expected):
                self.assertAlmostEqual(actual, value)
        center = MODULE.project(quad, 0.5, 0.5)
        self.assertGreater(abs(center[0] - sum(x for x, _ in quad) / 4), 0.001)

    def test_physical_probe_oracle_rejects_swapped_channels_and_ignored_alpha(self):
        image = Image.new("RGB", (800, 600))
        colors = [tile[:3] for tile in MODULE.TILES[:12]] + [
            [0, 0, 0], [0, 137, 0], [0, 0, 188], [55, 112, 169]]
        for index, rgb in enumerate(colors):
            x, y = index % 4 * 200, index // 4 * 150
            image.paste(tuple(rgb), (x, y, x + 200, y + 150))
        metadata = {"editor": "source", "selected": True,
                    "viewport": {"dpr": 1}, "active": {"x": 0, "y": 0, "width": 800, "height": 600},
                    "crop": {"x": 0.01, "y": 0.01, "width": 0.05, "height": 0.05},
                    "pattern": {"rgba": MODULE.TILES}}
        capture = {"window_dpi": 96, "capture_mode": "visible owned foreground window",
                   "client_desktop_origin": [0, 0], "desktop_bounds": [0, 0, 800, 600]}
        self.assertTrue(MODULE.analyze(image, metadata, capture)["passed"])
        red, green, blue = image.split()
        self.assertFalse(MODULE.analyze(Image.merge("RGB", (blue, green, red)), metadata, capture)["passed"])
        ignored_alpha = copy.deepcopy(metadata)
        for rgba in ignored_alpha["pattern"]["rgba"]:
            rgba[3] = 255
        self.assertFalse(MODULE.analyze(image, ignored_alpha, capture)["passed"])


if __name__ == "__main__":
    unittest.main()
