# Codex Plan — Golden Core structural performance refactor

## Mission

Refactor Chataigne2 / Golden Core so loading, saving, duplicating, removing, renaming, refreshing the UI, and other structural graph operations remain fast on large projects.

The current unacceptable symptom is:

```txt
[ui-http] snapshot scope=WholeGraph nodes=3053 bytes=1672467 build_ms=8 serialize_ms=36 total_ms=14522
```

Since `build_ms` and `serialize_ms` are small, the missing time is almost certainly before measured build time: engine lock wait, active edit session cancellation, runtime contention, WebSocket dispatch contention, or another synchronous path around the shared engine.

The goal is not only to patch this specific log. The goal is to make the architecture future-proof for much larger projects.

---

## Non-negotiable performance invariants

Implement and preserve these invariants:

1. UI read paths must not lock the live mutable engine.
2. Normal UI edits must not trigger `WholeGraph` resync.
3. Rename must be a tiny delta.
4. Duplicate must be one structural transaction.
5. Remove must be one structural transaction.
6. Move/reorder must be one structural transaction.
7. Save must not block engine tick, UI snapshot, WebSocket dispatch, or ordinary edits.
8. Load/replace must prepare the engine and UI read model before publishing the new state.
9. Disk I/O and network I/O must never happen while holding live engine state.
10. Full graph traversal is allowed for load/replace/debug refresh only, not normal edits.
11. Svelte graph state must update by node/entity/version, not by invalidating the entire graph.
12. AddNodeTree remains valid for prepared subtree insertion, but sibling/forest/bulk edits need a general transaction primitive.
13. Every structural performance change needs timing logs and regression tests.
14. If an operation emits `resync_required` during normal editing, treat it as a bug unless the protocol epoch changed.

---

## Phase 0 — Baseline and instrumentation first

### Objective

Make the real bottleneck visible before deeper refactor.

### Files to inspect/change

```txt
submodules/golden_core/crates/transport_server/src/ui_server.rs
submodules/golden_core/crates/transport_server/src/project_host.rs
submodules/golden_core/crates/core/src/engine/runtime.rs
src-ui/src/lib/golden_ui/store/workbench.svelte.ts
src-ui/src/lib/golden_ui/store/graph.svelte.ts
src-ui/src/lib/golden_ui/transport/http.ts
src-ui/src/lib/golden_ui/transport/ws.ts or equivalent websocket client
```

### Add HTTP snapshot timing

In `/api/ui/snapshot`, split timing into:

```txt
request_parse_ms
lock_wait_ms
cancel_edit_ms
build_ms
serialize_ms
write_response_ms
total_ms
nodes
bytes
scope
cancel_active_edit_session
```

Important: start `lock_wait_ms` immediately before `lock_engine`.

Current code starts `build_ms` only after lock acquisition, which hides the real delay.

Expected log:

```txt
[ui-http] snapshot scope=WholeGraph nodes=3053 bytes=1672467 request_parse_ms=0 lock_wait_ms=14410 cancel_edit_ms=0 build_ms=8 serialize_ms=36 write_response_ms=4 total_ms=14458
```

or similar.

### Add WebSocket intent timing

For every UI intent:

```txt
[ui-ws] intent kind=rename|duplicate|remove|move|reorder|set_param lock_wait_ms=... apply_ms=... event_collect_ms=... events=... requires_resync=... total_ms=...
```

### Add WebSocket dispatch timing

For dispatch batches:

```txt
[ui-ws] dispatch clients=... subscriptions=... lock_wait_ms=... collect_ms=... serialize_ms=... send_ms=... events=... total_ms=...
```

### Add runtime tick phase timing

In `Engine::run_tick`, add guarded debug/perf logs:

```txt
[engine] tick total_ms=... resolve1_ms=... absorb_external_edits_ms=... apply_external_edits_ms=... inbox_precompute_ms=... inbox_preprocess_ms=... control_ms=... scheduled_ms=... stabilization_ms=... logger_sync_ms=... pending_edits=... inbox_events=...
```

Do not spam every tick by default. Add a threshold, for example:

```rust
const PERF_LOG_TICK_THRESHOLD_MS: u128 = 8;
```

Only log slow ticks unless a debug/perf flag is enabled.

### Add save timing

Current save already has serialize/write timing. Add:

```txt
lock_wait_ms
clone_or_snapshot_ms
serialize_ms
write_ms
total_ms
```

### Add frontend timing

Add browser-side timings for:

```txt
snapshot_fetch_ms
snapshot_read_body_ms
snapshot_json_parse_ms
snapshot_apply_graph_ms
svelte_flush_or_next_tick_ms
ws_batch_parse_ms
ws_batch_apply_ms
graph_patch_ms
```

Expected UI log:

```txt
[ui] snapshot nodes=3053 bytes=1672467 fetch_ms=14530 read_ms=3 parse_ms=22 apply_graph_ms=11 flush_ms=18 total_ms=14584
```

### Acceptance criteria

* The 14–16s gap is attributed to a named phase.
* Logs clearly distinguish server lock wait, server build, server serialization, network/body read, browser parse, and Svelte apply.
* Rename, duplicate, remove, save, load, and F5 all emit enough data to diagnose.

---

## Phase 1 — Prevent accidental WholeGraph resync

### Objective

Stop ordinary structural edits from falling back to full graph snapshots.

### Files to inspect/change

```txt
submodules/golden_core/crates/core/src/ui_sync.rs
submodules/golden_core/crates/core/src/engine/ui.rs
submodules/golden_core/crates/core/src/engine/apply_tree.rs
submodules/golden_core/crates/core/src/engine/persistence.rs
src-ui/src/lib/golden_ui/store/graph.svelte.ts
src-ui/src/lib/golden_ui/store/workbench.svelte.ts
src-ui/src/lib/golden_ui/transport/http.ts
src-ui/src/lib/golden_ui/types.ts
```

### Required protocol behavior

Normal operations must emit enough information for the UI to patch locally:

#### Rename

Must emit:

```rust
NodeMetaPatched {
    node,
    label,
    short_name_if_changed,
    enabled_if_changed,
    permissions_if_changed,
}
```

No parent order needed unless sort order depends on label.

#### Duplicate

Must emit:

```rust
GraphTransaction {
    ops: [
        NodeCreated { snapshot, parent, index },
        ...
        ChildrenReordered { parent, children },
    ],
}
```

or equivalent.

Every created node must include a full `UiNodeDto` snapshot. The parent must include current child order after insertion.

#### Remove

Must emit:

```rust
SubtreeRemoved {
    root,
    removed_ids,
    parent_after,
}
```

The UI should delete all removed IDs locally. It must not request WholeGraph.

#### Move

Must emit:

```rust
NodeMoved {
    node,
    old_parent,
    new_parent,
    old_parent_after,
    new_parent_after,
}
```

#### Reorder

Must emit:

```rust
ChildrenReordered {
    parent,
    children,
}
```

#### Param value edit

Must emit compact param patch, not node recreation.

### Add assertions

Add debug assertions or tests ensuring these operations do not emit transport resync:

```rust
assert_no_resync_required(rename_node(...));
assert_no_resync_required(duplicate_subtree(...));
assert_no_resync_required(remove_subtree(...));
assert_no_resync_required(move_node(...));
assert_no_resync_required(reorder_children(...));
```

### Acceptance criteria

* Rename does not call WholeGraph.
* Duplicate does not call WholeGraph.
* Remove does not call WholeGraph.
* Move/reorder do not call WholeGraph.
* Svelte graph store applies each event locally.
* Add tests proving `requiresResync` stays false for normal edits.

---

## Phase 2 — Introduce graph transactions

### Objective

Replace long chains of single-node structural events with one atomic transaction primitive.

### Add core types

In `ui_sync.rs` or a nearby protocol module, add or normalize:

```rust
pub struct UiGraphTransaction {
    pub tx_id: u64,
    pub epoch: u64,
    pub base_graph_version: u64,
    pub next_graph_version: u64,
    pub ops: Vec<UiGraphOp>,
}

pub enum UiGraphOp {
    NodeCreated {
        snapshot: UiNodeDto,
        parent: Option<NodeId>,
        index: Option<usize>,
    },
    SubtreeRemoved {
        root: NodeId,
        removed_ids: Vec<NodeId>,
        parent_after: Option<UiChildrenOrderPatch>,
    },
    NodeMoved {
        node: NodeId,
        old_parent: Option<NodeId>,
        new_parent: Option<NodeId>,
        old_parent_after: Option<UiChildrenOrderPatch>,
        new_parent_after: Option<UiChildrenOrderPatch>,
    },
    ChildrenReordered {
        parent: NodeId,
        children: Vec<NodeId>,
    },
    NodeMetaPatched {
        node: NodeId,
        patch: UiNodeMetaPatch,
    },
    ParamPatched {
        node: NodeId,
        param: NodeId,
        patch: UiParamPatch,
    },
    HistoryPatched {
        history: UiHistoryState,
    },
    LoggerPatched {
        records_added: Vec<LogRecord>,
        dropped_before: Option<u64>,
    },
}
```

Add:

```rust
pub struct UiChildrenOrderPatch {
    pub parent: NodeId,
    pub children: Vec<NodeId>,
}

pub struct UiNodeMetaPatch {
    pub label: Option<String>,
    pub short_name: Option<String>,
    pub enabled: Option<bool>,
    pub description: Option<Option<String>>,
    pub user_permissions: Option<NodeUserPermissions>,
}
```

Keep backwards compatibility if the generated protocol/types expect older event names. Prefer adding a new event kind and deprecating older ones gradually.

### Add internal transaction builder

Create something like:

```rust
pub struct UiGraphTransactionBuilder {
    tx_id: u64,
    base_graph_version: u64,
    ops: Vec<UiGraphOp>,
}
```

Use it from duplicate/remove/move/reorder/rename.

### Add transaction invariants

* Transaction ops must be ordered so the UI can apply them deterministically.
* Created parents must appear before created children.
* Removed children can be omitted if `removed_ids` contains all descendants.
* Child order patches must represent post-transaction state.
* Each transaction increments `graph_version` exactly once.
* Transactions must be replayable from event log.

### Acceptance criteria

* Duplicate of a large subtree emits one transaction.
* Remove of a large subtree emits one transaction.
* Move/reorder emit one transaction.
* UI applies one transaction atomically.
* No intermediate inconsistent UI state is visible.

---

## Phase 3 — Introduce immutable `UiReadModel`

### Objective

Move HTTP snapshots and WebSocket replay off the live engine.

### New module

Add a new core module:

```txt
submodules/golden_core/crates/core/src/ui_read_model.rs
```

or under:

```txt
submodules/golden_core/crates/core/src/engine/ui_read_model.rs
```

### Define snapshot model

```rust
use std::sync::Arc;
use std::collections::HashMap;

pub struct UiReadModel {
    current: arc_swap::ArcSwap<UiReadModelSnapshot>,
    event_log: UiEventLog,
}

pub struct UiReadModelSnapshot {
    pub epoch: u64,
    pub graph_version: u64,
    pub protocol_version: String,
    pub at: EngineTime,

    pub nodes: Arc<HashMap<NodeId, Arc<UiNodeDto>>>,
    pub children: Arc<HashMap<NodeId, Arc<[NodeId]>>>,
    pub parent: Arc<HashMap<NodeId, NodeId>>,

    pub schema: Arc<UiSchemaView>,
    pub history: Arc<UiHistoryState>,
    pub logger: Arc<UiLoggerState>,
    pub project_file: Arc<UiProjectFileSpec>,
    pub user_contexts: Arc<UiUserContextsDto>,
}
```

Use `arc-swap` if available. If not available, add it to the relevant `Cargo.toml`, or use `Arc<RwLock<Arc<...>>>` as a temporary bridge. Prefer `arc-swap`.

### Required methods

```rust
impl UiReadModel {
    pub fn from_engine<T: ProjectLifecycle>(engine: &Engine<T>) -> Self;

    pub fn current_snapshot(&self) -> Arc<UiReadModelSnapshot>;

    pub fn apply_transaction(&self, tx: UiGraphTransaction);

    pub fn replace_from_engine<T: ProjectLifecycle>(
        &self,
        engine: &Engine<T>,
        reason: UiReadModelReplaceReason,
    );

    pub fn snapshot_for_scope(&self, scope: UiSubscriptionScope) -> UiSnapshot;

    pub fn replay_since(
        &self,
        client_epoch: u64,
        client_graph_version: u64,
    ) -> UiReplayResult;
}
```

### Event log

Add:

```rust
pub struct UiEventLog {
    epoch: AtomicU64,
    retained_from_version: AtomicU64,
    events: Mutex<VecDeque<Arc<UiGraphTransaction>>>,
    max_events: usize,
    max_bytes: usize,
}
```

Replay result:

```rust
pub enum UiReplayResult {
    Events(Vec<Arc<UiGraphTransaction>>),
    SnapshotRequired {
        reason: UiSnapshotRequiredReason,
        current_epoch: u64,
        current_graph_version: u64,
    },
}
```

Snapshot required only for:

```rust
pub enum UiSnapshotRequiredReason {
    EpochChanged,
    ProtocolChanged,
    ClientTooOld,
    ExplicitDebugRefresh,
}
```

### Important design choice

Do not rebuild `Vec<UiNodeDto>` from engine during snapshot.

The snapshot route should read from `UiReadModelSnapshot`, clone `Arc` data, and only allocate the final DTO payload needed for serialization.

### Acceptance criteria

* HTTP snapshot can be built without locking the engine.
* WebSocket replay can be served without locking the engine.
* F5 no longer waits behind engine runtime ticks.
* WholeGraph snapshot total time becomes close to serialization + HTTP body write + browser parse/apply.

---

## Phase 4 — Introduce `EngineHost` actor

### Objective

Remove direct transport access to `Arc<Mutex<Engine<T>>>`.

### New host shape

Create or refactor transport host into:

```rust
pub struct EngineHost<T: ProjectLifecycle> {
    commands: EngineCommandSender<T>,
    read_model: Arc<UiReadModel>,
    session_id: String,
}
```

Command enum:

```rust
pub enum EngineCommand<T: ProjectLifecycle> {
    UiIntent {
        client_instance_id: Option<String>,
        intent: UiClientIntent,
        reply: oneshot::Sender<Result<UiIntentAck, UiIntentError>>,
    },
    LoadProject {
        path: PathBuf,
        reply: oneshot::Sender<Result<ProjectLoadResult, String>>,
    },
    SaveProject {
        path: PathBuf,
        reply: oneshot::Sender<Result<ProjectSaveResult, String>>,
    },
    NewProject {
        reply: oneshot::Sender<Result<(), String>>,
    },
    Shutdown,
}
```

If the project does not use async runtime internally, use `std::sync::mpsc` plus reply channels. Do not add Tokio unless the transport stack already uses it.

### Runtime ownership

The engine runtime thread owns:

```rust
let mut engine: Engine<T>;
let read_model: Arc<UiReadModel>;
let command_rx: Receiver<EngineCommand<T>>;
```

No other thread owns mutable engine access.

### Main loop

Pseudo-code:

```rust
loop {
    drain_commands_with_budget(&mut engine, &read_model, &command_rx)?;
    run_tick_with_budget(&mut engine, &read_model)?;
    publish_ui_transactions(&read_model)?;
    sleep_or_yield();
}
```

### Bridge path

This can be introduced gradually:

1. Keep old `Arc<Mutex<Engine<T>>>` temporarily.
2. Add `EngineHost` alongside it.
3. Move snapshot reads to `UiReadModel`.
4. Move WS replay to `UiReadModel`.
5. Move UI intents to `EngineCommand`.
6. Move save/load/new to `EngineCommand`.
7. Delete direct transport engine locks.

### Acceptance criteria

* `ui_server.rs` no longer stores `Arc<Mutex<Engine<T>>>` for normal HTTP/WS read paths.
* Runtime loop is the only owner of mutable engine state.
* Snapshot/replay reads use `UiReadModel`.
* UI intents are commands.
* Save/load/new are commands.
* There is no direct `lock_engine` in snapshot, WS replay, or WS dispatch.

---

## Phase 5 — Save without blocking the engine

### Objective

Make save independent from live engine lock.

### Add persistence projection

Create one of these:

Preferred:

```rust
pub struct PersistenceReadModel {
    current: ArcSwap<ProjectPersistenceSnapshot>,
}
```

or integrate into `UiReadModel` if project DTO is already available there.

Snapshot shape:

```rust
pub struct ProjectPersistenceSnapshot {
    pub epoch: u64,
    pub graph_version: u64,
    pub node_count: usize,
    pub sparse_project_json_ready: Option<Arc<String>>,
    pub sparse_project_dto: Arc<SparseProjectDto>,
}
```

### Save path

Instead of:

```rust
lock engine
serialize from engine
drop lock
write disk
```

use:

```rust
clone Arc<ProjectPersistenceSnapshot>
serialize from snapshot on worker thread
write disk on worker thread
return result
```

If precomputing pretty JSON eagerly is too expensive, store a serializable DTO snapshot and serialize on save. The key is: do not hold engine state while serializing.

### Save command behavior

Save command should:

1. Normalize path.
2. Clone persistence snapshot.
3. Spawn blocking worker or use a dedicated persistence worker.
4. Return quickly or reply when finished depending on UI expectations.
5. Never prevent rename/duplicate/remove/snapshot from proceeding.

### Logs

```txt
[project-host] save_project path='...' nodes=... bytes=... snapshot_clone_ms=... serialize_ms=... write_ms=... total_ms=...
```

### Acceptance criteria

* Save does not block snapshot.
* Save does not block rename.
* Save does not block runtime tick.
* Save does not hold mutable engine access during serialization or disk write.
* Saving a large project reports time, but UI stays responsive.

---

## Phase 6 — Load/replace as atomic publish

### Objective

Make loading prepare everything detached, then publish atomically.

### Load flow

```rust
let mut next_engine = load_sparse_project_file(...)?;
configure_loaded_engine(&mut next_engine)?;
prepare_engine_for_runtime(&mut next_engine)?;

let next_ui_model = UiReadModelSnapshot::from_engine(&next_engine);
let next_persistence_model = ProjectPersistenceSnapshot::from_engine(&next_engine);

engine_host.replace_engine(next_engine);
read_model.replace(next_ui_model);
persistence_model.replace(next_persistence_model);
event_log.bump_epoch();
```

### Important

Do not load then make the UI immediately ask the live engine for WholeGraph.

The read model must be available before publishing `ProjectLoaded` / `ProjectReplaced`.

### Logs

```txt
[project-host] load_project path='...' nodes=... file_read_ms=... rebuild_ms=... configure_ms=... prepare_ms=... ui_read_model_ms=... persistence_snapshot_ms=... publish_ms=... total_ms=...
```

### Acceptance criteria

* After load, first UI snapshot reads from prepared read model.
* Load does not expose half-prepared engine state.
* Project replacement bumps epoch and invalidates old event-log replay cleanly.
* UI receives a clear `ProjectReplaced { epoch, graph_version }`.

---

## Phase 7 — Runtime budget and dirty-region recomputation

### Objective

Prevent one tick or structural operation from monopolizing the engine.

### Add runtime budget

```rust
pub struct RuntimeBudget {
    pub max_tick_ms: u64,
    pub max_external_edits_per_tick: usize,
    pub max_pending_ui_commands_per_tick: usize,
    pub max_stabilization_rounds_per_tick: usize,
    pub max_callbacks_per_tick: usize,
}
```

### Add slow phase detection

If any phase exceeds budget, log:

```txt
[engine] slow_phase phase=stabilization elapsed_ms=... nodes_touched=...
```

### Dirty regions

Avoid global recompute after local edits where possible.

Track:

```rust
pub struct DirtyGraphRegions {
    pub topology_dirty: bool,
    pub schedule_dirty: bool,
    pub ui_nodes_dirty: FxHashSet<NodeId>,
    pub ui_children_dirty: FxHashSet<NodeId>,
    pub persistence_dirty: bool,
    pub schema_dirty: bool,
}
```

Use dirty regions to update `UiReadModel` incrementally.

### Acceptance criteria

* Rename does not trigger global resolve unless required.
* Param edit does not trigger structural graph rebuild.
* Duplicate/remove only dirty affected subtree and parents.
* Slow runtime phases are visible in logs.

---

## Phase 8 — Svelte 5 graph store refactor

### Objective

Make frontend graph application and rendering scale.

### Files to inspect/change

```txt
src-ui/src/lib/golden_ui/store/graph.svelte.ts
src-ui/src/lib/golden_ui/store/workbench.svelte.ts
src-ui/src/lib/golden_ui/store/session/*
src-ui/src/lib/golden_ui/transport/*
src-ui/src/lib/golden_ui/types.ts
```

### Normalize graph store

Use normalized state:

```ts
type GraphState = {
  nodes: Map<NodeId, UiNodeDto>;
  children: Map<NodeId, NodeId[]>;
  parent: Map<NodeId, NodeId>;
  nodeVersion: Map<NodeId, number>;
  childrenVersion: Map<NodeId, number>;
  graphVersion: number;
  epoch: number;
};
```

### Patch application

Add:

```ts
applyGraphTransaction(tx: UiGraphTransaction): void
applyNodeCreated(op: NodeCreated): void
applySubtreeRemoved(op: SubtreeRemoved): void
applyNodeMoved(op: NodeMoved): void
applyChildrenReordered(op: ChildrenReordered): void
applyNodeMetaPatched(op: NodeMetaPatched): void
applyParamPatched(op: ParamPatched): void
```

### Svelte 5 rules

* Do not replace the whole graph object for local patches.
* Do not rebuild the whole visible tree after one rename.
* Increment only the affected node/children versions.
* Keep selection, hover, warnings, history, logger in focused stores.
* Keep transport-specific logic out of session state.

### Virtualization

If the tree can display thousands of rows, add or prepare virtualization:

```txt
visible row model = expanded tree projection
rendered rows = viewport slice
```

Even if this is not fully implemented now, isolate tree projection so virtualization can be added without rewriting graph state again.

### UI acceptance criteria

* Rename invalidates one node row, not the full tree.
* Duplicate invalidates created subtree plus affected parent order.
* Remove invalidates removed subtree plus affected parent order.
* F5 parse/apply timing is logged.
* Applying a 3k-node snapshot should be well below one second on a normal dev machine.
* Applying a rename should be effectively instant.

---

## Phase 9 — Codegen / protocol updates

### Objective

Keep Rust protocol and TypeScript types aligned.

### Required commands

Run after changing protocol DTOs:

```bash
npm run codegen:golden-ui-protocol
npm run check
cargo fmt
cargo test -q
cargo check --manifest-path submodules/golden_core/Cargo.toml -q
```

If the standalone `golden_core` test suite has pre-existing module layout failures, document them clearly and still run all viable checks.

### Files likely affected

```txt
submodules/golden_core/crates/core/src/ui_sync.rs
submodules/golden_core/crates/core/src/engine/ui.rs
src-ui/src/lib/golden_ui/types.ts
src-ui/src/lib/golden_ui/store/graph.svelte.ts
```

### Acceptance criteria

* Generated TS types include new transaction/event types.
* Svelte code uses generated types, not hand-rolled duplicates.
* Old events are either supported during migration or removed cleanly with all call sites updated.

---

## Phase 10 — Regression tests and performance tests

### Rust tests

Add tests for:

```txt
rename_does_not_require_whole_graph
duplicate_subtree_emits_single_transaction
remove_subtree_emits_single_transaction
move_node_emits_parent_orders
reorder_children_emits_parent_order
snapshot_from_read_model_does_not_lock_engine
event_log_replays_since_version
event_log_requests_snapshot_when_client_too_old
load_prepares_read_model_before_publish
save_uses_persistence_snapshot_not_live_engine
```

### UI tests

Add tests for graph store patching:

```txt
apply_node_created_inserts_node_and_parent_order
apply_subtree_removed_deletes_descendants
apply_node_moved_updates_old_and_new_parent
apply_children_reordered_preserves_node_objects
apply_node_meta_patch_only_updates_one_node
apply_snapshot_replaces_epoch_cleanly
```

### Performance harness

Add a test or dev utility that creates synthetic graphs:

```txt
1k nodes
5k nodes
10k nodes
50k nodes
deep tree
wide sibling folder
many params
HTTP module-heavy project shape
OSC/value-node-heavy project shape
```

Measure:

```txt
load
initial snapshot
rename
duplicate subtree
remove subtree
save
F5 refresh
WS reconnect/replay
```

### Suggested acceptance targets

For 3k nodes:

```txt
rename < 16ms UI-visible
remove small subtree < 16ms UI-visible
duplicate medium subtree < 50ms UI-visible
snapshot server build+serialize < 100ms
snapshot total server time without lock contention < 150ms
save does not freeze UI
```

For 50k nodes:

```txt
rename remains tiny delta
remove/duplicate scale with affected subtree, not whole graph
snapshot allowed to take longer but never waits on engine lock
save may take time but does not freeze UI
```

Do not overfit exact milliseconds. The core invariant is that ordinary edits scale with affected region, not total project size.

---

## Phase 11 — Update AGENTS.md with permanent rules

Add a section like this:

```md
## Structural performance rules

- UI HTTP/WS read paths must not lock the live mutable engine.
- Normal UI edits must never require `WholeGraph` resync.
- Duplicate, remove, move, reorder, rename, and parameter edits must produce incremental UI deltas.
- Disk I/O and network I/O must never happen while holding live engine state.
- Full graph traversal is allowed on load/replace/debug refresh only, not on ordinary edits.
- Large structural edits must be represented as graph transactions, not long chains of single-node edits.
- AddNodeTree is preferred for prepared subtree creation, but sibling/forest edits must use transaction/batch primitives.
- Snapshot and replay must read from an immutable UI read model or event log, not from the engine.
- Svelte stores must update by entity/version and must not invalidate the whole graph for local edits.
- Every new structural feature must include timing logs and at least one large-graph regression test.
- Any new `resync_required` emission must explain why incremental replay is impossible.
```

---

## Phase 12 — Cleanup old architecture

After the new model works:

### Remove or quarantine

```txt
snapshot path locking live engine
WS dispatch path locking live engine
transport-level direct Engine<T> mutation
ordinary edit path emitting resync_required
whole-graph rebuild after rename
whole-graph rebuild after duplicate/remove/move/reorder
save serialization from live engine guard
```

### Keep only for debug/emergency

WholeGraph snapshot should remain available for:

```txt
first connection
project epoch change
protocol mismatch
event log retention miss
manual debug refresh
corruption recovery
```

---

## Final expected architecture

```txt
UI
  ↓ intents
Transport server
  ↓ commands
EngineHost actor owns mutable Engine<T>
  ↓ publishes transactions
UiReadModel + UiEventLog + PersistenceReadModel
  ↑ snapshots/replay/save read from immutable projections
Transport server
  ↑ HTTP/WS reads without engine lock
Svelte graph store
  ↑ applies normalized patches
```

---

## Final validation checklist

Before considering this done, verify:

```txt
[ ] F5 / WholeGraph snapshot no longer waits on engine lock.
[ ] Rename logs no WholeGraph and no resync_required.
[ ] Duplicate logs one graph transaction.
[ ] Remove logs one graph transaction.
[ ] Save does not freeze UI.
[ ] Load publishes prepared read model.
[ ] WebSocket replay works from event log.
[ ] Event log correctly asks for snapshot on epoch mismatch or client-too-old.
[ ] Svelte graph store applies patches locally.
[ ] Large graph synthetic tests exist.
[ ] AGENTS.md contains structural performance rules.
[ ] cargo test -q passes.
[ ] cargo check --manifest-path submodules/golden_core/Cargo.toml -q is run or documented if blocked by pre-existing layout issue.
[ ] npm run codegen:golden-ui-protocol passes.
[ ] npm run check passes.
```

The critical refactor is: **remove live engine locking from UI reads and move to an engine actor + immutable UI read model + event log + patch-only normal edit protocol**. Everything else supports that.

[1]: https://github.com/Golden-Geek/Chataigne2 "GitHub - Golden-Geek/Chataigne2: Chataigne comes back. · GitHub"
