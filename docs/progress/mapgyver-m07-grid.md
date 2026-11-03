# Revised Matrix grid

**M07/3 Accepted — 2026-11-03.** MapGyver owns Matrix document intents and presentation.
Golden owns context menus, selection, generic Inspector, history/project transport and the native
host. M07/4 remains required for full native layer/group/final pixels, scale and ownership qualification.

The full-panel virtual grid uses 7.5 rem square Cells, compressed scroll bounds and bounded
backend windows. Group rows have folder/accent/striped presentation. Ordinary settings use real
Parameters in the shared Inspector; the internal heading and custom property forms are removed.

| Original M07/3 requirement | Authoritative proof |
| --- | --- |
| Square full-panel grid, Layer/Group distinction and bounded presentation | `matrix-grid.mjs`, `matrix-viewport.mjs`, viewport tests and inspected screenshots |
| Layer/Group drag sort/reparent/root return; Column sorting | Actual mouse drags, whole-subtree retention, before/after placement, Escape and exact one-step history in `matrix-drag.mjs`; source row leaves the mounted window while dragging |
| Double-click rename | Actual Layer/Column inline input and exact Undo/Redo in `matrix-grid.mjs` |
| Centered plus and context Add Media; compact filled removal | Both creation paths are exercised, including context Reference creation; removal preserves Cell settings/identity and exact Undo |
| Context structural operations and exports | Filled Group/Column copies and row/Group/Column deletion with complete owned Cells and exact replay; backend copy/project suites retain derived exports |
| Shared Inspector survives virtualization | Layer, Group, Cell and Column typed controls; offscreen edits/history without scrolling selected objects into view; 64×64 keyboard scroll, tab/resize lifetime |
| Complete current caller parity | Rust DTO/generated equality and all four caller paths for creation, placement, settings, copy/removal, rejection, project/history and playback; accepted M07/1–2 evidence is retained |
| Truthful native playback feedback | Real bundled WebView2 Column/Cell input and Screen consumer: installed/steady submission, selection independence, Cell/Layer mix, interruption and final-consumer retirement clearing |
| Retained project/source/lifetime checklist | Real File New/browser upload, exact current reference hash, fresh epochs, stable saved IDs, fresh history and no stale receipts/errors; verified drop observer plus same-ID reload during a held drag; cyclic rejection and source loss/Undo with a healthy independent copy |

The backend supplies the generated six-kind catalog and shared defaults. `CreateCellMedia`
creates a real owned subtree in an existing or sparse Cell, preserves typed Cell settings/app
control and never implicitly launches. `DropItem` / `DropColumn` resolve complete committed
order once before the existing validated move path; the UI sends only target/side hints.
No-op drops preserve history; foreign/self/cyclic targets reject atomically.

Golden's main Inspector stacks parameter labels/controls below 22 rem. Native-sized/narrow
125% editing stays inside the panel without overlapping labels. Matrix and media drop handlers
reject detached DOM owners: a held drag across same-ID project reload cannot dispatch into the
new document or throw a late `contains` error.

The host's shared observation publisher clears the set when the final owner retires and on
shutdown, retries failed delivery and republishes current state for new preview generations.
This fixes the reproduced `Video idle` / stale submitted-marker defect. Hollow requested,
filled submitted and selected-label outlines remain separate; native status markers fit the
header at 125% scaling. Populated Cell media uses its normal Inspector; the runtime supplement
is inspected through a Matrix row.

Current acceptance:
- **392 library tests pass**, zero failures/two retained exclusions, **143.37 seconds**.
- Strict all-target native/process Clippy, generated-contract equality and both Cargo formatters pass.
- Svelte: **0 errors/warnings**; all **29 Matrix/application UI tests pass**.
- Combined current `--matrix-ui --matrix-project-ui` passes in **38.53 seconds**. Six focused
  real-input flows, actual project replacement/upload and held-input fencing pass. The explicit
  context Add Media/Reference path also passes separately after strengthening that assertion.
- Current `native-navigation.mjs --matrix-feedback` passes with an actual Intel renderer and
  96×64 Spout Screen consumer, zero page errors/rejected requests and normal owned-host shutdown.
  The prior two failures predate the idle publisher; its same-flow fix confirmation also passes.
- Final executable SHA256:
  `400C52778FAD525F60C35A3589B9F2ED1471E19FC712B437DF0C2086C3935A14`.
  Screenshots `target/m07-native-grid-feedback.png` and `target/m07-viewport-inspector.png`
  are visually inspected.

[Acceptance receipt](evidence/mapgyver-m07-grid-acceptance-2026-11-03.json).
[Earlier grid slice](evidence/mapgyver-m07-grid-slice-2026-11-03.json),
[drag slice](evidence/mapgyver-m07-drag-slice-2026-11-03.json) and
[native defect/software correction](evidence/mapgyver-m07-native-feedback-2026-11-03.json)
retain their original limits. Earlier ordinary-form fixtures are replaced by equivalent
current flows; no acceptance requirement is dropped.

The revised review uses `target/m07-grid-review`, explicitly native-sized 320×240 media/Layer
canvases and 500 ms Layer durations. Its original hash is checked before/after UUID-named upload.
Historical `target/m07-editor-review` artifacts remain untouched.

Validation also ran with concurrent manager-label/dock-theme edits in the shared checkout;
those changes are excluded from this milestone. The same interaction paths passed before
those display-name edits. This evidence accepts grid interaction/native observations, not
native pixels, physical scanout/timing, full render scale or release readiness. **Next: M07/4.**
