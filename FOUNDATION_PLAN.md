# Chataigne2 / golden_core — Foundation Plan

A complete plan to lock in performance, architectural cleanliness, and AI-resilient development practices before the project grows past the point where they're cheap to enforce.

This document is meant to live in the repo (as `FOUNDATION_PLAN.md` or split across `AGENTS.md` / `CONTRIBUTING.md` / `docs/perf/`). It assumes the engine described in the analysis: a node graph runtime, scheduler with rate buckets, event inbox/dispatch, edit pipeline, stabilization rounds, target of 50–200 fps with up to 20k+ nodes.

---

## 0. Guiding principles

These come first because every later decision flows from them. When in doubt, fall back to these.

1. **The tick path is sacred.** Anything that runs every tick is held to a different standard than anything else. No allocations if avoidable, no full-graph scans, no parent walks, no debug formatting, no string work.
2. **Sparse work for sparse changes.** A frame where 10 nodes update should cost on the order of 10 nodes of work, not 20k nodes of overhead. If you can't show that holds, the design is wrong.
3. **Caches must have an invalidation owner.** Every cached or derived value names the events that invalidate it, in a comment next to the field. No exceptions. This is the single rule that most protects you from AI drift.
4. **One mutation channel.** All structural and parameter changes go through `Edit`. No back doors. If a feature seems to need a back door, the back door is the bug.
5. **Phases are explicit and one-way within a tick.** Ingress → apply edits → resolve → scheduled updates → dispatch → stabilize → emit UI. Reentrancy is a smell; bounded stabilization is a guardrail, not a design.
6. **Worst case, not average case.** p99 frame time is the metric. Average fps lies. A 30 ms spike every 200 ticks ruins sequence timing even if the average is 2 ms.
7. **Benchmarks are part of the contract.** A perf-relevant change without a benchmark delta is incomplete, the same way a feature without tests is incomplete.
8. **Boring is good.** A simple structure that performs is better than a clever structure that performs slightly better. AI assistants generate clever code by default; push back toward boring.

---

## 1. Phase plan (chronological)

The work is sequenced so each phase builds on the previous one and you can ship value at every checkpoint. Each phase ends with a "definition of done" that is not subjective.

### Phase 0 — Instrumentation and baseline (week 1)

> **STATUS: PARTIAL** — per-phase wall-clock timing in `run_tick` (`eprintln` above 8 ms threshold) is done. Criterion benchmarks, baseline JSON, and `tick_stats()` accessor are NOT done.

You cannot fix what you cannot measure. Do this first or every later step is guesswork.

**Tasks**

- [x] Add per-phase timing inside `run_tick` (resolve, absorb, apply, precompute, preprocess, control, scheduled, stabilization, logger-sync).  Timing is always on and logged to stderr above `PERF_LOG_TICK_THRESHOLD_MS = 8 ms`.
- [ ] Gate timing behind a `perf` feature flag so release builds with no perf concern pay nothing.
- [ ] Add a per-tick counter struct (`tick_stats()` accessor): nodes due, callbacks fired, events emitted/routed, edits applied, stabilization passes, snapshot rebuilds, param-map rebuilds.
- [ ] Add a Criterion benchmark crate at `crates/core/benches/`. Three scenarios:
  - `tick_20k_passive`, `tick_20k_sparse_active`, `dispatch_10k_with_1k_listeners`
- [ ] Record baseline and commit as `crates/core/benches/baseline.json`.

**Definition of done**

- `cargo bench` runs the three scenarios.
- Baseline p50, p95, p99, and max tick times are recorded for each.
- Per-tick stats are accessible from a unit test.

**Why this is Phase 0**: every later phase is justified by deltas against this baseline. Without it, you're guessing whether the optimizations worked.

---

### Phase 1 — Cache `effective_enabled` (week 1, ~1 day)

> **STATUS: DONE** — `NodeData.effective_enabled: bool` stores the cache inline on each node (avoids a separate HashMap). All invalidation paths are wired. Hot-path `is_enabled(true)` is an O(1) field read.

Cheapest big win. Removes parent-chain walks from hot paths.

**Tasks**

- [x] Cache field: `effective_enabled: bool` on `NodeData` (default `true`; `#[serde(skip)]`).
- [x] `AddNode` / `AddUserItem`: initialize from parent in `apply_add_node_with_role` after `attach_node`.
- [x] `AddNodeTree`: initialize entire subtree before any callbacks fire (`apply_add_node_tree`).
- [x] `MoveNode`: recompute subtree via `subtree_effective_enabled_changes` + `queue_effective_enabled_callbacks` (`apply_move_node`). Only fires when parent actually changes.
- [x] `PatchMeta` when `enabled` changes: existing `queue_effective_enabled_callbacks` path in `apply_patch_meta`.
- [x] `is_enabled(node, true)` delegates to `effective_enabled` field (no parent walk). Doc-comment names invalidation events.
- [x] `is_effectively_enabled()` kept as parent-chain walker — used only for initialization, not in the hot path.

**Invalidation owner** (now on `NodeData`):

```rust
/// Effective enabled state accounting for all ancestors.
/// Written by the engine before each `on_effective_enabled_changed` call;
/// nodes can read this instead of querying the tree snapshot.
///
/// INVALIDATED BY:
///   - AddNode/AddUserItem/AddNodeTree (compute from parent at insert time)
///   - MoveNode (subtree_effective_enabled_changes + queue_effective_enabled_callbacks)
///   - PatchMeta when meta.enabled changes (queue_effective_enabled_callbacks subtree)
///
/// READ FROM: is_enabled(true), run_scheduled_updates, collect_execution_rules.
pub effective_enabled: bool,
```

**Definition of done**

- [x] No parent-chain walk inside `run_tick` — `is_enabled(true)` is now an O(1) field read.
- [ ] `tick_20k_passive` benchmark delta (needs Phase 0 benchmarks first).
- [ ] Unit test: toggle a parent's `enabled`, verify all descendants' cached values flip.

---

### Phase 2 — Incremental parameter value store (week 2, ~3–5 days)

> **STATUS: PARTIAL** — `parameter_values_cache: HashMap<NodeId, ParamValue>` exists on `Engine` and is shared by both `run_scheduled_updates` and `dispatch_precomputed_inbox_internal`. Cache is rebuilt from `self.nodes.iter()` when `parameter_values_dirty` is true (set on any structural change). `dispatch_precomputed_inbox_internal` was fixed to use this cache instead of doing its own full scan. A proper incremental `ParamStore` (populated on AddNode, updated on ParamChanged, removed on RemoveNode) is still TODO.

The biggest single win. Removes the per-tick `HashMap` rebuild that dominates frames at scale.

**Tasks**

- [x] `parameter_values_cache: HashMap<NodeId, ParamValue>` on `Engine`; rebuilt when `parameter_values_dirty`.
- [x] `run_scheduled_updates`: uses cache; updates it incrementally for `ParamChanged` events within the tick.
- [x] `dispatch_precomputed_inbox_internal`: now uses the shared cache instead of its own `self.nodes.iter()` scan.
- [ ] Proper `ParamStore` struct with `generation` and `per_node_generation` fields.
- [ ] Populate at `AddNode` family instead of marking dirty.
- [ ] Update exactly on `ParamChanged` event (not "mark all dirty on structural change").
- [ ] Remove on `RemoveNode`.
- [ ] Debug-mode consistency assertion (panic if `ParamChanged.new_value` ≠ store value).
- [ ] Audit: no remaining `self.nodes.iter()` in tick-path functions to build a param map.

**Definition of done**

- [ ] Grep for `self.nodes.iter()` inside `runtime.rs` and `dispatch.rs` returns zero in tick-path functions.
- [ ] `tick_20k_sparse_active` benchmark delta (needs Phase 0 benchmarks).
- [ ] Debug-mode consistency assertion in place.
- [ ] Test: `Edit::SetParam` → store updated → bound handle resolves correctly.

---

### Phase 3 — Listener reverse index (week 2, ~2 days)

> **STATUS: NOT STARTED** — `event_listeners: HashMap<NodeId, HashSet<EventSubscription>>` is the flat forward-only map. `collect_subscription_recipients` still iterates all subscribers.

Fixes the per-event listener scan in `collect_subscription_recipients`.

**Tasks**

- [ ] Replace `event_listeners: HashMap<NodeId, HashSet<EventSubscription>>` with `ListenerIndex` wrapper:
  ```rust
  pub(crate) struct ListenerIndex {
      by_subscriber: HashMap<NodeId, Vec<EventSubscription>>,
      by_origin: HashMap<NodeId, Vec<NodeId>>,
  }
  ```
- [ ] `collect_subscription_recipients`: look up `by_origin` for each ancestor in `ancestry_depths`.
- [ ] All four ops stay O(small): `add`, `remove`, `purge_for_node`, `recipients_for`.
- [ ] Use `Vec` not `HashSet` (per-subscriber counts almost always < 10).

**Definition of done**

- [ ] `dispatch_10k_with_1k_listeners` benchmark 5x p50 improvement.
- [ ] Property test: N random subscribers, M random events, reverse-index matches brute-force.

---

### Phase 4 — Tick-path discipline pass (week 3, ~3 days)

> **STATUS: NOT STARTED**

Now that the big-O wins are in, tighten the tick loop itself.

**Tasks**

- Audit `run_tick` end to end. For each phase, document:
  - What state it reads.
  - What state it writes.
  - Whether it can emit events (and if so, what kinds).
  - Whether it can append to `self.edits.pending`.
- Replace the unbounded reentrancy in `run_stabilization_rounds` with explicit categories:
  - **Allowed in stabilization**: param-derived recomputation, dependent updates triggered by `ParamChanged`.
  - **Forbidden in stabilization**: structural edits (`AddNode`, `RemoveNode`, `MoveNode`). If a node's update wants to mutate structure, that goes into a deferred queue and is applied at the start of the next tick.
- Lower `max_stabilization_passes_per_tick` from 256 to 16. Anything past 4 logs a structured warning. Anything past 16 errors. The current 256 limit hides graph design bugs.
- Add scratch buffers on the engine to avoid per-tick allocation:
  ```rust
  pub(crate) struct TickScratch {
      due_nodes: Vec<NodeId>,
      due_counts: HashMap<NodeId, usize>,
      seen_by_node: HashMap<NodeId, usize>,
      remaining_delta_by_node: HashMap<NodeId, Duration>,
      recipients: Vec<NodeId>,
      recipients_dedupe: HashSet<NodeId>,
  }
  ```
  Clear, don't drop, between ticks.

**Definition of done**

- `run_tick` has a doc-comment phase diagram listing the 7 phases, what they read/write, and what they can emit.
- Stabilization limit is 16 with a warning at 4.
- Tick-path allocations in steady state (no structural changes) are zero. Verify with a `dhat` or `tracking-allocator` test.

---

### Phase 5 — Scheduler bucket collection (week 3, ~2 days)

> **STATUS: NOT STARTED**

Removes the per-tick `topo_order` walk inside `collect_due_nodes`.

**Tasks**

- Pre-sort each bucket's `nodes` vector by topo index at resolve time. Store the topo-ordered node list once per bucket; never re-walk the global `topo_order` to collect due nodes.
- Track due buckets in a small set instead of scanning all buckets:
  ```rust
  due_buckets: SmallVec<[BucketId; 8]>,
  ```
- For multiple due buckets in the same round, k-way merge their topo-ordered lists by topo index. With most realistic graphs this is 2–3 buckets, so a tiny fixed-size merge is fine.

**Definition of done**

- `tick_20k_passive` shows that bucket collection cost is now proportional to due-node count, not total node count.
- A test with 20k nodes and 0 due buckets that verifies `collect_due_nodes` does no per-node work.

---

### Phase 6 — Compact subtree events (week 4, ~3 days)

> **STATUS: NOT STARTED**

Stops UI-event floods on bulk operations.

**Tasks**

- Add `EventKind::SubtreeInserted { root, parent, prev_sibling, node_count, summary }` and `EventKind::SubtreeRemoved { root, parent, node_count, summary }`. The summary carries enough metadata for the UI to virtualize without forcing per-node events.
- When `Edit::AddNodeTree` is applied with > N children (start with N=8), emit one `SubtreeInserted` instead of N `NodeCreated` + N-1 `ChildAdded`.
- The UI side must learn to consume these. That belongs in `Chataigne2`'s `src-ui`, but the engine API needs to support both modes for backward compatibility during the migration.
- File-load path: should go through `AddNodeTree` exclusively. No direct child-by-child reconstruction.

**Definition of done**

- Loading a 5,000-node project emits a single-digit number of UI events, not 5,000+.
- `Edit::AddNodeTree` with 1,000 children completes in under 50 ms wall-clock.

---

### Phase 7 — File splits (week 4, ~1 day)

> **STATUS: NOT STARTED**

Now-mechanical, but worth doing once the behavior is stable.

**Tasks**

- Split `runtime.rs` into a `runtime/` directory:
  - `runtime/mod.rs` — public API and re-exports
  - `runtime/tick.rs` — `run_tick`, `run_for`, `run_loop`
  - `runtime/scheduler.rs` — `ScheduleMgr`, `ScheduleBucket`, `collect_due_nodes`
  - `runtime/scheduled_updates.rs` — `run_scheduled_updates` and helpers
  - `runtime/stabilization.rs` — `run_stabilization_rounds`, `run_control_pass`
  - `runtime/limits.rs` — `RuntimeLimits`
  - `runtime/scratch.rs` — `TickScratch`
  - `runtime/errors.rs` — `EngineRuntimeError`
  - `runtime/trace.rs` — debug-only describe helpers
- Split `apply_tree.rs` similarly:
  - `apply_tree/insert.rs`, `remove.rs`, `r#move.rs`, `replace.rs`, `lifecycle.rs`, `batch.rs`, `loaded_subtree.rs`

**Why last and not first**: splitting before the behavior stabilizes just creates merge conflicts and forces the same change across multiple files. Split after.

**Definition of done**

- No file in the runtime or apply_tree modules exceeds 400 lines.
- Public API of `golden_core` is unchanged (verify with `cargo public-api` or equivalent).

---

### Phase 8 — Topological sort cleanup (week 5, ~1 day)

> **STATUS: NOT STARTED**

A smaller load-time win, but worth doing while you're in this code.

**Tasks**

- Replace `BTreeSet<u64>` ready-set with `Vec<NodeId>` used as a stack. Topological order is not unique; the BTreeSet was imposing an arbitrary node-id-based tiebreaker that nothing depends on.
- For deterministic output (useful for tests), sort the ready set once at the start by node id, then stack-pop in order.

**Definition of done**

- 20k-node `resolve()` completes in under 20 ms.
- All existing tests pass without changes (the new tiebreaker is at least as deterministic as the old one).

---

### Phase 9 — Fixed-step accumulator for sequence timing (week 5, ~2 days)

> **STATUS: NOT STARTED**

This is the change that actually addresses your "sequences need precision" requirement. Wall-clock-driven ticks are not enough.

**Tasks**

- Separate **logical time** (sequence-driven, fixed-step, monotonic in tick units) from **wall time** (used to drive `run_for` / `run_loop`).
- Add an accumulator pattern to `run_for` and `run_loop`:
  ```rust
  let mut accumulator = Duration::ZERO;
  let fixed_step = Duration::from_micros(5_000);  // 200Hz logical step
  loop {
      let now = Instant::now();
      let frame_elapsed = now - last;
      last = now;
      accumulator += frame_elapsed.min(MAX_FRAME_CATCHUP);  // clamp to avoid spiral of death
      while accumulator >= fixed_step {
          self.run_tick(fixed_step)?;
          accumulator -= fixed_step;
      }
      // optionally: render/UI emit phase here with interpolation factor
  }
  ```
- Sequences should schedule events at exact logical timestamps. Output backends receive events with their target timestamp and can pre-buffer if they support it (MIDI, OSC, DMX often do).
- Add a `late_event` counter: any event whose target timestamp is more than one fixed-step in the past at dispatch time gets logged.

**Definition of done**

- A sequence test that schedules 1,000 events at known intervals across 10 seconds verifies that emission jitter is bounded by one fixed-step (5 ms at 200 Hz), not by frame jitter.
- Late-event counter exists and is exposed.

---

### Phase 10 — Continuous benchmarks in CI (week 6, ~2 days)

> **STATUS: NOT STARTED**

Locks in the gains so they don't regress.

**Tasks**

- Add a GitHub Actions workflow that runs the three Criterion benchmarks on every PR.
- Compare against the baseline JSON in `main`. Fail the PR if any of these regress beyond a threshold:
  - `tick_20k_passive` p99: > 10% regression
  - `tick_20k_sparse_active` p99: > 10% regression
  - `dispatch_10k_with_1k_listeners` p99: > 15% regression (slightly more lenient because dispatch has higher variance)
- On `main` merge, update the baseline file automatically (or via a manual step if you want human approval on baseline shifts).
- Post a comment on the PR with before/after numbers for the three scenarios. AI assistants reading PR comments will then be aware of the regression and self-correct on follow-up PRs.

**Definition of done**

- CI workflow is green on a no-op PR.
- A deliberately-bad PR (re-introduces `self.nodes.iter()` in `run_scheduled_updates`) is caught and fails CI.

---

## 2. Architectural rules (enforceable, not aspirational)

These belong in `AGENTS.md` and `CONTRIBUTING.md`. Phrase them as commands, not preferences. Each rule has an enforcement mechanism.

### R1 — No full-graph scan in tick path

**Rule**: No call to `self.nodes.iter()`, `self.nodes.values()`, or equivalent inside any function reachable from `run_tick`.

**Enforcement**: A grep-based CI check. The tick-path functions are explicitly listed in a config file; any new full-iteration call inside them fails the check.

```bash
# tools/check_tick_path.sh
forbidden_patterns="self\.nodes\.(iter|values|keys)\("
tick_path_files="runtime/tick.rs runtime/scheduled_updates.rs runtime/stabilization.rs dispatch.rs"
for f in $tick_path_files; do
    if grep -E "$forbidden_patterns" "crates/core/src/engine/$f"; then
        echo "FAIL: full-graph iteration in tick-path file: $f"
        exit 1
    fi
done
```

**Exception process**: requires a benchmark showing the operation is bounded and a `// PERF-EXCEPTION:` comment explaining why.

### R2 — Caches declare their invalidation owner

**Rule**: Every field on `Engine` that derives from other state (caches, indexes, snapshots) must have a doc comment listing the events that invalidate it and the call sites that read it.

**Enforcement**: Documentation linter that checks every field of `Engine` for an `INVALIDATED BY:` line in its doc comment. Implementable as a custom `cargo xtask check-invalidation` in 50 lines.

### R3 — Mutations go through Edit

**Rule**: No `pub fn` on `Engine` that takes `&mut self` and mutates node state, parameter state, or topology. All such operations go through `Edit`.

**Enforcement**: A custom clippy lint or, more pragmatically, a code review checklist item. Any AI-generated PR that adds a `pub fn mutate_xyz(&mut self, ...)` gets rejected.

### R4 — Phase boundaries are explicit

**Rule**: `run_tick` may not call functions that emit `EventKind::*` *and* mutate `self.edits.pending` in the same call. One or the other.

**Enforcement**: A function-level attribute or naming convention. Functions ending in `_emit_only` may push to inbox; functions ending in `_apply_only` may push to edits. Stabilization rounds are the only place both are allowed, and that's why they're bounded.

### R5 — Benchmark deltas required for tick-path PRs

**Rule**: Any PR touching `runtime.rs`, `dispatch.rs`, `apply_tree.rs`, or files matching `runtime/*` requires posting before/after benchmark numbers in the PR description.

**Enforcement**: PR template that includes a "Benchmark delta" section. CI fails if the section is empty for a PR matching those paths.

### R6 — New node callbacks declare their cost

**Rule**: When a node implements `Node`, it must state in doc comments:
- Update rate (or "passive")
- Whether `update_requires_tree_snapshot` returns true (and why)
- Expected event fan-out per call (rough order of magnitude)
- Whether it can mutate graph structure

**Enforcement**: Documentation review. AI assistants generate node implementations frequently; this catches the ones that do unbounded work.

### R7 — Tree snapshots are opt-in and audited

**Rule**: Any node returning `true` from `update_requires_tree_snapshot` must include a `// SNAPSHOT-JUSTIFIED:` comment explaining why a scoped query isn't enough.

**Enforcement**: grep-based CI check. List of nodes requiring snapshots is published in `docs/perf/snapshot-users.md` and reviewed quarterly.

### R8 — Parameter binding is incremental

**Rule**: No code path may construct a `HashMap<NodeId, ParamValue>` from `self.nodes.iter()` after Phase 2 lands. The `param_store` is the only source.

**Enforcement**: grep for `engine_param_snapshot` in non-test, non-store files. If anything other than `ParamStore` calls it, that's a violation.

### R9 — No new fields without invalidation thinking

**Rule**: Adding a new field to `Engine` that derives from other state requires:
1. The doc comment from R2.
2. A test that mutates the source state and verifies the derived field updates.
3. A test that performs an inverse mutation and verifies the derived field unwinds.

**Enforcement**: Code review. This is the rule that most directly defends against AI-introduced staleness bugs.

### R10 — AI-generated code follows the same rules as human-generated code

**Rule**: PRs are not annotated with "AI-generated" as if that were a softer category. Either the diff meets the standard or it doesn't.

**Enforcement**: Cultural. State this explicitly in `CONTRIBUTING.md`.

---

## 3. Benchmark suite (the full version)

Phase 0 starts with three. Build out to this set over the first two months.

### Tick benchmarks

| Name | Setup | Measures |
| --- | --- | --- |
| `tick_20k_passive` | 20k nodes, none with update rate | Pure overhead floor; full-graph scans show up here |
| `tick_20k_sparse_1pct_active_200fps` | 20k nodes, 200 active at 200 Hz | Realistic sparse-update perf |
| `tick_10k_dense_20pct_active_100fps` | 10k nodes, 2k active at 100 Hz | Mid-density stress |
| `tick_5k_all_active_60fps` | 5k nodes, all active at 60 Hz | Worst case: every node active |
| `tick_with_param_changes` | 5k nodes, 100 param changes per tick | ParamStore + dispatch + dependent updates |

### Dispatch benchmarks

| Name | Setup | Measures |
| --- | --- | --- |
| `dispatch_10k_with_1k_listeners` | 10k nodes, 1k subscribers, 100 events/tick | Listener index efficiency |
| `dispatch_20k_sparse_subscriptions` | 20k nodes, 50 subscribers, 10 events/tick | Best-case routing |
| `dispatch_bubbling_deep_tree` | 1k nodes in a 100-deep chain, events bubble up | Bubbling cost |

### Structural benchmarks

| Name | Setup | Measures |
| --- | --- | --- |
| `load_add_node_tree_20k` | Load a 20k-node tree via `AddNodeTree` | Bulk insert path |
| `duplicate_subtree_5k` | Duplicate a 5k-node subtree under existing parent | Realistic duplicate |
| `remove_subtree_5k` | Remove a 5k-node subtree | Cleanup path; tests listener purge, param store cleanup |

### Stress benchmarks

| Name | Setup | Measures |
| --- | --- | --- |
| `stabilization_chain_20_deep` | A param chain where each node depends on the previous, 20 deep | Stabilization round budget |
| `event_storm_1000_per_tick` | 1k custom events per tick to 100 listeners | Inbox throughput |

### What every benchmark reports

Always:
- p50, p95, p99, max tick time
- Allocations per tick (via `dhat` or `tracking-allocator` in a separate non-criterion run)
- Per-tick stats from Phase 0 (events emitted, callbacks fired, snapshot rebuilds, etc.)

The `max` column matters more than `p50`. A scheduler that's fast on average and spiky in worst case will destroy sequence timing.

---

## 4. Repository structure for this work

```
Chataigne2/
├── AGENTS.md                  # Operating rules for AI assistants (rules R1-R10)
├── CONTRIBUTING.md            # Human-facing contribution guide
├── FOUNDATION_PLAN.md         # This document
├── docs/
│   ├── perf/
│   │   ├── tick-path-budget.md       # Per-phase time budgets at 200 fps
│   │   ├── snapshot-users.md         # Audited list of nodes needing tree snapshots
│   │   ├── benchmark-baseline.json   # Current baseline numbers
│   │   └── invalidation-map.md       # All cached fields and their invalidation events
│   ├── architecture/
│   │   ├── tick-phases.md            # Phase diagram of run_tick
│   │   ├── edit-pipeline.md          # How Edits flow through the system
│   │   └── event-routing.md          # How dispatch decides recipients
│   └── adr/                          # Architecture Decision Records
│       ├── 0001-edit-as-single-mutation-channel.md
│       ├── 0002-incremental-param-store.md
│       └── ...
├── tools/
│   ├── check_tick_path.sh            # R1 enforcement
│   ├── check_invalidation_docs.sh    # R2 enforcement
│   └── bench_compare.sh              # CI bench delta script
└── submodules/golden_core/
    └── crates/core/
        ├── benches/                  # Criterion benchmarks
        │   ├── tick.rs
        │   ├── dispatch.rs
        │   └── structural.rs
        └── src/engine/
            ├── runtime/              # After Phase 7 split
            ├── apply_tree/           # After Phase 7 split
            ├── dispatch.rs
            ├── param_store.rs        # New, Phase 2
            ├── listener_index.rs     # New, Phase 3
            └── effective_enabled.rs  # New, Phase 1
```

---

## 5. Architecture Decision Records (ADRs)

Start writing these now. They're how you remember *why* a decision was made when an AI three months from now wants to "simplify" by undoing it.

Format: short, dated, immutable. Once an ADR is "Accepted," it isn't edited; it's superseded by a new one.

Initial ADRs to write in week 1:

1. **0001 — Edit as the single mutation channel.** Why all mutations funnel through `Edit`. What problems it prevents. When (if ever) a back door would be considered.
2. **0002 — Incremental ParamStore.** Why the per-tick `HashMap` rebuild was removed. Invalidation rules. Consistency assertion in debug builds.
3. **0003 — Listener reverse index.** Why `by_origin` reverse map exists. Why `Vec` not `HashSet` for subscriptions per subscriber.
4. **0004 — Stabilization budget = 16.** Why the limit dropped from 256. What "warning at 4" means. When this should be revisited.
5. **0005 — Fixed-step logical time.** Why sequence timing uses an accumulator, not raw wall-clock per tick. Trade-offs (latency vs. precision).
6. **0006 — Tree snapshots are exceptional.** Default is no snapshot. Justification process for nodes that need one.
7. **0007 — UI structural diff events.** Why `SubtreeInserted` exists alongside per-node events. Migration plan.

---

## 6. AI-assisted development protocol

You explicitly raised this concern. Here's the operating model.

### Before asking an AI to do work

1. **Tell it which phase of the plan.** "Implement Phase 2 from FOUNDATION_PLAN.md" is a much better prompt than "speed up param resolution." The first carries the rules and definition of done with it.
2. **Point it at the relevant ADRs.** ADRs encode constraints AI assistants would otherwise re-derive (badly).
3. **Specify the benchmark expectations.** "This change must improve `tick_20k_sparse_active` p99 by at least 5x and not regress any other benchmark."

### During the work

1. **Require test-first or benchmark-first.** A new cache without a test for its invalidation is rejected on sight.
2. **Reject patches that add `pub fn` mutators.** R3 is not negotiable.
3. **Reject patches that add fields without invalidation comments.** R2 is not negotiable.
4. **Watch for "let me also fix..." sprawl.** AI assistants often grow a 1-file change into a 10-file change. Push back. Each PR does one thing.

### After the work

1. **Read every diff line.** Yes, every line. AI-generated code that compiles and passes tests can still violate phase boundaries in ways tests won't catch.
2. **Update the ADR if the design changed.** A change that contradicts an existing ADR either supersedes it (write the new ADR) or shouldn't merge.
3. **Verify benchmark numbers are in the PR description.** Don't take "I ran them and they looked fine" — paste the numbers.

### When AI assistants disagree with the rules

They will. They'll suggest "simpler" approaches that violate R1 or R3. Standard response: "The rule exists because X. If you think X is wrong, write an ADR proposing a new approach. Until then, follow the rule." Don't relitigate the rules every PR.

### Per-tool config files

The repo already has `.claude`, `.kilo`, and similar tool config dirs. Put a *minimal* `AI_RULES.md` in each that points back to the canonical `AGENTS.md`, so any AI tool entering this repo lands on the same rule set within one file read.

---

## 7. Testing strategy

Performance work without correctness tests is how you ship a fast wrong engine.

### Property tests (use `proptest`)

- Random sequence of `Edit`s applied through the pipeline must produce the same final state as applying them through a "naive" reference implementation.
- Random subscription configurations must produce the same routing recipients as a brute-force scan over all listeners.
- Random param mutations through the store must keep the consistency assertion (debug-mode only) intact across 10k operations.

### Determinism tests

- Replay the same sequence of edits twice from a fresh engine. The resulting state, including event order and effective-enabled cache, must be byte-identical.
- This is the test that catches HashMap-iteration-order bugs and other source of nondeterminism that creep in via "harmless" refactors.

### Failure-mode tests

- Stabilization passes > 16: must produce `InfiniteEventEditCycle` error, not silently complete.
- Cycle in dependency graph: must produce `DependencyCycle`, not infinite-loop.
- `update_requires_tree_snapshot` returning true on a node deeply nested in a 20k-node graph: snapshot is built, but only when actually needed.

### Snapshot tests for events

- Loading a 100-node project file emits a known-stable sequence of events. Snapshot it. Future changes that alter the event sequence (especially during the Phase 6 migration to compact subtree events) require deliberate snapshot updates with reviewer sign-off.

---

## 8. Time budgets at scale

For 200 fps you have 5 ms per tick. Here's the suggested distribution. These are targets to design against, not measurements.

| Phase | Budget at 200 fps | Notes |
| --- | --- | --- |
| `absorb_external_edits` | 0.2 ms | Should be near-zero in steady state |
| `apply_edits` (early) | 0.5 ms | Steady state: empty |
| `inbox precompute + preprocess` | 0.5 ms | Routing + per-node preprocessing |
| `apply_edits` (post-inbox) | 0.3 ms | Edits triggered by inbox preprocessing |
| `run_control_pass` | 0.3 ms | Param control evaluation |
| `run_scheduled_updates` | 2.5 ms | The main work |
| `run_stabilization_rounds` | 0.5 ms | Should usually do 0 passes |
| UI/log emit | 0.2 ms | Outbound only |
| **Total** | **5.0 ms** | |

When a phase blows its budget, the per-phase timer from Phase 0 will tell you. Without that timer, you're guessing.

For 20k nodes at 200 fps with 1% active (200 nodes), `run_scheduled_updates` budget = 2.5 ms / 200 nodes = 12.5 microseconds per node callback. That's tight but achievable for trivial nodes. For nodes that do real work (script execution, scene evaluation), they'll need to run on lower update rates or the active percentage will need to be lower. This is a graph design constraint, not an engine constraint, and it's why R6 (nodes declare their cost) matters.

---

## 9. What NOT to do

A list of plausible-sounding ideas that will hurt this project. AI assistants will suggest some of these. Pre-armoring against them.

- **Don't add a "skip ticks when load is high" mode.** It hides bugs and ruins sequence timing. Fix the root cause.
- **Don't add `Arc<RwLock<...>>` to share engine state with the UI thread.** The engine should run on one thread and emit events; the UI consumes events. Locking around the engine state is a recipe for jitter and deadlocks.
- **Don't add a generic "plugin" system to the engine.** It pushes complexity into the hot path forever. Keep the `Node` trait as the extension point.
- **Don't replace `HashMap` with a "faster" hasher across the board.** It might help, but only after profiling shows hashing is the bottleneck (it usually isn't). Premature.
- **Don't introduce `async`/`tokio` to the engine core.** Async runtimes have nondeterministic scheduling and per-task overhead that's wrong for a fixed-step engine. Async at the I/O boundary (transport, network) is fine; async inside `run_tick` is not.
- **Don't try to parallelize `run_scheduled_updates` early.** Parallelism on a node graph with shared state is genuinely hard. Get the single-threaded version sub-millisecond first. If you still need more, *then* think about parallelism, and the answer will probably be "shard by subgraph at the application level" rather than "parallel inner loop."
- **Don't add a "for performance, skip enabled checks in release builds" path.** The `effective_enabled` cache is the right answer; bypassing safety checks is not.
- **Don't store debug strings on hot-path data.** `describe_node` etc. are debug-only for a reason. Resist requests to "make it always available."

---

## 10. Milestone summary

| Week | Phase | Headline outcome |
| --- | --- | --- |
| 1 | 0 | Baseline measured, three benchmarks running, per-tick stats accessible |
| 1 | 1 | `effective_enabled` cached; no parent walks in tick path |
| 2 | 2 | `ParamStore` is the only source; per-tick `HashMap` rebuild gone |
| 2 | 3 | Listener reverse index; dispatch scales with subscriber count, not graph size |
| 3 | 4 | Tick path has zero steady-state allocations; stabilization budget is 16 |
| 3 | 5 | Bucket collection scales with due-node count |
| 4 | 6 | Compact subtree events; bulk operations don't flood the UI |
| 4 | 7 | Files split; no module over 400 lines |
| 5 | 8 | Topological sort cleanup; faster resolves |
| 5 | 9 | Fixed-step accumulator; sequence timing is decoupled from frame jitter |
| 6 | 10 | CI benchmarks gating PRs; regressions caught automatically |

After week 6, you have a foundation that lets you scale to 20k nodes at 200 fps with confidence, and a rule set that protects that foundation from drift as more contributors (human and AI) touch the codebase.

The goal of this document is to make the path from week 1 to week 6 mechanical. If a step is hard to start, the spec for it is too vague — refine the relevant phase section before doing the work, not during.

---

## 11. After the foundation

Once the 10 phases are complete, there are clear next directions, but none of them are urgent until you actually hit their respective limits:

- **Subgraph parallelism** if single-threaded `run_scheduled_updates` saturates a core. Shard by subtree, run subgraphs in parallel, merge events at the inbox.
- **Persistent param store** for crash recovery and undo/redo at the engine level (not just the editor level).
- **Hot-reload of node definitions** for scripted nodes — would require careful thinking about phase boundaries.
- **Time-travel debugging** by recording all `Edit`s. The single-mutation-channel design makes this almost free to implement.
- **Networked engine sync** for collaborative editing. Same observation: single mutation channel is the precondition.

Each of these deserves its own ADR before any code is written. Each one is a multi-week effort. Don't start them until the foundation is stable and a real use case demands them.

---

*End of plan.*
