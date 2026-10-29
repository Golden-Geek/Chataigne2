"""Audit owned TraceProcessor packet coverage without inventing missing GPU times."""

import argparse
from collections import Counter
from fractions import Fraction
import hashlib
import json
from pathlib import Path

TIMES = ('software_submit', 'hardware_submit', 'execution_start', 'execution_complete')
DURATIONS = ('software_queue', 'hardware_queue', 'execution')
ROLES = ('renderer', 'source-1', 'source-2', 'consumer')


def require(condition, message):
    if not condition:
        raise ValueError(message)


def integer(value, label, minimum=0):
    require(type(value) is int and minimum <= value <= (1 << 63)-1, f'invalid {label}')
    return value


def field(record, key, unit):
    value = record[key]
    require(isinstance(value, dict) and set(value) == {'has_value', 'is_partial', unit}, f'invalid {key}')
    require(type(value['has_value']) is bool and type(value['is_partial']) is bool, f'invalid {key} flags')
    if value['is_partial'] or not value['has_value']:
        require(value[unit] is None, f'unavailable {key} must not contain a fabricated time')
        return None
    return integer(value[unit], key)


def distribution(values):
    if not values:
        return None
    ordered = sorted(values)
    return {'samples': len(values), **{
        name: float(ordered[(percent*len(values)+99)//100-1])
        for name, percent in [('median_ms', 50), ('p95_ms', 95), ('p99_ms', 99), ('maximum_ms', 100)]}}


def analyze(report, manifest, manifest_sha256):
    require(type(report['schema_version']) is int and report['schema_version'] == 1, 'unsupported DMA observation schema')
    require(report['manifest_sha256'] == manifest_sha256, 'native manifest checksum differs')
    require(report['run_id'] == manifest['run_id'] and report['scenario'] == manifest['scenario'],
            'native run identity differs')
    frequency = integer(report['frequency'], 'trace frequency', 1)
    native_frequency = integer(manifest['qpc_frequency'], 'native frequency', 1)
    clock_ok = report['clock_type'] == 'PerformanceCounter' and frequency == native_frequency == report['native_frequency']
    losses = {k: integer(report[k], k) for k in ('lost_events', 'lost_buffers')}
    pids = [manifest['pid'], *manifest['fixture_pids']]
    require(len(pids) == 4 and len(set(pids)) == 4, 'expected four distinct native PIDs')
    expected = dict(zip(ROLES, [integer(p, 'PID', 1) for p in pids]))
    require(len(report['roles']) == 4 and {r['role']: integer(r['pid'], 'role PID', 1) for r in report['roles']} == expected,
            'owned role/PID mapping differs')
    grouped = {role: [] for role in ROLES}
    for packet in report['owned_packets']:
        require(packet['role'] in expected and integer(packet['pid'], 'packet PID', 1) == expected[packet['role']],
                'foreign packet PID or role')
        for key in ('adapter_id', 'node_id', 'node_engine_id'):
            integer(packet[key], key)
        times = [field(packet, key, 'qpc') for key in TIMES]
        durations = [field(packet, key, 'nanoseconds') for key in DURATIONS]
        # Validate all available pairs, even when other boundaries are unavailable.
        known = [t for t in times if t is not None]
        require(known == sorted(known), 'inverted DMA boundaries')
        for begin, end, duration in zip(times, times[1:], durations):
            if begin is not None and end is not None and duration is not None and clock_ok:
                error_ns = abs(Fraction((end-begin)*1_000_000_000, frequency)-duration)
                require(error_ns <= Fraction(1_000_000_000, frequency)+1, 'DMA duration differs from its QPC boundaries')
        grouped[packet['role']].append((packet, times, durations))
    outputs = []
    healthy = clock_ok and not any(losses.values())
    for role, rows in grouped.items():
        missing = Counter()
        observed = {key: [] for key in DURATIONS}
        covered = 0
        for packet, times, durations in rows:
            for key, value in zip((*TIMES, *DURATIONS), (*times, *durations)):
                if value is None:
                    missing[key] += 1
            complete = all(value is not None for value in (*times, *durations))
            covered += complete
            # Partial records never enter a timing distribution as observed zero.
            if complete and clock_ok:
                for key, begin, end in zip(DURATIONS, times, times[1:]):
                    observed[key].append(Fraction((end-begin)*1000, frequency))
        outputs.append({'role': role, 'pid': expected[role], 'expected_packets': len(rows),
                        'complete_packets': covered, 'unavailable_packets': len(rows)-covered,
                        'missing_fields': dict(missing),
                        'timing_coverage_complete': bool(rows) and covered == len(rows) and healthy,
                        'observed_complete_packet_intervals': {k: distribution(v) for k, v in observed.items()}})
    return {'schema_version': 1, 'evidence_kind': 'owned_dma_packet_coverage_not_qualification',
            'run_id': manifest['run_id'], 'clock_compatible': clock_ok, **losses,
            'timing_coverage_complete': all(r['timing_coverage_complete'] for r in outputs),
            'roles': outputs, 'gpu_critical_path_status': 'UNAVAILABLE',
            'limitations': ['packet intervals have no exact frame, source-image, D3D11 context or output association',
                            'trace-local adapter/node IDs do not identify the selected DXGI adapter LUID',
                            'missing packets are not detectable from this API; complete observed records do not prove complete native coverage',
                            'no native-buffer occupancy, per-output queue or GPU critical-path gate is closed']}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('packets', type=Path)
    parser.add_argument('manifest', type=Path)
    parser.add_argument('new_output', type=Path)
    args = parser.parse_args()
    manifest_bytes = args.manifest.read_bytes()
    report = analyze(json.loads(args.packets.read_text(encoding='utf-8')), json.loads(manifest_bytes),
                     hashlib.sha256(manifest_bytes).hexdigest())
    with args.new_output.open('x', encoding='utf-8', newline='\n') as output:
        json.dump(report, output, indent=2, allow_nan=False)
        output.write('\n')


if __name__ == '__main__':
    main()
