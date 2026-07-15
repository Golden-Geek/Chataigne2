from __future__ import annotations

import json
import sys
import tempfile
import unittest
from pathlib import Path


MIGRATION_DIR = Path(__file__).resolve().parents[1]
if str(MIGRATION_DIR) not in sys.path:
    sys.path.insert(0, str(MIGRATION_DIR))

from check_phase3_contracts import (
    dashboard_violations,
    foundation_violations,
    graph_ui_violations,
    runtime_value_boundary_violations,
)


class Phase3ContractTests(unittest.TestCase):
    def test_foundation_and_statechart_ownership_are_enforced(self) -> None:
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
            alchemist = root / "apps/chataigne/alchemist/src"
            alchemist.mkdir(parents=True)
            (alchemist / "value.rs").write_text("pub use golden_values::Value;\n", encoding="utf-8")
            (alchemist / "domain.rs").write_text(
                "pub struct AlchemistGraphDomain;\n"
                "pub struct AlchemistGraphDocument;\n",
                encoding="utf-8",
            )
            (alchemist / "lib.rs").write_text(
                "pub mod domain;\npub use domain::AlchemistGraphDomain;\n", encoding="utf-8"
            )
            (root / "apps/chataigne/alchemist/Cargo.toml").write_text(
                "[dependencies]\ngolden_graph = { path = '../graph' }\n", encoding="utf-8"
            )
            statechart = root / "crates/golden_statechart/src"
            statechart.mkdir(parents=True)
            (statechart / "domain.rs").write_text(
                "pub struct StatechartGraphDomain;\n"
                "pub struct StatechartGraphDocument;\n"
                "pub struct StatechartGraphAdapter;\n",
                encoding="utf-8",
            )
            (statechart / "lib.rs").write_text(
                "mod domain;\npub use domain::StatechartGraphDomain;\n", encoding="utf-8"
            )
            statechart_manifest = root / "crates/golden_statechart/Cargo.toml"
            statechart_manifest.write_text(
                "[dependencies]\ngolden_graph = { path = '../graph' }\n", encoding="utf-8"
            )
            (root / "crates/values/src/lib.rs").write_text(
                "pub struct ColorValue { pub red: f64 }\n", encoding="utf-8"
            )
            violations = foundation_violations(root)
            self.assertTrue(any("golden_engine" in violation.message for violation in violations))

            statechart_manifest.write_text("[dependencies]\n", encoding="utf-8")
            violations = foundation_violations(root)
            self.assertTrue(
                any("statecharts do not consume golden_graph" in violation.message for violation in violations)
            )

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
                        "tested_tree_base": "qualified-commit",
                        "cross_platform_product_gate": {
                            "status": "PASS",
                            "run_url": "https://example.test/actions/runs/1",
                            "tested_commit": "qualified-commit",
                            "completed_at": "2026-07-14T21:03:25Z",
                            "platforms": {"windows": "PASS", "macos": "PASS", "linux": "PASS"},
                        },
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

    def test_graph_ui_ownership_and_generated_revision_are_enforced(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            package = root / "packages/golden-graph-ui"
            (package / "components").mkdir(parents=True)
            (package / "generated").mkdir()
            (package / "package.json").write_text(
                json.dumps({"name": "golden_graph_ui"}), encoding="utf-8"
            )
            (package / "components/GraphCanvas.svelte").write_text(
                "GraphPresentationDocument SpatialIndex topologyRevision presentationRevision\n",
                encoding="utf-8",
            )
            (package / "generated/GraphRevision.ts").write_text(
                "export type GraphRevision = {};\n", encoding="utf-8"
            )
            (package / "types.ts").write_text(
                "export type { GraphRevision } from './generated/GraphRevision';\n", encoding="utf-8"
            )
            alchemist = root / "packages/golden-alchemist-ui"
            alchemist.mkdir(parents=True)
            (alchemist / "index.ts").write_text("export {};\n", encoding="utf-8")
            app = root / "apps/chataigne/ui"
            app.mkdir(parents=True)
            (app / "package.json").write_text(
                json.dumps({"dependencies": {"golden_graph_ui": "*"}}), encoding="utf-8"
            )
            adapter = app / "src/lib/graph/legacyGraphDocumentAdapter.ts"
            adapter.parent.mkdir(parents=True)
            adapter.write_text(
                "// Pure LegacyGraphDocumentAdapter with no authority.\n", encoding="utf-8"
            )

            self.assertEqual(graph_ui_violations(root), [])
            (alchemist / "index.ts").write_text("export { GraphCanvas };\n", encoding="utf-8")
            violations = graph_ui_violations(root)
            self.assertTrue(any("still exports" in violation.message for violation in violations))


    def test_runtime_value_public_alias_cannot_return(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            values = root / "crates/values/src/lib.rs"
            values.parent.mkdir(parents=True)
            values.write_text("pub enum Value {}\n", encoding="utf-8")

            alchemist = root / "apps/chataigne/alchemist/src/lib.rs"
            alchemist.parent.mkdir(parents=True)
            alchemist.write_text(
                "pub use value::{StableRef, ValueComponent};\n"
                "pub(crate) use golden_values::Value as RuntimeValue;\n",
                encoding="utf-8",
            )

            for manifest in (
                root / "apps/chataigne/Cargo.toml",
                root / "apps/chataigne/state_machine/Cargo.toml",
            ):
                manifest.parent.mkdir(parents=True, exist_ok=True)
                manifest.write_text("[dependencies]\ngolden_values.workspace = true\n", encoding="utf-8")

            consumer = root / "apps/chataigne/src/lib.rs"
            consumer.parent.mkdir(parents=True)
            consumer.write_text("use golden_values::Value as RuntimeValue;\n", encoding="utf-8")
            self.assertEqual(runtime_value_boundary_violations(root), [])

            consumer.write_text(
                "use chataigne_alchemist::{RuntimeValue, StableRef};\n",
                encoding="utf-8",
            )
            violations = runtime_value_boundary_violations(root)
            self.assertTrue(any("retired Alchemist" in violation.message for violation in violations))


if __name__ == "__main__":
    unittest.main()
