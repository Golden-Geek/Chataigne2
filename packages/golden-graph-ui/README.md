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
- `NodeView.svelte`: node markup and scoped node/socket styles, including app-provided snippets.
- `contracts.ts`: component props and internal gesture/measurement contracts.

Controllers expose reactive getters and explicit operations. The facade supplies narrow scene
views to camera/editing controllers; geometry receives presentation overlays and a connection
draft. Backend mutation semantics stay with the consuming domain.

`edge-routing.ts` owns the pure obstacle index, bounded route search, and SVG path construction.
`presentation-projection.ts` owns optimistic node overlays, spatial-index construction, viewport
queries, and document-ordered visible-edge selection. `camera-geometry.ts` owns inset normalization,
anchored zoom, graph bounds, and framing targets. Home and Frame Selection scan nodes once rather
than spreading large node arrays into `Math.min`/`Math.max`.

`spatial-index.ts` owns retained grid membership and exact bounding-box queries. `update` moves
only the requested entry, `setValue` replaces payload without moving geometry, and `remove`
retires its cell membership. Updates preserve insertion/document order. Coordinates are copied
and validated before mutation. Entries or queries exceeding the bounded cell budget use an
oversized-entry set or an entry scan instead of expanding an unbounded grid; broad queries can
therefore still visit the complete retained set.

`presentation-document.ts` owns immutable presentation snapshots, sequential deltas and independent
topology, geometry, payload, activity and preview revisions. These are UI contracts; generated
Rust wire revisions remain separate. Reuse one `GraphPresentationAdapter` per logical document.
Its array bridge compares changed input arrays in linear time, including semantically equal
allocations, and publishes only changed records. Pass a changing preview token as the third
`update` argument when app-owned preview data changes.

`retained-presentation.ts` retains source/effective identities, authored query order and spatial
membership. Sequential deltas update affected records; skipped deltas recover from the complete
snapshot, and a new adapter source replaces retained state. Transient/optimistic overlay records
use immutable raw rune state so absent-key lookups cannot accumulate deep-proxy subscriptions.
Navigation and selection query retained state; Home may materialize the complete document.

The component exposes `previewNodeMoves`, `previewNodeResize` and `clearNodePreviews` for
programmatic presentation previews. They use the pointer handlers' preview state and do not
persist edits. `presentationWork` returns read-only projection counters, excluding adapter
classification, routing, DOM, layout and paint. During move/resize previews, wires use cheap
curves; the obstacle cache rebuilds when routing resumes. Route caches are capped at 1,024
entries. Incident/obstacle-scoped invalidation and crossing-edge culling remain P10 work.

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
one mounted node/edge. It checks actual DOM geometry and bounded projection work. Its DOM-flush
and animation-frame samples do not qualify input delivery, compositor paint, dense visible scenes
or the full interaction workload. See the [scope and retained samples](../../docs/progress/p10-retained-presentation.md).

The browser test defaults to installed Edge on Windows, or Playwright's Chromium elsewhere.
Set `GOLDEN_GRAPH_BROWSER_PATH` for an explicit Chromium executable, or
`GOLDEN_GRAPH_BROWSER_CHANNEL` for an installed channel. The runner owns its browser/server and
closes both on success or failure. It imports no app-owned code and requires no backend or device.
