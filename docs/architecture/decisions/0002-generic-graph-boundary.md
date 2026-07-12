# ADR 0002: Make `golden-graph` the Sole Reusable Graph Foundation

- Status: Accepted
- Date: 2026-07-11

## Context

Generic topology, transactions, layout, canvas behavior, and graph state are currently entangled
with Alchemist, statecharts, UI packages, and application code. Copying those concepts into new
domains would create incompatible graph models and editors.

## Decision

`golden-graph` owns stable graph/node/port/edge IDs, graph documents and revisions, topology and
indexes, coherent transactions and deltas, generic validation/traversal, presentation metadata,
serialization envelope and migrations, protocol DTOs, and graph test infrastructure.

Domain meaning remains typed through a `GraphDomain` contract. Domains provide typed payloads,
ports, connection rules, validation, palette/inspector metadata, and optional compilation entry
points. Generic graph code neither imports Alchemist/statechart/app types nor erases payloads into
unchecked JSON.

`golden-graph-ui` is the one reusable Svelte 5 graph editor. It owns canvas mechanics, revisioned
stores, viewport/selection, spatial indexing, virtualization, generic commands, and extension
registries. Domain adapters provide rendering and semantic presentation; backend mutation semantics
remain backend-owned.

Alchemist and statecharts each depend on the graph foundation through their own domain adapters.
Neither `golden-graph` nor `golden-graph-ui` depends on Alchemist.

## Consequences

- Existing graph UI is moved and generalized through the live product instead of recreated from
  memory.
- Alchemist loses ownership of generic IDs, topology, layout, selection, and canvas behavior.
- Statecharts remain independent of Alchemist while sharing graph infrastructure.
- One-node edits must produce precise changes rather than clone complete graph maps; viewport work
  scales with visible/near-visible entities.
- The foundation remains one cohesive crate initially and splits only for measured value.

## Compliance

Phase 3 adapts a test domain, the real Alchemist domain, and the real statechart domain in that
order. Every baseline graph interaction and inspector route must pass in the real Chataigne UI
before duplicate ownership is removed.
