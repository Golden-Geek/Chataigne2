"""Read-only capture baseline for an owned native-alignment fixture.

Check that a capture contains both native red and blue overlays before attempting alignment.
This does not measure alignment or automate window input. --image reuses an existing capture.
"""
import argparse
import ctypes
from ctypes import wintypes
import hashlib
import json
import time
from pathlib import Path

from PIL import Image, ImageGrab


def validate_foreground(user, hwnd, pid, foreground_hwnd=None):
    """An explicit auxiliary capture is limited to an observed dialog in this fixture process."""
    expected = hwnd if foreground_hwnd is None else foreground_hwnd
    assert user.GetForegroundWindow() == expected, "owned fixture must be the foreground window"
    if expected != hwnd:
        owner = wintypes.DWORD()
        user.GetWindowThreadProcessId(expected, ctypes.byref(owner))
        assert owner.value == pid, "foreground dialog does not belong to the fixture PID"
        class_name = ctypes.create_unicode_buffer(256)
        assert user.GetClassNameW(expected, class_name, len(class_name))
        assert class_name.value == "#32770", "foreground auxiliary must be an owned native dialog"


def owned_window_bounds(hwnd, pid, foreground_hwnd=None):
    """Read physical bounds only for the caller's owned foreground window."""
    user = ctypes.windll.user32
    user.GetForegroundWindow.restype = wintypes.HWND
    user.GetWindowThreadProcessId.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.DWORD)]
    owner = wintypes.DWORD()
    user.GetWindowThreadProcessId(hwnd, ctypes.byref(owner))
    assert owner.value == pid, "capture HWND does not belong to the supplied fixture PID"
    user.GetClassNameW.argtypes = [wintypes.HWND, wintypes.LPWSTR, ctypes.c_int]
    validate_foreground(user, hwnd, pid, foreground_hwnd)
    user.SetThreadDpiAwarenessContext.argtypes = [ctypes.c_void_p]
    user.SetThreadDpiAwarenessContext(ctypes.c_void_p(-4))
    user.GetWindowRect.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.RECT)]
    rect = wintypes.RECT()
    assert user.GetWindowRect(hwnd, ctypes.byref(rect))
    bounds = (rect.left, rect.top, rect.right, rect.bottom)
    return bounds


def visible_owned_window(hwnd, pid, foreground_hwnd=None):
    """Read visible composited pixels only after checking the owned foreground window."""
    bounds = owned_window_bounds(hwnd, pid, foreground_hwnd)
    image = ImageGrab.grab(bbox=bounds, all_screens=True)
    assert bounds == owned_window_bounds(hwnd, pid, foreground_hwnd), "window moved during capture"
    return image, bounds


def client_geometry(hwnd):
    """Physical client placement for the same capture, independent of caller DPI virtualization."""
    user = ctypes.windll.user32
    user.SetThreadDpiAwarenessContext.argtypes = [ctypes.c_void_p]
    user.SetThreadDpiAwarenessContext(ctypes.c_void_p(-4))
    user.ClientToScreen.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.POINT)]
    user.GetDpiForWindow.argtypes = [wintypes.HWND]
    origin = wintypes.POINT()
    assert user.ClientToScreen(hwnd, ctypes.byref(origin))
    dpi = user.GetDpiForWindow(hwnd)
    assert dpi > 0, "owned fixture has no valid window DPI"
    user.IsZoomed.argtypes = [wintypes.HWND]
    user.IsZoomed.restype = wintypes.BOOL
    class MonitorInfo(ctypes.Structure):
        _fields_ = [("size", wintypes.DWORD), ("monitor", wintypes.RECT),
                    ("work", wintypes.RECT), ("flags", wintypes.DWORD)]
    user.MonitorFromWindow.argtypes = [wintypes.HWND, wintypes.DWORD]
    user.MonitorFromWindow.restype = wintypes.HANDLE
    monitor = user.MonitorFromWindow(hwnd, 2)
    info = MonitorInfo()
    info.size = ctypes.sizeof(info)
    user.GetMonitorInfoW.argtypes = [wintypes.HANDLE, ctypes.POINTER(MonitorInfo)]
    assert monitor and user.GetMonitorInfoW(monitor, ctypes.byref(info))
    return {"client_desktop_origin": [origin.x, origin.y], "window_dpi": dpi,
            "window_maximized": bool(user.IsZoomed(hwnd)),
            "monitor_bounds": [info.monitor.left, info.monitor.top,
                               info.monitor.right, info.monitor.bottom]}


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
    parser.add_argument("--foreground-hwnd", type=int, help="Exact observed owned native dialog covering the fixture")
    parser.add_argument("--wait-foreground-seconds", type=float, default=0, help="Bounded wait for supported external activation before guarded capture")
    source.add_argument(
        "--image", type=Path, help="Analyze an existing capture without capturing again"
    )
    parser.add_argument("--geometry-only", action="store_true")
    parser.add_argument("output", type=Path, nargs="?")
    args = parser.parse_args()
    if not 0 <= args.wait_foreground_seconds <= 60 or (args.wait_foreground_seconds and not args.visible):
        parser.error("foreground wait requires --visible and a duration between 0 and 60 seconds")
    if args.foreground_hwnd is not None and not args.visible:
        parser.error("--foreground-hwnd requires --visible")
    if args.geometry_only:
        if args.hwnd is None or args.pid is None:
            parser.error("--geometry-only requires --hwnd and --pid")
        bounds = owned_window_bounds(args.hwnd, args.pid)
        geometry = client_geometry(args.hwnd)
        assert bounds == owned_window_bounds(args.hwnd, args.pid)
        print(json.dumps({"desktop_bounds": bounds, **geometry}))
        return 0
    if args.output is None:
        parser.error("capture requires an output directory")
    if args.visible and (args.hwnd is None or args.pid is None):
        parser.error("--visible requires --hwnd and --pid")
    args.output.mkdir(parents=True, exist_ok=True)
    if args.hwnd is not None:
        assert args.hwnd > 0, "Supply the positive HWND of the owned fixture"
        if args.visible:
            assert args.pid and args.pid > 0, "--visible requires the fixture PID"
            if args.wait_foreground_seconds:
                user = ctypes.windll.user32
                user.GetForegroundWindow.restype = wintypes.HWND
                expected = args.foreground_hwnd or args.hwnd
                deadline = time.monotonic() + args.wait_foreground_seconds
                while user.GetForegroundWindow() != expected:
                    assert time.monotonic() < deadline, "owned foreground activation timed out"
                    time.sleep(.05)
            geometry = client_geometry(args.hwnd)
            image, bounds = visible_owned_window(args.hwnd, args.pid, args.foreground_hwnd)
            assert geometry == client_geometry(args.hwnd), "client placement/DPI changed during capture"
        else:
            image = ImageGrab.grab(window=args.hwnd)
        image.save(args.output / "baseline.png")
    else:
        image = Image.open(args.image)
    result = summarize(image)
    if args.visible:
        result.update(capture_mode="visible owned foreground window", desktop_bounds=list(bounds), **geometry)
        if args.foreground_hwnd is not None:
            result.update(capture_mode="visible owned fixture with exact owned foreground dialog", foreground_dialog=args.foreground_hwnd)
            result["foreground_dialog_bounds"] = list(owned_window_bounds(args.foreground_hwnd, args.pid))
    (args.output / "capture.json").write_text(
        json.dumps(result, indent=2) + "\n", encoding="utf-8"
    )
    print(json.dumps(result))
    return 0 if result["capture_usable_for_alignment"] else 2


if __name__ == "__main__":
    raise SystemExit(main())
