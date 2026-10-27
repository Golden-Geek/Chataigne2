# P11 actual-app Spatializer scale

The Chataigne app owns Spatializer endpoint semantics, geometry, inspector and custom map.
Golden owns the reusable canvas, navigation and session contracts. The qualification fixture
loads canonical project records through the actual default executable and authenticated
transport. It must exercise production operations and rendered content, rather than fabricate
a UI protocol snapshot. Output evaluation throughput retains P16; physical input/native webview
usability retains P17.

**All 20 calibration roundtrips and 16 correctness pressure groups pass. The 60-case timing
runner is implemented; creation now passes through 50k, but 100k completion fails.** P11 remains
4/6 checkpoints (67% rounded), with whole-plan coverage at 59%. This document and the
[machine-readable contract](../../apps/chataigne/ui/tests/browser/app-graph-scale/spatializer-contract.mjs)
fix scope before action timing. Delivery commits do not add acceptance cases.

## Source and declaration calibration

The Voronoi prototype is the canonical persisted fixture from `2f3b4dde`. The three radius-mode
prototypes were exported by the real backend after changing mode, then reduced to one source,
two targets and an empty generated values folder. No parameter declarations or project schema
were invented to fill omitted defaults. Expansion clones owned UUIDs, rewrites typed references
and supplies explicit authored labels/positions. All actual endpoint UUIDs and positions must
survive the backend roundtrip unchanged.

The primary scene has 32 sources. Targets occupy a 256-column grid at six spatial-unit intervals,
starting at `(2, 3)`; sources use the same spacing in rows above it, starting at `(2, -3)`.
The fixed 70rem by 36rem panel and owned 1680 by 1200 headless browser are unchanged.
Generated values use source-centric layout. Report source/target counts, generated folders and
matrix cells independently. These backend-record tiers do not mean 100k authored endpoints.
The backend caps each list at 4,096 endpoints and the matrix at 1,048,576 pairs.

Calibrate with `(sources, targets)` equal to `(32,8)`, `(32,16)`, `(32,64)`, `(8,32)` and `(16,32)`
in every mode. The first two determine the target coefficient; the third verifies the model.
The transposed scenes verify source expansion and source-folder cost independently. Calibration
requires all matrix cells, mode-specific radius fields, exact identities and positions. Setup
observations are not ordinary-action latency distributions.

Current backend calibration is `185 + s*S + t*T + S*T` records. The source coefficient includes
its generated values folder; `S*T` counts generated float cells.

| Mode          |   s |   t | Targets at 1k / 10k / 20k / 50k / 100k backend records |
| ------------- | --: | --: | ------------------------------------------------------ |
| Voronoi       |   3 |   3 | 20 / 277 / 563 / 1,420 / 2,849                         |
| Target radius |   3 |   3 | 20 / 277 / 563 / 1,420 / 2,849                         |
| Source radius |   4 |   2 | 20 / 284 / 579 / 1,461 / 2,931                         |
| Overlap       |   4 |   3 | 19 / 276 / 562 / 1,419 / 2,848                         |

For each tier, use the greatest target count fitting its backend-record budget after calibration.
Only the sub-target remainder may be unrelated Folder records. Verify the exact retained tree
count after publication. Creation temporarily adds one target's declarations and 32 cells;
undo must restore the baseline. Keep setup within a cumulative 300s across loading, generated
publication, projection and canvas readiness; preserve the transport's existing 120s load timeout.

## Fixed action matrix and gates

Use the same five backend-record tiers as State and Formula. Voronoi has six actions at every
tier: far-label discovery/reveal, single-target keyboard movement, one atomic movement of a
source and two targets, freeze-radius keyboard editing, backend target creation, and source
selection with its complete related-value collection. Target creation is undone between samples;
movement/radius direction alternates to keep the fixture stable.

The other three modes each qualify single-target keyboard movement and radius editing at all
five tiers. Radius editing targets a source in Source Radius mode and a target in the other modes.
This is **60 fixed action/mode/tier cases**, each with three warmups and 31 retained samples:
**1,860 retained samples and 180 warmups**. Partial selections or instrumented runs cannot close
qualification. Retain failures without relaxing gates or reducing density.

Normal gates are p95 first visible feedback <=200ms, completion <=2,000ms and maximum animation
callback stall <=200ms. At 100k stress they are 400ms / 4,000ms / 400ms. Feedback must be a new
rendered state, operation status, focus or geometry change. Completion requires the exact backend
identity/value, actual SVG geometry or inspector content, retained selection/focus and drained
editor work. Creation additionally waits for the new declarations and generated cells. Output
weight evaluation rate is a separate P16 dimension.

Creation now reports pending admission/dispatch through the editor's existing visible status
surface. Its observer accepts that actual painted status as first feedback, as allowed above;
the previous glyph-only observer matched the previous editor, which had no pending message.
Status is checked within the real panel rectangle and must differ from the pre-action content.
Completion independently still requires the created glyph, committed identity/declarations,
all 32 generated cells, retained focus and drained editing. A status message cannot satisfy
completion. Original glyph-only failures remain retained; numeric gates, density and samples
are unchanged. This observation rationale is recorded before the repaired timing rerun.

Keep at most 128 actual endpoint glyphs including retained focus/selection/gesture endpoints,
256 overview groups, 256 region polygons, 32 finder rows and 42 related-value rows. Preserve all
logical identities and collection entries. When detail cannot represent the complete view within
its budget, show an explicitly labeled overview; a truncated subset must not masquerade as the
complete map. Verify rendered geometry/content and actual DOM counts, rather than attributes alone.

## Fixed correctness pressure scenes

Eight groups exercise all four modes with Debug enabled at 50k and 100k: bounded presentation,
Home/detail roundtrip, exact selection, focused endpoint retention and related-value navigation.
Another eight load each mode at both `(32 sources, 4,096 targets)` and its transpose. These
deliberately exceed the 100k backend-record budget but stay within endpoint/pair caps. They verify
complete discoverability, bounded view/inspector mounting and exact endpoint focus in both list
orientations. They do not establish action p95 or maximum-pair output throughput.

All **60 timing cases and 16 correctness pressure groups**, the prior 13 native preservation
groups and owning checks are required before Spatializer scale closes. The app graph keyboard
checkpoint can then close; composed workspace discovery/selection/editing remains separate.

## Implementation and retained evidence

Small canonical roundtrips establish declaration counts and preserve authored identities/positions.
All **20 roundtrips** pass; the [preparation evidence](evidence/p11-spatializer-scale-preparation-2026-10-26.json)
links the four mode-specific records, canonical export provenance and source/asset fingerprints.
The existing **13 Spatializer native groups**, both four-action **1k State/Formula smoke tiers**,
strict graph-fixture Svelte checks, Prettier and normal bundled build pass. The smoke tiers do
not repeat the prior complete State/Formula qualifications. Constructor-only checks also reject
five invalid endpoint/pair inputs and preserve unique positions in all eight cap-shaped inputs;
those constructor-only observations preceded the complete pressure qualification below.
The custom map deliberately gives the shared canvas an empty graph document. The app now supplies
its complete endpoint ID/label catalog through Golden's public `GraphWorldNavigation` contract,
so the shared 32-row finder, reveal and selection framing work with actual SVG controls. Eleven
small-scene native correctness groups cover discovery, focus, keyboard edits, selection framing,
mode-control replacement, deletion/undo and session/project/space/unmount fences. The mode-control
check retained a failing run before the focus-restoration repair. This is navigation correctness,
not large-scene mounting or action timing qualification; see the
[navigation evidence](evidence/p11-spatializer-navigation-2026-10-26.json).

The bounded custom SVG and related-value implementation is described below. Existing backend
intents and module admission/publication are preserved. The early milestones below did not close
pressure or action qualification. The final forest-publication section records all 16 passing
pressure groups; all 60 timing cases still remain before graph checkpoint closure.

## Bounded presentation implementation

The app now plans endpoint mounting against the actual camera and root font size. Dense views
represent every visible endpoint in at most 256 counted groups; at most 128 real controls retain
focus, selection and active gestures. A complete lightweight catalog remains available through
the finder. A conservative distance bound includes offscreen Voronoi sites that can own visible
space; exact clipping still uses every enabled target. Too many candidates produce a labeled
region overview. Debug/influence geometry is detailed only through 128 total endpoints; larger
scenes explicitly summarize it and retain complete backend weights in the inspector.

The inspector uses shared virtual rows, stable row keys, filtering and explicit reveal. A read-only
grid reports the complete collection size while mounting at most 42 rows. A retained declaration
index removes repeated folder scans without caching stale parameter values. Ten focused tests
cover dense/coincident membership, radius intersection, retained focus, viewport conversion,
complete Voronoi candidates, large folder reads and declaration/source invalidation.

The real-backend pressure runner loads the fixed four-mode scenes, verifies actual overview
geometry and count conservation, Home/detail roundtrips, exact SVG focus and related-value
identities/weights, and retains a focused row while navigating to another. Its first run exposed
a harness precondition: the registered Home command requires focus inside the panel. The corrected
runner focuses the actual canvas and asserts command acceptance before checking the whole map.
Load and publication times are recorded separately. No action timing qualification is claimed.

The [bounded-view evidence](evidence/p11-spatializer-bounded-view-2026-10-26.json) records 459 passing
UI tests, 25 owning Spatializer cases, clean app/fixture Svelte checks, formatting and the normal
bundled executable. That exact artifact passes the prior 13 Spatializer / 11 navigation / four
State / five Formula native correctness groups. At that milestone, the full pressure rerun was
still open; small-scene correctness and constructor tests did not close those groups.

The next attempt reached a complete 50k backend matrix in **259,569.1ms**, but accepted Home
still showed only 350 of 1,452 endpoints. The app's 5% zoom floor clamps the fitted view above
the required zoom for the fixed 1,530-unit endpoint span. The
[retained failure](evidence/p11-spatializer-home-failure-2026-10-26.json) closes no pressure groups.
The subsequent repair lowered the app zoom floor to 0.1%; controls retain their screen-space hit
sizes. The 259.6s publication cost also required investigation before stress-tier qualification. No density,
mounting budget, setup deadline or action gate is relaxed.

The [wide-Home repair](evidence/p11-spatializer-wide-home-2026-10-26.json) also removes Golden's
implicit 1% floor for explicitly configured zoom limits. The twelfth native navigation group
verifies every real control inside the viewport across the 1,530-unit span, including the inspector
inset. The exact rebuilt executable passes 34 native groups and 459 UI tests; the shared canvas
browser regression and Svelte checks pass. This focused 16-endpoint reproduction does not qualify
large matrix pressure. After cancelling the redundant third attempt, the fourth complete pressure
run then started. All 16 pressure groups and 60 timing cases were still pending at that milestone.

## Publication baseline and read-view repair

The full run on `c3edd1c8` passes the Voronoi 50k pressure group: all 1,452 endpoints are accounted
for in 24 Home groups; detail mounts 10 glyphs and 19 polygons. The related-value check mounts
17 rows while retaining focused identity and verifying the real backend weight. Matrix publication
takes **232,919.0ms**, with all checks complete in 233,909.9ms. The next 100k group fails the fixed
300s setup deadline. This closes neither the full 16-group pressure selection nor action timing.

A directed, explicitly diagnostic 100k trace confirms repeated whole-tree work on unchanged service
turns. It observes 2,562 process snapshots at or above 10k nodes, accounting for 168.3s of snapshot
construction, and quiet wake turns with p95 103ms. These are instrumented observations, not action
latency distributions. The [baseline evidence](evidence/p11-spatializer-publication-baseline-2026-10-26.json)
retains the original full-run failure and directed trace. `GC_SPATIALIZER_PRESSURE_CASES` can select
fixed scene indices for diagnosis; any explicit selection or `GOLDEN_PERF_TRACE` is marked
diagnostic and cannot substitute for complete qualification.

The current app-owned repair retains one canonically leased read view between read-only service
turns. Inbox delivery, explicit requests, cancellation and queued edits invalidate it; the original
source/path guards still validate every queued write at application. Service and allocation ceilings
remain unchanged. All **38 owning cases**, strict all-target Clippy and the normal bundled build
pass. The exact artifact passes **34 native correctness groups**. The
[read-view evidence](evidence/p11-spatializer-read-view-2026-10-26.json) records the rerun: 50k
publication improves to **126,167.5ms**, but 100k still fails the unchanged 300s setup deadline.
Pressure/action qualification remained open at that milestone; caller attribution followed below.

## Snapshot caller attribution and archive repair

An opt-in caller trace on the same 50k Voronoi scene identifies **1,270 of 1,327** large process
snapshot builds in Mapping's archive-collision settlement check. Snapshot construction accounts
for **79.3s** in that diagnostic run; matrix publication takes 126.8s. The check clones the entire
project even when no Mapping processors exist. This is an app adapter issue reached by unrelated
Spatializer publication, rather than a reason to change Spatializer's fixed service limits.

The repair reads canonical live compression controls, skips inactive archives and uses Golden's
existing public UUID index for collision checks. Snapshot consumers and settlement share the same
typed archive decoder. Archive remapping and pending-edit application keep their original ordering;
the no-processor path still applies pending edits. Qualification must rerun the complete unchanged
16-group selection on the normal bundled executable before taking pressure credit.

All **90 processor regressions** pass, including two new canonical-control/pending-edit cases;
four existing manual cases remain ignored. Core's 635 cases pass (three existing ignored),
strict app/Core all-target Clippy passes, and the normal bundled executable passes all **34 native
correctness groups**. The [archive-read evidence](evidence/p11-spatializer-archive-reads-2026-10-27.json)
retains exact source/binary fingerprints, caller attribution and the full pressure attempt.

That complete selection passes Voronoi 50k with **19,672.2ms** publication and 100k with
**40,946.1ms** publication. The 50k observation improves from the preceding 126,167.5ms; these
single setup observations are not p95 action distributions. The next `(32 sources, 4,096 targets)`
scene fails the unchanged 300s cumulative setup gate. No full 16-group or action qualification is
claimed at that milestone. The endpoint-cap publication failure and full pressure/action
qualification stayed open; the following repairs address those boundaries.

## Endpoint-cap snapshot capacity

A directed cap reproduction now records actual intermediate dimensions and module warnings.
It stops at **119,059 records / 26 value folders / 106,496 cells**. Leasing that view requests
**1,016,862,484 bytes** and fails the canonical shared 1 GiB reservation budget. The retained
failure confirms a capacity stop rather than silently treating the partial matrix as complete.

Golden's base snapshot map now owns boxed records. Its sparse buckets hold small pointers;
retained accounting separately charges every complete record, allocator overhead and owned
metadata. Sparse overrides, other indexes, final background destruction and the 384-allocation /
1 GiB pool keep their existing accounting and admission. The public read API and source-revision
guards consume the same records. No Spatializer service bound or fixed pressure workload changes.

All **636 Core tests** pass (three existing ignored), including a 32-by-4,096 wide-catalog
regression: **131,105 records reserve 303,009,394 bytes** and lease successfully. Its UUID, value,
declaration and child lookups pass. Strict workspace/all-target Clippy and graph-fixture Svelte
checks pass. The normal bundled executable also passes all 34 native correctness groups.
The [storage evidence](evidence/p11-spatializer-snapshot-storage-2026-10-27.json) preserves the
diagnosed capacity stop, source/binary fingerprints, owning checks and full pressure attempt.

The complete, uninstrumented selection passes Voronoi 50k and 100k, then the formerly blocked
`(32 sources, 4,096 targets)` group: **143,641 records / 131,072 cells**, publication in
**58,702.8ms** and all checks complete in **60,281.5ms**. The next transposed scene times out
with **74,539 records / 2,002 value folders / 64,064 cells** and no capacity warning. The small-row
path still submits each completed row separately, rebuilding its read view after each application.
That result established the need for bounded small-row forests before guarded application,
preserving the existing 32-child construction bound, lifetime reservations and source/path fences.
The storage-only milestone did not close full pressure or action qualification.

## Small-row forest publication

The app now retains up to 16 complete small-row trees before one guarded application. It still
constructs at most 32 child parameters per turn, retains each tree's original row reservation,
and applies the captured source and Values topology fences. Changed topology revisits the first
unsubmitted row. Shared row-pool pressure flushes an admitted prefix so partial forests can make
progress without additional slots. Existing wide rows keep their single-tree path.

All **44 owning Spatializer tests** pass, including six new forest/identity/source/path/prefix/
pressure cases. Thirty-five new small rows use three guarded application turns in the regression.
Strict workspace/all-target Clippy, the normal bundled build and all **34 native correctness
groups** pass. The complete pressure rerun now passes the formerly failing Voronoi source cap:
all **143,641 records / 131,072 cells**, with checks complete in **163,941.0ms**.

The [forest-publication evidence](evidence/p11-spatializer-row-forests-2026-10-27.json) records the
complete default selection: **all 16 groups pass**, without diagnostic selection, tracing or CPU
profiling. Every mode passes 50k, 100k and both endpoint-cap orientations, with exact backend
records, complete matrix cells, counted overview membership, bounded SVG/inspector mounting and
retained focus/identity. All original source/path and allocation/service gates remain in force.

| Mode          | 4,096-source records | Matrix cells | Setup and checks |
| ------------- | -------------------: | -----------: | ---------------: |
| Voronoi       |              143,641 |      131,072 |      163,941.0ms |
| Target Radius |              143,641 |      131,072 |      164,623.8ms |
| Source Radius |              147,705 |      131,072 |      171,801.8ms |
| Overlap       |              147,737 |      131,072 |      171,795.9ms |

These are single setup/correctness observations, not action latency distributions. Implement and
run all **60 timing cases / 1,860 retained samples / 180 warmups** next. Pressure is complete;
graph checkpoint closure and composed workflows remain. **P11 stays 4/6, whole plan 59%.**

## Timing runner and retained creation failure

The runner now measures public panel operations against committed backend identities/values,
actual SVG transforms/radii, related-value content, retained focus and drained editor work.
Creation requires all 32 new matrix cells and undoes the operation outside the timed interval,
restoring exact baseline record/folder/cell counts before the next sample. Per-sample progress
and a host watchdog expose stalled observations without changing any timing gate.

All six Voronoi actions and the two actions in each radius mode pass at 1k in diagnostic subsets,
each with 3 warmups and 31 retained samples. The final 12 navigation and 13 authoring native
regression groups pass; graph-fixture Svelte checks report zero errors/warnings and Prettier passes.
The normal executable is unchanged from the complete pressure qualification.

A default full run was interrupted after ten passing cases: six Voronoi 1k and four Voronoi 10k.
Its execution handle is missing and no matching driver/backend process remains. There is no
terminal result, so this run is incomplete rather than a recorded timing failure.

A directed Voronoi 10k creation run completed every sample and undo restoration. **Completion
p95 is 8,931.6ms, exceeding the unchanged 2,000ms gate.** Feedback p95 is 51.8ms and maximum
animation-callback stall is 8.4ms. Identity, all new declarations/cells and rendered focus checks
pass. The [timing evidence](evidence/p11-spatializer-timing-runner-2026-10-27.json) retains raw
samples, interrupted-run status, the corrected finder-selector harness failure, source/binary
fingerprints and report hashes. Diagnostic subsets do not close full qualification.

Run `node apps/chataigne/ui/tests/browser/spatializer-scale-native.mjs` for the complete matrix.
For directed reproduction, set `GC_SPATIALIZER_SCALE_MODES=voronoi`,
`GC_SPATIALIZER_SCALE_TIERS=10000`, and `GC_SPATIALIZER_SCALE_ACTIONS=create`; remove all three
for qualification. Next repair the app-owned preparation/publication boundary: canonical endpoint
and unchanged matrix inspection currently consume repeated engine turns. Keep computation on
the existing worker and apply sparse changes with bounded guards. Rerun the unchanged full matrix
after that repair. **P11 remains 4/6 (67% rounded), whole plan 59%.**

## Sparse publication and keyboard-feedback failure

The worker now identifies noncanonical endpoints and sparse output changes from its leased
immutable snapshot. Canonical endpoints require no normalization scan wakes; unchanged cells
require no publication scan wakes. Existing-row edits share bounded batches across rows, with
at most 16 changed pairs and 32 node read guards. Detached construction keeps its 32-child turn
bound and existing small-row forests. Prepared accounting includes pair-index/removal storage
within the original 128 MiB pool. Values revisions fence worker-plan acceptance; failed receipts
and structure/meta reconciliation capture a new plan against the actual applied prefix.

All **50 owning Spatializer tests** pass, including six new sparse-plan, creation, stale-output
and live endpoint-bound regressions. Strict workspace/all-target Clippy, the normal bundled app
build, app/root/Core formatting and all **25 Spatializer native regression groups** pass.
The repaired executable also passes the complete unchanged **16-group pressure selection**.

The directed 10k creation reproduction now passes at **307.9ms completion p95**, versus the
retained 8,931.6ms failure. The default full timing attempt independently passes that case at
329.6ms, and 20k creation at 982.8ms. All required samples and undo restores complete.

That full timing attempt records **19 passing cases**, then fails case 20: Voronoi 50k
single-target movement has **281.5ms first-feedback p95 against the 200ms gate**. Completion
is 281.5ms and maximum callback stall is 20.8ms, both within their limits. No gate, scene density
or sample count changed. This is a new measured remaining failure, not full timing qualification.
The [sparse-publication evidence](evidence/p11-spatializer-sparse-publication-2026-10-27.json)
retains every sample, source/binary/report fingerprint, all pressure outcomes and machine details.

Next inspect the app-owned keyboard interaction feedback path: relative keyboard edits currently
wait for the backend receipt without the pointer gesture's pending preview. Keep relative-edit
policy in the backend, preserve actual rendered geometry/focus and failure retirement, and rerun
the complete fixed timing matrix after repair. **P11 remains 4/6 (67%), whole plan 59%.**

## Temporary command feedback and 50k creation

Keyboard nudges/radius changes and atomic moves now use temporary geometry hints. Backend intents
still own committed edits and relative-value resolution. Acknowledged publication boundaries keep
confirmed hints until actual graph data arrives; rejection, retired endpoints and owner/unmount
changes withdraw them. Earlier responses cannot remove newer hints for the same endpoint.

All **460 owning UI tests**, app/fixture Svelte checks, Prettier, root/Core formatting and the rebuilt
normal bundled executable pass. All **29 native Spatializer groups** pass, including four held-command
checks for real DTO/preview separation, repeated input, radius feedback, unmounting and actual
backend endpoint-deletion rejection. The [command-feedback evidence](evidence/p11-spatializer-command-feedback-2026-10-27.json)
retains exact source/binary/asset/report fingerprints and every measured sample.

The complete default timing attempt passes **22 cases**. At 50k, single-move feedback p95 is **5.8ms**
(previously 281.5ms), atomic movement 4.8ms and radius editing 5.4ms. All required completion,
geometry/focus and mounting checks pass. The following creation case fails: **344.1ms feedback
and 2,647.8ms completion p95**, versus unchanged 200ms / 2,000ms gates; maximum callback stall is
29.2ms. All 34 samples and undo restorations complete. The full matrix remains unqualified.

Next remove repeated same-row guards within each sparse planning turn and provide truthful pending
creation feedback through the existing status surface. Keep source/path guards, 16-pair and 32-read
bounds, real new identities and all 32 cells for completion. Then rerun the complete timing matrix
and pressure on the final artifact before closure. **P11 stays 4/6 (67%), whole plan 59%.**

## Creation publication and retained UI projection

Sparse publication now captures each row guard once per immutable planning turn. A new 32-cell
column uses exactly two bounded application batches, retaining the existing pair/read/construction
ceilings. Worker preparation also captures generated output identities. During active publication,
known output echoes need no full-tree inbox snapshot; ownership changes invalidate this catalog,
and unknown fields retain ordinary callback processing. An already supplied inbox snapshot may be
canonically leased only when the callback queued no edits. Apply-time source/path fences remain.

These changes and truthful pending creation status repair feedback but do not alone meet completion:
the guard-only diagnostic records **3,120.4ms** completion p95 and echo-catalog diagnostic records
**2,998.2ms**. Both retain all 34 samples and exceed the unchanged 2,000ms gate.

A CPU profile around the actual fixed 3/31 creation measurement identifies repeated full-project
module-picker iteration on matrix publications. The app now retains a module catalog from immutable
map deltas. Its directed case passes at 1,938.2ms, but the subsequent complete matrix again stops
after 22 passing cases: **50k creation completion is 2,219.7ms p95**, with 11.7ms feedback and 25.2ms
maximum callback stall. The diagnostic pass was insufficient evidence of the full workload.

The app subsequently retains endpoint presentation identities when their input DTOs and palette
color are unchanged, preserving geometry/navigation on unrelated matrix publications. The endpoint
cache diagnostic passes at **1,831.2ms completion / 12.1ms feedback p95**, with 12.6ms maximum stall.
The declaration index also extends unchanged membership prefixes/suffixes without rebuilding all
aliases, preserving first-child precedence and invalidating on unsupported changes or missing records.
These focused diagnostics still do not close the complete timing matrix or final-artifact pressure.

All **53 owning Rust tests**, strict workspace/all-target Clippy, **469 UI tests**, app/fixture Svelte
checks and the normal bundled build pass. The final native preservation run first exposed an undo
test ordering gap: its receipt can precede publication of the restored endpoint. The harness now
waits for that specific identity and asserts reveal acceptance before checking its actual glyph.
The original failed report is retained. All **30 native preservation groups** subsequently pass.

The final full default timing attempt passes **28 cases**, including every Voronoi case through
50k. Creation at 50k passes with **1,991.7ms completion / 12.2ms feedback p95** and 16.6ms maximum
callback stall, narrowly within the completion gate. At 100k, discovery, both movements and radius
editing pass; creation fails at **5,124ms completion p95**, versus the unchanged 4,000ms stress gate.
Feedback is 27ms and maximum callback stall 37.5ms. Both creation cases finish all 34 samples and
undo restorations. No full timing qualification or new P11 checkpoint is claimed.

The complete unchanged **16-group pressure selection passes on this final artifact**, including
all four modes, both backend tiers and both endpoint-cap orientations. The
[creation-projection evidence](evidence/p11-spatializer-creation-projection-2026-10-27.json) retains
source/binary/asset/report fingerprints, every timing sample, pressure outcomes and native failure/recovery.

A separate single-operation 100k CPU/snapshot trace uses the same actual measurement observer.
The browser spends substantial time idle while backend worker reads and subtree precondition
validation build full-project snapshots. Its snapshot totals also include setup and undo, and its
instrumented observation is not a p95 distribution. Next investigate scoped subtree-fence validation
at the Golden engine boundary while preserving exact revision semantics and application-time rejection.

The initial exploratory trace polled a fixture helper that scans the complete graph; its wall time
includes observer overhead and is not production timing evidence. The separate fixed-sample CPU
profile uses the actual measurement observer. Neither instrumented run qualifies latency gates.
