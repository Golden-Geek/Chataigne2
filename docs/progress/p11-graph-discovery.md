# P11 logical graph discovery and focus

Golden Graph UI owns the generic finder, bounded presentation and focus lifecycle. App callbacks
continue to own selection policy and mutations. This advances the graph checkpoint after `6544642b`;
P11 remains **4/6 closed checkpoints (67% rounded)** and the whole plan **59%**.

The Nodes toolbar control searches logical labels/IDs, including offscreen nodes, and pages through
all matches in source order with at most 32 mounted results. Search yields in 4ms work slices and
retires obsolete queries. Reveal selects through the existing callback, centers the requested node
and focuses its actual title. Overview has native title buttons. One focused node is retained through
panning; changing between full and overview presentation transfers focus. Removing a focused node
falls back to the canvas. Live filtering/rename/deletion, owner replacement and unmount retire stale
work, with input/canvas focus fallback where appropriate. No domain mutation policy is added.

The finder is positioned inside the canvas independently of toolbar width; its scrolling and control
events stay within the finder. An intermediate build caught a duplicate `$props.id()` call during
the positioning change; a suffix of the component's single ID repairs it. Final checks have zero
Svelte errors or warnings. The owning browser inspects finder rectangles as well as actual result
text, logical IDs, pressed selection, focused titles and mounted bounds.

## Fixed production qualification

The [workload contract](p11-workspace-qualification-contract.md#graph-discovery-extension) was fixed
before timing: 1k/10k/20k/50k/100k nodes, one chain edge per subsequent node, fixed viewport/grid,
three warmups and 31 retained samples for search, clear, page round trip and reveal from overview.
All **20 combinations / 620 retained samples / 60 warmups** pass unchanged limits. Worst feedback
p95 is **45.8ms**, completion p95 **53.1ms**, and maximum observed callback stall **59.33ms**.
Setup is measured separately (741.4ms at 100k). Finder results stay within 32 and this scene's canvas
within 128 nodes, including one focus pin. These are operation timings, not physical-key usability.

All five tiers also pass superseded-query, filter focus fallback, unselected overview activation,
pan retention, releasing an offscreen pin when focus leaves the canvas, selected-to-overview
replacement, deletion, rename, source replacement and unmount
checks. The existing production keyboard rejection/lifecycle groups and retained canvas smoke pass.
No keyboard or mouse input is synthesized.

Evidence: [raw discovery matrix](evidence/p11-graph-discovery-2026-10-26.jsonl),
[shared keyboard regression](evidence/p11-graph-discovery-keyboard-2026-10-26.jsonl),
[retained canvas smoke](evidence/p11-graph-discovery-canvas-smoke-2026-10-26.json), and
[source/build fingerprints](evidence/p11-graph-discovery-source-2026-10-26.json).
UI suites pass **400 cases**; app Svelte check reports zero errors/warnings. Both Cargo formatters
and package-owned formatting pass. Inventory is **2,200 sources / 27 oversized**; touched source
files stay below 1,000 lines. The locked default build passes in **2m19s** with a fresh frontend;
the source/build record preserves its binary hash.

```powershell
npm run test:discovery --workspace golden_graph_ui
npm run test:keyboard --workspace golden_graph_ui
npm run test:browser --workspace golden_graph_ui
npm test
npm run check --workspace chataigne-ui
```

Before graph closure, qualify collapsed/compound endpoints, all actual app adapters (Alchemist,
state machine and Spatializer), read-only/source replacement and authoritative mutation/rejection/
reload, plus movement/connection timing. Complete composed workflows remains the other open P11
checkpoint. Native usability belongs to P17 and full-product throughput to P16.
