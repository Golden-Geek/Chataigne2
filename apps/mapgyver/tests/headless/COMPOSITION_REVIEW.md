# Composition editor verification

MapGyver owns child-media authoring, Placement Parameters, geometry and panel queries.
Golden owns generic Inspector/history, panel persistence, transport and the desktop host.
The current phase audit and retained qualification limits live in
[the M06 panel report](../../../../docs/progress/mapgyver-m06-panels.md).

From the repository root:

```text
cargo build --locked -p MapGyver --bin MapGyver
node apps/mapgyver/tests/headless/media.mjs --composition-review
node apps/mapgyver/tests/headless/media.mjs --composition-ui --composition-project-ui
node apps/mapgyver/tests/browser/native-navigation.mjs --composition
node apps/mapgyver/tests/browser/native-navigation.mjs --composition-panels
```

Set `MAPGYVER_TEST_BINARY` to select an isolated ordinary app executable. Native checks use
the actual bundled WebView2 window, isolated profiles and normal owned-process shutdown.
Browser checks deliberately do not claim native pixels. The separate
[native raster fixture](../composition_pixels/README.md) verifies complete Composition images.

The headless review saves `target/m06-editor-review/CompositionReview.mapgyver`, a 320×240
Composition named **Review Composition**, with two instances of **Review Red** and one of
**Review Green**. Sources/instances are opaque, Normal blending; Green is last in draw order.
Its 20×16 Solid Color sources need no external files or running sender. The adjacent
`review.json` records stable identities and the project hash. Regeneration replaces this
test-owned reference project.

Current automated checks:

- The headless review verifies 30 absolute move/rotate/resize samples grouped into three
  history gestures, structural/source/rejection contracts and project restoration.
- `--composition-ui` verifies 130 children with 128-item pages plus independent selected
  lookup, generic Position/degree Rotation/history, simultaneous independent panels,
  persisted lock/page state, actual reconnect, copy/removal, New during held input and
  same-document Load. `--composition-panels` repeats that flow in the actual native app.
- `--composition-project-ui` uses actual File New/Open controls, a held move gesture,
  generic Resolution/history, review upload/stable IDs and generic rename/Undo/Redo.
- Native `--composition` verifies all eight resize handles fixing the opposite anchor,
  one-axis edges, all four outside-corner rotation targets/cursors, smooth anchored zoom,
  translation at zoom, shared Inspector selection, Escape, exact grouped history and tabs.
- `--owned-media-ui` retains actual default Reference/Alt-copy drops, independent variants,
  typed Placement editing, missing-target recovery and Save/New/Load.

For manual review, launch the app, load the saved project through **File → Open** and select
**Review Composition** in Media Manager. Expand it to select actual child media; the shared
Inspector shows their Placement Parameters. A Composition panel follows compatible document
selection or can be locked while browsing sources. Open another panel from a docking group's
plus menu to compare independent locks.

Drag source Media onto the canvas to insert a Reference, or Alt-drop to copy independently.
Drop onto an existing child to replace its source while keeping Placement. Use the typed
Output Parameter for a specific channel. Ordinary numeric settings, signed/nonuniform Scale,
Pivot, Rotation in degrees, Opacity, Blend and Enabled belong in the Inspector.

Drag a shape/move handle, any corner/edge resize handle, or just outside a corner to rotate.
Corners fix their opposite corner; edges affect one local axis. Each admitted drag takes one
Undo and one Redo. Escape discards later held samples. Hide/reopen or reload the project to
verify old selection/input does not replace the new generation. Removing a source preserves
the authored Reference child; Undo restores its target.

Physical presentation latency and whole-app scale are retained qualification gates. Passing
browser input, native view installation or a screenshot alone does not establish those budgets.
