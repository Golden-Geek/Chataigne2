"""The Source landmark's crop strokes lie inside its independent red source image."""
from pathlib import Path
import sys
import unittest

from PIL import Image, ImageDraw

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from visible_zoom import independent_landmark_alignment


GEOMETRY = {"viewport": {"width": 800, "height": 600, "dpr": 1},
            "active": {"x": 0, "y": 0, "width": 800, "height": 600}}
CAPTURE = {"capture_mode": "visible owned foreground window", "capture_usable_for_alignment": True,
           "size": [800, 600], "window_dpi": 96,
           "client_desktop_origin": [0, 0], "desktop_bounds": [0, 0, 800, 600]}


def source_with_toolbar():
    image = Image.new("RGB", (800, 600))
    draw = ImageDraw.Draw(image)
    draw.rectangle((80, 60, 719, 539), fill=(255, 0, 0))
    draw.rectangle((160, 120, 639, 479), outline=(112, 170, 255))
    draw.rectangle((350, 10, 450, 11), fill=(112, 170, 255))
    return image


class SourceLandmarkToolbar(unittest.TestCase):
    def test_toolbar_outside_red_source_does_not_replace_a_crop_edge(self):
        result = independent_landmark_alignment(source_with_toolbar(), GEOMETRY, CAPTURE, "source")
        self.assertTrue(result["settled_alignment_verified"])
        self.assertFalse(result["input_latency_verified"])

    def test_toolbar_cannot_supply_a_missing_crop_stroke(self):
        image = source_with_toolbar()
        ImageDraw.Draw(image).rectangle((158, 118, 162, 481), fill=(255, 0, 0))
        with self.assertRaises(AssertionError):
            independent_landmark_alignment(image, GEOMETRY, CAPTURE, "source")


if __name__ == "__main__":
    unittest.main()
