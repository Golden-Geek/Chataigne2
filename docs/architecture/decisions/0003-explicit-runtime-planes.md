# ADR 0003: Explicit runtime planes

- Status: Accepted
- Date: 2026-07-10

## Decision

The runtime has six separately owned planes: control, compilation, input/IO, semantic data,
effects, and observation. The control actor solely owns the editable project. Compilation
publishes immutable runtime generations. Semantic execution uses dense slots and never
accesses editable documents. Effects commit deterministically. Observation is bounded,
interest-driven, and never owns functional computation.

Transport interacts through typed handles and queues; it never locks a shared engine.
Continuous values, reliable events, structural intents, edit-session values, samples,
previews, and diagnostics each have explicit delivery policies.

## Consequences

Steady-state value processing cannot build project snapshots, traverse graph topology,
serialize protocol data, or allocate in proportion to project size. Backpressure is
observable and bounded. Slow observation clients cannot delay control or semantics.
