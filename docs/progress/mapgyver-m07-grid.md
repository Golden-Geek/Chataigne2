# Revised Matrix grid

**M07/3 Active — 2026-11-03.** This slice delivers the square full-panel workbench grid,
contextual structural/media operations and inline rename. Drag sorting/reparenting, Column
sorting and the full revised grid/native-feedback audit remain required; no gate is awarded.

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

Next deliverable: drag sort/reparent Layers/Groups and sort Columns through existing validated
backend intents, then reconcile the complete grid/project/native-feedback fixtures before M07/3
acceptance. All original budgets and requirements remain in force.
