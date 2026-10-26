# P11 shared graph keyboard delivery

This implements the shared graph interaction path after tree closure in `574be597`. The graph
checkpoint remains open: app-consumer and complete large-graph qualification are still required.
See [ownership and lifecycle](../architecture/graph-keyboard.md).

## Delivered behavior

GraphCanvas now exposes keyboard node selection, movement, endpoint activation and cancellation.
Node-title Enter/Space selects; arrows move selected nodes in world rem, with a larger Shift step.
Movement visits only selected IDs, supports multi-selection and shares pointer movement commits.
No movement callback means no keyboard move or optimistic commit. Inputs and embedded controls
keep their own key handling. Selection buttons expose pressed state independently of rename permission.

All seven rendered socket-button variants carry direction, identity, label and pending-selection
state. Endpoint activation supports either order, same-endpoint cancellation, same-side replacement,
disabled sockets and the current caller connection policy. A common pointer/keyboard dispatcher
handles synchronous/asynchronous failure and retires old feedback after newer requests or scope changes.
The canvas provides accessible instructions and request feedback; no graph policy is duplicated in UI.

Interaction scopes retire gestures and optimistic state across owner replacement. Alchemist supplies
its session, snapshot generation and formula UUID. Older move rejections cannot clear a newer equal-
coordinate request. Activity/position publications preserve pending endpoint selection; topology changes
cancel it. A regression initially caught a false reset on ordinary activity publication. Separately
derived source/scope identities repair it, and the existing retained-canvas smoke passes again.
A second regression proves that topology changes must not suppress a dispatched request's later
failure. The dispatcher keeps that request lifetime separate from endpoint-selection cancellation.

## Evidence and scope

Four new unit cases cover directed endpoint choice, cancellation/replacement, finite multi-node
world-unit movement, and exactly three record reads for three selected IDs in a 100k-node graph.
That is a proportional-work assertion, not a mounted 100k keyboard timing qualification.

The production owned-browser fixture exercises five groups: movement/rejection, connection policy,
activity/topology behavior, scope/equal-coordinate stale-move rejection, and asynchronous connection
failure/newer-request fencing. It calls the component operations used by handlers, inspects actual
node positions, socket pressed state and request output, and reports zero page errors. No input is synthesized.

The existing canvas smoke separately preserves its 1k/10k/20k/50k/100k retained projection, preview,
resize, routing, crossing/obstacle, loop and 1,100-wire overflow checks. Its development-browser
observations do not replace P10's production timing evidence or the new P11 action matrix.

Logical offscreen discovery and overview focus are now qualified in the subsequent
[graph discovery delivery](p11-graph-discovery.md). Before graph checkpoint closure, complete collapsed/
compound endpoint cases, all actual app adapters (including read-only and source replacement),
authoritative mutation/rejection/reload checks, and a fixed five-tier action/timing matrix.
Then complete P11's composed-workflow checkpoint. These requirements do not add checkpoints.
P11 remains **4/6 (67% rounded)** and whole-plan scope coverage remains **59%**.

## Reproduction

```powershell
npm test
npm run check --workspace chataigne-ui
npm run test:keyboard --workspace golden_graph_ui
npm run test:browser --workspace golden_graph_ui
```

The production graph fixture, raw component results and source/artifact fingerprints are retained
with this milestone. Physical-key/webview focus/DPI/usability evidence remains P17; product-scale
runtime throughput remains P16.

Validation: **400 UI unit cases** pass (27 audio / 56 graph / 191 Golden / 126 app), app Svelte check
reports zero errors/warnings, and the locked default build passes in **1m55s** with a fresh frontend.
Both Cargo formatters and package-owned source formatting pass. See the
[source/build record](evidence/p11-graph-keyboard-source-2026-10-26.json),
[production component results](evidence/p11-graph-keyboard-2026-10-26.jsonl) and
[development-browser smoke](evidence/p11-graph-keyboard-canvas-smoke-2026-10-26.json).
