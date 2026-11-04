# MapGyver implementation status

**Updated: 2026-11-04 · Overall: `█████░░░░░` 50% · 6/12 phases complete**

**Verified gates: `████████░░` 75% · 50/67 accepted** (phase completion above remains 6/12).

**Current topic:** urgent Shared Texture cadence regression reported 2026-11-04.
**Next action:** compare the Spout Screen in a separate receiver with the editor preview, then repair the selected boundary and verify sustained moving content.
**Blockers:** that manual comparison is pending; milestone delivery is rejected by GitHub SSH authentication (`Permission denied (publickey)`).
**Execution:** M03/9 alone is Active for the user's urgent Shared Texture regression; M11/4 is Pending with its File/parity return action. Existing accepted gates retain their tested limits.
Retained physical/timing gates stay in the explicit return queue.
**Priority update:** [M11 feedback corrections](../plan/MapGyver_Implementation_Plan.md#m11--resolve-the-2026-11-03-feedback-before-video-and-final-qualification)
adds ten gates, **all ahead of M10 and M09**. Current functional order: M11 → M10 → M09;
M08/4's retained qualification must still pass before M10/M09. The recorded ordering change
uses AGENTS.md's functional-delivery rule and the plan's rule 6; no requirement is waived.
Earlier acceptance covers its tested contract; changed behavior must pass M11. Planning accepts no gate.
**Remaining requirement:** complete app-control transient retirement, M11 feedback, Video and retained
physical/performance/final qualification. Retained requirements and timing/retirement budgets
remain intact; M11/2 explicitly revises the fixed allocation-capacity policy.
**Newly usable:** [Expanded typed channel controls](mapgyver-m11-inspector.md#expanded-typed-channel-controls).
Color edits need no nested Parameters folder; kind/sampling controls are gone and sampling is
fixed to Linear. Native editing/CRUD/history/paging and both-GPU Fit/Composition pixels pass.
Fit Mode, unit-preserving clocks and typed GPU Preferences remain delivered.
M11/4 stays open for the single Picture File control and final complete Inspector acceptance.
**Usable now:** typed media/References and independent previews; Screen/Surface mapping and
native outputs; child-media Composition/editors; Matrix model/playback/grid and typed owned
app control. The phase reports below own their detailed acceptance and remaining qualification.
**Last delivered capability:** direct typed Color editing, fixed Linear sampling and rename-event
correction; 407 app tests, six integration tests, strict lint, UI checks, 78 native images and
native/headless browser editing/history/paging pass.
This slice closes no gate; verified progress remains 50/67. Retained physical/device-removal/timing
qualification remains required.
M08 remains 3/4 and Blocked on its separate retirement barrier.
Retained physical/timing requirements remain open; M11 owns changed feedback scope.

| Phase                                                                                                                                                              | State                           | Progress          | Gates | Subphase progression                                                                                                                                                                                                         |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------- | ----------------- | ----- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| [M00 Baseline](mapgyver-m00-baseline.md)                                                                                                                           | Accepted                        | `██████████` 100% | 1/1   | Baseline 1/1                                                                                                                                                                                                                 |
| <a id="m01-acceptance-items"></a>[M01 Native route](mapgyver-m01-native-feasibility.md#functional-route-decision--2026-10-28)                                      | Accepted                        | `██████████` 100% | 4/4   | Host/GPU/lifetime/decision 4/4                                                                                                                                                                                               |
| <a id="m02-acceptance-items"></a>[M02 Shared shell](mapgyver-m02-shell.md)                                                                                         | Accepted                        | `██████████` 100% | 7/7   | Build/identity/discovery/tools/host/isolation 7/7                                                                                                                                                                            |
| <a id="m03-acceptance-items"></a>[M03 Rendering](mapgyver-m03-render-foundations.md)                                                                               | Active Shared Texture regression | `████████░░` 78%  | 7/9   | Foundations 7/7; recovery 0/1; qualification 0/1                                                                                                                                                                             |
| <a id="m04-acceptance-items"></a>[M04 Media](mapgyver-m04-source-parity.md#m04-phase-audit)                                                                        | Accepted                        | `██████████` 100% | 8/8   | Sources 6/6; Reference 1/1; Preview 1/1                                                                                                                                                                                      |
| <a id="m05-acceptance-items"></a>[M05 Screens/editors](../plan/MapGyver_Implementation_Plan.md#m05--deliver-screens-surfaces-and-shared-navigation)                | Physical qualification retained | `████████░░` 80%  | 8/10  | Authoring/warp/calibration 3/3; editors 2/2; physical 1/3; bounds/workbench stats 2/2                                                                                                                                        |
| <a id="m06-acceptance-items"></a>[M06 Composition](../plan/MapGyver_Implementation_Plan.md#m06--deliver-child-media-composition-and-editor)                        | Accepted                        | `██████████` 100% | 5/5   | [Child model 1/1](mapgyver-m06-model.md); [dependencies 1/1](mapgyver-m06-dependencies.md); [native output 1/1](mapgyver-m06-native.md); [editor geometry 1/1](mapgyver-m06-editor.md); [panels 1/1](mapgyver-m06-panels.md) |
| <a id="m07-acceptance-items"></a>[M07 Matrix](../plan/MapGyver_Implementation_Plan.md#m07--deliver-child-media-matrix-playback-and-grid)                           | Accepted                        | `██████████` 100% | 4/4   | [Model 1/1](mapgyver-m07-model.md#m071-acceptance--2026-11-03); [playback 1/1](mapgyver-m07-playback.md); [grid 1/1](mapgyver-m07-grid.md); [native/scale 1/1](mapgyver-m07-native.md)                                       |
| <a id="m08-acceptance-items"></a>[M08 App control](../plan/MapGyver_Implementation_Plan.md#m08--deliver-reusable-optional-app-control)                             | Blocked: complete retirement    | `████████░░` 75%  | 3/4   | Golden process 1/1; [configuration/demand 1/1](mapgyver-m08-configuration.md#configuration-and-demand-acceptance--2026-11-03); [runtime 1/1](mapgyver-m08-runtime.md); [remaining acceptance 0/1](mapgyver-m08-retirement.md) |
| <a id="m11-acceptance-items"></a>[M11 Feedback](../plan/MapGyver_Implementation_Plan.md#m11--resolve-the-2026-11-03-feedback-before-video-and-final-qualification) | Pending urgent cadence repair     | `███░░░░░░░` 30%  | 3/10  | [Stability 1/1](mapgyver-m11-stability.md); [capacity 1/1](mapgyver-m11-capacity.md); [GPU 1/1](mapgyver-m11-gpu.md#native-preferences-acceptance); media/discovery 0/2; canvas/Composition/Matrix 0/3; identity/pattern 0/2                                                                                                                         |
| [M10 Video](../plan/MapGyver_Implementation_Plan.md#m10--deliver-video-playback-in-codec-tiers)                                                                    | Pending                         | `░░░░░░░░░░` 0%   | 0/4   | Common codecs 0/1; HAP 0/1; DXV/WebP 0/1; bundled integration 0/1                                                                                                                                                            |
| [M09 Final qualification](../plan/MapGyver_Implementation_Plan.md#m09--qualify-the-actual-app-and-windows-artifact)                                                | Waiting                         | `░░░░░░░░░░` 0%   | 0/1   | Revised actual-app qualification 0/1                                                                                                                                                                                         |

**Counting:** overall = fully accepted phases / 12, rounded to the nearest percent.
Phase/subphase progress = verified gates / that scope's fixed gates; bars round to tenths.
There are **50/67 accepted gates**. Complete typed Preferences and native adapter checks accept M11/3.
M08 stays 3/4. M07 remains
complete at 4/4 and fully accepted phases remain 6/12. Denominators are unchanged.
The 2026-11-03 feedback added M11 with ten fixed gates, changing the gate denominator 57 → 67
and phase denominator 11 → 12. Planning itself added no acceptance. M11 owns changed-contract
verification; prior evidence is reusable only where its tested behavior remains applicable.
The additional bar uses accepted gates / 67.
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
| Active   | M03/9    | Urgent user report: fast Spout sender, roughly 2 FPS received imagery while Solid Color stays smooth. Isolated receive/handoff copy counts are healthy; pending manual Screen-output versus editor-preview comparison selects the next repair. Verify changing pixels after repair. Original GPU critical-path/window timing and private-growth correction remain required in this gate; no unchanged benchmark reruns or budget revision. |
| Pending  | M11/4    | [Clocks, Fit Mode and direct Color editing delivered](mapgyver-m11-inspector.md). Return after urgent cadence repair: one typed Picture File with single asset authority, final complete generic-Parameters browser/caller/layout acceptance. Inspector checkpoint `974741f8` is committed locally; SSH fetch failed with publickey rejection, push unverified. |
| Accepted | M11/3    | [Typed GPU Preferences and native acceptance](mapgyver-m11-gpu.md#native-preferences-acceptance): five cold starts, both explicit GPUs, separate persisted choice, truthful restart requirement, pinned renderer recreation, missing choice without fallback and recovery. Eight full pixel checks and 447 independent copies pass. |
| Accepted | M11/2    | [Complete capacity acceptance](mapgyver-m11-capacity.md#m112-acceptance--quiescence-releases-context-bindings): both GPUs pass eight 4K producers/two aliases, 56 complete images, bounded native ownership, reuse and real pressure/recovery. NVIDIA/Intel retire in 61.721/260.242 ms; pressure cases 49.990/93.013 ms, within the original two-second bound. |
| Accepted | M11/1    | [Native stability acceptance](mapgyver-m11-stability.md#m111-acceptance--native-input-and-project-transitions): recorded selection/reconnect fixes, actual Inspector/Matrix input, independent outputs, guarded desktop pixels, three close/reopen cycles and New/Load with old imagery retired. No prior brief-unavailable waiver used. |
| Blocked  | M08/4    | [Exact native barrier](mapgyver-m08-retirement.md): singleton root exited, active/pending zero, one uncaptured lifetime. Functional UI/script/project/access paths pass. Return with a documented identity-preserving completion mechanism for that lifetime, or an explicit user contract revision, then verify the retained exact fixture with original budgets. No unchanged probes or cleanup claim. Still required before M10/M09. |
| Accepted | M07/4    | [Full native audit](mapgyver-m07-native.md): both GPUs pass Layer/Group/Final/preview blends/alpha/order, shared source, deletion/Undo, 1000×1000/six-Cell bursts/reload/interruption and twelve completion-safe retirements. Bulk typed creation is corrected; M07 is complete at 4/4.                                                                                                                                                                                                                                                                      |
| Accepted | M08/3    | [Actual runtime audit](mapgyver-m08-runtime.md): both Spout readiness modes, process sharing/handoff, timeout/crash/backoff, native pre-resume cancellation, final-use retention, stale-project rejection and pinned root/branch/leaf retirement pass. M08 is 3/4; complete UI/target/access scope remains M08/4.                                                                                                                                                                                                                                            |
| Pending  | M11/5–10 | After M11/4 closure: Manual/Auto Shared Texture, shared canvas controls/Surface styling, Composition-owned creation, Matrix previews/Group output exposure, identity/tabs/footer and the official pattern. Execute the numbered [feedback gates](../plan/MapGyver_Implementation_Plan.md#m11--resolve-the-2026-11-03-feedback-before-video-and-final-qualification) sequentially. All ten precede M10 and M09. |
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
- **Current batch/handoff:** urgent cadence repair, user priority at 10:42 CET / product stopping
  time 11:27 CET; investigation hard limit 11:42 CET (2026-11-04), baseline plus one repaired
  experiment maximum. Owner: Golden native Spout receive/bridge and MapGyver native publication.
  Baseline hypothesis: input receive/publication cadence is slow independently of the 60 FPS
  compositor. Two bounded checks against the user's Spout Demo Sender did not reproduce it:
  NVIDIA copied 166 times in 3.003 s without a consumer; with renderer handoff it copied 178
  times in 3.003 s, with zero SDK/bridge Busy observations or HRESULT failures. Intel cannot
  open that sender's texture (E_INVALIDARG); it is not treated as a working fallback.
  These are copy counts, **not proof of changing pixels or actual-app cadence**. The two-experiment
  allowance is consumed; do not extend this investigation with another probe variant. The next
  boundary decision requires the pending manual comparison: is MapGyver's Spout Screen image
  also slow in a separate Spout Receiver, or only in its editor preview? Return with that result
  to select source/rendering versus window presentation and verify actual moving content.
  The current live scene has two Demo Sender inputs feeding Matrix layers and a Spout Screen;
  the recording shows the slow warped preview. No runtime fix or acceptance is claimed.
  Golden Video software validation passes: 184 tests, 21 explicit hardware tests ignored;
  strict D3D11 library/test Clippy passes. The finite live isolation test is opt-in.
  Computer Use native pipe/browser host failed after bounded recovery; local app API at 7011
  works and the recording is decoded; the specific remaining manual comparison is requested above.
  M11/4's fully checked partial capability
  is isolated in local commit `974741f8`; fetch failed `Permission denied (publickey)` and delivery
  remains queued. No acceptance denominator change: 50/67, 6/12 phases. Preserve the user's
  concurrent work and do not restart their active app/processes without preserving its project.

## Maintenance rule

Replace current state and the single current-batch/handoff entry in place. Keep this page
focused on progress, missing acceptance and next action. Link necessary detailed results in
owning reports/evidence; do not append daily logs, transcripts or duplicate dashboards.
At each implementation batch start, replace the handoff entry with deliverable, owner, proving
check and stopping time; at its end record only the current result and exact remaining scope.
