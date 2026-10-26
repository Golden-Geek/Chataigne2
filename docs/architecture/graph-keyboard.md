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

`connections.svelte.ts` owns dispatch/failure feedback for both pointer and keyboard requests.
Its request generation prevents older failures from replacing a newer request's feedback.
Topology changes cancel an uncommitted endpoint choice; they do not retire an already dispatched
request's eventual failure feedback. Request lifetime belongs to the dispatcher, not endpoint selection.
`editing.svelte.ts` uses per-commit tokens so an older move rejection cannot clear a newer optimistic
move at the same coordinates. A caller reports failed asynchronous mutations by rejecting its promise.

The public `interactionScope` token fences domain lifetimes when a host reuses a presentation adapter.
Its default is the graph presentation source. Hosts must replace the token when switching domains
or source generations with the same adapter/IDs. Alchemist supplies session/snapshot/formula identity.
Scope changes retire pointer gestures, optimistic edits and pending endpoint/request state.
Keyboard endpoint selection also cancels on topology changes; position, activity and preview updates
preserve it. Primitive identity fields are derived separately before constructing a scope object so
ordinary document publications cannot trigger a false reset.

The [shared delivery evidence](../progress/p11-graph-keyboard.md) covers the initial component path.
Complete app-consumer, offscreen/overview/compound-socket and fixed large-graph qualification remains
required before closing P11's graph checkpoint. Native input/usability remains P17.
