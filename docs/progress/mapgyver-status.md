# MapGyver implementation status

**Updated: 2026-11-03 · Overall: `█████░░░░░` 45% · 5/11 phases complete**

**Verified gates: `████████░░` 77% · 44/57 accepted** (phase completion above remains 5/11).

**Current topic:** M07/1 model and M07/2 playback Accepted; M07/3 revised grid is Active.
**Next deliverable:** Drag sort/reparent Layers/Groups and sort Columns, followed by complete revised grid/project/native-feedback acceptance.
**Execution:** M08/1–2, M04/1–8, M05/4,6–7,9–10 and M06/1–5 accepted; M07/1–2 accepted; M07/3 alone is Active on `mapgyver`.
Retained physical/timing gates stay in the explicit return queue.
**Remaining requirement:** revised reusable app control, Matrix model/grid/playback, Video and retained
physical/performance/final qualification. Original budgets and acceptance scope remain intact.
**Newly usable:** [Square full-panel Matrix grid](mapgyver-m07-grid.md), inline row/Column rename,
centered empty-Cell media creation, compact removal and contextual structural actions. Ordinary
settings use the shared Inspector, including offscreen Cells. Full Matrix playback is verified: Cell → Column → Layer timing, independent Cell
triggers, atomic Columns, disabled/empty destinations, interrupted shared-time mixes and final-use
demand. Native Layer/Group/Final fade observations pass on both GPUs.
New Matrices open as 3×3 grids with sparse empty addresses and four paired outputs.
Actual catalog creation, exact Undo/Redo, complete subtree copies and empty project reload are verified;
queued viewport reads after deletion clear safely and recover on Undo.
Cell position, opacity and enabled are typed Parameters in the generic Inspector,
including offscreen editing and atomic configuration of sparse empty addresses. Native-sized child
content, Cell × Layer opacity, live edits, Undo and disabled holds pass on both GPUs.
Layer position, canvas size and transition duration are typed Parameters in the generic
Inspector, with atomic Configure/history/copy/project behavior. Actual native placement/resize/Undo
passes on both GPUs; Cell → Column → Layer durations drive independent playback. Layer/Group
opacity, all blend modes, render enablement and offscreen Inspector editing remain available.
Columns own typed transition and App control settings in the generic Inspector;
standalone Applications/catalog/associations are removed. An empty Matrix address can also own
app control without media. The actual Matrix action opens its generic Inspector with App file, integer pre/post
and real Argument/Environment children. Column-relative demand launches the owned process and
preserves its PID between neighbor and exact activation; leaving its range retires it.
History/project/copy use stable Column/Cell identities. Equal normalized configurations share one
physical runtime across Columns, Cells and Matrices. Changing one consumer splits only its lease;
matching settings rejoin the existing process, and the final consumer/unfinished plan controls retirement.
Double-click triggers one Cell without moving the Column anchor or other Layers; Cell/Column
transition overrides, independent requested/submitted markers and actual unload are available.
Disabled destinations preserve the prior Cell and originating Column app even without a renderer;
independent outgoing deadlines and unfinished-render completion preserve their required lifetimes.
**Usable now:** native Source crop/mapping/calibration, held Inspector feedback and selection/history;
immutable background reads keep loaded controls usable; functional native zoom alignment is accepted;
responsive Screen target/resolution/per-output FPS and Surface source-media tree
supplements; actual submitted FPS, selected GPU, CPU submit time and tracked renderer allocations
in the shared workbench footer, with coarse observations and automatic stream recovery;
backend Fill Screen and centered Fit to Screen preserve cropped-source aspect
and other Surface settings, with one-step Undo and native automatic Picture sizing;
Alt-drag from Surface bodies or corners translates the entire quad as one
Undo gesture, including at zoom; 1/Shift-10-output-pixel corner movement with held-repeat/chord grouping into
one Undo gesture, independent of editor zoom; actual Screen Surface/Inspector and background
selection, selected/hover handles, independent per-editor corner selection and undoable pointer
dragging; shared smooth anchored
zoom in Screen, Surface Source and Composition;
simultaneous independent Media Preview panels with follow/lock, channel choice,
paged grid, actual reconnect/project recovery and independent output lifetime;
typed source/variant/Reference authoring, generic Inspector and paged Channels,
actual Composition child media/Placement and Matrix Cell media, default Reference and independent
Alt-copy drops, automatic/pinned outputs, unavailable-target recovery and owned-media retirement.
**Last delivered capability:** [Square grid and contextual creation](mapgyver-m07-grid.md).
389 library tests, strict Clippy, 29 UI tests, clean Svelte and five bundled browser flows pass.
M07/3 remains Active for sorting/reparenting and complete native-feedback acceptance. [M07/2
playback remains Accepted](mapgyver-m07-playback.md); [M08/2 remains Accepted](mapgyver-m08-configuration.md#configuration-and-demand-acceptance--2026-11-03).
M06 remains 5/5 and M04 remains 8/8.
No timing/physical requirement is waived.
Whole-app Q04 and broader source/render/display qualification retain their existing gates.

| Phase                                                                                                                                                | State                           | Progress          | Gates | Subphase progression                                                                  |
| ---------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------- | ----------------- | ----- | ------------------------------------------------------------------------------------- |
| [M00 Baseline](mapgyver-m00-baseline.md)                                                                                                             | Accepted                        | `██████████` 100% | 1/1   | Baseline 1/1                                                                          |
| <a id="m01-acceptance-items"></a>[M01 Native route](mapgyver-m01-native-feasibility.md#functional-route-decision--2026-10-28)                        | Accepted                        | `██████████` 100% | 4/4   | Host/GPU/lifetime/decision 4/4                                                        |
| <a id="m02-acceptance-items"></a>[M02 Shared shell](mapgyver-m02-shell.md)                                                                           | Accepted                        | `██████████` 100% | 7/7   | Build/identity/discovery/tools/host/isolation 7/7                                     |
| <a id="m03-acceptance-items"></a>[M03 Rendering](mapgyver-m03-render-foundations.md)                                                                 | Blocked                         | `████████░░` 78%  | 7/9   | Foundations 7/7; recovery 0/1; qualification 0/1                                      |
| <a id="m04-acceptance-items"></a>[M04 Media](mapgyver-m04-source-parity.md#m04-phase-audit) | Accepted | `██████████` 100% | 8/8 | Sources 6/6; Reference 1/1; Preview 1/1 |
| <a id="m05-acceptance-items"></a>[M05 Screens/editors](../plan/MapGyver_Implementation_Plan.md#m05--deliver-screens-surfaces-and-shared-navigation)  | Physical qualification retained | `████████░░` 80%  | 8/10  | Authoring/warp/calibration 3/3; editors 2/2; physical 1/3; bounds/workbench stats 2/2 |
| <a id="m06-acceptance-items"></a>[M06 Composition](../plan/MapGyver_Implementation_Plan.md#m06--deliver-child-media-composition-and-editor)          | Accepted | `██████████` 100% | 5/5 | [Child model 1/1](mapgyver-m06-model.md); [dependencies 1/1](mapgyver-m06-dependencies.md); [native output 1/1](mapgyver-m06-native.md); [editor geometry 1/1](mapgyver-m06-editor.md); [panels 1/1](mapgyver-m06-panels.md) |
| <a id="m07-acceptance-items"></a>[M07 Matrix](../plan/MapGyver_Implementation_Plan.md#m07--deliver-child-media-matrix-playback-and-grid)             | Active grid | `█████░░░░░` 50% | 2/4 | [Model 1/1](mapgyver-m07-model.md#m071-acceptance--2026-11-03); [playback 1/1](mapgyver-m07-playback.md); grid 0/1; native/scale 0/1 |
| <a id="m08-acceptance-items"></a>[M08 App control](../plan/MapGyver_Implementation_Plan.md#m08--deliver-reusable-optional-app-control)               | Runtime qualification pending | `█████░░░░░` 50% | 2/4 | Golden process 1/1; [configuration/demand 1/1](mapgyver-m08-configuration.md#configuration-and-demand-acceptance--2026-11-03); runtime 0/1; app acceptance 0/1 |
| [M10 Video](../plan/MapGyver_Implementation_Plan.md#m10--deliver-video-playback-in-codec-tiers)                                                      | Pending                         | `░░░░░░░░░░` 0%   | 0/4   | Common codecs 0/1; HAP 0/1; DXV/WebP 0/1; bundled integration 0/1                     |
| [M09 Final qualification](../plan/MapGyver_Implementation_Plan.md#m09--qualify-the-actual-app-and-windows-artifact)                                  | Waiting                         | `░░░░░░░░░░` 0%   | 0/1   | Revised actual-app qualification 0/1                                                  |

**Counting:** overall = fully accepted phases / 11, rounded to the nearest percent.
Phase/subphase progress = verified gates / that scope's fixed gates; bars round to tenths.
There are **44/57 accepted gates**; denominators are unchanged. M07/2 adds one accepted gate
for full revised playback, including current native interruption and retained process/lifetime proof.
M07 is 2/4; M08 remains 2/4 and fully accepted phases remain 5/11. Prior source/output/editor evidence remains valid.
The additional gate-progress bar uses accepted gates / 57, rounded; it does not replace or
inflate the unchanged phase-completion calculation above.
Partial work, planning and unavailable checks earn no gate.

**Scope revision:** the prior plan was 5/10 phases (50%), with 40/48 gates accepted.
The updated requirements reopen affected M04–M08 acceptance and add M10. M04 changes 6/6→1/8
(reopen 1,2,4,5,6; add Reference/Preview); M05 5/8→3/10 (reopen 6,7; add bounds/supplements);
M06 4/4→0/5 (reopen 1–4; add independent panels); M07 3/4→0/4 (reopen 1,2,4);
M08 3/4→1/4 (reopen 2,3); M10 adds 0/4. Other gates are unchanged.
This adds nine gates, reopens sixteen and changes the phase denominator from 10 to 11.
Earlier evidence is retained for reuse, with its original schema/build limits; no new feature
or gate was accepted by merging the plan.

## Execution board and return queue

Exactly one gate is selected Active. Pending rows are the return queue, not concurrent work.

| State    | Gate(s)          | Missing requirement / concrete next or return action                                                                                                                                                                                                                                                                      |
| -------- | ---------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Active | M07/3 | Complete the full-panel square-cell grid, Layer/Group distinction, drag sort/reparent, Column sort, double-click rename, centered empty-cell Add Media/context operations, compact filled-cell removal and shared Inspector/presentation. Remove internal header/custom property forms; preserve bounded offscreen editing and truthful feedback. |
| Accepted | M07/2 | [Full playback audit](mapgyver-m07-playback.md): Cell/Column/Layer precedence, independent and atomic launches, disabled/empty/interruption rules, shared-time source/process demand and final-use retention pass. Current native three-color Layer/Group/Final fades pass on both GPUs. M07 is 2/4. |
| Accepted | M07/1 | [Complete model audit](mapgyver-m07-model.md#m071-acceptance--2026-11-03): typed Layer/Group/Column/Cell owners, real child media, 3×3 sparse defaults, all callers, exact history/project, single/multi-owner copy, schema admission and bounded editable lifetime pass. Model gate remains Accepted. |
| Accepted | M08/2            | [Configuration/demand audit](mapgyver-m08-configuration.md#configuration-and-demand-acceptance--2026-11-03): typed ownership, standalone removal, normalized sharing, neighbor/direct semantics and disabled-render/outgoing/final-use through interruption/reorder/deletion/Undo/projects pass. M08 is 2/4; broader runtime/access/UI qualification remains open. |
| Accepted | M06/5            | [Independent Composition panels](mapgyver-m06-panels.md): bounded off-page selection, simultaneous persisted follow/lock/page state, actual reconnect/File Open/New/held-input fencing and native tab/close lifetime. M06 is complete at 5/5. |
| Accepted | M06/4            | [Native anchored editor](mapgyver-m06-editor.md): all eight resize and four outside rotation/cursor cases, actual smooth zoom, translation at zoom, shared Inspector selection, Escape, exact grouped Undo/Redo and tab lifetime pass. |
| Accepted | M06/3            | [Full native Composition matrix](mapgyver-m06-native.md): both GPUs, nine original stages, 54 complete images, alpha/blends/pivots/signed scale/degrees/resize/missing/Undo/reload and completion-safe retirement pass. Active-area oracle reconciled with the existing black-canvas contract; scenes, quantization bounds, 2/255 residual tolerance and budgets preserved. |
| Accepted | M06/2            | [Dependencies and shared production](mapgyver-m06-dependencies.md): all current media kinds, mixed cycles, missing/deletion/Undo, 100k validator/4,096-deep authored graph and exact five-producer/six-alias compiled sharing pass. Native pixels are separately accepted under M06/3. |
| Accepted | M06/1            | [Degree-based child-media model](mapgyver-m06-model.md): typed Placement v2, complete structural/API/Parameter/history/project, old-radian rejection, actual Inspector Rotation and four caller paths pass. Native pixels are separately accepted under M06/3. |
| Accepted | M04/6            | [Complete source parity](mapgyver-m04-source-parity.md): revised callers, six all-source media/variant copy cases, exact history/project, typed Reload/script/command UI and both-adapter native missing/relink/replacement/alpha/4K recreation/retirement pass. M04 phase is complete. |
| Accepted | M04/5            | [Complete typed source UI](mapgyver-m04-source-ui.md): fresh variant permissions, CRUD/import/relink/selector/independence, shared settings, folder/catalog capabilities, bounded Channels and exact guard/history/reload pass. Deleted-channel reads retire safely and secondary-button menus preserve enabled state/history. |
| Accepted | M04/4            | [Complete Spout source acceptance](mapgyver-m04-spout.md): exact discovery/selection, full lifecycle and healthy-peer isolation, fixed-output resize metadata/pixels, both adapter mismatch directions/recovery, RGBA/alpha, declared overlap/storage limits and retirement pass. |
| Accepted | M05/4            | [Concurrent output independence](evidence/mapgyver-concurrent-output-2026-11-03.json): fullscreen/rectangle live native pixels and stable independent Spout frames through actual editor close, OS-confirmed minimize/restore and project-New retirement. |
| Accepted | M05/9            | [Stable output HWND](evidence/mapgyver-output-bounds-2026-11-03.json): position-only session preservation, resize/Undo, fullscreen across both current displays, return to rectangle and completion-safe disable pass with one native window creation and exact bounds/red pixels. |
| Accepted | M05/6–7          | [Native paint and accepted-frame input](evidence/mapgyver-native-zoom-sync-2026-11-03.json): Screen and internal-landmark Source motion, output-specific moving native hits, offset browser event routing and complete native selection/keyboard/translation/Fill/Fit/Source lifecycle suites pass. Physical latency is not inferred. |
| Accepted | M05/10           | [Complete outliner/footer contract](evidence/mapgyver-workbench-status-2026-11-03.json): bounded responsive target/resolution/source rows, actual individual output FPS and public shared footer with native values and recovery. |
| Accepted | M04/8            | [Complete original contract verified](evidence/mapgyver-media-preview-native-2026-11-03.json): eight simultaneous native views, independent persisted panel choices, actual reconnect/New/Load, completion-based close/lock retirement and stable independent Screen/Spout output with exact pixel oracles. |
| Accepted | M04/7            | [Original functional contract audited and passed](evidence/mapgyver-reference-native-2026-11-03.json): typed/API/bundled parity, cycles/identity/history/project, six complete native source/hardware cases and the bounded 10k backend fixture. Whole-app Q04 remains M09 with M06/M07 dependencies. |
| Accepted | M04/1–2          | [Source authoring/API](mapgyver-m04-source-api.md) and [native variants/Resolution/sharing](mapgyver-m04-variants.md), including both hardware adapters and stable identity through history/reload.                                                                                                                       |
| Pending | M07/4; M08/3–4 | After M07/3, complete full native/scale acceptance; then aggregate app-control runtime/UI qualification, actual picker, commands/snippets and retained retirement/access cases. Prior partial process/renderer evidence remains reusable. |
| Pending  | M10/1–4          | After revised M04–M08 contracts, implement common-codec transport, HAP, DXV/WebP and bundled recovery/performance in that order.                                                                                                                                                                                          |
| Blocked  | M03/4            | Missing genuine device-removal/recovery acceptance. Return after a recorded safe reset/removal route is authorized and software prerequisites pass; preserved injected-recreation evidence alone is insufficient.                                                                                                         |
| Pending  | M03/9            | Original GPU critical-path/window timing and reproduced private-growth failure need corrective qualification, not additional user/tool permission. Return in its bounded qualification lane with an ownership correction and supported timing evidence; no unchanged benchmark reruns or budget revision. |
| Blocked  | M05/5,8          | Full physical hotplug, 150/200% and negative-origin/latency coverage is unverified. Native pixels and the known local displays now permit automatic cases; run the retained exact physical matrix, preserving missing configurations and measurement requirements. |
| Waiting  | M09/1            | All revised feature and retained qualification gates; then complete Q01–Q06, actual-app statistics and Windows artifact checks. Reconcile the older cross-editor source-drop fixture's channel-row/reference-only assumptions with current ownership before final regression qualification. |

## Evidence and handoff

- [Historical acceptance and batch evidence](mapgyver-evidence-through-2026-11-02.md) preserves
  the complete previous ledger, including the unfinished Matrix Parameter batch and rejected
  timing/memory proposals. Its old execution boards are non-operative.
- [Qualification contract](mapgyver-qualification-contract.md) owns retained budgets, revised
  Composition/Reference and Video workloads, and separately accounted software decode uploads.
- Native user observations: [Screen/Surface](evidence/mapgyver-m05-manual-editor-acceptance-2026-11-02.json),
  [Composition](evidence/mapgyver-m06-manual-editor-acceptance-2026-11-02.json),
  [Matrix playback](evidence/mapgyver-m07-manual-playback-acceptance-2026-11-02.json).
  Brief “preview unavailable” on restore was accepted for now; other timing/alignment budgets remain.
- Prior Matrix Parameter and icon/presentation work is preserved, unaccepted, in named stash
  `mapgyver-pre-revision-matrix-parameters-and-icons-2026-11-02`, object
  `f550ec0c03e907e9ebab44891538f1030358c30a`. Do not restore it automatically. Its typed-row
  approach is reconciled in the current conversion; its older model/host/presentation
  changes remain un-restored and unaccepted.
- **Current batch/handoff:** M07/3 square-cell workbench grid, started 21:36 CET / stopping time
  22:21 CET. Delivered: full-panel square Cells, clear Group presentation, inline rename,
  contextual structural operations, backend-owned media creation and compact removal. Ordinary
  settings forms/internal header are removed; shared Inspector remains editable offscreen.
  Owner: MapGyver Matrix UI/backend authoring. **389 library tests pass**, two retained exclusions;
  strict native/process all-target Clippy, generated equality, both formatters, 29 UI tests and
  Svelte 0 errors/warnings pass. Five bundled flows pass: grid/context/history, typed Parameters,
  3×3 catalog defaults, Column/Cell shared process lifecycle and sparse Cell app settings.
  The revised grid screenshot is inspected. [Receipt and remaining scope](mapgyver-m07-grid.md).
  **Gate changes: none.** M07/3 alone remains Active. Exact next deliverable: drag sort/reparent
  Layers/Groups and sort Columns, then complete grid/project/caller/native-feedback acceptance.
  M07/1–2 remain Accepted; M07/4 and M08/3–4 Pending. Counts: 44/57 gates (77%), 5/11 phases
  (45%); denominators/budgets unchanged. Saved stash and unrelated user changes remain intact.
  Host recovery proxy is now running: VS Code extension-host PID 30884 → launcher 34752 →
  proxy 27472 → app-server 27832; journal confirms enabled startup at 20:37:37 UTC. The old
  “reload required” note is superseded. Existing 23/23 tests/read-only RPC remain verified;
  watched-thread routing and recovery from a real live capacity failure remain unobserved.
  Recovery awards no product gate.

## Maintenance rule

Replace current state and the single current-batch/handoff entry in place. Keep this page
focused on progress, missing acceptance and next action. Link necessary detailed results in
owning reports/evidence; do not append daily logs, transcripts or duplicate dashboards.
At each implementation batch start, replace the handoff entry with deliverable, owner, proving
check and stopping time; at its end record only the current result and exact remaining scope.
