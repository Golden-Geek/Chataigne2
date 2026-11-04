"""Actual owned-window frames through preview close/reopen and intentional project replacement."""
import argparse
import ctypes
from ctypes import wintypes
import json
from pathlib import Path
import time

from PIL import ImageChops
from window_capture import visible_owned_window


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("directory", type=Path)
    parser.add_argument("--hwnd", type=int, required=True)
    args = parser.parse_args()
    ready = json.loads((args.directory / "ready.json").read_text())
    result = {"result": "failed", "frames": 0, "unexpected_native": 0, "changed_ui": 0,
              "stages": {"tabs": 0, "new": 0, "loaded": 0, "replacing": 0}}
    initial_ui = None
    snapshots = set()
    def stage_now():
        if (args.directory / "load-settled").exists():
            return "loaded"
        if (args.directory / "load-started").exists():
            return "replacing"
        if (args.directory / "new-settled").exists():
            return "new"
        if (args.directory / "new-started").exists():
            return "replacing"
        return "tabs"
    try:
        deadline = time.monotonic() + 30
        while time.monotonic() < deadline:
            before = stage_now()
            image, bounds = visible_owned_window(args.hwnd, ready["pid"])
            after = stage_now()
            origin = wintypes.POINT(0, 0)
            user = ctypes.windll.user32
            user.ClientToScreen.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.POINT)]
            assert user.ClientToScreen(args.hwnd, ctypes.byref(origin))
            box = ready["box"]
            x = round(origin.x - bounds[0] + (box["x"] + box["width"] / 2) * ready["dpr"])
            y = round(origin.y - bounds[1] + (box["y"] + box["height"] / 2) * ready["dpr"])
            colors = [image.getpixel((x+dx, y+dy))[:3] for dx in range(-3,4) for dy in range(-3,4)]
            red = sum(r > 160 and g < 80 and b < 100 for r,g,b in colors) >= 45
            ui = ready["ui_box"]
            left = round(origin.x - bounds[0] + ui["x"] * ready["dpr"])
            top = round(origin.y - bounds[1] + ui["y"] * ready["dpr"])
            crop = image.crop((left, top, left + round(ui["width"] * ready["dpr"]),
                               top + round(ui["height"] * ready["dpr"]))).convert("RGB")
            if initial_ui is None:
                assert red, "initial locked native preview must be red"
                initial_ui = crop
                (args.directory / "capture-started").write_text("started")
            changed = sum(max(color) > 8 for color in ImageChops.difference(crop, initial_ui).get_flattened_data())
            if changed > crop.width * crop.height * 0.005:
                if result["changed_ui"] == 0:
                    initial_ui.save(args.directory / "ui-reference.png")
                    crop.save(args.directory / "ui-changed.png")
                result["changed_ui"] += 1
            stage = after if before == after else "replacing"
            result["unexpected_native"] += int((stage in ("tabs", "loaded") and not red) or (stage == "new" and red))
            result["stages"][stage] += 1
            result["frames"] += 1
            if stage not in snapshots:
                image.save(args.directory / f"{stage}.png")
                snapshots.add(stage)
            if (args.directory / "complete").exists():
                break
            time.sleep(0.01)
        else:
            raise AssertionError("native project transition capture deadline")
        assert result["stages"]["tabs"] >= 8
        assert result["stages"]["new"] >= 3 and result["stages"]["loaded"] >= 3
        assert result["unexpected_native"] == 0, "live peer content must persist; old project pixels must retire"
        assert result["changed_ui"] == 0, "unrelated application menu must not flash"
        result["result"] = "pass"
    except Exception as error:
        result["error"] = str(error)
        raise
    finally:
        (args.directory / "pixels.json").write_text(json.dumps(result, indent=2))
        (args.directory / "capture-finished").write_text("finished")
        print(json.dumps(result))


if __name__ == "__main__":
    main()
