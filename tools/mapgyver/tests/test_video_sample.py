"""Conservative clock endpoints for the Q06 source-age failure bound."""
from copy import deepcopy
from pathlib import Path
import runpy
import unittest

AGE = runpy.run_path(str(Path(__file__).parents[1] / "video_sample.py"))["publication_age_lower_bound"]
UPLOADS = runpy.run_path(str(Path(__file__).parents[1] / "video_sample.py"))["observed_source_uploads"]
MODULE = runpy.run_path(str(Path(__file__).parents[1] / "video_sample.py"))


def receiver_report():
    cases = [{'file': f'D:/{name}.mov', 'width': 3840, 'height': 2160, 'rate': 60,
              'frames': 240, 'sha256': name * 64, 'frame_code_offset': index * 2048}
             for index, name in enumerate(('a', 'b'))]
    return {'fixtures': {'cases': cases},
            'outputs': [{'fixture': deepcopy(case)} for case in [*cases, cases[0]]]}


class PublicationAgeBoundTests(unittest.TestCase):
    def test_independent_reference_receiver_reuses_the_canonical_file_not_a_third_case(self):
        report = receiver_report()
        assigned = {}
        for index, output in enumerate((3, 4, 5)):
            identity, fixture = MODULE['receiver_fixture'](report, index, {output}, assigned)
            assigned[identity] = fixture
        self.assertEqual([f['frame_code_offset'] for f in assigned.values()], [0, 2048, 0])
        self.assertIs(assigned[3], assigned[5])
        self.assertIsNot(assigned[3], assigned[4], 'equal extents must not collapse independent files')

    def test_missing_ambiguous_or_duplicate_native_owners_cannot_claim_reference_coverage(self):
        report = receiver_report()
        for owners, assigned in [(set(), {}), ({3, 4}, {}), ({3}, {3: 'already observed'})]:
            with self.assertRaises(ValueError):
                MODULE['receiver_fixture'](report, 2, owners, assigned)
        with self.assertRaisesRegex(ValueError, 'declared output'):
            MODULE['receiver_fixture'](report, 3, {6}, {})

    def test_unknown_duplicate_or_mutated_fixture_metadata_is_rejected(self):
        for field in ('file', 'width', 'height', 'rate', 'frames', 'sha256', 'frame_code_offset'):
            report = receiver_report()
            declared = report['outputs'][2]['fixture']
            declared[field] = 'wrong' if field in ('file', 'sha256') else declared[field] + 1
            with self.subTest(field=field), self.assertRaises(ValueError):
                MODULE['receiver_fixture'](report, 2, {5}, {})
        report = receiver_report()
        report['fixtures']['cases'].append(deepcopy(report['fixtures']['cases'][0]))
        with self.assertRaisesRegex(ValueError, 'canonical fixture'):
            MODULE['receiver_fixture'](report, 2, {5}, {})

    def test_equal_extent_files_keep_distinct_streams_rates_and_payloads(self):
        bindings = [{"stream": stream, "file": file, "reference": {"channel": stream}}
                    for stream, file in [('a', 'D:/a.mov'), ('a2', 'd:\\A.mov'), ('b', 'D:/b.mov')]]
        streams = MODULE['streams_by_file'](bindings)
        fixture = {'file': 'd:\\a.mov', 'width': 3840, 'height': 2160, 'rate': 60}
        uploads = [{'stream': stream, 'sequence': index, 'bytes': size, 'rows': 2160, 'row_pitch': 15360}
                   for index, (stream, size) in enumerate([('a', 100), ('a2', 200), ('b', 900)])]
        visible = [{'stream': stream, 'sequence': seq} for stream, seq in [('a', 1), ('a', 1), ('a2', 2), ('b', 3)]]
        selected = streams[MODULE['PureWindowsPath'](fixture['file'])]
        result = MODULE['source_phase_stats'](fixture, selected, uploads, visible, {}, 2)
        self.assertEqual(result['streams'], ['a', 'a2'])
        self.assertEqual(result['observed_upload_calls'], 2)
        self.assertEqual(result['upload_bytes'], 300)
        self.assertEqual(result['visible_publication_rate'], 1)
        missing = MODULE['source_phase_stats'](fixture, set(), uploads, visible, {}, 2)
        self.assertFalse(missing['source_binding_available'])
        self.assertEqual(missing['upload_bytes'], 0)
        with self.assertRaisesRegex(ValueError, 'binding'):
            MODULE['streams_by_file']([bindings[0], {**bindings[0], 'file': 'D:/b.mov'}])

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
