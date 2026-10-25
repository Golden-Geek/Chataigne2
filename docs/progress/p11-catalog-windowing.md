# P11 manager/catalog checkpoint

Golden UI's shared ContextMenu owns app-agnostic catalog presentation. NodeAddButton and
node/graph context menus supply their catalog and actions through the existing public API.
The menu now delegates large collection mounting and filtering to ContextMenuRows, backed by
the shared VirtualRows height index. Placement calculations live in a focused presentation
module; the parent owns portals, actions and submenu lifetimes.

Menus above 128 items use a searchable 22rem-wide list, at most 32rem high. Smaller menus retain
compact columns. The large surface creates one CSS grid child, avoiding a hidden grid track for
each logical item. Stable IDs retain focused DOM across reordering; absent IDs use object identity.
Repeated IDs are distinguished by occurrence, so callers should supply unique stable IDs when
identity must survive reordering. Focused rows stay mounted outside the viewport. Filtering out
the focused item returns focus to the list, and retiring a parent anchor closes its submenu.
Action routing and domain creation policy are unchanged.

## Qualification

The [contract](p11-workspace-qualification-contract.md#creation-catalog-extension) fixes five
tiers before timing. Production ContextMenu consumes descriptors from the actual
buildCreatableItemMenu builder. Root and nested layers expose the same 1k/10k/20k/50k/100k
catalog. Correctness verifies actual far-item text/identity, focused DOM after far reveal and
reversal, empty-filter focus fallback, nested search, visible submenu/row bounds, orphan retirement
and compact rendering. The explicit 14rem maximum-width option is also qualified. Every large
layer mounts at most 42 rows.

Five filtering cases retain **155 measured samples and 15 warmups**. All fixed gates pass:

| Items | Feedback p95 | Completion p95 | Maximum callback interval |
| ----- | ------------ | -------------- | ------------------------- |
| 1k | 1.0ms | 8.6ms | 4.4ms |
| 10k | 2.0ms | 8.7ms | 4.4ms |
| 20k | 2.9ms | 8.5ms | 4.3ms |
| 50k | 5.8ms | 12.6ms | 8.4ms |
| 100k stress | 10.8ms | 16.8ms | 12.6ms |

[Catalog samples](evidence/p11-catalog-2026-10-25.jsonl) retain every warmup and measurement.
The complete [manager/picker/outliner regression](evidence/p11-catalog-collection-regression-2026-10-25.jsonl)
also passes all 15 action/tier cases, 465 measurements and 45 warmups, plus variable rows.
The final timing runs execute separately from other validation commands.
The [source/build record](evidence/p11-catalog-source-2026-10-25.json) identifies the same unchanged
runtime/fixture source for both runs and the final production assets, machine and tools.

The initial [orphan baseline](evidence/p11-catalog-orphan-baseline-2026-10-25.jsonl) and
[fixture failure](evidence/p11-catalog-fixture-baseline-2026-10-25.jsonl) preserve development
failures. Setup had retained its prior no-match query; fixing setup and qualifying actual visible
bounds prevents an offscreen submenu from passing on DOM membership alone. The runtime now
retires missing anchors at the layout boundary. Workload sizes and timing gates stayed fixed.

`npm test` passes **261 cases**. App `check` and Golden UI `check:browser` report zero errors and
warnings. The final locked default app build passes in **2m46s**, rebuilding the frontend;
executable SHA-256 is `71b9d4e6cff31de767ea4c5bf52ff879e14be1bab833ee9582b37b20f852862c`.
Root/Golden Core Cargo formatting, owned Prettier and whitespace checks pass.
Every touched runtime/test source is below 1,000 lines. No protocol or Rust runtime changed.

Together with [shared tree windowing](p11-tree-windowing.md), this closes P11's manager/catalog
checkpoint. **P11 is 1/6 complete (17% rounded)**. Historical logs, dashboard viewport awareness,
tree keyboard navigation, graph keyboard operations and complete composed workflows remain.
Native input/usability and full-product timing retain P17/P16 ownership.
