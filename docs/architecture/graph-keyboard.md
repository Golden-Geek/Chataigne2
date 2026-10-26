# Graph keyboard ownership

`golden_graph_ui` owns node/endpoint selection, movement presentation and keyboard event adaptation.
The host supplies `onGraphSelectionChange`, movement callbacks, `canConnect` and `onConnect`.
Those callbacks own domain validation, graph mutation, persistence and authoritative results.
The keyboard path uses the same movement commit and connection dispatcher as pointer gestures.

Node-title Enter/Space selects; arrows move selected nodes by 0.25 world rem, or 2 rem with Shift.
Embedded editors and other controls retain their arrow keys. Socket Enter/Space selects an endpoint;
activating an opposite endpoint forms the usual directed request, in either selection order.
Repeating an endpoint clears it, choosing another on the same side replaces it, and Escape cancels.
Disabled sockets and the caller's current connection policy remain authoritative. The canvas exposes
the same operations as public component methods so hosts and browser fixtures need not synthesize input.

The Nodes toolbar control opens logical label/ID discovery, independently of canvas culling.
`navigation.svelte.ts` owns cancellable, time-sliced search and 32-result paging; `NodeNavigator`
owns the search/paging controls. Pages cover the entire source without mounting a full node list.
Reveal selects through the existing callback, centers the node and focuses its actual title.
It raises overview zoom to at least 0.5 within the caller's zoom bounds. A focused canvas node
is pinned outside the viewport until focus leaves it. Overview/full-node replacement transfers
focus to the new title; deletion falls back to the canvas. Finder controls stay inside the canvas
and retain their own scrolling and input events. Scope replacement retires pending discovery/reveal.

Collapsed nodes expose a native Expand/Collapse control when the host provides its persistence
callback. Compound body sockets expose native expansion buttons with expanded/disabled state;
nested children remain reachable and a connected descendant keeps its ancestors expanded.
Keyboard activation accepts currently exposed sockets, including the collapsed first socket.
Hiding or disabling a pending endpoint cancels it. Missing callbacks cause no optimistic mutation;
ports without a connection callback are disabled.

`socket-connections.ts` indexes occupancy separately by direction, allowing input and output IDs to
match on one node without affecting each other. Its length-prefixed keys preserve arbitrary IDs.
`geometry.svelte.ts` owns sparse socket expansion and geometry invalidation. Expansion updates
touch one map entry; owner/source replacement clears it and invalidates only affected node layouts.

`connections.svelte.ts` owns connection policy checks and dispatch for both pointer and keyboard.
`feedback.svelte.ts` owns shared request/failure feedback for connection, movement and collapse.
Its action generation prevents older failures from replacing a newer action's feedback, while
older rejected edits still roll back their own optimism when their owner remains current.
Topology changes cancel an uncommitted endpoint choice; they do not retire an already dispatched
request's eventual failure feedback. Request lifetime belongs to the dispatcher, not endpoint selection.
`editing.svelte.ts` uses per-commit tokens so an older move or collapse rejection cannot clear a
newer optimistic edit at the same value. A caller reports failed asynchronous mutations by rejecting
its promise. Automatic endpoint cancellation preserves already-dispatched mutation failure feedback.

The public `interactionScope` token fences domain lifetimes when a host reuses a presentation adapter.
Its default is the graph presentation source. Hosts must replace the token when switching domains
or source generations with the same adapter/IDs. Alchemist supplies session/snapshot/formula identity.
Scope changes retire pointer gestures, optimistic edits and pending endpoint/request state.
Keyboard endpoint selection also cancels on topology changes; position, activity and preview updates
preserve it. Primitive identity fields are derived separately before constructing a scope object so
ordinary document publications cannot trigger a false reset.

The [shared delivery evidence](../progress/p11-graph-keyboard.md) covers the initial component path.
Logical discovery and overview focus pass the [five-tier qualification](../progress/p11-graph-discovery.md).
Collapsed/compound body sockets, rejection lifetimes and five-tier movement/connection timings pass
the [socket qualification](../progress/p11-graph-sockets.md). Complete app-consumer, authoritative
receipt/rejection/reload qualification remains before graph closure. Native input/usability remains P17.
