from __future__ import annotations

import importlib.util
import json
import tempfile
import unittest
from pathlib import Path


SCRIPT = Path(__file__).resolve().parents[1] / "check_phase7_contracts.py"
SPEC = importlib.util.spec_from_file_location("check_phase7_contracts", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)

CONTRACT_FILES = (
    "crates/core/src/ui_sync.rs",
    "crates/codegen_support/src/lib.rs",
    "crates/transport_server/src/ui_server.rs",
    "crates/transport_server/src/ui_server/outbound_queue.rs",
    "docs/product/manifests/phase7-cutovers.v1.json",
    "packages/golden-ui/transport/ws.ts",
    "packages/golden-ui/transport/http.ts",
    "packages/golden-ui/store/workbench.svelte.ts",
    "packages/golden-ui/index.ts",
)
GENERATED_FILES = ("UiClientMessage.ts", "UiServerMessage.ts", "UiDataPlane.ts", "UiControlPhase.ts")


def copy_contract_tree(root: Path, copy: Path) -> None:
    for relative in CONTRACT_FILES:
        target = copy / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text((root / relative).read_text(encoding="utf-8"), encoding="utf-8")
    generated = copy / "packages/golden-ui/generated/rust_protocol"
    generated.mkdir(parents=True)
    for name in GENERATED_FILES:
        (generated / name).write_text("export type Placeholder = never;\n", encoding="utf-8")


class Phase7ContractTests(unittest.TestCase):
    def test_current_tree_satisfies_phase7_contracts(self) -> None:
        root = Path(__file__).resolve().parents[3]
        self.assertEqual(MODULE.collect_violations(root), [])

    def test_hand_written_websocket_protocol_is_rejected(self) -> None:
        root = Path(__file__).resolve().parents[3]
        with tempfile.TemporaryDirectory() as directory:
            copy = Path(directory)
            copy_contract_tree(root, copy)

            server = copy / "crates/transport_server/src/ui_server.rs"
            server.write_text(server.read_text(encoding="utf-8") + "\nenum WsClientMessage {}\n", encoding="utf-8")
            violations = MODULE.collect_violations(copy)
            self.assertTrue(any("hand-declares" in violation for violation in violations))

    def test_incomplete_checkpoint_dashboard_is_rejected(self) -> None:
        root = Path(__file__).resolve().parents[3]
        with tempfile.TemporaryDirectory() as directory:
            copy = Path(directory)
            copy_contract_tree(root, copy)
            dashboard_path = copy / "docs/product/manifests/phase7-cutovers.v1.json"
            dashboard = json.loads(dashboard_path.read_text(encoding="utf-8"))
            dashboard["validation_state"] = "CONSTRUCTION"
            dashboard_path.write_text(json.dumps(dashboard), encoding="utf-8")

            violations = MODULE.collect_violations(copy)
            self.assertTrue(any("runnable checkpoint" in violation for violation in violations))


if __name__ == "__main__":
    unittest.main()
