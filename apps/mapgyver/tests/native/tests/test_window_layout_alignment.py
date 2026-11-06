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
SPEC = importlib.util.spec_from_file_location("visible_zoom", NATIVE / "visible_zoom.py")
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class WindowLayoutAlignment(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.samples = []
        cls.hidden = []
        archives = (
            ("mapgyver-m05-window-layout-2026-11-06/native-layout.zip",
             ("auto-screen-100", "auto-screen-125", "auto-source-100", "auto-source-125")),
            ("mapgyver-m05-dpi-2026-11-06/native-dpi.zip",
             ("screen-150", "source-150", "screen-200", "source-200")),
            ("mapgyver-m05-editor-layout-2026-11-06/editor-layout.zip",
             ("screen-100-corrected", "source-100", "screen-125", "source-125")),
        )
        for name, cases in archives:
            with zipfile.ZipFile(ROOT / "docs/progress/evidence" / name) as source:
                for case in cases:
                    layout = "editor-layout.json" if "editor-layout.zip" in name else "window-layout.json"
                    result = json.loads(source.read(case + "/" + layout))
                    if "hidden" in result:
                        cls.hidden.append((case, result["hidden"],
                            source.read(case + "/editor-hidden/baseline.png"),
                            source.read(case + "/initial/baseline.png")))
                    for stage in result["stages"]:
                        cls.samples.append((case + "/" + stage["stage"],
                            "screen" if "screen" in case else "source", stage,
                            source.read(case + "/" + stage["stage"] + "/baseline.png")))

    def test_all_forty_eight_real_editor_states_have_independent_pixel_alignment(self):
        self.assertEqual(len(self.samples), 48)
        for name, editor, stage, raw in self.samples:
            with self.subTest(name=name), Image.open(BytesIO(raw)) as image:
                result = MODULE.independent_landmark_alignment(
                    image, stage["geometry"], stage["capture"], editor)
                self.assertTrue(result["settled_alignment_verified"])
                self.assertFalse(result["input_latency_verified"])

    def test_three_physical_pixel_projection_error_rejects_every_window_state(self):
        for name, editor, stage, raw in self.samples:
            with self.subTest(name=name), Image.open(BytesIO(raw)) as image:
                geometry = copy.deepcopy(stage["geometry"])
                geometry["active"]["x"] += 3 / geometry["viewport"]["dpr"]
                result = MODULE.independent_landmark_alignment(image, geometry, stage["capture"], editor)
                self.assertFalse(result["settled_alignment_verified"])

    def test_mismatched_real_window_dpi_cannot_qualify_scaling(self):
        for name, editor, stage, raw in self.samples:
            with self.subTest(name=name), Image.open(BytesIO(raw)) as image:
                capture = copy.deepcopy(stage["capture"])
                capture["window_dpi"] += 24
                with self.assertRaises(AssertionError):
                    MODULE.independent_landmark_alignment(image, stage["geometry"], capture, editor)

    def test_hidden_editor_retires_pixels_that_exist_in_the_visible_control(self):
        self.assertEqual(len(self.hidden), 4)
        for name, result, hidden, visible in self.hidden:
            with self.subTest(name=name):
                self.assertGreater(result["pixels"], 10000)
                self.assertEqual(result["native_red_pixels"], 0)
                for raw, expected in ((hidden, False), (visible, True)):
                    with Image.open(BytesIO(raw)) as image:
                        left, top, right, bottom = result["roi"]
                        self.assertTrue(0 <= left < right <= image.width)
                        self.assertTrue(0 <= top < bottom <= image.height)
                        region = image.convert("RGB").crop(result["roi"])
                        count = sum(MODULE.red(pixel) for pixel in region.get_flattened_data())
                        if expected:
                            self.assertGreater(count, 10000)
                        else:
                            self.assertEqual(count, 0)


if __name__ == "__main__":
    unittest.main()
