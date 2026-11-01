# Composition editor acceptance

The headless flow verifies backend intents and saves an ordinary project for real editor review.
It does not manufacture desktop input evidence. Composition and source-picker behavior are app
owned; the shared host, transport, history and project file codec remain Golden contracts.

The opt-in bundled-browser check is `node apps/mapgyver/tests/headless/media.mjs --composition-ui`.
It uses real DOM input and read-only backend correlation. Batch89 verifies its full form/source/
ordering/copy/removal/transform/Escape/rejection/source-loss/panel/history flow and dock-width
layout. Three earlier failures remain recorded; the final run synchronizes rendered form values
after history commits before pointer input. Browser input cannot prove native imagery/extents,
physical presentation or every item below, including project replacement. Do not repeat the
passing candidate to fill those different gaps.
See the owning [ledger](../../../../docs/progress/mapgyver-status.md#m06-acceptance-items).

Generate and verify the review project from the repository root:

```text
cargo build --locked -p MapGyver --bin MapGyver
node apps/mapgyver/tests/headless/media.mjs --composition-review
```

The resulting `target/m06-editor-review/CompositionReview.mapgyver` has a 320×240 Composition
named **Review Composition**, with two instances of **Review Red** and one of **Review Green**.
Its channels need no external media or running sender. The adjacent `review.json` records stable
identities and the project hash. Each regeneration replaces this test-owned reference project.

Launch `target/debug/MapGyver.exe`, use **File → Open** to load that project, select the
**Composition** tab and choose **Review Composition / Channel 1**. Keep the reference file
unchanged; reopen it to reset the scene. Report each group below as pass/fail/unavailable.

1. **Selection/source picker:** select a shape and its row; both show the same instance. Search
   for Review Green, assign it to the first red instance and apply. The other red stays unchanged.
   Add another source instance. Choosing the Composition itself rejects feedback visibly without
   changing existing instances or leaving controls busy.
2. **Move/rotate/scale:** drag a shape/pivot, its round rotation handle and square scale handle.
   Each complete drag takes one Undo and one Redo. Release outside the preview; controls recover.
3. **Numeric editing:** apply position, negative/nonuniform scale, off-center pivot, rotation,
   opacity, each blend mode and Enabled. The image/forms agree; other shared instances stay put.
4. **Ordering/identity:** rename, duplicate, move earlier/later and remove an instance. Undo/Redo
   restores the intended order, definition and selection without modifying its source media.
5. **Cancellation/lifetime:** Escape during a drag and hide/reopen the Composition tab. Accepted
   edits stay coherent; unsent motion stops and controls do not remain busy. Reopen the saved
   reference scene and confirm old selection or late work does not replace the new scene.
6. **Missing source:** remove Review Red in Media Manager. Its authored instances retain their
   identity/missing-source indication while Green works. Undo restores Red and its bindings.

Real pointer/keyboard, panel and on-screen observations are required to close M06 gate 4.
Headless requests, component tests and pixel readbacks alone cannot close these checklist items.
