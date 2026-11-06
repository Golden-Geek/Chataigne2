import sys
import unittest
from pathlib import Path
from PIL import Image, ImageDraw

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from motion_frames import measure, match_native_controls


class MotionFrameTests(unittest.TestCase):
    def fixture(self, editor, shift=0):
        image = Image.new('RGB', (200, 150), 'black')
        draw = ImageDraw.Draw(image)
        bounds = (20, 15, 179, 134) if editor == 'source' else (52, 39, 147, 110)
        draw.rectangle(bounds, fill='red')
        draw.rectangle((40+shift, 30+shift, 160+shift, 120+shift), outline=(100, 160, 255))
        return image

    def test_independent_landmarks_measure_both_editors(self):
        for editor in ['source', 'screen']:
            result = measure(self.fixture(editor), (0, 0, 200, 150), editor)
            self.assertLessEqual(result['maximum_image_overlay_error_px'], 1)

    def test_shifted_overlay_and_wrong_reference_do_not_pass(self):
        self.assertGreater(measure(self.fixture('screen', 4), (0, 0, 200, 150), 'screen')['maximum_image_overlay_error_px'], 1)
        with self.assertRaises(AssertionError):
            measure(self.fixture('screen'), (0, 0, 200, 150), 'source')

    def test_missing_edges_fail(self):
        with self.assertRaises(AssertionError):
            measure(Image.new('RGB', (200, 150), 'red'), (0, 0, 200, 150), 'screen')

    def test_native_control_correspondence_and_bad_rectangle(self):
        frame = {'file': 'sample.png', 'blue_centres': [90.5, 330.5, 70.5, 250.5]}
        native = [{'hit': {'rect': {'x': 0, 'y': 0, 'width': 200, 'height': 150}}}]
        capture = {'client_desktop_origin': [10, 10], 'desktop_bounds': [0, 0, 400, 300]}
        geometry = {'viewport': {'dpr': 2}}
        result = match_native_controls([frame], native, capture, geometry)
        self.assertTrue(result['all_sampled_controls_spatially_matched'])
        self.assertFalse(result['exact_temporal_join'])
        native[0]['hit']['rect']['x'] = 10
        self.assertFalse(match_native_controls([frame], native, capture, geometry)['all_sampled_controls_spatially_matched'])
        with self.assertRaises(AssertionError):
            match_native_controls([frame], [], capture, geometry)


if __name__ == '__main__':
    unittest.main()
