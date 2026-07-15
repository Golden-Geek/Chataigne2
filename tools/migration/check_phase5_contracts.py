"""Validate Phase 5 statechart, condition, context, and processor cutovers."""

from __future__ import annotations

import json
import re
import sys
from dataclasses import dataclass
from pathlib import Path


REQUIRED_CUTOVERS = {
    "statechart_graph_document",
    "statechart_ui_document",
    "compiled_condition_runtime",
    "processor_ownership",
    "context_lane_runtime",
    "action_mapping_composition",
    "legacy_statechart_condition_runtime_removed",
}


@dataclass(frozen=True)
class Violation:
    path: Path
    message: str


def implementation_violations(root: Path) -> list[Violation]:
    violations: list[Violation] = []

    workspace = root / "Cargo.toml"
    workspace_source = workspace.read_text(encoding="utf-8")
    for member in ('"crates/condition"', '"apps/chataigne/processor"'):
        if member not in workspace_source:
            violations.append(Violation(workspace, f"workspace lacks Phase 5 member {member}"))

    statechart_model = root / "crates/golden_statechart/src/model.rs"
    statechart_source = statechart_model.read_text(encoding="utf-8")
    if "document: StatechartGraphDocument" not in statechart_source:
        violations.append(Violation(statechart_model, "statechart does not own the canonical graph document"))
    if "GraphTransaction::for_document" not in statechart_source:
        violations.append(Violation(statechart_model, "statechart edits bypass graph transactions"))

    statechart_domain = root / "crates/golden_statechart/src/domain.rs"
    domain_source = statechart_domain.read_text(encoding="utf-8")
    if "StatechartGraphAdapter" in domain_source:
        violations.append(Violation(statechart_domain, "removed statechart graph adapter remains"))

    statechart_ui = root / "packages/golden-statechart-ui/statechart-document.ts"
    if not statechart_ui.is_file():
        violations.append(Violation(statechart_ui, "statechart UI document projection is missing"))
    panel = root / "apps/chataigne/ui/src/lib/state_machine/components/StateMachinePanel.svelte"
    panel_source = panel.read_text(encoding="utf-8")
    if "StatechartDocumentView" not in panel_source or "LegacyGraphDocumentAdapter" in panel_source:
        violations.append(Violation(panel, "state-machine panel is not cut over to the statechart UI document"))

    condition_authoring = root / "crates/condition/src/authoring.rs"
    authoring_source = condition_authoring.read_text(encoding="utf-8")
    for contract in ("InputValue", "InputNode", "Group", "Script"):
        if contract not in authoring_source:
            violations.append(Violation(condition_authoring, f"condition authoring lacks {contract}"))
    condition_runtime = root / "crates/condition/src/runtime.rs"
    runtime_source = condition_runtime.read_text(encoding="utf-8")
    for contract in ("CompiledConditionProgram", "ConditionInputProvider", "ConditionRuntime"):
        if contract not in runtime_source:
            violations.append(Violation(condition_runtime, f"compiled condition runtime lacks {contract}"))

    manager = root / "apps/chataigne/src/state_machine_nodes/manager.rs"
    manager_source = manager.read_text(encoding="utf-8")
    for required in ("compile_manager_condition", "evaluate_compiled_condition", "ConditionBinding"):
        if required not in manager_source:
            violations.append(Violation(manager, f"live condition manager lacks {required}"))
    for obsolete in ("fn condition_group_valid(", "fn input_value_condition_valid("):
        if obsolete in manager_source:
            violations.append(Violation(manager, f"editable-tree condition runtime remains: {obsolete}"))

    processor = root / "apps/chataigne/processor/src/processor.rs"
    processor_source = processor.read_text(encoding="utf-8")
    for contract in ("compiled_condition", "condition_runtimes", "rebuild_execution_plan"):
        if contract not in processor_source:
            violations.append(Violation(processor, f"processor runtime lacks {contract}"))
    state_machine_lib = root / "apps/chataigne/state_machine/src/lib.rs"
    if "chataigne_processor::*" not in state_machine_lib.read_text(encoding="utf-8"):
        violations.append(Violation(state_machine_lib, "state-machine facade does not use the processor package"))

    formulas = root / "apps/chataigne/builtin_formulas"
    if {path.name for path in formulas.glob("*.json")} != {"Action.json", "Mapping.json"}:
        violations.append(Violation(formulas, "the user-facing formula catalog is not exactly Action and Mapping"))
    return violations


def dashboard_violations(root: Path) -> list[Violation]:
    path = root / "docs/product/manifests/phase5-cutovers.v1.json"
    document = json.loads(path.read_text(encoding="utf-8"))
    violations: list[Violation] = []
    if document.get("schema_version") != 1 or document.get("phase") != 5:
        violations.append(Violation(path, "dashboard must identify Phase 5 schema version 1"))
    if document.get("validation_state") != "CHECKPOINT_RUNNABLE":
        violations.append(Violation(path, "Phase 5 dashboard is not at a runnable checkpoint"))
    product_gate = document.get("product_gate", "")
    if not re.search(
        r"^PASS: Win-x64 local report target/product-gate/\d{8}T\d{6}Z/product-gate-report\.json",
        product_gate,
    ):
        violations.append(Violation(path, "Phase 5 dashboard lacks exact passing product-gate evidence"))
    cutovers = document.get("cutovers", [])
    ids = {row.get("cutover_id") for row in cutovers if isinstance(row, dict)}
    if ids != REQUIRED_CUTOVERS:
        violations.append(Violation(path, f"cutover set differs: {sorted(ids ^ REQUIRED_CUTOVERS)}"))
    for row in cutovers:
        if not isinstance(row, dict) or not row.get("owner") or not row.get("evidence"):
            violations.append(Violation(path, "every Phase 5 cutover needs an owner and evidence"))
    if document.get("temporary_adapters") != []:
        violations.append(Violation(path, "Phase 5 leaves temporary adapters behind"))
    return violations


def check(root: Path) -> list[Violation]:
    return implementation_violations(root) + dashboard_violations(root)


def main() -> int:
    root = Path(__file__).resolve().parents[2]
    violations = check(root)
    if violations:
        for violation in violations:
            print(f"{violation.path.relative_to(root)}: {violation.message}", file=sys.stderr)
        return 1
    print("Phase 5 statechart, condition, context, and processor contracts are valid.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
