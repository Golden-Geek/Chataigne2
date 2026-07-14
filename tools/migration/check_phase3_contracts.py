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
}
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
    forbidden_source = re.compile(r"\b(?:golden_engine|golden_alchemist|chataigne)\b", re.IGNORECASE)
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

    alchemist_value = root / "crates/golden_alchemist/src/value.rs"
    alchemist_source = alchemist_value.read_text(encoding="utf-8")
    if "pub use golden_values" not in alchemist_source:
        violations.append(Violation(alchemist_value, "Alchemist does not consume canonical golden_values"))
    if re.search(r"pub enum RuntimeValue\b|pub struct ColorValue\b", alchemist_source):
        violations.append(Violation(alchemist_value, "Alchemist still owns canonical value definitions"))

    values = root / "crates/values/src/lib.rs"
    values_source = values.read_text(encoding="utf-8")
    if not re.search(r"pub struct ColorValue\s*\{[^}]*red:\s*f64", values_source, re.DOTALL):
        violations.append(Violation(values, "canonical color channels are not f64"))
    return violations


def dashboard_violations(root: Path) -> list[Violation]:
    path = root / "docs/product/manifests/phase3-cutovers.v1.json"
    document = json.loads(path.read_text(encoding="utf-8"))
    violations: list[Violation] = []
    if document.get("schema_version") != 1 or document.get("phase") != 3:
        violations.append(Violation(path, "dashboard must identify Phase 3 schema version 1"))

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
    return violations


def check(root: Path) -> list[Violation]:
    return foundation_violations(root) + dashboard_violations(root)


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
