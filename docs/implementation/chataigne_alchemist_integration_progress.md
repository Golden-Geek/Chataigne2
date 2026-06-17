# Chataigne Alchemist Integration Progress

## Current Phase

Phase 0 - Repository orientation and baseline audit is complete and ready
for the Phase 0 supercommit.

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

## Pending Tasks

- Complete the Phase 0 supercommit.
- Start Phase 1 by introducing a formula catalog/source model before changing
  processor references.

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

## Blocking Design Issues

- Built-in `Action` and `Mapping` cannot be represented cleanly because the
  current processor formula reference model accepts only project
  `NodeReference` targets.
- The processor palette is currently derived from the user formula library,
  so visibility cannot distinguish library entries from processor-creatable
  catalog entries.
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

- Phase 0 stays documentation and baseline validation only. Catalog/runtime
  changes start in Phase 1 after this clean baseline supercommit.
- Built-ins must not be faked as hidden project formula nodes. The next code
  change needs a typed source/catalog boundary before processor creation is
  modified.
- The current checkout does not expose a standalone `golden_core` Cargo
  package. Until that package exists here, root workspace formatting is the
  applicable Rust formatting pass.

## Migration Notes

- Current processor creation IDs are `state_processor:<uuid>` and current
  processors persist project formula references as node references. Phase 2
  needs to decide whether to migrate those references or intentionally make a
  clean schema break.
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

## Tests Added

- None in Phase 0. Baseline validation used existing tests.

## Supercommit History

- Pending commit:
  `supercommit: chataigne alchemist integration phase 0 - baseline audit`
