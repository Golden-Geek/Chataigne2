# MapGyver implementation status

**Updated: 2026-11-02 · Overall: `███░░░░░░░` 27% · 3/11 phases complete**

**Current topic:** revised M04 concurrent native variants and shared Resolution.
**Next deliverable:** verify M04/2's distinct native source sizes, forced Resolution, shared
consumers and stable channel identities through resize/reorder/history/reload.
**Execution:** M04/1 accepted; M04/2 selected next on `mapgyver`, with one active gate.
**Blocker for next delivery:** none known. Retained native/timing/memory/display blockers are below.
**Usable now:** shared typed media Resolution/Sampling and bounded Channels editing through
the generic Inspector, media-owned procedural canvases and Matrix settings, sender-only Shared
Texture authoring, typed source commands and named scripting from the Inspector, source media,
Screen/Surface editing, Composition and Matrix playback.
**Last delivered capability:** command execution Undo preserves authored configuration after rapid
vector-field editing. Golden text fields commit discrete writes; slider gestures retain grouping,
and delayed edit-session reopens respect cancellation and project lifetime. Source automation's
defaults, commands, methods/callbacks/snippets, generic editing and history are now accepted.
**Last accepted:** [M04/1 source authoring/API](mapgyver-m04-source-api.md). Retained prior-schema
native Screen/Surface, Composition and Matrix observations do not close revised native gates.

| Phase                                                                                                                                                | State                           | Progress          | Gates | Subphase progression                                                                  |
| ---------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------- | ----------------- | ----- | ------------------------------------------------------------------------------------- |
| [M00 Baseline](mapgyver-m00-baseline.md)                                                                                                             | Accepted                        | `██████████` 100% | 1/1   | Baseline 1/1                                                                          |
| <a id="m01-acceptance-items"></a>[M01 Native route](mapgyver-m01-native-feasibility.md#functional-route-decision--2026-10-28)                        | Accepted                        | `██████████` 100% | 4/4   | Host/GPU/lifetime/decision 4/4                                                        |
| <a id="m02-acceptance-items"></a>[M02 Shared shell](mapgyver-m02-shell.md)                                                                           | Accepted                        | `██████████` 100% | 7/7   | Build/identity/discovery/tools/host/isolation 7/7                                     |
| <a id="m03-acceptance-items"></a>[M03 Rendering](mapgyver-m03-render-foundations.md)                                                                 | Blocked                         | `████████░░` 78%  | 7/9   | Foundations 7/7; recovery 0/1; qualification 0/1                                      |
| <a id="m04-acceptance-items"></a>[M04 Media](../plan/MapGyver_Implementation_Plan.md#m04--deliver-source-variants-reference-media-and-media-preview) | Active                          | `███░░░░░░░` 25%  | 2/8   | Sources 2/6; Reference 0/1; Preview 0/1                                               |
| <a id="m05-acceptance-items"></a>[M05 Screens/editors](../plan/MapGyver_Implementation_Plan.md#m05--deliver-screens-surfaces-and-shared-navigation)  | Pending; physical gates blocked | `███░░░░░░░` 30%  | 3/10  | Authoring/warp/calibration 3/3; editors 0/2; physical 0/3; bounds/workbench stats 0/2 |
| <a id="m06-acceptance-items"></a>[M06 Composition](../plan/MapGyver_Implementation_Plan.md#m06--deliver-child-media-composition-and-editor)          | Pending                         | `░░░░░░░░░░` 0%   | 0/5   | Child model 0/1; dependencies 0/1; native output 0/1; transforms 0/1; panels 0/1      |
| <a id="m07-acceptance-items"></a>[M07 Matrix](../plan/MapGyver_Implementation_Plan.md#m07--deliver-child-media-matrix-playback-and-grid)             | Pending                         | `░░░░░░░░░░` 0%   | 0/4   | Model 0/1; playback 0/1; grid 0/1; native/scale 0/1                                   |
| <a id="m08-acceptance-items"></a>[M08 App control](../plan/MapGyver_Implementation_Plan.md#m08--deliver-reusable-optional-app-control)               | Pending                         | `███░░░░░░░` 25%  | 1/4   | Golden process 1/1; config/demand 0/1; runtime 0/1; app acceptance 0/1                |
| [M10 Video](../plan/MapGyver_Implementation_Plan.md#m10--deliver-video-playback-in-codec-tiers)                                                      | Pending                         | `░░░░░░░░░░` 0%   | 0/4   | Common codecs 0/1; HAP 0/1; DXV/WebP 0/1; bundled integration 0/1                     |
| [M09 Final qualification](../plan/MapGyver_Implementation_Plan.md#m09--qualify-the-actual-app-and-windows-artifact)                                  | Waiting                         | `░░░░░░░░░░` 0%   | 0/1   | Revised actual-app qualification 0/1                                                  |

**Counting:** overall = fully accepted phases / 11, rounded to the nearest percent.
Phase/subphase progress = verified gates / that scope's fixed gates; bars round to tenths.
There are **25/57 accepted gates**. Partial work, planning and unavailable checks earn no gate.

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

| State    | Gate(s)          | Missing requirement / concrete next or return action                                                                                                                                                                                                                                                                   |
| -------- | ---------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Active   | M04/2            | Verify distinct native source sizes, disabled/enabled shared media Resolution, shared consumers and stable variants through resize/reorder/history/reload. Compile/software checks must pass before running the corrected native fixture.                                                                              |
| Accepted | M04/1            | [Default manager, detached creation/import, authoritative identity/defaults and complete command/function/callback/snippet API](mapgyver-m04-source-api.md), including the corrected generic vector-field/command history boundary.                                                                                    |
| Pending  | M04/4–8          | After M04/2, finish revised Spout/native checks, including the corrected lifecycle and declared-input capacity cases in the linked evidence; audit generic UI/lifetime and broad revised project/resource parity, then Reference and Media Preview. M04/3 Picture worker bounds retain acceptance.                     |
| Pending  | M05/6,7,9,10     | After working revised media, deliver shared smooth navigation, Screen handles/keyboard/Fill/Fit, stable visible output-bounds updates and outliner supplements.                                                                                                                                                        |
| Pending  | M06/1–5          | After M04 ownership/Reference and M05 viewport contracts, replace instances with child media, use degrees/new anchors and verify independent follow/lock panels.                                                                                                                                                       |
| Pending  | M08/2–4; M07/1–4 | After M06, establish reusable app-control configuration/runtime keys, then revised Matrix ownership/grid/trigger semantics and aggregate demand. Verify actual picker and retained retirement/access cases; remove Applications workflow.                                                                              |
| Pending  | M10/1–4          | After revised M04–M08 contracts, implement common-codec transport, HAP, DXV/WebP and bundled recovery/performance in that order.                                                                                                                                                                                       |
| Blocked  | M03/4            | Missing genuine device-removal/recovery acceptance. Return after a recorded safe reset/removal route is authorized and software prerequisites pass; preserved injected-recreation evidence alone is insufficient.                                                                                                      |
| Blocked  | M03/9            | Original GPU critical-path proof/window timing remain unresolved; private-growth criterion failed. Return with supported bounded-error timing evidence or an explicit scope decision, and a concrete ownership correction for the preserved memory failure. No unchanged benchmark reruns or implicit budget revision. |
| Blocked  | M05/4,5,8        | Missing physical concurrent-output/hotplug/DPI/presentation coverage. Return with reported display resolution/scaling/layout/available outputs and actual native access; run the exact retained matrix.                                                                                                                |
| Waiting  | M09/1            | All revised feature and retained qualification gates; then complete Q01–Q06, actual-app statistics and Windows artifact checks.                                                                                                                                                                                        |

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
- **Current milestone:** M04/1 accepted. The [requirement-by-requirement audit](mapgyver-m04-source-api.md)
  verifies the delivered default manager, detached creation/import, identity/defaults, typed
  commands, script methods/callbacks/snippets, generated contracts and usable generic editing.
  The combined browser workflow reproduced a real vector-field edit-session leak: command Undo
  also reset its settings. Golden now gives text commits discrete history, retains acknowledged
  drafts, and cancels/finishes queued gesture reopens with host/project ownership guards.
  Canonical depth-zero queries and the actual Inspector verify configuration survives Undo.
  **Checks:** 285 MapGyver library tests passed/one existing ignored; 228 Golden UI tests passed,
  including 11 edit-session ownership cases; both app UI checks report zero errors/warnings.
  Rebuilt bundled app source, combined Channels/Resolution/command/script/catalog workflows and
  ordinary source/history/Save/New/Load pass. Rust is unchanged; prior strict Clippy is retained.
  A user-opened default executable was locked; an isolated build of the same app entrypoint
  verified the rebuilt bundle, and its temporary Cargo manifest entries were removed.
  Root/Golden formatting and diff checks pass. No native experiment ran.
  Batch 2026-11-02 23:08–23:53 CET, delivered within its cutoff. **Gate change:** M04/1 Active →
  Accepted; M04/2 is the sole next Active gate. **Next:** revised concurrent native variants,
  media Resolution/sharing and identity acceptance, after terminal compile/software prerequisites.
  Broad legacy browser ports and native resource parity remain required under M04/6; Reference,
  Preview and all retained return-queue requirements remain open. Shared Texture retains
  [partial native evidence](evidence/mapgyver-spout-metadata-native-2026-11-02.json).
  Counts: **25/57 gates, M04 2/8 (25%), 3/11 phases (27%)**; no denominator change.

## Maintenance rule

Replace current state and the single current-batch/handoff entry in place. Keep this page
focused on progress, missing acceptance and next action. Link necessary detailed results in
owning reports/evidence; do not append daily logs, transcripts or duplicate dashboards.
At each implementation batch start, replace the handoff entry with deliverable, owner, proving
check and stopping time; at its end record only the current result and exact remaining scope.
