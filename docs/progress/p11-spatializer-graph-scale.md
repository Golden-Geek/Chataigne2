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

Custom SVG and related-value presentation still need bounded views. Implement those at their
owning UI layers, preserving existing backend intents and module admission/publication. All
**60 timing cases and 16 pressure groups remain pending**; no checkpoint closure is claimed yet.
