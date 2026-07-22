#!/usr/bin/env python3
"""Run and record the full-workbench graph scale qualification."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import shutil
import subprocess
import sys
import tempfile
from collections.abc import Mapping, Sequence
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

if __package__:
    from .graph_fixture import DEFAULT_GRAPH_NODE_COUNT, write_fixture
else:
    from graph_fixture import DEFAULT_GRAPH_NODE_COUNT, write_fixture


EVIDENCE_ID = "graph.full-workbench-10k.local"
HOOK_ID = "graph-scale"
MAX_RENDERED_NODE_COUNT = 1_000
REQUIRED_STEPS = {
    "runtime-ready",
    "fixture-loaded",
    "outliner-rename",
    "inspector-mutation",
    "live-value-feedback",
    "formula-interaction",
    "state-machine-interaction",
    "project-save",
    "save-reload-verified",
    "temporary-project-unloaded",
}


def utc_now() -> str:
    return datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def command_output(root: Path, command: Sequence[str]) -> str:
    result = subprocess.run(
        command,
        cwd=root,
        capture_output=True,
        check=False,
        text=True,
        encoding="utf-8",
    )
    if result.returncode != 0:
        detail = result.stderr.strip() or result.stdout.strip() or "command failed"
        raise ValueError(f"{' '.join(command)}: {detail}")
    return result.stdout.strip()


def working_tree_sha(root: Path) -> str:
    with tempfile.TemporaryDirectory(prefix="chataigne-graph-scale-index-") as directory:
        index_path = Path(directory) / "index"
        environment = os.environ.copy()
        environment["GIT_INDEX_FILE"] = str(index_path)
        for command in (("git", "read-tree", "HEAD"), ("git", "add", "-A", "--", ".")):
            result = subprocess.run(
                command,
                cwd=root,
                env=environment,
                capture_output=True,
                check=False,
                text=True,
                encoding="utf-8",
            )
            if result.returncode != 0:
                detail = result.stderr.strip() or "git working-tree capture failed"
                raise ValueError(f"{' '.join(command)}: {detail}")
        result = subprocess.run(
            ("git", "write-tree"),
            cwd=root,
            env=environment,
            capture_output=True,
            check=False,
            text=True,
            encoding="utf-8",
        )
        if result.returncode != 0:
            detail = result.stderr.strip() or "git write-tree failed"
            raise ValueError(f"git write-tree: {detail}")
        return result.stdout.strip()


def resolve_output_dir(root: Path, value: Path | None) -> Path:
    target_root = (root / "target").resolve()
    output_dir = (
        value.resolve()
        if value is not None and value.is_absolute()
        else (root / value).resolve()
        if value is not None
        else target_root
        / "qualification"
        / "graph-scale"
        / datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    )
    if not output_dir.is_relative_to(target_root) or output_dir == target_root:
        raise ValueError("graph scale output must be a child of the workspace target directory")
    return output_dir


def resolve_product_binary(root: Path, value: Path | None) -> Path:
    if value is not None:
        return value.resolve() if value.is_absolute() else (root / value).resolve()
    suffix = ".exe" if os.name == "nt" else ""
    return (root / "target" / "release" / f"Chataigne2{suffix}").resolve()


def validate_browser_report(report: Mapping[str, Any], expected_node_count: int) -> dict[str, Any]:
    errors: list[str] = []
    if report.get("contract") != "chataigne-product-browser-gate-v1":
        errors.append("browser report has the wrong contract")
    if report.get("status") != "passed":
        errors.append("browser workflow did not pass")

    steps = {
        step.get("step"): step
        for step in report.get("steps", [])
        if isinstance(step, Mapping) and isinstance(step.get("step"), str)
    }
    missing_steps = sorted(REQUIRED_STEPS - set(steps))
    if missing_steps:
        errors.append(f"browser workflow is missing steps: {missing_steps}")

    formula = steps.get("formula-interaction", {})
    total_node_count = formula.get("totalNodeCount")
    visible_node_count = formula.get("visibleNodeCount")
    rendered_node_count = formula.get("renderedNodeCount")
    if total_node_count != expected_node_count:
        errors.append(
            f"workbench reported {total_node_count!r} graph nodes; expected {expected_node_count}"
        )
    if not isinstance(visible_node_count, int) or visible_node_count < 1:
        errors.append("workbench reported no visible graph nodes")
    if rendered_node_count != visible_node_count:
        errors.append("visible and rendered graph node counts differ")
    if isinstance(rendered_node_count, int) and rendered_node_count > MAX_RENDERED_NODE_COUNT:
        errors.append(
            f"rendered graph nodes exceed the bounded-DOM limit: {rendered_node_count}"
        )

    issues = report.get("issues")
    if not isinstance(issues, Mapping):
        errors.append("browser report has no issue summary")
    else:
        for issue_kind, entries in issues.items():
            if isinstance(entries, list) and entries:
                errors.append(f"browser report contains {len(entries)} {issue_kind}")

    network = report.get("network")
    totals = network.get("totals") if isinstance(network, Mapping) else None
    received_frames = totals.get("receivedFrames") if isinstance(totals, Mapping) else None
    if not isinstance(received_frames, int) or received_frames < 1:
        errors.append("browser workflow received no runtime WebSocket frames")

    return {
        "passed": not errors,
        "errors": errors,
        "total_node_count": total_node_count,
        "visible_node_count": visible_node_count,
        "rendered_node_count": rendered_node_count,
        "max_rendered_node_count": MAX_RENDERED_NODE_COUNT,
        "steps": sorted(steps),
        "websocket_received_frames": received_frames,
    }


def run_logged(
    root: Path,
    command: Sequence[str],
    log_path: Path,
    environment: Mapping[str, str] | None = None,
) -> subprocess.CompletedProcess[str]:
    result = subprocess.run(
        command,
        cwd=root,
        env=dict(environment) if environment is not None else None,
        capture_output=True,
        check=False,
        text=True,
        encoding="utf-8",
    )
    log_path.write_text(result.stdout + result.stderr, encoding="utf-8")
    return result


def build_report(
    root: Path,
    output_dir: Path,
    graph_node_count: int,
    port: int,
    product_binary: Path,
) -> dict[str, Any]:
    started_at = utc_now()
    tested_tree_sha = working_tree_sha(root)
    commit_sha = command_output(root, ("git", "rev-parse", "HEAD"))
    rustc = command_output(root, ("rustc", "-Vv"))
    cargo = command_output(root, ("cargo", "-V"))
    output_dir.mkdir(parents=True, exist_ok=True)

    fixture_path = output_dir / f"graph-scale-{graph_node_count}.noisette"
    fixture = write_fixture(
        root / "apps/chataigne/tests/samples/test_simple_load.noisette",
        fixture_path,
        graph_node_count,
    )
    build_command = ("cargo", "build", "--release", "-p", "Chataigne2")
    build_result = run_logged(root, build_command, output_dir / "cargo-build.log")

    hook_command: tuple[str, ...] = ()
    hook_result: subprocess.CompletedProcess[str] | None = None
    browser_report_path = (
        output_dir
        / "product-gate"
        / f"{HOOK_ID}-artifacts"
        / f"{HOOK_ID}.browser-report.json"
    )
    browser_report: dict[str, Any] = {}
    validation = {
        "passed": False,
        "errors": ["release product build failed"],
    }
    if build_result.returncode == 0:
        powershell = shutil.which("pwsh") or shutil.which("powershell")
        if powershell is None:
            validation = {"passed": False, "errors": ["PowerShell was not found on PATH"]}
        elif not product_binary.is_file():
            validation = {
                "passed": False,
                "errors": [f"release product binary is missing: {product_binary}"],
            }
        else:
            browser_report_path.unlink(missing_ok=True)
            hook_command = (
                powershell,
                "-NoProfile",
                "-NonInteractive",
                "-File",
                str(root / "tools/product-gate/hooks/ui-workflow.ps1"),
                "-Id",
                HOOK_ID,
                "-SourceFixturePath",
                str(fixture_path),
                "-FixtureFileName",
                fixture_path.name,
                "-Port",
                str(port),
                "-ProductBinary",
                str(product_binary),
                "-BrowserTimeoutSeconds",
                "180",
                "-DisableBrowserTrace",
            )
            environment = os.environ.copy()
            environment["PRODUCT_GATE_REPOSITORY_ROOT"] = str(root)
            environment["PRODUCT_GATE_RUN_DIRECTORY"] = str(output_dir / "product-gate")
            environment["PRODUCT_GATE_SMOKE_STARTUP_TIMEOUT_SECONDS"] = "240"
            hook_result = run_logged(
                root,
                hook_command,
                output_dir / "browser-hook.log",
                environment,
            )
            if browser_report_path.is_file():
                try:
                    loaded = json.loads(browser_report_path.read_text(encoding="utf-8"))
                    if isinstance(loaded, dict):
                        browser_report = loaded
                        validation = validate_browser_report(loaded, graph_node_count)
                    else:
                        validation = {"passed": False, "errors": ["browser report is not an object"]}
                except json.JSONDecodeError as error:
                    validation = {"passed": False, "errors": [f"invalid browser report: {error}"]}
            else:
                validation = {"passed": False, "errors": ["browser workflow produced no report"]}

    finished_at = utc_now()
    browser_bytes = browser_report_path.read_bytes() if browser_report_path.is_file() else b""
    status = (
        "PASS"
        if build_result.returncode == 0
        and hook_result is not None
        and hook_result.returncode == 0
        and validation.get("passed") is True
        else "FAIL"
    )
    return {
        "schema_version": 1,
        "evidence_id": EVIDENCE_ID,
        "status": status,
        "commit_sha": commit_sha,
        "tested_tree_sha": tested_tree_sha,
        "command": " ".join(
            [*build_command, "&&", *(hook_command or ("<browser-hook-not-run>",))]
        ),
        "toolchain_fingerprint": {
            "rustc": rustc,
            "cargo": cargo,
            "os": platform.platform(),
        },
        "started_at": started_at,
        "finished_at": finished_at,
        "exit_code": hook_result.returncode if hook_result is not None else build_result.returncode,
        "ignored_or_skipped": [],
        "artifact_id": browser_report_path.relative_to(root).as_posix()
        if browser_report_path.is_file()
        else None,
        "artifact_hash": sha256_bytes(browser_bytes) if browser_bytes else None,
        "fixture": fixture,
        "product_binary": {
            "path": product_binary.relative_to(root).as_posix()
            if product_binary.is_relative_to(root)
            else str(product_binary),
            "sha256": sha256_bytes(product_binary.read_bytes()) if product_binary.is_file() else None,
        },
        "measured_result": validation,
        "browser_status": browser_report.get("status"),
    }


def main(arguments: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path.cwd())
    parser.add_argument("--output-dir", type=Path)
    parser.add_argument("--graph-node-count", type=int, default=DEFAULT_GRAPH_NODE_COUNT)
    parser.add_argument("--port", type=int, default=7030)
    parser.add_argument("--product-binary", type=Path)
    options = parser.parse_args(arguments)
    root = options.root.resolve()
    try:
        if options.graph_node_count < 1:
            raise ValueError("graph-node-count must be positive")
        output_dir = resolve_output_dir(root, options.output_dir)
        product_binary = resolve_product_binary(root, options.product_binary)
        report = build_report(
            root,
            output_dir,
            options.graph_node_count,
            options.port,
            product_binary,
        )
    except ValueError as error:
        print(f"Graph scale qualification error: {error}", file=sys.stderr)
        return 2

    report_path = output_dir / "graph-scale-report.json"
    report_path.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(f"Graph scale report: {report_path.relative_to(root).as_posix()}")
    if report["status"] != "PASS":
        for error in report["measured_result"].get("errors", []):
            print(f"- {error}", file=sys.stderr)
    return 0 if report["status"] == "PASS" else 1


if __name__ == "__main__":
    raise SystemExit(main())
