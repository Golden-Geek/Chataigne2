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
the canvas supplies live measurements and revision dependencies.

Domain packages adapt their typed graph model into these presentation contracts. Product DTO
adaptation, command registration, persistence, and panel composition remain in the consuming app.
