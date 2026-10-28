"""Validate successful renderer import reports without inferring physical GPU residency."""

import math

COUNTERS = (
    "polls", "metadata_busy", "no_publication", "same_surface", "cache_hits",
    "cache_misses", "opens", "evictions", "pruned", "cleared", "acquire_timeouts",
    "reader_changes", "reader_clears",
)
STAGES = (
    "bind_device", "metadata_snapshot", "prune_imports", "cache_lookup",
    "open_resource", "cast_mutex", "cache_insert", "acquire_key",
    "release_reader", "clear_imports",
)
_MILLISECONDS = ("mean_ms", "median_ms", "p95_ms", "p99_ms", "maximum_ms")


def _require(condition, message):
    if not condition:
        raise ValueError(message)


def _uint(value, label):
    _require(type(value) is int and value >= 0, f"{label} must be an exact nonnegative integer")
    return value


def _counts(value, label):
    _require(isinstance(value, dict) and set(value) == set(COUNTERS),
             f"{label} counter fields differ from the native schema")
    for name in COUNTERS:
        _uint(value[name], f"{label}.{name}")
    _require(value["polls"] == sum(value[name] for name in (
        "metadata_busy", "no_publication", "same_surface", "cache_hits", "cache_misses")),
        f"{label} poll partition does not reconcile")
    _require(value["cache_hits"] + value["cache_misses"]
             == value["acquire_timeouts"] + value["reader_changes"],
             f"{label} acquisition partition does not reconcile")
    _require(value["opens"] == value["cache_misses"],
             f"{label} successful opens do not match cache misses")
    return value


def _distribution(value, expected, label):
    if value is None:
        _require(expected in (None, 0), f"{label} distribution is missing")
        return 0
    _require(isinstance(value, dict)
             and set(value) == {"samples", *_MILLISECONDS},
             f"{label} distribution fields differ from the native schema")
    samples = _uint(value["samples"], f"{label}.samples")
    _require(samples > 0, f"{label} zero samples require a null distribution")
    _require(expected is None or samples == expected,
             f"{label} sample count does not match its operation count")
    for name in _MILLISECONDS:
        number = value[name]
        _require(type(number) in (int, float) and number >= 0
                 and (type(number) is int or math.isfinite(number)),
                 f"{label}.{name} must be finite and nonnegative")
    _require(value["median_ms"] <= value["p95_ms"] <= value["p99_ms"] <= value["maximum_ms"],
             f"{label} percentiles are not ordered")
    _require(value["mean_ms"] <= value["maximum_ms"],
             f"{label} mean exceeds its maximum")
    return samples


def _stage_counts(counts, sampled):
    acquisitions = counts["cache_hits"] + counts["cache_misses"]
    return {
        "bind_device": 0 if sampled else 1,
        "metadata_snapshot": counts["polls"],
        "prune_imports": acquisitions,
        "cache_lookup": acquisitions,
        "open_resource": counts["opens"],
        "cast_mutex": counts["opens"],
        "cache_insert": counts["opens"],
        "acquire_key": acquisitions,
        # A warmup reader can be released in the sample window; its acquisition
        # belongs to a different window, so only all-window releases reconcile.
        "release_reader": None if sampled else counts["reader_changes"],
        "clear_imports": counts["no_publication"] + (0 if sampled else 1),
    }


def validate_imports(imports, expected_sources=2):
    """Return the original reports after checking successful Source shutdown invariants.

    This contract is for the Source polling/finish implementation in the native harness.
    Finite tests that call Imports directly have different poll/cleanup partitions.
    """
    _uint(expected_sources, "expected_sources")
    _require(isinstance(imports, list) and len(imports) == expected_sources,
             f"renderer imports require exactly {expected_sources} source reports")
    for index, report in enumerate(imports):
        label = f"renderer imports[{index}]"
        _require(isinstance(report, dict), f"{label} report is malformed")
        for name in ("capacity", "peak_cached", "cached_after_clear",
                     "live_imports_after_clear", "peak_live_imports"):
            _uint(report.get(name), f"{label}.{name}")
        capacity = report["capacity"]
        _require(capacity == 3, f"{label} cache capacity must be three")
        _require(report["peak_cached"] <= capacity, f"{label} cache exceeded capacity")
        _require(report["cached_after_clear"] == 0 and report["live_imports_after_clear"] == 0,
                 f"{label} retained imports after clear")
        _require(report["peak_live_imports"] <= capacity + 1,
                 f"{label} live aliases exceeded the transient bound")
        all_counts = _counts(report.get("all"), f"{label}.all")
        sampled = _counts(report.get("sampled"), f"{label}.sampled")
        for name in COUNTERS:
            _require(all_counts[name] >= sampled[name],
                     f"{label}.{name} sampled count exceeds all windows")
        _require(all_counts["opens"] == sum(all_counts[name] for name in (
            "pruned", "cleared", "evictions")),
            f"{label} imported aliases do not reconcile with removal")
        stages = report.get("stages")
        _require(isinstance(stages, list) and len(stages) == len(STAGES),
                 f"{label} requires exactly ten operation stages")
        seen = set()
        expected_all = _stage_counts(all_counts, False)
        expected_sampled = _stage_counts(sampled, True)
        for stage in stages:
            _require(isinstance(stage, dict) and set(stage) == {
                "stage", "all", "sampled", "truncated_all", "truncated_sampled"},
                f"{label} stage fields differ from the native schema")
            name = stage["stage"]
            _require(isinstance(name, str) and name in STAGES and name not in seen,
                     f"{label} has an unknown or duplicate operation stage")
            seen.add(name)
            stage_label = f"{label}.{name}"
            for window in ("all", "sampled"):
                _require(_uint(stage[f"truncated_{window}"], f"{stage_label}.truncated_{window}") == 0,
                         f"{stage_label} timing samples were truncated")
            all_samples = _distribution(stage["all"], expected_all[name], f"{stage_label}.all")
            sampled_samples = _distribution(stage["sampled"], expected_sampled[name],
                                            f"{stage_label}.sampled")
            _require(all_samples >= sampled_samples,
                     f"{stage_label} sampled distribution exceeds all windows")
            if name == "release_reader":
                changes = sampled["reader_changes"]
                _require(max(0, changes - 1) <= sampled_samples <= changes + 1,
                         f"{stage_label} sampled releases violate reader conservation")
    return imports
