# Chataigne Alchemist Integration Progress

## Current Phase

Phase 14 - manager reference ANodes as bridges is complete. The Chataigne
manager ref ANodes now compile as explicit StableRef-backed bridge nodes instead
of unsupported placeholders. Inputs and Conditions read manager-provided
ValueSets from `EvaluationCtx.inputs`, Outputs emits command intents toward an
outputs-manager target, and missing/unbound refs remain explicit diagnostics
with no fake fallback values. The next phase is Phase 15 - remove duplicated
manager-specific filter and condition logic.

## Completed Tasks

- Created the project-local progression document required before implementation work.
- Created branch `alchemist_next_move` from `main`.
- Read `docs/ALCHEMIST_NEXT_MOVES.md` and selected Phase 0 as the only
  safe first step before catalog/runtime changes.
- Audited the current processor creation path, formula library boundary,
  formula surface/properties mirror, app-side Alchemist registration, and
  the existing value collection naming.
- Ran baseline Rust validation:
  `cargo test --workspace` passed with 279 app tests and 34
  state-machine tests. The existing 2 ignored Alchemist tests remain ignored
  as stale pre-manager-ref behavior.
- Ran `cargo fmt --all` from the repository root. Cargo metadata lists only
  `Chataigne2` and `chataigne_state_machine`, so there is no separate
  `golden_core` Cargo manifest to format in this checkout.
- Completed Phase 0 supercommit:
  `f4b9888 supercommit: chataigne alchemist integration phase 0 - baseline audit`
- Added the Phase 1 formula catalog boundary in
  `src/state_machine_nodes/catalog.rs`.
- Added typed formula sources for project formulas and built-ins.
- Added built-in catalog entries for exactly one `Action` and one `Mapping`
  source:
  `state_processor:builtin:chataigne.action@1` and
  `state_processor:builtin:chataigne.mapping@1`.
- Routed processor manager and processor folder palette refresh through
  `FormulaCatalog`.
- Added typed create-source parsing while keeping legacy
  `state_processor:<uuid>` project formula IDs accepted.
- Split catalog code out of `processor.rs`; `processor.rs` is now 727 lines
  and `catalog.rs` is 433 lines.
- Added tests for built-in catalog visibility, formula library hiding,
  built-in source parsing/resolution, and clean invalid built-in diagnostics.
- Ran targeted processor tests:
  `cargo test app::state_machine_nodes_processor::processor_tests -- --nocapture`
  passed with 11 tests.
- Ran full workspace validation:
  `cargo test --workspace` passed with 282 app tests and 34
  state-machine tests. The existing 2 ignored Alchemist tests remain ignored
  as stale pre-manager-ref behavior.
- Completed Phase 1 supercommit:
  `a6086e0 supercommit: chataigne alchemist integration phase 1 - formula catalog`
- Added persisted `ProcessorFormulaSourceState` on `StateProcessor`.
- Added typed processor source writes for both project formulas and built-ins.
- Kept legacy project processors viable by falling back to the old `Formula`
  node reference when the persisted source state is empty.
- Synced the typed source state when the legacy project `Formula` reference
  parameter changes.
- Updated processor creation so built-in `Action` and `Mapping` create
  source-backed processor nodes instead of failing creation.
- Updated processor warnings so valid built-in sources do not report a
  missing project formula.
- Added source-state serialization coverage.
- Ran targeted processor tests:
  `cargo test app::state_machine_nodes_processor::processor_tests -- --nocapture`
  passed with 15 tests.
- Ran full workspace validation:
  `cargo test --workspace` passed with 286 app tests and 34
  state-machine tests. The existing 2 ignored Alchemist tests remain ignored
  as stale pre-manager-ref behavior.
- Completed Phase 2 supercommit:
  `4fff469 supercommit: chataigne alchemist integration phase 2 - processor formula sources`
- Added a compile-time included built-in formula package file at
  `src/state_machine_nodes/builtin_formulas/chataigne.formulas.json`.
- Replaced catalog-time placeholder construction with package loading through
  `FormulaCatalog::from_builtin_package_source`.
- Added package validation for empty package IDs, empty formula IDs, duplicate
  formula sources, and JSON decode failures.
- Built-in `Action` and `Mapping` now resolve by cloning shipped package
  definitions, not by synthesizing formula metadata at the resolver boundary.
- Kept the Phase 3 formulas as explicit empty-graph definitions until managed
  regions land in Phase 4. No fake Action or Mapping runtime behavior was
  added.
- Added coverage proving the shipped package exposes exactly one `Action` and
  exactly one `Mapping`, with no Mapping variants.
- Ran targeted processor tests:
  `cargo test app::state_machine_nodes_processor::processor_tests -- --nocapture`
  passed with 16 tests.
- Ran full workspace validation:
  `cargo test --workspace` passed with 287 app tests and 34
  state-machine tests. The existing 2 ignored Alchemist tests remain ignored
  as stale pre-manager-ref behavior.
- Completed Phase 3 supercommit:
  `1ecd2c7 supercommit: chataigne alchemist integration phase 3 - builtin formulas`
- Added reusable managed-region authoring metadata to
  `golden_alchemist::FormulaSurface`.
- Added `ManagedRegionKind` values for `InputSet`, `FilterPipeline`,
  `OutputSet`, `ActionTrigger`, and `ActionCommands`.
- Added `ManagedRegionDefinition`, `ManagedSocketRef`,
  `ManagedRegionInstances`, `ManagedRegionInstance`, `ManagedItemInstance`,
  and `ManagedItemUiState`.
- Added `ManagedRegionId` and `ManagedItemId` to the Alchemist ID layer.
- Added `managed_regions` to `AlchemistFormulaInstance`; formula
  instantiation now materializes empty region instances from the formula
  surface definitions.
- Added explicit validation for managed-region instance references so stale
  or invalid region IDs report a diagnostic instead of being accepted
  silently.
- Declared the built-in `Action` managed regions in the shipped package:
  `trigger`, `pipeline`, and `commands`.
- Declared the built-in `Mapping` managed regions in the shipped package:
  `inputs`, `filters`, and `outputs`.
- Kept `ConditionGate` out of the managed region model; conditions remain
  future filter-capable ANodes.
- Kept project-authored formula surfaces on the existing section-only path
  with an empty managed-region list.
- Ran reusable Alchemist targeted tests:
  `cargo test -p golden_alchemist formula_tests -- --nocapture` passed with
  4 tests.
- Ran targeted processor tests:
  `cargo test app::state_machine_nodes_processor::processor_tests -- --nocapture`
  passed with 17 tests.
- Ran formatting:
  `cargo fmt --all` from the repository root,
  `cargo fmt --all` in `submodules/golden_alchemist_core`, and
  `cargo fmt --all` in `submodules/golden_core`.
- Ran full reusable Alchemist validation:
  `cargo test --workspace` in `submodules/golden_alchemist_core` passed with
  90 `golden_alchemist` tests and 4 `golden_statechart` tests.
- Ran full root workspace validation:
  `cargo test --workspace` passed with 288 app tests and 34 state-machine
  tests. The existing 2 ignored Alchemist tests remain ignored as stale
  pre-manager-ref behavior.
- Completed reusable Alchemist submodule Phase 4 commit:
  `06a7712 supercommit: chataigne alchemist integration phase 4 - managed regions`
- Added the Phase 5 `ValueSet` boundary model in
  `src/state_machine/src/value_set.rs`.
- Added stable `ValueLaneKey`, `ValueSetEntry`, `ValueSet`, and
  `ValueSetError` types for future InputSet, filter pipeline, and OutputSet
  work.
- Registered the app-specific Alchemist value type as
  `VALUE_SET_TYPE = "chataigne.value_set"` with the user-facing label
  `Value Set`.
- Renamed manager reference ANode sockets from `parameters` / `Parameters`
  to `values` / `Values`.
- Chose a clean schema break for the old `chataigne.param_array` runtime
  type. It is not registered as an alias and `ValueSet::from_runtime_value`
  rejects it explicitly.
- Updated the manager-reference unsupported diagnostic to refer to
  `ValueSet` resolution instead of `ParamArray` resolution.
- Updated architecture docs that still referenced manager fake defaults or
  input aggregation as `ParamArray`.
- Added coverage for ValueSet construction, stable lane keys, runtime
  extension serialization, old type rejection, registry registration, and
  manager socket exposure.
- Ran targeted ValueSet tests:
  `cargo test -p chataigne_state_machine value_set -- --nocapture` passed
  with 4 tests.
- Ran targeted manager-reference tests:
  `cargo test -p chataigne_state_machine manager_reference -- --nocapture`
  passed with 2 tests.
- Ran targeted ValueSet registration test:
  `cargo test -p chataigne_state_machine valueset_type -- --nocapture`
  passed with 1 test.
- Ran formatting:
  `cargo fmt --all` from the repository root,
  `cargo fmt --all` in `submodules/golden_alchemist_core`, and
  `cargo fmt --all` in `submodules/golden_core`.
- Ran full root workspace validation:
  `cargo test --workspace` passed with 288 app tests and 38 state-machine
  tests. The existing 2 ignored Alchemist tests remain ignored as stale
  pre-manager-ref behavior.
- Added reusable ANode role capability metadata to `golden_alchemist`.
- Added `ANodeRoleCapability`, `AutoWirePolicy`, `PipelineCardinality`, and
  `ManagedUiMode`.
- Added `ANodeDeclaration::role_capabilities` and
  `ANodeDeclaration::supports_role` as declaration-level metadata hooks.
- Added `ANodeRegistry::declarations_with_role` so managers and future
  pipeline compilers can discover filter-capable nodes without matching
  node type strings.
- Registered initial existing primitive filter capabilities for:
  `math`, `function`, `remap`, `smooth_filter`, `one_minus`, `inverse`,
  `negate`, `speed`, `coordinate_system`, `angle_conversion`,
  `convert_to_color`, and `extract_color`.
- Declared unary transform autowiring for existing unary filters with
  `value` input and `result` output.
- Declared aggregate/reshape cardinality metadata for existing primitives
  where the current node shape is already clear.
- Added reusable Alchemist coverage for filter-capable discovery,
  non-filter rejection, primary socket autowiring, and capability JSON
  roundtrip.
- Ran targeted reusable Alchemist tests:
  `cargo test -p golden_alchemist library_tests -- --nocapture` passed with
  38 tests.
- Ran full reusable Alchemist validation:
  `cargo test --workspace` in `submodules/golden_alchemist_core` passed with
  94 `golden_alchemist` tests and 4 `golden_statechart` tests.
- Ran full root workspace validation:
  `cargo test --workspace` passed with 288 app tests and 38 state-machine
  tests. The existing 2 ignored Alchemist tests remain ignored as stale
  pre-manager-ref behavior.
- Completed reusable Alchemist submodule Phase 6 commit:
  `28beff1 supercommit: chataigne alchemist integration phase 6 - anode capabilities`
- Added `ConditionGate` as a reusable primitive ANode in `golden_alchemist`.
- Registered `ConditionGate` as a filter-capable ANode through the Phase 6
  capability metadata, with gate autowiring from `value` and `condition` to
  output `value`.
- Added `ConditionGate` config for mode:
  `pass_when_true`, `pass_when_false`, `hold_last`, `output_default`, and
  `block_trigger`.
- Added explicit `gate_application` config:
  `whole` and `per_lane`.
- Implemented whole-value gating. This covers normal single values, triggers,
  command-intent-like extension values, and app-owned opaque `ValueSet`
  extension payloads as complete values.
- Left per-lane `ValueSet` gating explicitly unsupported until lane-aware
  ValueSet lowering exists; it returns a runtime diagnostic instead of
  silently pretending to gate lanes.
- Added `ConditionGate` runtime behavior tests for pass, block, hold-last,
  explicit default output, trigger blocking, and whole extension-value gating.
- Added reusable Alchemist capability coverage proving `ConditionGate`
  appears as a filter-capable ANode.
- Ran targeted reusable Alchemist tests:
  `cargo test -p golden_alchemist condition_gate -- --nocapture` passed with
  7 tests.
- Completed reusable Alchemist submodule Phase 7 commit:
  `6089db0 supercommit: chataigne alchemist integration phase 7 - condition gate anode`
- Added a reusable typed linear pipeline shape checker in
  `golden_alchemist`.
- Added `PipelineShape` variants for:
  `Single`, `ValueSet`, `Trigger`, `CommandIntent`, and `Unknown`.
- Added public pipeline check result types:
  `PipelineShapeCheckItem`, `PipelineShapeStep`, `PipelineShapeDiagnostic`,
  and `PipelineShapeResult`.
- Added `check_filter_pipeline_shapes` as the reusable Phase 8 boundary.
- The checker consumes Phase 6 `ANodeRoleCapability` metadata and rejects
  nodes that do not declare the `Filter` role.
- The checker records explicit cardinality transitions from
  `PipelineCardinality`: `Elementwise`, `Aggregate`, `Reshape`, `Expand`,
  and `WholeSet`.
- `ConditionGate` preserves the incoming shape through its `WholeSet`
  capability.
- `Aggregate` explicitly collapses `ValueSet<T>` to `Single<T>`.
- `Expand` explicitly turns `Single<T>` into `ValueSet<T>`.
- `Reshape` uses the declaration signature's primary output type; it reports
  a diagnostic when that output shape cannot be resolved.
- No merge or broadcast behavior is inferred without an explicit
  `Aggregate` or `Expand` capability.
- Added Phase 8 pipeline tests covering elementwise `ValueSet`, aggregate to
  `Single`, Pack Vec3-style reshape, invalid non-filter node rejection,
  `ConditionGate` shape preservation, and Broadcast-style expansion.
- Used shape-only test declarations for Pack Vec3 and Broadcast coverage
  instead of registering placeholder production ANodes.
- Ran targeted reusable Alchemist tests:
  `cargo test -p golden_alchemist pipeline -- --nocapture` passed with
  6 tests.
- Ran full reusable Alchemist validation:
  `cargo test --workspace` in `submodules/golden_alchemist_core` passed with
  107 `golden_alchemist` tests and 4 `golden_statechart` tests.
- Ran full root workspace validation:
  `cargo test --workspace` passed with 288 app tests and 38 state-machine
  tests. The existing 2 ignored Alchemist tests remain ignored as stale
  pre-manager-ref behavior.
- Completed reusable Alchemist submodule Phase 8 commit:
  `96437b6 supercommit: chataigne alchemist integration phase 8 - pipeline shape checker`
- Added the first Phase 9 reusable filter-pipeline lowering boundary in
  `golden_alchemist`.
- Added `PipelineLoweringCtx`, `PipelineLoweringDiagnostic`,
  `PipelineLoweringDiagnosticKind`, `FilterPipelineLoweringResult`, and
  `lower_filter_pipeline_region`.
- Added `AlchemistFormula::materialize_with_filter_pipelines` as an opt-in
  context-aware materialization path for formulas that want managed
  filter-pipeline regions lowered after ordinary property overrides.
- Kept the existing lightweight `AlchemistFormula::materialize` API unchanged
  for callers that do not have an ANode registry, value type registry, or
  explicit filter-pipeline starting shapes.
- The lowerer validates managed region kind, region instance identity,
  accepted role metadata, and declared input/output boundary sockets before
  graph mutation.
- The lowerer resolves each enabled managed item through `ANodeRegistry`,
  runs the Phase 8 shape checker, and refuses to mutate the graph when
  declarations are missing or the shape checker reports diagnostics.
- The lowerer inserts the actual managed `ANodeInstance`s into a draft graph
  and autowires only explicit linear policies:
  `UnaryTransform`, `Gate`, or paired primary input/output sockets.
- Disabled managed filter items are skipped during lowering while remaining
  in the authoring region instance.
- Non-filter items and aggregate/reshape/expand nodes without linear
  autowire sockets fail with diagnostics instead of implicit wiring.
- Added shape-trace-aware lowering diagnostics so `ValueSet` elementwise,
  aggregate, reshape, and expand transitions are not materialized as ordinary
  scalar graph wiring before lane-aware MapEach/reduction/broadcast support
  exists.
- Lowering diagnostics now carry stable typed kinds, including
  `UnsupportedValueSetElementwise`, `UnsupportedValueSetAggregate`,
  `UnsupportedValueSetReshape`, and `UnsupportedValueSetExpand`, so later
  Chataigne integration can branch on diagnostics without parsing messages.
- `AlchemistFormula::materialize_with_filter_pipelines` now preserves typed
  lowering diagnostics and shape diagnostics in
  `FormulaMaterializationError::ManagedRegionLoweringFailed`.
- Whole-set filters such as `ConditionGate` remain lowerable over `ValueSet`
  because they intentionally preserve the complete value shape.
- Ran targeted reusable Alchemist tests:
  `cargo test -p golden_alchemist pipeline -- --nocapture` passed with
  15 tests.
- Ran targeted reusable Alchemist formula tests:
  `cargo test -p golden_alchemist formula_tests -- --nocapture` passed with
  7 tests.
- Ran full reusable Alchemist validation:
  `cargo test --workspace` in `submodules/golden_alchemist_core` passed with
  116 `golden_alchemist` tests and 4 `golden_statechart` tests.
- Ran full root workspace validation:
  `cargo test --workspace` passed with 288 app tests and 38 state-machine
  tests. The existing 2 ignored Alchemist tests remain ignored as stale
  pre-manager-ref behavior.
- Added reusable `Clamp` and `Pack Vec3` primitive ANodes in
  `golden_alchemist`, including capability metadata, signatures, compile
  operations, and catalog tests.
- Fixed reusable compiler scheduling so `CompiledExecNode`s, state ranges,
  debug source maps, and runtime evaluation all share exec IDs assigned from
  true topological order instead of graph insertion order.
- Added the app-owned `ValueSetPipelineRuntime` for lane-aware elementwise
  filter execution. It compiles one scalar pipeline graph and evaluates each
  `ValueSet` lane through a stable `ContextKey`, preserving lane keys, labels,
  sources, logical ticks, runtime diagnostics, and debug samples.
- Added `ValueSetProjectionRuntime` for fixed-slot whole-set projection
  pipelines such as aggregate reductions and `Pack Vec3`.
- `ValueSetPipelineRuntime` uses `LaneRuntimePool` so stateful filters such as
  `Smooth Filter` keep independent memory per value lane without materializing
  one graph copy per lane.
- Added Phase 9 app runtime coverage for Remap + Clamp map chains,
  independent Smooth lane memory, aggregate reduction, Pack Vec3 projection,
  and ConditionGate inside a pipeline.
- Ran targeted Phase 9 app tests:
  `cargo test -p chataigne_state_machine value_set_pipeline -- --nocapture`
  passed with 6 tests.
- Ran targeted reusable Alchemist tests for compiler, library, pipeline, and
  formula lowering paths; all targeted filters passed.
- Ran full reusable Alchemist validation:
  `cargo test --workspace` in `submodules/golden_alchemist_core` passed with
  118 `golden_alchemist` tests and 4 `golden_statechart` tests.
- Ran full root workspace validation:
  `cargo test --workspace` passed with 288 app tests and 44 state-machine
  tests. The existing 2 ignored Alchemist tests remain ignored as stale
  pre-manager-ref behavior.
- Completed reusable Alchemist submodule Phase 9 commit:
  `5b1fca2 supercommit: chataigne alchemist integration phase 9 - filter pipeline lowering`
- Added the Phase 10 app-owned InputSet materialization boundary in
  `src/state_machine/src/input_set.rs`.
- Added `InputSetRuntime`, `InputSetItem`, and `InputSetMaterialization`.
- InputSet materialization reads selected `StableRef` sources from
  `EvaluationCtx.inputs` and emits `ValueSet` entries without fallback values.
- Added managed-region parsing for `ManagedRegionKind::InputSet` instances
  that contain authored input items with a `source` StableRef config field.
- Chose stable lane keys from persisted managed item IDs, so reordering inputs
  preserves lane identity independently of authored order.
- Disabled input items are excluded from the materialized `ValueSet`.
- Missing input source values produce an explicit `input_set_missing_source`
  diagnostic and no fake entry.
- Kept `InputsManagerRef` ANode runtime behavior unchanged and unsupported;
  manager reference bridges remain Phase 14 work.
- Ran targeted Phase 10 state-machine tests:
  `cargo test -p chataigne_state_machine input_set -- --nocapture` passed
  with 5 tests.
- Ran formatting:
  `cargo fmt --all` from the repository root,
  `cargo fmt --all` in `submodules/golden_alchemist_core`, and
  `cargo fmt --all` in `submodules/golden_core`.
- Ran full reusable Alchemist validation:
  `cargo test --workspace` in `submodules/golden_alchemist_core` passed with
  118 `golden_alchemist` tests and 4 `golden_statechart` tests.
- Ran full root workspace validation:
  `cargo test --workspace` passed with 288 app tests and 49 state-machine
  tests. The existing 2 ignored Alchemist tests remain ignored as stale
  pre-manager-ref behavior.

- Added the Phase 11 app-owned OutputSet materialization boundary in
  `src/state_machine/src/output_set.rs`.
- Added `OutputSetRuntime`, `OutputSetItem`, and
  `OutputSetMaterialization`.
- OutputSet materialization reads enabled managed output items with a
  `target` StableRef config field.
- Single runtime values produce one `chataigne.command` `RuntimeIntent` only
  when there is exactly one enabled output.
- `ValueSet` runtime values produce per-entry command intents by zipping
  ValueSet entries with enabled outputs in authored order.
- Idle triggers produce no intents, which lets blocked trigger gates suppress
  command output without fake fallback behavior.
- Single values with multiple enabled outputs now report an explicit
  diagnostic rather than silently broadcasting. Broadcast remains an explicit
  future filter capability.
- ValueSet/output count mismatches report an explicit diagnostic and dispatch
  nothing partially.
- Kept module transport and command dispatch outside OutputSet; the runtime
  boundary emits `RuntimeIntent`, and the existing state-machine arbitration
  path owns conversion to `CommandIntent` and dispatch.
- Ran targeted Phase 11 state-machine tests:
  `cargo test -p chataigne_state_machine output_set -- --nocapture` passed
  with 6 tests.
- Ran formatting:
  `cargo fmt --all` from the repository root,
  `cargo fmt --all` in `submodules/golden_alchemist_core`, and
  `cargo fmt --all` in `submodules/golden_core`.
- Ran full reusable Alchemist validation:
  `cargo test --workspace` in `submodules/golden_alchemist_core` passed with
  118 `golden_alchemist` tests and 4 `golden_statechart` tests.
- Ran full root workspace validation:
  `cargo test --workspace` passed with 288 app tests and 55 state-machine
  tests. The existing 2 ignored Alchemist tests remain ignored as stale
  pre-manager-ref behavior.

## Phase 12 - Built-in Mapping Managed Orchestration - Complete

- Added a generic managed formula runtime in `chataigne_state_machine` that
  composes InputSet, optional FilterPipeline, and OutputSet regions by managed
  region kind rather than by the Mapping formula id.
- Processor compilation now builds the managed runtime sidecar when a formula
  surface declares the reusable InputSet/FilterPipeline/OutputSet shape; ordinary
  formula graphs keep the existing compiled-graph execution path.
- Managed filters compile lazily from the materialized ValueSet payload type and
  lane count, so endpoint `StableRef` types are not treated as payload data
  types.
- Covered pass-through mapping, elementwise filter chains, aggregate projection,
  and Pack Vec3 projection with runtime tests using real built-in primitive
  declarations.
- Covered the `ProcessorRuntime` sidecar path with a managed Mapping-style
  processor test.
- Made `golden_alchemist` value-type and ANode registries cloneable so managed
  runtimes can lazily compile filter projections by observed payload type without
  holding compile-context borrows.
- Ran targeted Phase 12 state-machine tests:
  `cargo test -p chataigne_state_machine managed_formula -- --nocapture` passed
  with 5 tests.
- Ran full state-machine validation:
  `cargo test -p chataigne_state_machine` passed with 60 tests and 2 ignored
  stale pre-manager-ref tests.
- Ran formatting:
  `cargo fmt --all` from the repository root,
  `cargo fmt --all` in `submodules/golden_alchemist_core`, and
  `cargo fmt --all` in `submodules/golden_core`.
- Ran full reusable Alchemist validation:
  `cargo test --workspace` in `submodules/golden_alchemist_core` passed with
  118 `golden_alchemist` tests and 4 `golden_statechart` tests.
- Ran full root workspace validation:
  `cargo test --workspace` passed with 288 app tests and 60 state-machine tests.
  The existing 2 ignored Alchemist tests remain ignored as stale pre-manager-ref
  behavior.

## Phase 13 - Built-in Action Pipeline End-to-End - Complete

- Extended the generic managed formula runtime to support the Action managed
  region family without branching on the built-in Action formula id.
- Added an app-owned `ActionTrigger` runtime boundary that validates
  `ManagedRegionKind::ActionTrigger`, accepts authored input items, reads a
  `source` StableRef config field, and materializes a single trigger payload from
  `EvaluationCtx.inputs`.
- Added an app-owned `ActionCommands` runtime boundary that validates
  `ManagedRegionKind::ActionCommands`, accepts authored action items, reads a
  `target` StableRef config field, and emits `chataigne.command` runtime intents
  for enabled commands when the trigger is fired.
- Reused the managed FilterPipeline runtime for Action by wrapping the trigger
  as a one-lane ValueSet, so `ConditionGate` remains a normal filter-capable
  ANode rather than a special condition region.
- Covered trigger dispatch, ConditionGate blocking, ConditionGate passing, and
  the `ProcessorRuntime` Action sidecar path with state-machine tests.
- Ran targeted Phase 13 state-machine tests:
  `cargo test -p chataigne_state_machine managed_formula -- --nocapture` passed
  with 9 tests.
- Ran full state-machine validation:
  `cargo test -p chataigne_state_machine` passed with 64 tests and 2 ignored
  stale pre-manager-ref tests.
- Ran formatting:
  `cargo fmt --all` from the repository root,
  `cargo fmt --all` in `submodules/golden_alchemist_core`, and
  `cargo fmt --all` in `submodules/golden_core`.
- Ran full reusable Alchemist validation:
  `cargo test --workspace` in `submodules/golden_alchemist_core` passed with
  118 `golden_alchemist` tests and 4 `golden_statechart` tests.
- Ran full root workspace validation:
  `cargo test --workspace` passed with 288 app tests and 64 state-machine tests.
  The existing 2 ignored Alchemist tests remain ignored as stale pre-manager-ref
  behavior.

## Phase 14 - Manager Reference Bridges - Complete

- Registered StableRef value types for the Chataigne Conditions, Inputs, and
  Outputs manager bridge targets.
- Converted `ConditionsManagerRef`, `InputsManagerRef`, and
  `OutputsManagerRef` from explicit unsupported compile failures into real
  bridge ANodes with required `source` or `target` StableRef config fields.
- Kept bridge refs strict: missing, unbound, wrong value-type, or non-reference
  config reports compile diagnostics and does not emit fallback values.
- `InputsManagerRef` now reads a manager-provided ValueSet from
  `EvaluationCtx.inputs` and exposes it on its compact `values` output.
- `ConditionsManagerRef` now reads a manager-provided ValueSet and projects the
  `valid`, `on_true`, and `on_false` lanes to bool/trigger sockets.
- `OutputsManagerRef` now accepts either a ValueSet or primitive single value,
  uses an optional trigger input with a unit default, and emits
  `chataigne.command` intents toward the configured outputs-manager target.
- Kept the bridge implementation manager-agnostic: it imports no manager runtime
  code and only adapts runtime snapshots and command intents.
- Updated the app-layer formula editor diagnostic test so unconfigured manager
  refs remain invalid through the real formula materialization path.
- Ran targeted Phase 14 state-machine tests:
  `cargo test -p chataigne_state_machine alchemist -- --nocapture` passed with
  10 tests and 2 ignored stale tests.
- Ran targeted app editor diagnostic validation:
  `cargo test manager_reference_anodes_mark_formula_unavailable_in_editor_state -- --nocapture`
  passed with 1 test.
- Ran full state-machine validation:
  `cargo test -p chataigne_state_machine` passed with 70 tests and 2 ignored
  stale pre-manager-ref tests.
- Ran formatting:
  `cargo fmt --all` from the repository root,
  `cargo fmt --all` in `submodules/golden_alchemist_core`, and
  `cargo fmt --all` in `submodules/golden_core`.
- Ran full reusable Alchemist validation:
  `cargo test --workspace` in `submodules/golden_alchemist_core` passed with
  118 `golden_alchemist` tests and 4 `golden_statechart` tests.
- Ran full root workspace validation:
  `cargo test --workspace` passed with 288 app tests and 70 state-machine tests.
  The existing 2 ignored Alchemist tests remain ignored as stale
  pre-manager-ref behavior.

## Pending Tasks

- Remove duplicated Phase 15 manager-specific filter and condition evaluation
  paths now that the shared ANodes, managed filter pipeline, and manager bridge
  nodes own the reusable behavior.
- Project managed regions through the Rust protocol DTOs and generated
  TypeScript output for the Svelte editor phases.
- Keep broadcast/expand behavior explicit; `Expand` still needs a concrete
  target axis from future InputSet/OutputSet metadata before production
  lowering can safely materialize it.

## Baseline Architecture Summary

- `StateProcessorManager` is project-formula driven. It builds
  `UserCreatableItem` entries from direct `alchemist_formula` children under
  the project formula library.
- Processor creation currently uses string item IDs of the form
  `state_processor:<formula_uuid>`. `create_processor_for_formula_type`
  parses that string, creates a `StateProcessor`, and stores the formula as a
  `NodeReference`.
- `StateProcessor` stores only a project formula reference. It resolves the
  referenced formula by UUID against the process tree and requires the target
  node to be `FORMULA_NODE_TYPE`.
- Processor property UI is mirrored from the referenced formula tree:
  `processor_properties_tree` materializes exposed formula properties and
  `reconcile_formula_properties` keeps the processor's child property tree in
  sync.
- The formula library is the user/project formula tree. There is no separate
  catalog that can contain built-ins, package formulas, and project formulas
  with different visibility rules.
- Formula surface DTOs already exist in the Rust protocol and generated
  TypeScript output, which gives Phase 1 and later phases a protocol boundary
  to extend rather than inventing a parallel UI model.
- App-specific Alchemist registration lives in
  `src/state_machine/src/alchemist.rs`. At the Phase 0 baseline this still
  registered the narrow `chataigne.param_array` / `Parameter Array` value
  collection, which Phase 5 replaced with `chataigne.value_set` /
  `Value Set`.
- Existing manager-reference ANodes are intentionally incomplete at runtime:
  baseline tests assert that manager reference nodes compile as explicit
  unsupported diagnostics, and two stale pre-manager-ref runtime tests remain
  ignored.

## Important Existing Boundaries

- Processor tree nodes and project formula nodes are app-layer concerns under
  `src/state_machine_nodes`.
- Alchemist integration and Chataigne-specific value/type registration are in
  `src/state_machine/src/alchemist.rs`, not in the app shell.
- Processor runtime and preview behavior live in the reusable state-machine
  crate under `src/state_machine/src`.
- Formula UI surfaces flow through the state-machine protocol and generated
  `src-ui` DTOs.

## Formula Catalog Design

- `FormulaSourceRef` is the typed source boundary:
  `ProjectNode(NodeReference)` or
  `Builtin { package, formula_id, version }`.
- `FormulaCatalogEntry` owns source, label, description, visibility, and
  optional `ProcessorTemplateMeta`.
- `FormulaVisibility` separates formula library visibility from processor
  palette visibility, built-in duplication eligibility, and read-only
  processor inspection eligibility.
- `FormulaCatalog` is a service object, not a process-tree node. It lives in
  `src/state_machine_nodes/catalog.rs` and is consumed by processor manager
  and processor folder palette refresh.
- Built-ins are not inserted into the project formula library. They are
  catalog entries with built-in sources.
- Built-in formulas load from a shipped package asset under the app-owned
  `src/state_machine_nodes/builtin_formulas` boundary. The package owns the
  catalog metadata and formula identity for built-in `Action` and `Mapping`.

## Source Resolution Behavior

- Project formulas are cataloged from direct project formula library children,
  preserving the previous palette scope.
- Project create IDs now use the typed form
  `state_processor:project:<formula_uuid>`.
- Legacy project create IDs of the form `state_processor:<formula_uuid>`
  still parse so existing tests and transitional UI flows do not lose project
  formula creation.
- Built-in create IDs parse as
  `state_processor:builtin:<package>.<formula_id>@<version>`.
- Invalid built-in source strings and unknown built-in sources return explicit
  errors; they do not silently fall back to fake project nodes.
- Built-in processor instance creation now persists a typed built-in source
  state and clears the legacy project reference parameter.
- Built-in resolution now clones the formula definition loaded from the
  embedded package file. Unknown built-in sources still fail with an explicit
  `BuiltinFormulaNotFound` diagnostic path.

## Built-in Formula Package Format

- The initial package file is JSON and is compiled into the app with
  `include_str!`.
- The package has a package ID (`chataigne`) and a list of formula definitions.
- Each formula definition owns:
  `formula_id`, `version`, `label`, `description`, `tags`, visibility flags,
  and whether it is a processor template.
- Formula definitions can also carry normal Alchemist formula fields:
  graph, property schema, surface, context contract, and migrations.
- The initial shipped formula IDs are:
  `chataigne.action@1` and `chataigne.mapping@1`.
- The initial built-in formulas intentionally contain empty Alchemist graphs.
  Phase 4 will add managed regions; Phase 3 adds no fake runtime behavior.

## Built-in Formula Loading Path

- `FormulaCatalog::with_builtins` decodes
  `src/state_machine_nodes/builtin_formulas/chataigne.formulas.json`.
- `FormulaCatalog::from_builtin_package_source` validates and converts package
  definitions into catalog entries.
- `FormulaCatalog::resolve_builtin` returns the loaded formula definition for
  the matching built-in source.
- Built-ins remain hidden from the project formula library and visible in the
  processor palette through their package visibility metadata.

## Managed Region Model

- Managed regions are serialized authoring metadata on
  `golden_alchemist::FormulaSurface`.
- Region definitions are generic reusable formula-surface data, not
  Chataigne-specific runtime evaluators.
- Region instances live on `AlchemistFormulaInstance` as
  `ManagedRegionInstances`.
- `AlchemistFormula::instantiate` creates one empty instance for each managed
  region definition so empty built-in `Action` and `Mapping` processors are
  valid before later lowering/runtime phases exist.
- Supported Phase 4 region kinds are exactly:
  `InputSet`, `FilterPipeline`, `OutputSet`, `ActionTrigger`, and
  `ActionCommands`.
- `ConditionGate`, condition regions, and mapping-specific condition regions
  were intentionally not added.
- `ManagedSocketRef` can point a future region to formula graph sockets, but
  the Phase 4 built-ins keep sockets empty because their graphs are still
  intentionally empty.
- `ManagedRegionInstances::validate_against` reports unknown region IDs
  explicitly.

## Region Ownership

- `golden_alchemist` owns the reusable region data structures and validation
  primitive because they are formula-surface metadata.
- Chataigne owns the `Action` and `Mapping` built-in region declarations in
  `src/state_machine_nodes/builtin_formulas/chataigne.formulas.json`.
- Project-authored formula snapshots currently produce no managed regions;
  managed regions are introduced first for the shipped built-ins.

## Serialization Strategy

- `FormulaSurface.managed_regions` has a serde default so older serialized
  surfaces without managed-region metadata still deserialize as section-only
  surfaces.
- `ManagedRegionKind` serializes as snake_case for package readability, for
  example `filter_pipeline` and `action_commands`.
- Empty region instances serialize as normal instance state through
  `AlchemistFormulaInstance.managed_regions`.

## ValueSet Type Design

- `VALUE_SET_TYPE` is the app-owned Alchemist extension value type
  `chataigne.value_set`.
- `ValueSet` is a boundary collection type with:
  `entries: Vec<ValueSetEntry>` and `logical_tick: u64`.
- `ValueSetEntry` stores a stable `ValueLaneKey`, user-facing `label`,
  optional `StableRef` source, and the current `RuntimeValue`.
- `ValueLaneKey` rejects empty keys so future reorder and lane-memory work has
  a durable identity boundary instead of positional indices only.
- `ValueSet` serializes through `RuntimeValue::Extension` using a JSON payload.
  This keeps the reusable Alchemist runtime extension boundary intact while
  giving Chataigne a typed model for later InputSet and OutputSet phases.

## Renamed Symbols

- `PARAM_ARRAY_TYPE` was removed.
- `VALUE_SET_TYPE` now names the registered collection value type.
- The Inputs manager reference output socket is now `values` / `Values`.
- The Output Commands manager reference input socket is now `values` /
  `Values`.
- Manager-reference unsupported diagnostics now refer to `ValueSet`
  resolution.

## ValueSet Migration Choice

- Phase 5 intentionally rejects `chataigne.param_array` as a clean schema
  break.
- No alias, compatibility registration, or fake default conversion was added.
- A later migration phase may add an explicit project migration if saved
  projects with the old type need to be supported, but the runtime and type
  registry now use only `chataigne.value_set`.

## Phase 5 Affected Files

- `src/state_machine/src/value_set.rs`
- `src/state_machine/src/value_set_tests.rs`
- `src/state_machine/src/alchemist.rs`
- `src/state_machine/src/alchemist_tests.rs`
- `src/state_machine/src/lib.rs`
- `docs/ALCHEMIST_FORMULA_RUNTIME.md`
- `docs/ALCHEMIST_FUNCTIONAL_PLAN.md`
- `docs/implementation/chataigne_alchemist_integration_progress.md`

## Capability Metadata Design

- Capability metadata lives on `golden_alchemist::ANodeDeclaration` because
  it describes reusable node declarations, not Chataigne processor policy.
- `ANodeRoleCapability` records the managed surface role, primary sockets,
  autowire policy, pipeline cardinality, and compact/full managed UI mode.
- `AutoWirePolicy` currently supports:
  `None`, `UnaryTransform`, `Source`, `Sink`, and `Gate`.
- `PipelineCardinality` currently supports:
  `Elementwise`, `Aggregate`, `Reshape`, `Expand`, and `WholeSet`.
- `ManagedUiMode` currently supports:
  `FullGraph` and `CompactRow`.
- `ANodeRegistry::declarations_with_role(SurfaceItemKind::Filter)` is the
  discovery boundary future managers and pipeline compilers should use.

## Registered Initial Node Capabilities

- Elementwise filters:
  `function`, `remap`, `smooth_filter`, `one_minus`, `inverse`, `negate`,
  `speed`, `coordinate_system`, and `angle_conversion`.
- Aggregate filters:
  `math`.
- Reshape filters:
  `convert_to_color` and `extract_color`.
- Existing unary filters declare `AutoWirePolicy::UnaryTransform` with
  primary `value` input and `result` output.

## Temporary Compatibility Exceptions

- Dedicated `Clamp`, `MapRange`, `Math Aggregate`, `Pack Vec2`, `Pack Vec3`,
  `Select Input`, and `Broadcast` declarations do not all exist yet, so Phase
  6 did not invent placeholder nodes.
- Current `math` is registered as aggregate-capable because it already accepts
  a variable number of numeric inputs and emits one result, but later shape
  checking may refine operator-specific behavior.
- `convert_to_color` and `extract_color` are registered as reshape filters
  using their existing color/channel behavior. Dedicated pack/project nodes
  remain future work.

## Phase 6 Affected Files

- `submodules/golden_alchemist_core/crates/golden_alchemist/src/node.rs`
- `submodules/golden_alchemist_core/crates/golden_alchemist/src/registry.rs`
- `submodules/golden_alchemist_core/crates/golden_alchemist/src/lib.rs`
- `submodules/golden_alchemist_core/crates/golden_alchemist/src/library/anodes/mod.rs`
- `submodules/golden_alchemist_core/crates/golden_alchemist/src/library_tests.rs`
- `docs/implementation/chataigne_alchemist_integration_progress.md`

## ConditionGate Behavior

- `ConditionGate` is a reusable primitive Alchemist ANode, not a Mapping
  special case.
- Inputs:
  `value: TValue`, `condition: bool`, `default_value: TValue`.
- Outputs:
  `value: TValue`, `passed: bool`, `blocked: bool`.
- `PassWhenTrue` emits `value` when the condition is true, otherwise
  `default_value`.
- `PassWhenFalse` emits `value` when the condition is false, otherwise
  `default_value`.
- `HoldLast` stores the last passing value in node state and re-emits it while
  blocked; before any value has passed, it emits `default_value`.
- `OutputDefault` always emits `default_value` while blocked.
- `BlockTrigger` preserves trigger metadata while forcing `fired = false`
  when blocked.

## Gate Modes Implemented

- `PassWhenTrue`
- `PassWhenFalse`
- `HoldLast`
- `OutputDefault`
- `BlockTrigger`

## ValueSet Gating Semantics

- Phase 7 implements whole-value gating.
- App-owned `ValueSet` payloads are opaque extension values at the reusable
  Alchemist layer, so the whole extension payload is passed or blocked.
- Per-lane gating is intentionally not implemented yet. It requires the
  lane-aware ValueSet lowering work planned for later phases.

## Phase 7 Affected Files

- `submodules/golden_alchemist_core/crates/golden_alchemist/src/library/anodes/mod.rs`
- `submodules/golden_alchemist_core/crates/golden_alchemist/src/library/anodes/condition_gate.rs`
- `submodules/golden_alchemist_core/crates/golden_alchemist/src/library_tests.rs`
- `submodules/golden_alchemist_core/crates/golden_alchemist/src/runtime_tests.rs`
- `docs/implementation/chataigne_alchemist_integration_progress.md`

## Pipeline Shape Checker Design

- The checker is reusable `golden_alchemist` infrastructure, not
  Chataigne-specific processor policy.
- It accepts a linear sequence of ANode declarations plus their authored
  instances and walks from an initial `PipelineShape`.
- It discovers usable pipeline nodes through `ANodeRoleCapability` entries for
  `SurfaceItemKind::Filter`.
- Each accepted node adds a `PipelineShapeStep` recording the input shape,
  output shape, node type, and cardinality used.
- Invalid nodes add `PipelineShapeDiagnostic` entries and do not silently
  change the current shape.
- The checker does not mutate graphs or create sockets. Phase 9 lowering will
  use the trace to decide which graph edits to author.

## Pipeline Shape Transitions

- `Elementwise` preserves `Single<T>` and `ValueSet<T>`, updating the value
  type only when the node's primary output signature resolves to a concrete
  type.
- `WholeSet` preserves the complete input shape. This is how
  `ConditionGate` can gate a whole `ValueSet` without lane-aware lowering.
- `Aggregate` converts `ValueSet<T>` to `Single<T>` and preserves
  `Single<T>`.
- `Reshape` converts value shapes to `Single<Output>` using the declaration's
  primary output socket type.
- `Expand` converts `Single<T>` to `ValueSet<T>` with an unknown target axis
  until a real lowering phase supplies one.
- `Trigger`, `CommandIntent`, and `Unknown` remain explicit shapes. Non-whole
  value filters reject trigger and command-intent shapes for now.

## Phase 8 Affected Files

- `submodules/golden_alchemist_core/crates/golden_alchemist/src/lib.rs`
- `submodules/golden_alchemist_core/crates/golden_alchemist/src/pipeline.rs`
- `submodules/golden_alchemist_core/crates/golden_alchemist/src/pipeline_tests.rs`
- `docs/implementation/chataigne_alchemist_integration_progress.md`

## Phase 9 Lowering Strategy

- Phase 9 now has a reusable graph-authoring lowerer in
  `golden_alchemist::pipeline`.
- The lowerer is intentionally declaration-driven. It consumes
  `ANodeRegistry` and `ANodeRoleCapability` metadata rather than matching
  node type strings.
- Managed region instances remain authoring data. Lowering clones the shared
  formula graph into a draft, inserts the processor-instance managed ANodes,
  and returns the lowered graph only if all validation and graph edits
  succeed.
- `AlchemistFormula::materialize_with_filter_pipelines` applies ordinary
  surface overrides first, validates managed-region instances, requires an
  explicit initial `PipelineShape` for each filter region, and then lowers
  each filter region into the materialized graph.
- The original `AlchemistFormula::materialize` path remains a
  property-override-only API. This keeps registry-dependent managed lowering
  out of callers that cannot provide type and node registries.
- Linear graph autowiring currently supports declarations that expose:
  `AutoWirePolicy::UnaryTransform`, `AutoWirePolicy::Gate`, or explicit
  primary input and output sockets.
- `ConditionGate` lowers as a normal filter-capable ANode through its gate
  autowire metadata.
- Shape-changing or lane-wise `ValueSet` transitions are detected from the
  Phase 8 shape trace before reusable graph mutation. Elementwise, aggregate,
  reshape, and expand transitions involving `ValueSet` still return reusable
  diagnostics instead of lowering to invalid scalar wiring.
- Lowering diagnostics are typed with `PipelineLoweringDiagnosticKind`; this
  is the stable reusable boundary for app-owned `ValueSet` lane strategies and
  UI diagnostics.
- Disabled managed items are not inserted into the executable graph.
- Chataigne's app-owned lane runtime handles `ValueSet` elementwise map
  semantics by compiling one scalar graph and evaluating entries through
  lane-specific `ContextKey`s.
- Stateful lane filters use `LaneRuntimePool`, so each live value lane owns
  independent Alchemist memory while inactive lanes can be retained or dropped
  without recompiling the graph.
- Aggregate and pack/projection semantics are explicit fixed-slot whole-set
  projections. This matches the upcoming InputSet model, where selected inputs
  define stable lane order before a projection node evaluates.

## Phase 9 Current Limitations

- Built-in `Action` and `Mapping` package definitions still have empty graph
  boundaries. InputSet/OutputSet phases must supply concrete boundary sockets
  before the built-ins can call the reusable lowerer and app-owned lane
  runtimes end to end.
- `materialize_with_filter_pipelines` requires callers to provide the initial
  shape for each filter pipeline. Automatic shape derivation from boundary
  sockets remains deferred until the built-ins expose concrete typed boundary
  nodes.
- The app-owned projection runtime is fixed-slot by design. Dynamic arbitrary
  lane aggregation remains deferred until InputSet metadata can define stable
  selected lane order.
- Expand/broadcast to new `ValueSet` axes remains explicit future work.

## Phase 9 Affected Files

- `submodules/golden_alchemist_core/crates/golden_alchemist/src/lib.rs`
- `submodules/golden_alchemist_core/crates/golden_alchemist/src/compile.rs`
- `submodules/golden_alchemist_core/crates/golden_alchemist/src/compile_tests.rs`
- `submodules/golden_alchemist_core/crates/golden_alchemist/src/formula.rs`
- `submodules/golden_alchemist_core/crates/golden_alchemist/src/formula_tests.rs`
- `submodules/golden_alchemist_core/crates/golden_alchemist/src/library/anodes/mod.rs`
- `submodules/golden_alchemist_core/crates/golden_alchemist/src/library/anodes/pack_vec3.rs`
- `submodules/golden_alchemist_core/crates/golden_alchemist/src/library_tests.rs`
- `submodules/golden_alchemist_core/crates/golden_alchemist/src/pipeline.rs`
- `submodules/golden_alchemist_core/crates/golden_alchemist/src/pipeline_tests.rs`
- `src/state_machine/src/lib.rs`
- `src/state_machine/src/value_set_pipeline.rs`
- `src/state_machine/src/value_set_pipeline_tests.rs`
- `docs/implementation/chataigne_alchemist_integration_progress.md`

## Phase 10 InputSet Source Model

- InputSet is implemented in the app-owned state-machine crate, not in
  reusable `golden_alchemist`, because it materializes Chataigne runtime input
  sources into the app-owned `ValueSet` type.
- The first supported source model is an authored `StableRef` stored in each
  managed input item's `source` config field.
- At evaluation time, `InputSetRuntime` resolves each enabled source through
  `EvaluationCtx.inputs`.
- The materializer returns a `ValueSet` plus diagnostics. It does not insert
  placeholder values for unavailable sources.
- This phase deliberately does not alter `InputsManagerRef`; manager-reference
  bridge ANodes remain unsupported until the manager bridge phase.

## Phase 10 Stable Lane Key Strategy

- Managed input items use the persisted `ManagedItemId` as the lane identity.
- The `ValueLaneKey` format is `input:<managed_item_uuid>`.
- Reordering input items changes output order but preserves each item's lane
  key, allowing later lane memory and projection code to remain stable.
- Each `ValueSetEntry` also carries the original `StableRef` as `source`, so
  dispatch, diagnostics, and future UI can still show the selected endpoint.

## Phase 10 Supported Sources

- Runtime input snapshot values keyed by `StableRef`.
- StableRef-backed Chataigne module endpoint references are the intended first
  concrete source type.

## Phase 10 Unsupported Sources

- Dynamic discovery of all sources under a context axis.
- Input fallback/default values for missing sources.
- Direct manager-reference ANode evaluation.
- Output dispatch and end-to-end Mapping execution.

## Phase 10 Affected Files

- `src/state_machine/src/lib.rs`
- `src/state_machine/src/input_set.rs`
- `src/state_machine/src/input_set_tests.rs`
- `docs/implementation/chataigne_alchemist_integration_progress.md`

## Phase 11 OutputSet Intent Model

- OutputSet is implemented in the app-owned state-machine crate because it
  turns Chataigne-managed output targets into Chataigne command intents.
- OutputSet does not perform module IO.
- Each emitted intent uses kind `chataigne.command`, target from the authored
  `StableRef`, the evaluated payload value, and the current logical tick.
- The existing state-machine path already converts `RuntimeIntent` values into
  `CommandIntent` values and arbitrates them before dispatch.
- Managed output items use a `target` StableRef config field.

## Phase 11 Output Semantics

- No enabled outputs produces no intents and no diagnostics.
- A single non-ValueSet value requires exactly one enabled output.
- A single value with multiple enabled outputs is rejected with
  `output_set_single_value_requires_single_output`; this avoids hidden
  broadcasting.
- A `ValueSet` value requires the same number of entries and enabled outputs.
- ValueSet entries are zipped to enabled outputs in authored order and emit
  one command intent per entry.
- Idle trigger values emit no intent, allowing trigger gates to block command
  output cleanly.
- ValueSet/output count mismatch is rejected with
  `output_set_valueset_output_mismatch` and emits no partial intents.

## Phase 11 Dispatch Boundary

- OutputSet stops at `RuntimeIntent`.
- It does not access modules, transports, connection state, or reconnect
  behavior.
- Command dispatch remains in the existing Chataigne command dispatcher path
  after arbitration.

## Phase 11 Unsupported Cases

- Output formatting/transforms belong in filter or output-specific nodes and
  were not added in this phase.
- Command draft expansion remains future work; current OutputSet treats the
  evaluated payload as the command payload.
- Dynamic output target discovery is not implemented.
- Built-in Mapping orchestration is still pending.

## Phase 11 Affected Files

- `src/state_machine/src/lib.rs`
- `src/state_machine/src/output_set.rs`
- `src/state_machine/src/output_set_tests.rs`
- `docs/implementation/chataigne_alchemist_integration_progress.md`

## Known Missing UI Integration

- Managed regions are not yet exposed through the Rust protocol DTOs or
  generated TypeScript output.
- Processor UI still uses the existing formula property surface; Phase 16 will
  project managed regions into the Svelte surfaces.
- Phase 11 has reusable graph-materialization plus app-owned InputSet,
  filter/projection, and OutputSet runtime building blocks, but built-in
  Mapping/Action still need orchestration and protocol/UI projection before
  they can execute end to end.

## Processor Formula Reference Migration

- `StateProcessor` now persists `ProcessorFormulaSourceState`.
- New project processors store a project source state and still mirror the
  project formula into the existing `Formula` reference parameter.
- New built-in processors store a built-in source state and keep the legacy
  `Formula` reference empty.
- Existing project processors with only a legacy `Formula` reference continue
  to resolve through a fallback path.
- Formula reference parameter edits resync the typed source state to a project
  source.

## Creation Item Protocol

- Project formula creation uses
  `state_processor:project:<formula_uuid>`.
- Built-in formula creation uses
  `state_processor:builtin:chataigne.action@1` and
  `state_processor:builtin:chataigne.mapping@1`.
- Legacy `state_processor:<formula_uuid>` parsing remains as a narrow
  transition path for old project formula creation strings.

## Blocking Design Issues

- Built-in `Action` and `Mapping` can create source-backed processor
  instances and resolve through shipped package definitions, but their graphs
  are intentionally empty until Phase 4 adds managed regions.
- The processor palette and creation path still bridge through
  `UserCreatableItem` string IDs at the UI/node boundary.
- Processor creation is parsed from ad-hoc strings at the node creation
  boundary and then immediately loses source information by storing only a
  project node reference.
- Phase 5 renamed the value collection boundary to `ValueSet`, and Phase 8
  added a checker for pipeline shapes, but no InputSet, OutputSet, or pipeline
  lowering behavior exists yet.
- Manager references and manager-specific condition/filter concepts still
  exist beside Alchemist ANodes, so later phases must remove duplicated
  evaluation paths rather than layering more runtime branches onto them.

## Files Expected To Change

- `src/state_machine_nodes/processor.rs`
- `src/state_machine_nodes/catalog.rs`
- `src/state_machine_nodes/builtin_formulas/chataigne.formulas.json`
- `src/state_machine_nodes/formula.rs`
- `src/state_machine/src/alchemist.rs`
- `src/state_machine/src/protocol.rs`
- `src/state_machine/src/processor.rs`
- `src/state_machine/src/value_set.rs`
- `src/state_machine_nodes/manager.rs`
- `src/state_machine_nodes/conditions/`
- `src/state_machine_nodes/managed_nodes/`
- `src-ui/src/lib/state_machine/generated/`
- `src-ui/src/lib/state_machine/components/`
- `docs/implementation/chataigne_alchemist_integration_progress.md`

## Design Decisions Made

- Phase 0 stayed documentation and baseline validation only.
- Built-ins must not be faked as hidden project formula nodes. The next code
  change needs a typed source/catalog boundary before processor creation is
  modified.
- The current checkout does not expose a standalone `golden_core` Cargo
  package. Until that package exists here, root workspace formatting is the
  applicable Rust formatting pass.
- Phase 1 deliberately keeps actual built-in processor creation for Phase 2.
  Creating those processors before `StateProcessor` can store
  `FormulaSourceRef` would put built-in state in the wrong field.
- Phase 2 uses a persisted source-state model while retaining the existing
  project `Formula` reference parameter as a transitional UI/project-formula
  mirror.
- Phase 3 keeps Chataigne built-in package files in the app layer because
  `Action` and `Mapping` are product-owned formulas, not reusable
  `golden_*` package policy.
- Phase 3 uses a compile-time included JSON package. That gives deterministic
  startup, keeps desktop/headless behavior identical, and avoids a host/runtime
  file-discovery dependency before package management exists.
- Phase 3 ships empty graph definitions for built-ins instead of adding
  shortcut evaluators. Managed regions and lowering remain later phases.
- Phase 4 stores managed region definitions on `FormulaSurface`, because they
  are authoring metadata for formula surfaces and should not create a separate
  app-only side table.
- Phase 4 stores instance region state on `AlchemistFormulaInstance`, keeping
  future per-processor managed items with the formula instance rather than the
  shared built-in formula definition.
- Phase 4 deliberately does not add `ConditionGate` to the region kind list.
  Conditions will enter as filter-capable ANodes in a later phase.
- Phase 4 does not expose managed regions in the protocol yet. The backend
  model and built-in package format are now durable enough for later UI work,
  but Phase 16 owns Svelte projection and interaction design.
- Phase 5 keeps `ValueSet` app-owned and encoded as a
  `RuntimeValue::Extension` payload instead of adding a reusable Alchemist
  primitive. Later phases can decide whether generic collection/lane helpers
  belong in `golden_alchemist`, but the Chataigne type itself remains product
  owned.
- Phase 5 intentionally rejects the old `chataigne.param_array` runtime type
  rather than registering a compatibility alias. This matches the clean schema
  break stance until an explicit migration phase requires otherwise.
- Phase 6 keeps capability metadata on reusable ANode declarations. Chataigne
  processor managers should consume these capabilities rather than duplicating
  filter/action/input/output node lists.
- Phase 6 deliberately registers only capabilities for nodes that exist now.
  Future nodes must declare their own capabilities when they are added.
- Phase 7 keeps `ConditionGate` generic and reusable. Mapping, Action, and
  custom formulas will all use it through normal ANode capability discovery.
- Phase 7 implements whole-value gating first because per-lane `ValueSet`
  semantics require later lane-aware lowering work.
- Phase 8 keeps the pipeline checker declaration-driven and reusable. Chataigne
  lowering code should consume its diagnostics instead of duplicating shape
  policy in the app layer.
- Phase 8 uses shape-only test declarations for Pack Vec3 and Broadcast
  scenarios. This proves checker behavior without adding fake production
  ANodes before the catalog actually owns those nodes.

## Migration Notes

- Current processor creation IDs are `state_processor:<uuid>` and current
  processors persist project formula references as node references. Phase 2
  needs to decide whether to migrate those references or intentionally make a
  clean schema break.
- Phase 1 accepts both legacy `state_processor:<uuid>` and typed
  `state_processor:project:<uuid>` project source strings. This is a narrow
  transitional parser, not a long-term compatibility policy.
- Phase 2 does not force-migrate existing project processors. Empty source
  state plus a non-empty legacy formula reference resolves as a project source.
- The old value collection runtime type `chataigne.param_array` is not
  registered after Phase 5. `ValueSet::from_runtime_value` reports it as the
  wrong runtime type.
- `FormulaSurface.managed_regions` uses a serde default, so existing formula
  surfaces without the field deserialize as empty managed-region surfaces.
- Phase 14 introduces stable ref value types for manager bridges. Existing
  unconfigured manager ref ANodes intentionally stay invalid until a real
  manager source or target is selected.

## Known Risks

- The implementation plan spans formula catalog, processor references,
  managed regions, value collection typing, runtime lowering, and UI. Phase
  ordering must keep each step buildable.
- Catalog source modeling is the critical first code boundary. If it lands in
  the wrong layer, later built-in loading and UI visibility will duplicate
  policy.
- Large graph performance tests already exist and pass. Later managed-region
  and pipeline-lowering work must preserve sparse lane memory and shared
  compiled formula behavior.
- Unknown built-in source strings can still instantiate a processor if they
  are manually passed through the creation boundary; the processor surfaces an
  explicit missing-source warning. Package loading now owns the complete
  shipped built-in list, but the creation boundary still accepts parsed
  built-in source strings so diagnostics remain the guardrail until later
  hardening.
- Empty built-in graphs are valid for Phase 3 but not user-complete. Phase 4
  must add managed region definitions before the built-in surfaces become
  useful.
- Managed regions can lower to executable filter-pipeline graph behavior, but
  the built-in processors remain structurally valid rather than end-to-end
  useful until OutputSet and orchestration phases provide the remaining
  boundaries.
- Protocol/UI projection of managed regions is deferred, so frontend surfaces
  cannot use the new metadata yet.
- `ValueSet` has a typed payload model plus Phase 10 InputSet and Phase 11
  OutputSet materialization, but built-in Mapping does not orchestrate them
  end to end yet.
- Capability metadata now feeds both the Phase 8 pipeline shape checker and
  the Phase 9 managed-region lowering path.
- The primitive capability set remains intentionally conservative. Clamp and
  Pack Vec3 are now production ANodes; broadcast/select behavior remains
  deferred rather than faked.
- Per-lane ConditionGate mode is a declared config value but currently returns
  an explicit runtime diagnostic if selected. This avoids hidden fallback
  behavior before lane-aware ValueSet lowering exists.
- The Phase 8 checker validates pipeline shape transitions, not full graph
  type solving. Socket-level type compatibility remains the responsibility of
  the existing Alchemist type solver and Phase 9 lowering/materialization.
- `Expand` currently produces a `ValueSet` with an unknown axis. A later phase
  must choose the target axis explicitly when it lowers real broadcast regions.
- `materialize_with_filter_pipelines` requires explicit initial shapes from
  the caller. That keeps hidden type inference out of the materialization path
  until real built-in boundary nodes can provide typed sockets.
- Formula-level managed lowering failures now preserve both the reusable
  lowering diagnostics and the shape-checker diagnostics.
- The first Phase 9 lowerer deliberately rejects aggregate/reshape/expand
  nodes that do not expose linear autowire sockets. This avoids implicit
  merge/broadcast behavior until lane-aware lowering makes those transitions
  explicit.
- The first Phase 9 lowerer also rejects `ValueSet` elementwise lowering for
  scalar filters such as Remap. This prevents the graph from wiring a whole
  `ValueSet` extension value into a scalar socket while still allowing
  whole-set filters like `ConditionGate`.
- Phase 9 now has graph-authoring tests and app runtime tests for the
  lane-aware `ValueSet` execution strategy.
- Phase 14 bridges deliberately read manager outputs from `EvaluationCtx.inputs`
  rather than importing manager runtime code. That keeps the ANodes as
  IO-boundary adapters and avoids duplicated manager logic inside the graph
  evaluator.
- Phase 14 models `OutputsManagerRef` as a command-intent emitter targeting an
  outputs-manager StableRef. The command dispatcher remains the only boundary
  that should translate those intents into concrete module IO.

## Tests Added

- `builtin_formula_sources_parse_and_resolve`
- `invalid_builtin_formula_source_fails_cleanly`
- `builtins_are_not_formula_library_items`
- Updated `processor_manager_lists_custom_formulas` to assert catalog-backed
  built-ins plus the existing project formula entry.
- `processor_created_from_typed_project_item_keeps_source_and_reference`
- `processor_created_from_builtin_mapping_item_keeps_source_without_project_reference`
- `processor_formula_source_state_serializes_builtin_source`
- `builtin_mapping_processor_has_no_missing_formula_warning`
- `builtin_formula_package_loads_action_and_single_mapping`
- `managed_region_kind_roundtrips_through_json`
- `empty_managed_regions_are_instantiated_from_surface`
- `invalid_managed_region_reference_reports_diagnostic`
- `builtin_formulas_expose_empty_managed_regions`
- `value_set_constructs_with_stable_lane_keys`
- `value_set_rejects_empty_lane_keys`
- `value_set_roundtrips_through_runtime_extension_payload`
- `old_parameter_array_runtime_type_is_not_accepted_as_valueset`
- `valueset_type_is_registered_as_extension_without_legacy_alias`
- `manager_reference_sockets_expose_valueset`
- `filter_capable_node_discovery_is_declaration_driven`
- `non_filter_node_has_no_filter_capability`
- `primary_socket_autowiring_is_declared_for_unary_filters`
- `clamp_signature_is_declared`
- `pack_vec3_signature_is_declared`
- `capability_metadata_roundtrips_through_json`
- `condition_gate_declares_filter_gate_capability`
- `condition_gate_true_condition_passes_value`
- `condition_gate_false_condition_blocks_value`
- `condition_gate_hold_last_outputs_previous_passed_value`
- `condition_gate_output_default_uses_default_input`
- `condition_gate_block_trigger_suppresses_fired_edge`
- `condition_gate_whole_valueset_gate_uses_default_whole_value`
- `elementwise_filter_preserves_valueset_shape`
- `aggregate_filter_collapses_valueset_to_single`
- `reshape_filter_can_pack_valueset_items_to_vec3`
- `checker_rejects_nodes_without_filter_capability`
- `condition_gate_preserves_pipeline_shape`
- `expand_filter_broadcasts_single_value_to_valueset`
- `lowering_autowires_enabled_filter_items_into_graph`
- `lowering_skips_disabled_filter_items`
- `lowering_rejects_non_filter_items_without_mutating_graph`
- `lowering_requires_linear_autowire_sockets`
- `lowering_rejects_valueset_elementwise_until_lane_strategy_exists`
- `lowering_allows_whole_valueset_filters`
- `materialize_with_filter_pipelines_lowers_managed_filter_items`
- `materialize_with_filter_pipelines_requires_initial_shape`
- `materialize_with_filter_pipelines_rejects_valueset_elementwise_without_lane_strategy`
- `remap_clamp_chain_maps_each_lane`
- `smooth_filter_keeps_independent_lane_memory`
- `aggregate_reduces_multiple_lanes_to_one_value`
- `pack_vec3_projects_three_lanes_to_vector`
- `elementwise_remap_preserves_lanes_and_values`
- `whole_set_condition_gate_can_run_per_lane_with_defaults`
- `single_input_materializes_valueset_entry`
- `multiple_inputs_materialize_in_authored_order`
- `input_reorder_preserves_lane_identity`
- `disabled_input_is_excluded`
- `missing_input_reports_diagnostic_without_fake_value`
- `single_value_output_creates_expected_intent`
- `valueset_output_creates_per_entry_intents`
- `idle_trigger_output_creates_no_intent`
- `single_value_with_multiple_outputs_reports_diagnostic_without_broadcast`
- `valueset_output_count_mismatch_reports_diagnostic_without_partial_dispatch`
- `disabled_output_is_excluded`
- `manager_reference_nodes_require_configured_bridge_refs`
- `input_manager_bridge_exposes_valueset_from_runtime_source`
- `input_manager_bridge_missing_runtime_source_emits_no_fallback_sample`
- `condition_manager_bridge_exposes_bool_and_trigger_lanes`
- `output_manager_bridge_emits_command_intent_with_optional_trigger`
- `output_manager_bridge_emits_valueset_payload`
- `output_manager_bridge_suppresses_idle_trigger`
- Updated `manager_reference_anodes_mark_formula_unavailable_in_editor_state`
  to assert bridge diagnostics instead of unsupported-node diagnostics.

## Supercommit History

- Completed:
  `f4b9888 supercommit: chataigne alchemist integration phase 0 - baseline audit`
- Completed:
  `a6086e0 supercommit: chataigne alchemist integration phase 1 - formula catalog`
- Completed:
  `4fff469 supercommit: chataigne alchemist integration phase 2 - processor formula sources`
- Completed in the current supercommit:
- Completed:
  `1ecd2c7 supercommit: chataigne alchemist integration phase 3 - builtin formulas`
- Completed in the current supercommit:
  `supercommit: chataigne alchemist integration phase 4 - managed regions`
- Reusable Alchemist submodule commit:
  `06a7712 supercommit: chataigne alchemist integration phase 4 - managed regions`
- Completed in the current supercommit:
  `supercommit: chataigne alchemist integration phase 5 - value set`
- Completed in the current supercommit:
  `supercommit: chataigne alchemist integration phase 6 - anode capabilities`
- Reusable Alchemist submodule commit:
  `28beff1 supercommit: chataigne alchemist integration phase 6 - anode capabilities`
- Completed in the current supercommit:
  `supercommit: chataigne alchemist integration phase 7 - condition gate anode`
- Reusable Alchemist submodule commit:
  `6089db0 supercommit: chataigne alchemist integration phase 7 - condition gate anode`
- Completed in the current supercommit:
  `supercommit: chataigne alchemist integration phase 8 - pipeline shape checker`
- Reusable Alchemist submodule commit:
  `96437b6 supercommit: chataigne alchemist integration phase 8 - pipeline shape checker`
- Completed in the current supercommit:
  `supercommit: chataigne alchemist integration phase 9 - filter pipeline lowering`
- Reusable Alchemist submodule commit:
  `5b1fca2 supercommit: chataigne alchemist integration phase 9 - filter pipeline lowering`
- Completed in the current supercommit:
  `supercommit: chataigne alchemist integration phase 10 - input set region`
- Completed in the current supercommit:
  `supercommit: chataigne alchemist integration phase 11 - output set region`
- Completed:
  `49751b1 supercommit: chataigne alchemist integration phase 12 - builtin mapping`
- Reusable Alchemist submodule commit:
  `dd95244 support: make alchemist registries cloneable`
- Completed:
  `099715a supercommit: chataigne alchemist integration phase 13 - builtin action`
- Completed in the current supercommit:
  `supercommit: chataigne alchemist integration phase 14 - manager bridges`
