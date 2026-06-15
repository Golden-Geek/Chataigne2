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
processors own a shared `CompiledAlchemistFormula` reference plus their own
memory and property frame.

Default memory identity:

```text
ProcessorId + ContextKey + NodeStateSlot
```

No memory is shared between lanes unless a future explicit policy says so.
Sparse lane pools allocate memory only for lanes that actually evaluate and
only when the compiled Formula contains stateful nodes.

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

Each processor-originated preview sample must include the processor id and the
optional context key. Transition-originated diagnostics and effects use a
transition id and do not receive a processor lane key unless they explicitly
reference a processor-lane result as metadata.

Changing the selected preview lane only changes debug focus. It must not mutate
the Formula graph, Formula property defaults, processor overrides, or runtime
memory.
