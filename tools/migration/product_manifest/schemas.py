"""JSON Schemas and dependency-free validation for generated manifests."""

from __future__ import annotations

import re
from collections.abc import Mapping
from typing import Any

from .common import (
    CANONICAL_PERFORMANCE_FIXTURES,
    FILE_KINDS,
    SURFACE_KINDS,
    ManifestError,
    canonical_json,
)


def inventory_schema() -> dict[str, Any]:
    source = {
        "type": "object",
        "additionalProperties": False,
        "required": ["path", "line"],
        "properties": {
            "path": {"type": "string", "minLength": 1},
            "line": {"type": "integer", "minimum": 1},
        },
    }
    surface_entry = {
        "type": "object",
        "additionalProperties": False,
        "required": [
            "id",
            "kind",
            "name",
            "certainty",
            "discovery_methods",
            "sources",
            "facts",
        ],
        "properties": {
            "id": {"type": "string", "minLength": 1},
            "kind": {"type": "string", "enum": list(SURFACE_KINDS)},
            "name": {"type": "string", "minLength": 1},
            "certainty": {
                "type": "string",
                "enum": ["registered", "declared", "file_discovered"],
            },
            "discovery_methods": {
                "type": "array",
                "items": {"type": "string", "minLength": 1},
                "minItems": 1,
                "uniqueItems": True,
            },
            "sources": {"type": "array", "items": source, "minItems": 1},
            "facts": {"type": "object"},
        },
    }
    file_entry = {
        "type": "object",
        "additionalProperties": False,
        "required": ["id", "kind", "name", "path", "bytes", "sha256"],
        "properties": {
            "id": {"type": "string", "minLength": 1},
            "kind": {"type": "string", "enum": list(FILE_KINDS)},
            "name": {"type": "string", "minLength": 1},
            "path": {"type": "string", "minLength": 1},
            "bytes": {"type": "integer", "minimum": 0},
            "sha256": {"type": "string", "pattern": "^[0-9a-f]{64}$"},
        },
    }
    fixture_requirement = {
        "type": "object",
        "additionalProperties": False,
        "required": ["id", "status", "matches"],
        "properties": {
            "id": {
                "type": "string",
                "enum": list(CANONICAL_PERFORMANCE_FIXTURES),
            },
            "status": {"type": "string", "enum": ["present", "absent"]},
            "matches": {
                "type": "array",
                "items": {"type": "string", "minLength": 1},
                "uniqueItems": True,
            },
        },
    }
    return {
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": "product-inventory-v1.schema.json",
        "type": "object",
        "additionalProperties": False,
        "required": [
            "schema_version",
            "manifest_kind",
            "generated_by",
            "category_counts",
            "entries",
        ],
        "properties": {
            "schema_version": {"type": "integer", "enum": [1]},
            "manifest_kind": {
                "type": "string",
                "enum": ["product_surfaces", "product_files"],
            },
            "generated_by": {"type": "object"},
            "category_counts": {"type": "object"},
            "fixture_requirements": {
                "type": "array",
                "items": fixture_requirement,
                "uniqueItems": True,
            },
            "entries": {
                "type": "array",
                "items": {"oneOf": [surface_entry, file_entry]},
            },
        },
    }


def source_revisions_schema() -> dict[str, Any]:
    return {
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": "source-revisions-v1.schema.json",
        "type": "object",
        "additionalProperties": False,
        "required": ["schema_version", "manifest_kind", "generated_by", "entries"],
        "properties": {
            "schema_version": {"type": "integer", "enum": [1]},
            "manifest_kind": {"type": "string", "enum": ["source_revisions"]},
            "generated_by": {"type": "object"},
            "entries": {
                "type": "array",
                "items": {
                    "type": "object",
                    "additionalProperties": False,
                    "required": ["path", "gitlink", "parent", "url", "branch"],
                    "properties": {
                        "path": {"type": "string", "minLength": 1},
                        "gitlink": {
                            "type": "string",
                            "pattern": "^[0-9a-f]{40}$",
                        },
                        "parent": {"type": "string"},
                        "url": {"type": ["string", "null"]},
                        "branch": {"type": ["string", "null"]},
                    },
                },
            },
        },
    }


def parity_schema() -> dict[str, Any]:
    source = {
        "type": "object",
        "additionalProperties": False,
        "required": ["path", "line"],
        "properties": {
            "path": {"type": "string", "minLength": 1},
            "line": {"type": "integer", "minimum": 1},
        },
    }
    return {
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": "functional-parity-v1.schema.json",
        "type": "object",
        "additionalProperties": False,
        "required": ["schema_version", "manifest_kind", "generated_by", "rows"],
        "properties": {
            "schema_version": {"type": "integer", "enum": [1]},
            "manifest_kind": {"type": "string", "enum": ["functional_parity"]},
            "generated_by": {"type": "object"},
            "rows": {
                "type": "array",
                "items": {
                    "type": "object",
                    "additionalProperties": False,
                    "required": [
                        "capability_id",
                        "product_area",
                        "baseline_sources",
                        "discovered_facts",
                        "user_workflow",
                        "runtime_semantics",
                        "final_owner",
                        "evidence",
                        "last_passing_result",
                        "manual_evidence",
                        "migration_state",
                        "verification_state",
                        "temporary_adapter",
                        "approval",
                    ],
                    "properties": {
                        "capability_id": {"type": "string", "minLength": 1},
                        "product_area": {"type": "string", "minLength": 1},
                        "baseline_sources": {
                            "type": "array",
                            "items": source,
                            "minItems": 1,
                        },
                        "discovered_facts": {
                            "type": "object",
                            "required": [
                                "inventory_kind",
                                "inventory_name",
                                "discovery_is_behavioral_proof",
                            ],
                            "properties": {
                                "inventory_kind": {
                                    "type": "string",
                                    "minLength": 1,
                                },
                                "inventory_name": {
                                    "type": "string",
                                    "minLength": 1,
                                },
                                "discovery_is_behavioral_proof": {
                                    "type": "boolean",
                                    "enum": [False],
                                },
                            },
                        },
                        "user_workflow": {
                            "type": "object",
                            "additionalProperties": False,
                            "required": [
                                "status",
                                "steps",
                                "expected_feedback",
                                "placeholder",
                            ],
                            "properties": {
                                "status": {
                                    "type": "string",
                                    "enum": ["pending_characterization"],
                                },
                                "steps": {
                                    "type": "array",
                                    "items": {"type": "string"},
                                    "maxItems": 0,
                                },
                                "expected_feedback": {"type": "null"},
                                "placeholder": {
                                    "type": "string",
                                    "minLength": 1,
                                },
                            },
                        },
                        "runtime_semantics": {
                            "type": "object",
                            "additionalProperties": False,
                            "required": [
                                "status",
                                "inputs",
                                "outputs",
                                "state",
                                "ordering",
                                "timing",
                                "errors",
                                "recovery",
                                "placeholder",
                            ],
                            "properties": {
                                "status": {
                                    "type": "string",
                                    "enum": ["pending_characterization"],
                                },
                                "inputs": {
                                    "type": "array",
                                    "items": {"type": "string"},
                                    "maxItems": 0,
                                },
                                "outputs": {
                                    "type": "array",
                                    "items": {"type": "string"},
                                    "maxItems": 0,
                                },
                                "state": {"type": "null"},
                                "ordering": {"type": "null"},
                                "timing": {"type": "null"},
                                "errors": {"type": "null"},
                                "recovery": {"type": "null"},
                                "placeholder": {
                                    "type": "string",
                                    "minLength": 1,
                                },
                            },
                        },
                        "final_owner": {
                            "type": "object",
                            "additionalProperties": False,
                            "required": ["status", "path"],
                            "properties": {
                                "status": {"type": "string", "enum": ["pending"]},
                                "path": {"type": "null"},
                            },
                        },
                        "evidence": {
                            "type": "object",
                            "additionalProperties": False,
                            "required": ["id", "status", "kind", "artifact_id"],
                            "properties": {
                                "id": {"type": "string", "minLength": 1},
                                "status": {"type": "string", "enum": ["pending"]},
                                "kind": {"type": "string", "enum": ["unassigned"]},
                                "artifact_id": {"type": "null"},
                            },
                        },
                        "last_passing_result": {"type": ["object", "null"]},
                        "manual_evidence": {
                            "type": "object",
                            "additionalProperties": False,
                            "required": ["required", "status"],
                            "properties": {
                                "required": {"type": "null"},
                                "status": {"type": "string", "enum": ["pending"]},
                            },
                        },
                        "migration_state": {
                            "type": "string",
                            "enum": ["baseline"],
                        },
                        "verification_state": {
                            "type": "string",
                            "enum": ["pending"],
                        },
                        "temporary_adapter": {"type": ["object", "null"]},
                        "approval": {
                            "type": "string",
                            "enum": ["not_requested"],
                        },
                    },
                },
            },
        },
    }


def validate_schema(
    instance: Any, schema: Mapping[str, Any], location: str = "$"
) -> None:
    """Validate the JSON Schema subset used by the generated contracts."""
    if "oneOf" in schema:
        matches = 0
        errors: list[str] = []
        for candidate in schema["oneOf"]:
            try:
                validate_schema(instance, candidate, location)
                matches += 1
            except ManifestError as error:
                errors.append(str(error))
        if matches != 1:
            raise ManifestError(
                f"{location}: expected exactly one oneOf schema match; "
                + "; ".join(errors[:2])
            )
        return
    expected_type = schema.get("type")
    if expected_type is not None:
        expected = expected_type if isinstance(expected_type, list) else [expected_type]
        predicates = {
            "object": lambda value: isinstance(value, dict),
            "array": lambda value: isinstance(value, list),
            "string": lambda value: isinstance(value, str),
            "integer": lambda value: isinstance(value, int)
            and not isinstance(value, bool),
            "number": lambda value: isinstance(value, (int, float))
            and not isinstance(value, bool),
            "boolean": lambda value: isinstance(value, bool),
            "null": lambda value: value is None,
        }
        if not any(predicates[type_name](instance) for type_name in expected):
            raise ManifestError(f"{location}: expected {' or '.join(expected)}")
    if "enum" in schema and instance not in schema["enum"]:
        raise ManifestError(f"{location}: {instance!r} is not an allowed value")
    if isinstance(instance, str):
        if len(instance) < schema.get("minLength", 0):
            raise ManifestError(f"{location}: string is shorter than minLength")
        if "pattern" in schema and re.search(schema["pattern"], instance) is None:
            raise ManifestError(f"{location}: string does not match {schema['pattern']}")
    if isinstance(instance, (int, float)) and not isinstance(instance, bool):
        if "minimum" in schema and instance < schema["minimum"]:
            raise ManifestError(f"{location}: number is below minimum")
    if isinstance(instance, list):
        if len(instance) < schema.get("minItems", 0):
            raise ManifestError(f"{location}: array is shorter than minItems")
        if len(instance) > schema.get("maxItems", len(instance)):
            raise ManifestError(f"{location}: array is longer than maxItems")
        if schema.get("uniqueItems"):
            encoded = [canonical_json(item) for item in instance]
            if len(encoded) != len(set(encoded)):
                raise ManifestError(f"{location}: array items are not unique")
        item_schema = schema.get("items")
        if isinstance(item_schema, Mapping):
            for index, item in enumerate(instance):
                validate_schema(item, item_schema, f"{location}[{index}]")
    if isinstance(instance, dict):
        for key in schema.get("required", []):
            if key not in instance:
                raise ManifestError(f"{location}: missing required property {key!r}")
        properties = schema.get("properties", {})
        if schema.get("additionalProperties") is False:
            extras = sorted(set(instance) - set(properties))
            if extras:
                raise ManifestError(f"{location}: unexpected properties {extras}")
        for key, child_schema in properties.items():
            if key in instance and isinstance(child_schema, Mapping):
                validate_schema(instance[key], child_schema, f"{location}.{key}")
