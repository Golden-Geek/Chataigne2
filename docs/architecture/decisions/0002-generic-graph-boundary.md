# ADR 0002: `golden-graph` is the sole graph foundation

- Status: Accepted
- Date: 2026-07-10

## Decision

`golden-graph` owns generic graph identity, topology, transactions, revisions, precise
deltas, validation infrastructure, traversal, presentation metadata, persistence envelope,
and protocol. Typed graph domains supply payloads, ports, connection rules, validation,
rendering adapters, and optional compilation.

Alchemist and statecharts are independent domains built on this contract. Neither generic
graph code nor `golden-graph-ui` may import either domain.

## Consequences

Alchemist loses its generic graph, layout, selection, and canvas concepts. Statecharts do
not depend on Alchemist. Domain data remains typed rather than being erased into arbitrary
JSON. Two real domains and one test domain must prove the boundary in Phase 2.
