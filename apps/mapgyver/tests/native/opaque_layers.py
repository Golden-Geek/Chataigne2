"""Independent red/blue native witnesses and opaque owned-dialog compositing replay."""
import argparse
import json
from pathlib import Path
from PIL import Image


def red(pixel):
    r, g, b = pixel
    return r > 160 and g < 130 and b < 130


def blue(pixel):
    r, g, b = pixel
    return b > 160 and r < 130 and g < 130


def delta(a, b):
    return max(abs(x - y) for x, y in zip(a, b))


def verify(clear_red, clear_blue, red1, red2, blue1, red3, views, dialog):
    images = [image.convert('RGB') for image in [clear_red, clear_blue, red1, red2, blue1, red3]]
    assert len({image.size for image in images}) == 1, 'capture extents differ'
    pixels = [image.load() for image in images]
    assert len(views) == 2 and max(v[2]-v[0] for v in views) > min(v[2]-v[0] for v in views) * 1.25
    candidates = set()
    for left, top, right, bottom in views:
        witness = {(x, y) for y in range(max(0, top), min(images[0].height, bottom))
                   for x in range(max(0, left), min(images[0].width, right))
                   if red(pixels[0][x, y]) and blue(pixels[1][x, y])}
        assert len(witness) >= 64, 'both independent views need actual red and blue native pixels'
        candidates.update(witness)
    l, t, r, b = dialog
    covered = {p for p in candidates if l <= p[0] < r and t <= p[1] < b}
    uncovered = candidates - covered
    assert len(covered) >= 64, 'opaque dialog must actually overlap native colour'
    assert sum(red(pixels[2][p]) and blue(pixels[4][p]) and red(pixels[5][p]) for p in uncovered) >= 64, 'native content must keep changing outside the dialog'
    # Same-colour controls identify native dialog caret/OS animation only; never use the blue
    # held-out image to choose which pixels count toward opacity acceptance.
    stable = {p for p in covered if delta(pixels[2][p], pixels[3][p]) <= 2 and delta(pixels[2][p], pixels[5][p]) <= 2}
    assert len(stable) >= len(covered) * .99, 'same-colour dialog controls are not stable'
    error = max(delta(pixels[2][p], pixels[4][p]) for p in stable)
    assert error <= 2, f'native colour leaks through opaque dialog: RGB delta {error}'
    return {'independent_regions': 2, 'native_witness_pixels': len(candidates), 'covered_native_pixels': len(covered), 'stable_opaque_pixels': len(stable), 'maximum_rgb_delta': error, 'opaque_layer_verified': True}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('directory', type=Path)
    args = parser.parse_args()
    root = args.directory
    geometry = json.loads((root/'layers-ready.json').read_text(encoding='utf8'))
    metadata = [json.loads((root/name/'capture.json').read_text(encoding='utf8')) for name in ['clear-red', 'clear-blue', 'dialog-red1', 'dialog-red2', 'dialog-blue', 'dialog-red3']]
    for entry in metadata:
        for key in ['desktop_bounds', 'client_desktop_origin', 'window_dpi', 'monitor_bounds']:
            assert entry[key] == metadata[0][key], f'capture geometry changed: {key}'
    assert all(entry['foreground_dialog'] == metadata[2]['foreground_dialog'] for entry in metadata[2:])
    assert geometry['dpr'] * 96 == metadata[0]['window_dpi']
    ox = metadata[0]['client_desktop_origin'][0] - metadata[0]['desktop_bounds'][0]
    oy = metadata[0]['client_desktop_origin'][1] - metadata[0]['desktop_bounds'][1]
    scale = geometry['dpr']
    views = [(round(ox+v['rect']['left']*scale)+2, round(oy+v['rect']['top']*scale)+2,
              round(ox+v['rect']['right']*scale)-2, round(oy+v['rect']['bottom']*scale)-2) for v in geometry['views']]
    dl, dt, dr, db = metadata[2]['foreground_dialog_bounds']
    wl, wt, _, _ = metadata[0]['desktop_bounds']
    dialog = (dl-wl+20, dt-wt+64, dr-wl-20, db-wt-64)
    names = ['clear-red', 'clear-blue', 'dialog-red1', 'dialog-red2', 'dialog-blue', 'dialog-red3']
    result = verify(*(Image.open(root/name/'baseline.png') for name in names), views, dialog)
    (root/'opaque-result.json').write_text(json.dumps(result, indent=2)+'\n', encoding='utf8')
    print(json.dumps(result))


if __name__ == '__main__':
    main()
