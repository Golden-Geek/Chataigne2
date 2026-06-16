# Alchemist Formula Runtime Architecture

This document defines the runtime contract for Alchemist formulas, processor
instances, sparse context lanes, and output previews.

## Ownership Boundaries

Reusable formula compilation and evaluation mechanics belong in
`golden_alchemist_core`. Chataigne-specific processor policy, state-machine
integration, protocol DTOs, command arbitration, and product UI behavior belong
in the Chataigne workspace.

The app must not clone or patch a Formula graph for each processor instance.
The graph is authored once, compiled into a reusable plan, and evaluated with
runtime frames supplied by the processor lane currently being executed.

## Core Model

`Formula` is the authored recipe. It owns the graph, typed property
declarations, and default values. It does not own processor-specific runtime
state.

`CompiledFormula` is the optimized reusable executable plan produced from a
Formula graph, its property schema, and the node/value registries. It must be
shareable across processor instances.

`PropertyDecl` declares one Formula property. It has a stable id, display
metadata, a declared value type, a default value that conforms to that type,
and optional editor hints. The default value does not define the compiled
output type.

`PropertySlot` is the compiled slot assigned to a property declaration. Runtime
property reads use this slot instead of string lookup or graph materialization.

`PropertyFrame` is the runtime array of values supplied for one evaluation. It
is resolved from Formula defaults plus processor overrides or bindings for the
current context.

`ProcessorInstance` is an app-owned use of a Formula. It references the shared
Formula, supplies property overrides or bindings, and carries lifecycle and
command policy.

`ProcessorExecutionPlan` describes how a processor should run: which context
axes are available, which axes require evaluation, which axes require persistent
memory, and which execution strategy is valid.

`ContextAxis` is one stable dimension of lane variation, such as a device,
input channel, selected item, or other Chataigne-owned context source.

`ContextKey` is the stable identity of one actually evaluated context lane. It
is made from stable axis/item ids, not display indices. Reordering context items
must not move lane memory to the wrong item.

`ProcessorLane` is one processor/context combination that is actually evaluated.
Lanes are sparse; a processor under a multiplex may still run only one lane if
no formula input, output, state, or effect depends on lane-varying data.

`LaneMemory` is persistent state memory for one stateful processor lane. It is
private to the processor id, context key, and node state slot.

`GlobalStateMachineContext` is the non-lane state-machine evaluation context
used by transition guards and transition effects.

`DebugPreviewSession` is a bounded runtime capture subscription. It selects
which formula, processor, lane, nodes, and history depth are worth reporting to
the UI.

`ANodeOutputPreview` is a runtime-backed output value sample for an authored
ANode socket. It includes enough identity to map runtime execution back to the
Formula editor and, when applicable, to the selected processor lane.

## ANode Process And Send Policy

Each authored ANode carries app-visible processing controls in its config:
`process_on_input_change_only` and `send_on_output_change_only`. Both default
to `true`; declarations for continuous primitives such as LFO, noise,
metronome, smooth filter, speed, and delay opt out of input-change-only
processing by default.

`golden_alchemist_core` compiles those config values into each execution node.
Input-change-only nodes keep a lightweight per-lane process cache and are
skipped when their resolved runtime inputs are unchanged. Output-change-only
nodes still update runtime slots when they process, but they do not emit
debug/preview send samples when the semantic output value is unchanged. Idle
non-fired triggers compare as unchanged even when their tick metadata differs.

Preview inspection and runtime traffic are separate capture modes. Inspector
preview may force unchanged nodes to resample current values, while the
state-machine runtime preview reports only values that were actually sent so
the graph UI can highlight active wires and dim idle wires.

## Required Invariants

```text
The statechart has one active-state truth.
Processor multiplex dimensions do not clone or fork the statechart.
Only processor formula execution is lane-aware.
Transition guard/effect evaluation is global.
```

Statechart active configuration is never keyed by `ContextKey`. Transition
guards evaluate once in `GlobalStateMachineContext`, and transition effects run
once after a transition fires. If a transition needs processor-derived data,
that data must arrive through an explicit global aggregation or manager result.

## Manager Reference Nodes

Chataigne manager reference ANodes for Conditions, Inputs, and Output Commands
are app-owned product integrations, not reusable Alchemist primitives. Until
their real runtime contracts are implemented, these nodes compile to explicit
`chataigne_manager_node_unsupported` diagnostics.

They must not return fake defaults such as `false`, an empty `ParamArray`, or a
silently dropped command. The Formula editor surfaces the compile diagnostic
through the formula validity flag, formula diagnostics JSON, and authored ANode
warnings so unsupported manager behavior is visible before runtime.

The intended contracts remain:

```text
Inside processor formula:
  context_key may exist.
  output commands emit processor-origin intents with that optional context key.

Inside transition graph:
  no processor context key exists.
  output commands emit transition-origin intents after the transition fires.
```

## Property Runtime

Property nodes read typed runtime slots:

```text
Formula graph stays unchanged.
Formula compiles once.
Processor overrides resolve into PropertyFrame.
PropertyGetter reads from PropertyFrame by PropertySlot.
```

Compile-time validation must reject missing properties, invalid defaults,
invalid processor constants, and property/output type mismatches. A property
node output type comes from `PropertyDecl`, not from the default value.

## Evaluation And Memory

Compiled graph structure and persistent memory are separate. Evaluation receives
the shared compiled Formula, the current `PropertyFrame`, the current runtime
context, and either no persistent memory or the `LaneMemory` for the selected
processor lane.

The reusable core boundary is `evaluate_compiled_graph`: callers provide a
`CompiledAlchemistGraph`, an `AlchemistMemory` frame, and an `EvaluationFrame`.
`AlchemistRuntime` may remain as a convenience wrapper, but Chataigne
processors own a shared `CompiledAlchemistFormula` reference, an execution
plan, and a sparse lane-memory pool. They do not own a cached property frame.

Node declarations own their persistent state shape through `NodeStateLayout`.
The default maps `ExecutionKind::Stateful` to one runtime value slot and all
other execution kinds to no slots, but a declaration may request multiple
runtime value slots when its temporal behavior needs distinct memory cells.
The compiler assigns each enabled node a contiguous state slice from that
layout; disabled nodes receive an empty state slice.

Default memory identity:

```text
ProcessorId + ContextKey + NodeStateSlot
```

No memory is shared between lanes unless a future explicit policy says so.
Sparse lane pools allocate memory only for lanes that actually evaluate and
only when the compiled Formula contains stateful nodes.

`golden_alchemist` owns the reusable context identity primitives:
`ContextAxisId`, `ContextItemId`, `ContextKeyPart`, `ContextKey`,
`ContextValuePath`, `RuntimeContextFrame`, and `LaneRuntimePool`.
`ContextKey` is stable identity only; display labels and indices stay outside
the hot runtime key so reorder operations cannot move memory to the wrong lane.

Chataigne owns `ProcessorContextProvider`. The provider exposes available axes,
iterates the keys required by the current processor execution plan, and resolves
context values for future property/input binding work. `ProcessorRuntime`
retains a shared compiled Formula and a sparse `LaneRuntimePool`; lifecycle
memory resets clear the whole pool and the next evaluation lazily recreates only
the lanes that are still used.

During evaluation, `ProcessorRuntime` receives the live `Processor` instance and
resolves a fresh `RuntimePropertyFrame` for each evaluated context key. Constant
processor overrides therefore change the property frame without changing the
compiled Formula arc or the lane memory pool.

Formula lane analysis is computed during compilation. Node declarations may
declare direct context axes; those axes propagate through compiled input sources
so `FormulaAnalysis` can distinguish all explicit context reads, axes reaching
stateful nodes, and axes reaching effect emitters.

Processor lane analysis is app-owned. Chataigne combines `FormulaAnalysis` with
`ProcessorBindingAnalysis` to build a `ProcessorExecutionPlan`:

```text
required_eval_axes =
  processor property binding axes
  union formula explicit context axes
  union processor input binding axes
  union processor output routing axes
  union formula effect axes

required_memory_axes =
  if formula has stateful nodes:
    formula state axes
    union processor property binding axes
    union processor input binding axes
  else:
    empty
```

The plan selects one of four strategies: `SingleStateless`,
`MultiStateless`, `SingleStateful`, or `MultiStatefulSparse`. Evaluation uses
the plan's eval axes to choose context keys, and projects each context key onto
the plan's memory axes before accessing lane memory.

## Optimization Matrix

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

```text
Stateless does not always mean single evaluation.
It only means no persistent lane memory.
```

## Debug And Output Preview

Output preview capture is opt-in and bounded. The editor may request Formula
default preview, one processor default lane, one selected processor lane, or a
selected node subset. No graph/editor view should leave capture enabled.
Normal processor ticking uses `ProcessorDebugCapture::Off`; value samples for
previews or tests must be requested explicitly with a bounded history length.

Each processor-originated preview sample must include the processor id and the
optional context key. Transition-originated diagnostics and effects use a
transition id and do not receive a processor lane key unless they explicitly
reference a processor-lane result as metadata.

Changing the selected preview lane only changes debug focus. It must not mutate
the Formula graph, Formula property defaults, processor overrides, or runtime
memory.
