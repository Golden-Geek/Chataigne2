"""Verify actual foreground-window pixels against separately read DOM/native metadata."""
import argparse
import json
import math
from pathlib import Path

from PIL import Image


def verify(directory, stage):
    metadata = json.loads((directory / f"{stage}-dom.json").read_text())
    capture = json.loads((directory / stage / "capture.json").read_text())
    image = Image.open(directory / stage / "baseline.png").convert("RGB")
    assert capture["capture_mode"] == "visible owned foreground window"
    assert metadata["dpr"] == capture["window_dpi"] / 96
    scale = metadata["dpr"]
    origin = [capture["client_desktop_origin"][i] - capture["desktop_bounds"][i] for i in range(2)]

    def pixel(x, y):
        return tuple(round(origin[i] + value * scale) for i, value in enumerate([x, y]))

    def inside(r, x, y, margin=0):
        return r["x"] + margin <= x <= r["x"] + r["width"] - margin and r["y"] + margin <= y <= r["y"] + r["height"] - margin

    view, active = metadata.get("view"), metadata.get("active")
    assert view and active, "Expected a visible content rectangle"
    gray = metadata["theme"]["rgba"][:3]
    assert max(gray) - min(gray) < 40 and min(gray) > 0, gray
    probes = []
    # Avoid the floating toolbar and a physical pixel next to content/frame boundaries.
    for iy in range(1, 40):
        for ix in range(1, 40):
            x, y = view["x"] + view["width"] * (ix - 0.5) / 40, view["y"] + view["height"] * (iy - 0.5) / 40
            if metadata.get("toolbar") and inside(metadata["toolbar"], x, y, -3 * float(metadata.get("rootFont", "16px").removesuffix("px"))):
                continue
            if inside(active, x, y, -5):
                continue
            actual = image.getpixel(pixel(x, y))
            error = max(abs(a - b) for a, b in zip(actual, gray))
            assert error <= 2, {"probe": [x, y], "actual": actual, "gray": gray}
            probes.append(error)
    hits, handle_errors = [], []
    for hit in metadata["hits"]:
        sample = hit["native"].get("sample")
        x, y = hit["point"]
        if not inside(view, x, y, 2):
            continue
        if not sample and 'missing' in stage:
            continue
        assert sample and sample["hit"] == hit["id"], hit
        error = max(abs(sample["rect"][key] - active[key]) for key in ["x", "y", "width", "height"])
        assert error <= 0.05, error
        hits.append(error)
        if hit["id"].rsplit(":", 1)[-1] not in ["0", "1", "2", "3"]:
            continue
        radius = math.ceil(max(hit["bounds"]["width"], hit["bounds"]["height"]) * scale / 2 + 3)
        if not inside(view, x, y, radius / scale) or y < view["y"] + 45:
            continue
        px, py = pixel(x, y)
        white = [(px + dx + 0.5, py + dy + 0.5) for dx in range(-radius, radius + 1)
                 for dy in range(-radius, radius + 1)
                 if max(abs(a - b) for a, b in zip(image.getpixel((px + dx, py + dy)), [220, 234, 255])) <= 2]
        assert white, f"Native handle not visible: {hit['id']}"
        centre = [(min(p[i] for p in white) + max(p[i] for p in white)) / 2 for i in range(2)]
        expected = [origin[i] + hit["point"][i] * scale for i in range(2)]
        error = max(abs(a - b) for a, b in zip(centre, expected))
        assert error <= 1, {"handle": hit["id"], "actual": centre, "expected": expected}
        handle_errors.append(error)
    red = []
    left, top = pixel(view["x"] + 2, view["y"] + 45)
    right, bottom = pixel(view["x"] + view["width"] - 2, view["y"] + view["height"] - 2)
    for y in range(max(0, top), min(image.height, bottom)):
        for x in range(max(0, left), min(image.width, right)):
            r, g, b = image.getpixel((x, y))
            if r > 160 and g < 80 and b < 100:
                red.append((x, y))
    bounds_error = None
    image_edges_verified = False
    if red:
        source = metadata["label"] == "Surface source view"
        expected = dict(active)
        if not source:
            expected.update(x=active["x"] + active["width"] / 4, y=active["y"] + active["height"] / 4,
                            width=active["width"] / 2, height=active["height"] / 2)
        expected_bounds = [max(left, pixel(expected["x"], expected["y"])[0]),
                           max(top, pixel(expected["x"], expected["y"])[1]),
                           min(right, pixel(expected["x"] + expected["width"], expected["y"])[0]),
                           min(bottom, pixel(expected["x"], expected["y"] + expected["height"])[1])]
        actual_bounds = [min(x for x, _ in red), min(y for _, y in red), max(x for x, _ in red) + 1, max(y for _, y in red) + 1]
        bounds_error = max(abs(a - b) for a, b in zip(actual_bounds, expected_bounds))
        # A selected Screen's blue stroke occludes its red image edge. Do not count that
        # envelope as image alignment; require separate unoccluded Screen/Source captures.
        selected = any(hit["id"].rsplit(":", 1)[-1] in ["0", "1", "2", "3"] for hit in metadata["hits"])
        if source or not selected:
            assert bounds_error <= 1, {"actual": actual_bounds, "expected": expected_bounds}
            image_edges_verified = True
    result = {"stage": stage, "gray_samples": len(probes), "maximum_gray_error_255": max(probes, default=0),
              "native_hits": len(hits), "maximum_hit_rect_error_css": max(hits, default=0),
              "red_pixels": len(red), "red_bounds_error_px": bounds_error, "image_edges_verified": image_edges_verified,
              "physical_handles": len(handle_errors), "maximum_handle_error_px": max(handle_errors, default=0),
              "dpi": capture["window_dpi"]}
    (directory / f"{stage}-verified.json").write_text(json.dumps(result, indent=2) + "\n")
    return result


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("directory", type=Path)
    parser.add_argument("stages", nargs="+")
    args = parser.parse_args()
    for stage in args.stages:
        print(json.dumps(verify(args.directory, stage)))
