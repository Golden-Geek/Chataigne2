# Generic graph foundation

`golden-graph` is the only active owner of reusable graph identity, topology, transactions,
revisions, validation, traversal, presentation, and protocol envelopes.

Graph edits are applied in place through revision-checked transactions. Each operation records
an inverse; any operation or domain validation failure rolls the whole transaction back without
advancing the graph revision. Successful commits emit changes for only the affected node, edge,
geometry, comment, group, or bookmark. Topology indexes are updated with the same operation.

Domains keep payloads typed through `GraphDomain`. Alchemist and statecharts implement that
contract in their own crates, while `golden-graph` has no dependency on either domain or on the
Golden value system. JSON conversion requires an explicit `GraphProtocolAdapter`.

`golden-graph-ui` stores nodes, edges, geometry, selection, presentation, and viewport revisions
separately. Deltas mutate stable keyed maps, and the graph editor model uses a spatial index for
visible and hit-test queries. Alchemist and statechart packages supply UI domain adapters; the
generic canvas imports neither.

The phase evidence is recorded in
[`benchmarks/phase2/graph-foundation.v1.json`](../../benchmarks/phase2/graph-foundation.v1.json).
