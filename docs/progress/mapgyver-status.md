# MapGyver implementation status

**Updated: 2026-11-02 · Overall: `███░░░░░░░` 27% · 3/11 phases complete**

**Current topic:** revised M04 sender-only Shared Texture and native metadata ownership.
**Next deliverable:** remove channel-owned procedural canvas settings and project media
Resolution into Composition/Matrix; then complete manager menus and the module API.
**Execution:** implementing M04/1 on `mapgyver`; one active gate, delivered in reviewable slices.
**Blocker for next delivery:** none known. Retained native/timing/memory/display blockers are below.
**Usable now:** shared typed media Resolution/Sampling and bounded Channels editing through
the generic Inspector, sender-only Shared Texture authoring, source media, Screen/Surface
editing, Composition and Matrix playback.
**Last delivered capability:** Shared Texture metadata is native runtime state; authored
variants contain only the exact sender. Discovered inputs reserve actual pool storage before
allocation under the unchanged aggregate budget. Schema v4 rejects unsupported older media.
Software/browser verification passed; full native lifecycle acceptance remains open.
**Last accepted:** user-reviewed native Screen/Surface, Composition and Matrix playback behavior
on the prior schema. Remaining revised ownership and native editor acceptance stay open.

| Phase                                                                                                                                                | State                           | Progress          | Gates | Subphase progression                                                                  |
| ---------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------- | ----------------- | ----- | ------------------------------------------------------------------------------------- |
| [M00 Baseline](mapgyver-m00-baseline.md)                                                                                                             | Accepted                        | `██████████` 100% | 1/1   | Baseline 1/1                                                                          |
| <a id="m01-acceptance-items"></a>[M01 Native route](mapgyver-m01-native-feasibility.md#functional-route-decision--2026-10-28)                        | Accepted                        | `██████████` 100% | 4/4   | Host/GPU/lifetime/decision 4/4                                                        |
| <a id="m02-acceptance-items"></a>[M02 Shared shell](mapgyver-m02-shell.md)                                                                           | Accepted                        | `██████████` 100% | 7/7   | Build/identity/discovery/tools/host/isolation 7/7                                     |
| <a id="m03-acceptance-items"></a>[M03 Rendering](mapgyver-m03-render-foundations.md)                                                                 | Blocked                         | `████████░░` 78%  | 7/9   | Foundations 7/7; recovery 0/1; qualification 0/1                                      |
| <a id="m04-acceptance-items"></a>[M04 Media](../plan/MapGyver_Implementation_Plan.md#m04--deliver-source-variants-reference-media-and-media-preview) | Active                          | `█░░░░░░░░░` 13%  | 1/8   | Sources 1/6; Reference 0/1; Preview 0/1                                               |
| <a id="m05-acceptance-items"></a>[M05 Screens/editors](../plan/MapGyver_Implementation_Plan.md#m05--deliver-screens-surfaces-and-shared-navigation)  | Pending; physical gates blocked | `███░░░░░░░` 30%  | 3/10  | Authoring/warp/calibration 3/3; editors 0/2; physical 0/3; bounds/workbench stats 0/2 |
| <a id="m06-acceptance-items"></a>[M06 Composition](../plan/MapGyver_Implementation_Plan.md#m06--deliver-child-media-composition-and-editor)          | Pending                         | `░░░░░░░░░░` 0%   | 0/5   | Child model 0/1; dependencies 0/1; native output 0/1; transforms 0/1; panels 0/1      |
| <a id="m07-acceptance-items"></a>[M07 Matrix](../plan/MapGyver_Implementation_Plan.md#m07--deliver-child-media-matrix-playback-and-grid)             | Pending                         | `░░░░░░░░░░` 0%   | 0/4   | Model 0/1; playback 0/1; grid 0/1; native/scale 0/1                                   |
| <a id="m08-acceptance-items"></a>[M08 App control](../plan/MapGyver_Implementation_Plan.md#m08--deliver-reusable-optional-app-control)               | Pending                         | `███░░░░░░░` 25%  | 1/4   | Golden process 1/1; config/demand 0/1; runtime 0/1; app acceptance 0/1                |
| [M10 Video](../plan/MapGyver_Implementation_Plan.md#m10--deliver-video-playback-in-codec-tiers)                                                      | Pending                         | `░░░░░░░░░░` 0%   | 0/4   | Common codecs 0/1; HAP 0/1; DXV/WebP 0/1; bundled integration 0/1                     |
| [M09 Final qualification](../plan/MapGyver_Implementation_Plan.md#m09--qualify-the-actual-app-and-windows-artifact)                                  | Waiting                         | `░░░░░░░░░░` 0%   | 0/1   | Revised actual-app qualification 0/1                                                  |

**Counting:** overall = fully accepted phases / 11, rounded to the nearest percent.
Phase/subphase progress = verified gates / that scope's fixed gates; bars round to tenths.
There are **24/57 accepted gates**. Partial work, planning and unavailable checks earn no gate.

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

| State   | Gate(s)          | Missing requirement / concrete next or return action                                                                                                                                                                                                                                                                   |
| ------- | ---------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Active  | M04/1            | Shared Resolution/Sampling, Channels and sender-only authoring are software/browser verified. Finish procedural canvas/shared settings, manager context-menu visibility, creation/import defaults and complete command/function/callback/snippet API; verify revised generic/direct editing and history contracts.     |
| Pending | M04/2,4–8        | After M04/1, finish revised source/Spout/native checks, including the corrected lifecycle and declared-input capacity cases in the linked evidence; verify generic UI/lifetime, then Reference and Media Preview. M04/3 Picture worker bounds retain acceptance.                                                       |
| Pending | M05/6,7,9,10     | After working revised media, deliver shared smooth navigation, Screen handles/keyboard/Fill/Fit, stable visible output-bounds updates and outliner supplements.                                                                                                                                                        |
| Pending | M06/1–5          | After M04 ownership/Reference and M05 viewport contracts, replace instances with child media, use degrees/new anchors and verify independent follow/lock panels.                                                                                                                                                       |
| Pending | M08/2–4; M07/1–4 | After M06, establish reusable app-control configuration/runtime keys, then revised Matrix ownership/grid/trigger semantics and aggregate demand. Verify actual picker and retained retirement/access cases; remove Applications workflow.                                                                              |
| Pending | M10/1–4          | After revised M04–M08 contracts, implement common-codec transport, HAP, DXV/WebP and bundled recovery/performance in that order.                                                                                                                                                                                       |
| Blocked | M03/4            | Missing genuine device-removal/recovery acceptance. Return after a recorded safe reset/removal route is authorized and software prerequisites pass; preserved injected-recreation evidence alone is insufficient.                                                                                                      |
| Blocked | M03/9            | Original GPU critical-path proof/window timing remain unresolved; private-growth criterion failed. Return with supported bounded-error timing evidence or an explicit scope decision, and a concrete ownership correction for the preserved memory failure. No unchanged benchmark reruns or implicit budget revision. |
| Blocked | M05/4,5,8        | Missing physical concurrent-output/hotplug/DPI/presentation coverage. Return with reported display resolution/scaling/layout/available outputs and actual native access; run the exact retained matrix.                                                                                                                |
| Waiting | M09/1            | All revised feature and retained qualification gates; then complete Q01–Q06, actual-app statistics and Windows artifact checks.                                                                                                                                                                                        |

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
- **Current milestone:** M04/1 Shared Texture ownership. MapGyver channels persist only their
  exact sender selector; maximum dimensions, storage format, transfer and alpha Parameters
  were removed. Golden discovers BGRA/RGBA byte storage and applies its documented classic
  Spout sRGB/straight-alpha interpretation. Internal host bounds remain; discovered pool charges
  follow actual ownership through retirement under the unchanged aggregate capacity. Declared
  Golden bindings retain maximum reservations. Backend preflight and worker admission share
  that policy, with a software regression for the initial native capacity failure. Schema v4,
  generated contracts and consumers changed together; codegen also prunes obsolete owned DTOs.
  **Checks:** MapGyver 276 passed/1 existing ignored; Golden plan 30 passed; Golden input
  software 12 passed/1 hardware ignored; new backend preflight regression passed; strict
  app/Golden all-target Clippy with `native-qualification`; UI check (0 errors/warnings);
  ordinary app/sender builds; bundled sender-only Inspector, independent variants, history,
  shared settings/Channels/catalogs and source Save/New/Load; root/Golden and touched-file
  formatting passed. Corrected native fixture compiles. The broad legacy source-browser
  fixtures still need their full revised-workflow port; they are not claimed verified.
  **Native limit:** two inspected, prerequisite-gated attempts. The second verified pixels and
  recovery through explicit renamed-sender selection, then stopped at an old opaque-alpha
  metadata assertion. The fixture now waits for the required straight-alpha resize metadata;
  it was compiled, not run a third time. [Exact coverage and return action](evidence/mapgyver-spout-metadata-native-2026-11-02.json)
  retain fixed-output resize, cross-adapter/both-adapter completion, RGBA pixels and declared
  overlap/capacity checks for revised M04 native acceptance after M04/1. No native gate closed.
  Started 2026-11-02 20:38 CET, cutoff 21:23 CET. Sampling remains in `72853505`, Channels
  in `151ca682`, Resolution in `a6a5ea11`. **Remaining:** procedural canvas/shared ownership,
  manager menus, complete module API and revised full browser/native parity; Reference/Preview
  and all retained return-queue requirements. Counts stay 24/57; 3/11 phases, 27% overall.

## Maintenance rule

Replace current state and the single current-batch/handoff entry in place. Keep this page
focused on progress, missing acceptance and next action. Link necessary detailed results in
owning reports/evidence; do not append daily logs, transcripts or duplicate dashboards.
At each implementation batch start, replace the handoff entry with deliverable, owner, proving
check and stopping time; at its end record only the current result and exact remaining scope.
