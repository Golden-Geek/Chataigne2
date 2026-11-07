import json
from pathlib import Path
import sys
import tempfile
import unittest

from PIL import Image

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from pending_overlay import verify


class PendingOverlayPixels(unittest.TestCase):
    def scene(self, folder, color):
        capture = {'capture_mode': 'visible owned foreground window', 'window_dpi': 120,
                   'size': [80, 80], 'client_desktop_origin': [12, 22],
                   'desktop_bounds': [10, 20, 90, 100]}
        geometry = {'expected': [20, 20], 'stale': [10, 10], 'dpr': 1.25, 'color': color}
        (folder / 'capture.json').write_text(json.dumps(capture), encoding='utf8')
        (folder / 'geometry.json').write_text(json.dumps(geometry), encoding='utf8')
        image = Image.new('RGB', (80, 80))
        image.putpixel((27, 27), tuple(color))
        image.save(folder / 'baseline.png')
        return image, geometry

    def test_blue_and_theme_cyan_use_the_actual_projected_physical_pixel(self):
        for color in [[112, 170, 255], [35, 187, 217]]:
            with self.subTest(color=color), tempfile.TemporaryDirectory() as root:
                folder = Path(root)
                self.scene(folder, color)
                result = verify(folder)
                self.assertLessEqual(result['current_edge_error_px'], 1)
                self.assertIsNone(result['stale_edge_distance_px'])

    def test_missing_wrong_color_or_stale_edges_cannot_pass(self):
        for failure in ['missing', 'wrong', 'stale']:
            with self.subTest(failure=failure), tempfile.TemporaryDirectory() as root:
                folder = Path(root)
                image, _ = self.scene(folder, [35, 187, 217])
                if failure == 'missing':
                    image.putpixel((27, 27), (0, 0, 0))
                elif failure == 'wrong':
                    image.putpixel((27, 27), (255, 0, 0))
                else:
                    image.putpixel((14, 14), (35, 187, 217))
                image.save(folder / 'baseline.png')
                with self.assertRaises(AssertionError):
                    verify(folder)

    def test_grey_background_and_wrong_dpi_cannot_be_control_evidence(self):
        for failure in ['grey', 'dpi']:
            with self.subTest(failure=failure), tempfile.TemporaryDirectory() as root:
                folder = Path(root)
                _, geometry = self.scene(folder, [35, 187, 217])
                geometry['color' if failure == 'grey' else 'dpr'] = [100] * 3 if failure == 'grey' else 1
                (folder / 'geometry.json').write_text(json.dumps(geometry), encoding='utf8')
                with self.assertRaises(AssertionError):
                    verify(folder)


if __name__ == '__main__':
    unittest.main()
