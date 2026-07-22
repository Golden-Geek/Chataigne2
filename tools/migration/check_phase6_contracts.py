"""Validate the declared Phase 6 production runtime-center contracts."""

from __future__ import annotations

import json
import sys
from dataclasses import dataclass
from pathlib import Path


REQUIRED_CUTOVERS = {
    "actor_control_plane",
    "immutable_runtime_generation",
    "asynchronous_incremental_compiler",
    "dense_input_and_state_plane",
    "persistent_sparse_dense_scheduler",
    "deterministic_effect_commit",
    "safe_shadow_effect_suppression",
    "production_runtime_cutover",
    "runtime_metrics_ui",
}

REQUIRED_TEMPORARY_ADAPTER_FIELDS = {
    "adapter_id",
    "owner",
    "scope",
    "authoritative_path",
    "introduced_phase",
    "expiry_phase",
    "deletion_criteria",
    "deletion_issue",
    "tests",
    "side_effect_policy",
    "current_state",
    "removed_in",
}


@dataclass(frozen=True)
class Violation:
    path: Path
    message: str


def implementation_violations(root: Path) -> list[Violation]:
    violations: list[Violation] = []
    workspace = root / "Cargo.toml"
    workspace_source = workspace.read_text(encoding="utf-8")
    if '"crates/runtime"' not in workspace_source or 'golden_runtime = { path = "crates/runtime" }' not in workspace_source:
        violations.append(Violation(workspace, "workspace lacks the reusable golden_runtime member"))

    runtime = root / "crates/runtime/src"
    contracts = {
        "control.rs": ("ControlActor", "ControlHandle", "ControlStatus", "RuntimeMetrics"),
        "compiler.rs": ("CompilationService", "GenerationCompiler", "RuntimeChangeSet", "previous"),
        "generation.rs": ("RuntimeGeneration", "SemanticRuntime", "swap_generation", "StableStateBinding"),
        "input.rs": ("RuntimeInputHandle", "InputDelivery", "LosslessOrdered", "routes_for"),
        "scheduler.rs": ("PersistentBatchScheduler", "ExecutionMode", "Sparse", "Dense", "execute_into", "BatchScratch"),
        "effects.rs": ("EffectCommitMode", "ShadowSuppressed", "EffectBuffer", "EffectSink"),
        "metrics.rs": ("RuntimeMetricsSnapshot", "control_queue_depth", "effects_suppressed"),
    }
    for relative, required in contracts.items():
        path = runtime / relative
        if not path.is_file():
            violations.append(Violation(path, "Phase 6 runtime contract file is missing"))
            continue
        source = path.read_text(encoding="utf-8")
        for contract in required:
            if contract not in source:
                violations.append(Violation(path, f"Phase 6 runtime contract lacks {contract}"))

    application = root / "crates/core/src/application.rs"
    application_source = application.read_text(encoding="utf-8")
    if "control: ControlActor<ProductionState<T>>" not in application_source:
        violations.append(Violation(application, "ProductionRuntime is not actor-owned"))
    if "Mutex<Engine<T>>" in application_source or "lock_engine" in application_source:
        violations.append(Violation(application, "shared production engine locking remains"))
    if "runtime_metrics" not in application_source:
        violations.append(Violation(application, "production runtime metrics are not exposed"))
    if "input_port.publish" not in application_source:
        violations.append(Violation(application, "production module inputs bypass the dense input plane"))

    runtime_center = root / "crates/core/src/runtime_center.rs"
    runtime_center_source = runtime_center.read_text(encoding="utf-8")
    for contract in (
        "ProductionState",
        "CompilationService",
        "RuntimeInputMailbox",
        "PersistentBatchScheduler",
        "scheduled_node_to_work",
        "run_tick_with_compiled_schedule",
        "scheduler_outputs",
        "has no compiled kernel identity",
        "apply_dense_inputs",
        "recompile_blocking",
    ):
        if contract not in runtime_center_source:
            violations.append(Violation(runtime_center, f"production runtime center lacks {contract}"))
    if ".engine.run_tick(elapsed)" in runtime_center_source:
        violations.append(Violation(runtime_center, "production runtime still calls the rollback tick entry point"))

    engine_tick = root / "crates/core/src/engine/runtime/tick.rs"
    engine_tick_source = engine_tick.read_text(encoding="utf-8")
    if "run_tick_with_compiled_schedule" not in engine_tick_source:
        violations.append(Violation(engine_tick, "Engine domain arena lacks the compiled-schedule entry point"))
    scheduled_updates = root / "crates/core/src/engine/runtime/scheduled_updates.rs"
    scheduled_updates_source = scheduled_updates.read_text(encoding="utf-8")
    for contract in ("order_due_nodes", "ordered_due_nodes", "compiled_order_is_complete"):
        if contract not in scheduled_updates_source:
            violations.append(Violation(scheduled_updates, f"scheduled update cutover lacks {contract}"))

    ui_stats = root / "crates/core/src/ui_sync.rs"
    ui_stats_source = ui_stats.read_text(encoding="utf-8")
    for metric in (
        "generation_id",
        "control_queue_depth",
        "sparse_batches",
        "effects_suppressed",
    ):
        if metric not in ui_stats_source:
            violations.append(Violation(ui_stats, f"UI runtime metrics lack {metric}"))
    header = root / "packages/golden-ui/components/app/AppHeader.svelte"
    header_source = header.read_text(encoding="utf-8")
    if "engineRateDetail" not in header_source or "control_queue_depth" not in header_source:
        violations.append(Violation(header, "existing performance UI does not surface Phase 6 metrics"))

    generated = root / "packages/golden-ui/generated/rust_protocol/UiRuntimeStatsDto.ts"
    generated_source = generated.read_text(encoding="utf-8")
    if "effects_suppressed" not in generated_source or "control_queue_depth" not in generated_source:
        violations.append(Violation(generated, "generated UI protocol lacks Phase 6 runtime metrics"))
    for removed_metric in ("shadow_comparisons", "shadow_mismatches"):
        if removed_metric in generated_source or removed_metric in ui_stats_source:
            violations.append(Violation(generated, f"retired runtime metric remains: {removed_metric}"))
    if "golden.runtime.domain-node-adapter" in runtime_center_source or "compare_shadow_results" in runtime_center_source:
        violations.append(Violation(runtime_center, "retired Phase 6 domain adapter or shadow comparison remains"))
    return violations


def dashboard_violations(root: Path) -> list[Violation]:
    path = root / "docs/product/manifests/phase6-cutovers.v1.json"
    document = json.loads(path.read_text(encoding="utf-8"))
    violations: list[Violation] = []
    if document.get("schema_version") != 1 or document.get("phase") != 6:
        violations.append(Violation(path, "dashboard must identify Phase 6 schema version 1"))
    if document.get("validation_state") not in {"CONSTRUCTION", "CHECKPOINT_RUNNABLE"}:
        violations.append(Violation(path, "Phase 6 dashboard has an invalid validation state"))
    cutovers = document.get("cutovers", [])
    ids = {row.get("cutover_id") for row in cutovers if isinstance(row, dict)}
    if ids != REQUIRED_CUTOVERS:
        violations.append(Violation(path, f"cutover set differs: {sorted(ids ^ REQUIRED_CUTOVERS)}"))
    for row in cutovers:
        if not isinstance(row, dict) or not row.get("owner") or not row.get("evidence") or not row.get("state"):
            violations.append(Violation(path, "every Phase 6 cutover needs state, owner, and evidence"))

    adapters = document.get("temporary_adapters", [])
    for adapter in adapters:
        if not isinstance(adapter, dict):
            violations.append(Violation(path, "temporary adapter rows must be objects"))
            continue
        missing = REQUIRED_TEMPORARY_ADAPTER_FIELDS - set(adapter)
        if missing:
            violations.append(Violation(path, f"temporary adapter lacks fields: {sorted(missing)}"))
        if adapter.get("current_state") == "active" and not adapter.get("tests"):
            violations.append(Violation(path, "active temporary adapters need executable tests"))

    if document.get("validation_state") == "CHECKPOINT_RUNNABLE":
        if not str(document.get("product_gate", "")).startswith("PASS:"):
            violations.append(Violation(path, "runnable Phase 6 dashboard lacks product-gate evidence"))
        if not str(document.get("cross_platform_gate", "")).startswith("PASS:"):
            violations.append(Violation(path, "runnable Phase 6 dashboard lacks cross-platform evidence"))
        unfinished = [row.get("cutover_id") for row in cutovers if row.get("state") != "cut_over"]
        if unfinished:
            violations.append(Violation(path, f"runnable Phase 6 dashboard has unfinished cutovers: {unfinished}"))
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
    print("Phase 6 production runtime-center contracts are valid.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
