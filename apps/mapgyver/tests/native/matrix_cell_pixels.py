"""Verify actual Cell image centers in one owned foreground native test window."""
import argparse
import ctypes
from ctypes import wintypes
import json
from pathlib import Path
import time

from window_capture import visible_owned_window


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("directory", type=Path)
    parser.add_argument("--foreground-wait-seconds", type=int, default=0, choices=(0, 180, 480))
    args = parser.parse_args()
    ready = json.loads((args.directory / "ready.json").read_text())
    user = ctypes.windll.user32
    user.GetForegroundWindow.restype = wintypes.HWND
    user.GetWindowThreadProcessId.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.DWORD)]
    user.SetForegroundWindow.argtypes = [wintypes.HWND]
    user.IsWindowVisible.argtypes = [wintypes.HWND]
    user.GetWindowLongW.argtypes = [wintypes.HWND, ctypes.c_int]
    matches = []

    @ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.HWND, wintypes.LPARAM)
    def visit(hwnd, _):
        pid = wintypes.DWORD()
        user.GetWindowThreadProcessId(hwnd, ctypes.byref(pid))
        # Native presentation owns an additional uncaptioned window in this process.
        if (pid.value == ready["pid"] and user.IsWindowVisible(hwnd)
                and user.GetWindowLongW(hwnd, -16) & 0x00C00000 == 0x00C00000):
            matches.append(hwnd)
        return True

    user.EnumWindows(visit, 0)
    assert len(matches) == 1, "exactly one visible owned native test window required"
    hwnd = matches[0]
    user.SetForegroundWindow(hwnd)
    # Optional human setup wait, separate from render readiness and unchanged pixel guards.
    deadline = time.monotonic() + args.foreground_wait_seconds
    while user.GetForegroundWindow() != hwnd and time.monotonic() < deadline:
        time.sleep(.1)
    image, bounds = visible_owned_window(hwnd, ready["pid"])
    origin = wintypes.POINT(0, 0)
    user.ClientToScreen.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.POINT)]
    assert user.ClientToScreen(hwnd, ctypes.byref(origin))
    image.save(args.directory / "cells.png")
    observations = []
    for cell in ready["cells"]:
        box = cell["box"]
        x = round(origin.x - bounds[0] + (box["x"] + box["width"] * .5) * ready["dpr"])
        y = round(origin.y - bounds[1] + (box["y"] + box["height"] * cell.get("fraction_y", .4)) * ready["dpr"])
        pixels = list(image.crop((x - 1, y - 1, x + 2, y + 2)).convert("RGB").getdata())
        expected = cell["rgb"]
        if expected is None:
            assert all(not (g > 160 and r < 80 and b < 80) for r, g, b in pixels), "withdrawn green thumbnail remains"
        else:
            assert all(all(abs(a - b) <= 2 for a, b in zip(pixel, expected)) for pixel in pixels), (cell, pixels)
        observations.append({"expected": expected, "actual": sorted(set(pixels))})
    print(json.dumps({"result": "pass", "cells": observations, "bounds": bounds}))


if __name__ == "__main__":
    main()
