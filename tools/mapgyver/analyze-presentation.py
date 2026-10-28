"""Audit an output-only latest/vblank/latest control without granting cadence qualification."""

import argparse
import copy
import csv
import hashlib
import io
import json
from pathlib import Path
import runpy

TOOLS = Path(__file__).resolve().parent
BASE = runpy.run_path(str(TOOLS / "run-qualification.py"))
CAPTURE = runpy.run_path(str(TOOLS / "capture-presentmon.py"))
DATA = runpy.run_path(str(TOOLS / "presentmon_data.py"))
ROLES = runpy.run_path(str(TOOLS / "presentmon_roles.py"))
RELEASE = runpy.run_path(str(TOOLS / "analyze-release.py"))
SEQUENCE = ("latest", "output-vblank", "latest")


def require(condition, message):
    if not condition:
        raise ValueError(message)


def verify_stream(stream, label):
    require(isinstance(stream, dict) and type(stream.get("exit_code")) is int
            and stream["exit_code"] == 0, f"{label} process failed")
    require(stream.get("timed_out") is False and stream.get("valid_utf8") is True,
            f"{label} timed out or has unavailable text")
    require(stream.get("stderr") == "", f"{label} stderr is not empty")
    for name in ("stdout", "stderr"):
        require(isinstance(stream.get(name), str), f"{label} {name} is missing")
        raw = stream[name].encode("utf-8")
        require(type(stream.get(name + "_bytes")) is int and len(raw) == stream[name + "_bytes"],
                f"{label} {name} byte count differs")
        require(hashlib.sha256(raw).hexdigest() == stream.get(name + "_sha256"),
                f"{label} {name} checksum differs")


def window_context(record):
    """Keep configuration facts; query timestamps and explanatory scope are not configuration."""
    result = record.get("window_at_end")
    require(isinstance(result, dict) and set(result) == {"Ok"}, "owned-window observation unavailable")
    window = result["Ok"]
    keys = ("window_rect_physical", "client_origin_physical", "client_extent_physical", "dpi", "monitor")
    require(isinstance(window, dict) and all(key in window for key in keys), "window context is incomplete")
    mode = window.get("current_mode")
    require(isinstance(mode, dict) and set(mode) == {"Ok"}, "current monitor mode unavailable")
    require(type(window["dpi"]) is int and window["dpi"] > 0, "window DPI unavailable")
    require(isinstance(mode["Ok"], dict) and all(key in mode["Ok"] for key in
            ("width_pixels", "height_pixels", "nominal_refresh_hz")), "monitor mode is incomplete")
    context = {key: copy.deepcopy(window[key]) for key in keys}
    context["current_mode"] = {key: mode["Ok"][key] for key in
                               ("width_pixels", "height_pixels", "nominal_refresh_hz")}
    context["coordinate_context"] = window.get("coordinate_context")
    return context


def submission_summary(path, sample_seconds):
    """Retain separate denominators; late outcomes can overlap other categories."""
    names = ("attempts", "accepted_calls", "busy", "occluded", "no_target", "late_attempts")
    require(type(sample_seconds) is int and sample_seconds > 0, "invalid sample duration")
    require(all(type(path.get(name)) is int and path[name] >= 0 for name in names),
            "invalid submission counts")
    require(sum(path[name] for name in ("accepted_calls", "busy", "occluded", "no_target")) == path["attempts"],
            "submission outcomes do not partition attempts")
    require(path["late_attempts"] <= path["attempts"], "late count exceeds attempts")
    return dict(nominal_frame_slots_at_60_hz=sample_seconds * 60,
                actual_attempts_minus_nominal_slots=path["attempts"] - sample_seconds * 60,
                **{name: path[name] for name in names},
                accepted_call_intervals=path.get("accepted_call_intervals"),
                deadline_lateness=path.get("deadline_lateness"),
                scope=("Nominal slots, actual attempts and outcomes stay separate; "
                       "late counts are not added to busy/drop counts"))


def verify_cohort(records):
    require(len(records) == 3, "comparison requires exactly three captures")
    failures = []
    first = records[0]
    sessions = [record["session_name"] for record in records]
    if len(set(sessions)) != 3:
        failures.append("comparison reuses a capture session instead of three distinct controls")
    for index, record in enumerate(records):
        if record["requested_policy"] != SEQUENCE[index]:
            failures.append(f"run {index + 1} does not follow latest/output-vblank/latest")
        if record["inputs"] != first["inputs"]:
            failures.append(f"run {index + 1} native/source/tool fingerprints differ")
        if record["adapter"] != first["adapter"] or record["adapter"]["index"] != 0:
            failures.append(f"run {index + 1} adapter differs from the fixed adapter-0 control")
        if record["workload"] != first["workload"] or record["workload"] != {
                "scenario": "q02", "exercise": "timed", "sample_seconds": 120, "warmup_seconds": 30}:
            failures.append(f"run {index + 1} is not the fixed 30+120-second Q02 workload")
        if record["window_context"] != first["window_context"]:
            failures.append(f"run {index + 1} actual window/monitor configuration differs")
        if record["qpc_frequency"] != first["qpc_frequency"]:
            failures.append(f"run {index + 1} QPC frequency differs")
        interval = record["native_present_interval_qpc"]
        require(all(type(interval[key]) is int and 0 < interval[key] < 1 << 64 for key in ("begin", "end"))
                and interval["begin"] < interval["end"], "native Present interval is invalid")
        if index and interval["begin"] < records[index - 1]["native_present_interval_qpc"]["end"]:
            failures.append(f"run {index + 1} native Present interval precedes or overlaps the prior control")
    return failures


def role_summary(role, generation, groups, config, sample_seconds, admission):
    name = role["role"]
    index = 0 if name == "editor" else 1
    begin, end, frequency = DATA["_window"](config)
    rows = [row for address, values in groups.items() if address in role["addresses"].values()
            for row in values if role["begin_qpc"] <= row.qpc < role["end_qpc"]]
    latencies = [(row.display_qpc - row.qpc) * 1000 / frequency for row in rows
                 if begin <= row.qpc < end and not row.dropped and row.display_qpc is not None]
    csv_sample_presents = sum(chain["present_window"]["presents"] for chain in role["chains"])
    return dict(role=name, generation=role["generation"], raw_rows=role["raw_rows"],
                all_window_native_accepted_minus_raw_csv_rows=(
                    admission[index]["all"]["accepted_presents"] - role["raw_rows"]),
                native_sample_accepted_minus_csv_sample_presents=(
                    admission[index]["sampled"]["accepted_presents"] - csv_sample_presents),
                native_submission=submission_summary(
                    generation["submission_editor_output_spout"][index], sample_seconds),
                presentation_admission=admission[index], csv_chains=role["chains"],
                present_to_display_software_latency_ms=DATA["_distribution"](latencies),
                latency_scope=("Valid non-dropped Presents in the Present window only; "
                               "no source-to-output or optical latency claim"))


def loss_observations(evidence, native, csv_text):
    """A closed-session query is unavailable evidence, even when native/CSV facts are exact."""
    query = json.loads(evidence["etw_query"]["stdout"])
    require(query == evidence["etw_query_report"], "query stdout differs")
    derived_loss = DATA["loss_observation"](csv_text, evidence["presentmon"]["stderr"])
    require(derived_loss["no_loss_reported"], "collector reported loss")
    try:
        live = CAPTURE["validate_etw_query"](query, evidence["session_name"], native)
    except ValueError as error:
        # This one observed terminal condition does not relax stream/process verification.
        # A failed API cannot populate zero-valued counters from unwritten output memory.
        require(type(query.get("status")) is int and query["status"] == 4201
                and query.get("statistics") is None and query.get("response_validation_error") is None,
                "unsupported live ETW query failure")
        require(query.get("session_name") == evidence["session_name"], "failed query session identity differs")
        before, after = query.get("before_qpc"), query.get("after_qpc")
        ends = [chain["end_qpc"] for generation in native["generations"] for chain in generation["swapchains"]]
        require(ends and all(type(value) is int and 0 < value < 1 << 64 for value in [before, after, *ends])
                and max(ends) <= before <= after, "failed query clock bounds differ")
        require(evidence.get("route_verified") is False and evidence.get("live_etw_query_verified") is False
                and evidence.get("live_etw_observation") is None, "failed query is marked verified")
        require(evidence.get("validation_failure") == str(error), "stored live-query failure differs")
        require("loss_observation" not in evidence, "unexpected stored loss result after failed query")
        live = dict(available=False, before_qpc=before, after_qpc=after,
                    native_present_interval_end_qpc=max(ends), win32_status=query["status"],
                    status_message=query.get("status_message"), counters=None, all_counters_zero=None,
                    final_capture_zero_loss_verified=False, reason=str(error),
                    scope="Session was absent at the query; counters and final capture loss are unavailable")
        failure = "Live ETW query returned 4201 with unavailable statistics after session termination"
        return live, derived_loss, "derived_from_raw_after_failed_query", [failure]
    require(evidence.get("route_verified") is True and evidence.get("live_etw_query_verified") is True,
            "successful query is not marked verified")
    require(live == evidence["live_etw_observation"] and live["all_counters_zero"], "live loss observation differs")
    require(derived_loss == evidence["loss_observation"], "stored collector loss observation differs")
    return dict(available=True, **live), derived_loss, "stored_and_recomputed", []


def analyze_capture(path):
    raw = path.read_bytes()
    evidence = BASE["decode_evidence"](json.loads(raw))
    for flag in ("role_correlation_verified", "presentation_policy_verified", "inputs_unchanged_after_run"):
        require(evidence.get(flag) is True, f"{path.name}: {flag} is unverified")
    for key in ("native", "presentmon", "etw_query"):
        verify_stream(evidence[key], key)
    native = json.loads(evidence["native"]["stdout"])
    require(native["render_process_id"] == evidence["native_pid"], "native PID differs")
    BASE["compact_resources"](native)
    require(native == evidence["native_report"], "native stdout compaction differs")
    config = native["observer"]["frame_age"]["config"]
    csv_text = evidence["presentmon"]["stdout"]
    pid = evidence["native_pid"]
    require(DATA["analyze"](csv_text, pid, config) == evidence["analysis"], "unlabeled CSV analysis differs")
    correlation = ROLES["correlate"](csv_text, pid, native, config)
    require(correlation == evidence["role_correlation"], "native role correlation differs")
    requested = evidence["requested_presentation_policy"]
    policy = CAPTURE["validate_presentation_policy"](csv_text, requested, native, correlation)
    require(policy == evidence["presentation_policy_observation"], "presentation policy verification differs")
    live, loss, loss_source, verification_failures = loss_observations(evidence, native, csv_text)
    generations = native["generations"]
    require(len(generations) == 1 and generations[0]["generation"] == 0
            and generations[0]["end"] == "deadline_reached", "timed generation is incomplete")
    generation = generations[0]
    require(generation["fixed_geometry_observed"] and generation["sampled_geometry_mismatches"] == 0
            and generation["native_output_extent"] == [3840, 2160], "fixed Q02 geometry differs")
    admission = RELEASE["verify_presenter_admission"](generation)
    gate = RELEASE["verify_gpu_admission"](generation, native["resources"])
    workers = RELEASE["verify_source_workers"](generation)
    require(RELEASE["zero"](generation["allocations_after_fence"]), "renderer retained logical allocations")
    for child in [*native["sources"], native["observer"]]:
        require(RELEASE["zero"](child["resources"]["after_fence"]), "fixture/observer retained logical allocations")
    _, groups, count = DATA["_read"](csv_text, pid, config["frequency"])
    fields = csv.DictReader(io.StringIO(csv_text))
    flags, modes = {}, {}
    for row in fields:
        value = int(row["PresentFlags"])
        require(value == 8, "Present flags changed from DO_NOT_WAIT")
        flags[str(value)] = flags.get(str(value), 0) + 1
        modes[row["PresentMode"]] = modes.get(row["PresentMode"], 0) + 1
    workload = dict(native["workload"])
    workload.pop("presentation_policy")
    require(workload["sample_seconds"] == evidence["native_sample_seconds"]
            and workload["warmup_seconds"] == evidence["native_warmup_seconds"], "recorded native durations differ")
    chains = generation["swapchains"]
    require({record["role"] for record in chains} == {"editor", "native_output"}, "window role pair missing")
    contexts = {record["role"]: window_context(record) for record in chains}
    native_interval = dict(begin=min(record["begin_qpc"] for record in chains),
                           end=max(record["end_qpc"] for record in chains))
    dependencies = generation["dependency_trace"]["rows"]
    require(len(dependencies) == gate["sampled"]["admitted"], "dependency/admission counts differ")
    roles = [role_summary(role, generation, groups, config, workload["sample_seconds"], admission)
             for role in correlation["roles"]]
    sparse_latency = []
    for observation in native["resources"]["gpu_observations"]:
        values = [item["maximum_frame_latency"] for item in observation["presentation_editor_output"]]
        require(all(value == {"Ok": 1} for value in values), "observed frame latency setting differs")
        sparse_latency.append(values)
    timing = generation["timing"]
    record = dict(inputs=evidence["inputs"], requested_policy=requested, workload=workload,
                  session_name=evidence["session_name"],
                  qpc_frequency=config["frequency"], native_present_interval_qpc=native_interval,
                  adapter={key: evidence["adapter"][key] for key in ("index", "luid", "description", "software")},
                  window_context=contexts)
    summary = dict(artifact=path.name, artifact_sha256=hashlib.sha256(raw).hexdigest(),
                   native_pid=pid, started_utc=evidence["started_utc"], session_name=evidence["session_name"],
                   requested_policy=requested, native_policy=policy["native_policy"], raw_csv_rows=count,
                   qpc_frequency=config["frequency"], native_present_interval_qpc=native_interval,
                   exact_native_csv_and_query_streams_verified=True, window_context=contexts,
                   complete_capture_observations=not verification_failures,
                   verification_failures=verification_failures,
                   sample_present_window_qpc=correlation["sample_present_window_qpc"], roles=roles,
                   gpu_admission=gate,
                   source_worker_maximum_surfaces=[worker["maximum_surfaces"] for worker in workers],
                   source_worker_peak_surfaces=[worker["peak_surfaces"] for worker in workers],
                   renderer_final_logical_allocations=generation["allocations_after_fence"],
                   sparse_frame_latency_observations=sparse_latency,
                   admitted_dependency_frames=len(dependencies),
                   source_publication_absence_in_admitted_frames=[sum(row[index] is None for row in dependencies)
                                                                 for index in (1, 2)],
                   renderer_cpu_submission=timing["cpu_submission_work"], renderer_gpu_context=timing["gpu_execution"],
                   renderer_operations=generation["operation_timing"],
                   source_worker_cpu_submission=[worker["timing"]["cpu_submission_work"] for worker in workers],
                   source_worker_gpu_context=[worker["timing"]["gpu_execution"] for worker in workers],
                   independent_consumer_source_age=native["observer"]["frame_age"]["channels"],
                   present_flags_counts=flags, present_mode_counts=modes,
                   live_etw_observation=live, collector_loss_warnings=loss,
                   collector_loss_observation_source=loss_source,
                   final_capture_zero_loss_verified=False)
    return record, summary


def analyze(paths):
    require(len(paths) == 3, "comparison requires exactly three captures")
    records, runs = [], []
    for path in paths:
        record, run = analyze_capture(Path(path))
        records.append(record)
        runs.append(run)
    failures = verify_cohort(records)
    verification_failures = [f"run {index + 1}: {failure}" for index, run in enumerate(runs)
                             for failure in run["verification_failures"]]
    return dict(schema_version=1, evidence_kind="output_only_presentation_aba_control_not_qualification",
                analyzer_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
                cohort_configuration_verified=not failures, configuration_failures=failures,
                complete_capture_observations=not verification_failures,
                verification_failures=verification_failures,
                shared_inputs_sha256=hashlib.sha256(
                    json.dumps(records[0]["inputs"], sort_keys=True).encode()).hexdigest(),
                runs=runs,
                scope=("Separate A/B/A measurements; one B sample is not three qualifying samples. "
                       "Matched configuration does not establish stationary performance or causal improvement. "
                       "No pooled cadence, full GPU critical path, physical scanout, optical latency "
                       "or final zero-loss claim."),
                denominator_scope=("Native attempts/readiness/Present outcomes, CSV drops and "
                                   "Display-window events remain separate; late counts can overlap other outcomes."))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("captures", nargs=3, type=Path, metavar="CAPTURE")
    args = parser.parse_args()
    try:
        result = analyze(args.captures)
    except (OSError, ValueError, KeyError, TypeError) as error:
        result = dict(schema_version=1, evidence_kind="invalid_presentation_control",
                      cohort_configuration_verified=False, complete_capture_observations=False,
                      analysis_failure=str(error))
    print(json.dumps(result, indent=2))
    return 0 if result["cohort_configuration_verified"] and result["complete_capture_observations"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
