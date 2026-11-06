"""Dense, geometry-masked replay of the retained physical RGBA reference captures."""
import argparse
from io import BytesIO
import json
from pathlib import Path
import zipfile

import numpy as np
from PIL import Image

from pattern_pixels import expected_color, homography, project

ARCHIVE = Path('docs/progress/evidence/mapgyver-m05-preview-precision-2026-11-06/precision-captures.zip')
UNIT = [[0, 0], [1, 0], [1, 1], [0, 1]]


def line_distance(x, y, a, b):
    dx, dy = b[0] - a[0], b[1] - a[1]
    length = np.hypot(dx, dy)
    assert length > 0, 'Degenerate reference edge'
    return np.abs(dx * (y - a[1]) - dy * (x - a[0])) / length


def reference(metadata, capture, size):
    scale = metadata['viewport']['dpr']
    assert scale == capture['window_dpi'] / 96, 'Actual window DPI must match projection'
    assert capture['capture_mode'] == 'visible owned foreground window'
    bounds = capture['desktop_bounds']
    assert size == (bounds[2] - bounds[0], bounds[3] - bounds[1])
    active = metadata['active']
    origin = np.array(capture['client_desktop_origin']) - bounds[:2]
    origin = origin + np.array([active['x'], active['y']]) * scale
    extent = np.array([active['width'], active['height']]) * scale
    x, y = np.meshgrid(np.arange(size[0]) + 0.5, np.arange(size[1]) + 0.5)
    px, py = (x - origin[0]) / extent[0], (y - origin[1]) / extent[1]
    margin = 6 * scale
    mask = ((x > origin[0] + margin) & (x < origin[0] + extent[0] - margin)
            & (y > origin[1] + margin) & (y < origin[1] + extent[1] - margin))
    crop = metadata['crop']
    screen = metadata['editor'] == 'screen'
    assert screen or metadata['editor'] == 'source'
    quad = metadata['quad'] if screen else UNIT
    inverse = np.linalg.inv(homography(quad))
    denominator = inverse[2, 0] * px + inverse[2, 1] * py + inverse[2, 2]
    valid = np.abs(denominator) > 1e-12
    denominator = np.where(valid, denominator, 1)
    u = (inverse[0, 0] * px + inverse[0, 1] * py + inverse[0, 2]) / denominator
    v = (inverse[1, 0] * px + inverse[1, 1] * py + inverse[1, 2]) / denominator
    inside = valid & (u > 0) & (u < 1) & (v > 0) & (v < 1)
    mask &= valid

    def physical(point):
        return origin + np.array(point) * extent

    polygon = [physical(point) for point in quad]
    for a, b in zip(polygon, polygon[1:] + polygon[:1]):
        mask &= line_distance(x, y, a, b) > margin
    if screen:
        source_u, source_v = crop['x'] + u * crop['width'], crop['y'] + v * crop['height']
        overlay = inside & metadata['selected']
    else:
        source_u, source_v = u, v
        overlay = ((u > crop['x']) & (u < crop['x'] + crop['width'])
                   & (v > crop['y']) & (v < crop['y'] + crop['height']))
        corners = [[crop['x'], crop['y']], [crop['x'] + crop['width'], crop['y']],
                   [crop['x'] + crop['width'], crop['y'] + crop['height']],
                   [crop['x'], crop['y'] + crop['height']]]
        for a, b in zip(corners, corners[1:] + corners[:1]):
            mask &= line_distance(x, y, physical(a), physical(b)) > margin
    for boundary in [0.25, 0.5, 0.75]:
        for axis in range(2):
            start = crop['x'] if axis == 0 else crop['y']
            width = crop['width'] if axis == 0 else crop['height']
            value = (boundary - start) / width if screen else boundary
            if not 0 < value < 1:
                continue
            ends = [[value, 0], [value, 1]] if axis == 0 else [[0, value], [1, value]]
            a, b = [physical(project(quad, *point)) for point in ends]
            mask &= ~inside | (line_distance(x, y, a, b) > 2)
    indexes = (np.clip(np.floor(source_v * 4), 0, 3).astype(int) * 4
               + np.clip(np.floor(source_u * 4), 0, 3).astype(int))
    rgba = np.array(metadata['pattern']['rgba'])
    assert rgba.shape == (16, 4)
    plain = np.array([expected_color(value) for value in rgba])
    selected = np.array([expected_color(value, 24 / 255) for value in rgba])
    expected = np.where(overlay[..., None], selected[indexes], plain[indexes])
    expected = np.where(inside[..., None], expected, 0)
    alpha = inside & (rgba[indexes, 3] < 255)
    return expected, mask, inside, indexes, alpha


def analyze(image, metadata, capture):
    actual = np.array(image.convert('RGB')).astype(int)
    expected, mask, inside, indexes, alpha = reference(metadata, capture, image.size)
    errors = np.max(np.abs(actual - expected), axis=2)
    counts = [int(np.count_nonzero(mask & inside & (indexes == tile))) for tile in range(16)]
    assert min(counts) >= 9, 'Every visible reference tile requires native coverage'
    assert np.count_nonzero(mask & alpha) >= 9, 'Cropped alpha strips must be checked'
    bad = np.argwhere(mask & (errors > 2))
    return {
        'passed': len(bad) == 0,
        'maximum_rgb_error': int(errors[mask].max()),
        'pixel_count': int(np.count_nonzero(mask)),
        'tile_pixels': counts,
        'alpha_pixels': int(np.count_nonzero(mask & alpha)),
        'background_pixels': int(np.count_nonzero(mask & ~inside)),
        'bad_pixels': len(bad),
        'first_bad': [
            {'xy': [int(x), int(y)], 'actual': actual[y, x].tolist(),
             'expected': expected[y, x].tolist(), 'tile': int(indexes[y, x])}
            for y, x in bad[:8]
        ],
    }


def replay(archive):
    results = []
    with zipfile.ZipFile(archive) as source:
        for name in sorted(source.namelist()):
            if not name.endswith('/projection.json'):
                continue
            stage = name.removesuffix('/projection.json')
            metadata = json.loads(source.read(name))
            capture = json.loads(source.read(stage + '/capture.json'))
            with Image.open(BytesIO(source.read(stage + '/owned-window.png'))) as image:
                results.append({'stage': stage, **analyze(image, metadata, capture)})
    assert len(results) == 18
    return {'passed': all(row['passed'] for row in results), 'cases': results,
            'pixel_count': sum(row['pixel_count'] for row in results)}


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--archive', type=Path, default=ARCHIVE)
    parser.add_argument('--output', type=Path)
    args = parser.parse_args()
    result = replay(args.archive)
    if args.output:
        args.output.write_text(json.dumps(result, indent=2) + '\n', encoding='utf8')
    print(json.dumps(result))
    raise SystemExit(0 if result['passed'] else 2)
