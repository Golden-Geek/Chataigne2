# Performance Improvement Plan

Status: in progress
Scope: `Chataigne2`, `golden_core` (`golden_engine`), `golden_alchemist_core`, UI sync boundary
Audience: an AI coding agent executing phases in order

This plan reconciles two independent performance audits (an app-layer /
state-machine audit and a `golden_core` engine-substrate audit) into one
ordered, testable roadmap. The target workload is the one the repository
already commits to in `AGENTS.md`:

```text
Complex live sessions.
Many modules updating many values at high rates.
Many formulas and processors live at the same time.
Tens of thousands of nodes.
Smooth main engine loop.
Smooth engine <-> UI sync.
```

## How To Execute This Plan

- Execute phases in order. Later phases assume earlier invariants hold.
- Every phase ends with acceptance tests. Do not start the next phase with
  failing acceptance tests.
- Follow `AGENTS.md` working rules: correct boundary over nearest call site,
  no compatibility glue, tests in separate files, finish with `cargo fmt` on
  root and `golden_core`.
- Some code-level claims in this plan were produced by static audit and may
  drift from the tree. Phase 0 requires verifying each claim before editing.
  If a claim is wrong, record the correction in this document and adapt the
  phase; do not silently skip it.

## Locked Direction

These are the decisions this plan commits to. Individual phases implement
them; they are not re-debated per phase.

```text
D1. Value changes never trigger recompilation or formula rematerialization.
    Values flow through runtime frames (RuntimeInputSnapshot / lane frames),
    never through mutation of authored or materialized formula graphs.

D2. One compiled formula per (formula id, version, registry revision),
    shared via Arc across all processors, in every runtime host.
    FormulaCompileKey is the identity.

D3. StateMachineManager becomes a thin host over ChataigneStateMachineRuntime.
    It translates node-tree structure into the machine model incrementally
    and delegates compilation, lifecycle, and ticking. It does not own a
    second compile/eval orchestration.

D4. Debug/preview capture is subscription-driven and bounded in every
    production evaluation path. Unbounded capture is test/inspector-only.

D5. Full ProcessTreeSnapshot construction is a budgeted slow path.
    Steady-state ticks in a large idle-ish session build zero full snapshots
    unless a due node genuinely requires one; the goal state is an
    incremental snapshot so even that costs O(changes).

D6. Engine -> UI traffic is split into a reliable ordered structure channel
    and a lossy latest-wins value telemetry channel throttled to UI frame
    rate. UI bandwidth scales with UI frame rate, not engine event rate.

D7. Every performance property fixed in this plan gets a regression test
    (counter-based or timing-based) that fails when the property regresses.
```

## Known Hotspot Map (verified references)

App layer (verified against the tree during audit):

- `src/state_machine_nodes/manager.rs`
  - `run_processors` calls `collect_formulas(snapshot)` unconditionally per
    update at `STATE_MACHINE_RUNTIME_HZ` (full formula rematerialization).
  - `mark_formula_input_value_dirty` sets `runtime_cache.dirty = true`, so
    formula input value changes route through `rebuild_runtime_cache` and
    `compile_preserving_compatible_lanes` (value -> recompile amplification).
  - `run_processors` passes `RuntimeInputSnapshot::default()` and injects
    values by mutating `input_defaults` on rematerialized formulas via
    `apply_pending_trigger_inputs`.
  - `rebuild_runtime_cache` calls `runtime.compile(...)` per processor node
    with no `FormulaCompileKey` dedupe (violates D2 in the shipping host).
  - `run_processors` evaluates with
    `ProcessorDebugCapture::All { history_len: usize::MAX }` (violates D4).
  - `output_preview_signature` and `should_emit_runtime_log` build `format!`
    string keys per sample/log per tick.
  - `update_requires_tree_snapshot()` returns `true` unconditionally;
    `on_meta_changed`, `on_node_created`, `on_node_deleted`, and the
    fallback arm of `on_param_change` all set full `dirty = true`.
- `src/state_machine/src/state_machine.rs`
  - `ChataigneStateMachineRuntime::compile` already implements D2 correctly
    (`FormulaCompileKey` dedupe, shared `Arc<CompiledAlchemistFormula>`).
    This is the delegation target for D3.

Current implementation notes verified on 2026-07-02:

- `golden_core` and `golden_alchemist_core` are path dependencies under
  `submodules/`, not workspace members under root `crates/`.
- `golden_core` already has `TickStats` in
  `submodules/golden_core/crates/core/src/engine/tick_scratch.rs` with
  counters for due nodes, callbacks, emitted events, edits, stabilization
  passes, and snapshot rebuilds. The wider Phase 0 counter set remains TODO.
- `golden_alchemist::DebugCaptureMode::default()` now defaults to `Off`.
- `golden_alchemist` now compiles unconnected socket defaults as runtime-input
  sources with authored/default fallback via `formula_input_value_ref`.
- `StateMachineManager` now keeps separate topology, formula-structure,
  processor-override, and value-dirty planes. Formula input value changes update
  a runtime input frame and no longer force formula rematerialization or manager
  cache rebuilds when the formula is already cached.
- The shipping manager host now caches project formula materialization and
  shares one compiled formula `Arc` per `FormulaCompileKey` across processors,
  invalidating compiled entries when a formula subtree is structurally dirty.
- Runtime preview/log dedupe in `StateMachineManager` now uses typed keys and
  value signatures instead of per-sample formatted string keys.

Engine substrate (from the second audit; verify in Phase 0):

- `build_process_tree_snapshot()` is O(total nodes), clone-heavy; cached per
  tick via `get_or_build_tick_snapshot` but still forced every tick by any
  due node with `update_requires_tree_snapshot() == true`.
- `precompute_inbox_dispatch_since` clones each routed `Event` into a
  per-recipient `Vec` (O(events x recipients) allocation).
- `dispatch_precomputed_inbox_internal` builds a full snapshot whenever any
  per-node events exist.
- `evaluate_parameter_controls()` scans all nodes for parameter snapshots
  once any active control exists; expression controls run QuickJS on tick.
- Reference sync rebuilds a UUID map and scans all nodes; reference picker
  candidate collection is full-graph.
- `Node::needs_update()` defaults to `true`.
- `evaluate_compiled_graph` walks every exec node in topological order per
  evaluation, allocates a fresh input `Vec` per node, and clones values for
  change comparison (O(exec_nodes x lanes) even when almost nothing changed).
- `DebugCaptureMode::default()` in `golden_alchemist_core` is
  `All { history_len: usize::MAX }`.
- Stabilization is bounded (default 16 passes) — already healthy.
- `NodeStore` on `SlotMap`, `ListenerIndex`, bucketed scheduler,
  `AddNodeTree` + `SubtreeInserted` batching — already healthy; reuse, do
  not rebuild.

---

# Phase 0 — Verify, instrument, baseline

## Goal

Confirm the hotspot map against the actual tree, and put counters/tests in
place so every later phase can prove its effect.

## Steps

1. Verify each "engine substrate" claim above by reading the referenced
   code (`crates/core/src/engine/`, `golden_alchemist_core` runtime). Update
   the Known Hotspot Map section with exact function/file names where the
   audit's names drift.
2. Promote tick-phase timing from threshold `eprintln!` to structured
   counters on `TickStats` (or the equivalent), adding at minimum:
   `snapshot_builds`, `snapshot_build_ns`, `snapshot_nodes_cloned`,
   `dispatch_events_routed`, `dispatch_recipient_deliveries`,
   `dispatch_max_fanout`, `schedule_resolves`, `controls_params_scanned`,
   plus app-layer counters: `sm_formula_materializations`,
   `sm_formula_compiles`, `sm_runtime_cache_rebuilds`,
   `sm_debug_samples_captured`.
3. Add a perf test harness module (behind `#[ignore]` or a `perf` feature)
   in the pattern of `src/module/perf_tests.rs`, with builders for synthetic
   sessions: N passive nodes, N modules with M values, F formulas with P
   processors, K ANodes per formula.
4. Record baseline numbers for the Perf Test Matrix scenarios (end of this
   document) in a `docs/perf-baselines.md` table.

## Acceptance

```text
Hotspot map corrected and committed.
Counters exposed and asserted readable from tests.
Baselines recorded for all matrix scenarios.
No behavior changes.
```

---

# Phase 1 — Manager dirty-plane split (kills value -> recompile)

Highest-impact phase. Target file: `src/state_machine_nodes/manager.rs`.

## Goal

Formula input value changes cost one lane re-evaluation, never a cache
rebuild, never a compile, never a formula rematerialization.

## Steps

1. Replace the single `runtime_cache.dirty` flag with three explicit planes
   on `StateMachineRuntimeCache`:
   - `value_dirty: HashSet<NodeUuid>` (formula uuids with changed input
     values) — triggers re-evaluation only.
   - `override_dirty: HashSet<NodeId>` (existing
     `dirty_processor_overrides`) — refreshes processor property frames.
   - `structure_dirty: HashSet<NodeUuid>` plus a global
     `topology_dirty: bool` — the only plane allowed to rematerialize and
     recompile, scoped to affected formulas where possible.
2. Rewire invalidation:
   - `mark_formula_input_value_dirty` writes to `value_dirty` only. It must
     stop setting the structural flag.
   - `on_meta_changed` / `on_node_created` / `on_node_deleted` /
     `on_child_added` / `on_child_removed`: classify by subtree. Changes
     inside a formula library subtree mark that formula `structure_dirty`.
     Changes inside a processor node mark `override_dirty`. Changes that
     affect state/transition topology mark `topology_dirty`. Everything
     else is ignored by the runtime cache. The current fallback
     `dirty = true` arm in `on_param_change` must be removed; unmatched
     param changes do not touch the runtime cache.
3. Route values through runtime frames (implements D1):
   - Introduce a per-formula (or per-manager) input frame that
     `mark_formula_input_value_dirty` writes into: a map from
     (formula uuid, anode uuid, socket id) to `RuntimeValue`, with the
     existing trigger-edge treatment preserved (edge ids, fired-at tick).
   - `run_processors` builds the `RuntimeInputSnapshot` (or the equivalent
     lane frame the compiled runtime consumes) from that map instead of
     passing `RuntimeInputSnapshot::default()` and mutating
     `input_defaults` on materialized formulas.
   - `apply_pending_trigger_inputs` moves to the frame path; formula
     mutation for value injection is deleted.
   - If the compiled graph currently reads authored `input_defaults` at
     compile time only, extend compilation so input-value sockets read from
     runtime input refs (this is the contract `ALCHEMIST_FORMULA_RUNTIME.md`
     already specifies with `PropertyFrame`/`RuntimeInputSnapshot`). Do this
     in `golden_alchemist_core` at the correct boundary rather than
     app-side shims.
4. Cache formula materialization: keep a
   `HashMap<NodeUuid, (StructureVersion, AlchemistFormula)>` populated on
   demand and invalidated by `structure_dirty`. `collect_formulas(snapshot)`
   is only called for formulas whose cache entry is missing or stale;
   the unconditional per-update call is deleted.

## Non-goals

Compile dedupe (Phase 2), capture bounding (Phase 3), delegating to
`ChataigneStateMachineRuntime` (Phase 2/3 prepare it; full delegation may
land with them).

## Acceptance

```text
Perf test: 1_000 formula input value changes across one tick cause
  sm_runtime_cache_rebuilds == 0
  sm_formula_compiles == 0
  sm_formula_materializations == 0 (all cache hits)
  evaluated lanes reflect the new values on the same tick.
Perf test: editing an unrelated module parameter causes zero manager
  cache activity.
Existing state_machine_nodes tests pass (update any that asserted the old
  rebuild-on-value behavior; they encode the bug, replace them).
```

---

# Phase 2 — Shared compiles in the shipping host (D2)

## Goal

N processors on one formula share one `Arc<CompiledAlchemistFormula>` in
`manager.rs`, matching the already-proven behavior of
`ChataigneStateMachineRuntime::compile` and the
`ten_thousand_stateless_processors_share_compile_and_allocate_one_process_cache`
test.

## Steps

1. Add a `HashMap<FormulaCompileKey, Arc<CompiledAlchemistFormula>>` to the
   manager runtime cache. Invalidate entries only from `structure_dirty`.
2. In `rebuild_runtime_cache`, replace per-processor
   `runtime.compile(...)` / `compile_preserving_compatible_lanes(...)` with:
   look up or compile once per key, then
   `runtime.compile_from_shared_formula(...)` per processor, preserving
   compatible lanes when the formula version is unchanged.
3. Preferred shape (D3): extract the compile-key cache by reusing or thinly
   wrapping `ChataigneStateMachineRuntime::compile`'s existing dedupe rather
   than duplicating it in the manager. If full delegation is tractable now,
   take it; otherwise leave a `// D3` marker and keep the cache manager-side.

## Acceptance

```text
Perf test: 100 processors referencing 1 formula produce
  sm_formula_compiles == 1 after a structural change, and all 100 runtimes
  satisfy Arc::ptr_eq on their compiled formula.
Perf test: changing one formula's structure recompiles only that formula's
  key; processors on other formulas keep their Arc (ptr_eq stable).
```

---

# Phase 3 — Bounded capture and hot-loop hygiene (D4)

## Goal

Production evaluation captures nothing unless a UI subscription asks, and
the manager hot loop stops allocating strings per tick.

## Steps

1. Replace `ProcessorDebugCapture::All { history_len: usize::MAX }` in
   `run_processors` with capture derived from active preview subscriptions
   (the `DebugPreviewSession` contract in `ALCHEMIST_FORMULA_RUNTIME.md`):
   `Off` when no editor/inspector is attached to a formula or processor;
   `ProcessorLane { context_key, history_len }` with a small bounded
   history when one is. Wire subscription state from the existing preview
   protocol (`OutputPreviewStatus`, selected-lane plumbing already exists).
2. Change `DebugCaptureMode::default()` in `golden_alchemist_core` to a
   bounded/off default, or forbid production calls to paths that use the
   default (add an architecture test). Direct `AlchemistRuntime::evaluate()`
   in production paths must pass explicit capture.
3. Replace `format!`-string keys in `output_preview_signature` and
   `should_emit_runtime_log` with hashed/tuple keys
   (`(ProcessorId, ANodeId, SocketId, value hash/revision)`); reuse
   buffers where strings are unavoidable for display.

## Acceptance

```text
Perf test: 50 processors x 200-node formula evaluated for 100 ticks with no
  preview subscription yields sm_debug_samples_captured == 0 and no
  per-tick allocation growth in the capture path.
Preview behavior test: attaching a lane preview yields exactly the
  subscribed samples with bounded history; detaching returns capture to Off.
```

---

# Phase 4 — Engine substrate P0 set

All in `golden_core` (`crates/core/src/engine/`, `node/`, `parameter/`).
Verify exact names from Phase 0 before editing.

## 4a. Snapshot budget and snapshot-free dispatch (toward D5)

1. Make inbox dispatch snapshot-free by default: preprocess callbacks get no
   full snapshot unless the recipient declares it (mirror the existing
   `update_requires_tree_snapshot` opt-in pattern for preprocessing).
2. Add call-site reason tagging to snapshot builds (counter label), and a
   regression test: a plain parameter edit plus its dispatch in a 10k-node
   session builds zero full snapshots.
3. `StateMachineManager::update_requires_tree_snapshot()` becomes
   conditional after Phase 1: `true` only when structure/topology planes are
   dirty or a preview subscription needs snapshot-backed identity mapping.
   Steady-state value flow must not force the per-tick snapshot.

## 4b. Event references instead of per-recipient clones

Store routed events once (`Arc<Event>` or an event arena with indices) and
give recipients index slices. Remove the default preprocess `ctx.events`
clone. Add counters from Phase 0 to prove delivery cost scales with
`events_routed`, not `events x recipients` clone volume.

## 4c. Indexed active parameter controls

Maintain `active_control_params: IndexSet<NodeId>` plus a source-param ->
dependent-controls map, updated on control config/structure events.
`evaluate_parameter_controls` iterates the index and changed sources only.
Expression controls: evaluate on source change (or explicit rate), never as
an unconditional every-tick pass; keep QuickJS work off the steady-state
tick.

## 4d. Bulk creation is the only structural path

Audit paste/import/duplicate/template/auto-add flows (including OSC
auto-add and Formula ANode generation) and route any per-node creation
loops through `AddNodeTree` with prebuilt subtrees (sockets/config children
included), one lifecycle batch, one `SubtreeInserted` UI transaction.

## Acceptance

```text
Perf test (10k passive nodes, one param edit): zero schedule resolves,
  zero full snapshot builds, dispatch time bounded.
Perf test (1k param changes, low fan-out): dispatch cost scales ~O(changes);
  recipient_deliveries counter matches routing expectations; no per-recipient
  event clone allocations.
Perf test (1k controlled params, 10 changed sources): control pass touches
  only dependents of the 10 sources.
Perf test (paste 1k-node subtree): one AddNodeTree, one subtree UI
  transaction, bounded lifecycle snapshot count.
```

---

# Phase 5 — Alchemist runtime: dirty propagation

Target: `golden_alchemist_core` (`compile_graph`, `evaluate_compiled_graph`,
`AlchemistMemory`). Needed for tens of thousands of live linked ANodes;
only pays off after Phases 1–3 remove rebuild amplification.

## Steps

1. Compile-time: emit output-slot -> dependent-exec-node adjacency alongside
   the topological order. Use existing analysis
   (`has_always_process_nodes`, `has_input_gated_nodes`, state/effect axes)
   to select a runtime strategy per compiled graph:
   - Pure/input-gated graphs: dirty-queue propagation — seed dirty from
     changed inputs/properties/events, execute only reachable dirty nodes in
     topo order.
   - Graphs with always-process/time-dependent nodes: hybrid — always-set
     seeds every tick, everything else propagates.
2. Slot revision counters: bump a `u64` revision per value slot on write;
   input-change gating compares revisions before cloning values. Keep value
   comparison only where revision equality is insufficient (e.g. triggers'
   fired-edge semantics — preserve existing trigger tick/edge behavior).
3. Scratch buffers: per-runtime reusable input scratch
   (`SmallVec<[RuntimeValue; 4]>` or arity-class buffers); delete per-node
   `Vec` allocation in the eval loop.
4. Preserve lane semantics: dirty sets are per-lane where lane-varying data
   exists; the sparse-lane invariants from `ALCHEMIST_FORMULA_RUNTIME.md`
   (stateless lanes allocate no memory, stateful only for used lanes) must
   hold unchanged — keep the existing lane tests green.

## Acceptance

```text
Perf test (5k-ANode formula, one changed source): executed node count ==
  affected downstream count, not 5k; wall time scales with affected set.
Perf test (same formula, idle tick, no always-process nodes): zero exec
  nodes run.
All existing golden_alchemist_core runtime/lane tests pass unchanged in
  semantics (update only for new counters/strategy plumbing).
Microbench: eval loop shows no per-node heap allocation under a steady
  workload (assert via counter or allocator hook in the perf harness).
```

---

# Phase 6 — UI sync channels and scoped subscriptions (D6)

Targets: `crates/core/src/ui_sync.rs`, `crates/transport_server/src/ui_server.rs`,
`src-ui/src/lib/golden_ui/transport/`, stores.

## Steps

1. Classify UI events into structure-plane (create/remove/move/meta/edit
   acks/history — reliable, ordered, never dropped) and value-plane (param
   values, preview samples, lane summaries, logger volume — latest-wins).
2. Value-plane coalescing in the retained event log: one latest entry per
   (node, kind); structural entries retained verbatim. This bounds log
   growth under sustained high-rate values and fixes slow-client catch-up.
3. Throttle value-plane flush to a configurable UI frame rate (default
   30–60 Hz), latest-wins per param. Structure-plane flushes stay immediate.
4. Scoped subscriptions: extend `UiSubscriptionScope` beyond `WholeGraph` to
   subtree/panel scopes (outliner: hierarchy+meta; inspector: one subtree;
   formula canvas: one formula projection + its preview session). Scope
   structural graph transactions so churn in unrelated subtrees does not
   fan out to every client. Keep logger records out of graph snapshots —
   separate channel.
5. Optional (measure first): compact/binary encoding for the value channel
   if JSON serialization shows up in Phase 0 counters after 1–4.

## Acceptance

```text
Perf test: a param updating at 200 Hz for 5 s produces <= frame-rate-cap
  value messages per second on the wire and exactly the latest value on the
  client; structure events during the same window all arrive, in order.
Perf test: event log size under sustained value churn is bounded by
  (params x 1) + structural entries, not by event count.
Reconnect test: scoped resync transfers only the subscribed scope.
```

---

# Phase 7 — Reference indexes and remaining P2

1. Persistent UUID -> NodeId index maintained on add/remove, replacing
   on-demand map rebuilds in reference sync.
2. Reverse reference index (target -> referencing params); recompute
   missing-reference warnings only for impacted params on structural change.
3. Reference picker: cached candidate sets keyed by
   (constraint, root, filter revision); viewport/search-subset querying for
   huge graphs — no full-graph scan per open/keystroke.
4. Scheduler micro-optimizations (multi-bucket due-merge scratch storage)
   only after Phases 1–6 land and counters show it matters.

## Acceptance

```text
Perf test (50k-node project): reference picker open and per-keystroke
  filtering do not scan all nodes (assert via counter).
Structural edit recomputes missing-reference warnings only for impacted
  params.
```

---

# Perf Test Matrix (merged, final gate)

All scenarios run in the Phase 0 harness; each asserts counters and/or a
timing budget against the recorded baseline.

```text
S1  10k passive nodes, one param edit
    -> 0 schedule resolves, 0 full snapshots, bounded dispatch time
S2  10k nodes, 1k param changes, low fan-out
    -> dispatch ~O(changes); no per-recipient event clones
S3  High fan-out listeners
    -> recipient_deliveries and max_fanout counters exposed and bounded
S4  1k formula-input value changes in one tick            [guards Phase 1]
    -> 0 rebuilds, 0 compiles, 0 rematerializations; values visible same tick
S5  100 processors x 1 formula                            [guards Phase 2]
    -> 1 compile, Arc shared across all runtimes
S6  50 processors, no preview subscription, 100 ticks      [guards Phase 3]
    -> 0 debug samples captured
S7  5k-ANode formula compile
    -> compile time measured; no UI transaction explosion
S8  5k-ANode formula, one changed source                   [guards Phase 5]
    -> executed nodes == affected downstream set
S9  Paste/import 1k-node subtree
    -> one AddNodeTree, one SubtreeInserted, bounded lifecycles
S10 Enable/disable large subtree
    -> one schedule resolve, one effective-enabled recompute
S11 200 Hz param for 5 s with connected UI client          [guards Phase 6]
    -> wire messages <= frame cap; latest value correct; log bounded
S12 Reference picker in 50k-node project                   [guards Phase 7]
    -> no full-graph scan per interaction
```

The plan is complete when the milestone session is smooth:

```text
One synthetic 10k-node project.
Modules feeding 1k values/second into formula inputs.
20 formulas, 200 processors, shared compiles.
UI connected with scoped subscriptions.
Editing, dispatching, evaluating, and syncing concurrently,
with no full-graph resync and tick time inside budget.
```

# Node Author Rules (enforce in review from Phase 4 on)

```text
Periodic execution rules require a cheap needs_update implementation.
update_requires_tree_snapshot is rare and justified per call site.
Preprocess callbacks neither clone nor scan event bundles they do not need.
Structural creation from callbacks batches as NodeTree / AddNodeTree.
Device/network parsing lives outside the engine loop; callbacks consume
prepared events.
Expression controls are script execution, not cheap parameter math.
Debug capture and runtime logging default off in production paths.
```
