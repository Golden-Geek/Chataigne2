from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

from tools.qualification import transport_scale


def complete_output(target: int = 1_000, graph_roots: int = 72) -> str:
    snapshot = {"nodes": target + 250, "roots": graph_roots, "node_identity_sha256": "a" * 64}
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
        with self.assertRaisesRegex(ValueError, "valid node-identity digest"):
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
