"""Bounded native image/overlay observations for owned Screen and Source fixtures."""
import argparse
import itertools
import json
import time
from pathlib import Path
from PIL import ImageChops
from window_capture import summarize, visible_owned_window
from landmark_edges import linear_red_edges


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


def independent_landmark_alignment(image, geometry, capture, editor):
    """Oracle for native-alignment's fixed --independent-landmark axis-aligned fixture.

    The red source rectangle occupies 10–90% and is unobscured by the crop/quad stroke.
    Screen maps that source into its 20–80% quad; Source shows the full image with a
    20–80% crop. Infer overlay positions from those independent image edges as well as
    checking both against DOM/client/DPR projection. This measures settled pixels only.
    """
    assert editor in ("screen", "source")
    assert capture["capture_mode"] == "visible owned foreground window"
    assert capture["capture_usable_for_alignment"]
    assert list(image.size) == capture["size"]
    active, scale = geometry["active"], geometry["viewport"]["dpr"]
    assert scale > 0 and abs(capture["window_dpi"] / 96 - scale) < 1e-6
    offset = [a - b for a, b in zip(
        capture["client_desktop_origin"], capture["desktop_bounds"][:2])]

    def project(fractions):
        return [offset[0] + (active["x"] + active["width"] * v) * scale for v in fractions] + [
            offset[1] + (active["y"] + active["height"] * v) * scale for v in fractions]

    frame = project([0, 1])
    roi = tuple(round(frame[i]) for i in (0, 2, 1, 3))
    image = image.convert("RGB")
    _, edges = positions(image, roi)
    # Binary encoded-R thresholds identify a local transition, but their integer run
    # boundaries amplify sampling error when extrapolated to the independent overlay.
    threshold_edges = [edges[0], edges[1] + 1, edges[4], edges[5] + 1]
    red_edges = linear_red_edges(image, roi, threshold_edges)
    blue_centers = [edges[i] + 0.5 for i in (2, 3, 6, 7)]
    red_fractions = [0.26, 0.74] if editor == "screen" else [0.1, 0.9]
    blue_fractions = [0.2, 0.8]
    def infer(measured):
        inferred = []
        for start, end in ((measured[0], measured[1]), (measured[2], measured[3])):
            slope = (end - start) / (red_fractions[1] - red_fractions[0])
            inferred.extend(start + (v - red_fractions[0]) * slope for v in blue_fractions)
        return inferred

    inferred = infer(red_edges)

    def error(a, b):
        return max(abs(x - y) for x, y in zip(a, b))

    red_expected, blue_expected = project(red_fractions), project(blue_fractions)
    errors = {
        "image_to_projection_max_px": error(red_edges, red_expected),
        "overlay_to_projection_max_px": error(blue_centers, blue_expected),
        "image_to_overlay_max_px": error(inferred, blue_centers),
    }
    return {
        "red_edges": red_edges, "blue_centers": blue_centers,
        "edge_measurement": "local linear-light 50% transition at physical pixel centers",
        "threshold_red_edges": threshold_edges,
        "legacy_threshold_errors": {
            "image_to_projection_max_px": error(threshold_edges, red_expected),
            "overlay_to_projection_max_px": error(blue_centers, blue_expected),
            "image_to_overlay_max_px": error(infer(threshold_edges), blue_centers),
        },
        "red_expected": red_expected, "blue_expected": blue_expected,
        "inferred_overlay_edges": inferred, **errors,
        "settled_alignment_verified": all(value <= 1 for value in errors.values()),
        "input_latency_verified": False,
    }


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
