import copy
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from qpc_latency import audit


def fixture():
    samples, events, witnesses = [], [], []
    for index in range(30):
        direction = 1 if index % 2 == 0 else -1
        def frame(shift):
            pixels = [(0, 0, 0, 255)] * 32
            pixels[12 + shift] = (220, 150, 120, 255)
            for x in range(16 + shift, 32):
                pixels[x] = (0, 0, 255, 255)
            return bytes(channel for pixel in pixels for channel in pixel)
        before, response = {'red': 16, 'blue': 12.0}, {'red': 16 + direction * 3, 'blue': 12.0 + direction * 3}
        begin = 10_000_000_000_000_000 + index * 100000
        samples.append({'index': index, 'deltaY': direction * 120, 'begin_qpc': str(begin),
                        'before_qpc': str(begin - 10000), 'acquire_begin_qpc': str(begin + 30000),
                        'response_qpc': str(begin + 40000), 'after_qpc': str(begin + 41000),
                        'desktop_update_qpc': str(begin + 39000), 'upper_bound_ms': 40.0,
                        'before_serial': index * 10, 'response_serial': index * 10 + 2,
                        'before_protected_content_masked_out': False,
                        'response_protected_content_masked_out': False,
                        'before': before, 'response': response})
        events.append({'trusted': True, 'deltaY': direction * 120})
        witnesses.extend([frame(0), frame(direction * 3)])
    return {'region': {'left': 0, 'top': 0, 'width': 32, 'height': 1}, 'frequency': '1000000', 'observations': samples,
            'pid': 2, 'hwnd': 1, 'geometry': {'desktop_bounds': [0, 0, 32, 1], 'window_dpi': 96},
            'events': events, 'p95_upper_bound_ms': 40.0, 'p99_upper_bound_ms': 40.0, 'maximum_upper_bound_ms': 40.0,
            'within_50_ms': True}, b''.join(witnesses)


def state(result):
    return {'ready': {'frequency': result['frequency'], 'region': [0, 0, 32, 1],
                      'geometry': {'window': result['geometry']['desktop_bounds'], 'dpi': 96}},
            'complete': True, 'failure': None, 'stopped': True, 'observations': copy.deepcopy(result['observations']),
            'exit': {'code': 0, 'signal': None},
            'command': ['capture.exe', '--pid', '2', '--hwnd', '1', '--left', '0', '--top', '0',
                        '--width', '32', '--height', '1']}


class NativeQpcLatency(unittest.TestCase):
    def test_desktop_mask_notice_never_replaces_actual_owned_image_and_overlay_pixels(self):
        result, witnesses = fixture()
        for sample in result['observations']:
            sample['before_protected_content_masked_out'] = True
            sample['response_protected_content_masked_out'] = True
        observed = audit(result, witnesses, state(result))
        self.assertEqual(observed['masked_desktop_observations'], 30)
        with self.assertRaises(AssertionError):
            audit(result, bytes(len(witnesses)), state(result))
        result['observations'][0]['response_protected_content_masked_out'] = 'true'
        with self.assertRaises(AssertionError):
            audit(result, witnesses, state(result))

    def test_p99_cannot_be_replaced_with_a_better_claim(self):
        result, witnesses = fixture()
        result['p99_upper_bound_ms'] = 1
        with self.assertRaises(AssertionError):
            audit(result, witnesses, state(result))

    def test_one_native_clock_and_full_original_cohort(self):
        result, witnesses = fixture()
        self.assertTrue(audit(result, witnesses, state(result))['within_50_ms'])

    def test_stale_or_future_desktop_image_cannot_be_a_response(self):
        result, witnesses = fixture()
        for value in [result['observations'][0]['begin_qpc'], str(int(result['observations'][0]['response_qpc']) + 1)]:
            broken = copy.deepcopy(result)
            broken['observations'][0]['desktop_update_qpc'] = value
            with self.assertRaises(AssertionError):
                audit(broken, witnesses, state(broken))

    def test_missing_action_untrusted_input_and_forged_clock_fail(self):
        result, witnesses = fixture()
        for kind in ['missing', 'untrusted', 'clock']:
            broken = copy.deepcopy(result)
            if kind == 'missing':
                broken['observations'].pop()
            elif kind == 'untrusted':
                broken['events'][0]['trusted'] = False
            else:
                broken['observations'][0]['upper_bound_ms'] = 1
            with self.assertRaises(AssertionError):
                audit(broken, witnesses, state(broken))

    def test_no_pixels_or_forged_edge_metadata_cannot_pass(self):
        result, witnesses = fixture()
        with self.assertRaises(AssertionError):
            audit(result, witnesses[:-1], state(result))
        result['observations'][0]['response']['red'] += 1
        with self.assertRaises(AssertionError):
            audit(result, witnesses, state(result))

    def test_native_clock_identity_and_terminal_capture_are_required(self):
        result, witnesses = fixture()
        for kind in ['frequency', 'identity', 'exit']:
            broken = state(result)
            if kind == 'frequency':
                broken['ready']['frequency'] = '2000000'
            elif kind == 'identity':
                broken['command'][4] = '3'
            else:
                broken['exit']['code'] = 1
            with self.assertRaises(AssertionError):
                audit(result, witnesses, broken)

    def test_stale_baseline_invalid_acquisition_and_incomplete_terminal_state_fail(self):
        result, witnesses = fixture()
        for kind in ['baseline', 'acquisition', 'terminal', 'stop', 'maximum', 'region']:
            broken, terminal = copy.deepcopy(result), state(result)
            if kind == 'baseline':
                broken['observations'][0]['before_qpc'] = str(int(broken['observations'][0]['begin_qpc']) - 25001)
            elif kind == 'acquisition':
                broken['observations'][0]['acquire_begin_qpc'] = str(int(broken['observations'][0]['response_qpc']) + 1)
            elif kind == 'terminal':
                terminal['observations'].pop()
            elif kind == 'stop':
                terminal['stopped'] = False
            elif kind == 'maximum':
                broken['maximum_upper_bound_ms'] = 1
            else:
                terminal['command'][-1] = '2'
            if kind in ['baseline', 'acquisition']:
                terminal['observations'] = copy.deepcopy(broken['observations'])
            with self.assertRaises(AssertionError, msg=kind):
                audit(broken, witnesses, terminal)


if __name__ == '__main__':
    unittest.main()
