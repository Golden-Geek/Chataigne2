# P11 shared tree windowing

Golden UI owns reusable row geometry, mounted membership and focus retention. Manager, picker
and outliner panels own their projections and existing interaction policy. This partial P11
milestone qualifies those three consumers; creation catalogs, historical logs, dashboards,
keyboard operations and complete composed workflows remain open. P11 stays at **0/6, 0%**.

## Implementation and correctness

`VirtualRows` mounts the viewport plus overscan and one focused row. A keyed Fenwick height
index handles arbitrary manager supplements without clipping. Logical reveal reaches an
unmounted row directly; measured height changes preserve its anchor. Removing a focused item
returns focus to the parent scroller. Rem estimates are measured into the pixel-valued DOM APIs.

The shared iterative tree projection respects caller-owned child boundaries and hidden-row
hoisting. Matching reads each graph record once and reuses it through projection. Manager/picker
openness survives row unmounts, follows backend metadata, rejects stale asynchronous outcomes and
is fenced by document context and persistent identity. Existing outliner panel openness stays
panel-owned. No protocol or backend mutation semantics change.

Actual production components share one workbench session in a private headless Edge fixture.
Each 1k/10k/20k/50k/100k manager contains ten additional structural/nested nodes. The suite checks
actual selected rows, far reveal, focus pinning/removal, empty filtering, deep expansion after
unmount/remount, rejected collapse, constrained picker search and tall supplements. A separate
100k variable-row scene checks distant reveal and focus fallback. Lists mount at most 42 rows.

## Measurements

The [fixed contract](p11-workspace-qualification-contract.md) predates timing runs. Each action
has three warmups and 31 measured samples at each tier: **465 measured samples, 45 warmups**.
All 15 action/tier combinations pass. Values below are feedback / completion p95 in milliseconds.

| Items | Manager filter | Composed selection | Picker filter |
| ----- | -------------- | ------------------ | ------------- |
| 1k | 5.1 / 10.3 | 10.3 / 16.9 | 6.7 / 13.2 |
| 10k | 18.4 / 21.6 | 36.9 / 41.8 | 23.6 / 32.0 |
| 20k | 52.5 / 58.4 | 64.1 / 70.7 | 46.7 / 54.1 |
| 50k | 97.3 / 100.0 | 123.9 / 128.9 | 120.8 / 128.7 |
| 100k stress | 290.5 / 294.2 | 283.7 / 288.2 | 266.4 / 271.0 |

Normal feedback p95 and maximum callback stall pass the 200ms ceiling; stress passes 400ms.
Completion passes 2s normal / 4s stress. Recorded callback intervals include visible long frames:
this is operation-feedback qualification, without a 60-FPS, physical-input or native-usability claim.
Fixture construction is outside operation clocks and retained separately.

The [initial timed failure](evidence/p11-collection-filter-baseline-2026-10-25.jsonl) preserves
50k picker feedback p95 256.5ms and maximum callback stall 270.8ms. Caching records within the
filter pass and capturing pure configuration once removed repeated persistent-map and reactive
reads. Limits and density stayed fixed. The [passing raw report](evidence/p11-tree-windowing-2026-10-25.jsonl)
retains every sample and correctness result.

The [source/build record](evidence/p11-tree-windowing-source-2026-10-25.json) captures changed source,
lockfile, production asset hashes, machine and tools. Rebuilding from the owning workspace
exactly reproduces the measured bundle. A root-directory rebuild produces different Svelte
asset hashes, so reproduction uses `npm run test:browser --workspace golden_ui`.

## Validation

- `npm test`: 261 cases pass (audio 27, graph 52, Golden UI 67, app 115).
- App `check` and Golden UI `check:browser`: zero errors and warnings.
- Golden UI `test:browser`: all five collection tiers, variable rows and all 15 timed cases pass.

The locked default app build passes in 3m34s with UI rebuild. Executable SHA-256 is
`94c85b4e5e6fa25b708926ed8455aca1c0bd5d2532d17f717e3e340b1fe0eb3d`.
Root and Golden Core Cargo formatting, owned-source Prettier and whitespace checks pass.
The browser fixture has a typed in-memory transport and qualifies these mounted UI boundaries;
whole-product runtime timing and native usability retain P16/P17 ownership.
