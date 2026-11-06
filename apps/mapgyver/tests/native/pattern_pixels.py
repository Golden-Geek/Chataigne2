"""Independent authored RGBA pattern and guarded native desktop color probes."""
import argparse
import json
from pathlib import Path

import numpy as np
from PIL import Image

from window_capture import client_geometry, visible_owned_window


TILES = [
    [0, 0, 0, 255], [255, 255, 255, 255], [255, 0, 0, 255], [0, 255, 0, 255],
    [0, 0, 255, 255], [255, 255, 0, 255], [0, 255, 255, 255], [255, 0, 255, 255],
    [32, 32, 32, 255], [64, 128, 192, 255], [192, 64, 128, 255], [128, 192, 64, 255],
    [255, 0, 0, 0], [0, 255, 0, 64], [0, 0, 255, 128], [64, 128, 192, 192],
]


def create(directory):
    directory.mkdir(parents=True, exist_ok=True)
    image = Image.new("RGBA", (800, 600))
    for index, rgba in enumerate(TILES):
        x, y = index % 4 * 200, index // 4 * 150
        image.paste(tuple(rgba), (x, y, x + 200, y + 150))
    image.save(directory / "rgba-pattern.png")
    manifest = {"extent": [800, 600], "columns": 4, "rows": 4, "rgba": TILES}
    (directory / "pattern.json").write_text(json.dumps(manifest, indent=2), encoding="utf8")
    return manifest


def linear(value):
    value /= 255
    return value / 12.92 if value <= 0.04045 else ((value + 0.055) / 1.055) ** 2.4


def encoded(value):
    value = value * 12.92 if value <= 0.0031308 else 1.055 * value ** (1 / 2.4) - 0.055
    return round(max(0, min(1, value)) * 255)


def expected_color(rgba, overlay_alpha=0):
    # PNG is straight-alpha sRGB; compositing and the authored overlay use linear light.
    base = [linear(channel) * rgba[3] / 255 for channel in rgba[:3]]
    fill = [linear(channel) for channel in [120, 170, 250]]
    return [encoded(color * (1 - overlay_alpha) + cover * overlay_alpha)
            for color, cover in zip(base, fill)]


def homography(quad):
    # Solve the independent four-point projective correspondence, without renderer helpers.
    rows, values = [], []
    for (x, y), (a, b) in zip(((0, 0), (1, 0), (1, 1), (0, 1)), quad):
        rows.extend(([x, y, 1, 0, 0, 0, -a * x, -a * y],
                     [0, 0, 0, x, y, 1, -b * x, -b * y]))
        values.extend((a, b))
    h = np.linalg.solve(np.array(rows), np.array(values))
    return np.append(h, 1).reshape(3, 3)


def project(quad, u, v):
    h = homography(quad).reshape(-1)
    denominator = h[6] * u + h[7] * v + 1
    return [(h[0] * u + h[1] * v + h[2]) / denominator,
            (h[3] * u + h[4] * v + h[5]) / denominator]


def analyze(image, metadata, capture):
    assert metadata["editor"] in ("screen", "source")
    scale = metadata["viewport"]["dpr"]
    assert abs(capture["window_dpi"] / 96 - scale) < 1e-6
    assert capture["capture_mode"] == "visible owned foreground window"
    image = image.convert("RGB")
    active, crop = metadata["active"], metadata["crop"]
    offset = [a - b for a, b in zip(capture["client_desktop_origin"], capture["desktop_bounds"][:2])]
    probes = []
    for row in range(4):
        for column in range(4):
            u, v = (column + 0.5) / 4, (row + 0.5) / 4
            if metadata["editor"] == "screen":
                source = [crop["x"] + u * crop["width"], crop["y"] + v * crop["height"]]
                position = project(metadata["quad"], u, v)
                overlay = 24 / 255 if metadata["selected"] else 0
            else:
                source, position = [u, v], [u, v]
                inside = (crop["x"] < u < crop["x"] + crop["width"] and
                          crop["y"] < v < crop["y"] + crop["height"])
                overlay = 24 / 255 if inside else 0
            index = min(3, int(source[1] * 4)) * 4 + min(3, int(source[0] * 4))
            rgba = metadata["pattern"]["rgba"][index]
            expected = expected_color(rgba, overlay)
            xy = [round(offset[i] + (active[key] + position[i] * active[size]) * scale)
                  for i, (key, size) in enumerate((("x", "width"), ("y", "height")))]
            x, y = xy
            assert 1 <= x < image.width - 1 and 1 <= y < image.height - 1
            pixels = list(image.crop((x - 1, y - 1, x + 2, y + 2)).get_flattened_data())
            error = max(abs(a - b) for pixel in pixels for a, b in zip(pixel, expected))
            probes.append({"tile": [column, row], "source_uv": source, "projected_uv": position,
                           "xy": xy, "rgba": rgba, "overlay_alpha": overlay, "expected": expected,
                           "actual": [list(pixel) for pixel in sorted(set(pixels))], "maximum_error": error})
    maximum = max(probe["maximum_error"] for probe in probes)
    return {"passed": maximum <= 2, "maximum_rgb_error": maximum, "probes": probes,
            "pixel_count": len(probes) * 9, "tolerance": 2,
            "scope": "Settled flat tile interiors at independent projected points; no edge-alignment or latency claim"}


def capture(directory, hwnd, pid):
    metadata = json.loads((directory / "projection.json").read_text(encoding="utf8"))
    geometry = client_geometry(hwnd)
    image, bounds = visible_owned_window(hwnd, pid)
    assert geometry == client_geometry(hwnd)
    image.save(directory / "owned-window.png")
    captured = {"capture_mode": "visible owned foreground window", "desktop_bounds": list(bounds), **geometry}
    (directory / "capture.json").write_text(json.dumps(captured, indent=2), encoding="utf8")
    result = analyze(image, metadata, captured)
    (directory / "pixels.json").write_text(json.dumps(result, indent=2), encoding="utf8")
    return {"passed": result["passed"], "maximum_rgb_error": result["maximum_rgb_error"],
            "pixel_count": result["pixel_count"]}


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("directory", type=Path)
    parser.add_argument("--create", action="store_true")
    parser.add_argument("--hwnd", type=int)
    parser.add_argument("--pid", type=int)
    args = parser.parse_args()
    print(json.dumps(create(args.directory) if args.create else capture(args.directory, args.hwnd, args.pid)))
