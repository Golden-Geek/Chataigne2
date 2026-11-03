# Matrix editor acceptance

MapGyver owns Matrix document, playback and grid policy. Golden owns document/history/project
transport, shared selection, generic Inspector and the native host. The
[current M07/3 audit](../../../../docs/progress/mapgyver-m07-grid.md) owns acceptance evidence;
native pixels and large-grid render plans remain M07/4.

Run from the repository root against the ordinary app. Build with
`cargo build --locked -p MapGyver --bin MapGyver`, or set `MAPGYVER_TEST_BINARY` to a separately
linked current executable. Each flow uses an isolated host/profile and normal shutdown.

```text
node apps/mapgyver/tests/headless/media.mjs --matrix-review
node apps/mapgyver/tests/headless/media.mjs --matrix-ui
node apps/mapgyver/tests/headless/media.mjs --matrix-project-ui
node apps/mapgyver/tests/browser/native-navigation.mjs --matrix-feedback
```

The first command saves `target/m07-grid-review/MatrixReview.mapgyver` and a hash/identity
manifest in `review.json`. This revised scene has one Group, two Layers, three Columns and five
owned Reference Cells. Media and Layer canvases are explicitly 320×240, with native-sized child
content and 500 ms Layer durations. Upper opacity is 50%, Group opacity is 80%; Blue is visually
distinct from the mixed Red/Green Columns. It starts unlaunched. Historical artifacts under
`target/m07-editor-review` are preserved; the current check never overwrites that reference.

The current checklist composes focused real-input flows. It replaces the obsolete internal
header/property forms with catalog creation, context menus, inline rename, drag controls and
the generic Inspector; its acceptance requirements are retained:

| Requirement | Current proof |
| --- | --- |
| 3×3 defaults, sparse addresses, outputs and one-step creation history | `matrix-defaults.mjs` |
| Square full-panel Cells, contextual creation/removal, row/Group/Column copy/delete and exact history | `matrix-grid.mjs` |
| Layer/Group before/inside/root moves, Column sorting, Escape, active anchor and source leaving the virtual window | `matrix-drag.mjs` |
| Typed Layer/Group/Cell settings, offscreen Inspector and exact per-field history | `matrix-parameters.mjs` |
| Default Reference/Alt-copy, cyclic rejection, source loss/Undo, healthy independent copy and launched replacement | `matrix-owned-media.mjs` |
| 64×64 keyboard scroll, selected offscreen Column editing, tab/resize lifetime, bounded mounting and narrow 125% Inspector | `matrix-viewport.mjs` |
| Actual File New/browser upload, stable persisted IDs with fresh project epochs, empty history and held-drag fencing across identical-ID reload | `matrix-project.mjs` |
| Actual native selection/request/installed/submitted distinctions, independent Cell mix/interruption, steady completion and final-consumer retirement | `matrix-feedback.mjs` |

Project checks validate the current reference hash and upload a UUID-suffixed copy through the
real browser File menu. A normal drop verifies the request observer before an old held drag is
released into a same-ID project reload. No stale domain edit, page error or rejected receipt is
accepted. Browser upload does not qualify a native file dialog.

The native flow uses actual bundled WebView2 input and a real 96×64 Spout Screen consumer.
It checks submitted shared-plan observations and retirement clearing. These observations do
not establish physical scanout, receiver timing or native pixel correctness; those keep their
owning M03/M05/M07/4/M09 requirements.
