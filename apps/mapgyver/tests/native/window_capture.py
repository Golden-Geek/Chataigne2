"""Read-only capture baseline for an owned native-alignment fixture.

Check that a capture contains both native red and SVG blue before attempting alignment.
This does not measure alignment or automate window input. --image reuses an existing capture.
"""
import argparse
import ctypes
from ctypes import wintypes
import hashlib
import json
from pathlib import Path

from PIL import Image, ImageGrab


def visible_owned_window(hwnd, pid):
    """Read visible composited pixels only after checking the owned foreground window."""
    user = ctypes.windll.user32
    user.GetForegroundWindow.restype = wintypes.HWND
    user.GetWindowThreadProcessId.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.DWORD)]
    owner = wintypes.DWORD()
    user.GetWindowThreadProcessId(hwnd, ctypes.byref(owner))
    assert owner.value == pid, "capture HWND does not belong to the supplied fixture PID"
    assert user.GetForegroundWindow() == hwnd, "owned fixture must be the foreground window"
    user.SetThreadDpiAwarenessContext.argtypes = [ctypes.c_void_p]
    user.SetThreadDpiAwarenessContext(ctypes.c_void_p(-4))
    user.GetWindowRect.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.RECT)]
    rect = wintypes.RECT()
    assert user.GetWindowRect(hwnd, ctypes.byref(rect))
    bounds = (rect.left, rect.top, rect.right, rect.bottom)
    return ImageGrab.grab(bbox=bounds, all_screens=True), bounds


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
    parser.add_argument("--visible", action="store_true", help="Capture its visible desktop region, not PrintWindow")
    parser.add_argument("--pid", type=int, help="Required owned fixture PID for --visible")
    source.add_argument(
        "--image", type=Path, help="Analyze an existing capture without capturing again"
    )
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    if args.visible and (args.hwnd is None or args.pid is None):
        parser.error("--visible requires --hwnd and --pid")
    args.output.mkdir(parents=True, exist_ok=True)
    if args.hwnd is not None:
        assert args.hwnd > 0, "Supply the positive HWND of the owned fixture"
        if args.visible:
            assert args.pid and args.pid > 0, "--visible requires the fixture PID"
            image, bounds = visible_owned_window(args.hwnd, args.pid)
        else:
            image = ImageGrab.grab(window=args.hwnd)
        image.save(args.output / "baseline.png")
    else:
        image = Image.open(args.image)
    result = summarize(image)
    if args.visible:
        result.update(capture_mode="visible owned foreground window", desktop_bounds=list(bounds))
    (args.output / "capture.json").write_text(
        json.dumps(result, indent=2) + "\n", encoding="utf-8"
    )
    print(json.dumps(result))
    return 0 if result["capture_usable_for_alignment"] else 2


if __name__ == "__main__":
    raise SystemExit(main())
