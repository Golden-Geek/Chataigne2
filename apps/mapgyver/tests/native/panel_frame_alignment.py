"""Replay the red Final-preview bounds in the owned M12 frame/DOM recording.

Requires FFmpeg and NumPy. Checks only recorded red frames, not missing-frame or latency
budgets. Frames beyond a bounded DOM trace are explicitly unverified, never extrapolated.
"""
import argparse
import bisect
import json
from pathlib import Path
import subprocess

import numpy as np


def verify(directory, check_matte=False):
    observations = json.loads((directory / "observations.json").read_text())
    samples = observations["samples"]
    capture = json.loads((directory / "capture.json").read_text())
    ready = json.loads((directory / "ready.json").read_text())
    times = [sample["timeOrigin"] + sample["at"] for sample in samples]
    width, height = capture["size"]
    origin = [capture["client_desktop_origin"][axis] - capture["desktop_bounds"][axis]
              for axis in range(2)]
    dpr = ready["viewport"]["dpr"]
    background = ready.get("background", "").lstrip("#")
    if check_matte:
        assert len(background) == 6, "fixture requires an explicit six-digit background token"
    expected_matte = tuple(int(background[index:index + 2], 16) for index in (0, 2, 4)) if check_matte else None

    def expected(sample):
        rect = sample["views"][1]["rect"]
        return (round(origin[0] + rect["x"] * dpr),
                round(origin[1] + rect["y"] * dpr),
                round(origin[0] + (rect["x"] + rect["width"]) * dpr),
                round(origin[1] + (rect["y"] + rect["height"]) * dpr))

    result = {"result": "failed", "compared_red_frames": 0, "no_red_frames": 0,
              "outside_dom_trace": 0, "mismatches": [], "maximum_error_px": 0,
              "scope": "Rightmost uninterrupted red Final span bounds; capture interval plus 50ms DOM sampling margin; not full-raster acceptance",
              "input_latency_verified": False, "missing_frames_verified": False,
              "trace_overflow": observations["overflow"]}
    result.update(checked_matte_frames=0, matte_mismatches=[])
    decoder = subprocess.Popen(["ffmpeg", "-v", "error", "-i", str(directory / "native-frames.mkv"),
                                "-f", "rawvideo", "-pix_fmt", "rgb24", "pipe:1"], stdout=subprocess.PIPE)
    try:
        for frame in capture["frames"]:
            raw = decoder.stdout.read(width * height * 3)
            assert len(raw) == width * height * 3, "truncated movie"
            begin, end = frame["begin_unix_ns"] / 1e6, frame["end_unix_ns"] / 1e6
            if begin < times[0] or end > times[-1]:
                result["outside_dom_trace"] += 1
                continue
            pixels = np.frombuffer(raw, dtype=np.uint8).reshape(height, width, 3)
            red = (pixels[:, :, 0] > 200) & (pixels[:, :, 1] < 40) & (pixels[:, :, 2] < 40)
            columns = np.flatnonzero(red.any(axis=0))
            if not len(columns) or columns[-1] < width * 0.7:
                result["no_red_frames"] += 1
                continue
            # The expected Final image is one uninterrupted span. Fragments exposed behind
            # unrelated browser content do not establish correct clipping or alignment.
            gaps = np.flatnonzero(np.diff(columns) > 1)
            left = int(columns[gaps[-1] + 1] if len(gaps) else columns[0])
            right = int(columns[-1] + 1)
            rows = np.flatnonzero(red[:, left:right].any(axis=1))
            actual = (left, int(rows[0]), right, int(rows[-1] + 1))
            first = max(0, bisect.bisect_left(times, begin - 50) - 1)
            last = min(len(times), bisect.bisect_right(times, end + 50) + 1)
            error = min(max(abs(a - b) for a, b in zip(actual, expected(sample)))
                        for sample in samples[first:last])
            result["compared_red_frames"] += 1
            result["maximum_error_px"] = max(result["maximum_error_px"], error)
            if error > 2:
                result["mismatches"].append({"frame": frame["frame"], "actual": actual, "error_px": error})
            if check_matte and error <= 2:
                y = actual[1] - round(20 * dpr)
                points = [left + round(20 * dpr), right - round(20 * dpr)]
                colors = [tuple(int(value) for value in pixels[y, x]) for x in points]
                result["checked_matte_frames"] += 1
                if any(max(abs(a - b) for a, b in zip(color, expected_matte)) > 2 for color in colors):
                    result["matte_mismatches"].append({"frame": frame["frame"], "colors": colors})
        assert decoder.wait(timeout=15) == 0
        assert result["compared_red_frames"] >= 30, "insufficient comparable red frames"
        assert not result["mismatches"], "cropped/displaced red Final preview"
        assert not result["matte_mismatches"], "retirement paint or stale UI differs from the host backdrop"
        result["result"] = "pass"
        return result
    finally:
        if decoder.poll() is None:
            decoder.kill()
            decoder.wait()
        (directory / "frame-alignment.json").write_text(json.dumps(result, indent=2))
        print(json.dumps(result))


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("directory", type=Path)
    parser.add_argument("--check-matte", action="store_true")
    args = parser.parse_args()
    verify(args.directory, args.check_matte)
