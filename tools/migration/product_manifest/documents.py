"""Assemble, validate, write, and drift-check manifest documents."""

from __future__ import annotations

import json
import re
from collections.abc import Mapping
from pathlib import Path
from typing import Any

from .common import (
    CANONICAL_PERFORMANCE_FIXTURES,
    FILE_KINDS,
    GENERATOR_PATH,
    GENERATOR_VERSION,
    MANAGED_FILES,
    SCHEMA_VERSION,
    SURFACE_KINDS,
    ManifestError,
    canonical_json,
)
from .discovery import scan_product
from .schemas import (
    inventory_schema,
    parity_schema,
    source_revisions_schema,
    validate_schema,
)
from .source_refs import scan_submodule_refs


PRODUCT_AREAS = {
    "panel": "workbench",
    "command": "command",
    "node_type": "project_model",
    "anode": "formula",
    "formula": "formula",
    "module": "module",
    "script_method": "script",
    "script_callback": "script",
    "script_template": "script",
    "script_snippet": "script",
    "fixture": "fixture",
    "asset": "ux_asset",
    "submodule_ref": "source_revision",
}


def _parity_row(entry: Mapping[str, Any]) -> dict[str, Any]:
    capability_id = str(entry["id"])
    kind = str(entry["kind"])
    sources = entry.get("sources")
    if not isinstance(sources, list):
        sources = [{"path": entry["path"], "line": 1}]
    facts = dict(entry.get("facts", {}))
    facts.update(
        {
            "inventory_kind": kind,
            "inventory_name": entry.get("name", entry.get("path")),
            "discovery_is_behavioral_proof": False,
        }
    )
    return {
        "capability_id": capability_id,
        "product_area": PRODUCT_AREAS[kind],
        "baseline_sources": sources,
        "discovered_facts": facts,
        "user_workflow": {
            "status": "pending_characterization",
            "steps": [],
            "expected_feedback": None,
            "placeholder": f"Record the exact baseline user workflow for {capability_id}.",
        },
        "runtime_semantics": {
            "status": "pending_characterization",
            "inputs": [],
            "outputs": [],
            "state": None,
            "ordering": None,
            "timing": None,
            "errors": None,
            "recovery": None,
            "placeholder": (
                f"Characterize runtime semantics for {capability_id}; "
                "source discovery is insufficient evidence."
            ),
        },
        "final_owner": {"status": "pending", "path": None},
        "evidence": {
            "id": f"evidence/{capability_id}",
            "status": "pending",
            "kind": "unassigned",
            "artifact_id": None,
        },
        "last_passing_result": None,
        "manual_evidence": {"required": None, "status": "pending"},
        "migration_state": "baseline",
        "verification_state": "pending",
        "temporary_adapter": None,
        "approval": "not_requested",
    }


def _normalized_fixture_name(value: str) -> str:
    return re.sub(r"[^a-z0-9]+", "", value.casefold())


def _fixture_requirements(file_entries: list[dict[str, Any]]) -> list[dict[str, Any]]:
    fixture_paths = [
        entry["path"] for entry in file_entries if entry["kind"] == "fixture"
    ]
    requirements: list[dict[str, Any]] = []
    for fixture_id in CANONICAL_PERFORMANCE_FIXTURES:
        needle = _normalized_fixture_name(fixture_id)
        matches = sorted(
            path
            for path in fixture_paths
            if needle in _normalized_fixture_name(path)
        )
        requirements.append(
            {
                "id": fixture_id,
                "status": "present" if matches else "absent",
                "matches": matches,
            }
        )
    return requirements


def _validate_document_set(
    surfaces: Mapping[str, Any],
    files: Mapping[str, Any],
    source_revisions: Mapping[str, Any],
    parity: Mapping[str, Any],
) -> None:
    inventory_ids = [
        *(entry["id"] for entry in surfaces["entries"]),
        *(entry["id"] for entry in files["entries"]),
        *(f"submodule_ref/{entry['path']}" for entry in source_revisions["entries"]),
    ]
    if len(inventory_ids) != len(set(inventory_ids)):
        raise ManifestError("inventory capability IDs are not unique")
    row_ids = [row["capability_id"] for row in parity["rows"]]
    if len(row_ids) != len(set(row_ids)):
        raise ManifestError("parity capability IDs are not unique")
    if set(row_ids) != set(inventory_ids):
        missing = sorted(set(inventory_ids) - set(row_ids))
        stale = sorted(set(row_ids) - set(inventory_ids))
        raise ManifestError(
            f"parity rows do not match inventory; "
            f"missing={missing[:5]}, stale={stale[:5]}"
        )
    evidence_ids = [row["evidence"]["id"] for row in parity["rows"]]
    if len(evidence_ids) != len(set(evidence_ids)):
        raise ManifestError("parity evidence IDs are not unique")
    for document, kinds in ((surfaces, SURFACE_KINDS), (files, FILE_KINDS)):
        expected = {
            kind: sum(entry["kind"] == kind for entry in document["entries"])
            for kind in kinds
        }
        if document["category_counts"] != expected:
            raise ManifestError(
                f"{document['manifest_kind']} category_counts do not match entries"
            )
    expected_requirements = _fixture_requirements(files["entries"])
    if files.get("fixture_requirements") != expected_requirements:
        raise ManifestError("product_files fixture requirements do not match entries")
    if "fixture_requirements" in surfaces:
        raise ManifestError("product_surfaces must not contain fixture requirements")


def generate_documents(root: Path) -> dict[str, Any]:
    root = root.resolve()
    surface_entries, file_entries = scan_product(root)
    revisions = scan_submodule_refs(root)
    generator = {"path": GENERATOR_PATH, "version": GENERATOR_VERSION}
    surfaces = {
        "schema_version": SCHEMA_VERSION,
        "manifest_kind": "product_surfaces",
        "generated_by": generator,
        "category_counts": {
            kind: sum(entry["kind"] == kind for entry in surface_entries)
            for kind in SURFACE_KINDS
        },
        "entries": surface_entries,
    }
    files = {
        "schema_version": SCHEMA_VERSION,
        "manifest_kind": "product_files",
        "generated_by": generator,
        "category_counts": {
            kind: sum(entry["kind"] == kind for entry in file_entries)
            for kind in FILE_KINDS
        },
        "fixture_requirements": _fixture_requirements(file_entries),
        "entries": file_entries,
    }
    source_revisions = {
        "schema_version": SCHEMA_VERSION,
        "manifest_kind": "source_revisions",
        "generated_by": generator,
        "entries": revisions,
    }
    revision_inventory = [
        {
            "id": f"submodule_ref/{entry['path']}",
            "kind": "submodule_ref",
            "name": entry["path"],
            "path": entry["path"],
            "facts": {key: value for key, value in entry.items() if key != "path"},
        }
        for entry in revisions
    ]
    rows = [
        _parity_row(entry)
        for entry in [*surface_entries, *file_entries, *revision_inventory]
    ]
    rows.sort(key=lambda row: row["capability_id"])
    parity = {
        "schema_version": SCHEMA_VERSION,
        "manifest_kind": "functional_parity",
        "generated_by": generator,
        "rows": rows,
    }
    product_schema = inventory_schema()
    revisions_schema = source_revisions_schema()
    functional_parity_schema = parity_schema()
    validate_schema(surfaces, product_schema)
    validate_schema(files, product_schema)
    validate_schema(source_revisions, revisions_schema)
    validate_schema(parity, functional_parity_schema)
    _validate_document_set(surfaces, files, source_revisions, parity)
    missing_surface_kinds = [
        kind for kind, count in surfaces["category_counts"].items() if count == 0
    ]
    if missing_surface_kinds:
        raise ManifestError(
            "required product surface categories are empty: "
            + ", ".join(missing_surface_kinds)
        )
    return {
        "schemas/product-inventory-v1.schema.json": product_schema,
        "schemas/source-revisions-v1.schema.json": revisions_schema,
        "schemas/functional-parity-v1.schema.json": functional_parity_schema,
        "product-surfaces.v1.json": surfaces,
        "product-files.v1.json": files,
        "source-revisions.v1.json": source_revisions,
        "functional-parity.v1.json": parity,
    }


def write_documents(output_dir: Path, documents: Mapping[str, Any]) -> None:
    for relative in MANAGED_FILES:
        destination = output_dir / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        expected = canonical_json(documents[relative])
        if destination.is_file() and destination.read_bytes() == expected:
            continue
        temporary = destination.with_suffix(destination.suffix + ".tmp")
        temporary.write_bytes(expected)
        temporary.replace(destination)


def check_documents(
    output_dir: Path, documents: Mapping[str, Any]
) -> list[str]:
    drift: list[str] = []
    for relative in MANAGED_FILES:
        path = output_dir / relative
        expected = canonical_json(documents[relative])
        if not path.is_file():
            drift.append(f"missing: {relative}")
        elif path.read_bytes() != expected:
            drift.append(f"changed: {relative}")
    return drift


def validate_documents(output_dir: Path) -> None:
    try:
        schemas = {
            "product": json.loads(
                (output_dir / MANAGED_FILES[0]).read_text(encoding="utf-8")
            ),
            "revisions": json.loads(
                (output_dir / MANAGED_FILES[1]).read_text(encoding="utf-8")
            ),
            "parity": json.loads(
                (output_dir / MANAGED_FILES[2]).read_text(encoding="utf-8")
            ),
        }
        surfaces = json.loads(
            (output_dir / MANAGED_FILES[3]).read_text(encoding="utf-8")
        )
        files = json.loads(
            (output_dir / MANAGED_FILES[4]).read_text(encoding="utf-8")
        )
        revisions = json.loads(
            (output_dir / MANAGED_FILES[5]).read_text(encoding="utf-8")
        )
        parity = json.loads(
            (output_dir / MANAGED_FILES[6]).read_text(encoding="utf-8")
        )
    except (OSError, json.JSONDecodeError) as error:
        raise ManifestError(f"cannot load generated manifest set: {error}") from error
    validate_schema(surfaces, schemas["product"])
    validate_schema(files, schemas["product"])
    validate_schema(revisions, schemas["revisions"])
    validate_schema(parity, schemas["parity"])
    _validate_document_set(surfaces, files, revisions, parity)


def summary(documents: Mapping[str, Any]) -> str:
    surfaces = documents["product-surfaces.v1.json"]["entries"]
    files = documents["product-files.v1.json"]["entries"]
    counts: dict[str, int] = {
        kind: 0 for kind in (*SURFACE_KINDS, *FILE_KINDS)
    }
    for entry in [*surfaces, *files]:
        counts[entry["kind"]] = counts.get(entry["kind"], 0) + 1
    counts["submodule_ref"] = len(
        documents["source-revisions.v1.json"]["entries"]
    )
    return ", ".join(f"{kind}={counts[kind]}" for kind in sorted(counts))
