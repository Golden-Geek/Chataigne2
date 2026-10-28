"""Recompute the predeclared cached-import diagnostic without claiming qualification."""

import argparse
import hashlib
import json
from pathlib import Path
import runpy

HERE = Path(__file__).resolve().parent
PRESENTATION = runpy.run_path(str(HERE / "analyze-presentation.py"))
METRICS = runpy.run_path(str(HERE / "import_metrics.py"))
ACCURACY = runpy.run_path(str(HERE / "run-color-accuracy.py"))
BASE = PRESENTATION["BASE"]
require = PRESENTATION["require"]


def decode(path):
    return BASE["decode_evidence"](json.loads(path.read_bytes()))


def analyze(short, sustained, accuracy):
    records, summaries = [], []
    for path, seconds in ((short, 8), (sustained, 120)):
        record, summary = PRESENTATION["analyze_capture"](path)
        packet = decode(path)
        require(summary["complete_capture_observations"], "capture observations incomplete")
        require(record["requested_policy"] == "latest"
                and record["workload"]["sample_seconds"] == seconds
                and record["workload"]["warmup_seconds"] == 30,
                "predeclared policy/duration differs")
        require(packet["collector_tail_seconds"] == 10
                and packet["presentmon_timed_seconds"] == seconds + 40,
                "predeclared collector tail differs")
        generation = packet["native_report"]["generations"][0]
        PRESENTATION["RELEASE"]["verify_operations"](generation)
        imports = METRICS["validate_imports"](generation["renderer_source_imports"])
        gate = summary["gpu_admission"]
        require(all(item["all"]["polls"] == gate["submitted"]
                    and item["sampled"]["polls"] == gate["sampled"]["admitted"] for item in imports),
                "import polls differ from admitted renderer batches")
        require(all(item["sampled"]["opens"] == 0 for item in imports),
                "warmed diagnostic contains sampled imports")
        summary["renderer_source_imports"] = imports
        summary["collector_tail_seconds"] = packet["collector_tail_seconds"]
        operations = summary.pop("renderer_operations")
        summary["renderer_operations"] = dict(
            sampled_by_operation=dict(zip(operations["operation_names"], operations["sampled_by_operation"])),
            truncated_operation_samples=operations["truncated_operation_samples"],
            outliers_by_window={window: {key: operations[window]["outliers"][key]
                                        for key in ("total", "omitted")}
                                for window in ("warmup", "sampled")})
        records.append(record)
        summaries.append(summary)
    first, second = records
    for key in ("inputs", "adapter", "window_context", "qpc_frequency"):
        require(first[key] == second[key], f"frozen {key} differs")
    workload = [{key: value for key, value in record["workload"].items() if key != "sample_seconds"}
                for record in records]
    require(workload[0] == workload[1], "workload differs beyond declared sample duration")
    require(first["session_name"] != second["session_name"]
            and first["native_present_interval_qpc"]["end"] < second["native_present_interval_qpc"]["begin"],
            "sessions are duplicated, overlapping or out of order")

    packet = decode(accuracy)
    run = packet["run"]
    require(run["exit_code"] == 0 and not run["stderr"], "accuracy process failed")
    report = json.loads(run["stdout"])
    require(report == run["report"] and packet["inputs_unchanged_after_run"],
            "accuracy report or inputs differ")
    adapter = ACCURACY["admit_adapter"](packet["adapters"], 0, "unknown")
    validation = ACCURACY["validate_report"](report, "q02", adapter, True, False)
    require(validation == packet["validation"], "accuracy validation differs")
    METRICS["validate_imports"](report["renderer_source_imports"])
    require(packet["inputs"]["executable_sha256"] == first["inputs"]["executable_sha256"],
            "accuracy executable differs")
    capture_helpers = {f"tools/mapgyver/{name}" for name in
                       ("capture-presentmon.py", "presentmon_data.py", "presentmon_roles.py")}
    for name, digest in first["inputs"]["files"].items():
        if name not in capture_helpers:
            require(packet["inputs"]["files"].get(name) == digest, "accuracy shared input missing or different")
    return dict(
        schema_version=1,
        evidence_kind="bounded_renderer_import_reuse_diagnostic_not_qualification",
        verification=dict(exact_capture_recomputation=True, matching_frozen_inputs_and_context=True,
                          distinct_chronological_sessions=True, import_counter_and_stage_invariants=True,
                          full_image_accuracy=True, current_native_source_count=2),
        analyzer_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        import_validator_sha256=hashlib.sha256((HERE / "import_metrics.py").read_bytes()).hexdigest(),
        shared_inputs=first["inputs"],
        accuracy=dict(artifact=accuracy.name, artifact_sha256=hashlib.sha256(accuracy.read_bytes()).hexdigest(),
                      validation=validation, comparison=report["comparison"],
                      renderer_source_imports=report["renderer_source_imports"]),
        runs=summaries,
        limits=["No contemporaneous uncached control or causal repair is established",
                "Cadence, consumer age and GPU-context stalls remain open",
                "Context timing is not the complete GPU critical path",
                "Live zero loss counters do not establish final capture or analysis zero loss",
                "COM alias and logical allocation cleanup do not establish physical VRAM retirement",
                "One sustained run is not the required qualifying series; M01 remains 1/8"])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("short", type=Path)
    parser.add_argument("sustained", type=Path)
    parser.add_argument("accuracy", type=Path)
    args = parser.parse_args()
    print(json.dumps(analyze(args.short, args.sustained, args.accuracy), indent=2, allow_nan=False))


if __name__ == "__main__":
    main()
