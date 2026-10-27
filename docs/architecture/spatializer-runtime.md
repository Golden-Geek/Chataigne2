# Spatializer service ownership

Spatializer is an app-owned generator under
[`apps/chataigne/src/module/modules/generators/spatializer`](../../apps/chataigne/src/module/modules/generators/spatializer).
Golden supplies snapshots, admission, wake registration, edit fences and completion receipts.
Spatializer owns its geometry, endpoint policy, deterministic generated identities and matrix layout.
The app's existing script template and module callbacks remain at that same app boundary.

## Preparation and publication

The module is passive. Input callbacks request service; they do not calculate the Cartesian matrix
or traverse generated outputs. The preparation worker identifies noncanonical endpoints; at most
one listed endpoint is repaired per service turn. Canonical endpoints consume no scan wakes.
Endpoint declarations and primitive values use borrowed snapshot fields, including malformed kinds.

Count admission precedes copying labels, leasing snapshots, allocating values and starting workers.
A worker captures one immutable, canonically leased snapshot, then computes a source-major dense
matrix. `output_plan.rs` also inspects that leased view for changed cells, missing rows and obsolete
declarations. Only rows requiring work enter publication. Voronoi retains its sparse Delaunay
topology and existing influence equations. Temporary
weights scale by endpoints, rather than by the complete Cartesian product. Superseded continuous
configurations may coalesce; they are not ordered triggers or commands.

Publication retains its prepared result and cursor across turns. Existing rows service at most 16
changed pairs per turn, sharing batches across rows within the unchanged 32-read-guard ceiling.
Row phases reuse one authored guard while borrowing the same immutable planning snapshot; each
subsequent turn captures fresh guards. This admits a new 32-cell column in two 16-pair batches
without weakening source, destination or topology fences.
Missing rows are assembled as detached `NodeTree` subtrees, at most 32 child parameters
per turn, and submitted as one structural insertion after preparation finishes. A receipt gates
the next step. Rejection restarts reconciliation against the actual graph; an applied prefix is
never replayed as if it had rolled back. Cleanup applies the worker's obsolete-declaration list,
with at most 16 inner entries or one outer row per turn. Known whole-row insertion/removal may
involve up to 4,097 nodes; that is an explicit lifecycle bound, not a claim of interactive latency
at that size.

Rows with at most 32 cells retain up to 16 completed detached trees in a bounded forest before
one guarded application. Construction still creates at most 32 child parameters per service
turn; a small-row application contains at most 528 nodes. Wide rows retain their single-tree
application. Each staged tree keeps its existing row-pool reservation, including cancellation
and final disposal. Shared row pressure flushes an admitted prefix so competing modules cannot
deadlock while holding partial forests. A changed Values topology discards the unsubmitted
forest and revisits its first row; queued forests retain the original topology and source fences.

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
a native generation guard invalidates cancellation and teardown. The Values subtree revision
must still match the worker's inspection before a sparse plan is accepted. Structure/meta
reconciliation and failed receipts recapture a new worker plan against the applied graph;
they never replay a sparse plan that deliberately omitted unchanged entries. The subtree fence
shares the engine's snapshot cache. A cold fence can rebuild the ordinary full-tree snapshot; this contract
does not claim that whole-product snapshot construction is constant time. P09/P16 retain graph
scale and snapshot-cost qualification.

Preparation uses four process-wide worker slots and one slot per module, including blocked work
and cleanup. Cancellation advances the generation and relinquishes the actor's handle without
joining a worker or releasing its slot early. Failed preparation reports a warning and recovers
through an explicit delayed wake. Retryable worker failures and invalid configuration use typed
outcomes. Invalid configuration waits for an authored change; idle modules do not poll.
Prepared results are released to the background disposer after publication settles,
so idle modules do not permanently occupy preparation capacity.

Bounded normalization/publication continuations retain one canonically leased read view across read-only
turns. Inbox delivery, new input requests, cancellation and queued edits invalidate it, and the
next wake requests a fresh engine snapshot. The original apply-time source/path fences still
validate queued writes against current engine state, including changes whose callbacks have not
run. Idle, disabled, rejected and failed service paths release the view through the canonical
snapshot disposer. This removes repeated full-tree construction from unchanged continuation
turns; cold snapshots after mutations keep their existing cost and qualification requirements.

Prepared admission includes dense values and sparse pair-index capacity (24 bytes per admitted
pair), endpoint/row/repair storage, and 40 bytes per snapshot record for obsolete IDs and the
worker-captured generated-output echo set, including allocation/capacity rounding.
The latter is deliberately conservative: generated obsolete records can outnumber the new matrix
after an orientation/count change. Count admission and lifetime reservation still precede worker
allocation, and the shared 128 MiB ceiling remains unchanged. Plan storage and leased views keep
their existing background destruction path.

| Resource                                      | Admission / service bound                                                            |
| --------------------------------------------- | ------------------------------------------------------------------------------------ |
| Sources / targets                             | 4,096 each; at most 1,048,576 pairs, checked before allocation                       |
| Endpoint label / direct children              | 512 bytes / 16 children                                                              |
| Root parameter children / enum token          | 32 children / 64 bytes                                                               |
| Prepared lifetimes                            | 64 entries / 128 MiB process-wide, including final disposal                          |
| Prepared reservation                          | 64 KiB + 4,096 bytes per endpoint + 24 bytes per pair + 40 bytes per snapshot record |
| Detached row lifetimes                        | 16 entries / 128 MiB process-wide; 16 KiB + 8 KiB per planned node                   |
| Queued edit lifetimes                         | 64 entries / 64 MiB process-wide; 1 MiB per admitted batch                           |
| Active workers                                | Four process-wide / one per module, including cancellation and cleanup               |
| Existing-cell service / detached construction | 16 pairs / 32 child nodes per turn                                                   |

Snapshots use Golden's separate canonical 384-allocation / 1 GiB pool. All ceilings include
consumer-held and disposing payloads. Capacity pressure retains current authored values and uses
a 250 ms delayed retry; it does not create an unbounded queue or start extra threads. Matrix
admission is a representation ceiling, not qualification of a million-node interactive project.
The existing 512-source/1,024-target mathematical fixture remains part of owning validation.

While publication is active, its immutable output-ID set identifies generated value echoes
without a full-tree inbox read. Unknown inputs still use the ordinary snapshot/callback path.
Moves, removals, replacement and metadata/transaction events invalidate that classification
before callback dispatch. New child insertions do not reclassify existing IDs. An already
provided inbox snapshot may be canonically leased for the next publication turn only when the
callback queued no edits; apply-time source/path fences remain unchanged. The echo set is
released with preparation rather than occupying the shared pool while idle.

Local P03 qualification belongs in the [implementation ledger](../progress/implementation-status.md).
Actual 20k/50k editor interaction, whole-product performance and platform gates remain P11/P16/P17.

## Editor ownership

The app's `ui/src/lib/panels/modules/SpatializerEditorPanel.svelte` composes focused owners under
its `spatializer/` directory. `projection.svelte.ts` reads the public Golden graph model;
`geometry.svelte.ts` owns display polygons, bounds and debug overlays; `SpatializerWorld.svelte`
owns SVG markup and its scoped styles. `interaction.svelte.ts` owns pointer previews, keyboard
input and frame scheduling, with shared presentation types/constants beside those owners. One coordinate
conversion serves both creation and pointer display paths.

`module-catalog.ts` scans module identities once per snapshot generation, then consumes changed
node records. Matrix publications retain the same module list; add/remove/rename/replacement
updates return the current sorted identities. The picker no longer walks every generated cell
on ordinary value or structure updates.

`endpoint-cache.ts` retains endpoint presentation objects while their immutable node, position
and radius records and ordered palette color are unchanged. Computed matrix publications therefore
preserve geometry and navigation inputs; authored endpoint changes still publish current records.
Snapshot generation changes retire the cache, and temporary interaction hints remain separate.

Spatializer supplies an empty graph document and custom world content to the public Golden
GraphCanvas. Its endpoints are app-owned controls, not fabricated graph nodes or ports. The world
uses a small overflow-visible SVG coordinate basis; the actual canvas owns the viewport and camera.
Computed matrix values remain read-only in the inspector.

`spatializer/navigation.svelte.ts` adapts the complete endpoint catalog to Golden's public
`GraphWorldNavigation` hook. The shared paged finder reveals real SVG controls and Frame Selection
uses endpoint bounds, including radii. The app validates endpoint keys against current UUIDs;
session, snapshot, module and XY/XZ changes retire focus and discovery. Geometry-only edits retain
the catalog. A separate presentation revision restores focus when a mode change removes a radius
handle. Backend endpoint policy remains in the existing app operations.

`spatializer/view.ts` plans viewport membership in spatial units. Complete detail mounts at most
128 endpoint controls, including focused, selected and active-gesture retention. Dense views use
at most 256 counted overview groups; counts cover every visible endpoint, including coincident
sites. The finder still covers the complete catalog. Zooming or revealing a distant endpoint
restores its real control. A conservative distance bound admits all Voronoi sites that can own
part of the view, including offscreen sites; clipping still uses every enabled target. If the
candidate budget is exceeded, the view labels the region overview instead of drawing partial cells.

Detailed influence/debug overlays are limited to scenes of at most 128 endpoints, bounding
pair geometry and preserving the combined 256-polygon ceiling. Larger scenes explicitly summarize
source/target counts and direct users to the complete computed-value inspector. These presentation
limits do not change backend weights, endpoint caps or the matrix. `RelatedValues.svelte` composes
Golden's virtual rows with a read-only grid, filtering, reveal and focused-row retention. Its
3rem rows and 72rem maximum viewport keep mounting within 42 rows, including retained focus.
The declaration reader indexes visited folders and invalidates only changed membership/aliases
through Golden's published map deltas; ordinary value updates read current records through the
retained index. Appended or prepended children extend an existing index when all old membership
stays in order and every new record is available. First-child alias precedence is preserved;
middle insertion, missing records, removals and renames invalidate the index. Snapshot changes
clear the cache.

Spatializer allows zoom down to 0.1% so Home can fit wide endpoint maps; controls retain their
screen-space hit sizes. Golden respects that explicit range rather than silently clamping it
to 1%. The native wide-map regression checks every endpoint control against the actual canvas
viewport, including the inspector inset. Scale and publication qualification remain separate.

Endpoint mutations use the app-owned `chataigne.spatializer/edit` operation. Its canonical Rust
DTOs live in `apps/chataigne/systems/spatializer_protocol`; the public generator supplies the UI
types. The backend validates module and endpoint IDs/UUIDs, current XY/XZ space, direct ownership,
manual writable fields and finite coordinates before applying an atomic operation. XZ movement
preserves the latest authored height. Relative nudges and radius adjustments read current backend
values. Creation builds one complete detached target subtree using current backend defaults and
engine-owned unique naming.

`spatializer/authoring.ts` captures the session, projection generation, module and coordinate space
before shared bounded edit admission. Gestures retain one in-flight request and one latest unsent
sample under one acknowledged history group. Snapshot replacement, unmount and project barriers
retire pending input; an acknowledged group still receives its matching close after a projection
refresh. The engine's unique group ID protects a replacement or another owner's group from a stale
close. Backend failures release admission and remove previews; committed previews remain until
their event boundary publishes. Arrow keys move focused endpoints or resize focused radius
handles, including zero-radius handles. Add target uses the current viewport center as a display
hint; domain defaults stay in the backend.

Keyboard nudges/radius adjustments and atomic moves use temporary display hints just like pointer
gestures. Repeated key hints accumulate while their backend operations remain queued. Relative
intents still read current authored values on the backend; previews never write internal fields.
The authoring owner reports the acknowledged event boundary separately from the canonical edit
result. Successful previews remain until that publication reaches the graph, and rejection,
endpoint retirement, snapshot/space changes or unmounting withdraw them. An older receipt cannot
remove a newer hint for the same endpoint. Completion qualification still requires committed
values, actual geometry/focus and drained session work.
Creation reports pending dispatch on the existing status surface while the backend allocates
the real identity and declarations. It creates no speculative endpoint. Status is retired on
success/owner changes and replaced with the existing error on rejection; it never stands in for
the matrix publication completion checks.

The [presentation qualification](../progress/p11-spatializer-presentation.md) preserves the initial
six native groups; [authoring qualification](../progress/p11-spatializer-authoring.md) records the
expanded 13-group workflow and owning backend/UI regressions. Actual-app scale and composed
workflows remain P11 work. Display geometry does
not qualify runtime influence math.
