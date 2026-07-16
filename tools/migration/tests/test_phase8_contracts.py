from __future__ import annotations

import importlib.util
import json
import shutil
import tempfile
import unittest
from pathlib import Path


SCRIPT = Path(__file__).resolve().parents[1] / "check_phase8_contracts.py"
SPEC = importlib.util.spec_from_file_location("check_phase8_contracts", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)

CONTRACT_FILES = (
    ".github/workflows/release.yml",
    "Cargo.toml",
    "Cargo.lock",
    "package.json",
    "package-lock.json",
    "apps/chataigne/Cargo.toml",
    "apps/chataigne/tauri.conf.json",
    "apps/chataigne/alchemist/src/formula_tests.rs",
    "apps/chataigne/src/module/common/mod.rs",
    "apps/chataigne/src/module/common/streaming/module_helpers.rs",
    "apps/chataigne/src/module/common/streaming/module_helpers_tests.rs",
    "apps/chataigne/src/module/common/serial.rs",
    "apps/chataigne/src/module/modules/controllers/buttplug/transport.rs",
    "apps/chataigne/src/module/modules/controllers/buttplug/mod.rs",
    "apps/chataigne/src/module/modules/controllers/buttplug/tests.rs",
    "apps/chataigne/src/module/modules/controllers/buttplug/commands/tests.rs",
    "apps/chataigne/src/module/modules/controllers/gamepad/gamepad.rs",
    "apps/chataigne/src/module/modules/controllers/gamepad/gamepad_tests.rs",
    "apps/chataigne/src/module/modules/controllers/joycon/mod.rs",
    "apps/chataigne/src/module/modules/controllers/joycon/tests.rs",
    "apps/chataigne/src/module/modules/controllers/joycon/runtime/runtime_tests.rs",
    "apps/chataigne/src/module/modules/controllers/keyboard/keyboard.rs",
    "apps/chataigne/src/module/modules/controllers/keyboard/keyboard_tests.rs",
    "apps/chataigne/src/module/modules/controllers/kinect2/kinect2.rs",
    "apps/chataigne/src/module/modules/controllers/kinect2/kinect2_tests.rs",
    "apps/chataigne/src/module/modules/controllers/mouse/mouse.rs",
    "apps/chataigne/src/module/modules/controllers/mouse/mouse_tests.rs",
    "apps/chataigne/src/module/modules/controllers/streamdeck/streamdeck.rs",
    "apps/chataigne/src/module/modules/controllers/streamdeck/streamdeck_tests.rs",
    "apps/chataigne/src/module/modules/controllers/ultraleap/ultraleap.rs",
    "apps/chataigne/src/module/modules/controllers/ultraleap/ultraleap_tests.rs",
    "apps/chataigne/src/module/modules/generators/metronomes/metronomes_tests.rs",
    "apps/chataigne/src/module/modules/generators/metronomes/mod.rs",
    "apps/chataigne/src/module/modules/generators/signals/signals_tests.rs",
    "apps/chataigne/src/module/modules/generators/signals/mod.rs",
    "apps/chataigne/src/module/modules/generators/spatializer.rs",
    "apps/chataigne/src/module/modules/generators/spatializer_tests.rs",
    "apps/chataigne/src/module/modules/protocol/stream/tcpclient/transport.rs",
    "apps/chataigne/src/module/modules/protocol/stream/websocketclient/transport.rs",
    "apps/chataigne/src/module/modules/protocol/midi/midi_message/midi_message_tests.rs",
    "apps/chataigne/src/module/modules/protocol/midi/midi_module.rs",
    "apps/chataigne/src/module/modules/protocol/midi/midi_module_tests.rs",
    "apps/chataigne/src/module/modules/protocol/http/mod.rs",
    "apps/chataigne/src/module/modules/protocol/http/tests.rs",
    "apps/chataigne/src/module/modules/protocol/http/transport.rs",
    "apps/chataigne/src/module/modules/protocol/mqtt/mod.rs",
    "apps/chataigne/src/module/modules/protocol/mqtt/tests.rs",
    "apps/chataigne/src/module/modules/protocol/mqtt/transport.rs",
    "apps/chataigne/src/module/modules/protocol/osc/generic_osc_module_tests.rs",
    "apps/chataigne/src/module/modules/protocol/osc/osc_module_base.rs",
    "apps/chataigne/src/module/modules/protocol/stream/serial/mod.rs",
    "apps/chataigne/src/module/modules/protocol/stream/serial/tests.rs",
    "apps/chataigne/src/module/modules/protocol/stream/tcpclient/mod.rs",
    "apps/chataigne/src/module/modules/protocol/stream/tcpclient/tests.rs",
    "apps/chataigne/src/module/modules/protocol/stream/tcpserver/mod.rs",
    "apps/chataigne/src/module/modules/protocol/stream/tcpserver/tests.rs",
    "apps/chataigne/src/module/modules/protocol/stream/udp/mod.rs",
    "apps/chataigne/src/module/modules/protocol/stream/udp/tests.rs",
    "apps/chataigne/src/module/modules/protocol/stream/websocketclient/mod.rs",
    "apps/chataigne/src/module/modules/protocol/stream/websocketclient/transport_tests.rs",
    "apps/chataigne/src/module/modules/protocol/stream/websocketserver/mod.rs",
    "apps/chataigne/src/module/modules/protocol/stream/websocketserver/transport_tests.rs",
    "apps/chataigne/src/module/modules/system/app_control/app_control.rs",
    "apps/chataigne/src/module/modules/system/app_control/app_control_runtime.rs",
    "apps/chataigne/src/module/modules/system/app_control/app_control_tests.rs",
    "apps/chataigne/src/module/modules/system/os/os.rs",
    "apps/chataigne/src/module/modules/system/os/os_runtime.rs",
    "apps/chataigne/src/module/modules/system/os/os_tests.rs",
    "apps/chataigne/src/module/script_api/tests.rs",
    "apps/chataigne/src/state_machine_nodes/catalog_tests.rs",
    "apps/chataigne/ui/src/lib/panels/modules/SpatializerEditorPanel.svelte",
    "apps/chataigne/ui/src/routes/dashboard/+layout.svelte",
    "crates/core/src/node/dashboard/mod.rs",
    "crates/core/src/node/dashboard/tests.rs",
    "crates/io/src/lib.rs",
    "crates/core/src/engine/runtime/limits.rs",
    "crates/core/src/runtime_center.rs",
    "crates/core/src/app.rs",
    "crates/core/src/app/app_tests.rs",
    "crates/core/src/script/mod.rs",
    "crates/script/src/lib.rs",
    "crates/script/tests/runtime_contract.rs",
    "crates/persistence/Cargo.toml",
    "crates/persistence/src/file_store.rs",
    "crates/persistence/src/file_store_tests.rs",
    "crates/host_desktop/src/desktop.rs",
    "crates/transport_server/src/project_host.rs",
    "crates/transport_server/src/ui_server.rs",
    "crates/transport_server/src/ui_server/tests.rs",
    "docs/product/manifests/phase8-cutovers.v1.json",
    "docs/product/manifests/phase8-hardware-evidence.v1.json",
    "docs/product/manifests/product-files.v1.json",
    "docs/product/manifests/product-surfaces.v1.json",
    "docs/product/migration-progress.md",
    "docs/release-readiness.md",
    "packages/golden-ui/components/panels/dashboard/DashboardCanvas.svelte",
    "packages/golden-ui/components/panels/dashboard/DashboardPanel.svelte",
    "packages/golden-ui/components/panels/dashboard/DashboardViewer.svelte",
    "tools/release/check-signing.mjs",
    "tools/release/sign-windows.ps1",
)


def copy_contract_tree(root: Path, copy: Path) -> None:
    relative_paths = set(CONTRACT_FILES)
    product_surfaces = json.loads(
        (root / "docs/product/manifests/product-surfaces.v1.json").read_text(
            encoding="utf-8"
        )
    )
    for entry in product_surfaces.get("entries", []):
        relative_paths.update(
            source["path"]
            for source in entry.get("sources", [])
            if source.get("path")
        )
    product_files = json.loads(
        (root / "docs/product/manifests/product-files.v1.json").read_text(
            encoding="utf-8"
        )
    )
    relative_paths.update(
        entry["path"] for entry in product_files.get("entries", []) if entry.get("path")
    )

    for relative in sorted(relative_paths):
        target = copy / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(root / relative, target)


class Phase8ContractTests(unittest.TestCase):
    def test_current_tree_satisfies_phase8_contracts(self) -> None:
        root = Path(__file__).resolve().parents[3]
        self.assertEqual(MODULE.collect_violations(root), [])

    def test_app_owned_pending_channel_is_rejected(self) -> None:
        root = Path(__file__).resolve().parents[3]
        with tempfile.TemporaryDirectory() as directory:
            copy = Path(directory)
            copy_contract_tree(root, copy)
            pending = copy / "apps/chataigne/src/module/common/pending_channel.rs"
            pending.write_text("pub fn pending_channel() {}\n", encoding="utf-8")
            violations = MODULE.collect_violations(copy)
            self.assertTrue(any("pending channel remains" in item for item in violations))

    def test_pending_phase8a_dashboard_is_rejected(self) -> None:
        root = Path(__file__).resolve().parents[3]
        with tempfile.TemporaryDirectory() as directory:
            copy = Path(directory)
            copy_contract_tree(root, copy)
            path = copy / "docs/product/manifests/phase8-cutovers.v1.json"
            dashboard = json.loads(path.read_text(encoding="utf-8"))
            dashboard["subphases"][0]["state"] = "pending"
            path.write_text(json.dumps(dashboard), encoding="utf-8")
            violations = MODULE.collect_violations(copy)
            self.assertTrue(any("8A is not recorded" in item for item in violations))

    def test_signal_without_compiled_kernel_is_rejected(self) -> None:
        root = Path(__file__).resolve().parents[3]
        with tempfile.TemporaryDirectory() as directory:
            copy = Path(directory)
            copy_contract_tree(root, copy)
            path = copy / "apps/chataigne/src/module/modules/generators/signals/mod.rs"
            source = path.read_text(encoding="utf-8").replace(
                ".with_compiled_kernel(SIGNALS_COMPILED_KERNEL)", ""
            )
            path.write_text(source, encoding="utf-8")
            violations = MODULE.collect_violations(copy)
            self.assertTrue(any("Signal does not declare" in item for item in violations))

    def test_osc_without_compiled_kernel_is_rejected(self) -> None:
        root = Path(__file__).resolve().parents[3]
        with tempfile.TemporaryDirectory() as directory:
            copy = Path(directory)
            copy_contract_tree(root, copy)
            path = copy / "apps/chataigne/src/module/modules/protocol/osc/osc_module_base.rs"
            source = path.read_text(encoding="utf-8").replace(
                ".with_compiled_kernel(OSC_COMPILED_KERNEL)", ""
            )
            path.write_text(source, encoding="utf-8")
            violations = MODULE.collect_violations(copy)
            self.assertTrue(any("OSC does not declare" in item for item in violations))

    def test_http_without_bounded_request_channel_is_rejected(self) -> None:
        root = Path(__file__).resolve().parents[3]
        with tempfile.TemporaryDirectory() as directory:
            copy = Path(directory)
            copy_contract_tree(root, copy)
            path = copy / "apps/chataigne/src/module/modules/protocol/http/transport.rs"
            source = path.read_text(encoding="utf-8").replace("mpsc::sync_channel", "mpsc::channel")
            path.write_text(source, encoding="utf-8")
            violations = MODULE.collect_violations(copy)
            self.assertTrue(any("HTTP request backpressure" in item for item in violations))

    def test_controller_without_compiled_kernel_is_rejected(self) -> None:
        root = Path(__file__).resolve().parents[3]
        with tempfile.TemporaryDirectory() as directory:
            copy = Path(directory)
            copy_contract_tree(root, copy)
            path = copy / "apps/chataigne/src/module/modules/controllers/gamepad/gamepad.rs"
            source = path.read_text(encoding="utf-8").replace(
                '.with_compiled_kernel("chataigne.runtime.gamepad")', ""
            )
            path.write_text(source, encoding="utf-8")
            violations = MODULE.collect_violations(copy)
            self.assertTrue(any("controller `gamepad`" in item for item in violations))

    def test_unnamed_system_worker_is_rejected(self) -> None:
        root = Path(__file__).resolve().parents[3]
        with tempfile.TemporaryDirectory() as directory:
            copy = Path(directory)
            copy_contract_tree(root, copy)
            path = copy / "apps/chataigne/src/module/modules/system/os/os_runtime.rs"
            source = path.read_text(encoding="utf-8").replace('"os-metrics"', '""')
            path.write_text(source, encoding="utf-8")
            violations = MODULE.collect_violations(copy)
            self.assertTrue(any("OS background runtime" in item for item in violations))

    def test_spatializer_without_compiled_topology_is_rejected(self) -> None:
        root = Path(__file__).resolve().parents[3]
        with tempfile.TemporaryDirectory() as directory:
            copy = Path(directory)
            copy_contract_tree(root, copy)
            path = copy / "apps/chataigne/src/module/modules/generators/spatializer.rs"
            source = path.read_text(encoding="utf-8").replace("triangulate(&points)", "Default::default()")
            path.write_text(source, encoding="utf-8")
            violations = MODULE.collect_violations(copy)
            self.assertTrue(any("Spatializer compiled topology" in item for item in violations))

    def test_missing_module_icon_is_rejected(self) -> None:
        root = Path(__file__).resolve().parents[3]
        with tempfile.TemporaryDirectory() as directory:
            copy = Path(directory)
            copy_contract_tree(root, copy)
            path = copy / "docs/product/manifests/product-files.v1.json"
            inventory = json.loads(path.read_text(encoding="utf-8"))
            inventory["entries"] = [
                entry
                for entry in inventory["entries"]
                if entry.get("path")
                != "apps/chataigne/ui/src/lib/assets/icons/nodes/osc_module.svg"
            ]
            path.write_text(json.dumps(inventory), encoding="utf-8")
            violations = MODULE.collect_violations(copy)
            self.assertTrue(any("missing module icons" in item for item in violations))

    def test_non_atomic_project_host_save_is_rejected(self) -> None:
        root = Path(__file__).resolve().parents[3]
        with tempfile.TemporaryDirectory() as directory:
            copy = Path(directory)
            copy_contract_tree(root, copy)
            path = copy / "crates/transport_server/src/project_host.rs"
            source = path.read_text(encoding="utf-8").replace(
                "golden_persistence::write_file_atomically_with_recovery(path.as_str(), encoded.json.as_bytes())",
                "fs::write(path.as_str(), encoded.json.as_bytes())",
                1,
            )
            path.write_text(source, encoding="utf-8")
            violations = MODULE.collect_violations(copy)
            self.assertTrue(any("non-atomically" in item for item in violations))

    def test_disabled_native_packaging_is_rejected(self) -> None:
        root = Path(__file__).resolve().parents[3]
        with tempfile.TemporaryDirectory() as directory:
            copy = Path(directory)
            copy_contract_tree(root, copy)
            path = copy / "apps/chataigne/tauri.conf.json"
            config = json.loads(path.read_text(encoding="utf-8"))
            config["bundle"]["active"] = False
            path.write_text(json.dumps(config), encoding="utf-8")
            violations = MODULE.collect_violations(copy)
            self.assertTrue(any("native packaging" in item for item in violations))


if __name__ == "__main__":
    unittest.main()
