#!/usr/bin/env sh
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repository_root=$(CDPATH= cd -- "$script_dir/../.." && pwd)
check_installed=false
if [ "${1-}" = "--check-installed" ]; then
  check_installed=true
elif [ "$#" -ne 0 ]; then
  echo "Usage: $0 [--check-installed]" >&2
  exit 2
fi

python3 - "$repository_root" "$check_installed" <<'PY'
import json
import platform
import re
import subprocess
import sys
from pathlib import Path

root = Path(sys.argv[1])
check_installed = sys.argv[2] == "true"
manifest_path = root / "tools/bootstrap/toolchain.json"
manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
if manifest.get("schema_version") != 1:
    raise SystemExit("Unsupported toolchain manifest schema.")

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
        "python": rf"^Python {re.escape(manifest['python']['version'])}(?:\s|$)",
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

print(json.dumps({
    "schema_version": 1,
    "status": "PASS",
    "manifest": str(manifest_path),
    "installed_versions_checked": check_installed,
    "rust": manifest["rust"]["channel"],
    "node": manifest["node"]["version"],
    "npm": manifest["node"]["npm_version"],
    "python": manifest["python"]["version"],
}, separators=(",", ":")))
PY
