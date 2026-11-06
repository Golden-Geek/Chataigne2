"""Retain bounded whole-window samples and independent landmark/overlay measurements."""
import argparse
import json
import time
from pathlib import Path

from landmark_edges import linear_red_edges
from visible_zoom import positions
from window_capture import visible_owned_window


def measure(image, roi, editor):
    _, edges = positions(image, roi, blue_inside_red=editor == 'source')
    coarse = [edges[0], edges[1]+1, edges[4], edges[5]+1]
    red = linear_red_edges(image, roi, coarse)
    blue = [edges[i]+.5 for i in [2, 3, 6, 7]]
    lo, hi = (.1, .9) if editor == 'source' else (.26, .74)
    inferred = []
    for start, end in [(red[0], red[1]), (red[2], red[3])]:
        inferred.extend(start+(value-lo)*(end-start)/(hi-lo) for value in [.2, .8])
    return {'red_edges': red, 'blue_centres': blue,
            'maximum_image_overlay_error_px': max(abs(a-b) for a, b in zip(inferred, blue))}


def match_native_controls(frames, native, capture, geometry):
    """Spatially match painted control boundaries; this does not identify a scanout frame."""
    scale = geometry['viewport']['dpr']
    ox, oy = [a-b for a, b in zip(capture['client_desktop_origin'], capture['desktop_bounds'][:2])]
    result = []
    for frame in frames:
        candidates = []
        for index, sample in enumerate(native):
            if not sample.get('hit'):
                continue
            rect = sample['hit']['rect']
            projected = [ox+(rect['x']+rect['width']*v)*scale for v in [.2, .8]]
            projected += [oy+(rect['y']+rect['height']*v)*scale for v in [.2, .8]]
            candidates.append((max(abs(a-b) for a, b in zip(frame['blue_centres'], projected)), index))
        assert candidates, 'Native accepted-frame rectangles required'
        error, index = min(candidates)
        result.append({'file': frame['file'], 'error_px': error, 'native_sample': index})
    assert result
    maximum = max(s['error_px'] for s in result)
    return {'samples': result, 'maximum_native_control_pixel_separation_px': maximum,
            'all_sampled_controls_spatially_matched': maximum <= 1, 'exact_temporal_join': False}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('hwnd', type=int)
    parser.add_argument('pid', type=int)
    parser.add_argument('directory', type=Path)
    parser.add_argument('editor', choices=['source', 'screen'])
    args = parser.parse_args()
    initial = args.directory/'motion-start'
    geometry = json.loads((initial/'geometry.json').read_text(encoding='utf8'))
    capture = json.loads((initial/'capture.json').read_text(encoding='utf8'))
    active, scale = geometry['active'], geometry['viewport']['dpr']
    ox, oy = [a-b for a, b in zip(capture['client_desktop_origin'], capture['desktop_bounds'][:2])]
    roi = (round(ox+active['x']*scale), round(oy+active['y']*scale),
           round(ox+(active['x']+active['width'])*scale), round(oy+(active['y']+active['height'])*scale))
    # Zooming out keeps the complete fixed image within the initial, authenticated viewport.
    output = args.directory/'motion'
    output.mkdir()
    samples = []
    print(json.dumps({'capture_ready': True}), flush=True)
    started = time.perf_counter()
    for index in range(24):
        before = time.perf_counter()
        image, bounds = visible_owned_window(args.hwnd, args.pid)
        assert list(bounds) == capture['desktop_bounds']
        assert image.size == tuple(capture['size'])
        image = image.convert('RGB')
        sample = measure(image, roi, args.editor)
        file = f'frame-{index:02}.png'
        image.save(output/file)
        samples.append({'file': file, 'begin_ms': (before-started)*1000,
                        'end_ms': (time.perf_counter()-started)*1000, **sample})
        time.sleep(.016)
    widths = {round(s['red_edges'][1]-s['red_edges'][0]) for s in samples}
    error = max(s['maximum_image_overlay_error_px'] for s in samples)
    result = {'editor': args.editor, 'roi': roi, 'samples': samples,
              'distinct_native_red_widths': sorted(widths), 'maximum_image_overlay_error_px': error,
              'all_sample_images_retained': True, 'input_latency_verified': False}
    (output/'frames.json').write_text(json.dumps(result, indent=2)+'\n', encoding='utf8')
    assert len(widths) >= 3, 'At least three actual native widths must be sampled'
    assert error <= 1, f'Independent moving landmark/overlay error {error}px'
    print(json.dumps({'samples': len(samples), 'maximum_image_overlay_error_px': error}))


if __name__ == '__main__':
    main()
