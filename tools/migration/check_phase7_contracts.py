#!/usr/bin/env python3
"""Checks the Phase 7 generated protocol and UI transport cutover boundaries."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path


def read(root: Path, relative: str) -> str:
    return (root / relative).read_text(encoding="utf-8")


def collect_violations(root: Path) -> list[str]:
    violations: list[str] = []
    ui_sync = read(root, "crates/core/src/ui_sync.rs")
    codegen = read(root, "crates/codegen_support/src/lib.rs")
    server = read(root, "crates/transport_server/src/ui_server.rs")
    queue = read(root, "crates/transport_server/src/ui_server/outbound_queue.rs")
    client = read(root, "packages/golden-ui/transport/ws.ts")
    http = read(root, "packages/golden-ui/transport/http.ts")
    workbench = read(root, "packages/golden-ui/store/workbench.svelte.ts")
    public_index = read(root, "packages/golden-ui/index.ts")
    dashboard = json.loads(read(root, "docs/product/manifests/phase7-cutovers.v1.json"))

    for declaration in (
        "pub enum UiClientMessage",
        "pub enum UiServerMessage",
        "pub enum UiDataPlane",
        "pub struct UiInterest",
        "pub enum UiControlPhase",
        "pub struct UiPlaneDelta",
    ):
        if declaration not in ui_sync:
            violations.append(f"Rust protocol is missing `{declaration}`")

    if 'UI_PROTOCOL_VERSION: &str = "0.2.0"' not in ui_sync:
        violations.append("Phase 7 protocol version is not 0.2.0")

    for export in (
        'export_binding::<UiClientMessage>(&config, "UiClientMessage")',
        'export_binding::<UiServerMessage>(&config, "UiServerMessage")',
    ):
        if export not in codegen:
            violations.append(f"codegen is missing `{export}`")

    if "enum WsClientMessage" in server or "enum WsServerMessage" in server:
        violations.append("transport_server still hand-declares the WebSocket protocol")
    if "UiClientMessage as WsClientMessage" not in server or "UiServerMessage as WsServerMessage" not in server:
        violations.append("transport_server does not consume the Rust-owned protocol")

    if "WsOutboundQueue::new(DEFAULT_OUTBOUND_CAPACITY)" not in server:
        violations.append("WebSocket clients do not use a bounded outbound queue")
    if "QueuePushResult::Full" not in server or "disconnecting slow client" not in server:
        violations.append("slow-client isolation is not explicit")
    if "is_latest_wins" not in queue or "merge_keyed_preview_events" not in queue:
        violations.append("latest-wins value/observation/preview queue policy is incomplete")

    if "generated/rust_protocol/UiClientMessage" not in client:
        violations.append("TypeScript client does not import the generated client message")
    if "generated/rust_protocol/UiServerMessage" not in client:
        violations.append("TypeScript client does not import the generated server message")
    for legacy_runtime_call in (
        "httpClient.snapshot(",
        "httpClient.subscribe(",
        "httpClient.replay(",
        "httpClient.sendIntent(",
        "httpClient.sendIntents(",
    ):
        if legacy_runtime_call in client:
            violations.append(f"production client still uses runtime HTTP fallback `{legacy_runtime_call}`")
    if "requestAnimationFrame(flush)" not in client:
        violations.append("protocol planes are not committed coherently on animation frames")

    if "createHttpUiClient" in http or "createHttpUiClient" in public_index:
        violations.append("the former all-purpose HTTP UI client remains public")
    if "subscribeInterest?.(" not in workbench or "onResyncRequired" not in workbench:
        violations.append("workbench is not using per-view interest and typed scoped resync")

    generated = root / "packages/golden-ui/generated/rust_protocol"
    for name in ("UiClientMessage.ts", "UiServerMessage.ts", "UiDataPlane.ts", "UiControlPhase.ts"):
        if not (generated / name).is_file():
            violations.append(f"generated protocol binding is missing `{name}`")

    expected_cutovers = {
        "generated_multi_plane_protocol",
        "control_lifecycle",
        "interest_and_observation_planes",
        "bounded_slow_client_isolation",
        "frame_coherent_ui_client",
        "phase7a_session_workbench",
        "phase7b_core_authoring_ui",
        "phase7c_graph_alchemist",
        "phase7d_state_machine",
        "phase7e_modules_specialized_panels",
        "phase7f_packaging_remote_client",
        "phase6_ui_compat_removed",
    }
    cutovers = {item["cutover_id"]: item for item in dashboard.get("cutovers", [])}
    missing_cutovers = expected_cutovers - cutovers.keys()
    if missing_cutovers:
        violations.append(f"Phase 7 dashboard is missing cutovers: {sorted(missing_cutovers)}")
    if dashboard.get("phase") != 7 or dashboard.get("validation_state") != "CHECKPOINT_RUNNABLE":
        violations.append("Phase 7 dashboard does not record a runnable checkpoint")
    product_gate = dashboard.get("product_gate", "")
    if "20260716T070444Z/product-gate-report.json" not in product_gate or "37 required" not in product_gate:
        violations.append("Phase 7 dashboard does not record the passing 37-check product gate")
    for cutover_id in expected_cutovers - {"phase6_ui_compat_removed"}:
        if cutovers.get(cutover_id, {}).get("state") != "cut_over":
            violations.append(f"Phase 7 cutover is not complete: {cutover_id}")
    if cutovers.get("phase6_ui_compat_removed", {}).get("state") != "removed":
        violations.append("Phase 6 UI compatibility adapter is not recorded as removed")
    if dashboard.get("temporary_adapters") != []:
        violations.append("Phase 7 dashboard carries an unexpired protocol adapter")

    return violations


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[2])
    args = parser.parse_args()
    violations = collect_violations(args.root.resolve())
    if violations:
        for violation in violations:
            print(f"phase7 contract violation: {violation}", file=sys.stderr)
        return 1
    print("phase7 protocol and UI transport contracts: PASS")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
