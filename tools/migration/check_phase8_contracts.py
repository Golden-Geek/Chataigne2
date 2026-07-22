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
    dmx_module = read(
        root, "apps/chataigne/src/module/modules/protocol/dmx/mod.rs"
    )
    dmx_frame = read(
        root, "apps/chataigne/src/module/modules/protocol/dmx/frame.rs"
    )
    dmx_transport = read(
        root, "apps/chataigne/src/module/modules/protocol/dmx/transport.rs"
    )
    dmx_transport_tests = read(
        root, "apps/chataigne/src/module/modules/protocol/dmx/transport/tests.rs"
    )
    dmx_tests = read(
        root, "apps/chataigne/src/module/modules/protocol/dmx/dmx_tests.rs"
    )
    dmx_commands = read(
        root, "apps/chataigne/src/module/modules/protocol/dmx/commands.rs"
    )
    dmx_command_tests = read(
        root, "apps/chataigne/src/module/modules/protocol/dmx/command_tests.rs"
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
    app_control = read(
        root,
        "apps/chataigne/src/module/modules/system/app_control/app_control.rs",
    )
    app_control_runtime = read(
        root,
        "apps/chataigne/src/module/modules/system/app_control/app_control_runtime.rs",
    )
    app_control_tests = read(
        root,
        "apps/chataigne/src/module/modules/system/app_control/app_control_tests.rs",
    )
    os_module = read(root, "apps/chataigne/src/module/modules/system/os/os.rs")
    os_runtime = read(root, "apps/chataigne/src/module/modules/system/os/os_runtime.rs")
    os_tests = read(root, "apps/chataigne/src/module/modules/system/os/os_tests.rs")
    node_module = read(
        root, "apps/chataigne/src/module/modules/system/node_control/mod.rs"
    )
    node_commands = read(
        root, "apps/chataigne/src/module/modules/system/node_control/commands.rs"
    )
    node_tests = read(
        root,
        "apps/chataigne/src/module/modules/system/node_control/node_control_tests.rs",
    )
    node_command_tests = read(
        root,
        "apps/chataigne/src/module/modules/system/node_control/command_tests.rs",
    )
    spatializer = read(
        root,
        "apps/chataigne/src/module/modules/generators/spatializer.rs",
    )
    spatializer_tests = read(
        root,
        "apps/chataigne/src/module/modules/generators/spatializer_tests.rs",
    )
    spatializer_editor = read(
        root,
        "apps/chataigne/ui/src/lib/panels/modules/SpatializerEditorPanel.svelte",
    )
    dashboard_backend = read(root, "crates/core/src/node/dashboard/mod.rs")
    dashboard_tests = read(root, "crates/core/src/node/dashboard/tests.rs")
    dashboard_canvas = read(
        root,
        "packages/golden-ui/components/panels/dashboard/DashboardCanvas.svelte",
    )
    dashboard_panel = read(
        root,
        "packages/golden-ui/components/panels/dashboard/DashboardPanel.svelte",
    )
    dashboard_viewer = read(
        root,
        "packages/golden-ui/components/panels/dashboard/DashboardViewer.svelte",
    )
    dashboard_route = read(root, "apps/chataigne/ui/src/routes/dashboard/+layout.svelte")
    script_package = read(root, "crates/script/src/lib.rs")
    script_runtime = read(root, "crates/core/src/script/mod.rs")
    script_runtime_tests = read(root, "crates/script/tests/runtime_contract.rs")
    module_script_tests = read(root, "apps/chataigne/src/module/script_api/tests.rs")
    formula_catalog_tests = read(root, "apps/chataigne/src/state_machine_nodes/catalog_tests.rs")
    product_files = json.loads(
        read(root, "docs/product/manifests/product-files.v1.json")
    )
    product_surfaces = json.loads(
        read(root, "docs/product/manifests/product-surfaces.v1.json")
    )
    persistence_cargo = read(root, "crates/persistence/Cargo.toml")
    persistence_store = read(root, "crates/persistence/src/file_store.rs")
    persistence_tests = read(root, "crates/persistence/src/file_store_tests.rs")
    core_app = read(root, "crates/core/src/app.rs")
    core_app_tests = read(root, "crates/core/src/app/app_tests.rs")
    project_host = read(root, "crates/transport_server/src/project_host.rs")
    ui_server = read(root, "crates/transport_server/src/ui_server.rs")
    ui_server_tests = read(root, "crates/transport_server/src/ui_server/tests.rs")
    desktop_host = read(root, "crates/host_desktop/src/desktop.rs")
    formula_tests = read(root, "apps/chataigne/alchemist/src/formula_tests.rs")
    tauri_config = json.loads(read(root, "apps/chataigne/tauri.conf.json"))
    root_package = json.loads(read(root, "package.json"))
    package_lock = read(root, "package-lock.json")
    release_workflow = read(root, ".github/workflows/release.yml")
    release_preflight = read(root, "tools/release/check-signing.mjs")
    windows_signing = read(root, "tools/release/sign-windows.ps1")
    release_docs = read(root, "docs/release-readiness.md")
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
        "has no compiled kernel identity",
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

    for family, source, kernel in (
        ("App Control", app_control, "chataigne.runtime.app-control"),
        ("OS", os_module, "chataigne.runtime.os"),
    ):
        if kernel not in source or "with_compiled_kernel" not in source:
            violations.append(f"{family} does not declare its compiled kernel")
    for family, source, worker_name, interval in (
        ("App Control", app_control_runtime, "app-control-metrics", "WATCHED_APP_WORKER_INTERVAL"),
        ("OS", os_runtime, "os-metrics", "OS_METRICS_WORKER_INTERVAL"),
    ):
        for contract in ("thread::Builder", worker_name, interval, "unpark", "join"):
            if contract not in source:
                violations.append(f"{family} background runtime is missing `{contract}`")
    for platform_contract in ('cfg(windows)', 'target_os = "macos"', 'target_os = "linux"'):
        if platform_contract not in app_control_runtime and platform_contract not in os_runtime:
            violations.append(f"system runtime is missing platform contract `{platform_contract}`")
    for fixture, source in (
        ("app_control_module_stays_idle_without_watch_entries", app_control_tests),
        ("stale_running_value_only_reflects_actual_state_during_periodic_updates", app_control_tests),
        ("app_control_script_template_scaffolds_functions_and_callbacks", app_control_tests),
        ("os_module_updates_without_tree_snapshot", os_tests),
        ("wake_on_lan_magic_packet_has_expected_shape", os_tests),
        ("os_script_template_scaffolds_functions_and_callbacks", os_tests),
    ):
        if fixture not in source:
            violations.append(f"Phase 8F is missing system fixture `{fixture}`")

    for declaration in ('delaunator = "1.1.0"', "delaunator.workspace = true"):
        if declaration not in workspace and declaration not in app_manifest:
            violations.append(f"Spatializer is missing proven geometry dependency `{declaration}`")
    for contract in (
        "VoronoiTopology",
        "triangulate(&points)",
        "connect_collinear_topology",
        "topology.neighbours(current_index)",
    ):
        if contract not in spatializer:
            violations.append(f"Spatializer compiled topology is missing `{contract}`")
    for fixture in (
        "spatializer_math_covers_all_modes",
        "voronoi_weights_do_not_jump_when_source_crosses_target_cell_boundary",
        "value_layout_switches_between_source_and_target_centric_trees",
        "sparse_reload_preserves_single_declared_parameter_block",
        "spatializer_script_template_documents_value_matrix_surface",
        "spatializer_supported_scale_uses_sparse_delaunay_topology",
    ):
        if fixture not in spatializer_tests:
            violations.append(f"Phase 8G is missing Spatializer fixture `{fixture}`")
    for contract in (
        "sendSetParamIntent",
        "DragGesture",
        "selectedEndpoint",
        "startInspectorResize",
        "debugView",
    ):
        if contract not in spatializer_editor:
            violations.append(f"Spatializer editor is missing `{contract}`")

    for contract in (
        "DashboardWidgetTargetDescriptor",
        "DASHBOARD_PAGE_NODE_TYPE",
        "DASHBOARD_NODE_WIDGET_NODE_TYPE",
        "DASHBOARD_GENERIC_WIDGET_NODE_TYPE",
    ):
        if contract not in dashboard_backend:
            violations.append(f"dashboard backend is missing `{contract}`")
    for fixture in (
        "dashboard_catalog_exposes_pages_and_widgets",
        "dashboard_widgets_materialize_binding_constraints",
        "dashboard_widget_layout_dependencies_follow_parent_layout",
        "dashboard_page_vertical_layout_change_via_ui_intent_stabilizes",
        "ui_create_user_item_initial_params_materialize_dashboard_widget_without_follow_up_edits",
    ):
        if fixture not in dashboard_tests:
            violations.append(f"Phase 8G is missing dashboard fixture `{fixture}`")
    for contract in (
        "readDashboardDragPayload",
        "sendSetParamIntent",
        "FreeLayoutInteractionMode",
        "MarqueeSelectionState",
        "session?.selectNodes",
    ):
        if contract not in dashboard_canvas:
            violations.append(f"dashboard authoring canvas is missing `{contract}`")
    if "DashboardCanvas" not in dashboard_panel:
        violations.append("dashboard panel does not mount the authoring canvas")
    for contract in ("selectedPage", "hrefForPage"):
        if contract not in dashboard_viewer:
            violations.append(f"dashboard viewer is missing `{contract}`")
    if "DashboardViewer" not in dashboard_route:
        violations.append("dashboard route does not mount the persistent viewer")

    if "#![warn(missing_docs)]" not in script_package:
        violations.append("golden_script does not enforce its public package contract")
    for contract in (
        "ScriptBudgets",
        "callback_timed",
        "ScriptSourceStamp",
        "SCRIPT_HOST_CALL_BUDGET_MESSAGE",
        "ScriptRuntimeError::BudgetViolation",
    ):
        if contract not in script_runtime:
            violations.append(f"shared script runtime is missing `{contract}`")
    for fixture in (
        "public_script_runtime_loads_manifest_and_calls_exports",
        "public_script_runtime_enforces_host_call_budget",
        "public_script_runtime_reload_replaces_cached_manifest",
    ):
        if fixture not in script_runtime_tests:
            violations.append(f"Phase 8H is missing public script fixture `{fixture}`")
    if "module_script_templates_document_available_functions_for_each_module" not in module_script_tests:
        violations.append("Phase 8H is missing the module script-template surface fixture")

    phase8h_surface_baseline = {
        "anode": 37,
        "command": 50,
        "formula": 2,
        "module": 23,
        "node_type": 200,
        "panel": 14,
        "script_callback": 51,
        "script_method": 51,
        "script_snippet": 40,
        "script_template": 24,
    }
    phase8j_surface_expansion = {
        "anode": 0,
        "command": 5,
        "formula": 0,
        "module": 3,
        "node_type": 9,
        "panel": 0,
        "script_callback": 3,
        "script_method": 3,
        "script_snippet": 4,
        "script_template": 3,
    }
    expected_surface_counts = {
        kind: count + phase8j_surface_expansion[kind]
        for kind, count in phase8h_surface_baseline.items()
    }
    if product_surfaces.get("category_counts") != expected_surface_counts:
        violations.append(
            "Phase 8H baseline plus Phase 8J expansion registration counts drifted"
        )
    surface_entries = product_surfaces.get("entries", [])
    module_names = {
        entry.get("name") for entry in surface_entries if entry.get("kind") == "module"
    }
    template_scopes = {
        entry.get("facts", {}).get("scope")
        for entry in surface_entries
        if entry.get("kind") == "script_template"
    }
    if template_scopes - {"module"} != module_names or len(template_scopes) != len(module_names) + 1:
        violations.append("Phase 8H does not have exactly one script template per module")
    for entry in surface_entries:
        if entry.get("kind") not in {
            "formula",
            "script_callback",
            "script_method",
            "script_snippet",
            "script_template",
        }:
            continue
        for source in entry.get("sources", []):
            source_path = source.get("path")
            if not source_path or not (root / source_path).is_file():
                violations.append(f"Phase 8H surface `{entry.get('id')}` has a missing source")

    file_entries = product_files.get("entries", [])
    if product_files.get("category_counts") != {"asset": 93, "fixture": 3}:
        violations.append(
            "Phase 8H baseline plus Phase 8J expansion asset/fixture counts drifted"
        )
    asset_paths = {
        entry.get("path") for entry in file_entries if entry.get("kind") == "asset"
    }
    missing_module_icons = {
        module_name
        for module_name in module_names
        if f"apps/chataigne/ui/src/lib/assets/icons/nodes/{module_name}.svg" not in asset_paths
    }
    if missing_module_icons:
        violations.append(
            f"Phase 8H is missing module icons: {sorted(missing_module_icons)}"
        )
    for entry in file_entries:
        path = entry.get("path")
        if not path or not (root / path).is_file() or not entry.get("sha256"):
            violations.append(f"Phase 8H file inventory entry `{entry.get('id')}` is invalid")

    formulas = {
        entry.get("name"): entry
        for entry in surface_entries
        if entry.get("kind") == "formula"
    }
    if set(formulas) != {"Action", "Mapping"} or any(
        not entry.get("facts", {}).get("sha256") for entry in formulas.values()
    ):
        violations.append("Phase 8H formula assets do not exactly match Action and Mapping")
    for fixture in (
        "builtin_formula_with_sibling_svg_gets_icon_presentation",
        "builtin_formula_without_sibling_icon_has_no_icon_presentation",
        "processor_palette_builtin_items_expose_sibling_icon",
    ):
        if fixture not in formula_catalog_tests:
            violations.append(f"Phase 8H is missing formula-asset fixture `{fixture}`")

    if "golden_engine" in persistence_cargo:
        violations.append("golden_persistence still depends on the engine implementation")
    for contract in (
        "AtomicWriteFile",
        "RecoveryJournal",
        "backup_sha256",
        "pending_sha256",
        "write_file_atomically_with_recovery",
        "read_recovery_candidates",
        "restore_primary_from_backup",
    ):
        if contract not in persistence_store:
            violations.append(f"Phase 8I durable persistence is missing `{contract}`")
    for fixture in (
        "atomic_replacement_keeps_previous_complete_file_and_clears_journal",
        "recovery_candidates_preserve_corrupt_primary_and_last_complete_backup",
    ):
        if fixture not in persistence_tests:
            violations.append(f"Phase 8I is missing persistence fixture `{fixture}`")
    if "read_recovery_candidates" not in core_app or "push_project_file_recovery" not in core_app:
        violations.append("Phase 8I sparse project loader does not use recovery candidates")
    for fixture in (
        "recovering_sparse_file_load_uses_last_complete_atomic_backup",
        "large_sparse_project_roundtrip_preserves_ten_thousand_node_subtree",
    ):
        if fixture not in core_app_tests:
            violations.append(f"Phase 8I is missing project fixture `{fixture}`")
    if project_host.count("write_file_atomically_with_recovery") < 3:
        violations.append("project, preferences, and uploaded files are not all durable writes")
    if "fs::write(path.as_str(), encoded.json.as_bytes())" in project_host:
        violations.append("project host still writes the authoritative project non-atomically")

    for contract in ("headless", "GC_UI_BIND", "--no-remote", "run_with_ui_server_config"):
        if contract not in desktop_host:
            violations.append(f"Phase 8I host runtime is missing `{contract}`")
    for contract in ("UiDiscoveryDto", '"/.well-known/chataigne"', "relative_endpoints"):
        if contract not in ui_server:
            violations.append(f"Phase 8I host discovery is missing `{contract}`")
    if "discovery_document_uses_relative_open_lan_endpoints" not in ui_server_tests:
        violations.append("Phase 8I is missing the open-LAN discovery fixture")

    bundle = tauri_config.get("bundle", {})
    if bundle.get("active") is not True or bundle.get("targets") != "all":
        violations.append("Phase 8I Tauri native packaging is not active for all native targets")
    if set(bundle.get("icon", [])) != {"icons/icon.png", "icons/icon.ico"}:
        violations.append("Phase 8I Tauri package does not carry the release icons")
    if bundle.get("licenseFile") != "../../LICENSE":
        violations.append("Phase 8I Tauri package does not carry the repository license")
    scripts = root_package.get("scripts", {})
    if "tauri build" not in scripts.get("package", "") or "--no-bundle" not in scripts.get("package:check", ""):
        violations.append("Phase 8I root package and packaging-smoke commands are missing")
    if root_package.get("devDependencies", {}).get("@tauri-apps/cli") != "2.11.4":
        violations.append("Phase 8I Tauri CLI is not pinned in the root package")
    if '"node_modules/@tauri-apps/cli"' not in package_lock:
        violations.append("Phase 8I package lock does not contain the pinned Tauri CLI")
    for contract in ("windows-latest", "macos-15", "ubuntu-24.04", "actions/upload-artifact@v7"):
        if contract not in release_workflow:
            violations.append(f"Phase 8I native release matrix is missing `{contract}`")
    for contract in ("GC_REQUIRE_SIGNING", "APPLE_CERTIFICATE", "SIGN_KEY"):
        if contract not in release_preflight:
            violations.append(f"Phase 8I signing preflight is missing `{contract}`")
    for contract in ("WINDOWS_CERTIFICATE_THUMBPRINT", "WINDOWS_TIMESTAMP_URL", "signtool"):
        if contract.lower() not in windows_signing.lower():
            violations.append(f"Phase 8I Windows signing hook is missing `{contract}`")
    if "notarization" not in release_docs or "clean environment" not in release_docs:
        violations.append("Phase 8I release qualification documentation is incomplete")
    for fixture in (
        "formula_authoring_document_roundtrips_through_versioned_graph_envelope",
        "managed_region_kind_roundtrips_through_json",
    ):
        if fixture not in formula_tests:
            violations.append(f"Phase 8I is missing formula-schema fixture `{fixture}`")

    for declaration in (
        'artnet_protocol = "0.4.4"',
        'sacn = "0.11.1"',
        "artnet_protocol.workspace = true",
        "sacn.workspace = true",
    ):
        if declaration not in workspace and declaration not in app_manifest:
            violations.append(
                f"Phase 8J DMX expansion is missing reliable dependency `{declaration}`"
            )
    for family, node_type, kernel in (
        ("Art-Net", "artnet_module", "chataigne.runtime.artnet"),
        ("sACN", "sacn_module", "chataigne.runtime.sacn"),
    ):
        if node_type not in dmx_module or kernel not in dmx_module:
            violations.append(
                f"Phase 8J {family} module is missing its catalog type or compiled kernel"
            )
    for contract in (
        "DMX_SLOT_COUNT",
        "ARTNET_MAX_UNIVERSE",
        "SACN_MAX_UNIVERSE",
        "set_channel",
        "with_metadata",
    ):
        if contract not in dmx_frame:
            violations.append(f"Phase 8J DMX frame contract is missing `{contract}`")
    for contract in (
        "WORKER_COMMAND_CAPACITY",
        "sync_channel",
        "try_send",
        "latest_event",
        "replaced_frames",
        "stop_requested",
        "thread.join",
        "set_is_sending_discovery(false)",
        "set_nonblocking(true)",
        "AcnRootLayerProtocol::parse",
        "SacnSource",
        "ArtCommand",
    ):
        if contract not in dmx_transport:
            violations.append(f"Phase 8J bounded DMX transport is missing `{contract}`")
    for contract in (
        "ReconnectBackoff",
        "set_data_capabilities",
        "dmxFrameReceived",
        "setChannel",
        "sendFrame",
        "blackout",
    ):
        if contract not in dmx_module:
            violations.append(f"Phase 8J DMX module surface is missing `{contract}`")
    for contract in (
        "dmx_set_channel_command",
        "dmx_send_frame_command",
        "dmx_blackout_command",
    ):
        if contract not in dmx_commands:
            violations.append(f"Phase 8J DMX commands are missing `{contract}`")
    for fixture, source in (
        ("artnet_worker_sends_a_protocol_encoded_frame", dmx_transport_tests),
        ("artnet_worker_receives_latest_frame_without_an_unbounded_queue", dmx_transport_tests),
        ("sacn_worker_round_trips_a_unicast_frame", dmx_transport_tests),
        ("output_queue_reports_overload_instead_of_growing_without_bound", dmx_transport_tests),
        ("lighting_modules_are_distinct_catalog_items_with_distinct_kernels", dmx_tests),
        ("phase8j_modules_round_trip_through_sparse_project_persistence", dmx_tests),
        ("dmx_commands_are_project_creatable", dmx_command_tests),
    ):
        if fixture not in source:
            violations.append(f"Phase 8J is missing DMX fixture `{fixture}`")

    for contract in (
        'node("node_module"',
        "resolve_script_reference",
        ".cached_id()",
        "node_id_by_uuid",
        "param_value.is_none()",
        "setValue",
        "trigger",
        "nodeValueSet",
        "nodeTriggered",
    ):
        if contract not in node_module:
            violations.append(f"Phase 8J Node module is missing `{contract}`")
    for contract in (
        "node_set_value_command",
        "node_trigger_command",
        "ReferenceTargetKind::ParameterOnly",
    ):
        if contract not in node_commands:
            violations.append(f"Phase 8J Node commands are missing `{contract}`")
    for fixture, source in (
        ("node_script_methods_require_stable_references", node_tests),
        ("node_module_is_a_project_creatable_system_module", node_tests),
        ("node_commands_are_project_creatable", node_command_tests),
    ):
        if fixture not in source:
            violations.append(f"Phase 8J is missing Node fixture `{fixture}`")
    for contract in (
        "DMX_FUNCTION_DOCS",
        "DMX_CALLBACK_DOCS",
        "NODE_FUNCTION_DOCS",
        "NODE_CALLBACK_DOCS",
        "ArtNetModule::NODE_TYPE",
        "SacnModule::NODE_TYPE",
        "NodeModule::NODE_TYPE",
    ):
        if contract not in module_script_tests:
            violations.append(f"Phase 8J script surface fixture is missing `{contract}`")

    expansion_modules = {"artnet_module", "sacn_module", "node_module"}
    missing_expansion_modules = expansion_modules - module_names
    if missing_expansion_modules:
        violations.append(
            "Phase 8J new-feature catalog is missing modules: "
            f"{sorted(missing_expansion_modules)}"
        )

    subphases = {
        item.get("subphase_id"): item for item in dashboard.get("subphases", [])
    }
    if (
        dashboard.get("phase") != 8
        or dashboard.get("validation_state") != "CHECKPOINT_RUNNABLE"
    ):
        violations.append("Phase 8 dashboard does not record the runnable checkpoint")
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
    if subphases.get("8F", {}).get("state") != "runnable":
        violations.append("Phase 8F is not recorded as runnable")
    if subphases.get("8G", {}).get("state") != "runnable":
        violations.append("Phase 8G is not recorded as runnable")
    if subphases.get("8H", {}).get("state") != "runnable":
        violations.append("Phase 8H is not recorded as runnable")
    if subphases.get("8I", {}).get("state") != "runnable":
        violations.append("Phase 8I is not recorded as runnable")
    if subphases.get("8J", {}).get("state") != "runnable":
        violations.append("Phase 8J is not recorded as runnable")
    expected_report = "target/product-gate/phase8-final-candidate/product-gate-report.json"
    if expected_report not in dashboard.get("product_gate", ""):
        violations.append("latest Phase 8 product-gate evidence is not recorded")
    expected_commit = "b45a9b0a7a01ebee386e24a91daa42f897054bc6"
    if dashboard.get("tested_tree_base") != expected_commit:
        violations.append("Phase 8 dashboard does not name the qualified exact commit")
    cross_platform_gate = dashboard.get("cross_platform_gate", "")
    expected_run = "https://github.com/Golden-Geek/Chataigne2/actions/runs/29580856581"
    for required_evidence in (
        "PASS",
        expected_commit,
        expected_run,
        "native Windows, macOS, and Linux",
        "windows-arm64",
        "linux-aarch64",
        "linux-armhf",
        "aggregate exact-commit report passed",
    ):
        if required_evidence not in cross_platform_gate:
            violations.append(
                "Phase 8 cross-platform qualification is missing evidence: "
                f"{required_evidence}"
            )
    expected = {f"8{letter}" for letter in "ABCDEFGHIJ"}
    missing = expected - subphases.keys()
    if missing:
        violations.append(f"Phase 8 dashboard is missing subphases: {sorted(missing)}")
    if dashboard.get("carried_temporary_adapters") != [
        "phase6.app-domain-node-kernels"
    ]:
        violations.append("Phase 8 does not carry the governed Phase 6 domain adapter")
    if "State: `CHECKPOINT_RUNNABLE`; Phase 8" not in progress:
        violations.append("migration progress does not declare the Phase 8 runnable checkpoint")

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
    print("phase8 module and IO checkpoint contracts: PASS")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
