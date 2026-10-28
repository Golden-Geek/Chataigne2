"""Validate renderer stage intervals without treating them as GPU busy or cross-clock time."""

from fractions import Fraction
import heapq
from pathlib import Path
import runpy

_PIPELINE = runpy.run_path(str(Path(__file__).with_name("analyze-pipeline.py")))
CAPACITY = _PIPELINE["CAPACITY"]
STAGES = (
    "source_handoff", "composition", "spout_send", "native_output", "editor", "tail",
)
_COUNTERS = ("skipped", "invalid", "pending", "truncated")
_FRAME_FIELDS = {
    "frame", "submitted_qpc", "observed_completed_qpc", "frequency", "collected", "boundaries",
}
_U64_MAX = (1 << 64) - 1


def _require(condition, message):
    if not condition:
        raise ValueError(message)


def _uint(value, label):
    _require(type(value) is int and 0 <= value <= _U64_MAX,
             f"{label} must be an exact u64 integer")
    return value


def _fields(value, expected, label):
    _require(isinstance(value, dict) and set(value) == expected,
             f"{label} fields differ from the native schema")


def _zero(value, label):
    _require(_uint(value, label) == 0, f"{label} must be zero for complete stage coverage")


def _distribution(values):
    # Keep subtraction, ordering and the mean exact until the JSON-facing conversion.
    result = _PIPELINE["distribution"](values)
    if result is None:
        return None
    return {key: value if key == "samples" else float(value) for key, value in result.items()}


def _context_trace(timing, admission):
    trace = timing.get("gpu_trace")
    _fields(trace, {"columns", "rows", *_COUNTERS}, "context GPU trace")
    _require(trace["columns"] == _PIPELINE["GPU_COLUMNS"], "context GPU columns changed")
    for name in _COUNTERS:
        _zero(trace[name], f"context GPU trace.{name}")
    rows = trace["rows"]
    _require(isinstance(rows, list) and len(rows) <= CAPACITY, "context GPU trace exceeds its bound")
    for row in rows:
        _require(isinstance(row, list) and len(row) == 7, "context GPU row width")
        for name, value in zip(trace["columns"][:6], row[:6], strict=True):
            _uint(value, f"context GPU row.{name}")
        _require(type(row[6]) is bool, "context GPU collected must be a boolean")
    for name in ("skipped_gpu_queries", "invalid_gpu_queries", "pending_gpu_queries", "truncated_samples"):
        _zero(timing.get(name), f"timing.{name}")
    _require("gpu_execution" in timing, "context GPU distribution is missing")
    _PIPELINE["verify_trace"](timing, admission)
    return {row[0]: row for row in rows}


def analyze(generation, enabled: bool) -> dict:
    """Analyze the current runner's compact GPU trace and unchanged native stage-row dictionaries."""
    _require(type(enabled) is bool, "stage mode must be a boolean")
    _require(isinstance(generation, dict) and isinstance(generation.get("timing"), dict),
             "generation timing is missing")
    timing = generation["timing"]
    _require("gpu_stages" in timing, "GPU stage mode observation is missing")
    trace = timing["gpu_stages"]
    if not enabled:
        _require(trace is None, "disabled GPU stages must be null")
        return {"enabled": False}

    _fields(trace, {"stage_names", "rows", *_COUNTERS}, "GPU stage trace")
    _require(trace["stage_names"] == list(STAGES), "GPU stage names or order changed")
    for name in _COUNTERS:
        _zero(trace[name], f"GPU stage trace.{name}")
    rows = trace["rows"]
    _require(isinstance(rows, list) and len(rows) <= CAPACITY, "GPU stage trace exceeds its bound")
    admission = generation.get("gpu_admission")
    _require(isinstance(admission, dict) and isinstance(admission.get("sampled"), dict),
             "GPU admission is missing")
    submitted = _uint(admission.get("submitted"), "GPU admission.submitted")
    sampled = _uint(admission["sampled"].get("admitted"), "GPU admission.sampled.admitted")
    _require(len(rows) == submitted and sampled <= submitted, "GPU stage batch coverage mismatch")
    context = _context_trace(timing, admission)

    all_values = [[] for _ in STAGES]
    sampled_values = [[] for _ in STAGES]
    seen = set()
    sampled_count = 0
    longest = []
    for row in rows:
        _fields(row, _FRAME_FIELDS, "GPU stage row")
        for name in ("frame", "submitted_qpc", "observed_completed_qpc", "frequency"):
            _uint(row[name], f"GPU stage row.{name}")
        frame = row["frame"]
        _require(frame not in seen and frame in context, "GPU stage frame is duplicate or unmatched")
        seen.add(frame)
        _require(type(row["collected"]) is bool, "GPU stage collected must be a boolean")
        _require(0 < row["submitted_qpc"] <= row["observed_completed_qpc"] and row["frequency"] > 0,
                 "GPU stage QPC bounds or frequency invalid")
        boundaries = row["boundaries"]
        _require(isinstance(boundaries, list) and len(boundaries) == len(STAGES) + 1,
                 "GPU stage row requires seven boundaries")
        for value in boundaries:
            _uint(value, "GPU stage boundary")
        _require(all(a <= b for a, b in zip(boundaries, boundaries[1:])),
                 "GPU stage boundaries are not monotonic")
        expected = [frame, row["submitted_qpc"], row["observed_completed_qpc"],
                    boundaries[0], boundaries[-1], row["frequency"], row["collected"]]
        _require(expected == context[frame], "GPU stage identity or endpoints differ from context span")
        ticks = [b - a for a, b in zip(boundaries, boundaries[1:])]
        total_ticks = boundaries[-1] - boundaries[0]
        _require(sum(ticks) == total_ticks, "GPU stage intervals do not telescope")
        durations = [Fraction(value * 1000, row["frequency"]) for value in ticks]
        for index, duration in enumerate(durations):
            all_values[index].append(duration)
            if row["collected"]:
                sampled_values[index].append(duration)
        if row["collected"]:
            sampled_count += 1
            total = Fraction(total_ticks * 1000, row["frequency"])
            rank = (total, -frame)
            if len(longest) < 10 or rank > longest[0][:2]:
                detail = {
                    "frame": frame, "submitted_qpc": row["submitted_qpc"],
                    "observed_completed_qpc": row["observed_completed_qpc"],
                    "frequency": row["frequency"], "boundaries": list(boundaries),
                    "total_ticks": total_ticks, "total_ms": float(total),
                    "stage_ticks": dict(zip(STAGES, ticks, strict=True)),
                    "stage_ms": dict(zip(STAGES, map(float, durations), strict=True)),
                    "dominant_stage": STAGES[max(range(len(STAGES)), key=ticks.__getitem__)],
                }
                item = (*rank, detail)
                if len(longest) < 10:
                    heapq.heappush(longest, item)
                else:
                    heapq.heapreplace(longest, item)

    _require(seen == set(context), "GPU stage/context frame coverage mismatch")
    _require(sampled_count == sampled, "GPU stage sampled coverage mismatch")
    longest = [row for _, _, row in sorted(longest, key=lambda item: (-item[0], -item[1]))]
    return {
        "enabled": True, "frames": len(rows), "sampled_frames": sampled_count,
        "stages": [dict(stage=name, all=_distribution(all_values[index]),
                        sampled=_distribution(sampled_values[index])) for index, name in enumerate(STAGES)],
        "top_sampled_frames": longest,
        "scope": "Elapsed intervals on one renderer immediate context, including any gaps or dependencies between markers; not GPU busy time, cross-context critical path or scanout.",
        "interpretation": "Native-output and editor intervals include target acquisition, drawing and Present when attempted. A dominant interval locates elapsed time, not its internal cause. QPC values are identity/completion bounds, never subtracted from GPU ticks.",
    }
