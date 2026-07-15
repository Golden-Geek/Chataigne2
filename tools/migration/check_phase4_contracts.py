"""Validate Phase 4 Alchemist ownership and typed-graph cutovers."""

from __future__ import annotations

import json
import re
import sys
from dataclasses import dataclass
from pathlib import Path


ALCHEMIST_PATH = Path("apps/chataigne/alchemist")
REQUIRED_CUTOVERS = {
    "alchemist_ownership",
    "alchemist_ui_ownership",
    "alchemist_authoring_model",
    "alchemist_compiler_model",
    "alchemist_managed_pipeline_model",
    "alchemist_production_document_model",
    "alchemist_legacy_graph_removed",
}
REQUIRED_ADAPTERS: set[str] = set()
REQUIRED_ADAPTER_FIELDS = {
    "adapter_id",
    "owner",
    "scope",
    "expiry_phase",
    "deletion_criteria",
    "deletion_issue",
    "tests",
    "current_state",
}


@dataclass(frozen=True)
class Violation:
    path: Path
    message: str


def authoring_model_violations(root: Path) -> list[Violation]:
    violations: list[Violation] = []
    formula = root / ALCHEMIST_PATH / "src/formula.rs"
    formula_source = formula.read_text(encoding="utf-8")
    if "pub graph: AlchemistGraphDocument" not in formula_source:
        violations.append(Violation(formula, "AlchemistFormula is not authored by AlchemistGraphDocument"))
    if re.search(r"pub\s+graph:\s*AlchemistGraph\s*,", formula_source):
        violations.append(Violation(formula, "AlchemistFormula still owns the legacy graph shape"))
    if 'serde(with = "crate::domain::document_serde")' not in formula_source:
        violations.append(Violation(formula, "typed formula graph lacks its versioned persistence codec"))
    if "GraphTransaction::for_document" not in formula_source:
        violations.append(Violation(formula, "formula overrides bypass revisioned graph transactions"))
    if "AlchemistGraphAdapter::to_legacy" in formula_source:
        violations.append(Violation(formula, "formula managed-region materialization still lowers to AlchemistGraph"))

    domain = root / ALCHEMIST_PATH / "src/domain.rs"
    domain_source = domain.read_text(encoding="utf-8")
    if "AlchemistGraphEnvelope::deserialize(deserializer)?" not in domain_source:
        violations.append(Violation(domain, "typed formula persistence does not deserialize its envelope directly"))
    for obsolete in ("AlchemistGraphAdapter", "Legacy(AlchemistGraph)", "AuthoredGraphWire"):
        if obsolete in domain_source:
            violations.append(Violation(domain, f"removed graph compatibility remains: {obsolete}"))

    graph_model = root / ALCHEMIST_PATH / "src/graph.rs"
    graph_source = graph_model.read_text(encoding="utf-8")
    if re.search(r"pub\s+struct\s+AlchemistGraph\b", graph_source):
        violations.append(Violation(graph_model, "the former AlchemistGraph model still exists"))
    if "GraphEditError" in graph_source:
        violations.append(Violation(graph_model, "the former graph edit API still exists"))

    library = root / ALCHEMIST_PATH / "src/lib.rs"
    library_source = library.read_text(encoding="utf-8")
    if "AlchemistGraphAdapter" in library_source or "pub mod serialize;" in library_source:
        violations.append(Violation(library, "the public crate still exposes removed graph compatibility"))

    for source_path in (root / "apps/chataigne").rglob("*.rs"):
        source = source_path.read_text(encoding="utf-8")
        if re.search(r"\bAlchemistGraph\b|AlchemistGraphAdapter", source):
            violations.append(Violation(source_path, "Rust source still references the removed legacy graph model"))

    compiler = root / ALCHEMIST_PATH / "src/compile.rs"
    compiler_source = compiler.read_text(encoding="utf-8")
    if not re.search(r"pub fn compile_graph\(document:\s*&AlchemistGraphDocument", compiler_source):
        violations.append(Violation(compiler, "public compiler entry point does not require the typed document"))
    if "AlchemistGraphAdapter::to_legacy" in compiler_source or "compile_legacy_graph" in compiler_source:
        violations.append(Violation(compiler, "compiler still lowers the typed document to AlchemistGraph"))
    if "graph_revision: formula.graph.revision().sequence" not in compiler_source:
        violations.append(Violation(compiler, "formula compile keys do not use the authored graph revision"))

    typing = root / ALCHEMIST_PATH / "src/typing.rs"
    typing_source = typing.read_text(encoding="utf-8")
    if "AlchemistGraphAdapter::to_legacy" in typing_source:
        violations.append(Violation(typing, "type solver still lowers the typed document to AlchemistGraph"))
    if re.search(r"pub\s+fn\s+solve_types\s*\([^)]*&AlchemistGraph", typing_source):
        violations.append(Violation(typing, "type solver still exposes the former AlchemistGraph entry point"))

    pipeline = root / ALCHEMIST_PATH / "src/pipeline.rs"
    pipeline_source = pipeline.read_text(encoding="utf-8")
    if not re.search(
        r"pub fn lower_filter_pipeline_region\(\s*graph:\s*&AlchemistGraphDocument",
        pipeline_source,
    ):
        violations.append(Violation(pipeline, "managed filter lowering does not require the typed document"))
    if "pub graph: AlchemistGraphDocument" not in pipeline_source:
        violations.append(Violation(pipeline, "managed filter lowering does not return the typed document"))
    if "AlchemistGraphTransaction::for_document" not in pipeline_source:
        violations.append(Violation(pipeline, "managed filter lowering bypasses one revisioned transaction"))
    if "AlchemistGraphAdapter" in pipeline_source:
        violations.append(Violation(pipeline, "managed filter lowering still uses the legacy graph adapter"))

    value_set_pipeline = root / "apps/chataigne/state_machine/src/value_set_pipeline.rs"
    value_set_source = value_set_pipeline.read_text(encoding="utf-8")
    if "AlchemistGraphTransaction::for_document" not in value_set_source:
        violations.append(Violation(value_set_pipeline, "ValueSet pipeline builders bypass typed transactions"))
    if "AlchemistGraphAdapter" in value_set_source:
        violations.append(Violation(value_set_pipeline, "ValueSet pipeline builders still use the legacy graph adapter"))

    app_formula = root / "apps/chataigne/src/state_machine_nodes/formula.rs"
    app_source = app_formula.read_text(encoding="utf-8")
    if "AlchemistGraphAdapter" in app_source or re.search(r"let\s+mut\s+graph\s*=\s*AlchemistGraph::new", app_source):
        violations.append(Violation(app_formula, "the live Formula subtree still materializes the legacy graph"))
    if "AlchemistGraphDomain::new_document_with_identity" not in app_source:
        violations.append(Violation(app_formula, "the live Formula subtree does not preserve typed graph identity"))
    if "AlchemistGraphTransaction::for_document" not in app_source:
        violations.append(Violation(app_formula, "the live Formula subtree bypasses one typed transaction"))

    state_machine = root / "apps/chataigne/state_machine/src/state_machine.rs"
    state_machine_source = state_machine.read_text(encoding="utf-8")
    if "guard_graph: Option<AlchemistGraphDocument>" not in state_machine_source:
        violations.append(Violation(state_machine, "transition guards do not own typed graph documents"))
    if "effect_graph: Option<AlchemistGraphDocument>" not in state_machine_source:
        violations.append(Violation(state_machine, "transition effects do not own typed graph documents"))
    if "AlchemistGraphAdapter" in state_machine_source:
        violations.append(Violation(state_machine, "transition graph compilation still uses the legacy adapter"))
    return violations


def dashboard_violations(root: Path) -> list[Violation]:
    path = root / "docs/product/manifests/phase4-cutovers.v1.json"
    document = json.loads(path.read_text(encoding="utf-8"))
    violations: list[Violation] = []
    if document.get("schema_version") != 1 or document.get("phase") != 4:
        violations.append(Violation(path, "dashboard must identify Phase 4 schema version 1"))
    if document.get("revision", 0) < 7:
        violations.append(Violation(path, "Phase 4 dashboard predates checkpoint closure"))
    if document.get("validation_state") != "CHECKPOINT_RUNNABLE":
        violations.append(Violation(path, "Phase 4 dashboard is not at a runnable checkpoint"))
    product_gate = document.get("product_gate", "")
    if not re.search(
        r"^PASS: Win-x64 local report target/product-gate/\d{8}T\d{6}Z/product-gate-report\.json",
        product_gate,
    ):
        violations.append(Violation(path, "Phase 4 dashboard lacks exact passing product-gate evidence"))
    if "32/32 required checks passed" not in product_gate:
        violations.append(Violation(path, "Phase 4 product-gate evidence is incomplete"))

    cutovers = document.get("cutovers", [])
    ids = {row.get("cutover_id") for row in cutovers if isinstance(row, dict)}
    if ids != REQUIRED_CUTOVERS:
        violations.append(Violation(path, f"cutover set differs: {sorted(ids ^ REQUIRED_CUTOVERS)}"))
    for row in cutovers:
        if not isinstance(row, dict) or not row.get("owner") or not row.get("evidence"):
            violations.append(Violation(path, "every Phase 4 cutover needs an owner and evidence"))

    adapters = document.get("temporary_adapters", [])
    adapter_ids = {row.get("adapter_id") for row in adapters if isinstance(row, dict)}
    if adapter_ids != REQUIRED_ADAPTERS:
        violations.append(Violation(path, f"adapter set differs: {sorted(adapter_ids ^ REQUIRED_ADAPTERS)}"))
    for adapter in adapters:
        if not isinstance(adapter, dict):
            violations.append(Violation(path, "adapter rows must be objects"))
            continue
        missing = REQUIRED_ADAPTER_FIELDS - adapter.keys()
        if missing:
            violations.append(Violation(path, f"adapter {adapter.get('adapter_id')!r} lacks {sorted(missing)}"))
        if not adapter.get("tests"):
            violations.append(Violation(path, f"adapter {adapter.get('adapter_id')!r} has no executable tests"))
    return violations


def check(root: Path) -> list[Violation]:
    return authoring_model_violations(root) + dashboard_violations(root)


def main() -> int:
    root = Path(__file__).resolve().parents[2]
    violations = check(root)
    if violations:
        for violation in violations:
            print(f"{violation.path.relative_to(root)}: {violation.message}", file=sys.stderr)
        return 1
    print("Phase 4 Alchemist ownership and typed graph contracts are valid.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
