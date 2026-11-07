"""A fast capture alone cannot pass a slow image or displaced-control cohort."""
import copy
import math
from pathlib import Path
import sys
import unittest

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from motion_cadence import audit


def fixture(slow_image=False, displacement=0, occluded=False):
    width, height, count = 192, 8, 481
    frames, timestamps, raw = [], [], []
    for index in range(count):
        time = index / 60
        blue = 42 + round(20 * (2 / math.pi) * math.asin(math.sin(time * math.pi * 2)))
        image_time = (index // 6 * 6) / 60 if slow_image else time
        red = 42 + round(20 * (2 / math.pi) * math.asin(math.sin(image_time * math.pi * 2)))
        red += 20 + .5 + displacement
        linear = np.clip(.5 + (np.arange(width) + .5 - red) / 3, 0, 1)
        encoded = np.where(linear <= .0031308, linear * 12.92, 1.055 * linear ** (1 / 2.4) - .055)
        row = np.zeros((width, 4), dtype=np.uint8)
        row[:, 2] = np.rint(encoded * 255).astype(np.uint8)
        row[:, 3] = 255
        row[blue] = [255, 170, 120, 255]
        if occluded:
            row[int(red) + 4] = [255, 170, 120, 255]
        raw.append(np.tile(row[None, :, :], (height, 1, 1)).tobytes())
        frames.append({'serial': index, 'arrival_ns': str(1_000_000 + round(time * 1e9))})
        timestamps.append({'serial': index, 'ticks': str(round(time * 1e7))})
    events = [{'type': 'pointerdown', 'trusted': True, 'buttons': 1, 'at': 0}]
    events += [{'type': 'pointermove', 'trusted': True, 'buttons': 1, 'at': index + 1} for index in range(500)]
    events.append({'type': 'pointerup', 'trusted': True, 'buttons': 0, 'at': 501})
    geometry = {'window_dpi': 120}
    result = {
        'failure': None, 'problem': None, 'exit': {'code': 0, 'signal': None},
        'geometry': geometry, 'after': geometry, 'region': {'width': width, 'height': height},
        'projection': {'dpr': 1.25, 'corners': [[0, 0], [160, 0], [160, 120], [0, 120]]},
        'frames': frames, 'timestamps': timestamps,
        'begin_ns': '1000000', 'end_ns': '8001000000', 'requested': 500, 'events': events,
        'expected_separation_px': 20
    }
    return result, b''.join(raw)


class MotionCadenceReplay(unittest.TestCase):
    def test_complete_fast_independent_edges_pass(self):
        result = audit(*fixture())
        self.assertTrue(result['both_at_least_30_updates_per_second'])
        self.assertTrue(result['alignment_within_one_pixel'])
        self.assertGreater(result['image']['updates_per_second'], 40)

    def test_fast_capture_and_controls_cannot_hide_slow_video(self):
        result = audit(*fixture(slow_image=True))
        self.assertFalse(result['both_at_least_30_updates_per_second'])
        self.assertLess(result['image']['updates_per_second'], 15)
        self.assertGreater(result['controls']['updates_per_second'], 40)

    def test_displaced_image_cannot_pass_alignment(self):
        result = audit(*fixture(displacement=2))
        self.assertTrue(result['both_at_least_30_updates_per_second'])
        self.assertFalse(result['alignment_within_one_pixel'])

    def test_an_occluded_landmark_never_qualifies_by_discarding_ambiguous_frames(self):
        result = audit(*fixture(occluded=True))
        self.assertGreater(result['ambiguous_landmark_frames'], 0)
        self.assertFalse(result['alignment_within_one_pixel'])
        self.assertFalse(result['both_at_least_30_updates_per_second'])

    def test_missing_images_and_forged_timestamps_fail(self):
        result, raw = fixture()
        with self.assertRaises(AssertionError):
            audit(result, raw[:-1])
        with self.assertRaises(AssertionError):
            audit(result, bytes(len(raw)))
        forged = copy.deepcopy(result)
        forged['timestamps'][1]['ticks'] = forged['timestamps'][0]['ticks']
        with self.assertRaises(AssertionError):
            audit(forged, raw)

    def test_untrusted_input_or_failed_capture_cannot_qualify(self):
        result, raw = fixture()
        result['events'][5]['trusted'] = False
        with self.assertRaises(AssertionError):
            audit(result, raw)
        result['events'][5]['trusted'] = True
        result['exit']['code'] = 1
        with self.assertRaises(AssertionError):
            audit(result, raw)

    def test_expected_geometry_and_duplicate_timestamp_metadata_cannot_be_forged(self):
        result, raw = fixture()
        result['expected_separation_px'] = 22
        with self.assertRaises(AssertionError):
            audit(result, raw)
        result, raw = fixture()
        result['timestamps'][1]['serial'] = 0
        with self.assertRaises(AssertionError):
            audit(result, raw)


if __name__ == '__main__':
    unittest.main()
