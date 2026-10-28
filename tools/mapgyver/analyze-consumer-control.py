"""Audit consumer activity controls without qualifying a paused output consumer."""

import argparse
import hashlib
import json
from pathlib import Path
import runpy

HERE = Path(__file__).resolve().parent
GPU = runpy.run_path(str(HERE / "analyze-gpu-stages.py"))
CONSUMER = runpy.run_path(str(HERE / "consumer_observation.py"))
require = GPU["require"]


def analyze_capture(path):
    record, summary = GPU["analyze_capture"](path)
    packet = GPU["PRESENTATION"]["BASE"]["decode_evidence"](json.loads(path.read_bytes()))
    require(packet["consumer_control"] is True
            and packet["evidence_kind"] == "consumer_activity_control_not_qualification",
            "capture is not an explicit consumer activity control")
    native = packet["native_report"]
    observation = CONSUMER["validate"](native, packet["requested_consumer_mode"])
    require(packet["consumer_observation_verified"] is True
            and observation == packet["consumer_observation"], "consumer observation differs")
    require(record["gpu_stages"] is True, "consumer control requires staged GPU timing")
    if observation["consumer_mode"] == "active":
        GPU["PRESENTATION"]["RELEASE"]["verify_age"](native["observer"]["frame_age"])
        require(native["evidence_kind"] != "consumer_paused_gpu_sharing_diagnostic_not_qualification",
                "active observer has a paused diagnostic identity")
    else:
        require(native["evidence_kind"] == "consumer_paused_gpu_sharing_diagnostic_not_qualification",
                "paused observer lacks its non-qualifying diagnostic identity")
    observer = native["observer"]
    held = observer["resources"]["before_resource_drop"]
    summary["consumer_observation"] = observation
    summary["consumer_ready"] = observer["observation_ready"]
    summary["consumer_ready_received_qpc"] = observer["observation_ready_received_qpc"]
    summary["consumer_held_allocations_before_drop"] = {
        key: held[key] for key in ("live_textures", "live_bytes_by_owner")}
    record["consumer_mode"] = observation["consumer_mode"]
    return record, summary


def verify_cohort(records, summaries):
    require(len(records) == len(summaries) == 4, "expected paused smoke followed by active/paused/active")
    for index, (record, summary) in enumerate(zip(records, summaries, strict=True)):
        require(record["consumer_mode"] == ("paused" if index in (0, 2) else "active")
                and record["workload"]["sample_seconds"] == (8 if index == 0 else 120),
                "predeclared consumer mode or duration sequence differs")
        for key in ("inputs", "adapter", "window_context", "qpc_frequency"):
            require(record[key] == records[0][key], f"frozen {key} differs")
        workload = {key: value for key, value in record["workload"].items()
                    if key not in ("sample_seconds", "consumer_mode")}
        require(workload == {key: value for key, value in records[0]["workload"].items()
                            if key not in ("sample_seconds", "consumer_mode")},
                "workload differs beyond predeclared duration and consumer mode")
        if index:
            require(records[index - 1]["native_present_interval_qpc"]["end"]
                    < record["native_present_interval_qpc"]["begin"], "captures overlap or are out of order")
        if index > 1:
            require(summary["consumer_held_allocations_before_drop"]
                    == summaries[1]["consumer_held_allocations_before_drop"],
                    "long controls retained different logical consumer allocations")
    require(len({record["session_name"] for record in records}) == len(records), "session identity repeated")


def analyze(paths):
    records, summaries = [], []
    for path in paths:
        record, summary = analyze_capture(path)
        records.append(record)
        summaries.append(summary)
    verify_cohort(records, summaries)
    return dict(
        schema_version=1, evidence_kind="consumer_activity_control_not_qualification",
        verification=dict(exact_capture_recomputation=True, complete_stage_and_context_coverage=True,
                          matching_frozen_inputs_and_context=True, distinct_chronological_sessions=True,
                          pre_sample_consumer_quiescence=True, paused_work_counters_unchanged=True,
                          active_raw_age_markers=True, held_logical_consumer_allocations_match=True),
        analyzer_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        consumer_validator_sha256=hashlib.sha256((HERE / "consumer_observation.py").read_bytes()).hexdigest(),
        stage_validator_sha256=hashlib.sha256((HERE / "gpu_stages.py").read_bytes()).hexdigest(),
        shared_inputs=records[0]["inputs"], runs=summaries,
        limits=["Paused removes SDK receives/copy/flush/access and marker-log work together",
                "No mutex-specific or SDK-internal cause, GPU busy-time or causal-repair claim",
                "Paused consumer frame age is unavailable, never a passing zero",
                "Logical allocation equality does not prove equal physical residency",
                "One paused sample and two active controls do not establish a qualifying series",
                "Elapsed boundaries do not align GPU context clocks or measure physical scanout",
                "Live zero counters do not establish final capture or analysis zero loss",
                "All M01 acceptance gates retain their original scope"])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("captures", nargs=4, type=Path)
    args = parser.parse_args()
    print(json.dumps(analyze(args.captures), indent=2, allow_nan=False))


if __name__ == "__main__":
    main()
