import copy
import csv
import io
import json
from pathlib import Path
import runpy
import unittest

ROOT = Path(__file__).resolve().parents[3]
ANALYZE = runpy.run_path(str(ROOT/'tools/mapgyver/production_presentmon.py'))['analyze_presentmon']


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
        self.assertEqual(report['outputs'][0]['source_to_software_display_age'][0]['values'], [55.5])
        self.assertEqual(report['outputs'][0]['plan_publication_to_software_display_age']['values'], [85.5])
        self.assertIn('not app/native buffer occupancy', report['outputs'][0]['pending_software_display']['scope'])

    def test_missing_or_dropped_rows_never_shrink_the_accepted_image_cohort(self):
        for mode in ('missing', 'dropped', 'unknown display', 'no identity'):
            with self.subTest(mode=mode):
                parts = fixture()
                if mode == 'missing':
                    parts[3].pop(0)
                elif mode == 'dropped':
                    parts[3][0][3] = 1
                elif mode == 'unknown display':
                    parts[3][0][4] = 'NA'
                else:
                    parts[1]['Output'][0]['window_present'] = None
                report = analyze(parts)
                self.assertFalse(report['coverage_complete'])
                self.assertEqual(report['outputs'][0]['expected_accepted_images'], 1)
                self.assertFalse(report['outputs'][0]['pending_software_display']['coverage_complete'])

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
            field = 'source_to_software_display_age' if mode == 'source' else 'plan_publication_to_software_display_age'
            observed = report['outputs'][0][field]
            self.assertEqual(observed, [None, None] if mode == 'source' else None)
