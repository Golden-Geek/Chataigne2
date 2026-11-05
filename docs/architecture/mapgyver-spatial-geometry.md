# Spatial editor geometry

MapGyver's spatial panel shares viewport, native image lifetime, pointer capture and edit-group
mechanisms. Composition and Matrix adapters retain their own document queries and typed
authoring operations. Matrix documents are never converted into Compositions.

`media/spatial/protocol.rs` owns the shared eight resize directions and the rectangle gesture
contract: immutable origin, start and current canvas-texel points, and Translate or Resize mode.
Rust generates the TypeScript declarations. Composition uses the same resize directions with
its own rotated/pivoted transform contract; its geometry math remains in Composition.

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

The [per-Layer Cell-content view](mapgyver-cell-layout.md) supplies bounded navigation and
inactive preview. Cell canvas manipulation and grouped keyboard editing remain required in
M12/7. The verified [Cell placement contract](mapgyver-cell-placement.md) supplies
their independent natural/fixed Size and Fit/Fill/Stretch policy. Full original spatial/pixel
acceptance remains in the [current ledger](../progress/mapgyver-status.md).
