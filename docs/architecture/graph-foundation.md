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

Alchemist and statecharts now use canonical typed graph documents in their production authoring,
persistence, compiler, runtime, and UI projection paths. Their former Rust graph adapters are
removed. The ready-to-run `golden_core` facade exposes `golden_graph`, so the contract is part of the
reusable product stack rather than an app-local or disconnected crate.

## Graph UI Boundary

`golden_graph_ui` owns the domain-neutral Svelte canvas, graph presentation document, viewport
mechanics, spatial visible-node queries, and incident-edge indexes. Its `GraphRevision` TypeScript
type is generated from the Rust contract by `golden_codegen_support`; it is not hand-maintained.
Alchemist-specific assets and domain presentation adapters are app-owned in the Chataigne UI. The
imported `golden_alchemist_ui` package has been removed after its assets moved into the app; the
generic canvas remains in the app-agnostic `golden_graph_ui` package.

The working product transport does not yet publish partitioned revisions for every remaining graph
surface. A pure app-owned `LegacyGraphDocumentAdapter` is still used by the Alchemist and Spatializer
UI projections and advances every revision plane conservatively. Statechart UI no longer uses it.
The adapter has no effect authority and remains scheduled for deletion when Phase 7 graph protocol
DTOs carry canonical revisions directly.

## Mutation Cost

Transactions mutate the live document behind a local undo journal. Removing a node enumerates its
incident edges from topology indexes instead of scanning every edge, and a commit advances revisions
once regardless of operation count. The contract suite includes a 10,000-node localized removal
case to guard against whole-document snapshot rebuilds at this boundary.

Cutover state is recorded in
[`phase3-cutovers.v1.json`](../product/manifests/phase3-cutovers.v1.json).
