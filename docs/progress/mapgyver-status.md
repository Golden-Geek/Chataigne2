# MapGyver implementation status

## At a glance

**Updated: 2026-10-31 · Overall: `██████░░░░░░░░░░░░░░` 30% · 3/10 phases complete**

**Current topic / only active phase:** M04 source acceptance; **4/6 gates (67%)**.
Module, Solid Color, Picture and Shared Texture gates 1–4 are verified; Media Manager UI gate 5 is next.
**Execution mode:** completion first, per the user's 2026-10-31 directive. Close dependency-ready
acceptance within the current phase; do not create incidental features or expand several phases.
Other incomplete milestones remain an explicit return queue, not simultaneously active work.
M08 has **3/4 gates (75%)**; full control/parity and app qualification remain open.
M03 stays **7/9**; qualification no longer serializes product work. The
[execution correction](../plan/MapGyver_Implementation_Plan.md#execution-correction--2026-10-30)
changes work order, not acceptance. M04's UI and complete cross-source
parity/recovery acceptance remain open.

**Next:** M04 Media Manager UI gate 5: map existing UI/backend checks, then verify inspection,
channel/source/size/rename/reorder/delete operations through the actual app-owned interface.
Keep browser/native interaction focused on the existing controls; no desktop-recovery detour.
The M08 target-outcome
retirement question stays parked. M07 interaction
is parked after the bounded surface check; no automation recovery detour.
**Last delivered capability:** the default headless host now accepts the existing shutdown-file
option and returns normally after cancelling app services, listener, sockets and transport workers
(batch 46). The first/only ordinary-app run passes in **2.98 s**, with exit code zero, known-tree
cleanup and external survival. **187 app / 177 headless / 95 transport / 32 desktop-host tests**,
public API, strict Clippy and Chataigne compilation pass. Frontend remains **58 passing tests**
from batch 44. Native app graceful-close acknowledgement/full retirement qualification stays open.
**Latest acceptance evidence (batch 49):** Shared Texture gate 4 passes its first/only native run
in **80.70 s** on Intel and NVIDIA: **60 complete images**, exact-name discovery/rename behavior,
restart/resize/oversize recovery, independent healthy-channel continuity and both cross-adapter
rejection directions with source-local HRESULT `0x80070057`, no fallback and successful recovery.
**188 app / 161 Video unit + four public Video tests**, strict all-target/all-feature Clippy and
both formatters pass. No production code, tolerance or budget changed. Picture gate 3 remains
accepted by batch 48's 54 images / 33,177,836 texels. UI and complete intent parity retain gates 5–6.
**Retained M08 evidence (batch 43):** singleton isolation, known launcher-descendant termination,
breakaway denial and missing-file rejection are exercised in the ordinary app. Complete retirement
is unresolved; the full check failed on an overly specific invalid-image error expectation. That
test assumption is corrected without a third native run. Elevated-target denial remains unexercised.
**Project replacement (batch 45):** the ordinary headless HTTP check passes New/Save/Load,
known old-process cleanup, external isolation, fresh supervision/callback state, stable saved IDs
with fresh runtime instances, stale callback rejection and known-tree cleanup on forced host exit.
The first/only invocation takes **2.87 s**; batch 46 subsequently verifies normal headless host exit.
Batch 40's controller gate remains accepted. Actual form input and complete app recovery/scale
qualification remain open. **Product blocker:** none
for M08. The bounded UI check renders the Matrix panel but the
available helper reports higher target integrity and cannot verify input. Actual Matrix editor
interaction remains M07 gate 3; app-form input/full parity remains M08 gate 4. Independent app
pixels and full recovery/scale remain with their owning phases.
M04's full source/UI/recovery gates remain open and owned by M04. No timing/driver
qualification or M03 stash work is scheduled.
**Implementation limits:** native checks cover PNG relink, finite crop/warp/color oracles, live
Screen composition/Surface revisions, independent virtual Spout delivery without an editor and
physical rectangle geometry/lifetime and fullscreen geometry/API acceptance with concurrent Spout.
Full source/color/physical-output qualification remains open.
Native authoring used backend operations; form input and graphical drag cases were not exercised.
Keep these limits explicit without opening another automation or GPU-clock investigation.
**Qualification debt:** M03 gate 4 retains genuine device removal/stuck retirement; gate 9
retains full GPU/queue, cadence, capture/age, growth and deadline requirements. Latest Q02
sample calls remain 300/300/295; minimize/disconnect still miss four/nine Spout images and
age budgets. All recorded failures and original budgets remain operative. These requirements
must be satisfied for production readiness, within the bounded qualification lane below.

| Phase | State | Progress | Done / total items |
| --- | --- | --- | --- |
| [M00 Baseline/workloads](mapgyver-m00-baseline.md) | Complete locally | `████████████████████` 100% | 1/1 |
| [M01 Functional route decision](#m01-acceptance-items) | Complete locally | `████████████████████` 100% | 4/4 |
| [M02 Multi-app seams/shell](#m02-acceptance-items) | Complete locally | `████████████████████` 100% | 7/7 |
| [M03 Rendering/channels](#m03-acceptance-items) | Foundations ready; qualification open | `████████████████░░░░` 78% | 7/9 |
| [M04 Source media](#m04-acceptance-items) | Active closure: module/all three sources accepted; UI next | `█████████████░░░░░░░` 67% | 4/6 |
| [M05 Screens/Surfaces/editors](#m05-acceptance-items) | Output modes/both editors implemented; qualification open | `████████░░░░░░░░░░░░` 38% | 3/8 |
| [M06 Composition/editor](#m06-acceptance-items) | Functional producer/editor implemented; native/interactive qualification open | `█████░░░░░░░░░░░░░░░` 25% | 1/4 |
| [M07 Matrix/editor](#m07-acceptance-items) | Document/module and runtime accepted; grid/qualification open | `██████████░░░░░░░░░░` 50% | 2/4 |
| [M08 App lifetimes](#m08-acceptance-items) | Ownership/demand/controller accepted; editing/status/callbacks/default headless implemented; full parity/app qualification open | `███████████████░░░░░` 75% | 3/4 |
| [M09 App/artifact qualification](../plan/MapGyver_Implementation_Plan.md) | Not started | `░░░░░░░░░░░░░░░░░░░░` 0% | 0/1 |

Overall counts fully accepted milestones: `3 / 10 = 30%`; partial phases do not advance it.
Phase percentages count closed acceptance items, rounded to the nearest whole percent
(`M01: 4 / 4 = 100%`, `M02: 7 / 7 = 100%`, `M03: 7 / 9 = 78%`, `M04: 4 / 6 = 67%`, `M05: 3 / 8 = 38%`, `M06: 1 / 4 = 25%`, `M07: 2 / 4 = 50%`, `M08: 3 / 4 = 75%`). Bars use 20 cells, rounded from the exact fraction. Each phase uses
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

**Closure return queue:** finish M04's dependency-ready source gates; explicitly retain blocked
UI/hardware evidence, then revisit remaining M03, M05, M06, M07 and M08 acceptance before M09's
complete-app/artifact gate. Every original requirement remains required. A small optimization
consuming roughly a full day warrants a specific user-reviewed deferral/lower-gate proposal, not
automatic acceptance; correctness, ownership, data integrity and essential behavior stay required.
The earlier product/diagnostic ratio is historical bookkeeping in this closure mode; it must not
force new feature scope. Batch/investigation limits and honest gate accounting still apply.

### M08 acceptance items

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
| 4 | Complete generated UI/script/headless editing/control/observation parity; actual app integration and lifecycle scenarios, truthful singleton/external/elevated/launcher/breakaway outcomes, no implicit process-name adoption/kills, project replacement/shutdown and qualified recovery/cleanup. | Open |

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
| 3 | App-registered virtualized Matrix editor with source assignment, authoring/copy/reorder and separate selection/preparation/live state; complete UI/script/headless intent parity and real interactive evidence. | Open — grid/settings/reorder/renderer feedback software-verified in batches 26–28; batch 29 displays the panel but higher-integrity target/no changed click leaves real interaction/parity acceptance open |
| 4 | Native preview and independently consumed layer/group/final output pixels, ordering/blends/alpha/transition interruptions, source loss/Undo/recovery and bounded large-grid runtime/lifetime behavior satisfy the retained qualification requirements. | Open |

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
| 2 | Dependency correctness across all media types, including Composition/Matrix feedback; direct/indirect cycles reject atomically; source deletion/Undo and large/deep graphs remain coherent without owning/duplicating source media. | Open |
| 3 | Nested compositions render once per demanded output, shared sources retain independent transforms, and canvas resize, opacity/blends/alpha and rotation/pivot/scale are correct in native preview and output. | Open |
| 4 | Registered 2D editor/source picker with selection, drag/rotate/scale, grouped history and UI/script/headless intent parity; real interactive evidence covers the complete editor behavior. | Open |

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
| 6 | Screen editor/source selection uses backend intents; one drag is one undoable gesture. | Open — batch 16 implements handles/corner intents and verifies grouped history/cancellation in software; actual editor/source-selection input remains unverified |
| 7 | Surface Source editor exposes crop/mapping/calibration with coherent source, history and script behavior. | Open — batch 17 implements full-channel preview/docked crop editor and verifies geometry, history, persistence and QuickJS; native preview and actual editor input remain unverified |
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
| 5 | App-owned Media Manager UI for inspection and channel/source/size/rename/reorder/delete operations, using backend intents and generated contracts | Open |
| 6 | UI/script/headless intent parity, stable references through project/history roundtrip and functional native/resource-recreation checks across the source types | Open |

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
| M04 Source media | IN_PROGRESS | [Source acceptance](#m04-acceptance-items): module and all three source gates 1–4 accepted (4/6); Media Manager UI is next in the only active phase. Complete parity remains open. |
| M05 Screens/Surfaces/editors | IN_PROGRESS | [Authoring, crop/warp and calibration](#m05-acceptance-items) close gates 1–3; native preview, all targets, display recovery and both dockable editors are implemented. Native source-preview, actual input/hotplug/minimize and full physical qualification remain open. |
| M06 Composition/editor | IN_PROGRESS | [Authoring, nested production and graphical editor](#m06-acceptance-items): gate 1/4 accepted; producer software/native submission smoke and editor geometry/history/routing/fencing pass. Matrix dependencies, native pixels and real editor acceptance remain open. |
| M07 Matrix/editor | IN_PROGRESS | [Document/module, timed production and grid](#m07-acceptance-items): gates 1–2/4 accepted; bounded grid/settings/reorder and correlated renderer feedback pass software checks. Actual interaction/parity acceptance and full qualification remain open. |
| M08 App lifetimes | IN_PROGRESS | [Owned process/model/controller/Matrix retention](#m08-acceptance-items): gates 1–3/4 accepted; lifecycle controller qualification includes actual crash/backoff, suspended cancellation, timeout and descendant cleanup. Editing/status/callbacks/default headless and operator-visible service failures/recovery work; complete app/mode/UI/unsupported-target parity and recovery/cleanup acceptance retain gate 4. |
| M09 App/artifact qualification | NOT_STARTED | Requires implemented product and fixed native workloads |

## Next action

Close **M04 Media Manager UI gate 5**: inspect existing app-owned controls and software evidence,
then verify inspection/channel/source/size/rename/reorder/delete operations through the real
interface using backend intents and generated contracts. Resolve any demonstrated defect at its
owning boundary. Keep interaction finite and focused; do not turn missing UI evidence into another
desktop-recovery investigation. Source gates 2–4 are accepted; do not rerun them to accumulate evidence.

Keep M04 as the only active phase. Continue its UI and cross-source parity gates
in order; a genuinely blocked gate must name its missing prerequisite and remain in the
return queue. Remaining M03/M05/M06/M07/M08 requirements and M09 full-app qualification all remain
required. Missing UI/physical evidence stays open. No new incidental product feature, parked
retirement/clock/driver investigation or concurrent phase expansion is scheduled. Preserve
Chataigne's root default and unfinished P11 work.

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
