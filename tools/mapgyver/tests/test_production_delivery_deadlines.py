import copy
from pathlib import Path
import runpy
import unittest

ROOT = Path(__file__).resolve().parents[3]
TIMING = runpy.run_path(str(ROOT/'tools/mapgyver/tests/test_production_deadlines.py'))
ANCHOR = TIMING['DEADLINES']
DELIVERY = runpy.run_path(str(ROOT/'tools/mapgyver/production_delivery_deadlines.py'))['analyze_delivery_deadlines']


def fixture(count=120, window=None):
    rows = [TIMING['schedule'](i) for i in range(count)]
    end = (count*TIMING['FREQUENCY']+59)//60
    rows.append(TIMING['schedule'](count, 'Fence', observed=end+100_000_000))
    outputs, presents, captures = [], [], []
    for row in rows[:-1]:
        begin = row['clock']['Qpc']['slot_start_qpc']+10_000
        deadline = row['clock']['Qpc']['slot_end_qpc']
        for output in (1, 2, 3):
            endpoint = {'generation': 1, 'addresses': [100+output]*3} if output != 2 else None
            image = {'shared_handle': 1000, 'frame': row['frame_index']+1} if output == 2 else None
            outputs.append({'token': row['token'], 'output': output, 'outcome': 'Accepted',
                            'consumer_api': {'begin': begin, 'end': begin+10_000},
                            'window_present': endpoint, 'spout_image': image})
            if output == 2:
                captures.append({'token': row['token'], 'spout_image': image, 'missing': None,
                                 'completion_observed_qpc': deadline-1})
            else:
                presents.append({'token': row['token'], 'output': output, 'endpoint': endpoint,
                                 'present_qpc': begin+1, 'disposition': 'Displayed',
                                 'display_qpc': {'numerator': str(deadline-1), 'denominator': '1'}})
    window = window or TIMING['WINDOW']
    schedule = ANCHOR.analyze_deadlines(rows, outputs, window, TIMING['FREQUENCY'])
    return [schedule, outputs, {'collector_healthy': True, 'present_observations': presents}, {'records': captures}]


def analyze(parts, **trust):
    return DELIVERY(*parts, TIMING['FREQUENCY'], lambda values: {'samples': len(values)} if values else None,
                    **{'telemetry_complete': True, 'capture_complete': True, **trust})


def changed(parts, output):
    token = parts[0]['slots'][10]['token']
    native = next(o for o in parts[1] if o['token'] == token and o['output'] == output)
    witness = (next(o for o in parts[3]['records'] if o['token'] == token) if output == 2 else
               next(o for o in parts[2]['present_observations'] if o['token'] == token and o['output'] == output))
    return parts[0]['slots'][10]['deadline_qpc'], native, witness


class DeliveryDeadlineTests(unittest.TestCase):
    def test_every_expected_slot_including_pre_window_api_uses_exact_delivery_witness(self):
        parts = fixture()
        result = analyze(parts)
        self.assertTrue(result['deadline_verdict'])
        self.assertEqual(result['expected_slots'], 60)
        self.assertEqual(result['outputs'][0]['counts']['OnTime'], 60)
        first = result['outputs'][0]['records'][0]
        self.assertEqual(first['frame_index'], 59)
        trace = next(o for o in parts[1] if o['token'] == first['token'] and o['output'] == 1)
        self.assertLess(trace['consumer_api']['end'], TIMING['WINDOW']['begin_qpc'])

    def test_late_gpu_completion_poll_is_unknown_not_a_definite_miss(self):
        parts = fixture()
        deadline, _, capture = changed(parts, 2)
        capture['completion_observed_qpc'] = deadline+1
        result = analyze(parts)
        self.assertIsNone(result['deadline_verdict'])
        output = result['outputs'][1]
        self.assertEqual(output['counts'].get('Miss', 0), 0)
        self.assertEqual(output['counts']['Unknown'], 1)
        self.assertEqual(output['counts']['late_upper_bounds'], 1)
        self.assertGreater(output['miss_fraction_upper_bound'], 0.001)

    def test_reported_late_software_display_and_dropped_frame_are_definite_observed_misses(self):
        for kind in ('late', 'dropped'):
            parts = fixture()
            deadline, _, display = changed(parts, 1)
            if kind == 'late':
                display['display_qpc'] = {'numerator': str(deadline*2+1), 'denominator': '2'}
            else:
                display['disposition'] = 'Dropped'
                display['display_qpc'] = None
            result = analyze(parts)
            self.assertFalse(result['deadline_verdict'])
            self.assertEqual(result['outputs'][0]['counts']['Miss'], 1)

    def test_late_native_api_begin_proves_miss_even_without_a_capture(self):
        parts = fixture()
        deadline, native, capture = changed(parts, 2)
        native['consumer_api'] = {'begin': deadline+1, 'end': deadline+100}
        parts[3]['records'].remove(capture)
        result = analyze(parts)
        self.assertFalse(result['deadline_verdict'])
        self.assertEqual(result['outputs'][1]['counts']['late_api_begin'], 1)

    def test_cpu_return_preemption_cannot_erase_an_early_display_or_ready_witness(self):
        for output in (1, 2):
            parts = fixture()
            deadline, native, _ = changed(parts, output)
            native['consumer_api']['end'] = deadline+100
            self.assertTrue(analyze(parts)['deadline_verdict'])

    def test_missing_schedule_or_delivery_rows_keep_the_full_denominator_unknown(self):
        for missing in ('schedule', 'window', 'capture'):
            parts = fixture()
            if missing == 'schedule':
                parts[0]['slots'][10].update(known=False, disposition='Missing', token=None)
            else:
                _, _, witness = changed(parts, 1 if missing == 'window' else 2)
                parts[2]['present_observations'].remove(witness) if missing == 'window' else parts[3]['records'].remove(witness)
            result = analyze(parts)
            self.assertIsNone(result['deadline_verdict'])
            self.assertEqual(result['expected_slots'], 60)
            self.assertTrue(any(o['counts'].get('Unknown') for o in result['outputs']))

    def test_foreign_device_does_not_borrow_a_same_serial_witness(self):
        parts = fixture()
        _, _, display = changed(parts, 1)
        display['token'] = copy.deepcopy(display['token'])
        display['token']['device']['generation'] = 2
        self.assertIsNone(analyze(parts)['deadline_verdict'])

    def test_stale_endpoint_duplicate_witness_and_reversed_clock_are_rejected(self):
        for damage in ('endpoint', 'duplicate', 'clock'):
            parts = fixture()
            _, native, display = changed(parts, 1)
            if damage == 'endpoint':
                display['endpoint'] = {'generation': 2, 'addresses': [101]*3}
            elif damage == 'duplicate':
                parts[2]['present_observations'].append(copy.deepcopy(display))
            else:
                display['display_qpc'] = {'numerator': str(native['consumer_api']['begin']-1), 'denominator': '1'}
            with self.assertRaises(ValueError):
                analyze(parts)

    def test_partial_telemetry_or_untrusted_delivery_cannot_pass(self):
        for trust in ({'telemetry_complete': False}, {'capture_complete': False}):
            self.assertIsNone(analyze(fixture(), **trust)['deadline_verdict'])
        parts = fixture()
        parts[2]['collector_healthy'] = False
        self.assertIsNone(analyze(parts)['deadline_verdict'])

    def test_missing_image_or_endpoint_identity_is_not_an_exact_deadline_witness(self):
        for output in (1, 2):
            parts = fixture()
            _, native, witness = changed(parts, output)
            if output == 2:
                native['spout_image'] = witness['spout_image'] = None
            else:
                native['window_present'] = witness['endpoint'] = None
            with self.assertRaises(ValueError):
                analyze(parts)

    def test_contradictory_nonaccepted_display_remains_unknown(self):
        parts = fixture()
        _, native, display = changed(parts, 1)
        native['outcome'] = 'Busy'
        self.assertIsNone(analyze(parts)['deadline_verdict'])
        display['disposition'] = 'Missing'
        display['display_qpc'] = None
        self.assertFalse(analyze(parts)['deadline_verdict'])

    def test_exact_one_per_thousand_budget_is_inclusive(self):
        count = 1000
        end = TIMING['ORIGIN']+(count*TIMING['FREQUENCY']+59)//60+1
        parts = fixture(count, {'begin_qpc': TIMING['ORIGIN']+1, 'end_qpc': end})
        for number in (1, 2):
            display = parts[2]['present_observations'][number*2]
            display['disposition'] = 'Dropped'
            display['display_qpc'] = None
            self.assertEqual(analyze(parts)['deadline_verdict'], number == 1)
