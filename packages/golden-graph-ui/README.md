# golden_graph_ui

Reusable Svelte 5 graph editing for Golden domains.

This package owns generic canvas mechanics and visuals: viewport and selection behavior, infinite
pan and zoom, animated framing, node dragging and resizing, connection previews, spatial culling,
edge routing, and domain-neutral presentation contracts. It does not own Alchemist, Statechart, or
Chataigne mutation semantics.

Domain packages adapt their typed graph model into these presentation contracts. Product DTO
adaptation, command registration, persistence, and panel composition remain in the consuming app.
