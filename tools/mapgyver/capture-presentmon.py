"""Bounded PID-filtered PresentMon route control; not full timing/scanout qualification."""

import argparse
import base64
import concurrent.futures
import csv
import datetime
import hashlib
import io
import json
from pathlib import Path
import platform
import runpy
import subprocess
import sys
import uuid

ROOT = Path(__file__).resolve().parents[2]
BASE = runpy.run_path(str(Path(__file__).with_name("run-qualification.py")))
PRESENTMON = ROOT / "target/qualification-tools/presentmon-2.6.0/PresentMon-2.6.0-x64.exe"
PRESENTMON_SHA256 = "b2a706bc6ad475749e3b7e3409263aa1e6906d45bdcf993f6dbc0f660188f1af"


def parse_args(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("adapter", type=int)
    parser.add_argument("scenario", choices=("q01", "q02"))
    parser.add_argument("--presentmon", type=Path, default=PRESENTMON)
    parser.add_argument("--nvidia-overlay", choices=("on", "off", "unknown"), default="unknown")
    parser.add_argument("--sample-seconds", type=int, default=8)
    parser.add_argument("--warmup-seconds", type=int, default=30)
    parser.add_argument("--collector-tail-seconds", type=int, default=10,
                        help="additional collector duration beyond native warmup/sample total (2..60 seconds)")
    parser.add_argument("--presentation-policy", choices=("latest", "output-vblank"), default="latest")
    parser.add_argument("--gpu-stages", action="store_true",
                        help="record diagnostic GPU stage timestamps with the latest presentation policy")
    parser.add_argument("--output", type=Path, help="exclusively create a generated packet under the existing target directory")
    args = parser.parse_args(argv)
    if args.adapter < 0:
        parser.error("adapter must be nonnegative")
    if args.sample_seconds < 8 or args.warmup_seconds < 0 or args.sample_seconds + args.warmup_seconds > 3600:
        parser.error("sample must be at least 8 seconds, warmup nonnegative, total at most 3600")
    if not 2 <= args.collector_tail_seconds <= 60:
        parser.error("collector tail must be between 2 and 60 seconds")
    if args.gpu_stages and args.presentation_policy != "latest":
        parser.error("GPU stage timing requires the latest presentation policy")
    if args.output is not None:
        args.output = args.output.resolve()
        if not args.output.is_relative_to((ROOT / "target").resolve()) or not args.output.parent.is_dir() \
                or args.output.exists():
            parser.error("output must be a new file below the existing target directory")
    return args


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def fingerprints(executable, presentmon):
    result = BASE["fingerprints"](executable)
    for name in ("capture-presentmon.py", "presentmon_data.py", "presentmon_roles.py"):
        path = Path(__file__).resolve().with_name(name)
        result["files"][path.relative_to(ROOT).as_posix()] = digest(path)
    result["presentmon_sha256"] = digest(presentmon)
    return result


def decode(data):
    return data.decode("utf-8-sig", errors="replace")


def validate_gpu_stages(native_report, requested):
    if native_report["workload"]["gpu_stages"] is not requested:
        raise ValueError("requested and native GPU stage selections differ")
    generations = native_report["generations"]
    if not generations or any((generation["timing"]["gpu_stages"] is not None) != requested
                              for generation in generations):
        raise ValueError("native GPU stage report presence differs from the requested selection")


def communicate(process, seconds, presentmon=None, session=None):
    cleanup = None
    try:
        stdout, stderr = process.communicate(timeout=seconds)
        timed_out = False
    except subprocess.TimeoutExpired:
        timed_out = True
        if presentmon is not None:
            # Only this invocation's freshly generated UUID is eligible for timeout cleanup.
            # Never stop the default session, an installed service, or a user-supplied name.
            cleanup_command = [str(presentmon), "--session_name", session, "--terminate_existing_session"]
            try:
                stopped = subprocess.run(cleanup_command, capture_output=True, timeout=5)
                cleanup = dict(command=cleanup_command, exit_code=stopped.returncode,
                               stdout=decode(stopped.stdout), stderr=decode(stopped.stderr))
            except (OSError, subprocess.SubprocessError) as error:
                cleanup = dict(command=cleanup_command, failure=str(error))
        if process.poll() is None:
            process.kill()
        stdout, stderr = process.communicate(timeout=5)
    valid_utf8 = True
    try:
        stdout.decode("utf-8-sig")
        stderr.decode("utf-8-sig")
    except UnicodeDecodeError:
        valid_utf8 = False
    return dict(exit_code=process.returncode, stdout=decode(stdout), stderr=decode(stderr),
                valid_utf8=valid_utf8,
                original_bytes_base64=(None if valid_utf8 else dict(stdout=base64.b64encode(stdout).decode("ascii"),
                                                                   stderr=base64.b64encode(stderr).decode("ascii"))),
                timed_out=timed_out, timeout_cleanup=cleanup,
                stdout_bytes=len(stdout), stdout_sha256=hashlib.sha256(stdout).hexdigest(),
                stderr_bytes=len(stderr), stderr_sha256=hashlib.sha256(stderr).hexdigest())


def validate_etw_query(query, session, native_report):
    if not isinstance(query, dict) or type(query.get("status")) is not int or query["status"] != 0 \
            or query.get("response_validation_error") is not None:
        raise ValueError("live ETW query failed or returned an invalid identity")
    statistics = query.get("statistics")
    if query.get("session_name") != session or not isinstance(statistics, dict) \
            or statistics.get("returned_session_name") != session:
        raise ValueError("live ETW session identity mismatch or unavailable statistics")
    before, after = query.get("before_qpc"), query.get("after_qpc")
    ends = [chain["end_qpc"] for generation in native_report["generations"] for chain in generation["swapchains"]]
    if not ends or any(type(value) is not int or not 0 < value < 1 << 64 for value in [before, after, *ends]) \
            or not max(ends) <= before <= after:
        raise ValueError("live ETW query does not follow the recorded native Present intervals")
    counters = {key: statistics.get(key) for key in ("events_lost", "log_buffers_lost", "realtime_buffers_lost")}
    if any(type(value) is not int or not 0 <= value < 1 << 32 for value in counters.values()):
        raise ValueError("live ETW loss counters are unavailable or malformed")
    return dict(before_qpc=before, after_qpc=after, native_present_interval_end_qpc=max(ends),
                queried_after_native_present_interval=True, counters=counters,
                all_counters_zero=not any(counters.values()), final_capture_zero_loss_verified=False,
                scope="Successful independent live API query only; later event delivery/shutdown and PresentMon analysis loss remain unverified")


def validate_presentation_policy(csv_text, requested, native_report, correlation):
    policies = {"latest": "latest", "output-vblank": "output_vblank"}
    policy = native_report["workload"]["presentation_policy"]
    if requested not in policies or policy != policies[requested]:
        raise ValueError("requested and native presentation policies differ")
    reader = csv.DictReader(io.StringIO(csv_text, newline=""))
    columns = {name.casefold(): name for name in (reader.fieldnames or [])}
    if not all(name in columns for name in ("swapchainaddress", "qpctime", "syncinterval")):
        raise ValueError("presentation policy requires address/QPC/SyncInterval CSV columns")
    checked = {"editor": 0, "native_output": 0}
    expected = {"editor": 0, "native_output": int(policy == "output_vblank")}
    for row in reader:
        address = hex(int(row[columns["swapchainaddress"]], 16))
        qpc = int(row[columns["qpctime"]])
        matches = [record for record in correlation["roles"]
                   if address in record["addresses"].values() and record["begin_qpc"] <= qpc < record["end_qpc"]]
        if len(matches) != 1:
            raise ValueError("presentation policy row lacks an exact native role match")
        role = matches[0]["role"]
        if role not in expected or row[columns["syncinterval"]] != str(expected[role]):
            raise ValueError("CSV SyncInterval does not match the requested output-only policy")
        checked[role] += 1
    declared = {role: sum(record["raw_rows"] for record in correlation["roles"] if record["role"] == role)
                for role in checked}
    if not all(checked.values()) or checked != declared:
        raise ValueError("presentation policy row coverage differs from native role correlation")
    return dict(native_policy=policy, requested_policy=requested, expected_sync_interval_by_role=expected,
                rows_checked_by_role=checked, scope="Exact CSV policy validation; no cadence or latency pass")


def collect(args, evidence):
    executable = ROOT / "target/release/mapgyver_native_probe.exe"
    session = "MapGyver-M01-Present-" + uuid.uuid4().hex
    total_seconds = args.sample_seconds + args.warmup_seconds
    capture_seconds = total_seconds + args.collector_tail_seconds
    evidence.update(schema_version=2, evidence_kind="bounded_presentmon_roles_and_live_loss_query_not_full_timing_qualification",
                    started_utc=datetime.datetime.now(datetime.timezone.utc).isoformat(),
                    session_name=session, native_sample_seconds=args.sample_seconds, native_warmup_seconds=args.warmup_seconds,
                    collector_tail_seconds=args.collector_tail_seconds,
                    presentmon_timed_seconds=capture_seconds, declared_nvidia_overlay_state=args.nvidia_overlay,
                    requested_presentation_policy=args.presentation_policy,
                    requested_gpu_stages=args.gpu_stages,
                    environment=dict(platform=platform.platform(), machine=platform.machine()),
                    role_correlation_verified=False, live_etw_query_verified=False, presentation_policy_verified=False,
                    scope="PID-filtered PresentMon v1 events with separate exact native identity correlation; no optical latency, alignment or full GPU-path claim")
    before = fingerprints(executable, args.presentmon)
    evidence["inputs"] = before
    if args.presentmon.resolve() == PRESENTMON.resolve() and before["presentmon_sha256"] != PRESENTMON_SHA256:
        raise ValueError("default portable PresentMon checksum differs from the pinned official release")
    help_result = subprocess.run([str(args.presentmon), "--help"], capture_output=True, timeout=5)
    evidence["presentmon_help"] = dict(exit_code=help_result.returncode, stdout=decode(help_result.stdout), stderr=decode(help_result.stderr))
    help_text = decode(help_result.stdout) + decode(help_result.stderr)
    required = ("--process_id", "--session_name", "--output_stdout", "--v1_metrics", "--qpc_time",
                "--no_console_stats", "--no_track_input", "--timed", "--terminate_after_timed")
    if help_result.returncode not in (0, 1) or not all(option in help_text for option in required):
        raise ValueError("installed PresentMon does not advertise all required options")
    inventory = subprocess.run([str(executable), "inventory"], capture_output=True, check=True, timeout=15)
    adapters = json.loads(decode(inventory.stdout))
    selected = [a for a in adapters if a["index"] == args.adapter and a["software"] is False]
    if len(selected) != 1:
        raise ValueError("selected hardware adapter missing")
    if "nvidia" in selected[0]["description"].lower() and args.nvidia_overlay != "off":
        raise ValueError("NVIDIA requires verified overlay Off before startup")
    evidence["adapter"] = selected[0]
    native_command = [str(executable), "qualify-staged" if args.gpu_stages else "qualify",
                      str(args.adapter), args.scenario, str(args.sample_seconds), str(args.warmup_seconds)]
    if not args.gpu_stages:
        native_command.append(args.presentation_policy)
    native = subprocess.Popen(native_command, stdout=subprocess.PIPE, stderr=subprocess.PIPE, cwd=ROOT)
    evidence["native_command"] = native_command
    evidence["native_pid"] = native.pid
    capture_command = [str(args.presentmon), "--process_id", str(native.pid), "--session_name", session,
                       "--output_stdout", "--v1_metrics", "--qpc_time", "--no_console_stats", "--no_track_input",
                       "--timed", str(capture_seconds), "--terminate_after_timed"]
    evidence["capture_command"] = capture_command
    # Drain both streams concurrently; a full CSV pipe must never throttle the trace consumer.
    with concurrent.futures.ThreadPoolExecutor(max_workers=2) as pool:
        native_result = pool.submit(communicate, native, total_seconds + 60)
        capture_result = None
        try:
            capture = subprocess.Popen(capture_command, stdout=subprocess.PIPE, stderr=subprocess.PIPE, cwd=ROOT)
            capture_result = pool.submit(communicate, capture, capture_seconds + 20, args.presentmon, session)
        except (OSError, subprocess.SubprocessError) as error:
            evidence["presentmon"] = dict(failure=str(error))
        evidence["native"] = native_result.result()
        if capture_result is not None:
            # Query after the native measurement/teardown, while the collector drains its tail.
            # The CSV consumer continues concurrently; no observer process is added in-sample.
            query_command = [str(executable), "trace-status", session]
            evidence["etw_query_command"] = query_command
            try:
                query_process = subprocess.Popen(query_command, stdout=subprocess.PIPE, stderr=subprocess.PIPE, cwd=ROOT)
                evidence["etw_query"] = communicate(query_process, 5)
            except (OSError, subprocess.SubprocessError) as error:
                evidence["etw_query"] = dict(failure=str(error))
            evidence["presentmon"] = capture_result.result()
    evidence["inputs_unchanged_after_run"] = before == fingerprints(executable, args.presentmon)
    try:
        native_run = evidence["native"]
        capture_run = evidence["presentmon"]
        if native_run["exit_code"] != 0 or native_run["stderr"] or native_run["timed_out"]:
            raise ValueError("native control failed or timed out")
        report = json.loads(native_run["stdout"])
        if report["render_process_id"] != native.pid:
            raise ValueError("native report PID mismatch")
        BASE["compact_resources"](report)
        evidence["native_report"] = report
        validate_gpu_stages(report, args.gpu_stages)
        if capture_run["exit_code"] != 0 or capture_run["timed_out"] or not capture_run["valid_utf8"]:
            raise ValueError("PresentMon failed or timed out")
        data = runpy.run_path(str(Path(__file__).with_name("presentmon_data.py")))
        evidence["analysis"] = data["analyze"](capture_run["stdout"], native.pid, report["observer"]["frame_age"]["config"])
        roles = runpy.run_path(str(Path(__file__).with_name("presentmon_roles.py")))
        evidence["role_correlation"] = roles["correlate"](
            capture_run["stdout"], native.pid, report, report["observer"]["frame_age"]["config"])
        evidence["role_correlation_verified"] = True
        evidence["presentation_policy_observation"] = validate_presentation_policy(
            capture_run["stdout"], args.presentation_policy, report, evidence["role_correlation"])
        evidence["presentation_policy_verified"] = True
        query_run = evidence["etw_query"]
        if query_run["exit_code"] != 0 or query_run["timed_out"] or query_run["stderr"] or not query_run["valid_utf8"]:
            raise ValueError("live ETW query process failed or timed out")
        query_report = json.loads(query_run["stdout"])
        evidence["etw_query_report"] = query_report
        evidence["live_etw_observation"] = validate_etw_query(query_report, session, report)
        evidence["live_etw_query_verified"] = True
        evidence["loss_observation"] = data["loss_observation"](capture_run["stdout"], capture_run["stderr"])
        if not evidence["loss_observation"]["no_loss_reported"]:
            raise ValueError("PresentMon reported lost data")
        if not evidence["live_etw_observation"]["all_counters_zero"]:
            raise ValueError("independent live ETW query reported loss")
        if not evidence["inputs_unchanged_after_run"]:
            raise ValueError("capture inputs changed")
        evidence["route_verified"] = True
    except (ValueError, KeyError, TypeError) as error:
        evidence["route_verified"] = False
        evidence["validation_failure"] = str(error)
    return evidence


def main():
    args = parse_args()
    evidence = {}
    try:
        collect(args, evidence)
    except (OSError, ValueError, KeyError, TypeError, subprocess.SubprocessError) as error:
        evidence["route_verified"] = False
        evidence["preparation_or_collection_failure"] = str(error)
    packet = BASE["encode_evidence"](evidence, "gzip-base64")
    if args.output is None:
        print(packet, flush=True)
    else:
        # Generated capture output, never a source/evidence-file editor. Exclusive creation
        # preserves prior runs; repository evidence is promoted separately after verification.
        payload = (packet + "\n").encode("utf-8")
        with args.output.open("xb") as output:
            output.write(payload)
        print(json.dumps(dict(output_file=str(args.output), bytes=len(payload),
                              sha256=hashlib.sha256(payload).hexdigest(),
                              route_verified=evidence["route_verified"],
                              role_correlation_verified=evidence.get("role_correlation_verified", False),
                              presentation_policy_verified=evidence.get("presentation_policy_verified", False),
                              live_etw_query_verified=evidence.get("live_etw_query_verified", False))), flush=True)
    return 0 if evidence["route_verified"] else 1


if __name__ == "__main__":
    sys.exit(main())
