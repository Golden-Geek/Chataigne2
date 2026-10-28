"""Capture the finite authored-image diagnostic; never label it as timing qualification."""

import argparse
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
BASE = runpy.run_path(str(Path(__file__).with_name("run-qualification.py")))
KIND = "finite_authored_color_alpha_gpu_transfer_not_timing_qualification"
PROCESS_KIND = "finite_authored_color_alpha_cross_process_bridge_not_timing_qualification"
PROCEDURAL_KIND = "finite_procedural_card_cross_process_bridge_not_timing_qualification"
ROLES = ["source_a", "source_b", "renderer", "consumer"]


def require(condition, message):
    if not condition:
        raise ValueError(message)


def parse_args(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("adapter", type=int)
    parser.add_argument("scenario", choices=("q01", "q02"))
    parser.add_argument("--profile", choices=("release", "debug"), default="release")
    parser.add_argument("--nvidia-overlay", choices=("on", "off", "unknown"), default="unknown")
    parser.add_argument("--encoding", choices=("json", "gzip-base64"), default="gzip-base64")
    parser.add_argument("--processes", action="store_true", help="use independent producer/consumer children and the source-worker bridge")
    parser.add_argument("--procedural", action="store_true", help="check the actual procedural card shader; requires --processes")
    args = parser.parse_args(argv)
    if args.adapter < 0:
        parser.error("adapter index must be nonnegative")
    if args.procedural and not args.processes:
        parser.error("--procedural requires --processes")
    return args


def admit_adapter(adapters, index, overlay):
    selected = [a for a in adapters if a.get("index") == index]
    require(len(selected) == 1, "selected adapter is missing or ambiguous")
    adapter = selected[0]
    require(adapter.get("software") is False, "hardware adapter required")
    if "nvidia" in adapter.get("description", "").lower():
        require(overlay == "off", "NVIDIA requires verified overlay Off before native startup")
    return adapter


def integer(value, maximum):
    return type(value) is int and 0 <= value <= maximum


def validate_report(report, scenario, adapter=None, processes=False, procedural=False):
    """Validate a passing report. Failed native reports remain intact in the capture."""
    extent = [3840, 2160] if scenario == "q02" else [1920, 1080]
    count = extent[0] * extent[1]
    kind = PROCEDURAL_KIND if procedural else PROCESS_KIND if processes else KIND
    require(not procedural or processes, "procedural check requires processes")
    require(report["evidence_kind"] == kind and report["scenario"] == scenario, "wrong diagnostic/scenario")
    if processes:
        require(report["fixture"] == ("procedural" if procedural else "authored"), "wrong source fixture")
    require(report["passed"] is True, "native accuracy diagnostic did not pass")
    if adapter is not None:
        require(report["adapter_luid"] == adapter["luid"]
                and report["adapter_description"] == adapter["description"], "adapter identity changed")
    require(report["source_extents"] == [extent, [1280, 720]], "source extents changed")
    require(report["initial_pixel_upload_calls"] == (0 if procedural else 2)
            and report["initial_pixel_upload_bytes"] == (0 if procedural else (count + 1280 * 720) * 4),
            "initial pixel-upload accounting disagrees")
    require(report["full_image_readbacks"] == 1, "full-image readback count changed")
    comparison = report["comparison"]
    require(comparison["extent"] == extent and comparison["compared_pixels"] == count,
            "full-image coverage missing")
    require(comparison["tolerance_per_component"] == 2, "fixed accuracy tolerance changed")
    require(comparison["mismatched_pixels"] == 0 and comparison["first_differences"] == [],
            "pixels outside tolerance remain")
    require(comparison["differing_components_bgra"] == [0] * 4, "component mismatch totals disagree")
    require(len(comparison["maximum_error_bgra"]) == 4
            and all(integer(v, 2) for v in comparison["maximum_error_bgra"]), "component error exceeds tolerance")
    require(report["senders_unregistered"] is True, "sender registration survived cleanup")
    require(len(report["devices"]) == 4 and [d["role"] for d in report["devices"]] == ROLES,
            "independent device roles missing/repeated")
    for device in report["devices"]:
        snapshot = device["after_fence"]
        require(snapshot["live_bytes_by_owner"] == snapshot["retiring_bytes_by_owner"] == [0] * 4
                and snapshot["live_textures"] == snapshot["retiring_textures"] == 0,
                "tracked texture ownership survived retirement")
        require(isinstance(device["retirement_ms"], (int, float))
                and math.isfinite(device["retirement_ms"]) and device["retirement_ms"] >= 0,
                "retirement observation invalid")
        ops = device["operations"]
        source = device["role"] in ("source_a", "source_b")
        require(ops["uniform_upload_bytes"] == (32 if procedural and source else 0), "uniform upload accounting disagrees")
        expected_readbacks = count if device["role"] == "consumer" else 0
        require(ops["diagnostic_readback_pixels"] == expected_readbacks, "readback pixel accounting disagrees")
        require(ops["explicit_gpu_copy_calls"] == int(device["role"] == "consumer")
                and ops["explicit_gpu_copy_bytes"] == expected_readbacks * 4,
                "explicit diagnostic GPU copy accounting disagrees")
        require(ops["shader_draws"] == (2 if device["role"] == "renderer" else int(procedural and source))
                and ops["clears"] == int(device["role"] == "renderer"), "composite work accounting disagrees")
        require(ops["sdk_send_calls"] >= int(device["role"] != "consumer"), "GPU sender work missing")
        expected_observations = (0 if processes else 2) if device["role"] == "renderer" else int(device["role"] == "consumer")
        require(ops["sdk_texture_observations"] >= expected_observations,
                "GPU receiver observations missing")
    if processes:
        require(report["devices"][2]["operations"]["sdk_receive_calls"] == 0,
                "renderer bypassed the source-worker SDK boundary")
        pids = report["process_ids"]
        require(len(pids) == 4 and len(set(pids)) == 4 and all(type(p) is int and p > 0 for p in pids)
                and report["children_reaped"] is True, "independent process identities/cleanup missing")
        workers = report["source_workers"]
        require(len(workers) == 2, "source-worker bridge reports missing")
        for worker, source_extent in zip(workers, report["source_extents"]):
            require(worker["adapter_luid"] == report["adapter_luid"] and worker["maximum_surfaces"] == 3
                    and 1 <= worker["peak_surfaces"] <= 3, "bridge identity/capacity invalid")
            publications = worker["publications"]
            require(publications > 0 and worker["receive_calls"] >= publications, "bridge did not publish")
            resources = worker["resources"]
            snapshot = resources["after_fence"]
            require(snapshot["live_bytes_by_owner"] == snapshot["retiring_bytes_by_owner"] == [0]*4
                    and snapshot["live_textures"] == snapshot["retiring_textures"] == 0, "bridge ownership retained")
            ops = resources["operations"]
            require(ops["diagnostic_readback_pixels"] == ops["uniform_upload_bytes"] == ops["shader_draws"] == 0,
                    "bridge performed CPU pixel transfer or drawing")
            require(ops["explicit_gpu_copy_calls"] == publications
                    and ops["explicit_gpu_copy_bytes"] == publications * source_extent[0] * source_extent[1] * 4,
                    "bridge GPU copy accounting disagrees")
            require(ops["sdk_receive_calls"] == worker["receive_calls"]
                    and ops["sdk_texture_observations"] >= publications, "bridge receive accounting disagrees")
            admission = worker["gpu_admission"]
            require(admission["mechanism"] == "timeline_fence" and admission["maximum_in_flight"] == 2
                    and 1 <= admission["peak_in_flight"] <= 2
                    and admission["submitted"] == admission["completed"] and admission["pending"] == 0
                    and admission["reserved"] is False, "bridge submission ownership did not drain")
    require(math.isfinite(report["elapsed_seconds"]) and report["elapsed_seconds"] > 0,
            "elapsed observation invalid")
    return {"passed": True, "full_image_pixels_verified": count, "tolerance_per_component": 2,
            "scope": ("fixed procedural shader/card accuracy through independent processes and source-worker bridge; no timing or scanout claim"
                      if procedural else "authored-image cross-process transfer and existing source-worker bridge; no timing, procedural-card or scanout claim"
                      if processes else "authored-image GPU transfer/sampling only; no timing, procedural-card, scanout or cross-process claim")}


def fingerprints(executable):
    result = BASE["fingerprints"](executable)
    script = Path(__file__).resolve()
    result["files"][script.relative_to(ROOT).as_posix()] = hashlib.sha256(script.read_bytes()).hexdigest()
    return result


def text_output(value):
    return value.decode("utf-8-sig", errors="replace") if isinstance(value, bytes) else value or ""


def collect(args):
    executable = ROOT / f"target/{args.profile}/mapgyver_native_probe.exe"
    command = [str(executable), "color-procedural-accuracy" if args.procedural else "color-accuracy-processes" if args.processes else "color-accuracy", str(args.adapter), args.scenario]
    evidence = {
        "schema_version": 1, "evidence_kind": PROCEDURAL_KIND if args.procedural else PROCESS_KIND if args.processes else KIND,
        "processes": args.processes,
        "procedural": args.procedural,
        "started_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        "command": command, "profile": args.profile, "declared_nvidia_overlay_state": args.nvidia_overlay,
        "environment": {"platform": platform.platform(), "machine": platform.machine(), "python": platform.python_version()},
        "timeout_seconds": 120,
    }
    try:
        before = fingerprints(executable)
        evidence["inputs"] = before
        inventory = subprocess.run([str(executable), "inventory"], cwd=ROOT, capture_output=True, timeout=15, check=True)
        adapters = json.loads(text_output(inventory.stdout))
        evidence["adapters"] = adapters
        adapter = admit_adapter(adapters, args.adapter, args.nvidia_overlay)
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        evidence["preparation_failure"] = str(error)
        return evidence, 1
    try:
        result = subprocess.run(command, cwd=ROOT, capture_output=True, timeout=120)
        run = {"exit_code": result.returncode, "stdout": text_output(result.stdout), "stderr": text_output(result.stderr)}
    except subprocess.TimeoutExpired as error:
        run = {"exit_code": None, "failure": "native diagnostic exceeded 120-second deadline",
               "stdout": text_output(error.stdout), "stderr": text_output(error.stderr)}
    except OSError as error:
        run = {"exit_code": None, "failure": str(error), "stdout": "", "stderr": ""}
    evidence["run"] = run
    try:
        run["report"] = json.loads(run["stdout"])
        require(run["exit_code"] == 0 and not run["stderr"], "native process failed or wrote stderr")
        evidence["validation"] = validate_report(run["report"], args.scenario, adapter, args.processes,args.procedural)
    except (ValueError, KeyError, TypeError) as error:
        evidence["validation"] = {"passed": False, "error": str(error)}
    try:
        evidence["inputs_unchanged_after_run"] = before == fingerprints(executable)
    except OSError as error:
        evidence["inputs_unchanged_after_run"] = False
        evidence["fingerprint_failure"] = str(error)
    passed = evidence["validation"]["passed"] and evidence["inputs_unchanged_after_run"]
    return evidence, 0 if passed else 1


def main():
    args = parse_args()
    evidence, exit_code = collect(args)
    print(BASE["encode_evidence"](evidence, args.encoding))
    return exit_code


if __name__ == "__main__":
    sys.exit(main())
