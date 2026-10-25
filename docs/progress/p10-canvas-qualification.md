# P10 mounted canvas qualification

The generic canvas owns presentation, camera mechanics, retained geometry, routing and DOM
membership in `packages/golden-graph-ui`. App-owned adapters and backend edit/undo semantics keep
their existing owners. The [fixed qualification contract](p10-canvas-qualification-contract.md)
defines this workload and its measurement boundaries.

## Implementation and tradeoffs

`GraphWire` consumes stable per-edge snapshots; completing one route updates that wire rather
than every mounted SVG path. Equivalent input edge allocations retain their published identities.
Clear segments skip grid search. Route slices retain hard job/visit/check caps with a cooperative
4ms target. A view also has a total 32,768-visit / 65,536-shortcut-check allowance, replenished by
geometry/topology or admission changes. Exhaustion caches curves, with search-region dependencies
preserved for later obstacle invalidation. All 1,100 wires remain rendered despite the 1,024-route
cache cap. These are presentation quality/work limits, not graph edits.

Camera scaling uses a world transform and direct background styles. Distant unselected nodes
(zoom below 0.4) and camera animations use lightweight overview boxes; selection/rename restores
full node controls and app snippets. Node identities, geometry, status, selection and wire hit
targets remain intact. This does not write authored collapse state. Wire color/opacity/width carry
status without per-path GPU filters.
Stable SVG groups of at most 32 authored edge indices isolate paint invalidation. Each group uses
its own world-positioned bounds/viewBox and compositing hint, preserving path/gradient coordinates.
This retains DOM/SVG editing and prevents one route update from repainting the entire dense view.

`RetainedMounts` applies existing payload changes immediately and reconciles new/removed DOM
membership in animation-frame batches of 32 nodes and 64 wires. Targets coalesce during navigation;
the complete visible membership remains required at settlement. This trades a longer bulk Home
completion for bounded frame work. The fixture waits for camera animation, mounting and route
queues, so deferred rendering cannot disappear from its completion measurement. Drivers stop at
idle and cancel on destruction.

## Workload and reproduction

Run `npm run test:qualify --workspace golden_graph_ui`. The runner builds the actual Svelte
canvas fixture for production, serves only loopback, launches an owned headless browser, and closes
both resources in `finally`. Timestamped JSONL under `target/qualification/` retains source/bundle
hashes, machine/browser details, setup durations, warmups, per-action samples, mounted/overview
counts, callback intervals, limits and failures. It performs no desktop or synthesized input.

There are eleven actions at each of 1k/10k/20k/50k/100k authored nodes: selection, single/multi
move preview and source-acknowledged commit, pan, anchored zoom, Frame Selection, Home, activity,
payload, self-loop movement and changing obstacles. Each has three warmups and 31 measured samples.
The normal view initially mounts 84 nodes; stress zoom 0.5 mounts 308. Home exposes substantially
more. Parallel and varied 1,100-wire scenes also have three warmups and 31 measured samples,
including cold mounting, routing and overflow settlement. The varied scene has 28 nodes and 576
distinct endpoint pairs. No source geometry, viewport density or gate was reduced after failure.

Feedback ends after a verified DOM/style/geometry update; intervals are actual animation callbacks.
Completion additionally requires expected source/camera state and empty queues. This is not
compositor FPS, physical input delivery, native webview or whole-app qualification. P08/P09 own
backend transactions/history, P16 owns composed-product scale and P17 owns native interaction.

## Preserved failures

- The delivered second-checkpoint smoke case took 8,053ms for 1,100 parallel wires. Separate
  production diagnostics confirmed about 9,353ms; the profiled development run took 10,825ms
  including profiler overhead. These were single diagnostics, not feedback/frame p95 samples.
- [Recovered camera baseline](evidence/p10-camera-baseline-2026-10-25.json) preserves 50 complete
  action records from the first production run. Initial 1k records were truncated by tool capture;
  the artifact explicitly makes no complete-matrix claim. Home callback p95 reached 750.2ms and
  50k Frame Selection reached 87.5ms, with a 404.1ms maximum callback interval.
- [Transform-only run](evidence/p10-camera-transform-2026-10-25.jsonl) retains the complete action
  matrix and parallel-wire samples. Zoom improved, but Home/frame stalls remained; the varied
  routing scene exceeded its 10-second observation timeout before completing its first warmup.
- [Overview/budget run](evidence/p10-overview-baseline-2026-10-25.jsonl) retains the following full
  matrix. Dense completion maxima were 683ms parallel and 1,310.5ms varied, but their callback
  pacing was still poor. Larger Home views failed callback gates despite faster completion.
  Those dense records gated completion only; the final runner also enforces normal feedback and
  callback limits for dense scenes. Earlier failures are never recast as qualification passes.
- [Bounded-mount run](evidence/p10-mounted-baseline-2026-10-25.jsonl) passes every ordinary tier,
  but varied-wire callback p95 remains 108.4ms. A production CPU profile has 787 idle samples out
  of 992 across 1,533ms, pointing beyond JavaScript execution. Isolating SVG paint groups reduces
  a subsequent warm diagnostic's intervals to about 8-12.5ms; that short probe is diagnostic only.

## Final production result

The [complete production run](evidence/p10-canvas-qualification-2026-10-25.jsonl) passes all
55 action/tier combinations and both dense scenes: **1,767 measured samples plus 171 warmups**.
The reference machine is Windows 10.0.26300, Intel Core Ultra 9 275HX, 24 logical processors,
with headless Edge 156.0.4314.42 and a 1680 x 1200 browser viewport. Source and production-bundle
hashes are retained in the artifact. One later fixture formatting change has an
[identical parsed TypeScript token tree](evidence/p10-fixture-format-equivalence-2026-10-25.json);
runtime source hashes remain exact.

Each p95 column below is the **largest per-action p95** among the tier's eleven actions, not a
pooled percentile. Maximum interval includes all retained callback tails. All values are ms.

| Authored nodes | Feedback p95 | Completion p95 | Callback p95 | Max callback | Largest mounted node view |
| -------------: | -----------: | -------------: | -----------: | -----------: | ------------------------: |
|          1,000 |         10.2 |          295.1 |         12.4 |         16.7 |                       516 |
|         10,000 |         10.0 |        1,022.4 |         16.8 |         37.4 |                     1,749 |
|         20,000 |         12.9 |        1,072.5 |         20.8 |         33.4 |                     1,749 |
|         50,000 |         19.9 |        1,136.6 |         20.8 |         37.4 |                     1,782 |
| 100,000 stress |         39.5 |        1,377.3 |         33.4 |         58.3 |                     1,749 |

Home retains the same node/edge membership as the failed baseline: 516/515 at 1k,
1,749/1,750 at 10k/20k/100k and 1,782/1,750 at 50k. Its final nodes use overview presentation;
all identities remain mounted. Normal starting views have 22 paint groups, stress 42 and Home
up to 107. These group counts are SVG paint surfaces, not measured GPU-layer counts.

| Dense scene                     | Feedback p95 | Completion p95 | Max completion | Callback p95 | Max callback |
| ------------------------------- | -----------: | -------------: | -------------: | -----------: | -----------: |
| Parallel, 2 nodes / 1,100 edges |         10.7 |          107.9 |          109.5 |          8.4 |         12.7 |
| Varied, 28 nodes / 1,100 edges  |         25.4 |          331.2 |          427.9 |         16.7 |         87.5 |

Both dense views settle all 1,100 mounted paths in 35 paint groups and retain 1,024 cache entries,
including fallback curves. The varied sample uses 32,716 search visits, within the total view cap.
This does not claim that every dense path avoids obstacles: budget-limited curves are intentional.
The separate crossing/loop/changing-obstacle correctness oracles pass at every tier. Owning
browser tests additionally verify SVG group coordinate alignment, preserved gradients/snippets,
selected/detail restoration and stable idle counters.

## Owning delivery checks

All **255 UI tests** pass (27 audio, 52 graph, 61 Golden UI, 115 app), Svelte reports **zero
errors / zero warnings**, and the mounted browser correctness suite passes, including actual
SVG group/world coordinate alignment. Root/Core Cargo formatting, source Prettier and whitespace
checks pass. The default locked app build passes in **3m44s**, with the frontend bundle rebuilt.
Its executable SHA-256 is
`6a450ed355708bf80e3f063fa60301c3b95b16b3ef0b35e90ccdf7acfbfa7451`.

All 47 other qualification source hashes still match; the one fixture formatting equivalence is
linked above. The retained qualification JSONL SHA-256 is
`bd04ab64493d5441dff895eeed69984c06103c0d2039df39ee798535c53467a6`.
Rust runtime and wire declarations are unchanged. P10's three local checkpoints are closed;
the next plan task is P11. The full P00-P18 goal remains active.
