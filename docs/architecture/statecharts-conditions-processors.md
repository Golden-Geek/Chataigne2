# State Machine And Alchemist Processing

Chataigne has one state-machine system and one formula-processing system. Both are app-owned.

## Ownership

- `apps/chataigne/systems/alchemist/` owns Formula and ANode models, compilation, execution, and
  the graph-domain adapter.
- `apps/chataigne/systems/alchemist/condition/` owns authored condition definitions, compilation,
  dense runtime state, and input/script evaluation contracts.
- `apps/chataigne/systems/alchemist/processor/` owns processors, Inputs, Filters, Outputs,
  context lanes, `ValueSet`, lifecycle, and managed pipelines.
- `apps/chataigne/systems/state_machine/model/` owns Chataigne state identity, hierarchy,
  transitions, active configuration, and graph-backed edits.
- `apps/chataigne/systems/state_machine/runtime/` composes the model with Alchemist processors,
  arbitration, and protocol DTOs.
- Each system's `integration/` folder adapts its kernels to Golden Core nodes.
- `apps/chataigne/ui/src/lib/systems/alchemist/` owns Alchemist presentation, while
  `apps/chataigne/ui/src/lib/systems/state_machine/` owns the product panel and its local
  state-machine document projection. Both render graph surfaces through `golden_graph_ui`.

There are deliberately no generic Golden condition or statechart packages. These concepts encode
Chataigne behavior and remain with the product.

## State graph authoring

State graph edits use the app-owned `chataigne.stateGraph/edit` operation. Its Rust DTOs and
socket constants live in `systems/state_machine/runtime/src/authoring.rs`; the state-machine
protocol exporter generates the UI contracts. Golden owns transport, transactions and the
reusable graph canvas, while `integration/state/authoring/` owns State membership, permissions,
layout parameters and complete transition construction.

Every request carries the manager and target NodeId/UUID pairs. The backend validates all move
targets before queuing edits, so a later stale or read-only target cannot publish an earlier
move. A connection inserts a detached transition subtree with its target reference already set.
Each operation commits as one existing app-operation transaction and one undo group.

The UI's focused `stateGraphEdits.ts` controller captures the session, snapshot generation,
manager identity and target identities before waiting for shared session admission. It returns
the committed result or rejection to the canvas. Confirmed operations drain before save/history/
project barriers; queued work retires on source replacement or unmount. The session's snapshot
generation is reactive so even a full snapshot with unchanged node identities cancels pending
graph endpoints. The panel no longer constructs transition children or writes layout parameters.

See [State graph qualification](../progress/p11-state-graph-authoring.md) for the actual native
panel's persistence/rejection checks and the remaining scale and other-consumer acceptance.

## Runtime path

The editable condition tree is authoring state. Alchemist lowers it into a flat compiled program
and direct parameter bindings. Each default or multiplexed processor lane evaluates that program
with its own migratable condition runtime. Inspector DTOs are projected from compiled observations,
so preview capture cannot invoke a second condition implementation.

Processor formulas compile once per semantic formula key and are shared by identical instances.
The app-owned state-machine manager materializes its initial project-formula cache during its
node-ready activation callback, before the first scheduled runtime tick. Formula structure
events invalidate that cache; active processor compilation and evaluation still occur on the
runtime path. This preparation is synchronous and can be substantial for a large authored graph.
The lane compiler combines inherited context axes, context-linked properties, and output axes into
a stable execution plan. Stateful memory is keyed by `ContextKey`, retained only for active lanes,
and initialized only for new lanes.

The state-machine runtime selects lifecycle and transition work, then dispatches through the same
Alchemist processor and output-intent path used elsewhere. It does not own alternate Formula,
condition, input, filter, or output implementations.

Formula inbox frames with multiple structural edits coalesce socket reconciliation, validation,
and external-file synchronization into one pass. Single structural edits and small ordinary
parameter frames retain their direct dispatch path; large frames use the same bulk path.
Socket reconciliation passes its materialized Formula to validation when extraction succeeds;
validation rematerializes only to report an extraction failure. The two passes still type-solve
and compile synchronously, so large structural edits remain a measured runtime cost.
State nodes request an inbox tree snapshot only for their own enabled metadata changes, which
can reconcile state-network activity. The state-machine manager still invalidates its runtime
cache for Formula edits, but only state-topology child changes trigger the full network walk.

## Scale and safety

Compiled conditions do not walk editable condition nodes in steady state. Formula kernels are
shared through `Arc`; stateless processors share process caches; stateful processors allocate
memory only for active lanes. Shadow tests are pure and have no command, trigger, device, or output
host, so they cannot duplicate effects.
