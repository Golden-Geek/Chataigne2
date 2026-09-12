#!/usr/bin/env python3
"""Compile a standalone Golden Audio consumer from a pinned Git revision."""

from __future__ import annotations

import argparse
import json
import subprocess
import tempfile
from pathlib import Path


def run(*args: str, cwd: Path | None = None) -> str:
    result = subprocess.run(args, cwd=cwd, text=True, capture_output=True)
    if result.returncode != 0:
        rendered = " ".join(args)
        raise SystemExit(
            f"command failed ({result.returncode}): {rendered}\n{result.stdout}{result.stderr}"
        )
    return result.stdout


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--repository", type=Path, default=Path(__file__).resolve().parents[2])
    parser.add_argument("--target-dir", type=Path, required=True)
    parser.add_argument("--report", type=Path)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    repository = args.repository.resolve()
    target_dir = args.target_dir.resolve()
    revision = run("git", "rev-parse", "HEAD", cwd=repository).strip()
    if subprocess.run(["git", "diff", "--quiet", "HEAD", "--"], cwd=repository).returncode != 0:
        raise SystemExit("external consumer qualification requires a clean committed repository")

    with tempfile.TemporaryDirectory(prefix="golden-audio-consumer-") as temporary:
        consumer = Path(temporary)
        (consumer / "src").mkdir()
        repository_uri = repository.as_uri()
        (consumer / "Cargo.toml").write_text(
            "\n".join(
                [
                    "[package]",
                    'name = "golden-audio-external-consumer"',
                    'version = "0.0.0"',
                    'edition = "2024"',
                    "publish = false",
                    "",
                    "[workspace]",
                    "",
                    "[dependencies]",
                    (
                        "golden_audio = { git = "
                        f'"{repository_uri}", rev = "{revision}", '
                        "default-features = false, features = "
                        '["desktop", "asio", "jack", "realtime"] }'
                    ),
                    "",
                ]
            ),
            encoding="utf-8",
        )
        (consumer / "src" / "main.rs").write_text(
            "use golden_audio::{AudioBackendState, compiled_cpal_backend_catalog};\n\n"
            "fn main() {\n"
            "    let _ = AudioBackendState::Compiled;\n"
            "    let _ = compiled_cpal_backend_catalog();\n"
            "}\n",
            encoding="utf-8",
        )

        metadata = json.loads(
            run(
                "cargo",
                "metadata",
                "--format-version",
                "1",
                "--manifest-path",
                str(consumer / "Cargo.toml"),
                cwd=consumer,
            )
        )
        run(
            "cargo",
            "check",
            "--locked",
            "--manifest-path",
            str(consumer / "Cargo.toml"),
            "--target-dir",
            str(target_dir),
            cwd=consumer,
        )

        packages = {package["name"]: package for package in metadata["packages"]}
        audio_package = packages.get("golden_audio")
        cpal_package = packages.get("cpal")
        if audio_package is None or not str(audio_package.get("source", "")).startswith("git+"):
            raise SystemExit("standalone consumer did not resolve golden_audio from the pinned Git source")
        if cpal_package is None:
            raise SystemExit("standalone desktop consumer did not resolve CPAL")

        cpal_manifest = Path(cpal_package["manifest_path"]).resolve()
        expected_suffix = Path("vendor/cpal-0.18.1/Cargo.toml")
        if cpal_manifest.parts[-len(expected_suffix.parts) :] != expected_suffix.parts:
            raise SystemExit(f"standalone consumer resolved unintended CPAL manifest: {cpal_manifest}")
        asio_source = cpal_manifest.parent / "src" / "host" / "asio" / "mod.rs"
        asio_text = asio_source.read_text(encoding="utf-8")
        patch_markers = (
            "fn device_by_id(&self, id: &DeviceId)",
            "Devices::by_name(self.asio.clone(), id.id().to_owned())",
        )
        if not all(marker in asio_text for marker in patch_markers):
            raise SystemExit("resolved CPAL source does not contain the reviewed exact-driver ASIO patch")

        report = {
            "schema_version": 1,
            "status": "PASS",
            "repository_revision": revision,
            "golden_audio_source": audio_package["source"],
            "cpal_version": cpal_package["version"],
            "cpal_manifest_suffix": expected_suffix.as_posix(),
            "consumer_features": ["desktop", "asio", "jack", "realtime"],
        }
        rendered = json.dumps(report, indent=2) + "\n"
        if args.report:
            report_path = args.report.resolve()
            report_path.parent.mkdir(parents=True, exist_ok=True)
            report_path.write_text(rendered, encoding="utf-8")
        print(rendered, end="")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
