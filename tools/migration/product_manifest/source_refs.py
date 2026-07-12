"""Load the exact Phase 0 source revisions imported into the monorepo."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

from .common import ManifestError

SOURCE_IMPORTS_PATH = Path("docs/product/source-imports.v1.json")
REQUIRED_ENTRY_KEYS = {"path", "gitlink", "parent", "url", "branch"}


def scan_source_revisions(root: Path) -> list[dict[str, Any]]:
    """Return the reviewed import revisions, independent of working-tree Git state."""
    path = root / SOURCE_IMPORTS_PATH
    try:
        document = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ManifestError(f"unable to load source import ledger {path}: {error}") from error

    entries = document.get("entries") if isinstance(document, dict) else None
    if not isinstance(entries, list) or not entries:
        raise ManifestError(f"source import ledger {path} must contain non-empty entries")

    normalized: list[dict[str, Any]] = []
    for entry in entries:
        if not isinstance(entry, dict) or set(entry) != REQUIRED_ENTRY_KEYS:
            raise ManifestError(
                f"source import ledger entries must contain exactly {sorted(REQUIRED_ENTRY_KEYS)}"
            )
        normalized.append(dict(entry))
    return sorted(normalized, key=lambda entry: entry["path"])
