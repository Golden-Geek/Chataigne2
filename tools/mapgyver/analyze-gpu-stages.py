"""Recompute an opt-in GPU boundary diagnostic; never declare cadence qualification."""

import argparse
import bisect
import csv
import hashlib
import io
import json
from pathlib import Path
import runpy

HERE = Path(__file__).resolve().parent
PRESENTATION = runpy.run_path(str(HERE / "analyze-presentation.py"))
PIPELINE = runpy.run_path(str(HERE / "analyze-pipeline.py"))
STAGES = runpy.run_path(str(HERE / "gpu_stages.py"))
IMPORTS = runpy.run_path(str(HERE / "import_metrics.py"))
require = PRESENTATION["require"]


def analyze_capture(path):
    record, summary = PRESENTATION["analyze_capture"](path)
    packet = PRESENTATION["BASE"]["decode_evidence"](json.loads(path.read_bytes()))
    enabled = packet["requested_gpu_stages"]
    require(type(enabled) is bool and record["workload"]["gpu_stages"] is enabled,
            "requested and native GPU stage modes differ")
    require(summary["complete_capture_observations"], "capture observations incomplete")
    require(record["requested_policy"] == "latest" and record["workload"]["warmup_seconds"] == 30,
            "predeclared policy or warmup differs")
    require(packet["collector_tail_seconds"] == 10
            and packet["presentmon_timed_seconds"] == record["workload"]["sample_seconds"] + 40,
            "predeclared collector tail differs")
    generation = packet["native_report"]["generations"][0]
    PRESENTATION["RELEASE"]["verify_operations"](generation)
    gate = generation["gpu_admission"]
    timing = generation["timing"]
    PIPELINE["verify_trace"](timing, gate)
    require(all(type(timing["gpu_trace"][key]) is int and timing["gpu_trace"][key] == 0
                for key in ("skipped", "invalid", "pending", "truncated")),
            "whole-frame GPU trace has incomplete coverage")
    imports = IMPORTS["validate_imports"](generation["renderer_source_imports"])
    require(all(item["all"]["polls"] == gate["submitted"]
                and item["sampled"]["polls"] == gate["sampled"]["admitted"] for item in imports),
            "import polls differ from admitted renderer batches")
    stages = STAGES["analyze"](generation, enabled)

    # CPU submission brackets associate observed CSV Present calls, not GPU execution clocks.
    spans = sorted(timing["gpu_trace"]["rows"], key=lambda row: row[1])
    starts = [row[1] for row in spans]
    require(starts == sorted(set(starts)), "renderer submission QPC is duplicated")
    role_qpcs = {role: [] for role in ("editor", "native_output")}
    correlation = packet["role_correlation"]["roles"]
    for row in csv.DictReader(io.StringIO(packet["presentmon"]["stdout"])):
        address, qpc = hex(int(row["SwapChainAddress"], 16)), int(row["QPCTime"])
        matches = [role for role in correlation if address in role["addresses"].values()
                   and role["begin_qpc"] <= qpc < role["end_qpc"]]
        require(len(matches) == 1, "CSV Present lacks one native role")
        role_qpcs[matches[0]["role"]].append(qpc)
    for values in role_qpcs.values():
        values.sort()
    by_frame = {row[0]: index for index, row in enumerate(spans)}
    for frame in stages.get("top_sampled_frames", []):
        index = by_frame[frame["frame"]]
        begin = starts[index]
        end = starts[index + 1] if index + 1 < len(starts) else record["native_present_interval_qpc"]["end"]
        frame["cpu_submission_bracket_qpc"] = [begin, end]
        frame["csv_present_rows_in_cpu_bracket"] = {
            role: bisect.bisect_left(values, end) - bisect.bisect_left(values, begin)
            for role, values in role_qpcs.items()}

    operations = summary.pop("renderer_operations")
    summary["renderer_operations"] = dict(
        sampled_by_operation=dict(zip(operations["operation_names"], operations["sampled_by_operation"], strict=True)),
        truncated_operation_samples=operations["truncated_operation_samples"],
        outliers_by_window={window: {key: operations[window]["outliers"][key] for key in ("total", "omitted")}
                            for window in ("warmup", "sampled")})
    summary["renderer_source_imports"] = imports
    summary["gpu_stages"] = stages
    record["gpu_stages"] = enabled
    return record, summary


def analyze(paths):
    require(len(paths) in (3, 4), "expected off/on/off controls and optional sustained staged capture")
    records, summaries = zip(*(analyze_capture(path) for path in paths), strict=True)
    expected_modes = [False, True, False, True][:len(paths)]
    for index, record in enumerate(records):
        require(record["gpu_stages"] is expected_modes[index]
                and record["workload"]["sample_seconds"] == (120 if index == 3 else 8),
                "predeclared stage mode or duration sequence differs")
        for key in ("inputs", "adapter", "window_context", "qpc_frequency"):
            require(record[key] == records[0][key], f"frozen {key} differs")
        workload = {key: value for key, value in record["workload"].items()
                    if key not in ("sample_seconds", "gpu_stages")}
        require(workload == {key: value for key, value in records[0]["workload"].items()
                            if key not in ("sample_seconds", "gpu_stages")}, "other workload settings differ")
        if index:
            require(records[index - 1]["native_present_interval_qpc"]["end"]
                    < record["native_present_interval_qpc"]["begin"], "captures overlap or are out of order")
    require(len({record["session_name"] for record in records}) == len(records), "session identity repeated")
    return dict(
        schema_version=1, evidence_kind="same_context_gpu_boundary_diagnostic_not_qualification",
        verification=dict(exact_capture_recomputation=True, complete_stage_and_context_coverage=True,
                          matching_frozen_inputs_and_context=True, distinct_chronological_sessions=True,
                          operation_admission_and_import_invariants=True),
        analyzer_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        stage_validator_sha256=hashlib.sha256((HERE / "gpu_stages.py").read_bytes()).hexdigest(),
        shared_inputs=records[0]["inputs"], runs=list(summaries),
        limits=["Off/on/off short controls observe perturbation and drift, not statistical equivalence",
                "Elapsed boundary intervals include command-delivery gaps, dependencies and scheduling",
                "No GPU busy-time, cross-context critical-path, scanout or causal-repair claim",
                "CSV Present rows in CPU brackets are observations, not GPU stage execution timestamps",
                "Live zero counters do not establish final capture or analysis zero loss",
                "No M01 gate or required sustained qualification series is completed by this diagnostic"])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("captures", nargs="+", type=Path)
    args = parser.parse_args()
    print(json.dumps(analyze(args.captures), indent=2, allow_nan=False))


if __name__ == "__main__":
    main()
