"""Run owned M01 windows and emit evidence to stdout; never edit the checkout or display settings."""

import argparse
import base64
import datetime
import gzip
import hashlib
import json
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[2]
PROBE = ROOT / "tools/mapgyver/native-probe"


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def encode_evidence(evidence, encoding):
    """Compression happens after the native process exits; no sample or failure is removed."""
    payload = json.dumps(evidence, separators=(",", ":")).encode("utf-8")
    if encoding == "json":
        return payload.decode("utf-8")
    if encoding != "gzip-base64":
        raise ValueError("unknown evidence encoding")
    return json.dumps({
        "wire_schema_version": 1,
        "encoding": "gzip-base64",
        "decoded_bytes": len(payload),
        "sha256": hashlib.sha256(payload).hexdigest(),
        "payload": base64.b64encode(gzip.compress(payload, mtime=0)).decode("ascii"),
    }, separators=(",", ":"))


def decode_evidence(packet):
    """Validate a stored compressed packet before interpreting its native evidence."""
    if packet.get("wire_schema_version") != 1 or packet.get("encoding") != "gzip-base64":
        raise ValueError("unknown compressed evidence envelope")
    payload = gzip.decompress(base64.b64decode(packet["payload"], validate=True))
    if len(payload) != packet["decoded_bytes"] or hashlib.sha256(payload).hexdigest() != packet["sha256"]:
        raise ValueError("compressed evidence length/hash mismatch")
    return json.loads(payload)


def fingerprints(executable):
    paths = [ROOT / name for name in ("Cargo.toml", "Cargo.lock", "package.json", "package-lock.json")]
    paths += [
        Path(__file__).resolve(), ROOT / "tools/mapgyver/collect-baseline.ps1",
        PROBE / "Cargo.toml", PROBE / "build.rs", PROBE / "tauri.conf.json",
    ]
    paths += list((PROBE / "src").rglob("*.rs"))
    paths += list((PROBE / "ui/src").rglob("*"))
    paths += list((PROBE / "ui").glob("*"))
    paths += list((PROBE / "ui/scripts").rglob("*"))
    paths += list((PROBE / "ui/dist").rglob("*"))
    return {
        "executable_sha256": digest(executable),
        "files": {path.relative_to(ROOT).as_posix(): digest(path) for path in sorted(set(paths)) if path.is_file()},
    }


def compact_resources(report):
    """Keep every 1 Hz observation, with a shared column schema instead of repeated field names."""
    resources = report["resources"]
    columns = ["elapsed_seconds", "in_sample_window", "generation", "settling"]
    for process in ("renderer", "source_a", "source_b", "observer"):
        columns += [f"{process}.{name}" for name in ("private_bytes", "working_set_bytes", "handle_count")]
    for segment in ("local", "nonlocal"):
        columns += [f"renderer_gpu_{segment}.{name}" for name in ("current_usage_bytes", "budget_bytes")]
    rows = []
    gpu_observations = []
    ids = None
    for sample in resources.pop("samples"):
        gpu_observations.append({
            "renderer_allocations": sample["renderer_allocations"],
            "presentation_editor_output": sample["presentation_editor_output"],
            "gpu_pipeline": sample["stage"]["gpu_pipeline"],
        })
        processes = [sample["renderer"], *sample["sources"], sample["observer"]]
        current_ids = [process["process_id"] for process in processes]
        if ids is not None and ids != current_ids:
            raise ValueError("resource process identity changed during the sample")
        ids = current_ids
        row = [
            sample["elapsed_seconds"], sample["in_sample_window"],
            sample["stage"]["generation"], sample["stage"]["settling"],
        ]
        for process in processes:
            row += [process[name] for name in ("private_bytes", "working_set_bytes", "handle_count")]
        for segment in sample["renderer_gpu_local_nonlocal"]:
            row += [segment["current_usage_bytes"], segment["budget_bytes"]] if segment else [None, None]
        rows.append(row)
    resources["process_ids_renderer_sources_observer"] = ids
    resources["columns"] = columns
    resources["rows"] = rows
    # Same index as rows, preserving typed/nested unavailable DXGI results without flattening
    # them into success-shaped numeric defaults.
    resources["gpu_observations"] = gpu_observations
    for generation in report.get("generations", []):
        for timing in [generation.get("timing", {}), *(worker.get("timing", {}) for worker in generation.get("source_workers", []))]:
            compact_gpu_trace(timing)
        dependencies = generation.get("dependency_trace")
        if dependencies is not None:
            dependencies["columns"] = ["renderer_frame", "source_a_batch", "source_b_batch"]
            dependencies["rows"] = [[frame["renderer_frame"], *frame["sources"]]
                                     for frame in dependencies.pop("frames")]
    compact_frame_age(report)
    children = [source["resources"] for source in report.get("sources", [])]
    children.append(report.get("observer", {}).get("resources"))
    children += [worker["resources"] for generation in report.get("generations", [])
                 for worker in generation.get("source_workers", [])]
    for child in children:
        if child is not None:
            compact_child_resources(child)


def compact_gpu_trace(timing):
    trace = timing.get("gpu_trace")
    if trace is None:
        return
    trace["columns"] = ["frame", "submitted_qpc", "observed_completed_qpc", "start_ticks", "end_ticks", "frequency", "collected"]
    trace["rows"] = [[span[key] for key in trace["columns"]] for span in trace.pop("spans")]


def compact_child_resources(resources):
    """Keep process-local payload ownership distinct from DXGI process usage."""
    allocation_fields = [
        "live_bytes_by_owner", "retiring_bytes_by_owner", "live_textures", "retiring_textures",
        "peak_live_plus_retiring_bytes", "peak_live_plus_retiring_textures",
    ]
    resources["sample_columns"] = ["elapsed_seconds", "source_phase", *allocation_fields]
    for segment in ("local", "nonlocal"):
        resources["sample_columns"] += [f"gpu_{segment}.{name}" for name in ("current_usage_bytes", "budget_bytes")]
    samples = resources.pop("samples")
    if len(samples) != resources["sample_count"]:
        raise ValueError("child resource sample count disagrees with report")
    rows = []
    for sample in samples:
        row = [sample["elapsed_seconds"], sample["source_phase"]]
        row += [sample["allocations"][name] for name in allocation_fields]
        for segment in sample["gpu_local_nonlocal"]:
            row += [segment["current_usage_bytes"], segment["budget_bytes"]] if segment else [None, None]
        rows.append(row)
    resources["sample_rows"] = rows


def compact_frame_age(report):
    """Retain every raw QPC/marker word; no distribution or invalid sample replaces evidence."""
    age = report.get("observer", {}).get("frame_age")
    if age is None:
        return
    columns = ["submitted_qpc", "completed_qpc"]
    for channel in ("a", "b"):
        columns += [f"{channel}.{word}" for word in ("frame", "magic", "qpc_low", "qpc_high", "checksum")]
    samples = age.pop("samples")
    if len(samples) != age["captured_samples"]:
        raise ValueError("frame-age sample count disagrees with report")
    age["sample_columns"] = columns
    age["sample_rows"] = [
        [sample["submitted_qpc"], sample["completed_qpc"], *sample["words"][0], *sample["words"][1]]
        for sample in samples
    ]


def parse_args(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("adapter", type=int)
    parser.add_argument("scenario", choices=("diagnostic", "q01", "q02"))
    parser.add_argument("--profile", choices=("debug", "release"), default="release")
    parser.add_argument("--samples", type=int, choices=(1, 2, 3), default=3)
    parser.add_argument("--seconds", type=int, default=120)
    parser.add_argument("--warmup", type=int, default=30)
    parser.add_argument(
        "--encoding", choices=("json", "gzip-base64"), default="json",
        help="lossless output envelope for collectors with limited terminal output capacity",
    )
    parser.add_argument(
        "--nvidia-overlay", choices=("off", "on", "unknown"), default="unknown",
        help="operator-declared state; retain separate native UI verification",
    )
    exercise = parser.add_mutually_exclusive_group()
    exercise.add_argument(
        "--recreate", type=int, metavar="CYCLES",
        help="run 1..100 explicit device rebuilds instead of timing qualification",
    )
    exercise.add_argument(
        "--editor-isolation", action="store_true",
        help="busy/blank/reconnect exercise; requires q01/q02, --seconds 185 --warmup 0 --samples 1",
    )
    exercise.add_argument(
        "--underlay", action="store_true",
        help="capture an interactive diagnostic session; requires diagnostic, --warmup 0 and --samples 1",
    )
    args = parser.parse_args(argv)
    if args.editor_isolation and (args.scenario == "diagnostic" or args.seconds != 185
                                  or args.warmup != 0 or args.samples != 1):
        parser.error("editor isolation requires q01/q02, --seconds 185 --warmup 0 --samples 1")
    if args.underlay:
        if args.scenario != "diagnostic" or args.warmup != 0 or args.samples != 1:
            parser.error("interactive underlay requires diagnostic, --warmup 0 and --samples 1")
    elif args.scenario == "diagnostic":
        parser.error("diagnostic requires --underlay; it cannot qualify Q01/Q02")
    if args.recreate is not None and not 1 <= args.recreate <= 100:
        parser.error("device recreation cycles must be 1..100")
    if args.seconds < 8 or args.warmup < 0 or args.seconds + args.warmup > 3600:
        parser.error("sample must be >=8 seconds, warmup >=0, and total <=3600")
    return args


def main():
    args = parse_args()
    executable = ROOT / f"target/{args.profile}/mapgyver_native_probe.exe"
    before = fingerprints(executable)
    inventory = json.loads(subprocess.check_output([str(executable), "inventory"], cwd=ROOT))
    baseline = subprocess.check_output(
        ["powershell", "-NoProfile", "-NonInteractive", "-Command",
         "[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new(); & ./tools/mapgyver/collect-baseline.ps1"],
        cwd=ROOT,
    )
    environment = json.loads(baseline.decode("utf-8-sig"))
    if args.editor_isolation:
        command = [str(executable), "editor-isolation", str(args.adapter), args.scenario]
        duration = 185
    elif args.underlay:
        command = [str(executable), "underlay", str(args.adapter), str(args.seconds)]
        duration = args.seconds
    elif args.recreate is not None:
        command = [str(executable), "recreate", str(args.adapter), args.scenario, str(args.recreate)]
        duration = 18 + args.recreate * 4
    else:
        command = [str(executable), "qualify", str(args.adapter), args.scenario, str(args.seconds), str(args.warmup)]
        duration = args.seconds + args.warmup
    for index in range(args.samples):
        started = datetime.datetime.now(datetime.timezone.utc).isoformat()
        print(
            f"Starting {args.scenario} adapter {args.adapter}, sample {index + 1}/{args.samples}",
            file=sys.stderr, flush=True,
        )
        try:
            result = subprocess.run(command, capture_output=True, cwd=ROOT, timeout=duration + 60)
            stdout = result.stdout.decode("utf-8-sig")
            report = json.loads(stdout) if result.returncode == 0 else None
            if report is not None:
                compact_resources(report)
            run = {
                "exit_code": result.returncode,
                "stderr": result.stderr.decode("utf-8", errors="replace"),
                "report": report,
            }
            if result.returncode != 0:
                run["stdout"] = stdout
        except subprocess.TimeoutExpired as error:
            # subprocess.run kills/reaps only our parent probe. Fixtures have a bounded frame
            # lifetime ending before this deadline; observer exits when its owned pipe closes.
            run = {
                "failure": "probe exceeded its duration plus 60-second cleanup deadline",
                "stdout": (error.stdout or b"").decode("utf-8", errors="replace"),
            }
        unchanged = before == fingerprints(executable)
        evidence = {
            "schema_version": 16,
            "evidence_kind": (
                "editor_busy_disconnected_reconnect_not_timing_qualification" if args.editor_isolation else
                "interactive_underlay_observations_not_timing_qualification" if args.underlay else
                "fixed_workload_submission_and_resource_observations_not_full_M01_qualification"
            ),
            "started_utc": started,
            "sample_index": index + 1,
            "requested_series_samples": args.samples,
            "command": command,
            "profile": args.profile,
            "declared_nvidia_overlay_state": args.nvidia_overlay,
            "inputs": before,
            "inputs_unchanged_after_run": unchanged,
            "environment": environment,
            "adapters": inventory,
            "run": run,
        }
        print(encode_evidence(evidence, args.encoding), flush=True)
        if not unchanged or run.get("exit_code") != 0:
            return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
