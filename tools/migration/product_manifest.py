#!/usr/bin/env python3
"""Generate and verify deterministic Phase 0 product manifests."""

from __future__ import annotations

# Direct execution places tools/migration on sys.path, so this resolves the
# sibling package without relying on repository installation state.
from product_manifest.cli import main


if __name__ == "__main__":
    raise SystemExit(main())
