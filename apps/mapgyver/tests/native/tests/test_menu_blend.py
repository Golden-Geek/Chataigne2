import sys
import unittest
from pathlib import Path
from PIL import Image

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from menu_blend import replace_layer, source_over, verify


class MenuBlendTests(unittest.TestCase):
    def fixture(self):
        clears = [Image.new('RGB', (12, 12), colour) for colour in ['red', 'blue']]
        layer = Image.new('RGBA', (12, 12), (16, 16, 16, 128))
        frames = [Image.new('RGB', (12, 12), source_over(colour, (16, 16, 16, 128)))
                  for colour in [(255, 0, 0), (255, 0, 0), (0, 0, 255), (255, 0, 0)]]
        return clears, frames, layer, [(x, y) for y in range(1, 11) for x in range(1, 11)], (0, 0)

    def test_reference_and_held_out_colour_pass(self):
        self.assertEqual(source_over((255, 0, 0), (16, 16, 16, 128)), (135, 8, 8))
        self.assertLessEqual(verify(*self.fixture())['held_out_blue_reference_max_rgb_error'], 2)

    def test_existing_selection_layer_is_not_composited_twice(self):
        base = (120, 170, 250, 24)
        menu = (24, 25, 26, 200)
        for native in [(255, 0, 0), (0, 0, 255)]:
            clear = source_over(native, base)
            expected = source_over(native, menu)
            actual = replace_layer(clear, base, menu)
            self.assertLessEqual(max(abs(a-b) for a, b in zip(actual, expected)), 1)
            self.assertGreater(max(abs(a-b) for a, b in zip(source_over(clear, menu), expected)), 2)

    def test_leaked_stale_and_opaque_layer_controls_fail(self):
        args = list(self.fixture())
        args[1][2] = args[0][1]
        with self.assertRaisesRegex(AssertionError, 'reference'):
            verify(*args)
        args = list(self.fixture())
        args[1][1] = Image.new('RGB', (12, 12), 'white')
        with self.assertRaisesRegex(AssertionError, 'not stable'):
            verify(*args)
        args = list(self.fixture())
        args[2] = Image.new('RGBA', (12, 12), (16, 16, 16, 255))
        with self.assertRaisesRegex(AssertionError, 'translucent'):
            verify(*args)


if __name__ == '__main__':
    unittest.main()
