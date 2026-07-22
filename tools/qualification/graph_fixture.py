#!/usr/bin/env python3
"""Generate the deterministic full-workbench graph scale fixture."""

from __future__ import annotations

import argparse
import copy
import json
import uuid
from pathlib import Path
from typing import Any


CONTRACT = "chataigne-graph-scale-fixture-v1"
DEFAULT_GRAPH_NODE_COUNT = 10_000
DEFAULT_SOURCE = Path("apps/chataigne/tests/samples/test_simple_load.noisette")
FORMULA_LABEL = "ActionTest"
UUID_NAMESPACE = uuid.UUID("919ba432-87b1-5de3-90d9-aa422d48f4ac")


def _children(node: dict[str, Any]) -> list[dict[str, Any]]:
    children = node.get("children")
    return children if isinstance(children, list) else []


def _find_child(node: dict[str, Any], node_type: str) -> dict[str, Any]:
    for child in _children(node):
        if child.get("type") == node_type:
            return child
    raise ValueError(f"fixture has no {node_type!r} child below {node.get('type')!r}")


def _find_formula(library: dict[str, Any], label: str) -> dict[str, Any]:
    for child in _children(library):
        if child.get("type") == "alchemist_formula" and child.get("meta", {}).get("label") == label:
            return child
    raise ValueError(f"fixture has no Alchemist formula labelled {label!r}")


def _find_anode(formula: dict[str, Any], label: str) -> dict[str, Any]:
    for child in _children(formula):
        if child.get("type") == "alchemist_anode" and child.get("meta", {}).get("label") == label:
            return child
    raise ValueError(f"formula has no ANode labelled {label!r}")


def _collect_uuid_mapping(value: Any, clone_index: int, mapping: dict[str, str]) -> None:
    if isinstance(value, dict):
        node_uuid = value.get("uuid")
        if isinstance(node_uuid, str):
            mapping[node_uuid] = str(
                uuid.uuid5(UUID_NAMESPACE, f"clone:{clone_index}:node:{node_uuid}")
            )
        for child in value.values():
            _collect_uuid_mapping(child, clone_index, mapping)
    elif isinstance(value, list):
        for child in value:
            _collect_uuid_mapping(child, clone_index, mapping)


def _replace_uuids(value: Any, mapping: dict[str, str]) -> Any:
    if isinstance(value, dict):
        return {key: _replace_uuids(child, mapping) for key, child in value.items()}
    if isinstance(value, list):
        return [_replace_uuids(child, mapping) for child in value]
    if isinstance(value, str):
        return mapping.get(value, value)
    return value


def _clone_anode(template: dict[str, Any], clone_index: int, columns: int) -> dict[str, Any]:
    mapping: dict[str, str] = {}
    _collect_uuid_mapping(template, clone_index, mapping)
    clone = _replace_uuids(copy.deepcopy(template), mapping)
    clone["meta"]["label"] = f"Scale Constant {clone_index + 1:05d}"
    clone["meta"]["decl_id"] = f"scale_constant_{clone_index + 1:05d}"
    clone["meta"]["short_name"] = f"scale_constant_{clone_index + 1:05d}"

    position = next(
        (
            child
            for child in _children(clone)
            if child.get("type") == "vec2" and child.get("meta", {}).get("decl_id") == "position"
        ),
        None,
    )
    if position is None:
        raise ValueError("ANode template has no position parameter")
    column = clone_index % columns
    row = clone_index // columns
    position.setdefault("data", {})["value"] = {"Vec2": [column * 15.0, row * 10.0]}
    return clone


def _promote_graph_editor(document: dict[str, Any]) -> None:
    dock_layout = document["ui_state"]["dock_layout"]
    panels = dock_layout["panels"]
    panels["alchemistEditor-1"]["title"] = f"Alchemist: {FORMULA_LABEL}"

    root = dock_layout["grid"]["root"]
    center_branch = root["data"][1]
    main_leaf = center_branch["data"][0]
    lower_branch = center_branch["data"][1]
    graph_leaf = lower_branch["data"][1]

    main_views = main_leaf["data"]["views"]
    main_leaf["data"]["views"] = [
        "alchemistEditor-1",
        *[view for view in main_views if view != "state-machine"],
    ]
    main_leaf["data"]["activeView"] = "alchemistEditor-1"
    graph_leaf["data"] = {
        "activeView": "state-machine",
        "id": graph_leaf["data"]["id"],
        "views": ["state-machine"],
    }

    outliner_state = panels.get("outliner", {}).get("params", {}).get("__gc_panel_state", {})
    outliner_state["opennessByNodeId"] = {}
    document["ui_state"]["selected_node_ids"] = []


def build_fixture(source: dict[str, Any], graph_node_count: int) -> tuple[dict[str, Any], dict[str, Any]]:
    if graph_node_count < 1:
        raise ValueError("graph_node_count must be positive")

    document = copy.deepcopy(source)
    library = _find_child(document["root"], "alchemist_formula_library")
    formula = _find_formula(library, FORMULA_LABEL)
    formula_uuid_mapping: dict[str, str] = {}
    _collect_uuid_mapping(formula, -1, formula_uuid_mapping)
    document = _replace_uuids(document, formula_uuid_mapping)
    library = _find_child(document["root"], "alchemist_formula_library")
    formula = _find_formula(library, FORMULA_LABEL)
    template = _find_anode(_find_formula(library, "bb"), "Constant")
    formula["meta"]["tags"] = [
        tag
        for tag in formula.get("meta", {}).get("tags", [])
        if tag != "chataigne.formula.external.file"
    ]

    preserved = [
        child
        for child in _children(formula)
        if child.get("type") not in {"alchemist_anode", "alchemist_connection"}
        and child.get("meta", {}).get("decl_id")
        not in {
            "external_formula_file",
            "external_formula_source",
            "external_formula_delete_file",
        }
    ]
    columns = 100
    clones = [_clone_anode(template, index, columns) for index in range(graph_node_count)]
    formula["children"] = [*preserved, *clones]
    _promote_graph_editor(document)

    metadata = {
        "contract": CONTRACT,
        "formula": FORMULA_LABEL,
        "graphNodeCount": graph_node_count,
        "columns": columns,
        "rows": (graph_node_count + columns - 1) // columns,
        "nodeSpacingRem": {"x": 15.0, "y": 10.0},
        "template": "constant",
    }
    return document, metadata


def write_fixture(source_path: Path, output_path: Path, graph_node_count: int) -> dict[str, Any]:
    source = json.loads(source_path.read_text(encoding="utf-8"))
    document, metadata = build_fixture(source, graph_node_count)
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(json.dumps(document, separators=(",", ":")), encoding="utf-8")
    return {**metadata, "output": str(output_path), "bytes": output_path.stat().st_size}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, default=DEFAULT_SOURCE)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--graph-node-count", type=int, default=DEFAULT_GRAPH_NODE_COUNT)
    args = parser.parse_args()
    print(
        json.dumps(
            write_fixture(args.source, args.output, args.graph_node_count),
            indent=2,
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
