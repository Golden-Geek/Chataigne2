"""Validate Phase 2 application seams and reusable-package boundaries."""

from __future__ import annotations

import json
import re
import sys
import tomllib
from dataclasses import dataclass
from pathlib import Path


REQUIRED_SEAMS = {
    "project_transactions",
    "graph_editing",
    "runtime_values",
    "observation",
    "module_io",
    "persistence",
    "host_lifecycle",
    "shadow_execution",
    "io_recording",
}
REQUIRED_ADAPTER_FIELDS = {
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
MIGRATION_STATES = {"baseline", "adapted", "shadowing", "cut_over", "old_path_removed"}


@dataclass(frozen=True)
class Violation:
    path: Path
    message: str


def _dependency_tables(document: dict[str, object]) -> list[dict[str, object]]:
    tables: list[dict[str, object]] = []
    for key in ("dependencies", "dev-dependencies", "build-dependencies"):
        value = document.get(key)
        if isinstance(value, dict):
            tables.append(value)
    targets = document.get("target")
    if isinstance(targets, dict):
        for target in targets.values():
            if isinstance(target, dict):
                tables.extend(_dependency_tables(target))
    return tables


def reusable_package_violations(root: Path) -> list[Violation]:
    violations: list[Violation] = []
    app_root = (root / "apps" / "chataigne").resolve()
    for manifest in sorted((root / "crates").glob("*/Cargo.toml")):
        document = tomllib.loads(manifest.read_text(encoding="utf-8"))
        for table in _dependency_tables(document):
            for name, specification in table.items():
                if str(name).lower().startswith("chataigne"):
                    violations.append(Violation(manifest, f"reusable crate depends on app crate {name!r}"))
                if isinstance(specification, dict) and isinstance(specification.get("path"), str):
                    dependency_path = (manifest.parent / specification["path"]).resolve()
                    if dependency_path == app_root or app_root in dependency_path.parents:
                        violations.append(Violation(manifest, f"path dependency enters {app_root}"))

    import_pattern = re.compile(
        r"(?:from\s+|import\s*(?:\([^)]*\)\s*from\s*)?)[\"'][^\"']*(?:apps/chataigne|@chataigne)[^\"']*[\"']"
    )
    for package in sorted((root / "packages").glob("golden-*")):
        for source in package.rglob("*"):
            if source.suffix not in {".ts", ".js", ".svelte"} or not source.is_file():
                continue
            text = source.read_text(encoding="utf-8")
            if import_pattern.search(text.replace("\\", "/")):
                violations.append(Violation(source, "reusable UI package imports app-owned code"))
    return violations


def runtime_boundary_violations(root: Path) -> list[Violation]:
    violations: list[Violation] = []
    raw_engine_lock = re.compile(r"(?:Arc\s*<\s*)?Mutex\s*<\s*Engine\s*<")
    for relative in ("crates/transport_server/src", "crates/host_desktop/src"):
        for source in (root / relative).rglob("*.rs"):
            if raw_engine_lock.search(source.read_text(encoding="utf-8")):
                violations.append(Violation(source, "host or transport owns a shared Engine mutex"))
    return violations


def dashboard_violations(root: Path) -> list[Violation]:
    path = root / "docs/product/manifests/phase2-seams.v1.json"
    document = json.loads(path.read_text(encoding="utf-8"))
    violations: list[Violation] = []
    if document.get("schema_version") != 1 or document.get("manifest_kind") != "phase2_application_seams":
        violations.append(Violation(path, "unsupported dashboard identity"))
    rows = document.get("rows")
    if not isinstance(rows, list):
        return [Violation(path, "rows must be a list")]
    seam_ids = {row.get("seam_id") for row in rows if isinstance(row, dict)}
    if seam_ids != REQUIRED_SEAMS:
        violations.append(Violation(path, f"seam set mismatch: expected {sorted(REQUIRED_SEAMS)}, got {sorted(seam_ids)}"))
    for row in rows:
        if not isinstance(row, dict):
            violations.append(Violation(path, "every row must be an object"))
            continue
        if row.get("migration_state") not in MIGRATION_STATES:
            violations.append(Violation(path, f"invalid migration state for {row.get('seam_id')}"))
        if not row.get("authoritative_path") or not row.get("facade_path") or not row.get("evidence"):
            violations.append(Violation(path, f"incomplete seam row {row.get('seam_id')}"))
    adapters = document.get("temporary_adapters")
    if not isinstance(adapters, list):
        return violations + [Violation(path, "temporary_adapters must be a list")]
    for adapter in adapters:
        if not isinstance(adapter, dict):
            violations.append(Violation(path, "every adapter must be an object"))
            continue
        missing = REQUIRED_ADAPTER_FIELDS - adapter.keys()
        if missing:
            violations.append(Violation(path, f"adapter {adapter.get('adapter_id')} misses {sorted(missing)}"))
        if not isinstance(adapter.get("tests"), list) or not adapter.get("tests"):
            violations.append(Violation(path, f"adapter {adapter.get('adapter_id')} has no executable tests"))
    return violations


def find_violations(root: Path) -> list[Violation]:
    return reusable_package_violations(root) + runtime_boundary_violations(root) + dashboard_violations(root)


def main() -> int:
    root = Path(__file__).resolve().parents[2]
    violations = find_violations(root)
    if violations:
        for violation in violations:
            print(f"{violation.path.relative_to(root)}: {violation.message}")
        return 1
    print("Phase 2 application seams and package boundaries are valid.")
    return 0


if __name__ == "__main__":
    sys.exit(main())

