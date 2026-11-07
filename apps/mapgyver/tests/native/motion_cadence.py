"""Replay a complete held-input WGC cohort using independent pixels and original timestamps."""
import json
import math
from pathlib import Path
import sys

import numpy as np
from landmark_edges import crossing


def measure(raw, width, height):
    pixels = np.frombuffer(raw, dtype=np.uint8).reshape(height, width, 4)[height // 2]
    b, g, r = pixels[:, 0], pixels[:, 1], pixels[:, 2]
    red = np.flatnonzero((r > 160) & (g < 80) & (b < 100))
    blue = np.flatnonzero((b > 180) & (r > 70) & (r < 190) & (g > 110) & (g < 215))
    assert len(red) and len(blue), 'Every active frame needs independent image/control pixels'
    group = np.split(blue, np.flatnonzero(np.diff(blue) > 1) + 1)[0]
    line = [(x, [int(pixel[2]), int(pixel[1]), int(pixel[0])]) for x, pixel in enumerate(pixels)]
    try:
        continuous = crossing(line, float(red[0]), True)
    except AssertionError:
        continuous = None
    if any(abs(float(at) + .5 - float(red[0])) <= 8 for at in blue):
        continuous = None
    return continuous, float(group.mean()) + .5, float(red[0])


def audit(result, raw):
    assert result['failure'] is None and result['problem'] is None
    assert result['exit'] == {'code': 0, 'signal': None}
    assert result['geometry'] == result['after']
    width, height = result['region']['width'], result['region']['height']
    size = width * height * 4
    assert len(raw) == len(result['frames']) * size
    assert 1 <= width <= 256 and 1 <= height <= 32
    assert result['geometry']['window_dpi'] == result['projection']['dpr'] * 96
    events = result['events']
    assert all(event['trusted'] for event in events)
    assert events[0]['type'] == 'pointerdown' and events[-1]['type'] == 'pointerup'
    assert all(a['at'] <= b['at'] for a, b in zip(events, events[1:]))
    assert [event['type'] for event in events].count('pointerdown') == 1
    assert [event['type'] for event in events].count('pointerup') == 1
    moves = [event for event in events if event['type'] == 'pointermove' and event['buttons'] == 1]
    assert len(moves) == len(events) - 2
    assert result['requested'] - 1 <= len(moves) <= result['requested'] + 1
    assert len(moves) > 200, 'A sustained real-input cohort is required'
    timestamps = result['timestamps']
    assert len(timestamps) in (len(result['frames']), len(result['frames']) + 1)
    assert all(row['serial'] == index for index, row in enumerate(timestamps))
    times = {row['serial']: int(row['ticks']) / 10_000_000 for row in timestamps}
    corners = np.asarray(result['projection']['corners'], dtype=float)
    assert corners.shape == (4, 2) and np.isfinite(corners).all()
    inner = np.asarray([corners[0] + (corners[1] - corners[0]) * u +
                        (corners[3] - corners[0]) * v
                        for u, v in [(.1, .1), (.9, .1), (.9, .9), (.1, .9)]])
    y = float((corners[0, 1] + corners[2, 1]) / 2)

    def left_edge(quad):
        crossings = [float(a[0] + (b[0] - a[0]) * (y - a[1]) / (b[1] - a[1]))
                     for a, b in zip(quad, np.roll(quad, -1, axis=0))
                     if min(a[1], b[1]) <= y < max(a[1], b[1])]
        assert len(crossings) == 2, 'The independent landmark must intersect this scanline'
        return min(crossings)

    separation = (left_edge(inner) - left_edge(corners)) * result['projection']['dpr']
    assert math.isclose(separation, result['expected_separation_px'], abs_tol=1e-6)
    begin, end = int(result['begin_ns']), int(result['end_ns'])
    assert 7.9 <= (end - begin) / 1e9 <= 10
    samples = []
    for index, frame in enumerate(result['frames']):
        assert frame['serial'] == index and index in times
        if index:
            assert times[index] > times[index - 1]
            assert int(frame['arrival_ns']) > int(result['frames'][index - 1]['arrival_ns'])
        if begin <= int(frame['arrival_ns']) <= end:
            red, blue, coarse = measure(raw[index * size:(index + 1) * size], width, height)
            samples.append({'time': times[index], 'red': coarse, 'blue': blue, 'continuous': red})
    assert len(samples) > 180
    duration = samples[-1]['time'] - samples[0]['time']
    assert 7.5 <= duration <= 10
    summary = {}
    for key in ['red', 'blue']:
        changes = [samples[0]['time']]
        last = samples[0][key]
        for sample in samples[1:]:
            if abs(sample[key] - last) >= .25:
                changes.append(sample['time'])
                last = sample[key]
        gaps = [b - a for a, b in zip(changes, changes[1:])]
        if changes[-1] < samples[-1]['time']:
            gaps.append(samples[-1]['time'] - changes[-1])
        assert gaps
        summary[key] = {
            'geometry_updates': len(changes) - 1,
            'updates_per_second': (len(changes) - 1) / duration,
            'gap_p95_ms': sorted(gaps)[math.ceil(len(gaps) * .95) - 1] * 1000,
            'gap_maximum_ms': max(gaps) * 1000,
            'intervals_over_33_334_ms': sum(gap > .033334 for gap in gaps),
            'intervals_over_100_ms': sum(gap > .1 for gap in gaps)
        }
    errors = [abs(sample['continuous'] - sample['blue'] - separation)
              for sample in samples if sample['continuous'] is not None]
    ambiguous = sum(sample['continuous'] is None for sample in samples)
    input_gaps = [b['at'] - a['at'] for a, b in zip(moves, moves[1:])]
    return {
        'complete': True, 'frames': len(samples), 'duration_seconds': duration,
        'trusted_moves': len(moves), 'image': summary['red'], 'controls': summary['blue'],
        'input_samples_per_second': len(moves) / ((end - begin) / 1e9),
        'input_gap_p95_ms': sorted(input_gaps)[math.ceil(len(input_gaps) * .95) - 1],
        'input_gap_maximum_ms': max(input_gaps),
        'ambiguous_landmark_frames': ambiguous,
        'maximum_image_control_error_px': max(errors) if errors else None,
        'alignment_within_one_pixel': ambiguous == 0 and bool(errors) and max(errors) <= 1,
        'both_at_least_30_updates_per_second': ambiguous == 0 and
            len(moves) / ((end - begin) / 1e9) >= 30 and
            all(value['updates_per_second'] >= 30 for value in summary.values()),
        'image_cadence_measure': 'Encoded red threshold crossings; ambiguous continuous landmark frames never qualify',
        'scope': 'Captured composited pixel changes during held input; no scanout or fitted input-latency claim'
    }


if __name__ == '__main__':
    directory = Path(sys.argv[1])
    result = audit(json.loads((directory / 'cadence.json').read_text()), (directory / 'cadence.bgra').read_bytes())
    (directory / 'cadence-audit.json').write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps(result))
    assert result['both_at_least_30_updates_per_second'], 'Native image and controls must each update at30+fps'
    assert result['alignment_within_one_pixel'], 'Independent native-image/control separation must stay within1px'
