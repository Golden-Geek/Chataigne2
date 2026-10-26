# P11 actual-app Spatializer scale

The Chataigne app owns Spatializer endpoint semantics, geometry, inspector and custom map.
Golden owns the reusable canvas, navigation and session contracts. The qualification fixture
loads canonical project records through the actual default executable and authenticated
transport. It must exercise production operations and rendered content, rather than fabricate
a UI protocol snapshot. Output evaluation throughput retains P16; physical input/native webview
usability retains P17.

**The workload and calibration are fixed; action qualification is pending.** P11 remains
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

## Current evidence and next implementation boundary

Small canonical roundtrips establish declaration counts and preserve authored identities/positions.
All **20 roundtrips** pass; the [preparation evidence](evidence/p11-spatializer-scale-preparation-2026-10-26.json)
links the four mode-specific records, canonical export provenance and source/asset fingerprints.
The existing **13 Spatializer native groups**, both four-action **1k State/Formula smoke tiers**,
strict graph-fixture Svelte checks, Prettier and normal bundled build pass. The smoke tiers do
not repeat the prior complete State/Formula qualifications. Constructor-only checks also reject
five invalid endpoint/pair inputs and preserve unique positions in all eight cap-shaped inputs;
those cap scenes have not been loaded for UI qualification yet.
The custom map deliberately gives the shared canvas an empty graph document. The app now supplies
its complete endpoint ID/label catalog through Golden's public `GraphWorldNavigation` contract,
so the shared 32-row finder, reveal and selection framing work with actual SVG controls. Eleven
small-scene native correctness groups cover discovery, focus, keyboard edits, selection framing,
mode-control replacement, deletion/undo and session/project/space/unmount fences. The mode-control
check retained a failing run before the focus-restoration repair. This is navigation correctness,
not large-scene mounting or action timing qualification; see the
[navigation evidence](evidence/p11-spatializer-navigation-2026-10-26.json).

The bounded custom SVG and related-value implementation is described below. Existing backend
intents and module admission/publication are preserved. All **60 timing cases and 16 pressure
groups remain pending**; no checkpoint closure is claimed yet.

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
State / five Formula native correctness groups. The full pressure rerun remains in progress;
small-scene correctness and constructor tests do not close those groups.

The next attempt reached a complete 50k backend matrix in **259,569.1ms**, but accepted Home
still showed only 350 of 1,452 endpoints. The app's 5% zoom floor clamps the fitted view above
the required zoom for the fixed 1,530-unit endpoint span. The
[retained failure](evidence/p11-spatializer-home-failure-2026-10-26.json) closes no pressure groups.
The app zoom floor is being lowered to 0.1%; controls keep their screen-space hit sizes. The
259.6s publication cost also needs investigation before stress-tier qualification. No density,
mounting budget, setup deadline or action gate is relaxed.

The [wide-Home repair](evidence/p11-spatializer-wide-home-2026-10-26.json) also removes Golden's
implicit 1% floor for explicitly configured zoom limits. The twelfth native navigation group
verifies every real control inside the viewport across the 1,530-unit span, including the inspector
inset. The exact rebuilt executable passes 34 native groups and 459 UI tests; the shared canvas
browser regression and Svelte checks pass. This focused 16-endpoint reproduction does not qualify
large matrix pressure. After cancelling the redundant third attempt, the fourth complete pressure
run is live. All 16 pressure groups and 60 timing cases remain pending.

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
Full pressure/action qualification remains open; caller attribution is the next diagnostic step.
