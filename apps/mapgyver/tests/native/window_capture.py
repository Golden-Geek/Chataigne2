"""Read-only capture baseline for an owned native-alignment fixture.

Check that a capture contains both native red and SVG blue before attempting alignment.
This does not measure alignment or automate window input. --image reuses an existing capture.
"""
import argparse
import hashlib
import json
from pathlib import Path

from PIL import Image, ImageGrab


def summarize(image):
    image = image.convert("RGB")
    colors = image.getcolors(maxcolors=image.width * image.height)
    red = sum(count for count, (r, g, b) in colors if r > 160 and g < 80 and b < 100)
    blue = sum(
        count
        for count, (r, g, b) in colors
        if b > 180 and 70 < r < 190 and 110 < g < 215
    )
    return {
        "size": list(image.size),
        "unique_colors": len(colors),
        "native_red_pixels": red,
        "overlay_blue_pixels": blue,
        "rgb_sha256": hashlib.sha256(image.tobytes()).hexdigest(),
        "capture_usable_for_alignment": red > 10000 and blue > 100,
        "alignment_verified": False,
    }


def main():
    parser = argparse.ArgumentParser()
    source = parser.add_mutually_exclusive_group(required=True)
    source.add_argument("--hwnd", type=int, help="HWND of the caller's owned fixture process")
    source.add_argument(
        "--image", type=Path, help="Analyze an existing capture without capturing again"
    )
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    if args.hwnd is not None:
        assert args.hwnd > 0, "Supply the positive HWND of the owned fixture"
        image = ImageGrab.grab(window=args.hwnd)
        image.save(args.output / "baseline.png")
    else:
        image = Image.open(args.image)
    result = summarize(image)
    (args.output / "capture.json").write_text(
        json.dumps(result, indent=2) + "\n", encoding="utf-8"
    )
    print(json.dumps(result))
    return 0 if result["capture_usable_for_alignment"] else 2


if __name__ == "__main__":
    raise SystemExit(main())
