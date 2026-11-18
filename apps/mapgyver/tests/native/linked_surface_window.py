"""Compare a guarded owned-window capture with the linked-Surface DOM geometry."""
import hashlib
import json
import sys
from pathlib import Path

import numpy as np
from PIL import Image

directory = Path(sys.argv[1])
geometry = json.loads((directory / "geometry.json").read_text())
capture = json.loads((directory / "capture.json").read_text())
image = Image.open(directory / "baseline.png").convert("RGB")
assert capture["capture_mode"] == "visible owned foreground window"
assert capture["cursor_stable"] and capture["capture_usable_for_alignment"]
assert hashlib.sha256(image.tobytes()).hexdigest() == capture["rgb_sha256"]
assert geometry["dpr"] == capture["window_dpi"] / 96
pixels = np.asarray(image).astype(int)
origin = np.array(capture["client_desktop_origin"]) - np.array(capture["desktop_bounds"][:2])
rect = geometry["svg"]
probes = []
for layer, name in zip(geometry["polygons"][:2], ["red", "blue"]):
    quad = np.array([[float(v) for v in pair.split(",")] for pair in layer["points"].split()])
    quad = (
        quad * [rect["width"], rect["height"]] + [rect["x"], rect["y"]]
    ) * geometry["dpr"] + origin
    channel = 0 if name == "red" else 2
    others = [index for index in range(3) if index != channel]
    mask = pixels[:, :, channel] - np.maximum(
        pixels[:, :, others[0]], pixels[:, :, others[1]]
    ) > 100
    for edge in range(4):
        start, end = quad[edge], quad[(edge + 1) % 4]
        delta = end - start
        normal = np.array([-delta[1], delta[0]]) / np.linalg.norm(delta)
        for along in [.2, .4, .6, .8]:
            expected = start + delta * along

            def inside(distance):
                x, y = np.floor(expected + normal * distance).astype(int)
                return bool(mask[y, x])

            assert not inside(-6) and inside(6), (name, edge, along, "wrong edge sides")
            crossing = next(float(t) for t in np.arange(-6, 6.01, .125) if inside(t)) - .0625
            # Color boundaries are inset by the rendered contour stroke (up to 2 physical
            # pixels here); include pixel-center rounding. This is not a subpixel claim.
            assert abs(crossing) <= 3, (name, edge, along, crossing)
            probes.append({"layer": name, "edge": edge, "along": along,
                           "expected": expected.tolist(), "signed_color_boundary_offset_px": crossing})
hits = geometry["hits"]
assert len(hits) == 5 and all(hit["sample"]["sample"]["hit"] == hit["id"] for hit in hits)
result = {"passed": True, "physical_edge_probes": len(probes),
          "max_boundary_offset_px": max(abs(probe["signed_color_boundary_offset_px"]) for probe in probes),
          "limit_px": 3, "native_matching_hits": len(hits), "probes": probes}
(directory / "edge-probes.json").write_text(json.dumps(result, indent=2))
print(json.dumps({key: value for key, value in result.items() if key != "probes"}))
