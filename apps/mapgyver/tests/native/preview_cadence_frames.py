"""Read-only motion witness for one owned, foreground native editor window."""
import argparse
import ctypes
from ctypes import wintypes
import hashlib
import json
from pathlib import Path
import time

from window_capture import visible_owned_window


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("directory", type=Path)
    args = parser.parse_args()
    ready = json.loads((args.directory / "ready.json").read_text())
    user = ctypes.windll.user32
    user.GetForegroundWindow.restype = wintypes.HWND
    hwnd = user.GetForegroundWindow()
    samples = []
    deadline = time.monotonic() + 6
    while time.monotonic() < deadline:
        image, bounds = visible_owned_window(hwnd, ready["pid"])
        origin = wintypes.POINT(0, 0)
        user.ClientToScreen.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.POINT)]
        assert user.ClientToScreen(hwnd, ctypes.byref(origin))
        box = ready["box"]
        scale = ready["dpr"]
        left = origin.x - bounds[0] + (box["x"] + box["width"] * .2) * scale
        top = origin.y - bounds[1] + (box["y"] + box["height"] * .2) * scale
        crop = image.crop((round(left), round(top),
                           round(left + box["width"] * .6 * scale),
                           round(top + box["height"] * .6 * scale))).convert("RGB")
        if not samples:
            image.save(args.directory / "first.png")
        samples.append({"at": time.monotonic(), "hash": hashlib.sha256(crop.tobytes()).hexdigest()})
        time.sleep(.01)
    image.save(args.directory / "last.png")
    changes = sum(a["hash"] != b["hash"] for a, b in zip(samples, samples[1:]))
    elapsed = samples[-1]["at"] - samples[0]["at"]
    result = {"samples": len(samples), "changes": changes, "elapsed": elapsed,
              "observed_changes_per_second": changes / elapsed, "frames": samples,
              "limits": "Regional visible-pixel changes, limited by capture rate; not scanout FPS."}
    (args.directory / "pixels.json").write_text(json.dumps(result, indent=2))
    print(json.dumps({key: value for key, value in result.items() if key != "frames"}))


if __name__ == "__main__":
    main()
