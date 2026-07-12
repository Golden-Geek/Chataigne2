"""Shared constants and canonical serialization for product manifests."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any


SCHEMA_VERSION = 1
GENERATOR_VERSION = "1.0.0"
GENERATOR_PATH = "tools/migration/product_manifest.py"
DEFAULT_OUTPUT_DIR = Path("docs/product/manifests")

MANAGED_FILES = (
    "schemas/product-inventory-v1.schema.json",
    "schemas/source-revisions-v1.schema.json",
    "schemas/functional-parity-v1.schema.json",
    "product-surfaces.v1.json",
    "product-files.v1.json",
    "source-revisions.v1.json",
    "functional-parity.v1.json",
)

SURFACE_KINDS = (
    "panel",
    "command",
    "node_type",
    "anode",
    "formula",
    "module",
    "script_method",
    "script_callback",
    "script_template",
    "script_snippet",
)
FILE_KINDS = ("fixture", "asset")
CANONICAL_PERFORMANCE_FIXTURES = ("P50-L1", "P5-L127")


class ManifestError(RuntimeError):
    """Raised when generation, validation, or drift checking fails."""


def canonical_json(value: Any) -> bytes:
    """Return the sole supported on-disk JSON representation."""
    return (json.dumps(value, indent=2, sort_keys=True, ensure_ascii=False) + "\n").encode(
        "utf-8"
    )
