#!/usr/bin/env python3
"""Checks the active Phase 8 module and IO ownership boundaries."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path


def read(root: Path, relative: str) -> str:
    return (root / relative).read_text(encoding="utf-8")


def collect_violations(root: Path) -> list[str]:
    violations: list[str] = []
    workspace = read(root, "Cargo.toml")
    app_manifest = read(root, "apps/chataigne/Cargo.toml")
    io_lib = read(root, "crates/io/src/lib.rs")
    serial = read(root, "apps/chataigne/src/module/common/serial.rs")
    common_mod = read(root, "apps/chataigne/src/module/common/mod.rs")
    tcp = read(
        root,
        "apps/chataigne/src/module/modules/protocol/stream/tcpclient/transport.rs",
    )
    websocket = read(
        root,
        "apps/chataigne/src/module/modules/protocol/stream/websocketclient/transport.rs",
    )
    buttplug = read(
        root,
        "apps/chataigne/src/module/modules/controllers/buttplug/transport.rs",
    )
    limits = read(root, "crates/core/src/engine/runtime/limits.rs")
    runtime_center = read(root, "crates/core/src/runtime_center.rs")
    signals = read(
        root, "apps/chataigne/src/module/modules/generators/signals/mod.rs"
    )
    signal_tests = read(
        root,
        "apps/chataigne/src/module/modules/generators/signals/signals_tests.rs",
    )
    metronomes = read(
        root, "apps/chataigne/src/module/modules/generators/metronomes/mod.rs"
    )
    metronome_tests = read(
        root,
        "apps/chataigne/src/module/modules/generators/metronomes/metronomes_tests.rs",
    )
    osc = read(
        root,
        "apps/chataigne/src/module/modules/protocol/osc/osc_module_base.rs",
    )
    osc_tests = read(
        root,
        "apps/chataigne/src/module/modules/protocol/osc/generic_osc_module_tests.rs",
    )
    midi = read(
        root,
        "apps/chataigne/src/module/modules/protocol/midi/midi_module.rs",
    )
    midi_tests = read(
        root,
        "apps/chataigne/src/module/modules/protocol/midi/midi_module_tests.rs",
    )
    midi_codec_tests = read(
        root,
        "apps/chataigne/src/module/modules/protocol/midi/midi_message/midi_message_tests.rs",
    )
    dashboard = json.loads(
        read(root, "docs/product/manifests/phase8-cutovers.v1.json")
    )
    progress = read(root, "docs/product/migration-progress.md")

    for declaration in (
        '"crates/io"',
        'golden_io = { path = "crates/io" }',
    ):
        if declaration not in workspace:
            violations.append(f"workspace is missing `{declaration}`")
    if "golden_io.workspace = true" not in app_manifest:
        violations.append("Chataigne does not consume the workspace golden_io package")

    for exported_type in (
        "PendingReceiver",
        "ReconnectBackoff",
        "BoundedQueue",
        "WorkerTask",
    ):
        if exported_type not in io_lib:
            violations.append(f"golden_io does not export `{exported_type}`")
    if "pub mod testkit;" not in io_lib:
        violations.append("golden_io does not expose deterministic test transports")

    old_pending = root / "apps/chataigne/src/module/common/pending_channel.rs"
    if old_pending.exists() or "mod pending_channel" in common_mod:
        violations.append("the app-owned pending channel remains after golden_io extraction")

    for required_use in ("BoundedQueue", "WorkerTask", "ReconnectBackoff"):
        if required_use not in serial:
            violations.append(f"Serial does not consume golden_io::{required_use}")
    for family, source in (
        ("TCP client", tcp),
        ("WebSocket client", websocket),
        ("Buttplug", buttplug),
    ):
        if "ReconnectBackoff" not in source:
            violations.append(f"{family} does not consume the shared reconnect policy")

    if "with_compiled_kernel" not in limits:
        violations.append("NodeExecutionRule cannot declare a compiled domain kernel")
    for required_runtime_contract in (
        "CompiledScheduledNode",
        "domain_kernels",
        "DOMAIN_NODE_ADAPTER_KERNEL",
    ):
        if required_runtime_contract not in runtime_center:
            violations.append(
                f"runtime compilation is missing `{required_runtime_contract}`"
            )
    for family, source, kernel in (
        ("Signal", signals, "chataigne.runtime.signals"),
        ("Metronome", metronomes, "chataigne.runtime.metronomes"),
    ):
        if kernel not in source or "with_compiled_kernel" not in source:
            violations.append(f"{family} does not declare its compiled kernel")
    for fixture, source in (
        ("signal_worker_fixture_is_deterministic_across_cycles", signal_tests),
        (
            "metronome_worker_fixture_preserves_tick_multiplicity_and_count",
            metronome_tests,
        ),
    ):
        if fixture not in source:
            violations.append(f"Phase 8B is missing deterministic fixture `{fixture}`")

    for family, source, kernel in (
        ("OSC", osc, "chataigne.runtime.osc"),
        ("MIDI", midi, "chataigne.runtime.midi"),
    ):
        if kernel not in source or "with_compiled_kernel" not in source:
            violations.append(f"{family} does not declare its compiled kernel")
    for contract in (
        "OSC_INTERFACE_REFRESH_INTERVAL_SECS",
        "interface_refresh_due",
        "transport_dirty",
    ):
        if contract not in osc:
            violations.append(f"OSC recovery is missing `{contract}`")
    for contract in (
        "MIDI_PORT_REFRESH_INTERVAL_SECS",
        "refresh_port_options",
        "self.input_dirty = true",
        "self.output_dirty = true",
    ):
        if contract not in midi:
            violations.append(f"MIDI recovery is missing `{contract}`")
    for fixture, source in (
        ("incoming_multi_message_auto_adds_missing_path_with_batched_trees", osc_tests),
        ("osc_module_root_enable_toggle_stops_and_restarts_transport", osc_tests),
        ("send_custom_message_command_sends_osc_packet_through_module_output", osc_tests),
        ("sparse_project_round_trip_preserves_saved_osc_command_tester_children", osc_tests),
        ("incoming_note_messages_create_one_direct_velocity_param", midi_tests),
        ("incoming_system_messages_populate_mtc_and_midi_clock_folders", midi_tests),
        ("midi_module_script_template_scaffolds_midi_callbacks_only", midi_tests),
        ("note_on_round_trips_through_midly_codec", midi_codec_tests),
    ):
        if fixture not in source:
            violations.append(f"Phase 8C is missing parity fixture `{fixture}`")

    subphases = {
        item.get("subphase_id"): item for item in dashboard.get("subphases", [])
    }
    if dashboard.get("phase") != 8 or dashboard.get("validation_state") != "CONSTRUCTION":
        violations.append("Phase 8 dashboard does not record the construction interval")
    if subphases.get("8A", {}).get("state") != "runnable":
        violations.append("Phase 8A is not recorded as runnable")
    if subphases.get("8B", {}).get("state") != "runnable":
        violations.append("Phase 8B is not recorded as runnable")
    if subphases.get("8C", {}).get("state") != "runnable":
        violations.append("Phase 8C is not recorded as runnable")
    expected_report = "target/product-gate/20260716T103001Z/product-gate-report.json"
    if expected_report not in dashboard.get("product_gate", ""):
        violations.append("latest Phase 8 product-gate evidence is not recorded")
    expected = {f"8{letter}" for letter in "ABCDEFGHIJ"}
    missing = expected - subphases.keys()
    if missing:
        violations.append(f"Phase 8 dashboard is missing subphases: {sorted(missing)}")
    if dashboard.get("carried_temporary_adapters") != [
        "phase6.app-domain-node-kernels"
    ]:
        violations.append("Phase 8 does not carry the governed Phase 6 domain adapter")
    if "State: `CONSTRUCTION`; the Phase 8" not in progress:
        violations.append("migration progress does not declare the Phase 8 construction interval")

    return violations


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[2])
    args = parser.parse_args()
    violations = collect_violations(args.root.resolve())
    if violations:
        for violation in violations:
            print(f"phase8 contract violation: {violation}", file=sys.stderr)
        return 1
    print("phase8 module and IO construction contracts: PASS")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
