from copy import deepcopy
from pathlib import Path
import runpy
import unittest

MODULE = runpy.run_path(str(Path(__file__).parents[1] / 'video_gpu_path.py'))
ANALYZE = MODULE['analyze_video_gpu_path']


def token(runtime, serial=1):
    return {'device': {'runtime': runtime, 'generation': 1}, 'serial': serial}


def distribution(values):
    return {'samples': len(values), 'p95_ms': max(values), 'p99_ms': max(values)} if values else None


def fixture():
    renderer, producer = token('renderer'), token('producer', 7)
    generation = {'device': renderer['device'], 'generation': 3}
    descriptor = {'extent': {'width': 3840, 'height': 2160}, 'format': 'Rgba8Unorm'}
    context = {'FrameProducer': {'stream': 'file-a'}}
    frames = [{'token': renderer, 'generation': generation, 'wall': {'begin': 110, 'end': 114}}]
    native = {
        'Gpu': [{'token': renderer, 'context': 'Renderer', 'status': 'Valid',
                 'begin': 1 << 62, 'end': (1 << 62) + 2, 'frequency': 1000,
                 'submitted_qpc': 111, 'completed_qpc': 115},
                {'token': producer, 'context': context, 'status': 'Valid',
                 'begin': 100, 'end': 101, 'frequency': 1000,
                 'submitted_qpc': 106, 'completed_qpc': 109}],
        'SourceCpu': [{'token': producer, 'context': context, 'wall': {'begin': 105, 'end': 107}}],
        'SourceSubmission': [{'token': producer, 'stream': 'file-a', 'descriptor': descriptor}],
        'SourceUse': [{'token': renderer, 'texture': 13, 'sequence': 29, 'stream': 'file-a',
                       'publication': {'source_token': producer}, 'descriptor': descriptor}],
        'Output': [{'token': renderer, 'output': 5}],
        'OutputSource': [{'generation': generation, 'output': 5, 'texture': 13, 'source_count': 1}],
    }
    return frames, native, [{'stream': 'file-a', 'file': 'D:/a.mp4'}]


def analyze(data=None, healthy=True):
    return ANALYZE(*(fixture() if data is None else data), 1000, distribution, healthy=healthy)


class VideoGpuPathTests(unittest.TestCase):
    def test_exact_source_token_not_texture_number_or_publication_sequence_proves_path(self):
        result = analyze()
        self.assertTrue(result['coverage_complete'])
        self.assertIs(result['observed_within_target'], True)
        self.assertEqual(result['cpu_enclosed_gpu_path_upper_bound']['p95_ms'], 10)
        self.assertEqual(result['records'][0]['source_tokens'], [token('producer', 7)])

    def test_shared_reference_and_texture_alias_charge_one_immutable_producer(self):
        frames, native, bindings = fixture()
        native['SourceUse'].append({**native['SourceUse'][0], 'texture': 27})
        native['Output'].append({'token': frames[0]['token'], 'output': 6})
        native['OutputSource'].append({**native['OutputSource'][0], 'output': 6, 'texture': 27})
        result = analyze((frames, native, bindings))
        self.assertTrue(result['coverage_complete'])
        self.assertEqual(len(result['records'][0]['source_tokens']), 1)
        self.assertEqual(result['context_span_serial_charge_not_critical_path']['p95_ms'], 3)

    def test_equal_extent_independent_file_keeps_a_separate_producer_dependency(self):
        frames, native, bindings = fixture()
        other = token('producer-b', 7)
        context = {'FrameProducer': {'stream': 'file-b'}}
        native['SourceCpu'].append({**native['SourceCpu'][0], 'token': other, 'context': context})
        native['Gpu'].append({**native['Gpu'][1], 'token': other, 'context': context})
        native['SourceSubmission'].append({**native['SourceSubmission'][0], 'token': other, 'stream': 'file-b'})
        native['SourceUse'].append({**native['SourceUse'][0], 'texture': 27, 'stream': 'file-b',
                                   'publication': {'source_token': other}})
        native['OutputSource'][0]['source_count'] = 2
        native['OutputSource'].append({**native['OutputSource'][0], 'texture': 27})
        bindings.append({'stream': 'file-b', 'file': 'D:/b.mp4'})
        result = analyze((frames, native, bindings))
        self.assertEqual(len(result['records'][0]['source_tokens']), 2)
        self.assertTrue(result['coverage_complete'])

    def test_held_thirty_fps_image_is_not_a_gpu_execution_failure(self):
        frames, native, bindings = fixture()
        frames[0]['wall'] = {'begin': 210, 'end': 214}
        native['Gpu'][0].update(submitted_qpc=211, completed_qpc=215)
        result = analyze((frames, native, bindings))
        self.assertTrue(result['coverage_complete'])
        self.assertEqual(result['cpu_enclosed_gpu_path_upper_bound']['p95_ms'], 110)
        self.assertEqual(result['gpu_path_lower_bound']['p95_ms'], 1)
        self.assertIsNone(result['observed_within_target'])
        self.assertFalse(result['definite_failure_lower_bound_exceeds_budget'])

    def test_independent_renderers_reuse_one_held_producer_query_without_rebinding_it(self):
        frames, native, bindings = fixture()
        other = token('renderer-b', 2)
        generation = {'device': other['device'], 'generation': 9}
        frames.append({**frames[0], 'token': other, 'generation': generation})
        native['Gpu'].append({**native['Gpu'][0], 'token': other})
        native['SourceUse'].append({**native['SourceUse'][0], 'token': other})
        native['Output'].append({'token': other, 'output': 5})
        native['OutputSource'].append({**native['OutputSource'][0], 'generation': generation})
        result = analyze((frames, native, bindings))
        self.assertEqual(result['covered_frames'], 2)
        self.assertTrue(result['coverage_complete'])
        self.assertEqual([row['source_tokens'] for row in result['records']], [[token('producer', 7)]] * 2)
        native['OutputSource'][1].update(texture=None, source_count=0)
        result = analyze((frames, native, bindings))
        self.assertEqual(result['input_renderer_batches'], 2)
        self.assertEqual(result['excluded_proven_source_free_batches'], 1)
        self.assertEqual(result['expected_frames'], 1)
        self.assertTrue(result['coverage_complete'])

    def test_actual_slow_local_gpu_span_can_still_prove_failure(self):
        data = fixture()
        data[1]['Gpu'][0].update(end=(1 << 62) + 15, completed_qpc=140)
        self.assertIs(analyze(data)['observed_within_target'], False)
        self.assertIsNone(analyze(data, healthy=False)['observed_within_target'])

    def test_missing_wrong_or_older_generation_routes_never_remove_the_denominator(self):
        for change in ('missing', 'partial', 'generation', 'zero', 'sentinel', 'bool'):
            data = fixture()
            routes = data[1]['OutputSource']
            if change == 'missing':
                routes.clear()
            elif change == 'partial':
                routes[0]['source_count'] = 2
            elif change == 'generation':
                routes[0]['generation'] = {'generation': 2}
            elif change == 'zero':
                routes[0]['source_count'] = 0
            elif change == 'bool':
                routes[0]['source_count'] = True
            else:
                routes.append({**routes[0], 'texture': None})
            with self.subTest(change=change):
                result = analyze(data)
                self.assertEqual(result['expected_frames'], 1)
                self.assertFalse(result['coverage_complete'])
                self.assertIsNone(result['observed_within_target'])

    def test_legacy_missing_origin_and_foreign_identity_are_incomplete_not_guessed(self):
        for change in ('publication', 'use', 'source', 'submission', 'context', 'stream', 'binding', 'descriptor', 'zero'):
            frames, native, bindings = deepcopy(fixture())
            if change == 'publication':
                native['SourceUse'][0]['publication'] = None
            elif change == 'use':
                native['SourceUse'].clear()
            elif change == 'source':
                native['SourceCpu'].clear()
            elif change == 'submission':
                native['SourceSubmission'].clear()
            elif change == 'context':
                native['SourceCpu'][0]['context'] = {'FrameProducer': {'stream': 'foreign'}}
            elif change == 'stream':
                native['SourceSubmission'][0]['stream'] = 'foreign'
            elif change == 'binding':
                bindings.clear()
            elif change == 'descriptor':
                native['SourceUse'][0]['descriptor'] = {'extent': {'width': 1, 'height': 1}}
            else:
                native['SourceUse'][0]['sequence'] = 0
            with self.subTest(change=change):
                result = analyze((frames, native, bindings))
                self.assertFalse(result['coverage_complete'])
                self.assertIsNone(result['observed_within_target'])

    def test_query_tail_health_and_gpu_clock_disjoint_keep_the_existing_enclosure_rules(self):
        data = fixture()
        data[1]['Gpu'][1].update(status='Disjoint', begin=None, end=None, frequency=None)
        self.assertIs(analyze(data)['observed_within_target'], True)
        self.assertIsNone(analyze(data, healthy=False)['observed_within_target'])
        data[1]['Gpu'][1]['status'] = 'Uncollected'
        self.assertIsNone(analyze(data)['observed_within_target'])
        for field in ('submitted_qpc', 'completed_qpc'):
            data = fixture()
            data[1]['Gpu'][1][field] = None
            self.assertIsNone(analyze(data)['observed_within_target'])

    def test_proven_source_free_batches_are_counted_and_cannot_qualify_video_alone(self):
        data = fixture()
        data[1]['OutputSource'][0].update(texture=None, source_count=0)
        result = analyze(data)
        self.assertEqual(result['excluded_proven_source_free_batches'], 1)
        self.assertEqual(result['expected_frames'], 0)
        self.assertFalse(result['coverage_complete'])
        self.assertIsNone(result['observed_within_target'])

    def test_duplicate_native_identities_and_missing_output_observations_are_explicit(self):
        for name in ('SourceUse', 'SourceCpu', 'Gpu', 'SourceSubmission', 'Output', 'OutputSource'):
            data = fixture()
            data[1][name].append(deepcopy(data[1][name][0]))
            with self.subTest(name=name), self.assertRaisesRegex(ValueError, 'duplicate'):
                analyze(data)
        data = fixture()
        data[1]['Output'].clear()
        result = analyze(data)
        self.assertEqual(result['expected_frames'], 1)
        self.assertFalse(result['coverage_complete'])


if __name__ == '__main__':
    unittest.main()
