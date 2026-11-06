import sys
from pathlib import Path
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from typography_pixels import unorm_interval, srgb


class StorageIntervals(unittest.TestCase):
    def test_exact_integers_have_no_extra_storage_choice(self):
        for value in [0, 25, 36, 128, 255]:
            self.assertEqual(unorm_interval(value/255), (value/255, value/255))

    def test_fractional_choices_follow_integer_ulp_limit(self):
        self.assertEqual(unorm_interval(25.5/255), (25/255, 26/255))
        self.assertEqual(unorm_interval(36.542/255), (36/255, 37/255))
        self.assertEqual(unorm_interval(36.7/255), (37/255, 37/255))

    def test_resolve_propagates_only_possible_intermediate_choices(self):
        samples = [unorm_interval(v/255) for v in [36.542,36.542,25.5,25.5]]
        lower, upper = (sum(v[i] for v in samples)/4 for i in [0,1])
        low, high = unorm_interval(lower, upper)
        self.assertEqual((srgb(low), srgb(high)), (96,99))
        self.assertGreater(srgb(low), 95)
        self.assertLess(srgb(high), 100)


if __name__ == "__main__":
    unittest.main()
