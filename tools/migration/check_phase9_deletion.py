from __future__ import annotations

import argparse
import json
import sys
from collections.abc import Sequence
from pathlib import Path


RETIRED_PATHS = (
    "packages/golden-alchemist-ui",
    "submodules",
    ".gitmodules",
    "docs/implementation/chataigne_alchemist_integration_progress.md",
    "docs/repo-transition-plan.md",
)
RETIRED_PRODUCTION_TOKENS = (
    "golden.runtime.domain-node-adapter",
    "shadow_comparisons",
    "shadow_mismatches",
)
CURRENT_DOCS = ("README.md", "ARCHITECTURE.md", "CONTRIBUTING.md", "docs/architecture.md", "docs/repo-map.md")
RETIRED_DOC_TOKENS = ("packages/golden-alchemist-ui", "crates/golden_alchemist", "submodules/golden_core")


def path_has_content(path: Path) -> bool:
    return path.is_file() or path.is_dir() and any(path.rglob("*"))


def collect_violations(root: Path) -> list[str]:
    violations: list[str] = []
    for relative in RETIRED_PATHS:
        if path_has_content(root / relative):
            violations.append(f"retired migration path remains: {relative}")

    production_files = [
        *sorted((root / "crates").rglob("*.rs")),
        *sorted((root / "apps").rglob("*.rs")),
        *sorted((root / "apps").rglob("*.ts")),
        *sorted((root / "apps").rglob("*.svelte")),
        *sorted((root / "packages").rglob("*.ts")),
        *sorted((root / "packages").rglob("*.svelte")),
    ]
    for path in production_files:
        source = path.read_text(encoding="utf-8")
        for token in RETIRED_PRODUCTION_TOKENS:
            if token in source:
                violations.append(f"{path.relative_to(root).as_posix()} contains retired token {token}")
    for relative in CURRENT_DOCS:
        source = (root / relative).read_text(encoding="utf-8")
        for token in RETIRED_DOC_TOKENS:
            if token in source:
                violations.append(f"current documentation still names retired path {token}: {relative}")

    dashboard_path = root / "docs/product/manifests/phase9-qualification.v1.json"
    dashboard = json.loads(dashboard_path.read_text(encoding="utf-8"))
    if dashboard.get("carried_temporary_adapters"):
        violations.append("Phase 9 dashboard still carries temporary adapters")
    return violations


def main(arguments: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Check final Phase 9 governed deletion.")
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[2])
    options = parser.parse_args(arguments)
    violations = collect_violations(options.root.resolve())
    if violations:
        for violation in violations:
            print(f"Phase 9 deletion violation: {violation}", file=sys.stderr)
        return 1
    print("Phase 9 governed deletion contracts: PASS")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
