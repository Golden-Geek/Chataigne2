"""Read-only, foreground/PID-guarded native pixels during the owned selection fixture."""
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
    result = {"result": "failed", "frames": 0, "red": 0, "blue": 0, "other": 0}
    first = None
    last = None
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
                colors = [
                    image.getpixel((x + dx, y + dy))[:3]
                    for dx in range(-3, 4)
                    for dy in range(-3, 4)
                ]
                red = sum(r > 160 and g < 80 and b < 100 for r, g, b in colors)
                blue = sum(b > 160 and g < 100 and r < 100 for r, g, b in colors)
                regions.append("red" if red >= 45 else "blue" if blue >= 45 else "other")
            result["frames"] += 1
            for color in ["red", "blue", "other"]:
                result[color] += int(color in regions)
            if first is None:
                first = image
                assert all(color == "red" for color in regions), (
                    f"initial native source must be red: {regions}"
                )
                (args.directory / "capture-started").write_text("started")
            last = image
            if (args.directory / "complete").exists():
                assert all(color == "red" for color in regions), (
                    "final selected native source must be red"
                )
                break
            time.sleep(0.01)
        else:
            raise AssertionError("native selection capture deadline")
        assert result["frames"] >= 16
        assert result["red"] > 0 and result["blue"] > 0, "both actual native sources must appear"
        assert result["other"] == 0, "blank/loading frames are not accepted"
        result["regions_per_frame"] = len(ready["boxes"])
        result["result"] = "pass"
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
