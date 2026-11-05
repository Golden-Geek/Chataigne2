import copy
from pathlib import Path
import runpy
import unittest

ANALYZE = runpy.run_path(str(Path(__file__).parents[1] / 'production_output_queue.py'))['analyze_output_queue']


def fixture():
    frames, outputs = [], []
    for serial in (1, 2):
        token = {'device': {'runtime': 'renderer', 'generation': 1}, 'serial': serial}
        frames.append({'token': token})
        for output in (1, 2, 3):
            image = {'shared_handle': 400, 'frame': serial}
            queue = {'SpoutLatestImage': {'image': image}} if output == 2 else {
                'Window': {'buffer_count': 2, 'maximum_frame_latency': 1,
                           'sync_interval': 0, 'admission_permit': True}}
            outputs.append({'token': token, 'output': output, 'outcome': 'Accepted',
                            'consumer_api': {'begin': serial * 10, 'end': serial * 10 + 1},
                            'publication_queue': queue, 'spout_image': image if output == 2 else None,
                            'window_present': None if output == 2 else {
                                'generation': 1, 'addresses': [output * 10 + i for i in (1, 2, 3)]}})
    return frames, outputs


class OutputQueueTests(unittest.TestCase):
    def test_capacity_proves_the_ceiling_without_inventing_instantaneous_occupancy(self):
        result = ANALYZE(*fixture(), healthy=True)
        self.assertTrue(result['coverage_complete'])
        self.assertIs(result['observed_within_target'], True)
        self.assertEqual([r['maximum_verified_capacity_frames'] for r in result['outputs']], [2, 1, 2])
        self.assertTrue(all(r['exact_pending_frames'] is None for r in result['outputs']))

    def test_missing_rows_metadata_api_or_untrusted_telemetry_remain_unknown(self):
        for mutate in (
            lambda rows: rows.pop(),
            lambda rows: rows[0].pop('publication_queue'),
            lambda rows: rows[0].__setitem__('consumer_api', None),
            lambda rows: rows[0].__setitem__('consumer_api', {'begin': 20, 'end': 10}),
            lambda rows: rows[0].__setitem__('window_present', None),
        ):
            frames, outputs = fixture()
            mutate(outputs)
            self.assertIsNone(ANALYZE(frames, outputs, healthy=True)['observed_within_target'])
        self.assertIsNone(ANALYZE(*fixture(), healthy=False)['observed_within_target'])

    def test_waitable_permit_latency_and_supported_presentation_are_required(self):
        for field, value in [('admission_permit', False), ('maximum_frame_latency', 2),
                             ('maximum_frame_latency', True), ('sync_interval', 2),
                             ('sync_interval', False), ('buffer_count', 0), ('buffer_count', 1)]:
            frames, outputs = fixture()
            outputs[0]['publication_queue']['Window'][field] = value
            self.assertIsNone(ANALYZE(frames, outputs, healthy=True)['observed_within_target'])

    def test_refresh_synchronization_keeps_the_same_verified_capacity(self):
        frames, outputs = fixture()
        for output in outputs:
            if output['output'] != 2:
                output['publication_queue']['Window']['sync_interval'] = 1
        result = ANALYZE(frames, outputs, healthy=True)
        self.assertTrue(result['coverage_complete'])
        self.assertIs(result['observed_within_target'], True)
        self.assertEqual([r['maximum_verified_capacity_frames'] for r in result['outputs']], [2, 1, 2])

    def test_shared_image_must_be_the_same_accepted_copy_not_an_old_or_guessed_version(self):
        for replacement in [None, {'shared_handle': 401, 'frame': 1},
                            {'shared_handle': 400, 'frame': 2},
                            {'shared_handle': True, 'frame': 1},
                            {'shared_handle': 400, 'frame': 1 << 31}]:
            frames, outputs = fixture()
            outputs[1]['publication_queue']['SpoutLatestImage']['image'] = replacement
            self.assertIsNone(ANALYZE(frames, outputs, healthy=True)['observed_within_target'])

    def test_bigger_capacity_is_not_a_measured_occupancy_failure(self):
        frames, outputs = fixture()
        for row in outputs:
            if row['output'] == 1:
                row['publication_queue']['Window']['buffer_count'] = 3
        result = ANALYZE(frames, outputs, healthy=True)
        self.assertTrue(result['coverage_complete'])
        self.assertIsNone(result['observed_within_target'])
        self.assertEqual(result['outputs'][0]['maximum_verified_capacity_frames'], 3)

    def test_window_recreation_fences_layout_even_if_interface_addresses_are_reused(self):
        frames, outputs = fixture()
        outputs[3]['publication_queue']['Window']['sync_interval'] = 1
        with self.assertRaisesRegex(ValueError, 'layout changes'):
            ANALYZE(frames, outputs, healthy=True)
        outputs[3]['window_present']['generation'] = 2
        self.assertIs(ANALYZE(frames, outputs, healthy=True)['observed_within_target'], True)
        frames[1]['token']['device']['generation'] = 2
        outputs[3]['window_present']['generation'] = 1
        self.assertIs(ANALYZE(frames, outputs, healthy=True)['observed_within_target'], True)

    def test_busy_attempts_publish_nothing_and_do_not_erase_previous_bound(self):
        frames, outputs = fixture()
        outputs[3]['outcome'] = 'Busy'
        outputs[3]['publication_queue'] = None
        self.assertIs(ANALYZE(frames, outputs, healthy=True)['observed_within_target'], True)
        outputs[0]['outcome'] = 'Busy'
        outputs[0]['publication_queue'] = None
        self.assertIsNone(ANALYZE(frames, outputs, healthy=True)['observed_within_target'])

    def test_duplicates_and_falsely_published_rejections_are_rejected(self):
        frames, outputs = fixture()
        with self.assertRaisesRegex(ValueError, 'duplicate output'):
            ANALYZE(frames, [*outputs, copy.deepcopy(outputs[0])], healthy=True)
        with self.assertRaisesRegex(ValueError, 'duplicate publication frame'):
            ANALYZE([*frames, frames[0]], outputs, healthy=True)
        outputs[0]['outcome'] = 'Busy'
        with self.assertRaisesRegex(ValueError, 'rejected output'):
            ANALYZE(frames, outputs, healthy=True)
        self.assertIsNone(ANALYZE([], [], healthy=True)['observed_within_target'])


if __name__ == '__main__':
    unittest.main()
