import copy
from pathlib import Path
import runpy
import unittest

ANALYZE = runpy.run_path(str(Path(__file__).resolve().parents[1]/'dma_packets.py'))['analyze']
ROLES = ('renderer', 'source-1', 'source-2', 'consumer')


def fixture():
    manifest = {'pid': 10, 'fixture_pids': [11, 12, 13], 'run_id': 'owned', 'scenario': 'Q02', 'qpc_frequency': 1000}
    record = {'adapter_id': 0, 'node_id': 0, 'node_engine_id': 0, 'type': 'Render'}
    for key, qpc in zip(('software_submit', 'hardware_submit', 'execution_start', 'execution_complete'), (100, 102, 105, 109)):
        record[key] = {'has_value': True, 'is_partial': False, 'qpc': qpc}
    for key, ns in zip(('software_queue', 'hardware_queue', 'execution'), (2_000_000, 3_000_000, 4_000_000)):
        record[key] = {'has_value': True, 'is_partial': False, 'nanoseconds': ns}
    report = {'schema_version': 1, 'manifest_sha256': 'hash', 'run_id': 'owned', 'scenario': 'Q02',
              'frequency': 1000, 'native_frequency': 1000, 'clock_type': 'PerformanceCounter',
              'lost_events': 0, 'lost_buffers': 0,
              'roles': [{'pid': pid, 'role': role} for pid, role in zip(range(10, 14), ROLES)],
              'owned_packets': [dict(copy.deepcopy(record), pid=pid, role=role) for pid, role in zip(range(10, 14), ROLES)]}
    return report, manifest


def analyze(parts):
    return ANALYZE(*parts, 'hash')


class DmaPacketTests(unittest.TestCase):
    def test_complete_packet_timings_keep_scope_separate_from_qualification(self):
        result = analyze(fixture())
        self.assertTrue(result['timing_coverage_complete'])
        self.assertEqual(result['roles'][0]['observed_complete_packet_intervals']['hardware_queue']['p95_ms'], 3)
        self.assertEqual(result['gpu_critical_path_status'], 'UNAVAILABLE')

    def test_partial_and_missing_fields_never_become_zero_or_reduce_cohort(self):
        for partial in (True, False):
            report, manifest = fixture()
            field = report['owned_packets'][0]['execution_start']
            field.update(is_partial=partial, has_value=partial, qpc=None)
            result = analyze((report, manifest))['roles'][0]
            self.assertEqual((result['expected_packets'], result['complete_packets'], result['unavailable_packets']), (1, 0, 1))
            self.assertEqual(result['missing_fields'], {'execution_start': 1})
            self.assertIsNone(result['observed_complete_packet_intervals']['execution'])
            self.assertFalse(result['timing_coverage_complete'])

    def test_partial_field_with_fabricated_number_is_rejected(self):
        parts = fixture()
        parts[0]['owned_packets'][0]['software_queue']['is_partial'] = True
        with self.assertRaisesRegex(ValueError, 'fabricated'):
            analyze(parts)

    def test_losses_or_clock_mismatch_prevent_coverage_claim(self):
        for key, value in [('lost_events', 1), ('lost_buffers', 1), ('clock_type', 'SystemTime'), ('frequency', 2000)]:
            parts = fixture()
            parts[0][key] = value
            self.assertFalse(analyze(parts)['timing_coverage_complete'])

    def test_absent_owned_process_is_not_an_empty_queue_pass(self):
        parts = fixture()
        parts[0]['owned_packets'] = parts[0]['owned_packets'][1:]
        result = analyze(parts)
        self.assertFalse(result['timing_coverage_complete'])
        self.assertEqual(result['roles'][0]['expected_packets'], 0)

    def test_reversed_boundaries_and_wrong_durations_are_rejected(self):
        for key, unit, value in [('hardware_submit', 'qpc', 99), ('hardware_queue', 'nanoseconds', 9_000_000)]:
            parts = fixture()
            parts[0]['owned_packets'][0][key][unit] = value
            with self.assertRaises(ValueError):
                analyze(parts)

    def test_manifest_role_and_packet_identity_are_checked(self):
        for mode in ('hash', 'run', 'pid', 'duplicate', 'foreign', 'bool frequency'):
            parts = fixture()
            if mode == 'hash':
                parts[0]['manifest_sha256'] = 'different'
            elif mode == 'run':
                parts[0]['run_id'] = 'different'
            elif mode == 'pid':
                parts[0]['roles'][0]['pid'] = 99
            elif mode == 'duplicate':
                parts[1]['fixture_pids'][0] = 10
            elif mode == 'foreign':
                parts[0]['owned_packets'][0]['pid'] = 99
            else:
                parts[0]['frequency'] = True
            with self.subTest(mode=mode), self.assertRaises(ValueError):
                analyze(parts)

    def test_zero_duration_is_usable_only_when_explicit_and_complete(self):
        parts = fixture()
        packet = parts[0]['owned_packets'][0]
        packet['hardware_submit']['qpc'] = 100
        packet['software_queue']['nanoseconds'] = 0
        packet['hardware_queue']['nanoseconds'] = 5_000_000
        result = analyze(parts)
        self.assertTrue(result['timing_coverage_complete'])
        self.assertEqual(result['roles'][0]['observed_complete_packet_intervals']['software_queue']['p95_ms'], 0)
