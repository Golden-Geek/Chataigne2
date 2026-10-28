"""Strict v1 CSV observations, with unlabeled swapchains and separate present/display windows."""

import csv
from dataclasses import dataclass
from decimal import Decimal, InvalidOperation
from fractions import Fraction
import hashlib
import io
import math
import re

_REQUIRED = ("processid", "swapchainaddress", "qpctime", "dropped", "msuntildisplayed", "msbetweendisplaychange")
_UINT = re.compile(r"[0-9]+\Z")
_ADDRESS = re.compile(r"0[xX]([0-9a-fA-F]+)\Z")
_LOSS = re.compile(r"warning:\s*([0-9]+)\s+ETW\s+(events|buffers)\s+were\s+lost\.", re.I)
_MAX_U64 = (1 << 64) - 1


def _require(condition, message):
    if not condition:
        raise ValueError(message)


def _integer(text, label, maximum=_MAX_U64):
    _require(isinstance(text, str) and len(text) <= len(str(maximum))
             and _UINT.fullmatch(text) is not None, f"{label} must be an integer")
    value = int(text)
    _require(0 < value <= maximum, f"{label} is out of range")
    return value


def _number(text, label):
    if text.strip().casefold() in ("", "na", "n/a"):
        return None
    _require(len(text) <= 128, f"{label} is oversized")
    try:
        value = Decimal(text)
    except InvalidOperation as error:
        raise ValueError(f"{label} is not numeric") from error
    _require(value.is_finite() and abs(value.as_tuple().exponent) <= 1024, f"{label} must be finite")
    _require(math.isfinite(float(value)), f"{label} is outside the supported numeric range")
    return Fraction(value)


def _window(config):
    try:
        values = [config[k] for k in ("start_qpc", "frequency", "warmup_seconds", "maximum_seconds")]
    except (KeyError, TypeError) as error:
        raise ValueError("native clock/window fields are missing") from error
    _require(all(type(v) is int for v in values), "native clock/window values must be integers")
    start, frequency, warmup, maximum = values
    _require(0 < start <= _MAX_U64 and 0 < frequency <= _MAX_U64
             and 0 <= warmup < maximum <= 3600, "invalid native clock/window")
    begin, end = start + warmup * frequency, start + maximum * frequency
    _require(end <= _MAX_U64, "native clock/window overflow")
    return begin, end, frequency


@dataclass(slots=True)
class _Row:
    raw: tuple
    qpc: int
    dropped: bool
    display_qpc: Fraction | None
    reported_interval_ms: Fraction | None
    positive_dropped_timing: bool


def _read(csv_text, expected_pid, frequency):
    _require(isinstance(csv_text, str) and 0 < len(csv_text) <= 128 * 1024 * 1024, "empty or oversized CSV")
    _require(type(expected_pid) is int and 0 < expected_pid <= 0xffffffff, "invalid target PID")
    reader = csv.reader(io.StringIO(csv_text.removeprefix("\ufeff"), newline=""), strict=True)
    try:
        headers = next(reader)
        canonical = [h.strip().casefold() for h in headers]
        _require(headers and all(canonical) and len(set(canonical)) == len(canonical), "empty or duplicate CSV headers")
        _require(all(c in canonical for c in _REQUIRED), "required v1 CSV columns are missing")
        columns = {name: i for i, name in enumerate(canonical)}
        groups = {}
        total = 0
        for fields in reader:
            total += 1
            _require(total <= 1_000_000 and len(fields) == len(headers), f"malformed CSV row {total}")
            pid = _integer(fields[columns["processid"]], "ProcessID", 0xffffffff)
            _require(pid == expected_pid, f"unexpected PID {pid}")
            address = _ADDRESS.fullmatch(fields[columns["swapchainaddress"]])
            _require(address is not None and len(address[1]) <= 16
                     and 0 < int(address[1], 16) <= _MAX_U64, "invalid swapchain address")
            chain = hex(int(address[1], 16))
            qpc = _integer(fields[columns["qpctime"]], "QPCTime")
            rows = groups.setdefault(chain, [])
            _require(not rows or qpc > rows[-1].qpc, f"non-increasing QPC in swapchain {chain}")
            dropped = fields[columns["dropped"]]
            _require(dropped in ("0", "1"), "Dropped must be 0 or 1")
            # Validate known timing fields while retaining every original field in each row.
            for name, index in columns.items():
                if name.startswith("ms") or name in ("timeinseconds", "displaylatency", "displayedtime"):
                    _number(fields[index], headers[index])
            until = _number(fields[columns["msuntildisplayed"]], "MsUntilDisplayed")
            interval = _number(fields[columns["msbetweendisplaychange"]], "MsBetweenDisplayChange")
            _require(until is None or until >= 0, "negative MsUntilDisplayed")
            _require(interval is None or interval >= 0, "negative MsBetweenDisplayChange")
            displayed = qpc + until * frequency / 1000 if dropped == "0" and until is not None and until > 0 else None
            rows.append(_Row(tuple(fields), qpc, dropped == "1", displayed,
                             interval if interval is not None and interval > 0 else None,
                             dropped == "1" and until is not None and until > 0))
        _require(total > 0, "CSV contains no frame rows")
        return headers, groups, total
    except (csv.Error, StopIteration) as error:
        raise ValueError("malformed or missing CSV data") from error


def _distribution(values):
    if not values:
        return None
    ordered = sorted(values)

    def numeric(value):
        result = float(value)
        _require(math.isfinite(result) and (result != 0 or value == 0), "timing summary is not representable")
        return result

    def rank(percent):
        return numeric(ordered[(percent * len(ordered) + 99) // 100 - 1])

    return dict(samples=len(ordered), mean_ms=numeric(sum(ordered) / len(ordered)),
                median_ms=rank(50), p95_ms=rank(95), p99_ms=rank(99), maximum_ms=numeric(ordered[-1]))


def _chain(address, rows, begin, end, frequency):
    present = [r for r in rows if begin <= r.qpc < end]
    observed_intervals, reported_intervals = [], []
    display_events = missing_predecessor = outside_predecessor = arrivals = tails = 0
    non_increasing = dropped_timing = 0
    first_display = last_display = previous_display = None
    for row in rows:
        dropped_timing += row.positive_dropped_timing
        display = row.display_qpc
        if display is None:
            continue
        if previous_display is not None and display <= previous_display:
            non_increasing += 1
        if begin <= row.qpc < end and display >= end:
            tails += 1
        if begin <= display < end:
            display_events += 1
            arrivals += row.qpc < begin
            first_display = display if first_display is None else first_display
            last_display = display
            if row.reported_interval_ms is not None:
                reported_intervals.append(row.reported_interval_ms)
            if previous_display is None:
                missing_predecessor += 1
            elif display > previous_display:
                observed_intervals.append((display - previous_display) * 1000 / frequency)
                outside_predecessor += previous_display < begin
        previous_display = display
    return dict(swapchain_address=address, label="unlabeled", raw_row_count=len(rows),
                first_present_qpc=rows[0].qpc, last_present_qpc=rows[-1].qpc,
                rows_before_present_window=sum(r.qpc < begin for r in rows),
                rows_after_present_window=sum(r.qpc >= end for r in rows),
                present_window=dict(presents=len(present), dropped=sum(r.dropped for r in present),
                    non_dropped=sum(not r.dropped for r in present),
                    unavailable_display_timing=sum(not r.dropped and r.display_qpc is None for r in present),
                    usable_display_timestamps=sum(r.display_qpc is not None for r in present),
                    display_tails_after_window=tails),
                display_window=dict(events=display_events, arrivals_from_before_present_window=arrivals,
                    missing_predecessor=missing_predecessor, intervals_using_before_window_predecessor=outside_predecessor,
                    first_display_qpc_exact=None if first_display is None else str(first_display),
                    last_display_qpc_exact=None if last_display is None else str(last_display),
                    observed_display_intervals_ms=_distribution(observed_intervals),
                    reported_display_change_intervals_ms=_distribution(reported_intervals)),
                non_increasing_display_pairs_all_rows=non_increasing,
                dropped_with_positive_display_time_all_rows=dropped_timing)


def analyze(csv_text, expected_pid, config):
    """Summarize software-visible display observations; caller retains the unchanged raw CSV."""
    begin, end, frequency = _window(config)
    headers, groups, count = _read(csv_text, expected_pid, frequency)
    return dict(schema_version=1,
                evidence_kind="presentmon_v1_unlabeled_software_display_observations_not_qualification",
                scope="PID/swapchain observations only; no editor/output role, 60-fps, scanout, optical latency or full M01 claim",
                expected_pid=expected_pid, columns=headers, raw_rows=count,
                csv_text_utf8_sha256=hashlib.sha256(csv_text.encode("utf-8")).hexdigest(),
                sample_present_window_qpc=dict(begin=begin, end=end, frequency=frequency),
                interval_percentiles="nearest rank; first in-window display can use a retained pre-window predecessor",
                chains=[_chain(address, rows, begin, end, frequency) for address, rows in groups.items()])


def loss_observation(stdout, stderr):
    """Absence of collector warnings is not an independent zero-loss measurement."""
    warnings = []
    for stream, text in (("stdout", stdout), ("stderr", stderr)):
        for number, line in enumerate(text.splitlines(), 1):
            match = _LOSS.search(line)
            suspect = re.search(r"warning|error", line, re.I) and re.search(r"lost|overflow", line, re.I) \
                and re.search(r"etw|events|buffers|presents", line, re.I)
            if match or suspect:
                warnings.append(dict(stream=stream, line_number=number, text=line,
                    kind=None if match is None else match[2].lower(),
                    reported_count=None if match is None else int(match[1])))
    reported = any(w["reported_count"] is None or w["reported_count"] > 0 for w in warnings)
    return dict(no_loss_reported=not reported, zero_loss_independently_verified=False,
                scope="collector warning observations only; absence does not prove zero ETW or analysis loss",
                loss_warnings=warnings,
                reported_event_loss_counts=[w["reported_count"] for w in warnings if w["kind"] == "events"],
                reported_buffer_loss_counts=[w["reported_count"] for w in warnings if w["kind"] == "buffers"])
