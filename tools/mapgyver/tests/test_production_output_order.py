import copy
from pathlib import Path
import runpy
import unittest

ANALYZE = runpy.run_path(str(Path(__file__).resolve().parents[1]/'production_output_order.py'))['analyze_output_order']


def fixture():
    token = {'device': {'runtime': 'owned', 'generation': 1}, 'serial': 1}
    rows = [{'token': token, 'output': output, 'outcome': 'Accepted',
             'conversion': {'begin': begin, 'end': begin+5},
             'consumer_api': {'begin': api, 'end': api+5},
             'delivery': {'begin': api-1, 'end': api+6},
             'window_present': None if output == 2 else {'generation': 1, 'addresses': [1, 2, 3]}}
            for output, begin, api in ((1, 30, 40), (2, 10, 20), (3, 50, 60))]
    return [{'token': token, 'wall': {'begin': 1, 'end': 100}}], rows


class OutputOrderTests(unittest.TestCase):
    def test_shared_first_order_is_verified_by_tokens_not_row_position(self):
        frames, rows = fixture()
        result = ANALYZE(frames, list(reversed(rows)))
        self.assertTrue(result['ordering_verified'])
        self.assertEqual(result['covered_frames'], 1)

    def test_early_window_work_reports_a_barrier_violation(self):
        frames, rows = fixture()
        rows[0]['conversion'] = {'begin': 2, 'end': 7}
        rows[0]['consumer_api'] = {'begin': 8, 'end': 9}
        rows[0]['delivery'] = {'begin': 7, 'end': 10}
        result = ANALYZE(frames, rows)
        self.assertFalse(result['ordering_verified'])
        self.assertIn('window 1 work precedes shared publication phase completion', result['records'][0]['violations'])

    def test_rejected_endpoint_can_skip_work_without_skipping_other_outputs(self):
        frames, rows = fixture()
        rows[0].update(outcome='Busy', conversion=None, consumer_api=None, window_present=None)
        result = ANALYZE(frames, rows)
        self.assertTrue(result['ordering_verified'])
        self.assertEqual(result['records'][0]['actual_api_calls'], 2)

    def test_missing_or_untrusted_timing_does_not_shrink_the_frame_cohort(self):
        for mode in ('missing record', 'missing conversion', 'missing api', 'missing identity', 'untrusted'):
            frames, rows = fixture()
            if mode == 'missing record':
                rows.pop()
            elif mode == 'missing conversion':
                rows[0]['conversion'] = None
            elif mode == 'missing api':
                rows[0]['consumer_api'] = None
            elif mode == 'missing identity':
                rows[0]['window_present'] = None
            result = ANALYZE(frames, rows, healthy=mode != 'untrusted')
            self.assertEqual(result['expected_frames'], 1)
            self.assertEqual(result['unknown_frames'], 1)
            self.assertFalse(result['ordering_verified'])

    def test_duplicates_invalid_clocks_and_empty_cohorts_cannot_pass(self):
        frames, rows = fixture()
        for f, r in ((frames+frames, rows), (frames, rows+[copy.deepcopy(rows[0])])):
            with self.assertRaises(ValueError):
                ANALYZE(f, r)
        rows[0]['conversion']['begin'] = True
        with self.assertRaises(ValueError):
            ANALYZE(frames, rows)
        self.assertFalse(ANALYZE([], [])['ordering_verified'])

    def test_timing_from_another_cpu_frame_does_not_certify_the_token(self):
        frames, rows = fixture()
        rows[0]['consumer_api'] = {'begin': 101, 'end': 105}
        result = ANALYZE(frames, rows)
        self.assertFalse(result['ordering_verified'])
        self.assertIn('output 1 timing outside CPU frame', result['records'][0]['unknown'])

    def test_an_output_api_cannot_precede_its_own_conversion(self):
        frames, rows = fixture()
        rows[0]['consumer_api'] = {'begin': 32, 'end': 33}
        result = ANALYZE(frames, rows)
        self.assertFalse(result['ordering_verified'])
        self.assertIn('output 1 API precedes its conversion', result['records'][0]['violations'])
