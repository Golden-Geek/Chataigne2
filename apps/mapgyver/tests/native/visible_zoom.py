"""Bounded same-frame native red/SVG blue observations for the owned Source fixture."""
import argparse
import itertools
import json
import time
from pathlib import Path
from PIL import ImageChops
from window_capture import summarize, visible_owned_window


def red(pixel):
    r, g, b = pixel[:3]
    return r > 160 and g < 80 and b < 100


def blue(pixel):
    r, g, b = pixel[:3]
    return b > 180 and 70 < r < 190 and 110 < g < 215


def positions(image, roi):
    left, top, right, bottom = roi
    cx, cy = (left + right) // 2, (top + bottom) // 2
    horizontal = [(x, image.getpixel((x, cy))) for x in range(left, right)]
    vertical = [(y, image.getpixel((cx, y))) for y in range(top, bottom)]
    offsets, edges = [], []
    for line in [horizontal, vertical]:
        reds = [at for at, pixel in line if red(pixel)]
        blues = [at for at, pixel in line if blue(pixel)]
        runs = [[at for _, at in group] for _, group in itertools.groupby(
            enumerate(blues), lambda value: value[1] - value[0])]
        assert reds and len(runs) == 2, "two blue edges and a red interior must be visible"
        first, last = sum(runs[0]) / len(runs[0]), sum(runs[-1]) / len(runs[-1])
        offsets.extend([min(reds) - first, last - max(reds)])
        edges.extend([min(reds), max(reds), first, last])
    return offsets, edges


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("hwnd", type=int)
    parser.add_argument("pid", type=int)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    image, _ = visible_owned_window(args.hwnd, args.pid)
    image = image.convert("RGB")
    assert summarize(image)["capture_usable_for_alignment"]
    r, g, b = image.split()
    mask = ImageChops.multiply(r.point([255 if v > 160 else 0 for v in range(256)]),
                              g.point([255 if v < 80 else 0 for v in range(256)]))
    mask = ImageChops.multiply(mask, b.point([255 if v < 100 else 0 for v in range(256)]))
    left, top, right, bottom = mask.getbbox()
    roi = (left - 12, top - 12, right + 12, bottom + 12)
    baseline, _ = positions(image, roi)
    image.save(args.output / "before.png")
    print(json.dumps({"capture_ready": True}), flush=True)
    samples, worst, worst_image = [], -1, image
    start = time.perf_counter()
    for _ in range(24):
        image, _ = visible_owned_window(args.hwnd, args.pid)
        offsets, edges = positions(image, roi)
        error = max(abs(a - b) for a, b in zip(offsets, baseline))
        samples.append({"elapsed_ms": (time.perf_counter() - start) * 1000,
                        "edge_offset_change_px": error, "edges": edges})
        if error > worst:
            worst, worst_image = error, image
        time.sleep(0.016)
    worst_image.save(args.output / "largest-offset.png")
    image.save(args.output / "after.png")
    result = {"samples": samples, "baseline_offsets_px": baseline,
              "max_edge_offset_change_px": worst,
              "limits": "visible color-threshold edge observations; no input-latency or full DPI-matrix acceptance"}
    (args.output / "zoom-pixels.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(result), flush=True)


if __name__ == "__main__":
    main()
