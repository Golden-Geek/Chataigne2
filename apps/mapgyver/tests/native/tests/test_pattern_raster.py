"""Retained native images exercise dense coverage and meaningful negative controls."""
import copy
from io import BytesIO
import json
from pathlib import Path
import sys
import unittest
import zipfile

import numpy as np
from PIL import Image

ROOT = Path(__file__).resolve().parents[5]
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import pattern_pixels
import pattern_raster


def samples(archive):
    with zipfile.ZipFile(archive) as source:
        for name in sorted(source.namelist()):
            if not name.endswith('/projection.json'):
                continue
            stage = name.removesuffix('/projection.json')
            yield (stage, json.loads(source.read(name)),
                   json.loads(source.read(stage + '/capture.json')),
                   source.read(stage + '/owned-window.png'))


class DenseNativePatternCaptures(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.cases = list(samples(ROOT / pattern_raster.ARCHIVE))

    def test_all_tiles_alpha_strips_and_screen_background_pass(self):
        self.assertEqual(len(self.cases), 18)
        pixels, alpha = 0, 0
        for name, metadata, capture, raw in self.cases:
            with self.subTest(stage=name), Image.open(BytesIO(raw)) as image:
                result = pattern_raster.analyze(image, metadata, capture)
                self.assertTrue(result['passed'], result['first_bad'])
                self.assertGreaterEqual(min(result['tile_pixels']), 9)
                self.assertLessEqual(result['maximum_rgb_error'], 2)
                if metadata['editor'] == 'screen':
                    self.assertGreater(result['background_pixels'], 0)
                pixels += result['pixel_count']
                alpha += result['alpha_pixels']
        self.assertEqual(pixels, 5830839)
        self.assertGreater(alpha, 700000)

    def test_swapped_channels_and_ignored_alpha_fail_every_case(self):
        for name, metadata, capture, raw in self.cases:
            with self.subTest(stage=name), Image.open(BytesIO(raw)) as image:
                red, green, blue = image.convert('RGB').split()
                swapped = Image.merge('RGB', (blue, green, red))
                self.assertFalse(pattern_raster.analyze(swapped, metadata, capture)['passed'])
                opaque = copy.deepcopy(metadata)
                for rgba in opaque['pattern']['rgba']:
                    rgba[3] = 255
                opaque_expected = pattern_raster.reference(opaque, capture, image.size)[0]
                _, mask, _, _, alpha = pattern_raster.reference(metadata, capture, image.size)
                changed = np.array(image.convert('RGB'))
                changed[mask & alpha] = opaque_expected[mask & alpha]
                self.assertFalse(pattern_raster.analyze(
                    Image.fromarray(changed), metadata, capture)['passed'])

    def test_single_pixel_corruption_outside_old_probes_is_detected(self):
        for name, metadata, capture, raw in self.cases:
            with self.subTest(stage=name), Image.open(BytesIO(raw)) as image:
                old = pattern_pixels.analyze(image, metadata, capture)
                _, eligible, inside, _, _ = pattern_raster.reference(metadata, capture, image.size)
                eligible &= inside
                for probe in old['probes']:
                    x, y = probe['xy']
                    eligible[y - 1:y + 2, x - 1:x + 2] = False
                # Select by geometry alone; never search actual pixels for a convenient color.
                y, x = np.argwhere(eligible)[0]
                changed = np.array(image.convert('RGB'))
                changed[y, x, 0] = (int(changed[y, x, 0]) + 128) % 256
                broken = Image.fromarray(changed)
                self.assertTrue(pattern_pixels.analyze(broken, metadata, capture)['passed'])
                result = pattern_raster.analyze(broken, metadata, capture)
                self.assertFalse(result['passed'])
                self.assertEqual(result['bad_pixels'], 1)

    def test_actual_dpi_mismatch_is_rejected(self):
        for name, metadata, capture, raw in self.cases:
            with self.subTest(stage=name), Image.open(BytesIO(raw)) as image:
                wrong = {**capture, 'window_dpi': capture['window_dpi'] + 24}
                with self.assertRaisesRegex(AssertionError, 'DPI'):
                    pattern_raster.analyze(image, metadata, wrong)

    def test_historical_intel_precision_failures_remain_failures(self):
        archive = ROOT / ('docs/progress/evidence/mapgyver-m05-preview-alpha-2026-11-06/'
                          'pattern-captures.zip')
        failures = 0
        for name, metadata, capture, raw in samples(archive):
            if not name.startswith('fixed-intel-125/') or not metadata['selected']:
                continue
            with self.subTest(stage=name), Image.open(BytesIO(raw)) as image:
                result = pattern_raster.analyze(image, metadata, capture)
                self.assertFalse(result['passed'])
                self.assertGreaterEqual(result['maximum_rgb_error'], 4)
                failures += 1
        self.assertEqual(failures, 6)


if __name__ == '__main__':
    unittest.main()
