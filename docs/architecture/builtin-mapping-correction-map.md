# Built-in Mapping correction map

This map records the implementation at corrective baseline `a94b9a7e` and the
boundary changes completed by later corrective phases. The baseline is the
starting point for phases R00–R08 in
`docs/plan/builtin-mapping-codex-plan.md`; it does not describe the desired
final authoring contract.

## Corrective baseline hierarchy

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

## R01 resulting hierarchy

R01 replaces that baseline with one authored processor tree:

```text
StateProcessor
├── Inputs                         visible managed region
│   └── Input Source              ordinary ANode item and typed controls
├── Filters                        visible managed region
│   └── Filter                    ordinary ANode item and descendants
└── Outputs                        visible managed region
    └── OutputTarget              transitional ANode item until R02
```

Factories create each managed region directly under `StateProcessor` and omit
Formula property managers whose roles would duplicate those regions. Runtime
extraction, conversion, palette refresh, state-machine dirty tracking, context
lookup, and debug-node discovery consume the direct regions. The hidden
`StateProcessorManagedRegions` type remains registered only so old persisted
snapshots can decode and migrate.

Migration moves each legacy region rather than cloning it, moves any children
from a duplicate role manager into the matching region, and removes the legacy
root and duplicate manager. Processor, region, ANode, and nested control UUIDs
therefore remain stable, including during sparse load before the built-in
Formula asset is available. Fresh and migrated Mappings expose the same tree to
snapshot lookup, outliner, inspector, references, scripts, and headless edits.

`ProcessorFormulaInspector.svelte` now always composes the standard child
renderer. Preview controls remain optional header data and no longer provide
the authored-item inventory. Mapping-specific OutputTarget adapters are
accepted only as transitional R01 data; R02 replaces them with concrete shared
command children.

## R02 resulting command boundary

Action and Mapping now use the same registered generic and module command
catalogs and the same concrete command factories. A Mapping Outputs region
creates the selected command or output group directly. Each concrete Mapping
command also owns a `MappingCommandBindings` child whose ordinary controls
select the whole result, a stable tuple element, a component, or a typed
constant for delivery and command-argument overrides. The filtered result may
therefore remain an ordered heterogeneous tuple, reduce several sources to one
value, or pack X/Y/Z into one Vec3 command argument without a channel model.

The processor snapshot adapter lowers those concrete command descendants to
the domain-neutral output runtime contract. The state-machine command boundary
validates target parameters and applies incoming typed values only to the
invocation; it never writes those values back into the command's authored
parameters. Existing delay, stagger, cancellation, grouping, occurrence,
accepted-send cache, and queued dispatch behavior remains shared with Action.

`Invoke Existing Command` remains an explicit advanced generic command for
references to commands owned elsewhere. The R02 migration replaces each old
Mapping `OutputTarget` in place while preserving its UUID, position,
presentation, target, and binding configuration. Referenced external commands
remain with their original owner. A malformed target or a legacy constant that
ordinary controls cannot represent is retained in a read-only resolution field
with a warning; no compatibility evaluator remains on the execution path.

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

`ConsequencesManager` remains a separate older `sm_consequence` registry
container. R02 consolidates Action and Mapping through the `OutputsManager`
command catalogs and concrete factories; it does not treat the unrelated
consequence registry as part of that shared command boundary.

## Current Mapping execution path

Mapping's file-authored surface metadata declares input, filter, and output
managed regions. The processor snapshot adapter turns input/filter ANode
children and concrete output commands into `ManagedRegionInstances`, using
each authored item UUID as its stable `ManagedItemId`. The processor crate
lowers the ordered Input Source values to one scalar or heterogeneous tuple,
compiles the linear filter chain, and materializes validated command intents
from each command's typed binding controls. The manager schedules dirty and
temporal work, retains state by processor/context/item identity, and dispatches
through the queued engine command boundary.

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

## R03 resulting filter authoring contract

The Filters region now exposes a registry-derived catalog containing every
declared managed filter application. Catalog membership and creation identity
depend only on the registered ANode declaration and application variant. They
do not depend on the current sources, tuple width, neighboring filters, or
whether the chain can execute. The same container acceptance and ANode factory
contract serves UI creation, headless edits, paste, and duplication.

A newly created filter is therefore a valid authored node even with no inputs
or after an incompatible stage. It serializes, reloads, undoes, and duplicates
with its UUID and ordinary controls intact. Execution remains stricter:
Mapping recompiles the ordered chain against the current typed tuple and blocks
dispatch at the first incompatible enabled filter. That filter receives an
attributed warning and the Filters manager receives a concise aggregate
warning. Reconciliation clears both when source or filter edits make the same
node executable.

Standard Mapping treats multiple input sources as one ordered heterogeneous
tuple. Aggregate applications whose optional input-count control remains on
automatic specialize to the tuple width at compile time, without rewriting the
authored control or putting the width into the creation identifier. Explicit
input counts remain explicit. Fixed-arity filters such as Vec3 packing and
extraction validate against the tuple they receive, so X/Y/Z can be packed and
delivered to one typed command while invalid partial input never dispatches.

## R04 resulting inspector and preview boundary

Mapping authoring now uses the same `NodeInspector` hierarchy as every other
processor. The retired Mapping inspector, region list, item editor, constant
editor, value-source editor, output-binding editor, and their parallel
selection/undo model no longer exist. Inputs, Filters, concrete commands,
argument bindings, parameters, warning badges, context menus, ordering, and
history are all supplied by the ordinary Golden node contracts. Formula
preview controls remain a small optional supplement to that hierarchy.

The Rust-generated preview catalog now carries processor and Formula identity
metadata only. It does not serialize Formula surface sections, managed-region
or item inventories, Mapping pipeline shapes, output-target walks, warning
copies, or runtime-state summaries. The authoring tree therefore renders and
edits correctly when preview is disabled or absent. Runtime samples and the
bounded lane overview stay in their dedicated demand-driven feeds; the editor
uses one leased subscription identity, replaces its mode when context changes,
and releases it when the panel closes.

Every registered ANode configuration field is materialized as an addressable
child parameter or a shared Golden curve/gradient resource node. Mapping
command delivery and argument bindings likewise expose individual typed child
parameters and accept normal `SetParam` intents through `ProductionRuntime`.
The only retained document field is a read-only, typed-migration fallback for
legacy bindings that cannot be represented by those controls.

Filter-chain warning reconciliation is event driven. Each Filters region
watches its own authored subtree, the corresponding Inputs subtree, its Formula
source, and referenced source parameters. Unrelated processor parameter edits
do not request a tree snapshot or recompile the chain. Relevant source schema,
resource, enabled-state, Formula, binding, and structural changes still update
durable backend warnings even when no preview or inspector client is present.

## Identity and persistence inventory

The migration must preserve processor UUIDs, managed region role IDs, authored
ANode UUIDs, stable tuple element identities, Formula source references,
context identities, and references to nested filter/resource parameters.
Visible processor surface declaration IDs derive from Formula property source
UUIDs. Hidden region declaration IDs derive from stable Formula managed-region
IDs. R01 moved existing authored item subtrees into the ordinary role managers
without cloning them or allocating replacement UUIDs.

Existing Mapping output adapters contain a referenced command UUID and a
serialized binding document. R02 migrates them through
`integration/processor/output_migration.rs`. A referenced external command is
not stolen or deleted, and ambiguous data is retained with an attributed
migration warning. Older binding-document normalization remains in
`integration/formula/binding_migration.rs`; processor sparse persistence,
duplication, and conversion coverage lives under `integration/processor/tests`.

R05 qualifies concrete commands as part of the expanded Mapping product. A
command copied from Action keeps the source command and target intact, receives
a new command UUID, and gains Mapping-local binding controls when it enters the
Mapping Outputs region. The output region owns that reconciliation through its
ordinary structural lifecycle. Converting a configured Mapping to a project
Formula keeps the concrete command, binding controls, filter resources, nested
resource UUIDs, and graph operations in place. Full and sparse persistence use
the same project codecs, and the converted Formula continues through the
ordinary processor inspector and Formula editor paths.

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
