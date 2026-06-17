# GPT-5.5 Implementation Plan — Chataigne Built-in Action/Mapping Formula Integration

## Objective

Implement the Chataigne integration with Alchemist so that:

* `Action` and `Mapping` are **shipped built-in formula files**, not hardcoded processor types.
* The Processor Manager exposes only:

  * `Action`
  * `Mapping`
  * user/project formulas marked as processor-creatable
* `Mapping` is a single flexible built-in formula:

  * single input
  * multiple inputs
  * parallel filters
  * aggregate/merge filters
  * structure conversion filters
  * condition gates
  * one or more outputs
* `ConditionGate` is an ordinary **filter-capable ANode**, not a dedicated mapping region.
* Chataigne managers and simplified UIs are **managed formula surfaces** over Alchemist graphs.
* No duplicated evaluation logic exists between managers and ANodes.

The implementation must be AAA-quality: clean architecture, explicit typing, no hidden hacks, no shortcut runtime branches, no special-case `Mapping` or `Action` evaluator.

---

## Mandatory Working Rules

Before starting implementation, create or update a project-local progression document, for example:

```text
docs/implementation/chataigne_alchemist_integration_progress.md
```

This progression plan must be updated continuously as work advances.

The progression document must include:

```text
Current phase
Completed tasks
Pending tasks
Design decisions made
Migration notes
Known risks
Tests added
Supercommit history
```

After each major phase, perform a **supercommit**.

A supercommit means:

```text
1. Code compiles.
2. Tests for the phase pass.
3. Formatting/lints are clean, or documented if blocked.
4. Progression document is updated.
5. Commit message clearly describes the completed phase.
```

Suggested supercommit format:

```text
supercommit: chataigne alchemist integration phase N - <phase title>
```

Do not continue into the next phase before the current phase is buildable and the progression document has been updated.

---

# Phase 0 — Repository Orientation and Baseline Audit

## Goal

Understand the current state of:

* Alchemist formulas
* Processor creation
* state machine processor runtime
* manager ANodes
* filter nodes
* condition nodes
* current `param_array` / `parameters_array` usage
* formula library and formula references

## Tasks

Inspect the following areas:

```text
golden_alchemist_core / crates/golden_alchemist
src/state_machine
src/state_machine_nodes
src/module
src-ui
```

Specifically audit:

```text
StateProcessor
StateProcessorManager
FormulaLibrary
FormulaSurface
ANodeDeclaration
ANode signatures
CompiledNodeOperation
RuntimeValue
RuntimeContextFrame
LaneRuntimePool
Condition nodes
Filter nodes
Manager reference ANodes
```

Identify every place where logic is currently duplicated between:

```text
conditions as manager items
conditions as ANodes
filters as manager items
filters as ANodes
inputs/outputs as managers
inputs/outputs as graph nodes
```

## Deliverables

Update the progression document with:

```text
Baseline architecture summary
Important existing seams
Blocking design issues
Files expected to change
```

## Validation

Run the existing test/build commands used by the repository.

If the repo does not currently compile, document the exact baseline failure before changing code.

## Supercommit

```text
supercommit: chataigne alchemist integration phase 0 - baseline audit
```

---

# Phase 1 — Introduce Formula Catalog and Built-in Formula Sources

## Goal

Separate the user-visible project formula library from the complete formula catalog.

The system must support formula sources from:

```text
Project formula nodes
Built-in shipped formula files
```

## Design

Introduce a source reference similar to:

```rust
pub enum FormulaSourceRef {
    ProjectNode(NodeReference),
    Builtin {
        package: Arc<str>,
        formula_id: Arc<str>,
        version: u32,
    },
}
```

Introduce a catalog entry model similar to:

```rust
pub struct FormulaCatalogEntry {
    pub source: FormulaSourceRef,
    pub label: String,
    pub description: String,
    pub visibility: FormulaVisibility,
    pub processor_template: Option<ProcessorTemplateMeta>,
}

pub struct FormulaVisibility {
    pub show_in_formula_library: bool,
    pub show_in_processor_palette: bool,
    pub can_duplicate_to_library: bool,
    pub open_readonly_from_processor: bool,
}
```

The Formula Library remains the user/project formula tree.

The Formula Catalog resolves all formulas:

```text
project formulas
built-in formulas
future package formulas
```

## Tasks

Implement:

```text
FormulaSourceRef
FormulaCatalogEntry
FormulaVisibility
ProcessorTemplateMeta
FormulaCatalog service
Formula resolver API
```

The resolver must be able to return an Alchemist formula from either:

```text
FormulaSourceRef::ProjectNode(...)
FormulaSourceRef::Builtin(...)
```

## Rules

Do not fake built-ins as hidden project nodes.

Do not expose built-in Action/Mapping formulas in the user formula library.

Do not hardcode Action/Mapping creation in the Processor Manager.

## Deliverables

Progression document updated with:

```text
Formula catalog design
Source resolution behavior
Open questions
Migration requirements
```

## Validation

Add tests for:

```text
catalog contains built-ins
built-ins hidden from formula library
built-ins visible in processor palette
project formulas still resolve
invalid builtin source fails cleanly
```

## Supercommit

```text
supercommit: chataigne alchemist integration phase 1 - formula catalog
```

---

# Phase 2 — Replace Processor Formula Reference Model

## Goal

Make `StateProcessor` reference formulas through `FormulaSourceRef`, not only project `NodeReference`.

## Tasks

Update `StateProcessor` so it stores something equivalent to:

```rust
pub struct ProcessorFormulaSelection {
    pub source: FormulaSourceRef,
    pub instance: AlchemistFormulaInstance,
    pub managed_regions: ManagedRegionInstances,
}
```

If `AlchemistFormulaInstance` already exists in a compatible form, reuse it.

If not, introduce the smallest clean structure needed for:

```text
formula source
instance parameters
managed region item instances
future overrides
```

Update all processor creation logic to use typed formula source references.

Replace string-only creation IDs such as:

```text
state_processor:<uuid>
```

with parsed typed source references:

```text
state_processor:project:<formula_uuid>
state_processor:builtin:chataigne.mapping@1
state_processor:builtin:chataigne.action@1
```

Parse at the UI/protocol boundary. Internally use typed enums, not repeated string parsing.

## Rules

The Processor Manager must not know about “Mapping” or “Action” as Rust concepts.

It should only display catalog entries where:

```rust
visibility.show_in_processor_palette == true
```

## Deliverables

Progression document updated with:

```text
Processor formula reference migration
Creation item protocol
Backward compatibility concerns
```

## Validation

Add tests for:

```text
create processor from project formula
create processor from built-in Action
create processor from built-in Mapping
invalid source fails with diagnostic
processor serialization roundtrip
```

## Supercommit

```text
supercommit: chataigne alchemist integration phase 2 - processor formula sources
```

---

# Phase 3 — Built-in Formula Package Loader

## Goal

Load shipped built-in formula files for:

```text
chataigne.action@1
chataigne.mapping@1
```

## Design

Built-in formula files should be normal formula definitions with extra catalog metadata.

Required behavior:

```text
Action and Mapping are available in processor palette.
Action and Mapping are not visible in formula library.
Action and Mapping can be opened read-only from processor.
Action and Mapping can eventually be duplicated into the project formula library.
```

Initial built-ins:

```text
Action
Mapping
```

No variants such as:

```text
Mapping: Multi Input
Mapping: Conditioned Output
Mapping: Advanced Mapping
```

There must be exactly one built-in Mapping entry.

## Tasks

Add built-in formula package loading from one of:

```text
embedded assets
resources directory
compile-time included files
```

Choose the cleanest option for the repository.

Add built-in metadata.

Add placeholder built-in formulas if the full managed region expansion is not implemented yet, but do not add fake runtime behavior.

## Built-in Mapping Formula Shape

Conceptually:

```text
InputSet -> FilterPipeline -> OutputSet
```

Managed regions:

```text
inputs:
  kind: InputSet

filters:
  kind: FilterPipeline

outputs:
  kind: OutputSet
```

## Built-in Action Formula Shape

Conceptually:

```text
ActionTrigger -> ActionPipeline -> ActionCommands
```

Managed regions:

```text
trigger:
  kind: ActionTrigger

pipeline:
  kind: FilterPipeline

commands:
  kind: ActionCommands
```

## Deliverables

Progression document updated with:

```text
Built-in formula package format
Built-in formula IDs
Built-in formula loading path
```

## Validation

Add tests for:

```text
builtin package loads
Action exists once
Mapping exists once
Mapping has no variants
built-ins hidden from formula library
built-ins visible in processor palette
```

## Supercommit

```text
supercommit: chataigne alchemist integration phase 3 - builtin formulas
```

---

# Phase 4 — Managed Region Model

## Goal

Represent editable regions inside built-in formulas without hardcoding processor types.

## Required Region Kinds

Implement only the necessary generic region kinds:

```rust
pub enum ManagedRegionKind {
    InputSet,
    FilterPipeline,
    OutputSet,
    ActionTrigger,
    ActionCommands,
}
```

Do **not** add:

```rust
ConditionGate
ConditionRegion
MappingConditionRegion
```

Conditions in mappings must be represented by filter ANodes.

## Data Model

Introduce or adapt:

```rust
pub struct ManagedRegionDefinition {
    pub id: ManagedRegionId,
    pub kind: ManagedRegionKind,
    pub label: String,
    pub input_socket: Option<SocketRef>,
    pub output_socket: Option<SocketRef>,
    pub accepted_roles: Vec<SurfaceItemKind>,
}

pub struct ManagedRegionInstance {
    pub region_id: ManagedRegionId,
    pub items: Vec<ManagedItemInstance>,
}

pub struct ManagedItemInstance {
    pub id: ManagedItemId,
    pub anode: ANodeInstance,
    pub enabled: bool,
    pub ui_state: ManagedItemUiState,
}
```

Use existing formula/surface structures where possible.

## Rules

Managed regions are authoring surfaces.

They are not runtime evaluators.

All execution must lower to normal Alchemist formula/graph evaluation.

## Deliverables

Progression document updated with:

```text
Managed region model
Region ownership
Serialization strategy
Known missing UI integration
```

## Validation

Add tests for:

```text
managed region serialization
empty Mapping regions are valid
empty Action regions are valid
region kind roundtrip
invalid region reference gives diagnostic
```

## Supercommit

```text
supercommit: chataigne alchemist integration phase 4 - managed regions
```

---

# Phase 5 — Rename Parameter Array to ValueSet

## Goal

Replace the narrow `parameters_array` / `param_array` concept with a value collection type suitable for inputs, mappings, filters, and outputs.

## Design

Prefer:

```rust
pub const VALUE_SET_TYPE: &str = "chataigne.value_set";
```

Runtime model:

```rust
pub struct ValueSet {
    pub entries: Vec<ValueSetEntry>,
    pub logical_tick: u64,
}

pub struct ValueSetEntry {
    pub key: ValueLaneKey,
    pub label: String,
    pub source: Option<StableRef>,
    pub value: RuntimeValue,
}
```

Use stable lane keys.

Do not rely only on positional indices.

## Tasks

Find and replace conceptual usage of:

```text
param_array
parameters_array
Parameter Array
parameters
```

where it actually means values.

Rename UI label to:

```text
Values
```

or:

```text
Value Set
```

Maintain migration compatibility only if the project format already requires it. Otherwise prefer a clean schema break and document it.

## Rules

Do not make every ANode array-aware.

`ValueSet` is a boundary and collection type.

Elementwise scalar filters should still operate through lane/map evaluation.

## Deliverables

Progression document updated with:

```text
ValueSet type design
Renamed symbols
Migration choice
Affected files
```

## Validation

Add tests for:

```text
ValueSet construction
stable lane keys
serialization roundtrip
old name rejection or migration
manager sockets expose ValueSet
```

## Supercommit

```text
supercommit: chataigne alchemist integration phase 5 - value set
```

---

# Phase 6 — ANode Role and Pipeline Capability Metadata

## Goal

Allow managers and filter pipelines to understand which ANodes are usable as filters, inputs, outputs, conditions, actions, etc., without hardcoding node types.

## Design

Add metadata similar to:

```rust
pub struct ANodeRoleCapability {
    pub role: SurfaceItemKind,
    pub primary_input: Option<SocketId>,
    pub primary_output: Option<SocketId>,
    pub autowire: AutoWirePolicy,
    pub cardinality: PipelineCardinality,
    pub ui_mode: ManagedUiMode,
}
```

Autowire policy:

```rust
pub enum AutoWirePolicy {
    None,
    UnaryTransform {
        input: SocketId,
        output: SocketId,
    },
    Source {
        output: SocketId,
    },
    Sink {
        input: SocketId,
        trigger: Option<SocketId>,
    },
    Gate {
        input: SocketId,
        condition: SocketId,
        output: SocketId,
    },
}
```

Pipeline cardinality:

```rust
pub enum PipelineCardinality {
    Elementwise,
    Aggregate,
    Reshape,
    Expand,
    WholeSet,
}
```

## Initial Capabilities

Register existing or new ANodes:

```text
Remap / MapRange:
  role: Filter
  cardinality: Elementwise

Clamp:
  role: Filter
  cardinality: Elementwise

Smooth:
  role: Filter
  cardinality: Elementwise

Function:
  role: Filter
  cardinality: Elementwise

Math Aggregate:
  role: Filter
  cardinality: Aggregate

Pack Vec2:
  role: Filter
  cardinality: Reshape

Pack Vec3:
  role: Filter
  cardinality: Reshape

Pack Color:
  role: Filter
  cardinality: Reshape

Select Input:
  role: Filter
  cardinality: Aggregate or Reshape

Broadcast:
  role: Filter
  cardinality: Expand

ConditionGate:
  role: Filter
  cardinality: WholeSet or Elementwise depending on mode
```

## Rules

Capabilities must be declarative.

Do not use ad-hoc `match node_type == ...` inside the filter pipeline compiler except where temporarily unavoidable and documented.

## Deliverables

Progression document updated with:

```text
Capability metadata design
Registered initial node capabilities
Temporary compatibility exceptions
```

## Validation

Add tests for:

```text
filter-capable node discovery
non-filter node rejected from filter pipeline
primary socket autowiring
capability serialization if applicable
```

## Supercommit

```text
supercommit: chataigne alchemist integration phase 6 - anode capabilities
```

---

# Phase 7 — ConditionGate as a Filter ANode

## Goal

Implement `ConditionGate` as a reusable ANode.

It must be usable in:

```text
Mapping filter pipeline
Action pipeline
Custom user formula
Full Alchemist graph
```

## Behavior

Conceptual sockets:

```text
input:
  value: Any or ValueSet
  condition: bool
  default_value: optional Any

output:
  value: same shape as input
  passed: bool
  blocked: bool
```

Modes:

```rust
pub enum ConditionGateMode {
    PassWhenTrue,
    PassWhenFalse,
    HoldLast,
    OutputDefault,
    BlockTrigger,
}
```

Shape behavior:

```text
Single<T> -> Single<T>
ValueSet<T> -> ValueSet<T>
Trigger -> Trigger
CommandIntent -> CommandIntent
```

Decide whether `ValueSet` gating is:

```text
whole-set gate
per-entry gate
```

This should be explicit in config:

```rust
pub enum GateApplication {
    Whole,
    PerLane,
}
```

## Rules

ConditionGate must not be special-cased in Mapping.

Mapping uses it only because it is a filter-capable ANode.

## Deliverables

Progression document updated with:

```text
ConditionGate behavior
Gate modes implemented
ValueSet gating semantics
Tests added
```

## Validation

Add tests for:

```text
true condition passes value
false condition blocks value
hold last behavior
default output behavior
trigger blocking
ValueSet whole gate
ValueSet per-lane gate if implemented
ConditionGate appears as filter-capable ANode
```

## Supercommit

```text
supercommit: chataigne alchemist integration phase 7 - condition gate anode
```

---

# Phase 8 — Filter Pipeline Shape Checker

## Goal

Implement a typed shape checker for linear Mapping and Action pipelines.

## Shape Model

Use a shape model similar to:

```rust
pub enum PipelineShape {
    Single {
        value_type: ValueTypeId,
    },
    ValueSet {
        item_type: ValueTypeId,
        axis: Option<ContextAxisId>,
    },
    Trigger,
    CommandIntent,
    Unknown,
}
```

If existing Alchemist typing can express this directly, reuse it.

## Behavior

The pipeline checker walks items in order:

```text
initial shape
  -> item 1 capability
  -> resulting shape
  -> item 2 capability
  -> resulting shape
  -> ...
```

Examples:

```text
ValueSet<float> + Remap -> ValueSet<float>
ValueSet<float> + MathAggregateSum -> Single<float>
ValueSet<float> + PackVec3 -> Single<vec3>
Single<float> + Clamp -> Single<float>
Single<float> + Broadcast -> ValueSet<float>
```

## Rules

Shape-changing must be explicit.

Do not silently merge multiple inputs.

Do not silently broadcast a single value to multiple outputs unless a `Broadcast` node exists.

## Deliverables

Progression document updated with:

```text
Pipeline shape rules
Shape diagnostics
Current unsupported transitions
```

## Validation

Add tests for:

```text
elementwise ValueSet pipeline
aggregate ValueSet to Single
pack floats to Vec3
invalid node rejected with useful diagnostic
ConditionGate preserves shape
Broadcast expands shape
```

## Supercommit

```text
supercommit: chataigne alchemist integration phase 8 - pipeline shape checker
```

---

# Phase 9 — Filter Pipeline Lowering

## Goal

Lower managed filter pipeline regions into normal Alchemist graphs.

## Behavior

A filter pipeline region must lower to an Alchemist graph using the actual ANode instances.

For an elementwise node over a `ValueSet`, use explicit lane/map semantics.

Do not duplicate scalar math logic.

Conceptual lowering:

```text
ValueSet<T>
  -> MapEach(Remap)
  -> MapEach(Clamp)
  -> MathAggregate
  -> ConditionGate
  -> Output
```

`MapEach` may be implemented as:

```text
runtime lane evaluation
compiler expansion helper
subgraph execution strategy
```

Choose the cleanest approach consistent with the current Alchemist runtime.

## Rules

Do not materialize one full graph copy per input lane.

Compile one pipeline graph and evaluate through lanes/context where possible.

Stateful filters must have independent lane memory.

## Deliverables

Progression document updated with:

```text
Lowering strategy
MapEach/lane strategy
Stateful filter memory behavior
Limitations
```

## Validation

Add tests for:

```text
Remap + Clamp chain produces same result as full graph
multiple inputs through Smooth keep independent memory
Aggregate reduces multiple values to one
PackVec3 produces expected vector
ConditionGate inside pipeline works
```

## Supercommit

```text
supercommit: chataigne alchemist integration phase 9 - filter pipeline lowering
```

---

# Phase 10 — InputSet Region

## Goal

Implement the `InputSet` managed region for Mapping.

## Behavior

The user can select one or more inputs.

The region outputs:

```text
ValueSet
```

Each entry must have:

```text
stable lane key
label
source reference
RuntimeValue
```

Supported initial input sources should include the currently existing Chataigne input concepts.

Do not overbuild unsupported module-specific sources yet.

## Rules

InputSet is a managed region, not an evaluator separate from Alchemist.

It is a boundary region that materializes runtime values into the formula pipeline.

## Deliverables

Progression document updated with:

```text
InputSet source model
Stable lane key strategy
Supported sources
Unsupported sources
```

## Validation

Add tests for:

```text
single input ValueSet
multiple input ValueSet
input reorder preserves lane identity
disabled input excluded or marked according to design
missing input produces diagnostic or invalid entry according to design
```

## Supercommit

```text
supercommit: chataigne alchemist integration phase 10 - input set region
```

---

# Phase 11 — OutputSet Region

## Goal

Implement the `OutputSet` managed region for Mapping.

## Behavior

The region consumes:

```text
Single<T>
ValueSet<T>
Trigger
CommandIntent
```

and produces runtime output intents.

Do not perform module IO directly inside Alchemist evaluation.

Use a runtime intent model.

Conceptual boundary:

```text
ANode evaluation -> RuntimeIntent -> Chataigne dispatcher -> modules
```

## Rules

Alchemist should stay deterministic and testable.

Module connection state, transport logic, reconnect behavior, and external IO remain outside pure formula evaluation.

## Deliverables

Progression document updated with:

```text
OutputSet intent model
Supported output targets
Dispatch boundary
Unsupported cases
```

## Validation

Add tests for:

```text
single value output creates expected intent
ValueSet output creates per-entry intents or structured intent according to design
blocked gate creates no intent
output formatting works if implemented
```

## Supercommit

```text
supercommit: chataigne alchemist integration phase 11 - output set region
```

---

# Phase 12 — Built-in Mapping End-to-End

## Goal

Create a working Mapping processor from the built-in `chataigne.mapping@1` formula.

## Required UX Model

The Processor Manager shows:

```text
Mapping
```

only once.

After creation, the Mapping processor exposes:

```text
Inputs
Filters
Outputs
```

No separate `Conditions` section.

Conditions are added through:

```text
Filters -> Add Filter -> ConditionGate
```

A UI shortcut may say:

```text
Add Condition
```

but it must insert a `ConditionGate` filter item.

## Runtime Behavior

The Mapping supports:

```text
single input -> filters -> output
multiple inputs -> elementwise filters -> multiple outputs
multiple inputs -> aggregate filter -> single output
multiple inputs -> pack/project filter -> structured output
condition gate inside filters
```

## Rules

No Mapping variants.

No hardcoded Mapping evaluator.

No condition region.

## Deliverables

Progression document updated with:

```text
Mapping end-to-end status
Supported mapping cases
Known missing UX
Known missing node types
```

## Validation

Add integration tests for:

```text
create built-in Mapping processor
single input remap output
multi-input parallel remap output
multi-input aggregate sum output
two floats pack to vec3
ConditionGate blocks output
ConditionGate passes output
processor serialization roundtrip
```

## Supercommit

```text
supercommit: chataigne alchemist integration phase 12 - builtin mapping
```

---

# Phase 13 — Action Pipeline End-to-End

## Goal

Create a working Action processor from the built-in `chataigne.action@1` formula.

## Required UX Model

The Processor Manager shows:

```text
Action
```

only once.

After creation, the Action processor exposes:

```text
Trigger
Pipeline
Commands
```

or equivalent simplified UI.

Conditions are represented by `ConditionGate` or related filter/gate ANodes in the pipeline.

## Runtime Behavior

The Action supports:

```text
trigger input
optional gate/filter pipeline
command/output intents
```

## Rules

No hardcoded Action evaluator.

No separate condition subsystem inside Action.

## Deliverables

Progression document updated with:

```text
Action end-to-end status
Supported action cases
Known missing UX
Known missing node types
```

## Validation

Add integration tests for:

```text
create built-in Action processor
trigger produces command intent
ConditionGate blocks command
ConditionGate passes command
processor serialization roundtrip
```

## Supercommit

```text
supercommit: chataigne alchemist integration phase 13 - builtin action
```

---

# Phase 14 — Manager Reference ANodes as Bridges

## Goal

Convert existing manager reference ANodes into clean bridge nodes.

Examples:

```text
ConditionsManagerRef
InputsManagerRef
OutputsManagerRef
```

These must not implement duplicate logic.

They should reference managed formula instances or manager regions and expose compact graph sockets.

## Behavior

Examples:

```text
ConditionsManagerRef:
  outputs:
    valid: bool
    on_true: trigger
    on_false: trigger

InputsManagerRef:
  outputs:
    values: ValueSet

OutputsManagerRef:
  inputs:
    values: ValueSet or Single
    trigger: optional
```

## Rules

No fake fallback values.

If manager evaluation is unavailable, emit an explicit diagnostic.

## Deliverables

Progression document updated with:

```text
Manager bridge behavior
Removed duplicate logic
Remaining unsupported bridges
```

## Validation

Add tests for:

```text
manager ref resolves valid target
manager ref invalid target gives diagnostic
no fallback values are emitted
input manager ref exposes ValueSet
condition manager ref exposes bool/trigger
```

## Supercommit

```text
supercommit: chataigne alchemist integration phase 14 - manager ref bridges
```

---

# Phase 15 — Remove Duplicated Filter and Condition Logic

## Goal

Delete or deprecate duplicated manager-specific evaluation logic.

## Tasks

Find old code paths for:

```text
input value condition manager evaluator
input node condition manager evaluator
script condition manager evaluator
remap filter manager evaluator
clamp filter manager evaluator
function filter manager evaluator
math filter manager evaluator
```

Replace with:

```text
managed ANode item
formula lowering
Alchemist evaluation
```

Wrappers may remain for tree/UI/persistence purposes only.

## Rules

A wrapper may own:

```text
label
enabled state
UI expansion state
node reference
managed item ID
```

A wrapper must not own:

```text
math behavior
comparison behavior
condition behavior
filter runtime behavior
output dispatch behavior
```

## Deliverables

Progression document updated with:

```text
Removed duplicated logic
Remaining compatibility wrappers
Deleted files/functions
```

## Validation

Add regression tests proving:

```text
manager filter result == direct ANode result
manager condition result == direct ANode result
```

## Supercommit

```text
supercommit: chataigne alchemist integration phase 15 - remove duplicate manager logic
```

---

# Phase 16 — UI Integration in Svelte 5 Runes

## Goal

Expose the new architecture through clean Chataigne UI surfaces.

## Required UI

Processor creation palette:

```text
Built-ins
  Action
  Mapping

Project Formulas
  <user processor-creatable formulas>
```

Mapping processor UI:

```text
Inputs
Filters
Outputs
```

Action processor UI:

```text
Trigger
Pipeline
Commands
```

Filter rows must show pipeline shape transitions where useful:

```text
3 floats -> 3 floats
3 floats -> 1 float
2 floats -> vec3
```

Condition UX:

```text
Add Filter -> ConditionGate
```

Optional shortcut:

```text
Add Condition
```

which inserts a `ConditionGate` filter.

## Rules

Use Svelte 5 runes only.

Avoid duplicated state.

UI should project backend managed regions and items, not invent a parallel frontend graph model.

## Deliverables

Progression document updated with:

```text
UI components changed
Processor creation UX
Mapping UI status
Action UI status
Known missing polish
```

## Validation

Add UI-level tests if the repo has them.

At minimum, manually verify and document:

```text
create Action
create Mapping
add input
add filter
add ConditionGate
add aggregate filter
add output
save/reload
```

## Supercommit

```text
supercommit: chataigne alchemist integration phase 16 - ui integration
```

---

# Phase 17 — Read-only Built-in Formula Inspection and Duplicate to Library

## Goal

Allow users to inspect built-in formulas without exposing them directly in the formula library.

## Features

From a processor using a built-in formula:

```text
Open Built-in Formula
Duplicate to Formula Library
```

Behavior:

```text
Open Built-in Formula:
  read-only
  clearly marked as built-in
  no accidental editing

Duplicate to Formula Library:
  copies formula into project formula library
  creates editable project formula
  processor may optionally switch to duplicated formula
```

## Rules

Do not store edits against the built-in formula source.

Do not make built-ins visible as normal library formulas.

## Deliverables

Progression document updated with:

```text
Inspection behavior
Duplication behavior
Switching behavior
Serialization behavior
```

## Validation

Add tests for:

```text
builtin open is read-only
duplicate creates project formula
duplicated formula appears in formula library
original builtin remains hidden
processor can switch to duplicated formula if implemented
```

## Supercommit

```text
supercommit: chataigne alchemist integration phase 17 - builtin formula inspection
```

---

# Phase 18 — Diagnostics, Migration, and Hardening

## Goal

Make the architecture robust and developer-friendly.

## Tasks

Add diagnostics for:

```text
invalid formula source
missing built-in formula
unknown managed region
invalid filter node in pipeline
shape mismatch
unsupported ValueSet transition
missing input source
invalid output target
ConditionGate incompatible mode
```

Add migration handling for:

```text
old parameter array type
old processor formula references
old manager filter items
old condition manager items
```

If no migration is required yet, document the clean schema break.

## Rules

Never fail silently.

Never insert default fake values to hide invalid graphs.

## Deliverables

Progression document updated with:

```text
Diagnostics list
Migration list
Known remaining risks
```

## Validation

Add tests for all diagnostics where practical.

## Supercommit

```text
supercommit: chataigne alchemist integration phase 18 - diagnostics and migration
```

---

# Phase 19 — Final QA and Architecture Documentation

## Goal

Finalize the feature with complete documentation and quality checks.

## Documentation

Update or add architecture documentation covering:

```text
Formula Catalog vs Formula Library
Built-in formulas
Processor formula references
Managed regions
Mapping architecture
Action architecture
ConditionGate as filter
ValueSet
Pipeline shape checking
MapEach / lane evaluation
Runtime intents
```

Update user/developer docs explaining:

```text
Action is a built-in formula
Mapping is a built-in formula
Mapping has Inputs / Filters / Outputs
Conditions are filters through ConditionGate
Complex branching requires custom formulas
```

## QA Matrix

Verify:

```text
Action creation
Mapping creation
Project formula processor creation
Built-in formulas hidden from library
Built-in formulas visible in processor palette
Mapping single input
Mapping multiple inputs parallel
Mapping multiple inputs aggregate
Mapping pack Vec3
Mapping ConditionGate
Action ConditionGate
Serialization roundtrip
Save/reload
Undo/redo if applicable
No duplicate evaluator paths
No fallback fake values
```

## Final Progression Document State

The progression document must end with:

```text
All phases completed or explicitly deferred
List of deferred items
Final architecture summary
Final test status
Final supercommit hash list
```

## Supercommit

```text
supercommit: chataigne alchemist integration phase 19 - final qa and docs
```

---

# Non-Negotiable Architecture Constraints

## Action and Mapping

`Action` and `Mapping` are built-in formula files.

They are not Rust processor subclasses.

They are not hardcoded processor kinds.

The processor manager must not contain logic like:

```rust
if kind == Mapping { ... }
if kind == Action { ... }
```

It must consume catalog entries.

## Mapping

There is only one built-in Mapping.

Do not expose:

```text
Mapping: Multi Input
Mapping: Conditioned Output
Mapping: Advanced Mapping
```

The single Mapping supports those workflows through its contents.

Mapping structure:

```text
Inputs -> Filters -> Outputs
```

No separate `Conditions` core region.

## ConditionGate

`ConditionGate` is an ANode.

It is filter-capable.

It can be used in:

```text
Mapping
Action
custom formulas
full graph editor
```

Condition logic inside Mapping must be represented by inserting `ConditionGate` into the filter pipeline.

## Filters

Filters are ANodes.

Manager filter rows are managed ANode instances.

No duplicated filter runtime logic.

## Conditions

Conditions are ANodes or formulas.

Manager condition rows are managed ANode instances or formula-backed items.

No duplicated condition runtime logic.

## ValueSet

Use `ValueSet` for multi-input/multi-value flows.

Do not make every ANode array-aware.

Elementwise scalar filters over `ValueSet` must use lane/map evaluation.

## Shape Changes

Multi-input behavior must be explicit.

Allowed examples:

```text
ValueSet<float> -> Remap -> ValueSet<float>
ValueSet<float> -> MathAggregateSum -> float
ValueSet<float> -> PackVec3 -> vec3
float -> Broadcast -> ValueSet<float>
```

Disallowed behavior:

```text
multiple inputs silently merging
single value silently broadcasting
conditions as hidden mapping subsystem
mapping variants for basic capabilities
```

## Escape Hatch

Mappings support linear shape-aware pipelines.

Complex branching, feedback loops, multi-path processing, and arbitrary topology require a custom formula.

Users should eventually be able to:

```text
Open Mapping as Formula
Duplicate Mapping built-in into Formula Library
Convert Mapping instance to custom formula
```

---

# Expected Final Architecture

```text
Formula Catalog
  Built-ins
    Action
    Mapping
  Project formulas
    User formulas marked processor-creatable

Processor Manager
  Creates processors from catalog entries

StateProcessor
  References FormulaSourceRef
  Owns formula instance data
  Owns managed region instances

Built-in Mapping Formula
  InputSet region
  FilterPipeline region
  OutputSet region

Built-in Action Formula
  ActionTrigger region
  FilterPipeline / ActionPipeline region
  ActionCommands region

ConditionGate
  Normal filter-capable ANode

FilterPipeline
  Linear managed region
  Uses ANode capabilities
  Tracks PipelineShape
  Lowers to Alchemist graph/runtime

ValueSet
  Stable-key multi-value runtime type

Runtime
  Alchemist evaluates formulas
  Chataigne dispatches RuntimeIntent to modules
```

Final target:

```text
one formula system
one node registry
one typing system
one runtime path
one debug path
one processor creation mechanism
multiple simplified UI surfaces
```
