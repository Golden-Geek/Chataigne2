"""Independent static circle/diagonal coverage in paired actual-app calibration captures."""

import argparse
import json
import math
from pathlib import Path
from PIL import Image


def encode(value):
    return round(
        255 * (12.92 * value if value <= 0.0031308 else 1.055 * value ** (1 / 2.4) - 0.055)
    )


def quantized(low, high):
    return (
        max(0, math.ceil(low * 255 - 0.6)) / 255,
        min(255, math.floor(high * 255 + 0.6)) / 255,
    )


def geometry(width, height, x, y):
    diameter = min(width, height)
    cell = max(diameter / 9, max(width, height) / 32, 1)
    stroke = min(1.5, max(0.5, cell / 160))
    radius_distance = abs(math.hypot(x - width / 2, y - height / 2) - diameter / 2)
    diagonal_distance = min(
        abs(height * x - width * y), abs(height * x + width * y - width * height)
    ) / math.hypot(width, height)
    return cell, stroke, radius_distance, diagonal_distance


def safe(width, height, x, y):
    cell, stroke, circle, diagonal = geometry(width, height, x, y)
    diameter = min(width, height)
    # Exclude dynamic grid, palette bands, labels, logo and all centered readouts.
    if min(x % cell, cell - x % cell, y % cell, cell - y % cell) < stroke + 1:
        return False
    if (
        cell - 1 <= y <= 2 * cell + 1
        or height - 2 * cell - 1 <= y <= height - cell + 1
    ):
        return False
    if x % cell < cell * 0.4 and y % cell < cell * 0.22:
        return False
    if abs(x - width / 2) < diameter * 0.45 and abs(y - height / 2) < diameter * 0.3:
        return False
    if abs(x - width / 2) > diameter * 0.49 or abs(y - height / 2) > diameter * 0.49:
        return False
    # A sub-millipixel boundary has ambiguous f32 coverage. Do not choose such landmarks.
    return min(abs(circle - stroke), abs(diagonal - stroke)) > 0.002


def expected(width, height, x, y, samples):
    values = []
    coverage = 0
    for sy in range(samples):
        for sx in range(samples):
            px, py = x + (sx + 0.5) / samples, y + (sy + 0.5) / samples
            cell, stroke, circle, diagonal = geometry(width, height, px, py)
            ink = min(circle, diagonal) <= stroke
            coverage += ink
            base = 0.2 if (math.floor(px / cell) + math.floor(py / cell)) % 2 else 0.1
            values.append(quantized(0.65 if ink else base, 0.65 if ink else base))
    low = sum(v[0] for v in values) / len(values)
    high = sum(v[1] for v in values) / len(values)
    if samples == 2:
        low, high = quantized(low, high)
    return encode(low), encode(high), 0 < coverage < samples * samples


def landmarks(width, height, kind):
    diameter = min(width, height)
    candidates = set()
    if kind == "circle":
        for i in range(1440):
            angle = i * math.tau / 1440
            x = width / 2 + diameter / 2 * math.cos(angle)
            y = height / 2 + diameter / 2 * math.sin(angle)
            for dy in range(-2, 3):
                for dx in range(-2, 3):
                    candidates.add((math.floor(x) + dx, math.floor(y) + dy))
    else:
        for x in range(width):
            for y in [
                height * (x + 0.5) / width, height * (1 - (x + 0.5) / width)
            ]:
                for dy in range(-2, 3):
                    candidates.add((x, math.floor(y) + dy))
    selected = []
    for x, y in sorted(candidates):
        if not (0 <= x < width and 0 <= y < height):
            continue
        positions = [(x + 0.5, y + 0.5)] + [
            (x + sx, y + sy) for sx in [0.25, 0.75] for sy in [0.25, 0.75]
        ]
        if all(safe(width, height, px, py) for px, py in positions):
            selected.append((x, y))
    assert len(selected) >= 512, (width, height, kind, len(selected))
    return [selected[i * len(selected) // 512] for i in range(512)]


def check(raw_directory, smooth_directory, label):
    images = [
        Image.open(directory / label / "pixels.png").convert("RGBA")
        for directory in [raw_directory, smooth_directory]
    ]
    assert images[0].size == images[1].size
    width, height = images[0].size
    results = []
    for kind in ["circle", "diagonal"]:
        points = landmarks(width, height, kind)
        for image, samples in zip(images, [1, 2]):
            maximum, fractional = 0, 0
            for x, y in points:
                low, high, partial = expected(width, height, x, y, samples)
                pixel = image.getpixel((x, y))
                error = max(max(0, low - c, c - high) for c in pixel[:3])
                maximum = max(maximum, error)
                assert error <= 2 and pixel[3] == 255, (
                    label, kind, samples, x, y, pixel, low, high, error
                )
                fractional += partial
            if samples == 2:
                assert fractional >= 32, (label, kind, fractional)
            results.append({
                "mode": "Raw" if samples == 1 else "Smooth2x",
                "feature": kind,
                "landmarks": len(points),
                "fractional_coverage": fractional,
                "maximum_error_outside_legal_interval": maximum,
            })
    return {"label": label, "extent": [width, height], "checks": results}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("raw", type=Path)
    parser.add_argument("smooth", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    report = {
        "passed": True,
        "checks": [check(args.raw, args.smooth, label) for label in ["landscape", "4k"]],
        "scope": (
            "512 deterministic static landmarks per feature/mode/size, independent circle "
            "and point-line distance, unchanged 2/255 legal-UNORM interval tolerance; "
            "not complete-image or timing qualification"
        ),
    }
    args.output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf8")
    print(json.dumps(report))


if __name__ == "__main__":
    main()
