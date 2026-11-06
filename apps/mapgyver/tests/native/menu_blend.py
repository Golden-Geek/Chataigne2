"""Compare actual native/WebView menu composition with an independent RGBA layer."""
import argparse
import json
from pathlib import Path
from PIL import Image

from opaque_layers import red, blue, delta


def source_over(background, foreground):
    alpha = foreground[3] / 255
    return tuple(
        round(foreground[c] * alpha + background[c] * (1 - alpha))
        for c in range(3)
    )


def replace_layer(clear, base, layer):
    base_alpha, alpha = base[3] / 255, layer[3] / 255
    assert base_alpha < 1, 'Native reference requires a nonopaque clear WebView layer'
    # Recover the native contribution from the clear physical composite. Compositing the
    # full menu WebView layer over that composite would apply selection paint twice.
    return tuple(
        round(layer[c] * alpha + (clear[c] - base[c] * base_alpha)
              * (1 - alpha) / (1 - base_alpha))
        for c in range(3)
    )


def verify(clears, frames, layer, candidates, offset, base_layer=None):
    cp = [image.convert('RGB').load() for image in clears]
    fp = [image.convert('RGB').load() for image in frames]
    lp = layer.convert('RGBA').load()
    bp = base_layer.convert('RGBA').load() if base_layer else None
    ox, oy = offset
    selected = []
    for point in candidates:
        x, y = point
        lx, ly = x-ox, y-oy
        if not (1 <= lx < layer.width-1 and 1 <= ly < layer.height-1):
            continue
        pixel = lp[lx, ly]
        base = bp[lx, ly] if bp else (0, 0, 0, 0)
        if base[3] >= 240:
            continue
        if not 32 <= pixel[3] <= 240:
            continue
        # Pick flat layer interiors independently of held-out blue; avoid text/edge raster phases.
        if any(max(abs(a-b) for a, b in zip(pixel, lp[lx+dx, ly+dy])) > 1
               for dx, dy in [(-1, 0), (1, 0), (0, -1), (0, 1)]):
            continue
        if bp and any(delta(base, bp[lx+dx, ly+dy]) > 1
                      or abs(base[3]-bp[lx+dx, ly+dy][3]) > 1
                      for dx, dy in [(-1, 0), (1, 0), (0, -1), (0, 1)]):
            continue
        selected.append(point)
    assert len(selected) >= 64, 'Actual translucent menu pixels over native colour required'
    stable = [
        p for p in selected
        if delta(fp[0][p], fp[1][p]) <= 2 and delta(fp[0][p], fp[3][p]) <= 2
    ]
    assert len(stable) >= .99 * len(selected), 'Same-colour menu controls are not stable'
    errors = []
    for clear, frame in [(cp[0], fp[0]), (cp[1], fp[2])]:
        differences = []
        for p in stable:
            local = (p[0] - ox, p[1] - oy)
            base = bp[local] if bp else (0, 0, 0, 0)
            differences.append(delta(frame[p], replace_layer(clear[p], base, lp[local])))
        errors.append(max(differences))
    assert max(errors) <= 2, f'WebView/native source-over reference RGB error {errors}'
    return {'translucent_native_witness_pixels': len(selected), 'stable_pixels': len(stable),
            'red_reference_max_rgb_error': errors[0], 'held_out_blue_reference_max_rgb_error': errors[1]}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('directory', type=Path)
    root = parser.parse_args().directory
    def load(name):
        return json.loads((root / name).read_text(encoding='utf8'))

    names = ['clear-red', 'clear-blue', 'menu-red1', 'menu-red2', 'menu-blue', 'menu-red3']
    metadata = [load(name+'/capture.json') for name in names]
    for entry in metadata:
        for key in ['desktop_bounds', 'client_desktop_origin', 'window_dpi', 'monitor_bounds']:
            assert entry[key] == metadata[0][key], f'Capture geometry changed: {key}'
    scene, menu = load('layers-ready.json'), load('menu-ready.json')
    assert scene['dpr'] == menu['dpr'] == metadata[0]['window_dpi']/96
    offset = tuple(
        a - b for a, b in zip(
            metadata[0]['client_desktop_origin'], metadata[0]['desktop_bounds'][:2]
        )
    )
    scale = scene['dpr']
    images = [Image.open(root/name/'baseline.png').convert('RGB') for name in names]
    assert len({im.size for im in images}) == 1
    pixels = [im.load() for im in images]
    candidates, witnesses, view_counts = [], [], []
    m = menu['rect']
    ml, mt = offset[0]+m['left']*scale+12, offset[1]+m['top']*scale+12
    mr, mb = offset[0]+m['right']*scale-12, offset[1]+m['bottom']*scale-12
    for view in scene['views']:
        r = view['rect']
        left = round(offset[0] + r['left'] * scale) + 12
        top = round(offset[1] + r['top'] * scale) + 12
        right = round(offset[0] + r['right'] * scale) - 12
        bottom = round(offset[1] + r['bottom'] * scale) - 12
        points = [
            (x, y) for y in range(top, bottom) for x in range(left, right)
            if red(pixels[0][x, y]) and blue(pixels[1][x, y])
        ]
        assert len(points) >= 64, 'Both independent native views require colour witnesses'
        witnesses.extend(points)
        covered = [p for p in points if ml <= p[0] < mr and mt <= p[1] < mb]
        candidates.extend(covered)
        view_counts.append({'view': view['label'], 'native_colour_witnesses': len(points),
                            'inside_menu_interior': len(covered)})
    uncovered = [p for p in witnesses if not (ml <= p[0] < mr and mt <= p[1] < mb)]
    changed = sum(red(pixels[2][p]) and blue(pixels[4][p]) and red(pixels[5][p]) for p in uncovered)
    assert changed >= 64
    layer = Image.open(root/'menu-layer.png')
    base_layer = Image.open(root/'clear-layer.png')
    assert layer.mode == 'RGBA', 'Independent transparent WebView layer required'
    assert base_layer.mode == 'RGBA' and base_layer.size == layer.size
    result = verify(images[:2], images[2:], layer, candidates, offset, base_layer)
    result['native_views'] = view_counts
    result['changing_uncovered_native_pixels'] = changed
    (root/'menu-blend-result.json').write_text(json.dumps(result, indent=2)+'\n', encoding='utf8')
    print(json.dumps(result))


if __name__ == '__main__':
    main()
