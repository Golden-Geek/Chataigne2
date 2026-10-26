# P11 graph sockets and mutation feedback

Golden Graph UI owns exposed socket selection, expansion, request feedback and optimistic display.
The host owns graph mutation, connection policy, persistence and authoritative acknowledgement.
This advances the shared graph path after `e9c5a486`; app-consumer qualification still owns closure
of P11's graph checkpoint. P11 stays **4/6 checkpoints (67% rounded)** and the whole plan **59%**.

Collapsed nodes now have a native Expand/Collapse control when their host supports the operation.
Compound body sockets support nested expansion and truthful expanded/disabled attributes. Connected
descendants keep ancestors expanded. Hidden endpoints cannot be activated; hiding or disabling a
pending endpoint cancels it. Read-only/missing callbacks cause no optimistic mutation, and ports
without connection callbacks are disabled. The public component operations are the same ones used
by the native controls and keyboard handlers; the owned browser never synthesizes input.

One feedback owner now serves connection, movement and collapse requests. Old failures cannot
replace newer feedback, but still retire their own current-owner optimism. Move/collapse tokens
also distinguish newer equal-value edits. Automatic endpoint cancellation preserves a dispatched
mutation's failure reporting. Source replacement retires feedback and sparse expansion state.

The production regression caught a real direction-aliasing defect: an input leaf connection could
expand outputs with the same IDs. The new immutable occupancy index separates input/output sets
and uses length-prefixed node/socket keys. Two unit cases cover same-ID directions and separator/
empty-ID collisions. The raw failing browser assertion is preserved with its passing successor.
Expansion state now uses a sparse map and revision, avoiding whole-history object copies per toggle.

## Qualification

The [fixed workload](p11-workspace-qualification-contract.md#graph-mutation-and-socket-extension)
retains 1k/10k/20k/50k/100k nodes and chain edges, with ordinary, nested compound and collapsed
ports on the last three nodes. Six actions cover single/multi movement, each endpoint order,
compound expansion/leaf connection, and collapsed-node expansion/connection. The final production
matrix passes **30 combinations / 930 samples / 90 warmups** under unchanged limits: maximum
feedback p95 **81.3ms**, completion p95 **85.6ms**, callback stall **157.6ms**.

Assertions inspect directed source edges, source and rendered positions, focused native titles,
pressed endpoint state, collapse acknowledgement and mounted bounds. Nine production correctness
groups cover the original shared behavior and all seven socket button variants, nested visibility,
occupied ancestor/descendant exclusion, direction aliasing, scope/read-only guards and stale failures.
The discovery regression also passes **20 combinations / 620 samples / 60 warmups**. Fixture callbacks
publish acknowledged positions/edges/collapse through GraphPresentationAdapter; this does not prove
that every actual app host returns authoritative backend failures or survives reload.

Evidence: [mutation timing](evidence/p11-graph-sockets-timing-2026-10-26.jsonl),
[discovery regression](evidence/p11-graph-sockets-discovery-2026-10-26.jsonl),
[socket failure reproduction](evidence/p11-graph-sockets-regression-2026-10-26.jsonl),
[correctness groups](evidence/p11-graph-sockets-2026-10-26.jsonl), and
[source/build record](evidence/p11-graph-sockets-source-2026-10-26.json).

The final full [canvas regression](evidence/p11-graph-sockets-canvas-2026-10-26.jsonl) also passes
all **55 action/tier combinations and both dense scenes**, retaining **1,767 samples / 171 warmups**
under the original P10 frame/feedback/completion gates. The [development smoke](evidence/p11-graph-sockets-smoke-2026-10-26.json)
preserves retained projection/routing behavior. All **402 UI unit cases** pass (27 audio / 58 graph /
191 Golden / 126 app), and app Svelte check reports zero errors/warnings. Touched runtime/tests
stay below 1,000 lines; inventory is **2,206 sources / 27 oversized**. Rust/protocol are unchanged.
Both Cargo formatters and package-owned formatting pass. The locked default build passes in
**2m17s** with a fresh frontend; its binary hash is retained in the source/build record.

```powershell
npm run test:keyboard --workspace golden_graph_ui
npm run test:keyboard:qualify --workspace golden_graph_ui
npm run test:discovery --workspace golden_graph_ui
npm run test:qualify --workspace golden_graph_ui
npm test
npm run check --workspace chataigne-ui
```

Remaining graph acceptance is actual app adapters: Alchemist, state machine and Spatializer,
including read-only/source replacement and authoritative mutation/rejection/reload. Inspect actual
host promises and queued-operation ownership; shared fixture success cannot stand in for them.
Spatializer uses an empty graph document with custom world content, so qualify its actual integration
without inventing graph ports. Composed workflows remains the other open P11 checkpoint. Physical
keyboard/webview usability stays with P17, full-product throughput with P16.
