import sys
import unittest
from pathlib import Path
from PIL import Image, ImageDraw

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from opaque_layers import verify


class OpaqueLayerTests(unittest.TestCase):
    def fixture(self):
        red = Image.new('RGB', (40, 24), 'red')
        blue = Image.new('RGB', (40, 24), 'blue')
        layers = []
        for source in [red, red, blue, red]:
            image = source.copy()
            ImageDraw.Draw(image).rectangle((8, 2, 31, 21), fill=(35, 35, 35))
            layers.append(image)
        return [red, blue, *layers], [(0, 0, 24, 24), (26, 0, 40, 16)], (8, 2, 32, 22)

    def test_real_witnesses_and_opaque_cover_pass(self):
        images, views, dialog = self.fixture()
        self.assertTrue(verify(*images, views, dialog)['opaque_layer_verified'])

    def test_transparent_cover_and_missing_colour_witness_fail(self):
        images, views, dialog = self.fixture()
        images[4] = images[1]
        with self.assertRaisesRegex(AssertionError, 'leaks'):
            verify(*images, views, dialog)
        images, views, dialog = self.fixture()
        images[1] = images[0]
        with self.assertRaisesRegex(AssertionError, 'red and blue'):
            verify(*images, views, dialog)

    def test_unrelated_overlay_and_unstable_controls_fail(self):
        images, views, dialog = self.fixture()
        with self.assertRaisesRegex(AssertionError, 'overlap'):
            verify(*images, views, (50, 50, 60, 60))
        ImageDraw.Draw(images[3]).rectangle(dialog, fill='white')
        with self.assertRaisesRegex(AssertionError, 'not stable'):
            verify(*images, views, dialog)


if __name__ == '__main__':
    unittest.main()
