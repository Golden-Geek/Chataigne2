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

The canvas currently still constructs indexes from full effective-node arrays. P10's next step
is retained projection/geometry integration in the focused controllers; the mutable index
alone does not establish incremental interaction or mounted performance.

Domain packages adapt their typed graph model into these presentation contracts. Product DTO
adaptation, command registration, persistence, and panel composition remain in the consuming app.

## Tests

`npm test --workspace golden_graph_ui` runs the package's unit suites and is included in the
root test command. `npm run test:browser --workspace golden_graph_ui` mounts the real canvas
against an isolated Vite fixture. It verifies actual rendered spatial queries, live presentation,
snippets, selection, nested/collapsed/header sockets, self-loops, gradients, bypass wires, scoped
label styling, and public Frame/Home/coordinate methods. Tests change fixture data and call public
component methods; they perform no keyboard/mouse synthesis and do not qualify interaction latency.

The browser test defaults to installed Edge on Windows, or Playwright's Chromium elsewhere.
Set `GOLDEN_GRAPH_BROWSER_PATH` for an explicit Chromium executable, or
`GOLDEN_GRAPH_BROWSER_CHANNEL` for an installed channel. The runner owns its browser/server and
closes both on success or failure. It imports no app-owned code and requires no backend or device.
