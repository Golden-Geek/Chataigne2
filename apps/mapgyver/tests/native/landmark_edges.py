"""Continuous red landmark boundaries for the fixed opaque native alignment fixture."""


def linear_red(pixel):
    value = pixel[0] / 255
    return value / 12.92 if value <= 0.04045 else ((value + 0.055) / 1.055) ** 2.4


def crossing(line, edge, rising):
    # A local window excludes the separate blue quad/crop stroke. Measure the midpoint
    # between the actual two plateaus in linear light, not an arbitrary encoded threshold.
    samples = [(at + 0.5, linear_red(pixel)) for at, pixel in line if abs(at + 0.5 - edge) <= 8]
    low, high = min(value for _, value in samples), max(value for _, value in samples)
    assert high - low > 0.25, "The fixed red/black landmark must have distinct plateaus"
    midpoint = (low + high) / 2
    found = []
    for (x, a), (_, b) in zip(samples, samples[1:]):
        if (a <= midpoint < b) if rising else (a >= midpoint > b):
            found.append(x + (midpoint - a) / (b - a))
    assert len(found) == 1, "One clean local landmark transition required"
    return found[0]


def linear_red_edges(image, roi, coarse):
    left, top, right, bottom = roi
    cx, cy = (left + right) // 2, (top + bottom) // 2
    horizontal = [(x, image.getpixel((x, cy))) for x in range(left, right)]
    vertical = [(y, image.getpixel((cx, y))) for y in range(top, bottom)]
    return [crossing(horizontal, coarse[0], True), crossing(horizontal, coarse[1], False),
            crossing(vertical, coarse[2], True), crossing(vertical, coarse[3], False)]
