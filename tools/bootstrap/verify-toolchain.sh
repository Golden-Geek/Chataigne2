#!/usr/bin/env sh
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repository_root=$(CDPATH= cd -- "$script_dir/../.." && pwd)
check_installed=false
check_qualification_tools=false
if [ "${1-}" = "--check-installed" ]; then
  check_installed=true
  shift
fi
if [ "${1-}" = "--check-qualification-tools" ]; then
  check_qualification_tools=true
  shift
fi
if [ "$#" -ne 0 ]; then
  echo "Usage: $0 [--check-installed] [--check-qualification-tools]" >&2
  exit 2
fi

python3 - "$repository_root" "$check_installed" "$check_qualification_tools" <<'PY'
import json
import platform
import re
import subprocess
import sys
from pathlib import Path

root = Path(sys.argv[1])
check_installed = sys.argv[2] == "true"
check_qualification_tools = sys.argv[3] == "true"
manifest_path = root / "tools/bootstrap/toolchain.json"
manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
if manifest.get("schema_version") != 1:
    raise SystemExit("Unsupported toolchain manifest schema.")

asio_sdk = manifest["audio"]["asio_sdk"]
if not re.fullmatch(r"[0-9a-f]{40}", asio_sdk["revision"]):
    raise SystemExit("The ASIO SDK revision must be a full lowercase Git commit.")
if asio_sdk["repository"] != "https://github.com/audiosdk/asio.git":
    raise SystemExit("The ASIO SDK repository must be the supported audiosdk/asio source.")
if not asio_sdk["repository"] or not asio_sdk["license_mode"] or not asio_sdk["required_paths"]:
    raise SystemExit("The ASIO SDK contract is incomplete.")
audio = manifest["audio"]
if set(audio["application_default_features"]) != {"asio", "jack", "realtime"}:
    raise SystemExit("Application audio defaults must contain exactly ASIO, JACK, and realtime.")
platform_contracts = {
    "windows": ("wasapi", {"wasapi", "asio", "jack"}, {"x86_64", "aarch64"}),
    "macos": ("coreaudio", {"coreaudio", "jack"}, {"x86_64", "aarch64"}),
    "linux": ("alsa", {"alsa", "jack"}, {"x86_64", "aarch64"}),
}
for name, (native, defaults, architectures) in platform_contracts.items():
    platform_audio = audio[name]
    if (
        platform_audio["native_host"] != native
        or set(platform_audio["application_default_hosts"]) != defaults
        or set(platform_audio["supported_architectures"]) != architectures
    ):
        raise SystemExit(f"Audio artifact matrix mismatch for {name}.")
    if defaults.intersection(platform_audio.get("optional_hosts", [])):
        raise SystemExit(f"Default audio hosts must not also be optional for {name}.")
if set(audio["linux"]["headless_only_architectures"]) != {"armv7"}:
    raise SystemExit("Linux armv7 must remain explicitly headless-only.")
if set(audio["linux"]["optional_hosts"]) != {"pipewire"}:
    raise SystemExit("Native PipeWire must remain the explicit optional Linux host.")

rust_version = (root / manifest["consumers"]["rust_version"]).read_text(encoding="utf-8").strip()
node_version = (root / manifest["consumers"]["node_version"]).read_text(encoding="utf-8").strip()
if rust_version != manifest["rust"]["channel"]:
    raise SystemExit("tools/bootstrap/rust-version does not match the canonical manifest.")
if node_version != manifest["node"]["version"]:
    raise SystemExit(".nvmrc does not match the canonical manifest.")

def command(*args):
    return subprocess.run(args, check=True, text=True, capture_output=True).stdout.strip()

if check_installed:
    rustc = command("rustc", "--version")
    cargo = command("cargo", "--version")
    node = command("node", "--version")
    npm = command("npm", "--version")
    python = command("python3", "--version")
    expected = {
        "rustc": rf"^rustc {re.escape(manifest['rust']['channel'])}(?:\s|$)",
        "cargo": rf"^cargo {re.escape(manifest['rust']['cargo_version'])}(?:\s|$)",
        "node": rf"^v{re.escape(manifest['node']['version'])}$",
        "npm": rf"^{re.escape(manifest['node']['npm_version'])}$",
        "python": rf"^Python {re.escape(manifest['python']['version'])}(?:\.\d+)?(?:\s|$)",
    }
    actual = {"rustc": rustc, "cargo": cargo, "node": node, "npm": npm, "python": python}
    for name, pattern in expected.items():
        if not re.search(pattern, actual[name]):
            raise SystemExit(f"Installed {name} does not match the canonical manifest: {actual[name]}")

    machine = platform.machine().lower()
    system = platform.system()
    if system == "Darwin":
        host_key = "macos_arm64" if machine in {"arm64", "aarch64"} else "macos_x64"
    elif system == "Linux":
        host_key = "linux_arm64" if machine in {"arm64", "aarch64"} else "linux_x64"
    else:
        raise SystemExit(f"Unsupported POSIX verification host: {system} {machine}")
    verbose = command("rustc", "-vV")
    host_match = re.search(r"(?m)^host:\s*(\S+)\s*$", verbose)
    expected_host = manifest["rust"]["hosts"][host_key]
    if not host_match or host_match.group(1) != expected_host:
        actual_host = host_match.group(1) if host_match else "missing"
        raise SystemExit(f"Rust host mismatch: expected {expected_host}, got {actual_host}")

if check_qualification_tools:
    cargo_deny = command("cargo-deny", "--version")
    cargo_machete = command("cargo-machete", "--version")
    expected_deny = manifest["qualification_tools"]["cargo_deny"]
    if not re.search(rf"^cargo-deny {re.escape(expected_deny)}(?:\s|$)", cargo_deny):
        raise SystemExit(f"Installed cargo-deny does not match pinned {expected_deny}: {cargo_deny}")
    expected_machete = manifest["qualification_tools"]["cargo_machete"]
    if cargo_machete != expected_machete:
        raise SystemExit(
            f"Installed cargo-machete does not match pinned {expected_machete}: {cargo_machete}"
        )

print(json.dumps({
    "schema_version": 1,
    "status": "PASS",
    "manifest": str(manifest_path),
    "installed_versions_checked": check_installed,
    "qualification_tools_checked": check_qualification_tools,
    "rust": manifest["rust"]["channel"],
    "node": manifest["node"]["version"],
    "npm": manifest["node"]["npm_version"],
    "python": manifest["python"]["version"],
}, separators=(",", ":")))
PY
