# State Machine Architecture

## Ownership

`golden_core` owns the canonical authored Formula hierarchy and persistence.
Formula, ANode, socket, connection, and configuration parameter records are
ordinary visible nodes.

`golden_alchemist` owns the app-agnostic transient graph model, type solver,
compiler, dense execution schedule, runtime memory, diagnostics, and primitive
ANode declarations. It does not own authoring persistence, Chataigne value
types, or product policy.

`golden_statechart` owns normalized hierarchical states, regions, active paths,
history, deterministic transition selection, and enter/exit lifecycle events.
It does not know about Processors.

The app-owned `src/state_machine` package registers Chataigne stable-reference
value types, facets, module inputs, and intent-emitting ANodes. It also owns
Processors, state-owned Processor Managers, Processor Groups, guard graphs, the
active execution matrix, command arbitration, Formula integration, and
protocol DTO generation.

The authored ownership chain is:

```text
State
  -> ProcessorManager
      -> direct Processors
      -> ProcessorGroups
          -> Processors
```

A Processor owns one `AlchemistFormulaInstance`. The Processor inspector reads
the instance's sectioned Formula Surface; graph exposure remains a lower-level
graph interface and is not the inspector contract.

## Processor Boundary

The Processor is a formula host. It may:

- resolve and instantiate a Formula asset;
- own per-instance exposed configuration and runtime memory;
- forward lifecycle and context to the Formula runtime;
- execute the compiled graph;
- publish diagnostics and emitted intents.

The Processor must not:

- recognize condition, consequence, filter, or output node types;
- evaluate comparators, reducers, temporal policies, or branch semantics;
- choose source projections or reference parameter visibility;
- dispatch behavior based on a Formula category or label.

All workflow semantics belong to the Formula graph. Rich app-owned concepts such
as condition lists are implemented as Managed ANodes that expose normal
`golden_core::Node` configuration and lower that configuration into executable
Alchemist graph fragments.

Action and Mapping are not special runtime concepts. They may eventually ship
as ordinary Formula template files authored with the same editor and loaded
through the same persistence path as every user Formula.

The reusable Rust crates live in the `golden_alchemist_core` submodule. Keeping
the Chataigne package under `src` makes its product ownership match
`src/module`; being a Rust package is an implementation detail, not a reusable
package claim.

## Runtime Boundary

Authored Formula node subtrees are persisted and edited by Golden Core. The
app materializes one transient Alchemist graph from a Formula subtree. Type
solving produces a resolved graph. Compilation then replaces authored IDs and socket names with
dense execution IDs, numeric value slots, fixed state ranges, and a precomputed
topological schedule. Runtime ticks never perform type inference or topology
sorting.

ANodes emit generic intents. Chataigne converts those into command, sequence,
and state-transition intents. Command arbitration runs after Processor
evaluation, and dispatch is an explicit separate operation.

The state-machine tick order is:

1. Recompile dirty graphs.
2. Sample inputs and inject events.
3. Evaluate transition guards.
4. Select a transition and emit exit/enter lifecycle events.
5. Rebuild the active Processor matrix.
6. Evaluate only active Processor graphs.
7. Collect and arbitrate intents.
8. Dispatch side effects.
9. Publish protocol and debug deltas.

## UI And Protocol

Rust DTOs in
`src/state_machine/src/protocol.rs` are the protocol source of
truth. Run `npm run codegen:state-machine-protocol` from `src-ui` to regenerate
TypeScript under `src-ui/src/lib/state_machine/generated`.

`golden_alchemist_ui` owns reusable Svelte 5 canvas mechanics: infinite
pan/zoom, animated framing, node dragging, slots, connection previews, and
wires.

Every new Chataigne project contains one fixed top-level `State Machine`
manager. States are ordinary app-owned hierarchy items created through the
manager catalog, and their canvas positions are persisted as node parameters.
The dockable `State Machine` panel projects those canonical hierarchy nodes
onto the reusable canvas; it does not own a parallel state list. Canvas
selection uses the shared workbench selection so the standard Inspector edits
the selected state.

App-owned protocol adapters and deeper Alchemist runtime stores live beside the
generated output. `golden_ui` provides only the generic workbench and required
panel hooks.

## Implementation Status

The running app has one generic Formula definition node and an empty Formula
Library by default. The Alchemist editor selects Formula nodes directly and
projects their real ANode, socket, connection, and parameter descendants onto
the reusable graph canvas. It edits them through standard Golden Core intents.
There is no `FormulaDocument` or opaque Formula JSON parameter.

Formula assets use Golden Core sparse subtree persistence. A `.formula` file is
the JSON representation of the Formula node subtree, including its ordinary
children and parameters.

The app `StateProcessor` is now a small host containing only a constrained
Formula reference. The previous condition/consequence interpreter and Formula
slot instantiation path have been removed.

The next foundation work is:

1. Connect the app Processor node to the generic Formula
   instance/compiler/runtime.
2. Author Formula Surfaces and bind Processor-specific exposed configuration.
3. Implement Managed ANode lowering for conditions, consequences, filters, and
   outputs.
4. Add context lanes, contextual intents, and contextual transitions on top of
   that boundary.
