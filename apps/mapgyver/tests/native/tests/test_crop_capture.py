import copy
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from crop_capture import verify_pointer_capture


class CropCaptureTests(unittest.TestCase):
    def fixture(self):
        return {'overflow': False, 'bounds': {'left': 10, 'top': 10, 'right': 30, 'bottom': 30},
                'events': [dict(type=kind, captured=cap, x=x, y=15, trusted=True, id=1)
                           for kind, cap, x in [('pointerdown', False, 15), ('gotpointercapture', True, 5),
                                                ('pointermove', True, 5), ('pointerup', True, 5),
                                                ('lostpointercapture', False, 5)]]}

    def test_trusted_captured_outside_sequence_passes(self):
        self.assertEqual(len(verify_pointer_capture(self.fixture())), 5)

    def test_untrusted_inside_uncaptured_and_missing_release_fail(self):
        for index, key, value in [(0, 'trusted', False), (2, 'x', 15), (2, 'captured', False),
                                  (3, 'captured', False), (4, 'captured', True), (4, 'id', 2)]:
            journal = copy.deepcopy(self.fixture())
            journal['events'][index][key] = value
            with self.assertRaises(AssertionError):
                verify_pointer_capture(journal)
        journal = self.fixture()
        journal['overflow'] = True
        with self.assertRaises(AssertionError):
            verify_pointer_capture(journal)


if __name__ == '__main__':
    unittest.main()
