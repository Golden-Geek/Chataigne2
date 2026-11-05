"""M11/10 full-image checks against the font ROM and independent layout/palette expectations."""

import argparse
from datetime import datetime, timedelta
import json
import math
from pathlib import Path
import re
from PIL import Image


def homography():
    points = [(0.12, 0.14), (0.84, 0.08), (0.91, 0.86), (0.06, 0.94)]
    matrix = []
    for (x, y), (u, v) in zip([(0, 0), (1, 0), (1, 1), (0, 1)], points):
        matrix.extend(
            [
                [x, y, 1, 0, 0, 0, -u * x, -u * y, u],
                [0, 0, 0, x, y, 1, -v * x, -v * y, v],
            ]
        )
    for column in range(8):
        pivot = max(range(column, 8), key=lambda row: abs(matrix[row][column]))
        matrix[column], matrix[pivot] = matrix[pivot], matrix[column]
        divisor = matrix[column][column]
        matrix[column] = [value / divisor for value in matrix[column]]
        for row in range(8):
            if row != column:
                factor = matrix[row][column]
                matrix[row] = [
                    a - factor * b for a, b in zip(matrix[row], matrix[column])
                ]
    return [matrix[row][8] for row in range(8)] + [1]


def srgb(value):
    return round(
        255
        * (12.92 * value if value <= 0.0031308 else 1.055 * value ** (1 / 2.4) - 0.055)
    )


def inspect(directory, observation, font):
    label = observation["label"]
    image = Image.open(directory / label / "pixels.png").convert("RGBA")
    width, height = image.size
    assert [width, height] == [
        observation["pixels"]["width"],
        observation["pixels"]["height"],
    ]
    if label == "restored-source":
        assert image.getextrema() == ((255, 255), (0, 0), (0, 0), (255, 255))
        return {"label": label, "solid_red_every_pixel": True}
    diameter = min(width, height)
    transform = homography() if label == "warped" else None

    def point(x, y):
        if transform:
            x, y = x / width, y / height
            h = transform
            divisor = h[6] * x + h[7] * y + 1
            x, y = (h[0] * x + h[1] * y + h[2]) / divisor * width, (
                h[3] * x + h[4] * y + h[5]
            ) / divisor * height
        return image.getpixel(
            (
                min(width - 1, max(0, math.floor(x))),
                min(height - 1, max(0, math.floor(y))),
            )
        )

    def line(length, top, alphabet, start=None, unit=None, ink=None):
        unit = unit or diameter * 0.0025
        start = (width - length * 9 * unit) / 2 if start is None else start
        ink = ink or (lambda pixel: min(pixel[:3]) >= 240)
        text = ""
        errors = 0
        for index in range(length):
            observed = [
                ink(point(start + (index * 9 + x + 0.5) * unit, top + (y + 0.5) * unit))
                for y in range(8)
                for x in range(8)
            ]
            candidates = [
                (
                    sum(
                        bit != bool((font[ord(char)][y] >> x) & 1)
                        for (y, x), bit in zip(
                            ((y, x) for y in range(8) for x in range(8)), observed
                        )
                    ),
                    char,
                )
                for char in alphabet
            ]
            error, char = min(candidates)
            text += char
            errors += error
        return text, errors

    top = (height - diameter * 0.24) / 2 - diameter * 0.05
    clock, errors = line(12, top, "0123456789:.")
    assert errors == 0 and re.fullmatch(r"\d\d:\d\d:\d\d\.\d\d\d", clock), (
        label,
        clock,
        errors,
    )
    before, after = observation["before"] / 1000, observation["after"] / 1000
    stamp = datetime.fromtimestamp(before).replace(
        hour=int(clock[:2]),
        minute=int(clock[3:5]),
        second=int(clock[6:8]),
        microsecond=int(clock[9:]) * 1000,
    )
    candidates = [(stamp + timedelta(days=delta)).timestamp() for delta in [-1, 0, 1]]
    assert any(before - 0.5 <= candidate <= after + 0.1 for candidate in candidates), (
        label,
        clock,
        before,
        after,
    )
    below = (height + diameter * 0.24) / 2 + diameter * 0.025
    expected = f"{width} x {height}"
    resolution, errors = line(len(expected), below, "0123456789 x")
    assert errors == 0 and resolution == expected, (label, resolution, errors)
    rates = []
    for length in range(7, 12):
        text, errors = line(length, below + diameter * 0.035, "0123456789. FPS-")
        if errors == 0 and re.fullmatch(r"\d+\.\d FPS", text):
            rates.append(float(text.split()[0]))
    if not rates:
        unavailable, errors = line(8, below + diameter * 0.035, "0123456789. FPS-")
        assert errors == 0 and unavailable == "--.- FPS", (label, unavailable, errors)
        assert label in {"portrait", "square", "4k", "resize", "live-a"}, label
        rate = None
    else:
        assert len(rates) == 1 and rates[0] > 0, (label, rates)
        rate = rates[0]
        assert abs(rate - observation["runtime"]["submitted_fps"]) < 5, (
            label,
            rate,
            observation["runtime"]["submitted_fps"],
        )
    cell = max(diameter / 9, max(width, height) / 32, 1)
    # B4 avoids palette rows, circle and diagonals at the five unwarped aspects.
    text, errors = line(
        2,
        cell * 3.06,
        "B4",
        start=cell * 1.06,
        unit=cell * 0.12 / 8,
        ink=lambda pixel: abs(pixel[0] - srgb(0.5 * 0.55 + 0.1 * 0.45)) <= 2
        and max(pixel[:3]) - min(pixel[:3]) <= 2,
    )
    if not transform:
        assert errors == 0 and text == "B4", (label, text, errors)
    for index in range(8):
        x = (index + 0.4) * width / 8
        expected_grey = srgb(index / 7)
        actual = point(x, cell * 1.5)
        assert all(abs(value - expected_grey) <= 2 for value in actual[:3]), (
            label,
            "brightness",
            index,
            actual,
            expected_grey,
        )
        palette = [
            (255, 255, 255),
            (255, 255, 0),
            (0, 255, 255),
            (0, 255, 0),
            (255, 0, 255),
            (255, 0, 0),
            (0, 0, 255),
            (0, 0, 0),
        ]
        actual = point(x, height - cell * 1.5)
        assert all(abs(a - b) <= 2 for a, b in zip(actual[:3], palette[index])), (
            label,
            "color",
            index,
            actual,
        )
    if transform:
        assert all(
            image.getpixel(point)[:3] == (0, 0, 0)
            for point in [
                (5, 5),
                (width - 6, 5),
                (5, height - 6),
                (width - 6, height - 6),
            ]
        )
    return {
        "label": label,
        "clock": clock,
        "resolution": resolution,
        "measured_renderer_fps": rate,
        "cell_label": text,
        "palette_samples": 16,
        "warp": bool(transform),
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("directory", type=Path)
    parser.add_argument("--font", required=True, type=Path)
    parser.add_argument("--capture-log", type=Path)
    args = parser.parse_args()
    words = re.findall(r"uint2\((\d+)u, (\d+)u\)", args.font.read_text(encoding="utf8"))
    assert len(words) == 128
    font = [
        [(int(words[code][y // 4]) >> (8 * (y % 4))) & 255 for y in range(8)]
        for code in range(128)
    ]
    if args.capture_log:
        data = args.capture_log.read_bytes()
        log = data.decode("utf-16" if data.startswith(b"\xff\xfe") else "utf-8-sig")
        observations = [
            json.loads(line) for line in log.splitlines() if line.startswith('{"label"')
        ]
        assert observations
        plan_lines = [
            line
            for line in (args.directory / "app.log").read_text().splitlines()
            if "plan installed; channels=0 screens=1 outputs=1" in line
        ]
        result = {
            "observations": observations,
            "sourceFreePlan": plan_lines[0],
            "withdrawnPlan": None,
            "gpu": observations[0]["runtime"]["gpu_adapter"],
        }
    else:
        result = json.loads((args.directory / "results.json").read_text())
    checked = [inspect(args.directory, row, font) for row in result["observations"]]
    clocks = {row["label"]: row["clock"] for row in checked if "clock" in row}
    if "live-a" in clocks:
        assert clocks["live-a"] != clocks["live-b"]
        assert clocks["live-a"][-3:] != clocks["live-b"][-3:]
        assert (
            next(row for row in checked if row["label"] == "live-b")[
                "measured_renderer_fps"
            ]
            is not None
        )
        first = Image.open(args.directory / "live-a" / "pixels.png")
        second = Image.open(args.directory / "live-b" / "pixels.png")
        edge = first.getpixel((120, 300))[:3]
        # At integer grid positions, a 2x subpixel sits exactly on the 0.75px
        # stroke boundary. Compare spatial phase at the odd-size resize instead:
        # both subpixels are strictly inside each chosen line, so background
        # parity and legal f32 edge rounding cannot alter the comparison.
        spatial = Image.open(args.directory / "resize" / "pixels.png")
        width, height = spatial.size
        cell = max(min(width, height) / 9, max(width, height) / 32, 1)
        stroke = min(1.5, max(0.5, cell / 160))
        first_x, second_x = math.floor(cell), math.floor(3 * cell)
        first_y = 300
        second_y = round(first_y - (second_x - first_x) * height / width)
        for x, boundary in [(first_x, cell), (second_x, 3 * cell)]:
            assert all(abs(x + subpixel - boundary) < stroke for subpixel in [0.25, 0.75])
        spatial_edge = spatial.getpixel((first_x, first_y))[:3]
        same_phase = spatial.getpixel((second_x, second_y))[:3]
        assert all(abs(a - b) <= 2 for a, b in zip(spatial_edge, same_phase))
        assert (
            sum(abs(a - b) for a, b in zip(edge, second.getpixel((120, 300))[:3])) > 32
        )
        assert all(
            abs(value - srgb(0.2)) <= 2 for value in first.getpixel((123, 300))[:3]
        )
    report = {
        "passed": True,
        "checks": checked,
        "live_clock_changed": "live-a" in clocks,
        "thin_diagonal_rainbow_flow": "live-a" in clocks,
        "routing_restored": any(row["label"] == "restored-source" for row in checked),
        "source_free_plan": result["sourceFreePlan"],
        "withdrawn_plan": result["withdrawnPlan"],
        "font": str(args.font),
        "gpu": result["gpu"],
    }
    (args.directory / "pixel-results.json").write_text(
        json.dumps(report, indent=2) + "\n"
    )
    print(json.dumps(report))


if __name__ == "__main__":
    main()
