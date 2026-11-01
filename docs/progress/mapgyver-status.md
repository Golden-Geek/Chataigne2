# MapGyver implementation status

## At a glance

**Updated: 2026-11-01 · Overall: `████████░░░░░░░░░░░░` 40% · 4/10 phases complete**

**Execution: Golden redesign functional input and project/lifecycle checks pass.**
Latest capability: Applications handles deletion-raced reads as typed absence, clears Inspector
data and restores properties through Undo. Complete Application and column/shared-process UI
flows now pass in the tree/Inspector layout. The redesign's browser migration is finished.
No full acceptance gate closes: M08 remains3/4 for native picker/desktop, singleton retirement
and full-app scope. Return on supported current-build native interaction/manual observations;
the user says native interaction is unavailable, so do not repeat that request. M03/M05–M07
qualification blockers remain in the board; M09 waits. No further browser rerun substitutes for
these missing requirements. Overall4/10=40%, with all original gates and budgets retained.

**M08 deleted-selection correction —21:15 CET, stop22:00 CET:** previous turn is progress,
checkpoint `917a569e` pushed and clean. Same gate4; retained2-run investigation is not reset.
Code review identifies the owning contract: an Inspect valid when queued can arrive after deletion;
graph-cache checks cannot make a remote read atomic with another edit. Add an explicit typed
ApplicationMissing query reply at the app backend, clear current Inspector data on it, retain
wrong-type/mutation failures and stale-reply fencing. Prove deletion/Undo/Redo without history or
process mutation and superseded missing replies in software before one acceptance run of the
materially corrected app plus the pending column fixture. No unchanged-candidate diagnostic run.
Regenerate protocol from Rust and keep strict unexpected-rejection checks; native gates retained.

**M08 correction result —21:19 CET:** the single acceptance run of the corrected contract passes
both full Application and column fixtures, **exit0/16,413ms** hosted flow. Applications has exactly
one expected invalid executable rejection and zero page errors; the deletion-raced Inspect is
typed ApplicationMissing, with no retry or failed receipt. Column association/distances/history,
missing definitions, copied references, two-Matrix shared demand, preload/postload, availability
retention/recovery, truthful runtime refresh and actual New cleanup pass with0rejections/errors.
All3owned PIDs are absent after cleanup; host exits0. Screenshots inspected: filtered Application
tree/shared Inspector; final New shows empty tree/no selection. Rust211pass/1existing ignore,
frontend69pass, Svelte0errors/0warnings, clippy and ordinary rebuild pass. Rust tests retain strict
missing-definition mutation and wrong-type rejection; all public callers return the same typed
absence without history mutation. [Evidence](evidence/mapgyver-m08-workbench-acceptance-2026-11-01.json).
M08's browser redesign scope is verified; remaining native/OS/retirement/full-app requirements
stay blocked with their existing return triggers. Do not reopen the completed stale-read
investigation or run another browser comparison. Publish this coherent correction.

**M08 Application-layout batch —21:04 CET, stop21:49 CET:** `4c760af5` pushed and remotely
verified; worktree clean. Only gate4's existing browser input is active. Owner: app Applications
tree/Inspector selection and Application/column browser fixtures. Preserve complete definition,
duplicate/enable/remove/invalid/history and no-launch-on-form-edit checks; preserve column
association distances/history, shared ownership, actual finite owned-process demand/recovery and
File New cleanup scope. Reuse the ordinary software-qualified candidate unless runtime changes
require rebuilding. At most two live comparisons/20minutes investigation; no native dialog,
OS retirement/full-app acceptance inferred and no unavailable-native route retry.

Software prerequisites pass:67frontend tests, Svelte0errors/0warnings, ordinary candidate rebuild
and the unchanged finite owned executable built through its existing fixture feature. Initial
combined build omitted that fixture-only feature and stopped before any live invocation; repaired
by separate ordinary-app and supported fixture builds. Invocation1 passes Application definition,
duplicate/enable/remove/history and invalid path checks, then strict receipt check finds2rejections
instead of the single expected invalid path. Column fixture has not run. Preserve this failure;
its extra receipt was not printed, so no cause is claimed. The Inspector had two overlapping
observation effects and could inspect an old selection while its node was removed/replaced.
Unify node/session observation and clear selection before reading when the graph owner is absent
or changes. Include receipt details on any further failure; final invocation retains all scope.

**M08 partial result —21:10 CET:** invocation2 still reaches the final strict receipt assertion:
one expected unsupported `.cmd` Configure rejection plus one **Inspect(application) → application
reference is missing** after the removal Redo/Undo sequence. Graph-owner checks alone do not
resolve the receipt/delta ordering; preserve the exact failure. Creation/name/full argument and
environment/readiness/stop/restart forms, definition Undo/Redo, duplicate Inspector adoption,
enablement independence, removal and invalid definition state/history assertions pass before
that assertion. The complete fixture and column process/lifetime migration are **not accepted**;
column fixture never starts and no owned target process launches. Both hosts exit0 normally.
Stop at2live invocations. Product changes pass67frontend tests, Svelte0errors/0warnings and
ordinary rebuild. Next: software-test and repair selection query dispatch across deletion-history
receipts and graph publication, then validate remaining column scope without relaxing rejection
checks. [Partial evidence](evidence/mapgyver-m08-workbench-partial-2026-11-01.json).
No gate closes: M08 stays3/4, overall4/10=40%; same active gate, all native blockers retained.

**M07 Matrix-layout batch — 2026-11-01 20:38 CET, stop 21:23 CET:** previous turn is progress,
pushed as `a74e3d1c`, clean checkout. Only M07 gate3's remaining redesigned input is active.
Owner: app Matrix window controller/grid/shared Inspector and existing browser fixtures. Preserve
full row/group/column/cell selection/assignment/settings/blends/names/copy/remove/move/history,
requested playback distinct from rendering, source loss/recovery, virtualized keyboard scroll/
resize/panel lifetime and actual New/review-upload/fresh-epoch/history scope. Toolbar creation
must adopt the Matrix Inspector; removal must retain a usable parent. Use existing backend
result IDs, guarded selection and shared canvas error overlay; no UI domain defaults or fake
native renderer observations. Run software/build prerequisites before at most two finite host
invocations/20 minutes investigation within21:23. Native feedback remains an open prerequisite.

Prerequisites pass:64frontend tests, Svelte0errors/0warnings and ordinary bundled build. Review
fixture SHA256 `75610307…0540ab9` matches its manifest. Invocation1 passes selection/assignment,
feedback-cycle rejection, all layer/group blends and exact history, then fails to commit the
column transition override; no page errors or other rejected request. The final form has its
override unchecked. The fixture had required the backend history result but not the Inspector's
corresponding refreshed Matrix transition. Require the rendered320x240/350ms/SmoothStep values
before drafting the column override and assert its checked100ms state immediately before Save.
Preserve the first failure; no product cause inferred yet. Invocation2 is the final permitted
live comparison, with all original scope/tolerances retained.

**M07 redesign input result — 20:51 CET:** invocation2 passes both complete original fixtures
and source/history/Save/New/Load baseline, **exit0 /26,265 ms** hosted flow. Toolbar creation
adopts the committed Matrix channel Inspector, and selected row/column removal retains a usable
parent. Backend result IDs and the existing pre-operation selection guard fence late replies.
Grid selection never authors or launches. Assignment/drafts, cycle rejection, every layer/group
blend, canvas/Matrix/column transitions, names, copy/remove/move/reorder, exact history, Cut/
Configured/Unload, source loss/Undo and panel lifetime pass. Twenty-four rows/sixteen columns,
keyboard scrolling and1440→2560→1440 resize preserve the document; only210/384 cells render.
The sole expected cycle rejection leaves geometry fixed within0.01CSS pixels; no page errors.
Actual File New replaces an active Matrix, clears selection/history, creates fresh identities
and project epochs, uploads the existing review through File Open, restores saved IDs and usable
loaded-project Undo/Redo, then New clears again. Zero rejected requests/page errors and unchanged
review SHA256. [Evidence](evidence/mapgyver-m07-workbench-input-2026-11-01.json).
The final screenshot exposes the offscreen-selection Inspector defect above: do not call the
complete redesigned editor accepted or reduce M07 to native-only scope. Checkpoint this result
before its focused correction; no third live invocation in this batch. No acceptance denominator
or native budget changes: M07 stays3/4 and overall4/10=40%.

**M07 offscreen Inspector correction — 20:53 CET, stop21:38 CET:** checkpoint `46f3977e` is
pushed and verified; worktree clean. Same active gate3. Owner: app Matrix typed viewport query,
cached view index and window selection context. Add at most one selected row/column/cell beside
the existing bounded grid reply, using indexed identity lookup; regenerate TypeScript from Rust.
Inspector selection must survive scrolling without whole-document reads or stale local caches.
Fence selected replies through existing session/project/history/read-queue ownership. Proof:
offscreen selected definitions and missing/history recovery at backend boundary, delayed selection
read tests, and real grid scroll→Inspector edit→Undo/Redo/project replacement. Compile/software
success before at most two live runs; preserve all current native/resource blockers.

Prerequisites pass:210Rust tests/1existing ignore (including generated DTO parity and selected
offscreen/history/missing-source bounds),65frontend tests, Svelte0errors/0warnings, clippy with
warnings denied and ordinary app rebuild. Clippy's enlarged enum finding is resolved by boxing
the optional selected snapshot; wire shape stays unchanged. Invocation1 stops at initial Group
selection with no page/rejection errors: JSON-string equality incorrectly treats reordered DTO
members as a different selection. Replace it with typed item-kind/ID/column comparison and add
the exact reordered-reply regression test before rebuilding. Invocation2 is the final live run.

**Offscreen correction result —21:01 CET:** invocation2 passes full Matrix input plus project
replacement, **exit0/27,214ms**. Real grid scrolling removes selected row/column from the DOM,
while Inspector retains their names and assigned source. Offscreen rename and exact Undo/Redo/
Undo work without scrolling; resize and actual New/review upload still pass. One expected cycle
rejection, zero other rejections/page errors. Rust210pass/1existing ignore, frontend67pass,
Svelte0errors/0warnings, clippy and ordinary build pass. Screenshot inspected: selected UI Lower/
UI First fields remain while grid shows Layer5+/Column8+. Optional selected reply is bounded to
one row/column/cell and uses identity indices, the existing coalesced queue and ownership fences.
[Correction evidence](evidence/mapgyver-m07-offscreen-inspector-2026-11-01.json).
No gate closes on browser-only evidence; M07 stays3/4 and overall4/10=40%. Publish before M08's
existing Application input/project fixture migration; no further Matrix host invocation.

**M06 Composition-layout batch — 2026-11-01 20:25 CET, stop 21:10 CET:** previous turn is
progress, pushed as `f023ad10`, clean checkout. Only M06 gate4's remaining redesign input is
active. Owner: app Composition controller/selection/Inspector/canvas and existing browser fixtures.
Preserve batch89's full forms/source/order/copy/remove/Translate/Rotate/Scale/rejection/Escape/
source-loss/history scope and batch95's accepted-drag New, old pointer disposal, fresh history,
actual review upload/stable IDs/post-load history. Creation must adopt the shared Inspector;
removal must retain a usable parent editing path. Reuse Golden selectionRevision fencing and
existing backend replies, with no UI-owned defaults or geometry semantics. Keep mapping errors
out of canvas layout using the existing proven overlay rule. Run software/build prerequisites
before at most two finite host invocations/20 minutes investigation, within21:10. Native imagery/
extent outlines/shape selection remain required and unavailable; no browser-only gate closure.

Prerequisites pass: ordinary rebuilt candidate SHA256 `b2239c57…a93ea9c`, Svelte0errors/0warnings,
63 frontend tests and JS syntax/Prettier. Toolbar channel creation and Inspector instance creation/
copy/removal now adopt the shared selection through pre-operation guarded backend result IDs;
pagination remains owned by the window context across Inspector remounts. Composition shares the
existing canvas error-overlay rule. The existing review project is present and matches its
manifest SHA256 `d4861eb6…122ed8`; do not regenerate it. First invocation combines full input and
project-lifetime fixtures, with their unchanged tolerances and expected rejection checks.

Invocation1 proves toolbar creation selects the channel Inspector, canvas controls resize and
self-reference rejects as expected, with no page errors or other rejection. It stops at the new
geometry-stability assertion. The baseline bounds were read immediately after backend resize,
before the rendered Composition was required to show400x300. The assertion did not capture its
two rectangles, so no runtime geometry cause is inferred. Wait for actual controller readiness
and the SVG's400x300 viewBox before taking the baseline; include both rectangles on any failure.
No tolerance or matrix is reduced. Invocation2 is the final permitted full input/project check.

**M06 redesign input result — 20:35 CET:** invocation2 passes both complete fixtures and the
ordinary source HTTP/history/Save/New/Load baseline, **exit0 /13,689 ms** hosted flow.
Toolbar Composition creation selects the channel Inspector; instance creation/copy selects its
committed identity; removal restores a usable parent channel. Guarded selection respects later
user/project changes, and pagination stays in the window context across Inspector remounts.
Canvas sizing, cycle rejection without history, source selection/drafts, complete instance form,
order/copy/removal, Translate/Rotate/Scale, Escape, invalid zero-scale input, source loss/Undo,
peer preservation and both manager/editor tab lifetime pass. Only the two explicit invalid edits
reject; zero page errors. Error overlay leaves rendered geometry unchanged within0.01 CSS pixels.
Actual New during accepted drag discards later held-pointer samples, fresh canvas history works,
review upload retains saved IDs and post-load instance Undo/Redo, then actual File New clears all
instances/history. That fixture has zero rejected requests/page errors, and review file hash is
unchanged. Screenshots are inspected: separate canvas and Inspector, native Preview unavailable,
source outline count0. [Current evidence](evidence/mapgyver-m06-workbench-input-2026-11-01.json).
Preserve invocation1's premature baseline-bounds failure. **No M06 gate closes:** native imagery/
extent-based outline/shape selection remain required. M06 stays3/4; overall4/10=40%, fixed budgets.
Publish before Matrix migration; do not repeat passing Composition/browser checks as native proof.

**M05 Source-layout batch — 2026-11-01 20:21 CET, stop 21:06 CET:** previous turn is progress,
published as `0f98ef81`, clean checkout. Only M05 gate7's outstanding redesign input is active.
Owner: app Surface Source canvas/shared Surface Inspector/Screen tree and the existing browser
fixture. Preserve batch87's full form/calibration/all-corner/origin/rejection/Escape/source-loss/
Undo/Redo/panel-lifetime scope. Reuse the software-verified `2b16382d…93df43f` ordinary app.
At most two finite invocations/20 minutes investigation within21:06; correct demonstrated defects
only. Native selected-channel/extent/crop-overlay preview remains required and unavailable, so
software success alone cannot close gate7. On completion retain that named blocker/return trigger
before moving to already implemented Composition UI acceptance; no native/helper retry.

**Source input migration result — 20:24 CET:** first/only invocation passes **exit0 /6,237 ms**
hosted flow, Chromium149.0.7827.55. The original batch87 scope is preserved: source/enablement/
opacity/crop/destination fields remain drafts until Apply; one Undo/Redo restores the complete
definition. Grid/ColorBars/Off preserve source/crop/mapping. All four crop corners and origin
movement preserve peers/non-crop fields, origin retains width/height, each gesture undoes/redoes
as one step. Invalid pointer/form input rejects without state/history change; Escape blocks later
held-pointer input. Actual source Delete/Undo shows Missing source without changing the binding,
then restores exact media/catalog IDs. Shared selection, manager/center tab lifetime and actual
full-panel geometry pass. Error show/clear leaves SVG bounds stable within0.01 CSS pixels.
Zero page errors; three expected, explicitly correlated invalid crop rejections and no others.
The screenshot is inspected: separate tree/crop canvas/Inspector, Preview unavailable explicit.
[Current input evidence](evidence/mapgyver-m05-workbench-source-input-2026-11-01.json).
Gate7 remains open for its original native full-source/extent/crop-overlay observation; M05 stays
4/8 and overall4/10. No native, physical, latency, alignment or pixel result is inferred.
Publish this checkpoint, then continue M06 gate4's remaining new-layout input scope while retaining
its native prerequisite. No second unchanged Source comparison or hardware/helper retry is needed.

**M05 stable-canvas closure batch — 2026-11-01 20:18 CET, stop 21:03 CET:** previous turn is
progress, published as `6e1348df` with a clean checkout. Only M05 gate6 is active. Reuse the
software-verified overlay candidate SHA256 `2b16382d…93df43f`; no prerequisite rebuild is needed.
Run the full source UI once as a regression of the changed creation-selection boundary, then the
complete original Screen selection/all-corner/Escape/rejection/history/panel-lifetime matrix.
The explicit SVG bounds checks must prove error show/clear causes no reflow. At most two finite
interaction invocations/20 minutes investigation, within21:03; correct only demonstrated failures.
Do not broaden into native/GPU/physical scope or reduce the original gate/tolerances.

**M05 gate6 accepted — 20:19 CET:** the first/only invocation passes full source UI and the
complete original Screen fixture, **exit0 /12,867 ms** hosted flow, Chromium149.0.7827.55.
All four real corner handles preserve peers/source/other corners and roundtrip exact catalog
state with one Undo/Redo per gesture. Source selection/drafts/Apply, invalid mapping without
state/history changes, Escape blocking later held-pointer input, manager/center panel switching
and full-panel geometry pass. Showing and clearing the error preserves actual SVG bounds within
0.01 CSS pixels; functional target tolerance remains0.005 normalized units. Two explicitly
invalid samples reject as expected; no other rejection and zero page errors occur.
The earlier creation-selection race and normal-flow error displacement failures stay retained.
The inspected screenshot shows separate tree, large edited quad and Inspector, with browser
preview explicitly unavailable. [Current evidence](evidence/mapgyver-m05-workbench-screen-acceptance-2026-11-01.json).
**M05 returns4/8; overall remains4/10=40%**, fixed denominators and unchanged native budgets.
Do not use a second unchanged comparison. Publish before the Source-input migration batch.

**M05 Screen input batch — 2026-11-01 20:07 CET, stop 20:52 CET:** previous turn is progress,
published as `6daa88ba` with a clean checkout. Only M05 gate6 is active. Owner: app Screen tree,
shared Screen/Surface Inspector and full-panel Screen Warping editor; existing browser fixture
owns real-input acceptance. Migrate original batch86 scope without weakening it: name/selection,
source draft/apply, all four corner gestures, peer/source preservation, invalid mapping/history,
Escape, one-gesture Undo/Redo and panel lifetime. Actual controls must be naturally enabled;
no native preview/physical/DPI inference. Reuse software-verified current ordinary app unless a
proven product fix changes it. At most two finite invocations/20 minutes investigation, within
20:52. Publish results before any Source/Composition/Matrix/native batch.

Invocation1 reaches Screen/Surface name/selection and source input with zero page/rejected-intent
errors, but Apply changes the newly created peer instead of the explicitly reselected Surface.
The app's delayed creation selection captures the selected identity only after awaiting creation;
reselecting the same old node does not change that identity. Golden already exposes authoritative
`selectionRevision`, including same-node clicks. Capture it before each asynchronous creation and
guard both reply and graph-delta selection against revision/project/session changes. The same
app-owned selection boundary is used by Screen, Media/folder and Application creation; no domain
mutation semantics or Golden API is duplicated. Add focused revision/lifetime regression checks,
then rebuild before the second/final full Screen comparison. Keep the first failed run.

**Invocation2 result — partial:** the rebuilt app keeps Apply on the explicitly selected Surface.
Source/peer preservation, grouped first-corner drag with exact Undo/Redo and invalid mapping with
no state/history change pass. Only the explicit invalid-geometry stage produces two expected
backend rejections; no page errors occur. The next accepted-drag case stops before Escape because
the actual y coordinate is0.245858 rather than0.2. The previous invalid-mapping banner occupies
normal flex layout above the canvas; clearing it on gesture start moves the centered mapping
coordinate space after pointer-down. Move that existing error into a positioned canvas overlay,
preserving visible rejection without reflow. This fix is software-checked but not a third live pass.
Both comparisons are consumed; keep M05 gate6 at3/8 and retain the same gate for the next turn.
Next comparison must finish Escape/all remaining corners/panel lifetime and assert error show/
clear leaves the actual SVG bounds unchanged. The source-selection regression is demonstrated
fixed; final geometry/gesture acceptance remains open. No native/physical threshold is changed.
Software is terminal: **63 frontend tests pass**, Svelte0errors/0warnings, ordinary bundled build,
JS syntax/Prettier and both Cargo formatters pass. The first check's test-only type-predicate
narrowing errors are fixed before the second invocation. Final overlay candidate is built but
not run, SHA256 `2b16382d…93df43f`. [Partial evidence](evidence/mapgyver-m05-workbench-input-partial-2026-11-01.json).

**M04 alert/source closure batch — 2026-11-01 20:02 CET, stop 20:47 CET:** previous turn is
progress (`3538db85`, clean checkout). Only M04 gate5 is active. App media Inspector owns any
reload error; the browser fixture owns observability/selection, not domain behavior. Reuse the
unchanged, software-verified ordinary binary with SHA256 `650530a7…32f6a1`.
First finite invocation captures the exact unexpected alert after Reload Picture. Fix only its
proven cause; second invocation runs Reload Picture/history plus the original complete source
matrix. Two invocations/20 minutes investigation maximum, within the batch stop. Preserve both
earlier failures; unchanged native/renderer qualifications are excluded. A pass must cover source
creation/settings/import/relink/independent selectors/guards/history/reload, not a reduced matrix.

First invocation captures `alerts:[""]`, zero page errors/rejected requests and an unchanged
catalog after one Reload Picture intent. Installed Dockview `dockview/liveRegionService.js`
lines27–35 creates a persistent `dv-live-region-assertive` with `role=alert`, empty when idle.
The corrected assertion permits only that exact empty announcer; any application alert or
nonempty Dockview announcement still fails. No production change or error suppression applies.
The second comparison retains Reload Picture redo/undo and the complete source matrix.

**M04 gate5 accepted — 20:05 CET:** invocation2 passes both real bundled-browser fixtures and
the ordinary source HTTP/history/Save/New/Load matrix, **exit0 /9,687 ms** hosted flow.
Chromium149.0.7827.55; **zero page errors/rejected requests**. The empty alert is verified as
Dockview's exact idle assertive announcer; every other alert remains a test failure.
Actual controls verify all three source kinds, independent Solid settings/extents/filter,
media/channel names, add/select/reorder/delete and last-channel guard, Picture import/assign/
relink/clear/reselect, independent exact Spout selectors and admitted layout, media deletion with
shared asset/peer retention, atomic top-level Picture import, exact Undo, browser reload, and
Reload Picture with one selected-asset intent preserving catalog and the prior redo/undo entry.
The screenshot is inspected: tree left, source parameters in Inspector, no manager forms.
The source gate's accepted backend/native/caller evidence is retained; none was rerun or expanded.
[Current acceptance evidence](evidence/mapgyver-m04-workbench-source-acceptance-2026-11-01.json)
preserves the two earlier migration failures and this batch's first empty-announcer failure.
**M04 returns to6/6; overall4/10=40%.** No denominator, budget or native requirement changes.
Publish before opening the M05 gate6 migration batch; no further M04 comparison is needed.

**M04 source acceptance batch — 2026-11-01 19:53 CET, stop 20:38 CET:** previous turn is
progress, published as `c0a52035` with a clean checkout. M04 gate5 is the sole active gate.
Owner: app Media Manager/Inspector and ordinary browser acceptance fixtures. Recover the full
original batch50 matrix (source creation/settings/rename/reorder/delete, Picture import/relink,
independent Spout selectors, guards, Undo/reload), including the missing top-level import affordance,
using existing backend operations. Migrate Picture reload input to the shared Inspector.
Run prerequisites first; at most two finite interaction invocations (source matrix, Reload Picture),
20 minutes of investigation and the 20:38 stop. Preserve failures and original native limits.
Close gate5 only on its complete new-layout scope; do not rerun old renderer qualification.

Invocation1 stops at the Sampling selector: the fixture uses exact label text, while wrapped
select labels include their option text. No page error or rejected request; creation, name and
channel-add input reached the actual Inspector. Use the observed combobox name prefixes for
selects. Invocation2 will verify Reload Picture first on the unchanged source baseline, then the
corrected full source matrix in the same isolated host. This keeps two host invocations total;
retain the first failure and stop rather than adding another comparison if it fails.

**Source batch result — partial, two invocations consumed:** invocation2 reaches the actual
Reload Picture Inspector control, sends exactly one selected-asset request and retains the exact
catalog/IDs. It stops at an unexpected global alert count before redo/undo or the corrected source
matrix. The alert content was not captured; its origin is unproven. Add text/body/screenshot and
rejected-request capture to that failure assertion; do not scope it away or infer a product cause.
Both hosts exit0 after their orderly sentinel; both browser instances close. No gate closes.
Top-level Picture import is restored through the plus menu into an Inspector-owned locator form,
using existing ImportPicture defaults/atomic operation. This new affordance compiles but its actual
input remains unverified. Source settings reached the first run; later cases remain unexecuted.
Svelte reports0 diagnostics, 61 frontend tests and the ordinary bundled build pass.
[Partial evidence](evidence/mapgyver-m04-workbench-source-migration-2026-11-01.json).
Checkpoint this useful source/fixture work. Next batch starts with the captured Reload Picture
alert and the corrected source matrix; no native, renderer or other phase work is substituted.

**Retained native constraint:** Native access remains blocked
through batches98–99 and the source/queue audit. On 2026-11-01 the user confirms native interaction
is unavailable for now; manual observations remain unverified. No live app/test remains to wait on. All original
requirements and accepted counts are retained; the goal is unfinished.

**Parked acceptance gate:** **M08 gate4, remaining native/OS/full-app acceptance**. Batch97 passes actual column
association/distance/history/missing-reference controls, shared processes across two Matrices,
pre/post holds, runtime snapshot input, availability recovery and New with a live owned process.
Batch96's last failure was a fixture expectation: unavailable neighbor-only demand is omitted;
direct associated-column demand correctly reports `Unavailable`. No production policy changed.
M06 gate4 returns for native imagery/extent outlines; M07 gate3 for native renderer feedback.
M05 gate7 stays open for native full-source preview. M05 is **4/8** after reaccepting gate6. Native/physical/OS/DPI,
M08 native dialog/retirement/full-app and remaining actual-editor scope stay open. Batch98 verifies
a real elevated owned target under the already elevated caller, with no elevation request.
Native access remains blocked: batch99's supported explicit-path launch resolves to installed
MapGyver1.0.0b7 instead of this checkout; both supported close attempts also fail activation.
The newly launched installed PID is cleaned up and absent. No current-build UI result is inferred.
Do not repeat helper retries, alternate launch routes, parity fixtures or review preparation.
M03 remains **7/9**; its original performance/resource/recovery requirements are retained below.
**Golden workbench redesign delivered — 2026-11-01:** all five requested design areas are
implemented: Golden manager trees, persisted Media folders, shared Inspector properties,
full-panel Screen/Source/Composition canvases, Matrix grid/icon actions and Applications tree.
The default layout prioritizes editors. Type icons distinguish folders/media/screens/surfaces.
Name/reorder/delete/source controls retain typed backend operations. Folder rename/move/removal
reject invalid ownership/cycles and preserve Undo identities.

**Verification:** **209 Rust tests pass /1 existing ignore; 61 frontend tests pass; both apps'
Svelte checks report zero errors/warnings; strict library Clippy, ordinary bundled build,
UI Prettier and both Rust formatters pass.** The final bundled-browser flow passes in **6,850 ms**,
Chromium149.0.7827.55, **zero page errors/rejected requests**. Actual controls verify nested
folder creation/name/moves, Media/channel rename/reorder, Screen rename/Surface rename/reorder/
removal, shared Inspector, full-panel canvas/grid bounds, Composition instance selection/handles,
Matrix assignment without launch, Applications and File New. Source HTTP/history/Save/New/Load
also passes in the same host. Browser preview remains explicitly unavailable.

[Evidence and all failed runs](evidence/mapgyver-ui-workbench-redesign-2026-11-01.json).
Ten cumulative finite comparisons: five failures, five passes. The first four failures are
fixture targeting/readiness defects. Invocation7 finds a real enabled Create Screen control whose
click is ignored during controller refresh; explicit disabled-state support in Golden's public
manager/add control fixes it. Later passes verify each concrete correction; failures stay retained.

The first batch ran **18:34–19:19 CET**, pushed as `c6f38586` before integration.
Integration started **19:21 CET**, original stop **20:06 CET**; the final app comparison completed
**19:50 CET**, within that limit. Additional comparisons verified concrete layout/preview fixes,
preserved-action/folder parity and readiness failures. No native retry/settings/budget change.

**Remaining redesign acceptance:** Media, Screen, Surface Source and complete Composition input/
project-lifetime matrices pass in the new layout. Matrix/Application fixtures still need migration
alongside native/full-app scope. Combined smokes alone closed no original gate; complete source/
Screen matrices supplied their acceptance. Overall is **4/10 (40%)**, fixed denominators; Source
and Composition input did not close their unverified native-integration gates.

**Retained native next action:** retain M08 gate4 and obtain native executable-picker and actual desktop lifecycle
observations against **this checkout's** executable when native interaction becomes available.
The user confirms it is unavailable for now. The supported launcher resolved to the wrong installed app, and activation remains
unavailable; the elevated-context hypothesis is not proved. The singleton retirement limit and complete
app/mode/recovery/cleanup remain required. Non-elevated denial was not exercised in this elevated
session. Do not repeat passing browser/caller/column/elevated-process preparation or helper retries.
M06/M07 return for real native image/outline/renderer observations; no injected status or parked
automation retry. Composition project lifetime and Matrix stale inspections now pass in scope.
**Blockers:** supported MapGyver window activation fails; actual M05 editor/fullscreen observations
remain missing. M03 GPU/window scope decisions are pending and unapplied; private checkpoint growth
**36.102 MiB >32 MiB** still fails with no established cause/correction. Genuine removal needs the
unapproved reset window. M05 physical 4K/other unavailable configurations remain unverified.

| Phase | State | Progress | Done / total items |
| --- | --- | --- | --- |
| [M00 Baseline/workloads](mapgyver-m00-baseline.md) | Complete locally | `████████████████████` 100% | 1/1 |
| [M01 Functional route decision](#m01-acceptance-items) | Complete locally | `████████████████████` 100% | 4/4 |
| [M02 Multi-app seams/shell](#m02-acceptance-items) | Complete locally | `████████████████████` 100% | 7/7 |
| [M03 Rendering/channels](#m03-acceptance-items) | Foundations ready; qualification open | `████████████████░░░░` 78% | 7/9 |
| [M04 Source media](#m04-acceptance-items) | Complete: original source UI matrix passes in Golden tree/Inspector | `████████████████████` 100% | 6/6 |
| [M05 Screens/Surfaces/editors](#m05-acceptance-items) | Full new-layout Screen/Source input passes; native/physical gates retained | `██████████░░░░░░░░░░` 50% | 4/8 |
| [M06 Composition/editor](#m06-acceptance-items) | Complete new-layout input/project lifetime verified; native image/extent/shape selection open | `███████████████░░░░░` 75% | 3/4 |
| [M07 Matrix/editor](#m07-acceptance-items) | New-layout input/project and offscreen Inspector editing verified; native feedback open | `███████████████░░░░░` 75% | 3/4 |
| [M08 App lifetimes](#m08-acceptance-items) | New-layout Application/column/process/New input verified; native/retirement/full-app gaps retained | `███████████████░░░░░` 75% | 3/4 |
| [M09 App/artifact qualification](../plan/MapGyver_Implementation_Plan.md) | Not started | `░░░░░░░░░░░░░░░░░░░░` 0% | 0/1 |

Overall counts fully accepted milestones: `4 / 10 = 40%`; partial phases do not advance it.
The 2026-11-01 user design correction reopened M04 gate5 (6/6 →5/6) and M05 gate6 (4/8 →3/8).
The complete source matrix reaccepts M04 gate5 (5/6 →6/6); complete Screen input reaccepts M05
gate6 (3/8 →4/8). Unverified native and Source-preview requirements remain open.
Denominators stay fixed; prior backend/native evidence is retained and no unverified layout is accepted.
Phase percentages count closed acceptance items, rounded to the nearest whole percent
(`M01: 4 / 4 = 100%`, `M02: 7 / 7 = 100%`, `M03: 7 / 9 = 78%`, `M04: 6 / 6 = 100%`, `M05: 4 / 8 = 50%`, `M06: 3 / 4 = 75%`, `M07: 3 / 4 = 75%`, `M08: 3 / 4 = 75%`). Bars use 20 cells, rounded from the exact fraction. Each phase uses
its whole plan acceptance as one item unless a fixed breakdown is recorded here before
implementation continues. On 2026-10-28 the user explicitly approved changing M01 from eight
full-qualification items to four functional-route items, allowing M02 to start. The original
eight-item matrix remains in the [native evidence](mapgyver-m01-native-feasibility.md#remaining-gate)
with continuing owners in M03/M05/M09; its failures are not converted to passes. M02's seven
fixed items below were defined before source implementation and cover its complete acceptance.
M03's provisional whole-phase item was split into nine fixed gates on 2026-10-28 before source
work; the combined scope and native budgets are unchanged, and all nine started open.
These counts measure accepted scope, not effort or product readiness. Partial evidence earns
no item closure; future breakdowns must preserve the complete acceptance scope and explain
denominator changes. See [milestones](#milestones) and [next action](#next-action) for detail.

**Completion policy:** one active gate, an explicit return trigger before leaving blocked work,
45-minute delivery batches and at most 20 minutes/two investigation experiments. A time box never
waives acceptance. Every unfinished M03/M05/M06/M07/M08/M09 item returns. The user requires 100%
of the agreed plan; a day-scale minor optimization may justify a precise proposal, never a silent
lower gate. Track cumulative effort across batches. Correctness/ownership remain required.
See [execution ordering](../plan/MapGyver_Implementation_Plan.md#completion-priority--2026-10-31).

**Execution board:** open phases are retained acceptance, not simultaneous implementation.

| State | Gate(s) | Next requirement / return condition |
| --- | --- | --- |
| Accepted | M04/5 | Complete source tree/Inspector matrix and Picture reload/history pass; M04 returns6/6. Earlier failures are preserved. |
| Accepted | M05/6 | Full Screen tree/Inspector/canvas input, all corners/Escape/rejection/history and stable mapping bounds pass. M05 returns4/8. |
| Blocked remaining scope | M05/7 | Complete new-layout Source input passes. Return only for actual native selected-channel/extent/crop-overlay preview when supported current-build native control or manual input is available. |
| Blocked remaining scope | M06/4 | Complete new-layout Composition input/history/project lifetime passes; return for actual native imagery/extent-based outlines and shape selection on supported native control/manual observation. |
| Blocked remaining scope | M07/3 | Full new-layout input/project and offscreen Inspector editing pass. Return for actual native preparation/installation/submission feedback on supported current-build native control or manual observation. |
| Blocked remaining scope | M08/4 | Full redesigned Application/column/process/New input passes. Return for native executable-picker/desktop lifecycle on supported current-build interaction or manual observations, plus unresolved singleton retirement/full-app evidence. User says native input unavailable; do not repeat that question or retry alternate launch/control helpers. |
| Blocked portions | M03/4, M03/9 | Explicit reset decision; supported timing evidence or an approved scope decision; concrete allocation-supported correction for the preserved memory failure. No pending proposal is applied. |
| Pending; physical/native checks blocked | M05/4,5,7,8 | Native editor/fullscreen/Source preview observations and the complete physical display/hotplug/DPI/alignment/latency matrix. Return on supported observation capability or required hardware access. |
| Pending; native observation blocked | M06/4, M07/3 | Actual native image/extent/selection integration and Matrix preparation/installation/submission feedback. Passing browser input/history is not rerun as a substitute. |
| Waiting | M09/1 | All retained M03–M08 acceptance, then complete-app/artifact qualification and shared Chataigne regression checks. |

Only a passed gate or a named missing prerequisite permits leaving the active gate. A batch cutoff
checkpoints work; it does not waive acceptance or automatically select another phase. The target
is **100% of agreed scope**. A roughly day-long minor optimization can prompt a specific proposal
with measured cost and consequences, requiring explicit agreement before any lower gate applies.

**Closure return queue:**
- **M03 gate 9:** prescribed Q01/Q02 on stabilized inputs, complete lifecycle/resource-return
  evidence and the preserved private-growth failure. Timing returns on a validated supported
  clock mapping with bounded error or an explicit [GPU-work decision](#gpu-acceptance-proposal--awaiting-user-decision).
  The [window-only deadline proposal](#window-deadline-proposal--awaiting-user-decision) also needs
  explicit approval. Neither is operative. Memory investigation returns only for a concrete
  ownership/allocation-supported correction; do not repeat unchanged long runs or parameter sweeps.
  Native owner inventories are verified only in a short control; driver physical size/transients
  remain unknown and all original budgets/failures stay recorded.
- **M03 gate 4:** genuine removal/stuck-drain recovery; return on the explicit reset decision and
  the required owned-window/process preparation. No reset is authorized by ordinary continuation.
- **M05 gate 4:** batch 72's fresh ordinary app and single supported fresh-selection retry both
  report `failed to activate captured window`. Return when supported activation is available or
  actual manual observations cover the missing editor/fullscreen states. Do not repeat helper
  workarounds. Independent Spout pixels pass; no actual editor/minimize/fullscreen behavior is inferred.
- **M05 gate7:** batch87 verifies actual Source form/calibration/crop/origin/history and missing
  source input. Return on native full-source preview observation (selected channel/extent and crop
  overlay), via concrete restored supported capability or manual evidence. Do not repeat input
  preparation or count the explicitly unavailable browser preview as a native pass.
- **M06 gate 4:** caller parity and the ordinary-app review scene are verified. Return on the
  requested six real editor/source-picker, gesture/history, form, ordering, cancellation/panel and
  missing-source observations, or changed supported activation availability. Do not interpret a
  pending response as pass/fail/unavailable or repeat completed fixture work while waiting.
  Batch89 completes real browser form/source/order/copy/remove/geometry/Escape/rejection/source-loss/
  panel/history and dock layout. Batch95 verifies accepted-drag New, old pointer disposal, fresh
  identity/history, actual review upload and post-load instance history. Return for actual native
  imagery/extent-based outlines and original unverified image/shape-selection integration, not a
  repeated browser preparation run. Earlier failures remain recorded.
  Batch 84's isolated documented WebView2 CDP attempt is unavailable: native app and frontend start,
  but no endpoint appears in20 seconds. Return only on actual observations or a concrete changed
  supported capability; no alternate flags/helper/registry/security workaround.
- **M07 gate 3:** 124 caller cases and ordinary-headless history/reload pass; the Matrix review
  scene and six-group checklist are ready. Return on actual grid/settings/selection/history/source
  observations or changed supported activation availability. No real input is inferred.
  Batch91 verifies actual browser source/settings/structure/history/selection/requested playback,
  source loss, keyboard virtualization, resize and panel lifetime. Batch94 identifies hidden Media
  Manager child queries using an obsolete catalog; owner fencing fixes the strict New/upload check
  with zero rejected requests/errors. All prior failures remain recorded. Return for actual native
  preparation/installation/submission feedback, not passing lifecycle/input preparation or injection.
- **M08 gate 4:** batch 81 verifies 144 caller paths, frontend logic and ordinary-headless lifecycle.
  Batch 83 also passes the corrected aggregate target-outcome check, with singleton retirement
  explicitly unverified. Return for actual form/dialog observations, supported evidence resolving
  the retirement gap and complete mode/recovery/cleanup evidence. Batch98 verifies an actual
  elevated owned target with token report/Ready/Idle/cleanup under the already elevated caller;
  no elevation is requested, and non-elevated denial is unexercised. Historical project replacement/shutdown proof remains scoped;
  callback/history parity does not certify unobserved retirement. Do not repeat completed parity.
  Batch85 also verifies actual browser forms/catalog/history, including all definition fields,
  environment modes and atomic rejection. Native executable-dialog input remains unverified.
  Batch96 verifies original association/distance/history/missing-reference and shared-process input
  across two Matrix scopes. Its final blocked-relaunch observation fails with no runtime entry;
  batch97 identifies the neighbor-only fixture expectation and passes direct-demand Unavailable,
  re-enable recovery and actual project New with a live process. Both earlier failures remain.
  Keep native dialog/OS/retirement/full-app gaps required; do not repeat definition/caller prep.
- **Remaining M05 → M06 → M07 → M08 → M09:** complete native/physical/DPI/input, composition/Matrix
  pixel/interaction/scale, app lifetime/parity and final app/artifact acceptance. Functional or
  checkpointed work is unfinished until its full gate passes. M09 requires all retained acceptance.

**Native-access result, batch99:** one supported explicit-path launch resolves to installed
`C:\Program Files\MapGyver\MapGyver.exe` / **1.0.0b7**, not the checkout binary. Both normal-close
inputs fail `failed to activate captured window`, including fresh selection. Guarded cleanup of
the newly launched PID47000 completes; later CIM/Get-Process confirms absence. No current-build
native input/preview/dialog acceptance is earned. Stop this route; return on supported control of
the exact checkout build or guided manual observations. [Evidence](evidence/mapgyver-m08-native-access-2026-11-01.json).

**Latest usable checkpoint, batch98:** actual elevation-required target passes the first/only app run
**exit0 /2.86 s**. The embedded/read-back manifest requires administrator; current caller and
target are observed elevated. Normal owned launch reaches Ready, token-report PID matches
**17672**, unload reaches Idle with PID absent and external **35760** retained, then host exits0.
All five reported PIDs are absent after fixture cleanup. No privilege adjustment/UAC is used.
The initial non-elevated guard refusal is retained; denial is not claimed. **M08 3/4; overall 4/10**
for native dialog/retirement/full-app gaps. [Evidence](evidence/mapgyver-m08-elevated-target-2026-11-01.json).

**Retained checkpoint, batch97:** the corrected comparison passes **exit0 /10,810 ms**, Chromium
**149.0.7827.55**, zero rejected requests/page errors. Disabled neighbor demand has no entry;
direct demand reports `Unavailable`, enabling starts a fresh launch/PID, and actual File New
clears definitions/runtime while retiring the live process. All three observed PIDs are absent,
and the host exits normally. **M08 remains 3/4; overall 4/10 (40%)** for native/OS/full-app gaps.
[Column UI/recovery/New evidence](evidence/mapgyver-m08-column-ui-2026-11-01.json).

**Retained checkpoint, batch96:** two app invocations, both fail. A regression-proven pending-removal
inspection race is fixed; **35 frontend tests** and the rebuilt app pass. The corrected actual UI
run reaches shared scopes/holds/stops and disabled in-use retention, then times out waiting for
`Unavailable` after relaunch; runtime reports no applications, with zero rejected UI requests.
Both hosts exit normally; all four reported owned PIDs are absent. Recovery and New are not claimed.
**M08 stays 3/4; overall 4/10 (40%).** No third run or phase switch follows from this checkpoint.
[Column integration evidence](evidence/mapgyver-m08-column-ui-partial-2026-11-01.json).

**Retained checkpoint, batch95:** Composition project lifetime passes its first/only invocation,
**exit0 /7,150 ms**, Chromium **149.0.7827.55**, zero rejected requests and page errors. Actual New
during accepted held-pointer motion discards the old scene safely; fresh canvas history, saved
instance IDs on browser upload, post-load editing/history and final menu New pass. The original
review file remains unchanged. Native image/extent outlines stay open: **M06 3/4**, overall
**4/10 (40%)**. [Composition lifetime evidence](evidence/mapgyver-m06-project-lifetime-2026-11-01.json).

**Retained checkpoint, batch94:** the trace identifies four obsolete Matrix InspectPlayback/Inspect
requests from Media Manager's hidden channel editor. Its catalog now belongs to one session/epoch;
children unpublish immediately during replacement, and stale reads/mutation replies cannot adopt
new selection. The strengthened strict New/upload check passes **exit0 /7,875 ms**, zero rejected
requests, missing-media messages and page errors. Svelte/build/formatting pass. **Correctness defect
resolved; no full gate closes:** M07 **3/4**, overall **4/10 (40%)**. Native feedback remains.
[Exact request and clean lifecycle proof](evidence/mapgyver-m07-project-lifetime-fixed-2026-11-01.json).

**Retained checkpoint, batch93:** a late grid refresh can adopt a new generation while retaining
old media; a new regression proves this before the fix. The controller now owns a reactive epoch,
unpublishes old viewport/child controls immediately and blocks old reads/edits. **34 frontend tests**,
final focused suite, Svelte, build and formatting pass. First app run catches an initially nonreactive
guard (fixed); the second still fails the original missing-media check. **No gate closes:** M07
**3/4**, overall **4/10 (40%)**. Stop controller hypotheses; next identify the exact rejected intent
through the existing fixture's prepared read-only observer.
[Viewport correction and retained failure](evidence/mapgyver-m07-viewport-owner-2026-11-01.json).

**Retained checkpoint, batch92:** actual File→New/browser upload verifies state/history/identity
replacement, but capture audit finds missing-media errors. A regression reproduces expired queued
target dispatch (red), now fixed for all seven app read-queue users; **33 frontend tests**, Svelte,
build and formatting pass. The second/final app run **exits1** on the strengthened stale-inspection
assertion; the symptom persists. Both owned hosts exit0 normally. No clean lifecycle/native pass,
third app run or gate closure: **M07 3/4, overall4/10 (40%)**.
[Lifecycle failure and bounded correction](evidence/mapgyver-m07-project-lifetime-2026-11-01.json).

**Retained checkpoint, batch91:** full actual Matrix browser candidate passes **exit0**, hosted flow
**21,401 ms**, Chromium **149.0.7827.55**, zero page errors. Layer/group settings for all four
blends, source/cycle rejection, structure/history, distinct selection/launch, truthful unavailable
renderer, missing source/Undo, tab lifetime and keyboard virtualization/resize pass. **24×16**
authored grid renders **54** cells after scroll; narrow grid is **317×192 CSS pixels**, not a
physical-output measurement. First run exposes an incorrect media-name expectation; actual view
contract displays channel name. Test corrected; no production behavior change. Project replacement
and actual native feedback remain open. **No gate closes:** M07 **3/4**, overall **4/10 (40%)**.
[Matrix browser evidence](evidence/mapgyver-m07-browser-input-2026-11-01.json).

**Retained checkpoint, batch90:** Matrix stacks grid/inspector in a narrow dock instead of squeezing
the grid below its sticky-label width. Build/Svelte (zero errors/warnings), syntax/Prettier and both
workspace formatters pass. Two actual browser runs exit1 at duplicate-label/render-lag fixture
selectors; both owned hosts exit0 normally. Corrected displayed-label waits are not run; later
candidate branches prove nothing yet. **No gate closes:** M07 **3/4**, overall **4/10 (40%)**.
[Matrix partial evidence](evidence/mapgyver-m07-browser-input-partial-2026-11-01.json).

**Retained checkpoint, batch89:** real Composition browser input passes **exit0**, hosted flow
**7,332 ms**, Chromium **149.0.7827.55**, zero page errors. First correction run reaches Scale but
fails; the second/final run waits for the selected form to render committed history values and
passes with the same tolerance. The first failure's precise mechanism was not captured. Source media/peers remain
unchanged and every gesture restores in one Undo/Redo. Actual docked layout is inspected/correct.
Native preview is explicitly unavailable, source outline count0, project replacement/other original
editor gaps stay open. **No gate closes:** M06 **3/4**, overall **4/10 (40%)**.
[Composition browser evidence](evidence/mapgyver-m06-browser-input-2026-11-01.json).

**Retained checkpoint, batch88:** two Composition checks exit1 at the form stage; both owned hosts
exit0 normally. Exact f32 expectations and a narrow-panel canvas layout defect are corrected,
without a third run. Svelte (zero errors/warnings), JS syntax/Prettier, rebuilt ordinary app and
both workspace formatters pass. Full-flow/visual acceptance remains open. **No gate closes:**
M06 **3/4**, overall **4/10 (40%)**.
[Composition checkpoint](evidence/mapgyver-m06-browser-input-partial-2026-11-01.json).

**Retained checkpoint, batch87:** actual Source UI passes on its first invocation, **exit0**, hosted
flow **7,069 ms**, Chromium **149.0.7827.55**, zero page errors. Complete form Apply/history, all
crop handles/origin, invalid samples, Escape, calibration choices, source loss/Undo and shared
selection/panel lifetime pass with read-only backend correlation. Screenshot explicitly shows
native preview unavailable. **No gate closes:** M05 **4/8**, overall **4/10 (40%)**. Gate7 returns
for actual native full-source preview; other original gates/budgets remain unchanged.
[Surface Source evidence](evidence/mapgyver-m05-surface-source-input-2026-11-01.json).

**Retained checkpoint, batch86:** actual Screen UI passes **exit0**, hosted flow **5,505 ms**,
Chromium **149.0.7827.55**, zero page errors. Selection/source drafts do not author; Apply binds
the exact shared channel. All four pointer handles preserve other corners/source/peer, one Undo/
Redo restores each complete gesture; invalid mapping, Escape and panel switching remain coherent.
**M05 gate6 closes: 4/8 (50%)**, overall **4/10 (40%)**. Inspected screenshot explicitly shows
native preview unavailable; physical/OS/DPI/latency requirements remain open. No production code
or budget changed. [Screen editor evidence](evidence/mapgyver-m05-screen-editor-input-2026-11-01.json).

**Retained checkpoint, batch85:** actual bundled Applications UI passes **exit0 /5.31 s**, Chromium
**149.0.7827.55**, zero page errors. Creation/rename, all definition fields, environment modes,
Save/Undo/Redo, copy, availability, removal and visible atomic rejection are backend-correlated;
no process launches. First attempt exposes an exact wrapped-select locator mismatch; test-only
matching correction passes on the second/final invocation. Host returns normally. Native dialog/
retirement/elevation/full-app acceptance stays open: **M08 3/4**, overall **4/10 (40%)**.
[Browser form evidence](evidence/mapgyver-m08-browser-forms-2026-11-01.json).

**Retained checkpoint, batch 84:** one actual native app starts with private profiles and loopback
ports, but its requested CDP endpoint is absent after20 seconds. The test exits **1**, native host
shuts down normally, and no MapGyver remains. No Composition action/connection/input pass occurs;
the opt-in candidate fixture preserves the unavailable route. JS syntax/Prettier pass. **No gate
closes:** M06/M07/M08 remain **3/4**, overall **4/10 (40%)**. Stop automation-route work; actual
editor observations remain pending. [CDP evidence](evidence/mapgyver-m06-webview-cdp-unavailable-2026-11-01.json).

**Retained checkpoint, batch 83:** aggregate target outcomes pass **exit 0 /5.28 s**: singleton
non-adoption/external survival (retirement still unverified), launcher root/known child retirement,
breakaway denial **error5**, missing image **2**, invalid image **216**, atomic `.cmd` rejection,
availability/overload/callbacks and normal host shutdown. All eight reported PIDs are absent after
cleanup. **41 application library tests** and current app/fixture builds pass. No source fix or
second process run; **M08 stays 3/4**, overall **4/10 (40%)**.
[Target-outcome evidence](evidence/mapgyver-m08-target-outcomes-2026-11-01.json).

**Retained checkpoint, batch 82:** the single native invocation passes **exit 0 /135.05 s** on Intel
and NVIDIA. **112 full-image raster captures /86,016 texels**, three interrupted Layer/Group/Final
mixtures per adapter, final blue outputs/preview and outgoing-demand retirement pass. Both sparse
1,000×1,000 grids admit eight of 1,000 scheduled requests with bounded runtime and balanced native
owners. **M07 gate 4 closes: 3/4 (75%)**; actual editor gate 3 remains pending, overall **4/10 (40%)**.
Global handle/private-growth, GPU/cadence and complete Q01–Q05 qualification remain with their
original M03/M05/M09 gates; no such budget pass is inferred. All five earlier failures remain.
[Native Matrix acceptance](evidence/mapgyver-m07-native-matrix-2026-11-01.json).

**Retained checkpoint, batch 81:** **144 application caller paths**, **20 frontend tests**, **206 Rust
tests /one existing hardware ignore**, strict Clippy, current ordinary app/fixture build and normal
headless lifecycle **exit 0 /2.33 s** pass. Persistent application state now participates in the
committed history observer. One first-run burst-recovery assertion failed; it now reports the
rejected Result, but its cause remains unproven. The full suite passes on rerun. Actual form/target/
full-app requirements remain open. **No gate closes:** M08 **3/4**, overall **4/10 (40%)**.
[Application caller parity](evidence/mapgyver-m08-caller-parity-2026-11-01.json).

**Retained checkpoint, batch 80:** the single invocation exits **101 /129.02 s**. Intel completes;
NVIDIA records three valid interrupted mixtures across Layer/Group/Final but the next unconditional
timeout assertion rejects `samples=3`. It now expires only pending observations. Four oracle tests
per executable and strict Clippy pass; no second native run. NVIDIA's endpoint/outgoing-demand
checks remain unverified. **No gate closes:** M07 **2/4**, overall **4/10 (40%)**. Stop native work
and select existing M08 gate-4 caller/edit/control/observation acceptance.
[Completed-observation timeout evidence](evidence/mapgyver-m07-completed-observation-timeout-2026-11-01.json).

**Retained checkpoint, batch 79:** the single native invocation exits **101** after **131.92 s**.
Intel completes **14 states /56 raster captures** plus three interrupted mixture samples and final
retirement; NVIDIA passes **13 states /52 captures**, sparse-grid burst and reload but misses the
six-second intermediate-fade observation window. All **108 captures /82,944 texels** have zero
residual outside legal UNORM intervals (nominal maximum **3/255**, not a nominal <=2 claim).
Concurrent owning-worker sampling compiles, four oracle tests per executable and strict Clippy
pass; **no second native invocation**. Original fade duration, sample count, tolerances and renderer
budgets remain. **No gate closes:** M07 **2/4**, overall **4/10 (40%)**.
[Fade-observation evidence](evidence/mapgyver-m07-native-fade-observation-2026-11-01.json).

**Retained checkpoint, batch 78:** Matrix parity verifies **124 caller cases**, **12 frontend tests**,
**202 Rust tests /one existing hardware ignore**, strict Clippy and normal-headless history/reload.
The initially unlaunched ordinary-app review scene and six-group checklist are ready. No native
experiment or actual input occurred. **No gate closes:** M07 **2/4**, overall **4/10 (40%)**.
[Matrix editor parity evidence](evidence/mapgyver-m07-editor-parity-2026-11-01.json).

**Retained checkpoint, batch 77:** one verification fails before raster samples because the new
adapter assertion treats logical resource admission as completed native initialization. Identity
validation now follows a matching actual native output frame. Four oracle tests per executable,
strict Clippy and both formatters pass; the corrected candidate is not native verified. Frozen
before-run hashes and read-only overlay-Off observations are retained. **No further native run**;
the chain stops at **three cumulative failures**. **No gate closes:** M07 **2/4 =50%**, overall
**4/10 =40%**. [Startup-observer evidence](evidence/mapgyver-m07-startup-observer-2026-11-01.json).
The earlier [52 captures /large-grid partial](evidence/mapgyver-m07-native-partial-2026-11-01.json)
remains evidence, not full acceptance. Matrix editor/caller parity is selected before more native work.
M06's [real editor observations](evidence/mapgyver-m06-editor-parity-2026-11-01.json) remain pending;
its [accepted raster evidence](evidence/mapgyver-m06-composition-pixels-2026-11-01.json) is historical,
not a new hardware verification of the extracted shared test fixture.
M05's [supported activation failure](evidence/mapgyver-m05-activation-2026-11-01.json) remains open.
The prior [native-owner checkpoint](evidence/mapgyver-m03-native-allocations-2026-11-01.json)
and [full private-growth failure](evidence/mapgyver-m03-embedded-shaders-lifecycle-2026-11-01.json)
remain unchanged; neither certifies driver residency or repairs the failed growth budget.

**Last accepted product capability:** Matrix Layer/Group/Final and production preview rasters,
ordering/blends/alpha, source loss/Undo, interrupted fades with outgoing retirement, sparse-grid
burst recovery and reload/fresh-device execution work on both adapters. Actual editor gestures,
physical presentation and global production budgets remain unverified.

## Detailed acceptance evidence

### M08 acceptance items

**User availability update — 2026-11-01:** native interaction is unavailable for now. The prior
guided-check question is answered; this supplies no native observation or acceptance evidence.
Keep native picker/desktop and all other unverified gates open. Return when actual native access
changes; do not repeat the same manual-check request or substitute passing browser tests.

**Blocked audit — 2026-11-01 07:14 UTC:** previous turn supplied new supported-launch failure
evidence; this source-only review establishes no production correction or acceptance progress.
Golden IO requires signalled root/captured handles and zero unobserved lifetimes; the saved
singleton packet reports RetirementUnverified without native accounting details sufficient to
identify a cause. No source policy change, app rerun, console/window variant or new probe occurs.
The native-access prerequisite remains unsatisfied across the previous two goal turns and this
audit; manual availability is unanswered and CIM confirms no remaining MapGyver process.
The full return queue retains native/physical M05–M07 observations, M08 dialog/retirement/full-app
scope, and M03 pending reset/timing decisions and preserved memory failure; M09 depends on those
gates. Resume on the named prerequisite/decision or concrete supported correction, not an unchanged
automatic continuation. Nothing is waived or marked complete. Standing milestone delivery remains
authorized when implementation resumes.

**Native launch-context check batch99:** starts 2026-11-01 **07:05 UTC**, stop **07:50 UTC**;
investigation ends **07:25 UTC**. Keep M08 gate4 active. Previous turn is progress: `c87b6394`
verifies a real elevated owned target. New concrete evidence is the development caller's elevated
token, consistent with the earlier integrity warning. The prior activation attempts used an app
launched by the command host. Test only the computer-use skill's supported `sky.launch_app` with
the exact current MapGyver executable; no Run dialog, shell/COM launch helper, token adjustment,
security setting, new binary or alternate automation API. Select the one returned app window,
then perform supported activation (one fresh-selection retry if needed). If native input works,
verify the existing executable-picker requirement in the owned app; inspect current scene before
authoring. If activation still fails, stop the input chain and preserve the exact outcome; no
further unchanged relaunch. The elevated-context hypothesis is not a proven cause in advance.

**Batch99 result:** no current-build native acceptance. The one supported explicit-path launch
instead starts installed MapGyver1.0.0b7, PID47000/parent30292, window117118890 titled
`MapGyver 1.0.0b7 - New unsaved session`. This is a different product binary; no authoring is
performed and none of its UI is counted. Supported normal-close input fails activation twice,
with fresh window selection before the single retry. The elevated-context hypothesis therefore
does not establish a working native route. Exact PID/path/parent-guarded process cleanup is
requested; after a transient immediate process observation, authoritative CIM/Get-Process shows
the PID absent. No settings, token, Run dialog, shell/COM helper, alternate API or relaunch is used.
Native access remains blocked pending supported control of the exact checkout build or actual
manual observations; the guided-check availability question remains unanswered. Stop automation
route work. M08 stays3/4, overall4/10; all native/retirement/full-app requirements remain.
[Small access result](evidence/mapgyver-m08-native-access-2026-11-01.json).

**Elevation-required target batch98:** starts 2026-11-01 **06:54 UTC**, stop **07:39 UTC**;
investigation ends **07:14 UTC**, max two finite app invocations including baseline. Only M08
gate4 is active. The previous turn is progress: `194d888a` verifies actual column/recovery/New UI.
Use the existing owned fixture with an SDK-embedded/read-back `requireAdministrator` manifest.
From a verified non-elevated caller, the ordinary public launch must report native error740,
no PID/readiness/retry or target-main report, preserve authored state, and unload cleanly.
Microsoft documents CreateProcess rejection for elevation-required targets; no shell/UAC/elevation
fallback, privilege change or user software is involved. This qualifies denial, not a running
elevated process, native dialog, complete singleton retirement or full-app qualification.
Finish fixture preparation, syntax/format and unchanged binary prerequisites before the app run.

**Preflight result, 06:56 UTC:** the preparer's non-elevated guard correctly stops before copying
or launching: this command host already has an elevated token. No app invocation was spent.
Use that observed existing context to verify the actual elevated target: fixture reports its own
token via `GetTokenInformation`, public launch reaches Ready, and unload retires the owned PID
while preserving the external fixture. No privilege adjustment/UAC is introduced. Keep the denied
non-elevated route as a separate unverified branch, not a fabricated error740 pass.

**Batch98 result:** first/only app invocation passes **exit0 /2.86 s** after fixture build,
strict fixture Clippy, embedded/read-back manifest and JS checks succeed. The actual target reports
`TokenIsElevated=true`, PID **17672**, matching public Ready status/launch; authored definition is
unchanged. ClearPlayback reaches Idle and known PID absence, preserving external **35760**.
Normal host shutdown exits0; host **6268** and fixture PIDs **35760/20428/49420/17672** are absent
after cleanup. Running-elevated-target behavior is verified only under the existing elevated
caller; the distinct non-elevated error740 branch was not exercised. No privilege adjustment,
UAC/shell fallback or user application is involved. Native dialog, singleton retirement and full
app/mode/recovery/cleanup stay open: M08 remains3/4, overall4/10. Native observations return on
restored supported input or guided manual observations; availability question pending. No helper
retry or passing preparation repeat. [Evidence](evidence/mapgyver-m08-elevated-target-2026-11-01.json).

**Blocked-relaunch correction batch97:** starts 2026-11-01 **06:50 UTC**, stop **07:35 UTC**.
The previous turn is progress: `e6abf7b8` fixes a regression-proven removal race and preserves two
failed app runs. Keep the original investigation cutoff **06:55 UTC**; no time-box reset.
Source evidence identifies the remaining mismatch: `runtime/capture.rs` deliberately omits
unavailable neighbor-only demand, while the new fixture launches Column1, a neighbor of the
associated Column2. The existing headless lifecycle check correctly selects the associated column.
Retain the disabled-neighbor/no-entry assertion, then use actual Column2 input for `Unavailable`,
enablement recovery and project New with a live process. This concrete fixture correction permits
one corrected comparison; no production policy change, new automation route or native claim.

**Batch97 result:** the single corrected comparison passes **exit0**, complete hosted flow
**10,810 ms**, zero rejected UI requests/page errors. Existing `runtime/capture.rs` omits
unavailable neighbor-only demand; preserving that assertion and selecting associated Column2
proves direct-demand `Unavailable`. Actual Allow new launches admits a fresh process/launch,
authored snapshots remain unchanged, and actual File New clears definitions/runtime and retires
the active process. Observed PIDs **33820/15204/46196** are absent; host exits0 normally.
No app source/binary/policy changed; current binary hashes match batch96. Both previous failures
remain recorded, not converted to passes. No native dialog, elevated target or complete tree/
singleton/full-app qualification is inferred. M08 gate4 stays active for those requirements.
Next qualify truthful elevation-required target behavior through the normal non-elevated launch
contract; no UAC/elevation/settings change. [Evidence](evidence/mapgyver-m08-column-ui-2026-11-01.json).

**Actual column/application integration batch96:** 2026-11-01 **06:35 UTC**, stop **07:20 UTC**;
investigation cutoff **06:55 UTC**, at most two finite app invocations. Only M08 gate4 is active.
Previous turn is progress: `61e08238` verifies Composition project/gesture lifetime, retaining
native imagery/outlines. Verify existing column association/distance/history/missing-reference
controls through actual UI, then actual owned-fixture readiness/shared holds/stop/availability/
project replacement and snapshot runtime feedback. Reuse current ordinary app and finite owned
executable with no arguments; no GPU/Spout/privilege/settings work. Backend calls only inspect;
all authoring and launches use real UI. Retain native dialog/singleton/elevation/full-app gaps.

**Batch96 result:** partial, not accepted. The first run reaches shared holds/normal stop but
fails selecting a restored definition before availability input; Remove intentionally clears
selection and Undo does not select it. Its receipt trace also proves an obsolete definition Inspect
during pending removal. A failing unit regression (one stale inspection instead of zero) supports
deferring `ApplicationController.refresh()` until the mutation reply reconciles selection.
All **35 Matrix/Application frontend tests**, Svelte (zero errors/warnings), syntax/Prettier and
the app rebuild pass. The fixture explicitly reselects the restored definition through the UI.
The second/final run has **zero rejected requests**, verifies disabled in-use process retention,
unloads/stops it, then requests the preload column while disabled. It fails waiting for
`Unavailable`: runtime `applications=[]`. No cause or correction is established; later availability
recovery and project replacement were not reached. Both hosts exit0 normally, and reported PIDs
3572/34536/20344/48408 are absent. Do not generalize this to complete tree/native retirement.
Keep M08 gate4 active, reduce the next action to the exact launch/demand/runtime mismatch, and
preserve both failed attempts. No third invocation, new phase or acceptance revision in this batch.
[Bounded result](evidence/mapgyver-m08-column-ui-partial-2026-11-01.json).

**Actual browser form batch 85:** started 2026-11-01 **04:48 UTC**, stop **05:33 UTC**. Only M08
gate 4 is active. MapGyver owns Applications forms and intents; Golden owns bundled transport,
history and project identity. Reuse existing ordinary headless/bundled-browser fixtures to drive
actual application creation, field editing, save/history, copy/availability/selection and rejection,
with read-only public backend correlation. Native file-dialog/preview/OS input and unverified
retirement/elevation stay open. One browser/host invocation after syntax/format prerequisites;
a concrete UI defect may receive one corrected verification, max two attempts/20 minutes total.
No new runtime framework, CDP/desktop retry, process-target rerun or settings/acceptance change.

**Batch85 result, 04:52 UTC:** real bundled browser input passes the second/final invocation,
**exit0 /5.31 s**, browser **149.0.7827.55**, zero page errors. All mutations/history commands use
DOM controls; public backend calls only observe. Create/rename, executable/directory/argument
boundaries, window mode, explicit Spout sender/first-image timeout, stop/restart fields and full
definition Save/Undo/Redo preserve exact values. Populated/empty/inherited environment modes stay
distinct. Copy has its own selected committed identity; availability Undo/Redo leaves the peer
enabled; Remove/Undo restores the persistent identity. A visibly rejected `.cmd` draft leaves the
saved definition and removal redo intact. No unloaded definition starts a process. The ordinary
host repeats source/history/reload checks and exits normally; no MapGyver remains.

The first invocation exercises create/rename/argument drafts, then an exact Window select-label
locator times out. Correct only test matching for wrapped-select labels, without production code
changes or another experiment after success. Syntax/Prettier and both formatters pass. Native
file-dialog, preview/physical/OS input, retirement/elevation/full-app evidence is not inferred.
**No gate closes: M08 3/4**, overall **4/10 (40%)**. Publish, then select existing M05 gate6 actual
Screen controls/source/gesture/history in the same ordinary bundled browser, only if naturally
enabled; retain native/physical requirements separately without bypassing controls or changing
acceptance. [Evidence](evidence/mapgyver-m08-browser-forms-2026-11-01.json).

**Aggregate target-outcome batch 83:** started 2026-11-01 **04:33 UTC**, stop **05:18 UTC**;
native/process experiment cutoff **04:53 UTC**, one existing-check invocation. Previous turn is
progress: `31938a8e` accepts M07 native Matrix gate 4 and preserves actual input/global budgets.
Only M08 gate 4 is active. MapGyver owns application lifecycle policy; Golden IO owns native
process/job containment and observation. After current build/software prerequisites finish,
run existing `check.mjs --target-outcomes --orderly-shutdown` with its corrected193/216 image
rejection expectation. Verify singleton non-adoption/external isolation, launcher child cleanup,
breakaway denial, invalid/missing image errors and normal host exit. Record actual conservative
retirement separately; no complete cleanup/elevation/input pass may be inferred. No GPU, desktop
activation retry, new target fixture or settings change. A named concrete correctness defect may
receive a bounded source correction, but no second process invocation in this batch.

**Batch 83 result, 04:34 UTC:** the one existing aggregate invocation exits **0 /5.28 s**, current
app/process-fixture builds, JS syntax and **41 application library tests** pass. Singleton duplicate
exits **17**, never adopts/terminates the external singleton and retains **RetirementUnverified**.
Launcher root **6428** exits **0** without false sender readiness; child **11508** terminates and
unload reaches **Idle**. Breakaway **32748** observes native **error5**, no escaped target, then
unloads **Idle**. Missing/invalid test images report **2 /216**, `.cmd` configuration rejects without
mutation and no shell/elevation/retry fallback is used. Host **39672**, independent external
**32920**, baseline roots **46380/49372** and singleton **40480** clean up normally; all eight known
PIDs are absent after test finally. Callbacks acknowledge **7**, availability and overload recover.

The existing Golden IO observer intentionally withholds completion when job accounting contains
unobserved lifetimes; source inspection establishes no concrete defect/cause for this singleton
gap. No policy relaxation, new process fixture, source correction or second run follows. Actual
elevation/form/full cleanup remain unverified. **No gate closes: M08 3/4**, overall **4/10 (40%)**.
Next, after publication, select M06 gate 4's one bounded supported CDP connection/input smoke using
the existing Playwright dependency. Desktop activation remains parked; no registry/security/helper
workaround. [Evidence](evidence/mapgyver-m08-target-outcomes-2026-11-01.json).

**Application parity batch 81:** started 2026-11-01 **04:17 UTC**, stop **05:02 UTC**. Only M08
gate 4 is active. MapGyver owns application definitions/Matrix associations/callback policy;
Golden owns shared host, script operation routing and process primitives. Reuse the existing
four-caller committed-operation harness to verify application authoring, bounded observations,
enablement, association/history and rejection. The immutable observer must include application
persistent state without treating runtime revision/instance changes as authored history. Reconcile
ordinary-headless integration and frontend coverage against the complete gate; do not infer real
input, unsupported outcomes or full app qualification. No native GPU, desktop activation retry,
new process fixture or parked Matrix experiment. Publish one coherent acceptance improvement;
only verified full scope may close the gate.

**Batch 81 result, 04:24 UTC:** four application parity tests cover **36 cases ×four callers =144**
paths: 14 authoring/rejection, four creation/copy, 14 bounded observations/stale tickets and four
column-association operations. Persistent definitions/labels/enablement and Matrix associations
now participate in the immutable history observer; recreated definition instances/revisions do
not become authored state. Replies expose committed identities, every changed operation has one
Undo/Redo step, reads/rejection create none and history cannot replay callbacks or commands.
Catalog bounds/pagination/search and enabled/configured/missing reference semantics are asserted.

The combined caller suite passes **14 tests**, application frontend logic **20**, full Rust library
**206 /one existing hardware ignore**, strict combined native/process-feature all-target Clippy,
ordinary app/fixture builds, JS syntax and both formatters pass. The first full library invocation
fails the existing configured-burst recovery publication assertion (205 pass/one fail/one ignore).
Its exact rejection is now retained in failures; the second full invocation passes. Cause remains
unproven; this assertion-message change is not claimed as a runtime fix.

The existing ordinary headless check exits **0 /2.33 s**, host **40944**, external **43200**, owned
roots **42704/26444**, callbacks acknowledged **7**. Launch/retention/unload, availability, overload
recovery, external isolation and normal sentinel shutdown pass; all four known PIDs are absent
after test cleanup. Actual form/dialog input, corrected aggregate target outcomes, elevated cases
and unobserved-tree/full-app cleanup remain open. No native GPU or desktop activation run occurs.
**No gate closes:** M08 **3/4**, overall **4/10 (40%)**. This separate reviewable acceptance delivery
fulfills M07's recorded return trigger: publish, then one bounded verification of its exact
pending-only timeout correction with original scope/budgets and all failures retained.
[Evidence](evidence/mapgyver-m08-caller-parity-2026-11-01.json).

**Product batch 46:** started 2026-10-31 **14:06 UTC**, recorded stop **14:51 UTC**.
Implement cancellable server ownership in Golden transport and reuse the existing automation
shutdown-file option for the default headless host. A stop must end listener/runtime/hub work,
close owned connections and release host services before returning normally; keep timing behavior
and actor/domain work at their existing boundaries. Verify idle and connected cancellation,
startup/error cleanup, public APIs, MapGyver normal-exit/known-tree cleanup and Chataigne compile/
shared-host regressions. Native verification follows terminal software prerequisites, at most
two invocations/20 minutes and the 14:51 cutoff. No UI/GPU/driver or parked retirement investigation.

Batch 46's native functional baseline starts **14:39 UTC**, cutoff **14:51 UTC** (the original
batch stop, earlier than a new 20-minute window). Prerequisites are terminal: **187 app tests /
one existing hardware ignore**, **177 headless tests**, **95 transport tests / one existing ignore**,
**32 desktop-host tests / two existing ignores**, the public headless API test, strict combined
all-target/all-feature Clippy, Chataigne binary compile, current ordinary MapGyver/finite fixtures,
generated drift, JS syntax/Prettier and both Cargo formatters. Run the existing check with
`--project-replacement --orderly-shutdown`: the sentinel must yield exit code zero, the known
owned tree must terminate and the independent external target must survive. Keep normal host exit
separate from target graceful-close acknowledgement or complete unobserved retirement. At most
two invocations; no second run after a passing result and no parked target-outcome case.

**Batch 46 result (14:39 UTC, before the 14:51 cutoff):** Golden transport exposes the one-way
`UiServerShutdown` signal. Async listener cancellation uses the existing Tokio runtime; active
HTTP/WebSocket sockets are tracked by owned handles. Listener, engine tick, hub, encoder and
connection workers stop cooperatively, with a bounded thread drain and explicit timeout/panic/
startup errors. Completed connection workers are reaped, keeping ownership bounded by admission.
Long tick waits check cancellation at most every 100 ms while retaining absolute deadlines.
App service guards are cancelled before transport drain. No HTTP shutdown endpoint or app-shell
bootstrap was introduced; shared Golden packages retain reusable ownership.

The default headless host reuses `--automation-shutdown-file PATH` with a cancellable coarse
watcher. Pre-existing sentinels still reject startup; metadata failures request stop and return an
error. Factory/guard disposal remain prompt; app-native retirement is not joined from a destructor.
Shared replacement-order tests now use per-case Arc state after parallel runs exposed their
shared global flags; no replacement semantics changed.

**Validation:** **187 app tests / one existing hardware ignore**, **177 headless tests**, **95
transport tests / one existing ignore**, **32 desktop-host tests / two existing ignores**, the
public headless API test, strict combined all-target/all-feature Clippy, Chataigne binary compile,
current MapGyver/finite fixtures, generated drift, JS syntax/Prettier, both required Cargo formatters
and whitespace checks pass. New tests cover shared cancellation, idle listener wake, blocked HTTP
closure/port release, already-stopped startup, long tick waits, blocked-worker timeout/recovery,
watcher cancellation/error and startup error cancellation.

The first/only ordinary-app invocation passes in **2.98 s**: host **45440**, external **24712**,
normal sentinel-driven exit **0**, and known final tree **15832/33896/7300** terminated. Save/New/
Load retires prior known PIDs **50192/50372/22144/49184**, then interim **35916**, preserves stable
definitions with fresh instances, rejects three stale callbacks, and admits fresh-project work.
Availability, overload recovery and independent external survival also pass. No second native
invocation, GPU/display/input or parked target-outcome run occurred; private test data is removed.

**No gate closes:** M08 stays **3/4 = 75%**, overall **3/10 = 30%**; denominators are unchanged.
Normal host exit and known-member cleanup do not prove an app's graceful-close acknowledgement,
complete unobserved retirement or full UI/mode/unsupported-target/recovery qualification. Product
deliveries are **42**; ten diagnostic-only batches are allowed, **four used**. Next is bounded M04
Solid Color gate 2 acceptance, keeping parked M03/M08 investigations and every original budget.

**Qualification batch 45:** started 2026-10-31 **13:51 UTC**, recorded stop **14:36 UTC**.
Use the fourth diagnostic allocation (41 product deliveries permit ten; three used). Through the
ordinary headless app and public project operations, replace a project with a running owned target,
verify old known processes terminate while an independent same-executable target survives, and
verify the fresh project publishes only its own host/services/application/callback observations.
Then admit a new owned target and verify shutdown releases that target without killing the external
one. Preserve project identity/launch fencing and separate authored replacement from OS cleanup.

Finish current app/fixture builds, relevant software tests/Clippy and syntax/format prerequisites
before native execution. Record the native start/deadline and allow at most two invocations/20
minutes, also bounded by 14:36 UTC. A reproduced blocker permits a focused correctness fix; the
parked target-outcome retirement/invalid-image question, elevated targets, GPU/driver/display and
desktop automation recovery are outside this batch. Partial evidence cannot close full M08 gate 4.

Batch 45's native baseline starts **14:02 UTC**, investigation cutoff **14:22 UTC** (before the
14:36 batch stop). Prerequisites are terminal: **187 app tests / one existing hardware ignore**,
**177 headless tests**, strict all-target/all-feature Clippy, current MapGyver/application fixture/
Golden IO tree fixture builds, generated drift, JS syntax/Prettier, both Cargo formatters and
whitespace checks. Run `node apps/mapgyver/tests/headless/check.mjs --project-replacement` once;
reserve the second invocation only for a concrete failure. The app receives a private temporary
`APPDATA` directory, so project operations do not write the user's preferences/recent projects.
Known tree members and the external same-executable process are observed independently. Host-exit
checks terminate the headless host; they do not qualify orderly/graceful host shutdown, which the
current headless entrypoint cannot request through the desktop-only shutdown-file option.

**Batch 45 result (14:02 UTC, before either cutoff):** the first/only ordinary-app invocation
passes in **2.87 s**, host **42288**, external same-executable PID **31752**. Public Save/New retires
known PIDs **3760/12864/19656/33348**; new-project supervision is healthy, the application catalog
is empty and callback acknowledgements restart at zero. Public Load retires interim PID **49860**,
restores saved definitions/stable IDs with fresh definition instances, and does not resume saved
Matrix playback or import old runtime callbacks. Three old-project callback requests fail with
the specific stale-project reason. The loaded project then admits a new three-process tree;
forced host exit terminates known PIDs **23152/41432/12884**, preserving the external target.
Prior launch/unload/availability/over-capacity recovery checks also pass. The test's private
preferences/project/report directory is removed only after its exact absolute target is checked.

**Validation:** **187 app tests / one existing hardware ignore**, **177 headless tests**, strict
all-target/all-feature Clippy, generated drift, current app/application/Golden IO fixture builds,
JS syntax/Prettier, both Cargo formatters and whitespace checks pass. No second native invocation,
GPU/display/desktop or parked target-outcome rerun occurred. No production/shared code changed.
These observations prove cleanup of the explicitly known members, not complete unobserved-lifetime
retirement or graceful host shutdown. The current headless host rejects the existing automation
shutdown-file option and runs until process termination; implementing orderly stop is the next
shared-host product task, with Chataigne regression checks for that boundary.

**No gate closes:** M08 stays **3/4 = 75%**, overall **3/10 = 30%**; denominators are unchanged.
Product deliveries remain **41**; ten diagnostic-only batches are allowed, now **four used**.
No consecutive diagnostic batch: implement orderly headless stopping next. Actual form input,
complete mode/control/unsupported-target parity, unverified retirement/elevated-target evidence
and full recovery/scale retain M08 gate 4.

**Product batch 44:** started 2026-10-31 **13:42 UTC**, recorded stop **14:27 UTC**.
Add executable browsing to the app-owned launch-definition form through Golden's public desktop
file-dialog API. Selection updates only the current draft; cancellation, failure, unmount and late
responses must not mutate another application or commit/launch anything. Keep explicit host-path
entry for browser/headless clients and backend-owned validation/configuration semantics. Verify
the draft lifecycle with focused UI tests, Svelte/build and relevant Rust/generated checks. No native
target-outcome rerun, desktop-automation recovery, GPU/driver or M03 investigation is scheduled.

**Batch 44 result (checks complete 13:49 UTC, before the 14:27 cutoff):** the Applications launch
definition offers a desktop `.exe` picker via `golden_ui/host/desktop`. `executable-selection.ts`
owns only the form-local path, pending state and optional error. One pending dialog blocks duplicate
requests and form editing/save; cancellation preserves exact paths, including spaces/Unicode.
Selection does not submit a configuration intent, infer a working directory, change arguments or
launch a target. The explicit Save action retains the same backend-owned configuration validation.

The form's existing identity/instance/definition key disposes the previous selection owner; late
choices and failures cannot update another application. Disabling the form discards the pending
choice, and dialog failure releases pending state so the user can retry. Browser/headless clients
keep explicit paths referring to the MapGyver host. No shared host/protocol/backend boundary changed.

**Validation:** **58 frontend tests**, including four focused asynchronous selection suites;
Svelte check (**zero errors/warnings**), production UI build (existing large-chunk advisory),
**187 app tests / one existing hardware ignore**, generated drift, normal MapGyver binary compile,
Prettier, both required Cargo formatters and whitespace checks pass. The first focused invocation
found a Node strip-only TypeScript limitation with constructor parameter properties; explicit
class fields repair it, and the focused/full suites pass. No native process/dialog/input or
target-outcome invocation ran. Actual file-dialog interaction/form input remains unverified.

**No gate closes:** M08 stays **3/4 = 75%**, overall **3/10 = 30%**; denominators are unchanged.
Product deliveries are **41**; ten diagnostic-only batches are allowed, **three used**. Next is
bounded ordinary-app project replacement/shutdown acceptance. Preserve the earlier failed complete
target-outcome run, unverified retirement/elevated-target evidence and actual form interaction in
gate 4; no unsupported-target, GPU or desktop-recovery detour is scheduled.

**Qualification batch 43:** started 2026-10-31 **13:29 UTC**, recorded stop **14:14 UTC**.
Use the third diagnostic allocation (40 product deliveries permit ten; two used). Verify ordinary
headless application outcomes through the existing HTTP check: a rejected singleton launch must
not adopt/stop the external instance; launcher-root exit must fail readiness and retire its known
descendants; breakaway must remain denied by the owned job; missing/invalid executable failures
must stay explicit without shell/elevation fallback. Use finite app-owned fixtures and unique
temporary paths/names. Elevated-target denial is accepted only with an actual relevant native
result, otherwise retain it open. No user process, GPU/display, desktop automation or M03 work.

Before native execution, finish fixture/app builds, relevant software tests, Clippy and syntax/
format checks. Record the native start/deadline; allow at most two invocations and 20 minutes,
also bounded by 14:14 UTC. A failed expected outcome authorizes a minimal reproduced correctness
fix; it does not authorize another probe family. These checks alone cannot close M08 gate 4.

Batch 43's native baseline starts **13:35 UTC**, investigation cutoff **13:55 UTC** (also before
the 14:14 batch stop). Prerequisites passed: **187 app tests / one existing hardware ignore**,
**177 GPU-off/headless tests**, strict all-target/all-feature Clippy, current ordinary app/fixture
builds, JS syntax/Prettier, generated drift, both Cargo formatters and whitespace checks. One
existing HTTP-check invocation exercises all fixed outcomes and the prior lifecycle/recovery
checks. A second invocation is reserved for a concrete failure; no unrelated experiment.

**First native invocation:** 13:35 UTC, **7.37 s**, failed the expected terminal `Failed` state
for the singleton duplicate. The generated snapshot instead reports **`RetirementUnverified`**,
root exit **17**, PID **48008**, no retry deadline; the baseline owned PID **36196** remains Ready.
This is unresolved tree-retirement evidence, not a successful retirement. Keep the runtime's
conservative state and the acceptance debt. The remaining invocation will explicitly retain
`RetirementUnverified` in its result while checking known-root termination, external isolation
and all other fixed cases. It does not convert that state into complete cleanup. No native
driver/overlay/GPU investigation or runtime change is authorized by this outcome.

**Second/final native invocation:** **13:37 UTC**, **4.86 s**. Sequential assertions reach the
invalid-image case after verifying singleton non-adoption/external survival, launcher root exit
with known descendant termination, actual breakaway rejection (**Windows error 5**), and missing
executable failure (**error 2**). The missing-file case also verifies `.cmd` authoring rejection
without snapshot mutation and no retry when disabled. The check then fails: this Windows loader
returns **error 216**, while the test expected only **193** for invalid test-image bytes. The
result is a failed invocation, not a green aggregate check or a cleanup qualification pass.

Corrected the test assumption to accept explicit native-image rejection **193 or 216**, recording
the actual code. Partial outcomes are now printed if a future invocation fails. Native retirement
gaps remain explicit `RetirementUnverified` results, never `Idle`/cleanup proof; known-root/child
termination and external non-adoption are checked separately. The expanded suite is opt-in:
`node apps/mapgyver/tests/headless/check.mjs --target-outcomes`; the default check retains batch
42's ordinary lifecycle/availability behavior. **No third native invocation ran**. The corrected
complete target-outcome check remains unverified. No production runtime or shared Golden code
changed; no driver/overlay/GPU/desktop recovery or additional probe was attempted.

Finite test-owned Windows fixtures use a unique named mutex, one launcher child and an explicit
breakaway attempt. All spawned targets are hidden and have a 20-second fallback lifetime. Their
test external singleton is explicitly closed; the app's owned job handles are released by the
test's `finally` host shutdown. Do not infer full unobserved-tree cleanup from that release.

**Software validation:** current app/fixture builds, **187 app tests / one existing hardware
ignore**, **177 headless tests**, strict all-target/all-feature Clippy, generated drift, JS syntax/
Prettier, both Cargo formatters and whitespace checks pass. Existing native error-code tests cover
access/unsupported/elevation-required propagation without fallback; actual elevated-target denial
is still open. The invalid-image expectation correction received syntax/format review only.

**No gate closes:** M08 remains **3/4 = 75%**, overall **3/10 = 30%**. Fixed denominators are unchanged.
Product deliveries remain **40**; ten diagnostic-only batches are allowed, now **three used**.
Next is the product launch-definition file-picker workflow. No consecutive diagnostic batch and
no repeated retirement investigation are scheduled; retain the explicit unresolved cases in gate 4.

**Product batch 42:** started 2026-10-31 **12:58 UTC**, recorded stop **13:43 UTC**.
Add shared-application availability control through generated backend intents, the Applications
panel, command requests and script methods/snippets. Backend owns local/effective enabled state,
identity and admission semantics; keep existing in-use/final-render holds and authored history
separate from process effects. Verify disabled admission, parent disablement, persistence/Undo,
command/hosted-script parity and stale UI mutation replies. Run only relevant bounded verification;
no GPU, display, desktop recovery or diagnostic-only investigation is scheduled.

Batch 42's ordinary headless functional check starts **13:27 UTC** after terminal success of
187 app tests (one existing hardware ignore), 177 headless tests, 54 frontend tests, Svelte
check, both Clippy configurations, generated drift and the current ordinary app/finite-fixture
build. Hypothesis: disabling blocks a new Matrix admission, while disabling/re-enabling an
already-used application preserves its PID and exposes retained/current configuration honestly.
Use the existing HTTP check, at most two invocations, with the original **13:43 UTC** batch
cutoff. Preserve overload recovery, cleanup and independent-process checks; no GPU or UI input.

**Batch 42 result (verified 13:27 UTC, before the 13:43 cutoff):** `SetEnabled` uses Golden's
standard metadata edit, one Undo step and no history change for the same local value. The generated
snapshot distinguishes `Enabled`, `Disabled` and `DisabledByParent`. Disabled copies retain their
local flag and receive new IDs; sparse project reload preserves it. Parent disablement cannot be
overridden by the application intent. Prepared captures become stale when effective enablement
changes, preventing delayed admission. Existing controller/final-render holds remain authoritative.

The Applications panel waits for backend observations and blocks duplicate pending edits. The
generic Media Operation command, actual hosted QuickJS `local.editMedia` and app-owned snippet
share the same intent/result callback. Software tests cover committed callback/history semantics,
Undo, persistence, stale prepared admission and late rejected replies after UI selection changes.

**Validation:** **187 app tests / one existing hardware ignore**, **177 headless tests**, **54
frontend tests**, Svelte check (**zero errors/warnings**), strict all-target/all-feature and
headless-library Clippy, generated drift, current ordinary app/fixture build, JS syntax/Prettier,
both required Cargo formatters and whitespace checks pass. No shared Golden code changed.

The first/only ordinary headless invocation passes in **2.31 s**: host **36284**, independent
same-executable PID **50824**, owned PIDs **48632/17448**, seven lifecycle acknowledgements.
Disabled active demand reports `Unavailable` with no PID; enabling admits a new launch. Disabling
and re-enabling that in-use process reports `Retained` then `Current` with unchanged PID.
Over-capacity error/recovery, neighbor retention, unload, host-exit cleanup and independent-process
survival also pass; test-owned processes are cleaned up. No second/native UI/GPU invocation ran.

**No gate closes:** M08 stays **3/4 = 75%**, overall **3/10 = 30%**; fixed denominators are unchanged.
Actual form interaction, full mode/control parity, unsupported-target outcomes and app recovery/scale
retain gate 4. **40 product deliveries** permit ten diagnostic-only batches; **two used**. Next is
a bounded M08 unsupported-target acceptance batch, with fixed cases and software prerequisites,
not another host-observability feature or parked M03 investigation.

**Product batch 41:** started 2026-10-31 **12:17 UTC**, recorded stop **13:02 UTC**.
Expose active host supervision, demand-admission failures/recovery and callback-service failures
through generated runtime status in the Applications panel and the same UI/script/headless query
contract. App-owned publishers must fence project and service owner, bound error metadata and
publish only changes; no periodic actor inspection or authored history. Verify an actual headless
over-capacity demand error and recovery without starting processes, plus stale publisher/project
and UI status cases. Preserve M08 gate 3 and leave full gate 4 acceptance open. No hardware,
GPU, desktop recovery or diagnostic-only experiment is scheduled.

After terminal software/Clippy prerequisites, run the existing ordinary headless HTTP check once
to verify the new health contract with an already-running owned process: over-capacity aggregate
admission must publish its reason, preserve the admitted PID and recover after the extra scopes
unload. Build the current app and finite fixture first. This is product verification, with at most
two invocations and the original **13:02 UTC** batch cutoff; no GPU, display or input automation.

The ordinary headless baseline starts **12:50 UTC**. Prerequisites are terminal: **184 app tests /
one existing hardware ignore**, **174 headless tests**, **53 frontend tests**, Svelte check with
zero errors/warnings, strict all-target/all-feature Clippy, generated drift and ordinary app/finite
fixture builds. Verify healthy host/services, visible aggregate-admission error and same-PID recovery,
then retain the existing unload/restart/host-exit/external-process checks.

**Batch 41 result (checks complete 12:50 UTC, before the 13:02 cutoff):** generated runtime
snapshots expose the attached Desktop/Headless host and independent demand/callback service
states. App-owned publishers fence project/owner identity, retain at most 512 error characters and
advance observations only when published values change. Callback attempt revisions prevent a late
startup acknowledgement from clearing an already-reported delivery failure, including an unchanged
retry error. Unreplayable journal faults remain backend-owned `Halted` outcomes with reload guidance.

Desktop observation attaches before process admission; an empty process list no longer hides a
capture/capacity failure. Headless and rendered/logical desktop admission use the same health
contract. Prepared captures must match the active supervisor before worker admission. Ordinary
desktop capture errors now retry while retaining the controller. The panel renders these generated
states; scripts/headless clients read the same `InspectRuntime` result and result callback contract.
No authored history, OS handles, launch argument/environment values or actor polling were added.

**Validation:** **184 app tests / one existing hardware ignore**, **174 headless tests**, **53
frontend tests**, Svelte check (**zero errors/warnings**), strict all-target/all-feature and
headless-library Clippy, generated drift, ordinary app/fixture build, JS syntax/Prettier, both
required Cargo formatters and whitespace checks pass. Software checks cover bounded publication,
project/owner/startup fences, callback halt projection, stale UI observations and real headless
65-definition capture failure/recovery with no admitted processes or authored-history changes.
No shared Golden boundary changed.

The first/only ordinary headless invocation passes in **2.04 s**: host **41228**, owned PIDs
**16580/28240**, independent same-executable PID **14924**, six lifecycle acknowledgements.
Over-capacity demand reports `application runtime admits at most 64 active/retiring definitions`,
preserves the currently admitted PID and clears after excess scopes unload. Existing preload/
post-retention/unload/restart/host-exit and external-process survival checks also pass; test-owned
processes are cleaned up. No second invocation or GPU/display/input experiment ran.

**No gate closes:** M08 remains **3/4 = 75%**, overall **3/10 = 30%**; denominators are unchanged.
Full app/mode/UI/control/unsupported-target parity and recovery/scale acceptance retain gate 4.
Product-delivery count is **39**; nine diagnostic-only batches are allowed, **two used**. Next is
shared-application availability control parity, not another diagnostic-only batch.

**Qualification batch 40:** started 2026-10-31 **11:58 UTC**, recorded stop **12:43 UTC**.
Use the second diagnostic allocation (38 product deliveries permit nine; one used previously).
Verify the remaining M08 gate 3 native crash/restart/backoff, pre-resume cancellation and owned
descendant forced-stop/replacement behavior through the existing runtime/fixture boundary.
Keep sender evidence, final-plan retention and stale outcome requirements intact. Hypothesis:
the existing controller honors these contracts with real Windows processes; failures must identify
a concrete ownership/admission/restart defect before changing production code. Compile/software
checks must finish first. Native execution admits at most two invocations and a 20-minute window;
record its start/cutoff before running. The investigation window is **11:58–12:18 UTC**, including
preparation; a continuation does not reset it. Do not add GPU, display, clock or general profiling work.

Native execution starts **12:13 UTC**, with the original **12:18 UTC** cutoff. Prerequisites are
terminal: **182 default app tests / one hardware ignore**, **172 GPU-disabled software tests / one
explicit native-fixture ignore**, strict all-target/all-feature Clippy, both finite fixture builds
and both GPU-disabled native test executable builds. First invoke the explicit pre-resume
cancellation case; second invoke the seven owned-process integration cases including the explicit
tree fixture case. No third invocation is allowed. Cross-package fixture cases stay explicitly
ignored in ordinary runs rather than depending silently on a prebuilt executable.

**Batch 40 result (12:13 UTC): M08 gate 3 closes.** Both native invocations pass before the
12:18 cutoff. The first proves actual suspended creation followed by pre-resume cancellation,
retained suspended ownership, verified cleanup and no execution of the target's report code
(**1/1**, 0.04 s). The second passes **7/7** owned-process integration cases (1.16 s).
New cases observe exit code 23 through three distinct launches, configured 150/300/300 ms capped
backoff deadlines with no early launch, cancellation of retry on intentional unload, externally
supplied readiness timing out with owned-process retirement, and forced stop/project retirement
of two three-process trees. Read-only SYNCHRONIZE handles pin each known root/branch/leaf lifetime
before cleanup; all signal before completion is accepted. No process-name observation or kill is used.
The fixtures are finite, hidden and owned by the test. GPU-disabled execution never loads a sender
or creates a graphics device/window. Existing native Spout evidence is retained from batch 35.

Gate 3's complete fixed scope maps to cumulative evidence, without a denominator or budget change:

| Required behavior | Verified evidence |
| --- | --- |
| Independent launch, process readiness and owned cleanup | Batches 32–34/39; all four existing native regressions pass again in batch 40 |
| Actual sender/first completed image and stale launch/sender fences | Batch 35 actual SDK/image test; tagged-evidence/project rejection regression passes again in batch 40 |
| Readiness timeout, crash/restart/capped backoff and unload cancellation | Fake-time suites plus batch 40's real owned-process timeout and three-launch crash/retry cases |
| Cancellation before resume and retained failed/suspended ownership | Fake resume-failure/cancellation suites plus batch 40's actual suspended-process cancellation |
| Shared demand without transient restart and final render/unload release | Batches 33–34 final-plan and no-render/two-Matrix cases; actual owned-process regressions pass again in batch 40 |
| Bounded graceful/forced stop, complete tree retirement and stale project outcomes | Gate 1's actual close/Job primitive, fake deadline/stuck-retirement suites, and batch 40's pinned root/branch/leaf stop and retired-project cases |

Reproduce the fixture builds with `cargo build --locked -p MapGyver -p golden_io
--no-default-features --features MapGyver/application-process-fixture,golden_io/process-fixture
--bin application_process_fixture --bin golden_io_process_fixture`. Compile tests with
`cargo test --locked -p MapGyver --no-default-features --features application-process-fixture
--lib --test application_runtime --no-run`. Run the returned library executable with
`--ignored native_pre_resume_cancellation --nocapture`, then the integration executable with
`--include-ignored --nocapture`. The fresh default software suite passes **182 / one existing
hardware ignore**; GPU-disabled software passes **172 / one explicit fixture ignore**. Strict
all-target/all-feature Clippy, generated drift, both required formatters and whitespace checks pass.

M08 is now **3/4 = 75%**. Overall remains **3/10 = 30%** because gate 4 is open. Full app/mode/
UI/script/transport parity, unsupported/singleton/elevated/launcher/breakaway outcomes and complete
recovery/cleanup acceptance remain gate 4; production GPU/display/cadence qualification retains its
existing phase owners. Product-delivery count stays **38**; diagnostic-only usage is now **2/9**.
Next is a product delivery for operator-visible supervision failures/control parity, not another
diagnostic batch or parked M03 investigation.

**Product batch 39:** started 2026-10-31 **11:35 UTC**, recorded stop **12:20 UTC**.
Wire default headless application demand supervision and ordered script callbacks through a
public shared-host service hook, with MapGyver policy in the app layer. Preserve thin startup,
event-driven actor access, project replacement, cancellation and process ownership. Verify the
public headless path with hosted lifecycle tests and an owned process only after software checks
pass. Shared host changes require focused Golden and Chataigne regression checks. No renderer,
desktop recovery or performance-qualification investigation is scheduled.

The bounded owned-process checks start after terminal software prerequisites: **182 app tests**
with one existing hardware ignore, **172 headless tests**, **90 transport tests / one ignore**,
**30 desktop-host tests / two ignores**, the public default-host fixture, strict all-target/
all-feature MapGyver/host/transport Clippy and the Chataigne binary check. First build the ordinary
app and finite owned fixture. Admit at most two native invocations, stopping by **12:12 UTC**
(or the original 12:20 batch cutoff, whichever arrives first). The ordinary `--headless` HTTP
baseline must start/retain/unload/restart only its owned fixture, acknowledge callbacks and clean
up on host exit while a separate same-executable process survives. Then run the existing four
owned-process regressions if the allowance remains. A failure changes only the reproduced product
boundary; no GPU, display or measurement investigation is admitted.

**Batch 39 result (native checks complete 11:55 UTC, before the 12:12/12:20 cutoffs):** ordinary
`--headless` startup attaches app-owned supervision and callback delivery through Golden's public
`HostServices`. The shared host retains a fallible service guard after listener binding; bind or
service-start failure occurs before recurring engine/transport loops start. Desktop native
presentation remains a separate mode-specific factory. App startup stays thin.

Application drivers now capture immutable metadata/guards on the actor and admit workers on the
host owner. Capture requests retain no process/controller owner; failed actor calls cannot discard
it. Prepared demand is revalidated before admission. Desktop logical/rendered and headless paths
share this boundary. The headless service checks guards off-actor, retries admission errors coarsely,
replaces retired project subscriptions and cancels without joining process retirement.

**Validation:** **182 app tests / one existing hardware ignore**, **172 headless tests**,
**90 transport tests / one ignore**, **30 desktop-host tests / two ignores**, public default-host
compilation, strict all-target/all-feature MapGyver/host/transport Clippy, Chataigne binary check,
ordinary app/fixture build, generated drift, JS syntax/Prettier and whitespace checks pass. New
tests cover prepared-engine service startup/failure, bind failure, stale definition/scope captures,
hosted script launch/unload callbacks and project replacement without a renderer.

The first native invocation, `node apps/mapgyver/tests/headless/check.mjs`, passes in **0.915 s**:
host **18756** starts owned PID **44912**, retains it through the post window, unloads it, restarts
as PID **40044** and acknowledges **six** phase events. Host exit retires its second process;
separate same-executable PID **42064** survives both owned unload and host exit. The test then
cleans up its external fixture. The second/final invocation passes all **four existing owned-process
regressions** in **0.51 s**, preserving sender fences, retained configuration, shared demand and
actual final-plan cleanup. No GPU, desktop-input or timing qualification ran.

**No gate closes:** M08 remains **2/4 = 50%**, overall **3/10 = 30%**, with unchanged denominators.
Complete restart/cancellation/process-tree/recovery acceptance and full UI/mode/unsupported-target
parity retain gates 3–4. Product-delivery count is **38**; nine diagnostic-only batches are allowed,
one used. Next is one bounded M08 lifecycle acceptance batch, not parked M03 work.

**Product batch 38:** started 2026-10-31 **11:00 UTC**, recorded stop **11:45 UTC**.
Deliver ordered app lifecycle callbacks through the app-owned Media/script boundary, distinct from
coalesced UI snapshots. Preserve short-lived transitions, bound retained delivery/backpressure and
report an explicit failure rather than silently losing callbacks. Fence projects/workers, preserve
original launch identities and current listener lifetimes; never delay process termination behind delivery. Verify
the public ingress route, callback payload/order, recovery/backpressure and history/project fences
with focused fake and hosted-script tests. Default headless startup integration follows this
callback slice. No renderer/GPU/editor recovery investigation is scheduled.

**Batch 38 result (checks complete 11:33 UTC, before the 11:45 cutoff):** the app-owned lifecycle
journal retains every published phase/launch change
independently of 10 Hz UI coalescing. Its 1,024-event queued/reserved bound admits eight cleanup
credits per launch; a full queue pauses new admission while existing owners can still terminate.
One off-actor consumer prepares at most 16 canonical script payloads and dispatches opaque
project/batch tickets through generated Media operations. Applied transient event receipts release
capacity without authored history. `onApplicationLifecycle` ships in the listening script template;
pending/acknowledged/fault metadata and admission backpressure appear in the Applications panel.
The desktop host attaches this consumer independently of renderer setup. Explicit hosts can attach
the same public source; automatic default headless wiring is next, not claimed here.

Failed edit receipts stop admission until project replacement rather than replay a potentially
applied prefix. Pre-dispatch host errors retry with coarse backoff. Consumer cancellation retains
ownership until a blocked sink actually returns. Current enabled listeners receive delayed events
with their original project/sequence/launch; removed listeners do not receive queued callbacks.
Acknowledgement means committed engine event delivery, not successful execution of user handlers.
Configuration-only status remains the latest-state stream.

**Validation:** **180 app tests pass / one existing hardware ignore**, **170 headless tests pass**,
including generated-protocol drift and hosted QuickJS phase ordering, removed listeners,
duplicate/stale dispatch, failed receipts, capacity accounting and cancelled blocked consumers.
Strict all-target/all-feature Clippy, the ordinary MapGyver build, **52 frontend tests**, Svelte
check (**zero errors/warnings**), changed-file Prettier, both required Cargo formatters and
whitespace checks pass. No hardware/native experiment ran; no shared Golden boundary changed.
**No gate closes:** M08 stays **2/4 = 50%**, overall **3/10 = 30%**; the denominator is unchanged.
Default headless service/control parity and complete process/UI/native/recovery/scale requirements
retain gates 3–4. Product-delivery count is **37**; nine diagnostic-only batches are allowed,
one used, none scheduled next. Next is default headless application service/callback attachment.

**Product batch 37:** started 2026-10-31 **10:18 UTC**, recorded stop **11:03 UTC**.
Deliver live app lifecycle observations and saved versus in-use configuration presentation in the
Applications panel. App workers own process/configuration truth; publish bounded generated metadata
without recurring actor/engine polling. Fence project, owner, launch and observation revisions;
preserve final-use retirement and avoid exposing secrets from executable arguments/environment.
Verify retained configuration, status changes, coalescing and stale subscription/reply rejection
with focused fake/backend/UI tests. Keep native qualification bounded if a real process check is
needed after software prerequisites; no GPU timing, desktop recovery or parked M03 investigation.

The owned-process observation baseline starts **10:51 UTC**, stops by the batch's **11:03 UTC**
cutoff and admits at most two invocations. Prerequisites are terminal: **176 app tests** with one
existing hardware ignore, **166 headless tests**, strict all-target/all-feature Clippy, **52 frontend
tests**, Svelte check (zero errors/warnings), generated-protocol drift and the owned-process test
executable build. Verify Current/Retained/Pending/NotRunning observations and advancing revisions
through a real owned process's shared demand, delete/Undo, retained configuration change and final
cleanup, plus the existing lifecycle regressions. No GPU/desktop interaction qualification is inferred.

**Batch 37 result (checks complete 10:57 UTC, before the 11:03 cutoff):** application workers now
publish coherent revisioned status metadata and decide
whether their captured in-use settings are current, pending replacement, retained without an
admitted definition, or no longer in use. Runtime payloads omit argument/environment values.
Reconciled definition instance/revision pairs fence UI comparisons; retained-node Undo advances
revision, while reload creates a fresh ephemeral definition lifetime. Stable application IDs and
authored persistence remain unchanged. Regression testing also found that the sparse codec omitted
all-default application data, which its decoder rejected; the app codec now restores its own
default definition so an unconfigured shared app is not skipped on reload.

Public app-owned `ApplicationObservations` subscribes outside the actor. One publisher per project,
a global two-worker lifetime bound and full-snapshot coalescing at 10 Hz keep publication separate
from process supervision and renderer retirement. Old worker owners and retired project guards
cannot replace current observations; sink callbacks run outside metadata locks. The desktop host
shares the generated snapshot stream with the Applications panel. UI bootstrap closes the subscribe/
read race, rejects stale projects/revisions/listeners and retries event registration independently
of engine inspection. Non-desktop UI retains explicit snapshot refresh; explicit headless callers
can use the same public subscription. This is latest-state observation, not a reliable lifecycle
transition journal. Full script callbacks and default headless host wiring remain next.

**Validation:** the first/only native invocation passes all **four owned-process regressions**
(1.15 s), including Current/Retained/Pending/NotRunning emissions with increasing revisions,
unchanged PID during retained configuration, delete/Undo and final cleanup. No second native
experiment ran. Final **176 app tests / one existing hardware ignore**, **166 headless tests**,
strict all-target/all-feature Clippy, ordinary MapGyver build and Chataigne binary check pass.
All **52 frontend tests**, Svelte check (**zero errors/warnings**), generated-protocol drift,
changed-file Prettier, both required Cargo formatters and whitespace checks pass. Tests cover
metadata backpressure/coalescing, stale owners/projects, exact configuration policy, listener/
bootstrap races, reconnect and definition lifetime/revision fencing. No actual desktop UI input,
GPU qualification, external/singleton/elevated target or full recovery/scale claim is made.

**No gate closes:** M08 stays **2/4 = 50%**, overall **3/10 = 30%**. Reliable lifecycle callbacks,
default headless service/control parity and complete UI/native lifecycle/recovery/scale acceptance
retain gates 3–4. Product-delivery count is **36**; nine diagnostic-only batches are allowed by
the four-product ratio, one used, none scheduled next. Continue callback/headless product work.

**Product batch 36:** started 2026-10-31 **09:55 UTC**, recorded stop **10:40 UTC**.
Deliver app-owned shared-application forms and Matrix column association/pre/post controls through
the existing generated Media/Application/Matrix backend intents. Backend owns defaults, identity,
validation and mutation semantics; UI edits explicit drafts and submits typed intents. Keep the
catalog paged/bounded and configuration separate from runtime side effects. Verify request routing,
shared identity, complete definition fields, stale selection/error behavior and backend/history
boundaries with focused UI/backend checks. No GPU/native/editor automation qualification experiment
is scheduled. Full lifecycle callbacks/parity and native qualification retain gates 3–4.

**Batch 36 result (checks complete 10:17 UTC, before the 10:40 cutoff):** the Applications panel
is registered as an app-owned dockable tab. Backend defaults and IDs drive create/duplicate;
rename/remove/configure use the generated Media/Application contracts. Focused definition fields
cover executable, exact argument vector, working directory, inherited versus explicit replacement
environment (including empty), window policy, sender/first-image readiness, stop and restart limits.
Selecting a new Spout readiness mode requires explicit evidence and timeout inputs, not UI defaults.
Catalog search/pages are bounded at 64 results. UI read queues retain one active/latest request,
reject stale project/history/selection outcomes and defer authoring during active edit groups.

Column forms add/remove shared associations and save explicit pre/post distances while preserving
transitions and saved missing references. One new backend `InspectReferences` query resolves at
most 64 unique IDs in request order, returning explicit missing definitions without fetching the
catalog or launching processes. Rust owns this DTO and result; the generator and checked-in TS
consumers were updated together. It avoids per-row catalog scans and false missing-state inference
from paged UI choices. Generic request coalescing moved from Matrix to the app's shared requests
folder. Tests prove reference lookup has no history/project/runtime effects and rejects oversized/
duplicate queries; UI tests cover exact form payloads, missing references, errors, stale outcomes,
one accepted creation, backend identity selection and bounded burst reads.

**Validation:** **173 app tests pass / one existing hardware ignore**, **163 headless tests pass**,
including generated-protocol drift; strict all-target/all-feature MapGyver Clippy, ordinary MapGyver
build and Chataigne binary check pass. All **47 frontend tests**, Svelte check (**zero errors and
warnings**), changed-file Prettier, both required Cargo formatters and whitespace checks pass.
No native/hardware/editor automation experiment ran; actual form input remains unverified.

**No gate closes:** M08 stays **2/4 = 50%**, overall **3/10 = 30%**. Live lifecycle observations,
pending/in-use configuration presentation, complete callbacks/control/default headless integration
and actual UI/native/recovery/scale qualification retain gates 3–4. Product-delivery count is
**35**. Next is the live lifecycle status/pending-configuration surface; no diagnostic-only batch
or parked M03 work is scheduled.

**Product batch 35:** started 2026-10-31 **09:33 UTC**, recorded stop **10:18 UTC**.
Connect real sender/first-frame observations from the reusable native video boundary to
MapGyver's application readiness controller. Keep process ownership and app policy app-owned;
poll/discover off the engine thread, fence observations by project/configuration/launch and
retain timeout/recovery semantics. Prove sender absence/presence, first-frame distinction and
stale-observation rejection with focused software tests, then at most two native checks after
passing prerequisites. Deliver the connected behavior without reopening GPU timing, editor
automation, drivers or parked M03 work. Gates 3–4 remain open until their full scope is verified.

The native readiness baseline starts **09:44 UTC**, stops by **10:04 UTC** (within the batch
cutoff) and admits at most two invocations. Prerequisites are terminal: **171 app tests** with
one pre-existing hardware ignore, **154 Golden Video tests** with 19 explicit hardware ignores,
strict MapGyver/Golden Video all-target/all-feature Clippy, and the feature-enabled native test
executable build. Verify absent sender cannot satisfy readiness, actual sender/received-image
completion can do so without injected evidence or renderer, storage is reserved/released around
native use and final unload proves owned process cleanup. Do not infer cadence or full recovery.

The first invocation passes the new actual owned-sender/first-image test (**1/1**, 1.24 s).
The second/final invocation runs the four existing owned-process regressions with unchanged
production code; the separately invoked hardware test stays ignored in that ordinary regression
command. All four pass (**4/4**, 1.01 s). No additional native experiment is scheduled for this batch.

**Batch 35 result (checks complete 09:54 UTC, before the 10:18 cutoff):** the existing application
IO worker now observes native Spout readiness,
independent of preview/output/renderer demand. Golden owns public worker-local `SpoutProbe`:
exact sender metadata alone satisfies sender-only rules without allocating graphics storage;
first-image rules require a successful SDK receive and its completed GPU event. Busy, unchanged,
missing and merely registered endpoints cannot satisfy an image rule. Native interfaces/pixels
never enter the actor or protocol. Shared 64-image/256 MiB reservations precede receiving-image
allocation and remain charged through actual completion/device removal. Endpoint changes withdraw
prior image evidence; removal permits recreation. Application policy keeps its 100 ms sampling,
startup timeout, launch/project/configuration/admission fences and process ownership on its worker.

The actual owned fixture delays sender startup; the test verifies waiting after resume, automatic
ready from a real 128×128 sender/image (no injected evidence), shared-pool accounting/release,
unchanged project/history and proven final unload/cleanup. Four prior owned-process regressions
also pass, including stale evidence/project replacement and renderer final-use retention. No
fixture process remains. Native tests preceded a final review fix: independent tagged readiness
may arrive while this probe's copy is pending, so a ready worker now releases its probe only after
a nonblocking completion check. Process supervision stays live until its process has stopped;
native cleanup never treats cancellation as a GPU fence. Final software/Clippy/build checks were
rerun after that guard; actual stuck-GPU/fault/recovery qualification remains explicitly open.
The standalone no-native library check also exposed a pre-existing manifest error hidden by the
test dev-dependency: Matrix timeline/capture always uses `golden_time`, but it was optional behind
the GPU feature. It is now an ordinary backend dependency, with duplicate dev/feature wiring
removed; `cargo check --locked -p MapGyver --no-default-features --lib` passes independently.

**Validation:** final default app suite **171 passed / one existing hardware ignore**; Golden Video
D3D11 software suite **154 passed / 19 explicit hardware ignores**; strict all-target/all-feature
MapGyver/Golden Video Clippy and ordinary MapGyver build pass. Headless suite **161/161** and
ordinary Chataigne binary check pass. Both required Cargo formatters and whitespace checks pass.
No UI or protocol declaration changed, so frontend/generated contracts reuse their prior checks.

**No gate closes:** M08 remains **2/4 = 50%**, overall **3/10 = 30%**. Complete native lifecycle/
restart/cancellation/process-tree/recovery/scale acceptance and full editing/control/observation
parity remain gates 3–4. The probe observes exact endpoint availability, not sender ownership by
a particular PID, cadence or continuous health. Dedicated app and column controls are next.
Product-delivery count is **34**; no diagnostic-only batch is scheduled.

**Product batch 34:** started 2026-10-31 **08:52 UTC**, recorded stop **09:37 UTC**.
Deliver committed project-wide Matrix application demand without a preview, Screen output, renderer
or GPU time authority. Keep column-only indexing/capture separate from cells and rendering; cache
with column/launch/project guards so the actor is contacted only on metadata changes. Merge logical
and rendered scopes atomically in the existing app-owned bridge and preserve final plan lifetimes.
Expose the same capture/service path to explicit headless owners. Verify direct/timed launches,
pre/post, multiple Matrices, reorder/delete, stale/failed commits, shared identity and actual owned
process start/stop with no renderer. Real Spout readiness remains next unless this slice finishes
early. No editor, GPU timing, driver or parked M03 investigation.

The no-render-consumer owned-process baseline starts **09:19 UTC**, stops by **09:37 UTC** and
admits at most two invocations. Prerequisites are terminal: **170 app tests/one existing hardware
ignore** and strict all-target/all-feature Clippy pass, including deterministic interrupted app fades.
Verify the same public driver loads timed column demand across two Matrices, shares one process,
preserves it when one scope unloads and stops it when the final scope unloads, without constructing
a renderer, video authority, preview or output. No actual Spout/native GPU evidence is inferred.

The first invocation passes all four integration tests. The second/final invocation follows a
targeted admission fix: refreshed metadata may resume the same still-admitted configuration, while
removed demand cannot resume it. A fake process changes the scope fence during suspended creation
and proves one create/resume with no close/restart. **171 app tests/one existing hardware ignore**
and strict Clippy are terminal before this final invocation. No further OS experiment is scheduled.

**Batch 34 result (checks complete 09:30 UTC):** public `ApplicationDriver` services committed
logical demand independently of rendering. The native host invokes it before renderer setup;
explicit headless callers can use the same API. Column-only capture caches launch/column revisions,
never builds cell/render indices and bounds active scopes/columns before indexing. Cell edits do
not invalidate this capture. Receipt-gated `ClearPlayback` and the Unload Matrix toolbar release
logical demand without changing authored data/history; final resource holds remain independent.
Suspended creation uses the newest valid demand for the same configuration and rejects removed
or changed configurations without transient restarts.

**Gate 2 acceptance audit:** batch 31 verifies persistent definitions, stable shared references,
column copying, configuration validation, reload and Undo/project history. The pure demand and
logical-capture suites cover clamped/non-wrapping pre/post distances, direct jumps, reorder/removal
and Undo. Multiple Matrix scopes share one app/PID; unloading one preserves it, unloading the last
stops it. The interrupted A→B→C fade test preserves A/B outgoing references after A's column and
definition are removed, releasing them only at settlement. Batch 33 verifies final-use retention
with an actual process and delayed software GPU fence. Fake-time/process admission, cancellation,
crash/backoff/timeout and stale project/readiness tests cover lifecycle boundaries. Launch/unload
leave serialized project/history unchanged, and discarded queued unloads have no visible effect.
These checks close the complete fixed model scope of gate 2; no denominator changes.

**Validation:** 171 app tests pass with one pre-existing hardware ignore; 161 headless tests pass.
Both bounded native invocations pass all four owned-process integrations, including timed launch
without renderer/video clock and shared-scope unload. No fixture process remains. Strict MapGyver
all-target/all-feature Clippy, ordinary MapGyver build, Chataigne binary check, 38 frontend tests,
Svelte check (zero errors/warnings), changed-file Prettier and generated-protocol drift checks pass.

**M08 gate 2 closes: 2/4 = 50%; overall remains 3/10 = 30%.** Real Spout sender/first-frame evidence,
native D3D lifecycle, full generated UI/script/headless control/observation parity (including default
headless bootstrap wiring), pending/in-use configuration presentation and special-target/recovery/
scale qualification remain open under gates 3–4 and their owning phases. Native readiness is never
inferred from a running process. Product-delivery count is 33; no diagnostic batch is scheduled.

**Product batch 33:** started 2026-10-31 **08:14 UTC**, recorded stop **08:59 UTC**.
Connect committed Matrix column/transition demand to the application lifecycle controller and
retain application owners through submitted plan/final renderer use. Keep capture bounded and
event-driven; use existing Golden lifetime mechanisms or add a minimal app-neutral retention hook.
Verify pre/post, direct jumps, shared scopes, outgoing/removal/configuration holds, rejected/stale
plans and retirement with software/fake controls before a bounded owned-process integration run.
Keep real Spout readiness and complete UI/special-target qualification explicit if not wired here.
No GPU timing, driver, editor automation or parked M03 investigation.

The owned-process/software-renderer integration baseline starts **08:43 UTC**, stops by the batch's
**08:59 UTC** cutoff and admits at most two invocations. Software prerequisites are terminal:
**164 app tests/one existing hardware ignore**, **113 Golden Video tests** and strict all-target/
all-feature MapGyver/Golden Video Clippy pass. Verify a real owned fixture starts from captured
Matrix neighbor demand, remains alive after project close while a software GPU fence is pending,
then stops after actual plan retirement. This is not native D3D11/Spout pixel or sender evidence.

**Batch 33 result:** Matrix capture freezes application references per admitted clip and caches
neighbor/preparation/active/outgoing demand across layer/group/final captures. Edits/removal retain
the old active/outgoing IDs; transition settlement releases logical outgoing references. Availability
is resolved from backend definitions, and project/app/Matrix guards fence suspended process admission.
The native host now owns an application bridge independently of native renderer/topology incarnations;
it reconciles committed rendered demand, binds final-use retention before publication, updates demand
after transition settlement and releases demand when the last render consumer disappears.

Golden Video adds only a payload-free `PlanLifetime` and read-only completion observer. Submission,
compiled/recovery plans and backend generations retain it; final release follows actual resource and
immutable-input disposal, never just Installed. Conservative input accounting includes its allocations.
MapGyver maps observers to at most 64 app identities/128 pending lifetimes per identity. Existing
process owners remain alive through controller close/drop or project invalidation while a final-use
hold exists; such holds can never start a process. Missing proof keeps both process ownership and
the global worker slot charged. No GPU/process/app policy or handle enters the generic token.

**Verification:** **164 app tests/one pre-existing hardware ignore**, **154 headless tests**, **113
Golden Video tests**, strict all-target/all-feature MapGyver/Golden Video Clippy, **38 frontend
tests**, Svelte **0 errors/0 warnings**, ordinary MapGyver build and Chataigne binary compile check
pass. Generated-contract drift checks pass in the app suite. Matrix tests cover clamped neighbor
capture, non-displayed preload, direct jumps, frozen active IDs through column/definition removal,
stale capture and required matching lifecycle admission. Golden tests prove Installed is insufficient,
delayed resource completion releases the old lifetime, and rejected input disposal releases its token.

The first and only owned-process integration invocation passes all **three tests**. The new case
starts the real finite fixture from column 2's demand for column 3, binds renderer use, moves outside
the neighbor window, replaces the plan and closes the project. The same PID remains Ready while
software GPU work is uncompleted, then reaches proven Idle only after completion/retirement; all
workers retire and no fixture remains. This does not exercise the bundled host interactively, a real
Spout producer or native D3D completion. No further native invocation is queued. Both formatters and
final whitespace checks run before delivery; final checks finish by **08:51 UTC**, before **08:59 UTC**.

**No gate closes:** M08 stays **1/4 = 25%**, overall **3/10 = 30%**. Explicit column demand without
a render consumer still needs project-wide capture. Actual Spout readiness, pending/in-use config
presentation, complete UI/script/headless observation/control parity, native full-lifecycle and
special-target/recovery qualification remain under gates 2–4. Large-index/large-grid qualification
remains open. Product deliveries become **32**, diagnostic-only allowance **8**, **one used**; no
diagnostic-only batch is scheduled. Next is no-render-consumer demand and real readiness wiring.

**Product batch 32:** started 2026-10-31 **07:41 UTC**, recorded stop **08:26 UTC**.
Deliver the MapGyver-owned application lifecycle controller and bounded background process owner,
using committed demand sets and explicit project/launch/readiness identities. Verify shared demand,
readiness timeout, crash/backoff, cancellation and bounded graceful/forced/unverified retirement
with fake process/time inputs before actual owned fixtures. Expose generated runtime observations
through the existing Media boundary. Preserve Matrix final-render/unload integration requirements;
do not claim those gates from a standalone controller. No GPU/editor/privilege recovery diagnostics.

The finite owned-process integration check starts **08:01 UTC**, stops by **08:21 UTC** and admits
at most two invocations including the baseline. Software prerequisites are terminal: 160 app tests
pass/one existing hardware ignore; strict all-target/all-feature MapGyver and Golden IO Clippy and
59 IO unit/9 owned-process regression tests pass. Verify PID stability under shared/reconciled holds,
authored removal/Undo, final stop, tagged readiness and project replacement. No actual Spout sender,
Matrix final-render bridge or special target compatibility is inferred from this headless fixture.

The first integration invocation passed both owned-process tests. The second/final invocation adds
configuration replacement while outgoing/final-use demand remains, followed by replacement only
after release and proven retirement. Final software prerequisites are terminal: **162 app tests
pass/one existing hardware ignore** and strict all-target/all-feature Clippy passes. No further
native fixture invocation or diagnostic expansion is authorized within this batch's default budget.

**Batch 32 result:** `applications::runtime::ApplicationRuntime` explicitly owns a live project's
whole committed demand set and independent process workers. The controller validates and admits
new owners before atomic publication; failure retains the old set. A shared global 64-slot pool
counts live, blocked and retiring owners across projects. Each worker retains its own definition,
not an old whole-project demand snapshot. No process creation, observation or wait enters an engine
node tick or authored transaction. Creation is suspended/job-owned; cancellation is checked before
resume. Golden IO resume failures now return the suspended owner, preserving cleanup responsibility.

The worker handles process-start or tagged sender/first-frame readiness, timeout, capped exponential
crash backoff, explicit re-enable, intentional unload, graceful close, force and truthful retirement.
Outgoing/final-use holds preserve an existing owner through definition changes/removal; configuration
replacement waits for their release and retirement proof. A stuck/unobserved tree reports
`RetirementUnverified`, retains its owner/global slot and blocks replacement until proof arrives.
Cancellation/drop never joins on the actor. Independent workers isolate blocked creation; Windows
creation itself has no hard latency guarantee. `InspectRuntime` returns project/launch IDs, phase,
process/exit status, bounded errors and monotonic deadlines through generated Media/script callbacks.

**Verification:** eight fake process/time cases and the existing model/module suite pass: **162 app
tests/one pre-existing hardware ignore**. Both actual process integration tests pass in the second
and final invocation, including stable PID across shared demand/removal/Undo, configuration changes
held through outgoing/final use, stop/relaunch after proof, stale/insufficient injected readiness and
project retirement. The finite headless fixture is force-stopped with exit 1 before its fallback
deadline; no fixture remains. **59 IO unit/9 owned process-window regression tests**, strict
MapGyver/Golden IO all-target/all-feature Clippy, generated drift checks and **38 frontend tests**
pass. **152 headless tests** pass; Svelte reports **0 errors/0 warnings**, frontend and ordinary
MapGyver builds and the ordinary Chataigne binary compile check pass. Both formatters and final
whitespace checks pass. Implementation and final verification finish by **08:14 UTC**, before the
recorded **08:26 UTC** stop. Both fixture invocations are complete; no extra native run is queued.

**No gate closes:** M08 remains **1/4 = 25%**, overall **3/10 = 30%**. Automatic committed Matrix
lease capture, final renderer/unload completion and real Spout observation are not wired. Dedicated
forms, full lifecycle callbacks/parity and special target/full recovery qualification retain gates
2–4. This is an explicit controller API, not automatic column-controlled execution in the bundled UI.
Product deliveries become **31**; diagnostic-only allocation remains **1 used of 7**. The next
product batch connects Matrix demand and renderer lifetime ownership; no further OS diagnostics.

**Product batch 31:** started 2026-10-31 **07:18 UTC**, recorded stop **08:03 UTC**.
Deliver MapGyver-owned shared application definitions, stable column associations and pure clamped
neighbor-demand resolution. Keep authored edits/history/project codecs and domain defaults in the
backend, expose generated DTOs through the existing Media command/script/callback/snippet boundary,
and keep process execution out of this model slice. Verify column 3 pre/post=1, edges, jumps/reorder,
shared definitions/scopes and preparation/active/outgoing holds with bounded fake demand inputs;
run relevant model/protocol/UI checks and ordinary build. Lifecycle OS effects/readiness/restart
remain the next batch. No editor/native/privilege recovery or further process diagnostics.

Before source implementation, M08's provisional whole-phase item is split into four fixed gates.
Together they retain its complete plan acceptance; no item starts closed. This changes M08's
denominator from one to four, not the overall ten-phase denominator or acceptance scope.

| Gate | Full acceptance scope | State |
| --- | --- | --- |
| 1 | Public app-neutral Golden IO owned-process API: explicit executable/arguments/environment/directory, suspended launch with Job Object ownership before resume, cancellation/drop safety, truthful root/tree observations, bounded termination/retirement and unsupported/access failures; actual owned executable/descendant checks. | Accepted — batch 30; retirement can remain explicitly unverified when transient lifetimes were not observed |
| 2 | MapGyver-owned persistent app definitions, stable references and column associations; clamped non-wrapping pre/post column distances, jumps/reorder/delete and aggregate demand across scopes/preparation/active/outgoing leases; fake-time/process checks and history/project boundaries. | Accepted — batches 31–34; logical/rendered demand and process/history evidence mapped in batch 34 below |
| 3 | Background launch/readiness/timeout, sender/first-frame evidence, crash/restart/backoff, intentional-unload cancellation, shared definitions without transient restarts, final render/unload completion, bounded graceful/forced stop and stale project/launch/readiness rejection; fake and actual owned process-tree checks. | Accepted — cumulative batches 32–35/39–40; batch 40 maps the full scope and verifies actual crash/backoff, suspended cancellation, timeout and pinned descendant cleanup |
| 4 | Complete generated UI/script/headless editing/control/observation parity; actual app integration and lifecycle scenarios, truthful singleton/external/elevated/launcher/breakaway outcomes, no implicit process-name adoption/kills, project replacement/shutdown and qualified recovery/cleanup. | Active — caller/headless/outcomes/forms, batch97 column/recovery/New input and batch98 actual elevated target pass in scope; native dialog/singleton retirement/full-app gaps remain; non-elevated denial not exercised |

**Batch 31 result:** `apps/mapgyver/src/applications` owns shared persistent executable, argument,
directory/environment, window, sender-readiness, deadline and restart definitions. Backend defaults,
unique labels, bounded catalog pages and typed authoring use the existing Media operation/command/
script/callback/template boundary; Rust generates all new TypeScript DTOs. Configuration validates
through the public pure Golden IO launch check without opening an executable or launching a process.
Copies of columns retain shared application IDs. Removing a definition preserves missing references;
Undo/reload restore authored identities. Renames and unchanged configuration preserve launch guards.
Column transition forms preserve their application associations.

The pure resolver unions preparation/active/outgoing cursors and retained final-use holds, reports
configured/unconfigured/disabled/missing availability and evaluates clamped column distances after
jumps/reorder. Direct holds survive deletion/disable until the runtime owner releases them. Inputs
admit 128 matrices, 100,000 total columns, 128 cursors, 4,096 holds/definitions and 64 associations per
column. Sparse resolution materializes no cells and collapses shared neighbor reasons before adding
exact-column reasons, avoiding a full association-by-scope fan-out. It is not yet connected to the
renderer or a process worker; no process readiness, transient-restart or final-render proof is claimed.
Dedicated application forms and complete lifecycle observation/control remain gate 4.

**Verification:** model/history/reload, invalid atomic edits, shared column copies, command creation
and hosted inspection callbacks, column 3 pre/post=1, clamp/jump/reorder, overlapping scopes, detached
holds and a 10,000-column sparse case are covered. **154 app tests pass/1 pre-existing hardware
ignore**, **144 headless tests pass**, with the five application tests rerun after the final resolver
change; **59 IO unit and 9 real owned-process/window tests pass**. Strict all-target/all-feature
MapGyver and Golden IO Clippy, generated-contract drift checks, **38 frontend tests**, Svelte **0
errors/0 warnings**, Prettier, frontend and ordinary app builds, and Chataigne's ordinary binary
compile check pass. Both formatters and whitespace checks pass. A frontend build collision during
parallel verification was corrected by serializing builds; the final ordinary builds pass.
Implementation/checks finished by **07:41 UTC**, before the **08:03 UTC** stop. No editor/native
experiment, process diagnostic or privileged recovery was opened.
**No gate closes:** M08 stays **1/4 = 25%**, overall **3/10 = 30%**.
Product deliveries become **30**; diagnostic-only allocation is **1 used of 7**. Next is runtime
demand capture and background lifecycle behavior, not another diagnostic experiment.

**Product batch 30:** started 2026-10-31 **06:39 UTC**, recorded stop **07:24 UTC**.
Deliver Golden IO's public owned-process primitive under `crates/golden_core/runtime/io`, without
copying Chataigne's private App Control. Use supported Windows bindings for suspended creation,
Job Object assignment before resume, explicit tree ownership, cancellation/drop and bounded wait/
termination. Prove no execution before resume and cleanup of an owned child/grandchild fixture;
reject unsupported configurations explicitly. App definitions, readiness and demand/restart policy
stay in MapGyver for following batches. Run relevant crate/platform checks and both formatters;
commit/push this coherent mechanism before the next app batch. No desktop/native-render recovery.

**Batch 30 result:** `golden_io::owned_process` provides `ProcessSpec`, a typed `SuspendedProcess`
and sole `OwnedProcess` ownership. Creation uses the existing Windows bindings and a private,
non-inheritable kill-on-close Job Object assigned during suspended creation, verified before resume.
The split API allows cancellation before user code runs. Explicit absolute executable/argument/
directory/environment inputs are validated without implicit shell/name lookup or elevation fallback.
Close requests use verified, pinned job identities; termination applies only to the private job.
No Chataigne implementation or app lifecycle policy entered Golden IO.

The first descendant check exposed that zero active accounting can precede actual handle completion.
The API now separately observes root exit, pending captured handles and unobserved process lifetimes.
Known handles must signal before retirement is confirmed; missed transients leave an explicit gap
and bounded waits return false rather than claiming cleanup. The root-exit fixture exercises this
conservative outcome while independently proving the named child/grandchild handles terminate.
Creation-time observation and member checks avoid numeric PID reuse authorizing an unrelated close.
Strings/environment admit at most 32,767 UTF-16 units; member queries and retained descendant handles
are capped at 4,096. OS creation itself remains worker-only and is not claimed to have a hard latency
bound. Non-Windows support is explicitly unavailable; app-specific readiness/restart and arbitrary
elevated/singleton/launcher/breakaway qualification remain gates 2–4.

**Verification:** **59 IO unit tests and 9 actual owned process/window tests pass**, including
suspension/no execution, Unicode/quote/backslash/shell-metacharacter argv, environment/directory,
cancel/drop, root exit with descendants, graceful hidden-window close, caller-owned graceful signal,
forced exit code 259, missed-transient uncertainty, invalid/native errors and a same-name outside
fixture surviving the owned job's stop. Strict all-target/all-feature Golden IO Clippy and MapGyver
all-feature integration checks pass. No fixture process remains. Both formatters and final whitespace
checks run before delivery. No GPU, editor-input or driver/clock investigation was opened.
Implementation and final checks finished by **07:16 UTC**, within the recorded 45-minute batch.

**M08 gate 1 closes: 1/4 = 25%; overall remains 3/10 = 30%.** Full app lifetime behavior, UI/script/
headless parity and recovery qualifications are not accepted by this mechanism checkpoint. Product
deliveries become **29**; diagnostic-only allocation remains **1 used of 7**. Next is app-owned
definitions/column associations and neighbor-demand policy, not further process diagnostics.

### M07 acceptance items

**Batch94 result, 06:23 UTC:** one existing-fixture trace identifies four rejected operations:
InspectPlayback, Inspect, Inspect, Inspect, all for obsolete media `c970316e-61a6-4a19-a55f-d6c0276f9143`.
The unique producer of that pair is MediaManager's MatrixChannelEditor, not the Matrix grid/target
readers. Its parent retains catalog/selected channel while refreshing for a new project, allowing
the hidden child to bind old media to new session signals. MediaManager now publishes only a
catalog owned by the current session/generation, clears it during loading/disconnect and fences
async read/edit replies and disposal. Backend missing-reference validation remains unchanged.

Svelte0errors0warnings and the ordinary rebuild pass before the second/final invocation. The strict
check passes **exit0 /7,875 ms**, zero rejected control receipts, missing-media messages and page
errors. Real New/upload, selection/history reset, distinct project epochs, persisted IDs, fresh
controls and post-load history pass; inspected empty final scene has no stale-inspection error.
Original review SHA256 is unchanged; both hosts exit0 normally. All six lifecycle invocations
across batches92–94 remain recorded; no third run here. **M07 stays3/4**, with actual native
preparation/installation/submission feedback still unverified. Publish, then select M06 gate4's
remaining actual project/gesture lifetime checks; no repeat of either broad passing candidate.

**Exact rejected operation batch94:** 2026-11-01 **06:20 UTC**, stop **07:05 UTC**; investigation
cutoff **06:40 UTC**. Only M07 gate3 is active. Previous turn is progress: `a282cea3` fixes owned
viewport epoch but preserves strict failure. Reduce to the existing fixture's prepared read-only
intent/receipt correlation. One observation identifies the failed operation before further code;
at most one corrected verification afterward. Four prior lifecycle invocations and cumulative
effort remain recorded. No new framework, controller guess, native retry or weakened assertion.

**Batch93 result:** a new regression fails because a grid refresh arriving before parent observation
captures the new session generation with old media and dispatches it. Matrix state now holds its
own reactive generation, with session identity owned by the controller. Presentation uses only the
owned current viewport; stale controls block mutations and refresh synchronizes/reset ownership.
The regression also proves immediate unpublication, no stale launch/read, and fresh-control recovery.
All34 frontend tests pass; final focused5, Svelte0errors0warnings, rebuild and formatting pass.

Exactly two app invocations run. First reveals the initial private epoch guard is nonreactive,
leaving AddMatrix disabled; storing generation in observable state corrects that. Two static
TypeScript narrowing errors are resolved before rebuilding/retrying. Second reaches all lifecycle
state/history checks but still exits1 on the same missing-media assertion (two visible occurrences,
Logger deduplicates four rejections). Both hosts exit0 normally; no third run. The viewport defect
is fixed in regression scope, not established as the live symptom's cause. **M07 stays3/4**.
After two bounded corrective batches, reduce to one exact failed operation: the existing fixture
now observes only sent intents/rejected receipts, retaining stage, payload and error, excluding
authentication and snapshots. This observer is syntax/format checked but not run. Identify that
operation before further controller edits; no new probe framework or speculative correction chain.

**Owned viewport lifetime batch93:** 2026-11-01 **06:10 UTC**, stop **06:55 UTC**; investigation
cutoff **06:30 UTC**, at most two app invocations after software checks. Only M07 gate3 is active.
Previous turn is progress: `3cada047` fixes proven expired queued dispatch but preserves strict
replacement failure. Hypothesis: old owned viewport/target data can adopt the new session epoch
before parent observation resets it. Reproduce controller refresh in that ordering; fence owned
presentation/dispatch at its app boundary, then verify the unchanged strict New/upload assertion.
Do not rerun unchanged, add a new probe/route, suppress backend validation or lower acceptance.
Cumulative lifecycle evidence remains two prior invocations; continuation does not erase failures.

**Batch92 result, 06:06 UTC:** the first invocation exits0 on state/history assertions after
actual File→New replaces a requested column, a fresh Matrix is created/edited, and browser Open→
Load From uploads the unchanged saved review. Old selection/history clear, new IDs and three
distinct project epochs are observed, saved IDs survive upload, post-load mutation/history work,
and another New clears the scene. The reference SHA256 is unchanged. However the inspected final
capture shows missing-media errors (Logger deduplicates four rejections). This evidence rejects a
clean lifecycle pass despite the original test exit0; waiting for loading-overlay removal and a
zero-missing-media assertion strengthen the check.

Source review finds the app ReadQueue only fences responses, allowing a queued read to dispatch
after its context expires. A new TargetReader regression fails before the correction, then passes
for project/history/disconnect/target changes. ReadQueue now requires a context predicate before
dispatch; all seven Matrix/Application readers supply their existing context guard. In-flight
response guards and backend missing-reference validation remain intact. This is app-owned request
lifetime behavior, not a change to Golden's already-present project/intent barrier.

All **33 Matrix/Application frontend tests** pass, Svelte reports zero errors/warnings, ordinary
app rebuild and formatting pass. The second/final actual app invocation **exits1**: the stronger
assertion finds two visible occurrences of the same missing-media message, with Logger again
deduplicating four rejections. That is not two independent backend failures; the live symptom
persists after the unit-proven queue correction. Both hosts shut down normally; no MapGyver
remains. No third invocation, hidden validation relaxation, native claim or gate closure.
The browser upload uses the app's normal Documents/MapGyver directory; its copied review is
retained and the original reference hash is unchanged. Future fixture uploads now use UUID-suffixed
names with the same verified contents to avoid collisions; syntax/Prettier pass, not rerun.
**M07 stays3/4; overall4/10 (40%)**. Keep this correctness defect active. Next establish the
reference-owner/transition cause (e.g. old presented context adopting a new session generation is
only a hypothesis) and a supported correction before another strict check; do not rerun unchanged.

**Actual project replacement batch92:** 2026-11-01 **05:56 UTC**, stop **06:41 UTC**;
investigation cutoff **06:16 UTC**, at most two finite invocations. Only M07 gate3 is active.
Previous turn is progress: `eb968220` verifies the full browser input candidate; do not repeat it.
MapGyver owns Matrix selection/playback/editor state; Golden owns File menu/project transitions,
history and transport. Use actual File→New and browser Open→Load From with the existing unchanged
review project. Check active-request retirement, cleared selection/history, fresh usable controls,
persisted IDs on reload and fresh project epoch. All mutations use UI, backend requests only read.
No new automation route, native observation injection or acceptance revision. Actual native
preparation/installation/submission feedback and every retained original requirement stay open.

**Batch91 result, 05:53 UTC:** the displayed-label synchronization correction passes creation,
selection and source assignment/history. First run then stops because the fixture expects a media
name in the cell; `MatrixViewCell.source_label` is the selected channel's label. Correct that exact
expectation and synchronize displayed settings after history before editing them. Second/final
run passes the full candidate **exit0 /21,401 ms**, zero page errors. All authored mutations use
actual DOM controls; public backend operations only inspect committed identity/state.

Verified: layer/group opacity/Enabled and all four blend choices, source drafts/Apply/cycle rejection,
canvas/global and column transitions, row/group/column names/copy/removal, grouping/root moves and
column reorder with exact snapshots/IDs through Undo/Redo. Cut/Launch/Unload alter requested state
without authoring history; selection never launches, and unavailable native observation is shown
truthfully. Source Delete/Undo, tabs,24×16 grid keyboard scrolling with54 rendered cells, and browser
resize preserve authored state and source media/assets. Inspected capture confirms dock stacking
and visible scrolled rows; no physical/DPI/throughput or wide-panel branch pass is inferred.

Cumulative candidate results are three failures and one pass; no further run. Native preparation/
installation/submission observations and actual project replacement remain required, so M07 gate3
stays open **3/4**, overall **4/10 (40%)**. Publish, then target only the original project replacement
and stale-selection requirement via actual UI. Do not rerun this passing large candidate, invent
native observations or retry parked desktop/CDP activation.

**Corrective verification batch91:** 2026-11-01 **05:52 UTC**, stop **06:37 UTC**, investigation
cutoff **06:12 UTC**. Only M07 gate3 is active. Previous turn is progress: `0be21734` fixes narrow
dock layout and records two candidate failures. The specific displayed-label/refresh synchronization
correction reopens verification; cumulative failures remain two, not reset by this continuation.
Run the existing full real-input candidate; at most one further corrected comparison if a concrete
test/product defect explains a failure. No extra probe/automation route, native state injection,
scope revision or repeat of passing Composition input. All original acceptance remains required.

**Batch90 result, 05:50 UTC:** Matrix now queries its own container width to stack grid above
inspector in narrow docks. The previous minimum12rem aside could leave less than the sticky14rem
row-label width for the entire grid. Svelte has zero errors/warnings; ordinary app build passes.
The inspected actual failure capture shows stacked layout; full grid usability is not inferred.

Exactly two actual-input invocations exit1. First creates root layer/group/child and two columns,
then a cell locator is ambiguous because Layer1 exists legitimately in both root and group.
Second selects/renames the child via UI: backend inspection confirms UIChild, but the next click
runs while both rows still display old labels. Later source/settings/structure/history/runtime/
virtualization branches have not run. The fixture now waits for unique displayed row/column titles,
child's new title and visible refresh completion; those corrections are not rerun. Both shutdown-
file hosts exit0 normally and no MapGyver remains. Syntax/Prettier and both workspace formatters
pass. **M07 remains3/4, overall4/10 (40%)**, with two cumulative failures. Follow up only against
that concrete synchronization correction; no domain naming change, injected native state, new
automation route or acceptance revision. Every original live/complete-editor requirement remains.

**Actual Matrix input batch90:** started 2026-11-01 **05:44 UTC**, stop **06:29 UTC**;
investigation cutoff **06:04 UTC**, at most two finite browser invocations. Only M07 gate3 is
active. Batch89 verified Composition browser input/layout and retained native/complete-editor
gaps. MapGyver owns Matrix authoring/grid/playback intent semantics; Golden owns transport/history
and native preview. Exercise actual source/settings/structure/history controls, selection distinct
from launch, truthful unavailable native feedback, keyboard scrolling/resize/panel lifetime and
source loss. Backend requests only inspect. Reuse the existing bundled browser; no injected
renderer observations, metadata, native automation retry or acceptance revision. Correct narrow
dock geometry at the app panel boundary where the fixed aside otherwise leaves less width than
the grid's sticky row labels; use the existing container-query pattern and verify real layout.

**Pending-only timeout verification batch 82:** started 2026-11-01 **04:26 UTC**, stop **05:11 UTC**;
native experiment cutoff **04:46 UTC**, exactly one invocation. Published M08 batch 81 (`d50bd145`)
fulfills the recorded independent acceptance-delivery return trigger. Only M07 gate 4 is active.
MapGyver owns Matrix production; Golden owns GPU/shared preview execution. Hypothesis: the completed
three-mixture observation set no longer triggers a pending-only deadline, allowing final native
blue endpoint and outgoing channel retirement on NVIDIA. Original six-second fade, three samples,
tolerances and renderer budgets remain. Freeze current candidate identity after terminal software
success and observed overlay Off. No new source experiment or second invocation after failure;
preserve the exact remaining prerequisite rather than extending this chain. Actual input stays open.

**Batch 82 result — M07 gate 4 accepted, 04:29 UTC:** the one invocation passes **exit 0 /135.05 s**,
Intel LUID **00000000:00016c2a** and NVIDIA **00000000:000170d3**. Each adapter verifies **14 states ×
four independent outputs =56 captures**, **112 /86,016 texels** combined. Layer, Group, Final and
the production preview compositor match interval-aware linear-premultiplied/sRGB reference output:
zero residual outside legal UNORM intervals; nominal max **3/255** is retained. Four blends, post-
composite group opacity, layer alpha, order/Undo, source loss/Undo, empty column, sparse-grid burst
recovery and reload/fresh renderer all pass.

Each adapter produces three interrupted Layer/Group/Final mixtures with conserved RGB weight,
stable outgoing red/green ratio and monotonic progress; final blue raster/preview and outgoing
channel pruning to four pass. Each 1,000×1,000 grid has six cells and 1,000 configured launches at
a measured **9,994 ms** request span, eight admissions, bounded byte/in-flight/retiring/prepared/
outstanding work and no authored history. All six renderer lifetimes release plan/output/source
owners, with balanced plan creation/release; fresh readback storage returns idle. Actual adapter
identity and unchanged before/after executable hash are verified. Overlay remains observed Off.

Four oracle tests per executable pass first; batch 81's strict native/process-feature Clippy and
206-library/20-frontend checks remain valid with no intervening production code change. No second
native run follows success. This accepts native Matrix feature content/bounded runtime/lifetime,
not actual editor input, driver residency, physical scanout or global handle/private-growth/
CPU/GPU/cadence budgets. Original M03/M05 and integrated M09 Q01–Q05 scope stays open, including
all previously failed measurements. **Only gate 4 closes: M07 3/4 (75%)**, overall **4/10 (40%)**;
denominators and budgets unchanged. Publish, then select existing M08 aggregate target outcomes.
[Evidence](evidence/mapgyver-m07-native-matrix-2026-11-01.json).

**Concurrent native completion batch 80:** started 2026-11-01 **04:12 UTC**, stop **04:57 UTC**;
native experiment cutoff **04:32 UTC**, one invocation only. Previous turn is progress: published
`b7de2f95` preserves the fourth native failure and compiles concurrent owning-worker readbacks.
Only M07 gate 4 is active. MapGyver owns Matrix playback/production; Golden owns GPU execution.
After terminal software success and fresh observed overlay Off, freeze candidate identity and
verify the existing complete Matrix fixture once. Hypothesis: independent concurrent readbacks
observe the required three mixtures before the unchanged fade deadline. Success is complete
native coverage on both adapters with owner retirement; failure stops this chain and selects an
already implemented dependency-ready acceptance task. No new probe, timing framework, settings,
desktop activation retry or gate revision. Actual editor observations remain separately pending.

**Batch 80 result, 04:15 UTC:** one invocation exits **101 /129.02 s**, fifth cumulative run.
Intel completes; NVIDIA obtains three valid concurrent interrupted mixtures and passes weight,
outgoing-ratio and monotonicity assertions. The following unconditional deadline assertion then
fails with `samples=3`; NVIDIA final endpoint/demand retirement is not reached. All **108 captures /
82,944 texels** pass interval-aware raster checks; nominal max **3/255** remains explicit. Both
1,000-request spans measure **9,993 ms**, eight admissions each; all six renderer Drop owner checks
complete. Overlay is observed Off before/after, with no input/settings changes. Fix the timeout to
expire only a still-incomplete observation set, preserving fade duration/count/tolerances and all
production budgets. Four oracle tests per executable and strict Clippy pass; correction remains
native unverified. **Stop this native chain**, publish and select existing M08 gate-4 caller/edit/
control/observation acceptance. Return to this exact fixture after that separate reviewable
delivery. No gate closes: M07 **2/4**, overall **4/10 (40%)**.
[Evidence](evidence/mapgyver-m07-completed-observation-timeout-2026-11-01.json).

**Native Matrix verification batch 79:** started 2026-11-01 **04:05 UTC**, stop **04:50 UTC**;
native experiment cutoff **04:25 UTC**, one invocation only. Previous turn is progress: published
`7aa1798b` verifies caller parity and saves the ordinary review scene. MapGyver owns Matrix
playback/production; Golden owns GPU/shared preview execution. Hypothesis: identity checks after
a matching real frame, and fresh single-image transition probes, permit the existing complete
layer/group/final/preview, interrupted fade, sparse-grid and retirement test to finish on both
adapters. Run only after software prerequisites pass and observed overlay Off. A failure changes
the next action to its exact supported correction or records a return prerequisite; no second
native run, new probe framework, desktop activation retry or acceptance revision in this batch.

**Batch 79 result, 04:10 UTC:** exactly one native invocation exits **101 /131.92 s**. Actual
adapter logging now confirms Intel LUID **00000000:00016c2a** and NVIDIA **00000000:000170d3**.
Intel completes all checks; NVIDIA completes static content, sparse-grid burst/recovery and reload
but cannot obtain three intermediate interruption samples within the six-second window. Combined
**108 captures /82,944 texels** pass interval-aware raster checks, nominal max **3/255** retained.
Both 1,000-request launches measure **9,994 /9,993 ms** spans with eight admissions; engine runtime
bounds and all six renderer owner-retirement assertions pass, including NVIDIA unwind. Overlay is
observed Off before/after with no setting/input changes. Serial fresh receiver startup is a
fixture-serialization hypothesis, not an established production defect. Fresh probes now sample
Layer/Group/Final concurrently on owning workers; nine test readback credits cover three consumers,
without changing production budgets, fade duration, sample count or tolerances. Four oracle tests
per executable and strict Clippy pass; **the correction is native unverified**. Publish before one
bounded verification; a further failure stops this native chain for another dependency-ready task.
M07 stays **2/4**, overall **4/10 (40%)**. Actual Matrix observations have now been requested; the
prior Composition request remains pending. No response is inferred.
[Evidence](evidence/mapgyver-m07-native-fade-observation-2026-11-01.json).

**Matrix editor/caller batch 78:** started 2026-11-01 **03:51 UTC**, stop **04:36 UTC**.
Only M07 gate 3 is active. Extend the existing immutable authoring observer to include Matrix
documents, verify UI/QuickJS/hosted/command authoring and ephemeral launch semantics, and reuse the
ordinary headless host for a normal review project. Reconcile existing virtualized-grid and
selection/preparation/live frontend tests. Actual editor interaction remains required. No native
Matrix/Composition run, desktop activation retry or new diagnostic framework in this batch.
Software/check/format/build evidence and the saved review scene form the delivery; keep actual
input and every M07 gate-4 requirement open until independently verified.

**Batch 78 result:** four Matrix parity tests cover **31 cases across four callers =124** paths,
including immutable authored snapshots, committed identities, authoring/history, atomic rejection,
ephemeral Cut/configured launch/clear and bounded reads that never launch. The full library passes
**202 tests /one existing hardware ignore**; **12 Matrix frontend tests**, final four-test parity
verification, strict all-target native-feature Clippy, JavaScript syntax and Prettier pass.
The ordinary headless host exits **0** after seven operation/history cases and Save/New/Load;
`target/m07-editor-review/MatrixReview.mapgyver` is initially unlaunched, original scene restored,
SHA-256 `756103070d8615a6d40cb98644d1817726a74ce560aa6272ed68826120540ab9`.
Use [the six-group review](../../apps/mapgyver/tests/headless/MATRIX_REVIEW.md) for actual editor
acceptance. Those observations are unverified; M06's prior request is still pending. **No native
run, actual input or gate closure in this batch. M07 remains 2/4; overall 4/10 (40%)**. Publish this
checkpoint before one named bounded M07 gate-4 verification of the existing correction, retaining
all failures and scope. [Evidence](evidence/mapgyver-m07-editor-parity-2026-11-01.json).

**Corrected native completion batch 77:** started 2026-11-01 **03:41 UTC**, stop **04:26 UTC**.
Only M07 gate 4 is active. Batch 76 made progress with partial rasters/scale and a concrete probe
contract correction; its two failed invocations remain recorded. Source/compiled candidate hashes
still match the published checkpoint. Permit **one** corrected Matrix native verification now;
this is not a reset of the preceding investigation. Any failure stops the Matrix native chain.
If it passes, one separate M06 shared-fixture hardware regression is allowed for the extracted
oracle/ownership code. Record hashes before execution, explicit adapter identities, actual burst
span and normal cleanup. Read NVIDIA overlay state without activation/input; never change it.
Native work ends within 20 minutes and the batch stop. No new probe/clock/framework or relaxation.

**Batch 77 result — native stops, 03:48 UTC:** the frozen `b7387024` candidate source and executable
hashes match the published checkpoint; both test images are x64 and Cargo/npm locks are recorded.
NVIDIA Home's native **Enable and open overlay** action confirms Off before/after through Sky UIA,
with no activation, input or setting change. The one allowed Matrix run exits **101** in **0.02 s**,
before raster capture: resource accounting is admitted but `D3d11Control.snapshot().adapter` is still
`None` during asynchronous initialization. The fixture's added assertion was premature, not a
Matrix render/adapter failure. A requested Intel LUID is visible; no initialized adapter is certified.

The fixture now returns its owned lifetime before identity validation, then requires a matching
actual accepted output frame, initialized native device and exact requested adapter. This uses the
existing readiness boundary rather than a new wait/probe. Both executables compile, four oracle
tests pass in each, strict app all-target/native-qualification Clippy and both workspace formatters/
whitespace checks pass. The corrected candidate has new recorded hashes and **no native verification**.
No Matrix retry, M06 native regression, desktop activation retry, driver change or new diagnostic
mechanism follows. [Before/failure/corrected evidence](evidence/mapgyver-m07-startup-observer-2026-11-01.json).
Three cumulative native failures are preserved. **M07 remains 2/4 (50%); overall 4/10 (40%)**.
Publish this correction, then take gate 3's caller parity and ordinary-app review preparation before
any further native-completion batch. Every actual interaction and native requirement stays required.

**Native Matrix acceptance batch 76:** started 2026-11-01 **03:09 UTC**, stop **03:54 UTC**.
Only M07 gate 4 is active. MapGyver owns Matrix document/playback/production; Golden owns GPU
execution and native consumer/readback lifetimes. Reuse the finite raster oracle/owned renderer
fixture for independent Layer/Group/Final and production-preview images. Cover ordering, group
opacity, all blends, source deletion/Undo/reload, transition interruption and bounded large-grid
runtime/lifetime behavior. Compile/software tests and strict Clippy precede at most two native
invocations/20 minutes. Do not invent another clock/telemetry framework for transitions: if exact
evidence cannot be obtained through existing public contracts, preserve that missing scope and
stop the investigation. No desktop activation retry, driver setting change or M03 qualification.

Software prerequisites pass: both native fixtures compile, all three shared-oracle tests pass in
each integration executable, and strict app all-target/native-qualification Clippy passes.
First native invocation starts **03:21 UTC**, investigation cutoff **03:41 UTC**, at most two
invocations within the original batch stop. Reuse the verified test executable; retain its live
process handle and exact terminal result. No native clock/telemetry extension.

First invocation fails the Intel Group reference after its Layer pixels pass: actual
`[151,215,0,84]`, nominal `[154,217,0,84]`. The reference assumed unique rounding at repeated
half-step UNORM group-opacity conversions. The corrected shared oracle propagates the documented
0.6-ULP integer bounds through every pass, blend, resample and straight-alpha encoding. Its focused
regression includes this value and still rejects a lost contributor; no renderer/budget change.
Four oracle tests in each fixture and strict all-target Clippy pass. The launch stress now uses
**1,000 configured launches on a 10 ms schedule**, including interruptions, matching the Q05 burst
shape. Second/final native invocation starts **03:28 UTC**, still before the original **03:41**
investigation cutoff. No third native invocation or new measurement mechanism is scheduled.

**Batch 76 result — partial, native stops at two invocations:** the second attempt passes **13**
static states across Layer/Group/Final/production preview, **52 ×768 =39,936 texels**, including
four blends, group opacity, reorder/Undo, missing-source/Undo, empty columns, large sparse grid,
burst recovery, reload/device recreation and the initial opaque cut. All residual comparisons
are **0/255** outside the documented per-pass conversion intervals; maximum nominal difference
is **3/255**, retained separately. This run reaches only the first enumerated hardware adapter;
the prior inventory suggests Intel but no per-run LUID was printed, so that identity is unqualified.
The current candidate asserts/logs the actual D3d11Control adapter before capture. The raw execution
binary was not hashed before recompilation; the packet clearly distinguishes its unknown hash
from the current software-checked candidate's hash. No named full hardware qualification is claimed.

The authored grid has **1,000 layers ×1,000 columns /six populated cells**. **1,000 configured
launches** target a 10 ms schedule, including interruptions; **eight** publications consume the
fixed outcome credits, drain, then recover through a Cut and verified pixels. Logical byte,
in-flight/retiring/prepared and outstanding limits remain enforced. The recorded **10,559 ms** span
includes outcome draining/recovery; it is not an exact measured launch cadence. The candidate now
records launch span separately. Three renderer lifetimes retire with empty actual plan/source/
output owner inventories; the failed test exits normally with code **101**, without watchdog/kill.

The transition setup fails with `Spout readiness endpoint, format or bounded image capacity is
unavailable`. Source inspection establishes two fixture errors: the three persistent probes retain
receiving/staging credits, and `SpoutProbe::capture_pixels` explicitly returns a single observed
image. Fresh observations require fresh probes. The correction creates one fresh probe per read,
releasing receiving/staging/CPU credits before the next read; it preserves the **three-credit** cap.
A six-second fade permits finite fresh observations, and final-demand observation has a bounded
deadline. This is not a native-verified correction or a renderer fix. No fade/interruption samples
or second-adapter completion are accepted, and no third native run occurs in batch 76.

The CPU raster oracle and owned GPU fixture are shared under `tests/support`; no duplicated
production renderer or private package imports. Existing M06 hardware evidence is unchanged;
its extracted fixture has software checks but no fresh hardware regression in this batch.
Four oracle tests pass in each integration executable, including the discovered group-opacity
conversion and rejection of a missing contributor. Both final executables compile, strict app
all-target/native-qualification Clippy and both workspace formatters/whitespace checks pass.
[Exact partial/candidate evidence](evidence/mapgyver-m07-native-partial-2026-11-01.json),
[scope and reproduction](../../apps/mapgyver/tests/matrix_pixels/README.md).
**M07 gate 4 stays open: 2/4 (50%); overall 4/10 (40%)**. Publish this bounded checkpoint, then
verify only the concrete fresh-probe correction in a named native-completion batch. Preserve the
full gate, both failed invocations and every M03/M05/M06/M08/M09 return trigger.

**Qualification batch 29 (first diagnostic-only allocation):** started 2026-10-31 **06:31 UTC**;
interaction stop **06:51 UTC**, absolute handoff **07:16 UTC**. Exercise the delivered Matrix UI and
existing module/script/headless authoring boundary: selection versus launch, assignment, settings,
group/reorder, history and observed state. MapGyver owns behavior; the computer-use skill supplies
ordinary owned-window input. At most two finite surface attempts, with no browser/integrity/driver
recovery, no M03 stash, and no restart of batch 28's native correlation investigation. Preserve missing
or failed observations; fix concrete UI defects within this slice. Checkpoint/push before M08.
Twenty-eight product deliveries permit seven diagnostic-only batches; this uses the first allocation.

**Batch 29 result (06:37 UTC):** the ordinary checkout build launches its bundled UI and displays
the Matrix panel, empty-state controls and live backend connection. The available Windows helper
reports **“Accessibility is limited because the target window has higher Windows integrity than the
Computer Use helper.”** One screenshot-grounded Add Matrix click returned without any visible
change or new backend intent. This establishes neither successful input nor a product defect. Actual
creation/selection/assignment/settings/grouping/reorder/history and live-feedback gestures remain
unverified; **M07 gate 3 stays open**. No privilege, browser, security, driver or clock settings changed.

The initial skill launch resolved the installed older MapGyver instead of the supplied checkout
executable; that untouched owned test process was closed. The second/final surface attempt launched
`target/debug/MapGyver.exe` directly as an ordinary process and selected its exact returned window.
Both owned test processes are confirmed stopped; no project was edited or saved. The surface check
ended within seven minutes, below the 20-minute/two-attempt limit, with no recovery investigation.

**Focused verification:** **27 Matrix tests pass** with desktop features and **27 pass headless**;
**12 command/hosted-script tests pass**, including Matrix creation, committed AddLayers callbacks
and Undo/Redo. The common typed backend route, authored operations, bounded reads and transition
tests remain green. This is focused boundary evidence, not complete real-UI/parity acceptance. Both
workspace formatters and whitespace checks run before handoff. Batch 28's failed native assertion/
unverified correction and all physical/native qualification debt remain unchanged.

**No gate closes; M07 remains 2/4 (50%), overall 3/10 (30%).** Product-delivery count stays **28**;
diagnostic-only allocation used becomes **1 of 7**. No consecutive diagnostic batch is scheduled.
The next implementation is M08's public owned-process mechanism and app-owned column demand;
this known input limitation is not the default work queue.

**Product batch 28:** started 2026-10-31 **06:05 UTC**, recorded stop **06:50 UTC**.
Expose observed Matrix preparation/installation/output activity separately from selection and
requested launch. MapGyver owns Matrix-to-plan correlation; Golden Video owns renderer observations;
the Svelte panel presents fenced observations without inferring success from requests. Reuse public
runtime/host observation paths, test stalled/replaced/undemanded/error states and launch correlation,
then run relevant checks and ordinary app build. At most two finite native/interaction checks after
software prerequisites; no diagnostic framework, driver/clock/privilege recovery or parked M03 work.

**Batch 28 native check 1:** started **06:20 UTC**, absolute experiment stop **06:40 UTC**.
Software/build/strict Clippy prerequisites pass. Reuse the finite native Matrix Final/layer fixture:
both observations must correlate installation/submission with the captured launch, and completion
cleanup must report a steady incoming plan. A failure changes only the correlation implementation;
it does not authorize GPU timing, desktop automation recovery or new diagnostic infrastructure.
The first invocation omitted `native-video-smoke` and ran **zero tests**; it supplies no native
evidence. The corrected invocation is the second/final attempt under the same **06:40 UTC** stop.
The enabled fixture reached real native installation/frame submission but **failed** the added
`statuses.len() == 2` assertion: it correctly observes four produced Matrix passes, including shared
intermediates, rather than only the two output routes. The assertion now selects the two routed
references explicitly. It has **not been rerun** under the exhausted attempt allowance; no native
correlation acceptance is claimed, and the following stamp/cleanup assertions remain unverified.
This is retained verification debt, not authorization to start another native/profiling chain.

**Batch 28 result:** the Matrix pane and grid now distinguish requested launch, renderer preparation,
installed content and an observed submitted mix. Captured stamps contain launch, authored revision
and bounded transition participants; preparing alone never marks a mix submitted. Reliable outcomes
authorize installation metadata, and exact active plan/resource/device observations authorize frame
submission, including the current time-authority epoch. Pending metadata shares the runtime's
admission bound; one installed set is retained.
No GPU ownership or document index is pinned for feedback. Fault/recovery hides invalid generations;
rejected preparation preserves previous observed content, and historical runtime errors do not remain
current after recovery. The host reuses coalesced preview status events, excludes frame counters and
redelivers unchanged metadata once per layout registration. A new ephemeral project-instance field
in viewport reads fences stale native events even when persisted Matrix/channel IDs are identical.

**Software verification:** **148 app tests pass/1 pre-existing hardware-only ignore**, **138 headless
tests pass**, strict app all-target/all-feature Clippy and the ordinary bundled app build pass. Four
new renderer tests cover accepted-but-unobserved requests, installation without a matching frame,
previous output during a new launch, stale frame generations/time epochs, fault state, rejected CPU preparation,
recovery, reload with identical persisted IDs and admission-bounded burst cleanup. All **38 frontend
tests pass**, including project/channel/connection fences and distinct pending/previous observations;
Svelte reports **0 errors/0 warnings**, and app Prettier passes. Generated media/preview drift and both
workspace formatters are checked before delivery. The finite native fixture failed its new route-count
assumption as recorded above; its corrected assertion compiles but native correlation/cleanup
assertions have not passed. Actual UI gestures and independent output pixels remain unverified.
Final software checks, DTO drift and both workspace formatters finished by **06:30 UTC**, within
the recorded 45-minute batch. No further native attempt or recovery investigation ran.

**No gate closes: M07 remains 2/4 = 50%; overall remains 3/10 = 30%.** Gate 3 retains real editor
interaction and complete parity acceptance; gate 4 retains native pixels, large-grid and recovery
qualification. Next is one bounded interaction/parity batch, followed by functionally ready M08;
unavailable automation must not become another infrastructure investigation.

**Product batch 27:** started 2026-10-31 **05:53 UTC**, recorded stop **06:38 UTC**.
Complete Matrix editor naming, grouping/reorder and authored settings using backend-owned intents
and bounded target choices. MapGyver owns ordering, destination validation and defaults; Svelte
owns explicit form drafts and selection. Verify typed operations, history/project roundtrip and
stale UI drafts with relevant backend/frontend checks, Svelte/formatting and the ordinary app build.
Retain installed/live feedback and actual interaction as open scope unless directly verified; no
diagnostic-only, driver/clock/privilege or parked M03 work is scheduled.

**Batch 27 result:** the Matrix editor now exposes row/column naming, enabled/opacity/blend settings,
nested group/layer creation, arbitrary group/reorder destinations, canvas dimensions and Matrix/
column transitions. Transparent remains the backend's only admitted empty-cell policy and is shown
as such. Inheriting a transition stays explicit; overriding submits the displayed values. Independent
row/column heading selection makes their forms available with an empty opposite axis and performs
no backend mutation/launch. Source drafts follow the selected authored address; keyed settings forms
reset on authoritative changes instead of preserving an obsolete configuration silently.

`InspectTargets` searches all groups, siblings in a selected parent or columns, returning at most
**128 IDs/labels** per page (UI 64). Group labels include their ancestry, and all reads reuse the
authored-revision index. They do not copy the full document or alter history. One active/one latest
read and session/project/history/scope fences discard stale replies/errors; inspection defers through
shared edit groups. Enter in a destination search searches rather than submitting a move. Existing
Rust intents retain cycle/depth/sibling validation, identity, history and script/command ownership;
the Rust-generated query/result DTOs and script snippets are updated together.

**Verification:** **144 app tests pass/1 pre-existing hardware-only ignore**, **134 headless tests
pass**, strict all-target/all-feature app Clippy and the ordinary bundled app build pass. The new
backend query test covers paging past the visible window, group ancestry, rename/reparent/reorder,
Undo, project roundtrip, rejected query bounds and read-side history invariance. Existing settings/
identity/roundtrip tests cover the reused mutations. All **36 frontend tests pass**, including bounded
destination search coalescing, stale group/project/history/disconnect/disposal fences, independent
selection without launches and shared edit-group deferral. Svelte reports **0 errors/0 warnings**
and app Prettier passes; generated DTO drift and both workspace formatters are checked before delivery.
Implementation/software checks finished by **06:04 UTC**, within the recorded 45-minute limit.
No native/desktop experiment or automation recovery ran.

**No gate closes: M07 remains 2/4 = 50%; overall remains 3/10 = 30%.** Gate 3 retains observed
preparation/installed feedback and actual editor interaction/parity acceptance; gate 4 retains its
complete native/large-grid/recovery qualification. The next product task is runtime feedback, with
all existing qualification budgets and failures preserved.

**Product batch 26:** started 2026-10-31 **05:28 UTC**, recorded stop **06:13 UTC**.
Deliver bounded Matrix viewport queries and an app-registered virtualized grid with separate cell
selection, source assignment and explicit column launch controls. MapGyver owns query/intent policy;
the Svelte panel owns only viewport presentation and transport-fenced focused state. Do not transfer
or mount an eager full grid. Prove bounds, sparse cell addressing, stale-response fencing and
selection-versus-launch behavior with backend/frontend checks, then ordinary app build. Full copy/
reorder/configuration parity and actual interactive/live installation evidence remain gate 3 work
until verified. At most two finite interaction/native checks/20 minutes after software prerequisites
pass; no browser/privilege/driver/clock recovery or parked M03 work is scheduled.

**Batch 26 result:** the registered dockable Matrix panel now provides searched/paged Matrix and
source selection, bounded two-axis scrolling, sparse cell assignment, layer/group/column creation,
row/column copy/removal, explicit configured launch/Cut and the existing native Final preview binding.
Selecting a cell and assigning its source never launch a column. The grid labels requested state
only; installation/live output is not inferred from a successful launch request.

The Rust-generated viewport contract admits **128 rows, 64 columns and 4,096 addresses** maximum;
the UI requests at most **64 by 32 = 2,048** mounted addresses. Source choices return at most 128
IDs/labels per request (UI pages of 64), filtering Matrix selectors to Final channels. A cached sparse
address/order index is reused while the authored revision is unchanged; visible labels/settings/cell
payloads read current nodes. First construction/rebuild still visits the document and is not a
large-grid performance acceptance. Empty addresses remain absent from persistence/history. The UI
retains one active read and one latest replacement, fences stale session/project/history replies and
errors, and resets scrolling when switching Matrices. Transport session interfaces remain the only
connection dependency; all mutations use the existing backend/module/script route.

**Verification:** **143 app tests pass/1 pre-existing hardware-only ignore**, **133 headless tests
pass**, strict app all-target/all-feature Clippy and the ordinary bundled app build pass. All **32
frontend tests pass**, including 1,000 rapid scroll requests, stale replacement/disconnect/disposal,
selection/assignment-versus-launch and compressed 100,000-row/column address geometry. Backend tests
exercise a 1,000-by-1,000 sparse document, cache reuse, window bounds, rename/reorder/Undo/source loss
and project roundtrip without read-side history changes. Svelte checking reports **0 errors/0
warnings**; app Prettier passes. No native/desktop experiment or automation recovery ran. Generated
DTO drift and both workspace formatters are checked before delivery.

**No gate closes: M07 remains 2/4 = 50%; overall remains 3/10 = 30%.** Gate 3 still requires complete
settings/reorder controls, requested/preparation/installed feedback and real interaction/parity
evidence. Gate 4 retains full native pixels, large-grid budgets and recovery. This first usable grid
slice advances the product without reopening M03 qualification or changing any acceptance budget.

**Product batch 25:** started 2026-10-31 **04:56 UTC**, recorded stop **05:41 UTC**.
Repair retained Matrix clip bindings across column removal, cell reassignment and same-column
relaunch. MapGyver owns runtime clip identity and bounded resolved demand; Golden Video retains
GPU/input leases through final use. Start from the preserved failing regression, pin only admitted
binding metadata rather than whole document indices, preserve shared pass/time/burst bounds and
verify removal/reassignment/Undo, interrupted relaunch, completion and project reset. Run relevant
app/shared checks and ordinary build; at most two finite native checks/20 minutes after software
prerequisites pass. No desktop/driver/clock recovery or parked M03 work is scheduled.

Batch 25 software prerequisites: **140 passing / 1 ignored** default app tests, **130/130**
headless tests, strict all-target/all-feature app Clippy and ordinary bundled build pass. The
former ignored column-removal regression now passes normally. New checks cover reassignment and
same-column interruption, source removal/Undo, empty binding epochs, project reset, a fixed 4,096
binding ceiling/recovery, obsolete-index release and retained-edge feedback rejection.
Native experiment 1 starts **05:17 UTC**, ends no later than **05:37 UTC** (two runs/20 minutes
maximum for this chain). Hypothesis: removing the outgoing column and editing/relaunching the same
incoming column preserve all three native sources, then one cleanup generation installs/submits
and retires within the existing budgets. No additional pixel/timing/desktop experiment is scheduled.

**Batch 25 result:** the outgoing-column regression is repaired and enabled normally. Clip bindings
now retain only source references and empty decisions for admitted row/clip pairs, with a fixed
**4,096-entry ceiling per Matrix**. Source identity edits have authoring epochs independent of
runtime launches; a same-column relaunch after edits gets a distinct clip identity, while unchanged
column/epoch contributions can coalesce under the existing **32-clip ceiling**. First-demand binding
rejects cells changed/created after that clip epoch until relaunch, avoiding eager empty-row scans
or historical document snapshots. A weak-index regression proves the obsolete authored index is
released while the outgoing source reference remains alive. Removal/reassignment/Undo cannot rewrite
an already playing binding. Media content stays live through channel references; missing sources
remain unavailable and recover through the existing source path. Runtime binding state never persists.

All-media validation merges retained runtime edges **after** prospective authored replacements.
An atomic cell batch cannot erase a still-used edge to introduce feedback; the rejection leaves
history unchanged and Cut/materialization releases it. Completion clears obsolete binding metadata
and source demand; Golden's installed/retiring generations retain actual GPU/input leases until
their final token completes. No extra native ownership or per-frame actor work was introduced.

Native experiment 1 passed in **0.71 s** after terminal prerequisites: the fixture removes the
outgoing column, reassigns incoming cells and relaunches that same column during a fade. Both prior
sources and the new source remain in native demand, Final/layer targets stay stable, exactly one
cleanup publication installs and submits a new generation, all byte/in-flight/retirement bounds
hold and shutdown retirement completes. This strengthens batch 24's admitted-only cleanup check;
it still does not certify independent Spout consumption, full Matrix pixels, hotplug or Q05 scale.
No second native experiment or automation/driver/clock recovery ran.

Acceptance: existing atomic receipts, shared-time/empty-cell and blend/opacity checks, independent
shared-pass tests, 1,000 configured launches under fixed publication credits and the repaired binding/
feedback/Undo/reset/native final-use checks close **M07 gate 2**. **M07 is 2/4 = 50%; overall stays
3/10 = 30%** because no whole phase closes. The fixed denominator remains four; grid/input parity
and full native/interactive/large-grid qualification retain gates 3/4.

Final validation: **140 passing / 1 pre-existing hardware ignore** default app tests, **130/130**
headless tests, generated DTO drift, strict all-target/all-feature app Clippy, both root/Golden Core
formatting and the ordinary bundled build pass. The binding-retention regression is enabled and
passes. Shared Golden/runtime code and frontend files are unchanged; their batch 24 checks remain
the baseline. The single native check above passes at its stated scope; no full qualification is
inferred. This milestone is delivered before the recorded 05:41 UTC stop.

**Product batch 24:** started 2026-10-31 **04:29 UTC**, recorded stop **05:14 UTC**.
Deliver configured timed Matrix launches with deterministic shared-time fade/interruption state and
outgoing source retention. MapGyver owns transition policy and demand; Golden Video owns reusable
frame-time layer evaluation on its render worker. Do not publish per-frame plans or poll the engine.
Prove start/midpoint/end, interruption, source sharing/release and bounded bursts with fake-time/
reference checks, then app/shared regression checks and ordinary build. At most two finite native
experiments/20 minutes after prerequisites pass. No desktop/driver/clock recovery. If the primitive
needs a coherent intermediate checkpoint, preserve full M07 gate 2 scope and record its limits.

Batch 24 software prerequisites: **132 passing / 1 ignored** app tests, **122/122** headless,
Golden Video all-feature **156 passing / 20 ignored** unit tests and four integration tests,
strict all-target/all-feature app/Video Clippy, generated DTO drift, frontend **26/26**, Svelte
**0/0**, Chataigne compile and ordinary bundled app build pass. An initial Chataigne command
used `--lib` although the package has no library; the corrected package check passed.
Native experiment 1 starts **04:44 UTC** (no later than **05:04 UTC**, at most two experiments
in this chain). Hypothesis: the actual D3D11 backend evaluates the shared-time factor per submission,
giving independently calculated premultiplied pixels at Linear/SmoothStep samples and preserving
post-group opacity with one unchanged plan/allocation set. A pixel or allocation mismatch changes
the implementation; it does not authorize another timing/profiling investigation.

Experiment 1 passed in **0.37 s**: both curves at 0/250/500/750/1000/1500 ms, all pixels of
both the premultiplied mix and post-group-opacity target within two u8 levels of an independent
scalar oracle; created-texture count stays fixed and there are no external input subscriptions.
Experiment 2 hypothesis: the actual MapGyver renderer retains all three sources across a timed
interruption, shares Final/layer outputs with stable targets, publishes one completion cleanup
and retires within the existing byte/in-flight/generation limits. This checks runtime submission/
lifetime, not independently consumed app pixels, UI gestures or sustained Q05 budgets.

**Batch 24 result:** configured launches now resolve the column override/default and atomically
publish that transition with the desired column. One Matrix timeline samples positive outgoing
weights on interruption and fades that mix to the new column. Row sources shared across columns
merge before capture; rows use a normalized premultiplied sum, then group/final ordering and opacity
use the existing shared graph. Empty incoming cells fade to transparent. All exports use one
Golden Time capture anchor, and frame evaluation uses immutable layer envelopes on the native
render worker, with no frame-driven engine edits, actor capture or plan publication. Show-position/
rate changes do not restart monotonic fades; authority replacement settles to the target. The
first timed capture binds a launch; readiness or late installation does not rebase that anchor.

Existing publication credits, raw-input accounting, channel/edge/byte/generation budgets remain
unchanged. The new explicit runtime limit is **32 retained column contributions** per Matrix;
overflow rejects a distinct configured request without changing desired state, while Cut/relaunch
of a retained column and completion recover admission. A 1,000-configured-launch test admits exactly
the existing 16 test-runtime credits, keeps authored history unchanged and readmits after reliable
outcome drain. No per-frame refresh publishes during the fade; one endpoint cleanup prunes outgoing
source demand. GPU/input leases remain owned by installed/retiring generations through final use.

Experiment 2 passed in **0.71 s**: the actual app renderer submits frames for Cut, configured fade
and a three-source interruption with two native output bindings, stable Final/layer target IDs and
all outgoing sources in captured demand. One endpoint cleanup request is admitted, removed source
channels disappear from desired demand, byte/in-flight/retirement counters remain bounded and
shutdown retirement completes. This does not prove independent Spout consumption or live installation
of the cleanup generation. Both permitted native experiments are complete; no third run or desktop/
driver/clock recovery is scheduled.

Review identified a real remaining lifetime case. The explicit regression
`outgoing_cell_bindings_survive_column_removal_until_transition_end` **fails**: after launching A→B,
removing A's column withdraws source A at 500 ms of a 1,000 ms fade. Capture currently resolves
outgoing contributions from the edited live index. Preserve bounded resolved bindings/clip identity
through removal/reassignment and same-column relaunch before closing gate 2; do not reinterpret this
as accepted live-edit policy, copy whole indices per contribution or reopen native qualification.
The regression stays explicitly ignored with that gate's reason until repaired, and is not counted
as passing acceptance. **M07 remains 1/4 (25%); no gate closes.** The virtualized grid and full native/
interactive/large-grid qualification retain gates 3/4.
Final validation: **134 passing / 2 ignored** default app tests and **124 passing / 1 ignored**
headless tests. One ignored test is the explicitly reproduced binding-retention failure above;
the other default-only ignore is the pre-existing hardware check. Golden Video all-feature tests
pass **156 / 20 ignored**, plus four integration tests; the envelope hardware test also passes
explicitly as recorded above. Strict all-target/all-feature app/Video Clippy, generated DTO drift,
frontend **26/26**, Svelte **0 errors / 0 warnings**, app Prettier, Chataigne compile, both root/
Golden Core formatting and the ordinary bundled MapGyver build pass. No renderer pixel/budget
claim is inferred from the ignored regression or narrow smoke. This reviewable timed-runtime
checkpoint is delivered before the recorded 05:14 UTC stop; retained bindings are the next product
repair, not another measurement task.

**Product batch 23:** started 2026-10-31 **03:58 UTC**, recorded stop **04:43 UTC**.
Deliver atomic Matrix column launches and demanded layer/group/final execution through the existing
renderer and Golden Time boundary. Own launch/empty-cell/transition policy in MapGyver; keep frame
evaluation off the engine and UI. Preserve outgoing source demand through interrupted transitions
and keep submissions, retained state and resource admission bounded. Start with fake-time/reference
checks and shared-source/output deduplication, then the ordinary build. If the full transition
mechanism needs a separate shared primitive, checkpoint a coherent working launch/producer slice
with the complete remaining transition acceptance explicit; do not invent native qualification
prerequisites. At most two finite native experiments/20 minutes after software prerequisites pass.
No desktop/driver/clock recovery is scheduled, and M07 gate 2 stays open until its full scope passes.

Batch 23 scope decision: Golden currently has static layer opacity, not frame-time interpolation.
Deliver explicit Cut launches and shared Matrix production first; configured nonzero transitions
must reject rather than silently cut or resubmit plans every frame. Timed fades/interruption and
outgoing retention remain the next required slice, with the full gate 2 acceptance unchanged.
Use existing edit-completion receipts to keep ephemeral launch state invisible until commit and
discard failed requests; no new Golden Core callback/journal mechanism is needed.

**Batch 23 result:** explicit `LaunchColumn { mode: Cut }` now selects one column atomically for
all derived exports. A pending edit receipt cannot publish state; failed/discarded requests preserve
the preceding committed launch. Requests keep one latest ephemeral slot, add no authored history
and reset on reload. `Configured` accepts a zero-duration column override but rejects nonzero fades;
the existing 250 ms default is not silently converted to a cut. Foreign-column ownership rejects.
One dedicated playback event and small `InspectPlayback` response avoid catalog/document refreshes
on each launch. Existing commands/scripts/callbacks and updated snippets use the same generated DTOs.

The Final owner caches a sparse immutable authored index. Cuts reuse it; demand walks populated
immediate children and skips already represented group subtrees. Element changes/structure/enable,
deletion and Undo invalidate cached captures. The first capture after an authored revision rebuilds
the document index; its full large-grid cost remains qualification work. Matrix and Composition
share dependency ordering and one pass per demanded channel. Independent Final/Group/Layer consumers
share leaf sources. Groups apply opacity once after compositing their ordered children via a bounded
scratch pass when needed. Empty/unlaunched/deleted columns render transparent and demand no sources.
Scratch/edge admission preserves existing limits and can withdraw the affected producer while a
healthy output continues. Preview/Screen pass overhead is reserved before admitting scratch targets.

Media Manager now exposes bounded searchable explicit Cut buttons and committed requested-column
feedback, distinct from renderer installation; selecting or assigning a source never launches it.
The virtualized grid, preparation/live indicators, shared-time fades/interruption/outgoing retention
and full native/interactive/large-grid checks remain required. **No gate closes; M07 stays 1/4.**
No native/desktop experiment or recovery investigation ran in this batch.
Validation: **128 passing / 1 ignored** default Rust tests and **118/118** headless tests;
generated protocol drift checks, strict all-target app Clippy, **26/26** frontend tests, Svelte
**0 errors / 0 warnings**, app Prettier, both root/Golden Core formatting and the ordinary bundled
MapGyver build pass. Tests include 1,000 launches without authored history/data changes, pending/
discard isolation, zero-duration override and foreign-column rejection, shared-pass/post-group-alpha
plan structure, scratch admission beside a healthy output, empty columns, mute/delete/Undo and index
Arc reuse across cuts. Plan compilation/null execution does not verify native blend/alpha pixels.
The build and checks finished before the recorded 04:43 UTC stop; no investigation was extended.

Before M07 source implementation, its provisional whole-phase item is split into four fixed gates.
Together these retain the complete plan acceptance. All start open; M07 progress is closed gates / 4.
Overall remains 3/10 (30%); no existing gate, budget or qualification ownership changes.

| # | Acceptance scope | State |
| --- | --- | --- |
| 1 | Stable Matrix/layer/group/column/cell and exported channel identities; grouping, source assignment, create/copy/reorder/remove, validation including all-media feedback, project roundtrip and Undo/Redo; command/script/callback/snippet parity for the authored model. | Accepted — batch 22 evidence below |
| 2 | Atomic column launches and explicit empty-cell policy use shared time; deterministic layer/group/final blending and interrupted transitions preserve outgoing source demand until final use; independent consumers share passes and bursts cannot accumulate unbounded plans. | Accepted — batches 23–25 evidence above |
| 3 | App-registered virtualized Matrix editor with source assignment, authoring/copy/reorder and separate selection/preparation/live state; complete UI/script/headless intent parity and real interactive evidence. | Open — caller/full browser input and batch94 strict project lifecycle pass; actual native preparation/installation/submission feedback remains |
| 4 | Native preview and independently consumed layer/group/final output pixels, ordering/blends/alpha/transition interruptions, source loss/Undo/recovery and bounded large-grid runtime/lifetime behavior satisfy the retained qualification requirements. | Accepted — batch 82 completes both adapters, native content/interruption/endpoints, sparse-grid bounded-runtime and owner-retirement checks; global M03/M05/M09 qualification remains with its original owners |

**Product batch 22:** started 2026-10-31 **03:15 UTC**, recorded stop **04:00 UTC**.
Deliver Matrix media and the document/module foundation under `apps/mapgyver/src/matrix`: sparse
node-owned groups/layers/columns/cells, stable derived channel identities, atomic batched authoring,
source assignment, persistence/history, shared all-media dependency validation and existing media
command/script/callback/snippet integration. Add creation/inspection/canvas entry points in the app
UI. Avoid eager layer×column materialization and repeated whole-tree snapshots for a known batch.
Prove identity/order/copy/ownership/feedback/Undo/roundtrip and finite large sparse construction;
run DTO/frontend/Rust checks and bundled build. Matrix playback remains truthfully unavailable
until its producer is implemented. No native experiment or desktop/driver recovery is scheduled.

**Batch 22 result:** Matrix is now an app-owned media type with one Final document channel,
node-owned ordered groups/layers/columns and sparse cell nodes. Layer/group exports have independent
stable channel IDs; grouping, reorder, rename, source changes, history and roundtrip preserve them.
Backend operations create forests, configure settings, move/reorder/rename/remove, duplicate groups
or columns and assign 1–4,096 cells atomically. Group copy remaps internal exports while preserving
external sources; column copy allocates independent column/cell IDs. Clearing a cell retains its
identity; deleting a source preserves references and Undo restores it. Item/column removal cleans
owned cells/exports in the same transaction. Adding columns never creates a layer×column grid.

One iterative media DAG now covers Composition instances, all authored Matrix cells and derived
exports. It rejects direct/indirect and Matrix↔Composition/Matrix feedback, including inactive
columns. Batched assignments replace affected edges together instead of retaining removed edges.
Detached project validation rejects malformed ownership, cell addresses, orphan exports and cycles.
The roundtrip fixture caught a redundant enum tag being omitted by sparse persistence; element
codecs now use the registered node kind and serialize only its payload. A proposed 64-level tree
exceeded the shared JSON nesting guard. Matrix admits **32 item levels**, verified at that boundary,
**100,000 document elements** and **4,096 additions/assignments per batch**. These are explicit
authoring admission limits; no retained native budget/workload was reduced.

Final review reproduced generic outliner removal leaving a paired export behind. Golden's new
public, default-accepting `ProjectLifecycle::validate_ui_intent` hook checks app policy before any
mutation/history boundary. MapGyver requires typed Matrix operations for generic structural edits
to Matrix-owned nodes; mixed removals, moves, creation and copy requests reject atomically. Golden
contains no Matrix policy, and the bare engine remains a trusted internal mechanism. The regression
test failed before the guard and passes after it; no new diagnostic framework or native run was used.

Create Matrix is exposed in Media Manager and command presets. Its focused settings/inspection UI
supports canvas/transition configuration, root group/layer/column creation and source assignment;
bounded selectors/readback show authoritative data. Generic commands, hosted `editMedia`, committed
callbacks and app snippets use the same generated DTOs. The Final/Layer/Group producer remains
explicitly unavailable and a renderer regression keeps a healthy Solid output active alongside it.

Validation: **124 passing / 1 ignored** default tests, **114/114** headless tests, generated DTO
drift, **26/26** frontend tests, Svelte **0 errors / 0 warnings**, strict app/engine Clippy,
root/Golden Core formatting, shared engine regression tests (**659 passing / 3 ignored**), Chataigne compile regression and the
bundled ordinary MapGyver build pass. The finite 1,000-layer/1,000-column fixture uses two forests/
history steps, 1,001 exports and **zero cells**; one passing observation was **233.3517 ms**. This is
construction evidence, not Q04 10k/50k/100k or interaction/frame/memory budget qualification.
No native/UI experiment ran. **M07 gate 1 closes: 1/4 (25%)**. Gates 2–4, M06 gates 2–4 and all
other retained qualification remain open; overall stays **3/10 (30%)**. The next product deliverable
is atomic Matrix launches/transitions and demanded shared render passes, not more UI recovery.

### M06 acceptance items

**Batch95 result:** first/only invocation passes **exit0 /7,150 ms**, zero rejected control receipts,
missing-media messages or page errors. A real pivot drag is accepted at `[220.000015,160.000015]`
before actual Ctrl+N while the mouse is held. New clears selection/history/media; later held-pointer
motion/release cannot author into the new scene. Fresh Composition identities and canvas Apply/
Undo/Redo work. Actual File→Open→Load From uploads the unchanged saved review under a UUID filename,
preserving all three instance references. Loaded selection/name follows the saved scene; rename/
Undo/Redo stays in that project, and actual menu New clears it again. Inspected final empty editor
is usable without a loading overlay or stale instance. Original file hash remains unchanged.

The existing receipt observer is reused in a small shared test helper; Matrix's passing test body
is unchanged. Syntax/Prettier and both workspace formatters pass. No production/runtime change,
second app invocation, native image/extent injection or physical claim. **M06 stays3/4**, overall
**4/10 (40%)**; actual native imagery/extent-based outline/shape selection remains required. Publish,
then select M08 gate4's existing actual column association/distance/history/runtime UI integration;
do not repeat definition forms or the broad passing Composition/Matrix candidates.

**Composition project lifetime batch95:** 2026-11-01 **06:28 UTC**, stop **07:13 UTC**;
investigation cutoff **06:48 UTC**, at most two finite app invocations. Only M06 gate4 is active.
Previous turn is progress: `3589786f` fixes exact hidden Media Manager stale queries and passes
strict Matrix New/upload. Target remaining original Composition lifetime: New during an accepted
drag, held-pointer disposal, fresh identities/history/controls and actual saved-review upload.
Reuse existing browser/menu/receipt observation and unchanged review; no broad input rerun, fake
preview/extent, native automation retry or acceptance revision. Native imagery/outlines stay open.

**Batch89 result, 05:41 UTC:** first run exits1 at Scale after passing full form, ordering, copy/
removal, Translate and Rotate with complete history. Static review finds the test can observe a
backend history commit before the selected form renders it. Synchronize the form's position/
scale/rotation with that committed transform before taking real handle coordinates; retain the
same0.08 scale tolerance and include transform diagnostics on failure. The second/final run passes
the complete candidate, **exit0 /7,332 ms**, zero page errors. Exact source/channel identity,
mirrored/nonuniform scale, pivot, blend/opacity/Enabled, independent peers, all three grouped
gestures, Escape, invalid form with no history consumption, missing source/Undo and panel switching
are verified. No authored mutation uses a backend request. Cumulative results are three failures
and one pass; there is no additional app run.

Inspected capture confirms the dock-width fix: the canvas occupies the narrow panel width, with
forms stacked beneath. Preview remains explicitly unavailable; source outlines count0. The capture
also retains the expected invalid-scale alert. Native source-extent/outline/image integration,
project replacement and unobserved complete-editor checklist items stay required. M06 gate4 does
not close, **3/4**, overall **4/10 (40%)**. Return only for those gaps, not another passing full
candidate. After publication, select M07 gate3's existing grid/control/history/selection/live-state
acceptance through the already working browser route; do not reopen CDP/desktop work.

**Corrective verification batch89:** 2026-11-01 **05:39 UTC**, stop **06:24 UTC**; investigation
cutoff **05:59 UTC**. Only M06 gate4 is active. Batch88 delivered a specific dock-width layout
correction and exact f32 test expectations, rebuilt in `57a0cdc4`; those concrete changes reopen
verification, not the parked automation investigation. Cumulative previous failures remain two.
Run the existing corrected full browser flow and inspect its real capture. The first corrective
run passes forms/order/copy/removal/Translate/Rotate history but fails Scale; its capture confirms
the dock layout correction. One final comparison synchronizes the selected form's rendered f32
values with the committed state before pointer-down, retaining the same scale tolerance and
reporting actual transforms on failure. No further run in this batch; cumulative total at most4.
Verify the
remaining original checklist against the source/evidence before any gate closure; native imagery,
extent outlines and physical integration remain required. No new automation route or budget change.

**Actual Composition input batch88:** started 2026-11-01 **05:20 UTC**, stop **06:05 UTC**. Only
M06 gate4 is active. The workflow-only turn is no progress; `68d308e2` verifies Source controls/history and
retains native preview gate7. MapGyver owns Composition/source/instance/geometry intent semantics;
Golden owns transport/history/host preview. Reuse naturally enabled bundled-browser controls for
source selection, instance forms/order/copy/removal, translation/scale/rotation and grouped history,
cancellation/panel lifetime and missing-source behavior. Backend queries only observe. Max two
finite invocations/20 minutes investigation including baseline. Do not inject native metadata,
fake a preview/outline or infer physical/native integration. Any unavailable complete-editor scope
stays open; no guard bypass, CDP/desktop retry, driver/reset/settings or acceptance change.

**Batch88 result:** exactly two invocations exit1 at the complete-form stage. First, exact label
matching misses the wrapped Blend select; the second submits the definition but compares its f32
values with unrounded JavaScript decimals. The test now uses exact `Math.fround` expected values,
not a looser tolerance. No third app run occurs. Reached assertions prove canvas resizing, visible
atomic cycle rejection/history, two source admissions and non-authoring selection/drafts. Later
order/copy/remove, gestures, Escape, missing source and panel checks have not run. Both hosts exit0
normally; no MapGyver remains. The complete candidate is not a passing acceptance check.

The inspected failure capture also shows a narrow docked panel squeezing the canvas beside a
15rem form column because its media query uses the much wider browser viewport. Composition now
queries its own container width, following Media Manager/SurfaceWorkbench. Svelte reports zero
errors/warnings; syntax/Prettier, ordinary app rebuild and both workspace formatters pass. Layout
and full input require runtime verification. **M06 remains3/4; overall4/10 (40%)**. Preserve the two
failed invocations cumulatively; the specific assertion/layout corrections, not a new batch name,
are the basis for targeted follow-up. Native imagery/extents/physical integration and all original
checklist gaps stay required. No budgets, protocol or domain semantics change.

**Native editor connection batch 84:** started 2026-11-01 **04:41 UTC**, stop **05:26 UTC**;
connection investigation cutoff **05:01 UTC**, one owned-app launch only. Previous turn is
progress: `42013b45` verifies M08 aggregate outcomes and preserves its retirement/elevation/input
gaps. Only M06 gate 4 is active. MapGyver owns editor behavior; Golden owns native host/preview.
Use the existing Playwright dependency with its documented WebView2 CDP route, private app/WebView
profiles, two loopback ports and the existing shutdown-file host option. Hypothesis: this supported
browser route can observe and drive the actual bundled native editor despite unavailable desktop
activation. Prove connection and one real Composition panel selection without changing domain
state; if successful, continue the existing live session for the required editor observations.
If unavailable, record the exact prerequisite and stop, without a helper/registry/security change
or second launch. No GPU-performance, physical-DPI or input-alignment pass is inferred from CDP.

**Batch 84 result, 04:43 UTC:** the persistent JS sandbox lacks `process`, so its import fails before
any native launch. The normal Node test then performs the one actual launch. Native host listens
on **127.0.0.1:57814**, the bundled WebView frontend connects and receives its initial snapshot,
but the requested CDP endpoint never appears in **20 s**. The candidate test exits **1** without
connecting Playwright or clicking Composition. Shutdown-file cleanup obtains native exit0; no
MapGyver remains and isolated app/WebView data is removed. Native PID/CDP port were not emitted;
do not guess them or a runtime cause. Syntax/Prettier pass; this is unavailable-route evidence,
not a passing UI test. No second launch, alternate flags, helper or registry/security change.
**M06 remains3/4**, overall **4/10 (40%)**, actual Composition/Matrix observations still pending.
Publish, then select M08 gate4's actual application form editing/history via the already working
ordinary bundled-browser route, without native dialog/preview/OS-pointer/presentation claims.
[Evidence](evidence/mapgyver-m06-webview-cdp-unavailable-2026-11-01.json).

**Editor acceptance batch 75:** started 2026-11-01 **02:55 UTC**, stop **03:40 UTC**.
Only M06 gate 4 is active. MapGyver owns Composition editor/source-picker and domain intents;
Golden owns shared history, scripting, transport and project codecs. Reconcile existing UI and
caller-parity checks, add only missing Composition operation/geometry/history coverage, and prepare
a validated ordinary-app review project plus a short finite gesture checklist. Use current public
project APIs and existing host fixtures; no alternate desktop framework or activation retry.
Actual user/editor input remains required and cannot be inferred from software tests. At most
20 minutes/two investigation experiments; a review-ready partial result keeps the gate open.

**Batch 75 result — caller parity and review ready, 03:05 UTC:** the existing immutable Composition
query is exported through `media::composition::inspect`, matching the media channel read boundary.
The parity observer includes actual Composition snapshots without settling queued script requests.
Across UI intent, QuickJS generic operation, hosted `local.editMedia` and Media Operation command
nodes, **60** cases verify committed responses/allocated identities, source assignment, geometry,
rename/reorder/removal, cycle/invalid/wrong-owner rejection and exact Undo/Redo without callback replay.
The initial observer attempted a UI inspection intent and settled queued script work before taking
its history baseline; using the immutable query fixes that fixture defect, with no mutation behavior
change. Runtime and test modules remain separate; no duplicate protocol or UI geometry policy.

The existing isolated ordinary headless host runs
`node apps/mapgyver/tests/headless/media.mjs --composition-review` successfully. **Thirty** absolute
geometry samples form three Translate/Rotate/Scale gestures, each restored by one Undo/Redo while
shared peers remain unchanged. Source assignment, ordering, rename/copy/remove, cycle rejection and
source deletion/Undo pass, followed by Save/New/Load of the original review scene. The host exits
**0** normally and no fixture-owned MapGyver process remains. The saved project has a **320×240**
Composition with two Red instances and one Green; its SHA-256 is
`d4861eb6ace9c6ad24bacf5e19b3aed1292b6e370340629afd17d04369122ed8`.
[Evidence and source hashes](evidence/mapgyver-m06-editor-parity-2026-11-01.json),
[reproduction and six actual-editor checks](../../apps/mapgyver/tests/headless/COMPOSITION_REVIEW.md).

Validation: full app **198 pass /one existing hardware ignore**; Composition frontend **10/10**;
strict all-target/native-qualification Clippy, ordinary app build, JS syntax/Prettier, both workspace
formatters and whitespace checks pass. No desktop activation retry or native renderer experiment.
Actual editor observations have been requested and remain pending, not unavailable by assumption.
**Gate 4 stays open; M06 3/4 (75%), overall 4/10 (40%)**. Review/parity preparation is complete;
return on actual observations or changed supported activation, not another software-only fixture batch.

**Native-pixel acceptance batch 74:** started 2026-11-01 **02:37 UTC**, stop **03:22 UTC**.
Only M06 gate 3 is active. Reuse app-owned ChannelRenderer/Composition intents, Golden's native
Spout readback and existing preview-layout contract. Add a finite pixel acceptance fixture for
nested/shared sources, independent transforms/pivots, canvas resize, alpha/blends and missing
source/Undo. Use independent reference pixels, verify shared pass/resource identity and retire all
owned renderers/probes. Software compile/tests and strict Clippy must pass before at most two
native invocations/20 minutes; do not retry desktop activation or change system/GPU settings.
Preview evidence must exercise its real composition route; mere output delivery cannot close it.
If complete preview/output coverage is unavailable, preserve that exact scope and keep gate 3 open.

Software prerequisites pass: two independent-oracle tests, native fixture compilation and strict
fixture Clippy. First native invocation starts **02:44 UTC**, cutoff **03:04 UTC**, at most two
invocations. Its watchdog is 120 seconds; verify the existing live handle rather than restarting
on an observation timeout. No system setting change, desktop input or M03 qualification claim.

The first invocation passes all nine Intel cases and NVIDIA through transformed channel pixels,
then rejects NVIDIA's resized preview texel 391: actual `[0,224,0,255]`, nominal CPU
`[13,225,13,255]`. Investigation identifies the oracle's assumed unique intermediate UNORM
rounding. [D3D11 specification §3.2.3.6](https://microsoft.github.io/DirectX-Specs/d3d/archive/D3D11_3_FunctionalSpec.htm#3.2.3.6%20FLOAT%20-%3E%20UNORM)
allows integers within 0.6 ULP; this pixel's final linear preview conversion can legally choose
zero or one before sRGB encoding. The corrected oracle bounds that specific conversion before
encoding, preserving the two-byte residual allowance and all original plan budgets. A software
regression verifies the exact `[0,224,0,255]`–`[13,225,13,255]` legal interval and rejects unrelated
extra rounding. No renderer change or proven renderer defect. Three oracle tests now pass;
only one native retry remains, still within the original **03:04 UTC** investigation cutoff.

**Batch 74 result — gate 3 accepted, 02:51 UTC:** the second/final invocation passes in **44.67 s**.
Both **Intel LUID 00000000:00016c2a** and **NVIDIA LUID 00000000:000170d3** complete nine states:
Normal/Add/Multiply/Screen, changed independent transform, live 32×24→64×48 canvas resize, shared
source deletion, Undo and reload/fresh device. Each state independently reads two shared channel
outputs and the production native-preview compositor: **54 captures ×768 =41,472 RGBA texels**.
Every residual comparison is within **1/255** of the legally quantized reference interval, below
the fixed two-byte residual allowance. The raw nominal **13/255** near-black NVIDIA differences
remain recorded; they are not described as two-byte nominal matches. The first failed oracle
run and exact correction are preserved in the [compact packet](evidence/mapgyver-m06-composition-pixels-2026-11-01.json).

The test uses ordinary app intents and public ChannelRenderer, real PreviewController negotiation/
layout resolution and production PreviewLayout rendering with the same linear premultiplied RGBA8
descriptor as the ordinary host. The preview compositor's GPU image is delivered through a Spout
endpoint for bounded independent SDK/device readback. Five unique shared channel targets feed all
three consumers; existing plan tests verify the same deduplicated nested/shared pass construction.
Independent inverse-affine CPU sampling validates asymmetric rotated/mirrored/off-center-pivot
instances and premultiplied linear-light blending. This accepts **raster content**, not actual
on-screen gestures, window geometry or scanout; those remain M06 gate 4 and M05/M03 qualification.

All **four** renderer lifetimes retire, with created/released plan counts balanced, zero live plan
objects and empty source/output owners. Every readback pool returns idle; the test process is
absent after normal exit. No driver allocation-growth, cadence or M03 budget claim follows.
Three software oracle tests, strict app all-target/native-qualification Clippy, both formatters
and whitespace checks pass. [Fixture boundary/reproduction](../../apps/mapgyver/tests/composition_pixels/README.md).
Only M06 gate 3 closes: **3/4 (75%)**, overall **4/10 (40%)**, denominators/budgets unchanged.
Next is the real Composition editor/caller-parity gate; do not reopen the activation investigation.

**Dependency acceptance batch 73:** started 2026-11-01 **02:30 UTC**, stop **03:15 UTC**.
Only M06 gate 2 is active. App-owned media dependency validation and Composition/Matrix authoring
own this gate. Reconcile the existing cycle/deletion/history/scale tests, then cover only missing
all-media and deep/large dependency cases through those contracts. Verify rejected edits preserve
the whole document/history and missing references preserve identity through Undo. Large fixtures
must use detached batched construction or the existing pure graph boundary, not sequential UI
edits/full snapshots. Run focused acceptance, app regressions, strict app Clippy and both workspace
formatters. No GPU/UI/helper recovery or new qualification infrastructure. Investigation retains
the cumulative 20-minute/two-experiment limit; unresolved scope stays open at the stop.

**Batch 73 result — gate 2 accepted:** four focused regressions complete the existing dependency
coverage. [Dependency tests](../../apps/mapgyver/src/media/dependencies/tests/mod.rs) verify all
five source kinds (Solid Color, Picture, Shared Texture, Composition and Matrix) preserve the two
referencing instances through removal/Undo/Redo, restore the exact source and keep missing leaves
valid. A detached Media Manager forest constructs **4,096 nested compositions /16,384 composition
nodes**, sharing one leaf. Whole-project validation succeeds; adding the back edge returns the
specific dependency-cycle error with byte-identical persisted document and unchanged history.
Reload retains node count/identity and passes project validation. The focused forest insertion plus
validation takes **129.7883 ms** in this control; this is diagnostic timing, not a new budget.
A separate **100,000-node** chain/wide graph admits unresolved leaves and repeated references,
rejects a closing cycle and completes on a **128 KiB** thread stack, without recursive traversal.

The [opposite Matrix feedback test](../../apps/mapgyver/src/matrix/tests/validation.rs) rejects an
entire two-cell batch, including its otherwise-valid cell, without document/history changes.
Existing Composition tests cover direct/indirect cycles, wrong ownership, source deletion and
1,000-instance atomic history; Matrix validation covers Matrix/Matrix feedback, all leaf types,
loaded invalid feedback and bounded grouping depth. Existing rendering transition tests retain
outgoing runtime edges when validating atomic source replacement. Together these close gate 2's
dependency scope; native deletion/resize/transform pixels and actual editor behavior stay gates 3/4.

Final validation: `cargo test --locked -p MapGyver --lib` **196 pass /one existing hardware ignore**;
`cargo clippy --locked -p MapGyver --all-targets --features native-qualification -- -D warnings`,
both workspace formatters and whitespace checks pass. No production API or runtime behavior changes.
M06 is **2/4 (50%)**, overall **4/10 (40%)**; no acceptance denominator/budget changes. Next is one
bounded M06 gate 3 native-pixel batch; M05 activation and every M03 gate stay in the return queue.

**Product batch 21:** started 2026-10-31 **02:59 UTC**, recorded stop **03:44 UTC**.
Deliver graphical Composition instance selection and drag/rotate/scale through typed backend
geometry intents, one in-flight/latest-sample stream and coalesced grouped history. MapGyver owns
geometry and authoring policy; Svelte converts viewport input and renders presentation handles over
the existing native preview. Verify geometry/reference invariants, preserved unrelated fields,
history/protocol/script parity, UI fencing/cancellation and the bundled app. At most two finite
native/UI experiments after software prerequisites pass; stop investigation at 20 minutes or the
batch cutoff. Do not reopen parked M03 work or browser/driver recovery.

Batch 21 native/UI experiment 1 starts **03:09 UTC**, stops by **03:29 UTC** (and within the batch
cutoff). Software prerequisites passed: 109 default tests/1 ignored, 99 headless tests, 26 frontend
tests, 213 Golden UI tests, Svelte 0 errors/warnings, strict app Clippy and the bundled binary build.
Hypothesis: the owned ordinary app exposes the Composition panel and its source/instance controls;
graphical move/rotate/scale update authored values and preview, and Undo restores each gesture.
Use only supported computer-use APIs on the owned app window. An unavailable desktop surface is
a recorded limit, not authorization for browser/privilege recovery or another measurement tool.

**Batch 21 result:** the app now overlays source outlines and instance pivots on the existing native
Composition preview, with graphical selection, translation, rotation and nonuniform/mirrored scale.
Outlines use exact resolved source extents already reported by the renderer; unavailable sources
retain pivot/gizmo editing without a guessed image extent. At most 128 ordinary outlines/pivots
are mounted plus the selected instance; list pagination retains access to the full document.

Generated `EditInstanceGeometry` carries the original authored transform and absolute canvas-texel
start/current pointer samples. Rust computes translation, signed rotation and local-axis scale in
f64, validates the candidate and preserves current unrelated fields. Invalid/nonfinite/degenerate
samples and wrong ownership reject atomically. The reply patches one authoritative instance.
Self-data coalesced mutation callbacks retain one initial/final history state per explicit gesture;
100 software translation samples Undo/Redo as one step. Hosted QuickJS uses the same intent and
committed callback; app snippets document the shape.

Golden UI now owns the shared `LatestEditGesture` lifecycle. Surface and Composition adapters use
one in-flight request and one latest sample, project/session/group fencing, release draining and
Escape/cancel/close disposal of unsent input. Accepted prefixes remain one undoable gesture.
Media/Screen/Composition panels defer catalog inspection and conflicting controls while the
shared group is active; accepted samples patch the focused document instead of rescanning it.

Validation: **109 passing / 1 ignored** default Rust tests, **99/99** headless tests, generated DTO
drift checks, **26/26** frontend tests, **213/213** Golden UI tests, Svelte **0 errors / 0 warnings**,
strict app Clippy and root/Golden Core formatting pass. The bundled app build passes. The bounded
UI attempt finished **03:11 UTC**: two launch attempts consume the allowance (the supported launcher
first opened installed MapGyver 1.0.0b7; direct executable launch opened this checkout's 0.1.0 app).
The checkout screenshot shows the Composition panel, but the helper reports higher target Windows
integrity; one coordinate click produced no visible state change. No drag/form/preview-pixel/Undo
claim follows. Both untouched owned test processes were stopped and their absence verified.
No privilege/browser/driver recovery or further native experiment ran.

**M06 remains 1/4 (25%)**: Matrix-inclusive dependency correctness, full native transform/blend
pixels and real editor interaction retain gates 2–4. Its software-verified functional contracts
permit M07 implementation under the existing execution correction; these open gates are not passed
or transferred. Next is the Matrix authoring/module foundation, not another automation investigation.

**Product batch 20:** started 2026-10-31 **02:51 UTC**, recorded stop **03:36 UTC**.
Deliver the app-owned dockable Composition editor: select a Composition, inspect its ordered
instances, assign sources and edit independent transform/opacity/blend/enabled state through the
existing generated backend operations. Reuse the native channel preview and existing workbench
registration; no second runtime or UI-owned defaults/domain validation. Verify selection/reply
fencing and operation routing, Svelte/formatting, DTO drift and the bundled app build. Keep graphical
transform gestures and real interactive/native pixel qualification open if outside this slice.
No native experiment or browser/driver recovery investigation is scheduled for this batch.

**Batch 20 result:** registered an app-owned Composition dock panel in the initial workbench.
It lists Composition channels, binds the existing native channel preview, reuses canvas settings,
and exposes source assignment plus instance add/copy/rename/reorder/remove and independent
position/scale/rotation/pivot/opacity/blend/enabled forms. Backend operations remain the single
owner of defaults, labels, validation, dependency checks, history and authored mutations. Source
picker values preserve the exact chosen channel identity even when the catalog changes; missing
authored references are displayed explicitly, without substituting another channel.

Instance navigation renders 50 rows per page; searchable source options render up to 100 matches
plus the retained selection. Selection and request state live in a focused app controller over
the public session operation interface. History/media events trigger inspection; no timer polls
the engine. Session/project/loading/disconnect/disposal fences suppress stale reads and mutations,
pending submissions are exclusive, and removal reconciles instance selection from backend replies.

Validation: **22/22 frontend tests** (seven new), **104 passing / 1 ignored** Rust tests including
generated-protocol drift checks, Svelte **0 errors / 0 warnings**, touched Prettier checks and
root/Golden Core formatting pass. The ordinary MapGyver binary rebuild includes the updated UI
bundle. No native/UI experiment ran; this proves software routing/fencing/build, not pointer/form
interaction, dock behavior or native preview pixels. **M06 remains 1/4 (25%)**. Graphical
selection/drag/rotate/scale with grouped history is the next product slice; gate 4 stays open.

**Product batch 19:** started 2026-10-31 **02:11 UTC**, recorded stop **02:56 UTC**.
Deliver demanded nested Composition capture and shared render-pass production through the existing
ChannelRenderer. Capture only bounded active dependencies with instance/membership/content guards;
solve affine position/scale/rotation/pivot on the preparation worker and reuse Golden layers/blends.
Prove shared-source/pass deduplication, nested outputs, reference geometry, source loss/recovery and
stale-plan rejection before native checks. Preserve existing render/resource budgets and isolate
unsupported/over-budget Composition routes. At most two finite native experiments/20 minutes after
passing software prerequisites; no new profiler/clock/driver or browser recovery investigation.
Native experiment 1 starts **02:42 UTC**, stops no later than the batch cutoff **02:56 UTC**.
Hypothesis: the existing D3D11/Spout backend installs and renders the app-generated shared nested
passes across edits and source Undo, then retires. Require the current installed frame identity,
three prepared channels/two output consumers and stale-capture rejection. This is submission/
lifetime smoke, not pixel/color, physical output or cadence qualification.

**Batch 19 result:** demanded captures now expand active Composition dependencies once per channel,
bounded by the caller's channel limit and 4,096 visited instance records. Disabled/zero-opacity
instances do not demand their sources. Instance, membership and content guards invalidate captures
after authoring, source removal and Undo. The preparation worker orders nested passes, shares each
source target and maps independent position/scale/rotation/pivot into Golden's existing projective
layers with opacity/blend/filter. Fixed output resolution preserves the authored canvas mapping.
Missing inputs leave transparent areas; unavailable Spout inputs retain source-only warmup demand.
Rejected instance/dependency/guard admission keeps that Composition unavailable and prunes orphan
source IO/passes. Existing renderer resource and dependency limits are unchanged.

Final checks: **104 passing / 1 ignored** default tests, **94/94** headless tests, strict app Clippy,
root/Golden Core formatting and the ordinary MapGyver build pass. Software references cover nested
source/pass deduplication, affine interior coordinates including mirrored scale/rotation/pivot,
all four blend declarations, source loss/Undo, stale captures, fixed sizing and bounded admission.
An existing fullscreen test now waits for asynchronous resource telemetry before asserting counts.
One native experiment passed at **02:42 UTC**: the ignored
`nested_compositions_share_sources_and_native_outputs_across_edits_and_source_undo` fixture
completed in **0.22 s**, installed current frames for three prepared channels/two Spout outputs,
rejected a stale capture and retired after edits and source removal/Undo. Native fixture build and
strict Clippy passed before execution. This is API/submission/lifetime smoke; no native pixel/color,
preview, physical presentation or cadence acceptance is inferred. No second experiment or profiler
investigation ran. **M06 remains 1/4 (25%)**, with Matrix-inclusive dependency correctness, full native
transform/blend pixels and the interactive editor open. The next slice is the Composition editor.

Before M06 source changes, its provisional whole-phase item is split into four fixed gates below.
Together they preserve the complete plan acceptance; all started open. Overall remains 3/10 (30%);
the M06 percentage is closed gates / 4. No retained qualification budget or earlier gate changes.

| # | Acceptance scope | State |
| --- | --- | --- |
| 1 | App-owned Composition media/canvas and stable ordered instances; independent source references/transforms, validated authoring, persistence and Undo/Redo; command nodes, script methods, callbacks and snippets use the same backend operations. | Accepted — batch 18 evidence below |
| 2 | Dependency correctness across all media types, including Composition/Matrix feedback; direct/indirect cycles reject atomically; source deletion/Undo and large/deep graphs remain coherent without owning/duplicating source media. | Accepted — batch 73 evidence above |
| 3 | Nested compositions render once per demanded output, shared sources retain independent transforms, and canvas resize, opacity/blends/alpha and rotation/pivot/scale are correct in native preview and output. | Accepted — batch 74 production preview-compositor/channel raster evidence; actual UI/presentation stays gate 4 and M05 |
| 4 | Registered 2D editor/source picker with selection, drag/rotate/scale, grouped history and UI/script/headless intent parity; real interactive evidence covers the complete editor behavior. | Open — caller parity and [complete new-layout input/project lifetime](evidence/mapgyver-m06-workbench-input-2026-11-01.json) retain batch89/95 scope. Actual native imagery/extent-based outline/shape-selection integration remains. |

**Product batch 18:** started 2026-10-31 **01:40 UTC**, recorded stop **02:25 UTC**.
Deliver the Composition authoring/module foundation under MapGyver media: canvas/channel type,
stable ordered instance nodes, transforms, validated references/cycles and atomic edit operations,
with persistence/history, command/script/callback/snippet integration and UI creation/canvas controls.
Instances are independent document nodes; batch creation avoids repeated full-tree rebuilds.
No renderer readiness is claimed until the composition producer is implemented; retain truthful
unavailable output without disrupting healthy channels. Nested render-plan integration is the next
product slice, not an M03 qualification detour. Prove authoring, malformed/cycle rejection,
history/reload and module parity; run software/UI checks and the real bundle. Keep native/UI
qualification open when unavailable, with no browser/driver/clock recovery investigation.

**Batch 18 result:** Composition joins the media/channel contract with an authored logical canvas
and independent UUID-backed ordered instance nodes. Backend operations add/copy batches, configure
source/position/scale/rotation/pivot/opacity/blend/enabled state, rename, reorder, remove and inspect.
Transforms use canvas texels/source-fraction pivots/radians; fixed output resolution retains the
logical canvas layout. Missing references remain authored, with no source substitution. Direct
and indirect Composition feedback reject before mutation; edit/load validation is iterative.
Matrix integration and full demanded-graph/native correctness remain in gates 2–3.

Create Composition is exposed in Media Manager and command presets; canvas controls, hosted
`editMedia`, generic commands, committed callbacks and app-owned snippets share the generated DTOs.
Golden's existing blend enum gains serialization/TS derives, avoiding another blend declaration.
Add/copy admits 1–4,096 instances per forest and 100,000 per output; one finite 1,000-instance
check measured **49.2255 ms**, one history transaction and no source duplication. This is a software
creation observation, not a sustained/frame/input budget pass.

The initial roundtrip test caught canvas-dependent factory defaults dropping sparse positions.
Factories now use canonical context-free defaults; AddInstances explicitly centers new placements.
Default and resized-canvas roundtrips preserve positions, source IDs, order and independent state.
Final validation: **99 passing / 1 ignored** default tests, **89/89** headless tests, **110/110**
Golden Video tests and **14/14** frontend tests; DTO drift, Svelte (0 errors/warnings), formatting,
strict app/video Clippy, Chataigne compile regression and bundled build pass. Runtime publication
keeps Composition truthfully unavailable and healthy outputs active until the producer is added.
No native experiment or M03 investigation ran. M06 gate **1** closes: **1/4 (25%)**; other phase
counts and overall **3/10 (30%)** are unchanged. Product slice **18**; next is nested render integration.

### M05 acceptance items

**Actual Surface Source batch87:** started 2026-11-01 **05:08 UTC**, stop **05:53 UTC**. Only
M05 gate7 is active. Previous turn is progress: `78361726` accepts Screen editor gate6. MapGyver
owns crop/mapping/calibration policy; Golden owns transport/history/native preview. Reuse the
ordinary bundled-browser fixture for naturally enabled Source form/calibration and all crop
handles/origin, invalid samples, Escape, shared selection/source loss and grouped history, with
read-only backend correlation. Max two finite invocations/20 minutes investigation including
baseline; source correction only for a concrete failure. Native full-source preview remains
separately required; do not close gate7 on DOM behavior alone or infer physical/OS/DPI evidence.
No guard bypass, CDP/desktop retry, new native fixture, settings change or gate reduction.

**Batch87 result, 05:15 UTC:** first/only invocation passes **exit0**, hosted flow **7,069 ms**,
Chromium **149.0.7827.55**, zero page errors. All mutations/history commands use real DOM controls;
public backend requests only inspect. Source/enablement/opacity/crop/destination form values stay
draft until Apply, and one Undo/Redo restores the complete definition. Grid/ColorBars/Off preserve
source/crop/mapping. All four crop corners and origin motion preserve peers/non-crop fields; each
sampled gesture undoes/redoes as one step. Invalid pointer/form samples reject without mutation or
history. Escape keeps accepted position and blocks later held-pointer motion. Actual source Delete/
Undo yields Missing source without rewriting the saved binding, then restores media IDs/catalog.
Shared selection and Screen/Source tab switching remain coherent; ordinary host exits normally.

Inspected screenshot shows the crop controls and **Preview unavailable**. Functional input uses
0.005 normalized-unit tolerance and retains actual final coordinates; no native full-source,
physical1-pixel/OS capture/DPI/latency inference. Syntax/Prettier and both formatters pass; no
production code changes or second run. **Gate7 remains open**, M05 **4/8**, overall **4/10 (40%)**.
Return on actual native selected-channel/extent/crop-overlay preview evidence; do not repeat this
input preparation. Publish, then select M06 gate4 actual Composition controls/history through the
existing naturally enabled browser route, retaining complete-editor/native/physical requirements.
[Evidence](evidence/mapgyver-m05-surface-source-input-2026-11-01.json).

**Actual Screen editor batch86:** started 2026-11-01 **04:56 UTC**, stop **05:41 UTC**. Only M05
gate6 is active. Previous turn is progress: `95c646b2` verifies actual Applications UI/history.
MapGyver owns Screen/Surface geometry and intents; Golden owns transport/history/native preview.
Use naturally enabled bundled Screen controls, source selection and real browser pointer input;
correlate committed geometry and one-step Undo/Redo through read-only public queries. Verify
selection, invalid mapping/cancellation and panel lifetime as needed for the complete editor gate.
Native scanout/underlay alignment, OS capture/DPI/latency stay in the original physical gates.
No guard/renderer bypass or invented preview. Max two finite browser/host attempts and20 minutes
of investigation, including baseline; publish a coherent verified result before further scope.
No CDP/desktop activation, GPU/reset/settings or parked qualification detour.

**Batch86 result — gate6 accepted, 05:03 UTC:** two browser/host invocations pass. The first
verifies source/selection and corner0/history/rejection/Escape; the final run adds remaining three
handles after the gate audit. Chromium **149.0.7827.55**, hosted flow **5,505 ms**, zero page errors,
normal host exit0 and no remaining MapGyver. All mutations/history commands come from naturally
enabled DOM controls; public backend requests only inspect. Screen/Surface creation/rename,
selection, source search/picker/Apply preserve stable references; selection and source draft alone
do not author. Every corner changes only its own coordinate, preserves source/peer and undoes/
redoes the entire sampled gesture in one step. Invalid mapping rejects visibly without mutation
or a history step. Escape keeps accepted position and blocks subsequent held-pointer motion;
panel switching keeps the catalog and controls coherent. Shared media remain unchanged.

The inspected screenshot shows all four edited handles and **Preview unavailable**. Input targets
use a **0.005 normalized-unit functional tolerance**, with actual final coordinates retained in
evidence; no physical1-pixel/OS capture/DPI/latency inference. Batch16's backend/gesture history,
coalescing/cancellation/stale/disposal proof remains cumulative evidence. Syntax/Prettier and both
formatters pass. **Only gate6 closes: M05 4/8 (50%)**, overall **4/10 (40%)**, original scope and
budgets retained. Publish, then select gate7's actual Surface Source crop/mapping/calibration/form/
history input; its native preview and complete physical matrix remain open.
[Evidence](evidence/mapgyver-m05-screen-editor-input-2026-11-01.json).

**Concurrent-output closure batch 72:** started 2026-11-01 **02:25 UTC**, stop **03:10 UTC**.
Only M05 gate 4 is active. Reuse the existing ordinary-app concurrent rectangle/fullscreen/Spout
fixture and its finite editor-state checkpoints. MapGyver owns editor behavior; Golden owns native
targets and the supported desktop launcher. Build/check the existing helpers first, then verify
activation of one fresh normal-integrity app window and actual editor open/minimize/restore/close
with concurrent output observations. The native investigation ends after 20 minutes/two experiments
or at the batch stop, whichever comes first. A fresh-selection activation retry is the only allowed
recovery; another failure stops input and leaves the gate open. No alternate automation framework,
M03 experiment, system setting change or acceptance revision is authorized by this batch.

**Batch 72 result:** the current app, receiver and launcher build successfully; both fixture JS
syntax checks pass. The supported launcher creates a fresh ordinary app (**PID 31768**, window
**69736366**). Explicit activation fails with `failed to activate captured window`; one fresh
selection/activation retry fails identically. Input stops immediately, without another launcher,
framework or experiment. All three targets are active through the public app operations; the
independent Spout receiver records **90** copies and its **96×64** RGBA bar capture has all eight
expected colors. Actual fullscreen pixels and editor open/minimize/restore/close are unverified.
The fixture exits **1** at its unadvanced initial checkpoint. The app exits **0**, unforced,
through its existing sentinel; launcher/app/receiver processes are absent and launch control is
removed. The [compact evidence](evidence/mapgyver-m05-activation-2026-11-01.json) retains raw hashes,
exact failures and the zero-error pixel comparison. Both workspace formatters and whitespace checks
pass. No gate closes: M05 **3/8**, overall
**4/10 (40%)**. Return requires supported activation or real manual observations of the missing
states. After this checkpoint, select dependency-ready **M06 gate 2**, retaining every M05/M03 gate.

**Concurrent-output acceptance batch 57:** started 2026-10-31 **18:27 UTC**, recorded stop
**19:12 UTC**. Previous turn made progress: `21ef6a99` rejects/reverts the failed M03 interval
candidate and explicitly schedules this one existing M05 gate 4 acceptance batch. Only gate 4
is active. Reuse the ordinary app/public Screen operations and independent receiver flow;
verify fullscreen, rectangle and Spout concurrently with finite pixel observations, then real
editor open/minimize/close and output changes while those editor states hold. Use private app
data and only owned app/windows/processes. Generic host/window/GPU ownership remains reusable;
app calibration/editor policy stays in MapGyver. Add only finite fixture observations/handshakes
needed for this gate, not new production controls or a diagnostic framework. Software/build
checks precede at most two native flows/20 minutes and the 19:12 stop. Native computer use is
authorized and uses the supported skill; no desktop/driver/overlay setting changes. Full DPI,
input/latency, hotplug and physical matrix acceptance retain gates 5–8. Return to M03 after
this one batch, preserving every requirement and its pending GPU-reset question.

Batch 57's first native flow starts **18:42 UTC**, cutoff **19:02 UTC** (and at most two flows),
before the 19:12 batch stop. Software prerequisites are terminal: **192 app tests / one existing
hardware ignore**, strict app/native-qualification-bin Clippy, debug app/receiver builds, JS syntax
and Prettier, both Cargo formatters and whitespace checks. The catalog/pixel observation roles use
public Golden APIs and Rust-owned app display DTOs; no production protocol changes. The first
compile identified private bounds fields; the fixture now uses their existing public accessors.
Run `node apps/mapgyver/tests/native_output/check.mjs --acceptance` once, retaining its process
handle through UI checkpoints, finite receivers, pixel captures and normal shutdown. Only a
concrete failure permits the second flow. Pixel PNGs/UI observations stay local with a compact
handoff record; do not infer DPI/latency or a gate pass from mere API acceptance.

First flow reaches all three active targets and **179** independent Spout copies; its 96x64 PNG
matches all eight expected RGBA bars exactly, and computer use captures the rectangle bars.
It stops at the initial UI checkpoint because the terminal-launched app is high integrity while
the desktop helper reports lower integrity/limited accessibility. The app exits normally through
its sentinel; the checkpoint timeout is retained, not a native-output acceptance failure/pass.
The corrected fixture uses a dedicated finite launcher started by the desktop helper, passing
private app data/bind/shutdown settings to its child without a shell or security-setting change.
The app and launcher use only ordinary process creation; all control stays in the supported
computer-use APIs. Launcher build and strict bin Clippy pass. The second/final flow starts
**18:52 UTC**, still bounded by the original **19:02** native cutoff and 19:12 batch stop.

**Batch 57 result:** both planned flows activate fullscreen **(2560, 0, 1920x1080)**, rectangle
**(80, 120, 640x360)** and a **96x64** Spout target concurrently through the ordinary app.
Independent receivers observe **179 / 104** copies. Fresh probe PNGs compare **6,144 RGBA texels**
per flow against all eight authored bars with **zero byte error**. The supported computer-use
capture includes the rectangle; its eight decoded bar centers have maximum RGB error **1/255**
in both flows. Those captures are **512x288 JPEG logical screenshots**, not raw physical-pixel
alignment evidence. Fullscreen visible pixels are not captured and remain unverified.

The first app inherits high terminal integrity and exposes limited accessibility. The finite
desktop-helper launcher fixes that: the second app has a full accessible workbench tree. However,
click activation reports **`failed to activate captured window`**, and one fresh-selection/explicit
activation retry reports the same error. Input is stopped under the supported recovery rules;
no editor opening, minimization or closure is inferred. Both **initial** checkpoints time out,
so both native flows exit **1**. Their apps exit **zero** via the existing shutdown sentinel;
no owned MapGyver window or either retained app PID remains, and the launch-control file is removed.

The [partial packet](evidence/mapgyver-m05-concurrent-partial-2026-10-31.json) records exact
artifacts, pixel comparisons, display identities, app exits and unverified scope. Source helpers
and the [finite workflow](../../apps/mapgyver/tests/native_output/README.md) are checkpointed.
After the two flows, launcher status publication becomes atomic, an owned-child/watchdog guard
and local-data isolation are added, and control cleanup becomes unconditional. These hardening
changes pass compilation/strict Clippy but are **not** claimed as a third native verification.
No further native attempt or helper workaround is run. **M05 stays 3/8; M03 stays 7/9; overall
stays 4/10 (40%)**. Return to M03 after this scheduled batch; M05 resumes when supported activation
works and its actual fullscreen/editor observations can be collected. All original gates remain.

**Product batch 17:** started 2026-10-31 **01:17 UTC**, recorded stop **02:02 UTC**.
Deliver full-source native preview with graphical crop-corner/origin gestures, preserving the
Screen quad, source and other Surface settings. MapGyver owns geometry intent semantics and
validation; UI owns normalized pointer input and presentation. Unify the existing corner route
under a generated Surface geometry intent and reuse one bounded gesture/session controller for
both editors. Prove crop legality, preserved fields, grouped Undo/Redo and stale/cancel behavior;
register independent dockable views over one window-owned selection/catalog/gesture context.
run software checks and the bundled build. Interactive/native input remains open when unavailable;
do not restart browser/integrity recovery or M03 diagnostics. Native experiments, if available,
retain the two-experiment/20-minute limit and must follow passing software prerequisites.

**Batch 17 result:** the Surface Source view registers the assigned channel directly for a full,
uncropped preview and overlays crop corners plus an origin/move handle. The canonical generated
`EditSurfaceGeometry` intent unifies quad-corner, crop-corner and crop-origin edits; the earlier
corner-only command is removed. Backend math fixes the opposite crop corner during resize,
preserves width/height during movement and rejects invalid candidates before mutation. Source,
destination quad, calibration, opacity and enable state survive crop gestures. Missing sources
retain their identities; no substitute channel is selected.

Screen Manager and Surface Source are separate default dock tabs over one window-owned authoring
context, catalog, selection and gesture controller. Shared selection follows either view; closing
all views stops new inspections and finalizes accepted input. Numeric fields remain available for
keyboard editing. The existing Golden edit session/history and native preview contracts are reused;
no new Golden API, GPU pipeline or polling loop is added.

Checks complete by **01:37 UTC**, ahead of the original **02:02 UTC** cutoff: **91 passing / 1 ignored**
default tests, **81/81** headless tests and **12/12** frontend tests. Coverage includes all four crop
corners, movement, 100-sample grouped Undo/Redo, preserved fields, no-op/invalid edits, missing
sources, persistence roundtrip and the real QuickJS request/result route. Rust-generated DTO drift,
Svelte (0 errors/warnings), UI formatting, strict app Clippy and the latest bundled executable build
pass; both workspace formatters ran. Existing browser unavailability is retained without another
automation-recovery investigation. Native full-source preview and actual pointer/dock behavior
remain unverified. **M05 stays 3/8**, M03 7/9, M04 1/6 and overall 3/10 (30%); no denominator or gate
changes. Product slice **17** is delivered; proceed to M06 against the verified public contracts.

**Product batch 16:** started 2026-10-31 **00:40 UTC**, recorded stop **01:25 UTC**.
Deliver graphical Screen Surface selection/corner dragging through backend-owned corner intents
and grouped history, preserving source/crop/opacity and rejecting invalid quads atomically.
The UI owns viewport coordinates, pointer capture and presentation; backend validation/mutation
and the existing history boundary own document changes. Prove multiple drag samples make one
undoable gesture, cancellation/invalid samples preserve coherent state, and the wired UI compiles.
Use an available real UI flow within at most two experiments/20 minutes; retain unavailable native
input/pixel/DPI qualification without an automation recovery detour.
Golden will provide explicit opt-in coalescing of adjacent same-node callback history inside an
edit session, retaining the original before/final after state so drag duration does not grow history.
**Batch 16 result:** `MoveSurfaceCorner` reads the current backend definition, changes only the
requested corner and validates before mutation. SVG selection/handles sit above native preview
and error covers. One in-flight request plus one latest point bounds pending input; authoritative
Surface replies update the editor without per-sample catalog inspection. Normal release drains
the final point. Escape/cancellation/closure discards unsent input and finalizes accepted changes
as one undoable gesture; project/session fences reject late replies. Generic opt-in history
coalescing retains original/final state, with other edits and noncontiguous states as barriers.
Self-data scope skips reference/context rescans; metadata changes still invalidate conservatively.

Validation completed **01:15 UTC**, before the original **01:25 UTC** stop: **87 passing / 1 ignored**
MapGyver default tests, **77/77** headless tests, **659 passing / 3 ignored** Golden engine tests,
and **11/11** frontend tests. These include 100 samples with one Undo/Redo, invalid/no-op points,
10,000 samples retained as one history step, invalidation scopes, cancellation and stale replies.
Strict app/engine Clippy, Svelte (0 errors/warnings), UI formatting/build and the bundled executable
build pass. Raw generated DTOs remain checked against Rust and excluded from hand formatting.
The automation inventory exposes no browser; creating the in-app browser returns unavailable.
Interactive selection/drag and full source-editor input remain unverified. **M05 stays 3/8**, M03
7/9, M04 1/6 and overall 3/10 (30%); no gate or denominator changes. This is product slice **16**.

**Product batch 15:** started 2026-10-31 **00:01 UTC**, recorded stop **00:46 UTC**.
Deliver native display enumeration with stable device-path identities, a generated catalog/UI
selector, and fullscreen Screen resolution/recovery without substituting another display.
Golden owns OS enumeration and physical metadata; MapGyver owns intended-target selection,
missing-display state and output reconciliation. Prove identity/reorder/disconnect recovery in
software and available fullscreen native geometry/delivery within two finite native experiments
or 20 minutes. Keep unavailable hotplug/mixed-DPI/physical pixel/input qualification open.
Software suites, strict Clippy, Svelte checking and the real native executable build pass.
Native checks start **00:21 UTC** and stop by **00:41 UTC** or two experiments: native catalog
enumeration/fixture, then the ordinary fullscreen/rectangle/Spout flow.

**Batch 15 result:** Golden's public `native_displays` returns bounded active monitor device paths,
labels and checked physical desktop bounds. It uses the locked Windows SDK API, admits at most
32 paths/128 modes and retries topology-buffer races at most three times. Returned identities are
canonical device paths, never ordinal/index, friendly label, current position or primary-display
selection. OS device reinstallation can change a path; such selections remain explicitly missing.
MapGyver owns the generated Rust/TypeScript catalog/request/event DTOs, desktop selector and
target resolution policy. Discovery does not create GPU resources or run on engine/UI threads.

The preparation worker coalesces explicit refreshes and rescans once per second for topology/error
recovery. Only changed catalog data/errors advance revision guards and emit metadata. Captured
plans retain that guard, so a display change triggers recapture without an authored edit. Fullscreen
routes resolve the exact intended identity to current physical position/extent; missing or oversized
targets withdraw only their output/source demand, preserving healthy target capture. Query failures
clear fullscreen availability until recovery. Saved choices are never replaced with another display.
Disabled fullscreen previews also use the intended display's available extent/aspect. Resolved
targets have one typed endpoint/extent contract instead of duplicated authored/runtime dimensions.
Window retirement, limits and brief shared-session interruption on topology changes are unchanged.

**Native evidence:** both finite experiments passed by **00:25 UTC**:

- Production catalog scan found **two** distinct device-path identities: a 2560×1600 display at
  (0,0) and RTK FHD HDR 1920×1080 at (2560,0). The exact generated DTO fixture stays local at
  `target/m05-display-catalog-batch15.json`; raw identities are not needed in this handoff.
- The ordinary app runs fullscreen, rectangle and virtual Spout simultaneously, with no frontend
  or editor loaded. The selected fullscreen is exactly **(2560,0,1920×1080)**, and all three native
  consumers record an API-accepted submission. Changing that Screen to an unavailable identity
  withdraws its window, leaving two output routes; restoring the original identity resumes the same
  logical output and exact bounds. This is a target-selection scenario, **not physical hotplug**.
  Independent Spout image-copy counts are **180 Bars, 179 Grid, 178 restored media, 0 disabled
  (1,978 absent polls), 180 re-enabled/resized**, with zero receive failures/active-layout mismatches.
  The app and all five receivers exit **0**; no native-window teardown error or OS setting change
  occurs. Artifacts stay local at `target/m05-spout-1793406255600`.

Native output logs now record first API acceptance once per consumer/device; they are not scanout,
pixel/color, unique-frame or cadence proof. Both experiments are consumed. No further native test,
automation recovery, driver/clock work or physical-monitor reconfiguration is scheduled this batch.
Real cable hotplug, mixed-DPI/negative-origin monitor arrangements, selector/form input, minimize,
graphical gestures and the complete physical-presentation matrix retain their original gates.

Software checks cover exact-identity/reorder stability, no fallback when another display has the
same name/geometry, loss/error/recovery, revised bounds, disabled-preview extent, guard invalidation
and oversized-target isolation. The headless/Null path also publishes, withdraws and restores a
fullscreen route from injected metadata without native OS/GPU work. Rust-generated protocol drift,
Svelte checks, real bundled build, strict app/host all-feature Clippy and syntax checks pass; final
suite totals and format/delivery evidence follow below. No budgets or acceptance denominators change.
Final validation: **85 passing / 1 ignored** default-feature library tests and **75/75** engine-only
tests pass. The ignored native catalog test was run explicitly as the first experiment above.
The added Null-runtime test verifies metadata-driven fullscreen publication/withdrawal/restoration;
the catalog guard test proves presentation invalidation without a document edit. Strict Clippy,
both workspace formatters, finite-flow syntax and final whitespace checks pass. Implementation and
checks finished by **00:37 UTC**, including final selector labels and the embedded UI rebuild,
before the recorded **00:46 UTC** stop.
**M05 remains 3/8 (38%)**, M03 7/9, M04 1/6, overall 3/10 (30%). Product delivery count is **15**;
the next product deliverable is graphical Surface selection/corner dragging with grouped history.

**Product batch 14:** started 2026-10-30 **23:42 UTC**, recorded stop **00:27 UTC** (Oct 31).
Deliver physical rectangle output windows from Screen target/enabled state, sharing the native
renderer with previews/Spout and retaining output independently of editor visibility. Golden's
desktop host owns reusable window creation/physical geometry and lifetime; MapGyver owns target
policy/routing. Prove signed-coordinate validation, explicit GPU-before-window retirement,
enable/disable/reconfigure and concurrent rectangle/Spout delivery. Native hypothesis: a borderless
host-owned native window preserves the authored physical client rectangle and receives the shared
Screen texture without an editor. Stop native investigation at 20 minutes or two experiments,
including baseline, and preserve full fullscreen/display/DPI/gesture qualification scope.
Software prerequisites pass. Native checks start **23:49 UTC** and stop by **00:09 UTC** (Oct 31)
or two experiments: finite host geometry/destruction, then the ordinary concurrent-output app flow.

**Batch 14 result:** Golden's desktop host now exposes `NativeOutputBounds` and
`NativeOutputWindow` through the public `golden_core::app` facade. The host owns a borderless,
topmost, nonactivating output HWND and an event-driven per-monitor-v2 window thread. Checked
signed client rectangles reject empty/overflowing bounds; geometry is queried on that thread,
avoiding caller DPI virtualization. User close/resize and DPI-suggested resizing do not release
or change the borrowed target. `close` joins actual native destruction. No app-specific policy,
GPU objects, WebView or recurring engine work moved into this reusable owner.

MapGyver captures enabled rectangle and Spout targets under one seven-Screen-output admission
limit plus the existing host preview consumer; the eight-output/resource budgets are unchanged.
Runtime Screen IDs and channel/Screen pass sharing are retained. The preparation owner creates
windows, checks their exact physical client bounds, borrows HWNDs through Golden Video's public
binding API, and retains them until actual GPU and picture-worker retirement. Position/extent/
enablement changes retire the old shared session/windows before replacement, preserving the
documented brief interruption on topology changes. Surface edits and editor visibility republish
only the plan. Physical windows use opaque sRGB BGRA output; Spout retains straight alpha. Runtime
availability/error presentation now names generic Screen outputs rather than assuming Spout.

**Native evidence:** both planned experiments passed, with no additional probe or rerun:

- The public host owner creates an exact **(-80, 40, 320×180)** client rectangle, reports matching
  physical bounds, and `close` returns only after `IsWindow` proves its HWND gone. This establishes
  signed coordinates, not a negative-origin monitor/DPI/hotplug qualification pass.
- The ordinary desktop app runs rectangle and Spout Screens together with no frontend/editor
  loaded. It checks exact **(80, 120, 640×360)** rectangle client bounds, then moves/resizes the
  same logical output to **(120, 180, 480×270)**. Independent Spout receive-copy counts are
  **180 Bars, 181 Grid, 180 restored-media, 0 disabled (1,970 absent polls), 179 re-enabled/resized**,
  with zero receive failures/active-layout mismatches. The app and five receivers exit **0**,
  with no native-window teardown error. Logs show completion-retired sessions before replacement.

The finite flow is `node apps/mapgyver/tests/native_output/check.mjs --rectangle`; artifacts remain
local at `target/m05-spout-1793404312320`. The computer-use inventory during the live run returned
the main app window; no output-window screenshot was obtained before its finite inspection interval
ended. No screenshot/physical-pixel, input, minimize, mixed-DPI, full concurrency, color or latency
acceptance is inferred. Both native experiments are consumed and all owned processes are terminal.
No automation recovery, driver setting, M03 investigation or additional native run is scheduled.

Validation: **82/82** app default-feature and **74/74** engine-only tests pass; the desktop host has
**30 passing, 2 ignored** normal tests, with the new native geometry/destruction test explicitly
run and passing as above. Strict app/host all-feature bins/tests Clippy, Svelte checking, finite-flow
syntax and the real app/native fixture build pass. New focused checks cover signed bounds/overflow
and rectangle/Spout capture, distinct endpoint alpha contracts and shared admission without actor
window creation. **M05 stays 3/8 (38%)**, denominator unchanged; M03 stays 7/9, M04 1/6 and overall
3/10 (30%). Product-delivery count is **14**. Next is display catalog/fullscreen activation, retaining
all display identity/recovery, physical/visual/input/minimize and editor qualification requirements.
Final review also keeps failed Win32 class registration retryable by caching only successful
registration. Final strict Clippy/host software checks and both workspace formatters pass; checks
finished **23:59 UTC**, before the recorded **00:27 UTC** stop.

**Product batch 13:** started 2026-10-30 **23:25 UTC**, recorded stop **00:10 UTC** (Oct 31).
Deliver enabled Screen virtual Spout targets through the shared native worker, retaining render
demand independently of preview panels. MapGyver owns target/routing policy; Golden owns bounded
native output execution and retirement. Prove stable output identity, enable/disable/reconfigure
and source-independent calibration with unit/integration checks and a finite native receiver.
Native hypothesis: the existing Spout output backend can consume the authored Screen plan while
no editor is registered; disabling removes its sender and re-enabling recovers it. Stop native
investigation after 20 minutes from its first run or two experiments, including the baseline.
Physical windows, display recovery and graphical gestures remain open; no M03 profiling work.
Prerequisite app suites, strict Clippy, Svelte checking and the native executable build passed.
The first finite native flow starts **23:36 UTC**; its investigation stop is **23:56 UTC** or
two experiments, before the overall batch stop.

**Batch 13 result:** enabled virtual Spout targets now activate from Screen policy in the existing
shared native presentation worker. Bounded capture unions editor and output demand, deduplicates
channels/Screens and retains guards for disabled targets, so outputs initialize and continue with
no preview registration or viewport metrics. The runtime ID registry remains bounded to the current
128-Screen catalog and preserves identities through toggle/rename/reorder. Seven virtual outputs
plus the host window fit the unchanged eight-consumer limit. Duplicate enabled sender names fail
atomically, including project validation; output names leave room for Golden's versioned SDK
counter namespace. App output policy chooses BGRA8 sRGB straight alpha and Golden owns conversion,
cadence, endpoint recovery and completion-retained storage. Errors/states reuse preview status.

Golden's existing native bindings are immutable for a session. Changing target enablement, name or
extent completes retirement of the old shared session before replacement. This explicitly interrupts
other outputs briefly and restarts prepared source sessions; same-name senders/HWND swapchains never
overlap. Surface edits and editor geometry/visibility only republish the shared render plan. Full
target-concurrency/reconfiguration acceptance remains open; no new dynamic-binding framework or
resource ceiling was introduced.

**Native evidence:** the first flow's receiver observed no sender because HTTP readiness preceded
Tauri startup (the no-frontend launch still waited on the default development frontend URL). The
fixture now explicitly uses its own listener as frontend URL and waits for actual native Ready
state before receiving. No product/GPU workaround was needed. The second/final flow completed
successfully by **23:38 UTC** with the ordinary desktop app and no frontend/editor loaded:

| Condition | Independent SDK image-copy submissions | Result |
| --- | --- | --- |
| Color Bars with no editor | 181 | Delivered |
| Grid with no editor | 179 | Delivered |
| Calibration Off, authored media route restored | 179 | Delivered |
| Screen disabled after confirmed session retirement | 0; 1,984 absent polls | Sender withdrawn |
| Re-enabled at 128×64, previously 96×64 | 181 | Recovered at new extent |

All receive failures and active-layout mismatches are zero. The app reports calibration plans with
zero media channels and Off with one restored channel, retaining output ID 2 after re-enable.
The app and all five independent receivers exit **0**. Artifacts remain local at
`target/m05-spout-1793403416752`; the failed readiness flow is retained at
`target/m05-spout-1793403377149`. The repeatable finite API/receiver check lives at
`apps/mapgyver/tests/native_output/check.mjs`; marker decode errors in this reused fixture are
expected for calibration/solid images and are not pixel/color evidence. No frames are read back by
the app. This is functional independent SDK delivery, not display, cadence or latency qualification.
Both native experiments are consumed; no further native investigation is scheduled this batch.

Validation: **81/81** default-feature and **74/74** engine-only app library tests pass; strict
all-feature bins/tests Clippy, Svelte checks, finite-check syntax and the native executable build
pass. The bounded catalog, shared-channel demand, no-editor capture, calibration demand withdrawal,
duplicate-name rejection, sender bounds and identity/toggle/reorder rules have direct checks.
Ordinary-app calibration visual/form-input, minimized/concurrent-target coverage, physical windows,
display recovery and graphical gestures remain open. **M05 stays 3/8 (38%)**, denominator unchanged;
M03 stays 7/9, M04 1/6 and overall 3/10 (30%). This is product-delivery batch **13**. Next is physical
rectangle output activation; no M03 qualification work is resumed.
Implementation/checks finished by **23:41 UTC**, before the recorded **00:10 UTC** stop.
Both workspace formatters, the final whitespace check and finite-check syntax pass before delivery.

**Product batch 12:** started 2026-10-30 **22:57 UTC**, recorded stop **23:42 UTC**.
Deliver Grid/Color Bars calibration without a source, retaining authored source/crop/routing for
restoration when calibration is Off, plus ordered Surface alpha composition. Owners: Golden's
generic procedural layer/shader contract and MapGyver's calibration policy. Proof: bounded layer
admission, routing/history tests, finite native pixel oracles and a functional app preview flow.
Native hypothesis: procedural patterns render through the same mapped compositing path without
source textures or CPU frame transport; turning Off restores the original source definition.
Native investigation stops by **23:17 UTC** or two experiments (including the pixel baseline).
Keep full output/display/editor qualification open; no M03 profiling or automation recovery.

**Batch 12 result:** Golden layers now carry a typed texture or procedural source. Checked
GridPattern and an eight-color bar palette render directly in the mapped shader at target
resolution, with no source texture, per-Surface calibration texture, decoder or CPU frame pixels.
Procedural layers still consume the existing draw/dependency limit, so zero texture dependencies
cannot bypass draw admission. Native accounting includes the **112 + 160 = 272 bytes** of shared
shader constant buffers; no resource, input, timing or qualification ceiling was increased.

MapGyver owns its 16×9 white/black grid and eight-bar RGB palette. Calibration substitutes the
procedural source only in the prepared layer and covers the full authored quad, preserving the
saved source/crop. Quad, opacity, enablement and painter order still apply. Active calibration
demands no media channel or asset; Off restores the original source route/crop/identity. Preview
capture recognizes source-independent content and no longer reports calibration as unsupported.
The UI explains that calibration leaves source crop saved for Off. Source-independent patterns
also work with an explicitly unassigned source. Surface reorder/Undo retains each layer's alpha
and mapping.

**Native evidence:** the first hardware attempt failed during shader creation because `line` is
an HLSL keyword. The agent had launched it before inspecting the prerequisite suite failure;
AGENTS.md now explicitly requires terminal prerequisite success before dependent native checks.
The identifier was corrected and the software shader test expanded to all four real entry points.
Only after that test passed did the second/final hardware experiment run: **448/448 pixels**
matched independent bars, grid, source crop, mapped coverage, fractional alpha, reversed overlap
order and subsequent texture-path oracles at **2/255** tolerance. It completed **23:13:15 UTC**,
before the 23:17 stop. Both native experiments are consumed; no ordinary-app calibration preview
flow ran. That visual/input evidence remains open with editor/target integration; no third native
run or timing/automation investigation was added.

Validation: **77/77** default-feature and **71/71** engine-only app library tests pass, including
Grid/Bars/Off routing, zero source demand, retained crop/quad/opacity, truly unassigned source,
Surface order/Undo and generated-contract drift. Golden's **110** portable unit tests and **4**
integration tests pass; its D3D11 suite has **149** passing and **18** ignored tests. All shader
entry points compile; the procedural hardware oracle was run explicitly as above. Strict app/
Golden all-feature bins/tests Clippy, Svelte checking and the real bundled build pass.
Implementation and validation finished by **23:24 UTC**, within the recorded batch stop;
both workspace formatters and the final whitespace check pass before milestone delivery.

**M05 gate 3 closes** for runtime source-independent calibration/restoration and ordered alpha
composition from backend/runtime and finite GPU evidence: **3/8 (38%)**, denominator unchanged.
Ordinary-app calibration visual/input acceptance remains an explicit editor evidence gap; it is
not inferred from the pixel fixture. Physical/Spout activation, display recovery, graphical editor
gestures and full physical qualification retain gates 4–8. Overall remains **3/10 (30%)**, M03
**7/9**, M04 **1/6**. Next is virtual Spout output activation with an end-to-end calibration check.

**Product batch 11:** started 2026-10-30 **22:34 UTC**, recorded stop **23:19 UTC**.
Deliver native composed Screen preview in the Screen Manager through the existing shared host:
typed app-owned preview source keys, bounded Screen/channel capture, shared renderer demand,
revision invalidation and status/error presentation. Owners: MapGyver preview preparation and UI;
Golden keeps native geometry, visibility fences and GPU lifetimes. Proof: mixed Screen/channel
plan and capture tests, generated-contract/Svelte/app checks and at most two native functional
experiments. Hypothesis: the Screen source key resolves to the same composed texture as runtime
Screen publication, clipped by the existing native compositor, without a separate renderer.
Native investigation stops by **22:54 UTC** or two experiments; if capture/input remains limited,
retain that limit and finish the software-backed delivery without automation recovery or M03 work.

**Batch 11 result:** the Screen Manager shows composed native Screen textures through the same
preview session, renderer and window compositor as Channels. Tagged Rust-owned Channel/Screen
source keys and status DTOs have a dedicated generator and UI consumer. Shared routing/capture/
host code moved from `media/preview` to `src/preview`; Golden geometry/visibility/GPU mechanisms
are unchanged. Channel rendering still owns source preparation; app preview captures bounded
Screen/channel unions and guards on the actor, then prepares their shared dependency graph on
the worker. Preview-only Screen routes require no physical/Spout binding. Existing limits remain.

Screen and Surface edits invalidate the capture even when channel definitions remain current.
Screen deletion/Undo and capture-limit/unsupported-calibration errors retain recovery guards.
Unavailable Screen views have explicit per-source status and no resolved region; healthy Channel
views continue. Typed source identities remain distinct and stable across JSON property order.
Rectangle/Spout targets supply preview extents; unresolved fullscreen/unconfigured targets use
the existing backend default canvas. Target activation is still separate and unimplemented.

**Native evidence:** one ordinary bundled-app run displayed a red perspective-mapped Surface
and a half-opacity green Surface, with visible overlap and the physical rectangle target disabled.
Disabling the green Surface removed its pixels; restoring the same authored Surface brought them
back. Native layout 3 installed with channel counts **2 → 1 → 2**. Backend operations authored the
fixture; supported native screenshots observed output. No Windows input/drag, physical output,
quantitative color/alpha or latency acceptance is claimed from those images. The owned app exited
**0 at 22:47:55 UTC**; native investigation ended before 22:54 after one functional flow. No second
experiment, browser recovery, settings change or M03 investigation was needed.

Validation: **75/75** default-feature and **69/69** engine-only library tests pass, including mixed
Channel/Screen view deduplication, one compositor output, Surface guards, deletion/Undo, isolated
unsupported Screen recovery, bounds and generated DTO drift. All **8/8** app UI tests pass; strict
all-feature bins/tests Clippy, Svelte checks and the real bundled build pass. The new generator
initially needed `GC_SKIP_UI_BUILD=1` to bootstrap its frontend imports; the native run used the
subsequent ordinary build with embedded UI. Both workspace formatters pass. Implementation/checks
finished by **22:55 UTC**, before the recorded **23:19 UTC** stop; the batch finishes early.
M05 stays **2/8 (25%)**: complete calibration/order/alpha, target/output, display, graphical editor/
gesture and physical qualification gates remain open. Overall **3/10 (30%)**, M03 **7/9**, M04 **1/6**.
The next product deliverable is generated calibration without a source and restored routing.

**Product batch 10:** started 2026-10-30 **22:13 UTC**, recorded stop **22:58 UTC**.
Deliver checked source-crop/perspective mapping in Golden Video's ordered compositing layers and
MapGyver Surface render-plan preparation against existing channel textures. Owners:
`crates/golden_video` (generic math/renderer) and `apps/mapgyver/src/screens` (authored policy).
Proof: projective/reference geometry tests, atomic invalid authoring/plan admission, app tests and
one finite hardware pixel-pattern check using the existing native test harness. Hypothesis:
inverse projective sampling after crop will match independently calculated reference texels and
preserve backdrop/order outside the mapped quad. A mismatch permits one focused correction/rerun;
native investigation stops by **22:33 UTC** or two experiments. No timing/clock, overlay, display
placement or automation investigation. Full M05 runtime/output/editor gates retain their scope.

**Batch 10 result:** Golden's ordered Layer now accepts an optional checked ProjectiveMapping.
The renderer inverse-projects each target coordinate into the unit source quad, then samples the
authored source crop; outside the quad is transparent over the accumulated backdrop. Mirrored and
off-target quads work. Checked private coefficients prevent arbitrary singular matrices entering
a plan. f64 preparation validates the actual f32 corner correspondence and finite positive branch.
Full-target/region draws retain their ordinary pixel entry point. Compositing reuses existing
ping-pong storage, with no new texture per Surface. Its shared constant buffer is now **112 bytes**
(formerly 64), charged through the existing native resource accounting; no budget was increased.

Canonical Point/Quad/UnitRect definitions now live in Golden rather than duplicate app geometry;
MapGyver's DTO generator/consumers are updated together. Surface authoring uses the same checked
mapping constructor, so invalid crops/quads reject before edits/history change. Screen capture
copies only bounded metadata and project/membership/Surface guards on the actor; preparation
solves mapping off the frame/engine loop. The public Screen-route publication API shares one
channel render and one Screen pass among same-sized consumers. Mapping edits preserve texture
identity; source deletion produces an explicit transparent channel placeholder and Undo restores
it. Stale/mixed-project captures reject before replacing the live renderer project.

**Reference proof:** the new finite D3D11 test checks **192/192 pixels** across cropped identity,
perspective and mirrored/off-target patterns against independent analytic texel/alpha oracles,
at **2/255** tolerance. It verifies transparent outside coverage over a fractional-alpha backdrop.
The existing hardware color/alpha/filter/blend oracle also passes. Those two finite tests exhaust
this batch's native experiment allowance; both completed by **22:22 UTC**, before the 22:33 stop.
No GPU timing, physical output, overlay or automation investigation ran. Readback is confined to
the finite test harness; ordinary frame paths remain native.

Validation: **72/72** default-feature and **66/66** engine-only MapGyver library tests pass,
including shared Screen consumers, actual captured perspective data, stable texture identity,
source delete/Undo, stale capture/project rejection, atomic invalid authoring and generated DTO
drift. Golden has **109/109** portable tests and **148** D3D11-feature tests passing with **17**
hardware/qualification tests ignored in the ordinary suite; the two hardware oracles above were
run explicitly. Strict app/Golden all-feature bins/tests Clippy and Svelte checking pass.
The bundled app build and both workspace formatters pass. Implementation/checks finished by
**22:33 UTC**, before the recorded **22:58 UTC** stop; the batch ends without adding another probe.

**M05 gate 2 closes** for the checked crop/warp primitive and app Screen preparation/publication:
**2/8 (25%)**, with the denominator unchanged. Native Screen preview/target activation, calibration
pattern rendering, full overlapping-Surface acceptance, display recovery and editor/physical
qualification retain gates 3–8. Calibration-selected Screen routes currently return an explicit
unsupported error rather than substituting the source. Overall remains **3/10 (30%)**, M03 **7/9**,
M04 **1/6**. Next product slice is native Screen preview through the existing shared host.

**Product batch 9:** started 2026-10-30 **21:55 UTC**, recorded stop **22:40 UTC**.
Deliver the registered Screen Manager authoring panel: Screen/Surface selection and CRUD,
explicit target settings, channel assignment and authored mapping/calibration controls using
the generated ScreenEdit contract. Owner: `apps/mapgyver/ui/src/lib/screens` and app UI registration.
Proof: Svelte/build checks, operation/client tests and a bounded native UI observation if the
supported desktop route is available. A form/field change is one submitted atomic edit; graphical
drag gestures and rendered mapping remain separate acceptance. Investigation stops by
**22:15 UTC** or two experiments; do not recover the unavailable browser or resume M03 diagnostics.

**Batch 9 result:** the required Screen Manager tab is registered beside Media Manager through
Golden's public panel API. It exposes Screen/ordered Surface creation, selection, rename, moves
and removal; explicit fullscreen identity/physical rectangle/Spout settings; and source, crop,
destination corners, opacity, enablement and calibration forms. Draft values come from the
authored DTOs, with explicit input for new target fields and no frontend domain defaults/IDs.
Each Apply uses one ScreenEdit through the existing session interface. Missing source references
remain selectable; disappearance while editing does not silently choose another source.

`authoring.ts` owns presentation state and asynchronous reply fencing. History, snapshots,
connection/loading and both app-event topics refresh the catalogs without polling or a private
WebSocket dependency. Late responses from old reads, sessions, projects and disposed panels are
ignored. Navigation pages bound Screen/Surface rows to 50; source search bounds options to 100
matches plus the exact selected source, avoiding an unbounded DOM for large authored catalogs.

Validation: Svelte checking passes with zero errors/warnings; all **7/7** app UI tests pass
(five new authoring-controller lifecycle/rejection/duplicate-submit cases plus the two existing
media cases). The final bundled app build, Prettier and both workspace formatters pass.
Implementation/checks finished by **22:11 UTC**, before the recorded **22:40 UTC** stop.
The ordinary bundled app displayed the registered
Screen Manager and empty state. The single native run used public loopback operations to author
a signed physical rectangle and cropped/calibration-selected Surface from a Solid Color channel;
the live UI issued fresh event-driven catalog reads. Configured-field captures were obscured by
another window, and helper/app integrity still limits input. No input automation, configured-form
visual correctness or full UI/script/headless parity is claimed. The owned app exited **0** at
**22:03:26 UTC**; native investigation ended well before 22:15 with no second experiment.

The UI explicitly states that authored settings do not yet activate outputs. Graphical mapping,
one-drag/one-Undo, the complete editor pair and runtime/native requirements remain open:
M05 stays **1/8 (13%)**, M04 **1/6**, M03 **7/9**, overall **3/10 (30%)**. This is the ninth
product capability; no gate closes from form/build evidence. Next is actual crop/warp rendering,
not another desktop automation or qualification investigation.

**Product batch 8:** started 2026-10-30 **21:44 UTC**, recorded stop **22:29 UTC**.
Deliver persistent Screen Manager/Screen/ordered Surface authoring through the atomic app
operation route, including command nodes, script methods, callbacks and embedded snippets.
Owner: `apps/mapgyver/src/screens`; public media/channel references connect existing sources.
Proof: create/configure/reorder/remove, invalid edits with no state/history change, source
delete/undo, project roundtrip and actual command/QuickJS execution. No native experiment is
needed for this backend deliverable. Investigation stops by **22:04 UTC** or two experiments.

Before implementation, M05's former single whole-phase placeholder is expanded to these eight
fixed gates. Overall phase count remains 10; none is accepted by defining this breakdown.

| # | Acceptance scope | State |
| --- | --- | --- |
| 1 | Default Screen Manager; atomic Screen/ordered Surface authoring with stable source references, validated targets/crops/quads, project/history roundtrip, commands, script methods, callbacks and app-owned snippets. | Accepted — batch 8 evidence below |
| 2 | Runtime crop followed by perspective warp matches reference patterns; invalid/singular mapping fails atomically. | Accepted — batch 10 reference and publication evidence above |
| 3 | Calibration with no source restores authored routing; overlapping Surfaces preserve order and alpha. | Accepted — batch 12 backend/runtime and finite GPU evidence |
| 4 | Physical fullscreen, physical desktop rectangle and virtual Spout targets work concurrently and continue when editors close/minimize. | Open — batches 13–15 verify all modes, three-consumer API acceptance and no-editor demand; physical pixels/full concurrent/minimize scope remain |
| 5 | Display catalog/recovery preserves intended identity across hotplug, mixed DPI and negative desktop origins. | Open — batch 15 implements catalog/exact identity and software loss/error/recovery; actual hotplug and full physical arrangement/DPI qualification remain |
| 6 | Screen editor/source selection uses backend intents; one drag is one undoable gesture. | Accepted again after redesign — [complete Screen input/history and stable canvas](evidence/mapgyver-m05-workbench-screen-acceptance-2026-11-01.json) preserves batch86's full scope; source/peer preservation, all corners, rejection, Escape and panel lifetime pass. |
| 7 | Surface Source editor exposes crop/mapping/calibration with coherent source, history and script behavior. | Open — [complete new-layout Source input](evidence/mapgyver-m05-workbench-source-input-2026-11-01.json) preserves batch87 form/calibration/all crop/origin/rejection/Escape/source-loss/history scope. Prior backend/script proof retained; native full-source preview remains unverified. |
| 8 | Full retained physical presentation matrix: exact geometry, alpha/color/reference patterns, 100/125/150/200% and mixed DPI, negative origins, overlays/pointer capture, measured alignment/input latency and editor independence. Unavailable configurations remain open. | Open |

**Batch 8 result:** `src/screens` owns persistent Screen Manager, Screen and Surface nodes and
the `mapgyver.screens.edit` route. CreateScreen builds the Screen and first full-source/full-screen
Surface as one detached subtree and one history entry. Backend labels/IDs, configure, rename,
ordered Screen/Surface moves, removal and read-only inspection work through typed operations.
Screens default to disabled/unconfigured; authored fullscreen identity, signed physical desktop
rectangle and exact-name virtual Spout target data does not itself create outputs.

Surfaces retain channel references while calibration is authored or a source is deleted.
Missing-source crop/calibration edits preserve routing; Undo restores the source. New assignment
validates ownership against the public media API. Invalid crops, nonfinite/concave/crossed/collapsed
quads, opacity and physical bounds are rejected before mutation. Off-screen convex/mirrored quads
are allowed. Source availability, native handles and device observations are not persisted.

Screen Manager's Add menu exposes Screen Operation, Create Screen and an embedded Screen Script.
`editScreens`/`inspectScreens`, command Execute/`execute()` and committed/rejected
`onAppOperationResult` callbacks use Golden's existing bounded atomic operation service. Disabled
or destroyed request owners and failed originating edit batches cannot leak queued operations.
Rust generates the UI DTOs; the typed operation adapter consumes them without domain defaults.
Shared backend label allocation moved to `src/app/labels.rs`; existing media behavior is retained.

Validation: **69/69** default-feature and **63/63** no-default-feature library tests pass, including
real QuickJS manager calls, command trigger/callbacks, production actor dispatch/Undo, project
roundtrips for all three target descriptions, order/removal/source recovery and generated DTO drift.
Strict all-feature bins/tests Clippy, Svelte checking, the bundled app build and both workspace
formatters pass. Implementation/checks finished by **21:55 UTC**, before the **22:29 UTC** stop;
the batch finishes early rather than expanding its scope. No native experiment or M03 investigation was needed.
Only M05 gate 1 closes: **1/8 (13%)**; overall remains **3/10 (30%)**, M03 **7/9**, M04 **1/6**.
Next product slice is the Screen Manager/Screen editor authoring UI; actual warp/calibration,
physical/Spout outputs and display/editor qualification retain gates 2–8.

### M04 acceptance items

**Qualification batch 52:** started 2026-10-31 **16:43 UTC**, recorded stop **17:28 UTC**.
M04 gate 6 is the only active work. Previous turn is progress: pushed `f18de683` adds the missing
public Picture reload path, generated contract/UI/snippet and passing worker/caller tests.
Verify that concrete new behavior, reusing current proven binaries and existing finite fixtures.
Fixed live matrix: (1) one isolated ordinary host runs the complete source HTTP/history/Save/New/
Load check including new reload/history assertions, then opens its bundled UI and verifies the
actual Reload Picture control sends the same asset intent, preserves the exact catalog and adds
no undo step; (2) run the updated existing Picture native pixel test on both hardware adapters,
whose in-place replacement now uses `ReloadAsset` plus normal recapture instead of the direct
runtime hook. Keep the previous 2/255 oracle, production limits, payload ownership and retirement.
At most two live invocations/20 minutes, also bounded by 17:28; no third corrective run, GPU timing,
driver, desktop recovery or other phase scope. Compile/check the browser helper before execution.
Only close gate 6 after both terminal results and a requirement-by-requirement M04 reconciliation.

Batch 52 live matrix starts **16:46 UTC**, cutoff **17:06 UTC**, before the 17:28 batch stop.
Checkout is clean at `f18de683` before adding the finite UI helper. Prerequisites remain terminal
for the unchanged Rust/UI candidate: **192 app tests / one existing hardware ignore**, **152 caller
executions**, strict all-target/all-feature Clippy, **58 frontend tests**, Svelte zero diagnostics,
ordinary bundled build and updated native test compilation. Candidate app/test hashes match batch
51's recorded values. New JS helpers pass syntax/Prettier; outbound UI intents are JSON, as the
existing WebSocket transport specifies. Invocation one is `media.mjs --ui-reload`; only after its
terminal success run the exact updated ignored Picture test as invocation two. A failure retains
gate 6; no extra invocation, threshold relaxation or alternative fixture is permitted this batch.

Invocation one passes in **2,375 ms** of checked operations: the full ordinary HTTP matrix,
including ReloadAsset followed by undo of the preceding relink and redo, preserves exact source
catalog/IDs; the real bundled browser Reload Picture control sends exactly one selected-asset
request, leaves no alert/page error, and preserves the preceding RemoveMedia redo/undo entry.
Chromium **149.0.7827.55**, viewport **1440×1000**; the owned browser closes and host exits **0**.
Proceed only with the predeclared updated native Picture test, invocation two, within the same
16:46–17:06 window. Native public-intent replacement remains unverified until its terminal result.

**Batch 52 result — M04 gate 6 accepted, 16:48 UTC:** invocation two passes **1/1 in 46.09 s**
on Intel Graphics LUID `0:93226` and NVIDIA GeForce RTX 5080 Laptop GPU LUID `0:94419`.
The existing fixed matrix now replaces the file through the public `ReloadAsset` intent plus
ordinary capture/publication. **54 complete output images / 33,177,836 texels** meet the original
**2/255** oracle, including partial/zero alpha, missing/JPEG recovery, sharing, undo, relink,
in-place replacement, unsupported-gamma rejection, recovery and 4K save/reload/recreation. Peer
prepared identity, upload sharing/no per-frame upload, stable references, observer pool release
and actual asset-worker/GPU retirement all pass. No tolerance, production budget, driver setting
or runtime code changes were needed in this batch.

The new [optional browser helper](../../apps/mapgyver/tests/browser/picture-reload.mjs) is loaded
only with `--ui-reload`; the default [HTTP test](../../apps/mapgyver/tests/headless/media.mjs)
keeps its ordinary Node-only path. Actual browser input records exactly one selected-asset reload
request, then verifies unchanged catalog, no error, preserved redo and undo, and zero page errors.
The same owned host verifies the complete source HTTP matrix with new reload/history assertions,
peer isolation and exact Save/New/Load restoration before normal exit. Both live invocations are
terminal successes within the original window; no third or corrective run occurs.

**Original M04 acceptance audit:**

| Requirement | Accepted evidence |
| --- | --- |
| Default Media Manager, detached creation/import, authoritative IDs, commands/methods/callbacks/snippets | Gate 1 module tests and app-owned registration; batch 51's 152 caller executions additionally verify returned owned identities, defaults, single committed callbacks and atomic history/allocation. |
| At least two differently sized channels per source kind, Automatic/Fixed sizing and shared consumers | Native Solid Color batch 47, Picture batches 48/52 and Shared Texture batch 49 use two channels in one relevant media and independent A/A/B consumers. Source extents/output policies and shared production/resources are checked. |
| Worker Picture decode, color/alpha and source/pixel/decode/cache bounds | Gate 3 codec/Golden Assets/renderer tests establish admission, worker execution, retained lease/cancellation/retirement and production 4K PNG bounds. Batches 48/52 observe actual PNG/JPEG output; unsupported layouts fail explicitly, not by silently raising budgets. |
| Missing/relinked/replaced pictures; one failed source leaves a healthy peer | Native Picture matrix plus public-intent worker tests and batch 52 replacement now exercise the real caller route. Reload changes no locator/asset/channel ID or authored history. No automatic watcher is claimed. |
| Exact independent Spout selection/discovery, absence/restart/rename/resize, isolated failure and explicit adapter mismatch | Accepted gate 4 / batch 49 checks actual outputs, advancing healthy peer, exact-name discovery, no implicit renamed selection, bounded oversize recovery and both cross-adapter error directions without fallback. |
| Media Manager UI, project/history, UI/script/headless parity and functional native resource recreation | Gate 5 actual UI authoring; batch 51 typed caller matrix, sparse/full codecs and source recreation evidence; batch 52 actual new Reload Picture UI/HTTP plus updated native replacement. Original source-gate tests retain controlled renderer/device teardown/recreation, upload/resource ownership and stable references. |

Candidate hashes match their actual runs: ordinary bundled app
`7e354546503c10fc2a532ecff8c4d187ed981903757c91bdb57363291973b1c4`; native test
`c9b851d1721f08bc375d8465939dec393055265d73ede5a1ba2da2f2797bb681`.
New JS helpers pass syntax/Prettier and both Cargo formatters/whitespace checks pass; Rust/UI
production sources are unchanged from the verified batch 51 candidate.

**M04 closes at 6/6 = 100%; overall is 4/10 = 40%.** Denominators, acceptance scope, prior failed
evidence and original budgets are unchanged. Native picker/preview input is not inferred from
the browser route and is not invented as an extra source gate. Genuine device removal/stuck
retirement and full rendering performance remain M03 gates 4/9; physical/display/editor/full-app
requirements stay with M05–M09. Return to M03 gate 4 as the only next active gate after delivery.

**Qualification batch 51:** started 2026-10-31 **16:08 UTC**, recorded stop **16:53 UTC**.
M04 gate 6 is the only active acceptance work. Owners: app media intents/module commands/QuickJS,
shared headless transport and existing source capture/rendering. Audit the original gate scope:
UI/script/headless intent parity, stable project/history references and native resource recreation
across Solid Color/Picture/Shared Texture. Reuse gates 1–5 and existing native evidence, identify
missing cross-source caller cases, and add only focused public-path checks needed to prove parity.
Do not repeat passing source pixel qualification or introduce picker/preview input as new gates.
Run software/build/Clippy prerequisites before any live check; at most two live invocations/20
minutes and the 16:53 batch stop. No other phase, driver, GPU timing or desktop recovery work.
Previous goal turn is progress: pushed `d35930dc` closes Media Manager UI gate 5 with actual input.

**Batch 51 fixed parity matrix:** clone one authored baseline with Solid Color/Picture/Shared
Texture, two channels each, Automatic/Fixed sizing and one shared Picture asset. Compare 27
mutation/inspection/rejection cases through UI intents, actual generic QuickJS, hosted QuickJS
and the public command trigger: configure, rename media/channel, reorder, remove channel/media,
inspect channel/manager/asset, relink/remove asset, wrong source kind and wrong channel owner.
Require identical typed results/catalogs, one committed callback for script/command callers,
read-only/rejected history neutrality, one-step mutation undo/redo and no replayed request. Also
verify creation/default/committed identity for all three sources × four callers (120 executions
total) and all-source history plus sparse/full project roundtrip without transient GPU data.
QuickJS normalizes JSON numeric representation; decode its actual reply through the Rust-owned
DTO before comparing values, rather than treating JSON `1` versus `1.0` as a domain difference.

The missing ordinary-host check uses only public HTTP intents: three media/six channels, explicit
source/size/filter settings, peer isolation, rename/reorder/remove/undo/redo, shared asset relink,
atomic rejection, Spout disconnect, Save/New/Load with exact catalog/IDs and orderly shutdown.
No renderer, OS target fixture or frontend is instantiated; native recreation is mapped from
the already accepted source evidence. Software/build/Clippy prerequisites must finish first.

Batch 51 ordinary-host baseline starts **16:18 UTC**, cutoff **16:38 UTC**, before the 16:53
batch stop. Prerequisites are terminal: **191 app library tests pass / one existing hardware
ignore**, including the full fixed caller matrix and sparse/full roundtrip; strict all-target/
all-feature MapGyver Clippy, ordinary bundled binary build, JS syntax/Prettier, both Cargo
formatters and whitespace checks pass. Execute only `tests/headless/media.mjs`; allow one
corrective invocation only for a concrete failure and keep the original source/native limits.

The first live invocation passes source settings, identity/peer isolation, history, relink,
rejection, disconnect and Save/New, and the host loads the saved project successfully. The test
then fails because it guessed an `ok` property on the load reply. The Rust-owned `UiProjectPathDto`
actually returns `path`, optional UI state and recovery diagnostics. Correct the observer to check
that DTO and the unchanged exact-catalog/reference assertions; no host behavior or acceptance is
relaxed. The failed invocation still exits its owned host normally with code zero. Recheck JS
syntax/formatting, then use the one reserved final invocation for the original matrix.

The second/final ordinary-host invocation passes the original matrix in **282 ms**, then exits
normally with code zero. Completion audit identifies allocation replies as the remaining software
comparison gap: add channel for all three source kinds, ImportAsset, and atomic ImportPicture
for a new media or an existing fixed channel. Add those six cases × four callers before final
software execution, requiring returned owned IDs, backend defaults, preserved output settings,
one-step undo/redo and unchanged peers. This makes **144 caller executions**, without another
live run, added feature, budget change or scope beyond the original channel/import parity gate.

**Batch 51 partial result — M04 gate 6 stays open:** **192 app library tests pass / one existing hardware
ignore**, including four focused parity/codec tests. The final strict all-target/all-feature
MapGyver Clippy run passes. The ordinary binary build, JS syntax/Prettier, both Cargo formatters
and whitespace checks pass. The second/final ordinary-host invocation passes the original fixed
matrix in **282 ms** and returns normally with exit **0**; no owned MapGyver process remains.
The first invocation's incorrect load-response assumption remains recorded above. Other observer
corrections used the actual reply DTO (no namespace field) and Rust-typed JSON numeric comparison;
no runtime behavior, tolerance or budget was changed to make the original checks pass.

| Final parity requirement | Verified evidence |
| --- | --- |
| UI, generic QuickJS, hosted module and command caller parity | [Focused parity tests](../../apps/mapgyver/src/media/tests/commands/parity.rs): 29 mutation/inspection/rejection cases × four callers, source creation × three kinds × four callers, and six allocation cases × four callers = **152 executions**, including valid/wrong-owner reload. All compare committed typed values/IDs and peer state, callback correlation/count, rejection/history neutrality, one-step undo/redo and no request replay. Existing [module tests](../../apps/mapgyver/src/media/tests/commands.rs) retain public presets, passive commands, cancellation, read-only inspection, hosted snippets and app-owned callback wiring. |
| Real UI rather than only a transport mock | Accepted gate 5 / batch 50: [browser evidence](evidence/mapgyver-m04-ui-2026-10-31.json) covers actual creation/settings/rename/reorder/delete, Picture import/relink, independent Spout selectors, guards, undo and reload, with read-only backend correlation. Its native-picker/preview limits remain explicit; no new prerequisite is invented. |
| Ordinary headless host uses the same source intents | [Repeatable HTTP check](../../apps/mapgyver/tests/headless/media.mjs), run against ordinary `MapGyver.exe --headless --no-frontend --no-remote` with private APPDATA: three media/six channels, typed source/size/filter settings, peer continuity, shared asset relink, atomic rejection, Spout disconnect, history and exact Save/New/Load catalog/reference restoration. The supported sentinel yields exit zero. No frontend, renderer or OS application target is instantiated. Newly added `ReloadAsset`/history assertions remain unexecuted; that version is not claimed to pass. |
| Stable authored references and project/history roundtrip across source types | New parity tests verify all three sources' Automatic/Fixed definitions, shared asset references and history through **both sparse and full codecs**, rejecting transient GPU/resource fields from sparse output. Ordinary-host Save/New/Load preserves the exact catalog. Existing [Picture import tests](../../apps/mapgyver/src/media/tests/picture_import.rs) retain atomic import/assignment and QuickJS returned asset/channel identity. |
| Functional native resources and recreation across source types | Accepted gate 2 / [Solid Color native test](../../apps/mapgyver/tests/solid_color.rs): actual output sharing/color/extents, save/reload and fresh renderer/device on Intel/NVIDIA. Accepted gate 3 / [Picture native test](../../apps/mapgyver/tests/picture_pixels/mod.rs): decode/upload/share/relink/replacement, 4K save/reload/recreate and asset/GPU retirement on both adapters. Accepted gate 4 / [Shared Texture native test](../../apps/mapgyver/tests/shared_texture/mod.rs) retains independent inputs and recovery on both adapters; [existing channel recreation evidence](mapgyver-m03-render-foundations.md#app-channel-publication-bridge--2026-10-29) verifies authored selector/resize/reconnect/history/project identity, stale observation rejection and a fresh same-adapter runtime/device. This is controlled resource recreation; genuine removal/stuck retirement remains M03 gate 4. |
| One Rust-owned contract and backend semantics | Existing generated-output drift test is included in the passing app suite. UI and scripts pass explicit intents; backend defaults/IDs, asset rules and history remain app-owned. No copied protocol declaration or caller compatibility shim is introduced. |

The earlier passing ordinary-host baseline used binary SHA-256
`56d0e7957e2e1795421c4291a1ef7b61a691c9b63113baf8f7b7540e38276f69`.
**Completion audit correction, before committing:** the Picture runtime retry hook was only used
by tests/custom hosts. The ordinary app offered no caller intent to reprepare an in-place replaced
file. That is a real gate 6 parity gap, not an optional optimization. Add `ReloadAsset` to the
Rust-owned contract/generated TS, validate the asset and advance only its runtime revision, and
expose Reload Picture in the existing UI plus the app-owned script snippet. It performs no file IO
on the actor, changes no authored locator/IDs/history, and reuses capture invalidation and worker
decode rather than adding a watcher or transport-specific command. The public-intent software
worker test proves missing/replaced recovery, stale capture rejection, unchanged serialized data/
history and healthy-peer retention. Caller parity covers success and wrong-owner rejection.

**Still open:** actual Reload Picture input, the new HTTP reload/history assertions and the updated
native Picture replacement case now using the public intent. The earlier **282 ms** HTTP/native
hook evidence does not prove these new paths. Both live invocations were already consumed; do not
run a third. Checkpoint useful implementation and tests, then qualify this concrete new behavior
in the next bounded batch. M04 remains **5/6 = 83%**, overall **3/10 = 30%**, unchanged denominators
and original requirements. No M03/other phase work starts before this gate is resolved.

Checkpoint validation is terminal: **192 app tests / one existing hardware ignore**, including
all **152 caller executions**, the public-intent worker recovery test and generated TS drift;
strict all-target/all-feature Clippy; **58 frontend tests**, Svelte **0 errors/0 warnings** and
UI/JS Prettier/syntax; both Cargo formatters; ordinary bundled build and updated native test
compilation. Restored the ordinary default-feature app binary after the qualification-feature
test build so the next UI/HTTP run uses the intended host. Candidate hashes, not live passes:
`target/debug/MapGyver.exe` SHA-256
`7e354546503c10fc2a532ecff8c4d187ed981903757c91bdb57363291973b1c4`;
`picture_rendering-a0263ade992b6f1b.exe` SHA-256
`c9b851d1721f08bc375d8465939dec393055265d73ede5a1ba2da2f2797bb681`.

**Qualification batch 50:** started 2026-10-31 **15:54 UTC**, recorded stop **16:39 UTC**.
M04 Media Manager UI gate 5 is the only active acceptance work. Owners: app-owned media components
and their existing backend intent contract. Map controls/software coverage, then verify inspection,
channel/source/size/rename/reorder/delete through the running interface. Prefer the app's supported
browser/headless route if it permits ordinary UI input without native integrity barriers; preserve
native preview qualification limits. Record fixed interaction cases/prerequisites before execution;
at most two interaction experiments/20 minutes and the 16:39 batch stop. Fix demonstrated defects
at their owning boundary; no UI/domain duplication, desktop recovery or other phase expansion.
Previous goal turn is progress: pushed `a9a280d6` closes Shared Texture gate 4 with actual evidence.

**Batch 50 fixed interaction matrix, before execution:** use a fresh isolated ordinary headless
host serving its bundled UI, and the existing Playwright Chromium installation (the in-app browser
is unavailable). Through actual controls: inspect the default empty Media Manager; create Solid
Color/Picture/Shared Texture; rename media and channels; add/select/reorder/delete a channel while
retaining one; author independent Solid RGBA and automatic/fixed extents/filter; import/assign,
select and relink a Picture host locator; set/change/clear an exact Spout selector and its admitted
layout; delete a media and inspect the resulting catalog. Correlate committed UI results with
read-only public backend inspection and verify stable references/peer independence. Observe the
rendered layout and browser-native-preview limitation; no native picker/discovery/preview input
claim is inferred from the browser route. Undo/reload may confirm authoring continuity, while
complete cross-source script/headless parity remains gate 6. Only owned throwaway project data
is edited. Keep 1440×1000 browser input fixed; capture observations without a new framework.

Prerequisites pass: rebuilt ordinary MapGyver bundled binary, Svelte **zero errors/warnings**,
Prettier and **58 frontend tests**. Interaction baseline starts **15:57 UTC**, cutoff **16:17 UTC**,
also bounded by 16:39. The owned host is `http://127.0.0.1:59475`, executive session **57325**, with
private temporary APPDATA and its recorded orderly-shutdown sentinel. No user project is opened.

**Batch 50 result — Media Manager UI gate 5 accepted, 16:04 UTC:** actual browser input through
the bundled UI passes the fixed matrix, with public **read-only** `InspectManager` calls confirming
committed backend values. [Compact evidence](evidence/mapgyver-m04-ui-2026-10-31.json) records the
binary/browser, cases, observer failures and limits; [catalog](evidence/mapgyver-m04-ui-catalog-2026-10-31.png)
and [channel editor](evidence/mapgyver-m04-ui-channel-2026-10-31.png) captures show the actual layout.
No mock transport or direct backend authoring substitutes for the UI actions.

- Empty/default manager, creation of all three M04 source media, media/channel renaming, add/select/
  reorder/delete and last-channel protection pass. Independent Solid settings preserve the Primary
  defaults while Secondary becomes Fixed 320×180 / Nearest / RGBA `[.25,.5,.75,.5]`. Delete/undo
  restores exact channel IDs and order.
- Picture import/assign and relink retain the asset/channel IDs; clearing/reselecting the catalog
  asset works. Top-level absolute-path import creates asset/media/channel atomically; one UI undo
  restores the exact prior catalog. Paths deliberately refer to missing files: decoding/pixels are
  owned by gate 3, not inferred from this authoring check.
- Shared Texture sends exact user-selected sender/layout/size/filter values. Selecting, changing
  and clearing A leaves B's complete committed channel unchanged. Browser mode explicitly offers
  manual host selectors/locators and shows `Preview unavailable`; native picker/discovered-sender
  widget and preview input are not claimed. Full intent parity remains gate 6; those platform
  helpers are evidence limits, not new acceptance prerequisites added by this batch.
- Media deletion retains the reusable shared asset and unrelated media; undo restores the exact
  media/channel/source identity. Refresh and browser reload retain the committed catalog. Browser
  reload does not constitute engine project reload. No page errors or final manager alerts occur.

Two owned browser lifetimes are used within the original interaction window. The first exact tab
name locator omitted its accessible close-button name and timed out before authoring; kernel reset
closed that browser, verified absent before replacement. The second uses observed tab text and
completes the fixed matrix. A Node REPL cross-realm strict-equality issue is resolved by normalizing
both JSON operands and checking original committed values; no authoring is repeated to mask it.
No production source change, new browser dependency, platform-security change or UI recovery
framework is needed. The headless host stays live throughout, then exits **0** after its supported
shutdown sentinel; the owned browser is closed.

Validation: ordinary bundled MapGyver build, **58 frontend tests**, Svelte **0 errors/0 warnings**,
Prettier, both Cargo formatters and whitespace checks pass. Rust/native source behavior was not
changed or redundantly requalified. Binary SHA-256:
`56d0e7957e2e1795421c4291a1ef7b61a691c9b63113baf8f7b7540e38276f69`.
Only M04 gate 5 closes: **5/6 = 83%**; overall remains **3/10 = 30%**, denominators unchanged.
Stay in M04 for full UI/script/headless/history/native-recreation intent parity gate 6. Other
phase acceptance and physical/performance qualification stay open. Native picker/preview input
was not observed; derive remaining prerequisites from the original gates without expanding scope.

**Qualification batch 49:** started 2026-10-31 **15:45 UTC**, recorded stop **16:30 UTC**.
M04 gate 4 Shared Texture is the only active acceptance work. Owners: app media capture/rendering
and Golden Video's existing native discovery/input supervision. Map prior evidence to independent
channels and shared consumers, automatic/fixed sizes, sender absence/restart/rename/resize, healthy
peer continuity and explicit adapter mismatch. Verify missing app-path cases with existing owned
sender fixtures and finite observers; do not replace backend contracts or add a diagnostic framework.
Run software/build/Clippy prerequisites first. Record the fixed native matrix/start before execution;
at most two invocations/20 minutes, also bounded by 16:30. Preserve budgets and failures; no driver,
GPU timing, physical-display/UI recovery or other phase work. Partial evidence leaves gate 4 open.
Previous goal turn is progress: pushed `eb311af9` closes Picture gate 3 with actual native evidence.

**Batch 49 fixed native matrix, before execution:** on each enumerated hardware adapter, one
Shared Texture media has Automatic A and Fixed 3×2 B, selecting two owned producer processes.
Three independent Spout pixel observers use A/A/B; two productions/subscriptions must serve them.
Compare complete tiny output images at the existing 2/255 tolerance for: independent red/blue;
A absent with B still publishing blue; A restart/resize 2×2→4×3 green; A oversized 8×8 against its
4×4 admitted maximum, transparent rejection and B continuity; admitted-size recovery; rename to
a new exact sender without implicitly selecting it, then explicit selection; B input resize to
4×3 cyan while its channel stays Fixed 3×2; A explicitly selecting a producer on the other hardware
adapter; and recovery to the same-adapter sender. Cross-adapter acceptance requires a failed
source-local HRESULT, no foreign publications or adapter/device fallback, transparent A and live B.
Keep stable channel references, require installed-generation output submission fences, discovery
start/stop/restart/rename freshness and actual observer/render/process retirement. Prior app tests
retain project/history/recreation evidence; full cross-source intent parity remains gate 6.

Use existing 64 KiB renderer resources, 4×4 input admission, four bounded source slots and a
separate 64 KiB receiving/staging/CPU observer pool. No production code, color tolerance, adapter
fallback, driver setting, source-budget or cadence changes are planned. A native result contrary
to those expectations must remain a failure and guide the one reserved corrective invocation.

Batch 49 native baseline starts **15:50 UTC**, cutoff **16:10 UTC**, before the 16:30 batch stop.
Prerequisites are terminal: **188 app tests / one existing hardware ignore**, Golden Video
**161 unit + four public integration tests / 20 existing hardware ignores**, strict all-target/
all-feature MapGyver Clippy, both Cargo formatters, rebuilt `native_source_fixture.exe` and
`channel_rendering-f720e7eaa9bd21d7.exe`. Run only the exact new ignored Shared Texture acceptance
test, looping both adapters within the same fixed invocation. Reserve the second invocation for
a concrete failure; no additional probe/cadence/driver investigation is authorized by this batch.

**Batch 49 result — Shared Texture gate 4 accepted, 15:51 UTC:** the first/only invocation passes
**1/1 in 80.70 s**, before both cutoffs; no second experiment is needed. Intel Graphics LUID
`0:93226` and NVIDIA GeForce RTX 5080 Laptop GPU LUID `0:94419` pass the same ten-epoch matrix.
All **60 complete images / 280 texels** meet the fixed **2/255** oracle; reported samples are exact.
The small fixture deliberately tests source identity, policies, recovery and pixels, not scale or
cadence. No production code, admission budget, color tolerance or driver/overlay setting changes.

| Shared Texture requirement | Verified evidence |
| --- | --- |
| Independent channels and shared consumers; automatic/fixed sizes | [Fixed app/native test](../../apps/mapgyver/tests/shared_texture/mod.rs): one media owns Automatic A and Fixed 3×2 B, distinct native subscription bindings, and three independent observers on A/A/B. A resolves 2×2→4×3 while B's 3×2 channel persists through its own 3×2→4×3 input change. Installed-plan output fences precede every full-image observation. |
| Discovery, absence/restart/rename/resize and rejected input recovery | Same test verifies exact-name catalog freshness at start/stop/restart/rename/exit, transparent unavailable output, 8×8 rejection against 4×4 admission, admitted-size recovery, no implicit renamed selection and explicit re-selection. [Discovery unit tests](../../crates/golden_video/src/native/discovery/tests/mod.rs) retain finite bounds, exact names, invalid UTF-8 and registry-churn handling. |
| One failed channel remains isolated | Every failure retains the peer's exact prepared channel/source, requires advancing peer publications, and observes its live blue/cyan output while the failed channel is transparent. The peer remains intact through absence, oversize, rename and cross-adapter failure. |
| Explicit adapter mismatch without fallback, followed by recovery | Known foreign producers on both opposite hardware adapters fail source-local shared-resource opening with HRESULT `0x80070057` (`E_INVALIDARG`), state Recovering, zero publications/copies/bridge slots. Selected renderer adapter/device remain unchanged; the healthy source continues. Explicit selection back to the local producer restores output. This proves the known cross-adapter scenario; it does not classify arbitrary HRESULT failures as adapter mismatch or claim a dedicated UI diagnostic. |
| Stable references, project/history and resource lifetime | New native test retains references throughout and waits observer/render retirement plus exit-zero owned producers and disappearance from discovery. [Existing app native evidence](mapgyver-m03-render-foundations.md#app-channel-publication-bridge--2026-10-29) retains selector/resize/reconnect, undo, codec reload, stale project/device/layout rejection and fresh runtime/device recreation. [App publication tests](../../apps/mapgyver/src/media/rendering/tests/publication.rs) retain sharing, history/reload, placeholders and fresh selector bindings. Full cross-source UI/script/headless parity remains gate 6. |

Validation: **188 passing app tests / one existing hardware ignore**; Golden Video **161 unit +
four public integration tests pass / 20 existing hardware ignores**; strict all-target/all-feature
MapGyver Clippy, both Cargo formatters, fixture/test builds and whitespace checks pass. Qualified
test SHA-256 (`channel_rendering-f720e7eaa9bd21d7.exe`):
`7ce17165c3b7270869df77dc607c1bb5a963d07c6c8fe162c079be2d0bc0a7e0`.
Producer SHA-256 (`native_source_fixture.exe`):
`7e172879ef8131016af907da9c4e8b66896a2d7ed85b215fa4b0fe046b378504`.
Only M04 gate 4 closes: **4/6 = 67%**; overall remains **3/10 = 30%**, denominators unchanged.
Stay in M04 for Media Manager UI gate 5, then full intent parity gate 6. No other phase gate,
physical-output/UI input, device-removal or performance qualification is inferred.

**Qualification batch 48:** started 2026-10-31 **15:26 UTC**, recorded stop **16:11 UTC**.
M04 remains the only active phase. Close Picture gate 3 only after mapping the full existing
worker/decode/cache bounds and asset-lifecycle evidence, then verifying the missing native pixel
cases through app authoring/capture/ChannelRenderer. Cover two differently sized channels in one
Picture media, automatic/fixed sizing, color/alpha, shared consumers, missing/relinked/replaced
images and stable references with a healthy independent channel. Use existing fixtures and the
finite pixel observer; no new renderer or diagnostic framework. Preserve codec/color limits and
honest failure states. Native execution follows terminal software/build/Clippy prerequisites and
is limited to two invocations/20 minutes and the 16:11 batch stop. No driver/overlay/UI recovery,
GPU timing profiling or other phase expansion; partial evidence leaves gate 3 open.

**Batch 48 fixed matrix, before native execution:** one Picture media, two stable channels,
three independent Spout receivers on routes A/A/B, on every enumerated hardware adapter. A uses
Automatic sizing; B stays Fixed 3×2. Compare every output texel at the existing 2/255 tolerance:
2×2 PNG straight alpha (including transparent hidden RGB), absent B/transparent fallback,
automatic missing-file recovery to 5×3 JPEG, shared JPEG preparation without duplicate upload,
undo, relink to partial-alpha PNG, in-place replacement through the existing explicit IO retry
hook, unsupported gamma rejection with healthy A retained, and recovery back to JPEG. Save/reload
stable references, recreate native rendering and compare two complete 3840×2160 PNG outputs plus
the independent JPEG output. Check upload counts stay fixed across subsequent frames, no native
input receivers, and both asset-worker and GPU retirement. This does not claim an automatic file
watcher or GUI retry control; complete source intent parity remains gate 6.

Keep production limits unchanged: 64 MiB encoded source, 128 MiB decode, 256 MiB prepared cache,
512 MiB renderer resources, 257/260 MiB input/retained-input budgets. The finite observer uses a
separate bounded 192 MiB pool for receiving/staging/CPU pixels of concurrent 4K outputs. Existing
worker tests must establish admission-before-decode, cancellation/stale publication, bounded IO,
retained clone/cache pressure and retirement; codec tests cover PNG/JPEG/color/orientation errors.
Large JPEG is not qualified here and remains subject to the documented decode admission limit.

Batch 48 native baseline starts **15:41 UTC**, cutoff **16:01 UTC**, also before the 16:11 batch
stop. Prerequisites are terminal: **188 MapGyver library tests / one existing hardware ignore**,
**17 Golden Assets tests**, strict all-target/all-feature MapGyver Clippy, both Cargo formatters
and the exact `picture_rendering-a0263ade992b6f1b.exe` build. Execute only the fixed ignored
Picture pixel test, looping hardware adapters in that single invocation. Reserve invocation two
for a concrete failure and retain all original tolerances, budgets and failure evidence.

Invocation one fails after passing missing/recovery, JPEG sharing, undo, relink-alpha and explicit
in-place replacement pixel epochs: the test expected `Unavailable` for an unsupported explicit
gamma, but the renderer correctly reports `Incompatible`. Correct only that observer expectation;
the codec rejection and acceptance requirement stay intact. The observer's watchdog also treated
its completion channel disconnect during panic as timeout; distinguish actual timeout so a test
failure retains its original diagnosis. Rebuild and rerun the same fixed matrix once, within the
original window. Unsupported-gamma pixels, subsequent recovery and 4K remain unverified until then.

**Batch 48 result — Picture gate 3 accepted, 15:43 UTC:** the second/final invocation passes
**1/1 in 46.09 s**, before the original 16:01 native cutoff and 16:11 batch stop. Both enumerated
hardware adapters pass the same matrix: Intel Graphics LUID `0:93226` and NVIDIA GeForce RTX 5080
Laptop GPU LUID `0:94419`. Across nine epochs × three outputs × two adapters, **54 complete images /
33,177,836 texels** meet the unchanged **2/255** oracle. JPEG blue observes 254 against expected
255; tested PNG alpha/colors are exact in reported samples. Unsupported explicit gamma reports
`Incompatible` and transparent output while the independent channel retains its prepared source.
Missing JPEG recovers automatically; replacement explicitly invokes the existing IO retry hook
without authored history. This is not automatic file-watcher or UI retry acceptance.

Automatic channel extents resolve to 2×2 PNG, 5×3 JPEG and 3840×2160 PNG; the second channel stays
Fixed 3×2. Output A's first fixture deliberately remains 2×2 through its uniform JPEG epoch;
the recreated 4K fixture observes every texel of both 3840×2160 outputs. Shared JPEG consumers
retain the same prepared texture without another upload; subsequent frames do not upload images
again. Save/reload preserves both channel and asset IDs. No native input receivers are created;
observer pool release and actual asset-worker/GPU retirement pass on both adapters.

| Picture requirement | Verified evidence |
| --- | --- |
| Worker preparation and bounded IO/decode/cache lifetime | [Golden Assets preparation tests](../../crates/golden_assets/tests/preparation.rs): background worker identity, source/seek/aggregate-read bounds, admission before decoder entry, held-clone item/byte pressure, cancellation, stale binding/file replacement rejection and final retirement off the consumer. All 17 crate tests pass. |
| PNG/JPEG, color/alpha, dimensions and decode workspace | [App codec tests](../../apps/mapgyver/src/media/picture/tests/mod.rs): straight-alpha/sRGB metadata, grayscale/palette transparency, opaque JPEG, explicit unsupported color/16-bit rejection, pre-decode dimension/allocation admission, malformed/truncated inputs and EXIF orientation. Included in 188 passing app tests. |
| Differing channels, production 4K budgets, lifecycle and relocation | [App renderer tests](../../apps/mapgyver/src/media/rendering/tests/pictures.rs): shared 4K preparation at production limits, automatic/fixed sizes, source sharing, undo/relink/reload, missing/replaced images, project-root relocation, healthy peer continuity and close/retirement. Included in the same app run. |
| Actual color/alpha, sharing, missing/relink/replacement and recreate | [Fixed native pixel test](../../apps/mapgyver/tests/picture_pixels/mod.rs), driven through app authoring/capture/ChannelRenderer and independent Spout probe devices; the successful second invocation covers the complete predeclared matrix above. |

Strict all-target/all-feature MapGyver Clippy, both Cargo formatters and whitespace checks pass.
Qualified binary `target/debug/deps/picture_rendering-a0263ade992b6f1b.exe` SHA-256:
`a1e6d28a946368a54345e707f4297ae2e865ce16770935e00508f6813ec4e914`.
Only M04 gate 3 closes: **3/6 = 50%**; overall remains **3/10 = 30%**, denominators unchanged.
Gate 4 Shared Texture is next in the same phase; UI/full intent parity retain gates 5–6.
No production code/budget/tolerance changes, large-JPEG qualification, physical display/UI input,
performance profiling or other phase acceptance are inferred. Retain invocation one's observer
failure above; do not rerun this passing qualification just to accumulate evidence.

**Qualification batch 47:** started 2026-10-31 **14:45 UTC**, recorded stop **15:30 UTC**.
Use the fifth diagnostic allocation (42 product deliveries permit ten; four used). Qualify the
fixed Solid Color gate 2 through app-owned authoring/capture/ChannelRenderer and actual D3D11/Spout
consumers: two channels in one media, differing automatic/fixed extents, transparent/partial/opaque
color and alpha, and two independent consumers sharing one channel. Preserve backend defaults,
reference identity and shared production; do not substitute mock pixels or an unrelated renderer.

Existing public fixtures decode markers, not arbitrary colors. Add only the qualification-only
finite pixel observation needed at Golden's existing native probe boundary, with explicit payload/
completion ownership and bounds. Verify software/defaults/sharing and compile/Clippy first. Before
native execution record the fixed matrix/start/cutoff; allow at most two invocations/20 minutes,
also bounded by 15:30 UTC. No driver/overlay/display/UI recovery, cadence profiling or M03/M08
parked investigation. Close gate 2 only if its full original scope is demonstrated.

**Fixed pixel matrix (before native execution):** one Solid Color media owns two stable channels.
Initially A is backend-default Automatic **1920×1080**, B is Fixed **7×3**. Three independent
Spout consumer/probe devices receive routes **A/A/B**, sharing exactly two app productions.
Straight linear colors are `[.25,.5,.75,.5]` and `[.75,.5,.25,.25]`; next A becomes transparent
and B opaque. Then swap Automatic/Fixed policies, save/reload with the same references and recreate
the renderer. Compare every texel of all three outputs in every epoch to an independent sRGB/
straight-alpha oracle, tolerance **2/255**, on every enumerated hardware adapter. Use fresh probes
per epoch and require each output's accepted submission to reach the newly installed plan frame.
No colour tolerance or production budget changes after observing native results. Renderer resource
cap and the shared receiving/staging/CPU tight-texel payload pool are each **64 MiB**; each probe
retains staging until its actual GPU completion/removal and CPU pixels retain their reservation.

Batch 47's native baseline starts **15:13 UTC**, cutoff **15:30 UTC** (the original batch stop,
earlier than a new 20-minute window). Prerequisites are terminal: **188 app tests / one existing
hardware ignore**, **161 Golden Video qualification-feature software tests / 20 existing hardware
ignores**, the focused sharing/alpha plan test and CPU snapshot-budget test, strict all-target/
all-feature MapGyver/Golden Video Clippy, Chataigne compile, generated drift, both Cargo formatters,
whitespace and the current `solid_color-5bbcad322b32fda7.exe` build. Run its single explicit ignored
acceptance test, looping the enumerated hardware adapters within one fixed experiment. The second
invocation is reserved for a concrete failure; no extra probe, tolerance change or GPU tuning.

**Batch 47 result / gate 2 acceptance (15:14 UTC):** the first/only native invocation passes
**1/1 in 20.82 s** on **Intel Graphics (LUID 0:93226)** and **NVIDIA GeForce RTX 5080 Laptop GPU
(LUID 0:94419)**. Across three epochs per adapter, **18 complete images / 20,736,168 texels** meet
the fixed **2/255** sRGB/alpha oracle. Partial-alpha, transparent-black and opaque outputs pass;
sample deviations are zero or one code value, without changing the tolerance.
The executed test binary SHA-256 is
`6e5cfb649a1161c4691a2ac263784066aee0c9d58a21987b157d8858ccc63950`.

The two channels belong to one app-owned Solid Color media. Automatic 1920×1080 and Fixed 7×3
work concurrently, then exchange policies across save/reload and renderer recreation with stable
references. Three native output routes A/A/B and independent probe devices share two productions;
the software plan check proves two passes/three outputs and shared target identity. Native output
acceptance must reach the installed plan's frame token before sampling. Both channel resources
and every probe's receiving/staging/CPU reservations retire; renderer retirement completes normally.
There are no input receivers or uploaded images in the Solid Color renderer.

Golden's qualification-only `SpoutProbe::capture_pixels` reuses its owned device/receiver and
completion fence. It nonblockingly stages the single observed image, then returns raw RGBA8 and
observed extent with a retained CPU payload reservation. It does not infer color/alpha tags or
expose native objects, and no production readiness/frame-transport path calls it. Fresh probes
give fresh epoch observations. Bounds charge tight texel payloads, separate from driver padding.

**Validation:** **188 app tests / one existing hardware ignore**, **161 Golden Video software
tests / 20 existing hardware ignores**, the new shared-production/linear-alpha plan test and
CPU snapshot-budget lifetime test, strict all-target/all-feature MapGyver/Golden Video Clippy,
Chataigne compile, generated drift, native-test build, both Cargo formatters and whitespace checks
pass. No second native run, UI/display/driver/overlay change, performance profiler or parked
investigation occurred. Native coverage is source output, not physical presentation or UI input.

**Only M04 gate 2 closes:** **2/6 = 33%**; overall remains **3/10 = 30%**, denominators unchanged.
Picture, Shared Texture, UI and complete cross-source parity retain gates 3–6. No other phase gate
closes. Historical delivery counts are **42 product / five qualification batches**; the user's
completion-first directive now overrides quotas. Stay in M04 and close Picture gate 3 next,
retaining the explicit return queue for every other incomplete milestone.

**Product batch 7:** started 2026-10-30 **21:05 UTC**, recorded stop **21:50 UTC**.
Deliver a working native Picture path for ordinary HD/4K images within the declared source-memory
contract. Owners: MapGyver preview configuration and the existing image admission/rendering path.
Proof: source-budget/admission regressions, app/build checks and at most two native functional
experiments. The retained tiny-Picture run already identifies admission failure; inspect the
configuration/ownership boundary before choosing a correction. Native investigation stops by
**21:25 UTC** or its second experiment. No timing/clock/overlay qualification or M03 stash work.

**Batch 7 result:** native 4K PNG Picture integration works. The ordinary app displayed a red
3840×2160 PNG, then a green 1920×1080 PNG after explicit relink, then Source Unavailable for
a missing path, then red again after relink recovery. Public loopback backend operations drove
authoring; supported native screenshots observed output. Channel and asset identifiers stayed
stable. The single native experiment ended normally with exit 0 at **21:19:17 UTC**, before the
investigation limit; no second experiment or timing/overlay investigation was needed. This is
scaled preview evidence, not physical 4K scanout or interactive form verification.

Production preview admission now lives in `src/media/preview/config.rs`. Raw input caps change
explicitly from **1/4 MiB** per-plan/aggregate to **257/260 MiB**, accounting for shared retained
image leases already bounded by the unchanged **256 MiB** preparation cache. There is no extra
pixel copy. Source input remains **64 MiB**, decode workspace **128 MiB**, GPU resources
**512 MiB**, native Spout input **256 MiB**; M03 qualification ceilings are unchanged. The first
4K regression timed out because PNG admission summed non-overlapping buffer phases. Charging
the peak live codec/raw, raw/RGBA and RGBA/orientation phases admits 4K PNG within the same
decode cap. JPEG's conservative admission is unchanged; no 4K JPEG claim is made.

Validation: **56/56** MapGyver library tests pass, including production-config 4K PNG sharing
across automatic 4K and fixed 640×360 channels; the bundled app builds, strict all-feature
Clippy for bins/tests passes, and the no-default-features bins check passes. Both workspace
formatters pass. Implementation and checks finished by **21:44 UTC**, within the recorded stop.
M04 stays **1/6**: this narrow native flow does
not close its full Picture/source lifecycle, color/alpha, sharing, UI or parity requirements.
Next is M05 Screen/Surface authoring, using the plan's existing working-contract prerequisite;
remaining M04 acceptance is retained rather than transferred to M09.

**Product batch 6:** started 2026-10-30 **20:34 UTC**, recorded stop **21:19 UTC**.
Deliver bounded native Spout sender discovery with explicit channel selection in the Shared
Texture editor. Owners: Golden Video's native SDK boundary and MapGyver's metadata worker/UI.
Proof: bounded decoding/service tests, generated DTO drift, Svelte/build/Clippy checks and one
bounded native discovery observation using existing sender fixtures where practical. Discovery
must not initialize the renderer, poll the engine or select an active sender implicitly. Native
Picture/M03 qualification remains parked. Any investigation stops after 20 minutes/two experiments.

Native functional experiment 1 will reuse the existing independent source fixture: one unique
owned sender must be absent before start, present after accepted frames, absent after stop,
present after restart and absent after an exit-zero teardown. The discovery caller opens no
renderer/receiver. This tests registry freshness and exact-name enumeration, not timing/resource
qualification. A failure permits one focused correction/rerun within the investigation limit;
do not add a sender-count sweep or resume M03 measurement work.

**Product batch 5:** started 2026-10-30 **19:59 UTC**, recorded stop **20:44 UTC**.
Deliver Media Manager command nodes, script-callable methods, committed-result callbacks and
app-owned snippets through the existing typed media operations. Owner: `apps/mapgyver/src/media`
and its app registration. Proof: real command/QuickJS execution, atomic failure/history/project
tests, generated-contract checks and the bundled app build. Keep native qualification parked.
Any investigation stops after 20 minutes/two experiments; preserve incomplete integration if
the public command boundary requires more than this batch, without shrinking full M04 scope.

Defined before M04 source implementation on 2026-10-30. This replaces the provisional
whole-phase item with six fixed gates covering the same complete acceptance; no scope is
removed and all six start open. M04 percentage is verified closed gates divided by six;
overall remains fully accepted phases divided by ten.

| Gate | Acceptance | State |
| --- | --- | --- |
| 1 | Media Manager in the default project, backend authoring/inspection and module commands, script methods, callbacks and app-owned snippets wired through the same operations | Accepted — [evidence](#media-module-commands-and-scripts--2026-10-30) |
| 2 | Solid Color with at least two differently sized channels, automatic/fixed sizing, color/alpha and shared consumers through the app/native rendering path | Accepted — batch 47: full-image color/alpha/extents/sharing/reload/recreation on Intel and NVIDIA |
| 3 | Picture worker decode/preparation, pixel/decode/cache bounds, color/alpha, missing/relinked/replaced assets and shared consumers | Accepted — batch 48: bounded worker/codec/lifetime software evidence and full native pixel/lifecycle matrix on Intel and NVIDIA |
| 4 | Independent multi-channel Shared Texture selection/discovery, absence/restart/rename/resize recovery, one failed channel remaining isolated and explicit adapter mismatch | Accepted — batch 49: fixed app/native pixel/lifecycle matrix on Intel and NVIDIA, including both cross-adapter rejection directions and healthy peer continuity |
| 5 | App-owned Media Manager UI for inspection and channel/source/size/rename/reorder/delete operations, using backend intents and generated contracts | Accepted again after redesign — [complete new-layout source matrix](evidence/mapgyver-m04-workbench-source-acceptance-2026-11-01.json), plus tree/folder/Inspector workbench and backend folder history/roundtrip. Original batch50 scope is preserved. |
| 6 | UI/script/headless intent parity, stable references through project/history roundtrip and functional native/resource-recreation checks across the source types | Accepted — batches 51–52: 152 caller executions, sparse/full codecs, full ordinary HTTP/history/Save/New/Load, actual Reload Picture UI and native public-intent replacement/recreation on Intel/NVIDIA |

#### Media module commands and scripts — 2026-10-30

- Media Manager now offers a Media Operation command, Rust-serialized creation presets for Solid
  Color/Picture/Shared Texture and a hosted Media Script. A command's Request parameter accepts
  the canonical `MediaEdit`, covering all existing authoring/inspection operations; Execute and
  its script `execute()` method use the same route. No second domain model or Chataigne-private
  command/module dependency was introduced.
- Manager methods `editMedia(requestId, requestJson)` and `inspectMedia(requestId)` expose the same
  operations. App-owned snippets are embedded with `include_str!`, work in packaged builds and
  subscribe once for `onAppOperationResult`. Applied callbacks carry committed backend identities;
  domain rejections carry no partial mutation or history. Request JSON is bounded to 64 KiB before
  DTO decoding; Golden retains its existing request/result lifetime admission and eight-per-turn
  service bound. Manager/command nodes are passive and command inboxes read bound values without
  requiring a whole-tree snapshot. Catalog/inspection remains separate from command/script nodes.
- Golden exposes its existing deferred app request path to nodes via
  `ProcessCtx::enqueue_app_operation`, preserving callback completion fences, weak source ownership,
  atomic authoring and admitted result disposal. App source owners expire on destruction/disablement;
  disabled commands reject execution. Undo does not resurrect abandoned requests. A small custom
  script-method hook preserves ordinary node methods when adding app-specific methods.
- Functional tests caught and fixed two shared runtime defects: opaque method/mutation callbacks
  now synchronize bound parameter values before running (including immediately after project load),
  and identical trigger pulses emit events without an authored undo step or clearing redo. Command
  undo therefore reverses the structural operation once; replay does not fire the command again.
- **Verified:** **50/50 MapGyver tests pass**, including nine new command/method suites for
  all creation presets through public user-item/trigger intents, real hosted QuickJS success and
  rejection callbacks, read-only inspection, cancellation/deletion/disablement, failed originating
  batches, default-method fallback, embedded factory snippets and command project roundtrip with
  no load-time execution. The production actor services an ordinary UI trigger without manual
  request pumping, creates exactly one history step and undoes the structural operation once.
  That test initially compared history before startup cleared it; its corrected baseline is
  captured after runtime startup. The final code passes **656 shared engine tests**, with three
  existing ignored tests, including a new trigger-event/history/redo regression. Strict Clippy
  passes for app/engine **all features, bins and tests**; the bundled desktop build passes.
  All-feature compilation also exposed two native test cleanup callers missed by the earlier
  mutable `close` API change; both are corrected. Native test code was type-checked by Clippy,
  not executed, and no native Picture or M03 experiment was started in this batch. The
  `--no-default-features --bins` check also passes, preserving engine/headless launch paths.
- **M04 gate 1 closes** from the verified module integration above. M04 advances **0/6 → 1/6
  (17%)**; no denominator or acceptance scope changes. M03 remains **7/9**, overall **3/10 (30%)**.
  Gates 2–6 retain the full source/native/recovery/UI parity requirements, including unavailable
  interactive form checks, sender discovery and the retained Picture display/admission limits.
- Implementation/check work completed at **20:33 UTC**, before the recorded **20:44 UTC** stop.
  Both workspaces are formatted. This is a reviewable product checkpoint for milestone delivery;
  the next batch starts with sender discovery, without restoring M03's parked investigation.

#### Shared Texture sender discovery — 2026-10-30

- Golden Video now exposes a worker-local Spout discovery owner and bounded exact-name catalog.
  Enumeration never opens a graphics device/receiver, selects the global active sender or exposes
  native handles. SDK calls are capped independently of the advertised count; names are validated
  as exact UTF-8, sorted and deduplicated. Invalid/omitted entries and count truncation are explicit.
- MapGyver's existing native metadata worker scans only on an explicit request. The host control
  callback reads a cache/sets one coalesced flag; it performs no SDK enumeration, engine polling or
  renderer construction. Failed scans keep the prior names and allow a fresh SDK owner on retry.
  The app caps output at 64 names and 48 KiB of encoded names, preserving the host's existing
  **64 KiB** control reply bound even for heavily escaped names. No resource budget was raised.
- Added Rust-owned native request/snapshot DTOs and generated TypeScript. App requests coexist
  with the unchanged Golden preview protocol. Revisioned metadata events reject late scanning
  replies after completion. Catalog state is ephemeral and absent from project/history data.
- Shared Texture now offers discovered-name selection, refresh and exact-name entry. Scans never
  choose a sender, infer source format/defaults or replace a missing authored name. Selection is
  a draft until the existing Apply intent commits it. Browser/headless exact-name authoring remains
  available; the live catalog currently uses the desktop-native capability. Interactive form
  behavior and complete cross-host/source parity remain unverified/open under gates 5/6.
- **Native functional experiment 1 passed:** the existing independent fixture's unique owned
  sender was absent before start, present after accepted frames, absent after stop, present after
  restart and absent after its verified **exit 0** teardown. The discovery caller opened no
  renderer/receiver. This proves live registry freshness/exact-name enumeration, not timing,
  resource, pixel delivery or full per-channel recovery qualification. No second native experiment,
  native Picture run, M03 profiling or diagnostic archive was added.
- **Verification:** **55/55 app tests**, **2/2 Golden native discovery tests**, **2/2 Node identity/
  stale-reply tests** pass. Tests cover native call/count bounds, invalid names/churn, cache
  coalescing/recovery/revision exhaustion, encoded reply bounds, late-response ordering, all
  existing preview request variants and generated-contract drift. Svelte check: **0 errors /
  0 warnings**; Prettier, strict app/video Clippy with **all features/bins/tests**, bundled desktop
  build and `--no-default-features --bins` check pass. Both workspaces formatted.
- No further M04 gate closes: **1/6 (17%)**, M03 **7/9**, overall **3/10 (30%)**. Full multi-channel
  rendering/recovery, explicit adapter mismatch, interactive selection and source/native parity
  retain their complete acceptance scope. Next product deliverable: native Picture integration
  within the existing source-memory contract, without reopening GPU-clock qualification.
- Implementation/check work finished at **20:59 UTC**, before the recorded **21:19 UTC** stop.
  The native check reused `native_source_fixture` and the ignored
  `channel_rendering::discovery_tracks_owned_sender_start_stop_and_restart_without_creating_a_receiver`
  test; its source process and test exited successfully. The milestone is delivered before the
  next implementation batch.

**First product batch:** started 2026-10-30 18:07 UTC, feature verification completed at 18:32 UTC,
within the recorded **18:52 UTC** stop (45 minutes). Owners: `apps/mapgyver/src/media` and
`apps/mapgyver/ui`. The deliverable was a usable Media Manager/Solid Color authoring panel with
backend defaults, ordered channel inspection and typed color/size editing. The M03 stash stayed parked.

#### First authoring slice — 2026-10-30

- Delivered a registered, primary Media Manager panel: create Solid Color media; add/select,
  rename, reorder and remove channels; rename/remove media; edit straight linear RGBA,
  automatic/fixed extent and sampling. Backend operations retain ownership of IDs, defaults,
  label allocation, validation and history. The UI collects explicit user intents only.
- Added Rust-owned `InspectManager` and generated ordered media/channel DTOs. Catalog inspection
  walks the manager/media/channel lists once, excludes assets and does not mutate history.
  History/snapshot/reconnection signals refresh the panel without engine polling.
- Fixed the reproduced default-app startup failure (`Media Manager is missing`) at the lifecycle
  boundary. Golden's shared transport startup now settles detached initialization, runs the
  app candidate hook and its queued edits/validation, then activates resources. MapGyver's
  structural validation belongs in that candidate hook, rather than the earlier post-load hook.
  This ordering matches project replacement and applies to desktop/headless startup.
- Verification: **32/32 MapGyver library tests**, including read-only catalog, ordering/history,
  project roundtrip and generated-contract drift; **88 shared transport tests passed**, one
  existing ignored; both relevant Clippy runs with warnings denied; Svelte check **0 errors /
  0 warnings**, production UI build and bundled desktop build. Both workspaces formatted.
- Live bundled app opened on backend 7011. Native inspection worked; native input was limited by
  the helper/app integrity mismatch. Edge exercised the actual bundled UI against that app's
  real backend: create Solid Color, add a second channel, author `[0.2, 0.4, 0.6, 0.5]` at
  **640×360 / Nearest**, leave the first channel white/automatic/Linear, Undo/Redo, rename,
  reorder and reload/reconnect. Final UI build committed the same settings. Narrow-pane check
  verified `scrollWidth == clientWidth == 224` after removing implicit grid minimum overflow.
  A local review screenshot is `target/mapgyver-m04-authoring.jpg` (not a qualification archive).
- Scope still open: native app preview/consumer integration, Picture/Shared Texture editors and
  remaining source acceptance, module command nodes/methods/callbacks/app-owned snippets, and
  complete cross-source UI/script/headless/native verification. Existing backend/script tests
  are useful evidence, not full module integration. No M04 gate closes from this slice alone.

Architecture/entrypoints: [app ownership and authoring guide](../../apps/mapgyver/README.md).

**Product batch 2:** started 2026-10-30 18:35 UTC; implementation and available checks completed
at 18:55 UTC, before the recorded **19:20 UTC** stop (45 minutes). Owners:
`apps/mapgyver/src/media` and `apps/mapgyver/ui`. The deliverable was Picture creation/import,
asset selection/relink and independent size/sampling through existing asset/worker contracts.
Planned proof was authoring/history/project and generated-contract tests, Svelte check/build,
and a live UI exercise. Tests/builds passed; app observation succeeded, but **interactive UI
verification remains unavailable**, as detailed below. No M03 stash restoration or native
qualification was performed. Full M04 acceptance stays open.

#### Picture authoring — 2026-10-30

**Product batch 3 checkpoint:** started 2026-10-30 18:57 UTC; stop by **19:42 UTC** (45 minutes).
Owners: MapGyver's app/media preview integration and the existing public Golden host/video
preview seams. Deliver a native preview of the selected authored channel in the ordinary app,
including explicit unavailable-source state. Reuse the production renderer; no CPU frame/HTML
image transport or app-private host copies. Proof: routing/lifetime tests, builds and a bounded
native functional observation where automation permits. Timing/resource qualification stays
with M03; its stash remains parked. Preserve all remaining M04 source/module acceptance.

#### Ordinary app native preview — 2026-10-30

**Product batch 4:** started 2026-10-30 19:43 UTC; stop by **20:28 UTC**.
Deliverable: working native Picture preview/switching with normal owned shutdown after the
retained fixes, then functionally ready Shared Texture source editing. Owners: app media preview,
rendering and editor components. The correctness investigation stops by **20:03 UTC**, at most
two experiments including the baseline. Hypothesis: old-request rejection and retained picture
leases caused the prior covered preview/stuck exit; the current fixes should produce a visible
PNG, update on relink/missing-source changes and exit zero through the shutdown sentinel.
If contradicted, inspect the failing boundary once, apply a focused correction and rerun once;
then preserve the evidence and continue independent source authoring instead of adding probes.
Proof: existing relevant tests/checks and bounded public-API/native observation. No M03 profiling.

#### Shared Texture source editor and bounded Picture correction — 2026-10-30

- Added Shared Texture creation and per-channel exact sender selection/disconnection, accepted
  source maximum dimensions, storage/color/alpha and independent automatic/fixed output size and
  sampling. Every field starts from the backend's generated definition; existing `ConfigureChannel`
  owns validation, history and persistence. No implicit active-sender fallback or UI-owned defaults.
  Discovery and live multi-sender recovery remain open; this is the source-editing slice.
- The two Picture experiments are consumed. The baseline remained unavailable but exited **0**
  through its owned shutdown sentinel, verifying the earlier lease-retirement fix. The second run
  exposed the actual error: **render plan input metadata exceeds its admission bound**. The second
  app/listener are stopped; its final exit code was not retained. No third native Picture run was
  started in this batch, and native switching/relink/missing-source display remains unverified.
- Fixed the status lookup: snapshot and native-event references serialize object properties in
  different orders; stable channel identity now uses the explicit media/channel tuple. Added one
  passing Node regression. Errors also log once when the worker's published status changes.
- One source inspection found preparation retaining the entire **128 MiB** decode scratch allowance
  for a tiny result, exceeding the native app's **1 MiB** plan bound. Golden Assets now shrinks the
  reservation to the decoder's retained result plus the existing envelope. Worst-case working
  reservations, source/decode/cache/admission bounds and M03 budgets are unchanged. A regression
  proves two tiny retained results coexist with room for only one worst-case decoder; the byte-
  pressure test uses a real retained allocation and still proves rejection before decoder entry.
- Verification so far: **17 Golden Assets tests**, **41 MapGyver tests**, **107 Golden Video tests**
  and **1 Node identity test** pass. Picture renderer tests now use the native app's 128 MiB decode
  allowance with its unchanged 1 MiB plan bound. Svelte check: **0 errors / 0 warnings**; strict
  Clippy for app/assets bins/tests passes. Native Picture display after the accounting correction
  stays open; larger pictures still need explicit integration within the complete source budgets.
  The bundled desktop build also passes. Both workspaces formatted.
- Live bundled-app check used the authenticated public app-operation API to create two Shared
  Texture channels. Configuring A with an exact missing sender, **640×360** source maximum,
  opaque alpha and **480×270 / Nearest** output left B unconfigured/automatic with its original
  **1920×1080** source maximum. Structural inspection assertions passed. An initial ad-hoc
  assertion compared JSON property order and was corrected to structural equality; this was a
  checker error, not a failed authoring transaction. The visible app showed both channels, the
  authored size and **Source Unavailable**, and confirmed native layout installation. Native
  scrolling remained ineffective under the known integrity mismatch, so source-form interaction
  itself remains unverified. The owned app exited **0** through its sentinel.
- Feature/check work finished at **19:58 UTC**, before the 20:28 stop. No acceptance gate closes;
  source editing is delivered, while discovery, full recovery, module integration and cross-source
  acceptance remain open. Next product batch: module commands/script methods/callbacks/snippets.

- Integrated the public Golden preview session/component and production ChannelRenderer into
  the ordinary Windows app. Native video is the Windows default; headless and engine-only builds
  start no presentation. GPU construction is deferred until a visible region is demanded.
- Added a typed host presentation factory receiving the existing authoritative runtime, and
  bounded read-only `capture_app` for owned app metadata. No duplicate engine, private host imports,
  live engine access from UI, CPU frame transport or qualification-host copy was introduced.
  Existing public-host/native smoke call sites use the new factory contract.
- The app compositor shares channel textures across views, preserves independent native output
  routes, applies Golden's clipping/visibility guard and uses an opaque dark window backing.
  Generic dock mounts now forward Svelte context; explicit background hooks reveal only installed
  native surfaces. Status/availability uses Rust-generated DTOs and metadata events.
- Picture preparation follows the current saved project directory, including save-as rebasing.
  Authored media operations now emit one retained atomic notification for other UI/script/headless
  callers. Inspection remains read-only. This fixed a reproduced stale catalog after HTTP edits;
  complete cross-client history parity remains part of M04 gate 6.
- **Native functional observation:** the actual bundled `MapGyver.exe` was driven through its
  authenticated public loopback app-operation API. Creating/configuring a red **640×360** Solid
  Color updated the ordinary UI and produced an observed red native region. The worker confirmed
  matching installation, frame identity and accepted output token. This is not scanout/color/DPI
  or latency qualification. Native input remains limited; no Edge permission bypass was attempted.
- The Picture follow-up imported the owned alpha PNG and switched away from Solid Color, but
  remained covered as unavailable. The worker incorrectly treated superseded older outcomes as
  current failures; handling now checks the latest request. Shutdown then waited with Picture
  leases still held by the renderer. `close` now releases preparation/desired state before waiting,
  and the native owner captures retirement handles before closing. Added a regression checking
  retirement while the renderer object remains alive. The old owned test process **16492** was
  terminated after confirming the stuck shutdown; this is retained failure evidence, not success.
  **At the batch-3 cutoff, native Picture switch/relink/missing-source and post-fix native shutdown
  were unverified.** Batch 4 above records the subsequent baseline shutdown pass and display failure.
- Available verification before final lifetime fix: **40 MapGyver tests passed**, including
  compositor sharing/fences, authoritative capture, picture-root relocation and generated DTOs;
  **88 transport tests passed / one existing ignored**, **6 Golden preview tests passed**, public
  default-host test passed, all-feature app/smoke binaries checked, Svelte **0 errors / 0 warnings**,
  bundled build and Clippy passed. Final lifetime regression and strict checks are rerun below;
  do not infer their result from the earlier pass. The batch stops here for verification/delivery.
- Final lifetime-fix checks completed at **19:41 UTC**: **41/41 app tests passed**, including
  retirement without dropping the renderer; strict Clippy passed. Both workspaces formatted.
  No further native experiment was started before the 19:42 stop. Post-fix native Picture and
  shutdown verification are the next bounded action; the earlier successful red preview stands.
- No M03/M04 acceptance gate closes. M03 remains **7/9** and M04 **0/6**; source/native/module,
  recovery and qualification requirements retain their original scope and budgets.

- Added one typed `ImportPicture` app intent. A new Picture imports an asset and detached
  media/channel tree in one transaction. Importing into an existing Picture channel preserves
  its output settings and leaves other channels unchanged. Rust owns IDs, default source/size/
  sampling, labels and collision handling, including reserved labels inside the detached batch.
- Manager inspection now returns a separate, ordered asset catalog with labels and optional
  authored locators. Shared asset selection/relink uses stable catalog identity. Unconfigured
  or missing references are represented explicitly; the UI never picks a replacement implicitly.
- The Media Manager adds blank Picture creation, atomic import/create, import/assign, catalog
  selection, relink and common automatic/fixed extent/sampling controls. Native source browsing
  uses Golden's public file picker; browser users can supply explicit host locators. Picture and
  Solid Color share one focused output-settings form. The UI performs no decode or frame transport.
- **36/36 MapGyver library tests passed**, including four new import tests: complete one-step
  Undo/Redo, stable IDs/project roundtrip, label reservation, per-channel output preservation,
  shared-asset relink, atomic invalid-target/path rejection, and real QuickJS committed-result/
  history parity. Existing bounded worker decode/color/alpha/replacement/shared-consumer tests
  and generated TypeScript drift checks remain green. Clippy with warnings denied passed;
  Svelte check reports **0 errors / 0 warnings**; production UI and desktop builds pass.
- Live development app reached backend 7011/frontend 5174 and displayed the new controls.
  Native accessibility reported a helper/app integrity mismatch; a coordinate click produced no
  observable action. The browser connector inventory was empty. One bounded recovery attempt
  to launch the installed Edge was rejected: **“Computer Use was not approved to use msedge.”**
  No alternate launch bypass or permission loop was attempted. Picture form interaction, native
  Browse and live source/history UI behavior therefore remain **unverified**, despite backend/
  script tests. The owned app/frontend were stopped and both listeners released.
- Native preview/consumer integration, Shared Texture source editing, module command nodes/
  methods/callbacks/snippets and complete M04 cross-source acceptance remain open. **No acceptance
  gate closes** from source/build evidence or the unavailable interactive check.

### Anti-drift correction — 2026-10-30

The user identified the prolonged M03 investigation as a failure to advance the product.
The agent had treated two broad qualification gates as an unlimited prerequisite for M04,
allowing each diagnostic to create another measurement task. Useful fixes did not justify
that execution pattern. The plan now separates functional dependencies from final acceptance;
all original gates remain required. The dashboard denominator and completed counts are unchanged.

Apply [AGENTS.md's limits](../../AGENTS.md#autonomous-progress-and-anti-drift): 45-minute feature
batches; investigations stop at 20 minutes or two experiments; at least four product-delivery
batches per diagnostic-only batch; no consecutive diagnostic-only turns by default. After
two batches without a usable capability or acceptance closure, reduce the feature slice or
choose another functionally ready product task. Do not add another measurement prerequisite.
The 2026-10-31 reinforcement requires checking the recorded cutoff before expanding scope;
continuations do not reset investigation limits. The dashboard names a delivered capability
alongside phase gates. Batch 16 retains its original 01:25 UTC cutoff.

**Product-delivery count since correction: 39. Diagnostic-only batch allowance: 9, two used.**
The first authoring slice and the new atomic Picture import capability (verified through backend/
QuickJS) count as product delivery; the earlier workflow/docs checkpoint does not. Picture UI
interaction is not counted as verified acceptance. The observed ordinary-app Solid Color preview
is the third usable capability. Shared Texture source editing is the fourth; its backend/native
metadata observation does not claim interactive form or recovery acceptance. The failed Picture
display checks are not gate closure. The available diagnostic batch is not the default next task;
module commands/scripts are the fifth delivered capability and close M04 gate 1. Desktop sender
discovery/selection is the sixth; its native registry check does not close full source acceptance.
Native 4K PNG display with relink/missing/recovery is the seventh; full Picture acceptance stays open.
Screen/Surface backend authoring with commands/scripts is the eighth and closes M05 gate 1.
The registered Screen Manager authoring UI is the ninth; full editor/input acceptance stays open.
Checked GPU crop/warp plus shared Screen-plan publication is the tenth and closes M05 gate 2.
Native Screen preview with observed Surface withdraw/restore is the eleventh; full editor gates stay open.
Source-independent calibration/restoration and ordered procedural layers are the twelfth and close
M05 gate 3.
Editor-independent virtual Spout target activation with independent SDK delivery is the thirteenth;
the full physical/virtual target and editor/minimize gate stays open.
Host-owned physical rectangle targets with signed geometry, move/resize and concurrent Spout
delivery are the fourteenth; full physical pixels/fullscreen/minimize qualification remains open.
The generated display catalog/selector and intended-identity fullscreen activation/recovery are
the fifteenth; actual hotplug/minimize and the full presentation matrix remain open.
Screen Surface selection/handles and backend corner gestures with bounded, grouped history are
the sixteenth; actual editor input remains unverified.
Full-source crop editing and independent docked editors with shared selection are the seventeenth;
native source preview and actual pointer/dock behavior remain unverified.
Composition authoring/module integration is the eighteenth and closes M06 gate 1.
Shared nested Composition production is the nineteenth, with software geometry/dependency checks
and native submission/lifetime smoke; native transform/blend pixels and the 2D editor remain open.
Dockable Composition source/instance/canvas controls and native preview binding are the twentieth;
graphical transform gestures and real interactive/native preview evidence remain open.
Graphical Composition selection/geometry and grouped history are the twenty-first; software/native
submission foundations permit M07, with real input and complete native pixel qualification open.
Sparse Matrix authoring/module integration is the twenty-second and closes M07 gate 1; runtime
launches/transitions, the virtualized grid and native qualification remain open.
Atomic Matrix cuts, shared derived production and explicit launch controls are the twenty-third;
full timed transitions, virtualized grid, native pixels and interaction qualification remain open.
Timed Matrix fades/interruption and Launch controls are the twenty-fourth. Retained binding edits
remain an explicit gate 2 repair, with grid and full qualification still open.
Retained clip bindings across authored edits and installed native cleanup are the twenty-fifth,
closing M07 gate 2; the virtualized grid and full qualification remain open.
The bounded Matrix grid, full settings/reorder forms and correlated renderer observations are the
twenty-sixth through twenty-eighth capabilities. Batch 29 used one diagnostic allocation and left
actual input acceptance open. Public Golden IO process ownership is the twenty-ninth; shared app
definitions and the pure column-demand model are the thirtieth. The independent application lifecycle
controller and generated runtime observation are the thirty-first. Rendered Matrix application
demand and final-plan process retention are the thirty-second. No-consumer application demand
and explicit Matrix unload are the thirty-third. Real native Spout startup/image readiness without
a renderer is the thirty-fourth. Shared-application definition forms and Matrix app/distance
controls are the thirty-fifth. Live process/configuration observations are the thirty-sixth.
Ordered, receipt-gated lifecycle callbacks are the thirty-seventh.
Default headless application supervision/callback attachment is the thirty-eighth.
Operator-visible supervision/admission health and recovery is the thirty-ninth.
The allowance is nine at
`floor(39 / 4)`, following the four-product-to-one-diagnostic ratio; two were used. The bounded
M08 lifecycle acceptance batch is complete; consecutive diagnostic-only batches remain prohibited by default.
Qualification may interrupt only for a reproduced blocker on the selected product path or an
explicit user request; otherwise it resumes in its scheduled lane after product delivery.

Unfinished agent-owned clock/collector work is parked locally in stash
`67adbae342a0ec851b581974e616dedfc16d2179`
(`mapgyver: parked M03 clock and collector investigation after anti-drift correction 2026-10-30`).
It contains the SDK clock probe and an unverified native collector-policy change. The probe
built and returned data; it does not establish calibrated production timing. Raw local results
remain under `target/qualification-clock-calibration-cmd-fixed-2026-10-30`. Do not automatically
restore or continue this stash on goal continuation. Published renderer behavior remains at
`6350f5ac`; no unfinished diagnostic change is part of the product baseline.

### Delivery checkpoint — 2026-10-28

The repository-wide [delivery rule](../../AGENTS.md#implementation-plan-delivery), committed as
`21d5bae`, now requires commits and pushes for every meaningful implementation milestone in
every plan. The current `mapgyver` checkpoint preserves the existing M00 baseline, architecture,
M01 harness and historical evidence. M01 remains at **1/8** acceptance items; this source
checkpoint does not qualify the remaining native gates or advance the dashboard.

Checkpoint validation:

- `cargo test --locked -p mapgyver_native_probe --lib`: 38 passed.
- `cargo test --locked -p mapgyver_native_probe --features desktop`: 52 passed; six hardware
  tests ignored. No new native timing, scanout or hardware qualification is claimed.
- `python -m unittest discover -s tools/mapgyver/tests`: 36 passed. Two color-accuracy tests
  now mock platform inventory so their native-process mock cannot intercept Windows platform
  detection; the first run exposed that fixture error and the corrected suite passes.
- Probe UI `check`, `check:generated` and `build`: passed using `npm.cmd`; Svelte reports zero
  errors/warnings and Rust/TypeScript DTOs match. All 100 MapGyver evidence JSON files parse.
- Required `cargo fmt --all` at the root and Golden Core manifests completed.

The follow-up coherent checkpoint adds NVIDIA fixed-card accuracy, the first working official
PresentMon capture and strict CSV analysis, plus lossless large-integer evidence transport.
Its independent review recomputes all 1,192 trace rows exactly and verifies the four NVIDIA
reports against their original stdout. All 44 Python tests pass again; required root/Core
formatters and whitespace checks pass. Native sources remain unchanged from their existing
52-test checkpoint, so no new native build or hardware result is inferred. This checkpoint
also leaves M01 at 1/8 and overall progress at 1/10.

Publication through the live-counter/warmed checkpoint is verified: the remote query returns
`0e4a6deba1bbcffddb555e65750090bc05a42a44` for `refs/heads/mapgyver` at
`git@github.com:Golden-Geek/Chataigne2.git`. This external publication clears the earlier
automatic-review rejection. That checkpoint was pushed and upstream/remote equality verified
before this batch. The output-only policy control, sampled admission and complete retained
three-run evidence follow that baseline and must be delivered before the next batch.

Milestone `8138e397` (`Test MapGyver output presentation with sampled backpressure`) and delivery
note `b07c9753` are now published. The user explicitly approved these commits, including the
reviewed code/tests/docs, three diagnostic packets (10,542,779 bytes total), plan and analysis,
to `git@github.com:Golden-Geek/Chataigne2.git`, branch `mapgyver`. The push succeeded and an
independent remote query returned `b07c9753f923d8492945d9e8e7aa8a654f6426ea`, matching local HEAD
with zero ahead/behind commits and a clean checkout. This resolves the earlier automatic-review
rejection and publication blocker; the M01 acceptance blockers above remain unchanged.

Milestone `4fd531d2` bounds renderer imports by weak allocation identity and splits handoff
timings. It is published on `origin/mapgyver`; an independent remote query returned
`4fd531d23de4f11c2e539cbd933051bc9f588602`, matching local HEAD with zero ahead/behind commits
and a clean checkout before this delivery note. [Its native evidence](mapgyver-m01-native-feasibility.md#bounded-renderer-imports-and-split-handoff-timing--2026-10-28)
and [strict analysis](evidence/mapgyver-m01-import-cache-analysis-2026-10-28.json) retain two new
Intel captures and 4K cross-process accuracy. Sampling performs no resource opens; each cache
peaks at three aliases and drains to zero. The sustained run still accepts 6,548/7,200 output
attempts with 44 CSV drops, so it closes no additional gate. The longer explicit collector tail
allows both live queries to succeed without changing the older unavailable observation.

### M01 acceptance items

| Item | State | Accepted scope / remaining work |
| --- | --- | --- |
| Ordinary underlay/DOM/native-input functional feasibility | Done | Native observations at 100%/125% establish the scoped route; full DPI/input/latency qualification stays with M05. |
| D3D11/Spout two-source GPU composition and caller-device ownership | Done | Independent processes, finite pixel-accuracy checks and GPU-only frame transport establish functional feasibility. Renderer budgets stay with M03. |
| Scoped editor/output independence, resize/restart and ownership cleanup | Done | Recorded interruption/reconnect/rebuild smoke establishes the functional mechanism. Sustained retirement/recovery budgets stay with M03 and owning features/M09. |
| Host/backend/binding decision with explicit later-gate ownership | Done | Ordinary Tauri/Wry underlay, D3D11 and pinned `spout2-sys` 0.1.1 are selected for the thin shell; full qualification is explicitly not passed. |

Evidence and limitations: [functional route decision](mapgyver-m01-native-feasibility.md#functional-route-decision--2026-10-28)
and [fixed qualification contract](mapgyver-qualification-contract.md). This is the user-approved
functional gate, not a retrospective pass of the original eight-item matrix.

### M02 acceptance items

| Item | State | Accepted scope / remaining work |
| --- | --- | --- |
| Public UI-bundling boundary | Done | [Public helper](../../crates/golden_core/support/codegen/src/ui_bundle/mod.rs) adopted by Chataigne; 19 support tests, strict Clippy and a real app build check with UI bundling enabled pass. SDK/formula/node/Tauri steps remain app-owned. |
| MapGyver host/UI scaffold | Done | [Thin shell](mapgyver-m02-shell.md#implementation-boundaries) uses Golden's public host/workbench, app identity, project format/data directory, resources and capabilities. Fresh projects have the standard Dashboard; no placeholder Media/Screen managers. |
| App branding and browser-state isolation | Done | [Backend metadata and browser namespaces](mapgyver-m02-shell.md#implementation-boundaries) separate layout, selection, recent path and credentials; app projects carry a validated identity. Stream Deck's editor belongs to Chataigne. |
| Generic discovery and endpoint isolation | Done | [Rust-owned discovery DTOs](mapgyver-m02-shell.md#implementation-boundaries), generated protocol 0.13.0 and distinct backend/frontend ports; app identity is checked before UI admission and on fresh credentials. |
| Explicit app selection in tools | Done | [Cargo/npm/xtask/watch/package commands](mapgyver-m02-shell.md#reproduction) select MapGyver explicitly; root defaults remain Chataigne. |
| Typed optional native-presentation host lifecycle | Done | [Public extension and native smoke](mapgyver-m02-shell.md#optional-native-presentation-lifecycle) keep window construction/UI preparation at Golden's host, task resources on its worker, HWND destruction after retirement, and joining after the event loop. Default/headless paths do not prepare presentation. |
| Combined launch and boundary verification | Done | [Concurrent bundled/dev/headless runs](mapgyver-m02-shell.md#live-windows-verification--2026-10-28), own save/reopen and foreign-file rejection pass; engine/protocol packages and both full desktop apps compile and test through public boundaries. Native quality remains unqualified. |

### M03 acceptance items

**Native allocation ownership batch 71:** started 2026-11-01 **01:40 UTC**, stop by **02:25 UTC**.
Previous turn is progress: `1d5716c7` delivers the complete GPU CPU enclosure and retains the
unqualified original GPU budget. Its scope proposal is pending and not operative. Only M03
gate 9 is active. Deliver the missing native allocation/alias ownership observation at Golden's
source/texture/output boundary, with app-owned capture consuming public metadata. Define what
the API can actually establish: descriptor texel charges and owned allocations/aliases are not
driver physical residency or allocator overhead. Avoid duplicate charges for shared aliases;
retain unknown SDK/driver counters explicitly. Hypothesis: matching actual object/lease ownership
to reservation checkpoints can establish whether app-owned resources return, without guessing
the cause of the failed +32 MiB process-private peak. Source/API investigation ends by
**02:00 UTC**, at most two fixed analyses/experiments. Prefer existing resource owners and
metadata over a tracing/profiling framework. Gate native verification on terminal software/
Clippy/build success; at most one finite mechanism check. No 100-cycle memory rerun unless new
ownership evidence identifies a concrete correction; no reset, timing probe or budget revision.
Checkpoint/push the coherent result and exact remaining coverage before the next batch.

**Batch 71 result:** Golden Video exposes `allocations_snapshot` for actual committed native
owners. A plan texture's metadata guard publishes release after its native texture/views drop;
Rc reuse and weak cache entries add no charge. Source pool/descriptor, imported view owners
and completion-retained reader counts are separate, coalesced metadata. Imported aliases add no
owned storage. Existing resize/retirement handshakes remain intact; source bridge zero-count
publication now follows actual object/handle release. Windows report the checked two-buffer
backing; an accepted Spout send establishes its one shared image/layout. Partial SDK owners
remain unknown. Outputs keep their allocation observation through actual endpoint retirement.
No recurring resource opens, GPU query, pixel readback, global allocator or driver change is added.
The total is validated texel payload, not driver allocation size/residency or a transient peak.

Rust owns capture schema **11**, generated in the same change. Ordinary renderer snapshots and
final native state consume the public inventory; lifecycle checkpoints/baseline/final/retirement
also record it for future required acceptance. Those lifecycle additions have software checks
only; no new 100-cycle run is claimed. **168 Video tests /23 existing hardware ignores**, strict
Video all-target/all-feature and MapGyver native-bin/test Clippy, **304 tooling tests**, generated
drift and both workspace formatters pass. Tests cover shared/weak ownership, peaks versus current
storage, partial SDK state, uncharged aliases, invalid/duplicate inventories and final retention.

One finite Q01 release check uses **MSVC 14.51.36231 /SDK 10.0.26100.0**, executable SHA-256
`2e23eb210ad27da532fa18743b8df7df5098016dd935e2e979baad9c25a5624c`, NVIDIA adapter 1 on the
retained Intel-owned display route. **2 s warmup /10 s sample /1 s each interruption** complete
normally in **17.042 s**. All **96** sample inventories are known and constant: **5** owned plan
textures, **6** source bridges and native output storage total **100,454,400 bytes /95.801 MiB**.
Up to **4** renderer import/view owners are uncharged. Final inventory has all five plan objects
released, no bridge/import/read leases, no output owners and zero known texel payload. Driver
physical bytes remain unknown. This establishes this short owner-release mechanism, not a
repair/cause of the preserved **36.102 MiB >32 MiB** growth failure.

Main output accepts **340/600** with **260 Busy** outcomes; Spout/editor each accept **600/600**.
Preserve that adverse cadence without assigning a cause or running another timing control.
All **345** inputs and paired overlay-Off identities/QPC brackets match; all three children exit
zero/unforced, the job is empty and all four PIDs are absent. Actual retirement **133.398 ms**
retains both HWNDs; whole-parent shutdown **446.332 ms**. All **22** local raw hashes plus paired
analysis/build/environment hashes verify. [Owner evidence](evidence/mapgyver-m03-native-allocations-2026-11-01.json).

**Decision:** the short criterion stays **UNAVAILABLE**, qualification false; no gate closes,
M03 **7/9**, overall **4/10**. Stop expanding renderer instrumentation. The GPU/window decisions,
original resource/growth/recovery budgets and final prescribed evidence remain required with
the dashboard return triggers. After this checkpoint is pushed, take one bounded **M05 gate 4**
acceptance batch under the plan's anti-drift rule, beginning with supported activation of a fresh
ordinary app window. Preserve the earlier activation failure until its exact missing flow is
verified. This changes execution order, not acceptance scope or any budget.

**Full GPU-path observation batch 70:** started 2026-11-01 **01:19 UTC**, stop by **02:04 UTC**.
Previous turn is progress: `4510ebbd` delivers native publication-capacity metadata and the
finite schema-9 control, retaining sustained eligibility and every existing failure. Only
M03 gate 9 is active. Deliver the missing cross-context GPU-path observation, or establish
the exact supported-API/evidence prerequisite that prevents it before further native series.
Owner: Golden Video's opt-in worker-local timing and app-owned qualification analysis. The
current source/renderer dependency records are exact but their separate GPU counters are not
calibrated. Hypothesis: a supported clock correlation or a complete conservative time enclosure
can provide a truthful full-path bound against unchanged p95 12 ms /p99 16.667 ms. Do not replace
the path with summed marginal percentiles, active GPU duty, one PID's Present interval or the
renderer context alone. Source/API investigation ends by **01:39 UTC**, at most two fixed
analyses/experiments, including existing-data analysis. A negative result ends that investigation
and records a concrete return trigger; it cannot seed another probe framework or budget waiver.
Do not restore or continue the parked SDK-clock/collector stash. No global power/driver changes,
GPU reset, memory rerun or Q01/Q02 series. Any native mechanism check requires terminal software/
Clippy/build prerequisites and must fit this batch. Checkpoint/push before the next batch.

**Batch 70 result (investigation complete before 01:39 UTC):** existing schema-9 data has complete
source/renderer dependencies but no direct query-collection CPU stamp. Sparse snapshots can
enclose all **600** selected frames using the following snapshot's QPC after a matching real
completion certificate; this avoids treating a pre-read stamp as a completion instant. That
bound is **218.407 ms p95 /223.023 ms p99** and cannot prove the GPU target. Per-frame context
span charge is **3.211 /3.965 ms**, which excludes gaps and is not the critical path. The parked
October-30 SDK clock/collector work remains untouched; reading its retained manifest does not
validate or resume its unverified production-clock mapping. PresentMon GPU duty/one-PID frame
grouping and node-clock queries without an exact D3D11-context mapping are not substitutes.

Golden's opt-in `GpuTiming` now retains QPC immediately before its first query marker submission
and after the existing complete nonblocking query reads. The latter is an upper bound including
collection delay. No new GPU marker/query, flush, wait, allocation, resource open, global clock,
power policy or scheduling change is made. Clock/query failure remains explicit and cannot
affect rendering or retirement. Source/renderer CPU bounds and exact retained identities permit
a complete conservative enclosure of both app source-worker scopes and renderer/output scope;
one QPC tick covers endpoint quantization. Independent GPU counter origins are never compared.
Disjoint GPU clocks cannot invalidate an independently certified CPU enclosure, but a missing
query collection cannot provide its end. Per-frame context sums remain descriptive only.
Rust owns capture schema **10** and its generated metadata; earlier captures gain no timing pass.

**Verification:** **164 Video tests /23 existing hardware ignores**, strict Video all-target/
all-feature and MapGyver native-bin/test Clippy, **299 qualification-tool tests**, generated
fixture drift and both workspace formatters pass. Seven focused enclosure tests cover exact
dependencies, retained age, collection tails, missing/foreign identity, disjoint clocks, invalid
timing, quantization/large counters and duplicate/untrusted coverage. An analyzer integration
test proves short-run ineligibility even when a synthetic complete bound meets the budget.

The one native Q01 mechanism check uses release **MSVC 14.51.36231 /SDK 10.0.26100.0**, executable
SHA-256 `f30f82e4aa798e1f69995c974fbeab7b179acd04d70d7a13c08d96b33a2ce836`, NVIDIA adapter 1
on the retained Intel-owned display route. **2 s warmup /10 s sample /1 s each interruption**
complete normally in **16.976 s**, exit **0**, timeout/force **false**, owned-job count **0**.
All **600** sample frames have both exact source and renderer query enclosures; all outputs
accept **600/600**, with complete publication-capacity metadata **2/1/2**. The new full bound is
**16.891 ms median /18.754 ms p95 /19.658 ms p99 /20.116 ms max**. It is too loose to prove
**12 /16.667 ms**, not an actual GPU-budget failure. The independent correlated context-span
serial charge is **1.905 /3.194 /3.230 /4.114 ms**; neither that value nor a short sample closes
the original GPU criterion. No performance repair is inferred from comparison with batch 69.

All **342** execution inputs are frozen. Paired environment/overlay-Off identity and QPC brackets
verify. All three fixture children exit zero/unforced; all four PIDs are absent. Actual native
retirement is **120.862 ms**, both HWNDs are retained until it completes, and whole-parent shutdown
is **420.408 ms**. Final runtime is stopped with zero reserved/in-flight/prepared/retiring work.
All **22** local raw-file hashes and paired-analysis/build/environment hashes verify; the compact
[evidence](evidence/mapgyver-m03-gpu-path-enclosure-2026-11-01.json) preserves exact native input
identity and distributions. No new trace session was requested.

**Decision:** end this timing investigation after one existing-data analysis and one finite native
check. Full GPU critical-path acceptance stays **UNAVAILABLE**, with original budgets. Returning
requires a validated supported clock-to-context mapping with bounded error, or explicit approval
of the precise GPU-work proposal below. While that decision is pending, take the missing
allocation/ownership observation next within gate 9. The private-growth failure, window/reset
decisions, prescribed series and M05–M09 acceptance remain required. **No gate closes; M03 7/9.**

#### GPU acceptance proposal — awaiting user decision

**Not operative.** Original acceptance remains a full elapsed GPU critical path at **p95 ≤12 ms /
p99 ≤16.667 ms**. This measurement gap has persisted since the October-28 native qualification;
the parked October-30 clock probe never established a validated production mapping. This batch
adds a complete nonblocking CPU enclosure but still cannot prove the original budget. Elapsed
age of the blocker is not claimed as 24 hours of measured coding effort; no new batch resets
the cumulative investigation history.

**Recommended precise revision:** replace that one acceptance metric with **per-frame correlated
GPU context-span serial charge, p95 ≤12 ms /p99 ≤16.667 ms**. For every selected renderer frame,
join both exact retained app source-worker GPU batches and its renderer/output GPU batch, sum
their individual elapsed spans after conversion by each span's own valid frequency, then compute
percentiles over those complete per-frame sums. Require exact device/source identities and no
missing, failed, disjoint or invalid query spans. Do not add marginal percentiles or treat missing
work as zero. Parallel source scopes are conservatively both charged. Keep the prescribed three
independent release **30+120-second Q01 and Q02** samples and all existing telemetry coverage.

**Tradeoff:** this proves a GPU-work budget, including waits inside each context span, but removes
the mandatory calibrated elapsed-time guarantee across contexts; gaps between scopes are not
charged. Keep the full CPU enclosure reported as a separate diagnostic and preserve the original
unqualified critical-path evidence. All end-to-end output deadlines, source-age/response,
publication-capacity, CPU, memory-growth, allocation, lifecycle/recovery and physical-output gates
stay unchanged. It is not an approval to release, a current native-series pass, or a memory waiver.
The finite example is **3.194/3.230 ms** correlated charge versus **18.754/19.658 ms** CPU bound;
its short duration qualifies neither the original nor proposed sustained criterion.

Approval must explicitly select this revision before the plan/contract/analyzer acceptance is
changed. Keeping the original requires a separately planned validated clock-correlation route;
it does not authorize restoring the parked probe/collector or silently continuing a profiler chain.

#### Earlier M03 verification batches

**Published-output queue batch 69:** started 2026-11-01 **01:04 UTC**, stop by **01:49 UTC**.
Previous turn is progress: `d8e382f5` publishes the complete embedded-shader lifecycle result,
retaining the failed 32 MiB maximum-growth criterion and ending that experiment chain. Only
M03 gate 9 remains active. Deliver public, worker-owned output publication/consumption bounds
and qualification coverage for the existing ≤2 published-but-not-consumed frames/output rule.
Ownership belongs in Golden Video's native output boundary; app-owned capture/analysis consumes
the public observation. Distinguish app in-flight batches, SDK latest-image storage and native
presentation capacity; an API accept, GPU fence or PresentMon pending count alone cannot certify
consumer occupancy. Preserve unknown states explicitly. Source/API investigation ends by
**01:24 UTC**, at most two fixed analyses/experiments. Verify with focused policy/coverage tests,
strict Clippy and, only after software prerequisites, at most one finite native mechanism check.
Do not start a Q01/Q02 series until all required instrumentation is stable. No memory rerun,
driver/reset action, window deadline revision or parallel phase work. Checkpoint/push the
coherent result and actual remaining coverage before the next implementation batch.

**Batch 69 result:** Golden Video's accepted output trace now publishes `OutputQueueBound`.
Windows read back the actual two-buffer flip-discard descriptor and latency-one setting during
cold creation. A trace retains the worker's readiness permit immediately before Present and the
same zero sync interval used by that call. Spout reuses the accepted shared-image/version
metadata from its pinned single-image send path: subsequent versions overwrite that image under
its ownership lock. No new shared-resource open, per-frame COM layout query, CPU pixel readback,
driver change or SDK-side allocation is introduced. The receiver-side potentially allocating
`GetSenderTexture` helper is deliberately not used to inspect a sender.

The analyzer joins every selected accepted output to its API and native incarnation/shared image,
requiring readiness, latency and stale-image replacement policy. The complete back-buffer count
is a conservative capacity bound, not an instantaneous DWM/GPU occupancy estimate; the single
Spout image has capacity one. Missing metadata remains unknown. Capacity above two is insufficient
proof rather than a measured occupancy failure. The qualification contract clarifies this
sufficient proof without changing the **≤2** budget or any GPU/physical/latency requirement.
Rust owns capture schema **9** and the generator updates its committed JSON; prior captures
cannot acquire a retrospective queue pass.

**Checks:** **164 Video tests /23 existing hardware ignores**, strict all-target/all-feature
Video Clippy and MapGyver native-bin/test Clippy pass. **291 qualification-tool tests** pass,
including eight focused capacity/identity/unknown/recreation cases and an analyzer integration
case that preserves short-run ineligibility. Rust-generated fixture drift and both workspace
formatters pass. Exact typed native observations remain metadata on their owning worker;
no UI-owned domain logic or duplicated TypeScript DTO is added.

The single finite NVIDIA native control uses **MSVC 14.51.36231 /SDK 10.0.26100.0**, release
executable SHA-256 `692fca29fbd6097461aeb0f424b62b9e55c2714b0f13062026d933c63c9f5b81`.
It completes **17.056 s** total, exit **0**, without timeout/force and with an empty owned job.
The **10-second** sample attempts 600 opportunities/output: main accepts **582**, records
**18 Busy**, Spout accepts **600**, editor accepts **600**. All **1,782** accepted publications
have matching verified queue metadata; bounds are **2/1/2** frames. Plan-age and telemetry
coverage are complete. The main API cadence/deadline observation remains adverse, and the
one-second interruption controls cannot satisfy the required 60 seconds. No PresentMon session
was requested, so window software-display/physical-consumption claims stay unavailable.

All **341** execution inputs are frozen; paired environment/overlay-Off identities and QPC
brackets verify the same NVIDIA renderer → Intel-owned display route. All three fixture children
exit zero/unforced, all four recorded PIDs are absent, actual retirement is **111.201 ms**,
and whole-parent shutdown is **418.801 ms**. Both HWNDs survive until actual GPU retirement;
final runtime is stopped with zero reserved/in-flight/prepared/retiring work. All **22** raw
file hashes and paired-analysis/build/environment hashes are preserved and checked in the
[compact evidence](evidence/mapgyver-m03-output-queue-2026-11-01.json).

**Decision:** queue observation now works end to end, but the short criterion and complete run
remain **UNAVAILABLE**, qualification **false**. No acceptance gate closes; M03 stays **7/9**.
Complete the missing full GPU-path observation next at its owning boundary. Physical allocation
evidence, preserved **36.102 MiB >32 MiB** growth, pending window/reset decisions and all later
phase acceptance remain required. No repeated native queue control, parameter sweep or memory
experiment follows this result. Freeze instrumentation before the prescribed final series.

**Native preparation memory batch 68:** started 2026-11-01 **00:26 UTC**, stop by **01:11 UTC**.
Previous turn is progress: `e08ede9b` verifies the complete corrected lifecycle workload and
isolates its failed maximum private checkpoint growth, **40.445 MiB >32 MiB**, while final
drained growth is 6.852 MiB and logical/handle/shutdown checks pass. Only gate 9 is active.
Use at most two fixed analyses of the existing 100-checkpoint record plus owning native
preparation/teardown source to distinguish sustained retention from repeat preparation cost.
Hypothesis: repeated fixed-HLSL compilation during each new renderer/device lifetime contributes
avoidable process-private preparation peaks. Native `Pipeline::new` currently compiles all four
unchanging entry points on every initialization. A bounded correction is justified only if
it removes proven repeated preparation at the right boundary and preserves shader behavior,
resource ownership, build portability and budgets; no driver/cache cause is presumed.
Investigation ends by **00:46 UTC** or the two analyses. Do not add an expanding profiler chain
or rerun the unchanged baseline. If supported, deliver the coherent preparation correction,
meaningful shader/contract checks and at most one native verification after software/strict
Clippy/build prerequisites pass. A full live acceptance check may finish on its original handle
within the 01:11 batch stop. Preserve previous failures and all thresholds. No global allocator/
timer/driver tuning, GPU reset, window-gate change or app-identity workaround. Checkpoint/push
before the next batch; no compiler optimization alone closes the renderer gate.

**Batch 68 checkpoint:** the existing dataset separates larger full-extent checkpoints from
smaller ones but does not establish a leak or driver-cache cause. Source inspection proves four
unchanging HLSL programs were compiled on every native device/pipeline initialization. Golden
Video now compiles those programs in its build script and embeds the resulting bytecode. Device
shader/state objects retain their worker/device ownership and retirement; no global cache or
runtime fallback is added. Entry points, profiles, source name and flags are unchanged. Native
D3D11 generation requires a Windows build host; portable plan/null builds skip generation.
Qualification's independent marker shaders retain their explicit compiler path. Build-script
inputs are included in qualification fingerprints.

**Verified:** **164 Golden Video tests pass / 23 existing hardware tests ignored**, including
independent byte-for-byte comparison of all four programs against their former runtime compiler
inputs. Strict all-target/all-feature Video Clippy and MapGyver native-bin/test Clippy pass;
the no-default-feature Video check, **282 qualification-tool tests** and both workspace
formatters pass. No native run is claimed by this checkpoint, and no
memory reduction is inferred. Batch 67's **40.445 MiB >32 MiB** peak failure remains operative.
**No acceptance gate closes; M03 remains 7/9.** Next is one unchanged, pinned full lifecycle
acceptance run, not another profiler or parameter sweep. All pending decisions and return-queue
items remain required; reaching this batch's time limit cannot waive them.

**Batch 68 native result (complete before the 01:11 UTC stop):** after terminal software/Clippy
checks, the pinned release build succeeds with **MSVC 14.51.36231 / SDK 10.0.26100.0** and
executable SHA-256 `8818da5b46c02c889558e3629452aeb2dd8864603f95ba02f0627c55f8e1457c`.
The one fixed NVIDIA-adapter-1 run completes **100/100** source restart/resize, project replacement
and native-output recreation cycles, plus **1,000** accepted edits in **100 × 10** bursts scheduled
at 100 Hz. **373** requests are superseded; every recorded healthy-source/peer-absence check
passes. No new native experiment, cache, allocator, driver, timing or threshold change is made.

The maximum post-baseline checkpoint is **271,753,216 private bytes**, versus the warmed cycle-10
baseline **233,897,984**: **37,855,232 bytes /36.1015625 MiB >32 MiB**. Final drained private
memory is **239,116,288 bytes**, growth **5,218,304 /4.9765625 MiB**; it cannot override the maximum.
Baseline/final handles are **939/935**, logical reserved texture bytes **160,358,672** and active
plan bytes **32,256,000** return exactly, with two source bindings and no prepared/retiring
generation. The native verdict is eligible **FAILED_NATIVE**, parent exit **1** solely for the
growth budget. The different peak from batch 67 does not establish an isolated causal benefit.

QPC verifies minimum post-work pause **2.0000044 s**, baseline/final drains **10.0031315 /
10.0015329 s**. Native owner retirement is **2,647.833 ms**; final checkpoint to observed parent
exit bounds whole-parent shutdown by **2,796.897 ms**. Total invocation duration is
**1,142.773 s**, with watchdog timeout **false**, forced cleanup **false** and owned-job count **0**.
All **103** fixture exit sidecars are zero/unforced and all **103** distinct recorded PIDs are
absent on final verification. A transient earlier PID-presence assertion is retained as an
observation requiring recheck, not evidence of a native child surviving its normal exit.

All **339** execution fingerprints are frozen across the run; paired overlay-Off environment
inventories, selected NVIDIA renderer → Intel-owned display route and QPC brackets verify.
The default WebView inventory is environment evidence, not executed full-app WebView acceptance.
All **419** local raw-file hashes are checked; compact evidence preserves the exact native
summary, including full-precision process timestamps, build/environment hashes and canonical raw
tree digest. [Complete result](evidence/mapgyver-m03-embedded-shaders-lifecycle-2026-11-01.json).

**Decision:** memory investigation/experiment chain ends here. Keep the shader preparation change
for its proven removal of repeated compilation and exact shader equivalence; do not claim it
fixes private growth. Complete the missing published output-queue observations next within gate
9. The memory failure returns when new ownership/allocation evidence supports a specific
correction, retaining the unchanged maximum-growth criterion. GPU critical path, physical
allocation identity, final Q01/Q02, pending window-deadline decision, genuine removal/stuck-drain
and later phase acceptance remain open. **No gate closes; M03 remains 7/9**, overall **4/10**.

**Full settled lifecycle acceptance batch 67:** started 2026-10-31 **23:48 UTC**, stop by
2026-11-01 **00:33 UTC**. Previous turn is progress: `fe8cb154` corrects the admitted-versus-
actual source descriptor, post-work pause and owned-fixture lifetime; six tests and strict
Clippy pass. Only gate 9 is active. Verify one fixed full 100-cycle renderer lifecycle run
with 1,000 burst edits, two-second post-work pauses, cycle-10 baseline, 10-second baseline/
final drains and unchanged +10 handles / +32 MiB growth limits. Use one pinned NVIDIA release
build, fresh paired overlay-Off/environment observations, 1,800-second source/consumer lifetimes
and an explicitly recorded 1,800-second outer watchdog. Do not edit source during capture.
Validate actual QPC pause/burst receipts, all counts, final native summary, exact logical return,
peak private growth, every child exit and final owned-job cleanup before accepting this scope.
No full renderer/physical-allocation/queue/GPU-path claim follows from this narrower result.
No new timing/driver/ownership investigation is scheduled. Broader investigation remains closed;
the existing fixed acceptance check may finish on its original live handle past the 20-minute
observation boundary, within this batch's stop. Do not restart for an observation timeout or
add a second native run. Preserve batch 66's prefix/private excess and batch 65's invalid-binding
failure. Checkpoint/push the actual result before the next batch; the unapproved window gate
proposal and genuine-removal decision remain pending without blocking independent measurements.

**Batch 67 result:** the exact pinned release build succeeds at **23:51:06 UTC**, executable
SHA-256 `f26905f0ecd9d9c960475cec9504378056df2c0e7e0439bf17c50d6c94ef9279`, MSVC
**14.51.36231**, Windows SDK **10.0.26100.0**. Six tests and strict app/bin/test Clippy from the
unchanged `fe8cb154` source remain applicable. The single fixed NVIDIA run finishes **00:10:33
UTC** in **1,149.027 seconds**, exit **1** for its measured growth-budget failure, with timeout
**false** and forced cleanup **false**. The recorded fixture and external watchdog lifetimes
are both **1,800 seconds**. No source, route, driver, Present or threshold change occurs.

All **100** cycles complete, each with actual source resize/restart, healthy-source/output
advance while the peer is absent, retirement-acknowledged project replacement and production
native endpoint retirement/recreation. There are **1,000** accepted geometry-edit receipts in
**100 bursts of 10**, each scheduled at 100 Hz; **386** requests are superseded. This reports
the actual burst shape and is not a full-app/Matrix Q05 claim. Raw QPC brackets verify every
post-work pause: minimum **2.0000695 s**, warmed baseline pause **10.001492 s**, final pause
**10.0014405 s**. These checks resolve the prior fixture pause/lifetime gaps without reducing
the requested workload or resource limits.

| Resource measurement | Cycle-10 baseline | Final drained state / observed maximum |
| --- | --- | --- |
| Handles | 939 | 935, delta **−4** |
| Private bytes | 229,220,352 | Final **236,404,736**, delta **7,184,384 bytes / 6.8515625 MiB** |
| Peak post-baseline checkpoint private bytes | Same baseline | **271,630,336**, growth **42,409,984 bytes / 40.4453125 MiB** |
| Logical texture reservation | 160,358,672 | **160,358,672**, exact return |
| Active plan bytes | 32,256,000 | **32,256,000**, exact return |
| Prepared / retiring / input bindings | 0 / 0 / 2 | **0 / 0 / 2** |

The unchanged verifier is eligible and fails the **32 MiB maximum checkpoint growth** budget;
handles and logical return pass their scoped checks. The lower final drained growth does not
override the peak criterion. This is neither proof of an unbounded leak nor proof of a bounded
driver cache. No cause is inferred from process counters alone. Physical GPU allocation identity
also remains unknown. Do not substitute final-only semantics or a higher allowance silently.

All **103** owned source/consumer exit sidecars report zero/unforced exits. The renderer/native
window retirement interval is **2,658.4435 ms**; from the final checkpoint through parent exit,
including fixture and timing-worker cleanup, the QPC upper bound is **2,825.153 ms**, under five
seconds. The owned job has zero descendants at close, and all **101 distinct recorded PIDs**
are subsequently absent; some PIDs were reused between already-reaped child incarnations.
All **337** selected source/asset/tool inputs remain unchanged and match the pinned build.
Fresh native Home UIA observations show overlay Off before/after; paired inventories match
and bracket the native run. The [compact artifact](evidence/mapgyver-m03-lifecycle-full-2026-11-01.json)
retains the exact full summary, method, distributions, hashes and limitations, plus a reproducible
hash of all **419** local raw files. No raw capture archive is added to Git.

This full acceptance check was allowed to finish on its verified original handle beyond the
20-minute observation boundary; broader investigation remained closed, and no second native
experiment ran. Next is a bounded attribution of the already captured native private-memory
peaks and an owning implementation correction if justified. No unchanged baseline rerun, new
profiler prerequisite chain or budget reduction is authorized by this result. Window deadline
revision and genuine removal still await their explicit decisions. Complete GPU path, queue,
physical allocation identities, Q01/final Q02 and all later phase acceptance remain required.
**No whole acceptance item closes: M03 remains 7/9; overall remains 4/10 (40%).** Deliver this
measured result before the next implementation batch.
Final verification at **00:23 UTC** checks all 419 raw hashes, 337 unchanged inputs, exact native
summary/counters, pause/drain receipts and the whole-parent shutdown bound. The native 64-bit
creation FILETIME is preserved exactly; a JavaScript-number rounding in its copied artifact
was caught and corrected before delivery. Both Cargo formatters and remote reconciliation
pass. This batch ends with the evidence checkpoint before **00:33 UTC**; no new investigation
or unchanged native rerun is started.

**Lifecycle fixture correction batch 66:** started 2026-10-31 **23:13 UTC**, stop by **23:58 UTC**.
Previous turn is progress: `2f1c575e` adds the fixed lifecycle workload and preserves its first
resize rejection. Only gate 9 remains active. **Initial hypothesis, withdrawn after the public
contract check below:** correct Golden's source pool to distinguish the
same logical texture ID across completion-retained binding incarnations. Each prepared plan
must retain and select its exact subscription/readers, including same-ID descriptor or sender
changes; source/worker/storage bounds, healthy paths, project acknowledgement and retirement
must remain intact. No app/channel identity churn or renderer restart workaround. Verify old/
new plan overlap and capacity refusal with meaningful tests, then run the unchanged 100-cycle
fixture once after software/strict Clippy/build prerequisites pass. Investigation ends by
**23:33 UTC** or two experiments; no timing/Present or driver investigation. Preserve batch 65's
failure. Keep the same live handle through observation timeouts. Checkpoint/push the bounded
correction and actual native outcome before opening more scope. Missing full GPU/queue/physical
allocation evidence and unapproved window/recovery decisions remain open; no partial check
closes the full renderer gate.

**Contract reconciliation, 23:16 UTC:** Golden's documented contract and existing native
regressions explicitly distinguish a Spout subscription's admitted maximum from each plan's
actual texture descriptor. Resize within that maximum retains the binding; genuinely changing
the immutable selector/maximum requires a fresh resource identity, as the app compiler already
does. Batch 65 mistakenly rewrote both descriptors, so its `UnsupportedPlan` rejection is
correct. The initial diagnosis of a Golden pool defect is withdrawn. Fix the fixture to keep
the Q01 maximum and change only its actual external target; add regression coverage for this
distinction. Stop broader ownership investigation now. No core/API change or identity churn is
needed for the fixed 100-cycle resize workload. All original lifecycle counts, spacing, bursts,
growth/drain/ownership budgets and failure evidence remain unchanged. After software/build
checks, run that corrected fixed series once. This is a concrete correction of the failing
fixture, not a gate relaxation or a second timing investigation.

**Batch 66 result:** five regression tests and strict app/bin/test Clippy pass before the
pinned release build. The new test would reject the old fixture: the immutable maximum remains
1920×1080 while actual target extent alternates 1920×1080 / 1280×720. Source/channel/texture
identity stays stable through real receiver resize. Each streaming checkpoint now requires a
new renderer copy and API acceptance newer than the preceding submission on all three outputs.
No Golden source code changes. The executable is
`a729ee730dc2997373486b7dda821f4748158f4b12a5718b081bedf575d04099`, MSVC **14.51.36231**,
Windows SDK **10.0.26100.0**.

The single fixed full-count attempt is terminal at **23:36:34 UTC** after **900.101 s**:
watchdog timeout **true**, forced cleanup **true**, exit **1**. It completes **95** cycles,
**950** distinct preview edits and **370** superseded outcomes, with **96** normal source-exit
sidecars. It reaches cycle 96 but never completes the 100-cycle final drain/summary. All captured
inputs remain unchanged; all **337** selected hashes match that build. Paired fresh native Home
UIA observations show overlay Off; inventories match. The owned job is empty after cleanup and
all **100** recorded native PIDs are absent. The raw failure and original batch-65 capture stay
preserved. The planned live check was allowed to finish on its original handle beyond the
23:33 investigation cutoff; no new investigation or second native experiment started.

Cycle-10 baseline: **939 handles / 226,955,264 private bytes**. Last matched-size checkpoint,
cycle 94: **934 handles / 265,527,296 private bytes**. Maximum post-baseline checkpoint growth
is **45,875,200 bytes (43.75 MiB)**, beyond the unchanged **32 MiB** budget; maximum checkpoint
handle delta is **−1**, final matched delta **−5**. These are incomplete-prefix observations,
not a full pass or a diagnosed leak. Matched-size logical reservation and active-plan bytes
equal baseline; physical allocation identities remain unverified. Completed-cycle retirement
maximum is **2,678.7643 ms**. No full shutdown/retirement certificate is inferred from the
watchdog's forced end.

The minimum cycle work is **8.8951 s**. The old wait used `cycle_start + 2 seconds`, so work
that already took longer received no two-second pause after completion. Private checkpoints
were taken immediately after constructing the replacement renderer. Correct that fixture:
wait the full requested pause **after** lifecycle work, retaining the required 10-second
baseline/final drains; record the pause's QPC bracket. Keep the memory/handle verdict unchanged,
including peak checkpoint growth, and do not call the observed memory excess fixed. The source/
consumer fixture lifetime is now **1,800 seconds**, and the next owned-process watchdog must
use that same bound. This permits the unchanged 100-cycle workload to finish; it reduces no
workload count or acceptance budget. The new pacing and lifetime have **six** passing tests,
strict app/bin/test Clippy and both formatters, but **no native verification yet**. Their five
changed input files are explicitly separated from the captured source version.

The [compact prefix artifact](evidence/mapgyver-m03-lifecycle-prefix-2026-11-01.json) retains
the exact limits, counters, launcher, build/environment identities, primary hashes and a
reproducible hash of all **399** raw files. Full logs remain local. The earlier artifact's
ownership-defect interpretation is explicitly superseded; its raw failure is unchanged.
Next is the corrected fixed 100-cycle measurement, with a properly sized external watchdog;
if growth still exceeds budget after the proper pause/drain, investigate that measured resource
boundary before another run. Full GPU path, queue, physical texture identity and all other
acceptance remain required. The window deadline proposal remains unapproved. **No gate closes:
M03 stays 7/9; overall stays 4/10 (40%).** Checkpoint/push before the next batch.
All final artifact/source/build checks, six tests, strict Clippy, both formatters and remote
reconciliation complete by **23:46 UTC**, before the **23:58 UTC** stop. Deliver this bounded
correction/evidence checkpoint; no second native attempt runs in this batch.

**Renderer lifecycle measurement batch 65:** started 2026-10-31 **22:47 UTC**, stop by
**23:32 UTC**. Previous turn is progress: `df3a8600` verifies the prescribed precise-timer Q02
series, preserves its window failures and delivers an unapproved window-only gate proposal.
Only gate 9 remains active. Add the missing applicable renderer lifecycle handle/private-memory
and exact live-texture/drain measurement at the existing production runtime/native fixture
boundary. Use its actual source/output/project replacement and superseding-edit operations;
preserve the fixed Q05 counts, two-second lifecycle spacing, post-first-ten-cycle baseline,
10-second drain and original +10 handles / +32 MiB growth budgets. Matrix/full-app Q05 stays
with M09. First identify reusable fixture paths and make one reviewable implementation slice.
Software tests/strict Clippy/build must finish successfully before at most one bounded native
verification. Investigation stops by **23:07 UTC** or two experiments; no timing/Present
investigation, threshold change, GPU reset, driver/overlay change or parked-stash restoration.
No partial lifecycle result closes the full renderer gate. Checkpoint/push before further work;
the window deadline decision and remaining GPU/queue observations stay explicit return items.

**Batch 65 implementation decision, 22:59 UTC:** reuse the existing owned source/consumer roles,
Golden `VideoRuntime`, public process counters and production `NativeOutputWindow` owner.
Each composite cycle restarts/resizes source A while source B and output delivery advance,
replaces the project through its actual retirement acknowledgement, publishes ten distinct
geometry edits on an absolute 100 Hz schedule, then retires/recreates both native window
endpoints and the Spout endpoint. All three lifecycle categories recur with at least two
seconds between cycle starts. Capture 100 completed cycles / 1,000 burst edits; sample the
warmed baseline after cycle 10 and final live state after a 10-second drain. Retain sampled
working set/peak working set, private bytes, handles, resource reservations and child exits.
Use the already implemented worker-owned precise waiter for burst/cycle pacing. Window owners
survive actual GPU retirement on every success/failure path; only the external owned-process
watchdog may terminate a stuck run. The fixed native check may finish after 23:07, within the
23:32 batch stop; this is planned verification, not an extension of open-ended investigation.
Logical reservations and physical GPU allocation identities remain explicitly separate;
successful handle/private measurements alone cannot close the full renderer gate.

**Batch 65 result:** four lifecycle regression tests and strict app/bin/test Clippy pass before
the pinned release build. It selects MSVC **14.51.36231**, Windows SDK **10.0.26100.0**, executable
SHA-256 `13b6993b8db96aa503a26b95638f75d761fa83ef63205b01a228f773ba337070`.
The one planned NVIDIA native attempt finishes **23:06:14 UTC**, before the 23:07 investigation
cutoff. It exits **1 in 7.1668 s** on the first changed-source plan, with no timeout or outer
forced cleanup. Zero cycles complete and the warmed baseline never exists. Initial two-source
streaming and source-B/output progress while source A is absent occur before the rejection;
these narrow observations do not qualify the requested lifecycle series.

The error is `Rejected(Backend: render plan has an unsupported or unbound native resource)`.
The owning branch is `native/inputs/mod.rs`'s `InputPool::ensure`: its cache looks up only
`TextureId`, then returns `UnsupportedPlan` when the requested `SpoutSource` differs. **The
following initial diagnosis was superseded by batch 66's contract reconciliation above.** Golden's
public `TextureId` is logical identity **within a render plan**. Retaining an older plan must
not permanently forbid a valid new source descriptor in a later plan. The next correction
belongs at this Golden binding/resource-generation boundary, preserving old reader/import
leases until their actual GPU uses retire. Do not work around it by restarting the renderer
before a source change or by forcing app/channel identity churn. Keep this failing fixture.

All **337** selected inputs remain unchanged during the native attempt; paired fresh native
Home UIA observations show overlay Off and the paired inventories match. The owned job has
zero descendants at parent close, and PIDs **12584/41656/39380/21212/23992** are absent. The
captured build did not persist individual child exit results on RAII-only failure cleanup;
that limit is explicit. `OwnedRole` now writes an exit sidecar on both explicit and Drop
cleanup, including a forced flag. Four tests/strict Clippy pass again for that correction;
it has **not** run natively. The capture stays bound to its original source/SDK/executable
fingerprints; the changed cleanup source is not passed off as natively verified.

The [compact artifact](evidence/mapgyver-m03-lifecycle-resize-rejection-2026-11-01.json) retains
the actual error, launcher, paired environment/build hashes and all 19 raw-file hashes. Raw
source/consumer logs remain local. There is no second native attempt, GPU reset, timing or
Present sweep. Logical live-set return, physical texture identities, lifecycle growth, complete
GPU path and queue observations remain open. The window deadline proposal remains unapproved;
all original budgets remain active. Both formatters and relevant software checks pass before
checkpoint delivery. **No gate closes: M03 remains 7/9 and overall remains 4/10 (40%).**
Next is the bounded source-binding correction and its resize/old-generation ownership coverage,
then this unchanged fixed lifecycle series. Q01/Q02 final series wait for the remaining
measurement instrumentation to stabilize, as recorded above. All unfinished phase returns remain.
Final software/formatting/artifact checks and remote reconciliation finish by **23:12 UTC**,
well before the **23:32 UTC** batch stop. Deliver the fixture/failure checkpoint now; do not
use the remaining batch time to reopen timing research or repeat the failed native run.

**Sustained timing acceptance batch 64:** started 2026-10-31 **22:22 UTC**, stop by
**23:07 UTC**. Previous turn is progress: `9cdba745` delivers worker-owned precise deadlines,
regression coverage and a finite native control, with no gate closure. Only gate 9 is active.
Deliver one fixed prescribed NVIDIA Q02 series on that exact verified release build and route:
three samples, each 30-second warmup, 120-second measurement and 60-second minimize/disconnect
intervals, with PresentMon and paired fresh environment/overlay-Off observations. No runtime,
Present, driver, cadence, join or threshold change. Verify current executable/source provenance
before launch and all owned processes/trace sessions after terminal completion. Keep the same
live execution handle through observation timeouts. The native acceptance observation is bounded
by **22:42 UTC**; do not launch another experiment or reopen timing investigation. If the series
passes observed budgets, retain that evidence and proceed to the explicitly missing GPU/queue/
growth requirements; if it fails, preserve the exact failing criteria and decide the next
bounded action rather than repeating a control or resetting cumulative investigation effort.
Q01 on the same route remains queued. No missing measurement or short control closes a gate.
Checkpoint and push the result before the next batch; all original acceptance remains required.

**Batch 64 result:** the one planned prescribed series finishes **22:38:56 UTC**, before the
22:42 native observation cutoff, exit **1** because window deadlines fail. The paired-environment
analysis finishes **22:40:21 UTC**, also exit **1** for that same observed failure. All three
samples use the unchanged batch-63 release executable, MSVC **14.51.36231**, Windows SDK
**10.0.26100.0**, NVIDIA renderer LUID `00000000:000170d3` and Intel display LUID
`00000000:00016c2a`. Fresh native Home UIA observations show `Enable and open overlay` before
and after; overlay stays Off. Paired inventories match, and environment identity passes all
three samples. Source/workload/Present/cadence/deadline/join/driver settings are unchanged.

| Sample | Main/editor misses out of 7,200 each | Spout misses | Exact sample Spout captures | CPU wall p95/p99 ms | Renderer-context GPU p95/p99 ms | Frame-start lateness p95/p99 ms | Shutdown ms |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | 214 / 214 | 0 | 7,200 / 7,200 | 0.6848 / 0.8268 | 3.487680 / 3.508512 | 0.5361 / 0.5902 | 441.9306 |
| 2 | 36 / 37 | 0 | 7,200 / 7,200 | 0.6371 / 0.7305 | 3.447200 / 4.198880 | 0.5242 / 0.5642 | 457.6566 |
| 3 | 8 / 8 | 0 | 7,200 / 7,200 | 0.6151 / 0.7063 | 1.845632 / 1.941184 | 0.5374 / 0.6230 | 435.4011 |

Every observed criterion passes except `deadline_misses`. Every accepted sample Spout image
has an exact completed independent capture; all six 60-second interruption cohorts and their
age/independence checks pass. Context-local GPU measurements remain explicitly narrower than
the required full GPU path. Complete GPU critical path, published queue/age and renderer
lifecycle handle/private growth/drain remain unmeasured; physical/DPI and full-app Q05 retain
M05/M09 ownership. Historical before/after captures support the practical timing improvement
but are not a controlled isolation of its cause. Windows still exceed the unchanged budget.

All **48 raw hashes** are recomputed, all **333 execution inputs** match current source, and
the executable still matches its build provenance. All 12 renderer/fixture processes exit zero
without forced cleanup and are subsequently absent; all three owned ETW sessions return **4201**.
The complete raw captures/analyses stay local; the [compact evidence](evidence/mapgyver-m03-precise-q02-prescribed-2026-10-31.json)
retains criteria, distributions, hashes and exact limitations. No second native experiment,
new profiler or timing sweep occurs. Both Cargo formatters and evidence/source checks precede
checkpoint delivery. **No acceptance item closes; M03 remains 7/9, overall 4/10 (40%).**
The pacing issue has already consumed day-scale cumulative effort; do not reset that clock.
Prepare the explicit decision below, then measure the still-required renderer lifecycle growth
and remaining GPU/queue observations before another long series. Q01 remains required on the
final mechanism/route. Gate 4 still awaits its separate genuine-removal/reset decision.
Artifact/source/build verification, both Cargo formatters and remote reconciliation complete
by **22:45 UTC**, before the recorded **23:07 UTC** stop. Deliver this evidence/proposal checkpoint
without changing an operative gate, then request the one scope decision. Runtime sources remain
unchanged; the prior software/strict-Clippy prerequisites and exact batch-63 build still apply.

#### Window deadline proposal — awaiting user decision

**Proposal only; not approved and not operative.** The current 16.667 ms / 0.1% gate and every
recorded failure remain in force. No code or qualification contract is changed by this proposal.

For compositor-managed native window/editor display in M03 and integrated M09 qualification,
allow one additional cadence slot for the software-display witness: use the end of the next
slot, approximately **33.333 ms from the current slot start at 60 Hz**, with the same **≤0.1%**
miss budget. Calculate the boundary from the exact anchored rational cadence, not a rounded
millisecond constant. Keep 60 fps publication/API cadence, complete image/display cohorts and
all measured present/display interval distributions. Continue reporting the original one-slot
window miss counts as a stricter diagnostic so the remaining pacing debt stays visible.

Spout keeps the original one-slot **16.667 ms / ≤0.1%** requirement. GPU p95/p99, CPU, complete
source-to-output p95 **50 ms**, queue depth, allocation, lifecycle growth, retirement, shutdown,
recovery, physical/DPI/input and image-correctness requirements remain unchanged. This proposal
does not close the unavailable full GPU/queue/lifecycle measurements or any other phase.

Rationale: this bounded optimization has consumed roughly a day across repeated investigations.
The complete current Q02 series delivers all 7,200 images per output per sample with exact
window witnesses and independent Spout captures. Its remaining window over-deadline maximums
are **3.8755/11.6461/2.2758 ms**, all below one extra frame period. This motivates a principled
one-refresh allowance for composed display; it does not prove a particular compositor/driver
cause or retroactively qualify a failed series. **Tradeoff:** up to one additional refresh period
of window latency is tolerated; within-bound pacing jitter could remain. Pixel identity,
complete cohorts, cadence/interval reporting and the separate source-age budget stay required.

If approved, update the contract, plan and exact analyzer/regression coverage together, retain
historical strict failures, finish missing measurement instrumentation and run fresh prescribed
Q01/Q02 on one frozen input set. If declined, retain the strict gate and require new causal
evidence or a demonstrably suitable display route before another timing correction; continue
the independent missing gate-9 measurements rather than repeating a blind timing variant.

**Window-deadline audit batch 63:** started 2026-10-31 **21:54 UTC**, stop by **22:39 UTC**.
Previous turn is progress: `52bf1090` verifies full-series NVIDIA Q02 environment, exact capture/
age, Spout deadlines and independence. Windows are the only observed failing endpoint; GPU-path,
queue and lifecycle requirements remain unavailable. Gate 9 alone is active. Use the existing
three captures to reconcile native scheduler slot epochs, actual Present calls and exact display
witnesses. Decide whether the correction belongs in runtime/window scheduling or the app-owned
deadline join; do not assume a cause or change the metric to manufacture a pass. The investigation
ends **22:14 UTC** (20 minutes), with at most two fixed offline analyses of the existing data and
no new profiler/baseline/driver or timeout/Present sweep. If a concrete defect is proven, implement
its bounded correction, add meaningful regression coverage and preserve the original failures.
Missing evidence stays open. Compile/software checks must pass before any necessary finite
native verification, which remains bounded by the 22:39 batch stop. If a contract interpretation
change is actually needed, prepare a precise reviewable proposal and seek user approval before
applying it. Checkpoint/push the result before opening further work; Q01 remains in the return queue.

**Audit decision, 22:09 UTC:** native slot/QPC inverse checks and exact Present joins show no
sample-origin or epoch mismatch. Actual composed-flip display intervals vary near 8/25 ms, and
both windows commonly share a display timestamp. Native frame CPU work begins **1.52/1.52/1.62 ms
late at p95** relative to its recorded slot; Present is issued **1.65/1.61/1.76 ms** after slot
start. The IO worker currently uses a condition-variable timeout. The two fixed offline analyses
are complete (a case-sensitive CSV-column typo was corrected within the second); stop broader
investigation now, before 22:14. Do not change the deadline join or infer a shader cause.

Implement one bounded IO-boundary correction: opt-in worker-owned Windows high-resolution
deadline timer with an event tied to the existing wake generation; native video opts in, ordinary
workers retain their default wait. Preserve cancellation, queued-command priority, checkpoint
race handling and kernel-handle retirement. Verify notification-before-wait, in-wait publication,
long-deadline cancellation and repeated expiry/notification through real kernel events. Add IO
sources to execution fingerprints. The hypothesis is that precise deadline waiting reduces
observed frame wake jitter; it is **not** a proven fix for every display miss. Software/strict
Clippy/build prerequisites precede at most one fixed finite NVIDIA Q02 control with PresentMon,
within the 22:39 stop. Retain only a supported timing improvement with ownership/wake behavior
intact; preserve all remaining window failures and original budgets. No parameter sweep.

**Batch 63 result, 22:20 UTC:** implemented the opt-in timer/event at the Golden IO boundary,
enabled only by native Video. Its worker retains both kernel handles; notification callbacks
retain only a weak signal reference. Wake generation/checkpoint races, queued-command versus
deadline priority, cancellation, repeated timer expiry/rearming and post-retirement notifications
have regression coverage. No global timer-resolution change, busy spin, cadence/anchor rewrite,
Present change or threshold change. IO source/Cargo inputs now join capture fingerprints.
**63 IO tests, 163 video tests (23 explicit hardware ignores), 107 qualification-tool tests** and
strict IO/video all-target and MapGyver native-bin Clippy pass. The pinned release build selects
MSVC **14.51.36231** and Windows SDK **10.0.26100.0**; executable SHA-256
`e77757ad83d56054d9e5ef986d8dcb9362c5a9d57b551fefa5fa74151da4ad9d`.

One fixed NVIDIA Q02 control (5-second warmup, 10-second sample, one-second interruptions,
PresentMon) completes normally. Exact token/slot frame-start lateness is **0.257/0.534/0.572 ms
median/p95/p99** across 600 sample frames, maximum **0.853 ms**. Each output accepts **600/600**;
main/editor each miss **1/600** display deadlines, Spout misses zero and independently captures
**600/600** accepted images. Minimize/disconnect each observes **60/60** complete Spout images
and 60 main deliveries; the criterion remains unavailable because its required duration is
60 seconds. CPU wall p95 **0.616 ms**, renderer-context GPU p95 **1.649 ms**; the latter still
does not measure the full GPU path. Both HWNDs survive actual **142 ms** resource retirement;
shutdown is **449 ms**. All four owned native processes exit zero without forced cleanup, all
are subsequently absent, and ETW live session query returns **4201** (absent).

Fresh paired environment observations preserve overlay Off and NVIDIA-renderer/Intel-display
identity. All **333** selected execution inputs are unchanged, including **30** IO inputs.
The first environment-analysis invocation mistakenly selected the series root; its missing-
manifest error is retained, and the corrected sample-directory analysis exits zero. The short
control is **UNAVAILABLE**, never qualified. The two fixed offline analyses and single planned
finite verification are the only experiments in this batch. Retain the supported timing
mechanism; no further wake/Present sweep. **No acceptance item closes; M03 remains 7/9.**
The user clarified 100% completion and explicit returns for unfinished phases; AGENTS.md now
requires cumulative effort tracking so a new batch cannot reset the day-scale decision point.
Next is the fixed sustained Q02 check, followed by Q01 and the remaining GPU/queue/growth
evidence. Real removal still awaits its explicit external reset decision. Checkpoint/push this
bounded result before opening the next batch; the recorded batch stop remains 22:39 UTC.
[Compact evidence](evidence/mapgyver-m03-precise-worker-q02-2026-10-31.json).

**NVIDIA Q02 acceptance batch 62:** started 2026-10-31 **21:28 UTC**, stop by **22:13 UTC**.
Previous turn is progress: `471ee4db` implements and verifies paired build/environment evidence
and the actual GPU/display route. Only M03 gate 9 is active. The existing executable and all
303 selected execution inputs still match the verified build: MSVC **14.51.36231**, explicit
Windows SDK **10.0.26100.0**, executable SHA-256
`1d24d813a817d7cfef8d4028b1e0521cb26eb857070a6e1bffca51d561da5cc8`.
Existing **163 Golden Video / 192 app / 107 analysis** tests, strict Clippy, formatting, selected
release build and real environment preflight remain applicable; do not repeat unchanged checks.

Explicitly select NVIDIA hardware index **1**, LUID **`00000000:000170d3`**, rendering to the
Intel-owned display route established in batch 61. Capture fresh overlay-Off/UIA and complete
environment evidence before and after one full Q02 release series: three samples, each **30 s
warmup + 120 s measurement**, separate **60 s minimized / 60 s disconnected** editor intervals,
held previews, unchanged 4K/two-source workload and all original budgets, with PresentMon. Native
start **21:29 UTC**, cutoff **21:49 UTC**, before the batch stop. Keep source/assets and the
same verified process handle fixed; a polling timeout never authorizes a restart. Preserve raw
and initial analysis, then add a new environment-aware analysis after actual terminal cleanup.
No timing/SDK/driver/power/overlay setting change, source edit, new profiler, second performance
candidate or parked-stash restoration. A passing Q02 still requires Q01 on this same build/route;
do not mix adapters' samples or infer physical 4K/full GPU-path/queue/lifecycle qualification.

**Terminal result, 21:49 UTC:** original and additional environment-aware analyses are retained.
The complete prescribed NVIDIA Q02 series is **FAILED_NATIVE**, with **window displayed
deadlines as its only observed failing criterion**. Full-series **environment identity passes
in all three samples**: actual MSVC/SDK/executable/source, driver/default WebView/AC power and
display identity stay fixed, before/after QPC snapshots bracket the native work, fresh native
Home UIA exposes `Enable and open overlay` at both ends, and the setting is unchanged.

| NVIDIA Q02 measured evidence | Sample 1 | Sample 2 | Sample 3 |
| --- | --- | --- | --- |
| Main accepted / Busy, of 7,200 | 7,200 / 0 | 7,200 / 0 | 7,200 / 0 |
| Editor accepted / Busy, of 7,200 | 7,200 / 0 | 7,200 / 0 | 7,200 / 0 |
| Spout accepted / Busy, of 7,200 | 7,199 / 1 | 7,199 / 1 | 7,200 / 0 |
| Main deadline misses, of 7,200 | 4,572 | 1,841 | 139 |
| Editor deadline misses, of 7,200 | 4,572 | 1,846 | 141 |
| Spout deadline misses, of 7,200 | 1 | 1 | 0 |
| Exact accepted Spout image captures | 7,199 / 7,199 | 7,199 / 7,199 | 7,200 / 7,200 |
| Missing accepted images, minimized / disconnected | 0 / 0 | 0 / 0 | 0 / 0 |
| Editor independence | Pass | Pass | Pass |
| Environment identity | Pass | Pass | Pass |
| CPU inclusive wall p95 / p99, ms | 0.5569 / 0.6511 | 0.6564 / 0.7761 | 0.6950 / 0.7996 |
| Renderer-context GPU p95 / p99, ms | 3.5225 / 4.1014 | 4.2178 / 5.7830 | 3.3831 / 3.5038 |
| Complete shutdown, ms | 446.4 | 454.0 | 426.0 |
| Actual runtime retirement, ms | 134.4 | 130.1 | 123.9 |

All remaining observed criteria, including API cadence, exact capture/age and Spout deadline
bounds, pass. The complete GPU critical path, renderer lifecycle handle/private growth and
published queue/age requirements remain unavailable. Renderer-context timings do not certify
the complete GPU path. Native/client backing is genuinely 4K, but the selected physical display
is still 1920×1080; physical 4K/DPI/input remain with M05. No cross-adapter guarantee is inferred.

The exact same native/collector process handles finish normally, with zero forced cleanup. Each
submitted frame has exactly three final output records, **zero duplicate token/output rows** and
raw outcome totals matching runtime counters. All **48 raw hashes** verify and execution inputs
stay unchanged. All three owned trace sessions are independently absent (`4201`), no recorded
renderer/source PID remains live, and both HWNDs are retained until actual retirement before
output destruction. The initial and environment-aware analyses stay separate and hashed.

This evidence verifies complete-series environment pinning and narrows the observed failure to
the window display endpoint. It does not establish whether the cause is scheduling, presentation
or a deadline/cohort mapping defect. Next inspect the existing native schedule, Present and
display-event epochs, with a specific decision and bounded scope, before selecting a correction.
Do not repeat this baseline, sweep timeouts/Present modes, restore the parked collector stash,
redefine the deadline gate or spend another batch on GPU shader tuning from these timings.
Q01 on this same build/route remains required after that decision. M03 **7/9**, overall **40%**,
with gate 9 alone active. [Hashed compact evidence](evidence/mapgyver-m03-nvidia-q02-prescribed-2026-10-31.json).

Final root/Golden Core formatters, whitespace and compact-evidence integrity checks pass.
Checkpoint preparation completes by **21:54 UTC**, before the recorded 22:13 stop. This batch
changes status/evidence only; no further native experiment or implementation/threshold change.

**Environment/route acceptance batch 61:** started 2026-10-31 **20:46 UTC**, stop by **21:31 UTC**.
Previous turn is progress: `37901bea` captures the complete unchanged Q02 failure, ruling out
accepting the Q01 correction as sufficient 4K qualification. Gate 9 alone remains active.
Deliver read-only reusable DXGI adapter/display-route metadata in Golden Video and app-owned
qualification environment capture/validation. Reuse the existing baseline inventory; identify
the actually selected linker/Windows SDK from rustc's supported printed link invocation, not
installed-tool guesses. Record build/executable hashes, drivers, WebView, power, exact adapter/
display route and explicit overlay observations with missing evidence kept unavailable. Verify
missing/changed/mismatched identity rejection and real hardware enumeration without creating
rendering resources. Use the computer-use skill for the NVIDIA overlay observation; preserve
the user's setting. Software checks and a release build precede at most two finite native checks
within 20 minutes of their recorded start and the 21:31 cutoff. A short capture may prove the
environment path but cannot qualify Q01/Q02. No timeout/Present sweep, GPU reset, driver/power
change, new profiler or parked-stash restoration. Checkpoint/push this delivery before scheduling
any full NVIDIA series; both workloads must eventually qualify on the same recorded route.

Software checks pass: **163 Golden Video tests / 23 explicit hardware ignores**, **192 app tests /
one existing hardware ignore**, **106 production-analysis tests**, strict native all-target and
qualification-bin Clippy, and both formatters. The first release build succeeds, but build
provenance correctly fails closed because rustc's printed command omits inherited SDK search
paths. Preserve its logs at `target/qualification-env-build-2026-10-31-b61`; no native check used
incomplete provenance. The corrective build explicitly selects the observed installed SDK
`C:/Program Files (x86)/Windows Kits/10/Lib/10.0.26100.0` with UM/UCRT `/LIBPATH` arguments;
the actual printed linker invocation must verify that selection. A recorded final-binary
metadata salt forces a real link observation without rebuilding dependencies or changing runtime
budgets/pacing. Native display rectangles are documented as caller-DPI-dependent DXGI metadata,
leaving actual physical placement with the host. NVIDIA Home UIA exposes `Enable and open overlay`
(Off), left unchanged; supported activation retry fails, so no further UI input is attempted.

Read-only route observations: both active GDI displays belong to Intel LUID
`00000000:00016c2a`; NVIDIA RTX 5080 Laptop GPU LUID **`00000000:000170d3`**, hardware index **1**,
has no directly attached output. A NVIDIA-rendered window here is therefore an explicitly
recorded cross-adapter display route, not an undocumented same-adapter assumption. Preliminary
PowerShell serialization stringified nested rectangle arrays at depth 8; retain that preliminary
snapshot as failed component evidence. Increase serialization depth and reject truncated shapes;
the full analysis suite now passes **107 tests**. The final provenance build succeeds with the
selected SDK and source hashes, executable SHA-256
`1d24d813a817d7cfef8d4028b1e0521cb26eb857070a6e1bffca51d561da5cc8`.

Finite native verification starts **21:20 UTC**, cutoff **21:31 UTC** (the batch stop).
Select NVIDIA index **1** explicitly for one diagnostic Q02 run: **1 s warmup, 3 s measurement,
1 s minimized and 1 s disconnected**, no PresentMon. This checks paired environment/route
evidence, exact identity and real ownership; it cannot qualify cadence/display Q01/Q02. A fresh
UIA `Enable and open overlay` observation and complete before snapshot are recorded before
launch. Observe overlay again and capture the after snapshot only after the actual owned run
finishes; analyze to a new output without replacing the original raw/analysis. No extra native
experiment or full NVIDIA series is added to this batch.

**Terminal result, 21:21 UTC:** the component verification succeeds. Fresh complete before/after
snapshots have identical pinned identities and bracket the actual native run in machine QPC;
fresh UIA overlay observations also bracket it. The environment analyzer reports
**`observed_within_target: true`**, renderer NVIDIA LUID `00000000:000170d3`, display owner Intel
LUID `00000000:00016c2a`, **`same_adapter_display: false`**. Driver PCI identity, actual printed
MSVC **14.51.36231** / explicitly selected Windows SDK **10.0.26100.0**, default host WebView
version, AC power and the executable/source hashes are present. Before/after inventory preserves
typed nested metadata after the serialization correction. No setting was toggled.

All three outputs accept **180/180** measured frames, and the independent Spout cohort captures
**180/180** accepted images with zero missing. CPU inclusive wall p95/p99 **0.5432/0.6272 ms**;
renderer-context GPU p95/p99 **4.1174/5.8648 ms**, explicitly not full-path or sustained evidence.
Actual runtime retirement is **132 ms**, complete shutdown **450 ms**, both HWNDs remain alive
through retirement and output destruction follows it. Native/source processes exit normally,
none remain live, all **15** native raw hashes and **303** unchanged execution inputs verify.
No PresentMon session is created. The separate environment analysis preserves the original
capture/analysis rather than rewriting them. Source, snapshot, link-log and library hashes are
retained in the compact evidence; analyzer evidence digests use sorted JSON, while the evidence
file map separately hashes actual file bytes.

This completes the environment-recording implementation and its real component verification,
not the full-series acceptance item. Diagnostic scope keeps environment criterion status
**UNAVAILABLE** despite the true observed result. Full NVIDIA Q02 and Q01 with fresh paired
snapshots/PresentMon remain next, on this same explicit route and unchanged budgets; Intel Q02's
failures and every other unmeasured requirement remain required. M03 **7/9**, overall **40%**.
[Hashed compact evidence](evidence/mapgyver-m03-environment-route-2026-10-31.json).

Final root/Golden Core formatting, whitespace and current build/source/executable identity
checks pass. Checkpoint preparation completes by **21:29 UTC**, before the recorded 21:31 stop.
No second native experiment, full NVIDIA series, setting change or acceptance waiver occurred.

**Q02 acceptance batch 60:** started 2026-10-31 **20:24 UTC**, stop by **21:09 UTC**.
Previous turn is progress: published `a105f9b0` corrects native window admission and passes every
observed criterion in the complete Q01 release series, including previously failing deadlines
and independence. Only M03 gate 9 remains active. Run the complete prescribed Q02 series against
that exact implementation/executable: first source and both main outputs **3840×2160**, second
source **1280×720**, two held editor previews, original 60 Hz cadence/budgets, three release
samples with 30 s warmup + 120 s measurement and separate 60 s minimize/disconnect intervals,
PresentMon enabled. No new performance candidate, parameter sweep, profiler, driver/overlay
change or source edit. A single fixed series may finish under its original verified process
handle; do not restart it on observation timeout. Preserve all failures and unavailable metrics;
4K client/backing size is not physical 4K scanout or M05 display/DPI acceptance. Native start
**20:24 UTC**, cutoff **20:44 UTC**, before the batch stop. Prerequisite software, strict Clippy,
native pixel/ownership tests and release build passed in batch 59; compare the current executable
and every recorded execution-input hash to that verified capture before starting instead of
repeating unchanged checks. Update the dashboard/evidence and checkpoint/push before any next
implementation batch. M03 gate 4 still awaits the reset decision; all later return triggers stay.

**Terminal result, 20:41 UTC:** the complete three-sample Q02 series is **FAILED_NATIVE**.
All **301** execution-input hashes and the release executable match the verified Q01 build
before startup; inputs remain unchanged through all runs. The native window owner verifies
actual **3840×2160** physical client/backing at **(2560, 0)**, DPI **1**, without sizing retries.
The selected physical monitor is **1920×1080**; no physical 4K, DPI, alignment or scanout pass is
inferred. No implementation, adapter, driver, overlay, workload, cadence or budget changed.

| Q02 measured evidence | Sample 1 | Sample 2 | Sample 3 |
| --- | --- | --- | --- |
| Main accepted / Busy | 7,192 / 8 | 7,195 / 5 | 7,198 / 3 |
| Editor accepted / Busy | 7,175 / 25 | 7,155 / 45 | 7,116 / 84 |
| Spout accepted / Busy | 7,199 / 1 | 7,196 / 4 | 7,199 / 1 |
| Main deadline misses, of 7,200 anchored slots | 7,037 | 7,087 | 4,277 |
| Editor deadline misses, of 7,200 anchored slots | 7,198 | 7,173 | 6,984 |
| Exact accepted Spout image captures | 7,195 / 7,199 | 7,189 / 7,196 | 7,197 / 7,199 |
| Missing accepted images, minimized / disconnected | 1 / 1 | 1 / 0 | 2 / 0 |
| CPU inclusive wall p95 / p99, ms | 0.5619 / 0.7097 | 0.5916 / 0.8970 | 0.5301 / 1.1823 |
| Renderer-context GPU p95 / p99, ms | 15.0306 / 16.2581 | 15.1067 / 16.5147 | 13.3021 / 16.3720 |
| Outputs GPU-partition median / p95, ms | 7.6141 / 11.0453 | 7.7988 / 10.3905 | 5.5720 / 8.9110 |
| Complete shutdown, ms | 409.8 | 449.9 | 418.5 |
| Actual runtime retirement, ms | 110.2 | 119.3 | 111.8 |

The three native runs and owned collectors exit normally. Every submitted frame token has exactly
three final output records, with **zero duplicate token/output rows**; raw outcome totals match
runtime counters. All **48 raw hashes** verify, all three owned trace sessions are independently
absent (`4201`), and no recorded renderer/source PID remains live. Software/API cadence, displayed
deadlines and accepted-image capture cohorts remain separate; boundary-selected API rows are not
renormalized to improve the anchored 7,200-slot deadline counts. Observed captured source-age p95
is below 50 ms, but missing images prevent full capture/age acceptance. GPU partitions include
scheduling/dependency/command-feed gaps; they do not isolate shader cost or calibrate contexts.
The complete GPU-path, lifecycle-growth, queue and environment requirements remain unavailable.

This evidence rules out treating the Q01 admission correction as sufficient 4K qualification.
Next pin the required toolchain/SDK, driver, WebView, power and overlay identity and inspect the
actual adapter/display route using existing inventory/public mechanisms. The qualification
contract explicitly permits one named route: any switch to NVIDIA must be recorded before its
run, satisfy the overlay-Off prerequisite, and qualify **both** Q01/Q02 on that same route.
Do not claim an Intel/driver/display cause from these timings alone, add profiling prerequisites,
repeat this fixed Intel baseline, sweep timeout/Present constants, or reduce the 4K gate.
Q01's observed passes stay valid at their scope; all Q02 failures remain retained. M03 **7/9**,
overall **40%**, one active gate. [Hashed compact evidence](evidence/mapgyver-m03-window-admission-q02-2026-10-31.json).

Final root/Golden Core formatters, whitespace and compact-evidence integrity checks pass;
checkpoint preparation completes **20:45 UTC**, before the recorded 21:09 stop. This batch
changes evidence/status only; its earlier software/native prerequisite results remain applicable
through the verified identical executable and 301 execution-input hashes. No extra native run.

**Window-admission correction batch 59:** started 2026-10-31 **19:55 UTC**, stop by **20:40 UTC**.
Previous turn is progress: `7ef717e2` removes the duplicated qualification output-window owner
and captures the full production-route Q01 series. Its existing trace identifies admission Busy
before consumer APIs; no new baseline is needed. Own the correction in Golden Video's native
output scheduling, not MapGyver UI or qualification thresholds. Deliver ready outputs first,
then give initially busy windows one shared bounded kernel-wait opportunity within the same
submission/fence. Never retry an already-issued Present or wait on a foreign Spout consumer.
Prove permit consumption, bounded aggregate waiting, ready-output ordering, exact one-event/API
accounting and actual retirement with deterministic tests and existing native checks. Only after
compile/software prerequisites pass, allow one corrected Q01 series within 20 minutes of its
recorded start and the 20:40 cutoff. If the correction fails, preserve/revert it as appropriate;
do not sweep timeout/Present constants, add profilers or change acceptance budgets. Gate 9 alone
is active; the reset decision and other phases keep their recorded return triggers.

Software prerequisites pass: Golden Video **163 passed / 23 explicit native ignores**, app
**192 passed / one existing hardware ignore**, strict Golden Video all-target and MapGyver
qualification-bin Clippy, root/Golden Core formatting and whitespace. Two new real-kernel-event
tests cover ready-before-retry order, exact permit consumption, no retry after an issued Present,
hidden-window signal preservation, and **128 pending windows sharing one requested 1 ms timeout**.
The correction uses one preallocated retry list, one final output observation and the existing
submission completion guard; there is no extra GPU lifetime or pixel transport. OS scheduling
can exceed the requested timeout, so no hard wall-time guarantee is inferred. Release build
must finish before native verification. Retain the correction only if actual cadence/Busy and
CPU evidence support it without ownership, exact accounting or healthy-output regressions;
remaining software-display or other gate failures stay separate and required.

Release build succeeds in **2m 55s**. Native verification starts **20:03 UTC**, cutoff
**20:23 UTC**: first the existing two native window pixel/ownership tests, then one prescribed
three-sample Q01 release series (30 s warmup, 120 s sample, separate 60 s minimize/disconnect
intervals, PresentMon), directory `target/qualification-m03-window-admission-q01-2026-10-31-b59`.
Keep the exact running process handle and freeze code/assets through collection and analysis.
No further experiment or setting change is authorized by this batch.

**Terminal result, 20:20 UTC:** retain the correction. Every observed Q01 criterion passes in
the full prescribed release series, including the previously failing API cadence, anchored
software-display/Spout deadlines and editor independence. This is a concrete renderer correction;
it does not close gate 9 or certify unmeasured requirements. Series status is **UNAVAILABLE**,
not fully qualified. Candidate executable SHA-256:
`13e045b534124f23abbec9a1b8b727f1e4c85bf4079f6009cbb176da6c311bbd`.

| Q01 measured acceptance | Sample 1 | Sample 2 | Sample 3 |
| --- | --- | --- | --- |
| Main accepted / Busy, of 7,200 | 7,200 / 0 | 7,200 / 0 | 7,200 / 0 |
| Editor accepted / Busy, of 7,200 | 7,200 / 0 | 7,200 / 0 | 7,200 / 0 |
| Spout accepted / Busy, of 7,200 | 7,200 / 0 | 7,200 / 0 | 7,199 / 1 |
| Main / editor / Spout deadline misses | 0 / 0 / 0 | 0 / 1 / 0 | 0 / 0 / 1 |
| Exact accepted Spout image captures | 7,200 / 7,200 | 7,200 / 7,200 | 7,199 / 7,199 |
| Editor independence | Pass | Pass | Pass |
| CPU inclusive wall p95 / p99, ms | 0.6475 / 0.7280 | 0.5380 / 0.6274 | 0.5601 / 0.6227 |
| Renderer-context GPU p95 / p99, ms | 3.2306 / 3.8086 | 3.8580 / 4.0884 | 3.4514 / 3.8922 |
| Complete shutdown, ms | 386.6 | 406.7 | 397.6 |
| Actual runtime retirement, ms | 95.9 | 99.2 | 102.8 |

Across all phases, raw output records cover **16,225 / 16,223 / 16,225** submitted frame tokens
with exactly three final outcomes per token and **zero duplicate token/output rows**. Raw
Accepted/Busy/Unavailable totals match actual control counters exactly; admission retries do not
invent deliveries or double-count Busy. All **48 raw hashes** verify and execution inputs remain
unchanged. All renderer/source/collector processes exit normally, all three owned trace sessions
are independently absent (`4201`), and no recorded renderer/source PID remains live. Actual
retirement precedes HWND destruction in every run. Existing native pixel/ownership checks pass
**2/2 in 0.23 s**, including six flip-buffer frames, all 16 pixels/frame within **2/255**.

The correction is retained because its real signal/ordering behavior is verified, the prescribed
series meets observed cadence/CPU/independence budgets, and ownership/accounting stay intact.
This is not a claim that one comparison isolates all performance variation. Main/Spout still have
rare Busy outcomes outside measurement; they remain recorded. Next run Q02 with this exact
behavior in a new bounded batch, then close the remaining GPU-path, lifecycle-growth, queue and
environment requirements. No timeout/Present sweep, new profiler, driver change or gate waiver.
M03 remains **7/9** and overall **40%**. [Hashed compact evidence](evidence/mapgyver-m03-window-admission-q01-2026-10-31.json).

Final root/Golden Core formatters, whitespace and source ownership review pass. Checkpoint
preparation completes by **20:24 UTC**, within the recorded 20:40 stop; no third native check ran.

**Production-window qualification batch 58:** started 2026-10-31 **19:13 UTC**, recorded stop
**19:58 UTC**. Previous turn made progress: `8f9139f3` adds finite app output/pixel acceptance
and retains its supported-activation blocker without closing M05 gate 4. The scheduled M05
batch is complete; only M03 gate 9 is active again. The qualifier still creates a separate Tauri
output window and retries its DPI sizing, while actual MapGyver Screens use Golden's dedicated
`NativeOutputWindow`. Replace the duplicated fixture owner with that public production host
owner on the reporting worker. Retain exact physical client/bounds observations, GPU-before-HWND
retirement, independent editor handling and every original workload/criterion. This corrects
qualification route fidelity; it is not a proven display-deadline root cause. No pacing-constant
sweep, parked stash, UI-helper workaround or driver/monitor/overlay change. Build/software checks
precede one prescribed Q01 series (three 30+120-second samples, each with both 60-second editor
intervals and PresentMon), within a fixed 20-minute native window and the 19:58 stop. Preserve
all failures and missing metrics; physical/DPI/full-app gates and pending genuine reset remain.

Batch 58 native verification starts **19:26 UTC**, cutoff **19:46 UTC**, before the 19:58 batch
stop. Prerequisites are terminal: **192 app tests / one existing hardware ignore**, **100
production-analysis Python tests**, strict app/qualification-bin Clippy, both formatters,
whitespace checks and the release build (**3m 06s**). A remaining metadata `Ordering` import was
restored after compiler feedback; no native experiment ran against the failing build. Candidate
executable SHA-256: `5ef26b1570982e41203201a6b00ed59e66ae5bbd825e2b7b412af41232539a63`.

**Terminal result:** all three prescribed native runs and PresentMon collectors exit normally;
each owned trace session is independently absent (`4201`) afterward and no recorded renderer/
source child remains live. All **48 raw hashes** verify and execution inputs stay unchanged.
Each output has the actual production owner, physical client/backing **1920×1080**, origin
**(2560, 0)** and DPI scale **1**; client/bounds checks pass without sizing retries. Actual
retirement precedes HWND destruction in every run. Complete shutdown is **388/409/400 ms**;
runtime retirement is **99/95/100 ms**. This corrects the qualifier's route, not production pacing.

The series **fails**. Main output accepts **6,530/7,200** in sample 1, then **7,200/7,200** in
samples 2/3. Sample 1 has **670 Busy**, **670 late display** and **1,340 deadline misses**;
editor misses are **1/1/9**. Spout sample cohorts are complete **7,200/7,200** in all three.
Independence fails in samples 1/2: sample 1 retains main Busy outcomes and one disconnected
Spout Busy; sample 2 lacks one accepted Spout image during minimize. CPU p95 is
**0.536/0.591/0.626 ms**; renderer-context GPU p95 is **3.436/3.974/4.460 ms**, explicitly not
the complete GPU path. Full GPU-path, lifecycle growth, queue and environment requirements remain
unavailable. M03 stays **7/9**, overall **40%**; no acceptance budget changes.

The existing first-run raw trace records **1,216 main-output Busy outcomes before any consumer
API call**, across all phases. The next bounded source slice should inspect that admission
boundary and preserve delivery to ready outputs, real completion fences and exact API witnesses.
No new native experiment is added to this batch. Genuine removal still awaits the explicit
reset decision; M05 activation and all later acceptance retain their return queue.
[Hashed compact evidence](evidence/mapgyver-m03-production-window-q01-2026-10-31.json).

Final root/Golden Core formatters and whitespace checks pass; checkpoint preparation completes
at **19:54 UTC**, before the recorded batch stop. No production pacing change is retained.
Run exactly the prescribed Q01 release series on adapter 0 with three 30+120-second samples,
both 60-second editor intervals and `--presentmon`, directory
`target/qualification-m03-production-window-q01-2026-10-31-b58` (absent before start). Preserve
the same process handle and frozen source/assets through all samples/tails. CLI stdout is
discarded because full analysis is already saved locally; only compact results are inspected.

**Presentation correction batch 56:** started 2026-10-31 **18:01 UTC**, recorded stop **18:46 UTC**.
Previous turn made progress: `0fa30df0` captures the complete prescribed Q01 series and narrows
the failing criterion to window software display deadlines. Gate 9 alone is active; gate 4's
GPU-reset decision remains pending. Investigate the existing deadline/Present records and the
native output owner, then implement one justified presentation correction with original budgets
and independent output behavior preserved. Software checks/build precede at most two fixed native
invocations/20 minutes, also bounded by 18:46. Do not repeat the passing baseline unchanged,
restore the parked stash, change monitor/driver/overlay settings or add a diagnostic framework.
Read-only `EnumDisplayDevicesW` inventory maps both `DISPLAY1` and `DISPLAY2` to the same Intel
device `PCI/VEN_8086/DEV_7D67`; a cross-adapter desktop path is therefore not the demonstrated
cause. Existing accepted API/deadline and display records must justify the next implementation.

**Fixed hypothesis:** all sampled APIs submit on time, while late window software display
arrives after the deadline. The current flip-discard path uses `Present(0, DO_NOT_WAIT)`, whose
documented interval-zero semantics permit cancelling queued frames. Test one explicit vblank
per admitted frame (`Present(1, DO_NOT_WAIT)`) while retaining maximum frame latency one,
zero-timeout readiness, existing color conversion and independent Spout output. This is a
candidate, not an established root cause. Microsoft's
[Present contract](https://learn.microsoft.com/en-us/windows/win32/api/dxgi/nf-dxgi-idxgiswapchain-present)
and [low-latency sample](https://learn.microsoft.com/en-us/windows/uwp/gaming/reduce-latency-with-dxgi-1-3-swap-chains)
define these semantics. First run the existing direct-window pixel/flip-buffer test; then one
Q01 candidate sample with 30-second warmup, 120-second measurement and both 60-second editor
interruptions, with PresentMon. One sample is explicitly diagnostic and cannot close the
three-sample gate. Keep the candidate only if display deadlines improve without output/age/
retirement regressions; otherwise revert it and preserve the failed control. No parameter sweep.

Batch 56 native checks start **18:13 UTC**, hard cutoff **18:21 UTC** to keep the original
20-minute investigation bounded from 18:01, earlier than the 18:46 batch stop. Prerequisites
are terminal: **161 unit + four public integration tests / 23 hardware ignores**, strict Golden
Video all-target/all-feature Clippy, exact native test build, release runner rebuild (**2m 45s**),
both formatters and whitespace checks. Pixel-test binary SHA-256:
`1aac443a54a338adb5ad473e6e694d8efe8fb1116cc720204415e5956fa66a0c`;
candidate release: `1df092290dbcef0a9c2aa990655ae20a324e44bae2596efa46b3152acba6dc8a`.
Invocation one is the existing exact
`native::outputs::window::tests::hardware_direct_window_encodes_once_and_tracks_flip_buffers_across_presents`.
Only after its pass, invocation two is Q01 adapter 0 release, one sample, 30+120 seconds and
60-second minimize/disconnect intervals, with `--presentmon --diagnostic`, new directory
`target/qualification-m03-q01-sync1-2026-10-31-b56`. No new UI/asset/collector settings.

**Batch 56 result:** the direct-window test passes **1/1 in 0.18 s**, all six successive frames
and 16 pixels/frame within **2/255**. The candidate's single full-duration Q01 control completes
all intervals normally, but analysis returns **FAILED_NATIVE** for `deadline_misses`: main has
**5/7,200** late software displays, Spout **0/7,200**, and editor **893/7,200**. Editor lateness
p95/p99/max is **0.8760 / 0.9068 / 5.0381 ms**. Every accepted Spout image is independently
captured (**7,200/7,200**), age stays within target and complete shutdown is **1,444.1173 ms**.
This does not justify retaining interval one: its editor deadline failure remains inside the
variable baseline range. **The candidate is reverted to the exact published production source.**
The release runner is rebuilt from that restored source before handoff; no further GPU test or
parameter variant is run. Original scope, budgets and the **7/9 / 4/10** gate counts remain.
Restoration rebuild passes in **2m 44s**; its executable SHA-256 is
`948c68eb74318e42ffac22781304139730be646a57763e94a8b0c639c367a073`.
No owned renderer/fixture process remains, and the owned ETW session reports **4201 (absent)**.
The restored source passes **161 unit + four public integration tests / 23 hardware ignores**
again; required root/Golden Core formatters and whitespace checks pass. The production source
diff is empty. Next M05 work starts from the existing
`apps/mapgyver/tests/native_output/check.mjs` flow, preserving its prior API-only limits and
adding the specific concurrent pixels/editor-independence evidence needed by gate 4.

The [compact rejected-control packet](evidence/mapgyver-m03-sync1-control-2026-10-31.json)
records the candidate source/executable hashes, exact diff, failed criterion, distributions and
all **16 raw-file hashes**, recomputed against the local raw directory. Full analysis SHA-256 is
`a0d7f5f7c8082cd5012b47aa2a0eaddf13920a4568fede59db7d879c087519eb`.
The original three-sample Q01 baseline is preserved. A one-sample candidate is diagnostic,
not replacement qualification. Pixel correctness alone cannot make this timing change acceptable.

**Anti-drift handoff:** batches 55/56 yielded required evidence and rejected a concrete candidate,
but closed no acceptance gate. Apply AGENTS.md's two-batch rule: take one already implemented,
dependency-ready **M05 gate 4** concurrent-output acceptance batch, then return to M03's actual
presentation-pacing boundary. This is an explicit return trigger, not abandonment or a waiver.
Gate 4's real device-removal test remains ready and separately awaits approval. No new feature,
simultaneous gate expansion or further diagnostic variant is authorized by this handoff.

**Qualification batch 55:** started 2026-10-31 **17:32 UTC**, recorded stop **18:17 UTC**.
Previous turn made progress: `a277c2ca` verifies the corrected real-source preflight. The explicit
GPU-reset question is pending; no response is not approval. Source-worker review confirms its
existing drain checks its own device removal/completion, separately from renderer cancellation;
no speculative ownership change or redundant test is added. Gate 4 remains open and returns
after explicit reset approval. Gate 9 is now the only active gate. This is the next dependency-
ready acceptance after two batches without full gate closure, not new feature expansion.

Deliver one current prescribed Q01 qualification series using the existing runner/analyzer:
release build, three independent samples, 30-second warmup + 120-second measurement and two
60-second interruption intervals per sample, current exact source/asset fingerprints and owned
PID-filtered PresentMon. Keep every original budget and missing observation. Full series is one
fixed experiment, at most 20 minutes from its recorded native start; no extra control or parameter
sweep. Build/check software first; native failure ends the series through existing cleanup.
Archive raw evidence under a fresh target directory and publish compact measured results/failures.
No parked collector stash, driver/overlay change or GPU reset. Approval arriving during this run
does not authorize resetting its GPU mid-sample; retire this owned run before the recovery test.

Prerequisites: current preview/qualification UI builds; **100 production-analysis Python tests
pass**. The pinned PresentMon executable matches its required SHA-256
`b2a706bc6ad475749e3b7e3409263aa1e6906d45bdcf993f6dbc0f660188f1af` and the public trace-query
executable exists. No external UI URL/bind override is set. Release rebuild is tracked in its
original process handle; no native run starts before its terminal success. Read-only environment
inventory observes Windows **26300.9711**, Perf power scheme on AC, Intel driver
**32.0.101.8331**, NVIDIA driver **32.0.15.9201**, WebView2 **156.0.4314.50**, Rust/Cargo **1.97.0**
and Visual Studio Build Tools **18.5.11723.231** / Community **18.10.12224.181** installations.
These installed tool observations do not identify the compiler-selected MSVC/SDK or close the
environment criterion. Active controller modes are **2560x1600/240 Hz** and **3840x2160/60 Hz**;
physical output/DPI/input remains M05. No settings or overlay state changes occur.

Batch 55's single prescribed-series invocation starts **17:39 UTC**, cutoff **17:59 UTC**,
before the 18:17 batch stop. Release build succeeds in **3m 26s**; required formatters and
whitespace checks pass. Executable SHA-256:
`6f99800c17a87f11abe7d39c05dd9a57453092a36da3d576f10e4cb1a63e0b8b`;
trace-query helper: `7d244b07bd7172945d3ccef3fd9fef2713bfb18f4f4ebf439167dd619c2cc3f6`.
Command: `python tools/mapgyver/production_qualification.py run --directory target/qualification-m03-q01-full-2026-10-31-b55 --scenario Q01 --adapter 0 --profile release --samples 3 --warmup 30 --seconds 120 --isolation-seconds 60 --presentmon`.
The new directory is absent before execution. Retain the same live process handle through all
samples and collector tails; no second experiment or source/asset changes during capture.

**Batch 55 result (terminal before the 17:59 native cutoff):** the fixed **prescribed series**
completes all three independent native runs and collector tails on Intel LUID `00000000:00016c2a`.
The analyzer exits **1 / FAILED_NATIVE**, preserving the actual display-deadline failures below;
all three native renderers and collectors exit **zero**. This is a full required series, not
another short diagnostic. All before/after execution inputs and executable/tool hashes match.

| Q01 sample | Main / Spout / editor accepted | Main / Spout / editor deadline misses, each /7,200 | CPU p95 / p99 ms | Renderer-context GPU p95 / p99 ms | Complete shutdown ms |
| --- | --- | --- | --- | --- | --- |
| 1 | 7,200 / 7,199 / 7,200 | 3 / 1 / 1,269 | 0.6481 / 0.7294 | 4.8865 / 5.5846 | 1,343.3419 |
| 2 | 7,200 / 7,200 / 7,200 | 496 / 0 / 568 | 0.6683 / 0.7488 | 3.9068 / 4.7371 | 1,352.1745 |
| 3 | 7,200 / 7,200 / 7,200 | 1 / 0 / 45 | 0.6197 / 0.6770 | 5.9668 / 6.2661 | 1,285.1592 |

Every accepted Spout image has an exact completed independent capture: **7,199 / 7,200 / 7,200**
with **zero missing images**. Each renderer GPU cohort has **7,200 valid spans**, no missing
queries. Source/consumer/window age, native preparation, plan handoff, complete telemetry,
software-display joins, API/producer cadence, logical allocation/admission/private memory,
editor independence and actual retirement/shutdown criteria pass. Both interruption intervals
are 60 seconds per sample. Logical texture peak is **144,691,472 bytes** in every sample;
renderer private peaks are **144,470,016 / 143,712,256 / 143,839,232 bytes**. Renderer telemetry
drains **194,747 / 194,737 / 194,727** rows without drops/clock failures. Runtime retirement is
**99.2243 / 94.5621 / 95.2123 ms**, distinct from complete launcher-observed shutdown above.

Only the **deadline_misses** criterion fails in these captures. Window software-display misses
exceed the unchanged **0.1%** target in every editor sample and the second main sample; Spout
stays within it. Source-age p95 remains below **50 ms**. This narrows the demonstrated failing
path to window software presentation timing; it does not prove a scheduler/compositor root cause
or justify changing shader/source handling. Context-local GPU spans are not a cross-context
critical path. That measurement, renderer lifecycle growth, native queue occupancy and complete
environment pinning remain unavailable; M05/M09 retain their separate physical/full-app gates.

The [25 KiB summary](evidence/mapgyver-m03-q01-prescribed-2026-10-31.json) retains per-sample
criteria, distributions, exact failures, input identity and all **48 raw-file hashes**. All hashes
are recomputed against the retained local directory `target/qualification-m03-q01-full-2026-10-31-b55`;
the full **199,119,999-byte** analysis SHA-256 is
`0c9f14121acc41289982dcc699a79bce57f10c946ddad91424f317fe251e492d`.
Raw captures and full analysis remain local, not a large new Git archive. No owned renderer or
fixture PID remains; all three owned ETW sessions return **4201 (absent)** after normal collector
exit. No further native experiment occurs. **M03 stays 7/9, overall stays 4/10 (40%)**; no original
scope, workload or budget changes. Next is a bounded Golden Video window-presentation/pacing
correction justified by this failure, or gate 4's concrete reset check after explicit approval.

**Recovery correction batch 54:** started 2026-10-31 **17:29 UTC**, recorded stop **18:14 UTC**.
Previous turn made progress: `047af44a` preserves the passing held-observation case, the failed
source preflight and its explicit fixture correction. Reopening is justified by that concrete
configuration change: the runtime previously disabled source admission; it now receives the
same bounded input limits as existing input integration tests. This changes whether the prepared
real-removal harness can exercise source ownership. No new probe or repeated passing matrix.
Only M03 gate 4 is active. Rebuild/inspect software prerequisites, then run exactly the corrected
`hardware_source_recovery_fixture_preflight_without_device_removal` case once. Require streaming
and renderer copies for two actual source processes per GPU, independent green output, actual
runtime retirement and normal producer exits with no real removal. Failure permits a concrete
ownership/fixture correction, not an expanding measurement chain. Keep the existing failed result.
The native observation window is at most 20 minutes, also bounded by 18:14; record its actual start
before execution. No GPU reset is authorized. If preflight passes, checkpoint it before requesting
the specific external-reset decision. Gate 4 remains open until its full acceptance is verified.

Batch 54's single native invocation starts **17:30 UTC**, cutoff **17:50 UTC**. Software
prerequisites from the corrected checkpoint are terminal: **161 unit + four public integration
tests / 23 hardware ignores**, strict all-target/all-feature Clippy, both Cargo formatters and
the current exact test build. Test executable SHA-256:
`02d1f2ef453f08ad30615240172af2b03019a7f5a8de7ee5d3cd98bf993d1ca5`;
source fixture remains `8a237afda4e852a8806994533bdae45a056510c4f5815fef76e8bf25ebe48ee2`.
There are no live source-fixture processes before this invocation.

**Batch 54 result:** **1/1 passes in 9.23 s**, normal exit zero. The prepared test iterates
hardware adapters, starts two independent real Spout source processes per adapter, observes both
sources streaming with publications and renderer copies, holds/releases native completion
observations and independently verifies green output pixels. It reports **zero actual removals**,
completes all runtime shutdown/zero-resource assertions in **471 ms**, then receives normal exit
from every owned producer. A post-run process inventory confirms no source fixture remains.
This validates the corrected fixture and real source ownership path; it does not prove device
removal or persistently blocked source/driver drain recovery. **M03 stays 7/9, overall 4/10 (40%)**.
Next concrete decision is approval for one `C:/Windows/system32/DXCap.exe -forcetdr` check with the
already prepared exact guarded removal test. Its GPU-wide reset can interrupt other apps, so
ordinary command authorization is insufficient. If approved, observe the READY marker before
reset, require a real removed-device HRESULT and retain failures/cleanup outcomes. If deferred,
keep gate 4 open with that return trigger and take gate 9 next; no automatic scope reduction.

**Recovery batch 53:** started 2026-10-31 **16:55 UTC**, recorded stop **17:40 UTC**.
M03 gate 4 is the only active gate. Previous turn is progress: pushed `d7f8dd3d` closes all six
M04 gates. Reconcile actual removal/recreation, healthy-path independence, cancellation and
stuck-retirement ownership against current production-runtime tests. Prefer a documented
process-owned D3D11 device-removal mechanism if supported; never reset an adapter/driver, alter
display settings or disturb other apps. Add only the smallest native qualification seam needed
to exercise the actual runtime and inspect real removal/completion evidence. Keep all original
resource, retry and retirement requirements; mocked timeouts cannot substitute for completion.
Software/build/Clippy prerequisites precede native runs; at most two fixed native invocations/
20 minutes, also bounded by 17:40. No Q01/Q02 profiling, parked stash or other phase expansion.

**Batch 53 scope reconciliation:** installed D3D11 bindings do not provide the D3D12
[`ID3D12Device5::RemoveDevice`](https://learn.microsoft.com/en-us/windows/win32/api/d3d12/nf-d3d12-id3d12device5-removedevice)
API. Microsoft's [documented D3D11 test](https://learn.microsoft.com/en-us/windows/uwp/gaming/handling-device-lost-scenarios)
uses `dxcap -forcetdr`, which triggers a GPU-wide timeout/reset. `C:/Windows/system32/DXCap.exe`
is installed. No adapter/reset command is executed without an explicit user decision because it
can interrupt other GPU apps. This is an unavailable native event until approved, not a passed
mock or an automatically waived requirement.

**Fixed safe native matrix:** real production VideoRuntime/D3D11/Spout resources on each hardware
adapter, with a separate healthy runtime. A test-only, per-control seam delays delivery of actual
query completions; it never invents a completion or device-loss HRESULT and is absent from normal
builds (including qualification executables without `cfg(test)`). With two in-flight uses, change
project and close; assert old native resources/bytes remain owned, reliable project acknowledgement
and retirement remain pending, close returns within 50 ms, and healthy output submissions and
independent green pixel observation continue. Release the observation gate, then require actual
query-based retirement, the single deferred project acknowledgement, zero owned resource bytes
and complete shutdown within five seconds. This is completion-observation starvation, not a
physical GPU hang/removal, and cannot by itself close gate 4.

The separately ignored external-removal test is prepared behind
`GOLDEN_VIDEO_EXTERNAL_DEVICE_REMOVAL=1`. It starts owned plans on hardware adapters, withholds
observations, prints a readiness marker and waits for an actual `GetDeviceRemovedReason` error.
It records that real old-device HRESULT before production reset drops the device, requires the
same authored plan on a fresh device/resource generation, checks bounds/output pixels and final
retirement. No reset is triggered by the test itself; absent a genuine event, it fails rather than
claiming removal. Approval for a disruptive reset is a separate final decision after safe checks.

Batch 53 native window starts **17:18 UTC**, cutoff **17:38 UTC**, before the 17:40 batch stop.
Prerequisites are terminal: Golden Video **161 unit + four public integration tests pass / 22
hardware tests ignored**, strict all-target/all-feature Golden Video Clippy, both Cargo formatters,
whitespace and the exact native test build. Candidate
`golden_video-b9cf17ec8c4ecf03.exe` SHA-256:
`ca63b40f47baa3d003ab9587918b093689b512c7ebcd07e412bff1b635f7e220`.
Invocation one is only the held-observation/healthy-output case, with no reset. It passes **1/1
in 7.38 s**, exit zero, on Intel and NVIDIA. Each held owner retains **1,040 bytes** with actual
query serial 2 observed and serial 0 delivered. Close returns in **2 microseconds** on both;
healthy runtimes submit **13/94 frames** while old work remains retained. Releasing observations
retires in **23/10 ms**, total close **310/1,667 ms**, with exactly one deferred project-change
acknowledgement, independent green pixels, stopped workers and zero owned resource bytes.

The external-removal harness now also owns two real independent Spout source processes per
adapter, so eventual removal exercises source workers/bridges/drains, not only output devices.
The second and final native invocation is its **non-disruptive source fixture preflight**, before
requesting a GPU-reset decision. It requires real input streaming/copies, held completion release,
independent green output, complete runtime retirement and normal producer exit; zero actual
removals must be observed. Current software prerequisites pass **161 unit + four public integration
tests / 23 hardware ignores**, strict Clippy and exact test build. Candidate SHA-256:
`1d9fa0199ac37945500393c90f9518a7065bc973a4ec1448098be51700da20a1`;
`native_source_fixture.exe`: `8a237afda4e852a8806994533bdae45a056510c4f5815fef76e8bf25ebe48ee2`.
Run only `native::backend::tests::retirement::removal::hardware_source_recovery_fixture_preflight_without_device_removal`
with `--ignored --exact --nocapture`, within the original 17:38 cutoff. No reset tool is started;
genuine removal remains a future explicitly approved invocation and gate 4 stays open.

**Second/final invocation result:** **0/1, exit 1 in 0.56 s**, rejected at initial install with
`ResourcePressure: native source capacity is retained by existing or retiring subscriptions`.
The new test owner incorrectly used `InputLimits::default()` (inputs disabled), unlike the existing
input integration fixture. The owner now accepts explicit limits and both source suites reuse the
existing bounded four-source/maximum-4x4 storage contract. This is a test fixture correction; no
production admission limit changes. Native retry is intentionally not run in this batch: both
fixed invocations are consumed. The failure stays recorded and the corrected harness remains
unqualified until its next bounded preflight. No GPU-reset approval is requested before that pass.

**Checkpoint validation:** after the fixture correction, **161 unit + four public integration
tests pass / 23 hardware tests ignored**, strict all-target/all-feature Golden Video Clippy and
both required Cargo formatters pass. No owned source fixture process remains after the failed
preflight. The observation seam is test-only; normal production resource/admission behavior is
unchanged. **M03 remains 7/9; overall remains 4/10 (40%)**. Return trigger: the next fixed source
preflight must pass before the genuine-reset decision; actual device removal and stuck-drain
recovery retain their original acceptance. The next turn must not restart the passing
held-observation experiment or expand a diagnostic framework.

| Item | State | Accepted scope / remaining work |
| --- | --- | --- |
| Immutable render/resource plan compiler | Done | [Compiler evidence](mapgyver-m03-render-foundations.md#immutable-render-plan-compiler--2026-10-28): explicit texture/color/alpha, deterministic whole-graph validation/demand compilation, bounded raw work/checked logical bytes, 29 tests including a 10k chain. No runtime/native admission pass is inferred. |
| Bounded worker/null runtime and atomic install | Done | [Runtime evidence](mapgyver-m03-render-foundations.md): Golden IO workers/fences, one preparation/latest desired plan, bounded reliable outcomes and resource admission, frame-boundary install, at most two in-flight uses/two retiring generations, completion-based worker-local destruction. Null resources simulate allocation/use; native proof stays open. |
| D3D11/Spout backend and host integration | Done | [Native input/output evidence](mapgyver-m03-render-foundations.md#bounded-spout-inputs--2026-10-29): opaque GPU resources, explicit LUID/caller-device ownership, four-mode conversion/composition, independent-process Spout inputs and window/Spout output consumers pass. Shared-host regression passes. Full recovery/steady-state budgets retain gates 4/9; physical presentation remains M05. |
| Runtime observations and device recovery | Open | [Startup retry](mapgyver-m03-render-foundations.md#admitted-startup-retries--2026-10-29) and [endpoint recovery](mapgyver-m03-render-foundations.md#independent-native-endpoint-recovery--2026-10-29) verify baseline admission, isolated output retry/state/fenced replacement, source withdrawal and simulated failed-reset/cancellation/project fencing. Genuine native removal/recreation and stuck-drain recovery/retirement bounds remain required. |
| App-owned media/channel backend contracts | Done | [Authoring and live bridge](mapgyver-m03-render-foundations.md#app-channel-publication-bridge--2026-10-29): typed persistent references, backend defaults/operations, generated contracts, demanded-leaf capture and guarded plan publication. History/reload, source changes/resize/reconnect and controlled native runtime/device recreation preserve channel identity. Genuine device-removal recovery stays gate 4; dedicated manager controls/commands/templates stay M04. |
| Shared production-time authority | Done | [Golden Time](../../crates/golden_time/README.md) supplies checked coordinates/rational mappings and authority/discontinuity epochs consumed by the threaded video scheduler; fake-time activation/shared-output and stale-epoch tests pass. Shared P14 plan records this evidence; audio/DMX acceptance remains open. |
| Shared persistent asset/preparation boundary | Done | [App Picture/asset integration](mapgyver-m03-render-foundations.md#app-assets-and-picture-resources--2026-10-29): app-owned stable references/locators, sparse/full codec/history/QuickJS, missing/replaced/relinked/relocated sources, bounded worker decode and retained CPU/GPU resources. Native upload/share/recreation and stale installation rejection pass. Chataigne audio/collect/scheduling P15 acceptance and full native qualification remain open. |
| Native preview registration/geometry contracts | Done | [Preview contract/native consumer](mapgyver-m03-render-foundations.md#native-preview-registration-and-geometry--2026-10-29): Rust-generated DTOs, host-owned metrics, coalesced leased layout, Svelte clipping/visibility/retirement, finite crop/clear pixel oracle and actual shared-host lifecycle pass. Specialized editors/full physical/DPI/input stay M05–M07. |
| Native renderer qualification | Open | Unchanged Q01/Q02 CPU/GPU/cadence/frame-age/admission/memory/handle/shutdown budgets and applicable recovery against the production replacement; historical failures preserved. |

The [nine-item definition](../plan/MapGyver_Implementation_Plan.md#m03--deliver-renderresource-and-channel-foundations)
covers the original complete M03 acceptance. This is a denominator breakdown from 1 to 9, not a
scope reduction or a native pass. Time and asset primitives missing at their shared Golden owners
must be implemented and exercised once; audio-specific clocks/caches are not app project truth.

## Detailed status

Updated: 2026-10-29. Entry point: [MapGyver implementation plan](../plan/MapGyver_Implementation_Plan.md).

**State: M00, revised M01 and M02 complete locally; M03 implementation in progress.**
The user explicitly requested implementation over successive turns on 2026-10-27, then requested
a MapGyver branch. `mapgyver` was created from `137c9203` with the existing dirty checkout intact,
then externally fast-forwarded from `main` to the Chataigne P11 checkpoint `40acd67d` during M00.
The checkout was reconciled at external checkpoint `73548569` (`Checkpoint MapGyver baseline
and partial native feasibility`). NVIDIA/PresentMon checkpoint `8e4615b5` and native-role
checkpoint `203970bf` and live-counter/warmed checkpoint `0e4a6deb` are published; this batch
starts from the latter at `origin/mapgyver`.
M00 delivers the baseline tooling and fixed qualification contract. M01 owns an isolated native
Spout/D3D11 harness with locked Tauri/Svelte underlay/output windows, an independent consumer,
source-worker GPU bridges, bounded admission/retirement and lossless timing/resource evidence.
It has not delivered a production MapGyver shell or Golden protocol change.

The current accuracy diagnostic adds a CPU oracle for the existing procedural-card shader.
Intel fixed 1080p/4K cases compare all 10,368,000 composite pixels through independent processes
and the real source-worker bridge, with no exclusions and maximum component error 1/255 against
a fixed 2/255 tolerance. Producers upload only one 32-byte uniform each, with no CPU pixel
uploads. Matching authored-image controls share the same native checkpoint and captured wrapper. This establishes
fixed-card shader/transfer accuracy. NVIDIA now passes the same four cases on identical native
inputs. Those captures retain their exact earlier binary. The new diagnostic release reports
ephemeral presenter identities only in CLI generation reports; product DTOs/UI/persistence remain
unchanged. Short Intel Q01/Q02 traces match every raw row to editor/output roles using exact
PID, queried DXGI aliases and Present-QPC bounds. Q02 retains 85/20 drops out of 480 Presents
per role despite native accepted-call counts of 480/480. The later 30-second-warmup control
keeps Present flags/geometry unchanged and records no missing source publications during its
sample, but still observes one output drop/480 Presents and five editor readiness gaps.
Its independent live ETW query succeeds after the recorded Present intervals with all three
loss counters zero; final shutdown/analysis loss remains unverified. Report-only window
observations identify primary 240-Hz/125% and secondary 60-Hz/100% nominal configurations.
Role mapping is verified; physical displays, animated/lifecycle, previews and full performance remain open.
The earlier lifecycle build
verifies Intel/NVIDIA output through >60-second busy and blank-document windows, then fresh
preview reconnect. Those captures and startup-invalid markers retain their exact earlier inputs.
Before the lifecycle-fixture change,
Intel/NVIDIA Q01/Q02 each collected three full release samples on identical inputs: 86,395 exact
dependency joins within scoped call/CPU/age targets. Their fingerprints stay historical. Full
asynchronous GPU critical path is still open. See [native evidence](mapgyver-m01-native-feasibility.md).

Earlier failures remain authoritative for their exact fingerprints: stalled cadence, a
207.798-ms combined QueryEnd interval, source-stop ordering and 47.926-MiB positive private
growth against 32 MiB. Stop ordering and timestamp-free event-query admission were repaired;
the old QueryEnd delay and memory-retention cause remain unproven. A later reconstruction
meets relative growth in one cohort with a higher absolute peak, so it does not close memory.

NVIDIA qualification requires verified overlay Off before startup. The approved on/off/on
control isolates handle growth to enabled conditions; no speculative driver teardown is added.
The earlier lifecycle turn restored On at 08:17:51 UTC. Current card-accuracy observations find
an already-Off setting at 09:19:55 and confirm Off at 09:22:32; it stays unchanged. Both adapters'
card captures share identical native source/assets/executable. The wrapper's transport-only
change protects large integer evidence. No overlay, display, driver or security setting changes
are made during this turn.

Primary 125% and secondary 100% native input sequences are recorded, including hidden/minimized
intervals over 60 seconds. [ETW preflight now passes with the administrator token](mapgyver-m01-native-feasibility.md#etw-access-after-administrator-relaunch--2026-10-28);
the agent sandbox still returns access denied. Displayed cadence, physical latency/alignment,
remaining display configurations and complete retirement stay open.
Chataigne's separate [ledger](implementation-status.md) and default root launch remain intact.

## Confirmed product decisions

- Product name: **MapGyver**, planned at `apps/mapgyver`, alongside Chataigne.
- Shared Golden Core and UI; Windows x64 first.
- Each Shared Texture channel receives its own Spout sender; channels run concurrently.
- Both preloading and postloading are distances in neighboring Matrix columns.
- Required media, screens/surfaces and four editors are recorded in the
  [architecture](../architecture/mapgyver.md); research is separated from native proof in
  [native rendering findings](../architecture/mapgyver-native-rendering.md).

## Milestones

| Milestone | State | Evidence / next acceptance |
| --- | --- | --- |
| M00 Reconciliation/workloads | VERIFIED_LOCAL | [Checkout/public-boundary/hardware handoff](mapgyver-m00-baseline.md), [captured inventory](evidence/mapgyver-baseline-2026-10-27.json), and fixed [qualification workloads/budgets](mapgyver-qualification-contract.md) |
| M01 Functional route decision | COMPLETE_LOCAL | [Approved decision](mapgyver-m01-native-feasibility.md#functional-route-decision--2026-10-28): ordinary Tauri/Wry + D3D11 + pinned Spout sys binding permit the thin shell. Full qualification remains with M03/M05/M09. |
| M02 Multi-app seams/shell | COMPLETE_LOCAL | [Shell/isolation and typed host lifecycle](mapgyver-m02-shell.md) close all seven fixed items. The real Windows smoke verifies STA/worker/window teardown; no GPU or physical-output qualification is inferred. |
| M03 Rendering/channels | IN_PROGRESS | [Compiler, runtime, native execution, channels, shared time/assets and preview contracts](mapgyver-m03-render-foundations.md) close 7/9 fixed gates. Production recovery and native qualification remain open. |
| M04 Source media | COMPLETE_LOCAL | All6/6 gates accepted. [Complete new-layout source UI](evidence/mapgyver-m04-workbench-source-acceptance-2026-11-01.json) reaccepts gate5; original source-native, command/script/headless, asset/history and recreation evidence is retained. |
| M05 Screens/Surfaces/editors | IN_PROGRESS | [Authoring, crop/warp, calibration and full new-layout Screen input](#m05-acceptance-items) close gates1–3 and6:4/8. Complete new-layout Source input also passes; native preview, hotplug/minimize and the physical matrix remain open. |
| M06 Composition/editor | IN_PROGRESS | [Authoring, dependencies, native rasters and actual browser lifetime](#m06-acceptance-items): gates1–3/4 accepted. Input/history/dock layout and accepted-drag New/review reload pass; native imagery/extent-based outline/shape selection retain gate4. |
| M07 Matrix/editor | IN_PROGRESS | [Document/module, native content, actual grid input and strict project lifetime](#m07-acceptance-items): gates1–2 and4/4 accepted. Stale Media Manager catalog fixed and strict lifecycle passes; actual native renderer feedback retains gate3. |
| M08 App lifetimes | IN_PROGRESS | [Owned process/model/controller/Matrix retention](#m08-acceptance-items): gates 1–3/4 accepted; lifecycle controller qualification includes actual crash/backoff, suspended cancellation, timeout and descendant cleanup. Editing/status/callbacks/default headless and operator-visible service failures/recovery work; complete app/mode/UI/unsupported-target parity and recovery/cleanup acceptance retain gate 4. |
| M09 App/artifact qualification | NOT_STARTED | Requires implemented product and fixed native workloads |

## Next action

Publish the Composition selection/input/project-lifetime checkpoint before another batch.
M06 gate4 retains its actual native imagery/extent-based outlines/shape-selection prerequisite;
return on supported current-build native control/manual observation. Do not repeat the now-passing
Media/Screen/Source/Composition fixtures unchanged or try alternate native helpers.
Continue only M07 gate3's remaining redesign work: migrate full Matrix grid/Inspector/settings/
structure/source/history and project-lifetime fixtures, preserving actual native preparation/
installation/submission feedback and all complete-app/resource/physical qualification requirements.

**Retained qualification queue:**

Continue from batch98's actual elevated-owned-target/token/Ready/Idle/cleanup result and batch97's
complete column/shared-process/availability-recovery/New UI check. Keep **M08 gate4** parked.
The next requirement needs actual native executable-picker and desktop lifecycle observations.
The last supported activation failed; return on a concrete supported capability change or guided
manual input when available; the user confirms native interaction is unavailable for now. Do not repeat preparation or try new helpers
to extend the blocked automation chain. Singleton retirement and complete app/mode/recovery/cleanup
remain open. Non-elevated denial was not exercised; a running elevated target does not verify the
opposite caller context. No privilege/security/settings change is authorized by this checkpoint.
No phase switch merely because a checkpoint did not close the gate; record an actual missing
prerequisite and return trigger if the active work becomes blocked.
M06 returns for native imagery/extent/shape selection, M07 for native renderer feedback and M05
gate7 for native full-source preview. No injection, new automation route or passing-preparation
rerun substitutes for those observations. All global budgets remain.

M06 and M07 actual-editor observations remain requested/pending against their saved review
projects; no response is inferred and preparation is not repeated. M08 caller/headless/aggregate
target/browser-form proof is complete within its recorded scope; native dialog, singleton
retirement, actual elevation and full-app gaps remain. Passing Matrix native coverage is not rerun.

M03 gates 4 and 9 retain every original requirement and the dashboard's precise return triggers:
unapproved reset, GPU/window scope decisions, private growth without a supported correction,
physical/opaque allocation limits and final prescribed evidence. Do not repeat long memory or
timing controls, restore the parked stash, change driver settings or apply pending proposals.
Complete remaining M05/M06/M07/M08 acceptance and return to all retained M03 requirements before
M09. Only one gate is active; this is an execution-order change, not a waiver or concurrent
expansion. Preserve Chataigne's root default and unfinished P11 work.

Record the slice's owner, observable result, validation and 45-minute stopping time before
coding. Ship and push a coherent product checkpoint before the next implementation batch.
Historical bookkeeping is **42 product deliveries / five qualification batches**; documentation
checkpoints are excluded. The user's completion-first directive suspends the earlier quota.
Acceptance closure now drives work; the investigation limits and every requirement remain intact.

M03 gates 4 and 9 remain open and owned by M03. Preserve every original budget, failure and
required verification. They are production-readiness requirements, not authorization to spend
another open-ended session on profiling before implementing the app. The parked clock/collector
stash is evidence of unfinished work, not the next execution entrypoint. Do not restore it by
default. Physical output/DPI and full-app qualification retain their M05/M09 owners.

**Approved scope correction — continuing acceptance ownership:**

| Milestone | Responsibility |
| --- | --- |
| M01 | Record functional feasibility and a host/backend/binding choice from the existing native evidence: ordinary Tauri/Wry underlay, D3D11, pinned Spout binding, two GPU sources, composition, editor independence and scoped ownership cleanup. Explicitly retain limitations and failed qualification results. |
| M02 | Proceed with the thin app shell and public multi-app/host seams after that functional decision. This does not approve production editors or claim qualified rendering. |
| M03 | Own reusable renderer scheduling, measured GPU/CPU budgets, resource bounds and applicable recovery/retirement fixes before accepting those mechanisms. |
| M05 | Own correct physical output placement, preview alignment, quantitative input/layout latency and the full display/DPI/input matrix before accepting those features. |
| M09 and owning features | Run the complete applicable sustained/recovery qualification against the actual app; preserve the fixed Q01/Q02 budgets and historical failures. Full product/Matrix Q05 remains here and with the features that introduce its behavior. |

The approved M01→M02 dependency is the functional route decision. The four-item M01 gate is
closed in its recorded scope; the original eight-item full-qualification matrix was **not
passed**. Its measured failures and unavailable checks retain their continuing owners. Future
native work belongs to the feature being delivered, with a bounded corrective batch and a
pass/fail checkpoint; a failed checkpoint produces a decision rather than a diagnostic loop.

Internal root-cause attribution, mandatory dual-vendor success, exhaustive SDK-internal copy
accounting, physical residency proof and optical scanout research are not additional gates to
creating a thin shell. Unknown observations remain unknown. The fixed required measurements
and budgets are not waived by this distinction. The plan, architecture, qualification contract
and evidence decision are revised together with this ledger.

Preserve Chataigne's unfinished P11 work and its separate ledger. No production source,
protocol, display configuration or overlay setting was changed by this review.

### Bounded owned-output fixture correction — 2026-10-28

The harness no longer chooses `available_monitors()[1]`. It selects only attached displays
belonging to the requested adapter, prefers a unique exact-size match and otherwise requires
one fitting display. Missing/ambiguous fixtures fail before window/GPU startup. A selected
4K adapter cannot silently use a different adapter's output or span smaller monitors.

Fixed-card output is borderless, shadow-free and nonresizable. Two ordered synchronous
owned-HWND setup calls first relocate into the target DPI context, then set the final physical
extent. After show, exact client origin/extent, monitor identity and containment must match
before the renderer, fixtures or consumer start. There are no placement retries, polling,
display-setting changes, new timing probes or protocol changes.

`cargo test --locked -p mapgyver_native_probe --features desktop` passes **104 tests**
(57 library, 47 binary), including nine new deterministic display-selection regressions;
seven hardware tests remain ignored. Strict desktop/all-target Clippy passes. Native
placement/DPI behavior is **not yet verified**; no capture or native GPU test was run.
At that source checkpoint the scope proposal was still pending and M01 was **1/8** under its
original definition. The later approval above changes ownership, not those test results.
This source fix does not establish a cadence, geometry or resource pass.

### Historical diagnostic backlog — superseded as an execution queue

The following retains the previous checkpoint's rationale and then-current next steps. It is
historical evidence, not the active execution queue; use the approved M02 action above.

Continue **M01**, the [native feasibility gate](../architecture/mapgyver-native-rendering.md#native-feasibility-gate),
on branch `mapgyver`, using the owned [native harness](../../tools/mapgyver/native-probe/README.md).
M02 depends on this gate; do not build production editors/shell around an unqualified underlay.

- The user-confirmed 4K display is connected as `DISPLAY5` on NVIDIA adapter 1, LUID
  `00000000:000170d3`, at `[4480,0,8320,2160]`, with 150% DPI observed in the rejected smoke.
  The Intel paused smoke remains rejected: native output is 3834×2143 and seven warmup CSV
  Present QPC transitions reverse within one swapchain. Fix owned-output placement and
  per-monitor-DPI sizing before another capture; preserve raw order and do not discard the
  failed smoke. Explicitly select and predeclare the new display/adapter route, and observe
  NVIDIA overlay Off before NVIDIA startup. One named adapter is the M01 requirement;
  mandatory dual-vendor success is not an additional gate.
- Pre-lifecycle Intel/NVIDIA Q01/Q02 release cohorts cover both fixed workloads with identical
  native inputs within that cohort. Preserve the earlier cadence/QueryEnd and memory failures; the split stages
  make a recurrence attributable to a smaller wall interval but establish no internal cause.
  Do not repeat unchanged performance cohorts in place of missing evidence.
- The predeclared output-only latest/vblank/latest control uses one new frozen release and
  matching window snapshots. B loses 31 native output submissions despite zero CSV drops;
  A2 latest drifts to 5,372/7,146 accepted attempts against 7,200 nominal slots. Do not adopt
  vblank or treat this as a completed qualifying series. The subsequent weak-identity import
  cache preserves reader ownership, passes full-image/native ownership checks and performs
  zero sampled opens in 30+8/30+120-second Intel controls. Long sampled poll stalls do not recur,
  but the sustained sample still loses 652 native output submissions (221 GPU coalesced plus
  431 readiness gaps) and has 44 CSV output drops. Investigate GPU/context scheduling and
  presenter readiness with a controlled experiment; do not assign historical stalls a cause
  from this uncontrolled comparison. Explicit ten-second collector tails permit both live
  queries to succeed; preserve A2's earlier status-4201 failure and unverified final loss.
- The opt-in stage diagnostic retains all 2,277/8,963 whole-frame joins for its short/sustained
  staged runs. Its largest sampled interval contains 143.0630 ms between post-composition and
  post-Spout markers, with zero observed Presents in that batch's CPU bracket. Isolate Spout
  send/consumer sharing and GPU scheduling before moving responsibility or claiming an SDK
  cause. Output still loses 75/7,200 native submissions and has 29 CSV drops; the return-to-off
  short control also drops 76/480 output Presents despite full native acceptance. Preserve
  separate GPU-pressure and presentation behavior, and do not infer equivalence from short
  off/on/off distributions. One sampled import per source remains visible in the long run.
- Both adapters' JS-busy/blank-document continuity now has native evidence and fresh reconnect
  ownership. Intel finite authored-image accuracy now compares every pixel through independent
  processes and the real source-worker bridge at 1080p/4K. The fixed procedural-card shader also
  matches its CPU reference, including frame digits and marker words, on both adapters. Continue
  lifecycle and preview/physical-output accuracy; fixed-image checks do not close those gates.
  Investigate supported cross-context GPU timing/correlation before treating context marginals,
  serial sums or CPU envelopes as the full GPU critical path. Source selection, budgets and
  native routes must stay truthful.
- Investigate private retention with controlled ownership/thread/report-data comparisons,
  preserving the warmed generation-10 baseline and every live peak. The later relative-growth
  pass has a higher absolute peak and cannot establish a memory repair. Full Q05 remains open.
- ETW provider/event capture, native role correlation and an independent live counter query now
  work outside the sandbox under the administrator account. Preserve the short-control Q02
  drops (85/20 per 480 Presents) and warmed-control output drop (1/480) plus five editor readiness
  gaps. Investigate steady GPU/compositor deadlines without treating 60 accepted calls/sec as
  displayed cadence. Live loss counters are zero at the successful query; final shutdown and
  PresentMon analysis loss remain unverified. The
  sandbox account still returns access denied (5). Use the working host execution path.
  The development account is already a Performance Log Users member; non-elevated access after
  a fresh Windows sign-in remains unverified. See [ETW access evidence](evidence/mapgyver-m01-etw-access-2026-10-28.json).
  Empty-session success, one-frame DXGI queue limits, accepted Present counts and consumer
  frame age do not qualify displayed cadence, scanout or physical input/geometry latency.
  Keep the renderer free of pixel readback.
- The available 100% secondary input sequence is now observed alongside the 125% primary.
  [Secondary evidence](evidence/mapgyver-m01-secondary-native-ui-2026-10-28.json) adds 32 native
  inputs plus activation restore, 65.458 seconds hidden and 79.263 seconds minimized.
  NVIDIA hidden/minimized input cases, OS 150/200%, negative origins, physical-pixel alignment,
  full-card qualification and physical 4K scanout remain open. Do not change monitor settings
  automatically. Use the computer-use skill for authorized native UI work.
- Long interactive captures should use the runner's explicit diagnostic underlay mode. The
  original 608-second native report is terminal-truncated; its surviving fields and native
  gesture observations are retained, with missing rows explicitly unavailable. The separate
  short runner control validates compaction, not reconstruction of the omitted report.
- NVIDIA qualification requires [overlay Off before process startup](mapgyver-qualification-contract.md#nvidia-overlay-prerequisite),
  with native verification and preservation/restoration of the current original setting. Latest
  card measurements started and ended with an already-Off setting; no toggle was performed.
  The on/off/on controls reproduce handle growth only when enabled, with the same loaded DLL;
  preserve those failures and do not add a speculative driver-teardown workaround.
- Both GPUs historically complete 100 controlled device rebuilds; the NVIDIA overlay-off
  [live-scene release capture](evidence/mapgyver-m01-scene-retirement-2026-10-28.json) adds a
  ten-second final live interval within scoped handle/private-growth targets. Complete Q05,
  genuine device removal/reset, physical retirement and complete-process/WebView shutdown
  remain open. Do not treat logical post-fence zero ownership as physical residency proof.

Renderer/source/consumer descriptor ownership, process DXGI observations, explicit harness
copy counts, bounded frame-age logs and per-output admission/cadence counters are available.
SDK-internal copies, physical residency and displayed statistics retain their narrower limits.
Keep desktop-capture sessions inactive during timing runs; prior slowdown/failure evidence
remains retained without an established cause.

Preserve Chataigne's unfinished engine/Spatializer P11 checkpoint (four of six fixed checkpoints)
and its [separate ledger](implementation-status.md). Shared P12 boundary repairs belong to M02;
P14/P15 time and asset contracts retain their shared Golden owners under M03. Root launch
behavior is unchanged. No production host/backend/binding decision is final yet.

## M03 shared reader ownership — 2026-10-30

A controlled static-source regression reproduces `DXGI_ERROR_INVALID_CALL` when a second
prepared target recursively acquires the same renderer device's already-held keyed mutex.
Previously, this caused source fallback/recovery; the old worker also converted every
ownership fault into `0x887A0005`, so that historical report cannot prove physical device loss.
Each input now has one fixed reader lease per bridge slot, reused for the same immutable
allocation and retained through its latest renderer completion serial. Retirement stays
completion-based, with no new texture, copy stage, queue or per-frame reader allocation.
Source recovery publishes the actual fault code before its request flag; actual source-device
removal status remains separate. Renderer send wait stays zero, source wait stays 2 ms per lock.

The [paired controls and exact display evidence](mapgyver-m03-qualification-runner.md#shared-reader-ownership-across-plan-generations--2026-10-30)
keep both sources at generation/pool 1, with no reported fault and no absent source-use
publication in the measured phases. The ten Q02 minimized-phase lineage-invalid captures are
gone. Both samples keep 300/300 exact accepted-Spout captures; Q02 editor accepts 295 calls.
All accepted sample window calls have exact software-display rows, but anchored deadline
failures remain. Q02 minimize captures 55/59 Spout images; disconnect captures 51/60 and
covered source ages reach 62.1145/66.1716 ms. Its instrumented renderer p95 is 15.8522 ms,
compared with 11.7707 ms before; added PresentMon means no isolated timing comparison is claimed.

The packet/archive retain two new native captures, per-phase image/software-display audits,
269 execution sources, the failing native repro and passing validation. Independent audit
checks all 1,199 sample GPU partitions and 1,647 exact accepted-Spout images across the sample/
interruption cohorts, preserving missing images and unknown pre-fix lineage. Validation passes
154 ordinary Rust, 17 Golden native, two app-native and 275 Python tests, strict clippy,
release build and both formatters. The native regression verifies two exact target images,
latest-fence retention and logical-fault recovery without a fabricated removal code.
**M03 stays 7/9; overall stays 30%.** Full GPU/queue, prescribed cadence/interruption/growth,
display deadlines/age and genuine removal/stuck retirement remain open.

## M03 explicit source send ownership wait — 2026-10-30

The detailed send API now takes a finite timeout per ownership lock. Renderer outputs always
pass zero, preserving independent healthy output progress; isolated fixture sources request
2 ms on their own workers. This separates renderer admission from an independent source's
60 Hz publication duty without adding retries, catch-up bursts, queues or textures. Busy still
copies no shared pixels and advances no version. The request does not bound opaque SDK/driver
wall time or close stuck-driver retirement. The existing SDK timeout overload implements it.

Rust owns the policy and generated capture metadata. Artifact schema 8 records the source wait
in initial/final snapshots; analysis rejects missing, excessive or changed policy and retains
the original cadence denominator. Old schema 7 evidence stays unchanged with its original
zero-wait source implementation. The vendor's three binding files and nine patch hashes are
updated; upstream SDK implementation files are unchanged.

The [paired short controls](mapgyver-m03-qualification-runner.md#isolated-source-send-ownership-wait--2026-10-30)
accept 300/300 publications from each source and 300 calls on every output at both sizes.
Each retains 300/300 exact accepted-Spout captures and the original logical memory peak.
Q02 renderer GPU p95/p99 is 11.7707/15.2588 ms, compared with the published 11.4896/13.5165 ms
baseline; improved source admission is not a GPU-tail improvement claim. Q02 minimized-editor
main/Spout counts fall to 56/59, with only 45/59 accepted Spout images captured. Disconnect
captures 56/60 and covered source-B p95 age is 57.0987 ms. Complete-run source admission-busy
counters remain five/seven; no sample or interruption failure is hidden.

Native held-ownership tests exercise both 0 and 2 ms: Busy leaves the image/version unchanged,
and release resumes exact pixels at the next counter. Validation passes 154 ordinary Rust,
17 Golden native, two app-native and 275 Python tests, strict Golden clippy, release build,
generated contracts, both formatters and vendor hash checks. The packet/raw archive preserve
268 execution sources, both controls and validation logs; independent audits reconstruct all
1,200 GPU partitions and exact accepted-Spout image joins including the unchanged baseline.
**M03 stays 7/9; overall stays 30%.** Editor-interruption, exact displayed deadlines/age, full
GPU/queue and applicable growth qualification, genuine removal and stuck retirement remain open.

## M03 rejected output-order experiments — 2026-10-30

Two bounded output-order experiments did not justify a renderer change. Staging every output
conversion before publication regresses short Q02 renderer-context GPU p95 from 11.4896 to
14.3252 ms. A simpler schedule prepared once per generation publishes shared outputs before
window conversion/Present, but regresses Q02 p95 further to 17.4177 ms and accepts only
270/300 main and 251/300 editor calls. Q01 p95 is 4.7699/3.7407/4.8264 ms for baseline/staged/shared.
These sequential short controls do not isolate cause; neither experiment earns retention.

Both implementations and their three schedule tests are removed. Golden Video native sources
match published `b68a5f06`; its shader branch, resource pools, source pacing, fences, copies,
SDK and plan-order output delivery remain intact. The retained app-owned offline audit observes
the rejected shared-first hypothesis on exact CPU frame tokens. It is descriptive, outside
the acceptance criteria, and correctly returns false for the restored plan order. CPU order
does not certify GPU readiness or queues.

The [packet](evidence/mapgyver-m03-shared-first-2026-10-30.json) and
[raw archive](evidence/mapgyver-m03-shared-first-controls-2026-10-30.zip) retain four new controls,
both rejected execution-source sets, validation logs and six independently reconstructed
cohorts including the unchanged published baseline. All 1,800 GPU partitions and 1,800 exact
accepted-Spout image/source-marker joins verify; no missing accepted image is discarded.
The shared-first property covers all 600 final experiment frames with zero unknowns, despite
its cadence/performance regression. Fixed acceptance remains **7/9; overall 30%**.

Experimental validation passes 156 ordinary Rust, 17 Golden native, two app-native and 274
Python tests, strict clippy and release build. After rollback, native source equality retains
the baseline's 153 ordinary/17 native/two app-native evidence; those hardware checks are not
claimed as newly rerun on the restored tree. Final Python, both formatters, whitespace and
archive checks and a rebuilt release executable validate the retained tools/docs checkpoint.
Source admission, complete GPU/
queue and cadence/growth coverage, genuine removal and stuck retirement remain open.

## M03 opaque output branch — 2026-10-30

The production change is one uniform shader branch that skips unnecessary sRGB transfer math
for opaque linear targets, including the existing hardware-encoded window path. SDK, wire
formats, resource pools, scheduling and generated contracts are unchanged. New native tests
check 96 independent finite color/alpha cases and alternate forced-flatten/branch draw controls.
The isolated 4K median is 0.802858 → 0.683555 ms, a 14.86% reduction, without a full-path claim.

The [paired native controls](mapgyver-m03-qualification-runner.md#opaque-output-shader-branch--2026-10-30)
retain fresh before-runs, a rejected combined hardware-Spout experiment and final branch-only
runs. Q02 renderer GPU p95/p99 changes 14.8372/16.6638 → 11.4896/13.5165 ms; Q01 p95 worsens
3.8685 → 4.7699 ms. All six runs retain 300/300 exact accepted-Spout captures and stable input
fingerprints. Final Q02 main/Spout accepts 300 calls each, editor 299; producers accept 290/296,
so cadence stays open. Fresh before/final logical peaks remain 144,691,264/468,172,864 bytes
for Q01/Q02. No memory reduction or physical-residency claim is made.

The unsupported-benefit SDK/storage change is removed completely; its 266 execution sources,
successful finite/native tests and regressing controls remain in the 5,824,018-byte evidence
archive. Independent reconstruction verifies all 1,800 renderer GPU partitions and 1,800 exact
accepted-Spout image joins. Final validation passes 153 ordinary Rust, 17 Golden native,
two app-native and 267 Python tests, strict Golden clippy, release build and both formatters.
Full GPU/queue, prescribed display/cadence/growth and genuine removal/stuck retirement remain
open. **M03 stays 7/9; overall stays 30%.**

## M03 DMA decoder boundary — 2026-10-30

The app-owned [offline exporter](../../tools/mapgyver/trace-packets/README.md) uses Microsoft's
locked TraceProcessor API. It filters the four manifest PIDs, preserves partial/missing values
as null, records decoder/input hashes and refuses to overwrite evidence. Its analyzer retains
every exported packet, reports missing fields and checks loss, clocks, identity and ordering.
No runtime, workload, shader, queue depth, DTO or graphics-setting change is made.

The [native experiment and limitations](mapgyver-m03-qualification-runner.md#dma-packet-decoder-boundary--2026-10-30)
retain the two unsuccessful capture controls and the completed GPU-profile capture. The latter
has zero reported final ETW loss but 16,272/16,272 owned packet timings partial, so queue and
critical-path acceptance remain unavailable. The 1.10 GiB system-wide ETL stays local; only
filtered observations and owned native artifacts are published. The profile is not added to
routine qualification. Root/Core formatting, a strict locked C# build and all 267 Python tests
pass, including eight new packet-coverage tests. **M03 stays 7/9; overall stays 30%.**

## M03 direct input bridge receive — 2026-10-29

The source worker now passes a free write-locked bridge slot directly to the pinned SDK's
receive-into API. It removes the private capture texture and capture-to-bridge copy, retaining
three slots, source/renderer completion authorities, static-image freshness, withdrawal and
resize/reconnect fences. `InputLimits::source_storage_bytes` shares one checked layout with
backend validation, source-pool admission and app fixtures: three bridge images plus one
sender alias. Source counts, pacing, marker identities and workload ceilings are unchanged.

The [packet](evidence/mapgyver-m03-direct-bridge-2026-10-29.json) and
[raw controls](evidence/mapgyver-m03-direct-bridge-controls-2026-10-29.zip) preserve new release
Q01/Q02 controls and the prior immutable baseline reference. Each uses five-second warmup/
sample and one-second interruption intervals, on the existing Intel route. Q02 main/Spout/
editor acceptance changes 300/299/271 → 300/300/296, with 300/300 exact Spout captures and no
invalid comparisons. Renderer GPU p95/p99 changes 16.9713/18.3391 → 15.5987/16.0877 ms; median
changes 13.2066 → 13.4602 ms. Q01 retains 300/300 outputs/captures and GPU p95 3.2120 ms.
Producer admission still fails; Q01 source A acceptance worsens 299 → 288/300.

Q02 baseline reservation falls 291,225,664 → 254,361,664 bytes (35.15625 MiB), while the recorded
total peak rises 505,036,864 → 575,078,464 bytes. The latter equals the new baseline plus three
106,905,600-byte generation charges; the prior peak equals its baseline plus two. This arithmetic
does not identify the unsampled transient roles or physical residency. Both peaks remain below
the unchanged 1 GiB bound, and no growth/retirement gate is closed.

Final checks: 153 ordinary Rust tests, 15 Golden native tests, two app-native channel/Picture
tests, 92 production Python tests, strict clippy, release runner and both Rust formatters pass.
Development checks caught and fixed an old admission multiplier and a missing app-fixture import.
No displayed age, critical-path, queue, genuine removal or full-series pass is inferred.
**M03 stays 7/9 (78%); overall stays 3/10 (30%)**.

## M03 direct window backbuffer rendering — 2026-10-29

Golden Video now borrows a prepared render target from each admitted window endpoint and
draws directly into its SRGB backbuffer view. The shader emits opaque linear RGB over black;
the view encodes once. Window conversion textures and their copy disappear, while fixed
two-backbuffer ownership, readiness permits and completion-fenced endpoint retirement remain.
Spout's UNORM conversion/storage, color/alpha handling and keyed sender path are unchanged.
The latency handle is owned before fallible view creation, and foreign contexts are rejected
before drawing. Tests read all 16 backbuffer pixels across six presentations against scalar
transfer oracles, and verify actual preparation allocates only the render-pass texture.

The [packet](evidence/mapgyver-m03-direct-window-2026-10-29.json) and
[raw controls](evidence/mapgyver-m03-direct-window-controls-2026-10-29.zip) preserve new Q01/Q02
release controls and reference the immutable prior stage controls. Scene, adapter, queue,
polling and budgets stay fixed. Each uses a five-second warmup/sample and one-second
interruption intervals. Initial generation textures fall 7 → 5. Q02 logical reserved peak
falls 578,764,864 → 505,036,864 bytes (70.3125 MiB); this is not a physical-residency claim.

Q02 main/Spout/editor acceptance changes 276/294/165 → 300/299/271 of 300 opportunities.
Exact accepted Spout captures change 272/294 → 299/299, with no invalid comparisons. Whole
renderer-context GPU p95 changes 21.7279 → 16.9713 ms; output-interval p95 changes 15.2477 →
13.1157 ms while its median stays 9.5033 → 9.5441 ms. These sequential short controls do not
isolate causality or establish a full GPU critical path. The Q02 CPU-start-selected GPU
cohort has 299 complete spans because one accepted output belongs to a frame starting before
the sample boundary; all 300 anchored deadline opportunities remain. Updated Q01 retains
300/300 acceptance/capture and GPU p95 3.8944 ms. No displayed-frame pass is inferred.

Validation: 152 regular Rust tests, all 15 native tests, 92 production Python tests, strict
clippy, final release runner build and both Rust formatters pass on the final source.
Genuine removal/stuck retirement, GPU/queue, cadence/admission, growth and full series remain
open. **M03 stays 7/9 (78%); overall stays 3/10 (30%)**.

## M03 renderer GPU stage partitions — 2026-10-29

Golden Video's optional diagnostics now prepare two intermediate timestamp queries per
renderer slot, inside its existing disjoint scope. Input-copy and render-pass boundaries
partition the exact completed batch; output conversion/delivery occupies the remaining
interval. There are still two slots and no additional frame allocation, event, flush,
admission dependency or wait. Source/fixture contexts allocate no intermediate queries.
Incomplete/out-of-order/duplicated marks cannot certify a partition, and slot reuse resets
marker state. Rust artifact schema 7 and its generated metadata advance together.

The [packet](evidence/mapgyver-m03-gpu-stages-2026-10-29.json) and
[raw control archive](evidence/mapgyver-m03-gpu-stages-controls-2026-10-29.zip) preserve new
release Q01/Q02 runs on the existing Intel route, each with five-second warmup/sample and
one-second interruption controls. All 600 sample spans have exact ordered stage joins.
Q02 input/render/output p95 are 7.2783/7.4998/15.2477 ms; these percentiles are not summed.
Outputs are the largest interval in 271/300 Q02 frames, median 9.5033 ms. The next performance
change should target output conversion/delivery. These elapsed spans include GPU scheduling,
dependency waits and command-feed gaps; they cannot attribute the interval solely to shaders.

Q01 accepts all 300 opportunities per output and independently captures 300/300 exact Spout
images. Q02 accepts 276/294/165 main/Spout/editor opportunities, with 272/294 exact captures
and no invalid comparisons. Missing captures and qualification failures remain open. No
PresentMon collector or display/driver/overlay change occurs; earlier display/deadline
evidence stays immutable. Whole renderer-context GPU p95 is 3.9429/21.7279 ms for Q01/Q02.

Validation: 152 ordinary Rust library/public tests, 13 native tests, a repeated two-test
timing check after the clippy style repair, 92 production Python tests, strict clippy, the
final release runner build and both Rust formatters. Independent raw reconstruction verifies
all 600 stage partitions and unchanged capture fingerprints. This measures an existing
failure; no resource, cadence, frame-age or recovery budget is relaxed. **M03 stays 7/9
(78%); overall stays 3/10 (30%)**.

## M03 hardware sRGB source decoding — 2026-10-29

Golden Video now prepares a decoded SRGB view for source-local straight/opaque sRGB storage,
while keeping raw views, shared input/output UNORM formats, logical byte charges and all
queue/admission limits unchanged. Encoded premultiplied sources still unpremultiply before
shader decoding. Hardware pixel oracles cover RGBA/BGRA, transparent colored texels, nearest/
linear filtering, straight/opaque/premultiplied alpha, and linear sources; the native input,
image recreation, region, recovery, marker-lineage and timing regressions pass.

The [evidence packet](evidence/mapgyver-m03-srgb-sampling-2026-10-29.json) and
[raw controls](evidence/mapgyver-m03-srgb-sampling-controls-2026-10-29.zip) retain one unchanged
Q02 baseline and updated Q01/Q02 controls on Intel LUID `00000000:00016c2a`. Each uses a
five-second warmup, five-second sample and one-second interruption controls. An isolated
same-device alternating sampler experiment reports 12 measured pairs after four warmup pairs,
four 4K draws per batch: median cost 2.0702 → 1.3827 ms per draw (33% reduction).

Q02 main acceptance changes 249/300 → 292/300, editor 156/300 → 174/300 and Spout 294/300 →
295/300; exact independent captures change 267/294 → 289/295 with no invalid comparisons.
These short sequential controls cannot attribute all end-to-end differences to this change:
renderer-context GPU p95 is 19.9860 → 20.3868 ms, so no overall GPU latency improvement is
claimed. Updated Q01 accepts and captures 300/300 on each applicable output/cohort.
PresentMon was not enabled; the previous window display/deadline evidence is preserved.

Validation: 148 regular library tests plus four public integration tests, 11 native regression
tests plus the isolated native cost control, strict Golden Video clippy, release runner build
and both Rust formatters. Native captures have stable before/after execution fingerprints;
the final source adds only the cost-test code after capture. No workload, budget or gate
denominator changes. **M03 stays 7/9 (78%); overall stays 3/10 (30%)**.

## M03 anchored delivery deadline bounds — 2026-10-29

The [deadline checkpoint](mapgyver-m03-qualification-runner.md#anchored-delivery-deadline-bounds--2026-10-29),
[analysis packet](evidence/mapgyver-m03-delivery-deadlines-2026-10-29.json) and
[derived archive](evidence/mapgyver-m03-delivery-deadlines-analysis-2026-10-29.zip) reanalyze
the existing [schema-6 raw archive](evidence/mapgyver-m03-window-present-controls-2026-10-29.zip)
without changing or rerunning native code. Independently reconstructed nested-ceil scheduler
boundaries, exact API/display joins and canonical source-word/capture joins reproduce every
classification. Original raw files and analyses remain unchanged; the new analyzer version is
hashed separately. All non-analysis capture hashes and the native binary still match.

Each output retains **180 anchored opportunities**. Q01 main/Spout/editor each have **180 on
time, zero proven misses and zero unknowns**. Q02 main has 137 late software displays plus 43
nonaccepted frames; editor has 91 late displays plus 89 nonaccepted frames. Both therefore
miss all 180 opportunities. Q02 Spout has **three proven on time, one nonaccepted/proven miss,
176 unknowns**: 158 late completion upper bounds and eighteen missing exact captures. Its one
known miss already exceeds the unchanged 0.1% budget; unknowns are not relabeled as GPU misses.

Window observations use reported software display time. An exact completed Spout capture can
prove GPU readiness by its CPU observation time; a late poll alone cannot prove late readiness.
Late publication API start and known unsubmitted/nonaccepted opportunities supply separate
failure bounds. Missing slots/witnesses remain in the denominator, and even a late CPU API
return cannot erase an earlier valid delivery witness. Physical scanout, queue occupancy and
GPU critical path remain separate evidence. The short controls do not satisfy the prescribed
series. The five-member derived archive is 190,528 bytes, SHA-256
`930548d805d6ae35e92d255ccb079095af5a3b1cbac83220da44af081e8b9824`.

Validation passes **85 production Python tests**, including exact 1/1000 inclusivity, pre-window
API/deadline cohorts, late polling, CPU preemption, missing/foreign/duplicate/stale witnesses
and partial telemetry; both required Rust formatters run. Rust/native implementation is
unchanged, so the earlier 182 regular/twelve native test baseline is retained without claiming
a new native run. Gates 4/9 remain open. **M03 stays 7/9 (78%); overall stays 3/10 (30%)**.

## M03 window Present/display correlation — 2026-10-29

The [window observation checkpoint](mapgyver-m03-qualification-runner.md#exact-window-presents-and-software-display-age--2026-10-29),
[schema-6 packet](evidence/mapgyver-m03-window-present-2026-10-29.json) and
[raw archive](evidence/mapgyver-m03-window-present-controls-2026-10-29.zip) retain exact endpoint
incarnations/three DXGI interface aliases and native Present API intervals. The optional pinned
PresentMon collector targets only the new renderer PID and an owned UUID session. The retained
probe supplies its public read-only session query, without running a legacy renderer. No
device/driver/display/overlay, SDK, scene, polling, queue-depth or budget setting changed.

All **1,194 Q01 and 875 Q02 CSV rows** join exactly to native calls, with no unmatched rows.
The sample main/editor cohorts are 180/180 and 180/180 (Q01), 137/137 and 91/91 (Q02), with no
missing/dropped/display/source/plan observations. Independent reconstruction reproduces every
raw join and source/publication/display-age distribution. Q01 main source-age p95 A/B is
32.185/28.376 ms; editor is 32.214/28.998 ms. Q02 main is 45.796/55.334 ms; editor is
52.630/63.386 ms, exceeding the unchanged 50 ms target. These are software display events,
not window pixel/physical scanout or optical observations. Pending-display backlog is neither
native buffer occupancy nor a GPU critical path, and its incomplete pre-window coverage stays open.

Q01 has 180/180/180 main/Spout/editor acceptances and 180 exact Spout captures. Q02 accepts
137/179/91, with 161 exact Spout images and 18 missing. Overall Q02 age remains unavailable
because its Spout cohort is incomplete; the unfavorable observed window distributions remain
explicit. All 262 execution hashes per capture and both external tool hashes match, telemetry
is lossless, live ETW counters cover the native calls, and no collector loss warning is reported.
Final collector-shutdown loss statistics remain unavailable. Retirement is actual/unforced,
with zero final resource bytes/in-flight work, in 0.095/0.124 s.

The initial trace finalized native work but failed decoding redirected UTF-16 output. Its raw
files and failure are preserved with unavailable input/live-query evidence. The fixed decoder
retains original bytes and explicit encoding, and the runner persists admission fingerprints/
native completion even if ancillary trace finalization fails. One fresh bounded pair verifies
the correction; no earlier run is silently promoted or overwritten.

Validation passes **182 regular Rust tests, twelve native GPU tests and 73 production Python
tests**, strict default/native all-target Clippy, generated fixture contracts, locked release
x64 build and both required formatters. Prescribed repetitions/durations, GPU critical path,
native buffer/GPU queue/deadline measurements, growth/handles and genuine removal/stuck
retirement remain open. **M03 stays 7/9 (78%); overall stays 3/10 (30%)**.

## M03 CPU plan lifecycle and first-use age — 2026-10-29

The [runtime observation checkpoint](mapgyver-m03-qualification-runner.md#cpu-plan-publication-receipt-and-first-use--2026-10-29),
[schema-5 packet](evidence/mapgyver-m03-plan-age-2026-10-29.json) and
[raw archive](evidence/mapgyver-m03-plan-age-controls-2026-10-29.zip) record admitted publication,
first worker receipt, atomic installation and terminal supersession/rejection/cancellation.
Golden's bounded recorder is allocated before worker startup. Receipt is recorded once across
preparation retries; reconstruction records a new resource/device installation with the same
authored identity. CPU metadata alone crosses these boundaries. GPU queue, physical presentation
and critical-path measurements remain separate. Schemas 1–4 retain their original analyses.

Each bounded Q01/Q02 control has nine published plans, eight first receipts, six installed/used
generations and three superseded inputs, including one unread input. All 389/385 native frame
rows and every accepted sample output join their exact lifecycle/resource identity. Independent
raw reconstruction reproduces every reported handoff/first-use/per-output age. Median
publication-to-receipt is 0.0161/0.0157 ms; the startup observations reach 76.504/80.916 ms.
Publication-to-first-use maxima are 82.708/97.093 ms across six observations, including startup.
These finite distributions establish measurement coverage, not a new latency budget or a
steady-state edit-latency pass. Retained plan age grows naturally while the same plan is used.

Q01 accepts 180/180/180 main/Spout/editor calls and captures all 180 exactly. Q02 accepts
158/174/108; 153 Spout images match exactly and 21 remain uncaptured, with no observed pixel
disagreement. Its observed source-A age p95 is 56.572 ms. Minimized/disconnected Q02 phases
still have incomplete capture/source joins and cadence failures. No device/driver/display/
overlay, SDK, scene, polling, queue-depth or budget change is included. All 259 execution
fingerprints per capture match; telemetry is lossless; cleanup is actual/unforced with zero
final bytes/in-flight work, retiring in 0.094/0.114 s.

Validation passes **182 regular Rust tests, twelve native GPU tests, 60 production Python
tests**, strict default/native all-target Clippy, generated fixture contracts, release x64
build and both required formatters. Existing complete GPU/output qualification, growth/handle
series and genuine device-removal/stuck retirement remain open. **M03 stays 7/9 (78%); overall
stays 3/10 (30%)**. No denominator, budget or historical failure is changed.

## M03 GPU keyed Spout handoff — 2026-10-29

The [ownership checkpoint](mapgyver-m03-qualification-runner.md#gpu-keyed-spout-image-handoff--2026-10-29),
[schema-4 packet](evidence/mapgyver-m03-keyed-handoff-2026-10-29.json) and
[raw archive](evidence/mapgyver-m03-keyed-handoff-controls-2026-10-29.zip) retain the isolated
failure, SDK fix and one bounded release Q01/Q02 control pair. GPU taps show correct renderer
input/conversion pixels and an incorrect outgoing image. Owned Spout senders now request GPU
keyed shared textures; detailed sends use zero-timeout CPU metadata then GPU ownership. Busy
copies no shared image and advances no counter. Unchanged hints need only CPU metadata access.
No device, driver, monitor or overlay setting changed. Scenes and budgets are unchanged.

The native regression checks direct SDK, source bridge, full runtime with/without GPU taps and
held-GPU Busy/resume: **518 available-image comparisons, zero mismatches**. One tapped startup
capture has no available source and is explicitly excluded. Test taps/readback are absent from
production builds. Legacy named/unversioned input tests continue to pass. SDK baseline and all
nine patched file hashes are verified; no fallback to CPU-only output ownership is added.

Q01 has **180/180 exact sample captures**, plus 60/60 in each short editor interruption. Q02
has **148/176 exact sample captures**, no observed pixel mismatch and 28 uncaptured images.
Its minimized/disconnected phases have 46/58 and 36/61 exact captures; the latter also has 16
missing-source captures. Q02 main/Spout/editor API acceptance is 138/176/100 of 180; source-age
p95 A/B is 58.565/48.107 ms on the observed sample subset. Those failures retain gate 9.
Q01 source admission also rejects four attempts per source; output coherence is not a producer
cadence pass. All 255 execution fingerprints match per capture, telemetry is lossless, and
retirement is actual/unforced with zero final resource bytes/in-flight work (0.095/0.120 s).

Validation passes **179 regular Rust tests, twelve explicit native GPU tests, 49 Python tests**,
strict default/native all-target Clippy, SDK all-feature/all-target and legacy desktop probe
checks, release x64 build and both required formatters. Independent raw reconstruction agrees
for every sample/interruption cohort. Full plan/queue/presentation/critical-path measurements,
prescribed series, growth/handles and genuine removal/stuck retirement remain open. **M03 stays
7/9 (78%); overall stays 3/10 (30%)**. The denominator and historical failures are unchanged.

## M03 copied-image lineage and source-retention age — 2026-10-29

The [checkpoint](mapgyver-m03-qualification-runner.md#exact-spout-output-cohorts-and-source-retention-age--2026-10-29),
[schema-4 packet](evidence/mapgyver-m03-output-cohort-2026-10-29.json) and
[raw archive](evidence/mapgyver-m03-output-cohort-controls-2026-10-29.zip) retain diagnostic
SDK copy versions through source publication, renderer delivery and independent capture.
Producer GPU words independently check each claimed renderer source image. SDK share-handle
identity uses its documented 32-bit field, accepting only zero/sign-extended representations;
actual native access handles are unchanged. Unknown foreign sources claim no exact version.

One short Intel release Q01/Q02 pair exposes unresolved dynamic lineage. Q01 captures all 180
sample-accepted Spout versions, but 141 disagree with the exact renderer source-use labels;
39 match both cards. Q02 captures 144 of 180: 138 disagree, six match and 36 are uncaptured.
All observed sample words still match legitimate producer records, demonstrating why the old
producer-to-capture age checks cannot establish renderer-output lineage. Independent raw
recomputation agrees for the sample and both interruptions. In one Q01 example, renderer batch
127 labels producer frames 231/223 while its matched capture contains 229/222. Earlier renderer
predictions often match; input-copy versus output-copy origin remains undetermined.

Source-publication-to-output-API clocks join every accepted sample call on all three outputs.
Spout p95 A/B retention is 18.435/7.599 ms (Q01) and 22.881/13.928 ms (Q02). These CPU metadata
ages do not prove delivery of the labeled pixels or establish GPU queue time. Sample main/
Spout/editor acceptance is 180/180/180 and 147/180/135. Minimized exact matches/accepted Spout
images are 60/60 and 3/55; disconnected matches are 8/53 and 2/60. Both overall interruption
gates remain open, including their required 60-second durations. Q02 cadence/producer/age
failures remain. Age distributions of the 39/six exact sample matches are incomplete subsets.

All 248 execution fingerprints match before/after capture and current source; all roles have
lossless telemetry and valid sample GPU rows. Both runtimes stop with zero resource bytes and
in-flight work; all owned processes exit unforced. Actual retirement takes 0.096/0.109 s.
Validation passes 179 regular Rust tests, eleven explicit native GPU regressions and 49
production Python tests, strict default/native all-target Clippy, locked release x64 build and
both required Rust formatters. The SDK fork, scenes, budgets, queue depth, polling and display/
driver/overlay settings are unchanged. M03 remains **7/9**, overall **3/10 (30%)**. The next
implementation isolates the pixel/version copy boundary before additional qualification runs.

## M03 resource reuse and preparation attribution — 2026-10-29

The [checkpoint](mapgyver-m03-qualification-runner.md#native-resource-reuse-and-preparation-attribution--2026-10-29),
[schema-3 packet](evidence/mapgyver-m03-resource-reuse-2026-10-29.json) and
[raw archive](evidence/mapgyver-m03-resource-reuse-controls-2026-10-29.zip) retain device-local
weak texture/view reuse and exact resource-generation preparation/first-use joins. Mutable
storage keeps immediate-context ordering and fresh per-generation source hints; immutable
images pin exact payload leases, deduplicating logical inputs without confusing equal asset
IDs or recycled addresses. Expired weak entries retain no allocation or asset. Per-generation
admission remains conservative. Old schema-1/2 captures retain their original analyses.

Both short Intel release controls create seven textures, then reuse all seven in each of seven
later preparations. Later preparation takes at most 0.0109/0.0106 ms; initial allocation takes
6.587/40.588 ms. First-submit wait reaches 12.818/22.914 ms. Independent raw recomputation joins
all 388/385 renderer submissions and verifies all 247 execution fingerprints. This measures
native preparation/first use, with full publication/receipt/output-queue age still unavailable.

Q01 sample outputs accept 180/180/180; Q02 accepts 155/180/124 (main/Spout/editor). Q01 short
interruptions meet observed cadence/age checks; Q02 minimized main/Spout accepts 56/59 of 60
and source-age p95 reaches 193.446 ms despite no new allocation. Sample captured source-age
p95 A/B is 33.612/52.810 and 73.588/84.199 ms. Q02 producer shortfalls remain. These observations
establish reuse, not a causal timing improvement or prescribed qualification. All roles retain
lossless telemetry, all sample GPU rows/markers are valid, and cleanup is unforced with zero
final resource bytes/in-flight work; actual retirement takes 0.099/0.115 s.

Validation passes 177 regular Rust tests, eleven explicit native GPU regressions and 36
production Python tests, strict default/native all-target Clippy, locked release x64 build and
both required Rust formatters. New native checks cover outstanding-generation reuse, descriptor/
payload changes, pixels, deduplication and actual asset release. Source tests now wait for real
read leases and isolate uncontended preflight before retaining actual Busy coverage. No display,
driver, overlay, polling, queue, scene or budget changed. M03 remains **7/9**, overall **3/10
(30%)**; gates 4 and 9 retain their complete recovery/qualification scope.

## M03 source copy and freshness — 2026-10-29

The [source checkpoint](mapgyver-m03-qualification-runner.md#source-copy-and-freshness-boundary--2026-10-29),
[packet](evidence/mapgyver-m03-source-freshness-2026-10-29.json) and
[raw archive](evidence/mapgyver-m03-source-freshness-controls-2026-10-29.zip) retain exact copied,
unchanged, busy and handshake outcomes at the public pinned SDK boundary. Per-instance counters
make no registry writes. Controlled peers explicitly opt into versioning; ordinary external
sources remain unversioned, including when an old positive counter exists. CPU preflight uses
the actual sender lock before skipping GPU work. Busy preserves the last valid publication;
session replacement and lost publication re-arm copied history after actual completion.
The [vendor provenance](../../vendor/spout2-sys-0.1.1/GOLDEN_PATCH.md) retains all source hashes
and licenses; source/header edits now invalidate the static archive and capture fingerprints.

The native regression verifies unchanged static-source polls without GPU batches, frozen-counter
foreign copies, actual named-mutex Busy after a successful copy, demand 0→1/missing-mailbox
recovery and same-name resize/reconnect. Counted output names have a shared 239-byte admission
limit; unversioned inputs retain 255 bytes. Keyed synchronization is compiled and source-reviewed,
without new hardware coverage.

One short Intel release Q01/Q02 pair completes with all three outputs accepting 180/180 sample
submissions and no missing software slots. Source GPU batches are 180/180 and 169/173, matching
admitted producer frames, versus the preceding controls' 1,156/1,231 and 306/482. Consumer batches
are 180/157. Renderer-context GPU p95/p99 is 3.809/4.378 and 9.471/13.112 ms; captured source-age
p95 A/B is 22.988/9.803 and 66.250/67.934 ms. These are unpinned observations, not a causal timing
claim or a complete output-frame age cohort. Full captures preserve all distributions and failures.

Interruption acceptance remains insufficient: minimized main/Spout counts are 45/46 and 28/44
of 60 expected opportunities, with observed age reaching 279.459/308.800 ms at p95. Q02's
disconnected main/Spout counts are 50/60. Both independence flags remain false; the one-second
interruptions also remain shorter than the required 60 seconds. Q02 producer shortfalls and GPU
outliers remain. All four roles have complete, lossless traces; all 244 fingerprints match;
actual retirement is 0.100/0.117 s with clean, unforced owned cleanup and zero final resource bytes.

Validation passes 177 regular Rust tests, ten explicit native GPU regressions, 32 production
Python tests, strict default/native all-target Clippy, desktop probe all-target compilation,
all-feature SDK compilation, the locked release x64 build and both required Rust formatters.
The fixed scenes, budgets, polling, queue depth and display/driver/overlay settings are unchanged.
M03 remains **7/9**, overall **3/10 (30%)**; genuine device removal/stuck retirement (gate 4)
and complete renderer qualification (gate 9) remain required.

## M03 output admission before conversion — 2026-10-29

The [output-admission checkpoint](mapgyver-m03-qualification-runner.md#output-admission-before-conversion--2026-10-29)
moves zero-timeout window readiness and availability ahead of per-output conversion. An exclusive
admitted-output borrow spans conversion through delivery; busy/hidden/recovering endpoints skip
that work while healthy outputs continue. Late Present Busy retains its readiness permit and
completion tracking covers the preceding backbuffer copy. Buffering, maximum latency, source
polling and all budgets are unchanged. Schema-2 rejected rows have no conversion/API interval;
successful preflight is retained in inclusive frame wall time without separate attribution.

Independent review found no material issue. The owned kernel-event test exercises real signal
consumption, hidden/resume behavior and late-Busy retention; it does not substitute for native
GPU trace evidence. Source copied/freshness status remains the next checkpoint at the public
binding, without registry changes or freezing unknown foreign senders on an untrusted counter.
No SDK/source-publication change is included. M03 stays **7/9**, overall **3/10 (30%)**.

The [packet](evidence/mapgyver-m03-output-admission-2026-10-29.json) and
[raw archive](evidence/mapgyver-m03-output-admission-controls-2026-10-29.zip) retain one short
Intel release Q01/Q02 pair. Every busy/unavailable row skips conversion/API: 68/223 rows across
the respective whole captures. Minimized editor rows skip all 60/47 conversions. Independent
raw recomputation verifies ordered brackets for all 1,093/878 accepted rows. Both sample main
and Spout outputs accept 180/180 submissions; editor acceptance is 180/119. Telemetry loss is
zero in all four roles, all 180 sample GPU spans are valid, fingerprints are stable, and cleanup
is actual, unforced and error-free. No late API Busy occurred in these native controls.

Renderer-context GPU p95/p99 is 7.533/10.285 ms (Q01), 18.470/21.025 ms (Q02); captured source-age
p95 A/B is 35.679/34.025 ms and 216.710/146.563 ms respectively. Capture age still lacks the full
output cohort. Independence does not pass: minimized main/Spout acceptance is 57/60 and 27/47
of 60 expected opportunities; Q02 disconnected acceptance is 30/56 of 60. The one-second
interruptions are shorter than the prescribed 60 seconds. Main output progress is functional
evidence, not a cadence pass. These unpinned diagnostics establish avoided work without a causal
performance claim. No device-removal, retirement-bound or qualification gate closes.

Validation passes 172 regular Rust tests, nine explicit native GPU regressions, 31 production
analyzer/deadline tests, strict default/native all-target Clippy, the locked release x64 build
and both required Rust formatters. Native finite pixel and app source/history/recovery checks
remain green. No display, driver, overlay, queue-depth or source-polling setting changed.

## M03 measurement integrity and output conversion — 2026-10-29

The [measurement checkpoint](mapgyver-m03-qualification-runner.md#measurement-integrity-and-equal-extent-conversion--2026-10-29)
and [schema-2 evidence](evidence/mapgyver-m03-measurement-integrity-2026-10-29.json) retain two new
short release controls. Bounded concurrent recording removes the former metadata-mutex loss;
all four processes have exact written/drained/raw counts and no full/clock losses. Exact QPC
mapping, per-opportunity admission and final coverage fences establish all 180 software slots
and their output joins in each sample. Replaced clocks become explicitly unmapped. Disabled
diagnostics retain the existing clock and create no cadence UUIDs. API deadlines remain
separate from physical output cadence; unverified criteria never become passes.

Equal-extent output conversion now uses the existing single-load path. Native finite pixel
oracles preserve color/alpha and scaled filtering. Q02 observes 172/180 native-window and
180/180 Spout acceptances, compared with 75/180 and 139/180 earlier. Q01 records 177/180 and
180/180. Renderer GPU p95 is 16.906 / 19.640 ms, still above the diagnostic target; both complete
180 valid context-local spans and 360 source joins. Q02's sparse capture observations have
high age; repeated/source captures do not prove every output frame's age. No sustained or
causal performance pass is claimed. Both controls exit cleanly with actual fenced retirement.

Validation passes 204 regular Rust tests, nine explicit native GPU regressions, 198 Python
tests, strict default/native Clippy, Chataigne all-target compilation, release x64 build and
required root/Core formatting. Touched source files remain under 1,000 lines. The archive retains
the exact inputs, statuses and unavailable observations. M03 stays **7/9**, overall **30%**;
source freshness/output backpressure and the remaining measurement/recovery gates are next.

## M03 production qualification runner — 2026-10-29

The [runner boundaries and reproduction](mapgyver-m03-qualification-runner.md) and
[evidence packet](evidence/mapgyver-m03-qualification-runner-2026-10-29.json) retain five native
captures in a SHA-256-identified [raw archive](evidence/mapgyver-m03-qualification-controls-2026-10-29.zip).
Independent GPU producers/consumer exercise the production Golden host/runtime, two held previews,
bounded opt-in CPU/GPU/source metadata, real LOCAL DXGI admission and sparse process observations.
Rust-generated fixture contracts, GPU marker checks, owned-process watchdog and offline analysis
preserve missing/invalid/lost observations. Normative GPU critical-path and output-frame age remain
unavailable; API/consumer-capture observations are not relabeled as those measurements.

Corrected controls use one-second warmup, three-second sample and one-second minimized/disconnected
intervals, on Intel adapter 0. Q01 accepts 180/180 main window and Spout submissions. Q02 accepts
75/180 main-window and 139/180 Spout submissions; its renderer-context GPU p95 is 25.268 ms.
Both achieve the requested output client/backing and 640×360 / 480×270 preview registrations,
advance main outputs through the short editor interruptions, retain both HWNDs through actual
retirement and exit all owned processes without force. Final native retirement takes 0.101 / 0.139 s.
Renderer telemetry drops 286 / 17 contended rows respectively; complete coverage is not established.

The three failed controls remain retained. Fixes select the known loopback fixture URL instead of
the WebView's recorded `about:blank`, settle the physical client after the 125%→100% monitor move,
and observe actual HWND destruction after Tauri's asynchronous destroy request. No display/driver/
overlay setting changed. Screenshot approval timed out; no visual/physical qualification is inferred.

Validation: 164 regular Rust tests, nine explicit native GPU regressions, 185 Python tests,
22 UI unit tests, Svelte check/lint and browser fixture/recovery checks pass. Strict all-target
native Clippy, locked release x64 build, both required formatters and whitespace checks pass.
M03 stays **7/9**, overall **3/10 (30%)**. No prescribed three-sample series was run while measurement
coverage and observed performance remain insufficient. M05/M09 criteria retain their owners;
renderer handle/growth/drain and genuine removal/recovery remain required under M03.

## M03 admitted startup verification — 2026-10-29

- [Admitted startup retries](mapgyver-m03-render-foundations.md#admitted-startup-retries--2026-10-29)
  and [artifact packet](evidence/mapgyver-m03-startup-retry-2026-10-29.json) resolve initial allocation
  retry within gate 4. Admission precedes native work; pending core operations cannot query GPU
  completion or fabricate loss. Retry retains the original baseline and first device identity.
- Time/project controls and one latest desired plan remain live while startup waits. Cancellation
  wakes backoff; terminal startup cannot park before wake-notifier installation. Current native
  readiness and first-success history are explicit, separate observations.
- **147 regular tests** and **eight explicit native tests** pass. Real missing-LUID factory
  attempts hold a 320-byte reservation, then recover on attempt 4 into device generation 1,
  install the pending plan and submit accepted native output. Both shared-host smokes, strict
  Clippy, both-app compilation, generated drift and required formatters pass.
- **M03 stays 7/9 = 78%, overall 3/10 = 30%**. No new whole gate closes. Genuine native removal
  and bounded stuck-source retirement remain gate 4; the production Q01/Q02 qualification runner
  is next under gate 9. No budgets or denominators change and no M01 investigation restarts.

## M03 endpoint recovery checkpoint — 2026-10-29

- [Endpoint recovery](mapgyver-m03-render-foundations.md#independent-native-endpoint-recovery--2026-10-29)
  and [artifact packet](evidence/mapgyver-m03-endpoint-recovery-2026-10-29.json) record partial gate 4.
  Fixed output slots retry independently at 250 ms–2 s, retaining failed native objects until
  real completion and destroying them before replacement. Busy/occluded endpoints stay allocated.
- Sources withdraw publication/old guards and report Recovering before a possibly stuck drain.
  Cancellation and renderer acknowledgement retain GPU ownership; they do not certify retirement.
  Runtime Time/Backend faults resolve separately; close fences late recovery and exposes Stopping.
- **138 regular tests** and **seven explicit native tests** pass. The owned Spout collision test
  proves initial output failure isolation, healthy-path continuation and exact-name recovery on
  attempt 3 with the same renderer device. Source/image/region/app regressions and both real
  shared-host smokes pass. Strict Clippy, both-app compilation, generated drift and formatting pass.
- **M03 remains 7/9 = 78%, overall 3/10 = 30%**. No new acceptance item closes. Initial device
  allocation retry is next; genuine native removal and bounded stuck-source recovery/retirement
  remain required for gate 4, followed by unchanged gate-9 renderer qualification. No M01 or
  display/NVIDIA overlay investigation was reopened.

## M03 preview contract verification — 2026-10-29

- [Preview registration and geometry](mapgyver-m03-render-foundations.md#native-preview-registration-and-geometry--2026-10-29)
  and [artifact packet](evidence/mapgyver-m03-preview-2026-10-29.json) close gate 8. Rust generates
  the local protocol; Golden owns bounded leased metadata, region crop/clear and Svelte bindings.
  The shared host supplies actual WebView/window metrics through its optional bounded callback.
- The real Svelte/native consumer passes initial/clipped/hidden/unregistered/empty/disposed
  phases, 103 submitted frames and 206 accepted output calls on Intel. It requires the window
  output's own accepted token before uncovering a current installed generation. Independent
  output and retirement before HWND release pass; this is not scanout or physical alignment.
- **171 regular Rust tests** and **22 preview UI tests** pass. All **six explicit native tests**
  pass, including eight finite 4×4 region grids with unchanged 2/255 tolerance, crop/removal/clear,
  active visibility withdrawal without reallocation, and existing image/Spout/app regressions.
  The host window-only/Spout-only/resized regression also passes.
- Built-fixture browser regression covers real DOM callback ordering, covers, clipping/removal
  and browser-only editing. Svelte checks report zero errors/warnings; both UI lints, strict
  default/native Clippy, both-app compilation, generated drift and root/Core formatting pass.
  The first native timeout, DOM acknowledgement race and finite-readback helper correction are
  retained in the packet; no tolerance or qualification budget was relaxed.
- **M03 7/9 = 78%, overall 3/10 = 30%**. Gates 4/9 remain open. Production recovery and bounded
  retirement are next; specialized editors/full physical/DPI/input remain M05–M07. No new M01
  investigation or display/NVIDIA overlay changes.

## M03 Picture/asset verification — 2026-10-29

- [App assets and Picture resources](mapgyver-m03-render-foundations.md#app-assets-and-picture-resources--2026-10-29)
  and [native packet](evidence/mapgyver-m03-picture-assets-2026-10-29.json) close gate 7. App catalog
  nodes reuse persistent node UUIDs, persist locators only and use the existing atomic operation,
  history, sparse/full codec and QuickJS routes. Capture reads demanded leaves only, with no IO.
- PNG/JPEG worker decoding, explicit SDR/profile/orientation policy, per-source missing retry,
  targeted file-change retry, relocation and healthy-source preservation pass. Image leases and
  automatic fences span CPU preparation, final native installation and resource reconstruction.
- **128 regular tests pass**: 30 app, 16 Assets, 79 Video unit and 3 public Video. All **five**
  explicit native tests pass on Intel LUID `00000000:00016c2a`, including actual app PNG upload,
  shared channels, relink/reload/fresh runtime and finite Golden alpha-filter/recreation pixels.
  Upload counters stay fixed across subsequent frames; images consume zero Spout receiver slots.
- Rebuilt host smoke, strict default/native all-target Clippy, both-app default checks, generated
  contracts, UI check/lint, root/Core formatting and whitespace checks pass. No native performance,
  hardware-removal, physical/DPI or complete Media Manager qualification is inferred.
- **M03 6/9 = 67%, overall 3/10 = 30%**. No denominator/budget changes. Gates 4/8/9 remain open;
  dedicated manager controls, commands/callbacks/snippets and ready-file watching remain M04.

## M03 shared asset checkpoint — 2026-10-29

- [Shared asset foundation](mapgyver-m03-render-foundations.md#shared-asset-reference-and-preparation-foundation--2026-10-29)
  implements persistent references/validated locators, pure capture/relocation/relink, guarded
  immutable worker preparation and lifetime admission through every prepared consumer.
- **16 Assets tests** verify contracts, missing/replaced sources, relocation/relink, cancellation,
  source/codec/item/byte bounds and off-thread final disposal. The independent public Video test
  installs a prepared synthetic extent and rejects a stale scheduled replacement while keeping
  its live generation. The complete regular app/Assets/native-feature Video suite passes **109**.
- Strict default/native-feature Clippy, both-app default checks, root/Core formatting and
  whitespace checks pass. Native/UI implementations and qualification budgets are unchanged;
  no new hardware or Picture decode/upload qualification is inferred.
- Gate 7 remains partial pending app codec/history/Picture and GPU resource integration. M03 is
  still **5/9 = 56%**, overall **3/10 = 30%**, with unchanged scope/counts. Shared P15 plan evidence
  is updated; Chataigne's audio migration, collect/relink and scheduled-media acceptance stay open.

## M03 channel bridge verification — 2026-10-29

- [App channel publication bridge](mapgyver-m03-render-foundations.md#app-channel-publication-bridge--2026-10-29)
  and [small evidence packet](evidence/mapgyver-m03-channel-bridge-2026-10-29.json) record demanded
  leaf capture, history-retained-node invalidation, source-only warmup and public guarded submission.
  Preparation runs outside the actor/UI; per-frame counters do not trigger plan publication.
- **92 regular tests pass:** 17 app, 73 Video unit and 2 public Video. The explicit app hardware
  test verifies independent 2×2/3×2 channels, selector change, 4×3 resize, reconnect, undo, codec
  reload and controlled full runtime/device recreation on Intel LUID `00000000:00016c2a`.
  Ready assertions require latest-request installation and a submitted frame of that generation.
  Old project/device/layout observations are rejected; persistent references remain unchanged.
- Both explicit Golden hardware tests and the rebuilt shared-host smoke pass. Those finite
  Golden pixel assertions remain separate from the app bridge's metadata/installation assertions.
  Genuine hardware device removal, sustained performance, physical output and DPI are unqualified.
- Strict default/native all-target app/Video Clippy, both-app default checks, UI check/lint,
  generated-contract drift, root/Core formatters and whitespace checks pass. Lockfile changes
  add only two existing dependency edges. No display/overlay setting or qualification budget changes.
- Gate 5 closes: **M03 5/9 = 56%, overall 3/10 = 30%**. The denominator stays nine.
  Gates 4/7/8/9 remain open. Next is shared persistent assets/preparation; full Media Manager
  controls/commands/templates and production Screen/preview consumers retain M04/M05 ownership.

## M03 plan-owned subscription checkpoint — 2026-10-29

- Entered from published `a307eeac` on a clean `mapgyver` checkout. Repairing the native
  startup-only binding restriction was the next prerequisite for live app channel publication.
  Subscriptions now travel inside revisioned plans; startup configuration reserves explicit
  worker/storage capacity across all overlapping prepared/live/retiring sources.
- [Subscription evidence](mapgyver-m03-render-foundations.md#plan-owned-source-subscriptions--2026-10-29)
  and its [artifact packet](evidence/mapgyver-m03-plan-sources-2026-10-29.json) cover fresh
  sender replacement, conflicting-ID rejection, independent worker/storage ceilings,
  asynchronous capacity reuse and preserved live pixels/device identity. The original
  receive/resize/reconnect cases and shared-host output smoke also pass. Public source-pressure
  rejection preserves `Running` state, the live generation and continued frame submissions.
- **82 regular tests + 2 explicit hardware tests pass**. Strict native app/Video/smoke and
  default Video Clippy, both default app checks, root/Core formatting and whitespace pass.
  A transient test-only nonblocking readback failure is preserved; its bounded readiness
  handling uses the original five-second deadline and unchanged numerical pixel tolerances.
- M03 remains **4/9 = 44%**, overall **3/10 = 30%**, with no denominator or budget change.
  The app-owned demand/publication/observation bridge and resource-recreation proof are next;
  the native subscription prerequisite does not itself close gate 5. No M01 profiling,
  display/overlay changes or native performance qualification was performed.

## M03 media/channel authoring checkpoint — 2026-10-29

- Entered from published `44e3ecd9` on a clean `mapgyver` checkout. The app now registers
  separate media/channel nodes using persistent typed UUID references. Backend operations own
  initial channels, labels, source settings, size policy and validation; creation inserts one
  detached subtree and leaf changes use Golden's authored edit/history path.
- [Authoring evidence](mapgyver-m03-render-foundations.md#mediachannel-authoring-contracts--2026-10-29)
  records real project roundtrip, rename/reorder/remove/undo/redo, missing/wrong-owner rejection,
  per-channel settings, backend size resolution and production-actor save coverage. QuickJS
  uses the same app operation and receives the committed identity with one undo step.
- Rust emits the app intent/result/size contracts and canonical Golden Video descriptor/filter
  declarations; a byte-comparison drift test passes. The app UI's adapter uses its session
  interface and generated intents, with no frontend identity/default/domain behavior.
- **11 app tests + 70 Video tests pass** (68 unit + 2 public in Video). The two native hardware
  tests remain explicitly ignored in this data-contract run; their prior hardware evidence is
  retained. Strict native-feature app/Video all-target Clippy, strict optional smoke-target
  Clippy, both default app compile checks, UI check/lint and root/Core formatting pass.
- M03 remains **4/9 = 44%**, overall **3/10 = 30%**. Gate 5 is partial: live renderer binding,
  fenced source observations and app identity through actual resource recreation are still
  required. Picture/assets, dedicated controls/command nodes/script templates, Composition and
  Matrix retain their planned owners. No new native performance/display pass is claimed.

## M03 bounded input verification — 2026-10-29

- Entered from published `03c71423` on a clean `mapgyver` checkout. Golden Video now owns
  independent demand-driven Spout receiver workers on the explicitly selected adapter LUID.
  Prepared/live/retiring plans retain demand. Each source has a capture texture, three bounded
  GPU bridge slots and independent completion; renderer leases survive through matching GPU
  completion. No pixels or native handles enter the engine/control protocol.
- [Input evidence](mapgyver-m03-render-foundations.md#bounded-spout-inputs--2026-10-29) and its
  [exact-artifact packet](evidence/mapgyver-m03-spout-inputs-2026-10-29.json) retain two owned
  independent producer processes, public-runtime reception, six finite composition observations,
  prepared-demand cancellation, full-bridge backpressure, static-frame reuse, reconnect, admitted
  resize, oversized-source rejection and continued healthy-source progress. Actual extent is
  observed separately from the configured maximum; startup bindings remain immutable.
- Native-feature suites pass **161 regular tests**: IO 58 unit, Time 32 unit + 1 public, Video
  68 unit + 2 public. Both separately selected hardware tests pass on Intel LUID
  `00000000:00016c2a`, including the nine existing color/alpha/filter/blend cases. The current
  shared-host smoke again passes window-only, Spout-only and combined resized-client phases,
  with stopped/zero-reservation observations and retirement before HWND destruction.
- Default Video tests pass **62 unit + 2 public**. Strict default/native all-target Clippy,
  strict source-fixture/host-smoke Clippy, both default app checks and root/Core formatters pass.
  No dependency versions, generated protocol contracts, main app or frontend changed.
- This closes fixed native execution **gate 3**: M03 is **4/9 = 44%**, overall **3/10 = 30%**.
  The denominator and all qualification budgets are unchanged. Gate 4 device-loss/full recovery,
  gate 5 app contracts/dynamic binding publication, gate 7 assets, gate 8 previews and gate 9
  native measurements remain open. SDK receive/send acceptance is not proof of fresh delivery;
  its possible 67-ms mutex wait and transient foreign-alias import remain documented limits.
  The main app is still the workbench. Next is gate 5's app-owned media/channel contracts.

## M03 native checkpoint — 2026-10-29

- Entered this batch from published `db9a1e06`, independently verified equal to the remote
  with a clean checkout. The optional Windows D3D11 backend now runs the real compiler/worker/
  core path, four-mode composition/conversion, persistent fixed window and Spout outputs.
- Baseline admission precedes output-pool allocation; generation charges include scratch and
  converted output textures. Submission claims and preallocated query guards retain ownership
  on unwind. New baseline admission/drop/reset and submission-unwind regressions pass.
- [Native evidence](mapgyver-m03-render-foundations.md#native-d3d11-execution-and-owned-host-consumer--2026-10-29)
  and its [exact-artifact packet](evidence/mapgyver-m03-native-smoke-2026-10-29.json) retain nine
  finite pixel cases at `2/255` tolerance and the public shared-host smoke on Intel LUID
  `00000000:00016c2a`. Separate output phases and the combined shared-producer plan pass after
  client resize; stopped observations show zero reserved bytes and native resources retired
  before HWND destruction. API acceptance is not proof of delivery/scanout.
- Native-feature suites pass **158 tests**, with the one hardware test then run explicitly
  and passed. Default Video tests pass **62 unit + 2 public**. Strict default/native and smoke
  Clippy, both default app compile checks, root/Core formatters and whitespace checks pass.
- Gate 3 remains **partial**, M03 remains **3/9 = 33%**, overall **3/10 = 30%**. Bounded Spout
  receiver workers are next. Fixed DXGI stretch, descriptor-only native storage charges and
  the SDK's possible 67-ms mutex wait/true-on-skip remain explicit limitations. No physical
  output/DPI, native device-loss or sustained Q01/Q02 budget pass is inferred. Existing
  failures and qualification ownership remain unchanged; the main app is still the workbench.

## M03 verification — 2026-10-28

### Worker/null runtime and shared time

- `golden_time` owns monotonic/show/sample coordinates, checked rational conversion, immutable
  mappings and authority/discontinuity epochs. Golden Video consumes it for absolute frame
  cadence and activation; multiple readers/outputs do not advance show effects independently.
- Golden IO's payload-free wake/checkpoint wait supports renderer deadlines without mailbox
  polling or lost service-to-wait notifications. One preparation worker feeds the latest desired
  plan; reliable terminal outcomes retain bounded admission credits until drained.
- Worker-local resources stay charged through preparation, live use and retirement. Atomic
  install preserves the old scene on rejection, stale project/revision/device/time work is fenced,
  and resource destruction waits for completion or confirmed device loss. Close/drop never joins
  the actor/UI thread; a timeout does not certify native-window retirement.
- `cargo test --locked -p golden_video -p golden_time -p golden_io`: **58 video unit + 2 public,
  32 time unit + 1 public, and 58 IO unit tests passed**. Strict all-target Clippy passes for all
  three crates. Both `MapGyver` and `Chataigne2` pass `cargo check --locked` with the additive IO
  API. Root default selection remains Chataigne; no dependency versions are upgraded.
  The final 151-test run includes clock/mapping publication, old-frame status and close-before-
  activation regressions, with no ignored tests. Root and Golden Core formatting passes.
- Gates 2 and 6 close, bringing M03 to **3/9 = 33%**; overall remains **3/10 = 30%**.
  The [detailed evidence](mapgyver-m03-render-foundations.md) distinguishes simulated null
  counters/device loss from native measurements. Six gates remain open, including native
  backend/recovery/qualification, app channel contracts, assets and preview geometry. Shared
  Chataigne P14 audio/DMX acceptance stays open; P15 assets are not implemented by this batch.

### Immutable plan compiler checkpoint

- Defined nine fixed complete-scope gates before source work, replacing the provisional
  whole-phase item. All started open; the scope/native budgets are unchanged.
- Added the standalone `golden_video` workspace crate and [render/resource compiler](mapgyver-m03-render-foundations.md).
  Typed metadata and immutable operations validate all branches, reject cycles/missing references,
  compile demand in deterministic order and count shared logical textures once with checked bytes.
  Inactive storage is pruned/compacted; no native/engine/Tauri dependency is introduced.
- `cargo test --locked -p golden_video`: **27 unit + 2 public tests passed**. Strict
  all-target Clippy passes. Coverage includes 10k iterative depth, resize identity, raw work/
  dimension/storage limits, overflow, source color/alpha metadata and finite premultiplied values.
- Gate 1 closes: **1/9 = 11%**. Overall stays **3/10 = 30%**. Root/Core formatting and whitespace
  checks pass. The Cargo lock change only registers the new crate using existing `thiserror`.
- Shared IO/time/assets reconciliation identifies existing reusable worker/fence primitives
  and missing production time/persistent asset contracts. P14/P15 are not marked complete;
  the next batch must consume the shared owners rather than create app-local alternatives.
- No native GPU measurements, pixel correctness, runtime atomic-install/retirement, device
  recovery or native budget acceptance is claimed. All eight remaining gates and historical
  native failures remain open at their existing owners.

## M02 verification — 2026-10-28

### Native-presentation lifecycle delivery

- Golden exposes `NativePresentation<R>`, an unstarted `NativePresentationTask`, its stop
  token and two presentation-aware launch entrypoints through the public facade. Default
  configuration stays non-generic/cloneable; default/headless launches start no presentation.
  UI preparation, native clipping and window construction stay in the shared desktop host.
- Explicit exit requests cancellation, keeps the UI pumping while the task retires, then
  destroys the main window and waits for its `Destroyed` event before allowing exit. Joining
  happens after `run_return`. Spontaneous task completion leaves unsaved work/editor open;
  confirmed frontend destruction uses the same host gate. Direct Tauri destroy permission
  is removed from both app capabilities.
- `cargo test --locked --profile dev -p golden_host_desktop -p golden_core -p MapGyver`:
  **24 host tests passed**, one subprocess helper ignored; **1 public facade** and **2 MapGyver**
  tests passed. Coverage includes repeated exit, destroy acknowledgement/code preservation,
  preparation failure, worker cancellation, success/error/panic and worker-side capture drops.
- The feature-gated `native_presentation_smoke` binary passed on Windows through the public
  host: real WebView COM access on the UI STA, underlay clipping, delivered cancellation,
  responsive UI during retirement, live HWND at worker resource drop, and invalid HWND after
  host return. It uses an isolated app identity, loopback port and owned backend data directory.
  The agent needed native WebView profile access outside its filesystem sandbox.
- Two smoke findings were corrected: an integration-test target lacked Tauri's binary manifest
  and failed before main; the supported feature-gated binary receives it. The first native run
  left its HWND live after `run_return`; the explicit destroy/ack gate fixes that observed failure.
  The worker's independent completion was also corrected to preserve frontend save/cancel.
- M02 closes **7/7 = 100%** with its denominator unchanged. Overall is **3/10 = 30%**.
  Engine/protocol-only compilation, strict all-target Clippy, both default app builds and
  required root/Core formatting pass. Existing GPU/cadence/resource/DPI failures and budgets
  remain with M03/M05/M09; no renderer, full-process or physical-output quality pass is claimed.

### Earlier shell and app-isolation delivery

- [Implementation and live evidence](mapgyver-m02-shell.md) cover the app-owned shell, public
  configured host, generic discovery, app-scoped UI state and explicit developer commands.
  Both apps' bundled and development windows subscribed concurrently on their own ports;
  headless hosts independently reached ready read models. Own save/reopen succeeded; foreign
  content returned HTTP 400 without changing live generation, file metadata or node count.
- Full application projects now require `application_id`, checked before sparse expansion,
  callbacks or replacement. Missing/foreign identity is a hard failure without backup recovery.
  This deliberately breaks untagged app projects; no migration is supplied. Only four owned
  Chataigne test/qualification fixtures were tagged. Generic graph/Preferences codecs remain
  independent.
- Final review caught and fixed upload validation occurring after a destination write, and
  `--no-frontend` selecting/admitting another app's development origin. Rejected foreign or
  untagged uploads now leave existing primary/backup files byte-for-byte unchanged and never
  resolve a new destination, including recovery requests. The host keeps the configured app
  URL and admits only its exact origin. `cargo test --locked --profile dev -p
  golden_transport_server -p golden_host_desktop` passes: **86 transport**, **13 desktop** and
  **1 public headless** test; two existing tests remain ignored.
- The affected persistence, engine, desktop, transport, protocol, codegen and xtask test suites
  pass. Engine regression after module extraction: **655 passed, 3 ignored**. MapGyver's two
  project tests and Golden's public-host consumer test pass. UI suites: **Golden 213/213** and
  **Chataigne 193/193**. Both Svelte checks report **zero errors and warnings**.
- `cargo build --locked -p MapGyver -p Chataigne2` and strict all-target Clippy pass. Golden's project I/O and
  sparse codecs now have focused modules under 1,000 lines, preserving public APIs. The
  workbench's type-only session contracts are likewise separate. The npm lockfile only
  registers/hoists the new workspace; dependency version/integrity sets are unchanged.
- M02 closes **6/7 = 86%** with the original seven-item denominator unchanged. The optional
  typed native-presentation host extension stays open. Overall remains **2/10 = 20%**.
  Required root/Core formatting and whitespace checks pass. No native renderer, DPI, input,
  resource-retirement or release qualification is inferred; see the report's limitations.

### Earlier public-bundling checkpoint

- Extracted `golden_codegen_support::ui_bundle` and converted Chataigne's build script to
  configure it. The public config identifies workspace/UI/source roots, npm workspace and
  dependency-cache namespace. Bundle identity includes caller directory, config, toolchain
  and frontend environment; app-specific native SDKs, formulas, node generation and Tauri
  preparation remain in the app. No production host, protocol or UI behavior changed.
- `cargo test --locked -p golden_codegen_support --lib`: **19 passed**. Cases include
  workspace/caller cache isolation, input additions/removals, output damage, skip/prebuilt
  behavior and stable escaped asset paths. Strict all-target Clippy passes.
- `cargo check --locked -p Chataigne2 --no-default-features` passes with `GC_SKIP_UI_BUILD`,
  `GC_UI_ASSUME_BUILT` and `GC_FORCE_NPM_CI` unset for that process. Its build script reports
  a real frontend rebuild; the generated Rust embeds **35 assets**, including root `index.html`,
  with a valid content-cache stamp. This checks the existing desktop-host consumer's build,
  not native launch, default audio-feature coverage or the future MapGyver app.
- First M02 item closes: **1/7 = 14%**, with six items still open. Overall remains **2/10**.
  Required root/Core formatting and diff checks pass; no native/GPU capture or display/overlay
  action was performed. The next source batch is the MapGyver scaffold and app identity seams.
- The user approved proceeding from functional feasibility to the thin shell. The plan,
  native architecture, qualification contract and route decision now assign renderer budgets
  to M03, full output/DPI/input acceptance to M05, and complete-app qualification to M09 and
  owning features. The original failures, budgets and evidence artifacts are preserved.
- The four revised M01 items are supported by their scoped native evidence and recorded
  technology choice. M01 is 4/4; seven complete-scope M02 items are fixed before implementation
  and started at 0/7 before the source work above. Overall accepted milestones are 2/10. This is an approved denominator
  change, not a claim that the original full gate passed or that the shell already exists.

## M01 verification — 2026-10-28

- Consumer-control milestone: 95 deterministic Rust tests, strict default/desktop Clippy,
  release build, generated bindings and required root/Core formatting pass; seven hardware
  tests stay ignored by the standard suite. All 167 Python tests pass. The first 30+8-second
  paused native run completes with fenced, unchanged work/resource-marker counters and zero
  final ownership, but is **rejected** for 3834×2143 geometry and seven raw per-chain QPC
  reversals. Complete original streams and an unordered-only diagnosis are retained. No
  active/paused/active long series is run. The user-confirmed 4K display is detected on NVIDIA
  adapter 1; future qualification must explicitly record that new route and observe overlay
  Off before NVIDIA startup. One named adapter is required, not mandatory dual-vendor success;
  full product Q05 stays with its owning milestones/M09. Counts remain M01 1/8 and overall 1/10.

- GPU-stage milestone: 78 deterministic Rust tests and strict default/desktop Clippy pass;
  seven hardware tests remain ignored by the standard suite. All 123 Python tests pass,
  including raw-u64 stage identity, interval conservation, complete coverage and mode guards.
  Generated bindings, release build and required root/Core formatting pass. The predeclared
  Intel off/on/off controls and staged 30+120-second run retain every native/query/CSV stream;
  independent analysis recomputes all 31,461 rows and exact stage joins. Short controls vary
  and sustained still loses 75 output submissions, so no causal repair or qualifying series
  is claimed. M01 remains 1/8 and overall 1/10. See [stage evidence](mapgyver-m01-native-feasibility.md#same-context-gpu-stage-boundaries--2026-10-28).

- Bounded-import milestone: `cargo test --locked -p mapgyver_native_probe --features desktop`
  passes 69 deterministic tests; seven hardware tests remain ignored by that command. The two
  focused Intel bridge ownership/timeout/resize/pixel tests are explicitly run and pass.
  Strict default and desktop/all-target Clippy, generated binding verification, release build
  and required root/Core formatting pass. The Python suite passes 104 tests, including strict
  import counter/stage validation and configurable collector-tail coverage. Release Q02
  authored-card accuracy covers all 8,294,400 pixels; predeclared 30+8/30+120-second captures
  preserve exact packets and all 20,508 raw CSV rows. Independent recomputation verifies native
  streams, hashes, role/policy/loss observations, identical inputs/context, cache bounds and
  zero logical/alias ownership after cleanup. Both owned trace sessions are absent and the
  pre-existing service remains PID 5864. This verifies bounded import reuse, not a causal
  performance repair or cadence qualification; M01 remains 1/8 and overall 1/10.

- [Output-only policy analysis](evidence/mapgyver-m01-output-policy-analysis-2026-10-28.json)
  recomputes the [predeclared A/B/A](evidence/mapgyver-m01-output-policy-plan-2026-10-28.json)
  and all 48,886 raw rows. Frozen inputs, distinct sessions, chronological native intervals,
  per-role policy and matching actual window contexts verify. Native facts remain separate
  from capture completeness: A1/B live counters are zero; A2 status 4201 leaves them unavailable,
  and the strict analyzer exits 1 with factual runs retained. B is not a repair and the late
  baseline drift remains authoritative. Latest stays default, M01 remains 1/8 (13%) and overall
  remains 1/10 (10%). All three packets are promoted byte-for-byte; owned sessions/processes
  are gone and the existing collector service retains PID 5864.
  Sixty deterministic Rust tests, strict default/desktop Clippy and 88 Python tests pass.
  Generated UI bindings match; actual root/Core formatters, both format checks and whitespace
  checks pass. The derived analysis recomputes exactly and deliberately exits 1 for incomplete
  A2 observations. All 126 evidence JSON files parse and preserve Git input bytes, 231 local
  links in the three updated docs resolve, and all 80 Rust sources remain below 1,000 lines.
  Frozen source/assets/helper/tool/native fingerprints still match after final formatting.
- [Warmed 4K analysis](evidence/mapgyver-m01-presentmon-warmed-analysis-2026-10-28.json) verifies
  the [complete capture packet](evidence/mapgyver-m01-presentmon-warmed-intel-q02-2026-10-28.json),
  kept byte-for-byte through exclusive generated output and promotion. All 4,537 raw rows retain
  SyncInterval 0 and native role identity. The sample has 480 renderer iterations, no missing
  source publications, 480 output Presents/one drop, and 475 editor Presents/no displayed drops.
  Five editor attempts coalesce at the readiness check; they are separate from DXGI Busy HRESULTs.
  The independent post-measurement query succeeds with zero event/log/realtime-buffer loss
  counters, while final capture and analysis loss remain open. Actual owned-window observations
  identify primary 240-Hz/125% and secondary 60-Hz/100% nominal modes; 4K client extent still
  exceeds the secondary monitor. No cadence or physical scanout pass is inferred.
- [Saved-control diagnosis](evidence/mapgyver-m01-presentmon-role-drop-diagnosis-2026-10-28.json)
  recomputes both earlier traces exactly and sorts query-reap rows before CPU-iteration bracketing.
  The two-second warmup includes source resize/absence/return: 397 post-return Q02 iterations
  contain 80 editor/20 output drops and mean scoped renderer GPU time 9.953 ms. This association
  motivates the warmed control; it does not prove a GPU/compositor cause or replace retained failures.
- [Read-only conditions](evidence/mapgyver-m01-warmed-display-conditions-2026-10-28.json) preserve
  OS/driver/WebView/power observations. Controller-level mode fields are not per-output evidence;
  DXGI still lists both attached outputs on Intel and none on NVIDIA. The
  [absent-session control](evidence/mapgyver-m01-etw-absent-query-2026-10-28.json) records status
  4201 and unavailable statistics on its identified earlier query build, rather than zero counters.
  Fifty-five deterministic Rust tests, strict default/desktop Clippy and 67 Python tests pass.
  Generated UI bindings match; actual root/Core formatters, both format checks and whitespace
  checks pass. Both embedded diagnoses reproduce exactly, all 121 evidence JSON files parse,
  227 local links in four updated docs resolve and all 79 Rust sources stay below 1,000 lines.
  The warmed capture's source/tool/native fingerprints still match after final formatting.
  MapGyver evidence JSON disables Git newline normalization so Windows checkouts preserve
  byte-level packet/artifact checksums; existing evidence contents remain unchanged.
  M01 remains 1/8; overall remains 1/10.
- [Exact native-role analysis](evidence/mapgyver-m01-presentmon-role-analysis-2026-10-28.json)
  verifies short Intel [Q01](evidence/mapgyver-m01-presentmon-role-intel-q01-2026-10-28.json)
  and [Q02](evidence/mapgyver-m01-presentmon-role-intel-q02-2026-10-28.json) captures on one
  new release fingerprint. All 2,368 raw CSV rows match queried native addresses and Present-QPC
  lifetimes, with separate editor/output summaries; each retains a display tail beyond the
  recorded Present interval. Q02 preserves 85 editor/20 output drops despite 480 accepted Presents per role.
  No collector loss is reported; independent zero loss, physical outputs and sustained cadence
  stay unverified. Both owned sessions are absent, native/collector exit 0 and no owned probe
  remains. Independent review recomputes raw streams, reports and analyses exactly. Fifty-two
  deterministic Rust tests, strict desktop Clippy and 54 Python tests pass. Generated UI bindings
  match; actual root/Core formatters, their checks and whitespace checks pass. All 116 evidence
  JSON files parse, 213 local links in the three updated docs resolve and all 75 Rust sources
  stay below 1,000 lines. Final captured input fingerprints still match. M01 stays 1/8 (13%);
  overall stays 1/10 (10%).
- [NVIDIA/PresentMon handoff audit](evidence/mapgyver-m01-nvidia-presentmon-handoff-2026-10-28.json)
  verifies eight exact card captures, the first official trace/analysis, source epochs, packet
  integrity, bindings, guardrails and cleanup. All 44 Python tests pass. The unchanged native
  source/executable retains its prior 52-test checkpoint. Required root/Core formatters and
  whitespace checks pass; overlay Off remains unchanged and the existing collector service
  retains PID 5864. M01 remains 1/8 (13%); overall accepted milestones remain 1/10 (10%).
- [NVIDIA/cross-adapter card analysis](evidence/mapgyver-m01-card-accuracy-nvidia-analysis-2026-10-28.json)
  validates all eight fixed procedural/authored captures, exact native integers, common native
  inputs, full pixel coverage and cleanup. Current overlay Off is observed before/after and
  left unchanged. The initial JavaScript-rounded copies remain archived; canonical files were
  restored from exact captured stdout, with no rerun or measurement change. The wrapper now
  defaults to opaque checksummed packets; tests preserve GPU ticks above 2^53 and adjacent ticks.
- [Official PresentMon route](evidence/mapgyver-m01-presentmon-intel-official-route-2026-10-28.json)
  and [independent analysis](evidence/mapgyver-m01-presentmon-official-route-analysis-2026-10-28.json)
  retains 1,192 raw PID-filtered rows, two unlabeled chains and exact eight-second windows:
  480 Presents/Display observations per chain, no reported drops or missing display timing.
  Shared compositor timestamps are identical and cannot be pooled as independent physical
  cycles. Recorded interval p99/max 20.8141/20.8759 ms remain; no timing budget passes by inference.
  No loss is reported; that historical capture has no native role metadata. Independent zero-loss
  proof, sustained series and physical latency remain open. The owned session is absent after
  exit; the existing Intel service stays running.
  The vendor silent failure and help-preflight failure remain preserved separately.
- [ETW access recheck](evidence/mapgyver-m01-etw-access-2026-10-28.json): after the user's
  administrator VS Code relaunch, the release probe creates and stops its empty owned session
  successfully outside the sandbox. The sandbox account still returns status 5. The development
  account is already a Performance Log Users member; no group or policy change was made here.
  Non-elevated access and actual display events remain unverified. No acceptance item closes;
  dashboard counts remain M01 1/8 and overall 1/10.

- [Procedural 1080p](evidence/mapgyver-m01-procedural-card-intel-q01-2026-10-28.json) and
  [4K](evidence/mapgyver-m01-procedural-card-intel-q02-2026-10-28.json) verify every fixed-card
  pixel through the actual shader, independent processes and real ingest bridge. Maximum error
  stays 1/255, with no mismatches/exclusions against tolerance 2/255. Each producer draws once
  with 32 uniform bytes and no CPU pixel upload; renderer/ingest readbacks stay zero. Child
  identities, bounded reports, exact copy costs and six-device cleanup validate. Authored-image
  controls share that captured checkpoint's inputs. Fifty-two deterministic Rust and thirty-six Python tests,
  strict default/desktop Clippy and actual root/Core formatting pass. Broader gates remain open:
  M01 stays 1/8 (13%), and overall accepted milestones stay 1/10 (10%). The
  [saved shader audit](evidence/mapgyver-m01-procedural-card-intel-validation-2026-10-28.json)
  verifies capture/input hashes, full coverage, cleanup, guardrails and dashboard scope.
- [Cross-process 1080p](evidence/mapgyver-m01-authored-card-processes-intel-q01-2026-10-28.json)
  and [4K](evidence/mapgyver-m01-authored-card-processes-intel-q02-2026-10-28.json) verify every
  pixel across two producer children, the existing ingest bridge, renderer and independent
  consumer child. Maximum error stays 1/255, no mismatches/exclusions against tolerance 2/255.
  All four PIDs are distinct, all children are reaped, bridge pools stay at three, explicit copy
  costs match publications, renderer SDK receives/readbacks are zero, and all six ledgers drain.
  Matching local controls share that earlier checkpoint's fingerprints. Forty-eight deterministic Rust and
  thirty-five Python tests, strict default/desktop Clippy and actual root/Core formatting pass.
  Static image accuracy across this route is established; procedural cards, previews, NVIDIA,
  sustained timing and physical retirement remain open. The
  [saved process-route audit](evidence/mapgyver-m01-authored-card-processes-intel-validation-2026-10-28.json)
  verifies all four capture hashes/inputs, generated bindings, guardrails and cleanup.
- [Intel authored-card 1080p](evidence/mapgyver-m01-authored-card-intel-q01-2026-10-28.json) and
  [4K](evidence/mapgyver-m01-authored-card-intel-q02-2026-10-28.json) compare every composite pixel
  with maximum BGRA error 1/255, no exclusions/mismatches against fixed tolerance 2/255, exact
  initial upload/readback costs, renderer readback/upload zero, unregistered senders and all four
  post-fence ownership ledgers zero. Both saved reports match the earlier local-only release inputs and
  wrapper hash. Forty-six deterministic Rust and thirty-four Python tests, strict default/desktop
  Clippy, generated bindings and actual root/Core formatting pass. This is the finite static
  transfer/sampling subset only; other native accuracy/timing/retirement gates stay open.
  [Saved audit](evidence/mapgyver-m01-authored-card-intel-validation-2026-10-28.json) records
  evidence hashes, exact coverage, cleanup, local links, source limits and validation scope.
- [NVIDIA editor isolation](evidence/mapgyver-m01-editor-isolation-nvidia-analysis-2026-10-28.json)
  verifies 61.0434 seconds of busy JavaScript and 61.0238 seconds on `about:blank`, with output
  progress and valid independent-consumer frames in every one-second interval. Fresh session 3
  restores both previews. All 8,126 renderer dependency joins validate; all three contexts have
  zero admission misses and tracked ownership drains. Three startup-invalid B markers and 29
  A-absence observations remain recorded. The normal control accepts 480 calls/path. All four
  final Intel/NVIDIA captures share identical inputs at that lifecycle checkpoint, now historical.
  [Checkpoint validation](evidence/mapgyver-m01-editor-isolation-nvidia-validation-2026-10-28.json)
  retains source/tool hashes. Overlay Off is natively verified
  before/after measurement; original On is restored at 08:17:51 UTC. Full-card/scanout/GPU-path
  qualification remains open. No runtime code changed in this evidence turn.
- [Final Intel editor isolation](evidence/mapgyver-m01-editor-isolation-final-intel-analysis-2026-10-28.json)
  validates 61.0285 seconds busy and 61.0144 seconds on `about:blank`, with no frontend status
  polls, native/Spout progress and fresh consumer pixels in every one-second interval. Reconnect
  restores two previews under session 3/revision 2. Startup B-invalid markers (3), A absence (31)
  and all 8,123 renderer dependency joins remain recorded; no admission misses occur. This is
  no 60-fps pass. The host owns session invalidation through its page-navigation callback. A separate
  normal-launch control validates geometry, 480 calls/path and complete query/dependency coverage.
  Both final captures match the lifecycle checkpoint inputs. Intermediate lifecycle captures and older performance
  cohorts remain historical. Forty deterministic Rust and twenty-nine Python tests, strict
  default/desktop Clippy, generated bindings, UI checks/build and actual root/Core formatting
  pass. See the [final validation](evidence/mapgyver-m01-editor-isolation-validation-2026-10-28.json).
- [Pre-lifecycle NVIDIA analysis](evidence/mapgyver-m01-submission-stages-nvidia-analysis-2026-10-28.json)
  retains three full release samples each for Q01/Q02, all with 7200 calls per renderer/source
  path, no admission misses, complete context query accounting, valid consumer markers and
  drained ownership. All 43,200 renderer dependencies and 161,768 raw context spans validate.
  CPU/per-context GPU/consumer-age targets meet their scoped budgets; full GPU path is unqualified.
  The short and timestamp-free controls share the same inputs with these six runs and the
  earlier eight Intel runs. Original overlay On is verified restored at 06:51:44 UTC.
- Both GPUs collected full Q01/Q02 cohorts on the same pre-lifecycle native inputs, with 86,395 admitted
  dependencies and 323,619 raw GPU spans across twelve runs. This does not replace the earlier
  failures, close physical/display/retirement criteria or qualify another hosting architecture.
- [NVIDIA/cross-adapter checks](evidence/mapgyver-m01-submission-stages-nvidia-validation-2026-10-28.json)
  verify all sixteen current run inputs, saved stream/envelope/tool hashes and the twelve release
  samples' scoped call/CPU/context-query/age targets. Thirty-seven Rust and twenty-six Python tests,
  strict default/desktop Clippy, bindings, actual root/Core formatting and checks pass. Verified
  136 local links, 61 Rust source limits and 88 tool source whitespace checks. No owned probe
  remains. Runtime source is unchanged; original overlay On is restored.
- [Split-stage Intel analysis](evidence/mapgyver-m01-submission-stages-intel-analysis-2026-10-28.json)
  retains three full 30+120-second Q01 and Q02 samples with identical current inputs. All
  43,195 admitted renderer batches join their exact dependencies, with 161,851 raw GPU spans
  verified. Q01 sample 1 retains five renderer/source-A and one source-B sampled admission
  misses; 7193/7193/7195 accepted calls meet the scoped 0.1% shortfall target. Other samples
  accept all 7200 calls/path. Q02 sample 2 retains 5/4/1 warmup misses. CPU/context query
  accounting, independent consumer markers and all post-fence ledgers validate.
- Current recorded CPU/per-context GPU/consumer-age targets meet their scoped budgets, but the
  full GPU critical path remains unqualified. Signal reaches 3.6933 ms, Flush 0.6641 ms and
  profiling end markers 0.040 ms. The earlier 207.798-ms combined QueryEnd failure is not
  reproduced or attributed. Q01 sample 1 retains a 203.290-ms correlated GPU-span outlier.
  The timestamp-free control completes 480 ticks without a final clear. All eight current
  runs match their native input hashes; no NVIDIA test or setting change occurs in this turn.
- [Final split-stage checks](evidence/mapgyver-m01-submission-stages-validation-2026-10-28.json)
  verify all eight current inputs, saved stream/envelope/tool hashes, scoped targets for all six
  release samples, 135 local links, 61 Rust source limits and 88 untracked tool whitespace checks.
  Thirty-seven deterministic Rust tests, twenty-six Python tests and the native Intel held-reader
  test pass. Strict default/desktop Clippy, generated bindings and actual root/Core formatting
  plus checks pass. No owned probe remains; M01 is still in progress.
- [Correlated Intel analysis](evidence/mapgyver-m01-pipeline-intel-analysis-2026-10-28.json)
  retains three identical historical-input Q02 release samples at 30+120 seconds. All 21,583 admitted
  renderer batches have complete source dependency coverage; all context queries, raw consumer
  markers and post-fence ledgers validate. Sample 3 accepts 7186/7187/7188 editor/output/Spout
  calls against 7200, failing 0.1% cadence. A 209.139-ms render tick contains 207.798 ms in
  QueryEnd; its internal cause remains open. All sustained consumer-age p95 values meet 50 ms;
  the short control's 56.480-ms A failure remains. The full GPU budget stays unqualified.
- A paced collector preserves the 3,615,951-byte replacement stream across 56 indexed chunks,
  with whole-stream SHA and all envelope lengths/hashes verified. The prior stale-binary and
  terminal-truncated/cancelled attempts remain recorded, not counted toward the series.
  Thirty-seven deterministic Rust tests, twenty-four Python tests and native Intel bridge
  ownership pass; strict default/desktop Clippy and generated bindings pass. No NVIDIA test or
  overlay change occurs in this turn; original On restoration remains the last observed state.
- [Final pipeline checks](evidence/mapgyver-m01-pipeline-validation-2026-10-28.json) verify
  its then-current runtime inputs, saved analysis/artifact hashes, 80,885 raw GPU spans, 129 local links,
  all 61 Rust source-file limits and 88 untracked tool source whitespace checks. Actual root/Core
  formatting and checks pass. No owned native probe remains; M01 stays in progress.
- [Timeline-completion analysis](evidence/mapgyver-m01-timeline-completion-analysis-2026-10-28.json)
  retains headless negative controls: NVIDIA event-only modes admit 4/480 ticks, while timestamp
  modes admit 480/480. The final binary reproduces event-warmup 4/480 and timeline-warmup 480/480;
  timeline admission and retirement also complete 480/480 without the final diagnostic clear.
  Intel timeline controls complete 480/480 too. No pixel readback, CPU image upload or dummy GPU
  work is added to the runtime. Modern D3D11 fence interfaces are required explicitly.
- Native NVIDIA 30+8-second Q01 (intermediate admission repair) and Q02 (final completion repair)
  controls each accept 480 calls per renderer path with zero admission misses and complete query
  coverage on all three contexts. Final Intel 2+8-second Q01 also accepts all 480 renderer calls;
  source contexts record 477/480 sampled receives with full query accounting and no admission
  misses. These short controls leave repeated 30+120-second final-variant series open.
- The final NVIDIA 100-rebuild control verifies all 100 independent generation markers, drains
  101 renderer/202 ingest scopes and retains raw consumer markers. Generation-10 baseline is
  595.520 MiB; maximum positive live private growth is 21.184 MiB, absolute peak 616.703 MiB.
  The earlier failing capture peaks at 609.410 MiB, so this is no memory-root-cause closure.
  Stable checkpoints plus the final live drain have 1055–1067 handles against baseline 1062;
  partial-setup sparse samples remain separate. Final live drain is 10.016 seconds and scoped
  teardown is 0.515 seconds. Full Q05/physical retirement remains open.
- [Current native conditions](evidence/mapgyver-m01-query-progress-conditions-2026-10-28.json)
  verify overlay Off before/after NVIDIA tests, then original On restored at 05:17:26 UTC.
  Those native controls, reconstruction and headless controls match their recorded release
  fingerprints. Intermediate/historical inputs remain distinct. Thirty-five deterministic Rust
  tests, sixteen Python tests and the native Intel bridge ownership test pass.
- [Final local checks](evidence/mapgyver-m01-timeline-completion-validation-2026-10-28.json)
  verify final capture fingerprints and analysis hashes, strict default/desktop all-target Clippy,
  generated Rust/TypeScript bindings, actual root/Core formatting plus checks, 122 local links,
  all 59 Rust source-file limits and 82 untracked tool source whitespace checks. No owned probe
  remains. This validation does not close the outstanding native qualification criteria.
- [Repaired NVIDIA analysis](evidence/mapgyver-m01-source-worker-stop-order-nvidia-analysis-2026-10-28.json)
  retains three 30+120-second samples each for Q01/Q02, with 7,200 accepted calls per path,
  zero sampled late/busy outcomes, full query coverage on all three contexts and scoped CPU/
  consumer-age targets met. All six retain 254–255 source-A warmup admission misses. The full
  asynchronous GPU critical path remains unqualified; warmup lifecycle/instrumentation effects
  were investigated by the later headless controls. All nine runs have identical historical inputs.
- The repaired performance captures retain 920 renderer rows, 1,812 ingest rows and 44,159 raw
  consumer observations, with all eight renderer/sixteen ingest/twenty-four child ledgers drained.
  The reconstruction adds 240 rows and 12,175 observations, with 101 renderer/202 ingest scopes
  retired. The [native overlay conditions](evidence/mapgyver-m01-source-worker-nvidia-conditions-2026-10-28.json)
  record Off before startup and after tests, then verified restoration of original On. No
  desktop snapshots occur during measurements; no display or other settings change.
- Historical checks validate all nine repaired captures against their then-current release inputs after
  formatting, all saved envelope/artifact/analyzer hashes, the preserved old failure and
  failed private-growth target, 116 local documentation links and 58 Rust source-file limits.
  The 35 deterministic Rust tests, fourteen Python tests, native Intel bridge test, strict
  default/desktop Clippy, generated UI bindings and root/Core formatting pass. No owned probe
  remains; NVIDIA overlay is verified restored On.
- The pre-repair [NVIDIA Q01 cohort](evidence/mapgyver-m01-source-worker-nvidia-q01-release-2026-10-28.json)
  retains two successful timed samples and a third shutdown failure with no final native report.
  Stop was visible before reader release. The [ordering regression](evidence/mapgyver-m01-source-worker-stop-order-regression-2026-10-28.json)
  fails under the old order and passes after reader/mailbox release precedes the Release-store.
  The repaired release controls accept all 480 calls per path; 35 deterministic Rust tests,
  fourteen Python tests and strict desktop Clippy pass. No numeric budget is relaxed.
- The repaired [NVIDIA 100-rebuild capture](evidence/mapgyver-m01-source-worker-stop-order-nvidia-recreation-2026-10-28.json)
  retires 101 renderer scopes and 202 source workers, verifies generations 1–100 and drains all
  tracked ledgers. Compared with generation 10's live checkpoint, handles range 1054–1067
  against baseline 1062. Maximum positive live private growth is **47.926 MiB**, failing the
  32-MiB target; final settling falls below the baseline. Post-release private observations
  separately reach +69.176 MiB and are not substituted for live comparisons. Final live
  settling is 10.017 seconds and scoped worker/window teardown is 0.531 seconds. Ownership
  success does not prove physical retirement or a complete resource-budget pass.
- [Source-ingest analysis](evidence/mapgyver-m01-source-worker-analysis-2026-10-28.json) retains
  three full 30+120-second Intel Q02 runs and two short release controls with identical inputs.
  Every sampled Q02 path accepts 7,200 calls, with full renderer/source query coverage and
  scoped CPU/consumer-age and individual-context GPU targets met. Full GPU critical path stays
  unqualified. Sample 2 retains a 16.862-ms GPU event; sample 3 a 4.876-ms query-end/flush
  operation and a warmup GPU-progress stall: 57 renderer coalesces at clock-origin seconds
  24.141–25.191, plus 50/46 source coalesces. There are no event omissions. The measured cap
  works under native pressure; neither sampled success nor isolation establishes a root cause.
  Source-ID repetitions remain explicit and are separate from displayed frames.
- Five renderer scopes, ten ingest reports and fifteen external child reports drain tracked
  ownership in those runs; evidence retains 470 renderer rows, 924 ingest rows and 22,558 raw
  consumer observations. A separate same-input [ten-rebuild control](evidence/mapgyver-m01-source-worker-recreation-2026-10-28.json)
  retires eleven renderer scopes/22 ingest workers, independently verifies generation markers
  and alpha, and keeps the final scene live for 10.017 seconds. Readiness is 157–191 ms and
  scoped worker/window teardown 0.335 seconds. Full Q05 and physical/complete-process retirement
  remain open.
- Input SDK receivers now own separate source-thread devices/contexts and recover the same
  explicit sender names. A bounded three-slot NT-handle/keyed-mutex bridge exposes GPU images
  without moving SDK instances or issuing renderer copies/readbacks/uploads. Each source also
  has independent two-batch GPU admission. Renderer reads fence before reader release and worker
  joining; every source ledger is reported separately and included in the conservative payload
  upper bound. Full asynchronous GPU critical path remains unqualified, preserving its budget.
- [Final short source-isolation controls](evidence/mapgyver-m01-source-worker-controls-2026-10-28.json)
  retain Intel Q01/Q02 at 2+8 seconds, unchanged identical inputs, exit 0 and empty stderr.
  Both accept all 480 calls per path while source A has 68.318/68.152-ms receive maxima on its
  worker. Renderer CPU maxima are 1.333/2.272 ms, with zero SDK receives on that context.
  Worker pools and every context's admission/ownership drain after fences. The native Intel
  boundary test holds three readers across 100 producer reuse rejections, then verifies one
  released slot's safe reuse and changed content. Only exact S_OK grants keyed-mutex ownership.
- The release build, 34 deterministic Rust tests, fourteen Python tests, explicit native bridge
  test, strict default/desktop Clippy and generated UI binding checks pass for source isolation.
  Schema 11 preserves worker timings and bounded admission; stale binaries, excess capacity,
  hidden renderer receives/copies, mismatched timing and retained ownership are rejected.
- Final checks validate all six release captures against current inputs after root/Core
  formatting, all saved envelope/artifact/analyzer hashes, 102 local documentation links and
  57 Rust source-file limits. The 34 deterministic and fourteen Python tests pass again;
  generated UI bindings and strict Clippy remain clean. No owned probe remains. Displays and
  NVIDIA settings are unchanged throughout this Intel-only batch; original overlay On remains.
- [GPU-admission analysis](evidence/mapgyver-m01-gpu-admission-analysis-2026-10-28.json) retains
  three independent 30+120-second Intel Q02 release samples, each with 7,200 accepted calls per
  path, full query coverage, zero sampled outliers and scoped CPU/GPU/consumer-age targets met.
  Each peaks at two batches and drains all 8,990 submissions. Native coalescing is not observed
  in this cohort; forced backlog is covered by policy/accounting tests. The earlier failures
  remain evidence and a root-cause fix is not claimed. All five control/series inputs match;
  verification covers 470 renderer rows, 22,560 raw consumer observations and zero tracked
  post-fence ownership in five renderer scopes and fifteen child reports.
- Final verification reruns all 33 deterministic Rust tests and thirteen Python tests, checks
  generated UI bindings, root/Core formatting, 90 local documentation links and source limits.
  Saved envelope/analyzer hashes and all five current release fingerprints still match after
  formatting and binding checks. No owned probe remains; NVIDIA overlay and displays were
  untouched during this Intel-only batch.
- Independent two-slot renderer admission uses FIFO completion markers outside the profiling
  ring. GPU-busy ticks issue no receive/draw/send work and remain Busy on all three cadence
  paths against unchanged targets. Runtime submission owns the final flush, including frames
  without profiling slots; sparse pipeline counters and post-fence draining are reported.
  This does not bound fixture/driver/display queues or remove blocking SDK lifecycle calls.
- [Two short release controls](evidence/mapgyver-m01-gpu-admission-controls-2026-10-28.json)
  retain Intel Q01/Q02 at 2+8 seconds, exit 0, empty stderr and unchanged inputs. Both peak at
  two batches and drain every submission. Each admits 477 sampled ticks without GPU coalescing,
  preserves one swapchain readiness miss per window and a roughly 68-ms lifecycle stall.
- Release build, 33 deterministic Rust tests, thirteen Python tests and strict default/desktop
  Clippy pass for the admission implementation. Tests exercise a blocked two-slot pipeline,
  FIFO reuse, error propagation, coalesced trace/cadence accounting and stale schema-9 rejection.
  Historical captures and their failures remain bound to their original fingerprints.
- [Operation-timing evidence](evidence/mapgyver-m01-operation-timing-analysis-2026-10-28.json)
  retains two 2+8-second Intel release controls and a separate three-sample 30+120-second Q02
  series with identical inputs. The new series' first sample has 18 CPU and six GPU outliers,
  twelve skipped queries and 150.230/140.561 ms CPU/GPU maxima. Its 68/68/48 call shortfall and
  772.239/294.210 ms consumer-age maxima remain failures. No CPU/GPU event is omitted.
  Other samples do not replace it; the second retains one editor readiness miss.
- Per-operation attribution puts the large intervals around receive A/B (111.582/103.566 ms),
  Spout send (52.063 ms) and query completion/flush/accounting (106.093 ms). Resource sampling
  stays below 0.04 ms and Present below 0.5 ms. Query gaps precede long CPU intervals. The short
  controls separately retain ~68 ms input-A lifecycle calls and the SDK cleanup-sleep finding;
  neither implies an established driver/mutex root cause for the long-run stalls.
- Release build, 30 deterministic Rust tests, twelve Python tests and strict default/desktop
  Clippy pass. Rust-owned trace DTOs retain separate warmup/sample caps, absent operations,
  maxima, omissions and frame/QPC identities; GPU records reuse existing queries. The independent
  verifier checks counts, ordering, spans and raw ticks without synthesizing historical data.
  No production runtime/UI/protocol or workload/acceptance-budget changes occur.
- Final evidence checks validate five immutable-input release runs, 470 renderer rows and
  22,527 raw consumer observations. All fifteen child reports and five renderer scopes drain
  tracked ownership; all trace events fit their caps with no omitted events. Current release
  fingerprints and analyzer/artifact hashes match. Generated UI bindings, root/Core formatting,
  101 local links, fences/whitespace and source limits pass. No owned probe remains.
- [Intel repeated-release analysis](evidence/mapgyver-m01-intel-release-analysis-2026-10-28.json)
  retains all six full-duration sample distributions and maxima. All runs exit 0 with empty
  native stderr, unchanged identical inputs, zero sampled geometry mismatches and verified
  per-chain queue limit 1. Scoped CPU and consumer-age p95 targets pass. Recorded GPU
  percentiles are within target, but Q02 sample 1 has 16 skipped queries and cannot qualify
  its complete GPU budget; the other five scoped GPU results pass.
  Q01 records 7,200 accepted calls per path in each sample. Q02 sample 1 retains 7,155 attempts,
  24/23 readiness misses, seven recorded late calls per path and large timing/age maxima.
  No full cadence or native-gate pass is claimed; repetitions/skipped source IDs remain visible.
- Lossless envelopes retain reports larger than the terminal limit. Q02's known stderr
  interleaving is explicitly demultiplexed and full decoded length/SHA-256 validated. The
  read-only analysis rederives all 43,165 raw consumer observations and checks 900 renderer
  resource rows. All six renderer scopes and eighteen child reports end at zero tracked
  ownership; scoped worker/window teardown is 0.419–0.441 seconds. Physical residency,
  displayed cadence and complete-process shutdown remain unqualified.
- The release build, 27 deterministic Rust tests and ten Python tests pass. Tests retain
  high integer clocks, corrupt envelopes/markers, failed/incomplete series and large reports.
  Favorable GPU percentiles cannot qualify incomplete query accounting.
  Analyzer/artifact hashes and the six-sample native input identity are retained in the saved
  analysis. Desktop capture stays inactive throughout; no display/NVIDIA setting changes occur.
- Final saved-evidence verification checks all six envelopes against current release inputs,
  analyzer/artifact hashes, 43,165 raw observations, 900 renderer rows and eighteen child fences.
  Q02's shortfall, maxima and sixteen missing GPU samples remain explicit failures/gaps.
  Root/Core formatting, 95 local links, fences, whitespace and source line limits pass;
  no owned native probe remains.
- [Secondary-display evidence](evidence/mapgyver-m01-secondary-native-ui-2026-10-28.json)
  records 32 native keyboard/mouse inputs plus one activation restore on the 100% secondary
  display, with settled menu/modal/clipping/dock/scroll/zoom observations and outside-region
  releases. Hidden and minimized intervals last 65.458 and 79.263 seconds respectively;
  independent native output remains visible and submission/send counters progress. No displayed
  cadence, freshness, physical-pixel alignment or quantitative latency pass is claimed.
- The 608.936-second interactive process exits 0 with matching final revision 63 and scoped
  teardown 0.486 seconds. Terminal truncation loses the middle of its large report; the
  [invalid captured JSON](evidence/mapgyver-m01-secondary-terminal-truncation-2026-10-28.txt)
  remains retained. Intact header, complete observer report (610 resource rows) and footer
  decode separately; source/renderer sections stay unavailable. Both owned windows close and
  capture resets. Input fingerprints are unchanged through the interactive run.
- The runner now captures explicit diagnostic underlay sessions with lossless row compaction
  and distinct interactive labeling. Five Python tests reject invalid qualification combinations
  and preserve observation/null-segment data. A separate eight-second native control exits 0
  with empty stderr, unchanged inputs and eight renderer rows; it does not replace missing
  long-run rows or repeat the gestures. The original and runner-control inputs remain distinct.
- Saved evidence verification redecodes the intact native fields from the retained truncated
  capture and reconciles all 610 observer rows. The later control matches current fingerprints;
  only the intentional runner edit separates it from the original interactive inputs. Root/Core
  formatting, 88 local documentation links, fences, whitespace and source line limits pass.
  No owned probe remains. Rust/UI runtime sources and the tested executable are unchanged.
- [Presentation-admission evidence](evidence/mapgyver-m01-present-admission-2026-10-28.json)
  preserves an initial Intel control, native ETW permission result and four final immutable-input
  Intel/NVIDIA Q01/Q02 debug controls. All exit 0 with empty stderr and unchanged inputs; all 80
  sparse swapchain observations report maximum frame latency 1. Whole-generation admission
  counters reconcile independently from sampled cadence. The renderer acknowledges a layout
  revision only after an accepted editor Present; this does not acknowledge physical visibility.
- The final complete suite passes 27 deterministic and five native tests, including all four
  reconstruction subruns. Four Python tests, strict default/desktop Clippy and generated UI
  binding checks pass. Original NVIDIA overlay On is restored and verified, settings closed
  and capture reset. Readiness-busy/late observations stay retained; no full cadence pass is claimed.
- Saved evidence checks reconcile the 40 sparse resource rows, admission counters and all
  twelve child zero-ownership fence reports. Current source/assets/binary fingerprints match
  the four final controls. Root/Core formatting and final whitespace/link checks pass; no owned
  probe remains. ETW reports status 5 / `0x80070005`, no owned session, providers or trace file;
  displayed-frame, physical latency and repeated release qualification remain open.
- [Child-resource evidence](evidence/mapgyver-m01-child-resources-2026-10-28.json) retains an
  initial Intel control and four final immutable-input Intel/NVIDIA Q01/Q02 controls, all with
  empty stderr and unchanged input checks. Their 160 child snapshots include all source phases;
  independent source A/B/consumer logical peaks are 19.775/7.031/7.949 MiB at Q01 and
  79.102/7.031/31.680 MiB at Q02, matching across GPUs. Every child drains tracked allocations
  after its final fence. These descriptor payload figures do not measure physical residency.
- Sources now use typed readiness/resource/completion messages and the same bounded JSON
  reader as the observer. The final suite passes 24 deterministic and five native tests,
  including phase-byte, copy-byte, source-upload, reaping and pipe-loss checks across both GPUs.
  Four Python tests, strict default/desktop Clippy and UI binding drift checks pass. Overlay
  Off is verified before/after native runs, then original On restored; settings/capture closed.
- Saved evidence checks rederive all 160 child snapshots, phase payloads, source uploads and
  explicit copy totals, including twelve zero-ownership post-fence reports. Current fingerprints
  match the final cohort. Root/Core format checks, whitespace and 73 local links pass; no
  source file exceeds the line limit and no owned probe remains.
- [Frame-age evidence](evidence/mapgyver-m01-frame-age-2026-10-28.json) preserves the initial
  456-sample drift-prone Intel control, the reproduced JSON precision failure and four final
  immutable-input Intel/NVIDIA Q01/Q02 controls. Each final control exits 0 with empty stderr,
  479–480 raw samples, zero invalid markers and one 20 KiB observer log readback. Renderer
  readback/uniform-upload counters remain zero. Query polling, source absence, repetitions,
  skipped source IDs and timing outliers remain explicit. Overlay Off is verified before/after
  native runs, original On restored, settings closed and capture reset.
- The final complete probe suite passes 19 deterministic and five native tests, including all
  four reconstruction subruns and source pipe-loss cleanup. Three Python tests and strict
  default/desktop Clippy pass; generated UI DTOs remain in sync. Exact summary comparison is
  preserved by the probe-local `serde_json` `float_roundtrip` feature, with a new regression.
- Final root/Golden Core format checks, whitespace and 70 local links pass. An independent
  saved-evidence check rederives all 1,918 marker checksums, sample clocks, frame counts and
  quantiles; current source/assets/binary fingerprints match the final four-control cohort.
  No owned probe remains.
- [Live-scene retirement evidence](evidence/mapgyver-m01-scene-retirement-2026-10-28.json) retains
  all 101 generation/checkpoint/fence records, 100 independently verified markers and 236 sparse
  process/allocation samples, including ten during final live settling. Release inputs remain
  unchanged; native exit is 0 with empty stderr. NVIDIA overlay Off was verified before process
  startup and after capture, then original On restored. Handle/private-growth targets pass for
  this reconstruction subtest; source/observer logical totals and physical residency remain open.
- [Settling controls](evidence/mapgyver-m01-settling-controls-2026-10-28.json) retain two completed
  positive controls and a native close during the observed settling phase. Cancellation returns
  exit 1 and no success report. The explicit reconstruction test passes all four GPU/scenario
  subruns; 14 deterministic tests, two Python tests, strict default/desktop Clippy and UI/generated
  checks pass. The final ten-second interval extends the watchdog by ten seconds, without
  reducing any operating interval, workload or numeric budget.
- Root/Golden Core formatting and format checks pass. Saved evidence table/count integrity,
  overlay restoration records, 66 local documentation links and whitespace checks pass;
  no owned native probe remains. Subsequent source edits only clarify comments and format the
  Python runner/tests; captures retain the exact pre-edit fingerprints used during each run.

## M01 verification — 2026-10-27

- The pinned sys binding builds statically and the linked SDK reports `2.007.017`. All 29
  packaged SDK files match the upstream tagged commit after CRLF normalization; exact hashes
  and package/license/ownership findings are retained in the [M01 report](mapgyver-m01-native-feasibility.md).
- The explicit native integration test passes on Intel and NVIDIA and checks software-adapter
  rejection. Final standalone smoke reports verify live resize, reconnect, content and fixture
  registration cleanup, with four diagnostic readbacks per adapter; counts are poll observations,
  not a throughput claim. [Evidence](evidence/mapgyver-m01-spout-smoke-2026-10-27.json).
- Strict probe/all-target Clippy, root/Core format checks and Chataigne default-member/dependency
  isolation checks pass. No Chataigne/Golden production source or generated protocol changed.
- The [underlay plumbing evidence](evidence/mapgyver-m01-underlay-plumbing-2026-10-27.json) records
  eight-second Intel/NVIDIA runs, each with 473 preview presents and 474 native output presents /
  Spout sends, matching host/browser 1.25 scale, clean worker/fixture/window shutdown and empty
  stderr. This proves submissions and control geometry, not visible layering or frame cadence.
- The Svelte 5 fixture builds with zero check errors/warnings. Rust-owned DTO generation has an
  independent executable and a passing drift check, including while the native probe is live.
  Three geometry tests and the final two-adapter native recovery test pass. Default and desktop
  strict Clippy pass. No app launch defaults changed.
- The [independent consumer evidence](evidence/mapgyver-m01-composite-consumer-2026-10-27.json)
  records four color/lifecycle checkpoints on Intel/NVIDIA, five finite two-pixel diagnostic
  readbacks per run, missing-output rejection and complete bounded GPU-query accounting.
  Seven deterministic and two native tests pass. Corrected CPU/GPU p95 values are retained with
  startup maxima; the superseded timing submission error is documented rather than counted.
- The [fixed-workload evidence](evidence/mapgyver-m01-fixed-workloads-2026-10-27.json) retains
  first warmed Intel release samples for Q01/Q02, each with 7,200 CPU/GPU samples, 150 sparse
  resource observations, exact output/unclipped preview geometry, zero omitted queries and
  empty stderr. CPU p95 is 0.567 / 0.639 ms and GPU span p95 is 1.257 / 4.807 ms respectively.
  The 4K client rectangle is allocated beyond the available monitor; 4K scanout is unqualified.
- Eight deterministic Rust tests, three explicit native tests across Intel/NVIDIA and two
  Python evidence-integrity tests pass. Fixed-mode coverage checks geometry, alpha and warmup
  exclusion. Release build, default/desktop strict Clippy, UI checks/build and DTO drift pass.
- Device reconstruction adds persistent-session / per-device scopes, independently observed
  generation markers, bounded GPU retirement fences, post-release process snapshots and
  explicit clean source exit. A source-readiness handshake fixes an observed startup race;
  parent-pipe EOF cancels fixtures. The current suite passes nine deterministic Rust tests,
  five explicit native tests and two Python evidence tests. Short Q01/Q02 reconstruction
  checks pass across both GPUs; genuine device removal/reset remains separate.
- Visual underlay correctness, full alpha/reference-card checks, mixed DPI/input, full Q01/Q02 timing/memory/latency, device recreation
  and sustained cleanup remain only partially qualified. M01 stays in progress and M02-M09 remain unstarted.
- [100-cycle release evidence](evidence/mapgyver-m01-device-recreation-2026-10-27.json) verifies
  every replacement generation on Intel/NVIDIA. Intermediate post-release private growth is
  9.875 / 52.180 MiB and handles range 346–354 / 439–831 respectively. NVIDIA retirement is
  unresolved; no 10-second driver drain or complete Q05 acceptance is claimed.
- [Native UI evidence](evidence/mapgyver-m01-native-ui-2026-10-27.json) records the menu layout
  failure and repair, 31 completed native inputs on the 125% primary display, >60-second hidden
  preview content/submission observations and a 125%→100% monitor transition. A secondary
  full gesture sequence, OS 150%/200%, negative origins, physical-pixel alignment and latency
  remain open. The 900-second interactive probe exits cleanly with matching revision 60;
  interactive capture timing is not Q01/Q02 performance evidence.

- [Headless ownership isolation](evidence/mapgyver-m01-ownership-isolation-2026-10-27.json)
  retains 17 runs / 2,600 scopes and a ten-second drain per run. Intel handles stay flat;
  NVIDIA texture creation retains handles even after a zero final device Release observation.
  Private memory largely drains, so it is tracked separately. Driver/overlay cause remains open.
- [Validation follow-up](evidence/mapgyver-m01-isolation-validation-2026-10-27.json) preserves
  failed sample-count/content checks before a computer-use session reset and a successful short
  Q01 control afterward. The subsequent complete suite passes nine deterministic and five native
  tests across both GPUs. Two Python evidence tests, default/desktop strict Clippy and root/Core
  format checks pass. No thresholds, retry limits or production teardown behavior changed.

- [Termination controls](evidence/mapgyver-m01-generation-termination-2026-10-27.json) reproduce
  an incorrect early-close success after 28.273 seconds of a requested 122-second Q01 run.
  The repaired worker records one typed end reason before cleanup and rejects interrupted
  qualification or recreation watchdog expiry. A native repeat returns cancellation; the
  interactive underlay still closes cleanly with `window_closed`. The redundant reconstruction
  boolean was removed. Two regression tests cover early close and incomplete final intervals;
  native scene/reconstruction tests assert successful reasons. Eleven deterministic tests,
  five native tests, two Python tests, strict default/desktop Clippy and root/Core format checks
  pass. Reconstruction was re-exercised after the report cleanup. M01 remains open.

- [Allocation/cadence evidence](evidence/mapgyver-m01-allocation-cadence-2026-10-27.json) retains
  four short Q01/Q02 debug controls, one immutable input set and all 40 sparse observations.
  Both GPUs record 55.010 / 179.595 MiB logical renderer peaks and zero tracked allocations
  after the final context fence. SDK calls, descriptor ownership, raw DXGI errors/counters and
  per-output acceptance/deadline metrics remain separate from displayed frames and physical residency.
  The final suite passes 14 deterministic and five native tests plus two Python tests and strict
  default/desktop Clippy. A standalone initial-black-frame failure is retained; all fixture
  startup paths now use the existing first-submitted-image readiness handshake.
- [Overlay controls](evidence/mapgyver-m01-nvidia-overlay-control-2026-10-27.json) retain the
  approved on/off/on test and verified restoration. Handles grow 242→422 with overlay enabled
  before/after, and stay at 221 when disabled. Capture DLL version 11.0.9.565 remains loaded
  throughout; its installed file hash and probe inputs stay unchanged. This isolates an
  environment-dependent failure, without proving NVIDIA's internal handle ownership/type.
  The user approved overlay-off as a qualification prerequisite, with no budget/workload changes;
  overlay-on evidence stays retained and the actual user setting remains restored to On.

## M00 verification — 2026-10-27

- Read-only `tools/mapgyver/collect-baseline.ps1` executed successfully twice under noninteractive
  Windows PowerShell. Parsed JSON records actual OS, WebView2, GPU driver, toolchain and locked
  host versions; unavailable native observations are explicit. Evidence capture preceded the
  branch switch and therefore correctly records `main` as its historical branch.
- PowerShell parser, evidence schema/lock-hash checks, local Markdown links, balanced code
  fences and whitespace checks passed for the M00 files.
- Root and Golden Core `cargo fmt --all` and final format checks passed. The six remaining
  unrelated dirty root/architecture documents were checked by SHA-256 and preserved byte-for-byte.
  Chataigne runtime/P11 changes are now in the externally integrated checkpoint `40acd67d`.
- No Rust/UI runtime changed; no runtime test-suite, native frame, Spout interop, DPI, frame-time
  or floor-OS pass is claimed. The chosen Windows 11 24H2/build 26100 floor is a qualification
  target; only build 26300.9711 is inventoried locally.

## Planning verification

- Ownership/dependency review and native-rendering source accuracy review completed.
- Ten touched documentation files: 118 local links resolve; code fences and whitespace checked.
- `cargo fmt --all -- --check` and the equivalent Golden Core manifest check passed.
- No application code changed and no native rendering probe ran in this planning session.
