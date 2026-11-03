# MapGyver implementation status

**Updated: 2026-11-03 · Overall: `███░░░░░░░` 27% · 3/11 phases complete**

**Current topic:** M05/6 shared smooth navigation and Screen interaction.
**Next deliverable:** backend-owned 1/10-output-pixel corner movement with grouped keyboard
repeat, then Alt translation and source-aspect-preserving Fill/Fit operations.
**Execution:** M04/1–3,7–8 accepted; M05/6 alone is Active on `mapgyver`. Source M04/4–6
stays in the return queue under the recorded functional order.
**Blocker for next delivery:** none known. Retained source/device/timing/memory/display gates remain below.
**Usable now:** actual Screen Surface/Inspector and background selection, selected/hover handles,
independent per-editor corner selection and undoable pointer dragging; shared smooth anchored
zoom in Screen, Surface Source and Composition;
simultaneous independent Media Preview panels with follow/lock, channel choice,
paged grid, actual reconnect/project recovery and independent output lifetime;
typed source/variant/Reference authoring, generic Inspector and paged Channels,
actual Composition child media/Placement and Matrix Cell media, default Reference and independent
Alt-copy drops, automatic/pinned outputs, unavailable-target recovery and owned-media retirement.
**Last delivered capability:** [Screen selection and local handles](evidence/mapgyver-screen-selection-2026-11-03.json):
Surface clicks drive the generic Inspector, background deselection survives refresh, selected/
hovered Surfaces expose handles, and two native editors retain independent corner choices.
Corner clicks/jitter and pan do not edit geometry/history. A real pointer drag produces one
Undo step and Undo restores the complete scene; retained invalid-warp and smooth zoom pass.
**Last accepted:** [M04/8 Media Preview](evidence/mapgyver-media-preview-native-2026-11-03.json),
including its complete original functional contract. M04/7 Reference remains accepted.
Whole-app Q04 and broader source/render/display qualification retain their existing gates.

| Phase                                                                                                                                                | State                           | Progress          | Gates | Subphase progression                                                                  |
| ---------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------- | ----------------- | ----- | ------------------------------------------------------------------------------------- |
| [M00 Baseline](mapgyver-m00-baseline.md)                                                                                                             | Accepted                        | `██████████` 100% | 1/1   | Baseline 1/1                                                                          |
| <a id="m01-acceptance-items"></a>[M01 Native route](mapgyver-m01-native-feasibility.md#functional-route-decision--2026-10-28)                        | Accepted                        | `██████████` 100% | 4/4   | Host/GPU/lifetime/decision 4/4                                                        |
| <a id="m02-acceptance-items"></a>[M02 Shared shell](mapgyver-m02-shell.md)                                                                           | Accepted                        | `██████████` 100% | 7/7   | Build/identity/discovery/tools/host/isolation 7/7                                     |
| <a id="m03-acceptance-items"></a>[M03 Rendering](mapgyver-m03-render-foundations.md)                                                                 | Blocked                         | `████████░░` 78%  | 7/9   | Foundations 7/7; recovery 0/1; qualification 0/1                                      |
| <a id="m04-acceptance-items"></a>[M04 Media](../plan/MapGyver_Implementation_Plan.md#m04--deliver-source-variants-reference-media-and-media-preview) | Pending source qualification    | `██████░░░░` 63%  | 5/8   | Sources 3/6; Reference 1/1; Preview 1/1                                               |
| <a id="m05-acceptance-items"></a>[M05 Screens/editors](../plan/MapGyver_Implementation_Plan.md#m05--deliver-screens-surfaces-and-shared-navigation)  | Active; physical gates blocked  | `███░░░░░░░` 30%  | 3/10  | Authoring/warp/calibration 3/3; editors 0/2; physical 0/3; bounds/workbench stats 0/2 |
| <a id="m06-acceptance-items"></a>[M06 Composition](../plan/MapGyver_Implementation_Plan.md#m06--deliver-child-media-composition-and-editor)          | Pending                         | `░░░░░░░░░░` 0%   | 0/5   | Child model 0/1; dependencies 0/1; native output 0/1; transforms 0/1; panels 0/1      |
| <a id="m07-acceptance-items"></a>[M07 Matrix](../plan/MapGyver_Implementation_Plan.md#m07--deliver-child-media-matrix-playback-and-grid)             | Pending                         | `░░░░░░░░░░` 0%   | 0/4   | Model 0/1; playback 0/1; grid 0/1; native/scale 0/1                                   |
| <a id="m08-acceptance-items"></a>[M08 App control](../plan/MapGyver_Implementation_Plan.md#m08--deliver-reusable-optional-app-control)               | Pending                         | `███░░░░░░░` 25%  | 1/4   | Golden process 1/1; config/demand 0/1; runtime 0/1; app acceptance 0/1                |
| [M10 Video](../plan/MapGyver_Implementation_Plan.md#m10--deliver-video-playback-in-codec-tiers)                                                      | Pending                         | `░░░░░░░░░░` 0%   | 0/4   | Common codecs 0/1; HAP 0/1; DXV/WebP 0/1; bundled integration 0/1                     |
| [M09 Final qualification](../plan/MapGyver_Implementation_Plan.md#m09--qualify-the-actual-app-and-windows-artifact)                                  | Waiting                         | `░░░░░░░░░░` 0%   | 0/1   | Revised actual-app qualification 0/1                                                  |

**Counting:** overall = fully accepted phases / 11, rounded to the nearest percent.
Phase/subphase progress = verified gates / that scope's fixed gates; bars round to tenths.
There are **28/57 accepted gates**. M04/8 adds one verified gate; denominators are unchanged.
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
| Active   | M05/6            | Shared smooth zoom and Screen Surface/Inspector/background/local-handle selection are delivered with bundled/native interaction. Finish grouped 1/10-output-pixel keyboard edits, Alt translation and backend Fill/Fit, then complete actual pointer/keyboard/native alignment acceptance; retain M05/8 physical matrix. |
| Accepted | M04/8            | [Complete original contract verified](evidence/mapgyver-media-preview-native-2026-11-03.json): eight simultaneous native views, independent persisted panel choices, actual reconnect/New/Load, completion-based close/lock retirement and stable independent Screen/Spout output with exact pixel oracles. |
| Accepted | M04/7            | [Original functional contract audited and passed](evidence/mapgyver-reference-native-2026-11-03.json): typed/API/bundled parity, cycles/identity/history/project, six complete native source/hardware cases and the bounded 10k backend fixture. Whole-app Q04 remains M09 with M06/M07 dependencies. |
| Accepted | M04/1–2          | [Source authoring/API](mapgyver-m04-source-api.md) and [native variants/Resolution/sharing](mapgyver-m04-variants.md), including both hardware adapters and stable identity through history/reload.                                                                                                                       |
| Pending  | M04/4–6          | Return after Reference's authored-model/rendering/API/UI product deliveries at the next allocated qualification batch. Finish full Spout lifecycle/cross-adapter/RGBA/declared-capacity cases, audit generic UI/lifetime and broad revised caller/Picture/resource parity. M04/3 Picture worker bounds retain acceptance. |
| Pending  | M05/7,9,10       | After M05/6, finish Surface Source interaction, stable visible output-bounds updates and outliner supplements.                                                                                                                                                           |
| Pending  | M06/1–5          | After M04 ownership/Reference and M05 viewport contracts, replace instances with child media, use degrees/new anchors and verify independent follow/lock panels. Replace the existing global Composition `InspectManager` refresh with bounded owner/catalog reads; Reference's paged query alone does not qualify whole-app scale. |
| Pending  | M08/2–4; M07/1–4 | After M06, establish reusable app-control configuration/runtime keys, then revised Matrix ownership/grid/trigger semantics and aggregate demand. Verify actual picker and retained retirement/access cases; remove Applications workflow.                                                                                 |
| Pending  | M10/1–4          | After revised M04–M08 contracts, implement common-codec transport, HAP, DXV/WebP and bundled recovery/performance in that order.                                                                                                                                                                                          |
| Blocked  | M03/4            | Missing genuine device-removal/recovery acceptance. Return after a recorded safe reset/removal route is authorized and software prerequisites pass; preserved injected-recreation evidence alone is insufficient.                                                                                                         |
| Blocked  | M03/9            | Original GPU critical-path proof/window timing remain unresolved; private-growth criterion failed. Return with supported bounded-error timing evidence or an explicit scope decision, and a concrete ownership correction for the preserved memory failure. No unchanged benchmark reruns or implicit budget revision.    |
| Blocked  | M05/4,5,8        | Missing physical concurrent-output/hotplug/DPI/presentation coverage. Return with reported display resolution/scaling/layout/available outputs and actual native access; run the exact retained matrix.                                                                                                                   |
| Waiting  | M09/1            | All revised feature and retained qualification gates; then complete Q01–Q06, actual-app statistics and Windows artifact checks.                                                                                                                                                                                           |

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
  `f550ec0c03e907e9ebab44891538f1030358c30a`. Do not restore it automatically; reconcile its
  typed-row work when revised M07 becomes active and its presentation work separately.
- **Current handoff:** M05/6 Screen selection/local-handle slice, started 2026-11-03 06:12 CET;
  cutoff 06:57 CET. MapGyver now preserves explicit Surface deselection, shares actual Surface/
  Inspector ownership and keeps corner selection local to each Screen preview. Golden's
  background/key hooks and selected-handle styling contain no app policy. Hover/selection
  expose at most eight handles; corner clicks/jitter do not edit geometry or open history.
  Actual dragging retains backend geometry and one Undo group.
  **Verified:** 17 authoring/gesture and 232 Golden UI tests, both app UI checks with zero
  diagnostics, viewport input and native UI lifecycle/continuity fixtures, ordinary bundled
  build, two-panel Screen selection, invalid-warp/history and smooth-navigation browser flows.
  Actual native WebView2 installs both panels, passes shared Inspector/background/hover/local
  corner/pan/jitter behavior, and verifies one real drag/one Undo restores the full scene; exit 0.
  [Evidence and remaining requirements](evidence/mapgyver-screen-selection-2026-11-03.json).
  **Gate changes:** none; M05/6 alone remains Active. Next deliver backend-owned 1/10-output-
  pixel movement and held keyboard repeat in one Undo group, then Alt translation and backend
  Fill/Fit. Full M05/6 input/alignment and M05/8 physical/DPI/latency acceptance stay required.
  Counts remain **28/57 gates, M04 5/8 (63%), M05 3/10 (30%), 3/11 phases (27%)**.

## Maintenance rule

Replace current state and the single current-batch/handoff entry in place. Keep this page
focused on progress, missing acceptance and next action. Link necessary detailed results in
owning reports/evidence; do not append daily logs, transcripts or duplicate dashboards.
At each implementation batch start, replace the handoff entry with deliverable, owner, proving
check and stopping time; at its end record only the current result and exact remaining scope.
