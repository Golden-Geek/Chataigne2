# Chataigne Alchemist Integration Progress

## Current Phase

Phase 4 - Managed region model is next. No Phase 4 implementation has started
yet.

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

## Pending Tasks

- Start Phase 4 by introducing the managed region model for built-in Mapping
  and Action surfaces.
- Define serializable managed region kinds for `InputSet`, `FilterPipeline`,
  `OutputSet`, `ActionTrigger`, and `ActionCommands`.
- Keep `ConditionGate` out of the managed region kind list; it belongs in the
  filter-capable ANode model in later phases.

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
  `src/state_machine/src/alchemist.rs`. It registers Chataigne value facets,
  including `PARAM_ARRAY_TYPE = "chataigne.param_array"` with the user-facing
  label `Parameter Array`.
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
- `chataigne.param_array` and the `Parameter Array` label are too narrow for
  the planned input/filter/output value collection boundary.
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
- Current value collection naming is `chataigne.param_array`; Phase 5 needs
  to either migrate that persisted type or explicitly reject old data.

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

## Supercommit History

- Completed:
  `f4b9888 supercommit: chataigne alchemist integration phase 0 - baseline audit`
- Completed:
  `a6086e0 supercommit: chataigne alchemist integration phase 1 - formula catalog`
- Completed:
  `4fff469 supercommit: chataigne alchemist integration phase 2 - processor formula sources`
- Completed in the current supercommit:
  `supercommit: chataigne alchemist integration phase 3 - builtin formulas`
