"""Read-only owned foreground pixels while the graph socket reconnects.

Sources and geometry stay fixed: each sampled region must keep its initial color.
This proves those regions, not whole-frame raster correctness or timing budgets.
"""
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
    parser.add_argument("--hwnd", type=int, required=True)
    args = parser.parse_args()
    ready = json.loads((args.directory / "ready.json").read_text())
    result = {"result": "failed", "frames": 0, "changed_regions": 0}
    initial = None
    first = last = None
    try:
        deadline = time.monotonic() + 30
        while time.monotonic() < deadline:
            image, bounds = visible_owned_window(args.hwnd, ready["pid"])
            origin = wintypes.POINT(0, 0)
            user = ctypes.windll.user32
            user.ClientToScreen.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.POINT)]
            assert user.ClientToScreen(args.hwnd, ctypes.byref(origin))
            regions = []
            for box in ready["boxes"]:
                x = round(origin.x - bounds[0] + (box["x"] + box["width"] / 2) * ready["dpr"])
                y = round(origin.y - bounds[1] + (box["y"] + box["height"] / 2) * ready["dpr"])
                regions.append([
                    image.getpixel((x + dx, y + dy))[:3]
                    for dx in range(-3, 4) for dy in range(-3, 4)
                ])
            if initial is None:
                initial = [region[24] for region in regions]
                assert all(max(color) > 160 for color in initial), "initial native regions must be bright"
                first = image
                (args.directory / "capture-started").write_text("started")
            for region, expected in zip(regions, initial):
                matching = sum(all(abs(a - b) <= 5 for a, b in zip(color, expected)) for color in region)
                result["changed_regions"] += int(matching < 45)
            result["frames"] += 1
            last = image
            if (args.directory / "complete").exists():
                break
            time.sleep(0.01)
        else:
            raise AssertionError("native reconnect capture deadline")
        assert result["frames"] >= 8
        assert result["changed_regions"] == 0, "blank or changed native regions are not accepted"
        result.update(result="pass", regions_per_frame=len(initial), initial_rgb=initial)
    except Exception as error:
        result["error"] = str(error)
        raise
    finally:
        if first is not None:
            first.save(args.directory / "first.png")
        if last is not None:
            last.save(args.directory / "last.png")
        (args.directory / "pixels.json").write_text(json.dumps(result, indent=2))
        (args.directory / "capture-finished").write_text("finished")
        print(json.dumps(result))


if __name__ == "__main__":
    main()
