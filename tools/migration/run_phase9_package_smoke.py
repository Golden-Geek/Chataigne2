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


PLATFORM_BUNDLES = {
    "windows": "nsis,msi",
    "macos": "app,dmg",
    "linux": "appimage,deb",
}


def utc_now() -> str:
    return datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def sha256_artifact(path: Path) -> str:
    if path.is_file():
        return sha256_file(path)
    digest = hashlib.sha256()
    for item in sorted(candidate for candidate in path.rglob("*") if candidate.is_file()):
        digest.update(item.relative_to(path).as_posix().encode("utf-8"))
        digest.update(b"\0")
        digest.update(bytes.fromhex(sha256_file(item)))
    return digest.hexdigest()


def native_platform() -> str:
    value = sys.platform
    if value == "win32":
        return "windows"
    if value == "darwin":
        return "macos"
    if value.startswith("linux"):
        return "linux"
    raise ValueError(f"unsupported package-smoke platform: {value}")


def resolve_npm() -> str:
    executable = shutil.which("npm.cmd") or shutil.which("npm")
    if executable is None:
        raise ValueError("npm was not found on PATH")
    return executable


def select_package_artifact(bundle_root: Path, platform_name: str) -> Path:
    if platform_name == "windows":
        matches = sorted((bundle_root / "nsis").glob("*-setup.exe"))
    elif platform_name == "macos":
        matches = sorted((bundle_root / "macos").glob("*.app"))
    else:
        matches = sorted((bundle_root / "appimage").glob("*.AppImage"))
    if len(matches) != 1:
        raise ValueError(f"expected exactly one {platform_name} package artifact, found {len(matches)}")
    return matches[0].resolve()


def find_product_binary(package: Path, platform_name: str, install_root: Path) -> tuple[Path, Path | None]:
    if platform_name == "windows":
        install_root.mkdir(parents=True, exist_ok=True)
        result = subprocess.run(
            (str(package), "/S", f"/D={install_root}"),
            capture_output=True,
            text=True,
            check=False,
        )
        if result.returncode != 0:
            raise ValueError(f"NSIS package installation failed with exit code {result.returncode}")
        binaries = {
            str(path.resolve()).casefold(): path.resolve()
            for pattern in ("Chataigne2.exe", "chataigne2.exe")
            for path in install_root.glob(pattern)
        }
        if len(binaries) != 1:
            raise ValueError("the installed Windows package contains no unique Chataigne2 executable")
        uninstallers = sorted(install_root.glob("uninstall*.exe"))
        return next(iter(binaries.values())), uninstallers[0].resolve() if len(uninstallers) == 1 else None
    if platform_name == "macos":
        binaries = sorted((package / "Contents" / "MacOS").iterdir())
        binaries = [path for path in binaries if path.is_file() and os.access(path, os.X_OK)]
        if len(binaries) != 1:
            raise ValueError("the macOS app bundle contains no unique executable")
        return binaries[0].resolve(), None
    package.chmod(package.stat().st_mode | 0o111)
    return package, None


def validate_browser_report(report: dict[str, Any]) -> None:
    required_steps = {
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
    if report.get("contract") != "chataigne-product-browser-gate-v1" or report.get("status") != "passed":
        raise ValueError("clean-package browser workflow did not pass")
    actual = {step.get("step") for step in report.get("steps", []) if isinstance(step, dict)}
    missing = required_steps - actual
    if missing:
        raise ValueError(f"clean-package browser workflow is missing steps: {sorted(missing)}")


def run(arguments: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Build and smoke the native Phase 9 package.")
    parser.add_argument("--platform", choices=sorted(PLATFORM_BUNDLES), default=native_platform())
    parser.add_argument("--output-dir", type=Path)
    parser.add_argument("--port", type=int, default=7051)
    parser.add_argument("--skip-build", action="store_true")
    options = parser.parse_args(arguments)
    root = Path(__file__).resolve().parents[2]
    target = (root / "target").resolve()
    output = (
        (root / options.output_dir).resolve()
        if options.output_dir is not None and not options.output_dir.is_absolute()
        else options.output_dir.resolve()
        if options.output_dir is not None
        else target / "phase9" / "package" / options.platform / datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    )
    if output == target or not output.is_relative_to(target):
        raise ValueError("Phase 9 package evidence must be below the workspace target directory")
    output.mkdir(parents=True, exist_ok=True)

    started_at = utc_now()
    tested_tree_sha = working_tree_sha(root)
    npm_executable = resolve_npm()
    if not options.skip_build:
        build = subprocess.run(
            (
                npm_executable,
                "run",
                "package",
                "--",
                "--bundles",
                PLATFORM_BUNDLES[options.platform],
            ),
            cwd=root,
            capture_output=True,
            text=True,
            encoding="utf-8",
            check=False,
        )
        (output / "package-build.log").write_text(build.stdout + build.stderr, encoding="utf-8")
        if build.returncode != 0:
            raise ValueError("native package build failed")

    package = select_package_artifact(target / "release" / "bundle", options.platform)
    product_binary, uninstaller = find_product_binary(package, options.platform, output / "installed")
    run_dir = output / "product-gate"
    environment = os.environ.copy()
    environment["PRODUCT_GATE_REPOSITORY_ROOT"] = str(root)
    environment["PRODUCT_GATE_RUN_DIRECTORY"] = str(run_dir)
    if options.platform == "linux":
        environment["APPIMAGE_EXTRACT_AND_RUN"] = "1"
    hook = root / "tools" / "product-gate" / "hooks" / "ui-workflow.ps1"
    powershell = shutil.which("pwsh") or shutil.which("powershell.exe")
    if powershell is None:
        raise ValueError("PowerShell is required for the clean-package browser workflow")
    smoke = subprocess.run(
        (
            powershell,
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-File",
            str(hook),
            "-Id",
            f"phase9-{options.platform}-clean-package",
            "-ProductBinary",
            str(product_binary),
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
    (output / "package-smoke.log").write_text(smoke.stdout + smoke.stderr, encoding="utf-8")
    if smoke.returncode != 0:
        raise ValueError("the packaged application browser workflow failed")
    report_id = f"phase9-{options.platform}-clean-package"
    browser_report_path = run_dir / f"{report_id}-artifacts" / f"{report_id}.browser-report.json"
    browser_report = json.loads(browser_report_path.read_text(encoding="utf-8"))
    validate_browser_report(browser_report)

    cleanup = "not_applicable"
    if options.platform == "windows":
        if uninstaller is None:
            raise ValueError("the Windows package installed no uninstaller")
        uninstall = subprocess.run((str(uninstaller), "/S"), check=False)
        if uninstall.returncode != 0:
            raise ValueError("the Windows package uninstaller failed")
        cleanup = "uninstaller_passed"

    report = {
        "schema_version": 1,
        "contract": "chataigne-phase9-clean-package-report-v1",
        "evidence_id": f"phase9.package.{options.platform}.clean",
        "status": "PASS",
        "platform": options.platform,
        "commit_sha": command_output(root, ("git", "rev-parse", "HEAD")),
        "tested_tree_sha": tested_tree_sha,
        "started_at": started_at,
        "finished_at": utc_now(),
        "package": package.relative_to(root).as_posix(),
        "package_sha256": sha256_artifact(package),
        "browser_report": browser_report_path.relative_to(root).as_posix(),
        "browser_report_sha256": sha256_file(browser_report_path),
        "cleanup": cleanup,
        "toolchain": {
            "rustc": command_output(root, ("rustc", "-V")),
            "cargo": command_output(root, ("cargo", "-V")),
            "node": command_output(root, ("node", "--version")),
            "npm": command_output(root, (npm_executable, "--version")),
            "python": platform.python_version(),
            "os": platform.platform(),
        },
    }
    report_path = output / "phase9-clean-package-report.json"
    report_path.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(f"Phase 9 clean-package report: {report_path.relative_to(root).as_posix()}")
    return 0


def main() -> int:
    try:
        return run()
    except (OSError, ValueError, json.JSONDecodeError) as error:
        print(f"Phase 9 clean-package smoke failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
