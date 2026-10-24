# P10 retained presentation qualification

This records the first-checkpoint capture based on `23bef838`. The routing limitations below
describe that capture; subsequent [scoped routing work](p10-routing-qualification.md) has its own
evidence. The [current ledger](implementation-status.md) owns execution state.

Golden Graph UI owns presentation invalidation and retained geometry. App-owned adapters supply
domain-neutral node/edge views; backend editing semantics and Rust wire revisions are unchanged.
See [package ownership](../../packages/golden-graph-ui/README.md) and the
[current task ledger](implementation-status.md#p10-completion-checkpoints).

## Implemented boundary

`GraphPresentationAdapter` publishes independent topology, geometry, payload, activity and preview
revisions, sequential record deltas, authored order and a complete recovery snapshot. Equivalent
allocations preserve the presentation document. Array classification remains linear when the
producer replaces an input array; this is not a sparse backend transport or a constant-time
activity adapter. Exhaustive field policies require new view fields to choose invalidation planes.
Alchemist/Spatializer consume the public Golden adapter; the app statechart boundary maps domain
vocabulary into it. Preview chips remain app-owned and provide a changing preview token.

`RetainedGraphPresentation` retains identity maps, spatial membership and authored query order.
Sparse previews touch affected entries; payload/activity replace values without moving cells.
Skipped deltas recover in linear time from the complete snapshot; replacement adapter sources
reset retained state. Selection and socket snapping query spatial state, and Frame Selection
looks up selected identities directly. Home intentionally visits the complete document.

Immutable overlay records use `$state.raw`. An initial mounted run exposed hidden document-sized
work despite constant projection counters: Svelte deep proxies remembered absent node-key reads,
and later key enumeration walked those subscriptions. A headless CPU profile located that cost.
Raw immutable records remove it at the state owner; no proxy/runtime workaround is added.

## Reproduction and reference machine

Run `npm run test:browser --workspace golden_graph_ui` from the repository root. The runner owns
an isolated Vite server and headless browser, then closes both. It changes fixture data and calls
public component methods without generating input events. Unit checks run through `npm test`.

Recorded on Windows 11 Home 10.0.26300, Intel Core Ultra 9 275HX, 24 logical processors,
Node 26.5.0, npm 11.17.0 and headless Edge 156.0.4314.42. The fixture uses an 800 x 600 CSS-pixel
browser viewport and a 10rem x 10rem canvas. This is a local development renderer, not a native
webview, production-bundle benchmark or CI hardware reference.

The [raw artifact](evidence/p10-retained-preview-2026-10-25.json) retains all **155 measured frames**,
activity/preview/resize probes, environment and changed-source SHA-256 hashes, based on `23bef838`.
Each tier has three excluded warmups and 31 measured frames. Nodes form a spaced grid with a
chain of N-1 edges. **One node and one edge are mounted at every tier**, verified from actual
DOM elements. Routing is enabled; active previews use the bounded curve-preview path.

## Results and limits

Nearest-rank p95 values, rounded to 0.1ms:

| Authored nodes | Authored edges | Preview DOM/layout flush p95 | Animation callback interval p95 | Activity classification/flush, one sample |
| ---: | ---: | ---: | ---: | ---: |
| 1,000 | 999 | 0.8ms | 4.3ms | 0.8ms |
| 10,000 | 9,999 | 0.7ms | 4.3ms | 1.7ms |
| 20,000 | 19,999 | 0.8ms | 4.3ms | 2.1ms |
| 50,000 | 49,999 | 0.6ms | 4.3ms | 4.1ms |
| 100,000 | 99,999 | 0.6ms | 4.3ms | 15.6ms |

Every measured preview frame applies **zero source records, one projected record, one geometry
measurement, one spatial update and zero full-node materializations/resets**. It verifies the
actual node's rendered position and nonzero layout bounds. Activity and preview-token updates
advance only their own revision planes and perform no geometry/spatial update or full-node
materialization. Resize updates one spatial entry; cancellation restores source geometry.
Unit fixtures independently cover retained identity/order, removals/reinsertion, source replacement,
skipped-delta recovery, connection-driven socket layout and optimistic preview precedence.

Flush timings start at the presentation call and end after Svelte flush, a DOM style assertion and
a layout read. Callback intervals cover sustained animation callbacks around those updates.
Neither establishes input-delivery latency, compositor paint, display FPS, dense visible scenes or
continued interactivity across a composed workbench. Activity has one sample per tier, excludes
replacement-array allocation and is not a p95 measurement or complete app-projection timing.

The first P10 checkpoint's retained-state/invalidation requirements are exercised here. The
remaining checkpoints still require incident/obstacle-scoped routes, crossing-edge culling and
full mounted drag/pan/zoom/frame/activity/loop/changing-obstacle measurements at normal/stress
tiers. Current route caching is capped at 1,024 entries, but routing resumes from a full obstacle
set after a preview ends. The scoped routing checkpoint must remove that rebuild and global
geometry-cache invalidation. These samples do not close the full performance checkpoint.
