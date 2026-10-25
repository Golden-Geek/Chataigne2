# P11 shared tree keyboard delivery

Golden UI now owns keyboard navigation for ManagerListPanel, OutlinerPanel and NodePickerModalHost
through one TreeRows component. Chataigne's processor manager also delegates to ManagerListPanel,
removing its duplicate recursive rendering and drag/drop implementation. See the
[ownership contract](../architecture/tree-navigation.md) and
[fixed qualification workload](p11-workspace-qualification-contract.md#tree-keyboard-extension).

## Behavior

Up/Down and Home/End move focus across the full logical projection, including unmounted rows.
Right expands or enters a branch; Left collapses or returns to the visible parent. Enter and Space
select through the existing session or picker policy. Ctrl/Meta toggle, Shift adds and Alt removes
for session selection. Navigation alone leaves selection unchanged. Search's Down operation enters
the tree. Embedded inputs and row controls retain their own keyboard behavior.

Rows expose roving tabindex, logical level, expanded/selected state and filtered sibling position/count.
Focus survives far scroll through bounded DOM retention. Collapse/removal/filtering choose a visible
fallback; an empty tree returns focus to its container. Search focus is preserved. New navigation,
new focus owners, source UUID replacement, owner changes and unmount fence pending focus work.

The projection now honors explicit collapse before automatic ancestor visibility. Outliner selection
opens ancestors once rather than continually overriding a later collapse. Panel-owned node lookup
caches reconcile persistent-map changes; ordinary mutable maps remain uncached, and empty owners
release retained records. Manager roots resolve their current graph record even when a caller's
provided DTO is older.

## Qualification and limits

The fixed matrix covers three production consumers at 1k/10k/20k/50k normal and 100k stress:
30 Home/End and branch expansion/collapse combinations, 930 retained samples and 90 warmups.
Correctness also covers real focus and label text, parent/child traversal, rejected expansion,
modifier selection, offscreen DOM identity, collapse of a focused descendant, search ownership,
empty/removal fallback and newer navigation/source replacement superseding pending focus.
Every fixture tree stays within 42 mounted rows. No keyboard/mouse events are synthesized.

Initial runs retained failures at 50k manager expansion (225ms stall) and 100k picker expansion
(404.2ms stall). Profiling identified repeated persistent-trie lookups of unchanged nodes, including
manager-root item enumeration. The panel-owned lookup projection removes those repeated lookups;
an additional repeat still failed at 100k manager expansion (408.4ms). Retained branch projection
now also avoids rebuilding unchanged rows and duplicate work on backend acknowledgements.
The workload and timing limits are unchanged. The fixture's persisted outliner state is reset per
setup so an earlier tier's expansion does not alter the next case's starting state.

The final [keyboard run](evidence/p11-tree-keyboard-2026-10-26.jsonl) passes all 30 combinations,
15 navigation/activation groups and 15 lifecycle groups, with no page errors. At 20k/50k normal,
worst per-case feedback p95 is 41.5ms, completion p95 47.5ms and maximum callback interval 45.8ms.
At 100k stress these are 96.9ms, 104.1ms and 262.6ms. All are within the fixed contract.
The [collection regression](evidence/p11-tree-collection-regression-2026-10-26.jsonl) also passes
all 15 combinations (465 samples / 45 warmups), five correctness tiers and variable-row focus
retention/removal. The [failed rows](evidence/p11-tree-keyboard-failures-2026-10-26.jsonl) preserve
raw failing samples and the initial fixture-state mismatch.

The shared implementation is delivered; **the tree checkpoint remains open** pending actual app
wrapper integration qualification and constrained-picker coverage (hidden/nonselectable rows,
view changes and projection/confirmation behavior). Those two requirements do not add checkpoints
to P11. Physical keyboard/focus usability remains P17 and whole-product throughput remains P16.

## Reproduction

Run from the repository root:

```powershell
npm test
npm run check:browser --workspace golden_ui
npm run check --workspace chataigne-ui
npm run test:browser --workspace golden_ui
npm run test:browser --workspace golden_ui -- --tree-keyboard
```

`--tree-profile` runs a separate diagnostic profiler for 50k manager expansion. Profiled samples
are not qualification evidence. Raw JSONL, source/build fingerprints and final validation are
recorded alongside this delivery in the current ledger and evidence files.

## Validation

- Root `npm test`: **391 passed** (27 audio / 52 graph / 187 Golden UI / 125 app).
- Browser and app Svelte checks: zero errors and warnings.
- Final keyboard and collection browser runs: all **45 timing combinations**, 1,395 retained
  samples and 135 warmups pass. Source/asset/machine hashes are in the
  [source record](evidence/p11-tree-keyboard-source-2026-10-26.json).
- Locked default app build: passes in **2m36s**, including a fresh production frontend.
- Both Cargo formatters, owned-source Prettier and whitespace checks pass. Inventory is
  **2,183 sources / 27 oversized**; no touched source exceeds 1,000 lines.

Rust and protocol sources are unchanged. This delivery leaves P11 at **3/6 closed checkpoints
(50%)** and whole-plan scope coverage at **58%**, with the two tree qualification requirements
above still explicit.
