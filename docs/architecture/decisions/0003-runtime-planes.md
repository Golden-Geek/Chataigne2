# ADR 0003: Separate the Runtime into Six Explicit Planes

- Status: Accepted
- Date: 2026-07-11

## Context

Editable project state, IO, semantic execution, effects, observation, and transport must scale and
fail independently. A shared externally accessible engine mutex or project-tree walk in steady-state
processing couples latency, serialization, devices, and UI load to semantic work.

## Decision

The final runtime has six planes:

1. The control plane exclusively owns the editable project, transactions, undo/redo, structural
   intents, persistence coordination, and compile requests.
2. The compilation plane consumes immutable revisions/change sets, compiles affected artifacts,
   keeps the last valid generation running, and swaps generations atomically.
3. The input/IO plane owns connections, recovery, parsing, timestamping, bounded streams, and output
   transmission outside semantic execution.
4. The semantic data plane evaluates immutable runtime generations through dense storage, dirty
   sets, rate domains, and deterministic semantic commits without project/graph traversal.
5. The effect plane stages worker effects and commits them in deterministic
   `(state, processor, lane, effect)` order before IO routing.
6. The observation plane projects subscribed visible data from immutable semantic commits and
   serializes bounded deltas off control/semantic threads.

External code uses typed handles and channels for control, input, observation, read models, and host
lifecycle. It never locks a shared engine directly. Delivery policies distinguish coalescible state,
lossless triggers/commands, structural transactions, external streams, previews, and diagnostics.

## Consequences

- IO async runtimes do not become the CPU semantic scheduler.
- The previous valid generation continues during compilation and compatible state migrates through
  stable typed keys.
- UI or slow-client pressure cannot block functional evaluation.
- Determinism is defined at semantic/effect publication boundaries, not worker completion order.
- Steady-state value processing forbids project-size traversal, JSON/DTO work, and proportional
  allocation.

## Compliance

Phase 2 introduces facades and shadow hooks before runtime replacement. Runtime cutover occurs only
after semantic digests, queue/backpressure behavior, effect isolation, generation swapping, and
real-product performance gates pass.
