"""Complete app-ingest/render GPU-path bounds without aligning GPU clock origins.

Independent context timestamp origins are never compared. A loose upper bound cannot
establish an actual GPU-budget failure; correlated GPU span sums are descriptive only.
"""

from fractions import Fraction
import json
import math


def bound_float(value, *, upper):
    result = float(value)
    rounded = Fraction(result)
    if (upper and rounded < value) or (not upper and rounded > value):
        result = math.nextafter(result, math.inf if upper else -math.inf)
    return result


def percentile(values, percent):
    ordered = sorted(values)
    return ordered[(len(ordered) * percent + 99) // 100 - 1]


def path_lower_bound(bounds, spans, frequency):
    # A local duration loses at most one GPU-counter tick to endpoint quantization.
    durations = [Fraction(0) if span is None else max(Fraction(0), span[0] - span[1]) for span in spans]
    candidates = [*durations]
    for first, (_, collected) in enumerate(bounds):
        for last, (submitted, _) in enumerate(bounds):
            if first != last:
                # Actual GPU start(last) >= CPU submission(last), and actual GPU
                # end(first) <= CPU collection(first). Include both local durations.
                # A signed gap remains a valid lower bound even when scopes overlap.
                # Two QPC ticks conservatively cover both sampled CPU endpoints.
                gap = Fraction((submitted - collected - 2) * 1000, frequency)
                candidates.append(gap + durations[first] + durations[last])
    return max(candidates)


def analyze_gpu_path(frames, native, frequency, distribution, *, healthy):
    if type(frequency) is not int or frequency <= 0:
        raise ValueError('invalid GPU-path QPC frequency')

    def key(value):
        return json.dumps(value, sort_keys=True, separators=(',', ':'))

    def index(rows, identity):
        result = {}
        for row in rows:
            token = identity(row)
            if token in result:
                raise ValueError('duplicate GPU-path identity')
            result[token] = row
        return result

    uses = index(native['SourceUse'], lambda r: (key(r['token']), r['texture']))
    sources = index(native['SourceCpu'], lambda r: key(r['token']))
    gpu = index(native['Gpu'], lambda r: (key(r['token']), key(r['context'])))
    index(frames, lambda r: key(r['token']))

    def counter(value):
        return type(value) is int and 0 < value < (1 << 64)

    def enclosure(cpu, context):
        row = gpu.get((key(cpu['token']), key(context)))
        if row is None or row.get('status') not in ('Valid', 'Disjoint', 'Invalid'):
            return None, None, 'GPU query collection missing/unsuccessful'
        begin, end = row.get('submitted_qpc'), row.get('completed_qpc')
        wall = cpu.get('wall')
        if not counter(begin) or not counter(end) or not isinstance(wall, dict) \
                or not counter(wall.get('begin')) or not counter(wall.get('end')):
            return None, None, 'query/CPU enclosure clock missing'
        if not wall['begin'] <= begin <= wall['end'] <= end:
            raise ValueError('GPU query enclosure contradicts its CPU submission interval')
        elapsed = None
        if row['status'] == 'Valid':
            first, last, hz = row.get('begin'), row.get('end'), row.get('frequency')
            if type(first) is not int or first < 0 or type(last) is not int or last < first \
                    or not counter(hz):
                raise ValueError('invalid context-local GPU path span')
            elapsed = Fraction((last - first) * 1000, hz)
            if elapsed > Fraction((end - begin + 1) * 1000, frequency) + Fraction(1000, hz):
                raise ValueError('GPU context span exceeds its complete CPU enclosure')
        return (begin, end), None if elapsed is None else (elapsed, Fraction(1000, hz)), None

    records, upper, lower, serial = [], [], [], []
    for frame in frames:
        missing, dependencies = [], []
        for texture in (1, 2):
            use = uses.get((key(frame['token']), texture))
            publication = use.get('publication') if use else None
            source = sources.get(key(publication['source_token'])) if publication else None
            context = source.get('context', {}).get('Source', {}) if source else {}
            if not source or not use.get('sequence', 0) \
                    or context.get('texture') != texture \
                    or context.get('renderer_device') != frame['token']['device']:
                missing.append(f'exact source {texture} dependency missing/foreign')
            else:
                dependencies.append((source, source['context']))
        bounds, spans = [], []
        for cpu, context in [(frame, 'Renderer'), *dependencies]:
            bound, span, reason = enclosure(cpu, context)
            if reason:
                missing.append(reason)
            else:
                bounds.append(bound)
                spans.append(span)
        record = {'renderer_token': frame['token'],
                  'source_tokens': [row['token'] for row, _ in dependencies],
                  'enclosure_begin_qpc': None, 'enclosure_end_qpc': None,
                  'upper_bound_ms': None, 'lower_bound_ms': None, 'context_span_serial_charge_ms': None,
                  'unknown_reasons': sorted(set(missing))}
        if not missing:
            begin, end = min(v[0] for v in bounds), max(v[1] for v in bounds)
            # One CPU-counter tick covers endpoint quantization; do not round a bound down.
            value = Fraction((end - begin + 1) * 1000, frequency)
            upper.append(value)
            minimum = path_lower_bound(bounds, spans, frequency)
            if minimum > value:
                raise ValueError('GPU-path lower bound exceeds its complete CPU enclosure')
            lower.append(minimum)
            record.update(enclosure_begin_qpc=begin, enclosure_end_qpc=end,
                          upper_bound_ms=bound_float(value, upper=True),
                          lower_bound_ms=bound_float(minimum, upper=False))
            if all(v is not None for v in spans):
                charged = sum(v[0] for v in spans)
                serial.append(charged)
                record['context_span_serial_charge_ms'] = float(charged)
        records.append(record)
    complete = healthy and bool(frames) and len(upper) == len(frames)
    observed = distribution([bound_float(v, upper=True) for v in upper])
    sufficient = complete and percentile(upper, 95) <= 12 and percentile(upper, 99) <= Fraction(16667, 1000)
    failed = complete and (percentile(lower, 95) > 12 or percentile(lower, 99) > Fraction(16667, 1000))
    lower_exact = {f'p{percent}': {'numerator': str(value.numerator), 'denominator': str(value.denominator)}
                   for percent in (95, 99) if lower
                   for value in [percentile(lower, percent)]}
    return {'expected_frames': len(frames), 'covered_frames': len(upper), 'coverage_complete': complete,
            'cpu_enclosed_gpu_path_upper_bound': observed,
            'gpu_path_lower_bound': distribution([bound_float(v, upper=False) for v in lower]),
            'gpu_path_lower_percentile_exact_ms': lower_exact,
            'context_span_serial_charge_not_critical_path': distribution([float(v) for v in serial]),
            'upper_bound_sufficient_for_gpu_target': bool(sufficient),
            'definite_failure_lower_bound_exceeds_budget': bool(failed),
            'cpu_endpoint_quantization_allowance_ticks': 1,
            'cpu_gap_quantization_allowance_ticks': 2,
            'gpu_duration_quantization_allowance_ticks': 1,
            'observed_within_target': True if sufficient else False if failed else None, 'records': records,
            'scope': 'both exact retained app source-worker GPU batches and renderer/output batch, before first marker submission through last successful query collection',
            'limitations': ['CPU enclosure includes GPU queues, command-feed gaps, retained-frame age and collection delay',
                            'an upper bound above budget is inconclusive, not an observed GPU failure',
                            'GPU clocks are not aligned; the context-span serial charge omits gaps and cannot qualify the critical path',
                            'external producer/consumer work, scanout and physical input response are separate measurements']}
