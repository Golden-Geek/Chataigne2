# Source-size inventory

This maintained reference records source size, not execution status. The
[implementation plan](../plan/Chataigne2_Codex_Implementation_Plan.md) owns P12 decomposition;
the [one current ledger](../progress/implementation-status.md) records progress. Historical
extraction notes and September counts are [archived](../archive/2026-10-18/progress/source-size-inventory.md).

## Scan and policy

Refreshed 2026-10-22 against delivered `3a38df47` plus C04 MQTT queued target-path fencing
on `main`. Scanned **1744 source files**; **36 exceed 1,000 lines**. The MQTT entrypoint stays
543 lines with target capture and its tests in focused input modules. The unused incremental
received-value path is removed; the shared planner and receipt-driven cursor remain. No new source-size exception is introduced. Remaining decomposition and exceptions belong
to P12; this inventory does not qualify that task.

The node enum macro remains the centralized built-in/consumer-variant dispatch registry:
each public Node hook must reach both sets of variants there. P12 still owns extracting its
mechanical forwarding. Codec generation now lives in the focused `support/macros/src/node_data.rs`
module rather than expanding the oversized macro entrypoint.

Discover files with `rg --files apps crates packages tools` using normal ignore rules.
Count UTF-8 text lines in `.rs`, `.ts`, `.svelte`, `.js`, `.mjs`, `.py`, `.ps1`, and `.sh`.
Exclude any path component named `generated`, `gen`, `build`, `node_modules`, `target`,
`.svelte-kit`, or `vendor`. Include tests. Generated outputs, lockfiles, and long-form docs
are outside the source limit.

Split cohesive ownership as features are touched; line count alone does not define a good
extraction. Keep runtime and tests separate under the owning `tests/` directory. Intentional
centralized registries may have a specific documented exception explaining the review cost
and alternatives; a blanket exception does not close P12. Refresh counts after relevant splits.

## Files above the limit

|  Lines | Source                                                                               | Kind           |
| -----: | ------------------------------------------------------------------------------------ | -------------- |
| 13,282 | `crates/golden_core/engine/src/engine/tests/engine.rs`                               | Test suite     |
|  6,423 | `packages/golden-ui/components/panels/dashboard/DashboardCanvas.svelte`              | Implementation |
|  6,098 | `apps/chataigne/systems/state_machine/integration/manager/mod.rs`                    | Implementation |
|  5,818 | `packages/golden-ui/components/common/AnimationCurveNodeEditor.svelte`               | Implementation |
|  5,027 | `crates/golden_core/support/macros/src/lib.rs`                                       | Implementation |
|  3,110 | `packages/golden-graph-ui/components/GraphCanvas.svelte`                             | Implementation |
|  2,596 | `apps/chataigne/src/module/modules/generators/spatializer/mod.rs`                    | Implementation |
|  2,561 | `apps/chataigne/ui/src/lib/panels/modules/SpatializerEditorPanel.svelte`             | Implementation |
|  2,422 | `apps/chataigne/ui/src/lib/systems/alchemist/components/AlchemistEditorPanel.svelte` | Implementation |
|  2,402 | `apps/chataigne/systems/alchemist/integration/formula/tests/mod.rs`                  | Test suite     |
|  2,075 | `crates/golden_core/engine/src/app/mod.rs`                                           | Implementation |
|  2,066 | `crates/golden_core/engine/src/engine/controls.rs`                                   | Implementation |
|  1,996 | `packages/golden-ui/components/common/AnimationCurveCanvas.svelte`                   | Implementation |
|  1,923 | `apps/chataigne/systems/alchemist/integration/processor/mod.rs`                      | Implementation |
|  1,866 | `apps/chataigne/systems/state_machine/integration/manager/tests/mod.rs`              | Test suite     |
|  1,846 | `crates/golden_core/engine/src/node/dashboard/tests/mod.rs`                          | Test suite     |
|  1,737 | `apps/chataigne/systems/alchemist/integration/processor/catalog.rs`                  | Implementation |
|  1,686 | `crates/golden_core/engine/src/node_macros.rs`                                       | Implementation |
|  1,662 | `packages/golden-ui/transport/http.ts`                                               | Implementation |
|  1,601 | `apps/chataigne/systems/alchemist/integration/processor/tests/processor.rs`          | Test suite     |
|  1,594 | `apps/chataigne/src/module/modules/controllers/joycon/mod.rs`                        | Implementation |
|  1,480 | `packages/golden-ui/components/common/NodeContextMenu.svelte`                        | Implementation |
|  1,474 | `apps/chataigne/systems/alchemist/src/tests/runtime.rs`                              | Test suite     |
|  1,433 | `apps/chataigne/systems/alchemist/integration/processor/tests/authoring.rs`          | Test suite     |
|  1,359 | `apps/chataigne/ui/scripts/ui-browser-tools.mjs`                                     | Implementation |
|  1,331 | `crates/golden_core/engine/src/node/curve/model.rs`                                  | Implementation |
|  1,275 | `apps/chataigne/src/module/modules/controllers/mouse/mouse/mouse_runtime.rs`         | Implementation |
|  1,231 | `packages/golden-ui/components/panels/inspector/ParameterInspector.svelte`           | Implementation |
|  1,217 | `apps/chataigne/systems/alchemist/processor/src/tests/processor.rs`                  | Test suite     |
|  1,210 | `apps/chataigne/src/module/modules/controllers/gamepad/gamepad/mod.rs`               | Implementation |
|  1,148 | `apps/chataigne/systems/alchemist/processor/src/tests/managed_formula.rs`            | Test suite     |
|  1,108 | `apps/chataigne/systems/state_machine/integration/manager/tests/command_dispatch.rs` | Test suite     |
|  1,104 | `apps/chataigne/src/module/modules/controllers/mouse/mouse/mod.rs`                   | Implementation |
|  1,054 | `apps/chataigne/systems/alchemist/integration/processor/tests/catalog.rs`            | Test suite     |
|  1,054 | `crates/golden_core/engine/src/node/curve/node.rs`                                   | Implementation |
|  1,021 | `apps/chataigne/src/module/modules/controllers/keyboard/keyboard/mod.rs`             | Implementation |
