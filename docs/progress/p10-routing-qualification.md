# P10 scoped routing and culling qualification

Golden Graph UI owns world-unit obstacle/edge geometry, viewport culling and route scheduling.
Backend graph editing semantics, Rust wire DTOs and app-owned presentation mapping are unchanged.
`retained-edges.ts` consumes geometry deltas from `RetainedGraphPresentation`; `edge-routing.ts`
owns bounded search/smoothing, and `wire-geometry.ts` owns conservative path bounds and SVG output.
The canvas composes these owners and schedules pending routing slices on animation frames.

## Correctness and bounded work

Node geometry deltas update individual obstacle entries and incident edge envelopes. Cached routes
depend on their bounded search regions: insertion, movement and removal invalidate routes whose
search could change, including an earlier failed search. Independent ranges preserve cached paths.
Source replacement, removals, endpoint changes, authored order and loops retain correct identity.
Unsafe integer ranges and routes exceeding the search-distance budget use curves without scanning
obstacles. Frame/cache work stops once the current queue is settled.

Viewport queries use conservative curve/search envelopes, followed by actual route/curve bounds.
Both endpoints may be offscreen. Bounds are conservative and can retain extra elements; they do
not discard a wire crossing the view. Exterior self-loops remain visible when search is unavailable.
Grid steps detect thin obstacles between sample points, and corner guards prevent rounding from
cutting indexed obstacles. Missing paths or budget exhaustion use curves, which may cross nodes.

The configured limits are:

| Resource | Limit |
| --- | ---: |
| Routed edges admitted in current view / cached routes | 1,024 |
| Jobs per animation-frame slice | 4 |
| Search visits per slice / per job | 4,096 / 2,048 |
| Shortcut smoothing obstacle checks per job | 512 |
| Path points per job | 256 |
| Obstacles per job / candidate visits per obstacle query | 128 / 512 |

Corner guards have an additional bound of path-points times obstacle-count comparisons. Limited
queries count coarse false positives and report incompleteness; incomplete queries use curves.
Overflow edges remain rendered as curves. Admission follows document order, and eviction prefers
entries outside that set so overflow cannot create an endless eviction/requeue cycle. Broad
ordinary spatial queries may still scan retained entries; this is not a constant-time renderer.

## Evidence and reproduction

Run `npm test` and `npm run test:browser --workspace golden_graph_ui`. The latter owns an isolated
headless browser/server and closes both. It changes fixture data and calls component methods,
without synthesizing input events or touching the user's desktop.

The [raw artifact](evidence/p10-scoped-routing-2026-10-25.json), based on `dc21c067`, retains **155
preview samples**, routing/resize/activity probes, correctness outcomes, overflow work and changed
source hashes. Reference machine: Windows 11 Home 10.0.26300, Intel Core Ultra 9 275HX / 24 logical
processors, Node 26.5.0, npm 11.17.0, headless Edge 156.0.4314.42.

- Unit suites pass **46 graph UI cases**, including fixed endpoint work at 1k/100k authored nodes
  and chain edges, search-region invalidation, preserved distant routes, edge order, changed ports,
  producer replacement, loops, capped queries, thin obstacles and bounded queue/cache settlement.
- Mounted cases verify crossing wires with **zero mounted endpoint nodes**, using actual SVG
  `isPointInStroke` checks inside the viewport. Routing mode changes render correctly without source
  edits. Moving/removing an obstacle invalidates one route and preserves the distant path; loop
  bounds remain outside the node. Empty scenes retire the paths.
- At 1k/10k/20k/50k/100k authored nodes (N-1 chain edges), the 10rem x 10rem fixture mounts one node
  and two edge elements. The added element is a row-wrap wire retained by conservative bounds;
  endpoint-only culling previously omitted it. Each move-preview frame updates one projected/spatial
  entry. Ending preview schedules **one route job**, with **zero full-node materialization**.
- A separate 1,100-wire mounted scene admits 1,024 cached routes, keeps overflow curves visible,
  settles its queue, and preserves routing counters over ten subsequent animation frames. The
  cumulative observed maxima are four jobs, 657 search visits and 16 shortcut obstacle checks per
  slice, below their limits. Unit cases also verify bounded cache/queue progress without churn.

Preview DOM/layout-flush p95 is **1.1 / 0.9 / 0.8 / 1.0 / 0.6ms** across the five tiers; callback
interval p95 is **4.3ms**. These have the same narrow definitions as the
[first-checkpoint report](p10-retained-presentation.md): a sparse visible scene, not input delivery,
compositor paint, display FPS or full workbench responsiveness.

The 1,100-wire scene takes **8,053ms** in its single Node-side wall-clock route-completion sample,
including browser messaging and scene setup. That is **not** first-visible-feedback latency, a
percentile, or successful dense-scene performance qualification. Curves are available while routing
is pending. The final P10 checkpoint must profile dense settlement/rendering and qualify the full
mounted drag, pan/zoom, framing, activity, crossing, loop and changing-obstacle action/frame matrix
at normal/stress tiers. These bounded-work and correctness results close only the routing/culling
checkpoint.
