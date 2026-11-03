# Revised Matrix grid

**M07/3 Active — 2026-11-03.** This slice delivers the square full-panel workbench grid,
contextual structural/media operations, inline rename and drag sorting/reparenting. The full
revised grid/project/native-feedback audit remains required; no gate is awarded.

MapGyver owns grid presentation and Matrix intents. Golden owns context menus, selection,
Inspector, history and document edits. The generated Rust viewport supplies the backend media
catalog; the UI does not choose media defaults or allocate labels. `CreateCellMedia` creates
one actual owned subtree in a sparse or existing empty Cell, preserving typed Cell settings and
app control. Occupied and foreign addresses reject atomically. All six kinds use the shared
media factory, including nested 3×3 Matrices and empty References.

Cells are 7.5 rem squares at every font scale, with the existing compressed scroll range and
bounded backend window. Group rows use a folder, accent and striped non-playable addresses.
Double-click edits row/Column labels in place. Empty Cells expose a centered plus/catalog and
context Add Media; filled Cells expose compact removal. Row/Column menus provide inspection,
creation, duplication and deletion. Settings forms and the internal header are removed; ordinary
settings use real Golden Parameters and the shared Inspector. Runtime feedback remains a
specialized supplement. The local-context-menu boundary prevents Golden's global node menu
from intercepting grid right-clicks.

Verification on the isolated bundled executable:

- 389 Rust library tests pass, zero failures, two retained exclusions, 128.63 seconds. New
  cases cover six kinds, existing/sparse Cells, no launch, exact history/project/copy, rejection
  and all four public callers. Both generated-contract equality suites pass.
- Strict all-target Clippy with native/process-fixture features passes. Both Cargo formatters
  pass. Svelte reports zero errors/warnings; all 29 Matrix/application UI tests pass.
- Actual browser interaction passes square/full-panel geometry, contextual media/structure,
  inline Layer/Column rename with exact Undo/Redo, stable owned media identity after Undo,
  removal preserving Cell settings, bounded offscreen Cell Inspector and no implicit launch.
- Typed Layer/Group/Cell parameter/history/offscreen checks and actual 3×3 catalog creation
  pass. Column/Cell app-control regressions pass: generic Inspector, shared PID, independent
  trigger/anchor, disabled destination hold and actual Unload retirement. Every browser run
  also passes ordinary source/history/Save-New-Load integration and normal host shutdown.
- Screenshot `target/m07-grid-ui.png` is visually inspected. Browser 149.0.7827.55. Executable
  SHA256: `C961F4C94BA20E2829E31990981D31495D9FDC8010707463218755C4485215BE`.

[Machine-readable receipt](evidence/mapgyver-m07-grid-slice-2026-11-03.json).
Browser previews report native observations unavailable; they do not establish native pixels
or physical output timing. M07/1–2 remain Accepted; M07/4 and M08/3–4 remain Pending.

Drag placement uses generated `DropItem` / `DropColumn` intents. The backend resolves the
complete committed order once, then uses the existing validated move path; the UI sends only
the hovered row/Column and pointer side. Row headers accept before/after and Group-middle
reparenting; the corner/background returns a row to root. Column headers accept left/right
sorting. Invalid/self/cyclic targets reject atomically; a drop into the current position creates
no Undo entry. One bounded selected viewport is sufficient; the UI never computes missing
preceding siblings. Project identity changes cancel held drag state.

Current drag proof: **390 library tests pass**, zero failures/two retained exclusions in 132.19
seconds. Strict all-target Clippy passes after its equivalent `rfind` cleanup; the affected
placement and four-caller parity tests pass again afterward. Generated equality, Svelte
0 errors/warnings and all 29 UI tests pass. Actual mouse drags verify Layer sorting, into-Group
and return-to-root moves, complete Group subtree retention, Column before/after sorting,
one-step exact Undo/Redo, Escape cancellation, owned Cell/media identity retention and stable
active Column identity. The square-grid/context/rename/offscreen-Inspector regression also
passes. Browser fixtures wait for the committed row order to reach the DOM before continuing
and move the mouse twice over the drop target before release. An earlier run failed to load a
dynamic JS asset; a subsequent run exposed the missing DOM/drag-over synchronization. The
final flow passes without retries or changes to acceptance predicates.

Current executable SHA256:
`FA019DC89EA52C1B1F94CF55AB7C416846D2208B191291A19A28B1EBAFACA981`.
Screenshot `target/m07-drag-ui.png` is visually inspected.
[Drag receipt](evidence/mapgyver-m07-drag-slice-2026-11-03.json).

Next deliverable: reconcile complete revised grid/project interaction, including keyboard/resize,
source replacement/rejection, pending lifetime and cross-window virtualization; verify truthful
current native playback feedback before M07/3 acceptance. The old ordinary-form fixtures retain
their uncovered requirements until replaced by equivalent current checks. All original budgets
and requirements remain in force.

The bounded native feedback batch (21:20–22:05 UTC) used its baseline and one repaired run.
Actual WebView2 input with a 96×64 Spout Screen consumer reaches installed/steady Matrix
submissions, distinct selection/requested/submitted state, independent Cell/Layer requests,
an interrupted mix and steady completion with unchanged authored state. The renderer is the
actual Intel adapter. The first run also exposes header status text overflowing the first Cell
at 125% scaling; the repaired header uses compact accessible markers and an independent selected
label outline. Its bounds checks and `target/m07-native-grid-feedback.png` visual inspection pass.
The runtime Inspector check explicitly selects a Matrix row after editing a populated Cell's
media. No native pixels or physical timing are inferred from these observations.

**Native feedback acceptance remains open:** the second run fails final-consumer retirement.
The footer becomes `Video idle`, while old installed/submitted Matrix observations remain.
Inspection identifies the ownerless/empty-topology early return in the host worker: it never
publishes an empty preview-status set. The implementation now shares successful/error/idle/
shutdown publication through `host/observations.rs`; delivery errors do not cache an unseen
clear, and a new preview generation republishes its current snapshot. Two regression tests
cover retirement, reconnect, deduplication and failed-clear retry. Full software results and the
remaining native return trigger are recorded in the owning receipt/status page. No further
native variants are added to this batch. The narrower generic Inspector layout also needs
review in the remaining complete-grid audit.

Return trigger: after the new host publisher's complete software and strict-lint prerequisites
pass, rebuild the isolated app and run the same native flow to confirm retirement clearing.
The two recorded native runs predate this newly implemented idle publication path; neither is
claimed as post-fix acceptance. **392 library tests pass** (130.70 seconds, zero failures/two
retained exclusions), including both new publication regressions and generated equality. Strict
all-target native/process Clippy passes. Svelte remains 0 errors/warnings and all 29 UI tests
pass; the bundled square-grid/context/rename/offscreen regression passes before the host-only
idle change. [Current native/software receipt](evidence/mapgyver-m07-native-feedback-2026-11-03.json).
