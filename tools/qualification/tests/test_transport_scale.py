from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

from tools.qualification import transport_scale


def complete_output(target: int = 1_000, graph_roots: int = 72) -> str:
    snapshot = {
        "nodes": target + 250,
        "roots": graph_roots,
        "node_identity_sha256": "a" * 64,
        "root_identity_sha256": "c" * 64,
    }
    row = {
        "contract": transport_scale.CONTRACT,
        "status": "PASS",
        "minimum_live_nodes": target,
        "graph_roots": graph_roots,
        "load_ms": 120,
        "client_snapshots": [snapshot.copy() for _ in range(3)],
        "resync_reasons": ["project_loaded"] * 3,
        "reconnect_snapshot": snapshot.copy(),
        "subscribed_clients_after_reconnect": 3,
        "session_consistent": True,
        "edited_param_uuid": "11111111-1111-4111-8111-111111111111",
        "edited_value_delta_clients": 3,
        "edited_value_snapshot_clients": 3,
        "intent_applied": True,
        "reconnect_edited_value": True,
        "save_pending_at_edit_send": True,
        "edit_ack_before_save_response": True,
        "saved_reload_value": "before_concurrent_edit",
        "saved_reload_resync_reasons": ["cursor_ahead_of_server_time"] * 3,
        "saved_reload_snapshots": [snapshot.copy() for _ in range(3)],
        "saved_reload_full_identity_stable": True,
    }
    return f"{transport_scale.RESULT_PREFIX}{json.dumps(row)}\n"


class ProductTransportScaleTests(unittest.TestCase):
    def test_accepts_exact_three_client_reconnect_result(self) -> None:
        row = transport_scale.parse_probe_result(complete_output(), 0, 1_000, 72)
        self.assertEqual([item["nodes"] for item in row["client_snapshots"]], [1_250] * 3)

    def test_rejects_missing_duplicate_and_failed_probe(self) -> None:
        output = complete_output()
        with self.assertRaisesRegex(ValueError, "one passing"):
            transport_scale.parse_probe_result("", 0, 1_000, 72)
        with self.assertRaisesRegex(ValueError, "one passing"):
            transport_scale.parse_probe_result(output + output, 0, 1_000, 72)
        with self.assertRaisesRegex(ValueError, "one passing"):
            transport_scale.parse_probe_result(output, 1, 1_000, 72)

    def test_rejects_missing_snapshot_and_wrong_fixture(self) -> None:
        output = complete_output()
        with self.assertRaisesRegex(ValueError, "fields differ"):
            transport_scale.parse_probe_result(output.replace('"load_ms": 120, ', ""), 0, 1_000, 72)
        with self.assertRaisesRegex(ValueError, "generated fixture"):
            transport_scale.parse_probe_result(output, 0, 10_000, 72)
        row = json.loads(output.split(transport_scale.RESULT_PREFIX, 1)[1])
        row["client_snapshots"].pop()
        with self.assertRaisesRegex(ValueError, "three client snapshots"):
            transport_scale.parse_probe_result(
                f"{transport_scale.RESULT_PREFIX}{json.dumps(row)}", 0, 1_000, 72,
            )

    def test_rejects_incomplete_resync_and_reconnect(self) -> None:
        output = complete_output()
        with self.assertRaisesRegex(ValueError, "resync to every client"):
            transport_scale.parse_probe_result(
                output.replace('"project_loaded", "project_loaded", "project_loaded"',
                               '"project_loaded", "project_loaded", "missing"'),
                0, 1_000, 72,
            )
        with self.assertRaisesRegex(ValueError, "recover three subscribed"):
            transport_scale.parse_probe_result(
                output.replace('"subscribed_clients_after_reconnect": 3', '"subscribed_clients_after_reconnect": 2'),
                0, 1_000, 72,
            )
        with self.assertRaisesRegex(ValueError, "one runtime session"):
            transport_scale.parse_probe_result(
                output.replace('"session_consistent": true', '"session_consistent": false'),
                0, 1_000, 72,
            )

    def test_rejects_insufficient_nodes_or_roots(self) -> None:
        output = complete_output()
        with self.assertRaisesRegex(ValueError, "authored-node target"):
            transport_scale.parse_probe_result(output.replace('"nodes": 1250', '"nodes": 999'), 0, 1_000, 72)
        with self.assertRaisesRegex(ValueError, "authored graph roots"):
            transport_scale.parse_probe_result(output.replace('"roots": 72', '"roots": 71'), 0, 1_000, 72)

    def test_rejects_divergent_or_invalid_node_identity_digests(self) -> None:
        output = complete_output()
        row = json.loads(output.split(transport_scale.RESULT_PREFIX, 1)[1])
        row["reconnect_snapshot"]["node_identity_sha256"] = "b" * 64
        with self.assertRaisesRegex(ValueError, "different node identities"):
            transport_scale.parse_probe_result(
                f"{transport_scale.RESULT_PREFIX}{json.dumps(row)}", 0, 1_000, 72,
            )
        row["reconnect_snapshot"]["node_identity_sha256"] = "short"
        with self.assertRaisesRegex(ValueError, "valid node_identity_sha256 digest"):
            transport_scale.parse_probe_result(
                f"{transport_scale.RESULT_PREFIX}{json.dumps(row)}", 0, 1_000, 72,
            )

    def test_rejects_missing_client_edit_evidence(self) -> None:
        row = json.loads(complete_output().split(transport_scale.RESULT_PREFIX, 1)[1])
        row["edited_value_delta_clients"] = 2
        with self.assertRaisesRegex(ValueError, "did not reach all three clients"):
            transport_scale.parse_probe_result(
                f"{transport_scale.RESULT_PREFIX}{json.dumps(row)}", 0, 1_000, 72,
            )
        row["edited_value_delta_clients"] = 3
        row["reconnect_edited_value"] = False
        with self.assertRaisesRegex(ValueError, "survive reconnect"):
            transport_scale.parse_probe_result(
                f"{transport_scale.RESULT_PREFIX}{json.dumps(row)}", 0, 1_000, 72,
            )
        row["reconnect_edited_value"] = True
        row["edited_param_uuid"] = "invalid"
        with self.assertRaisesRegex(ValueError, "UUID is invalid"):
            transport_scale.parse_probe_result(
                f"{transport_scale.RESULT_PREFIX}{json.dumps(row)}", 0, 1_000, 72,
            )

    def test_rejects_missing_save_reload_evidence(self) -> None:
        row = json.loads(complete_output().split(transport_scale.RESULT_PREFIX, 1)[1])
        row["save_pending_at_edit_send"] = False
        with self.assertRaisesRegex(ValueError, "outstanding save request"):
            transport_scale.parse_probe_result(
                f"{transport_scale.RESULT_PREFIX}{json.dumps(row)}", 0, 1_000, 72,
            )
        row["save_pending_at_edit_send"] = True
        row["edit_ack_before_save_response"] = False
        with self.assertRaisesRegex(ValueError, "before the save response"):
            transport_scale.parse_probe_result(
                f"{transport_scale.RESULT_PREFIX}{json.dumps(row)}", 0, 1_000, 72,
            )
        row["edit_ack_before_save_response"] = True
        row["saved_reload_resync_reasons"][1] = "missing"
        with self.assertRaisesRegex(ValueError, "did not resync every client"):
            transport_scale.parse_probe_result(
                f"{transport_scale.RESULT_PREFIX}{json.dumps(row)}", 0, 1_000, 72,
            )
        row["saved_reload_resync_reasons"][1] = "cursor_ahead_of_server_time"
        row["saved_reload_value"] = "unknown"
        with self.assertRaisesRegex(ValueError, "valid edit ordering"):
            transport_scale.parse_probe_result(
                f"{transport_scale.RESULT_PREFIX}{json.dumps(row)}", 0, 1_000, 72,
            )
        row["saved_reload_value"] = "after_concurrent_edit"
        row["saved_reload_snapshots"][1]["root_identity_sha256"] = "b" * 64
        with self.assertRaisesRegex(ValueError, "changed the authored root identities"):
            transport_scale.parse_probe_result(
                f"{transport_scale.RESULT_PREFIX}{json.dumps(row)}", 0, 1_000, 72,
            )
        row["saved_reload_snapshots"][1]["root_identity_sha256"] = "c" * 64
        row["saved_reload_snapshots"][1]["node_identity_sha256"] = "b" * 64
        with self.assertRaisesRegex(ValueError, "different node identities"):
            transport_scale.parse_probe_result(
                f"{transport_scale.RESULT_PREFIX}{json.dumps(row)}", 0, 1_000, 72,
            )
        row["saved_reload_snapshots"][0]["node_identity_sha256"] = "b" * 64
        row["saved_reload_snapshots"][2]["node_identity_sha256"] = "b" * 64
        with self.assertRaisesRegex(ValueError, "full-identity claim"):
            transport_scale.parse_probe_result(
                f"{transport_scale.RESULT_PREFIX}{json.dumps(row)}", 0, 1_000, 72,
            )

    def test_output_directory_stays_under_workspace_target(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.assertEqual(
                transport_scale.resolve_output_dir(root, Path("target/transport-run")),
                root / "target" / "transport-run",
            )
            with self.assertRaisesRegex(ValueError, "child of the workspace target"):
                transport_scale.resolve_output_dir(root, root)
            existing = root / "target" / "existing"
            existing.mkdir(parents=True)
            (existing / "report.json").write_text("{}", encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "already contains artifacts"):
                transport_scale.resolve_output_dir(root, existing)


if __name__ == "__main__":
    unittest.main()
