# Graph Foundation

`golden_graph` is the app-agnostic owner of authored graph mechanics. Domain crates supply typed
graph, node, port, and edge payloads plus semantic validation; the graph foundation does not know
about formulas, state transitions, Chataigne modules, runtime scheduling, or editor policy.

## Current Contract

The first Phase 3 graph slice establishes:

- stable graph, node, port, edge, comment, group, and viewport-bookmark identities;
- typed `GraphDomain` port schemas and validation;
- an indexed `GraphDocument` with direct incoming, outgoing, connection, and incident-edge lookup;
- revision-checked atomic `GraphTransaction` batches with local undo rather than whole-document
  cloning;
- coherent `GraphChangeSet`/`GraphDelta` output and separate topology, payload, and presentation
  revisions;
- selection-independent presentation state;
- deterministic topological and strongly-connected-component traversal without recursive graph
  walks;
- a versioned persistence envelope that rebuilds and validates topology indexes on load; and
- a typed test domain used by executable contract, rollback, persistence, traversal, and large-graph
  tests.

Alchemist and statecharts now have typed domain adapters while their working production paths remain
in place for the vertical Phase 4 and Phase 5 cutovers. The ready-to-run `golden_core` facade exposes
`golden_graph`, so the contract is part of the reusable product stack rather than an app-local or
disconnected crate.

## Graph UI Boundary

`golden_graph_ui` owns the domain-neutral Svelte canvas, graph presentation document, viewport
mechanics, spatial visible-node queries, and incident-edge indexes. Its `GraphRevision` TypeScript
type is generated from the Rust contract by `golden_codegen_support`; it is not hand-maintained.
`golden_alchemist_ui` retains only Alchemist-specific assets and domain presentation adapters.

The working product transport does not yet publish partitioned graph revisions. A pure app-owned
`LegacyGraphDocumentAdapter` therefore advances every revision plane conservatively. It has no
effect authority and is recorded for deletion when Phase 7 graph protocol DTOs carry the canonical
revision directly.

## Mutation Cost

Transactions mutate the live document behind a local undo journal. Removing a node enumerates its
incident edges from topology indexes instead of scanning every edge, and a commit advances revisions
once regardless of operation count. The contract suite includes a 10,000-node localized removal
case to guard against whole-document snapshot rebuilds at this boundary.

Cutover state is recorded in
[`phase3-cutovers.v1.json`](../product/manifests/phase3-cutovers.v1.json).
