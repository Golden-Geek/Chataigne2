# Source size inventory

## Ownership and scope

The owning feature keeps each source file readable and reviewable; generated outputs, lockfiles,
vendored dependencies, and long-form documentation are outside this source limit. This inventory
covers 1169 current Rust, TypeScript, Svelte, JavaScript, PowerShell, and shell source files under
`apps/`, `crates/`, `packages/`, and `tools/`, excluding `generated/`, `gen/`, `build/`, and
`node_modules/`. It was refreshed on 2026-09-12 after the T17 engine-script split.
The 44-file audit count was a historical baseline with a different source snapshot; 54 files
currently exceed 1,000 lines. No oversized runtime or test-source exception is approved yet.

`golden_engine::script` was split by ownership in this batch: its node lifecycle remains in
`script/mod.rs` (under 1,000 lines), while template expansion and the engine-to-VM host bridge
live in `script/template.rs` and `script/host.rs`. The full engine suite and strict Clippy pass.
The entries below are remaining work, not exceptions or evidence that a mechanical line-limit
split is sufficient. Prioritize boundaries already being changed; preserve behavior tests and
avoid mixing large structural moves into runtime race fixes.

## Files over 1,000 lines

| Lines | Source path | Current disposition |
| ---: | --- | --- |
| 13,350 | `crates/golden_core/engine/src/engine/tests/engine.rs` | Split focused test suites |
| 6,423 | `packages/golden-ui/components/panels/dashboard/DashboardCanvas.svelte` | Decompose presentation and state |
| 5,818 | `packages/golden-ui/components/common/AnimationCurveNodeEditor.svelte` | Decompose presentation and state |
| 5,188 | `apps/chataigne/systems/state_machine/integration/manager/mod.rs` | Split cohesive Rust module |
| 5,096 | `apps/chataigne/systems/alchemist/integration/formula/mod.rs` | Split cohesive Rust module |
| 5,056 | `crates/golden_core/support/macros/src/lib.rs` | Split macro families |
| 3,533 | `packages/golden-graph-ui/components/GraphCanvas.svelte` | Decompose presentation and state |
| 3,152 | `apps/chataigne/src/module/modules/protocol/midi/midi_module/mod.rs` | Split cohesive Rust module |
| 2,673 | `apps/chataigne/ui/src/lib/systems/alchemist/components/AlchemistEditorPanel.svelte` | Decompose presentation and state |
| 2,596 | `apps/chataigne/systems/alchemist/integration/formula/tests/mod.rs` | Split focused test suites |
| 2,596 | `apps/chataigne/src/module/modules/generators/spatializer/mod.rs` | Split cohesive Rust module |
| 2,561 | `apps/chataigne/ui/src/lib/panels/modules/SpatializerEditorPanel.svelte` | Decompose presentation and state |
| 2,524 | `crates/golden_core/engine/src/ui_sync.rs` | Split engine projection and intent application |
| 2,362 | `crates/golden_core/hosts/transport/src/ui_server/mod.rs` | Split cohesive Rust module |
| 2,325 | `crates/golden_core/engine/src/engine/controls.rs` | Split cohesive Rust module |
| 2,046 | `crates/golden_core/engine/src/app/mod.rs` | Split cohesive Rust module |
| 2,031 | `apps/chataigne/src/module/modules/system/app_control/app_control/mod.rs` | Split cohesive Rust module |
| 1,996 | `packages/golden-ui/components/common/AnimationCurveCanvas.svelte` | Decompose presentation and state |
| 1,846 | `crates/golden_core/engine/src/node/dashboard/tests/mod.rs` | Split focused test suites |
| 1,773 | `apps/chataigne/systems/state_machine/integration/manager/tests/mod.rs` | Split focused test suites |
| 1,684 | `apps/chataigne/systems/alchemist/integration/processor/catalog.rs` | Split cohesive Rust module |
| 1,673 | `packages/golden-ui/transport/http.ts` | Split transport/state concerns |
| 1,641 | `crates/golden_core/engine/src/node_macros.rs` | Split cohesive Rust module |
| 1,631 | `apps/chataigne/systems/alchemist/integration/processor/tests/processor.rs` | Split focused test suites |
| 1,594 | `apps/chataigne/src/module/modules/controllers/joycon/mod.rs` | Split cohesive Rust module |
| 1,487 | `apps/chataigne/systems/alchemist/src/runtime.rs` | Split cohesive Rust module |
| 1,480 | `packages/golden-ui/components/common/NodeContextMenu.svelte` | Decompose presentation and state |
| 1,447 | `apps/chataigne/src/module/modules/protocol/osc/generic_osc_module/tests/mod.rs` | Split focused test suites |
| 1,442 | `crates/golden_core/engine/src/engine/history.rs` | Split cohesive Rust module |
| 1,359 | `apps/chataigne/ui/scripts/ui-browser-tools.mjs` | Split browser harness |
| 1,334 | `apps/chataigne/systems/alchemist/src/tests/runtime.rs` | Split focused test suites |
| 1,331 | `crates/golden_core/engine/src/node/curve/model.rs` | Split cohesive Rust module |
| 1,320 | `apps/chataigne/systems/alchemist/integration/processor/mod.rs` | Split cohesive Rust module |
| 1,276 | `crates/golden_core/engine/src/node/core/behavior.rs` | Split cohesive Rust module |
| 1,275 | `apps/chataigne/src/module/modules/controllers/mouse/mouse/mouse_runtime.rs` | Split cohesive Rust module |
| 1,272 | `crates/golden_core/engine/src/engine/persistence/mod.rs` | Split cohesive Rust module |
| 1,231 | `packages/golden-ui/components/panels/inspector/ParameterInspector.svelte` | Decompose presentation and state |
| 1,230 | `apps/chataigne/src/module/modules/controllers/gamepad/gamepad/mod.rs` | Split cohesive Rust module |
| 1,225 | `apps/chataigne/systems/alchemist/processor/src/tests/processor.rs` | Split focused test suites |
| 1,166 | `apps/chataigne/src/module/modules/protocol/midi/commands/mod.rs` | Split cohesive Rust module |
| 1,164 | `apps/chataigne/systems/alchemist/processor/src/processor.rs` | Split cohesive Rust module |
| 1,139 | `apps/chataigne/src/module/modules/controllers/mouse/mouse/mod.rs` | Split cohesive Rust module |
| 1,110 | `packages/golden-ui/components/panels/logger/LoggerPanel.svelte` | Decompose presentation and state |
| 1,095 | `apps/chataigne/src/module/modules/generators/signals/mod.rs` | Split cohesive Rust module |
| 1,091 | `apps/chataigne/ui/src/lib/systems/state_machine/components/StateMachinePanel.svelte` | Decompose presentation and state |
| 1,090 | `apps/chataigne/src/module/common/received_values.rs` | Split cohesive Rust module |
| 1,086 | `apps/chataigne/src/module/modules/protocol/mqtt/mod.rs` | Split cohesive Rust module |
| 1,077 | `apps/chataigne/src/module/modules/system/app_control/app_control/app_control_runtime.rs` | Split cohesive Rust module |
| 1,054 | `crates/golden_core/engine/src/node/curve/node.rs` | Split cohesive Rust module |
| 1,052 | `apps/chataigne/src/module/modules/controllers/keyboard/keyboard/mod.rs` | Split cohesive Rust module |
| 1,014 | `apps/chataigne/src/module/modules/protocol/osc/osc_module_base.rs` | Split cohesive Rust module |
| 1,008 | `apps/chataigne/systems/alchemist/integration/processor/tests/catalog.rs` | Split focused test suites |
| 1,007 | `packages/golden-ui/components/panels/inspector/parameters/Vec2PadEditor.svelte` | Decompose presentation and state |
| 1,006 | `apps/chataigne/src/module/modules/audio/sound_card/integration.rs` | Split cohesive Rust module |

The live count can be reproduced with `rg --files` over the paths and extensions above, followed
by a line count per file. Recount after each cohesive split. Intentional centralized registries
may be documented as exceptions only after their review cost and alternatives are explained.
