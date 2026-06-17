# Chataigne Alchemist Integration Progress

## Current Phase

Phase 1 - Formula catalog and built-in formula sources is complete and
ready for the Phase 1 supercommit.

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

## Pending Tasks

- Complete the Phase 1 supercommit.
- Start Phase 2 by replacing `StateProcessor`'s project-only formula
  reference with a typed formula source selection.
- Make built-in `Action` and `Mapping` processor creation produce valid
  source-backed processor instances. Phase 1 intentionally stops at catalog
  visibility and built-in formula resolution because `StateProcessor` cannot
  yet persist a built-in source.

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
- Built-in formulas currently resolve to empty placeholder
  `AlchemistFormula` values with built-in metadata. Phase 3 replaces this
  placeholder with shipped built-in formula package loading.

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
- Actual built-in processor instance creation is deferred to Phase 2 because
  the current `StateProcessor` data model only stores a project
  `NodeReference`.

## Blocking Design Issues

- Built-in `Action` and `Mapping` are represented as catalog entries, but
  processor instances still cannot persist them until Phase 2 replaces the
  project-only formula reference model.
- The processor palette is catalog-driven, but processor creation still has
  to bridge through `UserCreatableItem` string IDs at the UI/node boundary.
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

## Migration Notes

- Current processor creation IDs are `state_processor:<uuid>` and current
  processors persist project formula references as node references. Phase 2
  needs to decide whether to migrate those references or intentionally make a
  clean schema break.
- Phase 1 accepts both legacy `state_processor:<uuid>` and typed
  `state_processor:project:<uuid>` project source strings. This is a narrow
  transitional parser, not a long-term compatibility policy.
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
- Until Phase 2 lands, built-in Action/Mapping palette entries identify valid
  catalog sources but do not yet create valid processor instances.

## Tests Added

- `builtin_formula_sources_parse_and_resolve`
- `invalid_builtin_formula_source_fails_cleanly`
- `builtins_are_not_formula_library_items`
- Updated `processor_manager_lists_custom_formulas` to assert catalog-backed
  built-ins plus the existing project formula entry.

## Supercommit History

- Completed:
  `f4b9888 supercommit: chataigne alchemist integration phase 0 - baseline audit`
- Pending commit:
  `supercommit: chataigne alchemist integration phase 1 - formula catalog`
