# Matrix editor acceptance

MapGyver owns Matrix authoring, playback and the grid. These checks use Golden's ordinary host,
transport, history and project codecs. Automated headless requests do not establish real input.

The opt-in bundled-browser check is `node apps/mapgyver/tests/headless/media.mjs --matrix-ui`.
It uses real DOM input and read-only backend correlation. Batch91 verifies its source/settings/
structure/history/selection/requested-playback/source-loss/scroll/resize/tab flow and narrow dock
layout. Three earlier failures are retained; final run passes after correcting fixture label/
render synchronization expectations. Actual project replacement, native preparation/installation/
submission feedback and unobserved complete-checklist cases remain required. Do not repeat this
passing candidate as proof of those different gaps or infer rendering from a launch request.

Use `node apps/mapgyver/tests/headless/media.mjs --matrix-project-ui` for the separate actual
File→New/browser Open→Load From lifecycle check with the existing saved review scene. Batch92's
initial state/history assertions pass, but capture audit exposes stale missing-media inspections.
The strengthened final check fails even after the unit-proven queued-read dispatch correction.
Keep that failure open; it is not a clean lifecycle pass. The fixture verifies the reference hash
and never modifies the original review file. Browser upload does not qualify a native file dialog.
Uploads use the app's normal browser project directory, not the private APPDATA tree. The fixture
now supplies a UUID-suffixed upload name to avoid review-file collisions; this post-run hardening
has syntax/format checks only. The original saved reference remains unchanged.
Batch93 verifies the UUID-named upload route, but the strict missing-media assertion still fails
after the viewport-owner correction. A small read-only intent/rejection observer is now prepared
to identify the exact failing request; it has not run. Do not add another controller hypothesis
without that provenance. Native feedback and clean lifecycle acceptance remain open.

Generate the validated review scene from the repository root:

```text
node apps/mapgyver/tests/headless/media.mjs --matrix-review
```

Use the existing `target/debug/MapGyver.exe` (build with `cargo build --locked -p MapGyver --bin
MapGyver` if needed), then **File → Open**
`target/m07-editor-review/MatrixReview.mapgyver`. Select the **Matrix** tab, search for
**Review Matrix**, and select it. The 320×240 scene has one group, two layers (Lower/Upper), three
columns and five source assignments. It starts **unlaunched**; the preview is initially empty.
Sources are local Solid Colors. The adjacent `review.json` records stable IDs and the file hash.

Report each group as pass/fail/unavailable; reopen the reference file to reset changes.

1. **Selection versus playback:** select rows, column labels and cells, including keyboard
   navigation. Selection changes the side inspector without launching or changing the requested
   column. Explicit Launch/Cut changes the requested column; preparation/submission feedback
   remains distinct from selection and the preview follows the actual rendered result.
2. **Assignment and history:** search Matrix Blue, assign it to a selected cell, then Undo/Redo.
   Other cells stay unchanged and assignment does not launch. Assigning Review Matrix itself
   rejects feedback visibly and leaves the document/control state coherent.
3. **Settings:** change layer/group opacity, blend and Enabled; canvas and transition settings;
   column transition override. Preview and forms agree after launching; edits use normal history.
4. **Structure and exports:** add/rename/copy/remove/reorder a layer, group and column; move a
   layer into/out of the group. Undo/Redo preserves intended cells and exported identities, with
   selection and available destination choices following the committed result.
5. **Grid lifetime:** scroll and resize the panel, hide/reopen the tab, then reopen the saved
   scene. No stale selection, requested/preparation feedback or late reply replaces the new scene.
   Controls remain usable. Inspecting an unlaunched restored scene does not launch it.
6. **Source loss:** remove Matrix Red in Media Manager. Its assignments show Missing source;
   Green/Blue remain usable. Undo restores Red and the original bindings. Unload clears playback.

Actual interaction stays M07 gate 3. Native fade/output verification stays gate 4; global resource,
cadence and physical-presentation qualification retain their existing M03/M05/M09 owners.
