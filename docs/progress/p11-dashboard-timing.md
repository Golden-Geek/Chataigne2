# P11 dashboard action qualification

The [fixed contract](p11-dashboard-timing-contract.md) defines 32 action/profile combinations at
five authored-node tiers, each with three warmups and 31 retained samples. The
[final qualification](evidence/p11-dashboard-timing-final-matrix-2026-10-26.json) passes all 160
combinations and preserves all 599 correctness groups. The dashboard checkpoint is complete locally;
P11 is 3/6 (50%) and whole-plan coverage is 58%. Earlier failed matrices remain below.

## Qualified result

All 4,960 samples and 480 warmups are retained. Workloads, fixture counts, correctness labels,
measurement/action code and limits match the third failed matrix. No profiling or explicit
collection runs in qualification. Each row below reports the worst action-level p95 and callback
maximum among that tier's 32 combinations; hierarchy minimums retain their actual setup counts.

| Authored target | Feedback p95 max (ms) | Completion p95 max (ms) | Callback p95 max (ms) | Callback max (ms) |
| --------------- | --------------------: | ----------------------: | --------------------: | ----------------: |
| 1k              |                 117.4 |                   382.0 |                  70.9 |             154.1 |
| 10k             |                 111.9 |                   383.0 |                  87.5 |             120.9 |
| 20k             |                 103.8 |                   386.6 |                 133.4 |             154.2 |
| 50k             |                 118.2 |                   384.5 |                  70.8 |             125.0 |
| 100k stress     |                 244.1 |                   475.3 |                  75.0 |             241.7 |

All original geometry, presentation and timing gates pass. Source fingerprints match for all 147
owning files, with no page errors. Three further P11 checkpoints remain: tree keyboard, graph
keyboard and composed workflows. Native usability retains P17 and product throughput P16.

## Scope and measurement

The production dashboard runs in an owned headless browser at 1680 x 1200. Sparse edits/navigation,
dense edit/run selection and exact picking, minimum-zoom edit/run views, six mixed parent layouts
and depth-eight framing use the existing fixtures. Counts include structural/parameter nodes;
100k authored nodes correspond to about 9k leaf widgets here. Hierarchy minimums exceed the small
baseline tiers and are reported explicitly. Setup and per-sample view/selection preparation are
outside action latency. No synthetic input or desktop control is used.

`dashboard-timing.ts` invokes actual component, selection, camera and edit operations. The focused
`dashboard-measure.ts` records correct rendered feedback, operation completion, acknowledged source
geometry, animation settlement, two further stable frames, callback intervals and presentation peaks.
Navigation feedback also requires scene content to intersect the viewport. The camera exposes its
view/animation state. Surface counters classify each mounted child once: ordinary detail first,
then additional frame, interaction, selection and fallback reasons. Total and per-cohort peaks are
reported independently. These diagnostics do not mutate authored state.

Timing gates remain 200ms feedback p95 / 200ms maximum callback stall / 2,000ms completion p95
for normal tiers, and 400ms / 400ms / 4,000ms at stress. Callback p95 is reported separately.
Existing geometry, mounting, hierarchy and browser-row budgets remain fixed. This measures the
specified UI/controlled-receipt boundary, not backend throughput, physical input or a 60-FPS target.

## Observed failures and corrections

The [initial 50k probe](evidence/p11-dashboard-timing-initial-probe-2026-10-25.json) retains all 32
combinations and their raw results. Twenty-nine pass. Rectangle selection used page coordinates for
a client-coordinate public API; the harness now projects its known rectangle through the actual
surface transform. This was a fixture failure.

Grid discovery and Frame exposed a runtime defect. The
[focused reproduction](evidence/p11-dashboard-grid-regression-2026-10-25.json) records a 48-row
layout with zero-height tracks, a distant selected child and no local scrolling. Large gaps exhausted
the constrained height while `minmax(0, 1fr)` collapsed the controls. Grid surfaces now scroll on both
axes and retain a uniform `2em` track floor, including omitted rows. This relative presentation
minimum preserves authored cell indices without consulting mounted content or changing node data.
The same child-discovery and Frame paths reveal the selected cell. Focused discovery then passes.

The shared selection store also used quadratic array membership for duplicate validation, merge,
remove and selected-state lookup. Ordered set membership now preserves first occurrence/order and
all selection modes with linear batch work. Adjacent app integration tests cover all modes, a
duplicate-rich 10k selection in a 20k graph, reset and node retirement. Shared graph test fixtures
consume the public Golden protocol/types boundary.

The [first complete matrix](evidence/p11-dashboard-timing-first-matrix-2026-10-25.json) passed
145/160 combinations. Eight mixed-layout combinations failed actual visibility/centering, while
seven sparse navigation/edit combinations exceeded timing gates. Every tier's raw warmups,
samples and failure diagnostics remain linked from that report; none are discarded.

Small tab containers wrapped headers until the body lay below their clip. At intermediate tiers,
camera zoom admitted complete tab/accordion content midway through Frame, displacing a selected
control after its earlier reveal. Tab headers now have one horizontally scrollable row with stable
relative height, and the body uses the remaining space. Non-free content repeats local reveal when
its rendered cohort changes. Frame prepares non-free ancestry even when a clipped shell already
exists, before measuring its actual visible bounds. The focused 10k mixed-layout probe then passes
all twelve discovery/Frame combinations.

The next [combined preflight](evidence/p11-dashboard-timing-cohort-regression-2026-10-25.json)
stopped after 453 passing groups: reveal reacted to new node-object arrays during ordinary value
publication, scrolled an actively resized container and correctly caused its coordinate-frame guard
to reject further movement. Reveal now depends on the mounted child-ID cohort instead of refreshed
node objects. The complete 164-group composition suite then passes again. This failure retains its
evidence and receives no completion credit.

The [100k resize CPU probe](evidence/p11-dashboard-resize-cpu-before-2026-10-25.json) located the
dominant repeated cost in parent-layout parameter lookup: about 79% of sampled time descended
through local parameter discovery into sibling enumeration. Each visible widget mapped all 9,090
siblings before excluding owned item subtrees. The shared graph projection now publishes a
structural-child index. Local declaration queries traverse only that index, preserving exact-name
precedence and basename fallback. Scalar edits preserve index identity; structural transactions
classify children within staged work, while appends reuse the existing prefix. Unit tests cover
10k-sibling bounded lookup, roles, materialization, order, movement, removal, reset, stale batch work
and bounded staged publication. The existing 7k-parent append test also remains a preservation gate.
The focused unprofiled 100k resize probe passes: feedback p95 25.1ms, completion p95 54.5ms and maximum
callback interval 50.5ms. Profiling adds overhead and serves diagnosis, not timing qualification.

The [second complete matrix](evidence/p11-dashboard-timing-second-matrix-2026-10-25.json) preserves
all 599 correctness groups and five sparse tiers, with 155/160 timing combinations passing.
Four fail maximum callback stalls (1k Home, 10k deep Frame, 20k vertical Frame and 50k mixed free
Frame). One 20k accordion Frame fails the original 0.5-CSS-pixel centering check after its camera
budget and fractional local scrolling settle. This is retained as a failed run, not partial closure.
Frame now waits for its animation, remeasures prepared targets under their original identity/path
fences, and corrects the center while preserving zoom. Newer navigation cancels the refinement.
The focused 20k accordion probe then passes 31 retained samples with feedback p95 23ms,
completion p95 329.4ms and maximum callback interval 116.7ms.

The [deep Frame CPU probe](evidence/p11-dashboard-deep-cpu-before-2026-10-25.json) identifies repeated
nested viewport/ancestor DOM measurements as a leading cost. The page now shares a measurement
snapshot within each synchronous presentation batch; the next microtask releases it, with explicit
page/disposal clearing. This keeps existing invalidation and publication timing while avoiding
repeated ancestor reads. Adjacent tests cover 512 shared reads, captured style values, next-batch
refresh, page isolation and retirement. Structural indexing also omits empty lists: the 10k-sibling
unit fixture retains only two index entries. This reduces index ownership without claiming a measured
heap or GC improvement. The focused unprofiled deep Frame probe then passes: feedback p95 105.3ms,
completion p95 375.8ms and maximum callback interval 78.9ms. At this point full requalification remained.

## Validation status

The final projection/layout runtime again passes all preceding **599 correctness groups / five
sparse tiers**. The corrected UI suites pass **379** cases: audio 27 / graph 52 / Golden UI 175 /
app 125. Browser/app Svelte checks pass. The locked default app build passes
in **3m20s**, reusing the unchanged production frontend bundle, executable SHA-256
`91321cac730eeb07d6478d1e6617dff2947bb4390a8722d6af774e3f493f117a`.
The [third complete matrix](evidence/p11-dashboard-timing-third-matrix-2026-10-25.json) passes
154/160 combinations with all 4,960 samples and 480 warmups retained. All 599 correctness groups,
five sparse tiers, geometry, feedback p95 and completion p95 gates pass. Six maximum callback-stall
gates fail: 10k mixed grid/tab Frame, 20k mixed free/accordion Frame, 50k sparse Frame and 50k
accordion discovery. The largest recorded interval is 666.7ms. Source fingerprints match for all
147 owning files, and no page errors occur. At this stage P11 remained 2/6 (33% rounded), whole plan 57%.

The [owned-process observation](evidence/p11-dashboard-renderer-memory-observation-2026-10-25.json)
records a 3,327.4MiB browser process working set during this cumulative sequence. This does not
measure live JavaScript heap or establish a GC cause or leak. It motivated the live/retained-heap
audit below before further runtime or measurement changes. Diagnostic collection and isolated probes
do not substitute for the ordinary qualifying matrix.
This failed state is superseded by the qualified result above after the ownership repair below.

## Retention diagnosis and harness repair

The [prefix heap probe](evidence/p11-dashboard-heap-prefix-before-2026-10-25.json) replays all
599 correctness groups, loads the 1k-node / 90-widget fixture, and records about 3.15GB before
explicit diagnostic collection and 542MB afterwards. The same fixture in a
[fresh renderer](evidence/p11-dashboard-heap-fresh-reference-2026-10-25.json) retains 9.7MB.
The [staged probe](evidence/p11-dashboard-retention-before-2026-10-25.json) locates retained jumps
after viewport, dense, hierarchy and preview-lifecycle checks. These are diagnostic collections;
none qualify action latency.

The harness retained removed controls through undisposed Playwright handles and two window globals
used for identity assertions. Scoped handles now use `finally` disposal after the same assertions.
The [handles-only replay](evidence/p11-dashboard-retention-handles-only-2026-10-25.json) reduces
collected heap after dense from 356MB to 12.8MB and after hierarchy from 374MB to 13.2MB. The initial
cleanup attempt exposed a block-scoped metadata variable; that
[harness error](evidence/p11-dashboard-retention-cleanup-scope-error-2026-10-25.json) is retained and fixed.

The fixture's preview helper also outlived its source. Clearing its retained ancestor decorations
reduces the remaining retained graph, but its unused lazy derived dependencies still hold the old
source. It now has a per-fixture `$effect.root` lifetime, with explicit decoration/publication cleanup
and root disposal before fixture replacement. This mirrors widget lifetime instead of retaining a
preview controller across unrelated graphs.

The [complete repaired probe](evidence/p11-dashboard-retention-after-2026-10-25.json) passes all
13 ownership checkpoints and all 599 correctness groups. Collected heap peaks at 14,760,180 bytes,
below the fixed 64MiB diagnostic ceiling. Listeners peak at 178 (limit 196). The resident view stays
at 18 widget shells and 353 connected elements. Total native-node counts include Svelte's lazily
cached template fragments (`from_html`), so an initially proposed total-node delta was rejected;
the actual widget limit, heap and listener bounds remain explicit. The verifier rejects the old
retention evidence and accepts the repaired evidence. The original cumulative 160-case timing
qualification then passes without explicit collection; its gates and workload are unchanged.
Keep the full P00–P18 goal active.
