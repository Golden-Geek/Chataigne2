# Built-in Mapping correction map

This map records the implementation at corrective baseline `a94b9a7e`. It is
the starting point for phases R00–R08 in
`docs/plan/builtin-mapping-codex-plan.md`; it does not describe the desired
final authoring contract.

## Current authored hierarchy

A built-in Formula is loaded from `apps/chataigne/resources/formulas/builtin`
by the app-owned Formula catalog. Formula property managers declare processor
surface roles. `integration/processor/surface.rs` materializes those roles as
ordinary `InputsManager`, `FilterChainManager`, `ConditionManager`, or
`OutputsManager` children of each `StateProcessor`.

Mapping currently has a second hierarchy under the same processor:

```text
StateProcessor
├── Inputs / Filters / Outputs       ordinary exposed surface managers
└── StateProcessorManagedRegions     hidden authoritative authoring root
    ├── inputs                       ANode Input Source items
    ├── filters                      ANode filter items
    └── outputs                      ANode OutputTarget items
```

`processor_managed_regions_tree` in `integration/processor/mod.rs` sets the
managed root's `show_in_inspector_content` flag to false. The region nodes own
the creation palettes. `integration/processor/managed_regions.rs` extracts
only children whose node type is `alchemist_anode`; all other children are
ignored. The visible surface managers and hidden managed regions therefore do
not form one authoritative editable tree.

`ProcessorFormulaInspector.svelte` delegates tuple-managed processors to
`MappingInspector.svelte`. That component inventories and edits managed items
from preview-catalog DTOs and uses `MappingRegionList.svelte`; ordinary child
rendering is only its fallback. Opening the inspector is not required to
materialize the backend tree, but the normal inspector is not the default
Mapping authoring surface.

## Action command path

The shipped Action asset is
`apps/chataigne/resources/formulas/builtin/Action.json`. Its Formula surface
declares Conditions, On True, and On False managers. Processor surface
materialization creates real `ConditionManager` and `OutputsManager` nodes.

`OutputsManager` in `integration/managed_nodes/mod.rs` enumerates the generic
command registry, module command catalogs, and `OutputGroup`. A module command
catalog entry carries an initial module-target reference, so creating it
produces a concrete owned command with its normal target and argument subtree.
The manager caches concrete output descendants and passes triggers to
`OutputSchedule` in `integration/managed_nodes/schedule.rs`. That scheduler
preserves delay, stagger, cancellation, occurrence identity, typed parameter
overrides, and bounded batches, then emits the existing module-command execute
event to the concrete command node. Generic and module command nodes consume
the same execute contract in their app-owned implementations.

`ConsequencesManager` is a separate older `sm_consequence` registry container;
it does not use the `OutputsManager` catalog. R02 must consolidate shared
command responsibilities deliberately rather than treating their labels as an
already unified implementation.

## Current Mapping execution path

Mapping's file-authored surface metadata declares input, filter, and output
managed regions. The processor snapshot adapter turns region ANode children
into `ManagedRegionInstances` and uses each ANode UUID as its stable
`ManagedItemId`. The processor crate lowers the ordered Input Source values to
one scalar or heterogeneous tuple, compiles the linear filter chain, and lowers
OutputTarget adapters into validated command intents. The manager schedules
dirty and temporal work, retains state by processor/context/item identity, and
dispatches through the queued engine command boundary.

The runtime pieces to preserve are:

- typed scalar/tuple layouts, projections, reductions, packing, extraction,
  conversion, suppression, and trigger occurrence flow;
- direct result slots, optional preview capture, sparse scheduling, shared
  executable specializations, and isolated temporal/context state;
- typed command argument validation and the existing queued command boundary;
- `OutputsManager`/`OutputGroup` scheduling and concrete generic/module command
  execution.

The authoring boundaries to adapt are:

- processor surface and managed-region construction/reconciliation;
- managed snapshot extraction and command lowering;
- filter catalog creatability versus applicability and node-local warnings;
- typed command argument controls and persisted migration;
- standard inspector composition and optional preview supplements;
- Mapping-to-Formula conversion and later frozen-source compression.

The final correction removes these normal-path concepts after their data is
migrated:

- the hidden authoritative managed tree beside visible surface managers;
- Mapping-only `OutputTarget` command-reference adapters and opaque `bindings`
  JSON as the editing interface;
- compile-dependent filter creation menus and backend shape rejection;
- the full Mapping-specific region editor and parallel selection/undo state.

## Identity and persistence inventory

The migration must preserve processor UUIDs, managed region role IDs, authored
ANode UUIDs, stable tuple element identities, Formula source references,
context identities, and references to nested filter/resource parameters.
Visible processor surface declaration IDs currently derive from Formula
property source UUIDs. Hidden region declaration IDs derive from stable Formula
managed-region IDs. R01 must move existing authored item subtrees into the
ordinary role managers without cloning them or allocating replacement UUIDs.

Existing Mapping output adapters contain a referenced command UUID and a
serialized binding document. R02 owns their typed migration. A referenced
external command must not be stolen or deleted; ambiguous adapters must retain
their data with an attributed migration warning. Existing migration code lives
in `integration/formula/binding_migration.rs`, while processor sparse
persistence, duplication, and conversion coverage lives under
`integration/processor/tests`.

The shipped `Mapping.json` and `Action.json` assets, sparse project fixtures,
old OutputTarget binding documents, converted custom Formula fixtures, copied
commands, and external references are the persisted inputs that require
requalification. Missing representation metadata will default to expanded,
editable nodes when compression is introduced.

## Test ownership

- App hierarchy, creation, migration, persistence, conversion, and backend
  intent tests: `integration/processor/tests`.
- Shared command catalog, scheduling, groups, and invocation behavior:
  `integration/managed_nodes/tests`, generic-command tests, module-command
  tests, and state-machine command-dispatch tests.
- Typed Formula runtime and filter semantics: `chataigne_alchemist` and
  `chataigne_processor` test directories.
- Ordinary inspector composition and bounded preview supplements:
  `apps/chataigne/ui/src/lib/systems/alchemist/tests` plus Golden UI tests.
- Compression state machine, frozen source, dependency preflight, and
  equivalence: app-owned processor/runtime tests introduced in R06–R07.
- Product, persistence, scale, package, and CI evidence: R05 and R08 status
  rows in `docs/progress/builtin-mapping-status.md`.

R00 adds backend characterization for the current hidden-versus-visible
boundary and a preview-independent tuple Sum delivered through the queued
engine path. Existing M01–M20 results remain historical evidence for runtime
behavior only; N01–N24 start unqualified under the corrected contract.
