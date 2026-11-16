# Spatial editor geometry

MapGyver's spatial panel shares viewport, native image lifetime, pointer/keyboard input and edit-group
mechanisms. Composition and Matrix adapters retain their own document queries and typed
authoring operations. Matrix documents are never converted into Compositions.

`media/spatial/protocol.rs` owns the shared eight resize directions and the rectangle gesture
contract: immutable origin, start and current canvas-texel points, and Translate or Resize mode.
Rust generates the TypeScript declarations. Composition uses the same resize directions with
its own rotated/pivoted transform contract; its geometry math remains in Composition.

Composition accepts finite zero scale on either or both axes as authored placement state.
The ordinary typed Scale Parameter and public Composition operations use the same validation;
nonfinite or f32-overflowing values still reject. The app's derived-layer builder omits zero-area
draws before constructing a projective mapping, preserving healthy siblings and a valid empty
Composition. Restoring nonzero scale restores the draw through normal revision/capture fencing.
The media, its selection and Inspector bindings remain present. Golden's mapping validation
continues to reject degenerate quads; no shared renderer exception or UI-side sentinel is needed.
The view projects live resize vertices from source fractions and additive displayed dimensions
around the fixed opposite anchor. It does not divide by collapsed dimensions or author a second
transform, so drag feedback can recover from zero while backend Parameters stay authoritative.

`CompositionEdit::TransformInstance` owns discrete Reset/Fit/Fill actions. Reset centers the
native-sized source with the default pivot and no rotation; Fit centers it with a uniform
scale that preserves its aspect; Fill stretches it to the complete canvas. The backend resolves
current source dimensions, including Reference overrides, and rejects unknown dimensions before
queuing edits. Reset remains available without a source. These actions change only the four
typed transform Parameters in one history transaction, preserving opacity, blend and enabled
metadata. Toolbar controls, ordinary node-menu contributions, the named script method and
typed Media Command all dispatch the same intent. The editor opens Golden's ordinary node
menu, retaining Copy/Duplicate/Delete rather than maintaining separate toolbar implementations.

Placement v3 contains geometry/output/opacity/blend Parameters only. The child's media metadata
is the sole authored enabled state; `InstanceDefinition.enabled` is a read projection and the
configuration operation translates its requested value into a metadata edit in the same history
transaction. Disabled media stay in document/editor queries and retain their Inspector bindings.
Render capture uses effective media enablement, so disabled children withdraw their source demand
without hiding their editable definitions. Golden's public projection helper reads enabled metadata
for ordinary nodes as well as Parameters with the existing UUID/generation/event fences.
Older Placement schemas reject explicitly; v2's duplicate Enabled Parameter is not silently ignored
or migrated. Current copy/Undo/reload use the v3 typed tree.

`MatrixEdit::EditLayerGeometry` resolves one Layer by UUID and checks at most 64 ancestors to
its nearest Matrix media owner. It runs before full Matrix document loading and never enumerates
Cells for a pointer sample. The backend preserves current non-geometric settings, computes each
sample from its immutable origin, rounds dimensions to whole pixels in 1..16384, and keeps the
opposite resize edge fixed. Nonfinite/overflowing coordinates, crossed edges, wrong node kinds
and foreign Matrix targets reject. The normal typed Parameters remain the authored state.

`spatial/pointer.ts` owns canvas coordinate projection, one pointer capture, final samples,
cancel/Escape and capture retirement for both adapters. Golden's `LatestEditGesture` owns one
edit group with one request in flight and one latest pending sample. Each adapter supplies a
typed backend intent and accepts only its current target's reply. Matrix's bounded visible rows
and independent selected Layer update from those replies; background reads cannot replace an
active gesture. Project/session/snapshot transitions fence late receipts.

The existing Matrix adapter provides Layer body dragging and eight resize handles, including
inactive Layers inside Layer Groups. The generic Inspector and canvas edit the same Parameters;
pointer editing never launches playback. Native input/history and retained binding checks are
recorded in the [Layer geometry receipt](../progress/evidence/mapgyver-m12-layer-geometry-2026-11-05.json).

The [per-Layer Cell-content view](mapgyver-cell-layout.md) supplies bounded navigation, inactive
preview and Cell move/resize. The verified [Cell placement contract](mapgyver-cell-placement.md)
supplies independent natural/fixed Size and Fit/Fill/Stretch policy.

Golden UI's public `store/keyboard-gesture` owns bounded held-arrow collection and asynchronous
edit-drain sequencing. It receives opaque target identities and current-start callbacks, and
has no app geometry or transport dependency. Screen warping supplies its corner adapter;
`spatial/keyboard.svelte.ts` supplies focused Composition/Matrix input. One arrow moves one
canvas pixel; Shift moves ten. Focused resize/rotation handles use their existing typed geometry
intent, while the canvas/body moves the selection. Each held-key set is one history group;
separate taps wait for closure and begin from the latest accepted geometry.

Viewport ownership excludes inputs and navigation controls. Focus loss flushes the current
gesture and discards queued input; Escape, changed target/mode/project/snapshot and unavailable
sessions cancel unsent input. Stale repeats cannot reopen a cancelled gesture. Handle buttons
retain focus during their active gesture while the ordinary admission guard rejects new starts.
Temporarily busy Composition/Matrix handles use `aria-disabled` while retaining DOM focus.
Native HTML disabling would blur the current resize target during history/projection refresh.
Pointer starts and the shared keyboard queue still use the existing current-geometry admission
guards; preserving focus never admits stale geometry or changes a cancellation fence.
Only the backend authors Parameters or validates coordinates and opposite anchors.

Admitted keyboard geometry selects the displayed object through the same document-selection
service as pointer input. Existing UUIDs resolve synchronously through Golden's graph index;
newly created nodes alone wait for a guarded graph delta. This keeps the Inspector and gesture
owner aligned when a selected Cell resolves to its Layer canvas, without scanning the graph
or weakening the cancellation rule for later selection changes.

The [complete M12/7 acceptance](../progress/evidence/mapgyver-m12-spatial-acceptance-2026-11-05.json)
reconciles native controls, qualified pixels and large sparse projects. The
[current ledger](../progress/mapgyver-status.md) retains separate physical/performance/final qualification.
