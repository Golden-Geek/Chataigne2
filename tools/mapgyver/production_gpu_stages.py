"""Exact same-context renderer partitions; no cross-context clock alignment is inferred."""

from collections import Counter
import json


def _uint(value, name, minimum=0):
    if type(value) is not int or not minimum <= value < (1 << 64):
        raise ValueError(f'invalid renderer GPU {name}')
    return value


def _token(token):
    device = token['device']
    if not isinstance(device['runtime'], str) or not device['runtime']:
        raise ValueError('renderer GPU token has no runtime authority')
    _uint(device['generation'], 'device generation')
    _uint(token['serial'], 'batch serial', 1)
    return json.dumps(token, sort_keys=True)


def analyze_renderer_stages(frames, gpu_rows, distribution):
    indexed = {}
    for row in gpu_rows:
        if row['context'] != 'Renderer':
            continue
        key = _token(row['token'])
        if key in indexed:
            raise ValueError('duplicate renderer GPU stage identity')
        indexed[key] = row
    intervals = {name: [] for name in ('inputs', 'render', 'outputs')}
    records, coverage, dominant, seen = [], Counter(), Counter(), set()
    for frame in frames:
        key = _token(frame['token'])
        if key in seen:
            raise ValueError('duplicate renderer frame in GPU stage denominator')
        seen.add(key)
        row = indexed.get(key)
        record = {'token': frame['token'], 'stage_ms': None, 'missing': None}
        records.append(record)
        if row is None:
            record['missing'] = 'GPU row missing'
        elif row['status'] != 'Valid':
            record['missing'] = 'whole GPU span unavailable'
        else:
            begin = _uint(row['begin'], 'begin')
            end = _uint(row['end'], 'end', begin)
            frequency = _uint(row['frequency'], 'frequency', 1)
            inputs = row.get('renderer_inputs_end')
            render = row.get('renderer_render_end')
            if inputs is None or render is None:
                record['missing'] = 'renderer stage markers unavailable'
            else:
                inputs = _uint(inputs, 'inputs boundary', begin)
                render = _uint(render, 'render boundary', inputs)
                if render > end:
                    raise ValueError('renderer GPU partitions exceed the enclosing span')
                spans = (inputs-begin, render-inputs, end-render)
                assert sum(spans) == end-begin
                values = {name: ticks*1000/frequency for name, ticks in zip(intervals, spans)}
                record['stage_ms'] = values
                for name, value in values.items():
                    intervals[name].append(value)
                maximum = max(values.values())
                largest = 'tied' if sum(v == maximum for v in values.values()) > 1 else max(values, key=values.get)
                dominant[largest] += 1
        coverage[record['missing'] or 'complete'] += 1
    return {'expected_batches': len(frames), 'covered_batches': coverage['complete'],
            'coverage_complete': bool(frames) and coverage['complete'] == len(frames),
            'coverage': dict(coverage), 'stages': {name: distribution(values) for name, values in intervals.items()},
            'largest_stage_counts': dict(dominant), 'records': records,
            'scope': 'same-clock partitions of each exact renderer span; includes GPU waits, scheduling and command-feed gaps',
            'limitations': ['stage percentiles cannot be added to reconstruct a frame percentile',
                            'these spans do not separate active GPU work from queue or dependency waits',
                            'no cross-context critical path or native buffer occupancy is inferred']}
