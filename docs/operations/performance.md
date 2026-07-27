# Performance Contracts

Performance is an architecture boundary, not a late UI optimization. The engine must remain usable
with 100,000 runtime values and the mounted workbench must remain responsive with 10,000 graph
nodes.

## Runtime

- IO parsing, timestamping, reconnect, and device polling run in `crates/golden_core/runtime/io` or app-owned workers.
- The actor-owned engine applies typed inputs and graph mutations; it performs no socket or device
  IO.
- Every periodic node declares a stable compiled-kernel identity. Production generation
  compilation rejects unnamed scheduled work.
- Value updates coalesce where the contract allows it. Triggers, commands, and effects preserve
  order and use bounded queues.
- Structural edits use `NodeTree`/`AddNodeTree` for known subtrees and avoid repeated whole-tree
  snapshots.
- Lifecycle-generated descendants are accumulated until the outer insertion stabilizes. The UI
  receives one completed subtree transaction for descendants inside that root, while generated
  siblings outside the root retain their own transactions.
- Batch lifecycle callbacks honor `lifecycle_requires_tree_snapshot` across the entire batch.
  When every node opts out, attached/init/ready stages do not clone the graph. User-item creation
  compares direct children without constructing whole-tree snapshots.

Run the release scalar qualification from the repository root:

```text
python tools/qualification/runtime_scale.py --output-dir target/qualification/runtime-scale/local
```

The report records dense, one-percent-dirty sparse, and idle distributions for two 100,000-value
partitions, determinism across 1/2/4/8 workers, missed deadlines, and output capacity.

## UI and graphs

`golden_graph_ui` keeps the graph document independent of rendered DOM. Viewport culling and keyed
stores limit work to visible nodes; a 10,000-node document must not mount the whole graph.

```text
python tools/qualification/graph_scale.py \
  --output-dir target/qualification/graph-scale/local \
  --port 7037
```

This launches the bundled product, loads the deterministic graph fixture, exercises outliner,
inspector, Formula, State Machine, live feedback, save/reload, and cleanup, and records total,
visible, and rendered node counts.

## Regression workflow

Use `cargo bench -p golden_engine` for local investigation. A meaningful regression above five
percent requires investigation against the same toolchain and fixture; do not raise timeouts,
reduce preview frequency, disable preview, or replace the full-workbench gate with a headless-only
benchmark.

The five-minute multi-client soak is documented in
[release-readiness.md](release-readiness.md). Longer endurance runs may be selected for a release
candidate. Reports under `target/` are disposable evidence and must not be committed.
