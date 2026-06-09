# Final Implementation Plan: State Machine, Processor Manager, Alchemist Formula, ANodes, Contexts, and Multiplexed Execution

## 1. Core philosophy

Chataigne2 should be built around this model:

```text
State Machine
  └─ State
      └─ ProcessorManager
          ├─ ProcessorGroup
          │   └─ Processor
          │       └─ Alchemist Formula Instance
          │           └─ Alchemist Formula
          │               └─ Alchemist Graph
          │                   └─ ANodes
          └─ Processor
              └─ Alchemist Formula Instance
```

The important ideas are:

```text
State contains a ProcessorManager.

ProcessorManager contains Processors and ProcessorGroups.

ProcessorGroup is both an organizational scope and an execution/context scope.

Processor is an instance of an Alchemist Formula.

Alchemist Formula is a reusable node-based processing model.

ANodes are the nodes inside an Alchemist Formula graph.

ANodes may expose inspector-facing controls.

The Processor inspector edits the Formula Surface, not raw graph internals.

The Alchemist graph editor edits the actual node graph.

Contexts are inherited and accumulating.

Multiplexing is not a Formula type.
Multiplexing is what happens when accumulated context resolves to several runtime lanes.
```

The end goal is:

```text
Beginner:
  uses ready-to-use Action and Mapping Processors.

Intermediate:
  edits conditions, filters, and outputs from the Processor inspector.

Advanced:
  opens the Alchemist graph and customizes the Formula.

Expert:
  creates reusable Custom Formulas and custom Managed ANodes.
```

---

# 2. Reusable engine separation

Alchemist should be reusable across Golden Core apps.

It may live inside the broader Golden workspace, but it should not be Chataigne-specific.

Recommended crate split:

```text
golden_alchemist
  reusable Alchemist graph engine
  app-agnostic
  typed graph IR
  ANode registry
  type solving
  graph lowering
  graph compilation
  runtime evaluator
  diagnostics
  formula surface model

golden_statechart
  reusable hierarchical statechart engine
  app-agnostic
  states
  meta-states
  transitions
  active state configuration
  enter/exit semantics

chataigne_alchemist
  Chataigne-specific Alchemist integration
  Chataigne value types
  Chataigne ANodes
  module inputs
  command outputs
  sequence/state command targets

chataigne_state_machine
  Chataigne-specific State Machine UX
  State owns ProcessorManager
  ProcessorManager owns Processors and ProcessorGroups
  Processor uses Alchemist Formula Instance
  context-aware runtime execution
```

`golden_alchemist` must not know about:

```text
Module
Sequence
State
CommandTarget
OSC
MIDI
DMX
Chataigne-specific processors
```

Those are registered by Chataigne-specific integration crates.

---

# 3. Final term definitions

## State

A State is an activation scope.

It owns:

```text
state lifecycle
state context contribution
ProcessorManager
transitions
meta-state / child state configuration
```

A State does not directly own Processors.

Correct:

```text
State -> ProcessorManager -> Processors / ProcessorGroups
```

Incorrect:

```text
State -> flat Processor list
```

---

## ProcessorManager

A ProcessorManager is the processing container of a State.

It owns:

```text
direct processors
processor groups
execution ordering
context contribution
lifecycle propagation
diagnostics aggregation
active processor runtime matrix
```

Rust shape:

```rust
pub struct ProcessorManager {
    pub id: ProcessorManagerId,
    pub processors: Vec<ProcessorId>,
    pub groups: Vec<ProcessorGroup>,
    pub context: ContextContribution,
    pub execution_policy: ProcessorExecutionPolicy,
}
```

---

## ProcessorGroup

A ProcessorGroup is an execution and context scope.

It is not merely a visual folder.

It can:

```text
group processors
enable / disable processors together
contribute context
expand dimensions
create multiplexed runtime lanes
define execution policy
define priority policy
```

Rust shape:

```rust
pub struct ProcessorGroup {
    pub id: ProcessorGroupId,
    pub label: String,
    pub processors: Vec<ProcessorId>,
    pub context: ContextContribution,
    pub execution_policy: ProcessorExecutionPolicy,
    pub enabled: bool,
}
```

---

## Processor

A Processor is a user-facing instance of an Alchemist Formula.

It owns:

```text
formula instance
enabled state
context contribution
memory policy
lifecycle policy
diagnostics
```

Rust shape:

```rust
pub struct Processor {
    pub id: ProcessorId,
    pub label: String,
    pub formula_instance: AlchemistFormulaInstance,
    pub context: ContextContribution,
    pub enabled: bool,
    pub memory_policy: ProcessorMemoryPolicy,
    pub lifecycle_policy: ProcessorLifecyclePolicy,
}
```

The Processor inspector displays the Formula Surface.

The Alchemist graph editor displays the Formula graph.

---

## Alchemist Formula

An Alchemist Formula is a reusable processing model.

It contains:

```text
graph
surface
context contract
version
migration rules
formula family
```

Rust shape:

```rust
pub struct AlchemistFormula {
    pub id: FormulaId,
    pub version: u32,
    pub label: String,
    pub family: FormulaFamily,
    pub graph: AlchemistGraph,
    pub surface: FormulaSurface,
    pub context_contract: FormulaContextContract,
    pub migrations: Vec<FormulaMigration>,
}
```

Final top-level Formula families:

```rust
pub enum FormulaFamily {
    Action,
    Mapping,
    CustomUser,
}
```

There is no `Multiplex` Formula family.

Multiplexing belongs to the context/dimension system.

---

## Alchemist Formula Instance

A Formula Instance is the per-Processor instance of a Formula.

Rust shape:

```rust
pub struct AlchemistFormulaInstance {
    pub formula_ref: FormulaRef,
    pub graph_instance: AlchemistGraph,
    pub surface_bindings: FormulaSurfaceBindings,
    pub overrides: FormulaOverrides,
    pub diagnostics: Vec<Diagnostic>,
}
```

This allows:

```text
built-in formulas
custom formulas
per-processor overrides
advanced graph edits
formula migrations
shared formula libraries
```

---

## Alchemist Graph

An Alchemist Graph is the authored node graph inside a Formula.

Rust shape:

```rust
pub struct AlchemistGraph {
    pub id: AlchemistGraphId,
    pub nodes: SlotMap<ANodeId, ANodeInstance>,
    pub edges: Vec<AEdge>,
    pub layout: GraphLayout,
    pub metadata: GraphMetadata,
}
```

The graph is edited in the advanced Alchemist editor.

---

## ANode

An ANode is a node inside an Alchemist Graph.

ANodes can be:

```text
Atomic ANodes
Domain Adapter ANodes
Managed ANodes
Composite ANodes
Formula-as-ANode, later
```

Every ANode has a graph face.

Some ANodes also expose a surface face.

```rust
pub trait ANodeDeclaration: Send + Sync {
    fn type_id(&self) -> ANodeTypeId;

    fn signature(
        &self,
        ctx: &SignatureCtx,
        instance: &ANodeInstance,
    ) -> ANodeSignature;

    fn surface(
        &self,
        ctx: &SurfaceCtx,
        instance: &ANodeInstance,
    ) -> Vec<SurfaceContribution>;

    fn apply_surface_edit(
        &self,
        ctx: &mut SurfaceEditCtx,
        instance: ANodeId,
        edit: SurfaceEdit,
    ) -> Result<(), SurfaceEditError>;

    fn compile(
        &self,
        ctx: &CompileCtx,
        instance: &ANodeInstance,
    ) -> CompileResult;
}
```

---

# 4. Correct Formula families

## Action Formula

Action is the successor of Chataigne 1.x Actions.

Concept:

```text
conditions
  -> consequences when true
  -> consequences when false
```

Processor inspector surface:

```text
Conditions
  + Add Condition
  condition rows
  all / any / custom mode

Consequences when true
  + Add Command
  command list

Consequences when false
  + Add Command
  command list

Options
  trigger mode
  edge mode
  debounce
  cooldown
  priority
```

Internal graph:

```text
ConditionsManagerANode
  -> TriggerModeANode
  -> CooldownANode
  -> BranchANode
      true  -> ConsequencesANode
      false -> ConsequencesANode
```

`ConditionsManagerANode` and `ConsequencesANode` are not Formula families.

They are Managed ANodes used by the Action Formula.

---

## Mapping Formula

Mapping is the successor of Chataigne 1.x Mappings.

Concept:

```text
input
  -> filters / transforms
  -> outputs
```

Processor inspector surface:

```text
Input
  source value

Filters
  + Add Filter
  filter chain

Outputs
  + Add Output
  command outputs

Options
  send mode
  rate limit
  priority
  conflict policy
```

Internal graph:

```text
InputSourceANode
  -> FilterChainANode
  -> OutputMappingANode
  -> ConsequencesANode
```

`FilterChainANode`, `OutputMappingANode`, and `ConsequencesANode` are not Formula families.

They are Managed ANodes or domain ANodes used inside Mapping.

---

## Custom User Formula

Custom User Formula is the advanced extension path.

It can contain:

```text
Atomic ANodes
Managed ANodes
Domain ANodes
Composite ANodes
context-aware ANodes
custom exposed surface
```

It allows users to create behavior that ready-to-use Actions and Mappings cannot express.

Example:

```text
Custom Formula: Fixture Intensity With Safety Limit

Surface:
  input source
  safety maximum
  smoothing
  contextual output target

Graph:
  InputSource
    -> FilterChain
    -> Min(SafetyMaximum)
    -> ContextualCommandTarget
    -> Consequences
```

---

# 5. What is not a Formula family

The following should not be presented as top-level Formula families:

```text
Input Condition
Filter Chain
Command Output
Sequence Launcher
State Controller
Multiplex
```

Correct classification:

```text
Input Condition
  component of Action

Filter Chain
  Managed ANode inside Mapping or Custom Formula

Command Output / Consequences
  Managed ANode inside Action, Mapping, or Custom Formula

Sequence Launcher
  consequence/output target type

State Controller
  consequence/output target type

Multiplex
  context/dimension/lane behavior
```

Final Formula families:

```text
Action
Mapping
Custom User
```

---

# 6. ANode levels

## Level 0 — Atomic ANodes

Low-level operations.

Examples:

```text
Constant
Add
Subtract
Multiply
Divide
Compare
And
Or
Not
MapRange
Clamp
Smooth
Edge
Gate
Latch
DelayOneTick
Select
Switch
```

These are reusable and app-agnostic where possible.

---

## Level 1 — Domain Adapter ANodes

These connect Alchemist to app-specific systems.

In Chataigne:

```text
ModuleValueInputANode
ModuleEventInputANode
CommandTargetANode
CommandIntentOutputANode
SequenceCommandANode
StateCommandANode
DashboardInputANode
DashboardOutputANode
```

These belong in Chataigne integration, not in generic `golden_alchemist`.

---

## Level 2 — Managed ANodes

Managed ANodes expose rich inspector UI and manage internal lists, rules, generated fragments, or mini-systems.

Examples:

```text
ConditionsManagerANode
FilterChainANode
ConsequencesANode
OutputMappingANode
ContextualTargetResolverANode
```

They are the key to preserving Chataigne 1.x simplicity.

Example:

```text
ConditionsManagerANode

Inspector:
  + Add Condition
  condition rows
  all / any mode

Lowered graph:
  condition 1 -> compare
  condition 2 -> compare
  condition 3 -> compare
  compare outputs -> reducer
```

---

## Level 3 — Composite ANodes

Composite ANodes contain an internal Alchemist graph.

```rust
pub enum ANodeBody {
    Atomic,
    Managed {
        managed_state: ManagedANodeState,
    },
    Composite {
        internal_graph: AlchemistGraph,
    },
}
```

Composite ANodes lower into executable graph fragments.

---

## Level 4 — Formula-as-ANode

Later, a Formula may be reusable as an ANode inside another Formula.

This is useful, but not required for the first implementation.

---

# 7. Formula Surface

The Formula Surface is the Processor inspector-facing interface.

It is assembled from:

```text
formula-level sections
ANode surface contributions
processor-level options
context preview
diagnostics
```

Rust shape:

```rust
pub struct FormulaSurface {
    pub sections: Vec<SurfaceSection>,
}

pub struct SurfaceSection {
    pub id: SurfaceSectionId,
    pub label: String,
    pub items: Vec<SurfaceItem>,
    pub source: SurfaceSource,
}

pub enum SurfaceSource {
    Formula,
    Processor,
    ANode {
        node_id: ANodeId,
        contribution_id: SurfaceContributionId,
    },
}
```

Important rule:

```text
ANodes may expose surface contributions.
The Formula decides how those contributions are assembled.
The Processor inspector displays the final Formula Surface.
```

This prevents the inspector from becoming a random dump of graph internals.

---

# 8. Processor inspector vs Alchemist graph editor

## Processor inspector

The Processor inspector is the normal user interface.

It displays:

```text
Processor name
Formula family
Formula Surface
context preview
runtime lane preview
diagnostics
Open Alchemist Graph button
```

Example Action Processor:

```text
Processor: Launch Clip On Button

Formula: Action

Conditions:
  Button A pressed

Consequences when true:
  Launch clip

Consequences when false:
  none

Options:
  rising edge
  cooldown 0 ms
```

Example Mapping Processor:

```text
Processor: Fader To Fixture Intensity

Formula: Mapping

Context:
  scene = act_1_scene_3
  fixture = 12 values

Execution:
  multiplexed
  lanes: 12

Input:
  MIDI fader 1

Filters:
  map 0..127 to 0..1
  smooth 100 ms

Outputs:
  contextual fixture intensity
```

---

## Alchemist graph editor

The Alchemist graph editor is the advanced interface.

It displays:

```text
ANodes
sockets
connections
managed ANodes
surface exposure badges
context-aware nodes
diagnostics
live values
```

Recommended graph editor modes:

```text
Authored View
  high-level Formula graph

Lowered View
  generated graph after Managed/Composite lowering

Runtime View
  per-context live values and diagnostics
```

---

# 9. Context system

## Core rule

Contexts are inherited and accumulating.

```text
Project context
  + State context
    + ProcessorManager context
      + ProcessorGroup context
        + Processor context
          = accumulated processor context
```

A Processor inside a ProcessorGroup inside a State can access all context layers.

Example:

```text
State context:
  scene = act_1_scene_3

ProcessorGroup context:
  fixture = [spot_01, spot_02, spot_03]

Processor context:
  color_layer = front_wash
```

Resolved runtime lanes:

```text
lane 1:
  scene = act_1_scene_3
  fixture = spot_01
  color_layer = front_wash

lane 2:
  scene = act_1_scene_3
  fixture = spot_02
  color_layer = front_wash

lane 3:
  scene = act_1_scene_3
  fixture = spot_03
  color_layer = front_wash
```

---

## ContextStack

Context should preserve provenance.

```rust
pub struct ContextStack {
    pub layers: SmallVec<[ContextLayer; 8]>,
}

pub struct ContextLayer {
    pub source: ContextSource,
    pub bindings: Vec<ContextBinding>,
}

pub enum ContextSource {
    Project,
    State(StateDefinitionId),
    ProcessorManager(ProcessorManagerId),
    ProcessorGroup(ProcessorGroupId),
    Processor(ProcessorId),
}
```

This allows the debugger to explain where every context value came from.

---

## ContextBinding

```rust
pub struct ContextBinding {
    pub dimension: ContextDimensionId,
    pub value: ContextValue,
    pub role: ContextRole,
}
```

Examples:

```text
scene = act_1_scene_3
fixture = spot_01
performer = alice
layer = front_wash
module = midi_controller_1
```

---

## ContextContribution

Every relevant scope can contribute context.

```rust
pub struct ContextContribution {
    pub mode: ContextContributionMode,
    pub bindings: Vec<ContextBindingExpr>,
}

pub enum ContextContributionMode {
    InheritOnly,
    Accumulate,
    Refine,
    OverrideExplicit,
}
```

Default mode:

```text
Accumulate
```

Same-dimension rule:

```text
Different dimensions accumulate.

Same dimension refines by default.

Replacement requires explicit override.
```

Example:

```text
State:
  fixture = all moving lights

ProcessorGroup:
  fixture = front truss moving lights
```

Result:

```text
fixture = intersection(all moving lights, front truss moving lights)
```

not accidental replacement.

---

# 10. Dimension and multiplexed execution

## Dimension

A Dimension is an axis of contextual variation.

Examples:

```text
scene
fixture
module
performer
layer
track
clip
sequence
device
```

A dimension may contain one value or multiple values.

---

## ContextFrame

A ContextFrame is one resolved runtime lane.

```rust
pub struct ContextFrame {
    pub key: ContextKey,
    pub stack: ContextStack,
}
```

Example:

```text
scene = act_1_scene_3
fixture = spot_02
layer = front_wash
```

---

## ContextSet

A ContextSet is the resolved list of ContextFrames.

```rust
pub struct ContextSet {
    pub frames: Vec<ContextFrame>,
}

impl ContextSet {
    pub fn is_multiplexed(&self) -> bool {
        self.frames.len() > 1
    }
}
```

---

## Multiplexed context

A context is multiplexed when it resolves to more than one runtime lane.

```text
ContextSet.frames.len() > 1
```

So Chataigne 1.x Multiplex becomes:

```text
context dimensions
  -> context resolution
  -> multiple ContextFrames
  -> multiple Processor runtime lanes
```

Multiplex is not a Formula.

Multiplexed execution is a property of resolved context.

---

## Examples

Single-lane context:

```text
scene = act_1_scene_3

ContextSet:
  1 frame
```

Multiplexed single-dimension context:

```text
fixture = [spot_01, spot_02, spot_03]

ContextSet:
  frame 1: fixture = spot_01
  frame 2: fixture = spot_02
  frame 3: fixture = spot_03
```

Multiplexed multi-dimensional context:

```text
fixture = [spot_01, spot_02]
layer = [front, back]
```

With Cartesian policy:

```text
frame 1: fixture = spot_01, layer = front
frame 2: fixture = spot_01, layer = back
frame 3: fixture = spot_02, layer = front
frame 4: fixture = spot_02, layer = back
```

With Zip policy:

```text
frame 1: fixture = spot_01, layer = front
frame 2: fixture = spot_02, layer = back
```

---

# 11. Processor runtime lanes

A Processor has one Formula Instance but can run across many context lanes.

```rust
pub struct ProcessorRuntime {
    pub processor_id: ProcessorId,
    pub compiled_formula: Arc<CompiledAlchemistGraph>,
    pub lanes: Vec<ProcessorLaneRuntime>,
}

pub struct ProcessorLaneRuntime {
    pub context: ContextFrame,
    pub memory: AlchemistMemory,
    pub last_status: ProcessorLaneStatus,
}
```

Execution:

```text
for each lane:
  evaluate same compiled Formula
  use lane context
  use lane memory
  emit contextual intents
```

Important rule:

```text
Same authored Formula.
Same compiled graph.
Different context per lane.
Different memory per lane.
Different emitted intents per lane.
```

---

# 12. Context-aware ANodes

Context-aware ANodes expose accumulated context to formulas.

Examples:

```text
CurrentContextANode
ContextValueANode
ContextualInputSourceANode
ContextualCommandTargetANode
ContextDimensionANode
ContextReducerANode
```

Example Mapping graph:

```text
InputSourceANode
  -> FilterChainANode
  -> ContextualCommandTargetANode
  -> ConsequencesANode
```

Runtime:

```text
lane 1:
  fixture = spot_01
  output target = spot_01.intensity

lane 2:
  fixture = spot_02
  output target = spot_02.intensity

lane 3:
  fixture = spot_03
  output target = spot_03.intensity
```

---

# 13. Runtime intents

No graph node should directly perform side effects.

Wrong:

```text
ANode sends OSC directly.
ANode launches Sequence directly.
ANode changes State directly.
```

Correct:

```text
ANode emits intent.
Runtime collects intents.
Arbiter resolves conflicts.
Dispatcher performs final side effect.
```

Every intent carries context.

```rust
pub struct RuntimeIntent {
    pub origin: IntentOrigin,
    pub context: ContextFrame,
    pub payload: RuntimeIntentPayload,
}
```

Chataigne command intent:

```rust
pub struct CommandIntent {
    pub origin: IntentOrigin,
    pub context: ContextFrame,
    pub target: CommandTargetRef,
    pub payload: CommandPayload,
    pub priority: i32,
    pub policy: CommandPolicy,
}
```

---

# 14. Contextual transitions

Transitions must define how they interact with context.

```rust
pub enum TransitionContextMode {
    Global,
    PerContext,
    AnyContext,
    AllContexts,
}
```

## Global

One transition affects the whole active State.

Example:

```text
Emergency Stop
```

## PerContext

Each context lane transitions independently.

Example:

```text
Each fixture transitions from Calibrating to Ready independently.
```

## AnyContext

If any lane satisfies the guard, one global transition fires.

Example:

```text
If any panic input fires, enter Panic State.
```

## AllContexts

All lanes must satisfy the guard.

Example:

```text
When all fixtures are calibrated, enter Show Ready.
```

---

# 15. Compilation pipeline

The Formula compiler must support Managed ANodes and surfaces.

Pipeline:

```text
1. Validate authored Formula graph.

2. Collect surface contributions from ANodes.

3. Build Formula Surface.
   Formula decides section order and relevance.

4. Validate surface bindings.

5. Lower Managed ANodes.
   ConditionsManager -> compare/reducer graph
   FilterChain -> filter sequence graph
   Consequences -> command intent graph

6. Lower Composite ANodes.

7. Build lowered graph.

8. Solve types.

9. Validate context contract.

10. Detect cycles.

11. Compile dense runtime graph.

12. Build debug source map back to authored ANodes and managed items.
```

Runtime diagnostics must point back to user-facing concepts.

Example:

```text
Runtime error:
  invalid command target

Source:
  Processor: Fader To Intensity
  Formula: Mapping
  Surface section: Outputs
  Managed ANode: Consequences
  Output item: Command #3
```

---

# 16. Type system

Alchemist should support VFX-Graph-style dynamic reshaping, but only at edit/compile time.

Example:

```text
Add<T>

default:
  T = Float

connect Vec3 to input A:
  T = Vec3
  input B becomes Vec3
  output becomes Vec3
```

Rules:

```text
No type solving during runtime ticks.

Forced type has priority over inferred type.

App-specific types are registered through a value type registry.

App-specific types should use facets/capabilities.
```

Formula runtime should use resolved, compiled types.

---

# 17. Implementation roadmap

## Phase 1 — Crate boundaries

Create or prepare:

```text
golden_alchemist
golden_statechart
chataigne_alchemist
chataigne_state_machine
```

Definition of done:

```text
golden_alchemist builds without Chataigne.
golden_statechart builds without Chataigne.
Chataigne-specific types are registered from Chataigne crates.
```

---

## Phase 2 — State owns ProcessorManager

Refactor State ownership:

```text
State
  -> ProcessorManager
      -> Processor[]
      -> ProcessorGroup[]
```

Implement:

```text
ProcessorManager
ProcessorGroup
execution ordering
enabled flags
basic lifecycle propagation
```

Definition of done:

```text
State no longer owns processors directly.
ProcessorManager runs direct processors and grouped processors.
ProcessorGroup acts as execution scope.
```

---

## Phase 3 — Formula abstraction

Implement:

```text
AlchemistFormula
AlchemistFormulaInstance
FormulaFamily
FormulaSurface
FormulaContextContract
FormulaRef
FormulaVersion
```

Definition of done:

```text
Processor owns FormulaInstance.
FormulaInstance owns graph instance.
Formula families are Action, Mapping, CustomUser.
```

---

## Phase 4 — ANode registry and graph model

Implement:

```text
ANodeDeclaration
ANodeRegistry
AlchemistGraph
ANodeInstance
AEdge
ANodeSignature
socket descriptors
graph serialization
```

Definition of done:

```text
Graph can add/remove/connect ANodes.
ANode catalog can be queried.
Graph can be serialized and deserialized.
```

---

## Phase 5 — ANode surface contributions

Implement:

```text
SurfaceContribution
SurfaceSection
SurfaceItem
SurfaceEdit
SurfaceSource
ANodeDeclaration::surface()
ANodeDeclaration::apply_surface_edit()
```

Definition of done:

```text
ANodes can expose inspector-facing controls.
Formula assembles surface contributions.
Processor inspector can edit ANode-backed surface items.
Surface edits go through normal edit pipeline.
```

---

## Phase 6 — Managed ANodes

Implement first Managed ANodes:

```text
ConditionsManagerANode
FilterChainANode
ConsequencesANode
OutputMappingANode
```

Definition of done:

```text
Action can expose conditions and consequences.
Mapping can expose input, filters, and outputs.
User can configure Action/Mapping without opening graph editor.
```

---

## Phase 7 — Lowering pipeline

Implement:

```text
Managed ANode lowering
Composite ANode lowering
LoweredGraph
DebugSourceMap
```

Definition of done:

```text
Managed ANodes compile into executable graph fragments.
Runtime diagnostics map back to managed items.
```

---

## Phase 8 — Type solving and compilation

Implement:

```text
ValueTypeRegistry
FacetRegistry
TypeConstraint
TypeVar
TypeBindings
forced bindings
inferred bindings
VFX-style reshaping
CompiledAlchemistGraph
dense ExecNodeId schedule
runtime memory layout
```

Definition of done:

```text
No type solving in runtime tick.
Dynamic reshaping works at edit/compile time.
Compiled graph uses dense runtime IDs.
```

---

## Phase 9 — Runtime evaluator

Implement:

```text
AlchemistRuntime
EvaluationCtx
AlchemistMemory
RuntimeIntent
RuntimeDiagnostic
DebugValueSample
```

Definition of done:

```text
Compiled graph evaluates.
Stateful nodes have memory.
Trigger is distinct from Bool.
Runtime emits intents, not side effects.
```

---

## Phase 10 — Action Formula

Implement built-in Action Formula.

Internal graph:

```text
ConditionsManagerANode
  -> TriggerModeANode
  -> CooldownANode
  -> BranchANode
      true  -> ConsequencesANode
      false -> ConsequencesANode
```

Definition of done:

```text
Ready-to-use Action Processor exists.
Inspector exposes conditions and consequences.
Advanced graph editor can open the Formula graph.
Commands emit intents.
```

---

## Phase 11 — Mapping Formula

Implement built-in Mapping Formula.

Internal graph:

```text
InputSourceANode
  -> FilterChainANode
  -> OutputMappingANode
  -> ConsequencesANode
```

Definition of done:

```text
Ready-to-use Mapping Processor exists.
Inspector exposes input, filters, outputs, and options.
Advanced graph editor can open the Formula graph.
Mapping works with or without context.
```

---

## Phase 12 — Accumulating context engine

Implement:

```text
ContextStack
ContextLayer
ContextSource
ContextBinding
ContextContribution
ContextFrame
ContextSet
ContextResolver
same-dimension refinement
explicit override
combine policies
```

Definition of done:

```text
State context accumulates with ProcessorManager context.
ProcessorManager context accumulates with ProcessorGroup context.
ProcessorGroup context accumulates with Processor context.
Same-dimension context refines by default.
Explicit override is required for replacement.
```

---

## Phase 13 — Multiplexed runtime lanes

Implement:

```text
ContextSet -> ProcessorLaneRuntime[]
per-lane memory
per-lane context
contextual intents
lane diagnostics
```

Definition of done:

```text
Processor can run one Formula across many context lanes.
Each lane has separate memory.
Each intent carries full accumulated context.
Processor inspector shows lane count.
```

---

## Phase 14 — Context-aware ANodes

Implement:

```text
CurrentContextANode
ContextValueANode
ContextualInputSourceANode
ContextualCommandTargetANode
ContextReducerANode
```

Definition of done:

```text
Formula can read accumulated context.
Mapping can resolve contextual targets.
Action can emit contextual consequences.
```

---

## Phase 15 — Statechart and contextual transitions

Implement or integrate:

```text
Statechart
MetaState
Transition
TransitionContextMode
Global
PerContext
AnyContext
AllContexts
```

Definition of done:

```text
Transitions work globally and per context.
Any/all context aggregation works.
Debug trace explains transition selection.
```

---

## Phase 16 — Intent arbitration and dispatch

Implement:

```text
RuntimeIntent collection
CommandIntent
StateIntent
SequenceIntent
DashboardIntent
priority policy
conflict policy
deterministic arbitration
dispatch phase
```

Definition of done:

```text
No side effect happens during graph evaluation.
Conflicting commands resolve deterministically.
Debug trace explains winner/loser.
```

---

## Phase 17 — Svelte 5 UI

Implement UI surfaces:

```text
State Machine editor
ProcessorManager inspector
ProcessorGroup inspector
Processor inspector
Alchemist graph editor
Formula Surface renderer
Context preview
Runtime lane preview
Diagnostics
Debug traces
```

Processor inspector must show:

```text
Formula family
Formula surface
context stack
multiplexed execution status
lane count
diagnostics
Open Alchemist Graph
```

Definition of done:

```text
Action and Mapping are usable without opening graph editor.
Advanced graph editor remains available.
Context and lanes are visible and debuggable.
```

---

# 18. First vertical slice

Build this first:

```text
State: Live Scene
  context:
    scene = act_1_scene_3

  ProcessorManager

    ProcessorGroup: Fixtures
      context:
        fixture = [spot_01, spot_02, spot_03]

      Processor: Intensity Mapping
        Formula: Mapping

        Inspector:
          Input:
            MIDI fader 1

          Filters:
            map 0..127 to 0..1
            smooth 100 ms

          Outputs:
            contextual fixture intensity

        Runtime:
          lane 1:
            scene = act_1_scene_3
            fixture = spot_01

          lane 2:
            scene = act_1_scene_3
            fixture = spot_02

          lane 3:
            scene = act_1_scene_3
            fixture = spot_03
```

This vertical slice validates:

```text
State owns ProcessorManager.
ProcessorManager owns ProcessorGroup.
ProcessorGroup contributes context.
Contexts inherit and accumulate.
Processor owns FormulaInstance.
Formula family is Mapping.
Formula Surface drives Processor inspector.
Managed ANodes expose filters and outputs.
Same Formula runs across multiple context lanes.
Each lane has separate memory.
Each emitted intent carries accumulated context.
Multiplexed execution exists without a Multiplex Formula.
```

---

# 19. Final invariants

These are the non-negotiable architecture rules:

```text
State contains ProcessorManager.

ProcessorManager contains Processors and ProcessorGroups.

ProcessorGroup is an execution and context scope.

Processor is an instance of an Alchemist Formula.

Formula families are Action, Mapping, and Custom User.

Multiplex is not a Formula family.

Multiplexed execution is produced by context dimensions resolving to multiple runtime lanes.

Conditions, filters, outputs, sequence commands, and state commands are Formula components, not top-level Formula families.

ANodes may expose inspector surface contributions.

Formula Surface is assembled from Formula-level layout and ANode surface contributions.

Processor inspector edits the Formula Surface.

Alchemist graph editor edits the authored graph.

Managed ANodes provide Chataigne 1.x usability.

Atomic ANodes provide advanced flexibility.

Managed and Composite ANodes lower into executable graph fragments.

Contexts are inherited and accumulating.

Different dimensions accumulate naturally.

Same dimensions refine by default.

Explicit override is required for replacement.

One compiled Formula can run across many context lanes.

Each context lane has independent runtime memory.

Every emitted intent carries accumulated context.

Side effects are dispatched only after deterministic arbitration.

Debugging must explain context, lanes, surfaces, lowered graph, and emitted intents.
```

---

# 20. Final summary

The final architecture is:

```text
State Machine activates States.

Each State owns a ProcessorManager.

ProcessorManager runs Processors and ProcessorGroups.

ProcessorGroups can add context and create multiplexed execution.

Processors are Formula Instances.

Formulas are reusable Alchemist graph models.

Action and Mapping are the main ready-to-use Formula families.

Custom User Formula is the advanced extensibility path.

ANodes can be atomic, domain-specific, managed, or composite.

Managed ANodes expose rich inspector controls and lower into executable graph fragments.

Contexts accumulate through Project, State, ProcessorManager, ProcessorGroup, and Processor.

Resolved contexts produce one or more ContextFrames.

One ContextFrame equals one runtime lane.

Multiple ContextFrames mean multiplexed execution.

The same compiled Formula runs once per lane with separate memory.

ANodes emit contextual intents.

The runtime arbitrates intents and dispatches side effects deterministically.
```

This gives Chataigne2 the right structure:

```text
simple enough to preserve Chataigne 1.x immediacy
deep enough for advanced custom graph logic
generic enough for reuse across Golden Core apps
explicit enough for deterministic runtime execution
context-aware enough to replace old Multiplex with a cleaner model
```
