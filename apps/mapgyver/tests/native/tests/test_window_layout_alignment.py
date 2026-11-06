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
        archive = ROOT / "docs/progress/evidence/mapgyver-m05-window-layout-2026-11-06/native-layout.zip"
        with zipfile.ZipFile(archive) as source:
            for case in ("auto-screen-100", "auto-screen-125", "auto-source-100", "auto-source-125"):
                result = json.loads(source.read(case + "/window-layout.json"))
                for stage in result["stages"]:
                    cls.samples.append((case + "/" + stage["stage"],
                        "screen" if "screen" in case else "source", stage,
                        source.read(case + "/" + stage["stage"] + "/baseline.png")))

    def test_all_twelve_real_window_states_have_independent_pixel_alignment(self):
        self.assertEqual(len(self.samples), 12)
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


if __name__ == "__main__":
    unittest.main()
