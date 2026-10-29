"""Anchored consumer deadline bounds, keeping late GPU polls explicitly uncertain."""

from collections import Counter
from fractions import Fraction
import json

MISS_BUDGET = Fraction(1, 1000)


def _uint(value, label, minimum=0):
    if type(value) is not int or not minimum <= value < (1 << 64):
        raise ValueError(f'invalid {label}')
    return value


def _key(value):
    return json.dumps(value, sort_keys=True)


def _token(value):
    device = value['device']
    if not isinstance(device['runtime'], str) or not device['runtime']:
        raise ValueError('missing deadline token authority')
    _uint(device['generation'], 'deadline device generation')
    _uint(value['serial'], 'deadline batch serial', 1)
    return _key(value)


def _unique(rows, key, label):
    result = {}
    for row in rows:
        identity = key(row)
        if identity in result:
            raise ValueError(f'duplicate {label}')
        result[identity] = row
    return result


def _rational(value):
    parts = [value[name] for name in ('numerator', 'denominator')]
    if any(not isinstance(v, str) or not 0 < len(v) <= 128 or not v.isascii() or not v.isdecimal() for v in parts):
        raise ValueError('invalid lossless display QPC')
    numerator, denominator = map(int, parts)
    if numerator <= 0 or denominator <= 0:
        raise ValueError('nonpositive display QPC')
    return Fraction(numerator, denominator)


def _outcome(trace):
    value = trace['outcome']
    name = value if isinstance(value, str) else next(iter(value)) if isinstance(value, dict) and len(value) == 1 else None
    if name not in ('Accepted', 'Busy', 'Unavailable', 'Failed', 'DeviceLost'):
        raise ValueError('unknown output disposition in deadline cohort')
    return name


def analyze_delivery_deadlines(schedule, outputs, windows, spout, frequency, distribution, *,
                               telemetry_complete, capture_complete):
    _uint(frequency, 'QPC frequency', 1)
    if not schedule['mapping_available']:
        return {'mapping_available': False, 'coverage_complete': False, 'expected_slots': None,
                'deadline_verdict': None, 'outputs': [], 'reason': schedule['reason']}
    slots = schedule['slots']
    if not slots or len(slots) != schedule['expected_slots']:
        raise ValueError('delivery deadline denominator differs from anchored slots')
    if any(right['frame_index'] != left['frame_index']+1 or right['deadline_qpc'] <= left['deadline_qpc']
           for left, right in zip(slots, slots[1:])):
        raise ValueError('delivery slots are not an ordered complete ordinal range')
    ids = [o['output'] for o in schedule['outputs']]
    if not ids or len(set(ids)) != len(ids) or any(i not in (1, 2, 3) for i in ids):
        raise ValueError('unsupported or duplicate deadline output')
    output_map = _unique(outputs, lambda o: (_token(o['token']), o['output']), 'native output observation')
    window_map = _unique(windows.get('present_observations', []),
                         lambda o: (_token(o['token']), o['output']), 'window delivery observation')
    spout_map = _unique(spout['records'], lambda o: _token(o['token']), 'Spout delivery observation')
    reports = []
    for output_id in ids:
        counts, records, lateness = Counter(), [], []
        for slot in slots:
            deadline = _uint(slot['deadline_qpc'], 'delivery deadline', 1)
            record = {'frame_index': slot['frame_index'], 'deadline_qpc': deadline, 'token': slot['token'],
                      'classification': 'Unknown', 'reason': None, 'ready_or_display_upper_qpc': None}
            records.append(record)
            if not slot['known']:
                record['reason'] = 'scheduler observation missing'
            elif slot['disposition'] != 'Submitted':
                record.update(classification='Miss', reason='known unsubmitted scheduler opportunity')
                counts['unsubmitted'] += 1
            else:
                token = _token(slot['token'])
                if slot['token']['device'] != schedule['device']:
                    raise ValueError('deadline token uses a foreign device lifetime')
                trace = output_map.get((token, output_id))
                if trace is None:
                    record['reason'] = 'native output observation missing'
                elif _outcome(trace) != 'Accepted':
                    observed = window_map.get((token, output_id)) if output_id != 2 else None
                    if observed and observed['disposition'] == 'Displayed' and windows.get('collector_healthy'):
                        record['reason'] = 'display observation contradicts nonaccepted native disposition'
                    else:
                        record.update(classification='Miss', reason='output did not accept this scheduled frame')
                        counts['not_accepted'] += 1
                else:
                    api = trace.get('consumer_api')
                    if api is None:
                        record['reason'] = 'native API interval unavailable'
                    else:
                        begin = _uint(api['begin'], 'delivery API begin', 1)
                        _uint(api['end'], 'delivery API end', begin)
                        observed = None
                        if output_id == 2:
                            capture = spout_map.get(token)
                            if capture and capture['missing'] is None and capture_complete:
                                observed = _uint(capture['completion_observed_qpc'], 'capture completion upper bound', 1)
                                if capture['spout_image'] is None or capture['spout_image'] != trace.get('spout_image'):
                                    raise ValueError('Spout deadline witness identifies another output image')
                                counts['capture_witnesses'] += 1
                        else:
                            present = window_map.get((token, output_id))
                            if present and windows.get('collector_healthy'):
                                if present['endpoint'] is None or present['endpoint'] != trace.get('window_present'):
                                    raise ValueError('window deadline witness identifies another endpoint incarnation')
                                qpc = _uint(present['present_qpc'], 'present QPC', 1) if present['present_qpc'] is not None else None
                                if qpc is not None and not begin <= qpc <= api['end']:
                                    raise ValueError('deadline Present does not belong to the exact API interval')
                                if present['disposition'] == 'Dropped':
                                    record.update(classification='Miss', reason='reported software display dropped this frame')
                                    counts['dropped'] += 1
                                elif present['disposition'] == 'Displayed':
                                    observed = _rational(present['display_qpc'])
                                    if qpc is None or observed < qpc:
                                        raise ValueError('display clock precedes its Present')
                                    counts['display_witnesses'] += 1
                        if observed is not None:
                            if observed < begin:
                                raise ValueError('delivery witness precedes native publication API')
                            bound = Fraction(observed)
                            record['ready_or_display_upper_qpc'] = {'numerator': str(bound.numerator),
                                                                  'denominator': str(bound.denominator)}
                            lateness.append(float(max(0, bound-deadline)*1000/frequency))
                            if observed <= deadline:
                                record.update(classification='OnTime', reason='observed completion/display by deadline')
                            elif output_id != 2:
                                record.update(classification='Miss', reason='reported software display after deadline')
                                counts['late_display'] += 1
                            else:
                                record['reason'] = 'late capture observation does not prove late GPU readiness'
                                counts['late_upper_bounds'] += 1
                        if record['classification'] == 'Unknown' and begin > deadline:
                            record.update(classification='Miss', reason='native publication API began after deadline')
                            counts['late_api_begin'] += 1
                        if record['classification'] == 'Unknown' and record['reason'] is None:
                            record['reason'] = 'trusted exact delivery observation unavailable'
            counts[record['classification']] += 1
        expected = len(slots)
        complete = counts['Unknown'] == 0 and telemetry_complete
        lower = counts['Miss']/expected
        upper = (counts['Miss']+counts['Unknown'])/expected
        verdict = False if Fraction(counts['Miss'], expected) > MISS_BUDGET else \
                  True if complete and Fraction(counts['Miss']+counts['Unknown'], expected) <= MISS_BUDGET else None
        reports.append({'output': output_id, 'expected_slots': expected, 'counts': dict(counts),
                        'coverage_complete': complete, 'miss_fraction_lower_bound': lower,
                        'miss_fraction_upper_bound': upper, 'deadline_verdict': verdict,
                        'observation_lateness_upper_bound': distribution(lateness), 'records': records,
                        'endpoint': 'Spout GPU-ready upper bound from exact independent capture' if output_id == 2 else
                                    'reported window software display'})
    verdicts = [o['deadline_verdict'] for o in reports]
    verdict = False if False in verdicts else True if all(v is True for v in verdicts) else None
    return {'mapping_available': True, 'coverage_complete': all(o['coverage_complete'] for o in reports),
            'expected_slots': len(slots), 'cadence': schedule['cadence'], 'device': schedule['device'],
            'miss_budget': {'numerator': MISS_BUDGET.numerator, 'denominator': MISS_BUDGET.denominator},
            'deadline_verdict': verdict, 'outputs': reports,
            'scope': 'all anchored deadlines in the sample; window software display and Spout GPU-ready bounds, not physical scanout',
            'limitations': ['late CPU completion polls are upper bounds, never automatic GPU deadline misses',
                            'missing or untrusted observations stay unknown; no frames are removed from the denominator',
                            'software display observations do not establish physical scanout or a GPU critical path']}
