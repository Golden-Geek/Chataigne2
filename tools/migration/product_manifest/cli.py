"""Command-line interface for Phase 0 manifest generation."""

from __future__ import annotations

import argparse
import sys
from collections.abc import Sequence
from pathlib import Path

from .common import (
    DEFAULT_OUTPUT_DIR,
    GENERATOR_PATH,
    MANAGED_FILES,
    ManifestError,
)
from .documents import (
    check_documents,
    generate_documents,
    summary,
    validate_documents,
    write_documents,
)


def main(arguments: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Generate and verify deterministic Phase 0 product manifests."
    )
    parser.add_argument("action", choices=("generate", "check", "validate"))
    parser.add_argument("--root", type=Path, default=Path.cwd())
    parser.add_argument("--output-dir", type=Path)
    options = parser.parse_args(arguments)
    root = options.root.resolve()
    output_dir = (
        options.output_dir.resolve()
        if options.output_dir is not None
        else root / DEFAULT_OUTPUT_DIR
    )
    try:
        if options.action == "validate":
            validate_documents(output_dir)
            print(f"manifest schema validation passed: {output_dir}")
            return 0
        documents = generate_documents(root)
        if options.action == "generate":
            write_documents(output_dir, documents)
            print(f"generated {len(MANAGED_FILES)} files ({summary(documents)})")
            return 0
        drift = check_documents(output_dir, documents)
        if drift:
            for message in drift:
                print(message, file=sys.stderr)
            print(
                f"manifest drift detected; run: {sys.executable} "
                f"{GENERATOR_PATH} generate",
                file=sys.stderr,
            )
            return 1
        validate_documents(output_dir)
        print(f"manifest drift check passed ({summary(documents)})")
        return 0
    except ManifestError as error:
        print(f"manifest error: {error}", file=sys.stderr)
        return 2
