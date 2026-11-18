"""Independent convex-polygon color oracle for the owned linked-Surface output."""
import json
import math
import sys
from PIL import Image

image = Image.open(sys.argv[1]).convert("RGBA")
layers = json.loads(sys.argv[2])
width, height = image.size
counts = {"red": 0, "blue": 0, "background": 0, "edge_skipped": 0}
for y in range(height):
    for x in range(width):
        color = (0, 0, 0)
        label = "background"
        edge = False
        for layer in layers:
            points = [(p["x"] * width, p["y"] * height) for p in layer["corners"]]
            crosses = []
            for a, b in zip(points, points[1:] + points[:1]):
                dx, dy = b[0] - a[0], b[1] - a[1]
                cross = dx * (y + 0.5 - a[1]) - dy * (x + 0.5 - a[0])
                crosses.append(cross)
                if abs(cross) / max(math.hypot(dx, dy), 1e-12) <= 2:
                    edge = True
            if all(value >= 0 for value in crosses) or all(value <= 0 for value in crosses):
                color = tuple(layer["rgb"])
                label = layer["label"]
        if edge:
            counts["edge_skipped"] += 1
            continue
        actual = image.getpixel((x, y))
        assert actual[:3] == color, (x, y, actual, color)
        if label != "background":
            assert actual[3] == 255, (x, y, actual)
        counts[label] += 1
assert counts["red"] > 1000 and counts["blue"] > 1000 and counts["background"] > 1000, counts
print(json.dumps({"width": width, "height": height, "counts": counts, "passed": True}))
