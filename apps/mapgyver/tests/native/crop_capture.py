"""Replay native viewport capture, exact gesture history and independent output pixels."""
import argparse
import hashlib
import json
from pathlib import Path

from PIL import Image, ImageChops


def verify_pointer_capture(journal):
    assert not journal['overflow']
    events = [e for e in journal['events'] if e['trusted']]
    bounds = journal['bounds']
    outside = lambda e: not (bounds['left'] <= e['x'] <= bounds['right'] and bounds['top'] <= e['y'] <= bounds['bottom'])
    requirements = [
        lambda e: e['type'] == 'pointerdown' and not outside(e),
        lambda e: e['type'] == 'gotpointercapture' and e['captured'],
        lambda e: e['type'] == 'pointermove' and e['captured'] and outside(e),
        lambda e: e['type'] == 'pointerup' and e['captured'] and outside(e),
        lambda e: e['type'] == 'lostpointercapture' and not e['captured'],
    ]
    matched = []
    for requirement in requirements:
        index = next((i for i, e in enumerate(events) if (not matched or i > matched[-1]) and requirement(e)), None)
        assert index is not None, 'trusted outside-view capture/release sequence missing'
        matched.append(index)
    assert len({events[i]['id'] for i in matched}) == 1
    return [events[i] for i in matched]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('directory', type=Path)
    root = parser.parse_args().directory
    load = lambda name: json.loads((root/name).read_text(encoding='utf8'))
    ready = load('ready.json')
    initial = load('arm-native.json')
    dragged = load('native-dragged.json')
    events = verify_pointer_capture(dragged['nativeJournal'])
    first = initial['handles'][0]['rect']
    assert first['left'] <= events[0]['x'] <= first['right'] and first['top'] <= events[0]['y'] <= first['bottom']
    baseline = initial['surface']['definition']
    changed = dragged['surface']['definition']
    assert changed['crop']['x'] == changed['crop']['y'] == 0
    for axis, extent in [('x', 'width'), ('y', 'height')]:
        assert abs(changed['crop'][extent] - baseline['crop'][axis] - baseline['crop'][extent]) <= 1e-6
    assert {k: v for k, v in changed.items() if k != 'crop'} == {k: v for k, v in baseline.items() if k != 'crop'}
    assert load('native-undo.json')['surface']['definition'] == baseline
    assert load('final-undo.json')['surface']['definition'] == baseline
    assert load('redo.json')['surface']['definition'] == changed
    physical = [load(stage+'/alignment.json') for stage in ['before', 'restored']]
    assert all(p['settled_alignment_verified'] for p in physical)
    maximum = max(p[key] for p in physical for key in ['image_to_projection_max_px', 'overlay_to_projection_max_px', 'image_to_overlay_max_px'])
    assert maximum <= 1
    source_width, source_height = ready['landmark']['extent']
    rx, ry, rw, rh = ready['landmark']['internalRedRect']
    sdk = []
    for stage, definition in [('sdk-before', baseline), ('sdk-dragged', changed), ('sdk-after', baseline)]:
        file = root/stage/'pixels.png'
        actual = Image.open(file).convert('RGB')
        assert actual.size == (800, 600)
        crop = definition['crop']
        q = definition['quad']['corners']
        assert q[0]['x'] == q[3]['x'] and q[1]['x'] == q[2]['x'] and q[0]['y'] == q[1]['y'] and q[2]['y'] == q[3]['y']
        rect = []
        for axis, extent, low, high, size in [('x', 'width', rx/source_width, (rx+rw)/source_width, 800), ('y', 'height', ry/source_height, (ry+rh)/source_height, 600)]:
            start, end = q[0][axis], q[2][axis]
            for value in [low, high]:
                local = max(0, min(1, (value-crop[axis])/crop[extent]))
                rect.append(round((start+(end-start)*local)*size))
        bounds = (rect[0], rect[2], rect[1], rect[3])
        expected = Image.new('RGB', actual.size, 'black')
        expected.paste((255, 0, 0), bounds)
        error = max(high for _, high in ImageChops.difference(actual, expected).getextrema())
        assert error <= 2, f'{stage}: independent crop reference RGB error {error}'
        sdk.append({'stage': stage, 'expected_red_rect': bounds, 'maximum_rgb_difference': error, 'sha256': hashlib.sha256(file.read_bytes()).hexdigest()})
    result = {'pid': ready['pid'], 'adapter': ready['adapter'], 'trusted_capture_events': events, 'crop': changed['crop'], 'exact_one_step_undo_redo': True, 'maximum_settled_alignment_px': maximum, 'sdk': sdk, 'input_latency_verified': False}
    (root/'crop-capture-result.json').write_text(json.dumps(result, indent=2)+'\n', encoding='utf8')
    print(json.dumps(result))


if __name__ == '__main__':
    main()
