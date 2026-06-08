# Chataigne2 / Golden Core — Final Alchemist & State Machine Implementation Blueprint

This is the canonical implementation direction.

The key decision is:

> **Alchemist should be a reusable Golden Core ecosystem engine, not a Chataigne-only subsystem.**

It may live outside the `golden_core` crate itself, but it should be designed as a reusable engine that any Golden Core app can depend on.

Chataigne2 then becomes one application that registers Chataigne-specific value types, ANodes, Processor models, and State Machine behavior on top of the reusable Alchemist runtime.

---

# 1. Final architecture

```text
golden_alchemist
  reusable typed node-graph engine
  app-agnostic
  no Chataigne-specific Module / Sequence / State types

golden_statechart
  reusable hierarchical statechart engine
  app-agnostic
  optional, but strongly recommended as separate reusable engine

chataigne_alchemist
  Chataigne-specific Alchemist integration
  registers Module / Sequence / State / CommandTarget value types
  registers Chataigne-specific ANodes

chataigne_state_machine
  Chataigne-specific State Machine UX and behavior
  owns State / MetaState / Processor product concepts
  uses golden_statechart + golden_alchemist

chataigne_processor_library
  built-in Processor models:
    Action
    Mapping
    Input Condition
    Multiplex
    Sequence Launcher
    State Controller
    Conductor
```

The conceptual hierarchy becomes:

```text
Statechart
  └─ State / MetaState
      └─ Processor
          └─ Alchemist Graph
              └─ ANode
```

Runtime hierarchy:

```text
Statechart Engine
  decides active scopes

Processor Runtime Matrix
  decides active Processor execution set

Alchemist Runtime
  evaluates compiled graphs

Command Intent Arbiter
  resolves and dispatches side effects
```

---

# 2. Crate strategy

## 2.1 `golden_alchemist`

This should be a standalone reusable crate in the Golden ecosystem.

Suggested location options:

```text
Option A:
  golden_core/crates/alchemist

Option B:
  sibling repository/crate:
    golden_alchemist

Option C:
  Chataigne2 workspace member:
    crates/golden_alchemist
  then later promoted to shared package
```

Preferred:

```text
crates/golden_alchemist
```

inside the shared Golden workspace or as a sibling reusable crate.

It should be reusable by any Golden Core app, not only Chataigne2.

## 2.2 `golden_alchemist` must not know Chataigne

Forbidden inside `golden_alchemist`:

```rust
ModuleId
SequenceId
StateId from Chataigne
CommandTarget
OSC-specific types
MIDI-specific types
DMX-specific types
Chataigne processor categories
```

Allowed:

```rust
StableRef
ValueTypeId
FacetId
ExtensionValue
CommandIntent-like generic output events, if kept abstract
RuntimeEvent
Graph
ANode
Compiler
Type solver
Diagnostics
```

## 2.3 Feature flags

Recommended feature layout:

```toml
[features]
default = ["serde"]
serde = ["dep:serde"]
rhai = []
protocol = []
debug_trace = []
golden_core_integration = []
```

The crate should work without Chataigne.

`golden_core_integration` can provide adapters to Golden Core concepts, but the core graph engine should remain independent.

---

# 3. Core principle: authored graph is not runtime graph

Never execute the authored graph directly.

There are three layers:

```text
Authored Graph
  stable IDs, layout, labels, config, exposed surface
  persisted and edited

Resolved Graph
  signatures, type bindings, diagnostics
  produced by validation/type solving

Compiled Graph
  dense runtime IDs, topological order, memory layout
  evaluated during ticks
```

This avoids:

- runtime type solving;
- runtime topology sorting;
- string lookups in hot paths;
- UI data leaking into execution;
- persistence being coupled to scheduler internals.

---

# 4. `golden_alchemist` public modules

Recommended module structure:

```rust
golden_alchemist
  ids
  value
  registry
  graph
  node
  typing
  compile
  runtime
  diagnostics
  expose
  serialize
  library
```

Detailed:

```text
ids
  AlchemistGraphId
  ANodeId
  ANodeTypeId
  SocketId
  ExecNodeId
  ValueTypeId
  FacetId
  ExposedDeclId

value
  RuntimeValue
  ValueTypeDescriptor
  ValueStorageKind
  ExtensionValue
  StableRef

registry
  ValueTypeRegistry
  FacetRegistry
  ANodeRegistry
  ConversionRegistry

graph
  AlchemistGraph
  ANodeInstance
  AEdge
  ANodeConfig
  GraphLayout

node
  ANodeDeclaration
  ANodeSignature
  InputSocketDecl
  OutputSocketDecl
  ExecutionKind

typing
  TypeConstraint
  TypeVar
  TypeBindings
  TypeSolveCtx
  TypeSolveResult
  ResolvedGraph

compile
  CompileCtx
  CompiledAlchemistGraph
  CompiledExecNode
  RuntimeStateLayout
  RuntimeSubscription

runtime
  AlchemistRuntime
  AlchemistMemory
  EvaluationCtx
  RuntimeEvent
  RuntimeOutput

diagnostics
  Diagnostic
  DiagnosticSeverity
  DiagnosticOrigin

expose
  ExposedSurface
  ExposedParam
  ExposedInput
  ExposedOutput
  ExposedAction

serialize
  DTOs and migration helpers

library
  reusable primitive ANodes
```

---

# 5. ID model

Do not use `String` IDs in hot runtime paths.

Use stable authored IDs and dense runtime IDs.

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ANodeId(Uuid);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ExecNodeId(u32);

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ANodeTypeId(SmolStr);

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ValueTypeId(SmolStr);

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FacetId(SmolStr);

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ExposedDeclId(SmolStr);
```

Authored graph IDs are stable across persistence.

Compiled IDs are regenerated on compile.

---

# 6. Value system

## 6.1 Runtime values

```rust
pub enum RuntimeValue {
    Unit,
    Bool(bool),
    Trigger(TriggerValue),
    Int(i64),
    Float(f64),
    String(Arc<str>),
    Vec2([f64; 2]),
    Vec3([f64; 3]),
    Color(ColorValue),
    Duration(Duration),
    Ref(StableRef),
    Extension(ExtensionValue),
}
```

## 6.2 Trigger is not Bool

This is non-negotiable.

```rust
pub struct TriggerValue {
    pub fired: bool,
    pub edge_id: u64,
    pub logical_tick: u64,
}
```

A bool means:

```text
condition is currently true
```

A trigger means:

```text
event fired once at a precise logical tick
```

They are different value types.

## 6.3 App-specific values through descriptors

`golden_alchemist` must not hard-code app concepts.

It should expose descriptors:

```rust
pub struct ValueTypeDescriptor {
    pub id: ValueTypeId,
    pub label: String,
    pub storage: ValueStorageKind,
    pub facets: Vec<FacetId>,
    pub conversions: Vec<ConversionRule>,
    pub default_value: RuntimeValueFactory,
    pub ui: ValueTypeUiDescriptor,
}
```

Chataigne registers:

```rust
registry.register_value_type(ValueTypeDescriptor {
    id: ValueTypeId::new("chataigne.module"),
    label: "Module".into(),
    storage: ValueStorageKind::StableRef,
    facets: vec![
        FacetId::new("node_ref"),
        FacetId::new("command_target"),
    ],
    conversions: vec![],
    default_value: module_default_ref,
    ui: module_ui_descriptor,
});

registry.register_value_type(ValueTypeDescriptor {
    id: ValueTypeId::new("chataigne.sequence"),
    label: "Sequence".into(),
    storage: ValueStorageKind::StableRef,
    facets: vec![
        FacetId::new("launchable"),
        FacetId::new("time_source"),
    ],
    conversions: vec![],
    default_value: sequence_default_ref,
    ui: sequence_ui_descriptor,
});

registry.register_value_type(ValueTypeDescriptor {
    id: ValueTypeId::new("chataigne.state"),
    label: "State".into(),
    storage: ValueStorageKind::StableRef,
    facets: vec![
        FacetId::new("activatable"),
    ],
    conversions: vec![],
    default_value: state_default_ref,
    ui: state_ui_descriptor,
});
```

## 6.4 Facets

Facets are what make app-specific types composable.

Example:

```rust
InputSocketDecl {
    id: SocketId::new("target"),
    label: "Target",
    constraint: TypeConstraint::Facet(FacetId::new("command_target")),
}
```

This allows any app-specific value implementing `command_target` to connect.

---

# 7. Type system

## 7.1 Separate type from value

Do not use runtime values as type representatives.

Correct:

```rust
RuntimeValue::Float(1.0)

ResolvedValueType {
    id: ValueTypeId::new("float"),
    layout: RuntimeValueLayout::InlineF64,
}
```

Incorrect:

```rust
ANodeValue::Float(0.0) as type
```

## 7.2 Type constraints

```rust
pub enum TypeConstraint {
    Any,
    Exact(ValueTypeId),
    Facet(FacetId),
    Primitive,
    NumericLike,
    Generic(TypeVar),
    OneOf(Vec<TypeConstraint>),
}
```

## 7.3 Type variables

```rust
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TypeVar(pub SmolStr);
```

Use named variables, not chars.

Examples:

```text
T
TValue
TTarget
TNumeric
```

## 7.4 VFX-Graph-style reshaping

A generic Add node:

```text
Add<T>
where T: NumericLike

inputs:
  a: T
  b: T

outputs:
  result: T
```

When connecting `Vec3` to `a`:

```text
T = Vec3
b becomes Vec3
result becomes Vec3
```

When forcing `T = Float`, connecting `Vec3` becomes invalid unless an explicit conversion exists.

## 7.5 Type binding precedence

```text
ForcedByUser
  > ForcedByModel
  > InferredFromConnection
  > Default
```

```rust
pub enum TypeBindingSource {
    Default,
    InferredFromConnection,
    ForcedByModel,
    ForcedByUser,
}
```

## 7.6 Conversion policy

Allowed automatically:

```text
Int -> Float
scalar broadcast if the ANode explicitly supports it
typed reference upcast to facet
registered non-lossy app conversion
```

Forbidden automatically:

```text
Float -> Int
String -> Float
State -> String
Module -> Sequence
arbitrary object conversions
command payload conversions
lossy narrowing
```

---

# 8. Authored graph model

```rust
pub struct AlchemistGraph {
    pub id: AlchemistGraphId,
    pub nodes: SlotMap<ANodeId, ANodeInstance>,
    pub edges: Vec<AEdge>,
    pub exposed: ExposedSurface,
    pub layout: GraphLayout,
    pub metadata: GraphMetadata,
}

pub struct ANodeInstance {
    pub id: ANodeId,
    pub type_id: ANodeTypeId,
    pub label: String,
    pub config: ANodeConfig,
    pub type_bindings: TypeBindings,
    pub forced_type_bindings: TypeBindings,
    pub ui: ANodeUiState,
}

pub struct AEdge {
    pub from: OutputSocketRef,
    pub to: InputSocketRef,
}
```

The authored graph is persisted.

It includes:

- user labels;
- graph layout;
- comments;
- groups;
- forced type bindings;
- exposed surface;
- app references.

It does not include:

- compiled schedule;
- runtime memory;
- live values;
- resolved caches;
- debug samples.

---

# 9. ANode declaration model

ANodes should be declared through reusable declarations.

```rust
pub trait ANodeDeclaration: Send + Sync {
    fn type_id(&self) -> ANodeTypeId;

    fn label(&self) -> &'static str;

    fn category(&self) -> &'static str;

    fn signature(
        &self,
        ctx: &SignatureCtx,
        instance: &ANodeInstance,
        bindings: &TypeBindings,
    ) -> ANodeSignature;

    fn solve_types(
        &self,
        ctx: &TypeSolveCtx,
        instance: &ANodeInstance,
    ) -> TypeSolveResult;

    fn compile(
        &self,
        ctx: &CompileCtx,
        instance: &ANodeInstance,
        resolved: &ResolvedANodeSignature,
    ) -> Result<CompiledANode, CompileDiagnostic>;
}
```

## 9.1 Execution kind

```rust
pub enum ExecutionKind {
    Pure,
    Stateful,
    EventSource,
    EffectEmitter,
    Subgraph,
}
```

## 9.2 Primitive reusable ANodes

`golden_alchemist` should ship with app-agnostic nodes:

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
Edge
Gate
Latch
MapRange
Clamp
Smooth
PreviousValue
DelayOneTick
Select
Switch
DebugLog
SubgraphInput
SubgraphOutput
```

No Chataigne-specific nodes here.

---

# 10. Chataigne-specific ANodes

Chataigne registers these in `chataigne_alchemist`:

```text
ModuleValueInput
ModuleEventInput
CommandTargetRef
CommandBuilder
CommandIntentOutput
SequenceRef
SequenceControlIntent
StateRef
StateTransitionIntentOutput
ProcessorRef
ProcessorEnabled
StateActive
TimeMachineClock
DashboardInput
DashboardOutput
```

These are not in `golden_alchemist`.

They are app-level plugins using the reusable engine.

---

# 11. Exposed surface

The exposed surface is the stable public contract of a Processor.

```rust
pub struct ExposedSurface {
    pub params: Vec<ExposedParam>,
    pub inputs: Vec<ExposedInput>,
    pub outputs: Vec<ExposedOutput>,
    pub actions: Vec<ExposedAction>,
}

pub struct ExposedParam {
    pub decl_id: ExposedDeclId,
    pub label: String,
    pub description: Option<String>,
    pub target: ANodeFieldPath,
    pub value_type: ValueTypeSpec,
    pub ui: ParamUiHints,
}
```

External bindings must target `ExposedDeclId`, not raw internal `ANodeId`.

This protects:

```text
dashboards
presets
scripts
processor model upgrades
copy/paste
undo/redo
protocol clients
shared processor libraries
```

If an internal ANode is deleted and an exposed param breaks, emit a diagnostic instead of silently corrupting the Processor.

---

# 12. Compilation pipeline

Compile whenever the graph or relevant registry changes.

```text
1. Validate graph structure.
2. Resolve ANode declarations.
3. Build initial signatures.
4. Solve type variables.
5. Apply forced type bindings.
6. Validate conversions.
7. Validate app references.
8. Detect cycles.
9. Allow cycles only through explicit stateful/delay ANodes.
10. Build dense execution nodes.
11. Allocate memory layout.
12. Build subscription table.
13. Build output intent routes.
14. Build debug source map.
15. Store diagnostics.
```

## 12.1 Compiled graph

```rust
pub struct CompiledAlchemistGraph {
    pub exec_nodes: Vec<CompiledExecNode>,
    pub topo_order: Vec<ExecNodeId>,
    pub state_layout: RuntimeStateLayout,
    pub input_bindings: Vec<InputBinding>,
    pub output_routes: Vec<OutputRoute>,
    pub subscriptions: Vec<RuntimeSubscription>,
    pub debug_map: DebugSourceMap,
}
```

## 12.2 Hot path rule

Do not do this:

```rust
for node_id in &execution_schedule {
    graph.nodes.get_mut(node_id).unwrap().evaluate(ctx);
}
```

Do this:

```rust
for exec_id in &compiled.topo_order {
    runtime.evaluate_exec_node(*exec_id, ctx, memory, outputs);
}
```

No string lookup.

No topology calculation.

No type inference.

No UI data.

---

# 13. Runtime evaluator

```rust
pub struct AlchemistRuntime {
    pub compiled: Arc<CompiledAlchemistGraph>,
    pub memory: AlchemistMemory,
}

pub struct EvaluationCtx<'a> {
    pub logical_tick: u64,
    pub delta_time: Duration,
    pub events: &'a [RuntimeEvent],
    pub inputs: &'a RuntimeInputSnapshot,
    pub registries: &'a RuntimeRegistries,
}

pub struct RuntimeOutput {
    pub intents: Vec<RuntimeIntent>,
    pub diagnostics: Vec<RuntimeDiagnostic>,
    pub debug_samples: Vec<DebugValueSample>,
}
```

Runtime intents are generic in `golden_alchemist`.

Chataigne maps them into:

```text
CommandIntent
StateTransitionIntent
SequenceIntent
DashboardIntent
LogIntent
```

---

# 14. Command intent boundary

No ANode should directly mutate external systems.

Wrong:

```text
ANode sends OSC packet directly.
ANode launches sequence directly.
ANode changes state directly.
```

Correct:

```text
ANode emits intent.
Engine collects intents.
Arbiter resolves conflicts.
Dispatcher applies final side effects.
```

## 14.1 Chataigne command intent

```rust
pub struct CommandIntent {
    pub origin: IntentOrigin,
    pub target: CommandTargetRef,
    pub payload: CommandPayload,
    pub priority: i32,
    pub policy: CommandPolicy,
    pub logical_tick: u64,
}

pub enum CommandPolicy {
    FireAndForget,
    LastWriterWins,
    HighestPriorityWins,
    Queue,
    DropIfSameAsPrevious,
    RateLimit(Duration),
    Blend(BlendPolicy),
}
```

This is where Conductor eventually lives.

---

# 15. Processor implementation

A Processor is a Chataigne concept using Alchemist internally.

```rust
pub struct ProcessorNode {
    pub id: ProcessorId,
    pub label: String,
    pub model_ref: Option<ProcessorModelRef>,
    pub graph: AlchemistGraph,
    pub exposed: ExposedSurface,
    pub lifecycle: ProcessorLifecyclePolicy,
    pub memory_policy: ProcessorMemoryPolicy,
    pub command_policy: ProcessorCommandPolicy,
    pub diagnostics: Vec<Diagnostic>,
}
```

Runtime:

```rust
pub struct ProcessorRuntime {
    pub id: ProcessorId,
    pub compiled: Option<Arc<CompiledAlchemistGraph>>,
    pub memory: AlchemistMemory,
    pub active: bool,
    pub dirty: ProcessorDirtyFlags,
    pub subscriptions: Vec<RuntimeSubscription>,
}
```

## 15.1 Processor lifecycle

```rust
pub enum ProcessorLifecycleEvent {
    StateEnter(StateId),
    StateExit(StateId),
    ProcessorEnable,
    ProcessorDisable,
    ProjectStart,
    ProjectStop,
}
```

Memory policy:

```rust
pub enum ProcessorMemoryPolicy {
    ResetOnStateEnter,
    ResetOnProcessorEnable,
    PreserveWhileProjectOpen,
    PreserveAcrossStateReentry,
}
```

---

# 16. Processor models

Built-in Actions, Mappings, Conditions, etc. should be Processor models.

```rust
pub struct ProcessorModel {
    pub id: ProcessorModelId,
    pub version: u32,
    pub label: String,
    pub category: ProcessorCategory,
    pub graph_template: AlchemistGraphTemplate,
    pub exposed_surface: ExposedSurface,
    pub migrations: Vec<ProcessorModelMigration>,
}
```

Instance:

```rust
pub struct ProcessorModelInstance {
    pub model_id: ProcessorModelId,
    pub model_version: u32,
    pub graph_instance: AlchemistGraph,
    pub overrides: FxHashSet<ExposedDeclId>,
}
```

This gives you:

```text
built-in processors
user-created processors
shared processor libraries
versioned upgrades
override tracking
```

---

# 17. Built-in Chataigne Processor models

## 17.1 Input Condition

```text
ModuleValueInput
  -> Compare / PatternMatch / Threshold
  -> Edge / Hold / Debounce
  -> TriggerOutput
```

Exposed:

```text
source
comparison mode
expected value / threshold
edge mode
debounce
trigger output
```

## 17.2 Action

```text
InputCondition
  -> Gate
  -> Cooldown
  -> CommandBuilder
  -> CommandIntentOutput
```

Exposed:

```text
trigger
target
payload
cooldown
priority
command policy
```

## 17.3 Mapping

```text
ModuleValueInput
  -> Filter
  -> MapRange
  -> Clamp
  -> Smooth
  -> Convert
  -> CommandBuilder
  -> CommandIntentOutput
```

Exposed:

```text
source
target
input range
output range
smoothing
send policy
```

## 17.4 Multiplex

```text
N inputs
  -> Selector / Router / Blend
  -> M command targets
```

## 17.5 Sequence Launcher

```text
Trigger
  -> SequenceRef
  -> SequenceIntentOutput
```

## 17.6 State Controller

```text
Trigger
  -> StateRef
  -> StateTransitionIntentOutput
```

Never transition immediately from graph evaluation.

## 17.7 Conductor

Initial version:

```text
CommandIntent inputs
  -> PriorityDomain
  -> TargetLock
  -> ArbitrationPolicy
  -> CommandIntent output
```

Long-term version:

```text
Conductor becomes state-machine-level arbitration policy
with a Processor-like UI.
```

---

# 18. `golden_statechart`

The State Machine also deserves a reusable engine.

Like Alchemist, it can be separate from `golden_core` but reusable across Golden apps.

```text
golden_statechart
  generic hierarchical statechart runtime
  no Chataigne-specific Processor or Module types
```

Chataigne integrates it with Processors and Alchemist.

## 18.1 Statechart model

```rust
pub struct Statechart {
    pub id: StatechartId,
    pub root_region: RegionId,
    pub regions: SlotMap<RegionId, Region>,
    pub states: SlotMap<StateId, StateNode>,
    pub transitions: Vec<Transition>,
    pub active: ActiveConfiguration,
}

pub struct Region {
    pub id: RegionId,
    pub parent_state: Option<StateId>,
    pub states: Vec<StateId>,
    pub initial: Option<StateId>,
}

pub struct StateNode {
    pub id: StateId,
    pub label: String,
    pub parent_region: RegionId,
    pub kind: StateKind,
    pub ui_layout: StateUiLayout,
}

pub enum StateKind {
    Leaf,
    Composite {
        regions: Vec<RegionId>,
    },
}
```

For v1:

```text
Composite states support one child region.
Parallel regions are reserved for later.
```

## 18.2 Active configuration

```rust
pub struct ActiveConfiguration {
    pub active_leaf_paths: Vec<StatePath>,
    pub active_scopes: FxHashSet<StateId>,
    pub history: FxHashMap<StateId, StateHistory>,
}
```

Do not store canonical runtime active state as `is_active` on every state.

That can be derived for UI.

## 18.3 Transition selection

```text
1. Evaluate eligible transitions from deepest active states upward.
2. Keep transitions whose guard fired or evaluated true.
3. Sort by source depth, priority, stable creation order.
4. Select transition.
5. Compute least common ancestor.
6. Exit states up to LCA.
7. Enter states down to target.
8. Emit lifecycle events.
```

## 18.4 Meta-state semantics

```rust
pub enum HistoryPolicy {
    None,
    Shallow,
    Deep,
}

pub enum EnterPolicy {
    InitialChild,
    LastActiveChild,
    Explicit(StateId),
}
```

---

# 19. Chataigne state machine integration

Chataigne State owns Processors.

The reusable `golden_statechart` state nodes do not need to know what a Processor is.

Use app-side attachment:

```rust
pub struct ChataigneStateMachine {
    pub chart: Statechart,
    pub processors_by_state: FxHashMap<StateId, Vec<ProcessorId>>,
    pub transitions: Vec<ChataigneTransition>,
}
```

A transition guard can be an Alchemist graph:

```rust
pub struct ChataigneTransition {
    pub transition_id: TransitionId,
    pub guard_graph: Option<AlchemistGraph>,
    pub effect_graph: Option<AlchemistGraph>,
}
```

---

# 20. Runtime tick

Final tick sequence:

```text
1. Apply pending edits.
2. Recompile dirty Alchemist graphs.
3. Sample module/input snapshots.
4. Inject runtime events.
5. Evaluate transition guards.
6. Select state transitions.
7. Emit StateExit lifecycle events.
8. Update active state configuration.
9. Emit StateEnter lifecycle events.
10. Update active Processor execution matrix.
11. Evaluate active Processor Alchemist graphs.
12. Collect command/state/sequence intents.
13. Arbitrate command conflicts.
14. Dispatch final side effects.
15. Emit UI/protocol/debug deltas.
```

This tick order should be explicit and non-reentrant.

---

# 21. Active execution matrix

Use Gemini’s active bucket idea, but with Processor granularity.

```rust
pub struct RuntimeExecutionMatrix {
    pub active_scopes: ActiveConfiguration,
    pub active_processors: Vec<ProcessorId>,
    pub processors_by_state: FxHashMap<StateId, Vec<ProcessorId>>,
    pub dirty_processors: FxHashSet<ProcessorId>,
}
```

When state changes:

```text
statechart computes exit vector
statechart computes enter vector
processor layer receives lifecycle events
execution matrix updates active processors
only active processors are evaluated
```

---

# 22. Persistence

Persist authored data:

```text
Statechart structure
State metadata
Transition metadata
Processor model references
Processor instance overrides
Alchemist authored graphs
Exposed surfaces
Type bindings
Forced type bindings
Graph layout
State layout
Comments/groups
```

Do not persist runtime data:

```text
compiled execution plans
runtime memory
runtime socket cache
resolved type cache
last tick values
debug samples
active processor vector
temporary diagnostics
```

## 22.1 Schema versions

```rust
ProjectSchemaVersion
StatechartSchemaVersion
AlchemistSchemaVersion
ProcessorModelVersion
ANodeTypeVersion
ValueTypeVersion
```

## 22.2 Migrations

```rust
pub trait ANodeMigration {
    fn from_version(&self) -> u32;
    fn to_version(&self) -> u32;
    fn migrate(&self, node: &mut SerializedANode) -> MigrationResult;
}
```

Model migrations must preserve `ExposedDeclId`s.

---

# 23. UI implementation with Svelte 5

Use Svelte 5 runes only.

No legacy event syntax.

No hand-maintained protocol duplicates when generated types are available.

## 23.1 Stores

```text
statechartStore.svelte.ts
  states, regions, transitions, active state IDs, selected state

processorStore.svelte.ts
  processors, exposed surfaces, diagnostics, selected processor

alchemistGraphStore.svelte.ts
  graph nodes, edges, local drag state, selection

alchemistTypeStore.svelte.ts
  value types, facets, socket compatibility

alchemistLibraryStore.svelte.ts
  ANode catalog, Processor model catalog

runtimeDebugStore.svelte.ts
  live values, transition traces, command intent traces
```

## 23.2 UI principles

```text
Rust owns canonical project state.
Svelte owns local interaction state.
Graph edits are sent as explicit edits.
UI receives deltas, not full snapshots when avoidable.
Large graph UI must be virtualized.
Socket compatibility comes from Rust diagnostics/type metadata.
```

## 23.3 Statechart UI

Composite states can render recursively, but runtime should stay normalized.

Good UI shape:

```ts
export interface StateUiNode {
    id: string;
    label: string;
    active: boolean;
    kind: 'leaf' | 'composite';
    layout: {
        x: number;
        y: number;
        width?: number;
        height?: number;
    };
    childRegionIds: string[];
}
```

Use an index:

```ts
class StatechartStore {
    statesById = $state(new Map<string, StateUiNode>());
    activeStateIds = $state(new Set<string>());

    applyDelta(delta: StatechartDelta) {
        const state = this.statesById.get(delta.stateId);
        if (!state) return;

        if (delta.kind === 'active_changed') {
            state.active = delta.active;
        }
    }
}
```

Avoid recursively scanning the full tree for every delta.

---

# 24. Diagnostics

## 24.1 Compile diagnostics

Required:

```text
Missing ANode declaration
Missing app-specific value type
Missing app reference
Type mismatch
Unresolved type variable
Invalid conversion
Cycle without delay node
Missing exposed target
Broken model override
Invalid transition target
Ambiguous transition
```

## 24.2 Runtime diagnostics

Required:

```text
Command target unavailable
Command rejected
Processor budget exceeded
Graph evaluation failed
Transition conflict
State reference stale
Module input stale
Sequence unavailable
```

## 24.3 Debug tools

Required:

```text
Live edge values
Last trigger fired marker
Per-ANode execution count
Processor command intent monitor
State transition trace
Lifecycle enter/exit trace
Type inference explanation
Command arbitration explanation
```

The debugger must answer:

```text
Why did this command fire?
Why did this transition happen?
Why did this connection fail?
Why did this Processor not run?
Why did this command lose arbitration?
```

---

# 25. Performance rules

Non-negotiable:

```text
No type inference in the tick path.
No graph topology sorting in the tick path.
No string lookup in hot graph evaluation.
No UI layout dependency in runtime.
No command dispatch during graph evaluation.
No full graph scan for sparse input events.
No full UI snapshot after small edits.
No hidden mutation outside the edit queue.
```

Dirty reasons:

```rust
pub enum DirtyReason {
    GraphStructureChanged,
    EdgeChanged,
    ANodeConfigChanged,
    TypeRegistryChanged,
    AppReferenceChanged,
    ExposedSurfaceChanged,
    ProcessorModelChanged,
    StateHierarchyChanged,
    TransitionChanged,
}
```

---

# 26. Implementation roadmap

## Phase 0 — Create crates and boundaries

Create:

```text
crates/golden_alchemist
crates/golden_statechart
crates/chataigne_alchemist
crates/chataigne_state_machine
```

Definition of done:

```text
golden_alchemist builds without Chataigne
golden_statechart builds without Chataigne
Chataigne-specific types exist only in Chataigne crates
```

## Phase 1 — `golden_alchemist` IDs, values, registries

Implement:

```text
IDs
RuntimeValue
ValueTypeDescriptor
FacetId
ValueTypeRegistry
ANodeTypeId
ANodeRegistry
Diagnostics
```

Definition of done:

```text
primitive value types registered
custom extension value type can be registered in a test
facet compatibility works
```

## Phase 2 — Authored graph model

Implement:

```text
AlchemistGraph
ANodeInstance
AEdge
GraphLayout
ExposedSurface
serialization
basic graph edits
```

Definition of done:

```text
create graph
add node
remove node
connect sockets
disconnect sockets
serialize/deserialize
```

## Phase 3 — ANode declarations

Implement primitive ANodes:

```text
Constant
Add
Compare
BoolAnd
BoolOr
BoolNot
Edge
Gate
MapRange
Clamp
DebugLog
```

Definition of done:

```text
ANode catalog query works
signatures work
unit tests per ANode
```

## Phase 4 — Type solver

Implement:

```text
TypeConstraint
TypeVar
TypeBindings
forced bindings
inferred bindings
diagnostics
VFX-style reshaping
```

Definition of done:

```text
Add defaults to Float
Vec3 connected to Add.a reshapes Add.b and output to Vec3
forced Float rejects Vec3
facet-based socket accepts registered app value
```

## Phase 5 — Compiler

Implement:

```text
graph validation
topological sorting
cycle diagnostics
dense ExecNodeId schedule
runtime memory layout
debug source map
```

Definition of done:

```text
compiled graph has no string lookup requirement
cycle is reported
delay node can allow feedback
```

## Phase 6 — Runtime evaluator

Implement:

```text
AlchemistRuntime
EvaluationCtx
AlchemistMemory
RuntimeOutput
RuntimeDiagnostic
DebugValueSample
```

Definition of done:

```text
pure math graph evaluates
edge trigger fires once
stateful memory works
runtime diagnostics propagate
```

## Phase 7 — `golden_statechart`

Implement:

```text
Statechart
Region
StateNode
Composite state
Transition
ActiveConfiguration
LCA enter/exit algorithm
history policy
```

Definition of done:

```text
leaf transitions work
composite state initial child works
deepest-first transition selection works
enter/exit lifecycle events emitted
```

## Phase 8 — Chataigne Alchemist integration

Implement Chataigne value types:

```text
Module
ModuleEndpoint
CommandTarget
Sequence
State
Processor
DashboardTarget
```

Implement Chataigne ANodes:

```text
ModuleValueInput
CommandBuilder
CommandIntentOutput
SequenceIntentOutput
StateTransitionIntentOutput
StateActive
```

Definition of done:

```text
a graph can read a module value and emit a command intent
a graph can emit a state transition intent
generic Alchemist remains Chataigne-free
```

## Phase 9 — Processor layer

Implement:

```text
ProcessorNode
ProcessorRuntime
Processor lifecycle
compiled graph cache
exposed surface UI mapping
memory policy
diagnostics
```

Definition of done:

```text
Processor owns graph
Processor compiles graph
Processor evaluates only when active
exposed params appear in UI model
```

## Phase 10 — Chataigne State Machine integration

Implement:

```text
ChataigneStateMachine
processors_by_state
transition guard Alchemist graph
active Processor matrix
intent collection
```

Definition of done:

```text
state activation controls Processors
transition guards work
meta-state initial child works
state enter/exit updates Processor lifecycle
```

## Phase 11 — Command intent arbitration

Implement:

```text
CommandIntent
CommandPolicy
priority
target conflict resolution
dispatch phase
debug trace
```

Definition of done:

```text
two Processors targeting same command resolve deterministically
debugger explains winner/loser
no command is dispatched during graph evaluation
```

## Phase 12 — Built-in Processor models

Implement:

```text
Input Condition
Action
Mapping
Multiplex
Sequence Launcher
State Controller
Conductor v0
```

Definition of done:

```text
each built-in behavior is an Alchemist graph template
each has exposed surface
expert can open internal graph
```

## Phase 13 — Svelte 5 UI

Implement:

```text
Statechart canvas
Processor inspector
Alchemist graph editor
ANode palette
socket compatibility
exposed surface editor
diagnostics panel
runtime debug overlay
```

Definition of done:

```text
all components use Svelte 5 runes
large graph interactions are local and batched
Rust validates graph edits
UI receives deltas
```

---

# 27. First vertical slice

Build this first:

```text
One StateMachine
  Two States
    Each State has one Processor
      Each Processor has one Alchemist graph

Graph:
  FakeModuleBoolInput
    -> RisingEdge
    -> DebugCommandIntent

Transition:
  FakeModuleBoolInput
    -> RisingEdge
    -> StateTransitionIntent
```

Required pieces:

```text
golden_alchemist:
  Bool
  Trigger
  Constant
  Edge
  DebugLog
  graph compile
  graph runtime

golden_statechart:
  two leaf states
  one transition
  enter/exit lifecycle

chataigne integration:
  fake module input
  debug command intent
  Processor wrapper
```

This validates:

```text
reusable graph engine
type solving
trigger semantics
state lifecycle
Processor activation
command intent boundary
basic UI inspection
```

Do not start with the full Action/Mapping/Multiplex library.

Start with this vertical slice.

---

# 28. Final non-negotiables

1. **Alchemist is reusable across Golden Core apps.**
2. **Alchemist is not Chataigne-specific.**
3. **Chataigne-specific value types are registered, not hard-coded.**
4. **Processor is the Chataigne-facing behavior capsule.**
5. **ANodes are not GoldenCore tree nodes.**
6. **Authored graphs are compiled before execution.**
7. **No type solving happens during runtime ticks.**
8. **No command is dispatched directly from graph evaluation.**
9. **Triggers are not bools.**
10. **Meta-states use explicit statechart semantics.**
11. **Exposed surfaces are stable public contracts.**
12. **Runtime uses dense IDs, not string lookups.**
13. **UI uses Svelte 5 runes and generated protocol types.**
14. **Built-in Actions/Mappings/Conditions are Processor models built from Alchemist graphs.**

---

# 29. Final summary

The final implementation should be:

```text
golden_alchemist:
  reusable typed visual graph compiler/runtime

golden_statechart:
  reusable hierarchical lifecycle engine

chataigne_alchemist:
  Chataigne-specific types and ANodes

chataigne_state_machine:
  Chataigne product behavior: States, MetaStates, Processors, transitions

processor models:
  reusable graph templates with stable exposed surfaces

runtime:
  statechart activates Processors
  Processors evaluate compiled Alchemist graphs
  graphs emit intents
  arbiter dispatches final side effects
```

This gives Chataigne2 its final philosophy:

> **Chataigne2 is not just a state machine with processors. It is a statechart-controlled orchestration environment where reusable, typed Alchemist graphs define behavior, Processors package that behavior for users, and all side effects pass through deterministic intent arbitration.**
