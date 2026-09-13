# Source size inventory

## Ownership and scope

The owning feature keeps each source file readable and reviewable; generated outputs, lockfiles,
vendored dependencies, and long-form documentation are outside this source limit. This inventory
includes 1,242 current Rust, TypeScript, Svelte, JavaScript, Python, PowerShell, and shell source
files under `apps/`, `crates/`, `packages/`, and `tools/`, excluding `generated/`, `gen/`, `build/`, and
`node_modules/`. It was refreshed on 2026-09-13 after the T17 script, UI-sync, persistence-adapter,
history, App Control, received-value, formula integration, processor presentation, multiplex
test, generic graph, and logger splits and the T19 qualification harnesses. The 44-file audit
count was a historical baseline; 45 files currently exceed 1,000
lines. No oversized runtime or test-source exception is approved yet.

`golden_engine::script` was split by ownership in this batch: its node lifecycle remains in
`script/mod.rs` (under 1,000 lines), while template expansion and the engine-to-VM host bridge
live in `script/template.rs` and `script/host.rs`. The full engine suite and strict Clippy pass.
`golden_engine::ui_sync` now keeps intent coordination in its root module, with focused conversion,
creation/duplication, and snapshot/event projection modules; all four files are under 1,000 lines.
Engine persistence now keeps its record/lifecycle application apart from metadata and recovery
types. Engine history keeps captured effects and the public history API apart from undo/redo replay.
The Chataigne App Control runtime now delegates platform window enumeration and actions to a
focused Windows-aware adapter; its process, folder-watch, and worker orchestration stay together.
Its module node keeps lifecycle and event coordination, while watch processing, watch structure,
and script request parsing have focused modules. All touched App Control sources are under 1,000
lines.

Chataigne's received-value adapter now keeps batched subtree planning in its main module and
isolates incremental single/multi-message application, including retry-on-parent-materialization,
in `received_values/incremental.rs`.

The app-owned Alchemist formula integration now keeps node lifecycle/registration in its root
(572 lines) and moves ANode/socket behavior, property surfaces, construction, value conversion,
snapshot reconstruction, external-file workflow, reconciliation, and library watching into
separate modules (all under 1,000 lines). App-node codegen registers the child node types through
the root module's public re-exports. A generator regression test covers this boundary.

The processor crate keeps execution and property-frame resolution in `processor.rs` (956 lines),
with debug capture and UI/preview projection in `processor/presentation.rs` (201 lines). The
multiplex app test keeps shared fixture/measurement helpers in `multiplex.rs` (417 lines), with
runtime timing (310), interaction/transaction checks (312), an opt-in scale probe (220), and
worker-equivalence checks (186) in adjacent test modules. State-manager scale fixture extraction
(50) is test-only.

The generic graph canvas now delegates edge routing, presentation projection, and camera geometry
to focused modules; its remaining node layout, interactions, and rendering still exceed the limit.
The logger panel keeps scrolling, filtering controls, and selection interaction (868 lines), while
record decoration, duplicate grouping, and clipboard projection live in `logger/log-projection.ts`
(267 lines). Seven direct logger projection tests cover its display modes and cache invalidation.

The entries below are remaining work, not exceptions or evidence that a mechanical line-limit
split is sufficient. Prioritize boundaries already being changed; preserve behavior tests and
avoid mixing large structural moves into runtime race fixes.

## Files over 1,000 lines

| Lines | Source path | Current disposition |
| ---: | --- | --- |
| 13,350 | `crates/golden_core/engine/src/engine/tests/engine.rs` | Split focused test suites |
| 6,423 | `packages/golden-ui/components/panels/dashboard/DashboardCanvas.svelte` | Decompose presentation and state |
| 5,818 | `packages/golden-ui/components/common/AnimationCurveNodeEditor.svelte` | Decompose presentation and state |
| 5,449 | `apps/chataigne/systems/state_machine/integration/manager/mod.rs` | Split cohesive Rust module |
| 5,056 | `crates/golden_core/support/macros/src/lib.rs` | Split macro families |
| 3,152 | `apps/chataigne/src/module/modules/protocol/midi/midi_module/mod.rs` | Split cohesive Rust module |
| 3,110 | `packages/golden-graph-ui/components/GraphCanvas.svelte` | Decompose presentation and state |
| 2,673 | `apps/chataigne/ui/src/lib/systems/alchemist/components/AlchemistEditorPanel.svelte` | Decompose presentation and state |
| 2,632 | `apps/chataigne/systems/alchemist/integration/formula/tests/mod.rs` | Split focused test suites |
| 2,596 | `apps/chataigne/src/module/modules/generators/spatializer/mod.rs` | Split cohesive Rust module |
| 2,561 | `apps/chataigne/ui/src/lib/panels/modules/SpatializerEditorPanel.svelte` | Decompose presentation and state |
| 2,362 | `crates/golden_core/hosts/transport/src/ui_server/mod.rs` | Split cohesive Rust module |
| 2,334 | `crates/golden_core/engine/src/engine/controls.rs` | Split cohesive Rust module |
| 2,050 | `crates/golden_core/engine/src/app/mod.rs` | Split cohesive Rust module |
| 1,996 | `packages/golden-ui/components/common/AnimationCurveCanvas.svelte` | Decompose presentation and state |
| 1,846 | `crates/golden_core/engine/src/node/dashboard/tests/mod.rs` | Split focused test suites |
| 1,774 | `apps/chataigne/systems/state_machine/integration/manager/tests/mod.rs` | Split focused test suites |
| 1,684 | `apps/chataigne/systems/alchemist/integration/processor/catalog.rs` | Split cohesive Rust module |
| 1,673 | `packages/golden-ui/transport/http.ts` | Split transport/state concerns |
| 1,646 | `crates/golden_core/engine/src/node_macros.rs` | Split cohesive Rust module |
| 1,631 | `apps/chataigne/systems/alchemist/integration/processor/tests/processor.rs` | Split focused test suites |
| 1,594 | `apps/chataigne/src/module/modules/controllers/joycon/mod.rs` | Split cohesive Rust module |
| 1,487 | `apps/chataigne/systems/alchemist/src/runtime.rs` | Split cohesive Rust module |
| 1,480 | `packages/golden-ui/components/common/NodeContextMenu.svelte` | Decompose presentation and state |
| 1,447 | `apps/chataigne/src/module/modules/protocol/osc/generic_osc_module/tests/mod.rs` | Split focused test suites |
| 1,359 | `apps/chataigne/ui/scripts/ui-browser-tools.mjs` | Split browser harness |
| 1,354 | `apps/chataigne/systems/alchemist/integration/processor/mod.rs` | Split cohesive Rust module |
| 1,334 | `apps/chataigne/systems/alchemist/src/tests/runtime.rs` | Split focused test suites |
| 1,331 | `crates/golden_core/engine/src/node/curve/model.rs` | Split cohesive Rust module |
| 1,281 | `crates/golden_core/engine/src/node/core/behavior.rs` | Split cohesive Rust module |
| 1,275 | `apps/chataigne/src/module/modules/controllers/mouse/mouse/mouse_runtime.rs` | Split cohesive Rust module |
| 1,255 | `apps/chataigne/systems/alchemist/processor/src/tests/processor.rs` | Split focused test suites |
| 1,231 | `packages/golden-ui/components/panels/inspector/ParameterInspector.svelte` | Decompose presentation and state |
| 1,230 | `apps/chataigne/src/module/modules/controllers/gamepad/gamepad/mod.rs` | Split cohesive Rust module |
| 1,166 | `apps/chataigne/src/module/modules/protocol/midi/commands/mod.rs` | Split cohesive Rust module |
| 1,139 | `apps/chataigne/src/module/modules/controllers/mouse/mouse/mod.rs` | Split cohesive Rust module |
| 1,095 | `apps/chataigne/src/module/modules/generators/signals/mod.rs` | Split cohesive Rust module |
| 1,091 | `apps/chataigne/ui/src/lib/systems/state_machine/components/StateMachinePanel.svelte` | Decompose presentation and state |
| 1,086 | `apps/chataigne/src/module/modules/protocol/mqtt/mod.rs` | Split cohesive Rust module |
| 1,054 | `crates/golden_core/engine/src/node/curve/node.rs` | Split cohesive Rust module |
| 1,052 | `apps/chataigne/src/module/modules/controllers/keyboard/keyboard/mod.rs` | Split cohesive Rust module |
| 1,014 | `apps/chataigne/src/module/modules/protocol/osc/osc_module_base.rs` | Split cohesive Rust module |
| 1,008 | `apps/chataigne/systems/alchemist/integration/processor/tests/catalog.rs` | Split focused test suites |
| 1,007 | `packages/golden-ui/components/panels/inspector/parameters/Vec2PadEditor.svelte` | Decompose presentation and state |
| 1,006 | `apps/chataigne/src/module/modules/audio/sound_card/integration.rs` | Split cohesive Rust module |

The live count can be reproduced with `rg --files` over the paths and extensions above, followed
by a line count per file. Recount after each cohesive split. Intentional centralized registries
may be documented as exceptions only after their review cost and alternatives are explained.
