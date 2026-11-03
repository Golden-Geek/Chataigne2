# MapGyver implementation status

**Updated: 2026-11-03 · Overall: `████░░░░░░` 42% · 5/12 phases complete**

**Verified gates: `███████░░░` 67% · 45/67 accepted** (phase completion above remains 5/12).

**Current topic:** M07/1–3 model/playback/grid Accepted; M07/4 native pixels/scale/lifetime is Active.
**Next deliverable:** Correct bulk typed Matrix authoring cost with software proof, then finish the unchanged full native sparse/burst/reload/lifetime matrix on both GPUs.
**Execution:** M08/1–2, M04/1–8, M05/4,6–7,9–10 and M06/1–5 accepted; M07/1–3 accepted; M07/4 alone is Active on `mapgyver`.
Retained physical/timing gates stay in the explicit return queue.
**Priority update:** [M11 feedback corrections](../plan/MapGyver_Implementation_Plan.md#m11--resolve-the-2026-11-03-feedback-before-video-and-final-qualification)
adds ten Pending gates, **all ahead of M10 and M09**. Current order: M07/4 → M08/3–4 → M11 → M10 → M09.
Earlier acceptance covers its tested contract; changed behavior must pass M11. Planning accepts no gate.
**Remaining requirement:** Matrix native/scale, reusable app-control qualification, M11 feedback, Video and retained
physical/performance/final qualification. Retained requirements and timing/retirement budgets
remain intact; M11/2 explicitly revises the fixed allocation-capacity policy.
**Newly usable:** [Complete revised Matrix grid](mapgyver-m07-grid.md): square Cells, contextual
media/structure operations, inline rename, virtualized drag/reparent/sort, shared offscreen
Inspector and actual native requested/submitted/retirement feedback. Source rejection/loss/
recovery and same-ID project reload during a held drag pass.
**Usable now:** typed media/References and independent previews; Screen/Surface mapping and
native outputs; child-media Composition/editors; Matrix model/playback/grid and typed owned
app control. The phase reports below own their detailed acceptance and remaining qualification.
**Last delivered capability:** M07/3 complete grid acceptance, including the native idle-status
correction and narrow shared Inspector layout. 392 library tests, strict Clippy, 29 UI tests,
Svelte 0/0, complete current browser/project checklist and real native feedback pass.
M07/4 and M08/3–4 retain their full requirements. No physical/timing budget is waived.

| Phase                                                                                                                                                              | State                           | Progress          | Gates | Subphase progression                                                                                                                                                                                                         |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------- | ----------------- | ----- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| [M00 Baseline](mapgyver-m00-baseline.md)                                                                                                                           | Accepted                        | `██████████` 100% | 1/1   | Baseline 1/1                                                                                                                                                                                                                 |
| <a id="m01-acceptance-items"></a>[M01 Native route](mapgyver-m01-native-feasibility.md#functional-route-decision--2026-10-28)                                      | Accepted                        | `██████████` 100% | 4/4   | Host/GPU/lifetime/decision 4/4                                                                                                                                                                                               |
| <a id="m02-acceptance-items"></a>[M02 Shared shell](mapgyver-m02-shell.md)                                                                                         | Accepted                        | `██████████` 100% | 7/7   | Build/identity/discovery/tools/host/isolation 7/7                                                                                                                                                                            |
| <a id="m03-acceptance-items"></a>[M03 Rendering](mapgyver-m03-render-foundations.md)                                                                               | Blocked                         | `████████░░` 78%  | 7/9   | Foundations 7/7; recovery 0/1; qualification 0/1                                                                                                                                                                             |
| <a id="m04-acceptance-items"></a>[M04 Media](mapgyver-m04-source-parity.md#m04-phase-audit)                                                                        | Accepted                        | `██████████` 100% | 8/8   | Sources 6/6; Reference 1/1; Preview 1/1                                                                                                                                                                                      |
| <a id="m05-acceptance-items"></a>[M05 Screens/editors](../plan/MapGyver_Implementation_Plan.md#m05--deliver-screens-surfaces-and-shared-navigation)                | Physical qualification retained | `████████░░` 80%  | 8/10  | Authoring/warp/calibration 3/3; editors 2/2; physical 1/3; bounds/workbench stats 2/2                                                                                                                                        |
| <a id="m06-acceptance-items"></a>[M06 Composition](../plan/MapGyver_Implementation_Plan.md#m06--deliver-child-media-composition-and-editor)                        | Accepted                        | `██████████` 100% | 5/5   | [Child model 1/1](mapgyver-m06-model.md); [dependencies 1/1](mapgyver-m06-dependencies.md); [native output 1/1](mapgyver-m06-native.md); [editor geometry 1/1](mapgyver-m06-editor.md); [panels 1/1](mapgyver-m06-panels.md) |
| <a id="m07-acceptance-items"></a>[M07 Matrix](../plan/MapGyver_Implementation_Plan.md#m07--deliver-child-media-matrix-playback-and-grid)                           | Active native/scale             | `████████░░` 75%  | 3/4   | [Model 1/1](mapgyver-m07-model.md#m071-acceptance--2026-11-03); [playback 1/1](mapgyver-m07-playback.md); [grid 1/1](mapgyver-m07-grid.md); [native/scale 0/1](mapgyver-m07-native.md)                                       |
| <a id="m08-acceptance-items"></a>[M08 App control](../plan/MapGyver_Implementation_Plan.md#m08--deliver-reusable-optional-app-control)                             | Runtime qualification pending   | `█████░░░░░` 50%  | 2/4   | Golden process 1/1; [configuration/demand 1/1](mapgyver-m08-configuration.md#configuration-and-demand-acceptance--2026-11-03); runtime 0/1; app acceptance 0/1                                                               |
| <a id="m11-acceptance-items"></a>[M11 Feedback](../plan/MapGyver_Implementation_Plan.md#m11--resolve-the-2026-11-03-feedback-before-video-and-final-qualification) | Pending before M10/M09          | `░░░░░░░░░░` 0%   | 0/10  | Stability/capacity/GPU 0/3; media/discovery 0/2; canvas/Composition/Matrix 0/3; identity/pattern 0/2                                                                                                                         |
| [M10 Video](../plan/MapGyver_Implementation_Plan.md#m10--deliver-video-playback-in-codec-tiers)                                                                    | Pending                         | `░░░░░░░░░░` 0%   | 0/4   | Common codecs 0/1; HAP 0/1; DXV/WebP 0/1; bundled integration 0/1                                                                                                                                                            |
| [M09 Final qualification](../plan/MapGyver_Implementation_Plan.md#m09--qualify-the-actual-app-and-windows-artifact)                                                | Waiting                         | `░░░░░░░░░░` 0%   | 0/1   | Revised actual-app qualification 0/1                                                                                                                                                                                         |

**Counting:** overall = fully accepted phases / 12, rounded to the nearest percent.
Phase/subphase progress = verified gates / that scope's fixed gates; bars round to tenths.
There are **45/67 accepted gates**. The 2026-11-03 feedback adds **M11 with ten fixed gates**,
changing the gate denominator **57 → 67** and phase denominator **11 → 12**. No acceptance is
added or removed by planning: M07 stays 3/4, M08 stays 2/4 and five phases remain accepted for
their earlier scope. M11 owns changed-contract acceptance; prior evidence is reusable only
where its tested behavior remains applicable. The additional bar uses accepted gates / 67.
Partial work, planning and unavailable checks earn no gate.

**2026-11-02 scope revision:** the prior plan was 5/10 phases (50%), with 40/48 gates accepted.
The updated requirements reopen affected M04–M08 acceptance and add M10. M04 changes 6/6→1/8
(reopen 1,2,4,5,6; add Reference/Preview); M05 5/8→3/10 (reopen 6,7; add bounds/supplements);
M06 4/4→0/5 (reopen 1–4; add independent panels); M07 3/4→0/4 (reopen 1,2,4);
M08 3/4→1/4 (reopen 2,3); M10 adds 0/4. Other gates are unchanged.
This adds nine gates, reopens sixteen and changes the phase denominator from 10 to 11.
Earlier evidence is retained for reuse, with its original schema/build limits; no new feature
or gate was accepted by merging the plan.

## Execution board and return queue

Exactly one gate is selected Active. Pending rows are the return queue, not concurrent work.

| State   | Gate(s)  | Missing requirement / concrete next or return action                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| ------- | -------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Active  | M07/4    | [Native checkpoint](mapgyver-m07-native.md): both-adapter Cell/Layer/fades and Intel pre-scale pixels pass. Both complete-scene attempts reject preview Layout after bulk 1000×1000/six-Cell authoring. Correct bulk typed-authoring cost with software proof before another native run. Sparse/burst/reload/sharing/final retirement on both GPUs remain required; no limits are waived.                                                                                                                                                                                            |
| Pending | M08/3–4  | After M07, aggregate app-control runtime/UI qualification, actual picker, commands/snippets and retained retirement/access cases. Prior partial process/renderer evidence remains reusable.                                                                                                                                                                                                                                                                                                                                                                                          |
| Pending | M11/1–10 | After the current M07/4 and M08/3–4 closure, resolve stable UI/previews, 4K resource capacity, GPU preference/options, minimal typed Channels/Fit Mode/time UI, Manual/Auto Shared Texture, shared canvas controls/Surface styling, Composition-owned creation, Matrix previews/Group output exposure, identity/tabs/footer and the official pattern. Execute the numbered [feedback gates](../plan/MapGyver_Implementation_Plan.md#m11--resolve-the-2026-11-03-feedback-before-video-and-final-qualification) sequentially; reproduce stability first. All ten precede M10 and M09. |
| Pending | M10/1–4  | Requires revised M04–M08 contracts **and all M11 gates Accepted**; then common-codec transport, HAP, DXV/WebP and bundled recovery/performance. Reuse M11 Channels/Fit Mode/time UI.                                                                                                                                                                                                                                                                                                                                                                                                 |
| Blocked | M03/4    | Missing genuine device-removal/recovery acceptance. Return after a recorded safe reset/removal route is authorized and software prerequisites pass; preserved injected-recreation evidence alone is insufficient.                                                                                                                                                                                                                                                                                                                                                                    |
| Pending | M03/9    | Original GPU critical-path/window timing and reproduced private-growth failure need corrective qualification, not additional user/tool permission. Return in its bounded qualification lane with an ownership correction and supported timing evidence; no unchanged benchmark reruns or budget revision.                                                                                                                                                                                                                                                                            |
| Blocked | M05/5,8  | Full physical hotplug, 150/200% and negative-origin/latency coverage is unverified. Native pixels and the known local displays now permit automatic cases; run the retained exact physical matrix, preserving missing configurations and measurement requirements.                                                                                                                                                                                                                                                                                                                   |
| Waiting | M09/1    | All revised feature, **M11 feedback** and retained qualification gates; then complete Q01–Q06, actual-app statistics and Windows artifact checks. Reconcile the older cross-editor source-drop fixture's channel-row/reference-only assumptions with current ownership and M11 before final regression qualification.                                                                                                                                                                                                                                                                |

## Evidence and handoff

- [Historical acceptance and batch evidence](mapgyver-evidence-through-2026-11-02.md) preserves
  the complete previous ledger, including the unfinished Matrix Parameter batch and rejected
  timing/memory proposals. Its old execution boards are non-operative.
- [Qualification contract](mapgyver-qualification-contract.md) owns retained budgets, revised
  Composition/Reference and Video workloads, and separately accounted software decode uploads.
- Native user observations: [Screen/Surface](evidence/mapgyver-m05-manual-editor-acceptance-2026-11-02.json),
  [Composition](evidence/mapgyver-m06-manual-editor-acceptance-2026-11-02.json),
  [Matrix playback](evidence/mapgyver-m07-manual-playback-acceptance-2026-11-02.json).
  Brief “preview unavailable” on restore was previously accepted for that scope; M11/1 now requires
  explicit flash/transition acceptance. Other timing/alignment budgets remain.
- Prior Matrix Parameter and icon/presentation work is preserved, unaccepted, in named stash
  `mapgyver-pre-revision-matrix-parameters-and-icons-2026-11-02`, object
  `f550ec0c03e907e9ebab44891538f1030358c30a`. Do not restore it automatically. Its typed-row
  approach is reconciled in the current conversion; its older model/host/presentation
  changes remain un-restored and unaccepted.
- **Current batch/handoff:** M07/4 native qualification checkpoint, started 23:12 CET /
  stopping time 23:57 CET. Owner: Matrix authoring/capture and the public-contract native fixture.
  Delivered fixture reconciliation for actual 3×3 defaults and explicit media/Layer sizes,
  plus a software ownership/order/project preflight. Production behavior is unchanged.
  [Native audit and receipt](mapgyver-m07-native.md): both-adapter Cell/Layer/fade suites and
  Intel pre-scale blend/alpha/order/source-loss/Undo/empty pixels pass. The optimized attempt
  records 108 complete image captures, zero error outside legal UNORM ranges, and six interrupted
  observation groups. **Complete native acceptance fails** after exact 1000×1000/six-Cell authoring:
  preview Layout is rejected before scale pixels, burst, recovery or reload. Lease expiry is
  inferred; its exact response was absent from the original assertion. The two native attempts
  are exhausted: 3 passed/1 failed in 198.47 seconds (dev) and 173.89 seconds (optimized test).
  No third native run and no resource/watchdog/tolerance changes.
  The final checkpoint adds exact rejection/stage reporting and distinct-owner/single-source
  assertions. **12 software tests and strict all-target native/process Clippy pass**; both Cargo
  formatters pass. The strengthened sharing assertion remains native-unrun.
  **Gate changes: none; M07/4 stays Active.** Current scope includes the separately delivered
  M11 feedback plan: **45/67 gates (67%), 5/12 phases (42%)**, with unchanged acceptance counts.
  Last usable capability remains M07/3's complete grid, milestone `fcff2f6d`.
  **Next product deliverable:** correct bulk typed-authoring cost at the Matrix/Golden boundary
  with software proof, preserving ownership/history/callback/UI semantics. Then finish the
  unchanged sparse/burst/reload/sharing/final-retirement matrix on both GPUs. Do not restart an
  unchanged native experiment; this checkpoint resets neither the investigation allowance nor
  retained requirements. M08/3–4, M11, M10 and retained physical/performance gates keep their queue.
  Concurrent app/manager-label/theme/architecture/browser work and the named stash remain preserved.
  Host capacity recovery is active on this goal's verified proxy route (reconfirmed 22:40 UTC);
  actual recovery from a live overload remains unobserved.

## Maintenance rule

Replace current state and the single current-batch/handoff entry in place. Keep this page
focused on progress, missing acceptance and next action. Link necessary detailed results in
owning reports/evidence; do not append daily logs, transcripts or duplicate dashboards.
At each implementation batch start, replace the handoff entry with deliverable, owner, proving
check and stopping time; at its end record only the current result and exact remaining scope.
