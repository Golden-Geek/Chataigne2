import copy
from pathlib import Path
import runpy
import unittest

ANALYZE = runpy.run_path(str(Path(__file__).parents[1] / 'production_gpu_path.py'))['analyze_gpu_path']


def token(runtime, serial=1):
    return {'device': {'runtime': runtime, 'generation': 1}, 'serial': serial}


def fixture(offset=0):
    frame = {'token': token('renderer'), 'wall': {'begin': offset + 110, 'end': offset + 114}}
    native = {'Gpu': [{'token': frame['token'], 'context': 'Renderer', 'status': 'Valid',
                       'begin': 1 << 62, 'end': (1 << 62) + 2, 'frequency': 1000,
                       'submitted_qpc': offset + 111, 'completed_qpc': offset + 115}],
              'SourceCpu': [], 'SourceUse': []}
    for texture in (1, 2):
        source = token(f'source-{texture}')
        context = {'Source': {'texture': texture, 'renderer_device': frame['token']['device']}}
        native['SourceCpu'].append({'token': source, 'context': context,
                                    'wall': {'begin': offset + 105, 'end': offset + 107}})
        native['SourceUse'].append({'token': frame['token'], 'texture': texture, 'sequence': 1,
                                    'publication': {'source_token': source}})
        native['Gpu'].append({'token': source, 'context': context, 'status': 'Valid',
                              'begin': texture * 100, 'end': texture * 100 + 1, 'frequency': 1000,
                              'submitted_qpc': offset + 106, 'completed_qpc': offset + 109})
    return [frame], native


def distribution(values):
    return {'samples': len(values), 'p95_ms': max(values), 'p99_ms': max(values)} if values else None


def analyze(frames=None, native=None, healthy=True):
    if frames is None:
        frames, native = fixture()
    return ANALYZE(frames, native, 1000, distribution, healthy=healthy)


class GpuPathTests(unittest.TestCase):
    def test_complete_cpu_enclosure_proves_budget_without_aligning_unrelated_gpu_origins(self):
        result = analyze()
        self.assertTrue(result['coverage_complete'])
        self.assertIs(result['observed_within_target'], True)
        self.assertEqual(result['cpu_enclosed_gpu_path_upper_bound']['p95_ms'], 10)
        self.assertEqual(result['context_span_serial_charge_not_critical_path']['p95_ms'], 4)
        self.assertEqual(len(result['records'][0]['source_tokens']), 2)

    def test_retained_source_age_and_collection_delay_keep_a_loose_bound_unknown(self):
        frames, native = fixture()
        native['Gpu'][0]['completed_qpc'] += 50
        result = analyze(frames, native)
        self.assertTrue(result['coverage_complete'])
        self.assertEqual(result['cpu_enclosed_gpu_path_upper_bound']['p95_ms'], 60)
        self.assertIsNone(result['observed_within_target'])
        self.assertEqual(result['context_span_serial_charge_not_critical_path']['p95_ms'], 4)
        frames, native = fixture()
        frames[0]['wall'] = {'begin': 210, 'end': 214}
        native['Gpu'][0].update(submitted_qpc=211, completed_qpc=215)
        self.assertEqual(analyze(frames, native)['cpu_enclosed_gpu_path_upper_bound']['p95_ms'], 110)

    def test_query_collection_of_every_dependency_is_required_including_the_source_tail(self):
        for field in ('submitted_qpc', 'completed_qpc'):
            frames, native = fixture()
            native['Gpu'][1][field] = None
            result = analyze(frames, native)
            self.assertFalse(result['coverage_complete'])
            self.assertIsNone(result['observed_within_target'])
        frames, native = fixture()
        native['Gpu'][1]['completed_qpc'] = 130
        self.assertEqual(analyze(frames, native)['cpu_enclosed_gpu_path_upper_bound']['p95_ms'], 25)

    def test_source_identity_and_renderer_device_are_not_guessed(self):
        for change in ('publication', 'device', 'texture'):
            frames, native = fixture()
            if change == 'publication':
                native['SourceUse'][0]['publication'] = None
            elif change == 'device':
                native['SourceCpu'][0]['context']['Source']['renderer_device'] = token('foreign')['device']
            else:
                native['SourceCpu'][0]['context']['Source']['texture'] = 7
            self.assertIsNone(analyze(frames, native)['observed_within_target'])

    def test_independent_cpu_enclosure_survives_gpu_clock_disjoint_but_not_missing_query_collection(self):
        frames, native = fixture()
        native['Gpu'][1].update(status='Disjoint', begin=None, end=None, frequency=None)
        result = analyze(frames, native)
        self.assertIs(result['observed_within_target'], True)
        self.assertIsNone(result['context_span_serial_charge_not_critical_path'])
        native['Gpu'][1]['status'] = 'Uncollected'
        self.assertIsNone(analyze(frames, native)['observed_within_target'])

    def test_clock_integrity_and_enclosing_interval_cannot_be_fabricated(self):
        for change in ({'submitted_qpc': 109}, {'completed_qpc': 113}, {'end': (1 << 62) + 50}):
            frames, native = fixture()
            native['Gpu'][0].update(change)
            with self.assertRaises(ValueError):
                analyze(frames, native)
        frames, native = fixture()
        native['Gpu'][0]['submitted_qpc'] = True
        self.assertIsNone(analyze(frames, native)['observed_within_target'])
        self.assertIs(analyze(*fixture(1 << 60))['observed_within_target'], True)

    def test_duplicate_or_missing_records_and_untrusted_capture_do_not_pass(self):
        frames, native = fixture()
        native['Gpu'].append(copy.deepcopy(native['Gpu'][0]))
        with self.assertRaisesRegex(ValueError, 'duplicate'):
            analyze(frames, native)
        frames, native = fixture()
        with self.assertRaisesRegex(ValueError, 'duplicate'):
            analyze([*frames, frames[0]], native)
        native['Gpu'].pop()
        self.assertIsNone(analyze(frames, native)['observed_within_target'])
        self.assertIsNone(analyze(healthy=False)['observed_within_target'])
        self.assertIsNone(analyze([], {'Gpu': [], 'SourceCpu': [], 'SourceUse': []})['observed_within_target'])


if __name__ == '__main__':
    unittest.main()
