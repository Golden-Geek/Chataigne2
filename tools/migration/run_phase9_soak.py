from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import shutil
import subprocess
import sys
from collections.abc import Sequence
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

try:
    from .run_phase9_scale import command_output, working_tree_sha
except ImportError:
    from run_phase9_scale import command_output, working_tree_sha


EVIDENCE_ID = "phase9.soak.module-network-multiclient-hardware.local"
MINIMUM_DURATION_SECONDS = 5 * 60
MINIMUM_HARDWARE_CYCLES = 3
HOST_FAILURE_MARKERS = (
    "disconnecting slow client",
    "reliable outbound queue exhausted",
    "intent timeout",
    "semantic mismatch",
    "thread panicked",
)


def utc_now() -> str:
    return datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def resolve_output_dir(root: Path, value: Path | None) -> Path:
    target = (root / "target").resolve()
    output = (
        (root / value).resolve()
        if value is not None and not value.is_absolute()
        else value.resolve()
        if value is not None
        else target / "phase9" / "soak" / datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    )
    if output == target or not output.is_relative_to(target):
        raise ValueError("Phase 9 soak output must be below the workspace target directory")
    return output


def validate_browser_report(report: dict[str, Any], duration_ms: int, client_count: int) -> None:
    if report.get("contract") != "chataigne-phase9-multiclient-soak-v1":
        raise ValueError("browser soak report has the wrong contract")
    if report.get("status") != "passed":
        raise ValueError("browser soak did not pass")
    if report.get("durationMs") != duration_ms:
        raise ValueError("browser soak duration does not match the requested duration")
    if report.get("clientCount") != client_count or len(report.get("clients", [])) != client_count:
        raise ValueError("browser soak did not retain every requested client")
    if not isinstance(report.get("iterations"), int) or report["iterations"] < 1:
        raise ValueError("browser soak completed no mutation iterations")
    for client in report["clients"]:
        totals = client.get("websocketTotals", {})
        if totals.get("receivedFrames", 0) < 1 or totals.get("sentFrames", 0) < 1:
            raise ValueError("a soak client has no bidirectional WebSocket traffic")
    if duration_ms >= MINIMUM_DURATION_SECONDS * 1000:
        plateau = report.get("memoryPlateau", [])
        if len(plateau) != client_count or any(item.get("status") != "passed" for item in plateau):
            raise ValueError("browser soak did not prove a stable per-client memory plateau")
        queues = report.get("queueSummary", [])
        if len(queues) != client_count or any(
            item.get("status") != "passed" or item.get("finalDepth") != 0 for item in queues
        ):
            raise ValueError("browser soak did not prove drained runtime control queues")


def validate_host_logs(paths: Sequence[Path]) -> None:
    source = "\n".join(path.read_text(encoding="utf-8", errors="replace") for path in paths).lower()
    matches = [marker for marker in HOST_FAILURE_MARKERS if marker in source]
    if matches:
        raise ValueError(f"host soak logs contain failure markers: {matches}")


def run(arguments: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Run the Phase 9 migration-stage soak gate.")
    parser.add_argument("--output-dir", type=Path)
    parser.add_argument("--duration-seconds", type=int, default=MINIMUM_DURATION_SECONDS)
    parser.add_argument("--hardware-cycles", type=int, default=MINIMUM_HARDWARE_CYCLES)
    parser.add_argument("--clients", type=int, default=3)
    parser.add_argument("--port", type=int, default=7041)
    parser.add_argument("--allow-short", action="store_true")
    options = parser.parse_args(arguments)
    root = Path(__file__).resolve().parents[2]
    output = resolve_output_dir(root, options.output_dir)
    output.mkdir(parents=True, exist_ok=True)
    if options.clients < 2:
        raise ValueError("at least two browser clients are required")
    if options.duration_seconds < 1 or options.hardware_cycles < 1:
        raise ValueError("duration and hardware cycles must be positive")
    if not options.allow_short and options.duration_seconds < MINIMUM_DURATION_SECONDS:
        raise ValueError("Phase 9 soak evidence requires at least five minutes")
    if not options.allow_short and options.hardware_cycles < MINIMUM_HARDWARE_CYCLES:
        raise ValueError(f"hardware soak evidence requires at least {MINIMUM_HARDWARE_CYCLES} full cycles")

    started_at = utc_now()
    tested_tree_sha = working_tree_sha(root)
    commit_sha = command_output(root, ("git", "rev-parse", "HEAD"))
    build_log = output / "cargo-build.log"
    hardware_log = output / "hardware-cycles.log"
    build = subprocess.run(
        ("cargo", "build", "-p", "Chataigne2"),
        cwd=root,
        capture_output=True,
        text=True,
        encoding="utf-8",
        check=False,
    )
    build_log.write_text(build.stdout + build.stderr, encoding="utf-8")
    if build.returncode != 0:
        raise ValueError("the product binary did not build for the soak")

    with hardware_log.open("w", encoding="utf-8") as stream:
        for cycle in range(options.hardware_cycles):
            result = subprocess.run(
                ("cargo", "test", "-p", "Chataigne2"),
                cwd=root,
                capture_output=True,
                text=True,
                encoding="utf-8",
                check=False,
            )
            stream.write(f"\n=== hardware simulator cycle {cycle + 1} ===\n")
            stream.write(result.stdout)
            stream.write(result.stderr)
            if result.returncode != 0:
                raise ValueError(f"hardware simulator cycle {cycle + 1} failed")

    hook = root / "tools" / "product-gate" / "hooks" / "phase9-soak.ps1"
    run_dir = output / "product-gate"
    environment = os.environ.copy()
    environment["PRODUCT_GATE_REPOSITORY_ROOT"] = str(root)
    environment["PRODUCT_GATE_RUN_DIRECTORY"] = str(run_dir)
    duration_ms = options.duration_seconds * 1000
    powershell = shutil.which("pwsh") or shutil.which("powershell.exe")
    if powershell is None:
        raise ValueError("PowerShell is required for the mounted-product soak")
    hook_result = subprocess.run(
        (
            powershell,
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-File",
            str(hook),
            "-DurationMilliseconds",
            str(duration_ms),
            "-ClientCount",
            str(options.clients),
            "-Port",
            str(options.port),
        ),
        cwd=root,
        env=environment,
        capture_output=True,
        text=True,
        encoding="utf-8",
        check=False,
    )
    (output / "soak-hook.log").write_text(hook_result.stdout + hook_result.stderr, encoding="utf-8")
    if hook_result.returncode != 0:
        raise ValueError("the multi-client browser soak failed")
    browser_report_path = run_dir / "phase9-soak-artifacts" / "phase9-soak.browser-report.json"
    browser_report = json.loads(browser_report_path.read_text(encoding="utf-8"))
    validate_browser_report(browser_report, duration_ms, options.clients)
    product_logs = (
        run_dir / "phase9-soak.product.stdout.log",
        run_dir / "phase9-soak.product.stderr.log",
    )
    validate_host_logs(product_logs)

    npm_executable = shutil.which("npm.cmd") or shutil.which("npm")
    if npm_executable is None:
        raise ValueError("npm was not found on PATH")

    finished_at = utc_now()
    report = {
        "schema_version": 1,
        "contract": "chataigne-phase9-soak-report-v1",
        "evidence_id": EVIDENCE_ID,
        "status": "PASS",
        "commit_sha": commit_sha,
        "tested_tree_sha": tested_tree_sha,
        "started_at": started_at,
        "finished_at": finished_at,
        "duration_seconds": options.duration_seconds,
        "hardware_cycles": options.hardware_cycles,
        "client_count": options.clients,
        "browser_iterations": browser_report["iterations"],
        "browser_report": browser_report_path.relative_to(root).as_posix(),
        "browser_report_sha256": sha256_file(browser_report_path),
        "memory_plateau": browser_report["memoryPlateau"],
        "runtime_queue_summary": browser_report["queueSummary"],
        "product_log_sha256": {path.name: sha256_file(path) for path in product_logs},
        "hardware_log_sha256": sha256_file(hardware_log),
        "toolchain": {
            "rustc": command_output(root, ("rustc", "-V")),
            "cargo": command_output(root, ("cargo", "-V")),
            "node": command_output(root, ("node", "--version")),
            "npm": command_output(root, (npm_executable, "--version")),
            "python": platform.python_version(),
            "os": platform.platform(),
        },
    }
    report_path = output / "phase9-soak-report.json"
    report_path.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(f"Phase 9 soak report: {report_path.relative_to(root).as_posix()}")
    return 0


def main() -> int:
    try:
        return run()
    except (OSError, ValueError, json.JSONDecodeError) as error:
        print(f"Phase 9 soak failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
