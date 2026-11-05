import copy
import csv
import io
import json
from fractions import Fraction
from pathlib import Path
import runpy
import unittest

ROOT = Path(__file__).resolve().parents[3]
PRESENTMON = runpy.run_path(str(ROOT/'tools/mapgyver/production_presentmon.py'))
ANALYZE = PRESENTMON['analyze_presentmon']
AGE_BOUNDS = PRESENTMON['_age_upper_bounds']


def fixture():
    token = {'device': {'runtime': 'renderer', 'generation': 1}, 'serial': 1}
    identity = {'project': {'runtime': 'renderer', 'generation': 0}, 'revision': 1}
    native = {'Output': [], 'SourceUse': [], 'FrameCpu': [{'token': token, 'generation': {'identity': identity}}],
              'Plan': [{'identity': identity, 'stage': 'Published', 'observed_qpc': 20}]}
    for output, begin, address in [(1, 100, 0xaa), (3, 120, 0xba)]:
        native['Output'].append({'token': token, 'output': output, 'outcome': 'Accepted',
                                 'consumer_api': {'begin': begin, 'end': begin+10},
                                 'delivery': {'begin': begin, 'end': begin+10},
                                 'window_present': {'generation': 1, 'addresses': [address, address+1, address+2]}})
    sources = []
    for source in (1, 2):
        image = {'frame': 1, 'shared_handle': source}
        sources.append([{'kind': 'source_frame', 'submission': {'spout_image': image,
                          'stamp': {'source': source, 'frame': 1, 'qpc': 40+10*source}}}])
        native['SourceUse'].append({'token': token, 'texture': source, 'sequence': 1,
                                    'publication': {'spout_image': image, 'publication_qpc': 70}})
    session = 'MapGyver-M01-Present-' + 'a'*32
    report = {'target_pid': 123, 'exit_code': 0, 'forced_cleanup': False, 'tools_unchanged': True,
              'session': session, 'stdout': '', 'stderr': '', 'live_query': {'exit_code': 0, 'stdout': json.dumps({
                  'status': 0, 'response_validation_error': None, 'session_name': session,
                  'before_qpc': 1000, 'after_qpc': 1001, 'statistics': {'returned_session_name': session,
                      'events_lost': 0, 'log_buffers_lost': 0, 'realtime_buffers_lost': 0}})}}
    rows = [[123, '0xab', 105, 0, '.5', 16], [123, '0xba', 125, 0, '.5', 16]]
    return report, native, sources, rows


def analyze(parts):
    report, native, sources, rows = parts
    stream = io.StringIO(newline='')
    writer = csv.writer(stream)
    writer.writerow(['ProcessID', 'SwapChainAddress', 'QPCTime', 'Dropped', 'MsUntilDisplayed', 'MsBetweenDisplayChange'])
    writer.writerows(rows)
    report['stdout'] = stream.getvalue()
    distribution = lambda values: {'samples': len(values), 'values': values} if values else None
    return ANALYZE(report, native, sources, {'begin_qpc': 90, 'end_qpc': 160}, 123, 1000, distribution)


class ProductionPresentMonTests(unittest.TestCase):
    def test_exact_pid_interface_alias_api_and_source_join_preserves_fractional_display_time(self):
        report = analyze(fixture())
        self.assertTrue(report['coverage_complete'])
        self.assertEqual(report['outputs'][0]['records'][0]['display_qpc'], {'numerator': '211', 'denominator': '2'})
        self.assertEqual(report['outputs'][0]['observed_source_to_software_display_age'][0]['values'], [55.5])
        self.assertEqual(report['outputs'][0]['source_to_software_display_age_upper_bounds'][0]['p95_upper_ms'], 55.5)
        self.assertEqual(report['outputs'][0]['plan_publication_to_software_display_age']['values'], [85.5])
        self.assertIn('not app/native buffer occupancy', report['outputs'][0]['pending_software_display']['scope'])

    def test_unknown_rows_never_shrink_or_qualify_the_accepted_image_cohort(self):
        for mode in ('missing', 'unknown display', 'no identity'):
            with self.subTest(mode=mode):
                parts = fixture()
                if mode == 'missing':
                    parts[3].pop(0)
                elif mode == 'unknown display':
                    parts[3][0][4] = 'NA'
                else:
                    parts[1]['Output'][0]['window_present'] = None
                report = analyze(parts)
                self.assertFalse(report['coverage_complete'])
                self.assertEqual(report['outputs'][0]['expected_accepted_images'], 1)
                self.assertFalse(report['outputs'][0]['pending_software_display']['coverage_complete'])

    def test_known_drop_is_accounted_without_an_invented_display_age_or_retirement(self):
        parts = fixture()
        parts[3][0][3] = 1
        report = analyze(parts)
        self.assertTrue(report['coverage_complete'])
        output = report['outputs'][0]
        self.assertEqual(output['dropped'], 1)
        self.assertEqual(output['records'][0]['display_disposition'], 'Dropped')
        self.assertIsNone(output['records'][0]['display_qpc'])
        self.assertIsNone(output['records'][0]['source_age_ms'])
        self.assertFalse(output['pending_software_display']['coverage_complete'])
        for bound in output['source_to_software_display_age_upper_bounds']:
            self.assertEqual(bound['accepted_images'], 1)
            self.assertEqual(bound['unbounded_age_images'], 1)
            self.assertIsNone(bound['p95_upper_ms'])
            self.assertIsNone(bound['p95_within_target'])

    def test_dropped_frame_still_requires_exact_source_and_plan_identity(self):
        for field in ('SourceUse', 'Plan'):
            parts = fixture()
            parts[3][0][3] = 1
            parts[1][field].clear()
            self.assertFalse(analyze(parts)['coverage_complete'])
        parts = fixture()
        parts[3][0][3] = 1
        parts[1]['SourceUse'][0]['publication']['publication_qpc'] = 111
        with self.assertRaisesRegex(ValueError, 'endpoint clock'):
            analyze(parts)

    def test_censored_p95_keeps_the_original_rank_and_unbounded_tail(self):
        bound = AGE_BOUNDS([Fraction(i) for i in range(19)], 20)
        self.assertEqual(bound['accepted_images'], 20)
        self.assertEqual(bound['finite_age_images'], 19)
        self.assertEqual(bound['ranks']['p95'], 19)
        self.assertEqual(bound['p95_upper_ms'], 18)
        self.assertTrue(bound['p95_within_target'])
        self.assertIsNone(bound['p99_upper_ms'])
        self.assertIsNone(bound['maximum_upper_ms'])
        self.assertEqual(bound['unbounded_statistics'], ['p99', 'maximum'])
        insufficient = AGE_BOUNDS([Fraction(i) for i in range(18)], 20)
        self.assertIsNone(insufficient['p95_upper_ms'])
        self.assertIsNone(insufficient['p95_within_target'])

    def test_p95_target_comparison_uses_exact_rationals_at_the_inclusive_boundary(self):
        self.assertTrue(AGE_BOUNDS([Fraction(50)] * 19, 20)['p95_within_target'])
        above = Fraction(50) + Fraction(1, 10**18)
        bound = AGE_BOUNDS([above] * 19, 20)
        self.assertGreater(bound['p95_upper_ms'], 50.0)
        self.assertFalse(bound['p95_within_target'])
        self.assertEqual(bound['p95_upper_exact_ms'],
                         {'numerator': str(above.numerator), 'denominator': str(above.denominator)})

    def test_numeric_upper_bounds_round_outward_from_exact_rationals(self):
        for value in (Fraction(1, 10), Fraction(3, 10), Fraction(50)-Fraction(1, 10**18),
                      Fraction(50), Fraction(50)+Fraction(1, 10**18)):
            with self.subTest(value=value):
                bound = AGE_BOUNDS([value], 1)
                self.assertGreaterEqual(Fraction(bound['p95_upper_ms']), value)
                self.assertEqual(bound['p95_within_target'], value <= 50)

    def test_excluding_drops_would_falsely_pass_the_latency_target(self):
        observed = [Fraction(49)] * 91 + [Fraction(51)] * 4
        conditional = AGE_BOUNDS(observed, 95)
        whole = AGE_BOUNDS(list(reversed(observed)), 100)
        self.assertEqual(conditional['p95_upper_ms'], 49)
        self.assertTrue(conditional['p95_within_target'])
        self.assertEqual(whole['p95_upper_ms'], 51)
        self.assertFalse(whole['p95_within_target'])
        self.assertEqual(whole['accepted_images'], 100)
        self.assertEqual(whole['unbounded_age_images'], 5)

    def test_empty_invalid_or_fabricated_age_counts_never_certify_a_percentile(self):
        self.assertIsNone(AGE_BOUNDS([], 0)['p95_within_target'])
        for values, total in [([Fraction(1)], 0), ([], -1), ([], True),
                              ([Fraction(-1)], 1), ([float('nan')], 1)]:
            with self.assertRaisesRegex(ValueError, 'whole-cohort'):
                AGE_BOUNDS(values, total)

    def test_complete_cohort_bounds_preserve_uncensored_nearest_rank_statistics(self):
        bound = AGE_BOUNDS([Fraction(i, 10) for i in range(1, 101)], 100)
        self.assertEqual([bound[name+'_upper_ms'] for name in ('median', 'p95', 'p99', 'maximum')],
                         [5, 9.5, 9.9, 10])
        self.assertEqual(bound['unbounded_statistics'], [])

    def test_wrong_pid_is_rejected_instead_of_becoming_a_webview_or_source_join(self):
        parts = fixture()
        parts[3][0][0] = 999
        with self.assertRaisesRegex(ValueError, 'unexpected PID'):
            analyze(parts)

    def test_wrong_address_or_qpc_outside_api_is_unmatched_without_nearest_time_guessing(self):
        for mode in ('address', 'time'):
            parts = fixture()
            parts[3][0][1 if mode == 'address' else 2] = '0xdd' if mode == 'address' else 111
            report = analyze(parts)
            self.assertEqual(len(report['unmatched_rows']), 1)
            self.assertFalse(report['coverage_complete'])

    def test_duplicate_claim_or_overlapping_alias_is_ambiguous(self):
        for mode in ('duplicate', 'overlap'):
            parts = fixture()
            if mode == 'duplicate':
                parts[3].insert(1, [123, '0xab', 106, 0, '.5', 16])
            else:
                parts[1]['Output'][1]['window_present']['addresses'][0] = 0xab
                parts[1]['Output'][1]['consumer_api']['begin'] = 105
            with self.assertRaisesRegex(ValueError, 'same native Present|ambiguous'):
                analyze(parts)

    def test_recycled_pointer_is_joined_only_to_its_new_api_incarnation(self):
        parts = fixture()
        output = copy.deepcopy(parts[1]['Output'][0])
        output['token']['serial'] = 2
        output['window_present']['generation'] = 2
        output['consumer_api'] = output['delivery'] = {'begin': 140, 'end': 149}
        parts[1]['Output'].append(output)
        parts[3].append([123, '0xab', 145, 0, '.5', 16])
        for use in copy.deepcopy(parts[1]['SourceUse']):
            use['token']['serial'] = 2
            parts[1]['SourceUse'].append(use)
        frame = copy.deepcopy(parts[1]['FrameCpu'][0])
        frame['token']['serial'] = 2
        parts[1]['FrameCpu'].append(frame)
        result = analyze(parts)
        self.assertTrue(result['coverage_complete'])
        self.assertEqual(result['outputs'][0]['records'][1]['endpoint']['generation'], 2)

    def test_failed_early_or_lossy_live_query_does_not_fabricate_zero_loss(self):
        for mode in ('absent', 'early', 'lost', 'forced'):
            parts = fixture()
            query = json.loads(parts[0]['live_query']['stdout'])
            if mode == 'absent':
                query['statistics'] = None
                query['status'] = 4201
            elif mode == 'early':
                query['before_qpc'] = 100
            elif mode == 'lost':
                query['statistics']['events_lost'] = 1
            else:
                parts[0]['forced_cleanup'] = True
            parts[0]['live_query']['stdout'] = json.dumps(query)
            report = analyze(parts)
            self.assertFalse(report['collector_healthy'])
            self.assertFalse(report['coverage_complete'])

    def test_missing_source_or_plan_does_not_become_zero_display_age(self):
        for mode in ('source', 'plan'):
            parts = fixture()
            parts[1]['SourceUse' if mode == 'source' else 'Plan'].clear()
            report = analyze(parts)
            self.assertFalse(report['coverage_complete'])
            field = 'observed_source_to_software_display_age' if mode == 'source' else 'plan_publication_to_software_display_age'
            observed = report['outputs'][0][field]
            self.assertEqual(observed, [None, None] if mode == 'source' else None)
