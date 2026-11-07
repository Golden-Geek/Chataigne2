"""Check a pending control edge using foreground pixels and independent DOM projection."""
import json
import math
import sys
from pathlib import Path
from PIL import Image


def verify(folder):
    capture = json.loads((folder / 'capture.json').read_text(encoding='utf-8'))
    geometry = json.loads((folder / 'geometry.json').read_text(encoding='utf-8'))
    assert capture['capture_mode'] == 'visible owned foreground window'
    scale = geometry['dpr']
    color = geometry['color']
    assert len(color) == 3 and all(type(value) is int and 0 <= value <= 255 for value in color)
    assert max(color) - min(color) >= 60, 'A distinct chromatic control edge is required'
    assert abs(capture['window_dpi'] / 96 - scale) < 1e-6
    image = Image.open(folder / 'baseline.png').convert('RGB')
    assert list(image.size) == capture['size']
    offset = [a - b for a, b in zip(capture['client_desktop_origin'], capture['desktop_bounds'][:2])]

    def project(point):
        return [offset[i] + point[i] * scale for i in range(2)]

    expected, stale = project(geometry['expected']), project(geometry['stale'])

    def distance(point):
        x, y = point
        pixels = [(a + .5, b + .5) for a in range(math.floor(x) - 5, math.ceil(x) + 6)
                  for b in range(math.floor(y) - 5, math.ceil(y) + 6)
                  if 0 <= a < image.width and 0 <= b < image.height
                  and math.dist(image.getpixel((a, b)), color) <= 40]
        return min((math.dist(point, p) for p in pixels), default=None)

    current_distance, stale_distance = distance(expected), distance(stale)
    result = {'expected_physical': expected, 'stale_physical': stale,
              'current_edge_error_px': current_distance, 'stale_edge_distance_px': stale_distance,
              'dpr': scale, 'window_dpi': capture['window_dpi'], 'color': color}
    (folder / 'pending-alignment.json').write_text(json.dumps(result, indent=2) + '\n', encoding='utf-8')
    assert current_distance is not None and current_distance <= 1, result
    assert stale_distance is None or stale_distance > 2, result
    return result


if __name__ == '__main__':
    print(json.dumps(verify(Path(sys.argv[1]))))
