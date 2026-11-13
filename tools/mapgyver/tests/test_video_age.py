"""Do not qualify missing, ambiguous or causally reversed Video age witnesses."""
import copy
import json
from pathlib import Path
import runpy
import unittest

ANALYZE = runpy.run_path(str(Path(__file__).parents[1] / 'video_age.py'))['analyze_video_age']


def fixture():
    token = {'device': {'runtime': 'renderer', 'generation': 1}, 'serial': 1}
    output = {'token': token, 'output': 3, 'outcome': 'Accepted',
              'spout_image': {'shared_handle': 100, 'frame': 0},
              'consumer_api': {'begin': 50, 'end': 100000}}
    use = {'token': token, 'texture': 1, 'stream': 'file', 'sequence': 4,
           'frame': {'pts_nanos': 100, 'start_nanos': 100, 'end_nanos': 200, 'publication_qpc': 20}}
    receiver = {'spout_image': output['spout_image'].copy(), 'receive_begin_qpc': 45,
                'completion_observed_qpc': 60}
    return {'outputs': [output], 'uses': [use], 'layouts': {'file': (1920, 1080)},
            'output_sources': {3: (1920, 1080)}, 'receivers': {3: [receiver]},
            'receiver_clocks': {3: 1000},
            'window': {'begin_qpc': 10, 'end_qpc': 100, 'frequency': 1000},
            'frames': {}}


def inspect(parts):
    return ANALYZE(**parts)['outputs'][0]


class VideoAgeTests(unittest.TestCase):
    def test_equal_extent_files_require_both_file_binding_and_actual_output_route(self):
        parts = fixture()
        token = json.dumps(parts['outputs'][0]['token'], sort_keys=True)
        generation = {'device': 'renderer', 'generation': 1}
        parts['frames'][token] = {'generation': generation}
        parts['output_routes'] = [{'generation': generation, 'output': 3, 'texture': 1, 'source_count': 1}]
        parts['output_streams'] = {3: {'file'}}
        other = copy.deepcopy(parts['uses'][0])
        other.update(stream='other-file', texture=2)
        other['frame']['publication_qpc'] = 49
        parts['uses'].append(other)
        parts['layouts']['other-file'] = (1920, 1080)
        self.assertEqual(inspect(parts)['publication_to_consumer_copy_upper_ms']['p95_upper_ms'], 42)
        parts['output_routes'][0]['texture'] = 2
        self.assertFalse(inspect(parts)['coverage_complete'], 'wrong file route must not borrow the expected publication')
        parts['output_routes'] = []
        self.assertEqual(inspect(parts)['missing_witnesses'], {'missing_or_incomplete_output_sources': 1})

    def test_real_completion_outward_quantization_and_post_api_preemption(self):
        result = inspect(fixture())
        self.assertTrue(result['coverage_complete'])
        age = result['publication_to_consumer_copy_upper_ms']
        self.assertEqual(age['accepted_images'], 1)
        self.assertEqual(age['p95_upper_ms'], 42)
        self.assertTrue(age['p95_within_target'])

    def test_equal_source_aliases_share_the_exact_publication(self):
        parts = fixture()
        alias = copy.deepcopy(parts['uses'][0])
        alias['texture'] = 2
        parts['uses'].append(alias)
        self.assertTrue(inspect(parts)['coverage_complete'])
        alias['sequence'] += 1
        self.assertEqual(inspect(parts)['missing_witnesses'], {'missing_or_ambiguous_source': 1})

    def test_independent_equal_sized_sources_cannot_be_joined_by_extent(self):
        parts = fixture()
        other = copy.deepcopy(parts['uses'][0])
        other['stream'] = 'other-file'
        parts['uses'].append(other)
        parts['layouts']['other-file'] = (1920, 1080)
        self.assertFalse(inspect(parts)['coverage_complete'])

    def test_different_actual_frame_metadata_is_ambiguous_even_with_equal_upload_counter(self):
        parts = fixture()
        alias = copy.deepcopy(parts['uses'][0])
        alias['frame']['publication_qpc'] += 1
        parts['uses'].append(alias)
        self.assertFalse(inspect(parts)['coverage_complete'])

    def test_cpu_file_does_not_borrow_another_sources_gpu_timing(self):
        parts = fixture()
        parts['output_sources'][3] = (3840, 2160)
        parts['layouts']['cpu-file'] = (3840, 2160)
        parts['uses'].append({**parts['uses'][0], 'stream': 'cpu-file', 'frame': None})
        self.assertEqual(inspect(parts)['missing_witnesses'], {'missing_file_publication': 1})

    def test_missing_publication_is_unbounded_not_an_upload_end_guess(self):
        for missing in (None, {'publication_qpc': None}):
            parts = fixture()
            parts['uses'][0]['frame'] = missing
            parts['uses'][0]['software_upload'] = {'cpu': {'end': 19}}
            age = inspect(parts)['publication_to_consumer_copy_upper_ms']
            self.assertEqual(age['unbounded_age_images'], 1)
            self.assertIsNone(age['p95_upper_ms'])

    def test_exact_sdk_version_and_handle_are_required(self):
        for field in ('frame', 'shared_handle'):
            parts = fixture()
            parts['receivers'][3][0]['spout_image'][field] += 1
            self.assertEqual(inspect(parts)['missing_witnesses'], {'missing_receiver_completion': 1})

    def test_duplicate_accepted_output_versions_are_ambiguous(self):
        parts = fixture()
        parts['outputs'].append(copy.deepcopy(parts['outputs'][0]))
        age = inspect(parts)['publication_to_consumer_copy_upper_ms']
        self.assertEqual(age['accepted_images'], 2)
        self.assertEqual(age['finite_age_images'], 0)

    def test_missing_or_mismatched_receiver_clock_stays_unknown(self):
        for clock in (None, 1001):
            parts = fixture()
            parts['receiver_clocks'][3] = clock
            self.assertFalse(inspect(parts)['coverage_complete'])

    def test_missing_completion_is_not_replaced_with_receive_or_api_end(self):
        parts = fixture()
        parts['receivers'][3][0]['completion_observed_qpc'] = None
        self.assertEqual(inspect(parts)['missing_witnesses'], {'missing_receiver_completion': 1})

    def test_missing_api_and_unassignable_phase_do_not_shrink_the_cohort(self):
        parts = fixture()
        parts['outputs'][0]['consumer_api'] = None
        self.assertEqual(inspect(parts)['missing_witnesses'], {'unassignable_phase': 1})
        token = json.dumps(parts['outputs'][0]['token'], sort_keys=True)
        parts['frames'][token] = {'wall': {'begin': 50, 'end': 80}}
        self.assertEqual(inspect(parts)['missing_witnesses'], {'missing_output_api': 1})

    def test_half_open_phase_and_empty_cohort_have_no_passing_percentile(self):
        parts = fixture()
        parts['outputs'][0]['consumer_api']['begin'] = 100
        result = inspect(parts)
        self.assertFalse(result['coverage_complete'])
        self.assertIsNone(result['publication_to_consumer_copy_upper_ms']['p95_within_target'])

    def test_unknown_images_contribute_unbounded_endpoints_to_percentiles(self):
        parts = fixture()
        parts['outputs'] *= 20
        # No unique version is a valid receiver join; do not compute a favorable subset.
        age = inspect(parts)['publication_to_consumer_copy_upper_ms']
        self.assertEqual(age['accepted_images'], 20)
        self.assertEqual(age['unbounded_age_images'], 20)
        self.assertIsNone(age['p95_upper_ms'])

    def test_partial_receiver_coverage_uses_the_whole_accepted_percentile_rank(self):
        for total, finite in ((10, 9), (20, 19)):
            parts = fixture()
            output, use, receiver = parts['outputs'][0], parts['uses'][0], parts['receivers'][3][0]
            parts['outputs'], parts['uses'], parts['receivers'][3] = [], [], []
            for index in range(total):
                row = copy.deepcopy(output)
                row['token']['serial'] = index + 1
                row['spout_image']['frame'] = index
                source = copy.deepcopy(use)
                source['token'] = row['token']
                parts['outputs'].append(row)
                parts['uses'].append(source)
                if index < finite:
                    witness = copy.deepcopy(receiver)
                    witness['spout_image'] = row['spout_image']
                    parts['receivers'][3].append(witness)
            age = inspect(parts)['publication_to_consumer_copy_upper_ms']
            self.assertEqual(age['accepted_images'], total)
            self.assertEqual(age['finite_age_images'], finite)
            self.assertEqual(age['p95_upper_ms'], None if total == 10 else 42)
            self.assertIsNone(age['maximum_upper_ms'])

    def test_source_join_requires_the_exact_renderer_device_and_batch(self):
        parts = fixture()
        parts['uses'][0]['token'] = copy.deepcopy(parts['uses'][0]['token'])
        parts['uses'][0]['token']['device']['generation'] += 1
        self.assertEqual(inspect(parts)['missing_witnesses'], {'missing_or_ambiguous_source': 1})

    def test_causal_reversal_and_invalid_frequency_are_rejected(self):
        for where, field, value in [('source', 'publication_qpc', 51),
                                    ('receiver', 'receive_begin_qpc', 61),
                                    ('receiver', 'completion_observed_qpc', 49)]:
            parts = fixture()
            target = parts['uses'][0]['frame'] if where == 'source' else parts['receivers'][3][0]
            target[field] = value
            with self.assertRaises(ValueError):
                inspect(parts)
        for invalid in (0, True):
            parts = fixture()
            parts['window']['frequency'] = invalid
            with self.assertRaises(ValueError):
                inspect(parts)


if __name__ == '__main__':
    unittest.main()
