# MapGyver implementation status

**Updated: 2026-11-04 · Overall: `████████░░` 79% · 6/12 phases complete**

**Verified acceptance: 53/67 gates. Phase completion: 6/12 (50%).**

**Current topic:** M11/6 Accepted: shared Golden navigation/context controls and Surface selection styling.
**Next action:** M11/7 Composition-owned media creation and reference/instance operations.
**Blocker:** none for M11/7. Original physical/timing and app-control retirement gates remain unaccepted.
**Execution:** M11/7 alone is selected Active; M11/1–6 are Accepted. M03/9 retains its original qualification in Pending.
Retained physical/timing gates stay in the explicit return queue.
**Priority update:** [M11 feedback corrections](../plan/MapGyver_Implementation_Plan.md#m11--resolve-the-2026-11-03-feedback-before-video-and-final-qualification)
adds ten gates, **all ahead of M10 and M09**. Current functional order: M11 → M10 → M09;
M08/4's retained qualification must still pass before M10/M09. The recorded ordering change
uses AGENTS.md's functional-delivery rule and the plan's rule 6; no requirement is waived.
Earlier acceptance covers its tested contract; changed behavior must pass M11. Planning accepts no gate.
**Remaining requirement:** complete app-control transient retirement, M11 feedback, Video and retained
physical/performance/final qualification. Retained requirements and timing/retirement budgets
remain intact; M11/2 explicitly revises the fixed allocation-capacity policy.
**Newly usable:** [Shared canvas controls](../architecture/mapgyver-canvas-controls.md): centered navigation and contextual actions, bounded offscreen Matrix Frame, selected-only Surface paint and history-safe geometry input.
**Previously delivered:** [Live Shared Texture controls](../architecture/mapgyver-sender-selection.md): inline Manual Detect, Auto sender choices and exact recovery, including after project reload.
**Previously delivered:** [Direct channel File controls](mapgyver-m11-inspector.md#direct-channel-file-controls): one File per Picture channel forwards the real catalog Parameter; initial assignment is one atomic import, shared editing/clear/history and the secondary asset picker work without permanent import forms.
[Expanded typed channel controls](mapgyver-m11-inspector.md#expanded-typed-channel-controls) remain delivered.
Color edits need no nested Parameters folder; kind/sampling controls are gone and sampling is
fixed to Linear. Native editing/CRUD/history/paging and both-GPU Fit/Composition pixels pass.
Fit Mode, unit-preserving clocks and typed GPU Preferences remain delivered.
M11/4 is Accepted with complete native/browser/caller/history coverage; see the [acceptance audit](mapgyver-m11-inspector.md#m114-acceptance).
**Usable now:** typed media/References and independent previews; Screen/Surface mapping and
native outputs; child-media Composition/editors; Matrix model/playback/grid and typed owned
app control. The phase reports below own their detailed acceptance and remaining qualification.
**Last delivered capability:** shared public Golden toolbar and shortcuts across GraphCanvas,
Matrix, Composition, Surface Source and Screen Warping. Native navigation/alignment,
Composition handles, Screen key/history, two-editor narrow selection, Matrix offscreen Frame
and Surface Source input pass. Delayed history projections cannot seed a new gesture with
an old corner origin. Browser invalid-quad styling and all software checks pass.
[Evidence](evidence/mapgyver-m11-canvas-controls-2026-11-04.json).
The [Shared Texture cadence hotfix](mapgyver-shared-texture-cadence.md) remains verified and pushed.
This closes M11/6: 53/67 verified gates (79%); phase completion stays 6/12 (50%).
M08 remains 3/4 and Blocked on its separate retirement barrier.
Retained physical/timing requirements remain open; M11 owns changed feedback scope.

| Phase                                                                                                                                                              | State                           | Progress          | Gates | Subphase progression                                                                                                                                                                                                         |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------- | ----------------- | ----- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| [M00 Baseline](mapgyver-m00-baseline.md)                                                                                                                           | Accepted                        | `██████████` 100% | 1/1   | Baseline 1/1                                                                                                                                                                                                                 |
| <a id="m01-acceptance-items"></a>[M01 Native route](mapgyver-m01-native-feasibility.md#functional-route-decision--2026-10-28)                                      | Accepted                        | `██████████` 100% | 4/4   | Host/GPU/lifetime/decision 4/4                                                                                                                                                                                               |
| <a id="m02-acceptance-items"></a>[M02 Shared shell](mapgyver-m02-shell.md)                                                                                         | Accepted                        | `██████████` 100% | 7/7   | Build/identity/discovery/tools/host/isolation 7/7                                                                                                                                                                            |
| <a id="m03-acceptance-items"></a>[M03 Rendering](mapgyver-m03-render-foundations.md)                                                                               | Pending qualification | `████████░░` 78%  | 7/9   | Foundations 7/7; recovery 0/1; qualification 0/1                                                                                                                                                                             |
| <a id="m04-acceptance-items"></a>[M04 Media](mapgyver-m04-source-parity.md#m04-phase-audit)                                                                        | Accepted                        | `██████████` 100% | 8/8   | Sources 6/6; Reference 1/1; Preview 1/1                                                                                                                                                                                      |
| <a id="m05-acceptance-items"></a>[M05 Screens/editors](../plan/MapGyver_Implementation_Plan.md#m05--deliver-screens-surfaces-and-shared-navigation)                | Physical qualification retained | `████████░░` 80%  | 8/10  | Authoring/warp/calibration 3/3; editors 2/2; physical 1/3; bounds/workbench stats 2/2                                                                                                                                        |
| <a id="m06-acceptance-items"></a>[M06 Composition](../plan/MapGyver_Implementation_Plan.md#m06--deliver-child-media-composition-and-editor)                        | Accepted                        | `██████████` 100% | 5/5   | [Child model 1/1](mapgyver-m06-model.md); [dependencies 1/1](mapgyver-m06-dependencies.md); [native output 1/1](mapgyver-m06-native.md); [editor geometry 1/1](mapgyver-m06-editor.md); [panels 1/1](mapgyver-m06-panels.md) |
| <a id="m07-acceptance-items"></a>[M07 Matrix](../plan/MapGyver_Implementation_Plan.md#m07--deliver-child-media-matrix-playback-and-grid)                           | Accepted                        | `██████████` 100% | 4/4   | [Model 1/1](mapgyver-m07-model.md#m071-acceptance--2026-11-03); [playback 1/1](mapgyver-m07-playback.md); [grid 1/1](mapgyver-m07-grid.md); [native/scale 1/1](mapgyver-m07-native.md)                                       |
| <a id="m08-acceptance-items"></a>[M08 App control](../plan/MapGyver_Implementation_Plan.md#m08--deliver-reusable-optional-app-control)                             | Blocked: complete retirement    | `████████░░` 75%  | 3/4   | Golden process 1/1; [configuration/demand 1/1](mapgyver-m08-configuration.md#configuration-and-demand-acceptance--2026-11-03); [runtime 1/1](mapgyver-m08-runtime.md); [remaining acceptance 0/1](mapgyver-m08-retirement.md) |
| <a id="m11-acceptance-items"></a>[M11 Feedback](../plan/MapGyver_Implementation_Plan.md#m11--resolve-the-2026-11-03-feedback-before-video-and-final-qualification) | Active Composition-owned creation     | `██████░░░░` 60%  | 6/10  | [Stability 1/1](mapgyver-m11-stability.md); [capacity 1/1](mapgyver-m11-capacity.md); [GPU 1/1](mapgyver-m11-gpu.md#native-preferences-acceptance); [media 1/1](mapgyver-m11-inspector.md#m114-acceptance); [discovery 1/1](../architecture/mapgyver-sender-selection.md); [canvas 1/1](../architecture/mapgyver-canvas-controls.md); Composition/Matrix 0/2; identity/pattern 0/2                                                                                                                         |
| [M10 Video](../plan/MapGyver_Implementation_Plan.md#m10--deliver-video-playback-in-codec-tiers)                                                                    | Pending                         | `░░░░░░░░░░` 0%   | 0/4   | Common codecs 0/1; HAP 0/1; DXV/WebP 0/1; bundled integration 0/1                                                                                                                                                            |
| [M09 Final qualification](../plan/MapGyver_Implementation_Plan.md#m09--qualify-the-actual-app-and-windows-artifact)                                                | Waiting                         | `░░░░░░░░░░` 0%   | 0/1   | Revised actual-app qualification 0/1                                                                                                                                                                                         |

**Counting:** overall = verified acceptance gates / 67, rounded to the nearest percent.
Phase/subphase progress uses its fixed gate count; bars round to tenths. Overall now uses the
existing acceptance fraction instead of phase completion so accepted subphases are visible;
phase completion remains separately shown as 6/12 (50%). Gate and phase denominators are unchanged.
There are **53/67 accepted gates**. M11/6 adds one verified gate; M11/1–5 remain accepted.
M08 stays 3/4; M07 remains complete at 4/4. The 2026-11-03 feedback added M11's ten fixed gates,
changing the gate denominator 57 → 67 and phase denominator 11 → 12. Planning added no acceptance.
M11 owns changed-contract verification; prior evidence is reused only within its tested scope.
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

| State    | Gate(s)  | Missing requirement / concrete next or return action                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| -------- | -------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Pending  | M03/9    | [Urgent cadence regression repaired](mapgyver-shared-texture-cadence.md): actual native moving pixels pass with diagnostics disabled. Missing original GPU critical-path/window timing and private-growth qualification. Return to the dedicated qualification cohort after M11 feedback acceptance; no timing budget or full-gate credit is inferred from this hotfix. |
| Accepted | M11/4    | [Complete typed Inspector acceptance](mapgyver-m11-inspector.md#m114-acceptance): compact File rows, real shared Parameters, stable File/Color drafts, full generic/source browser flows and native catalog/channel history/project/CRUD/paging checks. Fit pixels, clocks, commands and four-caller parity remain verified. |
| Accepted | M11/3    | [Typed GPU Preferences and native acceptance](mapgyver-m11-gpu.md#native-preferences-acceptance): five cold starts, both explicit GPUs, separate persisted choice, truthful restart requirement, pinned renderer recreation, missing choice without fallback and recovery. Eight full pixel checks and 447 independent copies pass. |
| Accepted | M11/2    | [Complete capacity acceptance](mapgyver-m11-capacity.md#m112-acceptance--quiescence-releases-context-bindings): both GPUs pass eight 4K producers/two aliases, 56 complete images, bounded native ownership, reuse and real pressure/recovery. NVIDIA/Intel retire in 61.721/260.242 ms; pressure cases 49.990/93.013 ms, within the original two-second bound. |
| Accepted | M11/1    | [Native stability acceptance](mapgyver-m11-stability.md#m111-acceptance--native-input-and-project-transitions): recorded selection/reconnect fixes, actual Inspector/Matrix input, independent outputs, guarded desktop pixels, three close/reopen cycles and New/Load with old imagery retired. No prior brief-unavailable waiver used. |
| Blocked  | M08/4    | [Exact native barrier](mapgyver-m08-retirement.md): singleton root exited, active/pending zero, one uncaptured lifetime. Functional UI/script/project/access paths pass. Return with a documented identity-preserving completion mechanism for that lifetime, or an explicit user contract revision, then verify the retained exact fixture with original budgets. No unchanged probes or cleanup claim. Still required before M10/M09. |
| Accepted | M07/4    | [Full native audit](mapgyver-m07-native.md): both GPUs pass Layer/Group/Final/preview blends/alpha/order, shared source, deletion/Undo, 1000×1000/six-Cell bursts/reload/interruption and twelve completion-safe retirements. Bulk typed creation is corrected; M07 is complete at 4/4.                                                                                                                                                                                                                                                                      |
| Accepted | M08/3    | [Actual runtime audit](mapgyver-m08-runtime.md): both Spout readiness modes, process sharing/handoff, timeout/crash/backoff, native pre-resume cancellation, final-use retention, stale-project rejection and pinned root/branch/leaf retirement pass. M08 is 3/4; complete UI/target/access scope remains M08/4.                                                                                                                                                                                                                                            |
| Accepted | M11/5 | [Live sender control acceptance](../architecture/mapgyver-sender-selection.md): Manual String + inline Detect, Auto Enum, live metadata, exact recovery, independent GPU outputs, adapter mismatch, history and post-load mode/absence/recovery. |
| Accepted | M11/6 | [Shared canvas controls](../architecture/mapgyver-canvas-controls.md): public toolbar/shortcuts, contextual actions, bounded Matrix Frame, selected-only paint, native navigation/Composition/keyboard/history/source and narrow-peer checks pass. |
| Active | M11/7 | Composition-owned media creation, Reference/instance operations and acceptance across callers. |
| Pending | M11/8–10 | After M11/7: Matrix previews/Group output exposure, identity/tabs/footer and the official pattern. Execute sequentially before M10 and M09. |
| Pending  | M10/1–4  | Requires revised M04–M08 contracts **and all M11 gates Accepted**; then common-codec transport, HAP, DXV/WebP and bundled recovery/performance. Reuse M11 Channels/Fit Mode/time UI.                                                                                                                                                                                                                                                                                                                                                                         |
| Blocked  | M03/4    | Missing genuine device-removal/recovery acceptance. Return after a recorded safe reset/removal route is authorized and software prerequisites pass; preserved injected-recreation evidence alone is insufficient.                                                                                                                                                                                                                                                                                                                                            |
| Blocked  | M05/5,8  | Full physical hotplug, 150/200% and negative-origin/latency coverage is unverified. Native pixels and the known local displays now permit automatic cases; run the retained exact physical matrix, preserving missing configurations and measurement requirements.                                                                                                                                                                                                                                                                                           |
| Waiting  | M09/1    | All revised feature, **M11 feedback** and retained qualification gates; then complete Q01–Q06, actual-app statistics and Windows artifact checks. Reconcile the older cross-editor source-drop fixture's channel-row/reference-only assumptions with current ownership and M11 before final regression qualification.                                                                                                                                                                                                                                        |

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
- [Shared Texture cadence repair](mapgyver-shared-texture-cadence.md) owns the resolved urgent
  investigation and its limits; fix `2d29e381` is pushed. Do not reopen unchanged source probes.
  Git delivery uses existing HTTPS credentials per command; the saved SSH remote is unchanged.
  Preserve concurrent user work and the running user's locked executable.
- **Current batch handoff (2026-11-04):** M11/6 Accepted, completed before the cumulative
  16:13 CET investigation cutoff. Shared controls and Surface styling are delivered; the
  final correction fences Screen input against stale history projections. Owners: Golden
  graph/viewport and MapGyver editor/backend boundaries. Rust 419 passed/2 ignored, strict
  Clippy, generated contracts, Svelte 0/0, graph 60/UI 236/MapGyver 91 tests, graph browser/
  keyboard and both formatters pass. Actual native navigation, Composition handles,
  Screen keyboard/history, bounded Matrix/Frame, narrow-peer selection and Surface Source
  input pass with normal owned-host shutdown; invalid-quad browser styling also passes.
  [Pinned receipt](evidence/mapgyver-m11-canvas-controls-2026-11-04.json). Checkpoint `33d84d62`
  is pushed; push this acceptance milestone before the next implementation batch. No physical,
  timing or retirement gate is inferred. Progress: 53/67 (79%), M11 6/10, phases 6/12.
  Next product deliverable: M11/7 Composition-owned creation and instance/reference operations.

## Maintenance rule

Replace current state and the single current-batch/handoff entry in place. Keep this page
focused on progress, missing acceptance and next action. Link necessary detailed results in
owning reports/evidence; do not append daily logs, transcripts or duplicate dashboards.
At each implementation batch start, replace the handoff entry with deliverable, owner, proving
check and stopping time; at its end record only the current result and exact remaining scope.
