"""Conservative clock endpoints for the Q06 source-age failure bound."""
from pathlib import Path
import runpy
import unittest

AGE = runpy.run_path(str(Path(__file__).parents[1] / "video_sample.py"))["publication_age_lower_bound"]
UPLOADS = runpy.run_path(str(Path(__file__).parents[1] / "video_sample.py"))["observed_source_uploads"]


class PublicationAgeBoundTests(unittest.TestCase):
    def test_source_worker_upload_before_publication_does_not_invent_an_age(self):
        self.assertIsNone(AGE({"cpu": {"end": 40}}, {"consumer_api": {"begin": 70}}, 1000,
                              precedes_publication=True))

    def test_source_upload_observers_share_one_actual_identity(self):
        upload = {"stream": "producer", "sequence": 2, "bytes": 8, "cpu": {"begin": 1, "end": 3}}
        other = {**upload, "sequence": 4}
        self.assertEqual(UPLOADS([{"software_upload": upload}, {"software_upload": upload.copy()},
                                  {"software_upload": None}, {}, {"software_upload": other}]), [upload, other])
        self.assertEqual(len(UPLOADS([{"software_upload": other}])), 1, "missing counter values are not invented uploads")

    def test_source_upload_identity_cannot_change_across_consumers(self):
        upload = {"stream": "producer", "sequence": 2, "bytes": 8}
        with self.assertRaises(ValueError):
            UPLOADS([{"software_upload": upload}, {"software_upload": {**upload, "bytes": 16}}])

    def test_inward_endpoints_do_not_invent_the_publication_time(self):
        value = AGE({"cpu": {"begin": 30, "end": 40}}, {"consumer_api": {"begin": 70, "end": 100}}, 1000)
        self.assertEqual(value, 30)
        self.assertLessEqual(value, (80 - 20) * 1000 / 1000)

    def test_post_api_preemption_cannot_inflate_a_failure_bound(self):
        value = AGE({"cpu": {"end": 40}}, {"consumer_api": {"begin": 50, "end": 100000}}, 1000)
        self.assertEqual(value, 10)

    def test_missing_coordinates_stay_unknown(self):
        self.assertIsNone(AGE({"cpu": None}, {"consumer_api": {"begin": 50}}, 1000))
        self.assertIsNone(AGE({"cpu": {"end": 40}}, {"consumer_api": None}, 1000))

    def test_invalid_order_and_frequency_are_rejected(self):
        with self.assertRaises(ValueError):
            AGE({"cpu": {"end": 40}}, {"consumer_api": {"begin": 39}}, 1000)
        with self.assertRaises(ValueError):
            AGE({"cpu": {"end": 40}}, {"consumer_api": {"begin": 50}}, 0)


if __name__ == "__main__":
    unittest.main()
