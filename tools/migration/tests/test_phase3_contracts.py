from __future__ import annotations

import json
import sys
import tempfile
import unittest
from pathlib import Path


MIGRATION_DIR = Path(__file__).resolve().parents[1]
if str(MIGRATION_DIR) not in sys.path:
    sys.path.insert(0, str(MIGRATION_DIR))

from check_phase3_contracts import dashboard_violations, foundation_violations


class Phase3ContractTests(unittest.TestCase):
    def test_foundation_dependency_on_engine_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for crate in ("model", "values", "parameters", "context", "graph"):
                source = root / "crates" / crate / "src"
                source.mkdir(parents=True)
                (source / "lib.rs").write_text("pub struct Foundation;\n", encoding="utf-8")
            (root / "crates/model/Cargo.toml").write_text(
                "[dependencies]\ngolden_engine = { path = '../core' }\n", encoding="utf-8"
            )
            (root / "crates/values/Cargo.toml").write_text("[dependencies]\n", encoding="utf-8")
            (root / "crates/parameters/Cargo.toml").write_text("[dependencies]\n", encoding="utf-8")
            (root / "crates/context/Cargo.toml").write_text("[dependencies]\n", encoding="utf-8")
            (root / "crates/graph/Cargo.toml").write_text("[dependencies]\n", encoding="utf-8")
            identity = root / "crates/core/src/node/core"
            identity.mkdir(parents=True)
            (identity / "identity.rs").write_text(
                "pub use golden_model::{DeclId, NodeId, NodeUuid};\n"
                "pub use golden_parameters::NodeReference;\n",
                encoding="utf-8",
            )
            parameter = root / "crates/core/src/parameter"
            parameter.mkdir(parents=True)
            (parameter / "mod.rs").write_text("pub use golden_parameters::*;\n", encoding="utf-8")
            (root / "crates/core/src/contexts.rs").write_text("pub use golden_context::*;\n", encoding="utf-8")
            (root / "crates/core/src/color.rs").write_text(
                "pub use golden_parameters::Color;\n", encoding="utf-8"
            )
            alchemist = root / "crates/golden_alchemist/src"
            alchemist.mkdir(parents=True)
            (alchemist / "value.rs").write_text("pub use golden_values::Value;\n", encoding="utf-8")
            (root / "crates/values/src/lib.rs").write_text(
                "pub struct ColorValue { pub red: f64 }\n", encoding="utf-8"
            )
            violations = foundation_violations(root)
            self.assertTrue(any("golden_engine" in violation.message for violation in violations))

    def test_complete_dashboard_is_accepted(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            path = root / "docs/product/manifests/phase3-cutovers.v1.json"
            path.parent.mkdir(parents=True)
            cutover_ids = {
                "identifiers",
                "canonical_values",
                "parameters",
                "context",
                "graph_contract",
                "graph_ui",
                "test_domain",
                "alchemist_domain",
                "statechart_domain",
            }
            path.write_text(
                json.dumps(
                    {
                        "schema_version": 1,
                        "phase": 3,
                        "cutovers": [
                            {"cutover_id": value, "state": "pending", "owner": "architecture", "evidence": ["test"]}
                            for value in sorted(cutover_ids)
                        ],
                        "temporary_adapters": [],
                    }
                ),
                encoding="utf-8",
            )
            self.assertEqual(dashboard_violations(root), [])


if __name__ == "__main__":
    unittest.main()
