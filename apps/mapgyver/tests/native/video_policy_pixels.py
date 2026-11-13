"""Report an independent pixel oracle; the owning fixture enforces matches/deadline."""

import argparse
import hashlib
import json
from pathlib import Path

from PIL import Image


def linear(value):
    value /= 255
    return value / 12.92 if value <= 0.04045 else ((value + 0.055) / 1.055) ** 2.4


def expected(index, profile):
    if profile == "hap":
        red, green = 4 + index * 3, 8 + index * 6
        return [
            linear((red << 3) | (red >> 2)),
            linear((green << 2) | (green >> 4)),
            linear((red << 3) | (red >> 2)),
        ]
    encoded = (16 + index * 24) / 219
    value = encoded / 4.5 if encoded < 0.081 else ((encoded + 0.099) / 1.099) ** (1 / 0.45)
    return [value] * 3


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("image")
    parser.add_argument("--frame", type=int)
    parser.add_argument("--profile", choices=["gray", "hap"], default="gray")
    args = parser.parse_args()
    image = Image.open(args.image).convert("RGBA")
    assert image.size == (160, 120), image.size
    points = [image.getpixel((x, y)) for x, y in [(40, 30), (80, 60), (120, 90)]]
    result = {
        "rgba": points,
        "sha256": hashlib.sha256(Path(args.image).read_bytes()).hexdigest(),
    }
    if args.frame is not None:
        target = expected(args.frame, args.profile)
        error = max(abs(linear(color[i]) - target[i]) for color in points for i in range(3))
        result.update(expected_linear=target, max_linear_error=error, tolerance=4 / 255)
        result["matches"] = error <= 4 / 255 and all(color[3] == 255 for color in points)
    print(json.dumps(result))


if __name__ == "__main__":
    main()
