# MapGyver implementation status

**Updated: 2026-11-03 · Overall: `███░░░░░░░` 27% · 3/11 phases complete**

**Current topic:** M04 Reference read/edit scale acceptance.
**Next deliverable:** native Shared Texture Unload/final-use retirement, then all three source
kinds on NVIDIA; finish M04/7 before Media Preview.
**Execution:** M04/1–2 accepted; M04/7 continues on `mapgyver`, with one active gate. Source
qualification M04/4–6 remains in the return queue under the plan's recorded functional order.
**Blocker for acceptance:** native Shared Texture final Unload/source retirement remains
unverified after a fixture timeout; NVIDIA cases were not reached. The corrected wait now drives
renderer notifications and needs its acceptance run. Backend receipt checks pass; other debt remains.
**Usable now:** shared typed media Resolution/Sampling and bounded Channels editing through
the generic Inspector, media-owned procedural canvases and Matrix settings, sender-only Shared
Texture authoring, typed source commands and named scripting from the Inspector, source media,
Screen/Surface editing, Composition and Matrix playback, paged Reference output inspection,
actual Composition media children with typed Placement, and typed Matrix Cells with Reference/
Alt-copy drops and independent source copies.
**Last delivered capability:** [Reference reads and fixed edits at 10k](../architecture/mapgyver-reference-media.md):
immutable pages and shared graph-event payloads keep 10k Reference reads at 0.102 ms and
Target edits at 22.6 ms p95 (50 ms limit), with atomic history/rollback and unchanged wire data.
Pages stay 10,726 bytes; 15 demanded aliases capture one producer/16 records. Bundled Target/
Resolution, paging, Inspector navigation and missing-target recovery pass. Whole-app Q04 remains open.
**Last accepted:** [M04/2 native source variants](mapgyver-m04-variants.md): all three source
kinds on Intel Graphics and NVIDIA RTX 5080, including shared Resolution, three consumers,
source resize, reorder/history/reload and pixels. Broader native qualification remains open.

| Phase                                                                                                                                                | State                           | Progress          | Gates | Subphase progression                                                                  |
| ---------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------- | ----------------- | ----- | ------------------------------------------------------------------------------------- |
| [M00 Baseline](mapgyver-m00-baseline.md)                                                                                                             | Accepted                        | `██████████` 100% | 1/1   | Baseline 1/1                                                                          |
| <a id="m01-acceptance-items"></a>[M01 Native route](mapgyver-m01-native-feasibility.md#functional-route-decision--2026-10-28)                        | Accepted                        | `██████████` 100% | 4/4   | Host/GPU/lifetime/decision 4/4                                                        |
| <a id="m02-acceptance-items"></a>[M02 Shared shell](mapgyver-m02-shell.md)                                                                           | Accepted                        | `██████████` 100% | 7/7   | Build/identity/discovery/tools/host/isolation 7/7                                     |
| <a id="m03-acceptance-items"></a>[M03 Rendering](mapgyver-m03-render-foundations.md)                                                                 | Blocked                         | `████████░░` 78%  | 7/9   | Foundations 7/7; recovery 0/1; qualification 0/1                                      |
| <a id="m04-acceptance-items"></a>[M04 Media](../plan/MapGyver_Implementation_Plan.md#m04--deliver-source-variants-reference-media-and-media-preview) | Active                          | `████░░░░░░` 38%  | 3/8   | Sources 3/6; Reference 0/1; Preview 0/1                                               |
| <a id="m05-acceptance-items"></a>[M05 Screens/editors](../plan/MapGyver_Implementation_Plan.md#m05--deliver-screens-surfaces-and-shared-navigation)  | Pending; physical gates blocked | `███░░░░░░░` 30%  | 3/10  | Authoring/warp/calibration 3/3; editors 0/2; physical 0/3; bounds/workbench stats 0/2 |
| <a id="m06-acceptance-items"></a>[M06 Composition](../plan/MapGyver_Implementation_Plan.md#m06--deliver-child-media-composition-and-editor)          | Pending                         | `░░░░░░░░░░` 0%   | 0/5   | Child model 0/1; dependencies 0/1; native output 0/1; transforms 0/1; panels 0/1      |
| <a id="m07-acceptance-items"></a>[M07 Matrix](../plan/MapGyver_Implementation_Plan.md#m07--deliver-child-media-matrix-playback-and-grid)             | Pending                         | `░░░░░░░░░░` 0%   | 0/4   | Model 0/1; playback 0/1; grid 0/1; native/scale 0/1                                   |
| <a id="m08-acceptance-items"></a>[M08 App control](../plan/MapGyver_Implementation_Plan.md#m08--deliver-reusable-optional-app-control)               | Pending                         | `███░░░░░░░` 25%  | 1/4   | Golden process 1/1; config/demand 0/1; runtime 0/1; app acceptance 0/1                |
| [M10 Video](../plan/MapGyver_Implementation_Plan.md#m10--deliver-video-playback-in-codec-tiers)                                                      | Pending                         | `░░░░░░░░░░` 0%   | 0/4   | Common codecs 0/1; HAP 0/1; DXV/WebP 0/1; bundled integration 0/1                     |
| [M09 Final qualification](../plan/MapGyver_Implementation_Plan.md#m09--qualify-the-actual-app-and-windows-artifact)                                  | Waiting                         | `░░░░░░░░░░` 0%   | 0/1   | Revised actual-app qualification 0/1                                                  |

**Counting:** overall = fully accepted phases / 11, rounded to the nearest percent.
Phase/subphase progress = verified gates / that scope's fixed gates; bars round to tenths.
There are **26/57 accepted gates**. Partial work, planning and unavailable checks earn no gate.

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
| Active   | M04/7            | Backend receipt prerequisite passes at 10k (read 0.102/edit 22.6 ms p95). [Native attempts](evidence/mapgyver-reference-native-2026-11-03.json) pass Intel Solid Color/Picture and Shared Texture through owned-copy removal, then time out on final source entry retirement. The compiled/linted fixture now pumps refresh/outcomes; run it to finish Unload and NVIDIA coverage. Full mixed/end-to-end qualification remains open. |
| Accepted | M04/1–2          | [Source authoring/API](mapgyver-m04-source-api.md) and [native variants/Resolution/sharing](mapgyver-m04-variants.md), including both hardware adapters and stable identity through history/reload.                                                                                                                       |
| Pending  | M04/4–6          | Return after Reference's authored-model/rendering/API/UI product deliveries at the next allocated qualification batch. Finish full Spout lifecycle/cross-adapter/RGBA/declared-capacity cases, audit generic UI/lifetime and broad revised caller/Picture/resource parity. M04/3 Picture worker bounds retain acceptance. |
| Pending  | M04/8            | After Reference working contracts, deliver independent dockable Media Preview instances with selection-follow/lock/channel/grid state and retirement; remove Inspector previews and complete native/bundled acceptance.                                                                                                   |
| Pending  | M05/6,7,9,10     | After working revised media, deliver shared smooth navigation, Screen handles/keyboard/Fill/Fit, stable visible output-bounds updates and outliner supplements.                                                                                                                                                           |
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
- **Current batch/handoff:** M04/7 mixed native Reference acceptance; MapGyver capture/routing
  and Golden Video own runtime behavior, the native fixture owns notification servicing.
  Start 2026-11-03 04:28 CET; cutoff 05:13 CET. Two native attempts exhausted this batch's
  allowance. [Exact results and limits](evidence/mapgyver-reference-native-2026-11-03.json).
  Golden 672/MapGyver 317 tests, strict lint, exact native/producer compilation and unchanged
  isolated backend scale passed before hardware. All four bundled Reference/default/Alt/Cell/
  Placement/missing/auto-pinned/history/project workflows passed again, with normal host exit.
  Intel Solid Color and Picture (alpha 128) complete every fixture stage. Shared Texture passes
  sharing/resampling, nested Composition/Matrix, reload, actual source deletion/Undo and,
  after adding a copied-input Ready guard, independent-copy creation and playing-copy removal.
  The corrected run then times out checking source entries after Unload; NVIDIA is untested.
  Source inspection shows idle/cache eviction requires later plan preparation after old demands
  retire. The fixture's count-only wait omitted the production refresh/outcome path; it now
  drives that path without relaxing count/deadline assertions. Strict selected-target Clippy
  and exact native compilation pass. **That final correction has not run on hardware.**
  Catalog count alone does not prove an SDK/native allocation leak.
  **Return action:** execute the compiled/linted corrected fixture to verify Shared Texture
  Unload/final-use retirement and all three kinds on NVIDIA. No more hardware probes in this batch.
  Process inventory after terminal failures contains no owned producer/test processes.
  Pushed `d05af117` retains the usable receipt improvement: at 10k References, read/edit p95
  0.102/22.6 ms. Full native/end-to-end and whole-app Q04 remain required. No complete gate
  closed: **26/57 gates, M04 3/8 (38%), 3/11 phases (27%)**; M04/7 alone remains Active.

## Maintenance rule

Replace current state and the single current-batch/handoff entry in place. Keep this page
focused on progress, missing acceptance and next action. Link necessary detailed results in
owning reports/evidence; do not append daily logs, transcripts or duplicate dashboards.
At each implementation batch start, replace the handoff entry with deliverable, owner, proving
check and stopping time; at its end record only the current result and exact remaining scope.
