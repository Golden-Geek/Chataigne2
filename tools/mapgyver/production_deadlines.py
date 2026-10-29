"""Exact scheduler-slot/software-API joins; no display or scanout inference."""

from collections import Counter, defaultdict
import json

NANOS = 1_000_000_000
MAX_U64 = (1 << 64) - 1
MISS_BUDGET = 0.001


class MappingError(ValueError):
    pass


def _require(value, reason):
    if not value:
        raise MappingError(reason)


def _uint(value, label, minimum=0, maximum=MAX_U64):
    _require(type(value) is int and minimum <= value <= maximum, f"invalid {label}")
    return value


def _ceil(value, divisor):
    return (value + divisor - 1) // divisor


def _identity(value):
    return json.dumps(value, sort_keys=True, separators=(",", ":"))


def _slot(anchor, index, numerator, denominator, origin, frequency):
    start = _uint(anchor + _ceil(index * NANOS * denominator, numerator), "slot start nanoseconds")
    end = _uint(anchor + _ceil((index + 1) * NANOS * denominator, numerator), "slot end nanoseconds")
    start_qpc = _uint(origin + _ceil(start * frequency, NANOS), "slot start QPC")
    end_qpc = _uint(origin + _ceil(end * frequency, NANOS), "slot deadline QPC")
    _require(start_qpc < end_qpc, "counter resolution cannot distinguish adjacent slot boundaries")
    return start, end, start_qpc, end_qpc


def _mapped(row, frequency):
    index = _uint(row["frame_index"], "frame index")
    anchor = _uint(row["anchor_monotonic_ns"], "cadence anchor")
    observed = _uint(row["observed_monotonic_ns"], "clock observation", maximum=MAX_U64 - 1)
    numerator = _uint(row["rate_numerator"], "rate numerator", 1, (1 << 32) - 1)
    denominator = _uint(row["rate_denominator"], "rate denominator", 1, (1 << 32) - 1)
    _require((numerator, denominator) == (60, 1), "fixed workload requires normalized60/1 rate")
    disposition = row["disposition"]
    _require(disposition in {"Submitted", "Busy", "NoDemand", "Rejected", "Fence"}, "unknown schedule disposition")
    clock = row["clock"]["Qpc"]
    origin = _uint(clock["origin_qpc"], "QPC origin", 1)
    _require(_uint(clock["frequency"], "clock frequency", 1) == frequency, "schedule QPC frequency differs")
    start, end, start_qpc, end_qpc = _slot(anchor, index, numerator, denominator, origin, frequency)
    _require((row["slot_start_monotonic_ns"], row["slot_end_monotonic_ns"]) == (start, end), "slot monotonic boundaries differ")
    anchor_qpc = _uint(origin + _ceil(anchor * frequency, NANOS), "anchor QPC")
    lower = _uint(origin + _ceil(observed * frequency, NANOS), "observation lower QPC")
    upper = _uint(origin + _ceil((observed + 1) * frequency, NANOS) - 1, "observation upper QPC")
    _require(lower <= upper, "impossible floor-quantized clock observation")
    declared = (clock["anchor_qpc"], clock["slot_start_qpc"], clock["slot_end_qpc"],
                clock["observed_qpc_lower"], clock["observed_qpc_upper"])
    _require(declared == (anchor_qpc, start_qpc, end_qpc, lower, upper), "QPC projection differs from checked ceil inverse")
    if disposition != "Fence":
        _require(start <= observed < end, "consumed opportunity is outside its declared slot")
    skipped = _uint(row["skipped_slots"], "skipped slots", maximum=index)
    coalesced = _uint(row["coalesced_slots"], "coalesced slots", maximum=skipped)
    _uint(row["epoch_generation"], "show epoch generation")
    _require(isinstance(row["cadence"], str) and row["cadence"], "missing cadence identity")
    if disposition == "Submitted":
        _require(isinstance(row["token"], dict) and row["token"]["device"] == row["device"], "submitted token has another device")
        _uint(row["token"]["serial"], "submitted batch serial", 1)
    else:
        _require(row["token"] is None, "unsubmitted schedule unexpectedly owns a token")
    if row["recorded_qpc"] is not None:
        _uint(row["recorded_qpc"], "recorded QPC", lower)
    return {"row": row, "index": index, "skipped": skipped, "coalesced": coalesced,
            "anchor": anchor, "origin": origin, "frequency": frequency, "rate": (numerator, denominator),
            "anchor_qpc": anchor_qpc, "deadline_qpc": end_qpc, "observed_lower": lower}


def _unavailable(reason, **details):
    return {"mapping_available": False, "coverage_complete": False, "reason": reason,
            "expected_slots": None, "outputs": [], "submission_verdict": None,
            "definite_failure_lower_bound_exceeds_budget": False,
            "scope": "anchored scheduler slots and native software API observations; never displayed cadence", **details}


def _expected(first, window):
    anchor, origin, frequency = first["anchor"], first["origin"], first["frequency"]
    numerator, denominator = first["rate"]
    coordinate = max(0, (window["begin_qpc"] - origin) * NANOS // frequency - anchor)
    index = max(0, coordinate * numerator // (NANOS * denominator) - 2)
    boundary = lambda ordinal: _slot(anchor, ordinal, numerator, denominator, origin, frequency)[3]
    corrections = 0
    while index and boundary(index - 1) >= window["begin_qpc"]:
        index -= 1
        corrections += 1
        _require(corrections <= 8, "unsupported clock/rate boundary quantization")
    while boundary(index) < window["begin_qpc"]:
        index += 1
        corrections += 1
        _require(corrections <= 8, "unsupported clock/rate boundary quantization")
    result = []
    while boundary(index) < window["end_qpc"]:
        _require(len(result) < 36_001, "expected-slot window exceeds bounded600-second capture")
        result.append((index, boundary(index)))
        index += 1
    _require(result, "sample window contains no anchored deadlines")
    return result


def analyze_deadlines(schedule_rows, output_rows, window, frequency, output_ids=(1, 2, 3)):
    if not schedule_rows:
        return _unavailable("no anchored scheduler rows")
    groups, invalid, unavailable = defaultdict(list), [], []
    for row in schedule_rows:
        try:
            if "Unavailable" in row["clock"]:
                unavailable.append({"recorded_qpc": row.get("recorded_qpc"), "reason": row["clock"]["Unavailable"]})
                continue
            mapped = _mapped(row, frequency)
            groups[(_identity(row["device"]), row["cadence"])].append(mapped)
        except (MappingError, KeyError, TypeError) as error:
            invalid.append({"frame_index": row.get("frame_index"), "reason": str(error)})
    if invalid:
        return _unavailable("invalid scheduler mapping", invalid_rows=invalid)
    relevant_unavailable = [row for row in unavailable if row["recorded_qpc"] is None
                            or window["begin_qpc"] <= row["recorded_qpc"] < window["end_qpc"]]
    if relevant_unavailable:
        return _unavailable("scheduler clock mapping unavailable", unavailable_rows=relevant_unavailable)
    candidates = []
    for identity, rows in groups.items():
        metadata = {(row["anchor"], row["origin"], row["frequency"], row["rate"]) for row in rows}
        if len(metadata) != 1:
            return _unavailable("cadence clock/anchor changed without a new identity")
        fences = [row for row in rows if row["row"]["disposition"] == "Fence"
                  and row["observed_lower"] >= window["end_qpc"]]
        if rows[0]["anchor_qpc"] <= window["begin_qpc"] and fences:
            candidates.append((identity, rows, min(fences, key=lambda row: row["observed_lower"])))
    if len(candidates) != 1:
        return _unavailable("one cadence with a conservative fence covering the sample is required")
    identity, rows, fence = candidates[0]
    for other, values in groups.items():
        if other != identity and any(row["row"]["disposition"] != "Fence"
                                     and window["begin_qpc"] <= row["observed_lower"] < window["end_qpc"] for row in values):
            return _unavailable("multiple cadence lifetimes overlap the sample")
    try:
        expected = _expected(rows[0], window)
    except MappingError as error:
        return _unavailable(str(error))
    consumed = {}
    ranges = []
    for row in rows:
        if row["skipped"]:
            ranges.append((row["index"] - row["skipped"], row["index"]))
        if row["row"]["disposition"] != "Fence":
            if row["index"] in consumed:
                return _unavailable("duplicate consumed slot identity")
            consumed[row["index"]] = row
    indexed_outputs = {}
    for output in output_rows:
        key = (_identity(output["token"]), output["output"])
        if key in indexed_outputs:
            return _unavailable("duplicate per-token output observation")
        indexed_outputs[key] = output
    ranges.sort()
    reports = {output: Counter() for output in output_ids}
    missing_slots, examples, slots = [], [], []
    range_index = 0
    for index, deadline in expected:
        while range_index < len(ranges) and ranges[range_index][1] <= index:
            range_index += 1
        skipped = range_index < len(ranges) and ranges[range_index][0] <= index < ranges[range_index][1]
        row = consumed.get(index)
        fenced_tail = index >= fence["index"]
        known = row is not None or skipped or fenced_tail
        slots.append({"frame_index": index, "deadline_qpc": deadline,
                      "token": row["row"]["token"] if row is not None else None,
                      "disposition": row["row"]["disposition"] if row is not None else
                                     "Skipped" if skipped else "Unconsumed" if fenced_tail else "Missing",
                      "known": known})
        if row is None and not skipped and not fenced_tail:
            missing_slots.append(index)
            continue
        if row is None or row["row"]["disposition"] != "Submitted":
            for counts in reports.values():
                counts["unsubmitted"] += 1
                counts["definite_misses"] += 1
                counts["observed_misses"] += 1
            continue
        for output, counts in reports.items():
            trace = indexed_outputs.get((_identity(row["row"]["token"]), output))
            if trace is None:
                counts["missing_output_rows"] += 1
                continue
            disposition = trace["outcome"]
            accepted = disposition == "Accepted"
            api = trace.get("consumer_api")
            if api is not None:
                try:
                    begin = _uint(api["begin"], "API begin", 1)
                    end = _uint(api["end"], "API end", begin)
                except (MappingError, KeyError, TypeError):
                    counts["invalid_api_intervals"] += 1
                    continue
                if begin > deadline:
                    counts["definite_misses"] += 1
                if accepted and end <= deadline:
                    counts["accepted_observed_on_time"] += 1
                else:
                    counts["observed_misses"] += 1
                    if len(examples) < 128:
                        examples.append({"frame_index": index, "output": output, "deadline_qpc": deadline,
                                         "consumer_api": api, "outcome": disposition})
            elif accepted:
                counts["missing_api_intervals"] += 1
            else:
                counts["observed_misses"] += 1
                counts["not_accepted"] += 1
    outputs = []
    complete = not missing_slots
    for output, counts in reports.items():
        unknown = sum(counts[name] for name in ("missing_output_rows", "invalid_api_intervals", "missing_api_intervals"))
        complete = complete and not unknown
        outputs.append({"output": output, "expected_slots": len(expected), **counts,
                        "observed_miss_fraction": counts["observed_misses"] / len(expected),
                        "definite_miss_fraction_lower_bound": counts["definite_misses"] / len(expected)})
    failed = any(row["definite_miss_fraction_lower_bound"] > MISS_BUDGET for row in outputs)
    upper_pass = complete and all(row["observed_miss_fraction"] <= MISS_BUDGET for row in outputs)
    return {"mapping_available": True, "coverage_complete": bool(complete), "expected_slots": len(expected),
            "device": rows[0]["row"]["device"], "slots": slots,
            "first_frame_index": expected[0][0], "last_frame_index": expected[-1][0],
            "cadence": identity[1], "fence_observed_qpc_lower": fence["observed_lower"],
            "missing_schedule_slots": missing_slots, "outputs": outputs, "late_examples": examples,
            "submission_verdict": False if failed else True if upper_pass else None,
            "definite_failure_lower_bound_exceeds_budget": failed,
            "scope": "deadlines in [sample begin,end); native API observations are not GPU/display completion",
            "reason": "late API-end observations alone may include blocking or post-call preemption"}
