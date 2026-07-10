#!/usr/bin/env python3
"""Validate the clean-sheet architecture contract and its evidence ledger."""

from __future__ import annotations

import json
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
ARCHITECTURE = ROOT / "docs" / "architecture"


def load_json(name: str) -> dict:
    path = ARCHITECTURE / name
    with path.open(encoding="utf-8") as source:
        return json.load(source)


def require(condition: bool, message: str) -> None:
    if not condition:
        raise SystemExit(message)


def validate_dependency_rules() -> None:
    contract = load_json("dependency-rules.v1.json")
    require(contract.get("schema_version") == 1, "dependency rules must use schema v1")

    rules = contract.get("rules", [])
    layers = [rule.get("from") for rule in rules]
    require(all(layers), "every dependency rule needs a source layer")
    require(len(layers) == len(set(layers)), "dependency source layers must be unique")

    for rule in rules:
        source = rule["from"]
        dependencies = rule.get("may_depend_on", [])
        require(source not in dependencies, f"{source} cannot depend on itself")
        require(
            len(dependencies) == len(set(dependencies)),
            f"{source} contains duplicate allowed dependencies",
        )

    for rule in contract.get("forbidden", []):
        source = rule.get("layer")
        imports = rule.get("imports", [])
        require(source, "every forbidden-import rule needs a layer")
        require(source not in imports, f"{source} cannot forbid itself as an import")
        require(len(imports) == len(set(imports)), f"{source} repeats a forbidden import")


def validate_parity_ledger() -> None:
    ledger = load_json("functional-parity.v1.json")
    require(ledger.get("schema_version") == 1, "parity ledger must use schema v1")
    require(ledger.get("phase") == 0, "parity ledger must belong to Phase 0")

    allowed = set(ledger.get("allowed_evidence", []))
    require(allowed == {"automated", "manual", "pending"}, "unexpected evidence policy")

    capabilities = ledger.get("capabilities", [])
    identifiers = [capability.get("id") for capability in capabilities]
    require(all(identifiers), "every parity capability needs an id")
    require(len(identifiers) == len(set(identifiers)), "parity capability ids must be unique")

    for capability in capabilities:
        identifier = capability["id"]
        evidence = capability.get("evidence")
        require(evidence in allowed, f"{identifier} has invalid evidence kind {evidence!r}")
        require(capability.get("final_owner"), f"{identifier} needs a final owner")

        if evidence == "automated":
            relative_path = capability.get("evidence_path")
            require(relative_path, f"{identifier} needs an automated evidence path")
            require((ROOT / relative_path).is_file(), f"{identifier} evidence does not exist: {relative_path}")
        elif evidence == "manual":
            require(capability.get("acceptance"), f"{identifier} needs a manual acceptance scenario")

    if ledger.get("status") == "frozen":
        pending = [capability["id"] for capability in capabilities if capability["evidence"] == "pending"]
        require(not pending, f"frozen parity ledger still has pending entries: {', '.join(pending)}")


def main() -> None:
    require(
        (ROOT / "docs" / "Golden_Architecture_Final_Plan.md").is_file(),
        "canonical Golden architecture plan is missing",
    )
    validate_dependency_rules()
    validate_parity_ledger()
    print("architecture contract: valid")


if __name__ == "__main__":
    main()
