"""Negative controls for the physical-pixel oracle, independent of a live GPU."""
import copy
import json
from pathlib import Path
import sys
import tempfile
import unittest

from PIL import Image, ImageDraw

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from verify_editor_surround import verify


class EditorSurroundTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.directory = Path(self.temporary.name)
        (self.directory / "source").mkdir()
        image = Image.new("RGB", (180, 180), (31, 32, 33))
        draw = ImageDraw.Draw(image)
        draw.rectangle((40, 40, 119, 119), fill=(255, 0, 0))
        draw.rectangle((57, 57, 62, 62), fill=(220, 234, 255))
        image.save(self.directory / "source/baseline.png")
        active = {"x": 40, "y": 40, "width": 80, "height": 80}
        self.metadata = {
            "label": "Surface source view", "dpr": 1,
            "view": {"x": 0, "y": 0, "width": 180, "height": 180},
            "active": active, "theme": {"rgba": [31, 32, 33, 255]},
            "hits": [{"id": "crop:0", "point": [60, 60],
                      "bounds": {"x": 56, "y": 56, "width": 8, "height": 8},
                      "native": {"sample": {"hit": "crop:0", "rect": copy.copy(active)}}}]
        }
        self.capture = {
            "capture_mode": "visible owned foreground window", "window_dpi": 96,
            "client_desktop_origin": [0, 0], "desktop_bounds": [0, 0, 180, 180],
            "cursor_desktop_position": [-100, -100]
        }

    def run_oracle(self, stage="source"):
        (self.directory / f"{stage}-dom.json").write_text(json.dumps(self.metadata))
        (self.directory / stage).mkdir(exist_ok=True)
        if stage != "source":
            (self.directory / stage / "baseline.png").write_bytes(
                (self.directory / "source/baseline.png").read_bytes())
        (self.directory / stage / "capture.json").write_text(json.dumps(self.capture))
        return verify(self.directory, stage)

    def test_reference_pixels_and_metadata_pass(self):
        result = self.run_oracle()
        self.assertTrue(result["image_edges_verified"])
        self.assertEqual(result["physical_handles"], 1)
        self.assertGreater(result["gray_samples"], 0)

    def test_three_pixel_image_shift_fails_even_with_matching_native_rect(self):
        self.metadata["active"]["x"] += 3
        self.metadata["hits"][0]["native"]["sample"]["rect"]["x"] += 3
        with self.assertRaises(AssertionError):
            self.run_oracle()

    def test_three_pixel_overlay_shift_fails(self):
        self.metadata["hits"][0]["point"][0] += 3
        with self.assertRaisesRegex(AssertionError, "handle"):
            self.run_oracle()

    def test_wrong_dpi_and_background_fail(self):
        self.metadata["dpr"] = 1.25
        with self.assertRaises(AssertionError):
            self.run_oracle()
        self.metadata["dpr"] = 1
        self.metadata["theme"]["rgba"] = [35, 36, 37, 255]
        with self.assertRaisesRegex(AssertionError, "probe"):
            self.run_oracle()

    def test_cursor_highlight_in_or_near_viewport_rejects_capture(self):
        for cursor in [[90, 90], [-20, 90]]:
            self.capture["cursor_desktop_position"] = cursor
            with self.assertRaisesRegex(AssertionError, "pointer"):
                self.run_oracle()

    def test_missing_source_uses_visible_dom_hits_and_retires_native_authority(self):
        self.metadata["preview"] = {"dataset": {"nativeOverlays": "false"}}
        hit = self.metadata["hits"][0]
        hit["native"]["sample"]["hit"] = None
        hit["domHit"] = hit["id"]
        result = self.run_oracle("source-missing")
        self.assertEqual(result["native_hits"], 0)
        self.assertEqual(result["dom_fallback_hits"], 1)
        self.assertEqual(result["physical_handles"], 1)
        hit["domHit"] = "wrong"
        with self.assertRaises(AssertionError):
            self.run_oracle("source-missing")

    def test_screen_backing_is_black_only_inside_content(self):
        self.metadata["label"] = "Screen view"
        self.metadata["hits"] = []
        image = Image.new("RGB", (180, 180), (31, 32, 33))
        draw = ImageDraw.Draw(image)
        draw.rectangle((40, 40, 119, 119), fill="black")
        draw.rectangle((60, 60, 99, 99), fill="red")
        image.save(self.directory / "source/baseline.png")
        self.assertGreater(self.run_oracle()["black_backing_samples"], 0)
        image.putpixel((112, 112), (31, 32, 33))
        image.save(self.directory / "source/baseline.png")
        with self.assertRaisesRegex(AssertionError, "black_backing"):
            self.run_oracle()


if __name__ == "__main__":
    unittest.main()
