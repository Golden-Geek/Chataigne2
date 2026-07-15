from __future__ import annotations

import json
import sys
import tempfile
import unittest
from pathlib import Path


MIGRATION_DIR = Path(__file__).resolve().parents[1]
if str(MIGRATION_DIR) not in sys.path:
    sys.path.insert(0, str(MIGRATION_DIR))

from check_phase4_contracts import authoring_model_violations, dashboard_violations


class Phase4ContractTests(unittest.TestCase):
    def test_legacy_formula_graph_and_compiler_entry_are_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            alchemist = root / "apps/chataigne/alchemist/src"
            app = root / "apps/chataigne/src/state_machine_nodes"
            processor = root / "apps/chataigne/processor/src"
            state_machine = root / "apps/chataigne/state_machine/src"
            alchemist.mkdir(parents=True)
            app.mkdir(parents=True)
            processor.mkdir(parents=True)
            state_machine.mkdir(parents=True)
            (alchemist / "formula.rs").write_text("pub graph: AlchemistGraph,\n", encoding="utf-8")
            (alchemist / "domain.rs").write_text(
                "enum AuthoredGraphWire { Legacy(AlchemistGraph) }\n"
                "fn convert() { AlchemistGraphAdapter::to_document(); }\n",
                encoding="utf-8",
            )
            (alchemist / "graph.rs").write_text(
                "pub struct AlchemistGraph;\npub enum GraphEditError {}\n",
                encoding="utf-8",
            )
            (alchemist / "lib.rs").write_text(
                "pub mod serialize;\npub use domain::AlchemistGraphAdapter;\n",
                encoding="utf-8",
            )
            (alchemist / "compile.rs").write_text(
                "pub fn compile_graph() { AlchemistGraphAdapter::to_legacy(); }\nfn compile_legacy_graph() {}\n",
                encoding="utf-8",
            )
            (alchemist / "typing.rs").write_text(
                "pub fn solve_types(graph: &AlchemistGraph) { AlchemistGraphAdapter::to_legacy(graph); }\n",
                encoding="utf-8",
            )
            (alchemist / "pipeline.rs").write_text(
                "pub graph: AlchemistGraph\nfn lower() { AlchemistGraphAdapter::to_legacy(); }\n",
                encoding="utf-8",
            )
            (processor / "value_set_pipeline.rs").write_text(
                "fn lower() { AlchemistGraphAdapter::to_document(); }\n",
                encoding="utf-8",
            )
            (state_machine / "state_machine.rs").write_text(
                "guard_graph: Option<AlchemistGraph>\neffect_graph: Option<AlchemistGraph>\n"
                "fn compile() { AlchemistGraphAdapter::to_document(); }\n",
                encoding="utf-8",
            )
            (app / "formula.rs").write_text(
                "fn materialize() { let mut graph = AlchemistGraph::new(); "
                "AlchemistGraphAdapter::to_document(&graph); }\n",
                encoding="utf-8",
            )

            violations = authoring_model_violations(root)

            self.assertTrue(any("legacy graph shape" in violation.message for violation in violations))
            self.assertTrue(any("typed document" in violation.message for violation in violations))
            self.assertTrue(any("compiler still lowers" in violation.message for violation in violations))
            self.assertTrue(any("type solver still lowers" in violation.message for violation in violations))
            self.assertTrue(any("former AlchemistGraph entry point" in violation.message for violation in violations))
            self.assertTrue(any("managed filter lowering" in violation.message for violation in violations))
            self.assertTrue(any("ValueSet pipeline builders" in violation.message for violation in violations))
            self.assertTrue(any("live Formula subtree" in violation.message for violation in violations))
            self.assertTrue(any("transition guards" in violation.message for violation in violations))
            self.assertTrue(any("transition effects" in violation.message for violation in violations))
            self.assertTrue(any("transition graph compilation" in violation.message for violation in violations))
            self.assertTrue(any("former AlchemistGraph model" in violation.message for violation in violations))
            self.assertTrue(any("former graph edit API" in violation.message for violation in violations))
            self.assertTrue(any("removed graph compatibility" in violation.message for violation in violations))

    def test_complete_phase4_dashboard_is_accepted(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            path = root / "docs/product/manifests/phase4-cutovers.v1.json"
            path.parent.mkdir(parents=True)
            path.write_text(
                json.dumps(
                    {
                        "schema_version": 1,
                        "phase": 4,
                        "revision": 7,
                        "validation_state": "CHECKPOINT_RUNNABLE",
                        "product_gate": (
                            "PASS: Win-x64 local report "
                            "target/product-gate/20260715T130619Z/product-gate-report.json "
                            "(32/32 required checks passed; 7 non-required checks NOT_RUN)"
                        ),
                        "cutovers": [
                            {"cutover_id": cutover, "owner": "owner", "evidence": ["test"]}
                            for cutover in sorted({
                                "alchemist_ownership",
                                "alchemist_ui_ownership",
                                "alchemist_authoring_model",
                                "alchemist_compiler_model",
                                "alchemist_managed_pipeline_model",
                                "alchemist_production_document_model",
                                "alchemist_legacy_graph_removed",
                            })
                        ],
                        "temporary_adapters": [],
                    }
                ),
                encoding="utf-8",
            )

            self.assertEqual(dashboard_violations(root), [])


if __name__ == "__main__":
    unittest.main()
