"""Observe the rejected shared-first hypothesis in fixed Q01/Q02 CPU traces.

The retained renderer uses plan order, so a false ordering_verified is expected there.
This observation is not an acceptance criterion, GPU-ready time or queue measurement.
"""
import json


def analyze_output_order(frames, outputs, healthy=True):
    key = lambda token: json.dumps(token, sort_keys=True)
    frame_keys = [key(row['token']) for row in frames]
    if len(set(frame_keys)) != len(frame_keys):
        raise ValueError('duplicate output-order frame')
    indexed = {}
    for row in outputs:
        identity = (key(row['token']), row['output'])
        if identity in indexed:
            raise ValueError('duplicate output-order record')
        indexed[identity] = row

    def interval(value):
        if value is None:
            return None
        if not all(type(value.get(k)) is int and 0 < value[k] < (1 << 64) for k in ('begin', 'end')) \
                or value['begin'] > value['end']:
            raise ValueError('invalid output-order interval')
        return value

    records, complete, violations, no_calls = [], 0, 0, 0
    for frame, identity in zip(frames, frame_keys):
        rows = [indexed.get((identity, output)) for output in (1, 2, 3)]
        missing = [] if healthy else ['native telemetry is not trusted']
        wall = interval(frame.get('wall'))
        if wall is None:
            missing.append('CPU frame wall timing missing')
        broken = []
        conversions, calls, shared_phases = [], [], []
        for output, row in zip((1, 2, 3), rows):
            if row is None:
                missing.append(f'output {output} record missing')
                continue
            conversion, api = interval(row.get('conversion')), interval(row.get('consumer_api'))
            delivery = interval(row.get('delivery'))
            if wall is not None and any(v is not None and not wall['begin'] <= v['begin'] <= v['end'] <= wall['end']
                                        for v in (conversion, api, delivery)):
                missing.append(f'output {output} timing outside CPU frame')
            if conversion is not None:
                conversions.append((output, conversion))
            if output == 2:
                if delivery is None:
                    missing.append('shared publication phase timing missing')
                else:
                    shared_phases.append(delivery)
            if api is not None:
                calls.append((output, api))
                if conversion is None:
                    missing.append(f'output {output} conversion timing missing')
                elif conversion['end'] > api['begin']:
                    broken.append(f'output {output} API precedes its conversion')
                if output in (1, 3) and row.get('window_present') is None:
                    missing.append(f'output {output} native Present identity missing')
            outcome = row['outcome'] if isinstance(row['outcome'], str) else next(iter(row['outcome']))
            if outcome == 'Accepted' and api is None:
                missing.append(f'output {output} accepted API timing missing')
            if outcome == 'Busy' and conversion is not None and api is None:
                missing.append(f'output {output} late-Busy API timing missing')
        if shared_phases:
            shared_end = max(v['end'] for v in shared_phases)
            for output in (1, 3):
                work = [v for target, v in conversions+calls if target == output]
                if work and min(v['begin'] for v in work) < shared_end:
                    broken.append(f'window {output} work precedes shared publication phase completion')
        covered = not missing
        complete += covered
        violations += bool(broken)
        no_calls += not calls
        records.append({'token': frame['token'], 'covered': covered, 'unknown': missing,
                        'violations': broken, 'actual_api_calls': len(calls)})
    return {'schema_version': 2, 'expected_frames': len(frames), 'covered_frames': complete,
            'unknown_frames': len(frames)-complete, 'frames_with_violations': violations,
            'frames_without_api_calls': no_calls,
            'ordering_verified': bool(frames) and complete == len(frames) and violations == 0,
            'records': records,
            'scope': 'fixed Q01/Q02 CPU-start-selected tokens; each conversion precedes its API and shared phases finish before window work; not GPU execution/completion',
            'limitations': ['rejected endpoints can have no conversion/API work; this does not qualify cadence',
                            'no GPU queue, native occupancy or complete GPU critical path is inferred']}
