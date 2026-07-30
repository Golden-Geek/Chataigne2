# Multiplex Performance and UI Responsiveness Finish Plan

## Goal

Finish and validate the in-progress work that restores `test_multiplex.noisette`
to a stable 100 Hz-or-better development runtime, keeps processor/state
manipulation bounded, and gives every explicit user action immediate,
truthful progress feedback.

## Completion Result — 2026-07-30

Status: complete.

- Websocket staging is plane-aware and preserves reliable ordering barriers.
  Structure and trigger work cannot be silently coalesced or dropped.
- Subscription-scoped overflow and graph-projection invalidation use one
  coordinated snapshot/resubscribe path. Stale socket, subscription, and
  projection generations cannot commit.
- Large graph transactions are projected incrementally into detached state,
  then published atomically. The replay cursor advances only after the
  consumer commits the complete batch.
- Latest-wins value/preview events coalesce only inside a safe reliable-event
  suffix.
- Multiplex cardinality and indexed preview lookup no longer materialize large
  Cartesian lane sets; full expansion remains capped at 65,536 keys.
- Duplicate initial parameters are staged before insertion and are no longer
  overwritten by app-owned duplicate offsets. Ordinary duplicates retain
  their existing offset behavior.
- Output batching preserves singleton command shape, explicit batch shape,
  mixed-event order, delayed batching, and the existing bounded work limits.
- Shared-formula tests use scoped overrides and reset watcher state, avoiding
  developer-file access and parallel-test snapshot interference.
- The pre-existing sample and backup files remained untouched.

Validation completed:

- `cargo test -p golden_parameters`: 5 passed.
- `cargo test -p golden_engine --lib`: 390 passed, 1 ignored benchmark.
- `cargo test -p golden_transport_server --lib`: 27 passed.
- `cargo test -p Chataigne2`: 514 unit tests and 1 integration test passed.
- `cargo check --workspace`: passed.
- UI: 22 files / 73 tests passed; Svelte check reported 0 errors and
  0 warnings.
- Explicit Prettier check passed for every changed UI file.
- Repository UI lint still reports only the pre-existing
  `ModuleIndicators.svelte` formatting issue documented below.

Warm serial performance evidence:

- Dirty engine runtime: 4,460 us average, 6,285 us p95, 6,636 us p99,
  7,175 us maximum, 0 deadline misses, 0 snapshot builds, 0 provider rebuilds,
  and 0 command-budget rejections.
- Production runtime with publication: 4,846 us average, 6,724 us p95,
  7,169 us p99, 8,451 us maximum, and 0 deadline misses.
- Engine duplication: 44-node processor in 18 ms with a 10 ms rebuild tick;
  534-node state in 29 ms with a 28 ms rebuild tick.
- Production duplication: processor transaction 19 ms (18,582 us apply,
  98 us publish) with a 12 ms rebuild tick; state transaction 33 ms
  (31,411 us apply, 1,009 us publish) with a 26 ms rebuild tick.

This file records the exact stop point on 2026-07-29. No implementation or
validation after this point should be assumed complete without rerunning the
checks below.

## Repository Safety

- Preserve the user's pre-existing edits in:
  - `apps/chataigne/test-samples/test_multiplex.noisette`
  - `apps/chataigne/test-samples/test_multiplex.noisette.backup`
- Do not reset, restore, reformat, or claim those sample changes.
- Continue using `apply_patch` for source edits.
- If the user's `cargo watch -x "run -- --dev"` process is running again, do
  not terminate it. Wait for it or coordinate validation around it.
- Keep app-specific behavior under `apps/chataigne`; keep reusable runtime,
  protocol, transport, persistence, and UI behavior in the owning Golden
  packages.

## Implemented Before the Stop

### Development runtime and hot paths

- Root development and test profiles use `opt-level = 3`, line-table debug
  information, and no incremental cache. This is intentional so development
  runtime performance represents the shipped hot path.
- Shared registries use one-time initialization instead of rebuilding common
  declarations.
- Alchemist processor execution reuses stateless scratch memory and avoids
  allocating the full lane table set for every lane.
- State-machine context lanes, formula plans, command dispatch plans, previews,
  and overview/debug capture are cached or demand-gated.
- Structural and formula invalidation is targeted instead of rebuilding every
  cached path.
- Runtime-center suffix scans and logger queries use indexed/binary-search
  boundaries.
- Scheduled updates use a stable binary heap.
- Non-finite parameter values are rejected at the parameter boundary.
- Custom-event JSON payloads are shared with `Arc` across routing copies.

### Bounded multiplex/output work

- Managed-output fan-out uses a compact execution/target cursor rather than
  materializing the Cartesian product.
- Immediate and delayed output work share a strict 512-execution per-tick
  budget.
- Accepted work is bounded to:
  - 64 fan-out jobs
  - 4,096 remaining executions
  - 65,536 distinct retained target entries
  - 4,096 delayed entries
- Oversized inbound batches are rejected before typed `Vec` deserialization.
- Cancellation uses a generation barrier, including an empty cancelling
  output.
- State-machine command resolution has a 4,096-action per-tick cap with stable
  prefix ordering, observable rejection counters, and throttled warnings.
- Over-budget accepted output work spans ticks. Requests beyond the explicit
  queue limits are rejected rather than allowed to monopolize the engine.

### Duplication and persistence

- Duplicate requests preflight destination, codec, catalog, initializer,
  dependency, and parameter-constraint failures into detached trees.
- Catalog state is captured once and each distinct parent is evaluated once.
- Prepared subtrees commit in one grouped UI transaction/history operation.
- Initial parameters are staged before live insertion.
- Preference persistence runs only when the Preferences subtree changed.
- Duplication code was extracted to
  `crates/golden_core/engine/src/engine/persistence/duplicate.rs`.
- Important limitation: app-owned live lifecycle callbacks still execute after
  insertion and may fail. Do not describe duplication as fully atomic across
  arbitrary lifecycle side effects unless that boundary is redesigned and
  tested.

### Read-model and transport publication

- The UI read model maintains an incremental projection, parent index, retained
  event indexes, and a lazy whole-graph snapshot.
- Production mutations capture and publish their read-model delta inside the
  same control-actor turn, preventing an older capture from overtaking a newer
  mutation.
- Project replacement and project-file metadata updates are ordered through
  the control actor.
- A replay pass sends all non-empty data planes in one atomic delta envelope.
- Value-plane coalescing uses indexed slots with bounded compaction instead of
  repeated linear scans.
- Latest-wins outbound merging no longer crosses a reliable delta barrier.
- A final local patch also treats `ResyncRequired` for the same subscription as
  an ordering barrier; its new regression has not yet been run.
- Protocol version remains `0.4.0`, with generated TypeScript protocol output.

### User-action feedback

- Explicit actions have visible lifecycle states:
  `Queued -> Received -> Processing -> Finishing`, with a visible failed state.
- The websocket reader emits `Received` before handing work to the hub.
- The hub emits `Accepted` before entering the control actor.
- Send/receipt timeouts are 4 seconds and the processing watchdog is 120
  seconds.
- The app header shows the active action, phase, spinner, and pending-action
  count.
- Failed feedback remains visible briefly instead of disappearing immediately.
- `UiAck` now carries both `earliest_event_time` and `latest_event_time`.
  Origin correlation still uses the first event; “Finishing” waits for the
  final event boundary and two paint opportunities.
- The intent queue uses a head cursor and per-generation/per-node maps, avoiding
  front-splice and repeated full-queue scans.

### Regression coverage already added

- Dirty-path and full-production multiplex runtime benchmarks.
- Processor and state duplication benchmarks through both the engine path and
  `ProductionRuntime`.
- Incremental read-model publication ordering tests.
- Output burst, cancellation, serialization, and memory-bound tests.
- Large pending-value coalescing and outbound ordering tests.
- UI intent lifecycle, action activity, custom-event, graph-store, and
  large-queue tests.
- The formerly monolithic module performance test was split into:
  - `apps/chataigne/src/module/tests/multiplex.rs`
  - `apps/chataigne/src/module/tests/runtime_scaling.rs`

## Historical Incomplete Work at the Stop Point

The active agents were interrupted at the user's request. Their filesystem
edits were present, and the following two areas were incomplete and
unvalidated at that time. Both are complete in the result recorded above.

### 1. Websocket resync and socket-generation state machine

Partial work is present in:

- `packages/golden-ui/transport/ws.ts`
- `packages/golden-ui/transport/subscription-resync.ts`
- `packages/golden-ui/store/session/snapshots.ts`
- `packages/golden-ui/transport/index.ts`
- `apps/chataigne/ui/src/lib/tests/webSocketResyncBarrier.test.ts`

Finish and verify these invariants:

1. On `ResyncRequired`, mark only the affected subscription as resyncing.
2. Reset its staged tail and stop/unsubscribe its server stream.
3. Await the snapshot callback and require the callback to return the exact
   applied snapshot boundary.
4. Set the subscription cursor to that boundary.
5. Send exactly one subscribe for the current socket/subscription generation.
6. Ignore a snapshot completion if the subscription closed, a newer resync
   started, or the socket generation changed.
7. Ignore all events from stale sockets. A stale `onclose` must not reject
   pending work, reset a newer subscription, or schedule another reconnect.
8. A subscription created while the socket is `CONNECTING` must be sent only by
   the successful `onopen` pass, not by both `onopen` and a pending promise.
9. A close-before-open failure must reject only the promise owned by that
   socket generation.
10. Repeated resync requests must converge to one current recovery, never
    overlap snapshots or replay an old staged tail.

Required regressions:

- staged tail exists before resync
- delta arrives while the snapshot is in flight
- reconnect occurs during resync
- repeated `ResyncRequired`
- subscription created while connecting
- stale socket closes after a new socket opened
- close before open

### 2. Weighted, bounded main-thread graph projection

Partial work is present in:

- `packages/golden-ui/transport/staged-frame-batches.ts`
- `packages/golden-ui/store/graph-event-projection.ts`
- `packages/golden-ui/store/graph.svelte.ts`
- `apps/chataigne/ui/src/lib/tests/graphStore.test.ts`
- `apps/chataigne/ui/src/lib/tests/webSocketBatchScheduling.test.ts`

The old 512-event budget is insufficient because one
`graphTransaction/subtreeInserted` event may contain tens of thousands of
nodes. Complete a weighted work scheduler with these properties:

1. Weight graph operations by actual node/operation work, not only top-level
   event count.
2. Build a large structural projection incrementally across animation frames.
3. Build into detached state and publish the complete transaction atomically;
   consumers must never observe a partially inserted subtree.
4. Do not advance the subscription cursor until the batch is actually
   committed.
5. Bound staged work by an explicit maximum weight/count.
6. On reliable overflow, clear staged work and invoke the coordinated resync
   path. Never silently drop structure/trigger events.
7. Latest-wins value/preview work may be coalesced within its safe ordering
   suffix.
8. Reset or generation changes must invalidate an in-flight detached
   projection before it can commit.
9. Keep `ws.ts` orchestration under 1,000 lines by extracting cohesive state
   machine helpers where practical.

Required regressions:

- one subtree insertion with at least 20,000 nodes spans multiple frames
- no partial graph is visible between frames
- final graph and child/parent/parameter indexes are exact
- cursor advances only after atomic publication
- sustained producer rate above consumer rate remains within the backlog bound
- reliable overflow requests resync
- reset during detached projection prevents the stale commit

## Static Review Before More Editing

1. Run `git status --short` and inspect every partial UI file listed above.
2. Confirm interrupted work contains no half-written branches, temporary
   instrumentation, skipped tests, or duplicated resync implementations.
3. Review the new `outbound_subscription_id` handling in
   `ui_server/outbound_queue.rs`.
4. Review the duplication lifecycle limitation and ensure comments/tests do
   not overstate atomicity.
5. Confirm new non-generated runtime/test files remain below 1,000 lines.
6. Keep the sample files untouched.

## Formatting and Code Generation

After implementation is complete:

```powershell
cargo fmt --all
cargo fmt --manifest-path crates/golden_core/Cargo.toml --all
```

Run protocol generation again only if the Rust protocol changes:

```powershell
cargo run --manifest-path Cargo.toml -p golden_codegen_support --bin golden_codegen -- ui-protocol packages/golden-ui/generated/rust_protocol
```

From `apps/chataigne/ui`, format every changed file under
`packages/golden-ui` with the app's explicit Prettier configuration. Do not rely
on package-local defaults:

```powershell
npx prettier --config .prettierrc --write <changed-package-files>
npx prettier --config .prettierrc --check <changed-package-files>
```

## Validation Sequence

Run focused failures first, then broad suites.

### Rust

```powershell
cargo test -p golden_parameters
cargo test -p golden_engine --lib
cargo test -p golden_transport_server --lib
cargo test -p Chataigne2 module::command::tests
cargo test -p Chataigne2 systems::alchemist
cargo test -p Chataigne2 systems::state_machine
cargo test -p Chataigne2 --lib
cargo check --workspace
```

If a filter does not match because of the app's re-exported module path, use
`cargo test -p Chataigne2 -- --list` and select the exact test prefix rather
than deleting coverage.

### UI

From `apps/chataigne/ui`:

```powershell
npx vitest run src/lib/tests/webSocketResyncBarrier.test.ts
npx vitest run src/lib/tests/webSocketBatchScheduling.test.ts
npx vitest run src/lib/tests/webSocketIntentLifecycle.test.ts
npx vitest run src/lib/tests/workbenchIntentActivity.test.ts
npx vitest run src/lib/tests/graphStore.test.ts
npm test
npm run check
npm run lint
```

The last known full UI state before the interrupted resync/projection edits was
63 passing tests and zero Svelte-check errors/warnings. That result is stale
after the interrupted edits and must not be used as final evidence.

The last known lint blocker was pre-existing and outside this change:
`apps/chataigne/ui/src/lib/components/modules/ModuleIndicators.svelte`.
Report it accurately if it remains the only failure.

### Performance evidence

Run each benchmark serially, preferably twice after the optimized test binary
is warm:

```powershell
cargo test -p Chataigne2 multiplex_sample_active_runtime_stays_realtime -- --nocapture --test-threads=1
cargo test -p Chataigne2 multiplex_sample_production_runtime_stays_realtime -- --nocapture --test-threads=1
cargo test -p Chataigne2 multiplex_sample_state_machine_edits_stay_interactive -- --nocapture --test-threads=1
cargo test -p Chataigne2 multiplex_sample_production_duplicate_transactions_stay_interactive -- --nocapture --test-threads=1
```

Capture and report the exact printed numbers:

- dirty engine tick average, p95, p99, maximum, and deadline misses
- production tick plus read-model publication average, p95, p99, and misses
- callback ticks, snapshot builds, provider rebuilds, and budget rejections
- processor/state duplicated node counts
- apply, publication, total transaction, and post-duplicate rebuild times

Acceptance targets:

- dirty engine average below 5 ms
- dirty engine p99 below 10 ms
- production runtime average below 6 ms
- production runtime p99 below 10 ms
- no more than two 10 ms host-deschedule outliers in 240 measured ticks
- zero steady-state snapshot builds
- zero checked-sample command-budget rejections
- processor engine duplicate below 50 ms; post-tick below 50 ms
- state engine duplicate below 100 ms; post-tick below 100 ms
- processor production transaction below 100 ms
- state production transaction below 150 ms

## Final Integrity Checks

```powershell
git diff --check
git status --short
```

Also:

- scan changed text files as strict UTF-8 and reject U+FFFD or common mojibake
  sequences
- inspect changed-file line counts
- confirm generated protocol output matches Rust
- confirm there are no legacy `on:` Svelte handlers in touched UI
- confirm touched layout uses relative units
- confirm no ignored reliable event path can silently overflow
- confirm all action phases retire after the final applied boundary or an
  explicit failure/timeout

## Validation State at This Stop

- `cargo fmt --all`: completed successfully.
- `cargo fmt --manifest-path crates/golden_core/Cargo.toml --all`: completed
  successfully.
- `cargo test -p golden_parameters`: 5 passed, 0 failed.
- `cargo test -p golden_engine --lib`: deliberately terminated during the
  optimized test-binary build when the user requested an immediate stop; no
  result is available.
- No final Rust workspace check, transport suite, Chataigne suite, UI suite,
  Svelte check, lint, or multiplex benchmark result is available for the
  current interrupted tree.

The first optimized compile is expected to be slower because dev/test now use
`opt-level = 3`; this is the deliberate compile-time tradeoff for representative
development runtime performance.
