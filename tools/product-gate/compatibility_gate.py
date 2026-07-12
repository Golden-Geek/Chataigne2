"""Compile one non-native compatibility target and emit a product-gate report."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform as host_platform
import subprocess
import sys
from pathlib import Path
from typing import TextIO


PLATFORMS = {
    "linux-armhf": "armv7-unknown-linux-gnueabihf",
    "linux-aarch64": "aarch64-unknown-linux-gnu",
    "windows-arm64": "aarch64-pc-windows-msvc",
}


def run_text(root: Path, *command: str) -> str:
    return subprocess.check_output(command, cwd=root, text=True).strip()


def stream_command(root: Path, log: TextIO, command: list[str], environment: dict[str, str]) -> int:
    process = subprocess.Popen(
        command,
        cwd=root,
        env=environment,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        errors="replace",
    )
    assert process.stdout is not None
    for line in process.stdout:
        sys.stdout.write(line)
        log.write(line)
    return process.wait()


def result(result_id: str, status: str, exit_code: int, reason: str, log_path: str) -> dict[str, object]:
    return {
        "id": result_id,
        "name": result_id,
        "status": status,
        "required": True,
        "exit_code": exit_code,
        "reason": reason,
        "log_path": log_path,
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--platform", choices=sorted(PLATFORMS), required=True)
    parser.add_argument("--target", required=True)
    parser.add_argument("--scope", choices=("app", "headless-core"), default="app")
    parser.add_argument("--report", type=Path, required=True)
    options = parser.parse_args()

    expected_target = PLATFORMS[options.platform]
    if options.target != expected_target:
        parser.error(f"{options.platform} requires target {expected_target}")

    root = Path(__file__).resolve().parents[2]
    report_path = options.report.resolve()
    report_path.parent.mkdir(parents=True, exist_ok=True)
    log_path = report_path.parent / "compatibility-build.log"
    relative_log_path = log_path.relative_to(report_path.parent).as_posix()
    commit = run_text(root, "git", "rev-parse", "HEAD")
    dirty = bool(run_text(root, "git", "status", "--porcelain=v1", "--untracked-files=all"))
    rustc_details = run_text(root, "rustc", "-vV")
    target_host = next(
        (line.removeprefix("host: ") for line in rustc_details.splitlines() if line.startswith("host: ")),
        "unknown",
    )
    toolchain_path = root / "tools" / "bootstrap" / "toolchain.json"
    toolchain_hash = hashlib.sha256(toolchain_path.read_bytes()).hexdigest()

    environment = os.environ.copy()
    environment["CARGO_NET_GIT_FETCH_WITH_CLI"] = "true"
    environment["GC_SKIP_UI_BUILD"] = "1"
    if options.scope == "headless-core":
        commands = [
            [
                "cargo",
                "check",
                "--manifest-path",
                "submodules/golden_core/Cargo.toml",
                "-p",
                "golden_engine",
                "--target",
                options.target,
            ],
            [
                "cargo",
                "check",
                "--manifest-path",
                "submodules/golden_alchemist_core/Cargo.toml",
                "--workspace",
                "--target",
                options.target,
            ],
            [
                "cargo",
                "check",
                "-p",
                "chataigne_state_machine",
                "--target",
                options.target,
            ],
        ]
    else:
        commands = [["cargo", "build", "--target", options.target]]
    with log_path.open("w", encoding="utf-8", newline="\n") as log:
        exit_code = 0
        for command in commands:
            exit_code = stream_command(root, log, command, environment)
            if exit_code != 0:
                break

    passed = exit_code == 0 and not dirty
    status = "PASS" if passed else "FAIL"
    reasons = []
    if dirty:
        reasons.append("working tree was dirty before the compatibility build")
    if exit_code != 0:
        reasons.append(f"compatibility compilation exited with {exit_code}")
    reason = "; ".join(reasons)
    results = [
        result("compatibility.compile", status, exit_code, reason, relative_log_path),
        result(f"platform.{options.platform}", status, exit_code, reason, relative_log_path),
    ]
    report = {
        "schema_version": 1,
        "gate": "chataigne-product-gate",
        "overall_status": status,
        "commit": {"sha": commit, "working_tree_dirty": dirty},
        "toolchain": {
            "target_host": target_host,
            "os_description": host_platform.platform(),
            "canonical_manifest_sha256": toolchain_hash,
        },
        "required_platforms": [options.platform],
        "compatibility_target": options.target,
        "compatibility_scope": options.scope,
        "results": results,
    }
    report_path.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8", newline="\n")
    print(f"{status} {options.platform} compatibility compilation ({options.target})")
    return 0 if passed else 1


if __name__ == "__main__":
    raise SystemExit(main())
