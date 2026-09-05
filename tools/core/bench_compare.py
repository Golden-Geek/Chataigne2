#!/usr/bin/env python3
"""Validate and compare complete Criterion bencher evidence.

Exit codes:
    0: complete, comparable evidence with no failure-threshold regression
    1: at least one comparable scenario exceeded its failure threshold
    2: missing, malformed, incomplete, or incomparable evidence
"""

from __future__ import annotations

import argparse
import json
import math
import re
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any

SCHEMA_VERSION = 1
MEASUREMENT_UNIT = "ns/iter"
BENCH_LINE = re.compile(
    r"^test (?P<name>.+?) \.\.\. bench:\s+"
    r"(?P<value>[0-9]+(?:,[0-9]{3})*)\s+"
    r"(?P<unit>\S+)"
    r"(?:\s+\(\+/-\s+[0-9]+(?:,[0-9]{3})*\))?\s*$"
)
ANSI_ESCAPE = re.compile(r"\x1b\[[0-9;]*m")


class EvidenceError(ValueError):
    """Raised when benchmark evidence cannot support a comparison."""


@dataclass(frozen=True)
class Fingerprint:
    host: str
    toolchain: str
    profile: str
    features: tuple[str, ...]

    @classmethod
    def from_value(cls, value: Any, source: str) -> Fingerprint:
        if not isinstance(value, dict):
            raise EvidenceError(f"{source} fingerprint must be an object")
        required = {"host", "toolchain", "profile", "features"}
        missing = sorted(required - value.keys())
        extra = sorted(value.keys() - required)
        if missing or extra:
            raise EvidenceError(
                f"{source} fingerprint fields differ from the schema "
                f"(missing={missing}, extra={extra})"
            )

        strings: dict[str, str] = {}
        for key in ("host", "toolchain", "profile"):
            item = value[key]
            if not isinstance(item, str) or not item.strip():
                raise EvidenceError(f"{source} fingerprint {key!r} must be a non-empty string")
            strings[key] = item.strip()

        features = value["features"]
        if not isinstance(features, list) or not features:
            raise EvidenceError(f"{source} fingerprint features must be a non-empty list")
        if any(not isinstance(feature, str) or not feature.strip() for feature in features):
            raise EvidenceError(f"{source} fingerprint features must contain non-empty strings")
        normalized_features = tuple(sorted(feature.strip() for feature in features))
        if len(set(normalized_features)) != len(normalized_features):
            raise EvidenceError(f"{source} fingerprint features must be unique")

        return cls(
            host=strings["host"],
            toolchain=strings["toolchain"],
            profile=strings["profile"],
            features=normalized_features,
        )


@dataclass(frozen=True)
class ScenarioBaseline:
    p50_ns: float
    p95_ns: float
    warning_fraction: float
    failure_fraction: float


@dataclass(frozen=True)
class Baseline:
    comparison_status: str
    qualification_note: str
    fingerprint: Fingerprint
    expected_scenarios: tuple[str, ...]
    scenarios: dict[str, ScenarioBaseline]


def _load_json(path: Path, description: str) -> Any:
    try:
        text = path.read_text(encoding="utf-8")
    except OSError as error:
        raise EvidenceError(f"cannot read {description} {path}: {error}") from error
    if not text.strip():
        raise EvidenceError(f"{description} {path} is empty")
    try:
        return json.loads(text)
    except json.JSONDecodeError as error:
        raise EvidenceError(f"{description} {path} is malformed JSON: {error}") from error


def _positive_finite(value: Any, label: str) -> float:
    if isinstance(value, bool):
        raise EvidenceError(f"{label} must be a positive finite number")
    try:
        number = float(value)
    except (TypeError, ValueError) as error:
        raise EvidenceError(f"{label} must be a positive finite number") from error
    if not math.isfinite(number) or number <= 0:
        raise EvidenceError(f"{label} must be a positive finite number")
    return number


def _percentage(value: Any, label: str) -> float:
    if isinstance(value, bool):
        raise EvidenceError(f"{label} must be a finite percentage")
    try:
        number = float(value)
    except (TypeError, ValueError) as error:
        raise EvidenceError(f"{label} must be a finite percentage") from error
    if not math.isfinite(number) or number < 0 or number > 100:
        raise EvidenceError(f"{label} must be between 0 and 100")
    return number / 100.0


def load_fingerprint(path: Path) -> Fingerprint:
    data = _load_json(path, "current fingerprint")
    if not isinstance(data, dict) or data.get("schema_version") != SCHEMA_VERSION:
        raise EvidenceError(
            f"current fingerprint must use schema_version {SCHEMA_VERSION}"
        )
    return Fingerprint.from_value(data.get("fingerprint"), "current")


def load_baseline(path: Path) -> Baseline:
    data = _load_json(path, "baseline")
    if not isinstance(data, dict) or data.get("schema_version") != SCHEMA_VERSION:
        raise EvidenceError(f"baseline must use schema_version {SCHEMA_VERSION}")

    comparison_status = data.get("comparison_status")
    if comparison_status not in {"qualified", "unqualified"}:
        raise EvidenceError("baseline comparison_status must be 'qualified' or 'unqualified'")
    qualification_note = data.get("qualification_note", "")
    if not isinstance(qualification_note, str):
        raise EvidenceError("baseline qualification_note must be a string")
    if comparison_status == "unqualified" and not qualification_note.strip():
        raise EvidenceError("an unqualified baseline must explain the qualification gap")

    if data.get("measurement_unit") != MEASUREMENT_UNIT:
        raise EvidenceError(f"baseline measurement_unit must be {MEASUREMENT_UNIT!r}")

    expected = data.get("expected_scenarios")
    if not isinstance(expected, list) or not expected:
        raise EvidenceError("baseline expected_scenarios must be a non-empty list")
    if any(not isinstance(name, str) or not name.strip() for name in expected):
        raise EvidenceError("baseline expected_scenarios must contain non-empty strings")
    normalized_expected = tuple(name.strip() for name in expected)
    if len(set(normalized_expected)) != len(normalized_expected):
        raise EvidenceError("baseline expected_scenarios must be unique")

    raw_scenarios = data.get("scenarios")
    if not isinstance(raw_scenarios, dict):
        raise EvidenceError("baseline scenarios must be an object")
    expected_set = set(normalized_expected)
    scenario_set = set(raw_scenarios)
    if scenario_set != expected_set:
        raise EvidenceError(
            "baseline scenarios do not match expected_scenarios "
            f"(missing={sorted(expected_set - scenario_set)}, "
            f"extra={sorted(scenario_set - expected_set)})"
        )

    scenarios: dict[str, ScenarioBaseline] = {}
    for name in normalized_expected:
        entry = raw_scenarios[name]
        if not isinstance(entry, dict):
            raise EvidenceError(f"baseline scenario {name!r} must be an object")
        if entry.get("unit") != MEASUREMENT_UNIT:
            raise EvidenceError(
                f"baseline scenario {name!r} unit must be {MEASUREMENT_UNIT!r}"
            )
        p50_ns = _positive_finite(entry.get("p50_ns"), f"{name}.p50_ns")
        p95_ns = _positive_finite(entry.get("p95_ns"), f"{name}.p95_ns")
        if p95_ns < p50_ns:
            raise EvidenceError(f"{name}.p95_ns must be greater than or equal to p50_ns")

        thresholds = entry.get("thresholds")
        if not isinstance(thresholds, dict):
            raise EvidenceError(f"{name}.thresholds must be an object")
        if set(thresholds) != {"warning_pct", "failure_pct"}:
            raise EvidenceError(
                f"{name}.thresholds must contain only warning_pct and failure_pct"
            )
        warning_fraction = _percentage(
            thresholds["warning_pct"], f"{name}.thresholds.warning_pct"
        )
        failure_fraction = _percentage(
            thresholds["failure_pct"], f"{name}.thresholds.failure_pct"
        )
        if warning_fraction > failure_fraction:
            raise EvidenceError(
                f"{name} warning threshold must not exceed its failure threshold"
            )
        scenarios[name] = ScenarioBaseline(
            p50_ns=p50_ns,
            p95_ns=p95_ns,
            warning_fraction=warning_fraction,
            failure_fraction=failure_fraction,
        )

    return Baseline(
        comparison_status=comparison_status,
        qualification_note=qualification_note.strip(),
        fingerprint=Fingerprint.from_value(data.get("fingerprint"), "baseline"),
        expected_scenarios=normalized_expected,
        scenarios=scenarios,
    )


def parse_bencher_output(text: str) -> dict[str, float]:
    if not text.strip():
        raise EvidenceError("benchmark results are empty")

    results: dict[str, float] = {}
    malformed_lines: list[str] = []
    for raw_line in text.splitlines():
        line = ANSI_ESCAPE.sub("", raw_line).strip()
        match = BENCH_LINE.fullmatch(line)
        if match is None:
            if "bench:" in line:
                malformed_lines.append(line)
            continue

        name = match.group("name").strip()
        unit = match.group("unit")
        if unit != MEASUREMENT_UNIT:
            raise EvidenceError(
                f"scenario {name!r} uses {unit!r}; expected {MEASUREMENT_UNIT!r}"
            )
        if name in results:
            raise EvidenceError(f"duplicate benchmark scenario {name!r}")
        value = float(match.group("value").replace(",", ""))
        if not math.isfinite(value) or value <= 0:
            raise EvidenceError(
                f"scenario {name!r} measurement must be a positive finite number"
            )
        results[name] = value

    if malformed_lines:
        preview = "; ".join(repr(line) for line in malformed_lines[:3])
        raise EvidenceError(f"malformed benchmark measurement line(s): {preview}")
    if not results:
        raise EvidenceError("benchmark results contain no measurements")
    return results


def load_results(path: Path) -> dict[str, float]:
    try:
        text = path.read_text(encoding="utf-8")
    except OSError as error:
        raise EvidenceError(f"cannot read benchmark results {path}: {error}") from error
    return parse_bencher_output(text)


def _fingerprint_difference(
    baseline: Fingerprint, current: Fingerprint
) -> list[str]:
    differences: list[str] = []
    for field in ("host", "toolchain", "profile", "features"):
        baseline_value = getattr(baseline, field)
        current_value = getattr(current, field)
        if baseline_value != current_value:
            differences.append(
                f"{field}: baseline={baseline_value!r}, current={current_value!r}"
            )
    return differences


def compare(
    baseline: Baseline, current_fingerprint: Fingerprint, results: dict[str, float]
) -> tuple[str, int]:
    if baseline.comparison_status != "qualified":
        raise EvidenceError(
            f"baseline is unqualified and cannot be used as a performance gate: "
            f"{baseline.qualification_note}"
        )

    differences = _fingerprint_difference(baseline.fingerprint, current_fingerprint)
    if differences:
        raise EvidenceError(
            "benchmark fingerprints are incomparable (" + "; ".join(differences) + ")"
        )

    expected = set(baseline.expected_scenarios)
    actual = set(results)
    if actual != expected:
        raise EvidenceError(
            "benchmark result scenarios do not match the explicit expected set "
            f"(missing={sorted(expected - actual)}, extra={sorted(actual - expected)})"
        )

    lines = [
        "## Benchmark results",
        "",
        "| Scenario | Baseline (ns/iter) | Current (ns/iter) | Delta | Status |",
        "|---|---:|---:|---:|---|",
    ]
    regressions: list[str] = []
    warnings: list[str] = []
    for name in baseline.expected_scenarios:
        scenario = baseline.scenarios[name]
        current_ns = results[name]
        delta_fraction = (current_ns - scenario.p50_ns) / scenario.p50_ns
        if delta_fraction > scenario.failure_fraction:
            status = (
                f"FAIL (>{scenario.failure_fraction:.0%} failure threshold)"
            )
            regressions.append(
                f"- `{name}`: {delta_fraction:+.1%} "
                f"(failure threshold {scenario.failure_fraction:.0%})"
            )
        elif delta_fraction > scenario.warning_fraction:
            status = (
                f"WARN (>{scenario.warning_fraction:.0%} warning threshold)"
            )
            warnings.append(
                f"- `{name}`: {delta_fraction:+.1%} "
                f"(warning threshold {scenario.warning_fraction:.0%})"
            )
        elif delta_fraction < -0.05:
            status = "improvement"
        else:
            status = "ok"
        lines.append(
            f"| `{name}` | {scenario.p50_ns:,.0f} | {current_ns:,.0f} | "
            f"{delta_fraction:+.1%} | {status} |"
        )

    if warnings:
        lines.extend(["", "### Warnings", "", *warnings])
    if regressions:
        lines.extend(["", "### Regressions", "", *regressions])
        exit_code = 1
    else:
        lines.extend(["", "No failure-threshold regressions detected."])
        exit_code = 0
    return "\n".join(lines) + "\n", exit_code


def invalid_summary(error: EvidenceError) -> str:
    return (
        "## Benchmark evidence invalid\n\n"
        f"Comparison did not run: {error}\n"
    )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--results", required=True, type=Path)
    parser.add_argument("--baseline", required=True, type=Path)
    parser.add_argument("--fingerprint", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()

    try:
        baseline = load_baseline(args.baseline)
        fingerprint = load_fingerprint(args.fingerprint)
        results = load_results(args.results)
        summary, exit_code = compare(baseline, fingerprint, results)
    except EvidenceError as error:
        summary = invalid_summary(error)
        exit_code = 2
        print(f"INVALID: {error}", file=sys.stderr)

    try:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(summary, encoding="utf-8", newline="\n")
    except OSError as error:
        print(f"INVALID: cannot write benchmark summary {args.output}: {error}", file=sys.stderr)
        return 2

    print(summary)
    if exit_code == 1:
        print("FAIL: benchmark regression threshold exceeded.", file=sys.stderr)
    return exit_code


if __name__ == "__main__":
    sys.exit(main())
