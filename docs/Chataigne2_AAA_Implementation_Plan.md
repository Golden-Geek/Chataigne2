# Chataigne2 AAA Foundation and Scalability Plan

Audit baseline: `Golden-Geek/Chataigne2` at commit `0b21ea8c71fec127b08b13865c699f8a119d81d9`

## Executive decision

Chataigne2 should keep its frictionless studio-network model: no accounts, passwords, tokens, pairing prompts, roles, or client approval flow. Every client that can reach the configured server interface is intentionally allowed to observe and control the application.

That is a valid product decision for a trusted art/studio LAN, but it must be described accurately: the software is not inherently immune to attack. It can manipulate project data, files, devices, protocols, and live-show state, so an unknown client on the same reachable network can cause real disruption. The target is therefore **open access with defensive engineering**, not an authentication system.

The invisible protections in this plan preserve ease of use while preventing accidental browser drive-by control, unbounded memory growth, denial of service from malformed or slow clients, and silent operational failures.

## Target state

The release is ready to carry an “AAA-grade foundation” claim only when all of the following are true:

- Repository gates are truthful: formatting, linting, full workspace tests, dependency auditing, protocol generation, and production builds all fail reliably when broken.
- Engine workloads have explicit resource budgets and never construct unbounded Cartesian products or queues.
- ValueSet and Alchemist execution use production data paths rather than debug capture as functional plumbing.
- Event delivery is bounded, deterministic where order matters, and incremental for sparse changes.
- The editor remains responsive on large graphs because updates and visibility queries scale with changed or visible items, not the whole graph.
- Spatializer complexity is measured and controlled at realistic target counts.
- Persistence has explicit migrations and crash-safe writes.
- The network API remains password-free and open, while browser-origin protections, message limits, backpressure, and observability are enforced.
- Large modules have clear ownership boundaries, duplicated implementations have one canonical owner, and generated protocol types are authoritative.

## Non-negotiable engineering rules

1. **Measure before and after.** Every performance change needs a reproducible benchmark or trace showing the affected workload.
2. **Put limits at ingress.** Validate cardinality, frame size, batch size, and queue capacity before allocating or cloning.
3. **One owner per invariant.** Resizing, protocol conversion, graph indexing, and persistence migration must each have one canonical implementation.
4. **Debug facilities cannot be required for normal execution.** Turning diagnostics off must not alter functional output.
5. **Sparse changes should cause sparse work.** A single parameter change must not rebuild an entire index or graph.
6. **Generated artifacts cannot silently drift.** CI must regenerate them and fail on any diff.
7. **No security theatre.** Do not add credentials or approval prompts. Do add protections that are invisible during normal studio use.

## Delivery sequence

### Implementation progression

| Stage | Status | Supercommit scope |
|---|---|---|
| 0 | Complete (2026-07-10) | Truthful CI, exact toolchains, dependency triage, versioned benchmark manifest/baseline, direct trigger result prerequisite, and persistence-baseline correctness |
| 1 | Complete (2026-07-10) | Checked, lazy, budgeted multiplex execution and incremental canonical reconciliation |
| 2 | Complete (2026-07-10) | Open-network hardening and observability |
| 3 | Complete (2026-07-10) | ValueSet direct output and cache invalidation |
| 4 | In progress | Incremental, deterministic, bounded events |
| 5 | Pending | Revision- and visibility-driven graph UI |
| 6 | Pending | Bounded Spatializer geometry |
| 7 | Pending | Canonical boundaries and deduplication |
| 8 | Pending | Persistence migration and crash resilience |

Progress is updated in the same Supercommit that completes each stage. A stage is marked complete only after its scoped tests and the repository formatting gates pass.

### Stage 0 — Establish truthful gates and performance baselines

**Status: Complete (2026-07-10).**

Evidence:

- `.github/workflows/ci.yml` now has one required quality job covering all Rust workspaces, generated protocol drift, UI lint/check/build, RustSec, and production npm auditing. The benchmark job is deliberately non-blocking and uploads its report.
- `tools/clippy_gate.py` fails on any warning beyond exact per-file debt baselines; existing debt cannot grow silently and the baseline can shrink without special configuration.
- `benchmarks/aaa/fixtures.v1.json` versions every required workload, while `benchmarks/aaa/baseline.v1.json` records host/toolchain metadata and the refreshed dispatch result. The stale dispatch median fell from about 200.9 ms to 19.36 ms on the named reference host.
- Node 24.18.0 and npm 11.16.0 are recorded for CI. Rust 1.95 is the package MSRV and CI toolchain.
- Production npm audit reports zero vulnerabilities. Three low-severity development-only `cookie` findings remain behind the SvelteKit development toolchain and are assigned to the Stage 5 dependency/bundle upgrade.
- The current Svelte slot warning, CSS minifier warnings, 1.37 MB editor chunk, and 206.5 KB Manager stylesheet are reproducible production-build findings assigned to Stage 5.
- Root workspace tests pass (379 app tests and 75 state-machine tests, with two pre-existing explicitly ignored tests), `golden_core` passes its workspace suite, and `golden_alchemist_core` passes 126 tests.
- Sparse persistence baselines now run app `init` to capture app-owned declared defaults while deferring `on_node_ready` side effects. Transition guards use a direct functional trigger result rather than requiring debug capture.

This stage comes first because later optimization claims are not trustworthy without repeatable measurements and complete CI.

#### Work packages

- Fix the current frontend lint failure across the 18 reported files.
- Remove the `|| echo ''` behavior from the preparation/code-generation path. Missing Rust/codegen tooling must fail with a clear message.
- Enable Rust formatting and checking in the root workflow.
- Run `cargo test --workspace`, not only the engine subset.
- Add `cargo clippy --workspace --all-targets --all-features -- -D warnings` with a small, documented temporary allowlist if necessary.
- Add `cargo audit` or equivalent advisory scanning and `npm audit --omit=dev` as release gates. Track development-only findings separately.
- Add a code-generation drift job: generate protocol bindings and fail if the worktree changes.
- Add frontend `check`, `lint`, production build, and relevant test suites to the same required workflow.
- Record the exact Rust, Node, npm, and platform toolchain versions.
- Refresh the stale dispatch baseline instead of relying on the current 200 ms comparison threshold.

#### Benchmark fixtures

Create versioned fixtures for at least:

- 1,000, 10,000, and 50,000 graph nodes with sparse parameter changes.
- Multiplex axes whose products are 64, 1,024, 16,384, and intentionally over budget.
- ValueSet pipelines with 1, 10, 100, and 1,000 entries.
- Event dispatch at 100 events × 1,000 listeners.
- Spatializer workloads at 50, 100, 250, 500, and 1,000 targets.
- Network fan-out with 1, 10, and 50 clients, including one deliberately slow client.
- Project save/load fixtures for the largest supported real project.

Capture wall time, p50/p95/p99 latency, allocations, peak resident memory, queue depth, dropped/coalesced event counts, and UI frame time where applicable.

#### Exit criteria

- A clean checkout cannot pass CI without running code generation and all required checks.
- The current production build warning and dependency advisories are triaged, with owners and remediation decisions.
- Benchmarks can be run locally and in a dedicated, non-blocking CI performance job.
- Baseline reports are committed in machine-readable form with hardware/toolchain metadata.

### Stage 1 — Contain multiplex cardinality and allocation risk

**Status: Complete (2026-07-10).**

Evidence:

- `ProcessorContextProvider` now exposes owned axis items once, derives lane counts directly from axis lengths, and produces context keys through a lazy mixed-radix iterator. Counting is O(number of axes), while the last axis advances fastest in stable order.
- Checked multiplication rejects per-axis, per-processor, runtime-total, and platform-size overflow before lane enumeration. Defaults are 4,096 items per axis, 16,384 lanes per processor, and 65,536 active lanes per runtime; `ProcessorMultiplexLimits` provides the expert override while checked arithmetic remains mandatory.
- The 64, 1,024, and 16,384 fixture cardinalities, intentional over-budget input, `usize` overflow, deterministic ordering, and runtime-total rejection have dedicated state-machine tests. Over-budget processors return runtime diagnostics without creating lane keys.
- Processor UI lane counts no longer enumerate context keys, and the manager constructs full processor DTOs only after the preview throttle has made a publish decision.
- `golden_core` is now the sole owner of multiplex list reconciliation. The duplicate `UserContextMultiplexNode` resize implementation and listener plumbing were removed.
- Reconcile targets are coalesced per list with monotonic generations, revalidated against the live count before execution, and drained in stable node order. Defaults cap an operation at 16,384 new nodes and one tick at 256 structural edits; all three reconcile limits are expert-configurable through `RuntimeLimits`.
- A changed count replaces stale work, over-budget requests leave the graph unchanged with a node warning, and each tick applies its reconcile slice as one edit transaction. The core test covers chunking, cancellation, and diagnostics; the complete `golden_engine` suite passes 332 tests with one benchmark intentionally ignored (333 total).
- The root workspace passes 379 app tests plus 77 state-machine tests (two stale tests remain explicitly ignored). Shared-formula environment tests now serialize their process-global override, App Control logger assertions match only their own diagnostic, and TCP recovery tests poll a bounded state transition instead of relying on a fixed scheduler delay.

This is the highest-priority runtime issue. Current multiplex context generation eagerly materializes a Cartesian product, clones prefixes, and is also used merely to count lanes. Large axis counts can create explosive CPU, memory, and queued work.

#### Work packages

- Replace eager `iter_context_keys` materialization with a lazy mixed-radix iterator.
- Add checked cardinality calculation before iteration. Overflow must return a typed error, never wrap or allocate.
- Add explicit, configurable budgets for:
  - items per axis;
  - lanes per processor;
  - total active lanes per runtime;
  - new nodes created by one reconcile operation.
- Select defaults from benchmark results. Expose an expert configuration override, but keep arithmetic and memory checks mandatory even when limits are raised.
- Separate `lane_count()` from lane enumeration so UI DTO construction never generates all context keys to obtain a count.
- Stop computing processor UI DTOs when there is no subscriber or publish decision requiring them.
- Make reconciliation incremental and chunk large additions across frames/ticks so a single request cannot monopolize the engine.
- Consolidate all resize/reconcile behavior into one canonical component. Remove the duplicate implementations in engine contexts and `UserContextMultiplexNode`.
- Batch structural events produced by reconciliation and emit one coherent change set.
- Add cancellation/version checks so stale reconciliation work is abandoned when inputs change again.

#### Exit criteria

- Cardinality overflow and over-budget input produce a clear diagnostic without material allocation.
- Counting lanes is O(number of axes), not O(number of lanes).
- Memory remains bounded during adversarial axis inputs.
- Reconciliation remains interruptible and does not create an unbounded command queue.
- Duplicate resize algorithms are removed.

### Stage 2 — Keep open networking, harden it invisibly

**Status: Complete (2026-07-10).**

Evidence:

- Open access remains the product policy. Native and script clients without an `Origin` header are accepted, while browser traffic is restricted to same-origin or explicitly configured origins and wildcard CORS has been removed.
- Host validation accepts the actual listening address, loopback aliases, configured hostnames, and the advertised mDNS name while rejecting DNS-rebinding hosts. Focused tests cover native, same-origin, allowlisted, foreign-origin, and rebound-host requests.
- The transport now has configurable limits for concurrent connections, HTTP requests, WebSocket messages, intent batches, subscriptions, message rate, identifiers, JSON depth and strings, paths, per-client outbound queues, and the shared hub-command queue. Handshakes and writes have explicit timeouts.
- Slow-client queues are bounded and degrade superseded event batches to an explicit resync marker. The 50-client soak test drives 1,000 batches per client and verifies every queue remains within capacity while dropped-message and resync metrics advance.
- Read-only health, metrics, and connection-info endpoints expose the listening address, open-access policy, advertised name, connection counts, drops, resyncs, protocol errors, and effective limits. mDNS advertises the ready server, and structured connection logs include stable connection IDs and remote addresses.
- The reusable app header displays an unobtrusive open-network indicator for non-loopback binding and offers copyable connection details and live queue/error metrics.
- Tauri now disables its global API and uses a restrictive application CSP with explicit local HTTP/WebSocket transport endpoints.
- `golden_core` workspace tests pass, including all 13 transport tests; the root workspace passes 379 app tests and 77 state-machine tests (two explicitly ignored); Svelte check and lint report zero findings; and the production UI build succeeds. The repository-wide strict-clippy debt recorded in Stage 0 remains unchanged and separately gated by its exact baseline.

#### Product policy: Open Studio Network

The intended policy is:

- no identity system;
- no passwords, tokens, roles, pairing, or client approval;
- any client that can reach the configured interface may read and mutate state;
- browser, native, OSC-style, command-line, and custom controller clients remain easy to connect;
- the UI clearly shows that network control is open and which interfaces are listening.

This is an explicit trust-boundary choice: **network reachability equals authorization**. Public-internet exposure should be documented as unsupported, but the software does not need to prevent an expert from configuring it.

#### Browser drive-by protection without authentication

- Serve the standard web UI and API from the same origin.
- For browser HTTP and WebSocket requests that include an `Origin` header, accept the same origin plus a small configurable allowlist for legitimate separate UIs.
- Continue accepting clients with no `Origin` header, preserving native applications, scripts, `curl`, and embedded controllers.
- Replace wildcard CORS with same-origin behavior or reflection of an explicitly allowed origin.
- Validate `Host` to prevent DNS-rebinding attacks while accepting the actual bound LAN IPs, hostnames, and advertised mDNS name.
- Test WebSocket upgrades and all mutating HTTP endpoints against foreign browser origins.

These checks do not identify users or restrict ordinary LAN clients. They prevent an unrelated website opened in a browser from silently controlling Chataigne2 through the local network.

#### Resource safety and availability

- Replace one-unbounded-thread-per-connection behavior with a bounded async runtime or bounded worker pool.
- Define configurable high-water limits for concurrent clients, HTTP body size, WebSocket frame size, intent batch size, subscription count, and messages per interval.
- Use bounded outbound queues. Coalesce superseded state/value events by key.
- If a client falls behind, drop intermediate state and send a fresh snapshot/resync marker rather than growing memory indefinitely.
- Apply backpressure or reject an oversized batch before decoding it into large structures.
- Put timeouts around handshakes, idle partial requests, writes, and shutdown.
- Validate numeric values, nesting depth, strings, paths, and collection sizes at protocol boundaries.
- Fuzz frame parsing, intent decoding, and reconnect/resync state machines.

These are capacity controls, not access controls. Defaults should be generous and configurable, and exceeding one should return a precise diagnostic.

#### Operational visibility

- Show listening interfaces, port, advertised mDNS name, client count, per-client queue pressure, dropped/coalesced messages, and recent protocol errors.
- Add structured logs with a stable connection ID and remote address. Do not log full sensitive project payloads by default.
- Expose a one-click “copy connection info” action and mDNS discovery so the hardened path is still easier to use than typing an IP.
- Add health and metrics endpoints that do not mutate state.
- Display a persistent but unobtrusive “Open network control” indicator when bound beyond loopback.

#### Desktop shell hardening

- Replace the null Tauri CSP with the narrowest CSP compatible with the application.
- Disable the global Tauri API unless a concrete feature requires it.
- Limit capabilities and URL/navigation scope to the application’s actual needs.
- Keep these changes transparent to the normal user workflow.

#### Explicitly out of scope

- accounts and passwords;
- permission roles;
- mandatory pairing codes;
- client approval dialogs;
- license checks masquerading as security;
- a default restriction to loopback only.

#### Exit criteria

- The normal LAN workflow still connects without credentials or prompts.
- Native clients and scripts without an `Origin` header continue to work.
- A random website cannot issue a browser WebSocket upgrade or mutating cross-origin request.
- A slow client cannot cause unbounded memory growth or stall other clients.
- Oversized/malformed requests fail cheaply and visibly.
- A 50-client soak test has stable memory, bounded queues, and a clean reconnect/resync path.

### Stage 3 — Remove debug plumbing from ValueSet execution

**Status: Complete (2026-07-10).**

Evidence:

- Compiled Alchemist graphs now resolve authored output sockets to runtime value slots through a reusable public API, and runtime memory exposes only initialized values for functional reads.
- ValueSet elementwise and projection pipelines read their typed result directly from runtime memory. Normal evaluation uses debug capture `Off`; it no longer clones or scans debug samples or forces unchanged inputs through the graph.
- Debug capture is an explicit optional observer. Tests run the same pipeline with capture disabled and enabled, proving identical lane values while only the observed run produces samples.
- Pure stateless lanes cache their direct result by stable lane key and input value. A sparse input change reevaluates one changed lane and reuses the unchanged lane; stateful lanes continue using independent sparse runtime memory; always-process graphs bypass the pure cache.
- `PipelineInvalidationReason` makes input, graph, time-dependent tick, external-side-effect, and debug-request invalidation explicit. Per-evaluation statistics expose evaluated and reused lane counts for tests and performance evidence.
- The ignored reproducible 10,000-lane benchmark reports 94.695 ms for initial evaluation and 20.023 ms for an unchanged cached pass on the Stage 0 reference environment, with zero lanes reevaluated. An initial O(lanes²) stale-cache pass was caught by this benchmark and replaced with O(lanes) hash membership.
- The state-machine strict scoped clippy gate passes, all 79 active state-machine tests pass (three explicitly ignored, including the benchmark), the root workspace passes, and the `golden_alchemist_core` workspace passes all 126 tests.

Before this stage, `ValueSetPipelineRuntime::evaluate` enabled debug capture to retrieve normal output, forced unchanged inputs through processing, cloned samples, and scanned captured debug data. The implementation below removes that diagnostic dependency from the production algorithm.

#### Work packages

- Add a direct, typed output slot or return value for pipeline evaluation.
- Make debug capture an optional observer of evaluation, never the owner of its result.
- Stop forcing unchanged inputs through the pipeline unless a processor declares time dependence or side effects.
- Cache outputs by input revision and processor/configuration revision.
- Reuse evaluation buffers and preallocate from a measured high-water mark.
- Avoid constructing debug samples when no debug subscriber is present.
- Introduce explicit invalidation reasons: input change, graph change, time-dependent tick, external side effect, and debug request.

#### Exit criteria

- Turning debug capture on or off produces identical functional output.
- An unchanged, pure pipeline performs no processor work.
- Evaluation does not clone or scan debug samples to obtain its result.
- The 1,000-entry benchmark shows bounded allocation and an agreed latency improvement over the baseline.

### Stage 4 — Make engine events incremental, deterministic, and bounded

#### Work packages

- Replace the current “all non-parameter events are structural” fallback with explicit event classification. A `Custom` event must declare whether it affects structure, indexes, values, UI only, or nothing cacheable.
- Update parameter/control indexes from precise deltas. Reserve full rebuilds for actual structural invalidation.
- Fix replay cursor advancement so a quiet subscription advances to the inspected sequence boundary rather than repeatedly rescanning unrelated events.
- Make topological order deterministic with stable node identifiers and a stable ready queue.
- Define and test ordering guarantees for structural events, value events, and client-visible snapshots.
- Add bounded channels or coalescing to every event fan-out path that can be outrun by a producer.
- Refresh the dispatch representation benchmark and choose the Arc/snapshot policy from current data.
- Add property tests for event classification and replay, plus deterministic-order tests across repeated runs.

#### Exit criteria

- A single value change does not rebuild structural indexes.
- Quiet subscribers do not repeatedly scan already-inspected unrelated events.
- Identical graphs produce identical topological order across runs and platforms.
- Event memory use is bounded under a stalled consumer.

### Stage 5 — Rebuild graph editor data flow around revisions and visibility

The graph store currently clones large maps on batches, while the canvas repeatedly scans nodes/edges and rebuilds geometry. This will dominate interaction latency as projects grow.

#### Work packages

- Replace whole-map `cloneState` updates with stable entity maps plus narrowly scoped revision counters.
- Partition revisions by concern: topology, node geometry, edge geometry, selection, values, and viewport.
- Make selectors subscribe to the smallest relevant revision.
- Add a spatial index for node bounding boxes and edge hit-test segments.
- Query visible/near-visible entities from the index; do not scan the entire graph on pan, zoom, or pointer movement.
- Cache connection routing by endpoint geometry revision.
- Move heavy layout/routing calculations to a worker where profiling proves main-thread contention.
- Virtualize inspector and list-heavy panels.
- Split `GraphCanvas.svelte`, `AlchemistEditorPanel.svelte`, and `SpatializerPanel.svelte` by behavior boundaries, not arbitrary line counts.
- Break the main bundle into route/feature chunks and lazy-load editor features that are not needed at startup.
- Audit the 206.5 KB Manager stylesheet for unused or duplicated rules.

#### Exit criteria

- A viewport interaction scales with visible/changed entities rather than total graph size.
- A one-node update does not copy the entire node and edge maps.
- The 10,000-node sparse-change fixture meets the agreed frame-time target on reference hardware.
- Main-thread long tasks over 50 ms are absent during ordinary pan/zoom/edit operations.
- The production bundle has an explicit size budget and regression gate.

### Stage 6 — Bound Spatializer complexity

The backend has an O(S × T²) path, and the UI Voronoi implementation can reach O(T³). This needs an explicit scale target rather than informal optimization.

#### Work packages

- Decide and document supported target counts for interactive editing and playback.
- Profile source movement, target movement, topology change, and parameter-only change separately.
- Cache target-only structures and recompute them only when target geometry/topology changes.
- Replace brute-force Voronoi construction with a proven Delaunay/Voronoi implementation or a measured sweep-line alternative.
- Share one geometry model between backend and UI where possible, or generate both from identical fixtures and tolerance rules.
- Incrementally update source weights when only a subset of sources moves.
- Move preview-only geometry work off the UI thread if it cannot meet the frame budget.
- Add numerical robustness tests for coincident targets, collinear points, empty sets, extreme coordinates, and NaN/Infinity rejection.

#### Exit criteria

- Complexity and supported counts are documented and verified by benchmarks.
- Target-only topology is not recomputed for source-only movement.
- Backend and UI produce matching results within a defined tolerance.
- The largest supported interactive fixture stays within the frame/audio-control latency budget.

### Stage 7 — Consolidate boundaries and remove duplication

This is not a cosmetic cleanup. The current large modules and repeated adapters multiply the cost and risk of every future change.

#### Work packages

- Make the generated protocol schema/types canonical. Handwritten types should either disappear or be explicit domain models behind one conversion boundary.
- Split the large conversion file by protocol domain and generate mechanical conversions where safe.
- Give the protocol, persistence, and script crates real ownership instead of acting only as re-export facades.
- Extract coherent subsystems from the largest Rust files:
  - manager lifecycle, command handling, subscriptions, and DTO publication;
  - formula parsing, semantic analysis, execution, and diagnostics;
  - UI sync snapshots, deltas, serialization, and transport.
- Consolidate the six identical stream `engine_call_script_method` implementations and the nine near-identical variants behind a shared adapter.
- Generate or centralize repeated parameter getters and generator helper code.
- Remove the inspector import cycle by moving shared contracts/state into a lower-level module.
- Introduce a duplication check for exact copied blocks and track near-duplication through review; do not pursue a literal “zero duplication” metric when two simple implementations have genuinely different ownership.
- Add an architectural decision record for each new boundary and its dependency direction.

#### Exit criteria

- One canonical implementation exists for each identified repeated behavior.
- Protocol generation and conversion ownership are unambiguous.
- The frontend import graph is acyclic for production modules.
- No high-churn handwritten source file exceeds an agreed size threshold without an explicit architectural justification.
- Public crates have real responsibilities, tests, and dependency rules.

### Stage 8 — Add persistence evolution and release resilience

Exact version rejection is acceptable during experimentation but not for mature project files. A professional art tool needs to protect creators’ work across upgrades and failures.

#### Work packages

- Introduce a migration registry with ordered, testable version-to-version transforms.
- Keep immutable fixtures for every released project format version.
- Define forward-version behavior: clear rejection, read-only recovery, or best-effort import. Never silently reinterpret unknown data.
- Write saves to a temporary file, flush/sync as appropriate, then atomically replace the target.
- Keep a configurable rolling backup and recovery journal for interrupted saves.
- Move serialization and disk I/O outside the long-held engine lock using an immutable snapshot/revision check.
- Validate sizes, nesting, references, and numeric values before applying loaded state.
- Add round-trip, migration, corruption, interrupted-write, and large-project tests.

#### Exit criteria

- Every historical fixture opens through an explicit migration path.
- A killed/interrupted save cannot destroy the last good project.
- Large saves do not block the engine for the duration of serialization and disk I/O.
- Corrupt or future-version files fail with actionable recovery information.

## Cross-cutting performance budgets

Final numeric thresholds should be selected from Stage 0 baselines on named reference hardware. The release gate should include at least:

| Area | Required budget |
|---|---|
| Engine tick | p95 and p99 budget for idle, sparse-change, and heavy-change fixtures |
| UI interaction | p95 frame time, long-task count, and input-to-paint latency |
| Multiplex | maximum checked lanes, allocation ceiling, and reconcile work per tick |
| ValueSet | per-entry evaluation latency and allocations for changed vs unchanged input |
| Event system | dispatch latency, replay scan count, and bounded queue memory |
| Spatializer | backend update time and UI preview frame time at each supported scale |
| Networking | fan-out latency, per-client queue ceiling, resync time, and soak-test memory slope |
| Persistence | snapshot pause, serialization time, write time, and peak memory |
| Startup/package | startup-to-interactive time and compressed bundle budget |

Performance gates should compare distributions, not a single fastest run. Fail only on statistically meaningful regressions to avoid flaky CI.

## Testing strategy

### Unit and property tests

- checked multiplex cardinality, iterator equivalence, overflow, cancellation, and budgets;
- event classification, cursor advancement, deterministic topology, and coalescing;
- ValueSet cache invalidation and debug-on/debug-off equivalence;
- protocol size/depth/numeric validation;
- persistence migration and corruption handling;
- Spatializer numerical edge cases.

### Integration tests

- create/edit/save/reload a representative large project;
- connect multiple clients, mutate state, disconnect, reconnect, and resync;
- browser same-origin success and foreign-origin rejection;
- native client success without credentials or `Origin`;
- slow-client queue saturation without impact on healthy clients;
- code generation from a clean checkout.

### Fuzz and soak tests

- protocol frames and intent payloads;
- project file parser and migration chain;
- graph mutation sequences;
- 8–24 hour network fan-out and engine edit/playback runs;
- repeated connect/disconnect storms and partial requests.

## Suggested pull-request breakdown

Keep changes reviewable and avoid combining behavioral refactors with broad formatting:

1. CI truthfulness, lint cleanup, codegen drift gate, baseline harness.
2. Checked multiplex cardinality and lazy iterator.
3. Incremental/bounded multiplex reconciliation and resize deduplication.
4. Open-network origin/Host policy and protocol limits.
5. Bounded network runtime, slow-client resync, observability, and soak tests.
6. ValueSet direct-output path and debug decoupling.
7. Precise control/event invalidation, replay cursor fix, deterministic topology.
8. Graph store revisions and incremental selectors.
9. Canvas spatial indexing, routing cache, and bundle splitting.
10. Spatializer algorithm/cache replacement.
11. Protocol canonicalization and adapter deduplication.
12. Manager/formula/UI-sync modularization and import-cycle removal.
13. Persistence migrations, atomic save, backups, and recovery tests.
14. Full release-candidate performance, fuzz, and soak qualification.

## Release checkpoints

### Foundation checkpoint

Stages 0–3 complete. This removes the most dangerous unbounded work, makes CI honest, retains password-free networking, and fixes the production/debug coupling.

### Scale checkpoint

Stages 4–6 complete. Engine, editor, and Spatializer behavior is deterministic, incremental, and benchmarked at published scale targets.

### Maintainability checkpoint

Stages 7–8 complete. Ownership boundaries, duplication, protocol generation, and project-file evolution are suitable for long-term releases.

### AAA release candidate

- All checkpoints complete.
- No unresolved critical or high-severity correctness/resource findings.
- Production dependency advisories have been fixed or accepted in writing with compensating controls.
- Reference performance budgets pass on every supported OS.
- Fuzzing and soak tests have no crash, deadlock, unbounded memory slope, or state divergence.
- Migration and recovery are verified against all released fixtures.
- Network documentation plainly states the Open Studio trust model.

## Immediate next actions

Start with these five items in order:

1. Make CI and code generation truthful and record baselines.
2. Implement checked multiplex cardinality and lazy iteration.
3. Add open-network origin/Host checks plus bounded frames and queues, without credentials.
4. Replace ValueSet debug-capture output plumbing.
5. Fix precise event invalidation, replay cursor advancement, and deterministic topology.

These actions address the highest-risk foundation problems before larger editor and architectural refactors begin.
