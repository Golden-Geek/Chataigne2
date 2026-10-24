# Graph Foundation

`golden_graph` is the app-agnostic owner of authored graph mechanics. Domain crates supply typed
graph, node, port, and edge payloads plus semantic validation; the graph foundation does not know
about formulas, state transitions, Chataigne modules, runtime scheduling, or editor policy.

## Current Contract

The graph contract provides:

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

Alchemist and statecharts use canonical typed graph documents in their production authoring,
persistence, compiler, runtime, and UI projection paths. The ready-to-run `golden_core` facade
exposes `golden_graph`, so the contract is part of the reusable product stack rather than an
app-local or disconnected crate.

## Graph UI Boundary

`golden_graph_ui` owns the domain-neutral Svelte canvas, graph presentation document, viewport
mechanics, spatial visible-node queries, and incident-edge indexes. Its `GraphRevision` TypeScript
type is generated from the Rust contract by `golden_codegen_support`; it is not hand-maintained.
Alchemist-specific assets and domain presentation adapters are app-owned in the Chataigne UI. The
generic canvas remains in the app-agnostic `golden_graph_ui` package.

The app-owned `GraphDocumentAdapter` converts list-backed Alchemist and Spatializer projections into
the reusable presentation document and advances revision planes conservatively when an input array
changes. It has no command or effect authority.

## Mutation Cost

Transactions mutate the live document behind a local undo journal. Removing a node enumerates its
incident edges from topology indexes instead of scanning every edge, and a commit advances revisions
once regardless of operation count. The contract suite includes a 10,000-node localized removal
case to guard against whole-document snapshot rebuilds at this boundary.

Node and edge values live in identity hash maps. Separate ordered indexes retain their authored
positions; removal and rollback change only the affected entries, without shifting survivors.
Identity lookup is expected constant time and authored-order insertion/removal is logarithmic.
Iteration follows the ordered index and looks up each value, trading an additional lookup per
visited record for bounded local mutation. Full traversal and persistence remain whole-document
operations; persistence materializes ordered maps once and preserves the existing keyed JSON shape.

Authored positions are private runtime indexes, not persisted identities. Rollback retains each
removed position, while equality compares the actual identity/value sequence rather than gaps in
those positions. Loading a saved document may compact those gaps without changing authored order.

Node layout is a sparse, key-ordered `BTreeMap`; replacing a position updates the existing entry.
Layout map iteration is deterministic by node identity and does not define authored node order.
The persisted layout remains a keyed object with the same values, so this storage change needs
no persisted-data migration. Comments, groups and bookmarks retain their own authored order.

P09's remaining structural checkpoint removes repeated group-membership scans. Its final
qualification checkpoint covers consumer history and fixed-delta work at 1k/10k/50k/100k nodes;
the earlier localized test alone does not establish those scale claims.
