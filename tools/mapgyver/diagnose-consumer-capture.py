"""Diagnose retained consumer-control failures without qualifying or rewriting their CSV."""

import argparse
import csv
import hashlib
import io
import json
from pathlib import Path
import runpy

HERE = Path(__file__).resolve().parent
PRESENTATION = runpy.run_path(str(HERE / "analyze-presentation.py"))
BASE = PRESENTATION["BASE"]
DATA = PRESENTATION["DATA"]
RELEASE = PRESENTATION["RELEASE"]
CONSUMER = runpy.run_path(str(HERE / "consumer_observation.py"))
STAGES = runpy.run_path(str(HERE / "gpu_stages.py"))
IMPORTS = runpy.run_path(str(HERE / "import_metrics.py"))
require = PRESENTATION["require"]
_ROLES = ("editor", "native_output")
_U64_MAX = (1 << 64) - 1


def _uint(value, label):
    require(type(value) is int and 0 <= value <= _U64_MAX, f"{label} must be an exact u64")
    return value


def _address(value):
    require(isinstance(value, str), "swapchain address must be text")
    match = DATA["_ADDRESS"].fullmatch(value)
    require(match is not None and len(match[1]) <= 16 and int(match[1], 16) > 0,
            "invalid swapchain address")
    return hex(int(match[1], 16))


def diagnose_csv(csv_text, expected_pid, swapchains, window):
    """Count rows and expose order violations in original CSV order, without interval metrics."""
    require(isinstance(csv_text, str) and 0 < len(csv_text) <= 128 * 1024 * 1024,
            "empty or oversized CSV")
    require(type(expected_pid) is int and 0 < expected_pid <= 0xffffffff, "invalid expected PID")
    begin, end, frequency = DATA["_window"](window)
    require(isinstance(swapchains, list) and len(swapchains) == 2
            and all(isinstance(chain, dict) for chain in swapchains)
            and {chain.get("role") for chain in swapchains} == set(_ROLES),
            "native swapchain role pair is missing or ambiguous")
    identities = []
    for chain in swapchains:
        lifetime_begin = _uint(chain.get("begin_qpc"), "native Present lifetime begin")
        lifetime_end = _uint(chain.get("end_qpc"), "native Present lifetime end")
        require(0 < lifetime_begin < lifetime_end, "invalid native Present lifetime")
        aliases = chain.get("addresses")
        require(isinstance(aliases, dict) and aliases, "native swapchain addresses are missing")
        identities.append(dict(role=chain["role"], begin=lifetime_begin, end=lifetime_end,
                               addresses={_address(value) for value in aliases.values()}))

    reader = csv.reader(io.StringIO(csv_text.removeprefix("\ufeff"), newline=""), strict=True)
    counts = {role: dict(raw_rows=0, sampled_rows=0, dropped_all=0, dropped_sampled=0,
                         rows_before_sample=0, rows_after_sample=0) for role in _ROLES}
    previous, duplicates, transitions = {}, {}, []
    total = 0
    try:
        headers = next(reader)
        canonical = [name.strip().casefold() for name in headers]
        require(headers and all(canonical) and len(set(canonical)) == len(canonical),
                "empty or duplicate CSV headers")
        columns = {name: index for index, name in enumerate(canonical)}
        require(all(name in columns for name in DATA["_REQUIRED"]), "required v1 CSV columns missing")
        next_line = reader.line_num + 1
        for index, fields in enumerate(reader):
            csv_line = next_line
            next_line = reader.line_num + 1
            total += 1
            require(total <= 1_000_000 and len(fields) == len(headers), f"malformed CSV data row {total}")
            pid = DATA["_integer"](fields[columns["processid"]], "ProcessID", 0xffffffff)
            require(pid == expected_pid, f"unexpected PID {pid}")
            address = _address(fields[columns["swapchainaddress"]])
            qpc = DATA["_integer"](fields[columns["qpctime"]], "QPCTime")
            matches = [identity for identity in identities if address in identity["addresses"]
                       and identity["begin"] <= qpc < identity["end"]]
            require(len(matches) == 1, "CSV row lacks exactly one native role/address/QPC lifetime match")
            role = matches[0]["role"]
            dropped = fields[columns["dropped"]]
            require(dropped in ("0", "1"), "Dropped must be 0 or 1")
            for name, column in columns.items():
                if name.startswith("ms") or name in ("timeinseconds", "displaylatency", "displayedtime"):
                    DATA["_number"](fields[column], headers[column])
            sampled = begin <= qpc < end
            row = dict(data_index=index, csv_line=csv_line, role=role, address=address,
                       qpc=qpc, dropped=dropped == "1", in_sample_window=sampled)
            count = counts[role]
            count["raw_rows"] += 1
            count["sampled_rows"] += sampled
            count["dropped_all"] += row["dropped"]
            count["dropped_sampled"] += sampled and row["dropped"]
            count["rows_before_sample"] += qpc < begin
            count["rows_after_sample"] += qpc >= end
            key = (address, qpc)
            duplicates.setdefault(key, []).append(row)
            prior = previous.get(address)
            if prior is not None and qpc <= prior["qpc"]:
                delta = qpc - prior["qpc"]
                transitions.append(dict(kind="backward" if delta < 0 else "equal",
                                        previous=prior, current=row, delta_qpc=delta,
                                        delta_ms=delta * 1000 / frequency))
            previous[address] = row
    except (csv.Error, StopIteration) as error:
        raise ValueError("malformed or missing CSV data") from error
    require(total > 0, "CSV contains no data rows")
    duplicate_groups = [dict(address=address, qpc=qpc, rows=rows, delta_qpc=0)
                        for (address, qpc), rows in duplicates.items() if len(rows) > 1]
    return dict(
        raw_rows=total, csv_text_utf8_sha256=hashlib.sha256(csv_text.encode("utf-8")).hexdigest(),
        sample_window_qpc=dict(begin=begin, end=end, frequency=frequency), roles=counts,
        original_order_preserved=True, present_qpc_order_valid=not transitions,
        backward_transitions=sum(item["kind"] == "backward" for item in transitions),
        equal_transitions=sum(item["kind"] == "equal" for item in transitions),
        present_qpc_transitions=transitions, duplicate_present_keys=duplicate_groups,
        order_dependent_metrics=None,
        scope="Raw/sample Present-row and Dropped-flag counts only; no rows sorted, removed or deduplicated; no Present/display interval or latency percentiles")


def diagnose(path):
    path = Path(path)
    raw = path.read_bytes()
    packet = BASE["decode_evidence"](json.loads(raw))
    for name in ("native", "presentmon", "etw_query"):
        PRESENTATION["verify_stream"](packet[name], name)
    native = json.loads(packet["native"]["stdout"])
    require(native["render_process_id"] == packet["native_pid"], "native PID differs")
    BASE["compact_resources"](native)
    require(native == packet["native_report"], "native stdout compaction differs")
    observation = CONSUMER["validate"](native, packet["requested_consumer_mode"])
    require(packet["consumer_observation_verified"] is True
            and observation == packet["consumer_observation"], "consumer observation differs")
    generations = native["generations"]
    require(len(generations) == 1 and generations[0]["generation"] == 0
            and generations[0]["end"] == "deadline_reached", "native timed generation is incomplete")
    generation = generations[0]
    PRESENTATION["CAPTURE"]["validate_gpu_stages"](native, packet["requested_gpu_stages"])
    stages = STAGES["analyze"](generation, packet["requested_gpu_stages"])
    operations = RELEASE["verify_operations"](generation)
    gate = RELEASE["verify_gpu_admission"](generation, native["resources"])
    presenter_admission = RELEASE["verify_presenter_admission"](generation)
    workers = RELEASE["verify_source_workers"](generation)
    imports = IMPORTS["validate_imports"](generation["renderer_source_imports"])
    require(all(item["all"]["polls"] == gate["submitted"]
                and item["sampled"]["polls"] == gate["sampled"]["admitted"] for item in imports),
            "renderer import polls differ from admitted batches")
    require(RELEASE["zero"](generation["allocations_after_fence"]), "renderer retained logical allocations")
    require(len(native["sources"]) == 2, "source fixture pair is missing")
    for child in [*native["sources"], native["observer"]]:
        require(RELEASE["zero"](child["resources"]["after_fence"]), "child retained logical allocations")
    csv_diagnosis = diagnose_csv(packet["presentmon"]["stdout"], packet["native_pid"],
                                 generation["swapchains"], native["sample_window"])
    for index, role in enumerate(_ROLES):
        count = csv_diagnosis["roles"][role]
        admission = presenter_admission[index]
        count["native_all_accepted"] = admission["all"]["accepted_presents"]
        count["native_sampled_accepted"] = admission["sampled"]["accepted_presents"]
        count["native_all_accepted_minus_raw_rows"] = count["native_all_accepted"] - count["raw_rows"]
        count["native_sampled_accepted_minus_sampled_rows"] = (
            count["native_sampled_accepted"] - count["sampled_rows"])
    expected_extent = [3840, 2160] if native["workload"]["scenario"] == "q02" else [1920, 1080]
    extent = generation["native_output_extent"]
    require(isinstance(extent, list) and len(extent) == 2
            and all(_uint(value, "native output dimension") > 0 for value in extent),
            "native output extent is malformed")
    mismatches = _uint(generation["sampled_geometry_mismatches"], "sampled geometry mismatches")
    fixed = generation["fixed_geometry_observed"]
    require(type(fixed) is bool, "fixed geometry observation must be a boolean")
    geometry = dict(valid=fixed and mismatches == 0 and extent == expected_extent,
                    fixed_geometry_observed=fixed, sampled_geometry_mismatches=mismatches,
                    expected_scenario_extent=expected_extent, native_output_extent=extent,
                    window_context={chain["role"]: PRESENTATION["window_context"](chain)
                                    for chain in generation["swapchains"]})
    query = json.loads(packet["etw_query"]["stdout"])
    live = PRESENTATION["CAPTURE"]["validate_etw_query"](query, packet["session_name"], native)
    flags = {name: packet[name] for name in (
        "route_verified", "consumer_observation_verified", "role_correlation_verified",
        "presentation_policy_verified", "live_etw_query_verified", "inputs_unchanged_after_run",
    )}
    require(all(type(value) is bool for value in flags.values()), "capture verification flags are malformed")
    return dict(
        schema_version=1, evidence_kind="consumer_capture_failure_diagnosis_not_qualification",
        qualification=False, artifact=path.name, artifact_sha256=hashlib.sha256(raw).hexdigest(),
        analyzer_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        validator_sha256={name: hashlib.sha256((HERE / name).read_bytes()).hexdigest()
                          for name in ("consumer_observation.py", "gpu_stages.py", "import_metrics.py")},
        captured_inputs=packet["inputs"], original_capture_flags=flags,
        original_validation_failure=packet.get("validation_failure"),
        verification=dict(stream_hashes_and_native_compaction=True,
                          native_operations_admission_and_zero_logical_ownership=True,
                          consumer_proof_and_stage_coverage=True, raw_csv_role_lifetimes=True),
        consumer_observation=observation, geometry=geometry, csv_diagnosis=csv_diagnosis,
        native_gpu_admission=gate, native_presenter_admission=presenter_admission,
        native_submission_editor_output_spout=[
            {name: path[name] for name in
             ("attempts", "accepted_calls", "busy", "occluded", "no_target", "late_attempts")}
            for path in generation["submission_editor_output_spout"]],
        native_operations=operations, renderer_source_imports=imports, gpu_stages=stages,
        source_worker_peak_surfaces=[worker["peak_surfaces"] for worker in workers],
        live_etw_observation_recomputed=live,
        collector_loss_warnings_recomputed=DATA["loss_observation"](
            packet["presentmon"]["stdout"], packet["presentmon"]["stderr"]),
        order_dependent_metrics=None,
        limits=["Original capture flags and raw CSV order remain unchanged",
                "Unordered row counts do not repair ordering or scene-geometry validation",
                "No Present/display intervals, software-latency percentiles or qualification pass",
                "Live loss counters are a snapshot, not final capture or analysis zero loss",
                "Stage intervals are elapsed bounds, not GPU busy time or a cross-context critical path"])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("capture", type=Path)
    args = parser.parse_args()
    print(json.dumps(diagnose(args.capture), indent=2, allow_nan=False))


if __name__ == "__main__":
    main()
