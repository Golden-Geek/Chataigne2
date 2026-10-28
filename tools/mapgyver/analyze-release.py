"""Read saved M01 release packets; validate raw markers and report scoped measurements only."""

import argparse
import json
import math
from pathlib import Path
import runpy

RUNNER = runpy.run_path(str(Path(__file__).with_name("run-qualification.py")))
PIPELINE = runpy.run_path(str(Path(__file__).with_name("analyze-pipeline.py")))


def require(condition, message):
    if not condition:
        raise ValueError(message)


def zero(snapshot):
    return (
        not any(snapshot["live_bytes_by_owner"])
        and not any(snapshot["retiring_bytes_by_owner"])
        and snapshot["live_textures"] == snapshot["retiring_textures"] == 0
    )


def gpu_timing_checks(timing):
    cpu, gpu = timing["cpu_submission_work"], timing["gpu_execution"]
    counters = {key: timing[key] for key in (
        "skipped_gpu_queries", "invalid_gpu_queries", "pending_gpu_queries", "truncated_samples",
    )}
    complete = (all(value == 0 for value in counters.values()) and cpu is not None and gpu is not None
                and cpu["samples"] == gpu["samples"])
    observed_within_target = gpu is not None and gpu["p95_ms"] <= 12 and gpu["p99_ms"] <= 16.667
    return {
        "gpu_query_accounting": counters,
        "gpu_query_accounting_complete": complete,
        "recorded_gpu_percentiles_within_target": observed_within_target,
        "gpu_budget_met": complete and observed_within_target,
    }


def rotate(value, bits):
    return ((value << bits) | (value >> (32 - bits))) & 0xffff_ffff


def verify_age(age):
    """Independent integer/checksum oracle; never replace absent/invalid/repeated observations."""
    config = age["config"]
    start = config["start_qpc"] + config["warmup_seconds"] * config["frequency"]
    end = config["start_qpc"] + config["maximum_seconds"] * config["frequency"]
    rows = age["sample_rows"]
    require(len(rows) == age["captured_samples"], "raw age count mismatch")
    previous_completion = 0
    prior = [None, None]
    channels = [{key: 0 for key in (
        "valid", "absent", "invalid", "repeated", "unique_frames", "skipped_source_frame_ids",
    )} for _ in range(2)]
    values = [[], []]
    for row in rows:
        submitted, completed = row[:2]
        require(len(row) == 12 and start <= submitted < end, "age capture outside declared window")
        require(completed >= submitted and completed >= previous_completion, "age completion clock/order")
        previous_completion = completed
        for index in range(2):
            words = row[2 + index * 5:7 + index * 5]
            channel = channels[index]
            if index == 0 and words == [0xff0d_0d0d] * 5:
                channel["absent"] += 1
                continue
            frame, magic, low, high, checksum = words
            qpc = low | (high << 32)
            expected = rotate(frame, 7) ^ low ^ rotate(high, 13) ^ magic ^ 0x9e37_79b9
            old = prior[index]
            if (magic != 0xa671_c300 | index or checksum != expected or qpc == 0 or qpc > completed
                or (old is not None and (frame < old[0] or qpc < old[1] or (frame == old[0] and qpc != old[1])))):
                channel["invalid"] += 1
                continue
            if old is not None and frame == old[0]:
                channel["repeated"] += 1
            else:
                channel["unique_frames"] += 1
                if old is not None:
                    channel["skipped_source_frame_ids"] += frame - old[0] - 1
            prior[index] = (frame, qpc)
            channel["valid"] += 1
            values[index].append((completed - qpc) * 1000.0 / config["frequency"])
    for index, channel in enumerate(channels):
        reported = age["channels"][index]
        require(all(reported[key] == value for key, value in channel.items()), "age channel count mismatch")
        ordered = sorted(values[index])
        distribution = reported["frame_age"]
        if not ordered:
            require(distribution is None, "empty age has a distribution")
            continue
        count = len(ordered)
        def rank(percent):
            return ordered[(count * percent + 99) // 100 - 1]
        expected = {
            "samples": count, "mean_ms": sum(ordered) / count,
            "median_ms": rank(50), "p95_ms": rank(95),
            "p99_ms": rank(99), "maximum_ms": ordered[-1],
        }
        require(all(math.isclose(distribution[key], value, rel_tol=1e-12, abs_tol=1e-9)
                    for key, value in expected.items()), "age distribution mismatch")
    return True


def verify_gpu_admission(generation, resources):
    admission = generation["gpu_admission"]
    require(admission["maximum_in_flight"] == 2 and 0 <= admission["peak_in_flight"] <= 2,
            "GPU pipeline capacity/peak mismatch")
    require(admission["pending"] == 0 and not admission["reserved"]
            and admission["completed"] == admission["submitted"], "GPU pipeline retained after fence")
    require(admission["pending_at_final_poll"] <= 2, "GPU final poll pending count")
    for window in (admission["warmup"], admission["sampled"]):
        require(window["attempts"] == window["admitted"] + window["busy"], "GPU attempt accounting mismatch")
    admitted = admission["warmup"]["admitted"] + admission["sampled"]["admitted"]
    require(admitted == admission["submitted"] == generation["spout_sends"], "GPU submit accounting mismatch")
    cpu = generation["timing"]["cpu_submission_work"]
    cpu_samples = cpu["samples"] if cpu is not None else 0
    require(admission["sampled"]["admitted"] == cpu_samples,
            "GPU admitted timing count mismatch")
    for path in generation["submission_editor_output_spout"]:
        require(path["attempts"] == admission["sampled"]["attempts"]
                and path["busy"] >= admission["sampled"]["busy"], "GPU coalescing missing from cadence")
    for observation in resources["gpu_observations"]:
        snapshot = observation["gpu_pipeline"]
        require(snapshot["maximum_in_flight"] == 2 and snapshot["peak_in_flight"] <= 2,
                "sparse GPU capacity/peak mismatch")
        require(snapshot["pending"] + int(snapshot["reserved"]) <= 2, "sparse GPU pipeline overflow")
        require(snapshot["submitted"] - snapshot["completed"] == snapshot["pending"], "sparse GPU prefix accounting")
        require(snapshot["submitted"] <= admission["submitted"] and snapshot["completed"] <= admission["completed"],
                "sparse GPU counters exceed final report")
    return admission


def verify_gpu_events(gpu, total, period, capacity):
    require(len(gpu["events"]) + gpu["omitted"] == gpu["total"], "GPU outlier count mismatch")
    require(len(gpu["events"]) <= capacity, "GPU outlier capacity exceeded")
    require((gpu["maximum"] is None) == (gpu["total"] == 0), "GPU maximum absent")
    for event in [*gpu["events"], *([gpu["maximum"]] if gpu["maximum"] else [])]:
        require(0 <= event["stamp"]["frame"] < total and event["frequency"] > 0, "GPU outlier stamp/frequency")
        require(event["observed_completed_qpc"] >= event["stamp"]["qpc"], "GPU completion predates submission")
        ns = (event["end_ticks"] - event["start_ticks"]) * 1_000_000_000 // event["frequency"]
        require(period <= ns <= gpu["maximum_score"], "GPU outlier raw ticks/score")


def verify_operations(generation):
    trace = generation["operation_timing"]
    names = trace["operation_names"]
    distributions = trace["sampled_by_operation"]
    admission = generation.get("gpu_admission")
    count = 19 if "gpu_signal" in names or "gpu_flush" in names else 17 if admission is not None else 16
    require(len(names) == len(distributions) == count and len(set(names)) == count, "operation registry mismatch")
    require(trace["qpc_frequency"] > 0, "operation QPC frequency absent")
    sampled = trace["sampled"]["frames"]
    total = sampled + trace["warmup"]["frames"]
    expected_sampled = admission["sampled"]["attempts"] if admission else generation["timing"]["cpu_submission_work"]["samples"]
    expected_total = (admission["sampled"]["attempts"] + admission["warmup"]["attempts"]
                      if admission else generation["spout_sends"])
    require(sampled == expected_sampled, "operation sample count mismatch")
    require(total == expected_total, "operation whole-generation count mismatch")
    require(trace["truncated_operation_samples"] == 0, "operation samples truncated")
    for distribution in distributions:
        require(distribution is None or 0 < distribution["samples"] <= sampled, "operation distribution count")

    def score(event):
        return max(event["elapsed_ns"], event["scheduled_delay_ns"])

    def check_event(event):
        require(0 <= event["stamp"]["frame"] < total and event["stamp"]["qpc"] > 0, "operation frame stamp")
        require(len(event["spans"]) == len(names), "operation span count")
        end = 0
        for span in event["spans"]:
            if span is not None:
                require(span["offset_ns"] >= end and span["elapsed_ns"] >= 0, "operation spans overlap")
                end = span["offset_ns"] + span["elapsed_ns"]
                require(end <= event["elapsed_ns"], "operation span exceeds frame")
        coalesced = event["gpu_admission_busy"] if admission else False
        if coalesced:
            require(not event["gpu_query_skipped"], "coalescing is not a missing profiling query")
            require(all(span is None for name, span in zip(names, event["spans"], strict=True)
                        if name not in ("resources", "gpu_admission", "observer_control")),
                    "coalesced frame issued GPU/SDK work")
        require(event["elapsed_ns"] >= trace["frame_period_ns"]
                or event["scheduled_delay_ns"] >= trace["frame_period_ns"] or event["gpu_query_skipped"]
                or coalesced
                or any(span is not None and span["elapsed_ns"] >= trace["operation_threshold_ns"]
                       for span in event["spans"]), "operation event does not meet a threshold")

    for window in (trace["warmup"], trace["sampled"]):
        log = window["outliers"]
        require(len(log["events"]) + log["omitted"] == log["total"] <= window["frames"], "outlier count mismatch")
        require(len(log["events"]) <= trace["outlier_capacity_per_window"], "outlier capacity exceeded")
        stamps = [event["stamp"]["frame"] for event in log["events"]]
        require(stamps == sorted(set(stamps)), "operation event order")
        require((log["maximum"] is None) == (log["total"] == 0), "maximum operation event absent")
        for event in [*log["events"], *([log["maximum"]] if log["maximum"] else [])]:
            check_event(event)
            require(score(event) <= log["maximum_score"], "maximum operation score mismatch")
        if log["maximum"] is not None:
            require(score(log["maximum"]) == log["maximum_score"], "maximum operation event score")
    gpu = generation["timing"]["gpu_outliers"]
    verify_gpu_events(gpu, total, trace["frame_period_ns"], trace["outlier_capacity_per_window"])
    return {
        "sampled_by_operation": dict(zip(names, distributions, strict=True)),
        "warmup_outliers": trace["warmup"]["outliers"],
        "sampled_outliers": trace["sampled"]["outliers"],
        "gpu_outliers": gpu,
    }


def verify_source_workers(generation):
    workers = generation["source_workers"]
    require(len(workers) == 2, "source worker pair missing")
    require(generation["operations"]["sdk_receive_calls"] == 0, "source SDK receive on renderer")
    require(generation["operations"]["explicit_gpu_copy_calls"] == 0, "source bridge copies on renderer")
    for worker in workers:
        require(worker["adapter_luid"] == generation["adapter_luid"], "source worker adapter mismatch")
        require(worker["maximum_surfaces"] == 3 and 1 <= worker["peak_surfaces"] <= 3,
                "source bridge capacity mismatch")
        resources = worker["resources"]
        require(resources["sample_count"] == len(resources["sample_rows"]), "source worker row mismatch")
        require(zero(resources["after_fence"]), "source worker retained ownership after fence")
        operations = resources["operations"]
        require(operations["sdk_receive_calls"] == worker["receive_calls"], "source receive count mismatch")
        require(operations["explicit_gpu_copy_calls"] == worker["publications"], "source bridge copy count mismatch")
        require(worker["publications"] <= operations["sdk_texture_observations"] <= worker["receive_calls"],
                "source publication count mismatch")
        require(all(operations[key] == 0 for key in ("diagnostic_readback_pixels", "uniform_upload_bytes", "sdk_send_calls")),
                "unexpected source worker upload/readback/send")
        distribution = worker["receive_wall_time_all_windows"]
        samples = distribution["samples"] if distribution is not None else 0
        require(samples + worker["truncated_receive_samples"] == worker["receive_calls"], "source receive timing count mismatch")
        if "gpu_admission" in worker:
            gate = worker["gpu_admission"]
            require(gate["maximum_in_flight"] == 2 and gate["peak_in_flight"] <= 2,
                    "source GPU admission capacity mismatch")
            require(gate["pending"] == 0 and not gate["reserved"] and gate["submitted"] == gate["completed"] == worker["receive_calls"],
                    "source GPU admission retained work/count mismatch")
            for window in (gate["warmup"], gate["sampled"]):
                require(window["attempts"] == window["admitted"] + window["busy"], "source GPU attempt accounting mismatch")
            require(gate["warmup"]["admitted"] + gate["sampled"]["admitted"] == worker["receive_calls"],
                    "source GPU admitted receive count mismatch")
            timing = worker["timing"]
            measured = timing["cpu_submission_work"]
            require((measured["samples"] if measured else 0) == gate["sampled"]["admitted"],
                    "source GPU sampled timing count mismatch")
            gpu_count = timing["gpu_execution"]["samples"] if timing["gpu_execution"] else 0
            require(gpu_count + sum(timing[k] for k in ("skipped_gpu_queries", "invalid_gpu_queries", "pending_gpu_queries"))
                    == gate["sampled"]["admitted"], "source GPU query accounting mismatch")
            verify_gpu_events(timing["gpu_outliers"], worker["receive_calls"], 16_666_667, 128)
    return workers


def analyze_sample(evidence):
    require(evidence["schema_version"] in (7, 8, 9, 10, 11, 12, 13, 14, 15), "unsupported release evidence schema")
    run = evidence["run"]
    if run.get("exit_code") != 0 or run.get("report") is None:
        return {"sample_index": evidence["sample_index"], "native_success": False, "failure": run}
    report = run["report"]
    if evidence["schema_version"] >= 15:
        require(report.get("status", {}).get("layout_session", 0) > 0,
                "schema 15 document session missing; rebuild the native probe")
    generations = report["generations"]
    require(len(generations) == 1, "timed release must have one device generation")
    generation = generations[0]
    require(generation["end"] == "deadline_reached", "release sample ended early")
    if evidence["schema_version"] >= 8:
        require("operation_timing" in generation, "schema 8 operation trace missing; rebuild the native probe")
    if evidence["schema_version"] >= 9:
        require("gpu_admission" in generation, "schema 9 GPU admission missing; rebuild the native probe")
    if evidence["schema_version"] >= 10:
        require("source_workers" in generation, "schema 10 source workers missing; rebuild the native probe")
    if evidence["schema_version"] >= 11:
        require(all("gpu_admission" in worker and "timing" in worker for worker in generation["source_workers"]),
                "schema 11 source GPU admission missing; rebuild the native probe")
    if evidence["schema_version"] >= 12:
        require(generation["gpu_admission"].get("mechanism") == "timeline_fence"
                and all(worker["gpu_admission"].get("mechanism") == "timeline_fence"
                        for worker in generation["source_workers"]),
                "schema 12 timeline admission missing; rebuild the native probe")
    if evidence["schema_version"] >= 13:
        require("dependency_trace" in generation and generation["timing"].get("gpu_trace") is not None
                and all(worker["timing"].get("gpu_trace") is not None for worker in generation["source_workers"]),
                "schema 13 pipeline dependencies missing; rebuild the native probe")
    if evidence["schema_version"] >= 14:
        require(generation["operation_timing"].get("operation_names", [])[-3:]
                == ["query_end", "gpu_signal", "gpu_flush"],
                "schema 14 split submission stages missing; rebuild the native probe")
    verify_age(report["observer"]["frame_age"])
    timing = generation["timing"]
    cpu, gpu = timing["cpu_submission_work"], timing["gpu_execution"]
    resources = report["resources"]
    require(len(resources["rows"]) == len(resources["gpu_observations"]), "renderer row count mismatch")
    columns = resources["columns"]
    all_rows = [dict(zip(columns, row, strict=True)) for row in resources["rows"]]
    sample_rows = [row for row in all_rows if row["in_sample_window"]]
    require(sample_rows, "missing warmed resource observations")
    children = [*report["sources"], report["observer"]]
    require(len(children) == 3, "missing source/consumer report")
    for child in children:
        require(child["resources"]["sample_count"] == len(child["resources"]["sample_rows"]), "child row mismatch")
    logical_peak = generation["allocations_after_fence"]["peak_live_plus_retiring_bytes"]
    workers = verify_source_workers(generation) if "source_workers" in generation else []
    logical_upper_bound = logical_peak + sum(worker["resources"]["after_fence"]["peak_live_plus_retiring_bytes"]
                                             for worker in workers)
    scenario = report["workload"]["scenario"]
    require(scenario in ("q01", "q02"), "release workload must be Q01/Q02")
    logical_ceiling = (512 if scenario == "q01" else 1024) * 1024 * 1024
    frame_age = report["observer"]["frame_age"]
    cadence = generation["submission_editor_output_spout"]
    for path in cadence:
        require(sum(path[key] for key in ("accepted_calls", "busy", "occluded", "no_target")) == path["attempts"],
                "submission outcome accounting mismatch")
        require(path["late_attempts"] <= path["attempts"], "late submission count exceeds attempts")
    require(cadence[2]["accepted_calls"] == (cpu["samples"] if cpu is not None else 0),
            "Spout accepted calls differ from measured submissions")
    expected_calls = report["workload"]["sample_seconds"] * 60
    for admission, accepted in zip(generation["admission_editor_output"], (
        generation["preview_presents"], generation["output_presents"],
    ), strict=True):
        require(admission["readiness_polls"] == admission["ready_signals"] + admission["not_ready"],
                "presentation readiness accounting mismatch")
        require(accepted == admission["accepted_presents"] <= admission["ready_signals"],
                "presentation acceptance accounting mismatch")
    expected_extent = [1920, 1080] if scenario == "q01" else [3840, 2160]
    result = {
        "sample_index": evidence["sample_index"], "scenario": scenario, "native_success": True,
        "native_stderr_empty": not run["stderr"],
        "inputs_unchanged": evidence["inputs_unchanged_after_run"],
        "fixed_geometry_observed": generation["fixed_geometry_observed"],
        "sampled_geometry_mismatches": generation["sampled_geometry_mismatches"],
        "native_output_extent": generation["native_output_extent"],
        "fixed_workload_geometry_met": (generation["fixed_geometry_observed"]
                                        and generation["sampled_geometry_mismatches"] == 0
                                        and generation["native_output_extent"] == expected_extent),
        "sparse_one_frame_queue_limit_verified": all(
            item["maximum_frame_latency"] == {"Ok": 1}
            for observation in resources["gpu_observations"]
            for item in observation["presentation_editor_output"]
        ),
        "renderer_upload_readback_zero": all(generation["operations"][key] == 0 for key in (
            "uniform_upload_bytes", "diagnostic_readback_pixels",
        )),
        "cpu_submission": cpu, "gpu_timeline": gpu,
        "cpu_budget_met": cpu is not None and cpu["p95_ms"] <= 4 and cpu["p99_ms"] <= 8,
        **gpu_timing_checks(timing),
        "calls_editor_output_spout": [{
            **item, "target_calls_at_60_hz": expected_calls,
            "accepted_call_shortfall": max(0, expected_calls - item["accepted_calls"]),
            "accepted_call_shortfall_percent": max(0, expected_calls - item["accepted_calls"]) / expected_calls * 100,
            "recorded_late_percent": item["late_attempts"] / item["attempts"] * 100 if item["attempts"] else None,
        } for item in cadence],
        "renderer_logical_peak_bytes": logical_peak,
        "renderer_logical_ceiling_met": logical_upper_bound <= logical_ceiling,
        "warmed_private_peak_by_process": {name: max(row[f"{name}.private_bytes"] for row in sample_rows)
                                           for name in ("renderer", "source_a", "source_b", "observer")},
        "frame_age_channels": frame_age["channels"],
        "consumer_age_p95_target_met": all(
            channel["invalid"] == 0 and channel["frame_age"] is not None
            and channel["frame_age"]["p95_ms"] <= 50 for channel in frame_age["channels"]
        ),
        "raw_marker_integrity_verified": True,
        "zero_tracked_after_fences": zero(generation["allocations_after_fence"])
                                     and all(zero(child["resources"]["after_fence"]) for child in children),
        "worker_window_shutdown_seconds": report["shutdown_seconds"],
        "scoped_shutdown_target_met": report["shutdown_seconds"] is not None and report["shutdown_seconds"] <= 5,
    }
    if "operation_timing" in generation:
        # Historical packets retain their actual older schema; no timing is synthesized.
        result["operation_timing"] = verify_operations(generation)
    if "gpu_admission" in generation:
        result["gpu_admission"] = verify_gpu_admission(generation, resources)
    if workers:
        result["source_workers"] = workers
        result["renderer_process_logical_peak_upper_bound_bytes"] = logical_upper_bound
        result["zero_tracked_after_fences"] &= all(zero(worker["resources"]["after_fence"]) for worker in workers)
        # Separate contexts run asynchronously. Marginal percentiles cannot be added to prove
        # the full pipeline's critical path; preserve the original budget as unqualified.
        result["renderer_context_gpu_budget_met"] = result["gpu_budget_met"]
        result["gpu_budget_met"] = False
        result["gpu_timing_scope"] = "separate context timelines; full pipeline critical path unqualified"
        result["source_context_gpu_checks"] = [gpu_timing_checks(worker["timing"])
                                               if "timing" in worker else None for worker in workers]
    if evidence["schema_version"] >= 13:
        result["pipeline_timing"] = PIPELINE["analyze_generation"](generation)
    return result


def adapter_identity(evidence):
    # Budgets/usage are dynamic observations, not adapter identity or driver revision.
    keys = ("index", "description", "luid", "dedicated_video_bytes", "shared_system_bytes", "software", "outputs")
    return [{key: adapter[key] for key in keys} for adapter in evidence["adapters"]]


def analyze_series(packets):
    evidence = [RUNNER["decode_evidence"](packet) for packet in packets]
    require(evidence, "empty series")
    first = evidence[0]
    repeated = len(evidence) == 3 and [item["sample_index"] for item in evidence] == [1, 2, 3]
    for item in evidence:
        require(item["inputs"] == first["inputs"], "series inputs changed")
        require(item["command"] == first["command"], "series command changed")
        require(adapter_identity(item) == adapter_identity(first), "series adapter identity changed")
        spec = item["run"].get("report", {}).get("workload") if item["run"].get("report") else None
        repeated &= (item["profile"] == "release" and item["inputs_unchanged_after_run"]
                     and item["requested_series_samples"] == 3 and spec is not None
                     and item["run"].get("exit_code") == 0
                     and spec.get("warmup_seconds") == 30 and spec.get("sample_seconds") == 120)
    return {
        "scope": "measured native submission/GPU/logical-resource/consumer-age subset; not full M01",
        "contract": "docs/progress/mapgyver-qualification-contract.md",
        "three_immutable_release_samples": bool(repeated),
        "samples": [analyze_sample(item) for item in evidence],
        "still_unqualified": [
            "displayed frame cadence/scanout, input/geometry-to-visible latency and physical-pixel alignment",
            "SDK/driver-internal copy counts, physical residency and combined app/WebView/complete-process shutdown",
            "full-card accuracy, all display/input configurations and full Q05 recovery/retirement",
        ],
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("artifact", type=Path)
    args = parser.parse_args()
    artifact = json.loads(args.artifact.read_text(encoding="utf-8"))
    print(json.dumps(analyze_series(artifact["packets"]), indent=2))


if __name__ == "__main__":
    main()
