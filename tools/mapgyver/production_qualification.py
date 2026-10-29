"""Capture/analyze the production Golden runtime; legacy probe schemas are not accepted."""

import argparse
from collections import Counter
import datetime
import hashlib
import json
import math
from pathlib import Path
import platform
import runpy
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[2]
CAPTURE_SCHEMA_FILE = "packages/golden-video-ui/tests/native/generated/capture-schema.json"
CAPTURE_SCHEMA_VERSION = json.loads((ROOT / CAPTURE_SCHEMA_FILE).read_text(encoding="utf-8"))["capture_schema_version"]
MIB = 1024 * 1024
SCENARIOS = {"Q01": ((1920, 1080), 512 * MIB, 1024 * MIB),
             "Q02": ((3840, 2160), 1024 * MIB, 2048 * MIB)}
RAW_FILES = ("manifest.json", "renderer-summary.json", "parent-summary.json", "renderer.jsonl",
             "source-1.jsonl", "source-2.jsonl", "consumer.jsonl")
DEADLINES = runpy.run_path(str(Path(__file__).with_name("production_deadlines.py")))
PREPARATION = runpy.run_path(str(Path(__file__).with_name("production_preparation.py")))
PLAN_AGE = runpy.run_path(str(Path(__file__).with_name("production_plan_age.py")))
COHORT = runpy.run_path(str(Path(__file__).with_name("production_cohort.py")))


class ArtifactError(ValueError):
    pass


def require(condition, message):
    if not condition:
        raise ArtifactError(message)


def integer(value, label, minimum=0, maximum=(1 << 64) - 1):
    require(type(value) is int and minimum <= value <= maximum, f"invalid {label}")
    return value


def load_json(path):
    def invalid(value):
        raise ArtifactError(f"nonfinite JSON number {value}")
    with path.open(encoding="utf-8-sig") as stream:
        return json.load(stream, parse_constant=invalid)


def load_rows(path):
    result = []
    with path.open(encoding="utf-8-sig") as stream:
        for number, line in enumerate(stream, 1):
            require(number <= 2_000_000 and len(line) <= 1024 * 1024, f"oversized raw stream {path.name}")
            require(line.endswith("\n"), f"incomplete raw row {path.name}:{number}")
            value = json.loads(line, parse_constant=lambda _: (_ for _ in ()).throw(ArtifactError("nonfinite JSON")))
            require(isinstance(value, dict) and isinstance(value.get("kind"), str), "raw row lacks kind")
            result.append(value)
    return result


def one(rows, kind):
    matches = [row for row in rows if row["kind"] == kind]
    require(len(matches) == 1, f"expected exactly one {kind} row")
    return matches[0]


def digest(path):
    result = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            result.update(chunk)
    return result.hexdigest()


def raw_hashes(directory):
    return {path.relative_to(directory).as_posix(): digest(path)
            for path in sorted(directory.rglob("*")) if path.is_file() and path.name != "analysis.json"}


def distribution(values):
    if not values:
        return None
    require(all(isinstance(value, (int, float)) and math.isfinite(value) and value >= 0 for value in values),
            "invalid timing observation")
    ordered = sorted(values)
    rank = lambda percentile: ordered[math.ceil(len(ordered) * percentile / 100) - 1]
    return {"samples": len(ordered), "median_ms": rank(50), "p95_ms": rank(95),
            "p99_ms": rank(99), "maximum_ms": ordered[-1]}


def token_key(token):
    require(isinstance(token, dict) and set(token) == {"device", "serial"}, "malformed completion token")
    device = token["device"]
    require(isinstance(device, dict) and set(device) == {"runtime", "generation"}
            and isinstance(device["runtime"], str), "malformed device lifetime")
    integer(device["generation"], "device generation")
    integer(token["serial"], "positive batch serial", 1)
    return json.dumps(token, sort_keys=True, separators=(",", ":"))


def interval(value, frequency):
    if value is None:
        return None
    begin = integer(value["begin"], "interval begin", 1)
    end = integer(value["end"], "interval end", begin)
    return (end - begin) * 1000 / frequency


def in_window(coordinate, window):
    return type(coordinate) is int and window["begin_qpc"] <= coordinate < window["end_qpc"]


def outcome(value):
    name = value if isinstance(value, str) else next(iter(value)) if isinstance(value, dict) and len(value) == 1 else None
    require(name in {"Accepted", "Busy", "Unavailable", "Failed", "DeviceLost"}, "unknown delivery disposition")
    return name


def events(rows):
    result = {name: [] for name in ("Plan", "Preparation", "FrameCpu", "Output", "SourceCpu", "SourceUse", "Gpu", "Schedule")}
    seen = set()
    for row in rows:
        if row["kind"] != "diagnostic":
            continue
        event = row["event"]
        require(isinstance(event, dict) and len(event) == 1, "invalid diagnostic enum")
        name, payload = next(iter(event.items()))
        require(name in result and isinstance(payload, dict), "unsupported diagnostic row")
        if name == "Plan":
            key = (name, json.dumps(payload["identity"], sort_keys=True), json.dumps(payload["stage"], sort_keys=True))
        elif name == "Preparation":
            key = (name, json.dumps(payload["generation"], sort_keys=True),
                   json.dumps(payload.get("wall"), sort_keys=True))
        elif name == "Schedule":
            key = (name, json.dumps(payload["device"], sort_keys=True), payload["cadence"],
                   integer(payload["frame_index"], "schedule index"), payload["disposition"] == "Fence",
                   payload["observed_monotonic_ns"] if payload["disposition"] == "Fence" else None)
        else:
            key = (name, token_key(payload["token"]), payload.get("output"), payload.get("texture"),
                   json.dumps(payload.get("context"), sort_keys=True))
        require(key not in seen, f"duplicate {name} diagnostic identity")
        seen.add(key)
        result[name].append(payload)
    return result


def health(snapshot, rows):
    count = sum(row["kind"] == "diagnostic" for row in rows)
    counters = {name: integer(snapshot.get(name), f"diagnostics {name}") for name in
                ("queued", "written", "drained", "dropped_full", "clock_failures")}
    complete = snapshot.get("enabled") is True and counters["queued"] == 0 \
        and counters["written"] == counters["drained"] == count \
        and all(counters[name] == 0 for name in ("dropped_full", "clock_failures"))
    return {"complete": complete, "raw_diagnostic_rows": count, **counters}


def cpu_metrics(rows, window, frequency):
    selected = [row for row in rows if in_window((row.get("wall") or {}).get("begin"), window)]
    walls = [interval(row["wall"], frequency) for row in selected]
    accounting = [(row["thread_cpu"]["kernel_100ns"] + row["thread_cpu"]["user_100ns"]) / 10_000
                  for row in selected if row.get("thread_cpu") is not None]
    return selected, {"inclusive_wall_upper_bound": distribution(walls),
                      "os_accounted_thread_cpu": distribution(accounting),
                      "thread_cpu_scope": "GetThreadTimes accounting granularity; not inferred by subtracting API wall time",
                      "missing_thread_accounting": len(selected) - len(accounting)}


def gpu_metrics(cpu_rows, gpu_rows, renderer=False):
    indexed = {(token_key(row["token"]), json.dumps(row["context"], sort_keys=True)): row for row in gpu_rows}
    states, durations, missing = Counter(), [], 0
    for cpu in cpu_rows:
        context = "Renderer" if renderer else cpu["context"]
        row = indexed.get((token_key(cpu["token"]), json.dumps(context, sort_keys=True)))
        if row is None:
            missing += 1
            continue
        status = row["status"] if isinstance(row["status"], str) else next(iter(row["status"]))
        states[status] += 1
        if status == "Valid":
            begin = integer(row["begin"], "GPU begin")
            end = integer(row["end"], "GPU end", begin)
            frequency = integer(row["frequency"], "GPU frequency", 1)
            durations.append((end - begin) * 1000 / frequency)
    return {"expected_batches": len(cpu_rows), "missing_rows": missing, "statuses": dict(states),
            "complete": bool(cpu_rows) and len(durations) == len(cpu_rows),
            "context_local_execution": distribution(durations),
            "scope": "one context only; never a cross-context critical path"}


def source_dependencies(frames, native):
    uses = {(token_key(row["token"]), row["texture"]): row for row in native["SourceUse"]}
    sources = {token_key(row["token"]): row for row in native["SourceCpu"]}
    gpu = {(token_key(row["token"]), json.dumps(row["context"], sort_keys=True)): row for row in native["Gpu"]}
    missing = []
    for frame in frames:
        identity = token_key(frame["token"])
        for texture in (1, 2):
            use = uses.get((identity, texture))
            publication = use.get("publication") if use else None
            source = sources.get(token_key(publication["source_token"])) if publication else None
            valid = source is not None and use.get("sequence", 0) > 0
            if valid:
                declared = source.get("context")
                context = declared.get("Source", {}) if isinstance(declared, dict) else {}
                valid = context.get("texture") == texture and context.get("renderer_device") == frame["token"]["device"]
                row = gpu.get((token_key(source["token"]), json.dumps(source["context"], sort_keys=True)))
                valid = valid and row is not None and row.get("status") == "Valid"
            if not valid:
                missing.append({"renderer_token": frame["token"], "texture": texture})
    return {"expected_references": len(frames) * 2, "missing_or_invalid": missing,
            "complete": bool(frames) and not missing,
            "critical_path_status": "UNAVAILABLE",
            "reason": "exact dependency identities do not calibrate separate GPU-context clocks or establish critical-path elapsed time"}


def marker_metrics(source_rows, consumer_rows, window, frequency):
    published = {}
    source_frequencies = []
    consumer_config = one(consumer_rows, "consumer_config")
    for source_id, rows in enumerate(source_rows, 1):
        config = one(rows, "source_config")
        previous = (0, 0)
        source_frequencies.append(config["qpc_frequency"])
        for row in rows:
            if row["kind"] != "source_frame":
                continue
            submission = row["submission"]
            stamp = submission["stamp"]
            require(stamp["source"] == source_id, "source packet identity differs from its role")
            frame = integer(stamp["frame"], "source frame", 1)
            integer(stamp["qpc"], "source QPC", 1)
            require(frame > previous[0] and stamp["qpc"] >= previous[1], "producer stamp order regressed")
            previous = (frame, stamp["qpc"])
            token_key(submission["token"])
            require(submission["token"]["device"]["runtime"] == config["source"]["instance"],
                    "producer token has a foreign lifetime")
            require(submission["token"]["serial"] == frame, "producer token differs from encoded frame")
            key = (source_id, frame)
            require(key not in published, "duplicate producer frame identity")
            published[key] = (stamp, row.get("marker_words"), outcome(submission["outcome"]))
    require(source_frequencies == [frequency, frequency]
            and consumer_config["qpc_frequency"] == frequency, "fixture QPC frequencies differ")
    channels = [{"valid": 0, "invalid": [], "repeated": 0, "unique_frames": 0,
                 "skipped_source_frame_ids": 0, "missing_canonical_words": 0} for _ in range(2)]
    ages, previous, seen_rows = [[], []], [None, None], set()
    for row in consumer_rows:
        if row["kind"] != "consumer_frame":
            continue
        frame = row["frame"]
        token_key(frame["token"])
        require(frame["token"]["device"]["runtime"] == consumer_config["consumer"]["instance"],
                "consumer row has a foreign lifetime")
        index = integer(frame["row"], "consumer row")
        require(index not in seen_rows, "duplicate consumer GPU-log row")
        seen_rows.add(index)
        if not in_window(frame.get("receive_begin_qpc"), window):
            continue
        raw = frame.get("raw_words")
        require(isinstance(raw, list) and len(raw) == 12, "consumer raw marker width differs")
        for word in raw:
            integer(word, "raw marker word", maximum=(1 << 32) - 1)
        for channel, name in enumerate(("left", "right")):
            result = frame.get(name, {})
            stamp = result.get("Ok") if isinstance(result, dict) and set(result) == {"Ok"} else None
            matched = published.get((channel + 1, stamp.get("frame"))) if isinstance(stamp, dict) else None
            completed = frame.get("completion_observed_qpc")
            reason = None
            if not matched:
                reason = "invalid marker or missing producer frame"
            elif stamp != matched[0] or stamp.get("source") != channel + 1 or matched[2] != "Accepted":
                reason = "decoded stamp differs from accepted producer record"
            elif matched[1] is None:
                channels[channel]["missing_canonical_words"] += 1
                reason = "producer omitted canonical Rust marker words"
            elif raw[channel * 6:(channel + 1) * 6] != matched[1]:
                reason = "raw GPU words differ from canonical producer words"
            elif type(completed) is not int or completed < stamp["qpc"]:
                reason = "missing or future completion observation"
            elif previous[channel] is not None and stamp["frame"] < previous[channel]:
                reason = "source frame regressed"
            if reason:
                channels[channel]["invalid"].append({"row": index, "reason": reason, "result": result,
                                                     "raw_words": raw[channel * 6:(channel + 1) * 6]})
                continue
            earlier = previous[channel]
            if stamp["frame"] == earlier:
                channels[channel]["repeated"] += 1
            else:
                channels[channel]["unique_frames"] += 1
                if earlier is not None:
                    channels[channel]["skipped_source_frame_ids"] += stamp["frame"] - earlier - 1
            previous[channel] = stamp["frame"]
            channels[channel]["valid"] += 1
            ages[channel].append((completed - stamp["qpc"]) * 1000 / frequency)
    for channel in range(2):
        channels[channel]["frame_age"] = distribution(ages[channel])
    return {"channels": channels,
            "scope": "producer CPU stamp to independent consumer's real GPU-completion observation; no scanout",
            "complete": all(channel["valid"] > 0 and not channel["invalid"] for channel in channels)}


def criterion(passed, eligible, owner_phase="M03", owner_gate=9, **evidence):
    return {"status": "UNAVAILABLE" if passed is None or not eligible else "PASSED_NATIVE" if passed else "FAILED_NATIVE",
            "observed_within_target": passed, "owner_phase": owner_phase, "owner_gate": owner_gate, **evidence}


def producer_metrics(streams, window, frequency):
    expected = (window["end_qpc"] - window["begin_qpc"]) * 60 / frequency
    reports = []
    for source, rows in enumerate(streams, 1):
        frames = [row["submission"] for row in rows if row["kind"] == "source_frame"
                  and in_window(row["submission"]["stamp"]["qpc"], window)]
        counts = Counter(outcome(frame["outcome"]) for frame in frames)
        shortfall = max(0, expected - counts["Accepted"])
        stamps = [frame["stamp"]["qpc"] for frame in frames]
        reports.append({"source": source, "counts": dict(counts), "expected_at_60hz": expected,
                        "accepted_shortfall_fraction": shortfall / expected,
                        "submission_intervals": distribution([(right - left) * 1000 / frequency
                                                               for left, right in zip(stamps, stamps[1:])]),
                        "within_target": bool(frames) and shortfall / expected <= 0.001
                                         and not counts["Failed"] and not counts["DeviceLost"]})
    return reports


def independence_metrics(manifest, summary, renderer, source_rows, consumer, frequency, native, consumer_events):
    reports = []
    requested = manifest.get("independence_seconds", 0)
    for name in ("editor_minimized", "editor_disconnected"):
        windows = [window for window in summary["phase_windows"] if window["name"] == name]
        if not windows:
            reports.append({"phase": name, "complete": False, "reason": "phase not captured"})
            continue
        require(len(windows) == 1, "duplicate independence phase")
        window = windows[0]
        duration = (window["end_qpc"] - window["begin_qpc"]) / frequency
        require(duration == requested, "independence phase duration differs from manifest")
        advances = [row for row in renderer if row["kind"] == "phase_outputs" and row["name"] == name]
        require(len(advances) == 1, "missing independence output token observations")
        before, after = advances[0]["before"], advances[0]["after"]
        advanced = len(before) == len(after) == 2 and all(
            left and right and left["device"] == right["device"] and right["serial"] > left["serial"]
            for left, right in zip(before, after))
        counts = {output: Counter(outcome(row["event"]["Output"]["outcome"]) for row in renderer
                                  if row["kind"] == "diagnostic" and "Output" in row["event"]
                                  and row["event"]["Output"]["output"] == output
                                  and in_window((row["event"]["Output"].get("delivery") or {}).get("end"), window))
                  for output in (1, 2)}
        age = marker_metrics(source_rows, consumer, window, frequency)
        cohort = COHORT["analyze_cohort"](native, source_rows, consumer, consumer_events, window, frequency, distribution)
        expected = duration * 60
        enough = duration > 0 and all(count["Accepted"] >= expected * 0.999
                                     and not count["Failed"] and not count["DeviceLost"] for count in counts.values())
        enough = enough and age["complete"] and all(channel["valid"] >= expected * 0.999 for channel in age["channels"])
        enough = enough and all(channel["frame_age"] and channel["frame_age"]["p95_ms"] <= 50
                                for channel in age["channels"])
        enough = enough and cohort["coverage_complete"] and cohort["observed_age_p95_within_budget"]
        reports.append({"phase": name, "duration_seconds": duration, "main_outputs_advanced": advanced,
                        "output_counts": counts, "frame_age": age, "spout_output_cohort": cohort,
                        "complete": advanced and enough})
    return {"phases": reports, "complete": requested == 60 and all(report["complete"] for report in reports)}


def resource_metrics(manifest, summary, renderer, window, frequency):
    _, ceiling, private_limit = SCENARIOS[manifest["scenario"]]
    requests = [row for row in renderer if row["kind"] == "adapter_memory" and in_window(row["qpc"], window)]
    resources = [row["snapshot"] for row in requests if row.get("snapshot") is not None]
    renderer_pid = manifest["pid"]
    all_processes = [row["snapshot"] for row in renderer if row["kind"] == "process_resources"
                     and row.get("snapshot") is not None]
    processes = [row for row in all_processes if row["process_id"] == renderer_pid]
    sample_processes = [row for row in processes if in_window(row["observed_qpc"], window)]
    minimum_rows = max(1, int(manifest["sample_seconds"]) - 1)
    budgets = [row["local"]["budget_bytes"] for row in resources if row.get("local") is not None]
    local_complete = len(resources) == len(requests) and len(resources) >= minimum_rows \
        and len(budgets) == len(resources) and all(budget > 0 for budget in budgets)
    for row in resources:
        require(row["adapter"] == manifest["adapter"] and row["process_id"] == renderer_pid
                and row["qpc_frequency"] == frequency, "DXGI observation identity differs")
    private = [row["memory"]["private_bytes"] for row in sample_processes if row.get("memory") is not None]
    unknown_processes = sum(row["kind"] == "process_resources" and row.get("snapshot") is None for row in renderer)
    private_complete = not unknown_processes and len(sample_processes) >= minimum_rows and len(private) == len(sample_processes)
    creations = {row["times"]["creation_filetime_100ns"] for row in processes if row.get("times") is not None}
    require(len(creations) <= 1, "renderer PID creation time changed during capture")
    final = summary["final_runtime"]["resources"]
    peak, limit = final["peak_bytes"], final["byte_limit"]
    observed_peak = max((row["local"]["current_usage_bytes"] for row in resources if row.get("local")), default=None)
    fixtures = []
    for role, pid in zip(("source_a", "source_b", "consumer"), manifest["fixture_pids"]):
        samples = [row for row in all_processes if row["process_id"] == pid
                   and in_window(row["observed_qpc"], window)]
        def field_peak(field):
            return max((row["memory"][field] for row in samples
                        if row.get("memory") is not None and field in row["memory"]), default=None)
        fixtures.append({"role": role, "process_id": pid, "sample_count": len(samples),
                         "private_peak_bytes": field_peak("private_bytes"),
                         "working_set_peak_bytes": field_peak("working_set_bytes"),
                         "handle_peak": max((row["handle_count"] for row in samples
                                             if row.get("handle_count") is not None), default=None)})
    return {"logical_reserved_peak_bytes": peak, "logical_workload_ceiling_bytes": ceiling,
            "logical_within_target": peak <= ceiling,
            "admission_limit_bytes": limit, "minimum_observed_local_budget_bytes": min(budgets) if budgets else None,
            "local_budget_coverage_complete": local_complete,
            "admission_within_target": limit <= min(ceiling, min(budgets) // 2) if local_complete else None,
            "renderer_local_usage_peak_bytes": observed_peak,
            "renderer_private_peak_bytes": max(private) if private else None,
            "renderer_private_coverage_complete": private_complete,
            "renderer_private_within_target": max(private) <= private_limit if private_complete else None,
            "renderer_handle_counts": [{"qpc": row["observed_qpc"], "count": row.get("handle_count")} for row in processes],
            "fixture_processes_excluded_from_renderer_private_total": True,
            "fixture_process_memory": fixtures,
            "webview_process_memory": None,
            "unknown_process_resource_rows": unknown_processes,
            "unknown_adapter_memory_rows": sum(row["kind"] == "adapter_memory" and row.get("snapshot") is None for row in renderer)}


def analyze_capture(directory):
    directory = Path(directory)
    manifest, summary, parent = [load_json(directory / name) for name in RAW_FILES[:3]]
    require(manifest.get("schema_version") == parent.get("schema_version") == CAPTURE_SCHEMA_VERSION, "unsupported production capture schema")
    require(manifest.get("scenario") in SCENARIOS, "not a Q01/Q02 production capture")
    frequency = integer(manifest["qpc_frequency"], "QPC frequency", 1)
    renderer, source_a, source_b, consumer = [load_rows(directory / name) for name in RAW_FILES[3:]]
    capture = one(renderer, "capture")
    require(capture["process_id"] == manifest["pid"] and capture["adapter"] == manifest["adapter"]
            and capture["scenario"] == manifest["scenario"] and capture["qpc_frequency"] == frequency,
            "renderer capture identity differs from manifest")
    require(summary["process_id"] == manifest["pid"], "summary belongs to another renderer")
    require(capture["requested_warmup_seconds"] == manifest["warmup_seconds"]
            and capture["requested_sample_seconds"] == manifest["sample_seconds"]
            and capture["requested_independence_seconds"] == manifest["independence_seconds"],
            "host and parent requested different workload durations")
    windows = summary["phase_windows"]
    warmup = one([{"kind": window["name"], **window} for window in windows], "warmup")
    integer(warmup["begin_qpc"], "warmup begin", 1)
    require((integer(warmup["end_qpc"], "warmup end", warmup["begin_qpc"]) - warmup["begin_qpc"]) / frequency
            == manifest["warmup_seconds"], "warmup clock/duration differs from manifest")
    sample = one([{"kind": window["name"], **window} for window in windows], "sample")
    integer(sample["begin_qpc"], "sample begin", 1)
    integer(sample["end_qpc"], "sample end", sample["begin_qpc"] + 1)
    duration = (sample["end_qpc"] - sample["begin_qpc"]) / frequency
    require(duration == manifest["sample_seconds"], "sample clock/duration differs from manifest")
    eligible = manifest["warmup_seconds"] == 30 and duration == 120 \
        and manifest.get("debug_assertions") is False and manifest.get("arch") == "x86_64"
    native, source_events, consumer_events = events(renderer), [events(source_a), events(source_b)], events(consumer)
    require(all(row["output"] in (1, 2, 3) for row in native["Output"]), "unknown renderer output binding")
    finals = [one(source_a, "source_final"), one(source_b, "source_final"), one(consumer, "consumer_final")]
    healths = [health(summary["diagnostics"], renderer)] + [health(final["diagnostics"], rows)
                for final, rows in zip(finals, (source_a, source_b, consumer))]
    complete = all(item["complete"] for item in healths) and summary.get("fixture_report_loss") == 0
    deadlines = DEADLINES["analyze_deadlines"](native["Schedule"], native["Output"], sample, frequency)
    deadline_verdict = deadlines["submission_verdict"]
    if not healths[0]["complete"] and deadline_verdict is True:
        deadline_verdict = None
    frame_rows, cpu = cpu_metrics(native["FrameCpu"], sample, frequency)
    preparation = PREPARATION["analyze_preparation"](native["Preparation"], native["FrameCpu"], frequency, distribution)
    plan_age = PLAN_AGE["analyze_plan_age"](native["Plan"], native["Preparation"], native["FrameCpu"],
                                          native["Output"], sample, frequency, distribution)
    renderer_gpu = gpu_metrics(frame_rows, native["Gpu"], renderer=True)
    dependencies = source_dependencies(frame_rows, native)
    markers = marker_metrics((source_a, source_b), consumer, sample, frequency)
    cohort = COHORT["analyze_cohort"](native, (source_a, source_b), consumer, consumer_events, sample, frequency, distribution)
    producers = producer_metrics((source_a, source_b), sample, frequency)
    independence = independence_metrics(manifest, summary, renderer, (source_a, source_b), consumer, frequency,
                                       native, consumer_events)
    consumer_final = finals[2]["consumer"]
    consumer_complete = consumer_final.get("final_readbacks") == 1 and consumer_final.get("finished") is True \
        and consumer_final.get("overflow") == consumer_final.get("mismatch") == consumer_final.get("failed_receives") == 0 \
        and consumer_final.get("device_removed") is False
    marker_count = min(channel["valid"] for channel in markers["channels"])
    markers["expected_samples_at_60hz"] = duration * 60
    markers["capture_count_at_least_nominal"] = consumer_complete and marker_count >= duration * 60 * 0.999
    markers["output_frame_cohort_coverage_complete"] = False
    markers["spout_output_cohort_coverage_complete"] = consumer_complete and cohort["coverage_complete"]
    local_contexts = []
    for role, values in [("renderer_sources", native), ("producer_a", source_events[0]),
                         ("producer_b", source_events[1]), ("consumer", consumer_events)]:
        contexts = {json.dumps(row["context"], sort_keys=True) for row in values["SourceCpu"]}
        for context in sorted(contexts):
            selected = [row for row in values["SourceCpu"] if json.dumps(row["context"], sort_keys=True) == context]
            selected, metric = cpu_metrics(selected, sample, frequency)
            local_contexts.append({"role": role, "context": json.loads(context), "cpu": metric,
                                   "gpu": gpu_metrics(selected, values["Gpu"])})
    output_metrics = []
    for output_id in (1, 2, 3):
        rows = [row for row in native["Output"] if row["output"] == output_id
                and in_window((row.get("delivery") or {}).get("end"), sample)]
        counts = Counter(outcome(row["outcome"]) for row in rows)
        accepted = sorted(row["delivery"]["end"] for row in rows if outcome(row["outcome"]) == "Accepted")
        expected = duration * 60
        shortfall = max(0, expected - len(accepted))
        intervals = [(end - begin) * 1000 / frequency for begin, end in zip(accepted, accepted[1:])]
        api = [interval(row["consumer_api"], frequency) for row in rows if row.get("consumer_api") is not None]
        output_metrics.append({"output": output_id, "counts": dict(counts), "expected_at_60hz": expected,
                               "accepted_shortfall": shortfall, "accepted_shortfall_fraction": shortfall / expected,
                               "accepted_call_intervals": distribution(intervals), "consumer_api_wall": distribution(api),
                               "within_target": bool(rows) and shortfall / expected <= 0.001
                                                and not counts["Failed"] and not counts["DeviceLost"]})
    extent = list(SCENARIOS[manifest["scenario"]][0])
    source_configs = [one(source_a, "source_config"), one(source_b, "source_config")]
    geometry = summary.get("geometry") or {}
    sizes = [view.get("physical_size") for view in geometry.get("views", [])]
    geometry_ok = len(sizes) == 2 and all(len(size) == 2 and all(abs(actual - expected) <= 1 for actual, expected in zip(size, target))
        for size, target in zip(sizes, ([640, 360], [480, 270]))) and summary["window"]["backing"] == extent \
        and summary["window"]["client"] == extent
    for index, config in enumerate(source_configs):
        descriptor = config["source"]["descriptor"]
        wanted = extent if index == 0 else [1280, 720]
        geometry_ok = geometry_ok and [descriptor["extent"]["width"], descriptor["extent"]["height"]] == wanted \
            and descriptor["format"] == "Bgra8Unorm" and descriptor["color_encoding"] == "Srgb" and descriptor["alpha_mode"] == "Straight"
        require(config["source"]["adapter"] == manifest["adapter"], "source selected a different adapter")
    require([config["pid"] for config in source_configs] + [one(consumer, "consumer_config")["pid"]] == manifest["fixture_pids"],
            "fixture PID identities differ from manifest")
    require(len(set([manifest["pid"], *manifest["fixture_pids"]])) == 4, "fixtures are not independent processes")
    resources = resource_metrics(manifest, summary, renderer, sample, frequency)
    inputs = load_json(directory / "inputs.json") if (directory / "inputs.json").exists() else None
    unchanged = inputs["before"] == inputs["after"] if inputs else None
    active = summary["active_runtime"]
    final = summary["final_runtime"]
    retired = summary["retirement"]
    ownership_ok = retired.get("actual_retirement") is True and retired.get("both_hwnds_retained_until_retirement") is True \
        and retired.get("output_destroyed_after_retirement") is True and final["state"] == "Stopped" \
        and final["resources"]["bytes_used"] == final["resources"]["in_flight"] == 0
    shutdown_ms = (integer(parent["qpc"], "parent shutdown QPC", retired["begin_qpc"]) - retired["begin_qpc"]) * 1000 / frequency
    children_ok = len(parent["children"]) == 3 and all(
        child.get("success") is True and child.get("forced") is False for child in parent["children"])
    require(sorted(child["pid"] for child in parent["children"]) == sorted(manifest["fixture_pids"]),
            "retirement belongs to different fixture processes")
    launch = load_json(directory / "launcher.json") if (directory / "launcher.json").exists() else None
    launcher_clock = launch and launch.get("exit_observed_qpc") is not None and launch.get("qpc_frequency") == frequency
    if launcher_clock:
        ended = integer(launch["exit_observed_qpc"], "launcher exit QPC", retired["begin_qpc"])
        shutdown_ms = (ended - retired["begin_qpc"]) * 1000 / frequency
    launcher_ok = launch is not None and launch.get("exit_code") == 0 and launch.get("timed_out") is False \
        and launch.get("forced_cleanup") is False and launch.get("owned_job_active_before_close") == 0
    functional = summary.get("functional_completed") is True and parent.get("renderer_returned_ok") is True \
        and active["state"] == "Running" and children_ok and (launch is None or launcher_ok)
    cpu_distribution = cpu["inclusive_wall_upper_bound"]
    cpu_ok = cpu_distribution is not None and cpu_distribution["p95_ms"] <= 4 and cpu_distribution["p99_ms"] <= 8
    age_ok = markers["complete"] and all(channel["frame_age"]["p95_ms"] <= 50 for channel in markers["channels"])
    criteria = {
        "functional_completion": criterion(functional, True),
        "fixed_scene": criterion(geometry_ok, eligible),
        "telemetry_coverage": criterion(complete, eligible, roles=healths),
        "input_identity": criterion(unchanged, True),
        "gpu_context_coverage": criterion(True if renderer_gpu["complete"] and dependencies["complete"]
                                          and all(context["gpu"]["complete"] for context in local_contexts) else None, eligible),
        "cpu_submission": criterion(True if cpu_ok and complete else None, eligible,
                                    reason="inclusive wall upper bound; exceeding it does not isolate CPU work from opaque API waits"),
        "output_api_cadence": criterion(all(row["within_target"] for row in output_metrics) if complete else None, eligible),
        "producer_api_cadence": criterion(all(row["within_target"] for row in producers) if complete else None, eligible),
        "submission_deadlines": criterion(deadline_verdict, eligible,
            reason="anchored native API observations; early acceptance does not prove GPU/display completion"),
        "deadline_misses": criterion(False if deadlines["definite_failure_lower_bound_exceeds_budget"] else None, eligible,
            reason="definite unsubmitted/late-start lower bounds can fail; early APIs cannot qualify normative output deadlines"),
        "editor_independence": criterion(independence["complete"] if complete else None,
                                         manifest.get("independence_seconds") == 60),
        "source_to_consumer_age": criterion(None, eligible,
            observed_capture_p95_within_budget=age_ok,
            reason="exact output-frame cohort measured for Spout only; native-window presentation age remains unavailable"),
        "spout_capture_identity": criterion(cohort["coverage_complete"] if complete and consumer_complete else None, eligible,
            scope="sample-accepted Spout output images, exact renderer source-use versions and GPU marker contents"),
        "spout_capture_age": criterion(cohort["observed_age_p95_within_budget"]
            if complete and consumer_complete and cohort["coverage_complete"] else None, eligible,
            reason="one first completed independent capture per accepted Spout image; missing images cannot be omitted"),
        "source_publication_age": criterion(cohort["source_publication_age"]["coverage_complete"] if complete else None, eligible,
            scope="CPU source-publication/receive to output API boundaries; no GPU/presentation queue budget inferred"),
        "gpu_critical_path": criterion(None, eligible, reason=dependencies["reason"]),
        "logical_texture_charge": criterion(resources["logical_within_target"], eligible),
        "dxgi_admission": criterion(resources["admission_within_target"], eligible),
        "renderer_private_memory": criterion(resources["renderer_private_within_target"], eligible),
        "completion_retirement": criterion(ownership_ok and children_ok, True),
        "shutdown": criterion(shutdown_ms <= 5000 if launcher_ok and launcher_clock and children_ok else None, eligible,
                              scoped_elapsed_ms=shutdown_ms),
        "renderer_handle_growth": criterion(None, eligible,
            required_handle_delta_maximum=10, required_private_growth_bytes=32 * MIB, required_drain_seconds=10,
            reason="applicable renderer lifecycle/resource-return series is absent; timing samples do not verify its handle/private-growth budgets"),
        "physical_display_dpi_input": criterion(None, eligible, owner_phase="M05", owner_gate=None,
                                                reason="M05-owned physical observations are absent"),
        "Q05_handles_growth": criterion(None, eligible, owner_phase="M09", owner_gate=None,
            reason="full-app Q05, including Matrix launches and transitions, has not been captured"),
        "native_preparation": criterion(preparation["coverage_complete"] if complete else None, eligible,
                                        scope="native preparation/first-use coverage; no latency budget inferred"),
        "plan_publication_age": criterion(plan_age["coverage_complete"] if complete else None, eligible,
                                          scope=plan_age["scope"]),
        "plan_age_and_published_queue": criterion(None, eligible, reason="CPU plan publication/receipt/install/first-use and source retention to output APIs measured; GPU/presentation queue age remains absent"),
        "environment_identity": criterion(None, eligible,
                                          reason="selected MSVC/SDK, driver, WebView, power and overlay observations are not pinned"),
    }
    return {"directory": str(directory), "manifest": manifest, "prescribed_sample": eligible,
            "criteria": criteria, "cpu": cpu, "renderer_gpu": renderer_gpu, "other_contexts": local_contexts,
            "dependencies": dependencies, "outputs": output_metrics, "producers": producers,
            "submission_deadlines": deadlines,
            "native_preparation": preparation,
            "plan_publication_age": plan_age,
            "spout_output_cohort": cohort,
            "frame_age": markers, "resources": resources,
            "independence": independence,
            "active_runtime_error_history": active.get("failure"), "raw_sha256": raw_hashes(directory),
            "input_fingerprints": inputs,
            "limitations": ["API acceptance is not displayed cadence or scanout", "GPU contexts are never summed into a critical path",
                            "WebView child-process memory is unavailable",
                            "warmed OS samples are sparse observations, not continuous physical-residency peaks"]}


def analyze_series(directories, diagnostic=False):
    captures = []
    for directory in directories:
        try:
            captures.append(analyze_capture(directory))
        except (ValueError, KeyError, TypeError, OSError) as error:
            captures.append({"directory": str(directory), "prescribed_sample": False, "artifact_error": str(error),
                             "criteria": {"artifact_integrity": criterion(False, True)},
                             "raw_sha256": raw_hashes(Path(directory)) if Path(directory).is_dir() else {}})
    manifests = [capture.get("manifest") for capture in captures]
    same = bool(manifests) and all(manifest is not None for manifest in manifests) and all(
        (manifest["scenario"], manifest["adapter"]) == (manifests[0]["scenario"], manifests[0]["adapter"]) for manifest in manifests)
    independent = same and len({manifest.get("run_id") for manifest in manifests}) == len(manifests)
    identities = [capture.get("input_fingerprints") for capture in captures]
    pinned = all(identity and identity["before"] == identity["after"] for identity in identities)
    same_inputs = pinned and all(identity["before"] == identities[0]["before"] for identity in identities)
    prescribed = not diagnostic and len(captures) == 3 and independent and same_inputs \
        and all(capture["prescribed_sample"] for capture in captures)
    statuses = [item["status"] for capture in captures for item in capture["criteria"].values()]
    full_passed = prescribed and bool(statuses) and all(item == "PASSED_NATIVE" for item in statuses)
    status = "PASSED_NATIVE" if full_passed else "FAILED_NATIVE" if "FAILED_NATIVE" in statuses else "UNAVAILABLE"
    renderer_items = [(name, item) for capture in captures for name, item in capture["criteria"].items()
                      if item["owner_phase"] == "M03" and item["owner_gate"] == 9]
    renderer_statuses = [item["status"] for _, item in renderer_items]
    renderer_passed = prescribed and bool(renderer_statuses) and all(item == "PASSED_NATIVE" for item in renderer_statuses)
    renderer_status = "PASSED_NATIVE" if renderer_passed else "FAILED_NATIVE" \
        if "FAILED_NATIVE" in renderer_statuses else "UNAVAILABLE"
    return {"schema_version": CAPTURE_SCHEMA_VERSION, "evidence_kind": "production_golden_runtime_qualification",
            "scope": "prescribed_series" if prescribed else "diagnostic_incomplete_series",
            "prescribed_three_sample_series": prescribed, "status": status, "captures": captures,
            "renderer_qualification": {"owner_phase": "M03", "owner_gate": 9, "status": renderer_status,
                                       "qualified": renderer_passed,
                                       "required_criteria": sorted({name for name, _ in renderer_items})},
            "qualified": full_passed}


def fingerprints(executable):
    paths = [ROOT / name for name in ("Cargo.toml", "Cargo.lock", "package.json", "package-lock.json", CAPTURE_SCHEMA_FILE)]
    paths += [Path(__file__), Path(__file__).with_name("production_process.py"),
              Path(__file__).with_name("production_preparation.py"),
              Path(__file__).with_name("production_plan_age.py"),
              Path(__file__).with_name("production_cohort.py"),
              Path(__file__).with_name("production_deadlines.py"), ROOT / "apps/mapgyver/Cargo.toml",
              ROOT / "crates/golden_time/Cargo.toml", ROOT / "crates/golden_video/Cargo.toml",
              ROOT / "apps/mapgyver/build.rs"]
    for directory in ("crates/golden_time/src", "crates/golden_video/src", "crates/golden_core/hosts/desktop/src",
                      "apps/mapgyver/tests/native_qualification", "packages/golden-video-ui"):
        paths += [path for path in (ROOT / directory).rglob("*") if path.suffix in (".rs", ".hlsl", ".ts", ".svelte", ".html")]
    vendor = ROOT / "vendor/spout2-sys-0.1.1"
    paths += [path for path in vendor.rglob("*") if path.suffix in (".rs", ".cpp", ".h", ".toml")]
    paths += [vendor / "UPSTREAM.json", vendor / "PATCHES.json"]
    def git(*arguments):
        result = subprocess.run(["git", *arguments], cwd=ROOT, capture_output=True, text=True, check=False)
        return result.stdout.strip() if result.returncode == 0 else None
    return {"executable_sha256": digest(executable), "git_head": git("rev-parse", "HEAD"),
            "git_status": git("status", "--porcelain=v1"),
            "execution_input_sha256": {path.relative_to(ROOT).as_posix(): digest(path) for path in sorted(set(paths)) if path.is_file()},
            "hash_scope": "selected execution sources/assets and locks; unrelated dirty files are listed but not hashed",
            "python": platform.python_version(), "os": platform.platform()}


def capture_command(args, directory):
    return [str(args.executable), "--role", "renderer", "--directory", str(directory),
            "--scenario", args.scenario, "--adapter", str(args.adapter), "--warmup", str(args.warmup),
            "--seconds", str(args.seconds), "--independence", str(args.isolation_seconds),
            "--trace-capacity", str(args.trace_capacity)]


def parse_args(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    analyze = commands.add_parser("analyze", help="offline only; preserve raw files and failures")
    analyze.add_argument("directories", type=Path, nargs="+")
    analyze.add_argument("--diagnostic", action="store_true")
    analyze.add_argument("--output", type=Path)
    run = commands.add_parser("run", help="launch newly owned native renderer/fixture processes")
    run.add_argument("--directory", type=Path, required=True)
    run.add_argument("--scenario", choices=SCENARIOS, default="Q01")
    run.add_argument("--adapter", type=int, default=0)
    run.add_argument("--profile", choices=("release", "debug"), default="release")
    run.add_argument("--executable", type=Path)
    run.add_argument("--samples", type=int, choices=(1, 2, 3), default=3)
    run.add_argument("--warmup", type=int, default=30)
    run.add_argument("--seconds", type=int, default=120)
    run.add_argument("--isolation-seconds", type=int, default=0, help="60 verifies each required interruption interval")
    run.add_argument("--trace-capacity", type=int, default=65536)
    run.add_argument("--diagnostic", action="store_true", help="required for short controls; never promotes them to qualification")
    args = parser.parse_args(argv)
    if args.command == "run":
        if args.adapter < 0 or args.seconds < 1 or args.warmup < 0 or args.isolation_seconds < 0 \
                or args.warmup + args.seconds + 2 * args.isolation_seconds > 600 or not 1 <= args.trace_capacity <= 65536:
            parser.error("invalid bounded adapter/duration/capacity")
        if not args.diagnostic and (args.samples, args.warmup, args.seconds, args.profile) != (3, 30, 120, "release"):
            parser.error("short/debug controls require --diagnostic; prescribed series is 3 x (30+120 seconds), release")
        args.executable = (args.executable or ROOT / "target" / args.profile / "native_qualification.exe").resolve()
        args.directory = args.directory.resolve()
        if not args.directory.is_relative_to((ROOT / "target").resolve()) or args.directory.exists():
            parser.error("capture directory must be a new directory below target; prior evidence is never overwritten")
    return args


def write_new(path, value):
    with path.open("x", encoding="utf-8", newline="\n") as stream:
        json.dump(value, stream, indent=2, allow_nan=False)
        stream.write("\n")


def run(args):
    from production_process import run_owned
    require(args.executable.is_file(), "production qualification executable is absent")
    args.directory.mkdir(parents=True, exist_ok=False)
    directories = []
    for index in range(1, args.samples + 1):
        directory = args.directory / f"sample-{index:03}"
        before = fingerprints(args.executable)
        command = capture_command(args, directory)
        print(f"Starting production {args.scenario} sample {index}/{args.samples}", file=sys.stderr, flush=True)
        with (args.directory / f"sample-{index:03}-stdout.log").open("xb") as stdout, \
                (args.directory / f"sample-{index:03}-stderr.log").open("xb") as stderr:
            try:
                execution = run_owned(command, args.warmup + args.seconds + args.isolation_seconds * 2 + 180, stdout, stderr)
            except (OSError, RuntimeError, subprocess.SubprocessError) as error:
                execution = {"exit_code": None, "timed_out": False, "forced_cleanup": True,
                             "watchdog_error": str(error), "owned_job_active_before_close": None}
        directory.mkdir(exist_ok=True)
        write_new(directory / "launcher.json", {**execution, "command": command,
                  "stdout_sha256": digest(args.directory / f"sample-{index:03}-stdout.log"),
                  "stderr_sha256": digest(args.directory / f"sample-{index:03}-stderr.log"),
                  "finished_utc": datetime.datetime.now(datetime.timezone.utc).isoformat()})
        after = fingerprints(args.executable)
        write_new(directory / "inputs.json", {"before": before, "after": after, "unchanged": before == after})
        directories.append(directory)
        if execution["exit_code"] != 0 or execution["timed_out"] or execution["forced_cleanup"] or before != after:
            break
    result = analyze_series(directories, args.diagnostic)
    write_new(args.directory / "analysis.json", result)
    print(json.dumps(result, allow_nan=False))
    return 1 if result["status"] == "FAILED_NATIVE" else 0


def main(argv=None):
    args = parse_args(argv)
    if args.command == "run":
        return run(args)
    result = analyze_series(args.directories, args.diagnostic)
    if args.output:
        write_new(args.output, result)
    print(json.dumps(result, allow_nan=False))
    return 1 if result["status"] == "FAILED_NATIVE" else 0


if __name__ == "__main__":
    sys.exit(main())
