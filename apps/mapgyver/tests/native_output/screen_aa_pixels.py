"""Compare one owned physical client against a finite straight-alpha SDK image."""

import argparse
import ctypes
from ctypes import wintypes
import json
from pathlib import Path

import numpy as np
from PIL import Image, ImageGrab
from window_state import snapshot


def compare(pid, source, directory, mode):
    before = snapshot(pid)
    assert len(before) == 1, f"expected one owned output: {before}"
    window = before[0]
    assert window["visible"] and not window["minimized"], window
    bounds = window["bounds"]
    width, height = bounds["extent"]["width"], bounds["extent"]["height"]
    assert 0 < width <= 3840 and 0 < height <= 2160
    sdk = np.asarray(Image.open(source).convert("RGBA"))
    assert sdk.shape == (height, width, 4), (sdk.shape, bounds)
    alpha = sdk[:, :, 3]
    maximum_alpha = int(alpha.max())
    assert maximum_alpha in (127, 128), maximum_alpha
    fractional = int(((alpha > 0) & (alpha < maximum_alpha)).sum())
    if mode == "Raw":
        assert fractional == 0, f"Raw has unexpected fractional coverage: {fractional}"
    else:
        assert fractional > 32, f"Smooth frame not observed: {fractional}"
    assert int((alpha == maximum_alpha).sum()) > width * height // 4
    assert int((alpha == 0).sum()) > width * height // 8

    # Native opaque presentation composites premultiplied linear light over black.
    # SDK pixels are straight sRGB; decode before multiplying by their coverage.
    encoded = sdk[:, :, :3].astype(np.float64) / 255
    linear = np.where(
        encoded <= 0.04045, encoded / 12.92, ((encoded + 0.055) / 1.055) ** 2.4
    ) * (alpha.astype(np.float64)[:, :, None] / 255)
    opaque = np.where(
        linear <= 0.0031308, linear * 12.92, 1.055 * linear ** (1 / 2.4) - 0.055
    )
    expected = np.rint(np.clip(opaque, 0, 1) * 255).astype(np.int16)
    left, top = bounds["left"], bounds["top"]
    image = ImageGrab.grab(
        bbox=(left, top, left + width, top + height), all_screens=True
    ).convert("RGB")
    after = snapshot(pid)
    assert len(after) == 1
    assert after[0]["hwnd"] == window["hwnd"] and after[0]["bounds"] == bounds
    assert after[0]["visible"] and not after[0]["minimized"]
    actual = np.asarray(image).astype(np.int16)
    assert actual.shape == expected.shape
    difference = np.abs(actual - expected)
    bad = np.any(difference > 2, axis=2)
    bad_count = int(bad.sum())
    user = ctypes.windll.user32
    user.GetDpiForWindow.argtypes = [wintypes.HWND]
    dpi = int(user.GetDpiForWindow(window["hwnd"]))
    report = {
        "passed": bad_count == 0,
        "pid": pid,
        "mode": mode,
        "window": window,
        "dpi": dpi,
        "pixels": width * height,
        "fractional_sdk_pixels": fractional,
        "sdk_alpha_levels": [int(value) for value in np.unique(alpha)],
        "maximum_rgb_error": int(difference.max()),
        "outside_2_byte_tolerance": bad_count,
        "capture": "actual composited desktop physical client; owned PID/class/bounds verified",
    }
    if bad_count:
        y, x = np.argwhere(bad)[0]
        report["first_mismatch"] = {
            "xy": [int(x), int(y)],
            "actual": actual[y, x].tolist(),
            "expected": expected[y, x].tolist(),
        }
    else:
        # Retain only verified renderer pixels, never an occluding application's image.
        directory.mkdir(parents=True, exist_ok=True)
        image.save(directory / "physical.png")
        (directory / "physical-result.json").write_text(
            json.dumps(report, indent=2) + "\n", encoding="utf8"
        )
    return report


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("pid", type=int)
    parser.add_argument("sdk", type=Path)
    parser.add_argument("directory", type=Path)
    parser.add_argument("mode", choices=["Raw", "Smooth2x"])
    args = parser.parse_args()
    try:
        result = compare(args.pid, args.sdk, args.directory, args.mode)
    except AssertionError as error:
        result = {"passed": False, "error": str(error), "pid": args.pid}
    print(json.dumps(result))
    raise SystemExit(0 if result["passed"] else 1)


if __name__ == "__main__":
    main()
