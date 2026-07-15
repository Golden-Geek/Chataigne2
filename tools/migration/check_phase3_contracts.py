"""Validate Phase 3 foundation ownership and cutover records."""

from __future__ import annotations

import json
import re
import sys
import tomllib
from dataclasses import dataclass
from pathlib import Path


FOUNDATION_CRATES = {
    "model": {"serde", "ts-rs", "uuid"},
    "values": {"serde", "smol_str", "ts-rs"},
    "parameters": {"golden_model", "golden_values", "serde", "serde_json", "thiserror", "ts-rs"},
    "context": {"golden_model", "golden_parameters", "serde", "ts-rs"},
    "graph": {"indexmap", "serde", "thiserror", "ts-rs", "uuid"},
}
ALCHEMIST_PATH = Path("apps/chataigne/alchemist")
REQUIRED_CUTOVERS = {
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
CUTOVER_STATES = {"pending", "adapted", "shadowing", "cut_over", "old_path_removed"}


@dataclass(frozen=True)
class Violation:
    path: Path
    message: str


def foundation_violations(root: Path) -> list[Violation]:
    violations: list[Violation] = []
    forbidden_source = re.compile(
        r"\b(?:golden_engine|golden_alchemist|chataigne_alchemist|chataigne)\b",
        re.IGNORECASE,
    )
    for directory, allowed_dependencies in FOUNDATION_CRATES.items():
        crate = root / "crates" / directory
        manifest = crate / "Cargo.toml"
        document = tomllib.loads(manifest.read_text(encoding="utf-8"))
        dependencies = document.get("dependencies", {})
        if isinstance(dependencies, dict):
            for dependency in dependencies:
                if dependency not in allowed_dependencies:
                    violations.append(Violation(manifest, f"foundation crate depends on {dependency!r}"))
        for source in (crate / "src").rglob("*.rs"):
            if forbidden_source.search(source.read_text(encoding="utf-8")):
                violations.append(Violation(source, "foundation source imports product or higher-level policy"))

    identity = root / "crates/core/src/node/core/identity.rs"
    identity_source = identity.read_text(encoding="utf-8")
    if "pub use golden_model::{DeclId, NodeId, NodeUuid};" not in identity_source:
        violations.append(Violation(identity, "engine identity API does not re-export golden_model identities"))
    if re.search(r"pub struct (?:NodeId|NodeUuid|DeclId)\b", identity_source):
        violations.append(Violation(identity, "engine still owns a foundation identity definition"))
    if "pub use golden_parameters::NodeReference;" not in identity_source:
        violations.append(Violation(identity, "engine identity API does not re-export golden_parameters NodeReference"))
    if re.search(r"pub struct NodeReference\b", identity_source):
        violations.append(Violation(identity, "engine still owns the parameter reference definition"))

    parameter = root / "crates/core/src/parameter/mod.rs"
    parameter_source = parameter.read_text(encoding="utf-8")
    if "pub use golden_parameters::*;" not in parameter_source:
        violations.append(Violation(parameter, "engine parameter API does not consume golden_parameters"))
    moved_parameter_modules = re.compile(
        r"\bmod\s+(?:canonical|constraints|control|projection|types|value|value_type)\s*;"
    )
    if moved_parameter_modules.search(parameter_source):
        violations.append(Violation(parameter, "engine still declares a moved parameter foundation module"))

    contexts = root / "crates/core/src/contexts.rs"
    contexts_source = contexts.read_text(encoding="utf-8")
    if "pub use golden_context::*;" not in contexts_source:
        violations.append(Violation(contexts, "engine context API does not consume golden_context"))
    if re.search(r"pub struct (?:ContextRegistry|ContextSnapshot)\b", contexts_source):
        violations.append(Violation(contexts, "engine still owns a context foundation definition"))

    color = root / "crates/core/src/color.rs"
    color_source = color.read_text(encoding="utf-8")
    if "pub use golden_parameters::Color;" not in color_source:
        violations.append(Violation(color, "engine color API does not consume the canonical parameter color"))
    if re.search(r"pub struct Color\b", color_source):
        violations.append(Violation(color, "engine still owns a duplicate color definition"))

    graph = root / "crates/graph/src/lib.rs"
    graph_source = graph.read_text(encoding="utf-8")
    required_graph_contract = {
        "GraphDomain",
        "GraphDocument",
        "GraphTransaction",
        "ConnectionPolicy",
        "GraphRevision",
        "GraphChangeSet",
        "GraphPresentation",
        "GraphEnvelope",
        "stable_topological_order",
    }
    for contract in sorted(required_graph_contract):
        if contract not in graph_source:
            violations.append(Violation(graph, f"golden_graph public contract lacks {contract}"))
    facade = root / "crates/core_facade/src/lib.rs"
    if not facade.is_file() or "pub use golden_graph::*;" not in facade.read_text(encoding="utf-8"):
        violations.append(Violation(facade, "golden_core facade does not expose the live golden_graph contract"))

    alchemist_value = root / ALCHEMIST_PATH / "src/value.rs"
    alchemist_source = alchemist_value.read_text(encoding="utf-8")
    if "pub use golden_values" not in alchemist_source:
        violations.append(Violation(alchemist_value, "Alchemist does not consume canonical golden_values"))
    if re.search(r"pub enum RuntimeValue\b|pub struct ColorValue\b", alchemist_source):
        violations.append(Violation(alchemist_value, "Alchemist still owns canonical value definitions"))

    alchemist_manifest = root / ALCHEMIST_PATH / "Cargo.toml"
    alchemist_dependencies = tomllib.loads(alchemist_manifest.read_text(encoding="utf-8")).get(
        "dependencies", {}
    )
    if not isinstance(alchemist_dependencies, dict) or "golden_graph" not in alchemist_dependencies:
        violations.append(Violation(alchemist_manifest, "Alchemist does not consume golden_graph"))
    alchemist_domain = root / ALCHEMIST_PATH / "src/domain.rs"
    alchemist_domain_source = alchemist_domain.read_text(encoding="utf-8")
    for contract in ("AlchemistGraphDomain", "AlchemistGraphDocument"):
        if contract not in alchemist_domain_source:
            violations.append(Violation(alchemist_domain, f"Alchemist graph adaptation lacks {contract}"))
    if "AlchemistGraphAdapter" in alchemist_domain_source:
        violations.append(Violation(alchemist_domain, "expired Phase 3 Alchemist adapter still exists"))
    alchemist_lib = root / ALCHEMIST_PATH / "src/lib.rs"
    alchemist_lib_source = alchemist_lib.read_text(encoding="utf-8")
    if "pub mod domain;" not in alchemist_lib_source or "AlchemistGraphDomain" not in alchemist_lib_source:
        violations.append(Violation(alchemist_lib, "Alchemist graph domain is not part of the public package API"))

    statechart_manifest = root / "crates/golden_statechart/Cargo.toml"
    statechart_dependencies = tomllib.loads(statechart_manifest.read_text(encoding="utf-8")).get(
        "dependencies", {}
    )
    if not isinstance(statechart_dependencies, dict) or "golden_graph" not in statechart_dependencies:
        violations.append(Violation(statechart_manifest, "statecharts do not consume golden_graph"))
    statechart_domain = root / "crates/golden_statechart/src/domain.rs"
    statechart_domain_source = statechart_domain.read_text(encoding="utf-8")
    for contract in ("StatechartGraphDomain", "StatechartGraphDocument"):
        if contract not in statechart_domain_source:
            violations.append(Violation(statechart_domain, f"statechart graph adaptation lacks {contract}"))
    statechart_lib = root / "crates/golden_statechart/src/lib.rs"
    statechart_lib_source = statechart_lib.read_text(encoding="utf-8")
    if "mod domain;" not in statechart_lib_source or "StatechartGraphDomain" not in statechart_lib_source:
        violations.append(Violation(statechart_lib, "statechart graph domain is not part of the public package API"))

    values = root / "crates/values/src/lib.rs"
    values_source = values.read_text(encoding="utf-8")
    if not re.search(r"pub struct ColorValue\s*\{[^}]*red:\s*f64", values_source, re.DOTALL):
        violations.append(Violation(values, "canonical color channels are not f64"))
    return violations


def runtime_value_boundary_violations(root: Path) -> list[Violation]:
    violations: list[Violation] = []
    values = root / "crates/values/src/lib.rs"
    values_source = values.read_text(encoding="utf-8")
    if re.search(r"pub\s+type\s+RuntimeValue\b", values_source):
        violations.append(Violation(values, "golden_values still exposes the retired RuntimeValue alias"))

    alchemist_lib = root / ALCHEMIST_PATH / "src/lib.rs"
    alchemist_source = alchemist_lib.read_text(encoding="utf-8")
    public_value_exports = re.search(r"pub\s+use\s+value::\{(?P<body>.*?)\};", alchemist_source, re.DOTALL)
    if public_value_exports and re.search(r"\bRuntimeValue\b", public_value_exports.group("body")):
        violations.append(Violation(alchemist_lib, "Chataigne Alchemist still publicly re-exports RuntimeValue"))

    public_import = re.compile(
        r"chataigne_alchemist::RuntimeValue|use\s+chataigne_alchemist::\{[^;]*\bRuntimeValue\b[^;]*\};"
    )
    for source_root in (root / "crates", root / "apps"):
        if not source_root.is_dir():
            continue
        for source in source_root.rglob("*.rs"):
            if source.is_relative_to(root / ALCHEMIST_PATH):
                continue
            if public_import.search(source.read_text(encoding="utf-8")):
                violations.append(Violation(source, "consumer imports the retired Alchemist RuntimeValue API"))

    for manifest in (root / "apps/chataigne/Cargo.toml", root / "apps/chataigne/state_machine/Cargo.toml"):
        dependencies = tomllib.loads(manifest.read_text(encoding="utf-8")).get("dependencies", {})
        if not isinstance(dependencies, dict) or "golden_values" not in dependencies:
            violations.append(Violation(manifest, "Alchemist product consumer lacks a direct golden_values dependency"))
    return violations


def graph_ui_violations(root: Path) -> list[Violation]:
    violations: list[Violation] = []
    package = root / "packages/golden-graph-ui"
    manifest = package / "package.json"
    document = json.loads(manifest.read_text(encoding="utf-8"))
    if document.get("name") != "golden_graph_ui":
        violations.append(Violation(manifest, "generic graph UI package has the wrong public name"))

    canvas = package / "components/GraphCanvas.svelte"
    canvas_source = canvas.read_text(encoding="utf-8")
    for contract in ("GraphPresentationDocument", "SpatialIndex", "topologyRevision", "presentationRevision"):
        if contract not in canvas_source:
            violations.append(Violation(canvas, f"generic graph canvas lacks {contract}"))
    forbidden_import = re.compile(r"from\s+['\"](?:golden_alchemist_ui|.*chataigne.*)['\"]", re.IGNORECASE)
    for source in [*package.rglob("*.ts"), *package.rglob("*.svelte")]:
        if forbidden_import.search(source.read_text(encoding="utf-8")):
            violations.append(Violation(source, "generic graph UI imports domain or product policy"))

    old_canvas = root / "packages/golden-alchemist-ui/components/GraphCanvas.svelte"
    if old_canvas.exists():
        violations.append(Violation(old_canvas, "Alchemist package still owns the generic graph canvas"))
    alchemist_index = root / "packages/golden-alchemist-ui/index.ts"
    if alchemist_index.is_file() and "GraphCanvas" in alchemist_index.read_text(encoding="utf-8"):
        violations.append(Violation(alchemist_index, "Alchemist package still exports the generic graph canvas"))

    generated_revision = package / "generated/GraphRevision.ts"
    if "export type GraphRevision" not in generated_revision.read_text(encoding="utf-8"):
        violations.append(Violation(generated_revision, "Rust-owned GraphRevision binding is missing"))
    graph_types = package / "types.ts"
    if "./generated/GraphRevision" not in graph_types.read_text(encoding="utf-8"):
        violations.append(Violation(graph_types, "graph presentation document duplicates GraphRevision"))

    app_manifest = root / "apps/chataigne/ui/package.json"
    app_dependencies = json.loads(app_manifest.read_text(encoding="utf-8")).get("dependencies", {})
    if not isinstance(app_dependencies, dict) or "golden_graph_ui" not in app_dependencies:
        violations.append(Violation(app_manifest, "Chataigne UI does not consume golden_graph_ui"))
    adapter = root / "apps/chataigne/ui/src/lib/graph/legacyGraphDocumentAdapter.ts"
    adapter_source = adapter.read_text(encoding="utf-8")
    adapter_contract = adapter_source.lower()
    if (
        "LegacyGraphDocumentAdapter" not in adapter_source
        or "pure" not in adapter_contract
        or "authority" not in adapter_contract
    ):
        violations.append(Violation(adapter, "legacy graph UI adapter is not explicitly bounded and pure"))
    return violations


def dashboard_violations(root: Path) -> list[Violation]:
    path = root / "docs/product/manifests/phase3-cutovers.v1.json"
    document = json.loads(path.read_text(encoding="utf-8"))
    violations: list[Violation] = []
    if document.get("schema_version") != 1 or document.get("phase") != 3:
        violations.append(Violation(path, "dashboard must identify Phase 3 schema version 1"))

    qualification = document.get("cross_platform_product_gate")
    expected_platforms = {"windows": "PASS", "macos": "PASS", "linux": "PASS"}
    if not isinstance(qualification, dict) or qualification.get("status") != "PASS":
        violations.append(Violation(path, "Phase 3 lacks passing cross-platform qualification"))
    else:
        if qualification.get("tested_commit") != document.get("tested_tree_base"):
            violations.append(Violation(path, "cross-platform qualification does not match the tested tree"))
        if qualification.get("platforms") != expected_platforms:
            violations.append(Violation(path, "cross-platform qualification must pass Windows, macOS, and Linux"))
        if not qualification.get("run_url") or not qualification.get("completed_at"):
            violations.append(Violation(path, "cross-platform qualification lacks run provenance"))

    cutovers = document.get("cutovers", [])
    ids = {cutover.get("cutover_id") for cutover in cutovers if isinstance(cutover, dict)}
    if ids != REQUIRED_CUTOVERS:
        violations.append(Violation(path, f"cutover set differs: {sorted(ids ^ REQUIRED_CUTOVERS)}"))
    for cutover in cutovers:
        if not isinstance(cutover, dict):
            violations.append(Violation(path, "cutover rows must be objects"))
            continue
        if cutover.get("state") not in CUTOVER_STATES:
            violations.append(Violation(path, f"invalid state for {cutover.get('cutover_id')!r}"))
        if not cutover.get("owner") or not cutover.get("evidence"):
            violations.append(Violation(path, f"cutover {cutover.get('cutover_id')!r} lacks owner/evidence"))

    for adapter in document.get("temporary_adapters", []):
        if not isinstance(adapter, dict):
            violations.append(Violation(path, "adapter rows must be objects"))
            continue
        missing = REQUIRED_ADAPTER_FIELDS - adapter.keys()
        if missing:
            violations.append(Violation(path, f"adapter {adapter.get('adapter_id')!r} lacks {sorted(missing)}"))
        if not adapter.get("tests"):
            violations.append(Violation(path, f"adapter {adapter.get('adapter_id')!r} has no executable tests"))
    for adapter in document.get("retired_adapters", []):
        if not isinstance(adapter, dict):
            violations.append(Violation(path, "retired adapter rows must be objects"))
            continue
        missing = REQUIRED_ADAPTER_FIELDS - adapter.keys()
        if missing:
            violations.append(Violation(path, f"retired adapter {adapter.get('adapter_id')!r} lacks {sorted(missing)}"))
        if adapter.get("current_state") != "old_path_removed":
            violations.append(Violation(path, f"retired adapter {adapter.get('adapter_id')!r} is not removed"))
        if not adapter.get("deletion_evidence"):
            violations.append(Violation(path, f"retired adapter {adapter.get('adapter_id')!r} lacks deletion evidence"))
    return violations


def check(root: Path) -> list[Violation]:
    return (
        foundation_violations(root)
        + runtime_value_boundary_violations(root)
        + graph_ui_violations(root)
        + dashboard_violations(root)
    )


def main() -> int:
    root = Path(__file__).resolve().parents[2]
    violations = check(root)
    if violations:
        for violation in violations:
            print(f"{violation.path.relative_to(root)}: {violation.message}", file=sys.stderr)
        return 1
    print("Phase 3 foundation ownership and cutover records are valid.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
