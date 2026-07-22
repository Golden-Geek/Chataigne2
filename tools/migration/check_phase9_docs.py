from __future__ import annotations

import argparse
import re
import sys
from collections.abc import Sequence
from pathlib import Path


REQUIRED_DOCS = (
    "ARCHITECTURE.md",
    "CONTRIBUTING.md",
    "docs/repo-map.md",
    "docs/module-authoring.md",
    "docs/ui-extension.md",
    "docs/performance.md",
    "docs/troubleshooting.md",
    "docs/release-readiness.md",
)
ROOT_LINKS = (
    "ARCHITECTURE.md",
    "docs/contributor-map.md",
    "docs/development.md",
    "docs/module-authoring.md",
    "docs/ui-extension.md",
    "docs/performance.md",
    "docs/troubleshooting.md",
)
MARKDOWN_LINK = re.compile(r"\[[^\]]+\]\(([^)]+)\)")


def collect_violations(root: Path) -> list[str]:
    violations: list[str] = []
    readme = (root / "README.md").read_text(encoding="utf-8")
    for relative in ROOT_LINKS:
        if f"({relative})" not in readme:
            violations.append(f"README does not link {relative}")
    for relative in REQUIRED_DOCS:
        path = root / relative
        if not path.is_file() or len(path.read_text(encoding="utf-8").splitlines()) < 8:
            violations.append(f"required final document is missing or empty: {relative}")
            continue
        source = path.read_text(encoding="utf-8")
        for target in MARKDOWN_LINK.findall(source):
            target = target.split("#", 1)[0]
            if not target or "://" in target or target.startswith("mailto:"):
                continue
            resolved = (path.parent / target).resolve()
            if not resolved.exists():
                violations.append(f"broken local link in {relative}: {target}")
    return violations


def main(arguments: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Check final Phase 9 contributor documentation.")
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[2])
    options = parser.parse_args(arguments)
    violations = collect_violations(options.root.resolve())
    if violations:
        for violation in violations:
            print(f"Phase 9 documentation violation: {violation}", file=sys.stderr)
        return 1
    print("Phase 9 final documentation contracts: PASS")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
