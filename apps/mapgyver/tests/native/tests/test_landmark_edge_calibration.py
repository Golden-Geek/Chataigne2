"""A perfectly aligned analytic edge must not fail because of threshold-run extrapolation."""
from pathlib import Path
import sys
import unittest

import numpy as np
from PIL import Image, ImageDraw

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from landmark_edges import linear_red_edges
from visible_zoom import independent_landmark_alignment


GEOMETRY = {"viewport": {"width": 2048, "height": 1232, "dpr": 1.25},
            "active": {"x": 429.4, "y": 188.5, "width": 1180, "height": 885}}
CAPTURE = {"capture_mode": "visible owned foreground window", "capture_usable_for_alignment": True,
           "size": [2578, 1558], "window_dpi": 120,
           "client_desktop_origin": [0, 0], "desktop_bounds": [-9, -9, 2569, 1549]}


def projected(fractions):
    active, scale = GEOMETRY["active"], GEOMETRY["viewport"]["dpr"]
    return [9 + (active["x"] + active["width"] * v) * scale for v in fractions] + [
        9 + (active["y"] + active["height"] * v) * scale for v in fractions]


def analytic_image(red_shift=0):
    red = projected([0.26, 0.74])
    blue = projected([0.2, 0.8])
    x = np.arange(CAPTURE["size"][0], dtype=float) + 0.5
    y = np.arange(CAPTURE["size"][1], dtype=float) + 0.5
    # A normalized, symmetric three-pixel linear reconstruction filter has its 50%
    # crossing exactly at the declared continuous source boundary.
    horizontal = np.clip(0.5 + (x - red[0] - red_shift) / 3, 0, 1) * np.clip(
        0.5 + (red[1] + red_shift - x) / 3, 0, 1)
    vertical = np.clip(0.5 + (y - red[2]) / 3, 0, 1) * np.clip(0.5 + (red[3] - y) / 3, 0, 1)
    pixels = np.zeros((len(y), len(x), 3), dtype=float)
    pixels[:, :, 0] = vertical[:, None] * horizontal
    mask = ((y[:, None] > blue[2]) & (y[:, None] < blue[3]) &
            (x > blue[0]) & (x < blue[1]))
    fill = np.array([120, 170, 250], dtype=float) / 255
    fill = np.where(fill <= 0.04045, fill / 12.92, ((fill + 0.055) / 1.055) ** 2.4)
    pixels[mask] = pixels[mask] * (1 - 24 / 255) + fill * (24 / 255)
    encoded = np.where(pixels <= 0.0031308, pixels * 12.92, 1.055 * pixels ** (1 / 2.4) - 0.055)
    image = Image.fromarray(np.rint(encoded * 255).astype(np.uint8))
    ImageDraw.Draw(image).rectangle(tuple(round(blue[i] - 0.5) for i in (0, 2, 1, 3)),
                                   outline=(112, 170, 255), width=1)
    return image


class LandmarkEdgeCalibration(unittest.TestCase):
    def test_aligned_filtered_image_passes_while_preserving_the_legacy_false_rejection(self):
        image = analytic_image()
        old = independent_landmark_alignment(image, GEOMETRY, CAPTURE, "screen")
        self.assertTrue(old["settled_alignment_verified"])
        self.assertGreater(old["legacy_threshold_errors"]["image_to_overlay_max_px"], 1)
        active = projected([0, 1])
        edges = linear_red_edges(image, tuple(round(active[i]) for i in (0, 2, 1, 3)), old["threshold_red_edges"])
        self.assertLess(max(abs(a - b) for a, b in zip(edges, projected([0.26, 0.74]))), 0.02)

    def test_continuous_edge_measurement_preserves_a_real_two_pixel_image_displacement(self):
        image = analytic_image(red_shift=2)
        old = independent_landmark_alignment(image, GEOMETRY, CAPTURE, "screen")
        self.assertFalse(old["settled_alignment_verified"])
        active = projected([0, 1])
        edges = linear_red_edges(image, tuple(round(active[i]) for i in (0, 2, 1, 3)), old["threshold_red_edges"])
        expected = projected([0.26, 0.74])
        for actual, value in zip(edges[:2], expected[:2]):
            self.assertAlmostEqual(actual - value, 2, delta=0.02)


if __name__ == "__main__":
    unittest.main()
