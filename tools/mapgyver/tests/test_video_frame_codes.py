import copy
from pathlib import Path
import runpy
import unittest

ANALYZE = runpy.run_path(str(Path(__file__).parents[1] / 'video_frame_codes.py'))['analyze_frame_codes']


def fixture():
    token = {'device': 'renderer', 'serial': 1}
    generation = {'device': 'renderer', 'generation': 2}
    marker = {'bits': 12, 'origin': [24, 24], 'step': 24, 'sentinel': [4, 24], 'threshold': 128}
    version = {'shared_handle': 9, 'frame': 20}
    words = [0x56464331, 1, 0xffffffff, 12] + [0] * 8
    for bit in range(12):
        words[4 + bit // 4] |= (235 if bit == 0 else 16) << ((bit % 4) * 8)
    events = {'SourceUse': [{'token': token, 'texture': 100, 'stream': 'file-a', 'sequence': 10,
                             'descriptor': {'extent': {'width': 1920, 'height': 1080}},
                             'frame': {'pts_nanos': 33_333_333}}],
              'OutputSource': [{'generation': generation, 'output': 3, 'texture': 100, 'source_count': 1}],
              'Output': [{'token': token, 'output': 3, 'outcome': 'Accepted', 'spout_image': version,
                          'consumer_api': {'begin': 15, 'end': 16}}]}
    receivers = {3: [{'row': 0, 'spout_image': version, 'raw_words': words,
                     'binary_frame_code': {'Ok': 1}, 'receive_begin_qpc': 16, 'completion_observed_qpc': 18}]}
    fixtures = {3: {'rate': 30, 'frames': 120, 'width': 1920, 'height': 1080, 'marker': marker}}
    return events, receivers, fixtures, {3: marker}, {'begin_qpc': 10, 'end_qpc': 20}, {
        __import__('json').dumps(token, sort_keys=True): {'generation': generation, 'wall': {'begin': 14}}}


def result(args):
    return ANALYZE(*args)['outputs'][0]


class VideoFrameCodeTests(unittest.TestCase):
    def test_mixed_constant_rates_and_loop_zero_use_original_pts(self):
        for rate, pts in ((24, 41_666_667), (30, 33_333_333), (60, 16_666_667)):
            args = fixture()
            args[2][3]['rate'] = rate
            args[0]['SourceUse'][0]['frame']['pts_nanos'] = pts
            self.assertTrue(result(args)['coverage_complete'])
        args = fixture()
        args[0]['SourceUse'][0]['frame']['pts_nanos'] = 0
        row = args[1][3][0]
        row['raw_words'][1] = 0
        row['raw_words'][4] = (row['raw_words'][4] & 0xffffff00) | 16
        row['binary_frame_code'] = {'Ok': 0}
        self.assertTrue(result(args)['coverage_complete'])

    def test_two_outputs_with_equal_dimensions_follow_their_own_prepared_sources(self):
        args = fixture()
        source = copy.deepcopy(args[0]['SourceUse'][0])
        source.update(texture=200, stream='second-file')
        args[0]['SourceUse'].append(source)
        route = copy.deepcopy(args[0]['OutputSource'][0])
        route.update(output=4, texture=200)
        args[0]['OutputSource'].append(route)
        out = copy.deepcopy(args[0]['Output'][0])
        out.update(output=4, spout_image={'shared_handle': 10, 'frame': 20})
        args[0]['Output'].append(out)
        args[1][4] = [copy.deepcopy(args[1][3][0])]
        args[1][4][0]['spout_image'] = out['spout_image']
        args[2][4], args[3][4] = args[2][3], args[3][3]
        self.assertTrue(all(row['coverage_complete'] for row in ANALYZE(*args)['outputs']))

    def test_exact_version_and_quantized_pts_match(self):
        value = result(fixture())
        self.assertTrue(value['coverage_complete'])
        self.assertEqual(value['matched_images'], 1)

    def test_unrelated_same_sized_file_is_not_selected_by_dimensions(self):
        args = fixture()
        other = copy.deepcopy(args[0]['SourceUse'][0])
        other.update(texture=200, stream='file-b', frame={'pts_nanos': 99_000_000})
        args[0]['SourceUse'].append(other)
        self.assertTrue(result(args)['coverage_complete'])

    def test_wrong_pixels_preserve_the_exact_counter_and_pts(self):
        args = fixture()
        args[0]['SourceUse'][0]['frame']['pts_nanos'] = 66_666_667
        value = result(args)
        self.assertEqual(value['mismatched_images'], 1)
        self.assertEqual(value['mismatch_examples'][0]['expected'], 2)
        self.assertFalse(value['coverage_complete'])

    def test_missing_or_duplicate_versions_do_not_shrink_the_cohort(self):
        for mode in ('missing', 'receiver duplicate', 'sender duplicate', 'route missing', 'route incomplete', 'contract', 'completion'):
            args = fixture()
            if mode == 'missing': args[1][3].clear()
            elif mode == 'receiver duplicate': args[1][3].append(copy.deepcopy(args[1][3][0]))
            elif mode == 'sender duplicate': args[0]['Output'].append(copy.deepcopy(args[0]['Output'][0]))
            elif mode == 'route missing': args[0]['OutputSource'].clear()
            elif mode == 'route incomplete': args[0]['OutputSource'][0]['source_count'] = 2
            elif mode == 'contract': args[3].clear()
            elif mode == 'completion': args[1][3][0]['completion_observed_qpc'] = None
            value = result(args)
            self.assertGreater(value['accepted_images'], 0)
            self.assertEqual(value['matched_images'], 0, mode)
            self.assertFalse(value['coverage_complete'])

    def test_aliases_collapse_only_the_same_publication_and_real_sources_remain_ambiguous(self):
        args = fixture()
        args[0]['SourceUse'].append(copy.deepcopy(args[0]['SourceUse'][0]))
        self.assertTrue(result(args)['coverage_complete'])
        args[0]['SourceUse'][1]['stream'] = 'other-source'
        self.assertFalse(result(args)['coverage_complete'])

    def test_corrupt_words_or_fabricated_decoded_values_are_not_pixel_proof(self):
        for word in (0, 1, 2, 3, 4, 11):
            args = fixture()
            args[1][3][0]['raw_words'][word] ^= 0xffffffff
            self.assertEqual(result(args)['invalid_markers'], 1)
        args = fixture()
        args[1][3][0]['binary_frame_code'] = {'Ok': True}
        self.assertEqual(result(args)['invalid_markers'], 1)

    def test_out_of_range_pts_and_reversed_clock_cannot_pass(self):
        args = fixture()
        args[0]['SourceUse'][0]['frame']['pts_nanos'] = 5_000_000_000
        with self.assertRaises(ValueError): result(args)
        args = fixture()
        args[1][3][0]['receive_begin_qpc'] = 19
        with self.assertRaises(ValueError): result(args)
