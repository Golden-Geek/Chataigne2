# golden_graph_ui

Reusable Svelte 5 graph editing for Golden domains.

This package owns generic canvas mechanics and visuals: viewport and selection behavior, infinite
pan and zoom, animated framing, node dragging and resizing, connection previews, spatial culling,
edge routing, and domain-neutral presentation contracts. It does not own Alchemist, Statechart, or
Chataigne mutation semantics.

`components/GraphCanvas.svelte` composes focused Svelte 5 controllers and renders the world,
wires and toolbar. The component's public props, snippets and navigation methods remain the
consumer boundary. Internal owners live under `components/canvas/`:

- `viewport.svelte.ts`: camera, framing, zoom, measurements and scroll-hover lifecycle.
- `gestures.svelte.ts`: selection, pointer gestures, connection drafts and input dispatch.
- `editing.svelte.ts`: transient/optimistic edits, backend callbacks and rename lifecycle.
- `geometry.svelte.ts`: node/socket layout, visible projection and route-cache composition.
- `mounting.svelte.ts`: bounded node/wire membership reconciliation through `RetainedMounts`.
- `NodeView.svelte`: node markup and scoped node/socket styles, including app-provided snippets.
- `NodeOverview.svelte`: lightweight distant/animated node presentation, with the same geometry
  and identity; selection/rename restores the detailed node and its editing controls.
- `GraphWire.svelte`: one stable wire view, SVG path, gradient, selection and activity styles.
- `contracts.ts`: component props and internal gesture/measurement contracts.

Controllers expose reactive getters and explicit operations. The facade supplies narrow scene
views to camera/editing controllers; geometry receives presentation overlays and a connection
draft. Backend mutation semantics stay with the consuming domain.
Camera scaling uses a world transform; background position/size are direct styles so changing the
camera does not invalidate inherited custom properties or relayout every node through CSS zoom.
Below zoom 0.4 and during camera animations, unselected nodes use overview boxes with labels,
status and port-presence marks. All visible nodes/wires remain mounted; selected and renaming
nodes keep their full controls/snippets. This is presentation detail, not authored collapse state.
Wires use color, opacity and stroke width for status/selection without per-path GPU filters.
Overview wires preserve geometry, hit targets, colors and status but omit CSS transitions
while the camera moves or the view is distant.
Selecting an overview node restores its controls and uses the existing selection/drag/rename
handlers. Beginning a pointer gesture cancels camera animation.

`RetainedMounts` retains visible DOM identities and applies existing-node payload changes
immediately. Its canvas driver admits/removes at most 32 nodes and 64 wires per animation frame,
coalescing new targets while the camera moves. Remaining authored nodes are preserved and the
complete visible membership settles over subsequent frames. `mountingWork` reports pending
membership separately from route work; completion checks wait for both queues. No idle frame
driver remains once the queues settle.

`retained-edges.ts` owns world-unit obstacle/edge indexes, incident connections, bounded cached
route dependencies and deferred route jobs. `edge-routing.ts` owns bounded orthogonal search and
obstacle-aware smoothing; `wire-geometry.ts` owns conservative curve/route bounds and SVG
serialization. `presentation-projection.ts` applies one node's overlays and maps viewport bounds.
`camera-geometry.ts` owns inset normalization, anchored zoom, graph bounds, and framing targets.
Frame Selection looks up selected identities; Home scans nodes without array spreads.

Custom point worlds can keep an empty `GraphDocument` and supply `worldNavigation` with their
`worldContent`. The public `GraphWorldNavigation` contract supplies a complete lightweight ID/label
catalog, selected IDs, bounds in canvas rem units, selection and real DOM focus targets. It reuses
the 32-row finder, reveal and Frame Selection without inventing graph-node shells. Returned focus
targets must belong to the supplied canvas; both HTML and SVG controls are supported.

Keep the provider object and its `source` identity stable during ordinary edits. Change `source`
when the logical document, session or coordinate space changes to retire discovery and focus.
Publish a new immutable `items` array when IDs or labels change, retaining it across geometry-only
updates. Change `presentationRevision` when focusable custom content can be replaced within the
same source, such as a disappearing handle; this restores focus without clearing discovery.
`worldContent` receives `focusedItemId` so bounded custom views can retain that item's real control.
The host owns its catalog, geometry, identity validation and mutations; Golden owns camera and
focus mechanics. Without a provider, ordinary graph-node navigation follows the document.

Explicit positive zoom limits below 1% are supported for wide documents. The numerical safety
floor is `0.000001`; default limits remain unchanged. The zoom slider accepts continuous values
so a fractional minimum does not impose an offset integer-percentage step.

`spatial-index.ts` owns retained grid membership and exact bounding-box queries. `update` moves
only the requested entry, `setValue` replaces payload without moving geometry, and `remove`
retires its cell membership. Updates preserve insertion/document order. Coordinates are copied
and validated before mutation. Entries or queries exceeding the bounded cell budget use an
oversized-entry set or an entry scan instead of expanding an unbounded grid; broad queries can
therefore still visit the complete retained set. `queryLimited` caps candidate visits (including
coarse false positives) and reports completeness; routing uses a curve when the query is incomplete.

`presentation-document.ts` owns immutable presentation snapshots, sequential deltas and independent
topology, geometry, payload, activity and preview revisions. These are UI contracts; generated
Rust wire revisions remain separate. Reuse one `GraphPresentationAdapter` per logical document.
Its array bridge compares changed input arrays in linear time, including semantically equal
allocations, and publishes only changed records. Pass a changing preview token as the third
`update` argument when app-owned preview data changes.
Equivalent wire allocations retain their previous published identities. Routing publishes stable
per-wire snapshots, so one completed route does not dirty every mounted SVG path.
Wires render in stable groups of at most 32 authored edge indices, each with its own positioned
SVG/viewBox and compositing hint. Updating a path isolates its paint surface from overlapping
wires in one SVG surface. Group bounds preserve the same world coordinates and gradient units.

`retained-presentation.ts` retains source/effective identities, authored query order and spatial
membership. Sequential deltas update affected records; skipped deltas recover from the complete
snapshot, and a new adapter source replaces retained state. Transient/optimistic overlay records
use immutable raw rune state so absent-key lookups cannot accumulate deep-proxy subscriptions.
Navigation and selection query retained state; Home may materialize the complete document.

The component exposes `previewNodeMoves`, `previewNodeResize` and `clearNodePreviews` for
programmatic presentation previews. They use the pointer handlers' preview state and do not
persist edits. `commitNodeMovePreview` and `commitNodeResizePreview` use the same callback and
optimistic acknowledgement path as gesture completion. `setCamera`, `panBy`, `zoomAt` and
`navigationState` expose the camera operations used by navigation handlers.
`presentationWork` returns read-only projection counters, excluding adapter
classification, routing, DOM, layout and paint. `routingWork` reports routing counters, cache/queue
size and maximum work per routing slice. Geometry deltas update individual obstacles and incident
edge envelopes. Cached routes depend on their bounded search regions, so obstacle insertion,
movement and removal invalidate affected routes while preserving unrelated cached paths.

Edge culling queries conservative curve/search envelopes and then actual route/curve bounds;
crossing wires remain candidates with both endpoints offscreen. During previews, unchanged cached
routes stay intact and invalidated routes use curves. No full obstacle rebuild runs when a preview
ends. Routing runs only while queued, in animation-frame slices of at most 64 jobs / 4,096 search
visits / 2,048 shortcut checks. A cooperative 4ms target is checked between jobs; one bounded job
may overrun it, and the observed maximum slice duration is reported. Clear segments skip search.
Each job has at most 2,048 visits, 128 obstacles, 512 smoothing obstacle checks and 256
path points; obstacle lookup is capped at 512 candidate visits. Unsafe integer ranges, unavailable
paths and exhausted budgets use curves. Corner rounding never cuts an indexed obstacle.
Each visible admission set also has a total 32,768 search-visit / 65,536 shortcut-check budget.
Geometry/topology or admission-set changes replenish it. Exhaustion caches curves for remaining
jobs, preserving stable settlement rather than retrying indefinitely. Cached curve fallbacks keep
their search-region dependencies so later obstacle changes can invalidate and reroute them.

The current view admits at most 1,024 routed edges in document order; additional edges remain
visible as curves. Cache eviction prefers entries outside that admission set, avoiding endless
eviction/requeue cycles. This limits route detail and work, not the number of rendered edges.
The cache includes successful routes and curve fallbacks; `cachedRoutes` counts cache entries,
not successful obstacle detours.
Normal/stress action and frame qualification passes the fixed P10 fixture; composed-product and
native qualification retain their later plan owners. See the [final results](../../docs/progress/p10-canvas-qualification.md).

Domain packages adapt their typed graph model into these presentation contracts. Product DTO
adaptation, command registration, persistence, and panel composition remain in the consuming app.

## Tests

`npm test --workspace golden_graph_ui` runs the package's unit suites and is included in the
root test command. `npm run test:browser --workspace golden_graph_ui` mounts the real canvas
against an isolated Vite fixture. It verifies actual rendered spatial queries, live presentation,
snippets, selection, nested/collapsed/header sockets, self-loops, gradients, bypass wires, scoped
label styling, and public Frame/Home/coordinate methods. Tests change fixture data and call public
component methods; they perform no keyboard/mouse synthesis. The retained-state fixture measures
31 preview frames after three warmups at 1k/10k/20k/50k/100k authored nodes, with chain edges and
one mounted node and two edge elements. It checks actual DOM geometry and bounded projection work,
including local routing resumption. Further cases exercise actual SVG strokes crossing the viewport,
obstacle changes, preserved distant paths, exterior loops and 1,100 mounted wires at cache overflow.
Its DOM-flush
and animation-frame samples do not qualify input delivery, compositor paint, dense visible scenes
or the full interaction workload. See [retained-state samples](../../docs/progress/p10-retained-presentation.md)
and [scoped routing qualification](../../docs/progress/p10-routing-qualification.md).

`npm run test:qualify --workspace golden_graph_ui` builds the fixture for production and exercises
the full declared action/frame matrix and both dense-wire scenes. It writes timestamped JSONL
reports under `target/qualification/`, retaining raw samples and source/bundle hashes. The
[qualification contract](../../docs/progress/p10-canvas-qualification-contract.md) fixes workloads,
gates and measurement boundaries; adding this harness does not itself close P10.

The browser test defaults to installed Edge on Windows, or Playwright's Chromium elsewhere.
Set `GOLDEN_GRAPH_BROWSER_PATH` for an explicit Chromium executable, or
`GOLDEN_GRAPH_BROWSER_CHANNEL` for an installed channel. The runner owns its browser/server and
closes both on success or failure. It imports no app-owned code and requires no backend or device.
