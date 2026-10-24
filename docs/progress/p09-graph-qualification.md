# P09 graph-document qualification

`golden_graph` owns identity storage, authored order, indexed topology/group membership and atomic
transactions. Alchemist and statecharts supply domain semantics; the application host owns user
history and transport. This qualification exercises those boundaries separately. It does not
qualify mounted-canvas or whole-product latency, which retain P10/P16.

## Fixed-delta measurements

The same [public-API harness](../../crates/golden_graph/tests/edit_scaling.rs) ran against runtime
`3214b4d9ea72e987d027dfd75c9a1d1b72fab2fb` and pre-P09
`4cb69d6881179675878accf823d085745a7ce794`. The harness SHA-256 matched on both checkouts:
`c62b909af7ac460ab1f2f74afd5604ccdfc614fec399152c255ec5cb0587d369`.
All **1,240 measured samples** and source/machine metadata are retained in
[the raw evidence](evidence/p09-graph-edits-2026-10-24.json); no measured outliers were discarded.
The long JSON preserves complete sample vectors rather than only summaries.

Reference machine: **BENPORTABLE**, Intel Core Ultra 9 275HX, 24 logical processors, Windows 11 Home
10.0.26300. Rust/Cargo **1.97.0**, `dev` profile with `opt-level=1`, `CARGO_INCREMENTAL=0`.
The workstation used its normal allocator/scheduler, without affinity or frequency locking.

Each fixture has N nodes and layout records, N-1 chain edges, and one group per 16 nodes.
Selection consists of 32 evenly spaced interior nodes: deletion touches 64 incident edges and
32 groups. Singleton cases use the first selected node. Each case has three predefined warmups
and 31 retained samples. The timer includes public transaction construction and commit; rejection
also includes rollback. Fixture creation, whole-document clone/drop, assertions and serialization
are outside the timer. The generic test domain performs no whole-document semantic validation.

Median transaction times below are **milliseconds, baseline → current**:

| Nodes | Move 32 | Delete 32 | Reject and roll back delete 32 |
| ---: | ---: | ---: | ---: |
| 1,000 | 0.055 → 0.012 | 0.287 → 0.082 | 0.565 → 0.129 |
| 10,000 | 0.671 → 0.042 | 3.273 → 0.240 | 6.125 → 0.304 |
| 50,000 | 6.411 → 0.056 | 33.762 → 0.387 | 53.871 → 0.469 |
| 100,000 | 14.243 → 0.056 | 72.024 → 0.431 | 115.988 → 0.545 |

At 100k, current p95 is **0.063 ms** for move-32, **0.545 ms** for delete-32 and **0.572 ms** for
rejected delete-32. Singleton move/delete medians are **0.008/0.025 ms**, versus **0.649/3.396 ms**
before P09. Increasing document size 100× grows current move-32/delete-32 medians about 4.8×/5.3×,
versus about 260×/251× before P09. This supports the removal of document-wide shifts and group
scans from fixed-delta edits, with ordered-index and cache costs still present.

Whole harness wall time was **41.81s current / 39.80s baseline**. It is dominated by deliberately
excluded cloning, disposal and verification; no overall harness or full-product speedup is claimed.
Domain/compiler validation, persistence, snapshots, transport, memory ceilings, rendering and
end-to-end action completion require their own measurements.

Reproduce from the repository root:

```powershell
$env:CARGO_INCREMENTAL='0'
cargo test-fast --locked -p golden_graph --features testkit --test edit_scaling
cargo test-fast --locked -p golden_graph --features testkit --test edit_scaling measure_fixed_graph_edits -- --ignored --exact --nocapture --test-threads=1
```

For comparison, create a detached worktree at `4cb69d68`, add the identical harness at the same
relative path, verify its hash, and run the second command there with the same compiler/profile.
The recorded baseline checkout is `D:\Projects\Dev\Chataigne2-p09-baseline-4cb69d68`; only its
measurement harness was added, with production source unchanged. It remains available for reruns.

## Consumer acceptance

- The public graph fixture verifies 32-node layout edits, deletion, full rollback, group/edge
  membership and reconstruction through `GraphDocumentData`.
- Alchemist applies a selection layout/delete transaction through its real domain, verifies
  unchanged semantic payloads and authored order, and validates restored canonical documents.
- Statecharts consume a batched layout transaction after reconstruction without changing state
  order, transitions or the active configuration.
- The application transport case performs a grouped two-node drag, undo/redo, backend selection
  deletion with incident-edge cleanup, and undo/redo with exact node IDs/UUIDs and moved geometry.
  It uses an authenticated, isolated headless child of the default executable.

Executed acceptance passes: **182 Alchemist**, **10 state-model**, **211 app Alchemist** cases,
the public graph integration case with default features and without Serde, and **both actual
native transport cases** (**21.77s**). The app's six skipped owning cases include those two
explicit native runs; four existing skips remain outside this qualification. Existing 21-case
graph runtime coverage remains applicable because runtime source did not change in this milestone.
Strict workspace/all-target and explicit testkit Clippy pass. The default executable builds and
is the artifact exercised by the native cases; its SHA-256 is
`1295b617ba3e8e173d333408e32bb861b1423236487bc9b17827d39a4347fd88`.

The [implementation ledger](implementation-status.md) records exact commands, evidence paths,
delivery and later qualification owners. P09's local criteria are satisfied; product-scale and
mounted-canvas claims still require their respective plan tasks.
