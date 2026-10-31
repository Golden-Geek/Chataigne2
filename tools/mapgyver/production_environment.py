"""Build provenance and read-only environment evidence for the production qualifier."""

import argparse
import datetime
import hashlib
import json
import os
from pathlib import Path
import re
import runpy
import subprocess

ROOT = Path(__file__).resolve().parents[2]


class EnvironmentMismatch(ValueError):
    """Observed evidence contradicts the fixed environment requirement."""


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def read(path):
    with Path(path).open(encoding="utf-8-sig") as stream:
        return json.load(stream)


def write_new(path, value):
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("x", encoding="utf-8", newline="\n") as stream:
        json.dump(value, stream, indent=2, allow_nan=False)
        stream.write("\n")


def link_identity(text):
    """Use only tools/library directories actually present in rustc's link invocation."""
    tokens = []
    for match in re.finditer(r'"(?:[^"\\]|\\.)*"', text):
        try:
            tokens.append(json.loads(match[0]))
        except json.JSONDecodeError:
            continue
    linkers = sorted(set(token for token in tokens if token.lower().endswith("\\link.exe")))
    if len(linkers) != 1:
        raise ValueError("printed link invocation must identify exactly one selected MSVC linker")
    linker = Path(linkers[0])
    msvc = re.search(r"[\\/]VC[\\/]Tools[\\/]MSVC[\\/]([^\\/]+)", str(linker), re.I)
    if not msvc or not linker.is_file():
        raise ValueError("selected MSVC linker is absent or its version directory is unknown")
    libraries = []
    for token in sorted(set(tokens)):
        if not token.upper().startswith("/LIBPATH:"):
            continue
        directory = Path(token[len("/LIBPATH:"):])
        sdk = re.search(r"Windows Kits[\\/]10[\\/]Lib[\\/](\d+\.\d+\.\d+\.\d+)[\\/](um|ucrt)[\\/]x64[\\/]?$",
                        str(directory), re.I)
        if sdk:
            anchor = directory / ("kernel32.lib" if sdk[2].lower() == "um" else "ucrt.lib")
            if not anchor.is_file():
                raise ValueError("selected Windows SDK library is absent")
            libraries.append({"version": sdk[1], "kind": sdk[2].lower(), "directory": str(directory),
                              "anchor": str(anchor), "anchor_sha256": digest(anchor)})
    if {item["kind"] for item in libraries} != {"um", "ucrt"}:
        raise ValueError("printed link invocation lacks selected UM/UCRT Windows SDK directories")
    return {"linker": str(linker), "linker_sha256": digest(linker), "msvc_version": msvc[1],
            "sdk_libraries": libraries, "scope": "actual Rust final link invocation; not installed-tool inference"}


def source_identity(executable):
    qualifier = runpy.run_path(str(Path(__file__).with_name("production_qualification.py")))
    values = qualifier["fingerprints"](executable)["execution_input_sha256"]
    return hashlib.sha256(json.dumps(values, sort_keys=True).encode()).hexdigest()


def build(directory, windows_sdk):
    directory = Path(directory).resolve()
    if not directory.is_relative_to(ROOT / "target") or directory.exists():
        raise ValueError("build evidence must use a new directory below target")
    executable = ROOT / "target/release/native_qualification.exe"
    before = source_identity(executable)
    directory.mkdir(parents=True)
    windows_sdk = Path(windows_sdk).resolve()
    sdk_paths = [windows_sdk / kind / "x64" for kind in ("um", "ucrt")]
    if not all((path / name).is_file() for path, name in zip(sdk_paths, ("kernel32.lib", "ucrt.lib"))):
        raise ValueError("explicit Windows SDK must contain x64 UM/kernel32 and UCRT libraries")
    command = ["cargo", "rustc", "--locked", "--release", "-p", "MapGyver", "--features",
               "native-qualification", "--bin", "native_qualification", "--", "--print", "link-args"]
    # A fresh Cargo target emits no link command. Salt only this final binary's symbols so
    # each new provenance directory records a real link; dependencies keep their cache.
    salt = hashlib.sha256(str(directory).encode()).hexdigest()[:16]
    command.extend(["-C", "metadata=qualification-" + salt])
    for path in sdk_paths:
        command.extend(["-C", "link-arg=/LIBPATH:" + str(path)])
    with (directory / "link-stdout.log").open("xb") as out, (directory / "build-stderr.log").open("xb") as err:
        result = subprocess.run(command, cwd=ROOT, stdout=out, stderr=err, check=False)
    if result.returncode:
        raise RuntimeError(f"qualification build failed ({result.returncode}); inspect {directory}/build-stderr.log")
    if before != source_identity(executable):
        raise ValueError("execution source inputs changed during build")
    selected = link_identity((directory / "link-stdout.log").read_text(encoding="utf-8"))
    record = {"command": command, "executable_sha256": digest(executable), "source_set_sha256": before,
              "selected": selected, "link_stdout_sha256": digest(directory / "link-stdout.log"),
              "build_stderr_sha256": digest(directory / "build-stderr.log"),
              "rustc": subprocess.run(["rustc", "-Vv"], capture_output=True, text=True, check=True).stdout.strip(),
              "compiler_environment": {key: os.environ.get(key) for key in (
                  "RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "CARGO_BUILD_TARGET", "RUSTC_WRAPPER",
                  "RUSTC_WORKSPACE_WRAPPER", "CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER")}}
    write_new(directory / "build.json", record)
    print(json.dumps({"build_identity": str(directory / "build.json"), "selected": selected,
                      "executable_sha256": record["executable_sha256"]}))


def observed(inventory, *keys):
    value = inventory
    for key in keys:
        value = value[key]
    if value.get("status") != "observed" or not value.get("items"):
        raise ValueError("required inventory unavailable: " + ".".join(keys))
    return value["items"]


def validate_hardware(hardware):
    for adapter in hardware["adapters"]:
        for output in adapter["outputs"].get("Observed", []):
            rect = output["desktop_coordinates"]
            if not isinstance(rect, list) or len(rect) != 4 or any(type(value) is not int for value in rect):
                raise ValueError("native DXGI rectangle metadata is truncated or invalid")
    for display in hardware["displays"]:
        extent = display["bounds"]["extent"]
        if not isinstance(extent, dict) or any(type(extent[key]) is not int or extent[key] <= 0
                                                for key in ("width", "height")):
            raise ValueError("native physical display metadata is truncated or invalid")


def capture(executable, build_file, overlay_file, output):
    executable, build_file = Path(executable).resolve(), Path(build_file).resolve()
    provenance = read(build_file)
    if digest(executable) != provenance["executable_sha256"] or source_identity(executable) != provenance["source_set_sha256"]:
        raise ValueError("executable/source inputs differ from the recorded build")
    for name, key in [("link-stdout.log", "link_stdout_sha256"), ("build-stderr.log", "build_stderr_sha256")]:
        if digest(build_file.parent / name) != provenance[key]:
            raise ValueError("build evidence hash mismatch")
    if link_identity((build_file.parent / "link-stdout.log").read_text(encoding="utf-8")) != provenance["selected"]:
        raise ValueError("selected compiler/SDK identity changed")
    command = ["powershell.exe", "-NoLogo", "-NoProfile", "-NonInteractive", "-File",
               str(ROOT / "tools/mapgyver/collect-baseline.ps1"), "-HardwareExecutable", str(executable)]
    machine = json.loads(subprocess.run(command, cwd=ROOT, capture_output=True, encoding="utf-8-sig",
                                       check=True, timeout=45).stdout)
    hardware = observed(machine, "native_hardware")[0]
    validate_hardware(hardware)
    overlay = read(overlay_file) if overlay_file else None
    identity = {"build": provenance, "hardware": hardware,
                "os": observed(machine, "operating_system"), "drivers": observed(machine, "video_controllers"),
                "webview": observed(machine, "webview2"), "power_scheme": observed(machine, "power", "active_scheme"),
                "rustc": observed(machine, "toolchain", "rustc"),
                "overlay_state": overlay.get("state") if overlay else None,
                "webview_selection_environment": {key: os.environ.get(key) for key in (
                    "WEBVIEW2_BROWSER_EXECUTABLE_FOLDER", "WEBVIEW2_RELEASE_CHANNEL_PREFERENCE")}}
    # Battery charge and instantaneous WMI display counters are observations, not identity pins.
    identity["drivers"] = [{key: row[key] for key in ("Name", "PNPDeviceID", "DriverVersion", "DriverDate")}
                           for row in identity["drivers"]]
    identity["drivers"].sort(key=lambda row: row["PNPDeviceID"])
    identity["webview"].sort(key=lambda row: (row["name"], row["pv"]))
    write_new(output, {"captured_at_utc": machine["captured_at_utc"], "clock": machine["clock"],
                       "identity": identity, "overlay_observation": overlay,
                       "inventory": machine, "build_record_sha256": digest(build_file)})
    print(json.dumps({"snapshot": str(output), "adapters": hardware["adapters"], "overlay": overlay}))


def analyze(before, after, adapter, monitor, executable_sha256, begin_qpc, end_qpc, frequency, webview_runtime=None):
    if before is None or after is None:
        return {"observed_within_target": None, "reason": "paired environment snapshots are absent"}
    try:
        identity = before["identity"]
        if identity != after["identity"]:
            raise EnvironmentMismatch("environment identity changed across capture")
        if identity["build"]["executable_sha256"] != executable_sha256:
            raise EnvironmentMismatch("environment build belongs to a different executable")
        build = identity["build"]
        for value in (executable_sha256, build["source_set_sha256"], build["link_stdout_sha256"],
                      build["build_stderr_sha256"], build["selected"]["linker_sha256"]):
            if not isinstance(value, str) or not re.fullmatch("[0-9a-f]{64}", value):
                raise ValueError("incomplete build provenance hashes")
        if not build["selected"]["msvc_version"] or not build["rustc"]:
            raise ValueError("selected compiler identity is absent")
        sdk = build["selected"]["sdk_libraries"]
        if {item["kind"] for item in sdk} != {"um", "ucrt"} or any(
                not item["version"] or not re.fullmatch("[0-9a-f]{64}", item["anchor_sha256"]) for item in sdk):
            raise ValueError("selected UM/UCRT Windows SDK provenance is incomplete")
        for field in ("os", "drivers", "webview", "power_scheme", "rustc"):
            if not identity[field]:
                raise ValueError("missing environment identity: " + field)
        hardware = identity["hardware"]
        validate_hardware(hardware)
        if hardware["ac_line_status"] not in (0, 1) or type(hardware["ac_line_status"]) is not int:
            raise ValueError("AC power source is unavailable")
        if not webview_runtime or hardware["webview_default_runtime_version"] != webview_runtime:
            raise ValueError("host/default WebView runtime identities differ or are unavailable")
        if any(identity["webview_selection_environment"].values()):
            raise ValueError("custom WebView selection needs explicit runtime evidence")
        if before["clock"]["frequency"] != frequency or after["clock"]["frequency"] != frequency:
            raise EnvironmentMismatch("environment/capture QPC frequencies differ")
        if not (before["clock"]["begin_qpc"] <= before["clock"]["end_qpc"] <= begin_qpc
                <= end_qpc <= after["clock"]["begin_qpc"] <= after["clock"]["end_qpc"]):
            raise EnvironmentMismatch("environment observations do not bracket the capture")
        adapters = identity["hardware"]["adapters"]
        selected = [item for item in adapters if item["id"] == adapter and not item["software"]]
        if len(selected) != 1:
            raise ValueError("selected hardware adapter identity is ambiguous or absent")
        pci = f"VEN_{selected[0]['vendor_id']:04X}&DEV_{selected[0]['device_id']:04X}"
        if not any(pci in row["PNPDeviceID"].upper() and row["DriverVersion"] and row["DriverDate"]
                   for row in identity["drivers"]):
            raise ValueError("selected adapter lacks a pinned PCI driver observation")
        owners = [item["id"] for item in adapters for output in item["outputs"].get("Observed", [])
                  if output["name"] == monitor and output["attached_to_desktop"]]
        if len(owners) != 1:
            raise ValueError("selected display ownership is ambiguous or unavailable")
        for snapshot in (before, after):
            overlay = snapshot.get("overlay_observation")
            if not overlay or overlay.get("state") not in ("On", "Off") or not overlay.get("method") or not overlay.get("observed_at_utc"):
                raise ValueError("explicit overlay observations are absent")
            if overlay["state"] == "Off" and overlay.get("control_text") != "Enable and open overlay":
                raise ValueError("overlay Off lacks the native UI enable-action observation")
            overlay_utc = datetime.datetime.fromisoformat(overlay["observed_at_utc"])
            clock_utc = datetime.datetime.fromisoformat(snapshot["clock"]["utc_begin"])
            if not overlay_utc.tzinfo or not clock_utc.tzinfo or abs((clock_utc - overlay_utc).total_seconds()) > 120:
                raise ValueError("overlay observation is stale or lacks a UTC offset")
        origin = datetime.datetime.fromisoformat(before["clock"]["utc_begin"])
        before_overlay = datetime.datetime.fromisoformat(before["overlay_observation"]["observed_at_utc"])
        after_overlay = datetime.datetime.fromisoformat(after["overlay_observation"]["observed_at_utc"])
        start_utc = origin + datetime.timedelta(seconds=(begin_qpc - before["clock"]["begin_qpc"]) / frequency)
        end_utc = origin + datetime.timedelta(seconds=(end_qpc - before["clock"]["begin_qpc"]) / frequency)
        if before_overlay > start_utc or after_overlay < end_utc:
            raise ValueError("overlay observations do not bracket the native capture")
        if selected[0]["vendor_id"] == 0x10de and identity["overlay_state"] != "Off":
            raise EnvironmentMismatch("NVIDIA qualification requires overlay Off")
        return {"observed_within_target": True, "display_adapter": owners[0], "renderer_adapter": adapter,
                "same_adapter_display": owners[0] == adapter,
                "before_evidence_sha256": hashlib.sha256(json.dumps(before, sort_keys=True).encode()).hexdigest(),
                "after_evidence_sha256": hashlib.sha256(json.dumps(after, sort_keys=True).encode()).hexdigest(),
                "scope": "pinned environment/route identity; no cross-adapter performance guarantee"}
    except EnvironmentMismatch as error:
        return {"observed_within_target": False, "reason": str(error)}
    except (KeyError, TypeError, ValueError) as error:
        return {"observed_within_target": None, "reason": str(error)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    b = commands.add_parser("build")
    b.add_argument("--directory", type=Path, required=True)
    b.add_argument("--windows-sdk", type=Path, required=True,
                   help="explicit version directory under Windows Kits/10/Lib; never inferred from installed versions")
    c = commands.add_parser("capture")
    c.add_argument("--executable", type=Path, default=ROOT / "target/release/native_qualification.exe")
    c.add_argument("--build", type=Path, required=True)
    c.add_argument("--overlay", type=Path)
    c.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if args.command == "build":
        build(args.directory, args.windows_sdk)
    else:
        capture(args.executable, args.build, args.overlay, args.output)


if __name__ == "__main__":
    main()
