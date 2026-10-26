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

Bounded normalization/publication scans retain one canonically leased read view across read-only
turns. Inbox delivery, new input requests, cancellation and queued edits invalidate it, and the
next wake requests a fresh engine snapshot. The original apply-time source/path fences still
validate queued writes against current engine state, including changes whose callbacks have not
run. Idle, disabled, rejected and failed service paths release the view through the canonical
snapshot disposer. This removes repeated full-tree construction from unchanged continuation
turns; cold snapshots after mutations keep their existing cost and qualification requirements.

| Resource                                      | Admission / service bound                                                       |
| --------------------------------------------- | ------------------------------------------------------------------------------- |
| Sources / targets                             | 4,096 each; at most 1,048,576 pairs, checked before allocation                  |
| Endpoint label / direct children              | 512 bytes / 16 children                                                         |
| Root parameter children / enum token          | 32 children / 64 bytes                                                          |
| Prepared lifetimes                            | 64 entries / 128 MiB process-wide, including final disposal                     |
| Prepared reservation                          | 64 KiB + 4,096 bytes per endpoint + 8 bytes per pair; includes geometry scratch |
| Detached row lifetimes                        | 16 entries / 128 MiB process-wide; 16 KiB + 8 KiB per planned node              |
| Queued edit lifetimes                         | 64 entries / 64 MiB process-wide; 1 MiB per admitted batch                      |
| Active workers                                | Four process-wide / one per module, including cancellation and cleanup          |
| Existing-cell service / detached construction | 16 pairs / 32 child nodes per turn                                              |

Snapshots use Golden's separate canonical 384-allocation / 1 GiB pool. All ceilings include
consumer-held and disposing payloads. Capacity pressure retains current authored values and uses
a 250 ms delayed retry; it does not create an unbounded queue or start extra threads. Matrix
admission is a representation ceiling, not qualification of a million-node interactive project.
The existing 512-source/1,024-target mathematical fixture remains part of owning validation.

Local P03 qualification belongs in the [implementation ledger](../progress/implementation-status.md).
Actual 20k/50k editor interaction, whole-product performance and platform gates remain P11/P16/P17.

## Editor ownership

The app's `ui/src/lib/panels/modules/SpatializerEditorPanel.svelte` composes focused owners under
its `spatializer/` directory. `projection.svelte.ts` reads the public Golden graph model;
`geometry.svelte.ts` owns display polygons, bounds and debug overlays; `SpatializerWorld.svelte`
owns SVG markup and its scoped styles. `interaction.svelte.ts` owns pointer previews, keyboard
input and frame scheduling, with shared presentation types/constants beside those owners. One coordinate
conversion serves both creation and pointer display paths.

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
retained index. Snapshot changes clear the cache.

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

The [presentation qualification](../progress/p11-spatializer-presentation.md) preserves the initial
six native groups; [authoring qualification](../progress/p11-spatializer-authoring.md) records the
expanded 13-group workflow and owning backend/UI regressions. Actual-app scale and composed
workflows remain P11 work. Display geometry does
not qualify runtime influence math.
