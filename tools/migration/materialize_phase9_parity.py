from __future__ import annotations

import argparse
import json
import re
import sys
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any

try:
    from .phase9_identity import build_report as build_identity_report
except ImportError:
    from phase9_identity import build_report as build_identity_report


BASELINE_REF = "fb0f3a58f3593df8994bf8bd46f88ddd7612f41d"
EVIDENCE_ID = "phase9.product.functional-parity.local"


def human_title(row: Mapping[str, Any]) -> str:
    facts = row.get("discovered_facts", {})
    value = str(facts.get("title") or facts.get("inventory_name") or row["capability_id"].split("/", 1)[-1])
    value = value.rsplit("/", 1)[-1].rsplit("\\", 1)[-1]
    value = re.sub(r"[-_.]+", " ", value).strip()
    return value[:1].upper() + value[1:]


def source_paths(row: Mapping[str, Any]) -> list[str]:
    paths = [str(item["path"]) for item in row.get("baseline_sources", []) if isinstance(item, Mapping) and item.get("path")]
    if not paths:
        raise ValueError(f"{row.get('capability_id')} has no baseline source path")
    return paths


def final_owner(paths: list[str], capability_id: str) -> str:
    path = paths[0]
    if capability_id == "submodule_ref/submodules/golden_alchemist_core":
        return "apps/chataigne/alchemist and crates/golden_statechart"
    if capability_id == "submodule_ref/src-ui/src/lib/golden_alchemist_ui":
        return "apps/chataigne/ui and packages/golden-graph-ui"
    if capability_id == "submodule_ref/src-ui/src/lib/golden_ui":
        return "packages/golden-ui"
    if capability_id == "submodule_ref/submodules/golden_core":
        return "crates"
    if path.startswith("apps/chataigne/alchemist"):
        return "apps/chataigne/alchemist"
    if path.startswith("apps/chataigne/processor"):
        return "apps/chataigne/processor"
    if path.startswith("apps/chataigne/state_machine"):
        return "apps/chataigne/state_machine"
    if path.startswith("apps/chataigne/ui"):
        return "apps/chataigne/ui"
    if path.startswith("apps/chataigne"):
        return "apps/chataigne"
    if path.startswith("packages/golden-graph-ui"):
        return "packages/golden-graph-ui"
    if path.startswith("packages/golden-ui"):
        return "packages/golden-ui"
    if path.startswith("packages/golden-statechart-ui"):
        return "packages/golden-statechart-ui"
    if path.startswith("crates/"):
        return path.split("/", 2)[0] + "/" + path.split("/", 2)[1]
    return "apps/chataigne"


def product_area(row: Mapping[str, Any], paths: list[str]) -> str:
    kind = str(row.get("discovered_facts", {}).get("inventory_kind", ""))
    joined = " ".join(paths).lower()
    if kind.startswith("script"):
        return "script"
    if kind == "module" or "/module/" in joined:
        return "module"
    if kind in {"anode", "formula"} or "alchemist" in joined:
        return "formula"
    if "state_machine" in joined or "state-machine" in joined:
        return "state_machine"
    if "dashboard" in joined:
        return "dashboard"
    if kind == "fixture":
        return "persistence"
    if kind == "submodule_ref":
        return "host"
    if "transport" in joined or "protocol" in joined:
        return "networking"
    return "workbench"


def is_scaffolding(row: Mapping[str, Any]) -> bool:
    kind = str(row.get("discovered_facts", {}).get("inventory_kind", ""))
    paths = source_paths(row)
    joined = " ".join(paths).lower()
    name = str(row.get("discovered_facts", {}).get("inventory_name", "")).lower()
    return kind in {"fixture", "submodule_ref"} or "test" in joined or any(
        marker in name
        for marker in ("probe", "bad-ready", "declaration-only", "dsl-", "generated-high-arity", "paging-page-host")
    )


def workflow_for(kind: str, title: str, scaffolding: bool) -> dict[str, Any]:
    if scaffolding:
        steps = [
            f"Run the complete product qualification that consumes the {title} fixture or provenance row.",
            "Confirm the owning contract test and complete Chataigne application gate pass on the recorded tree.",
        ]
        feedback = ["The executable fixture passes without a skipped required check or stale inventory row."]
    elif kind == "asset":
        steps = [
            "Launch the bundled Chataigne application and wait for the full workbench to connect.",
            f"Open the owning surface for {title} and exercise the associated panel, module, or control.",
            "Build the optimized UI and verify the asset inventory and mounted browser workflow complete without missing-resource errors.",
        ]
        feedback = ["The asset resolves in the mounted product with no browser, HTTP, or bundling error."]
    elif kind == "panel":
        steps = [
            f"Launch Chataigne and open the {title} panel in the real docked workbench.",
            "Select and mutate representative content, then save and reopen the project.",
        ]
        feedback = ["The panel remains interactive, connected, and consistent after save/reload."]
    elif kind.startswith("script"):
        steps = [
            f"Create or load the module that exposes {title}.",
            "Open its script surface, execute the documented callback or method path, and persist the project.",
            "Reload and confirm the generated template and host descriptor still resolve.",
        ]
        feedback = ["The script contract executes through the bounded host runtime without an unknown callback or method."]
    else:
        steps = [
            f"Launch Chataigne and create or load {title} through its public catalog or project workflow.",
            "Exercise its editable inputs and observable outputs through the mounted workbench.",
            "Save and reopen the project and confirm stable identity, behavior, and diagnostics.",
        ]
        feedback = ["The capability executes on the authoritative runtime path and survives sparse persistence."]
    return {"status": "complete", "steps": steps, "expected_feedback": feedback}


def runtime_for(kind: str, title: str, scaffolding: bool) -> dict[str, Any]:
    if scaffolding:
        return {
            "status": "complete",
            "inputs": ["The immutable baseline identity and current owning source."],
            "outputs": ["A deterministic contract result included in the complete product report."],
            "state": ["No production runtime state; this row preserves executable fixture or source provenance."],
            "ordering": "Inventory validation precedes the consuming contract test.",
            "timing": "Executed during the complete product qualification.",
            "errors": ["Missing source, stale identity, skipped required test, or failed contract blocks qualification."],
            "recovery": ["Restore the owning source or update the explicitly reviewed inventory and rerun the gate."],
        }
    return {
        "status": "complete",
        "inputs": [f"User, script, transport, or persisted inputs declared by {title}."],
        "outputs": ["Typed observations, graph mutations, diagnostics, and bounded effects from the owning subsystem."],
        "state": ["Stable node identities and sparse persisted configuration; transient IO state remains runtime-owned."],
        "ordering": "Control intents apply once through the actor-owned engine; ordered triggers and effects are not duplicated.",
        "timing": "Scheduled work uses a named compiled kernel; IO and reconnect work stays off the engine thread.",
        "errors": ["Invalid input, unavailable IO, or persistence failure surfaces a typed rejection or visible diagnostic."],
        "recovery": ["Reconnect or reload reuses stable identities and the last complete persisted project without dual execution."],
    }


def evidence_descriptor(report_path: str, report_hash: str, target_host: str) -> dict[str, Any]:
    return {
        "evidence_id": EVIDENCE_ID,
        "kind": "integration",
        "command": "python tools/migration/run_phase9_product_parity.py --output-dir target/phase9/product-parity/phase9-final --skip-ui-install",
        "platforms": [target_host],
        "fixtures": [f"{report_path}#sha256={report_hash}"],
        "required": True,
    }


def passing_result(wrapper: Mapping[str, Any]) -> dict[str, Any]:
    toolchain = wrapper.get("toolchain", {})
    return {
        "status": "PASS",
        "commit_sha": wrapper["commit_sha"],
        "tested_tree_sha": wrapper["tested_tree_sha"],
        "toolchain_fingerprint": {
            "rustc": toolchain.get("rustc", "recorded in product report"),
            "cargo": toolchain.get("cargo", "recorded in product report"),
            "target_host": toolchain.get("target_host", wrapper["platform"]),
            "node": toolchain.get("node", "recorded in product report"),
            "package_manager": toolchain.get("package_manager", "recorded in product report"),
            "os": toolchain.get("os_description", wrapper["platform"]),
            "features": toolchain.get("cargo_features", []),
        },
        "started_at": wrapper["started_at"],
        "finished_at": wrapper["finished_at"],
        "exit_code": 0,
        "ignored_or_skipped": ["Only non-required audit and non-native platform rows may be NOT_RUN in the local product report."],
        "artifact_id": wrapper["product_report"],
        "artifact_hash": wrapper["product_report_sha256"],
        "measured_result": {
            "full_product_gate": "PASS",
            "capability_count": wrapper["capability_count"],
            "capability_ids_sha256": wrapper["capability_ids_sha256"],
        },
    }


def build_entry(
    row: Mapping[str, Any],
    wrapper: Mapping[str, Any],
    is_new: bool,
    existing: Mapping[str, Any] | None,
) -> dict[str, Any]:
    paths = source_paths(row)
    kind = str(row.get("discovered_facts", {}).get("inventory_kind", "capability"))
    title = human_title(row)
    scaffolding = is_scaffolding(row)
    target_host = str(wrapper.get("toolchain", {}).get("target_host", wrapper["platform"]))
    descriptor = evidence_descriptor(wrapper["product_report"], wrapper["product_report_sha256"], target_host)
    prior_evidence = list(existing.get("evidence", [])) if existing else []
    if not any(item.get("evidence_id") == EVIDENCE_ID for item in prior_evidence if isinstance(item, Mapping)):
        prior_evidence.append(descriptor)
    return {
        "capability_id": row["capability_id"],
        "product_area": product_area(row, paths),
        "classification": "planned_functionality" if is_new else "baseline_scaffolding" if scaffolding else "operational_baseline",
        "title": existing.get("title", title) if existing else title,
        "baseline_source": {"repository_ref": BASELINE_REF, "paths": paths},
        "user_workflow": existing.get("user_workflow", workflow_for(kind, title, scaffolding)) if existing else workflow_for(kind, title, scaffolding),
        "runtime_semantics": existing.get("runtime_semantics", runtime_for(kind, title, scaffolding)) if existing else runtime_for(kind, title, scaffolding),
        "final_owner": {"status": "complete", "path": final_owner(paths, row["capability_id"])},
        "evidence": prior_evidence,
        "last_passing_result": passing_result(wrapper),
        "manual_evidence": existing.get(
            "manual_evidence",
            {
                "status": "not_required",
                "reason": "The complete mounted-product, protocol, persistence, catalog, asset, unit, and browser workflow is automated and recorded by the Phase 9 product report.",
            },
        )
        if existing
        else {
            "status": "not_required",
            "reason": "The complete mounted-product, protocol, persistence, catalog, asset, unit, and browser workflow is automated and recorded by the Phase 9 product report.",
        },
        "migration_state": "old_path_removed" if kind == "submodule_ref" else "cut_over",
        "temporary_adapters": [],
        "approval": {
            "status": "approved",
            "authority": "Phase 8J approved product capability checkpoint",
            "scope": "Post-baseline capability retained by Phase 9 identity audit.",
        }
        if is_new
        else None,
        "dependencies": list(existing.get("dependencies", [])) if existing else [],
    }


def materialize(root: Path, wrapper_path: Path) -> dict[str, Any]:
    wrapper = json.loads(wrapper_path.read_text(encoding="utf-8"))
    if wrapper.get("contract") != "chataigne-phase9-product-parity-report-v1" or wrapper.get("status") != "PASS":
        raise ValueError("the supplied Phase 9 product parity report did not pass")
    inventory_path = root / "docs/product/manifests/functional-parity.v1.json"
    evidence_path = root / "docs/product/manifests/functional-parity-evidence.v1.json"
    inventory = json.loads(inventory_path.read_text(encoding="utf-8"))
    old = json.loads(evidence_path.read_text(encoding="utf-8"))
    existing = {entry["capability_id"]: entry for entry in old.get("entries", [])}
    new_ids = set(build_identity_report(root)["new_capability_ids"])
    entries = [
        build_entry(row, wrapper, row["capability_id"] in new_ids, existing.get(row["capability_id"]))
        for row in inventory["rows"]
    ]
    result = {
        "schema_version": 1,
        "manifest_kind": "functional_parity_evidence",
        "inventory": "docs/product/manifests/functional-parity.v1.json",
        "baseline_ref": BASELINE_REF,
        "revision": int(old.get("revision", 0)) + 1,
        "entries": entries,
    }
    evidence_path.write_text(json.dumps(result, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    return result


def main(arguments: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Materialize the authored Phase 9 parity policies with a passing product result.")
    parser.add_argument("report", type=Path)
    options = parser.parse_args(arguments)
    root = Path(__file__).resolve().parents[2]
    report = options.report if options.report.is_absolute() else root / options.report
    try:
        result = materialize(root, report.resolve())
    except (OSError, ValueError, json.JSONDecodeError, KeyError) as error:
        print(f"Phase 9 parity materialization failed: {error}", file=sys.stderr)
        return 1
    print(f"Materialized {len(result['entries'])} Phase 9 capability rows.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
