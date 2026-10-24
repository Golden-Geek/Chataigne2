# Spatializer service ownership

Spatializer is an app-owned generator under
[`apps/chataigne/src/module/modules/generators/spatializer`](../../apps/chataigne/src/module/modules/generators/spatializer).
Golden supplies snapshots, admission, wake registration, edit fences and completion receipts.
Spatializer owns its geometry, endpoint policy, deterministic generated identities and matrix layout.
The app's existing script template and module callbacks remain at that same app boundary.

## Preparation and publication

The module is passive. Input callbacks request service; they do not calculate the Cartesian matrix
or traverse generated outputs. One endpoint is normalized per service turn. Endpoint declarations
and primitive values are read through borrowed snapshot fields, including malformed parameter kinds.

Count admission precedes copying labels, leasing snapshots, allocating values and starting workers.
A worker captures one immutable, canonically leased snapshot, then computes a source-major dense
matrix. Voronoi retains its sparse Delaunay topology and existing influence equations. Temporary
weights scale by endpoints, rather than by the complete Cartesian product. Superseded continuous
configurations may coalesce; they are not ordered triggers or commands.

Publication retains its prepared result and cursor across turns. Existing rows service 16 pairs
per turn. Missing rows are assembled as detached `NodeTree` subtrees, at most 32 child parameters
per turn, and submitted as one structural insertion after preparation finishes. A receipt gates
the next step. Rejection restarts reconciliation against the actual graph; an applied prefix is
never replayed as if it had rolled back. Cleanup scans 16 entries and removes at most one outer
row per turn. Known whole-row insertion/removal may involve up to 4,097 nodes; that is an explicit
lifecycle bound, not a claim of interactive latency at that size.

Matching declarations preserve existing node IDs and UUIDs. New nodes use the existing UUID
formulas; switching orientation retains its distinct, deterministic identities. Position, mode and
label updates do not recreate matching matrix cells. A cell keeps its previous value until a
qualified replacement value applies. Generated declarations are cleaned without deleting unrelated
children in the Values folder.
If a generated row or cell moves away, reconciliation restores its deterministic identity to the
owned location rather than inserting a duplicate UUID. Parameters and Values roots must remain
under their module. Standard connection/input callbacks remain available; generated value writes
retain the existing suppression of callback echoes.
Golden routes move notifications through the former parent as well as the child's current path,
so an idle owner can observe a generated row leaving its subtree.

## Fences and lifetimes

The source Parameters subtree revision is checked before using a result and at its edit-queue
position through `NodeReadGuard::capture_subtree`. This includes descendant writes whose callbacks
have not run. Node-local guards additionally protect the module, destination row and output nodes;
a native generation guard invalidates cancellation and teardown. The subtree fence shares the
engine's snapshot cache. A cold fence can rebuild the ordinary full-tree snapshot; this contract
does not claim that whole-product snapshot construction is constant time. P09/P16 retain graph
scale and snapshot-cost qualification.

Preparation uses four process-wide worker slots and one slot per module, including blocked work
and cleanup. Cancellation advances the generation and relinquishes the actor's handle without
joining a worker or releasing its slot early. Failed preparation reports a warning and recovers
through an explicit delayed wake. Retryable worker failures and invalid configuration use typed
outcomes. Invalid configuration waits for an authored change; idle modules do not poll.
Prepared results are released to the background disposer after publication settles,
so idle modules do not permanently occupy preparation capacity.

| Resource | Admission / service bound |
| --- | --- |
| Sources / targets | 4,096 each; at most 1,048,576 pairs, checked before allocation |
| Endpoint label / direct children | 512 bytes / 16 children |
| Root parameter children / enum token | 32 children / 64 bytes |
| Prepared lifetimes | 64 entries / 128 MiB process-wide, including final disposal |
| Prepared reservation | 64 KiB + 4,096 bytes per endpoint + 8 bytes per pair; includes geometry scratch |
| Detached row lifetimes | 16 entries / 128 MiB process-wide; 16 KiB + 8 KiB per planned node |
| Queued edit lifetimes | 64 entries / 64 MiB process-wide; 1 MiB per admitted batch |
| Active workers | Four process-wide / one per module, including cancellation and cleanup |
| Existing-cell service / detached construction | 16 pairs / 32 child nodes per turn |

Snapshots use Golden's separate canonical 384-allocation / 1 GiB pool. All ceilings include
consumer-held and disposing payloads. Capacity pressure retains current authored values and uses
a 250 ms delayed retry; it does not create an unbounded queue or start extra threads. Matrix
admission is a representation ceiling, not qualification of a million-node interactive project.
The existing 512-source/1,024-target mathematical fixture remains part of owning validation.

Local P03 qualification belongs in the [implementation ledger](../progress/implementation-status.md).
20k/50k authored-node interaction, whole-product performance and platform gates remain P09/P16/P17.
