"""Independent whole-cohort audit: pre-input native QPC to pixel-copy completion."""
import math

from desktop_latency import edge_positions


def audit(result, witnesses, capture_state):
    region = result['region']
    width, height = region['width'], region['height']
    assert 1 <= width <= 256 and 1 <= height <= 32
    size = width * height * 4
    frequency = int(result['frequency'])
    assert frequency > 0
    ready = capture_state['ready']
    assert capture_state['complete'] is True and capture_state['failure'] is None
    assert capture_state['exit'] == {'code': 0, 'signal': None}, 'Native capture must close normally'
    assert capture_state['stopped'] is True, 'Native stop acknowledgement required'
    assert ready['frequency'] == result['frequency'], 'Native clock frequency must match'
    assert ready['geometry']['window'] == result['geometry']['desktop_bounds']
    assert ready['geometry']['dpi'] == result['geometry']['window_dpi']
    assert ready['region'] == [region['left'], region['top'], region['left'] + width, region['top'] + height]
    args = capture_state['command'][1:]
    arguments = dict(zip(args[::2], args[1::2]))
    assert int(arguments['--hwnd']) == result['hwnd'] and int(arguments['--pid']) == result['pid']
    assert [int(arguments['--' + key]) for key in ['left', 'top', 'width', 'height']] == [
        region[key] for key in ['left', 'top', 'width', 'height']]
    samples, events = result['observations'], result['events']
    assert capture_state['observations'] == samples, 'Complete terminal cohort must match'
    assert len(samples) == len(events) == 30, 'Complete original cohort required'
    assert len(witnesses) == 60 * size, 'Every baseline/response pair required'
    values = []
    previous = 0
    for index, (sample, event) in enumerate(zip(samples, events)):
        assert all(type(sample[key]) is bool for key in [
            'before_protected_content_masked_out', 'response_protected_content_masked_out']), 'Native desktop mask notices required'
        direction = 1 if index % 2 == 0 else -1
        assert sample['index'] == index and sample['deltaY'] == direction * 120
        assert event['trusted'] is True and event['deltaY'] == sample['deltaY']
        begin, end, after = [int(sample[key]) for key in ['begin_qpc', 'response_qpc', 'after_qpc']]
        present = int(sample['desktop_update_qpc'])
        before, acquire = [int(sample[key]) for key in ['before_qpc', 'acquire_begin_qpc']]
        assert 0 < before <= begin and (begin - before) * 1000 <= 25 * frequency, 'Fresh baseline required'
        assert 0 < acquire <= end
        assert previous < begin < end <= after
        assert begin < present <= end, 'Response desktop image must be newer than the pre-input stamp'
        previous = after
        assert sample['before_serial'] < sample['response_serial']
        before = edge_positions(witnesses[index * 2 * size:(index * 2 + 1) * size], width, height)
        response = edge_positions(witnesses[(index * 2 + 1) * size:(index * 2 + 2) * size], width, height)
        assert before == sample['before'] and response == sample['response']
        assert all((response[key] - before[key]) * direction >= 2 for key in ['red', 'blue'])
        upper = (end - begin) * 1000 / frequency
        assert math.isclose(upper, sample['upper_bound_ms'], abs_tol=1e-9)
        values.append(upper)
    p95 = sorted(values)[math.ceil(len(values) * 0.95) - 1]
    assert math.isclose(p95, result['p95_upper_bound_ms'], abs_tol=1e-9)
    p99 = sorted(values)[math.ceil(len(values) * 0.99) - 1]
    assert math.isclose(p99, result['p99_upper_bound_ms'], abs_tol=1e-9)
    assert math.isclose(max(values), result['maximum_upper_bound_ms'], abs_tol=1e-9)
    assert result['within_50_ms'] == (p95 <= 50)
    return {'complete': True, 'actions': len(values), 'p95_upper_bound_ms': p95, 'p99_upper_bound_ms': p99,
            'masked_desktop_observations': sum(sample['before_protected_content_masked_out'] or
                                              sample['response_protected_content_masked_out'] for sample in samples),
            'within_50_ms': p95 <= 50,
            'scope': 'Native pre-input acknowledgement to pixel-copy completion; no scanout inference'}
