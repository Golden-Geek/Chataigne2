# Locked architecture decisions

Use these as non-negotiable requirements for the coding agent.

## 1. Formula properties are explicitly typed

A Formula property has a stable declared type. Its default value must conform to that type, but the default value must not define the compiled output type.

```text
PropertyDecl {
  id
  label
  declared_value_type
  default_value
  editor_hints
}
```

## 2. Processor instances must not materialize Formula graphs

No cloned graph plus patched config path for normal runtime.

The current core still has `AlchemistFormula::materialize`, which clones the formula graph and writes instance override values into node config fields. That is the architecture to replace, not extend. ([GitHub][2])

## 3. Stateful node memory is private per processor lane

Default memory identity:

```text
ProcessorId + ContextKey + NodeStateSlot
```

No sharing between lanes unless a future explicit node/formula policy asks for it.

## 4. Context lanes use stable IDs, not numeric indices

Indices are display/order metadata only. Memory identity must survive reorder, insert, delete, filter, and dynamic multiplex changes.

## 5. Lane memory is sparse and lazy

Never allocate a dense multiplex cross-product by default.

```text
Only allocate memory for lanes actually evaluated.
Only evaluate multiple lanes when formula inputs/outputs/effects require it.
```

## 6. State machine transitions are global

This supersedes the previous transition-lane wording.

There is no such thing as:

```text
statechart active state for lane 0
statechart active state for lane 1
statechart active state for lane 2
```

There is only:

```text
one Statechart active configuration
one transition-resolution pass
one authoritative truth of active/inactive states
```

Transition guards may use Alchemist graphs, but those graphs evaluate in a **global state-machine context**, not in processor multiplex lanes.

## 7. Debug samples and command origins are lane-aware where applicable

Processor-originated outputs, commands, diagnostics, and debug samples must include `processor_id` and optional `context_key`.

Transition-originated samples and effects should include `transition_id`, but **no processor lane key** unless they are explicitly referencing a processor-lane result as input/debug metadata.

## 8. ANode output value preview is first-class UI

The graph editor must be able to show output values directly on ANodes/sockets so the user can track value evolution across the graph.

This must be runtime-backed, typed, throttled, and lane-aware. It must not be a fake JS-side simulation.

---

# Revised coding plan for GPT-5.5 Extra High

## Phase execution protocol

Each phase is a bounded work unit and should end in one supercommit. The
supercommit should contain the implementation, tests, generated artifacts, and
docs needed for that phase to stand alone in review.

Do not leave a completed phase uncommitted. Once a phase reaches `Done`, prepare
and create its supercommit before starting the next phase.

Each phase must update the phase progress ledger before handoff:

```text
Status values:
  Todo        not started
  In progress active in the current chat
  Blocked     started but waiting on an explicit external decision or missing dependency
  Done        implemented, verified, and ready to be represented by a supercommit

Supercommit values:
  Pending     no phase commit has been prepared yet
  Ready       changes are complete and can be committed as one reviewable unit
  Committed   the supercommit exists
```

Use a fresh prompt chat per phase. The new chat should start by reading this
ledger, the phase section, and any phase-specific docs produced by earlier
phases. Do not carry half-remembered implementation details between phases.

## Phase progress ledger

| Phase | Status | Supercommit | Notes |
| --- | --- | --- | --- |
| 0 - Baseline and stale-test capture | Todo | Pending | Current stale manager-node tests are marked ignored until Phase 13 replaces removed pre-manager-ref behavior. |
| 1 - Architecture doc first | Done | Ready | Added `docs/ALCHEMIST_FORMULA_RUNTIME.md`. |
| 2 - Property schema and runtime property slots | Done | Ready | Added schema-driven property slots, runtime property frames, and non-materialized processor compile path. |
| 3 - Split compiled graph from memory | Done | Ready | Added `CompiledAlchemistFormula`, `FormulaCompileKey`, lower-level `evaluate_compiled_graph`, and shared compiled formula use in processor runtime. |
| 4 - Generalized state layout | Done | Ready | Added `NodeStateLayout`, declaration-owned state slot sizing, compiler wiring, and layout tests. |
| 5 - Context axes, stable context keys, sparse lane memory | Todo | Pending | Not started. |
| 6 - Formula and processor lane analysis | Todo | Pending | Not started. |
| 7 - Refactor `ProcessorRuntime` | Todo | Pending | Not started. |
| 8 - Statechart transitions stay global | Todo | Pending | Not started. |
| 9 - Lane-aware command arbitration for processors | Todo | Pending | Not started. |
| 10 - Runtime-backed ANode output preview | Todo | Pending | Not started. |
| 11 - Protocol DTOs and TypeScript generation | Todo | Pending | Not started. |
| 12 - Formula editor UX model | Todo | Pending | Not started. |
| 13 - Manager nodes: no silent fake behavior | Todo | Pending | Not started. |
| 14 - Performance and scalability pass | Todo | Pending | Not started. |

## Mission

Refactor Alchemist + State Machine toward this model:

```text
Formula
  shared authored recipe
  graph + typed property declarations + default values
  no processor-specific runtime state

CompiledFormula
  optimized reusable executable plan
  compiled from formula graph + property schema
  shared across processor instances

ProcessorInstance
  formula reference
  property overrides / bindings
  lifecycle policy
  command policy

ProcessorLane
  one actually-used processor/context combination
  selected by sparse context analysis

LaneMemory
  persistent state memory for one stateful processor lane

Statechart
  single global truth of active states
  not multiplied by processor lanes
```

This matches the repo direction: reusable Alchemist/runtime/statechart mechanics belong in `golden_alchemist_core`, while Chataigne processor policy, arbitration, value types, protocol DTOs, and product-specific behavior stay in `src/state_machine`; the State Machine panel and DTO adapters remain app-owned UI. ([GitHub][3])

The repository rules support this kind of clean break: it explicitly prefers clean architecture over smallest diff, warns against compatibility glue, requires correct boundaries, and requires Rust/TypeScript protocol types to have one source of truth. ([GitHub][4])

---

# Phase 0 — Baseline and stale-test capture

## Goal

Record the current failing or incomplete state before refactoring.

## Run

```bash
git submodule update --init --recursive

cargo fmt
cargo test

cargo fmt --manifest-path submodules/golden_alchemist_core/Cargo.toml
cargo test --manifest-path submodules/golden_alchemist_core/Cargo.toml

cd src-ui
npm run codegen:state-machine-protocol
npm run check
npm run lint
```

## Expected findings

The current transition runtime compiles guard graphs into `guard_runtimes` and evaluates them once per tick before calling `chart.step(...)`; `effect_graph` exists in the transition model but is not currently part of the visible compiled runtime path. ([GitHub][1])

The current core runtime still couples compiled graph and memory inside `AlchemistRuntime`, while debug samples only identify `exec_node`, `output_slot`, `value`, and `logical_tick`. That is not enough for selected-lane graph previews. ([GitHub][5])

## Acceptance

The PR starts with an honest note:

```text
Baseline captured.
Known stale tests listed.
Known nonfunctional manager-node stubs listed.
Known transition effect_graph gap listed.
```

---

# Phase 1 — Add the architecture doc first

## Add

```text
docs/ALCHEMIST_FORMULA_RUNTIME.md
```

## Must document

```text
Formula
CompiledFormula
PropertyDecl
PropertySlot
PropertyFrame
ProcessorInstance
ProcessorExecutionPlan
ContextAxis
ContextKey
ProcessorLane
LaneMemory
GlobalStateMachineContext
DebugPreviewSession
ANodeOutputPreview
```

## Required invariant text

```text
The statechart has one active-state truth.
Processor multiplex dimensions do not clone or fork the statechart.
Only processor formula execution is lane-aware.
Transition guard/effect evaluation is global.
```

## Required optimization matrix

```text
Stateless + no lane-varying data:
  evaluate once, no persistent memory

Stateless + lane-varying data:
  evaluate per used lane, no persistent memory

Stateful + no lane-varying data:
  one lane, one memory frame

Stateful + lane-varying data:
  sparse memory per actually evaluated lane
```

## Required warning

```text
Stateless does not always mean single evaluation.
It only means no persistent lane memory.
```

---

# Phase 2 — Property schema and runtime property slots in `golden_alchemist`

## Goal

Property nodes must read typed runtime slots, not compiled constants.

## Add core types

```rust
pub struct FormulaPropertyDecl {
    pub id: FormulaPropertyId,
    pub label: String,
    pub description: Option<String>,
    pub value_type: ValueTypeId,
    pub default_value: RuntimeValue,
    pub ui: PropertyUiHints,
}

pub struct FormulaPropertySchema {
    pub properties: IndexMap<FormulaPropertyId, FormulaPropertyDecl>,
}

pub struct CompiledFormulaProperty {
    pub id: FormulaPropertyId,
    pub slot: FormulaPropertySlotId,
    pub value_type: ValueTypeId,
    pub default_value: RuntimeValue,
}

pub struct RuntimePropertyFrame {
    values: Box<[RuntimeValue]>,
}
```

## Replace normal materialization path

Current formula instances use `FormulaOverrides` and `materialize()` to clone graph and patch config values. That must not remain the normal processor runtime path. ([GitHub][2])

New runtime path:

```text
Formula graph stays unchanged.
Formula compiles once.
Processor overrides resolve into RuntimePropertyFrame.
PropertyGetter reads from RuntimePropertyFrame by slot.
```

## Add operation

```rust
CompiledNodeOperation::ReadProperty(FormulaPropertySlotId)
```

## Compile-time validation

```text
Property default must conform to declared type.
Processor constant override must conform to declared type.
Property node referencing missing property id is invalid.
Property node output type comes from property schema, not default value.
```

## Tests

```text
property_decl_rejects_invalid_default
property_node_type_comes_from_schema
property_node_reads_default_from_runtime_frame
property_node_reads_processor_override_from_runtime_frame
changing_processor_override_does_not_recompile_formula
```

---

# Phase 3 — Split compiled graph from memory

## Goal

Compiled formulas are reusable; memory is supplied per evaluation lane.

Current `AlchemistRuntime` owns both `Arc<CompiledAlchemistGraph>` and `AlchemistMemory`. That is convenient but too coarse for processor lanes and shared compiled formulas. ([GitHub][5])

## Add lower-level evaluator

```rust
pub struct EvaluationFrame<'a, 'ctx> {
    pub ctx: &'a EvaluationCtx<'ctx>,
    pub properties: &'a RuntimePropertyFrame,
    pub context: &'a RuntimeContextFrame,
    pub debug: &'a mut DebugCaptureSink,
}

pub fn evaluate_compiled_graph(
    compiled: &CompiledAlchemistGraph,
    memory: &mut AlchemistMemory,
    frame: EvaluationFrame<'_, '_>,
) -> RuntimeOutput;
```

Keep `AlchemistRuntime` as a convenience wrapper only if needed, but processor runtime must not depend on one `AlchemistRuntime` per processor.

## Add compiled formula wrapper

```rust
pub struct CompiledAlchemistFormula {
    pub formula_ref: FormulaRef,
    pub graph: Arc<CompiledAlchemistGraph>,
    pub properties: CompiledFormulaPropertySchema,
    pub analysis: FormulaAnalysis,
    pub diagnostics: Vec<Diagnostic>,
}
```

## Compile cache key

```rust
pub struct FormulaCompileKey {
    pub formula_id: FormulaId,
    pub formula_version: u32,
    pub graph_revision: u64,
    pub property_schema_hash: u64,
    pub node_registry_hash: u64,
    pub value_type_registry_hash: u64,
}
```

Do **not** include processor override values.

## Tests

```text
two_processors_share_same_compiled_formula_arc
changing_override_does_not_change_compile_key
stateless_graph_has_no_persistent_state_memory
stateful_graph_gets_state_layout
```

---

# Phase 4 — Generalized state layout

## Goal

Temporal smoothing, delay, counters, debounce, hysteresis, and rolling buffers must not be forced into one opaque state slot.

## Add

```rust
pub enum NodeStateLayout {
    Stateless,
    RuntimeValues(usize),
}
```

## Extend node declaration API

```rust
fn state_layout(
    &self,
    instance: &ANodeInstance,
    resolved: &ResolvedANodeSignature,
) -> NodeStateLayout {
    match self.execution_kind() {
        ExecutionKind::Stateful => NodeStateLayout::RuntimeValues(1),
        _ => NodeStateLayout::Stateless,
    }
}
```

## Compiler rule

Replace any hardcoded:

```rust
state_size = 1 if Stateful else 0
```

with:

```rust
state_size = declaration.state_layout(...).slot_count()
```

## Tests

```text
stateful_node_can_request_three_slots
multiple_stateful_nodes_sum_state_slots
stateless_node_has_empty_state_slice
```

---

# Phase 5 — Context axes, stable context keys, and sparse lane memory

## Goal

Represent actual processor dimensions without dense matrix allocation.

## Core reusable types

```rust
pub struct ContextAxisId(/* stable */);

pub struct ContextItemId(/* stable */);

pub struct ContextKey {
    pub parts: SmallVec<[ContextKeyPart; 4]>,
}

pub struct ContextKeyPart {
    pub axis: ContextAxisId,
    pub item: ContextItemId,
}
```

## App-owned Chataigne provider

```rust
pub trait ProcessorContextProvider {
    fn available_axes(&self, processor_id: ProcessorId) -> AxisSet;

    fn iter_context_keys(
        &self,
        processor_id: ProcessorId,
        axes: &AxisSet,
    ) -> Box<dyn Iterator<Item = ContextKey> + '_>;

    fn resolve_context_value(
        &self,
        key: &ContextKey,
        axis: &ContextAxisId,
        path: &ContextValuePath,
    ) -> Option<RuntimeValue>;
}
```

## Sparse lane pool

```rust
pub enum LaneRuntimePool {
    Stateless,
    Stateful(IndexMap<ContextKey, AlchemistMemory>),
}
```

## Memory lifecycle

Existing processor memory policies must operate on the whole sparse lane pool:

```text
ResetOnStateEnter:
  clear all lane memories for processor

ResetOnProcessorEnable:
  clear all lane memories on enable

PreserveAcrossStateReentry:
  keep lane memory unless context item disappears

PreserveWhileProjectOpen:
  keep sparse lane memory while project remains open
```

## Tests

```text
processor_under_multiplex_without_context_reference_runs_one_lane
processor_bound_to_10_item_context_runs_10_lanes
nested_10_by_3_context_runs_30_lanes_only_when referenced
stateful_lanes_have_independent_memory
stateless_multi_lane_processor_allocates_no persistent memory
context_reorder_preserves_memory_by_stable_item_id
removed_context_item_evicts_lane_memory
```

---

# Phase 6 — Formula and processor lane analysis

## Goal

Compute required lanes from actual dependencies, not placement alone.

## Formula analysis

```rust
pub struct FormulaAnalysis {
    pub has_stateful_nodes: bool,
    pub has_effect_emitters: bool,
    pub explicit_context_axes: AxisSet,
    pub state_axes: AxisSet,
    pub effect_axes: AxisSet,
}
```

## Processor binding analysis

```rust
pub struct ProcessorBindingAnalysis {
    pub property_axes: AxisSet,
    pub input_axes: AxisSet,
    pub output_axes: AxisSet,
}
```

## Execution plan

```rust
pub struct ProcessorExecutionPlan {
    pub processor_id: ProcessorId,
    pub available_axes: AxisSet,
    pub required_eval_axes: AxisSet,
    pub required_memory_axes: AxisSet,
    pub strategy: ProcessorExecutionStrategy,
}

pub enum ProcessorExecutionStrategy {
    SingleStateless,
    MultiStateless,
    SingleStateful,
    MultiStatefulSparse,
}
```

## Rule

```text
required_eval_axes =
  property binding axes
  ∪ explicit context read axes
  ∪ input source axes
  ∪ output/effect routing axes

required_memory_axes =
  if formula has stateful nodes:
    axes reaching stateful nodes
  else:
    empty
```

## Important

A processor can be under a multiplex and still run once if it references nothing lane-varying and emits no lane-scoped side effects.

A stateless processor can still need many evaluations if its outputs differ by lane.

---

# Phase 7 — Refactor `ProcessorRuntime`

## Goal

Replace:

```text
ProcessorRuntime owns one materialized graph runtime
```

with:

```text
ProcessorRuntime owns shared compiled formula + sparse lane pool
```

## Target structure

```rust
pub struct ProcessorRuntime {
    pub id: ProcessorId,
    pub compiled: Option<Arc<CompiledAlchemistFormula>>,
    pub plan: Option<ProcessorExecutionPlan>,
    pub lanes: LaneRuntimePool,
    pub active: bool,
    pub dirty: ProcessorDirtyFlags,
    pub subscriptions: Vec<RuntimeSubscription>,
    pub diagnostics: Vec<Diagnostic>,
}
```

## Evaluation flow

```rust
for context_key in context_provider.iter_context_keys(processor_id, &plan.required_eval_axes) {
    let property_frame = RuntimePropertyFrame::resolve(
        &compiled.properties,
        &processor.formula_instance.bindings,
        &context_key,
        context_provider,
    );

    let runtime_output = match &mut self.lanes {
        LaneRuntimePool::Stateless => {
            evaluate_compiled_graph_stateless(...)
        }
        LaneRuntimePool::Stateful(lanes) => {
            let memory = lanes
                .entry(context_key.clone())
                .or_insert_with(|| AlchemistMemory::for_graph(&compiled.graph));

            evaluate_compiled_graph(...)
        }
    };

    attach processor_id + context_key to:
      diagnostics
      debug samples
      runtime intents
      ANode output preview samples
}
```

## Tests

```text
two_processors_share_compiled_formula
two_processors_have_independent_lane_memory
one_processor_two_lanes_have_independent_counter_state
override_change_rebuilds_property_frame_not_formula
```

---

# Phase 8 — Statechart transitions stay global

## Goal

Preserve the “one truth” state machine model.

## Required invariant

```text
Statechart active state is never keyed by ContextKey.
Transition guard evaluation is never expanded into processor lanes.
Transition effect evaluation is never expanded into processor lanes.
```

## Runtime model

```rust
pub struct StateMachineTransitionRuntime {
    pub transition_id: TransitionId,
    pub guard: Option<GlobalCompiledGraphRuntime>,
    pub effect: Option<GlobalCompiledGraphRuntime>,
}
```

or, if effect graphs are deferred:

```rust
pub struct StateMachineTransitionRuntime {
    pub transition_id: TransitionId,
    pub guard: Option<GlobalCompiledGraphRuntime>,
}
```

## Global context

Introduce an explicit type so nobody accidentally reuses processor lane APIs:

```rust
pub struct GlobalStateMachineContextFrame {
    pub logical_tick: u64,
    pub active_scopes: IndexSet<StateId>,
    pub inputs: RuntimeInputSnapshot,
    pub events: Vec<RuntimeEvent>,
}
```

## Guard semantics

```text
Each transition guard evaluates once per tick.
A transition is enabled if its global guard returns/fires true.
Statechart priority/conflict resolution chooses the transition.
```

Current tick logic already follows the outline of “collect fired transition IDs, then call `chart.step` once.” That conceptual shape should remain. ([GitHub][1])

## Effect semantics

Effect graphs, when implemented, run once after the transition fires.

```text
No per-lane transition effects.
No transition effect cross-product.
No lane-specific active state.
```

If a transition needs data produced by processors, that data must be exposed through an explicit global aggregation or manager result, not by implicitly iterating processor lanes from inside the transition.

## Tests

```text
guard_evaluates_once_even_when_active_processors_have_30_lanes
statechart_has_one_active_configuration
transition_effect_runs_once_after_transition
transition_effect_does_not_receive_processor_context_key
processor_lanes_do_not_create_parallel_statechart_truths
```

---

# Phase 9 — Lane-aware command arbitration for processors

## Goal

Processor-originated commands must remain explainable when many lanes emit commands.

## Origin model

```rust
pub enum IntentOrigin {
    Processor {
        processor_id: ProcessorId,
        context_key: Option<ContextKey>,
    },
    Transition {
        transition_id: TransitionId,
    },
    System,
}
```

`context_key` is optional because an uncontexted processor has one default lane.

## Arbitration rules

Keep conflict resolution deterministic:

```text
Group by command target.
Sort by processor priority, manager order, processor order, context key order, logical tick.
Apply policy.
Preserve losing origins for diagnostics/debug.
```

## Tests

```text
arbitration_preserves_processor_lane_origin
last_writer_wins_is_deterministic_across_lanes
queue_policy_keeps_all_lane_intents
rate_limit_can_be_target_scoped_or_origin_scoped
```

---

# Phase 10 — Runtime-backed ANode output preview

## Goal

The Formula editor should show live or preview output values directly on ANodes/sockets, so the user can understand value evolution across the graph.

This must support:

```text
Formula default preview
Processor single-lane preview
Processor selected-lane preview under multiplex
```

## Core debug data

Current debug samples identify only `exec_node`, `output_slot`, `value`, and `logical_tick`. That is insufficient for editor lane preview, because the same formula node can run for many processor/context lanes. ([GitHub][5])

Add a richer preview/debug sample:

```rust
pub struct ANodeOutputPreviewSample {
    pub formula_id: FormulaId,
    pub processor_id: Option<ProcessorId>,
    pub context_key: Option<ContextKey>,
    pub author_node_id: ANodeId,
    pub exec_node: ExecNodeId,
    pub output_socket: SocketId,
    pub value_type: ValueTypeId,
    pub value: RuntimeValue,
    pub logical_tick: u64,
    pub status: OutputPreviewStatus,
}

pub enum OutputPreviewStatus {
    Live,
    DefaultPreview,
    Stale,
    Error,
    Suppressed,
    Unavailable,
}
```

The sample must include both `author_node_id` and `exec_node`. UI needs author node IDs; runtime/debug internals often need exec node IDs.

## Capture modes

Do not capture every value for every lane by default.

```rust
pub enum DebugCaptureMode {
    Off,

    FormulaDefaults {
        formula_id: FormulaId,
        history_len: usize,
    },

    ProcessorLane {
        processor_id: ProcessorId,
        context_key: Option<ContextKey>,
        history_len: usize,
    },

    SelectedNodes {
        processor_id: Option<ProcessorId>,
        context_key: Option<ContextKey>,
        nodes: IndexSet<ANodeId>,
        history_len: usize,
    },
}
```

Default editor behavior:

```text
Formula selected:
  FormulaDefaults preview

Uncontexted processor selected:
  ProcessorLane with default context

Multiplexed processor selected:
  ProcessorLane for selected ContextKey

No graph/editor visible:
  DebugCaptureMode::Off
```

## UI components

Add focused Svelte 5 components/stores:

```text
src-ui/src/lib/state_machine/preview/anodeOutputPreviewStore.svelte.ts
src-ui/src/lib/state_machine/preview/formulaPreviewSessionStore.svelte.ts
src-ui/src/lib/state_machine/components/ANodeOutputValueChip.svelte
src-ui/src/lib/state_machine/components/ANodeOutputPreviewOverlay.svelte
src-ui/src/lib/state_machine/components/ProcessorLaneSelector.svelte
src-ui/src/lib/state_machine/components/FormulaPreviewModeSelector.svelte
```

Do not grow `AlchemistEditorPanel.svelte` into a larger orchestration object. The repo rules explicitly prefer focused UI stores and warn against god objects. ([GitHub][4])

## Visual behavior

Each ANode output socket should be able to show:

```text
last value
value type
fresh/stale/error state
trigger pulse state
optional short history
```

Suggested display:

```text
Float:       0.742
Bool:        true / false
Trigger:     fired / idle
String:      clipped text
Array/List:  [10] or preview of first values
Command:     target + short payload summary
Unit:        hidden by default
Error:       diagnostic marker
```

For evolution tracking, add a small history ring buffer:

```rust
pub struct OutputPreviewHistory {
    pub samples: VecDeque<ANodeOutputPreviewSample>,
    pub max_len: usize,
}
```

The UI can render this as a compact value history or mini sparkline later, but the first implementation should prioritize correctness and readability over visual complexity.

## Important UI invariant

Changing the selected lane only changes preview/debug focus.

It must not mutate:

```text
Formula graph
Formula property defaults
Processor property overrides
Runtime memory
```

## Tests

```text
formula_default_preview_shows_node_output_values
processor_preview_shows_override_resolved_values
multiplexed_processor_preview_filters_to_selected_lane
changing_selected_lane_changes_preview_samples_only
preview_capture_off_when_editor_not visible
large_graph_preview_does_not_capture_all_lanes
```

---

# Phase 11 — Protocol DTOs and TypeScript generation

## Goal

Expose context keys, lane summaries, output preview samples, and preview session state through canonical Rust DTOs.

The architecture doc says UI protocol types should have one source of truth, and AGENTS reinforces that Rust and TypeScript protocol declarations must not drift. ([GitHub][3])

## Add DTOs in `src/state_machine/src/protocol.rs`

```rust
pub struct ContextKeyDto {
    pub parts: Vec<ContextKeyPartDto>,
}

pub struct ContextKeyPartDto {
    pub axis_id: String,
    pub axis_label: String,
    pub item_id: String,
    pub item_label: String,
    pub index: Option<u32>,
}

pub struct ProcessorLaneSummaryDto {
    pub processor_id: ProcessorId,
    pub context_key: Option<ContextKeyDto>,
    pub label: String,
    pub has_memory: bool,
    pub last_tick: Option<u64>,
    pub diagnostics_count: usize,
}

pub enum FormulaPreviewModeDto {
    FormulaDefaults {
        formula_id: FormulaId,
    },
    ProcessorDefaultLane {
        processor_id: ProcessorId,
    },
    ProcessorLane {
        processor_id: ProcessorId,
        context_key: ContextKeyDto,
    },
}

pub struct ANodeOutputPreviewSampleDto {
    pub formula_id: FormulaId,
    pub processor_id: Option<ProcessorId>,
    pub context_key: Option<ContextKeyDto>,
    pub node_id: String,
    pub output_socket_id: String,
    pub value_type: String,
    pub value: RuntimeValueDto,
    pub logical_tick: u64,
    pub status: OutputPreviewStatusDto,
}
```

## Run

```bash
cd src-ui
npm run codegen:state-machine-protocol
npm run check
```

## Acceptance

No hand-maintained duplicate TypeScript protocol types.

---

# Phase 12 — Formula editor UX model

## Required behavior

### Clicking a Formula

```text
Mode:
  Formula default preview

Values:
  property defaults

Memory:
  ephemeral preview memory
  no live runtime mutation

Output preview:
  show ANode output values from default preview evaluation
```

### Clicking an uncontexted processor

```text
Mode:
  Processor default lane

Values:
  processor overrides / bindings

Memory:
  live lane memory if runtime exists

Output preview:
  show live output values for that processor
```

### Clicking a multiplexed processor

```text
Mode:
  Processor selected lane

UI:
  show lane selector

Values:
  processor overrides resolved for selected ContextKey

Memory:
  live selected-lane memory

Output preview:
  show only selected-lane values
```

### Editing

```text
Editing Formula graph:
  global to Formula

Editing Formula property default:
  global default

Editing Processor override:
  local to Processor instance

Changing selected lane:
  preview/debug only
```

## Acceptance

The UI must make it visually obvious which level is being edited:

```text
Formula recipe
Processor instance
Selected lane preview
```

---

# Phase 13 — Manager nodes: no silent fake behavior

## Goal

Manager nodes must either be real or visibly unfinished.

Current Chataigne Alchemist nodes include manager-style nodes, and earlier inspection showed placeholder behavior for conditions/inputs/outputs. Do not leave them silently returning fake defaults.

## Required choice for each manager node

Either:

```text
Implemented:
  real Chataigne behavior, tests, diagnostics, protocol output
```

or:

```text
Explicit WIP:
  diagnostic says unsupported/not implemented
  UI shows unavailable state
  tests assert the diagnostic
```

## Minimum useful implementation

```text
Inputs:
  resolve current context input values into ParamArray

Conditions:
  evaluate global condition manager when used by transition
  evaluate processor-lane condition only when used inside processor formula

Output Commands:
  emit lane-aware RuntimeIntent when used by processor
  emit global transition-origin RuntimeIntent when used by transition effect
```

The key distinction:

```text
Inside processor formula:
  context_key may exist

Inside transition graph:
  no context_key
```

---

# Phase 14 — Performance and scalability pass

## Hot-path rules

```text
No graph clone per processor instance.
No string lookup per property read.
No dense context matrix allocation.
No persistent lane memory for stateless formulas.
No all-lanes debug capture by default.
No unbounded output preview history.
No UI transport flood for every socket in large graphs.
```

## Implementation expectations

```text
Use slot IDs for hot property reads.
Use Arc<CompiledAlchemistFormula>.
Use SmallVec for short ContextKey parts.
Use sparse maps for lane memory.
Use deterministic ordering for arbitration and UI lane lists.
Use capture filters for preview/debug.
```

## Suggested stress tests

```text
compile one formula once, bind 10_000 processors
evaluate 10_000 stateless processors without persistent memory allocation
evaluate 1_000 stateful processors with sparse lane usage
preview selected lane only in a large graph
ensure compile count remains 1 for identical formula schema
```

---

# Final acceptance checklist

The task is complete when all of these are true:

```text
Formula properties are explicit typed declarations.
Property defaults no longer define compiled output type.
Property nodes read runtime property slots.
Processor instances do not materialize formula graphs.
Compiled formulas are shared across processor instances.
Processor overrides resolve into RuntimePropertyFrame.
Processor lanes use stable ContextKey identity.
Lane memory is sparse and private per processor/context.
Stateless multi-lane formulas allocate no persistent memory.
Stateful multi-lane formulas allocate memory only for used lanes.
Statechart active state is global and not context-keyed.
Transition guards evaluate once globally.
Transition effects, if implemented, run once globally.
Processor-originated debug samples are lane-aware.
Processor-originated command intents are lane-aware.
Transition-originated intents are global transition-origin intents.
ANode output value preview is runtime-backed.
Preview can show Formula defaults, processor default lane, or selected processor lane.
Changing selected lane does not edit formula or processor data.
Protocol DTOs are generated from Rust.
Svelte UI uses focused runes stores/components.
Stale tests are replaced with architecture tests.
Docs explain the runtime contract.
```

The single most important first milestone remains:

```text
One Formula
Two Processor instances
Same Arc<CompiledFormula>
Different property frames
Different lane memories
One stateful node proving isolation
ANode output preview showing the selected runtime lane
Statechart active state unchanged and global
```

That milestone proves the architecture is correct before adding the full multiplex UI, transition-effect cleanup, and manager-node production semantics.

[1]: https://github.com/Golden-Geek/Chataigne2/blob/main/src/state_machine/src/state_machine.rs "Chataigne2/src/state_machine/src/state_machine.rs at main · Golden-Geek/Chataigne2 · GitHub"
[2]: https://github.com/Golden-Geek/golden_alchemist_core/blob/3b0dd282b0c06f158e296a045b1960a499b790ca/crates/golden_alchemist/src/formula.rs "golden_alchemist_core/crates/golden_alchemist/src/formula.rs at 3b0dd282b0c06f158e296a045b1960a499b790ca · Golden-Geek/golden_alchemist_core · GitHub"
[3]: https://raw.githubusercontent.com/Golden-Geek/Chataigne2/main/ARCHITECTURE.md "raw.githubusercontent.com"
[4]: https://raw.githubusercontent.com/Golden-Geek/Chataigne2/main/AGENTS.md "raw.githubusercontent.com"
[5]: https://github.com/Golden-Geek/golden_alchemist_core/blob/3b0dd282b0c06f158e296a045b1960a499b790ca/crates/golden_alchemist/src/runtime.rs "golden_alchemist_core/crates/golden_alchemist/src/runtime.rs at 3b0dd282b0c06f158e296a045b1960a499b790ca · Golden-Geek/golden_alchemist_core · GitHub"
