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
    http = read(root, "apps/chataigne/src/module/modules/protocol/http/mod.rs")
    http_transport = read(
        root, "apps/chataigne/src/module/modules/protocol/http/transport.rs"
    )
    http_tests = read(root, "apps/chataigne/src/module/modules/protocol/http/tests.rs")
    mqtt = read(root, "apps/chataigne/src/module/modules/protocol/mqtt/mod.rs")
    mqtt_transport = read(
        root, "apps/chataigne/src/module/modules/protocol/mqtt/transport.rs"
    )
    mqtt_tests = read(root, "apps/chataigne/src/module/modules/protocol/mqtt/tests.rs")
    stream_queue = read(
        root, "apps/chataigne/src/module/common/streaming/module_helpers.rs"
    )
    stream_queue_tests = read(
        root, "apps/chataigne/src/module/common/streaming/module_helpers_tests.rs"
    )
    serial_module = read(
        root, "apps/chataigne/src/module/modules/protocol/stream/serial/mod.rs"
    )
    serial_tests = read(
        root, "apps/chataigne/src/module/modules/protocol/stream/serial/tests.rs"
    )
    tcp_client_module = read(
        root, "apps/chataigne/src/module/modules/protocol/stream/tcpclient/mod.rs"
    )
    tcp_client_tests = read(
        root, "apps/chataigne/src/module/modules/protocol/stream/tcpclient/tests.rs"
    )
    tcp_server_module = read(
        root, "apps/chataigne/src/module/modules/protocol/stream/tcpserver/mod.rs"
    )
    tcp_server_tests = read(
        root, "apps/chataigne/src/module/modules/protocol/stream/tcpserver/tests.rs"
    )
    udp_module = read(
        root, "apps/chataigne/src/module/modules/protocol/stream/udp/mod.rs"
    )
    udp_tests = read(
        root, "apps/chataigne/src/module/modules/protocol/stream/udp/tests.rs"
    )
    websocket_client_module = read(
        root,
        "apps/chataigne/src/module/modules/protocol/stream/websocketclient/mod.rs",
    )
    websocket_client_tests = read(
        root,
        "apps/chataigne/src/module/modules/protocol/stream/websocketclient/transport_tests.rs",
    )
    websocket_server_module = read(
        root,
        "apps/chataigne/src/module/modules/protocol/stream/websocketserver/mod.rs",
    )
    websocket_server_tests = read(
        root,
        "apps/chataigne/src/module/modules/protocol/stream/websocketserver/transport_tests.rs",
    )
    controller_sources = {
        "buttplug": read(root, "apps/chataigne/src/module/modules/controllers/buttplug/mod.rs"),
        "gamepad": read(root, "apps/chataigne/src/module/modules/controllers/gamepad/gamepad.rs"),
        "joycon": read(root, "apps/chataigne/src/module/modules/controllers/joycon/mod.rs"),
        "keyboard": read(root, "apps/chataigne/src/module/modules/controllers/keyboard/keyboard.rs"),
        "kinect2": read(root, "apps/chataigne/src/module/modules/controllers/kinect2/kinect2.rs"),
        "mouse": read(root, "apps/chataigne/src/module/modules/controllers/mouse/mouse.rs"),
        "streamdeck": read(root, "apps/chataigne/src/module/modules/controllers/streamdeck/streamdeck.rs"),
        "ultraleap": read(root, "apps/chataigne/src/module/modules/controllers/ultraleap/ultraleap.rs"),
    }
    controller_tests = {
        "buttplug": read(root, "apps/chataigne/src/module/modules/controllers/buttplug/tests.rs"),
        "buttplug_commands": read(root, "apps/chataigne/src/module/modules/controllers/buttplug/commands/tests.rs"),
        "gamepad": read(root, "apps/chataigne/src/module/modules/controllers/gamepad/gamepad_tests.rs"),
        "joycon": read(root, "apps/chataigne/src/module/modules/controllers/joycon/tests.rs"),
        "joycon_runtime": read(root, "apps/chataigne/src/module/modules/controllers/joycon/runtime/runtime_tests.rs"),
        "keyboard": read(root, "apps/chataigne/src/module/modules/controllers/keyboard/keyboard_tests.rs"),
        "kinect2": read(root, "apps/chataigne/src/module/modules/controllers/kinect2/kinect2_tests.rs"),
        "mouse": read(root, "apps/chataigne/src/module/modules/controllers/mouse/mouse_tests.rs"),
        "streamdeck": read(root, "apps/chataigne/src/module/modules/controllers/streamdeck/streamdeck_tests.rs"),
        "ultraleap": read(root, "apps/chataigne/src/module/modules/controllers/ultraleap/ultraleap_tests.rs"),
    }
    hardware = json.loads(
        read(root, "docs/product/manifests/phase8-hardware-evidence.v1.json")
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

    for family, source, kernel in (
        ("HTTP", http, "chataigne.runtime.http"),
        ("MQTT", mqtt, "chataigne.runtime.mqtt"),
        ("Serial", serial_module, "chataigne.runtime.serial"),
        ("TCP client", tcp_client_module, "chataigne.runtime.tcp"),
        ("TCP server", tcp_server_module, "chataigne.runtime.tcp"),
        ("UDP", udp_module, "chataigne.runtime.udp"),
        ("WebSocket client", websocket_client_module, "chataigne.runtime.websocket"),
        ("WebSocket server", websocket_server_module, "chataigne.runtime.websocket"),
    ):
        if kernel not in source or "with_compiled_kernel" not in source:
            violations.append(f"{family} does not declare its compiled kernel")
    for contract in (
        "BoundedQueue<StreamingIncomingMessage>",
        "MAX_PENDING_STREAM_MESSAGES",
        "MAX_PENDING_STREAM_WEIGHT",
        "take_dropped_message_count",
    ):
        if contract not in stream_queue:
            violations.append(f"stream backpressure is missing `{contract}`")
    if "incoming_stream_queue_applies_bounded_backpressure_without_reordering" not in stream_queue_tests:
        violations.append("Phase 8D is missing the bounded stream-queue fixture")
    for family, source in (("HTTP", http_transport), ("MQTT", mqtt_transport)):
        for contract in ("sync_channel", "try_send", "TrySendError::Full"):
            if contract not in source:
                violations.append(f"{family} request backpressure is missing `{contract}`")
    for fixture, source in (
        ("large_json_response_auto_adds_values_in_one_runtime_tick", http_tests),
        ("repeated_large_json_responses_update_existing_values_without_duplicate_folders", http_tests),
        ("incoming_json_publish_expands_under_topic", mqtt_tests),
        ("script_publish_json_request_encodes_payload_qos_and_retain", mqtt_tests),
        ("serial_module_root_enable_toggle_stops_and_restarts_transport_while_recovering", serial_tests),
        ("tcp_module_recovers_when_server_appears_and_after_connection_loss", tcp_client_tests),
        ("tcp_server_transport_closes_client_connections_when_stopped", tcp_server_tests),
        ("udp_module_root_enable_toggle_stops_and_restarts_transport", udp_tests),
        ("websocket_transport_client_connects_to_server", websocket_client_tests),
        ("websocket_transport_server_accepts_websocket_connection", websocket_server_tests),
    ):
        if fixture not in source:
            violations.append(f"Phase 8D is missing transport fixture `{fixture}`")

    for family, source in controller_sources.items():
        kernel = f"chataigne.runtime.{family}"
        if kernel not in source or "with_compiled_kernel" not in source:
            violations.append(f"controller `{family}` does not declare its compiled kernel")
    controller_fixtures = (
        ("buttplug_path_is_normalized_for_websocket_url", controller_tests["buttplug"]),
        ("buttplug_commands_are_module_command_items", controller_tests["buttplug_commands"]),
        ("selected_gamepad_events_update_values_after_axis_processing", controller_tests["gamepad"]),
        ("joycon_module_command_tester_creates_joycon_commands", controller_tests["joycon"]),
        ("joycon_report_heartbeat_marks_stale_after_timeout", controller_tests["joycon_runtime"]),
        ("keyboard_events_update_values", controller_tests["keyboard"]),
        ("reference_space_changes_joint_output_origin", controller_tests["kinect2"]),
        ("mouse_events_update_values", controller_tests["mouse"]),
        ("feedback_pushes_active_page_color_to_device", controller_tests["streamdeck"]),
        ("ultraleap_disconnect_resets_tracking_outputs", controller_tests["ultraleap"]),
    )
    for fixture, source in controller_fixtures:
        if fixture not in source:
            violations.append(f"Phase 8E is missing controller fixture `{fixture}`")
    hardware_entries = {entry.get("id"): entry for entry in hardware.get("entries", [])}
    expected_hardware = set(controller_sources)
    if set(hardware_entries) != expected_hardware:
        violations.append("Phase 8E hardware evidence does not exactly cover the controller registry")
    for family in expected_hardware:
        entry = hardware_entries.get(family, {})
        if entry.get("qualification") != "PASS":
            violations.append(f"controller `{family}` lacks deterministic adapter qualification")
        if not entry.get("adapter") or not entry.get("evidence"):
            violations.append(f"controller `{family}` lacks named adapter evidence")
        if not str(entry.get("physical_hardware", "")).startswith("NOT_RUN:"):
            violations.append(f"controller `{family}` physical-device status is not explicit")

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
    if subphases.get("8D", {}).get("state") != "runnable":
        violations.append("Phase 8D is not recorded as runnable")
    if subphases.get("8E", {}).get("state") != "runnable":
        violations.append("Phase 8E is not recorded as runnable")
    expected_report = "target/product-gate/20260716T110028Z/product-gate-report.json"
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
