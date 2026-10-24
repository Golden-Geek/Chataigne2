# golden_graph_ui

Reusable Svelte 5 graph editing for Golden domains.

This package owns generic canvas mechanics and visuals: viewport and selection behavior, infinite
pan and zoom, animated framing, node dragging and resizing, connection previews, spatial culling,
edge routing, and domain-neutral presentation contracts. It does not own Alchemist, Statechart, or
Chataigne mutation semantics.

`components/GraphCanvas.svelte` owns viewport state and interactions. `edge-routing.ts` owns the
pure obstacle index, bounded route search, and SVG path construction; the canvas supplies current
node geometry and retains its per-edge cache. `presentation-projection.ts` owns optimistic node
overlays, spatial-index construction, viewport queries, and document-ordered visible-edge selection;
the canvas supplies live measurements and revision dependencies. `camera-geometry.ts` owns inset
normalization, anchored zoom, graph bounds, and framing targets. Home and Frame Selection scan
nodes once rather than spreading large node arrays into `Math.min`/`Math.max`.

`spatial-index.ts` owns retained grid membership and exact bounding-box queries. `update` moves
only the requested entry, `setValue` replaces payload without moving geometry, and `remove`
retires its cell membership. Updates preserve insertion/document order. Coordinates are copied
and validated before mutation. Entries or queries exceeding the bounded cell budget use an
oversized-entry set or an entry scan instead of expanding an unbounded grid; broad queries can
therefore still visit the complete retained set.

The canvas currently still constructs indexes from full effective-node arrays. P10's next step
is focused canvas decomposition and retained projection/geometry integration; the mutable index
alone does not establish incremental interaction or mounted performance.

Domain packages adapt their typed graph model into these presentation contracts. Product DTO
adaptation, command registration, persistence, and panel composition remain in the consuming app.

## Tests

`npm test --workspace golden_graph_ui` runs the package's unit suites and is included in the
root test command. `npm run test:browser --workspace golden_graph_ui` mounts the real canvas
against an isolated Vite fixture and verifies rendered spatial queries through data updates.
This smoke test performs no keyboard/mouse synthesis and does not measure interaction latency.

The browser test defaults to installed Edge on Windows, or Playwright's Chromium elsewhere.
Set `GOLDEN_GRAPH_BROWSER_PATH` for an explicit Chromium executable, or
`GOLDEN_GRAPH_BROWSER_CHANNEL` for an installed channel. The runner owns its browser/server and
closes both on success or failure. It imports no app-owned code and requires no backend or device.
