import copy
from pathlib import Path
import runpy
import unittest

ROOT = Path(__file__).resolve().parents[3]
ANALYZE = runpy.run_path(str(ROOT / 'tools/mapgyver/production_plan_age.py'))['analyze_plan_age']


def fixture():
    identity = {'project': {'runtime': 'renderer', 'generation': 0}, 'revision': 1}
    generation = {'generation': 1, 'device': {'runtime': 'renderer', 'generation': 1},
                  'identity': identity, 'bytes': 32, 'output_count': 3}
    transition = lambda stage, tick: {'identity': identity, 'stage': stage, 'observed_qpc': tick}
    transitions = [transition('Published', 10), transition('Received', 20),
                   transition({'Installed': {'generation': generation}}, 40)]
    preparation = {'generation': generation, 'succeeded': True, 'wall': {'begin': 30, 'end': 35}}
    token = {'device': generation['device'], 'serial': 1}
    frame = {'generation': generation, 'token': token, 'wall': {'begin': 50, 'end': 80}}
    outputs = [{'output': output, 'token': token, 'outcome': 'Accepted',
                'delivery': {'begin': 60, 'end': 70}, 'consumer_api': {'begin': 61, 'end': 69}}
               for output in (1, 2, 3)]
    return [transitions, [preparation], [frame], outputs]


def analyze(parts):
    def distribution(values):
        return {'samples': len(values), 'values': values} if values else None
    return ANALYZE(*parts, {'begin_qpc': 1, 'end_qpc': 100}, 1000, distribution)


class PlanAgeTests(unittest.TestCase):
    def test_exact_cpu_handoff_install_and_output_age_have_distinct_meanings(self):
        report = analyze(fixture())
        self.assertTrue(report['coverage_complete'])
        self.assertEqual(report['publication_to_receipt']['values'], [10])
        self.assertEqual(report['publication_to_first_submit']['values'], [40])
        self.assertEqual(report['outputs'][0]['publication_to_api_end']['values'], [59])
        self.assertEqual(report['records'][0]['installations'][0]['install_to_first_submit_ms'], 10)
        self.assertIn('GPU queue', report['limitations'][-1])

    def test_unread_superseded_input_has_no_zero_receipt_or_first_use(self):
        parts = fixture()
        identity = copy.deepcopy(parts[0][0]['identity'])
        identity['revision'] = 2
        parts[0].extend([{'identity': identity, 'stage': stage, 'observed_qpc': tick}
                         for stage, tick in [('Published', 15), ('Superseded', 16)]])
        report = analyze(parts)
        self.assertTrue(report['coverage_complete'])
        self.assertEqual(report['accepted_publications'], 2)
        self.assertEqual(report['publication_to_receipt']['samples'], 1)
        self.assertIsNone(report['records'][1]['publication_to_receipt_ms'])
        self.assertEqual(report['records'][1]['installations'], [])

    def test_resource_recreation_keeps_original_plan_age_and_exact_device_join(self):
        parts = fixture()
        generation = copy.deepcopy(parts[1][0]['generation'])
        generation['device']['generation'] = 2
        generation['generation'] = 2
        parts[0].append({'identity': generation['identity'], 'observed_qpc': 85,
                         'stage': {'Installed': {'generation': generation}}})
        parts[1].append({'generation': generation, 'succeeded': True, 'wall': {'begin': 81, 'end': 84}})
        parts[2].append({'generation': generation, 'token': {'device': generation['device'], 'serial': 1},
                         'wall': {'begin': 90, 'end': 99}})
        report = analyze(parts)
        self.assertTrue(report['coverage_complete'])
        self.assertEqual(report['accepted_publications'], 1)
        self.assertEqual(report['installed_generations'], 2)
        self.assertEqual(report['publication_to_first_submit']['values'], [40, 80])

    def test_missing_publication_receipt_or_clock_never_certifies_coverage(self):
        for missing in ('Published', 'Received', 'clock', 'Installed'):
            with self.subTest(missing=missing):
                parts = fixture()
                if missing == 'clock':
                    parts[0][0]['observed_qpc'] = None
                elif missing == 'Installed':
                    parts[0].pop()
                else:
                    parts[0] = [r for r in parts[0] if r['stage'] != missing]
                self.assertFalse(analyze(parts)['coverage_complete'])

    def test_duplicate_receipt_or_install_is_not_silently_selected(self):
        for index in (1, 2):
            parts = fixture()
            parts[0].append(copy.deepcopy(parts[0][index]))
            with self.assertRaisesRegex(ValueError, 'duplicate'):
                analyze(parts)

    def test_pending_plan_without_terminal_result_stays_incomplete(self):
        parts = fixture()
        identity = copy.deepcopy(parts[0][0]['identity'])
        identity['revision'] = 2
        parts[0].append({'identity': identity, 'stage': 'Published', 'observed_qpc': 15})
        self.assertFalse(analyze(parts)['coverage_complete'])

    def test_rejected_plan_is_terminal_without_a_fabricated_install(self):
        parts = fixture()
        parts[0].pop()
        parts[0].append({'identity': parts[0][0]['identity'], 'observed_qpc': 25,
                         'stage': {'Rejected': {'code': 'InvalidPlan'}}})
        parts[1:] = [[], [], []]
        report = analyze(parts)
        self.assertTrue(report['coverage_complete'])
        self.assertIsNone(report['publication_to_first_submit'])

    def test_conflicting_installed_and_cancelled_results_are_rejected(self):
        parts = fixture()
        parts[0].append({'identity': parts[0][0]['identity'], 'stage': 'Cancelled', 'observed_qpc': 45})
        with self.assertRaisesRegex(ValueError, 'conflicting terminal'):
            analyze(parts)

    def test_reversed_receipt_preparation_install_frame_or_api_order_is_rejected(self):
        for stage in ('receipt', 'preparation', 'install', 'frame', 'api'):
            with self.subTest(stage=stage):
                parts = fixture()
                if stage == 'receipt':
                    parts[0][1]['observed_qpc'] = 5
                elif stage == 'preparation':
                    parts[1][0]['wall']['begin'] = 15
                elif stage == 'install':
                    parts[0][2]['observed_qpc'] = 34
                elif stage == 'frame':
                    parts[2][0]['wall']['begin'] = 39
                else:
                    parts[3][0]['consumer_api']['end'] = 81
                with self.assertRaisesRegex(ValueError, 'clock order'):
                    analyze(parts)

    def test_changed_resource_metadata_or_foreign_identity_cannot_match_by_counter(self):
        for foreign in (False, True):
            parts = fixture()
            parts[2][0]['generation'] = copy.deepcopy(parts[2][0]['generation'])
            if foreign:
                parts[2][0]['generation']['device']['runtime'] = 'other-renderer'
            else:
                parts[2][0]['generation']['bytes'] = 64
            with self.assertRaisesRegex(ValueError, 'foreign|differs'):
                analyze(parts)
