# Chataigne2 implementation plan for Codex in VS Code

Prepared 2026-09-05 from the full audit of **`5392728f51f9584c529b6e1e75f72e3d5ede7c85`**. Both the available checkout and a fresh connected-GitHub lookup of `main` returned that SHA while preparing this plan.

Repository: [Golden-Geek/Chataigne2](https://github.com/Golden-Geek/Chataigne2). Baseline: [5392728](https://github.com/Golden-Geek/Chataigne2/commit/5392728f51f9584c529b6e1e75f72e3d5ede7c85). This document specifies future implementation; it does not claim the fixes or acceptance checks have run.

## 1. Start here

Place this file at `docs/plans/audit-remediation.md` in your local checkout, open the repository root in VS Code, and give Codex this instruction:

```text
Read AGENTS.md and docs/plans/audit-remediation.md. Implement this plan in
dependency order, beginning with T00 and then the earliest unfinished task.

The plan covers the audit of 5392728f51f9584c529b6e1e75f72e3d5ede7c85.
Reconcile it with the current checkout before changing code. Inspect source
and relevant evidence; do not assume a commit message closes a finding.

Preserve the complete Chataigne UX, modules, formulas, state machines,
contexts, multiplex behavior, normal cargo run/watch/dev workflows,
app-agnostic Golden ownership, ASIO/JACK support, and password-free LAN use.

Work in small, coherent implementation batches. Follow the repository's
editing and test-placement rules. Add regression tests for the actual failure
contracts and use the current pinned toolchain. Keep a progress ledger at
docs/progress/audit-remediation-status.md with findings, changed files,
commands, outcomes, limitations, and the next dependency-ready task.

Continue through safe local work without asking for approval at each task.
Preserve unrelated changes. Do not publish, push, merge, create remote issues
or PRs, or interact with real devices unless separately authorized. Do not
weaken required features or tests to obtain a pass. If one platform or device
check is unavailable, record it and continue independent work; do not mark
that gate passed.

Before ending a work session, leave the current batch reviewable, update the
ledger, and report completed work, test results, remaining risks, and the next
task. A fix is complete only when its stated acceptance criteria have evidence.
```

The task is intentionally larger than one context window. Resume with: **“Read the plan and progress ledger, verify the current diff, and continue the next unfinished task.”** Do not restart completed work or treat a new session as permission to overwrite it.

## 2. Product and engineering contracts

These requirements govern every task. They are more important than a smaller diff or a better isolated benchmark.

- Preserve panels, layouts, outliner, inspector, dashboards, modules, command nodes, script functions/callbacks/templates, Formula and state editors, previews, undo/redo, and connected-client workflows. Add diagnostics through the existing UI conventions.
- Preserve Action/Mapping formula assets, managed Inputs/Filters/Outputs, ConditionGate behavior, formula sharing, stateful lane isolation, stable context identities through lane reorder, deterministic triggers/output ordering, and one authoritative state-machine state. A transition's global context must not silently become a multiplex lane context.
- Keep `apps/chataigne` as composition and product ownership. Alchemist and Chataigne module policy stay there. Golden engine, host, protocol, persistence, graph, audio, and UI facilities remain reusable through public contracts.
- Follow `AGENTS.md`: use the standard editing tool; Svelte 5 runes; generated protocol types; tests under the owning `tests/` directory; cohesive implementation files below 1,000 lines or a documented legitimate exception; no private path imports. Format root and Golden Core Rust workspaces at the end of a Rust batch.
- Internal API changes are allowed when they improve ownership. Update consumers, code generation, tests, and architecture docs together. Preserve user workflows and project meaning; a persisted-format change needs an explicit version/migration policy and fixtures. Do not introduce broad compatibility shims.
- Preserve `cargo run`, `cargo xtask watch`, `cargo run -- --dev`, headless startup, and ordinary packaging. Keep the convenient default Golden host and a documented lightweight consumer path.
- Keep ordinary supported artifacts capable of ASIO/JACK on their applicable hosts. Driver absence is recoverable. Preserve reconnect/device recovery and realtime ownership constraints.
- Preserve accessible, password-free LAN access. Bound requests, connections, queues, and resource use without introducing mandatory accounts, passwords, or a cloud service.

## 3. Execution order and closure rules

| Wave                                | Tasks   | Required result                                                                                                                     |
| ----------------------------------- | ------- | ----------------------------------------------------------------------------------------------------------------------------------- |
| 0 — establish evidence              | T00     | Current source, local changes, toolchain, test baseline, and finding ledger recorded                                                |
| 1 — repair release evidence         | T01–T03 | Platform failures diagnosed, narrow dependency repairs, truthful benchmark gate                                                     |
| 2 — contain failure                 | T04–T10 | Reliable notifications, bounded scripts, restored caches, deterministic order, coherent project switches/saves, honest edit results |
| 3 — remove scale bottlenecks        | T11–T14 | Bounded service, versioned snapshots, delta-based UI publication, useful sparse scheduling                                          |
| 4 — finish reusable ownership       | T15–T17 | Real package boundaries, ordinary audio artifact support, coherent source/docs cleanup                                              |
| 5 — qualify computation and product | T18–T19 | Measured real-kernel parallelism where justified, complete product evidence and remaining limitations                               |

Work sequentially by default. Dependencies below allow independent work to proceed when, for example, a macOS check is unavailable. T17's small gitlink/documentation cleanup can happen early; it must not delay P1 repairs. Split a task into several reviewable changes when ownership or size warrants it.

Track implementation separately from qualification. Finding statuses are **open**, **partially fixed**, **fixed**, or **unverified**. Check outcomes are **passed**, **failed**, **pending**, **skipped**, or **unavailable**. A skipped application build is not a passing build. An implementation can be ready locally while cross-platform qualification remains open.

For each finished task retain: starting SHA, patch/ending SHA when committed, dirty-tree state or patch fingerprint, owning files, invariant/decision, exact commands, platform/features/toolchain, results, and remaining checks. Performance evidence additionally needs fixture hashes, raw samples, hardware/OS, build profile, and measurement boundaries. Never attribute results from an uncommitted patch to the unmodified base SHA.

## 4. Implementation tasks

### Progress checkpoint — 2026-09-05

| Task    | Progress                                                                                           |
| ------- | -------------------------------------------------------------------------------------------------- |
| T00     | Complete                                                                                           |
| T01     | Implementation and Windows checks complete; Linux x64/ARM64 and macOS native qualification pending |
| T02     | Complete                                                                                           |
| T03     | Comparator/workflow complete locally; matching hosted reference baseline pending                   |
| T04     | Complete                                                                                           |
| T05     | Complete                                                                                           |
| T06–T19 | Not started                                                                                        |

The reviewed stop point is after T05. Resume at T06. Exact commands, evidence, and remaining
qualification gaps are recorded in `docs/progress/audit-remediation-status.md`.

### T00 — Reconcile the baseline and establish a resumable ledger

**Dependencies:** none. **Owner:** repository tooling and progress documentation.

1. Read root and applicable nested contributor instructions, `ARCHITECTURE.md`, `docs/guides/development.md`, and the relevant subsystem design docs. Record `git status --short`, HEAD, and the branch. Inspect the connected repository/ref or fetch remote refs when available; do not reset, clean, or overwrite local work.
2. Compare current source with the audit SHA. Populate every F01–F16 row in the ledger; identify already-fixed, moved, still-open, and unverified portions. Where source is newer, adapt paths and tests before implementing. Do not repeat an already proven fix.
3. Verify `tools/bootstrap/toolchain.json` and installed tools. At this baseline it pins Rust 1.97.0, Node 26.5.0, npm 11.17.0, and Python 3.14. Use the current manifest if changed; label unsupported local toolchains honestly.
4. Capture relevant existing test failures before editing. Retrieve CI jobs/logs for the exact starting SHA if accessible. Keep the historical CI links below as baseline evidence, not as proof of the current branch.
5. Inventory preservation scenarios and available platforms/devices. Use synthetic/mock endpoints for automated tests. A missing platform is a qualification gap, not a reason to delete its coverage.

**Acceptance:** one ledger includes all findings, task dependencies, exact starting state, commands available on the developer's OS, and known baseline failures. Begin T01 without asking the user to choose routine implementation details.

### T01 — Restore observable platform and UI build gates

**Finding:** F01. **Depends on:** T00. **Owners:** Golden Audio, app UI, CI.

**Start at:** `crates/golden_audio/src/realtime/priority.rs`; `crates/golden_audio/tests/playback_ordering.rs`; `crates/golden_audio/tests/playback_ordering_cases/mod.rs`; `apps/chataigne/ui/src/lib/components/modules/ModuleIndicators.svelte`; `.github/workflows/ci.yml` and `product-gate.yml`.

- Replace the derived `Debug` implementation for `AudioThreadPriorityGuard` with a portable diagnostic representation that does not require the foreign handle to implement `Debug`. Keep realtime promotion/demotion and feature coverage.
- Apply the configured formatter to the affected Svelte file. Do not add a test that merely verifies formatting.
- Diagnose `managed_streaming_continues_while_the_host_thread_is_stalled` and `force_restart_false_ignores_pending_and_active_duplicate_ids_without_replacement`. Trace command admission, worker delivery, render consumption, and observation publication. Introduce test barriers/acknowledgements at the relevant boundaries; continue render progress while waiting where required. Preserve the real host-stall scenario. Fix product behavior if the controlled reproduction still fails.
- Ensure platform compilation remains observable when UI lint fails, while retaining UI lint as a required release gate. Preserve actual bundled-UI build dependencies; do not manufacture a passing package from a stale bundle. Add appropriate pull-request coverage alongside existing push/manual checks, without privileged execution of untrusted PR code.

**Acceptance:** Linux x64/ARM64 realtime compilation succeeds; both named audio tests pass under controlled scheduling on macOS; pinned-toolchain UI lint/check/build pass; application matrix jobs actually execute. Keep unavailable native results open. No ignored tests, feature removal, arbitrary sleeps, or relaxed assertions as a substitute for diagnosis.

### T02 — Repair the dependency qualification failures narrowly

**Finding:** F12. **Depends on:** T00. **Owners:** workspace dependencies, Golden Audio.

**Start at:** root `Cargo.toml`, `Cargo.lock`, Golden Audio dependency usage, dependency-gate configuration.

- Verify current advisory data and dependency paths. The rechecked advisory targets are `h2 >=0.4.16` and `rtrb 0.3.5` on the 0.3 line. Prefer the narrow repair; `rtrb 0.4` changes `is_abandoned` behavior and requires a separate semantic review.
- Update constraints only as required and deliberately update the affected lockfile entries. Review unexpected transitive churn and duplicate-version policy changes.
- Rerun dependency qualification and affected stream ownership, abandonment, queue ordering, and reclamation tests. Retain the qualification that no reachable Chataigne exploit was established by the audit.

**Acceptance:** the locked graph satisfies current policy with no advisory suppression; ownership/ordering regressions are absent in the relevant tests. Record the advisory DB revision/date and exact dependency graph tested. Sources: [h2 advisory](https://github.com/rustsec/advisory-db/blob/main/crates/h2/RUSTSEC-2026-0258.md), [rtrb advisory](https://github.com/rustsec/advisory-db/blob/main/crates/rtrb/RUSTSEC-2026-0274.md).

### T03 — Make benchmark validation reject missing evidence

**Finding:** F13, gate portion. **Depends on:** T00. **Owner:** performance tooling.

**Start at:** `tools/core/bench_compare.py`; `crates/golden_core/engine/benches/baseline.json`; `.github/workflows/benchmarks.yml`; `tools/qualification/tests/`.

- Validate an explicit expected scenario set, schema, units, unique names, sample presence, and finite valid measurements. Empty/missing files, truncated runs, missing cases, duplicates, malformed values, and unaccounted-for case changes must produce a nonzero result.
- Treat nonmatching host/toolchain/profile/feature fingerprints as incomparable, with a clear qualification outcome. Do not compare the existing Windows baseline to Ubuntu as an equivalent performance gate.
- Preserve upstream benchmark command failures through output capture; retain stderr and raw results. Align documented warning/failure thresholds with executable policy.
- Keep a quick functional gate and a performance gate using matching reference hardware. Refresh baselines only from complete, explained measurements; never auto-bless a slowdown or a missing case.

**Acceptance:** negative fixtures fail for each invalid class; a valid matching dataset passes; a real regression fails; host mismatch cannot report “no regressions.” Existing qualification tests remain green. Product-wide evidence remains open until T19.

### T04 — Repair pending-work publication and drain/re-arm semantics

**Finding:** F02. **Depends on:** T00. **Owners:** `golden_io`, OSC integration.

**Start at:** `crates/golden_core/runtime/io/src/pending.rs`, its `src/tests/`, and `apps/chataigne/src/module/modules/protocol/osc/osc_module_base.rs`.

- Define the producer/consumer invariant in the channel API. Publish readiness after a successful enqueue, with a correct clear-before-drain/re-arm contract. A completed accepted send must not leave an undrained event unobservable until another event arrives. A producer paused before completing publication must eventually publish when resumed.
- Prefer a receiver-owned bounded-drain operation over requiring every consumer to manually coordinate flag clearing. When a turn exhausts its budget, conservatively re-arm readiness; a harmless extra poll is preferable to stranded work.
- Distinguish empty, disconnected, and rejected/full sends. Verify all consumers of the primitive, including OSC, follow the contract. T11 later adds full capacity policy; do not reintroduce the race during that migration.

**Acceptance:** deterministic barrier-driven regression reproduces the old store-before-enqueue interleaving and passes after the fix. Also cover concurrent producers, enqueue during drain, partial drain without a subsequent producer, final-item races, and disconnection. Prove OSC processes the pending event without an unrelated wakeup. Avoid tests whose correctness depends on thread sleeps or joining the producer before observing readiness.

### T05 — Interrupt scripts and define failure effects

**Finding:** F03. **Depends on:** T00. **Owner:** engine scripting initially; reusable pieces move in T15.

**Start at:** `crates/golden_core/engine/src/script/mod.rs` and its tests. Resolve the exact interrupt API from the locked `rquickjs` version before coding.

- Install an interrupt handler on every QuickJS runtime. Use monotonic deadlines and cancellation state, established before every JavaScript entry: initial evaluation, reload, exports, callbacks, jobs if supported, and any scripted teardown. Clear/restore the deadline on all exits; nested entries must not extend the outer deadline.
- Keep the handler cheap and nonblocking. Retain memory, recursion/stack, and host-call limits. Introduce explicit live-callback and load budgets compatible with the runtime's tick budget; distinguish a configurable allowance from a hard realtime guarantee.
- Audit host calls. A VM interrupt cannot preempt a blocking native host function. Move potentially blocking work behind bounded asynchronous contracts with cancellation, and validate/cap host-call inputs.
- Use an invocation effect journal: collect host mutations/commands; validate them; commit only after successful evaluation. Discard queued effects on exception, timeout, or budget failure. Use transactional engine edits where supported and deterministic external-command admission. Do not claim physical I/O can be rolled back after dispatch.
- On failure, expose a recoverable diagnostic and disable/reinitialize the affected script according to an explicit policy. Discarding host effects does not roll back JavaScript heap changes; do not blindly reuse a partially mutated failing context. Unrelated engine/control work must continue.

**Acceptance:** subprocess tests with an external watchdog cover infinite loops at top level and in callbacks, excessive finite work, memory exhaustion, recursion, failed callbacks with queued effects, nested invocations, and reload/teardown. Verify no failed invocation's staged effects escape, successful effect ordering remains intact, and later ticks/edits/saves progress. A failure must not hang the test runner. Moving an uninterruptible VM into an orphan thread is not completion.

### T06 — Restore the parameter cache on every recoverable exit

**Finding:** F05. **Depends on:** T00. **Owner:** engine tick and dispatch.

**Start at:** `crates/golden_core/engine/src/engine/runtime/scheduled_updates.rs` and `crates/golden_core/engine/src/engine/dispatch.rs`.

- Replace early returns around `mem::take` with an ownership-safe scoped-result boundary: extract once, run fallible work, restore once, then propagate the result. Prefer eliminating the extraction if borrowing can express the same contract cleanly; do not add unsafe pointers or clone the entire cache each tick.
- Ensure callback budgets are checked before admitting excess work. Specify which already-accepted tick edits remain after failure and maintain cache/graph consistency with that policy.
- Review adjacent temporary-state extraction for the same failure pattern. Limit additional fixes to actual affected ownership paths.

**Acceptance:** inject callback-budget and edit-absorption failures in scheduled updates and inbox dispatch. On the following tick, resolve unchanged and changed parameter bindings and verify values, node membership, cache contents, and continued useful work. Do not assume the whole failed tick rolled back. Panics/process abort are a separate policy; any claimed unwind recovery requires its own restoration evidence.

### T07 — Make topology and effect ordering deterministic

**Finding:** F07. **Depends on:** T00. **Owner:** engine schedule compilation.

**Start at:** `crates/golden_core/engine/src/engine/runtime/tick.rs` and schedule-generation tests.

- Define a canonical ready-node key from stable authored identity/order. Use a globally ordered ready set or equivalent stable compile ordinals for every newly ready tie. Do not rely on `HashMap` iteration or only sort the initial frontier.
- Preserve dependency constraints, cycle handling, and deterministic same-target write/trigger behavior. Document the chosen tie-breaker as an execution contract.
- Measure compile-time cost; do not add sorting to every live tick when order can be compiled once.

**Acceptance:** logically equivalent graphs with identical stable identities and varied insertion/hash iteration order yield identical schedules and ordered effects. Cover diamonds, several initially/newly ready nodes, disconnected components, cycles, and conflicting output targets. Reuse these fixtures for T14/T18 across worker counts.

### T08 — Make project replacement an explicit prepare/commit/retire operation

**Finding:** F04. **Depends on:** T05, T06. **Owners:** application runtime, lifecycle, read-model publication.

**Start at:** `crates/golden_core/engine/src/application.rs`, `src/app/mod.rs`, `src/runtime_center.rs`, and `src/engine/persistence/duplicate.rs` under the engine crate.

- Introduce a candidate preparation path for decode/configuration, detached validation, script preparation, and generation compilation. Keep the current project authoritative while this fallible work runs. Preparation must not activate live output/device ownership as a hidden side effect.
- Use a monotonic project-generation token distinct from document revision and compile generation. Define the commit point at which engine, read model, history/current-project metadata, and published generation switch consistently.
- Model exclusive-device handoff explicitly. Never run two owners simultaneously. If activation needs old-device release and then fails, restore old ownership when possible; otherwise retain the old authored project in a coherent paused/recoverable state with an error. Do not promise uninterrupted playback during an impossible handoff.
- Retire old resources outside the live actor/tick path where possible, using bounded cancellation/lifecycle work completed in T11. Discard stale compilation or preparation results using their generation tokens.
- Audit duplication lifecycle failures using the same ownership rules. Grouped UI publication alone is not rollback of module/device side effects.

**Acceptance:** fault injection at decode, preparation, script evaluation, compilation, device activation, publication preparation, and retirement shows an unambiguous active project and coherent UI/history. Test replacement during edits and with stale compiler results. Failure preserves a usable or explicitly paused old project; success changes generation once; no orphan device owner or candidate leaks remain.

### T09 — Serialize saves across the complete persistence transaction

**Finding:** F11. **Depends on:** T08. **Owners:** persistence service and project-host adapter.

**Start at:** `crates/golden_core/hosts/transport/src/project_host/mod.rs`, `hosts/transport/src/ui_server/mod.rs`, `services/persistence/src/file_store.rs`, and engine save APIs.

- Add a persistence coordinator in the persistence/service boundary. Assign a save ticket at acceptance containing project generation, document revision, monotonically ordered request ID, and destination identity before independent encoding can reorder requests.
- Define normalized destination identity for existing/nonexistent targets and applicable platform path semantics. Alias paths must not bypass coordination. Serialize backup, journal, temp-file, target replacement, and recovery cleanup for each destination as one logical operation.
- Default to ordered commits of accepted saves for the same destination. If coalescing supersedes an unstarted save, return an explicit superseded result. Different destinations may use bounded concurrency.
- Coordinate project replacement with an in-flight save lease/fence. Pending old-generation jobs must not commit after the new generation wins. Define how an already-committing operation completes before the switch; a generation check made long before a rename is insufficient. Keep disk work outside the actor and avoid actor/coordinator lock inversion.
- Update current path and saved/dirty revision only for the current generation and applicable winning request. A save of revision R must not mark later edits clean. Treat Save As races explicitly.

**Acceptance:** barrier-driven tests force an older encode/write to finish after a newer one, simultaneous same-path saves, path aliases, Save As races, replacement during save, and edits after snapshot capture. Latest accepted successful state and metadata are correct. Inject failure/crash boundaries across journal/backup/temp/rename/recovery on supported OSes; recover a valid committed revision, preserve actionable errors, and allow subsequent saves. Atomic rename alone does not close this task.

### T10 — Return real graph-edit failures through the public API

**Finding:** F14, acknowledgement portion. **Depends on:** T00. **Owner:** engine graph adapter and consumers.

**Start at:** `GraphEditing for ProductionRuntime<T>` in `crates/golden_core/engine/src/application.rs` and its UI/headless consumers.

- Replace the unconditional `Infallible` success contract with a typed error/result derived from the transaction acknowledgement. Return history/revision data only when that operation succeeded.
- Propagate rejection details consistently to UI, script, transport, undo/redo, and external consumers. Update generated contracts only if the wire contract changes. Do not duplicate wire declarations by hand.

**Acceptance:** rejected add/edit/undo/redo operations produce a real failure and leave history/UI aligned with engine state; accepted edits report the correct acknowledgement and revision. A small non-UI consumer test proves the public API observes failure.

### T11 — Bound admission, service turns, compilation, and retirement

**Finding:** F06. **Depends on:** T04, T08, T09, T10. **Owners:** shared I/O, control runtime, transport host, module adapters.

**Start at:** `runtime/io/src/pending.rs`; `runtime/control/src/control.rs`, `compiler.rs`; `hosts/transport/src/ui_server/mod.rs` under Golden Core; the OSC adapter from T04; lifecycle callers.

Produce one small capacity/overflow table in the owning docs, then implement each boundary as a separate reviewable slice:

| Boundary                    | Required policy                                                                                                                                                                         |
| --------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Shared I/O and OSC          | Item/byte bounds, bounded drain turns, retained readiness; explicit rejection for ordered events                                                                                        |
| Continuous values           | Coalesce by key only where intermediate samples are semantically replaceable; never silently apply this to triggers or commands                                                         |
| WebSocket hub/client output | Bound commands, retained bytes, per-client and aggregate work; yield dispatch turns; disconnect/resync lagging clients by explicit policy                                               |
| HTTP/control admission      | Bound total connections/tasks and request bytes/weight; reject overload promptly while keeping password-free LAN use                                                                    |
| Actor                       | Bounded request admission and service budgets; preserve progress for control, cancellation, replies, and feedback without starvation                                                    |
| Compiler                    | Keep at most the in-flight job plus the latest pending replaceable generation; replace superseded requests before building large snapshots; cancel/check staleness at useful boundaries |
| Lifecycle retirement        | Cooperative cancellation, interruptible I/O, bounded retirement capacity, and joins outside the live tick/actor                                                                         |

- Audit synchronous/blocking send paths for deadlocks after adding capacity. Audio/render callbacks must not wait on channels, allocate unbounded work, or destroy resources that require blocking cleanup.
- Bound both queue count and retained payload weight; bound total clients/workers as well as each queue. Expose depth/bytes, oldest age, rejection/coalescing counts, and recovery state.
- Do not create unlimited replacement threads when an old worker is stuck. Retain ownership safely, cap stuck/retiring slots, and report a recoverable resource failure. Moving `join()` to an unbounded cleanup queue is insufficient.

**Acceptance:** sustained producer rates above capacity reach a memory plateau; ticks, command replies, publication, and shutdown/cancellation continue; ordered events are accepted in order or explicitly rejected; continuous data converges to the latest accepted value. Test slow clients, reconnect storms, OSC bursts, superseded compilation, blocked worker shutdown, and partial drains. After load stops, backlog and feedback recover within the configured bounds.

### T12 — Separate immutable snapshots and encoding from actor progress

**Finding:** F10. **Depends on:** T08, T09, T11. **Owners:** read model, persistence document capture, compiler capture.

**Start at:** `crates/golden_core/engine/src/ui_read_model.rs`, project encoding in `application.rs`, capture in `runtime_center.rs`, and persistence service APIs.

- Publish versioned immutable/chunked state. Capture a coherent root/revision using a short lock or atomic root load; materialize DTOs and encode JSON outside locks needed by event publication and outside the control actor.
- Do not merely move the final JSON call: an O(N) clone under the actor/publication lock preserves the original bottleneck. Introduce structural sharing or bounded revision-consistent capture where necessary.
- Bind event cursor, project generation, and snapshot revision to the same captured state. Reuse completed immutable payloads where scope permits; keep client authorization/capability scope intact. Bound retained versions and caches, and avoid dropping huge old structures on latency-sensitive threads.
- Apply the same capture review to compiler requests. Keep persistence sequencing from T09 when encoding becomes concurrent.

**Acceptance:** snapshots racing edits are internally consistent and support correct delta resumption; publication does not wait behind whole-graph materialization/encoding. Three clients saving/resyncing while edits and I/O continue produce bounded retention and acceptable tick tails. Measure actor capture time separately from background materialization/encoding; verify actual scaling at 1k/10k/100k nodes.

### T13 — Make large UI transactions proportional to changed data

**Finding:** F08. **Depends on:** T03, T10. **Owner:** `golden_ui` projection and scheduler, app integration tests.

**Start at:** `packages/golden-ui/store/graph-event-projection.ts`, `packages/golden-ui/transport/staged-frame-batches.ts`, and existing workbench/graph/WebSocket tests under `apps/chataigne/ui/src/lib/tests/`.

- Introduce a versioned graph-index abstraction with structural sharing and atomic root publication. Prefer persistent/chunked indexes that copy touched paths/pages. Preserve node/parent/child/parameter lookup and iteration semantics for existing consumers. Select a library only after checking its fit with the locked toolchain and actual access patterns.
- Stage transaction deltas without cloning all pre-existing maps. Capture the exact base generation/cursor; commit all indexes and cursor once. A stale/cancelled transaction must release staged memory and enter the existing resync path when necessary.
- Support insertion, deletion, mixed structural edits, reparenting, undo/redo, and parameter-index changes. Retain the low-overhead small-transaction path with the same atomicity guarantees.
- Add a measured time budget alongside operation limits. Account for actual work, including changed descendants and index updates. Budget any compaction; prevent overlay chains, tombstones, old versions, or a post-commit flatten from reintroducing growing pauses.
- Check downstream Svelte stores/adapters: `new Map`, spreads, full arrays, derived indexes, or synchronous cleanup must not hide the same full-graph copy at publication.

**Acceptance:** preserve the real default-scheduler regression harness: insert 600 nodes into 1k/10k/100k existing nodes, including a separate parameter-heavy case. Assert atomic visibility, complete indexes, cursor order, and bounded cancellation. At fixed delta, count work to prove it is not linear in existing graph size. Proposed initial gate for the original parameter-empty chain fixture: at most 20 frame callbacks at each size with the default 512 work budget. This is a new target, not an observed result.

Also test large deletes/mixed edits, repeated transactions/compaction, resync during staging, stale generations, selection/focus, outliner/inspector agreement, and undo/redo. Run full-workbench action-to-paint measurements; scheduler frame counts alone do not qualify browser responsiveness. Keep the original 12/65/592-frame baseline separately labeled as synthetic staging evidence.

### T14 — Remove identity-only worker overhead and implement real sparse selection

**Finding:** F09, immediate portion. **Depends on:** T03, T07, T11. **Owners:** engine runtime center and reusable scheduler.

**Start at:** `crates/golden_core/engine/src/runtime_center.rs`; `crates/golden_core/runtime/control/src/scheduler.rs`; engine scheduled updates.

- Benchmark direct deterministic selection against `InputIdentityExecutor` round trips on matching fixtures. Replace worker dispatch that only returns IDs with direct ordered selection when the measurements support it; simplify dead paths without removing working Alchemist compilation.
- Build direct dirty-ID/ordinal or dirty-word indexes for sparse execution. Visit dirty work and necessary dependencies/due entries; avoid an unconditional walk over every schedule unit in sparse mode.
- Keep a dense traversal for workloads above a measured crossover point. Preserve deterministic ordering from T07, due-time semantics, duplicate dirty marking, removals, and generation changes. Define work selection separately from executing callbacks.

**Acceptance:** sparse/dense/direct paths produce identical selected work, values, and effects. Instrument visited units at 0%, 0.1%, 1%, 10%, and 100% dirty density; sparse visits must follow dirty work rather than total schedule length. Record p50/p95/p99/max, allocations, and workload crossover. Real node callbacks remain serial until T18; label that honestly.

### T15 — Move reusable contracts into their owning crates

**Finding:** F14, ownership portion; supports F10. **Depends on:** T05, T09, T10, T12. **Owners:** protocol, script, persistence, engine adapters, codegen.

**Start at:** `crates/golden_core/services/protocol/`, `runtime/script/`, `services/persistence/`, engine `ui_sync`/script/persistence modules, `support/codegen/`, and the default `golden_core` facade.

- Make neutral protocol DTOs depend on foundation types, not the engine. Keep intent application, engine read-model behavior, and concrete engine adapters in the engine layer. Update the generator, generated TypeScript, and consumers together.
- Move reusable VM/deadline/budget/effect primitives into `golden_script` with explicit host traits. Keep node/application bindings in an engine adapter and app-specific snippets/templates under the app. Do not move the same engine-dependent re-export behind another facade.
- Move project document/schema/version/codec responsibilities into persistence; let engine extraction/application use those contracts. Keep native dialogs and host save workflow in host adapters. Preserve the coordinator and failure tests.
- Preserve a ready-to-launch default host, plus explicit documented engine-only/protocol-only/audio-only surfaces. Verify dependency direction with external consumer fixtures, not only workspace builds.

**Acceptance:** a protocol/codegen consumer does not pull in engine/QuickJS/desktop; script primitives compile with a tiny fake host; persistence codecs round-trip fixtures without a desktop dependency. A Golden-only headless consumer and default full-host consumer compile using public APIs. App behavior/codegen checks remain green; no app-owned Alchemist dependency enters Golden crates.

### T16 — Qualify ASIO/JACK in ordinary application artifacts

**Finding:** F15. **Depends on:** T01, T02. **Owners:** Golden Audio features, app wiring, bootstrap and release tooling.

**Start at:** root/app/Golden Audio Cargo manifests; `package.json`; `tools/bootstrap/toolchain.json`; CI/release workflows; `vendor/cpal-0.18.1/CHATAIGNE_PATCH.md`.

- Define one supported-host/architecture matrix and connect it to ordinary app features, bootstrap requirements, packaging, and documentation. Preserve Windows WASAPI/ASIO and add JACK to ordinary supported desktop artifacts through appropriate feature forwarding. Keep native platform hosts and explicitly optional PipeWire behavior.
- Verify backend compilation, runtime library/driver discovery, stream opening, and physical continuity as separate capabilities. Missing JACK/ASIO installations must not prevent normal startup; only selected backends initialize, and reconnect remains recoverable. Explicitly document any unsupported host/architecture rather than silently dropping it.
- Verify the vendored CPAL ASIO patch and its upstream/removal condition. Choose a reproducible dependency distribution for external Golden Audio consumers; a root-only `[patch]` is not automatically inherited by downstream crates. Retain required licensing/provenance records without expanding this repair into a general license-policy rewrite.
- Test an external consumer outside the monorepo so successful resolution does not accidentally rely on this workspace's path patches.

**Acceptance:** feature inspection and actual built ordinary artifacts match the documented matrix; clean setup, default startup, missing-driver behavior, and backend selection pass on available platforms. Native stream/hotplug/continuity evidence is recorded only when executed on named devices/drivers. A standalone consumer resolves the intended CPAL implementation. CI backend probes alone cannot close hardware qualification.

### T17 — Remove obsolete repository entries and split cohesive responsibilities

**Finding:** F16. **Depends on:** T00 for hygiene; relevant behavior fixes before structural extraction. **Owners:** repository maintenance and touched subsystems.

- Confirm the four baseline gitlinks are unused by current builds/docs: `src-ui/src/lib/golden_alchemist_ui`, `src-ui/src/lib/golden_ui`, `submodules/golden_alchemist_core`, and `submodules/golden_core`. Remove only obsolete index entries, preserving any unrelated local contents. Do not add a fictitious `.gitmodules` to retain dead structure.
- Reconcile contradictory progress notes. Distinguish historical/incomplete attempts, measured final results, and qualification still pending; do not erase useful history.
- Refresh the size inventory. Split oversized modules by ownership while working nearby: script VM/host/effect handling, protocol DTOs versus application, projection versus canvas interaction, formula/state integration, dashboard/curve editor responsibilities. Avoid a huge mechanical rearrangement mixed into a race or persistence fix.
- Put tests under the appropriate `tests/` directories and document legitimate large-file exceptions. Do not solve a line limit by dense formatting or arbitrary helper fragments.

**Acceptance:** fresh monorepo setup does not need submodule initialization; submodule tooling no longer errors on orphan entries; touched ownership boundaries are documented; the inventory and exceptions are current; behavior-focused tests still cover extracted responsibilities. The baseline count of 44 oversized files is historical until recounted.

### T18 — Parallelize actual independent formula/lane computation where justified

**Finding:** F09, computation portion. **Depends on:** T07, T11, T14, T15; T13/T12 measurements inform the decision. **Owners:** app-owned Alchemist execution and reusable runtime contracts.

- Profile representative product workloads after removing identity overhead. First record whether pure formula/lane computation is still a significant bottleneck and where batching amortizes dispatch costs.
- If supported by evidence, expose real pure compute batches over immutable inputs and generation-bound compiled artifacts. Use lane-private state/scratch and stage state/effects until the defined commit boundary. Preserve formula sharing, stable context keys, global state-machine truth, and managed output limits.
- Commit results/effects in deterministic compiled order. Reject stale-generation results; define failure/cancellation without partial lane-state commits. Keep arbitrary mutable engine callbacks and native/device calls out of parallel kernels.
- Retain a sequential implementation and a measured crossover threshold. Avoid a second authoritative state-machine/formula runtime or Alchemist imports in Golden infrastructure.

**Acceptance:** one/two/four/eight workers, where available, produce equivalent values, lane state, state transitions, and ordered effects on actual product formulas. Include stateful operators, shared formulas, conflicts, triggering, lane reorder, cancellation, and generation replacement. Record end-to-end tick/latency/CPU/memory changes. If no useful improvement is demonstrated, retain the simpler serial production path, document the result, and describe parallel compute as deferred rather than completed.

### T19 — Qualify the complete product and record remaining limits

**Findings:** F13 product evidence; final acceptance of F01–F16. **Depends on:** relevant implementation tasks above; hardware-only checks may remain explicitly unavailable.

- Extend existing qualification tooling rather than inventing a second reporting system. Use the exact source/artifact/features under review. Every required scenario must produce a result or an explicit missing-evidence failure.
- Preserve the scheduler/kernel benchmarks but add real product fixtures: Formula compilation/evaluation, processors, state transitions, contexts, modules, outputs, transport, full workbench, and persisted reload. Use both 1k×100 and 10k×10 lane partitions, plus stateful variants; do not imply that 100k values, 100k authored nodes, and one 100k-key Cartesian expansion are equivalent capacities.
- Exercise 1k/10k/100k authored-node stress scenarios as supported by fixture construction, sparse/dense changes, create/duplicate/delete/undo/redo, multi-client reconnect/resync, edits during saves, script failure, queue overload, project replacement, and reload/recovery. Keep unsupported capacity explicit and user-visible.
- Run normal `cargo run`, watch, dev-server, headless, and package workflows. Test native webview/dialog/menu/focus/DPI/lifecycle separately from a packaged binary's headless smoke. Do not drive the user's desktop with synthetic input; use isolated permitted harnesses and documented manual native cases.
- Qualify named physical audio devices for stream continuity, underruns, callback/render timing, reconnect/hotplug, routing identities, latency/drift, and driver selection. Keep physical tests separate from no-device probes and simulated render tests.
- Define persistence version ownership and round-trip fixtures. Implement narrow migrations only when a schema change requires them; retain clear errors for unsupported future versions.

**Evidence to retain:** tick p50/p95/p99/max and deadline misses; allocations and retained memory; queue items/bytes/age/rejections; action-to-paint p50/p95/p99; browser long tasks and preview freshness; capture/encode/publication costs; audio continuity/recovery; exact source, fixture, platform, build, and feature fingerprints.

**Proposed initial budgets, to record on one reference machine before optimizing:**

| Scenario                              | Acceptance target and qualification boundary                                                                                                                                            |
| ------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Existing multiplex reference workload | Preserve the documented 10 ms tick target on the specified fixture/host; retain tail/max and missed-deadline counts, not just averages                                                  |
| UI projection                         | T13's ≤20 captured frame callbacks for the original 600-node synthetic insert at each graph size; separately measure render/transport                                                   |
| Full-workbench 600-node insertion     | Provisional p95 action-to-paint ≤250 ms at 10k nodes and ≤500 ms at 100k on the reference machine; stress targets, not a current support claim                                          |
| Staging service                       | Target approximately 4 ms of budgetable projection work per frame turn; record overruns and long tasks rather than claiming a hard OS scheduling guarantee                              |
| Overload/recovery                     | Document finite queue/memory limits, service budgets, and maximum intended recovery delay per boundary; demonstrate no sustained memory growth or starvation                            |
| Endurance                             | Keep quick smoke; add a measured multi-client endurance run and a longer release-candidate/device run chosen for the failure modes; duration alone is not proof of indefinite stability |

If a proposed budget is not achieved, identify the measured bottleneck and leave the gate open. Changing a target requires a documented capacity/product rationale, not a quieter benchmark or hidden omission. Optional canvas-index/startup optimizations follow profiling and must include visual edge/viewport and editor-loading regressions.

**Acceptance:** a final table covers F01–F16 with implementation and evidence separately, reports important regressions and unavailable checks, and identifies any release blockers still open. No release-ready or hardware-validated claim without the corresponding exact-artifact results.

## 5. Commands and verification workflow

These are commands present in the inspected repository or derived from its actual workspace/package names. Recheck the current manifests and platform prerequisites in T00. Run focused tests for a task, then the required affected gates; reserve the complete cross-platform/product suite for wave/release boundaries.

Use the existing VS Code tasks or `tools/bootstrap/bootstrap.sh` / `bootstrap.ps1` to enforce the repository toolchain. First-time setup can install prerequisites; follow the host's permissions and existing setup instructions.

```sh
# Read-only checkout baseline, from the repository root.
git status --short
git rev-parse HEAD

# Focused runtime correctness suites.
cargo test --locked -p golden_io
cargo test --locked -p golden_runtime
cargo test --locked -p golden_engine -- --test-threads=1
cargo test --locked -p golden_transport_server
cargo test --locked -p golden_persistence

# Portable audio core; native backend suites require their platform dependencies.
cargo test --locked -p golden_audio --no-default-features
cargo test --locked -p golden_audio --features realtime

# UI and qualification-tool gates.
npm test
npm run check
npm run lint
npm run build
python -m unittest discover -s tools/qualification/tests -v

# Required Rust formatting after a Rust implementation batch.
cargo fmt --all
cargo fmt --manifest-path crates/golden_core/Cargo.toml --all
git diff --check
```

Feature-specific suites must match the actual host matrix; do not blindly use `--all-features` across unsupported platforms. Run native/macOS playback suites with T01's exact test filters during diagnosis. Use `cargo metadata` / `cargo tree -e features` and the repository's Clippy/build policy to inspect public dependency and artifact features.

For codegen changes, explicitly run the affected generator scripts from `apps/chataigne/ui/package.json` and the audio package, then compare generated output. The baseline `prepare` script can swallow generator failure; a successful `npm ci` alone is not proof of fresh contracts. Make codegen failures visible in the relevant gate and avoid hand editing generated files.

For the complete product/dependency gate, use the repository's documented PowerShell/bootstrap entry point; on hosts with `pwsh`, the existing script supports:

```powershell
pwsh -NoProfile -File ./tools/product-gate/product-gate.ps1 -DependencyAudit
```

Match feature/platform options to the current script and CI configuration. Missing PowerShell/native prerequisites should be recorded, not hidden by a nominally successful subset.

Smoke these launch contracts using safe fixtures when reaching the relevant integration gate:

```sh
cargo run
cargo xtask watch
cargo run -- --dev
cargo run -- --headless
npm run package:check
```

The `--dev` path uses an existing Vite server (`npm run dev`); watch supervises its development flow. Do not start all these long-running processes simultaneously or leave them running. Packaging/native startup requires the local platform prerequisites and separate hardware authorization for real-device scenarios.

## 6. Finding coverage and baseline source map

All links below pin the audited source. At implementation time use the current checkout and record any moved ownership.

| Finding                                   | Implementation tasks | Baseline starting evidence                                                                                                                                                                                                                                                                                                             |
| ----------------------------------------- | -------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| F01 — release failures                    | T01, T19             | [priority guard](https://github.com/Golden-Geek/Chataigne2/blob/5392728f51f9584c529b6e1e75f72e3d5ede7c85/crates/golden_audio/src/realtime/priority.rs), [CI](https://github.com/Golden-Geek/Chataigne2/actions/runs/33959407568)                                                                                                       |
| F02 — pending readiness                   | T04, T11             | [pending channel](https://github.com/Golden-Geek/Chataigne2/blob/5392728f51f9584c529b6e1e75f72e3d5ede7c85/crates/golden_core/runtime/io/src/pending.rs)                                                                                                                                                                                |
| F03 — uninterruptible scripts/effects     | T05, T15             | [script runtime](https://github.com/Golden-Geek/Chataigne2/blob/5392728f51f9584c529b6e1e75f72e3d5ede7c85/crates/golden_core/engine/src/script/mod.rs)                                                                                                                                                                                  |
| F04 — destructive replacement             | T08, T11             | [application lifecycle](https://github.com/Golden-Geek/Chataigne2/blob/5392728f51f9584c529b6e1e75f72e3d5ede7c85/crates/golden_core/engine/src/application.rs)                                                                                                                                                                          |
| F05 — cache restoration                   | T06                  | [scheduled updates](https://github.com/Golden-Geek/Chataigne2/blob/5392728f51f9584c529b6e1e75f72e3d5ede7c85/crates/golden_core/engine/src/engine/runtime/scheduled_updates.rs), [dispatch](https://github.com/Golden-Geek/Chataigne2/blob/5392728f51f9584c529b6e1e75f72e3d5ede7c85/crates/golden_core/engine/src/engine/dispatch.rs)   |
| F06 — unbounded work/lifecycle            | T11, T12             | [control admission](https://github.com/Golden-Geek/Chataigne2/blob/5392728f51f9584c529b6e1e75f72e3d5ede7c85/crates/golden_core/runtime/control/src/control.rs), [compiler](https://github.com/Golden-Geek/Chataigne2/blob/5392728f51f9584c529b6e1e75f72e3d5ede7c85/crates/golden_core/runtime/control/src/compiler.rs)                 |
| F07 — topology ties                       | T07, T14, T18        | [topological ordering](https://github.com/Golden-Geek/Chataigne2/blob/5392728f51f9584c529b6e1e75f72e3d5ede7c85/crates/golden_core/engine/src/engine/runtime/tick.rs)                                                                                                                                                                   |
| F08 — UI index copying                    | T13, T19             | [projector](https://github.com/Golden-Geek/Chataigne2/blob/5392728f51f9584c529b6e1e75f72e3d5ede7c85/packages/golden-ui/store/graph-event-projection.ts), [scheduler](https://github.com/Golden-Geek/Chataigne2/blob/5392728f51f9584c529b6e1e75f72e3d5ede7c85/packages/golden-ui/transport/staged-frame-batches.ts)                     |
| F09 — identity work/full scans            | T14, T18             | [production integration](https://github.com/Golden-Geek/Chataigne2/blob/5392728f51f9584c529b6e1e75f72e3d5ede7c85/crates/golden_core/engine/src/runtime_center.rs), [selection](https://github.com/Golden-Geek/Chataigne2/blob/5392728f51f9584c529b6e1e75f72e3d5ede7c85/crates/golden_core/runtime/control/src/scheduler.rs)            |
| F10 — snapshots/encoding                  | T12, T15             | [read model](https://github.com/Golden-Geek/Chataigne2/blob/5392728f51f9584c529b6e1e75f72e3d5ede7c85/crates/golden_core/engine/src/ui_read_model.rs)                                                                                                                                                                                   |
| F11 — concurrent saves                    | T09, T12             | [save host](https://github.com/Golden-Geek/Chataigne2/blob/5392728f51f9584c529b6e1e75f72e3d5ede7c85/crates/golden_core/hosts/transport/src/project_host/mod.rs), [file transaction](https://github.com/Golden-Geek/Chataigne2/blob/5392728f51f9584c529b6e1e75f72e3d5ede7c85/crates/golden_core/services/persistence/src/file_store.rs) |
| F12 — dependency gate                     | T02, T19             | [qualification run](https://github.com/Golden-Geek/Chataigne2/actions/runs/33959407565), advisory links in T02                                                                                                                                                                                                                         |
| F13 — incomplete benchmarks/product proof | T03, T13, T14, T19   | [comparator](https://github.com/Golden-Geek/Chataigne2/blob/5392728f51f9584c529b6e1e75f72e3d5ede7c85/tools/core/bench_compare.py), [recorded multiplex results](https://github.com/Golden-Geek/Chataigne2/blob/5392728f51f9584c529b6e1e75f72e3d5ede7c85/docs/progress/multiplex-performance-stability-plan.md)                         |
| F14 — facades/edit acknowledgement        | T10, T15             | [protocol facade](https://github.com/Golden-Geek/Chataigne2/blob/5392728f51f9584c529b6e1e75f72e3d5ede7c85/crates/golden_core/services/protocol/src/lib.rs), [script facade](https://github.com/Golden-Geek/Chataigne2/blob/5392728f51f9584c529b6e1e75f72e3d5ede7c85/crates/golden_core/runtime/script/src/lib.rs)                      |
| F15 — ordinary audio/CPAL portability     | T16, T19             | [app features](https://github.com/Golden-Geek/Chataigne2/blob/5392728f51f9584c529b6e1e75f72e3d5ede7c85/apps/chataigne/Cargo.toml), [CPAL patch](https://github.com/Golden-Geek/Chataigne2/blob/5392728f51f9584c529b6e1e75f72e3d5ede7c85/vendor/cpal-0.18.1/CHATAIGNE_PATCH.md)                                                         |
| F16 — gitlinks/docs/source size           | T17                  | [repository instructions](https://github.com/Golden-Geek/Chataigne2/blob/5392728f51f9584c529b6e1e75f72e3d5ede7c85/AGENTS.md), tracked gitlink and source-size inventory from the baseline audit                                                                                                                                        |

## 7. Progress ledger template

Create `docs/progress/audit-remediation-status.md` during T00. Keep it short and link to deeper evidence artifacts when needed.

```markdown
# Audit remediation status

Audit baseline: 5392728f51f9584c529b6e1e75f72e3d5ede7c85
Working branch / starting SHA:
Current SHA and uncommitted patch state:
Toolchain / OS / features:

## Current batch

Task:
Owning layer and files:
Invariant / implementation decision:
Progress and remaining work:

## Finding status

| Finding | Status | Implementation evidence | Verification / gaps |
| ------- | ------ | ----------------------- | ------------------- |
| F01     | open   |                         |                     |

<!-- Include every finding through F16; retain separate subparts. -->

## Commands and results

| Command or CI job | SHA/patch | Environment | Outcome | Evidence |
| ----------------- | --------- | ----------- | ------- | -------- |

## Next task

Next dependency-ready task:
Known blockers and independent work that can continue:
Required native/hardware evidence still missing:
```

Finish each implementation session with the reviewable diff, updated ledger, and a short explanation of what changed, what was tested, and what remains. Completion of this plan means the evidence supports its claims; it does not automatically authorize publishing or release.
