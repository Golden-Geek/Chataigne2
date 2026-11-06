"""Replay recorded owned-desktop response bounds; an upper bound above budget is inconclusive."""
import math

import numpy as np


def edge_positions(raw, width, height):
    pixels = np.frombuffer(raw, dtype=np.uint8).reshape(height, width, 4)[height // 2]
    b, g, r = pixels[:, 0], pixels[:, 1], pixels[:, 2]
    red = np.flatnonzero((r > 160) & (g < 80) & (b < 100))
    blue = np.flatnonzero((b > 180) & (r > 70) & (r < 190) & (g > 110) & (g < 215))
    assert len(red) and len(blue), "witness must contain actual image and overlay edges"
    return {"red": int(red[0]), "blue": float(blue.mean())}


def audit(result, witnesses):
    region = result["region"]
    width, height = region["width"], region["height"]
    assert 1 <= width <= 256 and 1 <= height <= 32
    observations, events = result["observations"], result["events"]
    assert len(observations) == len(events) == 30, "complete fixed cohort required"
    size = width * height * 4
    assert len(witnesses) == len(observations) * 2 * size, "complete before/response witnesses required"
    values, previous = [], 0
    for index, (sample, event) in enumerate(zip(observations, events)):
        direction = 1 if index % 2 == 0 else -1
        assert sample["index"] == index and sample["deltaY"] == direction * 120
        assert event["trusted"] is True and event["deltaY"] == sample["deltaY"]
        begin, end = int(sample["dispatch_ns"]), int(sample["response_arrival_ns"])
        assert previous < begin <= end, "monotonic dispatch/response order required"
        previous = end
        assert sample["before_frame"] < sample["response_frame"]
        before = edge_positions(witnesses[index * 2 * size:(index * 2 + 1) * size], width, height)
        after = edge_positions(witnesses[(index * 2 + 1) * size:(index * 2 + 2) * size], width, height)
        assert before == sample["before"] and after == sample["after"], "edge metadata differs from raw pixels"
        assert all((after[k] - before[k]) * direction >= 2 for k in ("red", "blue"))
        upper = (end - begin) / 1e6
        assert math.isclose(upper, sample["upper_bound_ms"], abs_tol=1e-9)
        values.append(upper)
    p95 = sorted(values)[math.ceil(len(values) * 0.95) - 1]
    assert math.isclose(p95, result["p95_upper_bound_ms"], abs_tol=1e-9)
    assert (p95 <= 50) == result["within_50_ms"]
    return {"complete": True, "actions": len(values), "p95_upper_bound_ms": p95,
            "within_50_ms": p95 <= 50,
            "interpretation": "Conservative bound passes only when <=50ms; a larger bound alone does not prove actual latency exceeds 50ms"}
