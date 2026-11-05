import copy
from fractions import Fraction
from itertools import product
from pathlib import Path
import runpy
import unittest

MODULE = runpy.run_path(str(Path(__file__).parents[1] / 'production_gpu_path.py'))
ANALYZE = MODULE['analyze_gpu_path']


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

    def test_collection_delay_keeps_a_loose_bound_unknown_but_retained_age_can_prove_failure(self):
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
        result = analyze(frames, native)
        self.assertEqual(result['cpu_enclosed_gpu_path_upper_bound']['p95_ms'], 110)
        self.assertEqual(result['gpu_path_lower_bound']['p95_ms'], 101)
        self.assertIs(result['observed_within_target'], False)

    def test_overlapping_scopes_do_not_sum_local_durations_into_a_false_failure(self):
        frames, native = fixture()
        for row in native['Gpu']:
            row.update(end=row['begin'] + 8, submitted_qpc=106, completed_qpc=125)
        frames[0]['wall'] = {'begin': 105, 'end': 114}
        result = analyze(frames, native)
        self.assertEqual(result['context_span_serial_charge_not_critical_path']['p95_ms'], 24)
        self.assertEqual(result['gpu_path_lower_bound']['p95_ms'], 7)
        self.assertIsNone(result['observed_within_target'])

    def test_lower_bound_accounts_for_gpu_and_cpu_endpoint_quantization_at_exact_target(self):
        frames, native = fixture()
        renderer = native['Gpu'][0]
        renderer.update(frequency=1_000_000_000, end=renderer['begin'] + 12_000_001, completed_qpc=125)
        result = analyze(frames, native)
        self.assertEqual(result['gpu_path_lower_bound']['p95_ms'], 12)
        self.assertIsNone(result['observed_within_target'])
        renderer['end'] += 1
        result = analyze(frames, native)
        self.assertTrue(result['definite_failure_lower_bound_exceeds_budget'])
        self.assertIs(result['observed_within_target'], False)

    def test_failure_requires_the_original_full_cohort_percentile(self):
        def series(violations, high=14):
            frames, native = [], {'Gpu': [], 'SourceCpu': [], 'SourceUse': []}
            for index in range(100):
                part_frames, part = fixture(index * 1000)
                for row in [*part_frames, *part['Gpu'], *part['SourceCpu'], *part['SourceUse']]:
                    row['token']['serial'] = index + 1
                for row in part['SourceUse']:
                    row['publication']['source_token']['serial'] = index + 1
                renderer = part['Gpu'][0]
                renderer.update(end=renderer['begin'] + (high if index < violations else 13),
                                completed_qpc=index * 1000 + 131)
                frames.extend(part_frames)
                for key in native:
                    native[key].extend(part[key])
            return frames, native
        self.assertIsNone(analyze(*series(5))['observed_within_target'])
        self.assertIs(analyze(*series(6))['observed_within_target'], False)
        self.assertIsNone(analyze(*series(1, high=18))['observed_within_target'])
        self.assertIs(analyze(*series(2, high=18))['observed_within_target'], False)

    def test_untrusted_or_incomplete_capture_never_gets_a_definite_failure(self):
        frames, native = fixture()
        frames[0]['wall'] = {'begin': 210, 'end': 214}
        native['Gpu'][0].update(submitted_qpc=211, completed_qpc=215)
        self.assertIsNone(analyze(frames, native, healthy=False)['observed_within_target'])
        native['Gpu'].pop()
        result = analyze(frames, native)
        self.assertFalse(result['coverage_complete'])
        self.assertFalse(result['definite_failure_lower_bound_exceeds_budget'])
        self.assertIsNone(result['observed_within_target'])

    def test_bound_never_exceeds_possible_execution_spans_under_any_overlap(self):
        bounds = [(100, 108), (103, 112), (106, 115)]
        durations = [2, 3, 4]
        lower = MODULE['path_lower_bound'](bounds, [(Fraction(d), Fraction(0)) for d in durations], 1000)
        starts = [range(first, last - duration + 1) for (first, last), duration in zip(bounds, durations)]
        for positions in product(*starts):
            actual = max(p + d for p, d in zip(positions, durations)) - min(positions)
            self.assertLessEqual(lower, actual)

    def test_exact_verdict_survives_a_numeric_lower_bound_rounded_down_to_target(self):
        frames, native = fixture()
        renderer = native['Gpu'][0]
        renderer.update(frequency=10**18, end=renderer['begin'] + 12_000_000_000_000_002, completed_qpc=125)
        result = analyze(frames, native)
        exact = result['gpu_path_lower_percentile_exact_ms']['p95']
        value = Fraction(int(exact['numerator']), int(exact['denominator']))
        self.assertGreater(value, 12)
        self.assertLessEqual(Fraction(result['gpu_path_lower_bound']['p95_ms']), value)
        self.assertIs(result['observed_within_target'], False)

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
