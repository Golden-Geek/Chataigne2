"""Read-only actual desktop samples for one owned Matrix thumbnail fixture."""

import argparse
import ctypes
from ctypes import wintypes
import json
from pathlib import Path

from window_capture import client_geometry, visible_owned_window


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("directory", type=Path)
    args = parser.parse_args()
    ready = json.loads((args.directory / "ready.json").read_text(encoding="utf8"))
    user = ctypes.windll.user32
    user.SetThreadDpiAwarenessContext.argtypes = [ctypes.c_void_p]
    user.SetThreadDpiAwarenessContext(ctypes.c_void_p(-4))
    user.GetWindowThreadProcessId.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.DWORD)]
    user.IsWindowVisible.argtypes = [wintypes.HWND]
    user.GetWindowLongW.argtypes = [wintypes.HWND, ctypes.c_int]
    windows = []

    @ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.HWND, wintypes.LPARAM)
    def visit(hwnd, _):
        pid = wintypes.DWORD()
        user.GetWindowThreadProcessId(hwnd, ctypes.byref(pid))
        if (pid.value == ready["pid"] and user.IsWindowVisible(hwnd)
                and user.GetWindowLongW(hwnd, -16) & 0x00C00000 == 0x00C00000):
            windows.append(hwnd)
        return True

    user.EnumWindows(visit, 0)
    assert len(windows) == 1, "one visible owned main window required"
    hwnd = windows[0]
    geometry = client_geometry(hwnd)
    image, bounds = visible_owned_window(hwnd, ready["pid"])
    assert geometry == client_geometry(hwnd)
    origin = geometry["client_desktop_origin"]
    observations = []
    for probe in ready["probes"]:
        x, y = [round(origin[i] - bounds[i] + probe["xy"][i] * ready["dpr"]) for i in (0, 1)]
        assert 1 <= x < image.width - 1 and 1 <= y < image.height - 1
        pixels = list(image.crop((x - 1, y - 1, x + 2, y + 2)).convert("RGB").getdata())
        assert all(all(abs(a - b) <= 2 for a, b in zip(pixel, probe["rgb"])) for pixel in pixels), (probe, pixels)
        observations.append({"label": probe["label"], "expected": probe["rgb"], "actual": sorted(set(pixels))})
    image.save(args.directory / "owned-window.png")
    print(json.dumps({"passed": True, "pid": ready["pid"], "hwnd": hwnd, "geometry": geometry,
                      "probes": observations, "capture": "actual composited desktop, owned foreground PID and stable bounds"}))


if __name__ == "__main__":
    main()
