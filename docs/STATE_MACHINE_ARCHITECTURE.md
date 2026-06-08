# State Machine Architecture

## Ownership

`golden_alchemist` owns the app-agnostic authored graph, type solver, compiler,
dense execution schedule, runtime memory, diagnostics, and primitive ANodes.
It does not contain Chataigne value types or product policy.

`golden_statechart` owns normalized hierarchical states, regions, active paths,
history, deterministic transition selection, and enter/exit lifecycle events.
It does not know about Processors.

`chataigne_alchemist` registers Chataigne stable-reference value types, facets,
module inputs, and intent-emitting ANodes.

`chataigne_state_machine` owns Processors, state attachments, guard graphs,
the active execution matrix, command arbitration, built-in Processor models,
and protocol DTO generation.

## Runtime Boundary

Authored Alchemist graphs are persisted and edited. Type solving produces a
resolved graph. Compilation then replaces authored IDs and socket names with
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
`crates/chataigne_state_machine/src/protocol.rs` are the protocol source of
truth. Run `npm run codegen:state-machine-protocol` from `src-ui` to regenerate
TypeScript under `src-ui/src/lib/state_machine/generated`.

App-owned Svelte 5 rune stores and components live beside that generated
output. `golden_ui` remains app-agnostic and is not modified for
Chataigne-specific state-machine behavior.
