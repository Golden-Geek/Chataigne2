# Source-size inventory

This maintained reference records source size, not execution status. The
[implementation plan](../plan/Chataigne2_Codex_Implementation_Plan.md) owns P12 decomposition;
the [one current ledger](../progress/implementation-status.md) records progress. Historical
extraction notes and September counts are [archived](../archive/2026-10-18/progress/source-size-inventory.md).

## Scan and policy

Refreshed 2026-10-26 against delivered `81adcc37` plus tree checkpoint closure
on `main`. Scanned **2191 source files**; **27 exceed 1,000 lines**. TreeRows (235 lines),
the retained tree projection (273), navigation model (71), browser matrix and adjacent tests
remain below the limit. Picker host/selection/focus responsibilities and app qualification fixtures
stay below the limit. The existing AlchemistEditorPanel is qualified without editing it. The dashboard
canvas is 978 lines; its controllers, geometry, viewport, tests and stylesheet files remain
below the limit. No new source-size exception is introduced. Remaining decomposition and
exceptions belong to P12; this inventory does not qualify that task.
The surface controller is now 788 lines after extracting move and creation presentation ownership.
Confirmed-work admission, barriers and parameter drafts live in the 281-line session service; the
workbench facade remains 968 lines. The ordered facade is 46 lines, with separate move/resize controllers.
Dashboard actions are 681 lines after removing UI-owned move/placement/sizing sequences. Atomic
backend layout operations live in the 220-line `ui_sync/dashboard_layout.rs` module; their tests
remain 326 lines. Retained surface item order is a focused 56-line module, with adjacent tests.
Active coordinate-frame validation is a focused 67-line module. Free transforms remain 557 lines;
the composition fixture and browser matrix stay under the limit in separate owning test files.
The timing fixture, observation loop and browser driver have separate owners. Structural-child
index preparation is a focused 78-line shared graph projection module with separate consumer tests.
Page measurement snapshots are a focused 39-line module with adjacent tests. Camera and view-context
ownership remain under the limit at 674 and 362 lines respectively.

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
|  6,101 | `apps/chataigne/systems/state_machine/integration/manager/mod.rs`                    | Implementation |
|  5,818 | `packages/golden-ui/components/common/AnimationCurveNodeEditor.svelte`               | Implementation |
|  5,027 | `crates/golden_core/support/macros/src/lib.rs`                                       | Implementation |
|  2,561 | `apps/chataigne/ui/src/lib/panels/modules/SpatializerEditorPanel.svelte`             | Implementation |
|  2,402 | `apps/chataigne/systems/alchemist/integration/formula/tests/mod.rs`                  | Test suite     |
|  2,091 | `apps/chataigne/ui/src/lib/systems/alchemist/components/AlchemistEditorPanel.svelte` | Implementation |
|  2,066 | `crates/golden_core/engine/src/engine/controls.rs`                                   | Implementation |
|  1,996 | `packages/golden-ui/components/common/AnimationCurveCanvas.svelte`                   | Implementation |
|  1,992 | `crates/golden_core/engine/src/app/mod.rs`                                           | Implementation |
|  1,866 | `apps/chataigne/systems/state_machine/integration/manager/tests/mod.rs`              | Test suite     |
|  1,846 | `crates/golden_core/engine/src/node/dashboard/tests/mod.rs`                          | Test suite     |
|  1,693 | `crates/golden_core/engine/src/node_macros.rs`                                       | Implementation |
|  1,638 | `apps/chataigne/systems/alchemist/integration/processor/catalog.rs`                  | Implementation |
|  1,614 | `apps/chataigne/systems/alchemist/integration/processor/mod.rs`                      | Implementation |
|  1,601 | `apps/chataigne/systems/alchemist/integration/processor/tests/processor.rs`          | Test suite     |
|  1,480 | `packages/golden-ui/components/common/NodeContextMenu.svelte`                        | Implementation |
|  1,474 | `apps/chataigne/systems/alchemist/src/tests/runtime.rs`                              | Test suite     |
|  1,433 | `apps/chataigne/systems/alchemist/integration/processor/tests/authoring.rs`          | Test suite     |
|  1,359 | `apps/chataigne/ui/scripts/ui-browser-tools.mjs`                                     | Implementation |
|  1,331 | `crates/golden_core/engine/src/node/curve/model.rs`                                  | Implementation |
|  1,236 | `packages/golden-ui/components/panels/inspector/ParameterInspector.svelte`           | Implementation |
|  1,217 | `apps/chataigne/systems/alchemist/processor/src/tests/processor.rs`                  | Test suite     |
|  1,148 | `apps/chataigne/systems/alchemist/processor/src/tests/managed_formula.rs`            | Test suite     |
|  1,108 | `apps/chataigne/systems/state_machine/integration/manager/tests/command_dispatch.rs` | Test suite     |
|  1,054 | `crates/golden_core/engine/src/node/curve/node.rs`                                   | Implementation |
|  1,054 | `apps/chataigne/systems/alchemist/integration/processor/tests/catalog.rs`            | Test suite     |
