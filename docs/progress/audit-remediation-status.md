# Audit remediation status

Audit baseline: `5392728f51f9584c529b6e1e75f72e3d5ede7c85`

Working branch / starting SHA: `main` / `5392728f51f9584c529b6e1e75f72e3d5ede7c85`

Previous evidence baseline SHA: `2db5a0ac` contains T00–T15, the T16 audio-artifact wiring and independent
Git-consumer qualification, removal of the four obsolete gitlinks, repaired app test fixtures, and
the cross-platform artifact gate. The T17 script split, initial source-size inventory, and
documentation reconciliation, plus the UI-sync, persistence, history, and App Control runtime
splits, are committed. The App Control node and received-value splits and T18 initial
product-runtime baseline, formula integration, and app-node codegen splits are also committed. The
test-only T18 processor phase instrumentation, opt-in compiled-kernel probe, and source-pinned
measurements, processor presentation and multiplex test splits, and the opt-in stateful
100k-lane scale harness, test-only 1/2/4/8-worker comparison with reordered contexts, focused
distinct-state reorder regression, and unchanged-input requested-evaluation probe are committed.
Their source-pinned measurements are the current documentation patch. T19 now has
source-fingerprinted direct product-Formula and persisted full-workbench graph qualifications.
Activation prepares the initial project-formula cache before timed ticks; full-product
qualification remains open.

Toolchain / OS / features: Windows 11 10.0.26200 x64; Intel64 Family 6 Model 198; Rust and Cargo
1.97.0; Node 26.5.0; npm 11.17.0; Python 3.14.6. These match
`tools/bootstrap/toolchain.json`. Only `x86_64-pc-windows-msvc` is installed locally.

Remote reconciliation: `origin/main` returned the same audited SHA via `git ls-remote` on
2026-09-05. The exact-commit GitHub Actions runs are
[CI 33959407568](https://github.com/Golden-Geek/Chataigne2/actions/runs/33959407568),
[Benchmarks 33959407531](https://github.com/Golden-Geek/Chataigne2/actions/runs/33959407531), and
[Product Qualification 33959407565](https://github.com/Golden-Geek/Chataigne2/actions/runs/33959407565).

## Current batch

Task: T17 — repository cleanup and cohesive source splits are underway. Four orphan gitlinks were
removed from Git's index without deleting the clean local nested checkouts, so an independent Git
consumer no longer fails on their missing `.gitmodules` URLs. The engine script adapter now puts
template discovery/include expansion and host bridging in separate modules, keeping its lifecycle
file under 1,000 lines. The UI-sync adapter now separates conversion, creation/duplication, and
snapshot/event projection from intent coordination. Persistence now separates durable metadata
types from record application, and history separates replay mechanics from effect capture and its
public API. App Control separates platform window actions from process and folder watching, and
its node separates lifecycle, watch processing, structure, and script request parsing. The
received-value handling separates batch planning from incremental application. The app-owned
formula adapter now separates ANode/socket, property, construction, conversion, snapshot,
external-file, reconciliation, and library responsibilities. Its nested node files register
through the owning module's public re-exports. Processor debug/presentation projection now has
its own module, and the multiplex test separates runtime measurements from interaction checks.
The refreshed inventory finds 43 remaining oversized source files; none has an approved
exception. The old multiplex stop-point instructions are labeled historical.

T16 — ordinary desktop audio artifacts are implemented and qualified locally on Windows x64.
The app forwards ASIO, JACK, and realtime through `golden_audio`; the canonical toolchain manifest,
bootstrap verifiers, developer setup, docs, and CI artifact matrix agree. The reusable crate keeps
its native-only `desktop` default. Native PipeWire remains an explicit Linux option. A clean,
manifest-pinned ASIO SDK setup replaces the stale incomplete local SDK. The app's actual default
test artifact contains WASAPI, ASIO, and JACK; its headless runtime reached the ready health state
while JACK was absent. A standalone consumer from a clean Git revision compiled with all ordinary
audio features and resolved the reviewed vendored CPAL ASIO patch.

T15 remains complete: persistence, protocol, script, transport-only headless, and default full-host
external contracts and dependency checks passed. A T16 app-test build exposed six stale Alchemist
test fixtures using engine metadata patches at a protocol intent boundary; those now use the public
typed conversion, and all 513 app unit tests pass. The six-platform hosted app catalog gate has not
run on this patch, and no named physical stream, hotplug, or continuity test has been executed.

## T16 local qualification evidence

- Windows x64, Rust 1.97.0, default app features `asio,jack,realtime`: Cargo feature tree resolves
  `golden_audio` and vendored CPAL with those features; `cargo check --locked -p Chataigne2` passes
  with the pinned official ASIO SDK and prebuilt UI.
- `cargo test --locked -p golden_audio --features asio,jack,realtime`: 137 unit tests plus all
  integration suites pass. The no-stream backend probe reports ASIO and WASAPI `Available` and
  JACK `MissingServer`; this is discovery evidence, not a physical-stream claim.
- `cargo test --locked -p Chataigne2 --test default_audio_hosts` passes against the compiled app
  catalog. `cargo test --locked -p Chataigne2 --bin Chataigne2` passes all 513 tests, including 33
  Sound Card tests. The exact default-feature headless binary returned HTTP 200 at
  `/api/ui/health`, with `backend_ready` and `engine_read_model_ready` both true.
- `tools/qualification/external_audio_consumer.py` passes from clean commit `52e710d9`: a
  standalone Git consumer compiles `desktop,asio,jack,realtime` and resolves
  `vendor/cpal-0.18.1/Cargo.toml` with the exact-driver source patch. The report is at
  `target/qualification/t16-external-audio.json` (local, ignored by Git).
- PowerShell bootstrap and product-gate contract verifiers, Rust formatting, and CI workflow
  Prettier check pass. Bash bootstrap verification is not claimed locally because this Windows
  checkout converts `.sh` working-tree line endings to CRLF; hosted Unix checkout verification
  remains pending.

## T17 local qualification evidence

- `git submodule status` is empty after deleting the four obsolete index entries; all four clean
  nested local checkouts remain on disk at their original SHAs. A pinned external Golden Audio Git
  consumer then passed without submodule URL errors. `.gitattributes` now requires LF for fresh
  `.sh` checkouts.
- The engine script adapter is split into node lifecycle (851 lines), template resolution (229
  lines), and host bridging (252 lines). `golden_engine::ui_sync` is split into intent
  coordination (638 lines), conversions (210), creation/duplication (968), and snapshot/event
  projection (720). Persistence separates record application (898 lines) from metadata and
  recovery types (381); history separates effect capture/API (744) from replay (701). The
  App Control keeps worker/process/folder ownership in its runtime (879 lines), while window
  discovery and actions live in a platform adapter (209). Its module node splits lifecycle (869),
  watch processing (608), watch structure (462), and script request parsing (102). The inventory
  records the still-open source-size gate. Received-value
  batch planning (874 lines) and incremental application (221) are now separate.
- The app-owned formula integration root is 572 lines; its eight focused siblings range from 297
  to 824 lines. App-node codegen keeps nested node-bearing children under their declared module
  instead of path-importing them as duplicate top-level modules. Two generator regression tests,
  strict codegen/app Clippy, and all 513 app unit tests pass with `GC_SKIP_UI_BUILD=1` for the
  Rust-only app checks. The normal UI-embedding build was attempted but SvelteKit failed while
  writing `.svelte-kit/output/server/manifest-full.js`; no full-asset check is claimed for this
  patch.
- `cargo clippy --locked -p golden_engine --all-targets -- -D warnings` and all 415 active engine
  tests pass after the four adapter splits. The default-feature Chataigne check and all 513 app
  unit tests pass after the history/persistence split. The old multiplex progress note is
  explicitly archival; its July 2026 completion does not claim current product qualification.
- After both App Control splits, the default-feature Chataigne check, all 513 app unit tests, and
  strict `cargo clippy --locked -p Chataigne2 --all-targets -- -D warnings` pass.
- After the received-value split, the same default-feature check, strict app Clippy, and all 513
  app unit tests pass again.
- Processor execution/property resolution (956 lines) and debug/presentation projection (201)
  now have separate owners. The multiplex test keeps shared helpers (415 lines), runtime tests
  (311), and interaction tests (312) apart. Both processor feature modes pass 71/72 tests;
  strict feature-enabled app Clippy and all 513 default-feature app tests pass after extraction.
- Generic graph edge routing now owns obstacle bucketing, bounded path search, smoothing, and SVG
  path construction outside the Svelte canvas. The canvas still owns live node geometry, viewport
  interactions, and its per-edge cache. Generic presentation projection now separately owns
  optimistic overlays, spatial indexing, viewport bounds, and document-ordered visible-edge
  selection. Camera geometry now owns inset normalization, anchored zoom, graph bounds, and frame
  targets. Home and Frame Selection no longer spread all node positions into `Math.min`/`Math.max`;
  a direct 150k-node framing regression passes beyond the former spread-call argument limit.
  `GraphCanvas.svelte` fell from 3,533 to 3,110 lines across the three splits, so it remains
  oversized. Thirteen direct generic graph regressions, all 83 app UI tests, the zero-warning
  Svelte type check, and the production UI build pass locally.
- Logger record decoration, duplicate grouping, filtering, and clipboard presentation now belong
  in `logger/log-projection.ts` (267 lines); `LoggerPanel.svelte` retains panel state, controls,
  scroll/focus behavior, and rendering (868 lines). Seven direct projection tests, all 83 app UI
  tests, the zero-warning Svelte check, and a production UI build pass locally. The refreshed
  source inventory had 45 files above 1,000 lines at that point, with no approved exceptions.
- The app-owned state-machine panel now delegates indexed free-position search to
  `components/state-placement.ts` (132 lines), keeping viewport-center collection and the
  existing create intent in `StateMachinePanel.svelte` (976 lines). Four direct placement tests,
  including 10k existing states, all 87 app UI tests, the zero-warning Svelte check, and the
  production UI build pass locally. The refreshed inventory has 44 oversized files.
- The generic vec2 pad now delegates range validation, clamping, plot coordinates, and grid
  projection to `parameters/vec2-pad-geometry.ts`; the editor keeps interaction and trail state
  (884 lines). Four direct geometry tests, all 87 app UI tests, the zero-warning Svelte check, and
  the production UI build pass locally. The refreshed inventory has 43 oversized files.

## T18 initial profiling boundary

On Windows x64 at `22e1a439`, default app features (`asio,jack,realtime`), the optimized
test-profile product sample measured one serial
production multiplex run at average 5,261 µs, p95 8,103 µs, p99 9,120 µs, maximum 12,567 µs,
and one 10 ms deadline miss. The active sample's serial engine run measured average 4,849 µs,
p95 7,160 µs, p99 7,948 µs, maximum 8,597 µs, and no deadline misses. Its separate
engine-plus-incremental-publication measure averaged 4,864 µs with p95 7,174 µs and p99 7,963 µs.
Both used the real `test_multiplex.noisette` fixture and passed their regression tests. These are
single-run end-to-end observations, not isolated formula/lane compute timings, and their different
measurement scopes cannot be subtracted to infer formula cost. Pure compute profiling, batching
crossover, worker equivalence, and CPU/memory measurements remain open before any parallel path
decision.
Reproduce with `./tools/asio.ps1 -- cargo test --locked -p Chataigne2 --bin Chataigne2
--target-dir target/t16-app-default <test-name> -- --nocapture --test-threads=1`, using
`multiplex_sample_production_runtime_stays_realtime` and
`multiplex_sample_active_runtime_stays_realtime` as the respective test names.

Four further serial runs at `8c3e5548` with test-only app-owned phase timers measured eight
processors and 127 lanes each over 240 dirty ticks. Processor evaluation, which includes lane
enumeration and output assembly as well as compiled graph execution, took 32.8–33.8% of total
tick time; input preparation took 273–300 ms over those ticks. Three runs passed with zero
deadline misses; the fourth had three and failed the real-time assertion. This bounds, but does
not isolate, pure graph-kernel work or establish reliable deadline compliance. See
[T18 formula/lane profile](t18-formula-profile.md) for the exact samples, instrumentation scope,
and decision boundary. Production builds contain no timing instrumentation.

The opt-in `kernel-profiling` build at `dc3ff6d1` measured compiled-graph calls at 24.6–25.0%
of tick time and 73.5–74.7% of the processor-evaluation phase in four serial real-sample runs.
The probe perturbs tick timing, so these runs are not real-time qualification. An ideal zero-cost
eight-worker speedup of the entire measured kernel would still be bounded near 1.28× for this
sample. The same patch corrected processor property-frame resolution: context bindings now work
without an explicit override, and a missing context value falls back to the explicit override or
Formula default. All 72 processor tests with profiling, 71 without it, all 513 app tests with
profiling, and strict app Clippy in both feature modes pass.

At `1af6e8f1`, four serial invocations of each test-only direct processor partition (1,000×100
and 10,000×10) reused the real multiplex project's three-node stateful Formula and captured
input snapshot. All eight invocations evaluated 100,000 lanes with 100,000 distinct retained
lane memories and no diagnostics. Warmed three-tick medians ranged 145–155 ms and 156–164 ms,
respectively; evaluated-process RSS ranged 182–184 MB and 222–223 MB. The synthetic context
axis, disabled copied processor condition, missing output dispatch, and three-tick samples mean
these are Formula partition pilots, not complete product tick or release-capacity evidence.
That serial pilot alone did not establish worker equivalence or a production commit boundary.

At `2e345617`, four serial invocations per 100k-lane partition compared 1/2/4/8 test workers.
Every invocation matched the serial context order, ordered effects, diagnostics, and full final
lane memory after four ticks. Eight-worker median direct-evaluation time ranged 36–45 ms for
1,000×100 and 40–46 ms for 10,000×10, about 3.4–4.3× faster than each test's serial path, while
process CPU usage rose. The workers are scoped and test-only; the synthetic lane axis, disabled
copied condition, retained serial-memory comparison, and absent engine/output commit boundary
prevent a production or 10 ms capacity claim. Sparse/cancellation/generation and full-tick
evidence remain open.

At `258695d5`, one run per 100k-lane shape rotated stable lane keys midway through four ticks.
Serial and 2/4/8-worker variants again matched exact context and effect order, diagnostics, and
retained lane memory. The captured input is identical across lanes, so lane-distinct state
identity under reorder remains unproven. The feature-enabled 513-test functional suite and
strict app Clippy pass. Two explicitly serial full-suite runs with kernel timing enabled missed
one or both strict 5/6 ms averages; the same two tests passed when run alone. Instrumented full
suite timing is not release qualification and the production parallel boundary remains open.

At `2ae31055`, a focused two-lane processor regression binds a Boolean Formula property to
distinct context values, verifies retained memories differ, and reverses lane order without
replaying either trigger edge. All 72 default and 73 profiling-enabled processor tests pass,
as does strict profiling-enabled processor Clippy. This closes the small key-identity test gap,
not the lane-distinct 100k-lane product or production generation/cancellation boundaries.

At `37b6f3b7`, four runs per 100k-lane shape used the production processor API with unchanged
captured inputs after initialization. No intents replayed, but all 100k requested lanes still
entered the compiled graph on every warmed tick. Direct-evaluation medians ranged 94.9–96.2 ms
for 1,000×100 and 104.3–106.9 ms for 10,000×10. The feature-enabled app suite passes 513
active tests with eight manual scale tests ignored; strict app Clippy passes. This is not a
true idle engine tick or sparse-dirty crossover because every processor was deliberately
requested. It reinforces deferring a production worker path until end-to-end product benefit
and a generation-safe commit boundary are demonstrated.

## T19 direct Formula qualification report

At committed source `534865de`, `python tools/qualification/formula_scale.py` extended the
existing `tools/qualification` report pattern to the real persisted `Action` Formula. It ran
all eight opt-in app tests and required exactly two serial partitions, sixteen 1/2/4/8-worker
and reorder cases, and two unchanged-input partitions. Missing or duplicate cases, malformed
metrics, wrong Formula/effect counts, replayed unchanged-input intents, or an incomplete test
result fail the report. All 31 qualification-tool unit tests pass.

The local report at
`target/qualification/formula-scale/20260912T130918Z/formula-scale-report.json` is PASS for
**direct processor evaluation only**. It records commit `534865de`, tested tree
`06c1349bf828824162880a2d4f144b11175e78b0`, fixture SHA-256
`5ebd05f9390462b5d666c6b54e833bf30f97d6ad2b391146c288861202f09390`, default
`asio,jack,realtime` plus `kernel-profiling`, and raw-log SHA-256
`f174fdb45779f30a681b6a4a40715179e8180eb1e7044a09065eeed2688379c6`. One
source-pinned invocation measured serial forced-dense medians of 143/157 ms, unchanged-input
requested medians of 95/104 ms, and eight-worker non-reordered medians of 64/48 ms for
1,000×100 / 10,000×10. These are three-tick medians in one multi-test process, not p95/p99 or
end-to-end tick capacity. The report explicitly lists missing engine/output/UI/transport,
sparse-dirty crossover, and generation/cancellation evidence.

## T19 persisted authored-graph scale

`python tools/qualification/authored_graph_scale.py` builds deterministic streamed fixtures from
the real `test_simple_load.noisette` workbench, then requires the Chataigne app to load, prepare,
tick, sparsely save, and reload each fixture. The generator reports serialized records separately
from live engine nodes: sparse project loading prunes declared records, so serialized count is not
an authored-node capacity claim. The app test verifies the 1k/10k/100k minimum live-node thresholds
and that every cloned Formula graph-root UUID survives save/reload. All 39 qualification-tool tests
pass, including malformed and missing-result checks.

The local source-fingerprinted report is
`target/qualification/authored-graph-scale/20260912T161139Z/authored-graph-scale-report.json`
(schema 2; tested tree `2a25ebdd1cbec64a18d5b6920fa9e4c50bbff39a`, default `asio,jack,realtime`,
optimized app test with UI asset build skipped). Functional checks pass on this Windows x64 host:

| Minimum live nodes | Loaded / prepared / reloaded | Cloned graph roots preserved | Load / prepare / save / reload ms | Tick 1 / ticks 2–5 ms | Reload RSS |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 1,000 | 1,089 / 1,245 / 1,089 | 72 / 72 | 30 / 47 / 14 / 23 | 0.949 / 0.005, 0.041, 0.039, 0.002 | 26 MB |
| 10,000 | 10,091 / 10,247 / 10,091 | 715 / 715 | 235 / 418 / 110 / 238 | 0.682 / 0.005, 0.030, 0.030, 0.002 | 71 MB |
| 100,000 | 100,083 / 100,239 / 100,083 | 7,143 / 7,143 | 2,574 / 6,580 / 1,292 / 3,122 | 0.958 / 0.005, 0.060, 0.032, 0.002 | 513 MB |

All five ticks in this short sample meet the test's 8 ms interval, but **functional PASS is not a
real-time tail-latency, interaction, or release-capacity pass**. A callback-level trace identified the
state-machine manager as the remaining recurring requester. Its own generated condition-validity
result was being classified as a user processor override, dirtying the next tick. After correcting
that boundary and removing snapshot demand from callbacks that do not read the tree, each size
needs only one initial process-tree snapshot. Golden runtime activation now builds that snapshot
and the parameter-control index before timed ticks, discards any older tick-scoped snapshot, and
reuses the prepared snapshot only if no intervening edits invalidate it. Custom signals no longer
force a structural control-index scan. All five timed ticks at every size now build zero snapshots.
Test-only phase timing isolated about 65 ms of the former 100k first tick to initial project-formula
materialization in the app-owned state-machine manager. It now builds that cache in its node-ready
activation callback and invalidates it on subsequent formula structure events. A focused sample
test confirms materializations happen before tick one and are not repeated on that tick. The 100k
first tick fell from 61.80 ms in the previous full report to 0.958 ms here; preparation remains
synchronous at 6.58 s, and the short tick sample does not establish a tail-latency bound. Large
live edits, UI actions, transport fan-out, and recovery remain separate qualifications. This is
separate from T12's bounded
UI/transport/persistence snapshots.

The earlier one-node load/reload difference came from the synthetic fixture, not the codec:
its external-to-project Formula conversion omitted the hidden `formula_copy_source` child that
new project formulas have. Activation added that child, and save correctly preserved it. The
generator now includes one deterministic copy-source reference; the app test and report parser
require exact live-node-count equality after reload. All three sizes pass that stricter gate.

The manager tracks its own generated validity output between snapshots, so a later true-to-false
transition is not hidden by a stale value in the retained startup snapshot.

The latest full suite has 522 active app passes and four ignored manual T19 scale cases; the
previous intermittent multiplex active-runtime timing failure did not reproduce in this run.
Strict app/engine Clippy, 40 qualification-tool tests, and 422 active Golden Engine unit tests
pass. Preparation adds 156 runtime nodes at each authored-graph size. Sparse reload now has the
same live-node count as initial load, while full descendant/value equivalence remains open. The
persisted fixture test does not exercise all graph clones through Formula evaluation, live
edits/undo, UI paint, transport, recovery, or multi-client reconnect. Each scenario's generated fixture and raw log
have SHA-256 hashes in the report; no physical or cross-platform claim follows from it.

## T19 live graph edit baseline

An ignored app test uses the product `DuplicateNodes` batch path on loaded, active authored
Formula fixtures. It duplicates ten independent Constant ANode roots (140 live records) as one
history transaction, then asserts exact formula child-UUID order across undo and redo. On this
Windows x64 optimized app-test build, the generic engine's old per-root checkpoint/lifecycle path
took 1,026 ms at 10k live nodes and 15,798 ms at 100k. The batched path takes 98 ms and 1,395 ms
respectively. These are individual local diagnostic runs, not p95 or action-to-paint evidence.
One checkpoint and one lifecycle pass per creation-context group remove the repeated whole-tree
work during paste. The probe now also ticks the active runtime after duplicate, undo, and redo.
At 100k, batching multiple structural formula events into one reconciliation lowered the
post-duplicate tick from 4,341 ms to 1,421 ms and the post-undo tick from 3,351 ms to 1,849 ms
in separate local runs; the latest post-redo tick was 1,974 ms. Before history batching, undo
took 3,670 ms and redo 10,834 ms for the same ten roots. Same-parent multi-add replay now uses
one destroy pass for undo and one UI catalog snapshot plus ready batch for redo. A subsequent
100k run measured 383 ms undo and 1,042 ms redo; duplicate, post-duplicate tick, post-undo tick,
and post-redo tick were 1,304 ms, 1,221 ms, 1,602 ms, and 1,590 ms respectively. These are
single local diagnostics, not tail-latency or action-to-paint qualification. Further app-owned
state-machine event gating removed a second full-tree snapshot for unrelated custom events and
skipped state-network traversal for Formula child edits. One later 100k run measured 852 ms
post-duplicate, 1,076 ms post-undo, and 1,204 ms post-redo ticks; remaining dispatch work still
exceeds the product gate. The ignored live-edit probe now records manager Formula cache refresh,
catalog build, and runtime rebuild phase counters across the edit sequence. Tracing the same 100k
fixture showed socket sync at about 103-118 ms and validation at about 81-85 ms per structural
edit. Reusing socket sync's materialized Formula in validation reduced the latter to about 20-21 ms
and the traced Formula callback to about 125-139 ms; one run's post-duplicate tick measured
766 ms, with 1,043 ms post-undo and 1,176 ms post-redo. The full-tree dispatch snapshot still
costs about 0.25 s per structural edit. A phase trace attributed roughly 145-165 ms to cloning
100k node records, 25-29 ms to enabled-state traversal, and 65-70 ms to child indexing.
The child index now checks short sibling chains without allocating a hash set for each parent,
while retaining cycle detection for long chains. On a later 100k run, index construction was
mostly 53-63 ms, and the duplicate, undo, and redo ticks measured 686, 978, and 1,043 ms.
These are separate local samples, not a p95 comparison; record cloning remains the dominant
snapshot cost, and the
600-node full-workbench action target and sparse/dense edit, transport, and browser gates remain
open; this is not a live-edit capacity pass.

## T19 live multi-root removal baseline

The ignored active-runtime removal probe selects ten independent Constant ANode roots under one
Formula, removes them through `RemoveNodes` as one history transaction, and checks exact sibling
order and graph-root count through undo and redo. The 100k authored fixture removes 140 live
records from 100,239. Before removal batching, one local run measured 3,285 ms remove,
8,751 ms undo, and 2,838 ms redo. A later run with same-parent destroy, history replay, and UI
graph-transaction batching measured 746, 827, and 319 ms respectively; the first active ticks
after those edits measured 423, 571, and 824 ms. On the 10k fixture, the batched run measured
57, 66, and 26 ms for remove, undo, and redo. Generic Golden Engine tests cover nonadjacent
sibling removal and one UI graph transaction for each replay direction; the app UI test covers
multi-removal projection with the final parent-order patch. These are single local diagnostic
samples, not p95 or action-to-paint evidence. The 100k removal action and runtime ticks still
exceed the provisional 500 ms diagnostic threshold; the 600-node full-workbench product target
and sparse/dense edit, transport, browser, platform, and physical gates remain open.
An opt-in trace of the same 100k removal workflow attributed about 187-197 ms of each edit tick
to a required whole-tree dispatch snapshot and about 106-118 ms to the Formula recipient's
structural callback. Undo's created ANode also requested that snapshot. The snapshot cannot be
gated off for these structural events without replacing the data those callbacks read.
The `RemoveNodes` intent now validates every selected target before opening an edit session and
collapses selected descendants under their outermost selected roots. Focused Golden Engine tests
cover descendant-first selection, exact mixed-parent replay, and atomic rejection of missing or
root targets; large mixed-selection and browser qualification remain open.
History replay now reuses the post-restore UI catalog snapshot for ready callbacks when no edit
intervenes. A Golden Engine regression checks that restored callbacks see their nodes and that
undo adds no redundant ready-lifecycle snapshot. One later 100k local run measured 670 ms removal
undo and 644 ms duplicate redo, compared with separate earlier samples of roughly 830 ms and
1,042 ms. The same later runs measured 1,047 ms initial removal and 1,339 ms initial duplicate,
so neither full action nor post-edit ticks pass the product gate; these are not p95 comparisons.
The effective-enabled cache previously could not replace the snapshot's full-tree enabled
traversal: metadata/move history and node replacement could leave inherited state stale. Those
edit and replay paths now reconcile affected subtrees and callbacks. Project load and imported
subtree insertion initialize each decoded node's cache from its attached parent before lifecycle
callbacks, and disabled roots initialize from their own metadata. Process-tree snapshots now
project the cache directly, removing the full enabled-state traversal. Focused tests compare
every cached and projected value against a parent-chain reference through metadata, move,
replacement, add/remove replay, load, and import paths; the full engine and app suites pass.
Live metadata still must flow through engine edits rather than direct mutation of attached nodes.

An ignored active-runtime mixed-parent removal probe adds a two-node independent branch to the
persisted authored fixture, selects one descendant before its selected ANode parent, and removes
ten real Constant ANode roots plus the second-parent leaf in one `RemoveNodes` intent. It verifies
exact parent child-UUID order, graph-root identity, node counts, and one history transaction
through remove/undo/redo. At 10k, the former stepwise path measured 253/655/200 ms and the
disjoint-root batch measured 58/44/27 ms for those three actions. At 100k, the local samples were
4,095/11,855/3,514 ms before and 758/545/341 ms after. The generic engine regression checks
one graph transaction with a final patch for each parent; a nested-removal regression retains
stepwise replay, and the browser store checks both parent patches. These are separate single-run
diagnostics, not p95 or action-to-paint qualification. The 100k post-edit ticks still measured
427/601/824 ms after remove/undo/redo, so the product gate remains open.

After removing the snapshot enabled traversal, the retained authored-graph qualification at
`target/qualification/authored-graph-scale/20260912T212141Z/` passed at 1k/10k/100k; its five
sampled 100k startup/warmed ticks were 738/5/37/31/2 µs. One separate 100k live mixed-parent
removal run measured 700/496/315 ms for remove/undo/redo and 400/548/791 ms for the following
ticks. A separate ten-root duplicate run measured 1,001/318/514 ms for duplicate/undo/redo and
570/774/893 ms for the following ticks. These are single-run diagnostics with different run
conditions from the earlier samples, not an isolated effect size, p95, or action-to-paint pass.
The active-runtime and full-workbench product gates remain open.

The process-tree builder now moves the fields from an owned parameter state into its node record
instead of cloning a full editor parameter snapshot and cloning three fields again. Built-in
parameters expose only value, constraints, and control state to this path; custom nodes retain a
full-snapshot fallback. Child declaration indexes are built only for parents wider than 16
siblings, with an ordered scan for smaller parents. A focused test covers the 16/17-child boundary
and first-match semantics. Separate traces of the same 100k fixture measured roughly 118-140 ms
for node cloning and 45-50 ms for child indexing before these changes, versus 100-121 ms and
16-22 ms afterward. One mixed-parent remove/undo/redo run measured 578/402/257 ms for the actions
and 349/506/718 ms for following ticks; one ten-root duplicate run measured 815/270/408 ms and
517/684/756 ms respectively. The retained 1k/10k/100k authored-graph qualification under
`target/qualification/authored-graph-scale/20260912T215114Z/` passed. These are separate local
diagnostic samples, not p95 or action-to-paint proof. The active-runtime and full-workbench gates
remain open.

The 100k mixed-parent removal trace also found an 81 ms preamble on the first tick after undo,
consistent with releasing the prior large tick-scoped snapshot on the engine thread. Large,
uniquely owned tick snapshots now release through a bounded two-slot retirement pool; saturation
falls back to synchronous release and engine metrics expose active/peak/rejected counts. A focused
test covers background completion and full-capacity fallback. In a separate run on the same
100k fixture, the undo tick preamble measured 0 ms and the tick 450 ms, versus 81 ms and 503 ms
before; remove and redo ticks were 349 and 693 ms. These are local single-run diagnostics, not
p95 or action-to-paint evidence, and the product gate remains open.

The same 100k mixed-parent trace exposed 56-62 ms per post-edit tick in generic schedule
resolution: topological sorting inserted every passive leaf into an ordered frontier. Resolution
now sorts only scheduled nodes and dependency participants, while still validating dependencies
against all live nodes and rejecting passive cycles. In a separate local run of the same fixture,
resolve took 2 ms per edit tick; whole ticks remained 288-674 ms. These are single diagnostic
samples, not p95 or action-to-paint qualification.

Trace-only Formula phase counters on the same 100k authored fixture localize the next backend
cost. Each structural reconciliation of about 7.1k ANode children spent roughly 90-106 ms in
socket sync: 52-55 ms materializing the Formula graph, 8-10 ms solving types, and 24-26 ms
checking sockets. Within materialization, ANode extraction took 41-46 ms; graph transaction
assembly and commit together took about 9-10 ms. A trial that skipped a duplicate signature
construction for unforced bindings showed no clear improvement and was reverted. Reusing
unchanged ANode materialization would require explicit invalidation for config, connections,
surface, and history; the current trace does not establish a safe incremental path or a product
gate pass.

The live Formula node now caches pre-surface ANode instances and invalidates them from inbox
events, including layout edits that defer reconciliation. Connections, surface construction,
type solving, and typed-graph validation still run on every materialization; unclassifiable
deletions and transaction shapes fall back to full ANode extraction. Focused tests compare cached
and full results through type edits, deferred layout edits, removal, connection creation, and
fallback. In one later 100k mixed-parent removal probe, each edited Formula reused about 7,133
unchanged ANodes: extraction measured 6-8 ms and bulk socket sync 63-69 ms, versus the separate
pre-cache samples of 41-46 ms and 90-106 ms. A separate 100k duplicate/undo/redo probe passed
with roughly 7.1k reused ANodes per edit; its post-edit ticks measured 355/619/652 ms. Other
runtime consumers still materialize the graph afresh, and these isolated samples do not satisfy
the full-workbench p95 action-to-paint or recovery gates.
The source-fingerprinted authored-graph load/tick/save/reload report under
`target/qualification/authored-graph-scale/20260912T230825Z/` passed its 1k, 10k, and 100k
fixtures with the cache in place; it does not exercise live UI transport or browser paint.

The authored-graph qualification now supports `--live-edits`, running ten-root duplicate,
same-parent removal, and mixed-parent/descendant removal with undo, redo, and active ticks at
each of 1k/10k/100k. It retains per-case commands, exact fixture/log hashes, parsed counts,
and explicit missing-evidence failures. The source-fingerprinted report under
`target/qualification/authored-graph-scale/20260912T231503Z/` passed all twelve startup/live
scenarios while declaring `product_qualification: OPEN`. Its single 100k samples measured
784/251/394 ms for duplicate/undo/redo, 546/396/254 ms for removal, and 531/391/248 ms for
mixed removal; the following ticks were 341/587/612, 290/348/597, and 283/338/588 ms
respectively. These are diagnostic actions and ticks, not p95 action-to-paint; 600-node insertion,
sparse/dense parameter edits, transport, browser, recovery, platform, and physical gates remain
open.

The same runner accepts `--live-edit-roots 43` to exercise a fixed 602-record product Formula
edit (603 records for the mixed-parent case) at all three authored graph sizes. The
source-fingerprinted `target/qualification/authored-graph-scale/20260912T232610Z/` matrix passed
all twelve startup/live cases and verified exact edited-record counts. Its single 10k backend
duplicate/remove/mixed-remove actions measured 81/43/45 ms; at 100k they measured
838/593/527 ms, with following edit ticks of 359/297/284 ms and redo ticks of 619/612/593 ms.
This covers the backend edit/history shape, not browser paint, transport, parameter-dense
mutation, or p95 action-to-paint. The proposed full-workbench 100k gate remains open.

The ANode lifecycle now reconciles its authored structure at ready, not redundantly at init;
socket metadata initialization likewise does not request a tree snapshot. The fixed 602-record
100k duplicate probe still passed its structure and undo/redo assertions, and its init-stage
100k-node snapshot disappeared. One local backend duplicate sample fell from 834 to 648 ms;
the following tick measured 368 ms. These are single diagnostic samples, not an end-to-end
latency qualification, and the full-workbench gate remains open.

The source-fingerprinted `target/qualification/authored-graph-scale/20260912T234031Z/` matrix
passed all twelve startup/live scenarios with this change. Its single 100k duplicate action
measured 630 ms for the 602-record insert, followed by a 371 ms tick; same-parent and mixed
removal actions measured 618 and 542 ms. The report still declares
`product_qualification: OPEN`.

The authored-graph runner now also supports `--parameter-edits`: one Constant `config/value`
change and a batch touching 10% of authored Constant roots at each scale, with exact value,
undo/redo, dispatch-tick, and Formula-refresh-tick checks. This exposed a runtime invalidation
gap: the state-machine manager updated external Formula input values but did not dirty the
owning Formula after an ANode config edit. App-owned Formula change classification now refreshes
compiled content while excluding layout and generated validation fields. The source-fingerprinted
`target/qualification/authored-graph-scale/20260913T004026Z/` matrix passed all eighteen
startup/structural/parameter scenarios. Its 100k sparse/dense cases edited 1/715 parameters;
backend batch application took 0/3 ms, dispatch ticks 422/440 ms, and subsequent Formula-refresh
ticks 285/280 ms. These are single backend samples, not UI transport, browser action-to-paint,
or p95 latency evidence. The T19 product gate remains open.

For a same-type numeric Constant `config/value` edit, the app-owned ANode and Formula callbacks
now leave socket shape unchanged, while the state-machine runtime still refreshes the Formula
content on its scheduled tick. Type changes and other config edits retain full reconciliation.
The source-fingerprinted `target/qualification/authored-graph-scale/20260913T005315Z/` matrix
passed all eighteen scenarios again. Its single 100k sparse/dense samples edited 1/715 values:
the dispatch ticks measured 201/206 ms, down from 422/440 ms in the prior local run, and the
Formula-refresh ticks measured 263/263 ms. A trace of the sparse case showed one full-tree
snapshot on the dispatch tick instead of two, with no redundant ANode parameter mutations.
The remaining snapshot and refresh costs are still substantial; these samples do not close the
T19 product gate or establish p95 action-to-paint latency.

The app-owned Formula and ANode callbacks now index Constant value parameters when a structural
snapshot is available. For a same-type numeric value event they use that index and dirty only the
changed ANode's materialization entry; external-file Formulas, type changes, and structural edits
still require the snapshot path. StateMachineManager uses its retained structural snapshot to
classify this exact value event only while its runtime/catalog caches are current. The next
scheduled Formula refresh still builds a fresh snapshot to consume the changed value. The
source-fingerprinted `target/qualification/authored-graph-scale/20260913T011701Z/` matrix passed
all eighteen scenarios; its single 100k sparse/dense dispatch ticks measured 0/4 ms for 1/715
values, with 263/260 ms Formula-refresh ticks. A separate all-7,143-Constant probe passed value
and undo/redo replay checks with a 37 ms dispatch tick and 264 ms refresh tick. A trace confirmed
zero full-tree snapshots on the sparse dispatch tick and one on the refresh tick. These are local
backend samples, not p95 or browser action-to-paint evidence; the T19 product gate remains open.

The state-machine manager now owns a per-Formula ANode materialization cache. Same-type numeric
Constant edits dirty only their ANode entry; structural Formula invalidations discard the cache.
The authored parameter report contract is schema v5 and records the refresh tick's snapshot count
and Formula/catalog/runtime-cache phase counters. The source-fingerprinted
`target/qualification/authored-graph-scale/20260913T020025Z/` matrix passed all eighteen cases.
Its single 100k sparse/dense value-edit samples had 0/4 ms dispatch ticks and 232/225 ms
Formula-refresh ticks; Formula materialization accounted for 21.5/26.8 ms of those refreshes,
versus 57.7 ms before manager caching in a separate sparse local probe. Both refreshes still
cloned 100,239 nodes into one full-tree snapshot. An additional all-7,143-Constant batch passed
value and undo/redo replay, with a 46 ms dispatch tick and a 275 ms refresh tick. These are not
p95 or browser action-to-paint results, and the remaining snapshot cost keeps T19 open.

The retained process-tree snapshot now structurally shares its base node and UUID indexes and
overlays only changed parameter nodes. The state-machine manager classifies same-type numeric
Constant edits at its inbox boundary and refreshes those Formulas from an overlaid retained
snapshot; structural invalidations still request a fresh tree. The source-fingerprinted
`target/qualification/authored-graph-scale/20260913T022124Z/` matrix passed all eighteen
scenarios, including runtime Formula-value readback after edit, undo, and redo. Its single 100k
sparse/dense dispatch ticks measured 0/4 ms and refresh ticks 21/27 ms, with zero full-tree
snapshots and zero cloned snapshot nodes on both refreshes. Formula materialization accounted for
21.3/25.9 ms. A separate all-7,143-Constant batch also passed runtime-value readback and
undo/redo replay; its refresh built no snapshot and measured 85 ms, including 72 ms of Formula
materialization. This removes the prior 100k value-refresh snapshot cost; browser action-to-paint,
p95 tails, broader edit shapes, transport, and recovery remain unqualified.

The authored-graph report contract is now schema v6. Its startup/reload case compares the complete
ordered live tree before runtime preparation and after sparse save/reload: every node's depth,
type, declaration, label, and parameter value must match, while the separate graph-root UUID
assertion remains. The source-fingerprinted
`target/qualification/authored-graph-scale/20260913T025215Z/` matrix passed all eighteen cases
and verified all 1,089 / 10,091 / 100,083 loaded tree nodes at 1k / 10k / 100k. All 50
qualification-tool tests pass, including rejection of a missing or undercounted tree result.
Sparse-document byte/metadata equivalence and UUID identity for every declared descendant are
not claimed; browser, transport, recovery, and tail-latency qualification remain open.

The authored parameter report contract is schema v7. After each edit/undo/redo batch, the app
saves and reloads the product project, resolves every selected Constant through its preserved
authored graph-root UUID, verifies the edited parameter value, and checks the prepared Formula
runtime both before and after a tick. The source-fingerprinted
`target/qualification/authored-graph-scale/20260913T030500Z/` matrix passed all eighteen cases;
the 100k sparse/dense cases persisted and rematerialized 1/715 edited Constants. A separate
all-7,143-Constant 100k batch also passed those checks. This covers a completed backend save/reload
after an edit, not edits concurrent with a save, transport reconnect/resync, or browser recovery.

The product transport qualification now builds the current app binary and launches its reusable
headless host with isolated app data and a loopback UI listener. It loads each authored fixture
through the real project HTTP endpoint, observes `project_loaded` resync markers on three
workbench-plane WebSockets, checks three full snapshots and their complete node-identity digests,
then sends a WebSocket `setParam` intent for one authored Constant. All three clients must receive
the matching parameter delta and show the edited value in fresh full snapshots. The probe then
disconnects and reconnects one client in the same runtime session and verifies the edit persists.
The v3 probe also starts a project save, sends a second edit while its HTTP response is outstanding,
and requires the edit acknowledgement before that response. It reloads the saved file through the
real project endpoint, checks a recovery resync and snapshot on all three clients, equal node counts
and authored-root identity digests, and one consistent old-or-new captured value. A replacement can
resync with `cursor_ahead_of_server_time` when a subscription cursor exceeds the restored engine
clock. The saved reload's complete generated-node digest is recorded separately; it changed in the
local 1k/10k/100k runs, so generated descendant UUID stability is not claimed. The edit was captured
in the saved file in all three runs, with the acknowledgement preceding the save response. This is
client-observed save-request overlap, not proof of server-side capture/edit concurrency.

The source- and artifact-fingerprinted report under `target/qualification/transport-scale/` covers
1k/10k/100k; the initial 100k local baseline loaded in 7,299 ms and showed 100,247 live nodes
and 7,143 authored Constant roots. These individual runs do not establish p95 or browser
action-to-paint, server-side save-capture overlap, slow-client, endurance, packaged-native, or
physical-device evidence.

## Task status and dependencies

| Task | Dependencies                                  | Status                                                       |
| ---- | --------------------------------------------- | ------------------------------------------------------------ |
| T00  | none                                          | complete                                                     |
| T01  | T00                                           | complete locally; native qualification pending               |
| T02  | T00                                           | complete                                                     |
| T03  | T00                                           | complete locally; matching hosted reference baseline pending |
| T04  | T00                                           | complete                                                     |
| T05  | T00                                           | complete                                                     |
| T06  | T00                                           | complete                                                     |
| T07  | T00                                           | complete                                                     |
| T08  | T05, T06                                      | complete                                                     |
| T09  | T08                                           | complete                                                     |
| T10  | T00                                           | complete                                                     |
| T11  | T04, T08, T09, T10                            | complete                                                     |
| T12  | T08, T09, T11                                 | complete                                                     |
| T13  | T03, T10                                      | complete                                                     |
| T14  | T03, T07, T11                                 | complete                                                     |
| T15  | T05, T09, T10, T12                            | complete                                                     |
| T16  | T01, T02                                      | implemented and Windows-qualified; hosted matrix and hardware pending |
| T17  | T00; behavior fixes before related extraction | gitlinks removed, inventory refreshed, engine/App Control/formula, graph routing/projection/camera, logger, app-owned state placement, and generic vec2 geometry owners split; more cohesive splits pending |
| T18  | T07, T11, T14, T15; informed by T12/T13       | real 1,016-lane kernel, 100k-lane stateful partitions, worker/reorder equivalence, and requested unchanged-input cost measured; production parallel deferred pending sparse/lifecycle/full-tick evidence |
| T19  | relevant implementation tasks                 | authored 1k/10k/100k startup, full ordered-tree reload equivalence, 602-record structural edits, sparse/dense parameter replay and save/reload, plus three-client headless transport resync/edit/reconnect and save-request-overlap/reload pass locally; browser p95, server-side save-capture overlap, slow-client/endurance, platform, and physical evidence remain open |

## Finding status

Work proceeds from the exact audited baseline recorded above. Findings are marked fixed only when
the current branch contains implementation and verification evidence.

| Finding                            | Status          | Implementation evidence                                                                                                                                                                                                                                                                                                                                                                           | Verification / gaps                                                                                                                                                                                                                                           |
| ---------------------------------- | --------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| F01 — release failures             | partially fixed | Portable priority-guard `Debug`; callback/render progress barriers; formatted Svelte; independent UI quality/artifact jobs; PR triggers with read-only permissions                                                                                                                                                                                                                                | Windows realtime and complete UI lint/check/build pass. Linux x64/ARM64 compilation and both macOS regressions require patched-source CI.                                                                                                                     |
| F02 — pending readiness            | fixed           | Send publishes after enqueue; receiver-owned bounded drain clears and conservatively re-arms; all consumers migrated                                                                                                                                                                                                                                                                              | Deterministic barrier cases cover the old interleaving, concurrent producers, enqueue during drain, partial/final drains, and disconnect. A one-packet OSC test needs no unrelated wakeup, and T11 preserves the readiness invariant under bounded admission. |
| F03 — script interruption/effects  | fixed           | T05 installs nesting-safe monotonic interruption/cancellation, budgets and resource caps, input validation, success-only effect admission, heap quarantine, and clean reload; T15 moves the VM, enforcement, host contracts, and effect journal into `golden_script`.                                                                                                                          | Public fake-host contract and watchdog suites plus engine recovery pass. Effects are staged until callback success; already-admitted physical I/O is explicitly outside rollback guarantees.                                                                  |
| F04 — project replacement          | fixed           | T08 adds detached prepare/validate/compile, monotonic project generations, exclusive activation, atomic engine/runtime/read-model publication, explicit paused-state failure, stale candidate/compiler fencing, outside-actor retirement, and duplication rollback                                                                                                                                | Barrier and fault-injection coverage spans decode through resource cleanup. T11 caps concurrent detached-engine retirement and rejects overload before generation allocation or candidate preparation.                                                        |
| F05 — cache restoration            | fixed           | Scheduled updates and inbox dispatch borrow the cache in place; all fallible scheduled scratch extraction uses one restore boundary; excess callbacks are rejected before invocation                                                                                                                                                                                                              | Injected budget and edit-absorption failures prove unchanged/changed bindings, node membership, cache contents, accepted-edit policy, and next-tick progress. Panic/unwind recovery is not claimed.                                                           |
| F06 — unbounded work/lifecycle     | fixed           | T11 adds dual-bounded I/O, bounded OSC turns, nonblocking actor admission, constant compiler retention/cancellation, bounded HTTP/WebSocket admission, explicit slow-client policy, capacity-reserved project/device retirement, and bounded delayed recovery. T12 bounds completed UI snapshot retention, compiler layouts, fixed-shard persistence captures, and background transport encoding. | Saturation, recovery, shutdown, ownership, and three-client save/resync contention tests pass with explicit overload and retained-capacity evidence.                                                                                                          |
| F07 — topology ties                | partially fixed | T07 uses one UUID-ordered global ready frontier and compiles stable bucket/runtime order                                                                                                                                                                                                                                                                                                          | Equivalent-order, diamond, disconnected, cycle, and conflicting write/trigger fixtures pass. T14/T18 cross-worker real-kernel determinism remains pending.                                                                                                    |
| F08 — UI index copying             | fixed           | T13 publishes fixed-depth persistent indexes; proportionally projects insert, remove, move, reorder, metadata, and parameter operations under work-count and wall-clock frame bounds; incrementally indexes warnings; and removes hidden panel-wide graph discovery.                                                                                                                              | Scale, time, retention, reset, mixed-transaction, and production-workbench gates pass. Twenty 600-node inserts at 10k/100k nodes remain below the provisional p95 targets with zero action-window Long Tasks.                                                 |
| F09 — identity work/full scans     | partially fixed | T14 removes `InputIdentityExecutor` round trips, separates selection from execution, indexes touched dirty words, walks only set bits below the measured 50% crossover, and reserves hot-path scratch at generation install.                                                                                                                                                                      | Exact direct/sparse/dense selections and real executor digests pass; 0–10% visit counts follow dirty cardinality; warmed allocations are zero. T18's real 1,016-lane sample bounds processor evaluation at 32.8–33.8% of serial tick time; the opt-in compiled-kernel probe measures 24.6–25.0% of profiled tick time. Test-only direct 100k-lane stateful evaluation improves 3.4–4.3× with eight workers and equivalent ordered effects/lane memory, but sparse/lifecycle/production commit and full-tick capacity remain open; production computation stays serial. |
| F10 — snapshots/encoding           | fixed           | T12 uses fixed-shard copy-on-write UI, compiler, and persistence roots; revision/generation-bound captures; one bounded transport encoder with same-version whole-graph JSON reuse; worker-side compiler materialization; outside-actor sparse project materialization/encoding; bounded completed caches; and outside-actor retirement.                                                          | Persistence and transport capture scale at 1k/10k/100k. Three simultaneous saves/resyncs preserve coherent revisions while engine ticks and file I/O continue; exact local measurements are recorded below.                                                   |
| F11 — concurrent saves             | fixed           | T09 adds monotonic save tickets, normalized destination identity, ordered complete transactions, bounded cross-destination concurrency, generation fencing, and winner-only path/revision publication                                                                                                                                                                                             | Deterministic barriers cover reversed completion, aliases, Save As, edits, and replacement. Injected write/restore boundaries prove recovery and subsequent saves. T12 retains capture-scaling work under F10, not save-order correctness.                    |
| F12 — dependency gate              | fixed           | `h2` locked at 0.4.16; `rtrb` constraint and lock at 0.3.5; no advisory suppression added                                                                                                                                                                                                                                                                                                         | `cargo deny check`, `cargo machete`, and both backend-neutral/realtime Golden Audio suites pass against RustSec DB `5a0ebedfe8bdd2e295b171f4162f8c977bcad9a5` (2026-09-02). No reachable Chataigne exploit was established.                                   |
| F13 — benchmark/product proof      | partially fixed | T03 comparator rejects invalid/incomplete/incomparable evidence; workflow retains raw stdout/stderr, fingerprint, and upstream failures; T13 adds a production-browser gate; T14 adds source-fingerprinted release runtime and selection qualification.                                                                                                                                           | T13/T14 product gates pass. Historical values are explicitly unqualified; matching hosted reference and T19's final evidence matrix remain open.                                                                                                              |
| F14 — facades/edit acknowledgement | fixed           | T10 maps authoritative actor-turn acknowledgement into typed graph/project transaction results and adds a crate-external edit consumer. T15 puts codecs, wire DTOs, script declarations, and VM/runtime primitives in their owning crates while engine application stays in adapters; the full facade composes the ready-to-launch host.                                                           | Persistence, protocol, script fake-host, headless-host, and full-host external consumers pass. Focused dependency trees exclude forbidden engine, QuickJS, desktop, Tauri, audio, and Chataigne edges as applicable.                                      |
| F15 — ordinary audio portability | partially fixed | Default app forwards ASIO/JACK/realtime; canonical matrix, pinned SDK, app catalog test, and standalone Git/CPAL consumer are in place. | Windows x64 default artifact, headless startup, JACK missing-server state, 513 app tests, and external consumer pass. Hosted six-platform artifact gate and named physical streams remain unrun. |
| F16 — gitlinks/docs/source size | partially fixed | Four orphan gitlinks were removed without deleting local checkouts; script, UI-sync, persistence, history, App Control, received-value, formula, processor presentation, generic graph routing/projection/camera, logger projection, app-owned state placement, and generic vec2 geometry now have focused owners; stale multiplex notes are archival. | Independent Git consumer, 415 active engine and 513 app Rust tests, and strict Clippy passed after the prior splits. Thirteen generic graph, seven logger projection, four state placement, and four vec2 geometry tests, 87 app UI tests, the Svelte check, and production UI build pass after the UI splits. The refreshed inventory has 43 remaining oversized files and no approved exceptions. |

## Commands and results

| Command or CI job                                                                                                                                                 | SHA/patch       | Environment                                                    | Outcome | Evidence                                                                                                                                                                                                                                                                                     |
| ----------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------- | -------------------------------------------------------------- | ------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `git status --short`, `git rev-parse HEAD`, `git branch --show-current`                                                                                           | baseline        | Windows local                                                  | passed  | `main` at exact audit SHA; only `?? docs/plan/` at T00 start                                                                                                                                                                                                                                 |
| `git ls-remote origin refs/heads/main`                                                                                                                            | baseline        | GitHub remote                                                  | passed  | remote returned the same SHA                                                                                                                                                                                                                                                                 |
| pinned tool version commands                                                                                                                                      | baseline        | Windows local                                                  | passed  | installed Rust/Cargo/Node/npm/Python match manifest                                                                                                                                                                                                                                          |
| `npm run lint`                                                                                                                                                    | baseline        | Windows local                                                  | failed  | Prettier reports only `ModuleIndicators.svelte`                                                                                                                                                                                                                                              |
| `cargo test --locked -p golden_audio --no-default-features`                                                                                                       | baseline        | Windows x64, test profile                                      | passed  | 99 unit tests and 6 external/doc suites passed; playback integration is feature-gated                                                                                                                                                                                                        |
| `cargo test --locked -p golden_audio --features realtime`                                                                                                         | T01 patch       | Windows x64, test profile                                      | passed  | 137 unit tests; 12 playback tests including both repaired races; all integration/doc suites passed                                                                                                                                                                                           |
| `npm run lint`                                                                                                                                                    | T01 patch       | Windows local                                                  | passed  | all checked UI files match Prettier                                                                                                                                                                                                                                                          |
| `npm run check`                                                                                                                                                   | T01 patch       | Windows local                                                  | passed  | Golden Audio UI and Chataigne Svelte checks plus generated-binding freshness passed                                                                                                                                                                                                          |
| `npm run build`                                                                                                                                                   | T01 patch       | Windows local                                                  | passed  | production SvelteKit bundle built; existing large-chunk warning retained                                                                                                                                                                                                                     |
| `npx prettier --check .github/workflows/ci.yml .github/workflows/product-gate.yml`                                                                                | T01 patch       | Windows local                                                  | passed  | workflow formatting is canonical; `actionlint` is not installed locally                                                                                                                                                                                                                      |
| `tools/product-gate/tests/contract-tests.ps1`                                                                                                                     | T01 patch       | Windows local                                                  | failed  | pre-existing T16 contract drift: test expects `default = ["asio"]`, manifest currently has `default = ["asio", "realtime"]`                                                                                                                                                                  |
| CI / Build UI                                                                                                                                                     | baseline        | Windows hosted runner                                          | failed  | formatting failure; downstream application build matrix skipped                                                                                                                                                                                                                              |
| CI / Golden Audio hosts (Linux)                                                                                                                                   | baseline        | Ubuntu 24.04 x64                                               | failed  | `RtPriorityHandleInternal` does not implement `Debug`                                                                                                                                                                                                                                        |
| Product Qualification / linux-aarch64 compatibility                                                                                                               | baseline        | Ubuntu 24.04 ARM64                                             | failed  | same priority-guard `Debug` error                                                                                                                                                                                                                                                            |
| CI / Golden Audio hosts (macOS)                                                                                                                                   | baseline        | macOS 15 ARM64                                                 | failed  | host-stall progress assertion and duplicate-ID event timeout; 10 other playback tests passed                                                                                                                                                                                                 |
| CI / Golden Audio hosts (Windows)                                                                                                                                 | baseline        | Windows hosted runner                                          | passed  | backend-neutral and native feature suites plus no-open host probe passed                                                                                                                                                                                                                     |
| CI / golden_engine tests                                                                                                                                          | baseline        | Ubuntu hosted runner                                           | passed  | single-threaded engine suite passed                                                                                                                                                                                                                                                          |
| Benchmarks / regression check                                                                                                                                     | baseline        | Ubuntu hosted runner                                           | failed  | benchmark commands completed; comparator rejected the result, to be diagnosed in T03                                                                                                                                                                                                         |
| Cross-platform Product Qualification                                                                                                                              | baseline        | Windows/macOS/Linux runners                                    | failed  | all native product gates reported failure; exact detailed repair evidence remains task-specific                                                                                                                                                                                              |
| `cargo tree --locked -i h2` / `-i rtrb`                                                                                                                           | T02 patch       | Windows local                                                  | passed  | `h2 0.4.16` through reqwest/hyper; `rtrb 0.3.5` only through Golden Audio                                                                                                                                                                                                                    |
| `cargo deny check`                                                                                                                                                | T02 patch       | Windows local; advisory DB `5a0ebedf`                          | passed  | advisories, bans, licenses, and sources pass; existing duplicate/no-license warnings remain non-fatal policy output                                                                                                                                                                          |
| `cargo machete`                                                                                                                                                   | T02 patch       | Windows local                                                  | passed  | no unused workspace dependencies found                                                                                                                                                                                                                                                       |
| `cargo test --locked -p golden_audio --no-default-features`                                                                                                       | T02 patch       | Windows x64                                                    | passed  | backend-neutral ownership/ordering/reclamation coverage passed with `rtrb 0.3.5`                                                                                                                                                                                                             |
| `cargo test --locked -p golden_audio --features realtime`                                                                                                         | T02 patch       | Windows x64                                                    | passed  | 137 unit tests plus all integration/doc suites passed with repaired locked graph                                                                                                                                                                                                             |
| `python -m unittest discover -s tools/qualification/tests -q`                                                                                                     | T03 patch       | Windows local, Python 3.14.6                                   | passed  | 22 tests; benchmark fixtures cover valid matching evidence, actual regression, missing/empty/truncated/duplicate/extra cases, malformed/non-finite/wrong-unit values, schema/sample errors, every fingerprint field, and an unqualified baseline                                             |
| `cargo test --locked -p golden_io`                                                                                                                                | T04 patch       | Windows x64                                                    | passed  | 10 tests; deterministic pending-channel barriers cover publication, concurrent producers, enqueue-during-drain, partial/final drains, and disconnect                                                                                                                                         |
| `cargo test --locked -p Chataigne2`                                                                                                                               | T04 patch       | Windows x64                                                    | passed  | 515 app unit tests plus the default Windows audio-host integration test; includes one-packet OSC readiness without a second packet or command wake                                                                                                                                           |
| `cargo clippy --locked -p golden_io --all-targets -- -D warnings`                                                                                                 | T04 patch       | Windows x64                                                    | passed  | shared pending-channel API and tests are warning-free                                                                                                                                                                                                                                        |
| `cargo clippy --locked -p Chataigne2 --all-targets -- -D warnings`                                                                                                | T04 patch       | Windows x64                                                    | failed  | pre-existing strict-lint debt outside T04: one Alchemist `manual_flatten` and five `golden_engine` style lints; no T04 diagnostic was reported                                                                                                                                               |
| `cargo test --locked -p golden_script`                                                                                                                            | T05 patch       | Windows x64                                                    | passed  | 3 public runtime-contract tests and 4 runtime-safety tests pass; watchdog children cover top-level/load/reload and lifecycle infinite loops, cancellation, finite excess, memory, recursion, nesting, queued jobs, teardown, input caps, effect discard, and successful ordering             |
| `cargo clippy --locked -p golden_script --all-targets -- -D warnings`                                                                                             | T05 patch       | Windows x64                                                    | failed  | no T05 diagnostic; compilation stops on five pre-existing `golden_engine` style lints already recorded by the T04 app clippy run                                                                                                                                                             |
| `cargo test --locked -p golden_engine`                                                                                                                            | T05 patch       | Windows x64                                                    | passed  | 391 unit tests and 7 doctests pass; one stress benchmark remains intentionally ignored. Recovery coverage proves a failed script is quarantined while later tick, edit, project serialization, config replacement, and clean reload succeed.                                                 |
| `cargo test --locked -p Chataigne2`                                                                                                                               | T05 patch       | Windows x64                                                    | passed  | 515 app unit tests and the default Windows ASIO compile integration test pass with the corrected persisted script-source representation.                                                                                                                                                     |
| `cargo fmt --all` (root and Golden Core workspaces)                                                                                                               | T05 patch       | Windows x64                                                    | passed  | Rust sources are formatted in both required workspaces.                                                                                                                                                                                                                                      |
| `cargo test --locked -p golden_engine parameter_cache_recovery --no-fail-fast`                                                                                    | T06 patch       | Windows x64                                                    | passed  | Three focused tests inject an update-budget rejection plus scheduled-update and inbox edit-absorption failures, then prove cache, bindings, membership, accepted edits, and next-tick progress.                                                                                              |
| `cargo test --locked -p golden_engine --no-fail-fast`                                                                                                             | T06 patch       | Windows x64                                                    | passed  | 394 unit tests and 7 doctests pass; the pre-existing manual stress benchmark remains ignored.                                                                                                                                                                                                |
| `cargo clippy --locked -p golden_engine --all-targets -- -D warnings`                                                                                             | T06 patch       | Windows x64                                                    | failed  | No T06 diagnostic; the same five pre-existing strict-lint findings recorded at T04/T05 remain in persistence duplication, UI sync, and UI read-model code.                                                                                                                                   |
| strict Golden Engine clippy with the five recorded lint classes allowed                                                                                           | T06 patch       | Windows x64                                                    | passed  | All targets pass after allowing only `unnecessary_lazy_evaluations`, `map_entry`, and `collapsible_if`, confirming no additional T06 warning.                                                                                                                                                |
| `cargo fmt --all` (root and Golden Core workspaces)                                                                                                               | T06 patch       | Windows x64                                                    | passed  | Rust sources are formatted in both required workspaces.                                                                                                                                                                                                                                      |
| `cargo test --locked -p golden_engine schedule_ordering --no-fail-fast`                                                                                           | T07 patch       | Windows x64                                                    | passed  | Three deterministic fixtures pass; the explicit compilation measurement is ignored in ordinary suites.                                                                                                                                                                                       |
| 20,000-node canonical schedule resolve measurement                                                                                                                | T07 patch       | Windows x64, optimized test profile                            | passed  | Fixture `independent-uuid-ascending-v1`, source SHA-256 `6DC1F8CF…61EA5`; resolve-only samples in µs: 8195, 7789, 7577, 7411, 7252, 7634, 7658, 7557, 7349, 7394. Median 7567 µs; fixture construction excluded.                                                                             |
| `cargo test --locked -p golden_engine --no-fail-fast`                                                                                                             | T07 patch       | Windows x64                                                    | passed  | 397 unit tests and 7 doctests pass; the existing stress benchmark and explicit schedule measurement remain ignored by default.                                                                                                                                                               |
| `cargo clippy --locked -p golden_engine --all-targets -- -D warnings`                                                                                             | T07 patch       | Windows x64                                                    | failed  | No T07 diagnostic; the same five pre-existing strict-lint findings remain outside T07.                                                                                                                                                                                                       |
| strict Golden Engine clippy with the five recorded lint classes allowed                                                                                           | T07 patch       | Windows x64                                                    | passed  | All targets pass after allowing only the three known lint classes, confirming the T07 implementation and tests add no warning.                                                                                                                                                               |
| `cargo fmt --all` (root and Golden Core workspaces)                                                                                                               | T07 patch       | Windows x64                                                    | passed  | Rust sources are formatted in both required workspaces.                                                                                                                                                                                                                                      |
| `cargo test --locked -p golden_engine --no-fail-fast`                                                                                                             | T08 patch       | Windows x64                                                    | passed  | 405 unit tests and 7 doctests pass; the existing stress and explicit schedule-measurement tests remain ignored by default. Replacement fault injection, stale candidate fencing, paused-state behavior, input/read-model cutover, and duplication lifecycle rollback are covered.            |
| `cargo test --locked -p golden_transport_server -p Chataigne2 --no-fail-fast`                                                                                     | T08 patch       | Windows x64                                                    | passed  | 28 transport-host tests, 515 Chataigne unit tests, and the default Windows audio-host integration test pass. Host coverage proves malformed decode never enters replacement and exclusive destroy/ready/drop ordering is preserved.                                                          |
| `cargo clippy --locked -p golden_engine --all-targets -- -D warnings`                                                                                             | T08 patch       | Windows x64                                                    | passed  | All engine targets pass strict lint. T08 also removes the five previously recorded style findings in touched persistence/UI files and boxes rejected candidate ownership to keep the error result bounded.                                                                                   |
| `cargo clippy --locked -p golden_transport_server --all-targets -- -D warnings`                                                                                   | T08 patch       | Windows x64                                                    | passed  | The transport project-host changes and their tests are warning-free.                                                                                                                                                                                                                         |
| `cargo fmt --all` (root and Golden Core workspaces)                                                                                                               | T08 patch       | Windows x64                                                    | passed  | Rust sources are formatted in both required workspaces.                                                                                                                                                                                                                                      |
| `cargo test --locked -p golden_persistence --no-fail-fast`                                                                                                        | T09 patch       | Windows x64                                                    | passed  | 10 tests cover same-destination acceptance order, lexical aliases, bounded cross-destination concurrency, generation replacement/abort, dropped tickets, every durable-write boundary, recovery retry, and subsequent saves.                                                                 |
| `cargo test --locked -p golden_engine --no-fail-fast`                                                                                                             | T09 patch       | Windows x64                                                    | passed  | 410 unit tests and 7 doctests pass; the existing stress and explicit schedule-measurement tests remain ignored. Five application barriers cover exact saved revision, later edits, Save As, stale pending saves, and active-transaction replacement fencing.                                 |
| `cargo test --locked -p golden_transport_server --no-fail-fast`                                                                                                   | T09 patch       | Windows x64                                                    | passed  | All 28 transport-host tests pass with runtime-owned save and project-file metadata.                                                                                                                                                                                                          |
| `cargo test --locked -p Chataigne2 --no-fail-fast`                                                                                                                | T09 patch       | Windows x64                                                    | passed  | 515 application tests and the default Windows audio-host integration test pass.                                                                                                                                                                                                              |
| `cargo clippy --locked -p golden_persistence --all-targets -- -D warnings`                                                                                        | T09 patch       | Windows x64                                                    | passed  | The coordinator, file transaction, and fault-injection targets are warning-free.                                                                                                                                                                                                             |
| `cargo clippy --locked -p golden_engine --all-targets -- -D warnings`                                                                                             | T09 patch       | Windows x64                                                    | passed  | Runtime save capture, metadata publication, and replacement-fence targets are warning-free.                                                                                                                                                                                                  |
| `cargo clippy --locked -p golden_transport_server --all-targets -- -D warnings`                                                                                   | T09 patch       | Windows x64                                                    | passed  | Runtime-owned host save/load workflow and tests are warning-free.                                                                                                                                                                                                                            |
| `cargo fmt --all` (root and Golden Core workspaces)                                                                                                               | T09 patch       | Windows x64                                                    | passed  | Rust sources are formatted in both required workspaces.                                                                                                                                                                                                                                      |
| `cargo test --locked -p golden_engine --no-fail-fast`                                                                                                             | T10 patch       | Windows x64                                                    | passed  | 412 unit tests pass, 2 manual measurements remain ignored, the crate-external public-graph consumer passes, and 7 doctests pass. Typed edit rejection, unavailable undo/redo, and actor-turn success revision are covered.                                                                   |
| `cargo test --locked -p golden_transport_server --no-fail-fast`                                                                                                   | T10 patch       | Windows x64                                                    | passed  | All 28 transport tests pass while continuing to expose authoritative acknowledgement details and rejection phases.                                                                                                                                                                           |
| `cargo test --locked -p Chataigne2 --no-fail-fast`                                                                                                                | T10 patch       | Windows x64                                                    | passed  | 515 application tests and the default Windows audio-host integration test pass.                                                                                                                                                                                                              |
| `cargo clippy --locked -p golden_engine --all-targets -- -D warnings`                                                                                             | T10 patch       | Windows x64                                                    | passed  | The application facade, bounded typed error, UI acknowledgement mapping, and all test targets are warning-free.                                                                                                                                                                              |
| `cargo clippy --locked -p golden_transport_server --all-targets -- -D warnings`                                                                                   | T10 patch       | Windows x64                                                    | passed  | Transport consumers remain warning-free with the new engine contract.                                                                                                                                                                                                                        |
| `cargo test --locked -p golden_io --no-fail-fast`                                                                                                                 | T11 I/O         | Windows x64                                                    | passed  | All 15 tests pass, including dual-bound saturation, overload ownership, disconnect, deterministic readiness races, worker-command saturation, and bounded retirement saturation/recovery with retained resource ownership.                                                                   |
| `cargo check --locked -p Chataigne2`                                                                                                                              | T11 I/O         | Windows x64                                                    | passed  | OSC, serial, Sound Card retirement, and all other reusable worker/pending-channel consumers compile against bounded admission.                                                                                                                                                               |
| `cargo clippy --locked -p golden_io --all-targets -- -D warnings`                                                                                                 | T11 I/O         | Windows x64                                                    | passed  | Reusable bounded pending and worker primitives plus all test targets are warning-free.                                                                                                                                                                                                       |
| `cargo test --locked -p Chataigne2 osc_runtime --no-fail-fast`                                                                                                    | T11 I/O         | Windows x64                                                    | passed  | All 4 focused OSC runtime tests pass, including one-packet readiness, waker-driven output, and platform receive-error handling.                                                                                                                                                              |
| `cargo test --locked -p golden_runtime --no-fail-fast`                                                                                                            | T11 ctrl        | Windows x64                                                    | passed  | All 9 unit tests pass; the explicit scale qualification remains ignored. Saturation recovery, shutdown under saturation, and in-flight/latest-only compilation are deterministic.                                                                                                            |
| `cargo test --locked -p golden_engine --no-fail-fast`                                                                                                             | T11 ctrl        | Windows x64                                                    | passed  | 413 unit tests pass, 2 manual measurements remain ignored, the external consumer test passes, and 7 doctests pass. Project replacement saturation is rejected before generation allocation while two blocked retirements leave actor progress available.                                     |
| `cargo clippy --locked -p golden_runtime --all-targets -- -D warnings`                                                                                            | T11 ctrl        | Windows x64                                                    | passed  | Bounded actor admission, compiler replacement/cancellation, metrics, and all test targets are warning-free.                                                                                                                                                                                  |
| `cargo clippy --locked -p golden_engine --all-targets -- -D warnings`                                                                                             | T11 ctrl        | Windows x64                                                    | passed  | Runtime-center ticket retirement and cooperative compiler checkpoints are warning-free.                                                                                                                                                                                                      |
| `cargo test --locked -p golden_transport_server --no-fail-fast`                                                                                                   | T11 host        | Windows x64                                                    | passed  | All 36 transport-host tests pass, including connection saturation/recovery, HTTP request weight, outbound byte recovery and transactional rejection, hub overload semantics, intent-batch weight, and subscription policy.                                                                   |
| `cargo clippy --locked -p golden_transport_server --all-targets -- -D warnings`                                                                                   | T11 host        | Windows x64                                                    | passed  | Bounded connection, request, hub, subscription, and outbound admission plus all transport test targets are warning-free.                                                                                                                                                                     |
| `cargo check --locked -p Chataigne2`                                                                                                                              | T11 host        | Windows x64                                                    | passed  | The reusable bounded transport host composes through Golden Core and the thin Chataigne application shell.                                                                                                                                                                                   |
| `cargo fmt --all` (root and Golden Core workspaces)                                                                                                               | T11 host        | Windows x64                                                    | passed  | Rust sources are formatted in both required workspaces.                                                                                                                                                                                                                                      |
| `cargo test --locked -p Chataigne2 --no-default-features --no-fail-fast --target-dir target/codex-t11`                                                            | T11 life        | Windows x64, isolated target                                   | passed  | All 515 app tests pass after bounded Sound Card/Joy-Con lifecycle retirement and delayed-wake scheduling changes. The default Windows/ASIO path is compile-covered by the strict Clippy run; the isolated full test excludes ASIO because the configured temporary SDK is incomplete.        |
| `cargo clippy --locked -p golden_io -p golden_engine -p Chataigne2 --all-targets -- -D warnings`                                                                  | T11 life        | Windows x64                                                    | passed  | Reusable retirement admission, project replacement, app-owned device lifecycle adapters, and all their test targets are warning-free.                                                                                                                                                        |
| `cargo fmt --all` (root and Golden Core workspaces)                                                                                                               | T11 life        | Windows x64                                                    | passed  | Final lifecycle sources and deterministic tests are formatted in both required workspaces.                                                                                                                                                                                                   |
| `cargo test --locked -p golden_engine ui_read_model --target-dir target/codex-t12`                                                                                | T12 read        | Windows x64, isolated target                                   | passed  | All 19 focused read-model tests pass. The revision-race regression preserves old/new parameter values, event cursors, project generation, and 255/256 shared shards across a one-node edit.                                                                                                  |
| `cargo test --locked -p golden_engine --no-fail-fast --target-dir target/codex-t12`                                                                               | T12 read        | Windows x64, isolated target                                   | passed  | 414 unit tests pass, 2 manual measurements remain ignored, the crate-external consumer passes, and 7 doctests pass.                                                                                                                                                                          |
| `cargo clippy --locked -p golden_engine --all-targets --target-dir target/codex-t12 -- -D warnings`                                                               | T12 read        | Windows x64, isolated target                                   | passed  | Copy-on-write projection storage, snapshot caching, outside-actor read-model retirement, and all engine test targets are warning-free.                                                                                                                                                       |
| `cargo test -p golden_engine compiler_parameter_capture --target-dir target/codex-t12-compiler`                                                                   | T12 compiler    | Windows x64, isolated target                                   | passed  | The focused regression proves one parameter mutation preserves 255/256 immutable compiler-capture shards.                                                                                                                                                                                    |
| `cargo test --locked -p golden_engine --no-fail-fast --target-dir target/codex-t12-compiler`                                                                      | T12 compiler    | Windows x64, isolated target                                   | passed  | 415 unit tests pass, 2 manual measurements remain ignored, the crate-external consumer passes, and 7 doctests pass. Existing kernel validation and generation/input swap coverage also passes through worker-side layout materialization.                                                    |
| `cargo clippy --locked -p golden_engine --all-targets --target-dir target/codex-t12-compiler -- -D warnings`                                                      | T12 compiler    | Windows x64, isolated target                                   | passed  | Copy-on-write parameter storage, immutable schedule capture, deferred dense layout materialization, and all engine targets are warning-free.                                                                                                                                                 |
| `cargo test -p golden_engine application::project_persistence --target-dir target/codex-t12-persistence`                                                          | T12 persistence | Windows x64, isolated target                                   | passed  | All 6 coordinated-save tests pass, including immutable save/edit race contents and opaque script-config persistence.                                                                                                                                                                         |
| `cargo test -p golden_engine measure_immutable_project_capture_at_scale --target-dir target/codex-t12-persistence -- --ignored --nocapture`                       | T12 persistence | Windows x64, isolated target                                   | passed  | At 1,002/10,002/100,002 nodes: initial publication 0/4/63 ms; actor root capture 12/1/2 µs; outside-actor sparse materialization 16/164/1,699 ms; JSON encoding 0/3/39 ms.                                                                                                                   |
| `cargo test --locked -p golden_engine --no-fail-fast --target-dir target/codex-t12-persistence`                                                                   | T12 persistence | Windows x64, isolated target                                   | passed  | 418 unit tests pass, 3 manual measurements remain ignored, the crate-external consumer passes, and 7 doctests pass.                                                                                                                                                                          |
| `cargo clippy --locked -p golden_engine --all-targets --target-dir target/codex-t12-persistence -- -D warnings`                                                   | T12 persistence | Windows x64, isolated target                                   | passed  | Fixed-shard authored projection, dirty-node capture, sparse materialization, replacement retirement, and all engine targets are warning-free.                                                                                                                                                |
| `cargo test --locked -p golden_engine ui_read_model --no-fail-fast`                                                                                               | T12 transport   | Windows x64                                                    | passed  | All 19 focused immutable read-model tests pass, including revision-race coherence and shared lazy materialization.                                                                                                                                                                           |
| `cargo test --locked -p golden_transport_server --no-fail-fast`                                                                                                   | T12 transport   | Windows x64                                                    | passed  | All 40 transport tests pass. New deterministic cases cover bounded three-client contention, fourth-request rejection, old/new revision coherence across an edit, same-version encoded-payload reuse, protocol-envelope fidelity, and separate snapshot output capacity.                      |
| `cargo clippy --locked -p golden_engine -p golden_transport_server --all-targets -- -D warnings`                                                                  | T12 transport   | Windows x64                                                    | passed  | Immutable capture cache metadata, bounded background encoding, HTTP/WebSocket integration, outbound accounting, and all test targets are warning-free.                                                                                                                                       |
| `cargo test --locked -p golden_transport_server save_resync_and_engine_ticks_progress_during_three_client_snapshot_contention -- --nocapture --test-threads=1`    | T12 qualify     | Windows x64                                                    | passed  | With the snapshot worker deliberately blocked, three simultaneous saves completed in 14 ms while 2,165 ticks continued; maximum tick time was 302 µs and the edit took 429 µs. Three resync captures retained exactly one active plus two queued jobs and preserved their old/new revisions. |
| `cargo test --locked -p golden_transport_server measure_transport_snapshot_encoding_at_scale -- --ignored --nocapture --test-threads=1`                           | T12 qualify     | Windows x64                                                    | passed  | At 1,002/10,002/100,002 nodes: three captures took 20/2/2 µs each; materialization 0/6/75 ms; JSON encoding 0/5/46 ms; total three-client completion 1/14/147 ms; encoded payload 303,787/3,057,788/30,867,789 bytes; two same-version cache hits at every size.                             |
| `cargo test --locked -p golden_transport_server --no-fail-fast`                                                                                                   | T12 qualify     | Windows x64                                                    | passed  | 41 tests pass and the one explicit scale measurement remains ignored by default.                                                                                                                                                                                                             |
| `cargo clippy --locked -p golden_transport_server --all-targets -- -D warnings`                                                                                   | T12 qualify     | Windows x64                                                    | passed  | The combined persistence/resync/tick regression and scale harness are warning-free.                                                                                                                                                                                                          |
| `npx vitest run src/lib/tests/graphStore.test.ts src/lib/tests/webSocketBatchScheduling.test.ts`                                                                  | T13 index       | Windows x64, Node 26                                           | passed  | 11 focused tests pass. A fixed 600-node insert has identical ≤20 frame counts at 1k/10k/100k base sizes, remains atomically hidden until commit, and publishes exact node/parent/parameter indexes.                                                                                          |
| `npm test`                                                                                                                                                        | T13 index       | Windows x64, Node 26                                           | passed  | All 22 UI files and 75 tests pass.                                                                                                                                                                                                                                                           |
| `npm run check`                                                                                                                                                   | T13 index       | Windows x64, Node 26                                           | passed  | Svelte check reports 0 errors and 0 warnings.                                                                                                                                                                                                                                                |
| `npm run lint`                                                                                                                                                    | T13 index       | Windows x64, Node 26                                           | passed  | The complete UI tree passes Prettier.                                                                                                                                                                                                                                                        |
| `npx vitest run src/lib/tests/graphStore.test.ts src/lib/tests/webSocketBatchScheduling.test.ts`                                                                  | T13 remove      | Windows x64, Node 26                                           | passed  | 13 focused tests pass. A fixed 600-node removal has identical ≤4 frame counts at 1k/10k/100k base sizes. A partial 1,500-node removal is discarded on reconnect, then replayed from the unchanged cursor without exposing detached state.                                                    |
| `npm test`                                                                                                                                                        | T13 remove      | Windows x64, Node 26                                           | passed  | All 22 UI files and 77 tests pass.                                                                                                                                                                                                                                                           |
| `npm run check`                                                                                                                                                   | T13 remove      | Windows x64, Node 26                                           | passed  | Svelte check reports 0 errors and 0 warnings.                                                                                                                                                                                                                                                |
| `npm run lint`                                                                                                                                                    | T13 remove      | Windows x64, Node 26                                           | passed  | The complete UI tree passes Prettier.                                                                                                                                                                                                                                                        |
| `npx vitest run src/lib/tests/stagedFrameScheduler.test.ts src/lib/tests/webSocketBatchScheduling.test.ts`                                                        | T13 time budget | Windows x64, Node 26                                           | passed  | Nine focused tests pass. The injected monotonic clock proves a three-millisecond ceiling is rechecked after every unit and the event/cursor remain atomic.                                                                                                                                   |
| `npm test`                                                                                                                                                        | T13 time budget | Windows x64, Node 26                                           | passed  | All 23 UI files and 78 tests pass.                                                                                                                                                                                                                                                           |
| `npm run check`                                                                                                                                                   | T13 time budget | Windows x64, Node 26                                           | passed  | Svelte check reports 0 errors and 0 warnings.                                                                                                                                                                                                                                                |
| `npm run lint`                                                                                                                                                    | T13 time budget | Windows x64, Node 26                                           | passed  | The complete UI tree passes Prettier.                                                                                                                                                                                                                                                        |
| `npx vitest run src/lib/tests/graphStore.test.ts`                                                                                                                 | T13 retention   | Windows x64, Node 26                                           | passed  | Six focused tests pass. Retaining 1,001 versions of a 10k-entry map preserves historical reads and bounds one-key versions to nine copied trie nodes each.                                                                                                                                   |
| `npm test`                                                                                                                                                        | T13 retention   | Windows x64, Node 26                                           | passed  | All 23 UI files and 79 tests pass.                                                                                                                                                                                                                                                           |
| `npm run check`                                                                                                                                                   | T13 retention   | Windows x64, Node 26                                           | passed  | Svelte check reports 0 errors and 0 warnings.                                                                                                                                                                                                                                                |
| `npm run lint`                                                                                                                                                    | T13 retention   | Windows x64, Node 26                                           | passed  | The complete UI tree passes Prettier.                                                                                                                                                                                                                                                        |
| `npx vitest run src/lib/tests/webSocketBatchScheduling.test.ts`                                                                                                   | T13 mixed       | Windows x64, Node 26                                           | passed  | Nine focused tests pass. A move plus two large order rewrites and scalar patches remains invisible until its exact graph and cursor publish atomically.                                                                                                                                      |
| `npm test`                                                                                                                                                        | T13 mixed       | Windows x64, Node 26                                           | passed  | All 23 UI files and 80 tests pass.                                                                                                                                                                                                                                                           |
| `npm run check`                                                                                                                                                   | T13 mixed       | Windows x64, Node 26                                           | passed  | Svelte check reports 0 errors and 0 warnings.                                                                                                                                                                                                                                                |
| `npm run lint`                                                                                                                                                    | T13 mixed       | Windows x64, Node 26                                           | passed  | The complete UI tree passes Prettier.                                                                                                                                                                                                                                                        |
| `npm run measure:graph-paint -- --url http://127.0.0.1:4173/ --report artifacts/graph-action-to-paint.browser-report.json`                                        | T13 workbench   | Windows x64, Node 26, production Vite build, headless Chromium | passed  | Twenty 600-node inserts per fixture: 10k p50/p95/p99/max 40.8/44.9/49.2/49.2 ms; 100k 40.7/40.9/41.5/41.5 ms. Both action windows recorded zero Long Tasks and zero browser errors.                                                                                                          |
| `npx vitest run src/lib/tests/graphStore.test.ts`                                                                                                                 | T13 workbench   | Windows x64, Node 26                                           | passed  | Seven focused tests pass, including warning-index refresh after snapshot and metadata projection.                                                                                                                                                                                            |
| `npm test`                                                                                                                                                        | T13 workbench   | Windows x64, Node 26                                           | passed  | Golden Audio UI: 6 files/27 tests. Chataigne UI: 23 files/81 tests.                                                                                                                                                                                                                          |
| `npm run check`                                                                                                                                                   | T13 workbench   | Windows x64, Node 26                                           | passed  | Generated checks pass and both Svelte workspaces report 0 errors and 0 warnings.                                                                                                                                                                                                             |
| `npm run lint`                                                                                                                                                    | T13 workbench   | Windows x64, Node 26                                           | passed  | The complete Chataigne UI tree passes Prettier.                                                                                                                                                                                                                                              |
| `npm run build`                                                                                                                                                   | T13 workbench   | Windows x64, Node 26, production Vite build                    | passed  | Static client and SSR bundles build successfully; the existing large-chunk advisory remains non-fatal.                                                                                                                                                                                       |
| `cargo test --locked -p golden_runtime --test work_selection -- --nocapture`                                                                                      | T14 direct      | Windows x64, Rust 1.97, optimized dev/test profile             | passed  | Direct selection returns the exact compile-ordered IDs produced by identity-only worker dispatch.                                                                                                                                                                                            |
| `cargo test --locked -p golden_runtime --test work_selection measure_direct_selection_against_identity_worker_dispatch -- --ignored --nocapture --test-threads=1` | T14 direct      | Windows x64, Rust 1.97, optimized dev/test profile             | passed  | At 100k units and 1% dirty: direct p50/p95/p99/max 86/173/180/180 µs; eight-worker identity dispatch 399/437/448/448 µs.                                                                                                                                                                     |
| `cargo test --locked -p golden_runtime --no-fail-fast`                                                                                                            | T14 direct      | Windows x64, Rust 1.97                                         | passed  | Ten active unit/integration tests pass; two explicit scale measurements remain ignored by default.                                                                                                                                                                                           |
| `cargo test --locked -p golden_engine runtime_center --no-fail-fast`                                                                                              | T14 direct      | Windows x64, Rust 1.97                                         | passed  | Three production runtime-center tests pass, including input selected-work metrics and generation rebind behavior.                                                                                                                                                                            |
| `cargo clippy --locked -p golden_runtime -p golden_engine --all-targets -- -D warnings`                                                                           | T14 direct      | Windows x64, Rust 1.97                                         | passed  | Reusable selection, retained worker execution, engine integration, and all related test targets are warning-free.                                                                                                                                                                            |
| `cargo test --locked -p golden_runtime --test work_selection measure_sparse_dense_selection_crossover -- --ignored --nocapture --test-threads=1`                  | T14 sparse      | Windows x64, Rust 1.97, optimized test profile                 | passed  | At 0%/0.1%/1%/10% dirty, sparse visits 0/100/1,000/10,000 of 100k units with p95 0/0/1/14 µs. At 50%, sparse/dense p95 is 56/55 µs. All warmed samples allocate zero times.                                                                                                                  |
| `cargo test --locked -p golden_runtime --test work_selection measure_direct_selection_against_identity_worker_dispatch -- --ignored --nocapture --test-threads=1` | T14 sparse      | Windows x64, Rust 1.97, optimized test profile                 | passed  | With the dirty-word index, 100k/1%-dirty direct p50/p95/p99/max is 1/3/4/4 µs versus identity dispatch 327/353/363/363 µs.                                                                                                                                                                   |
| `python tools/qualification/runtime_scale.py --output-dir target/qualification/runtime-scale/t14-local`                                                           | T14 sparse      | Windows x64, Rust 1.97, release, four workers                  | passed  | Source-fingerprinted 100k-lane report: dense p95 339.7/3,618.2 µs, sparse p95 9.7/32.2 µs, idle p95 0.1/0.1 µs for the 1k×100/10k×10 partitions; zero deadline misses and stable 1/2/4/8-worker digests.                                                                                     |
| `cargo test --locked -p golden_engine --lib --no-fail-fast`                                                                                                       | T14 sparse      | Windows x64, Rust 1.97                                         | passed  | 418 tests pass; three explicit measurements remain ignored. Schedule ordering, due/replay behavior, effects, production input, generation changes, and graph removals remain green.                                                                                                          |
| `cargo clippy --locked -p golden_runtime -p golden_engine --all-targets -- -D warnings`                                                                           | T14 sparse      | Windows x64, Rust 1.97                                         | passed  | Dirty-word indexing, zero-allocation selection tests, engine reservation, and all related targets are warning-free.                                                                                                                                                                          |
| `cargo test --locked -p golden_persistence --target-dir target/t15-check`                                                                                         | T15 persistence | Windows x64, Rust 1.97                                         | passed  | All 12 unit tests and one crate-external public codec test pass, including engine- and host-free round trips and unsupported-version rejection.                                                                                                                                              |
| `cargo test --locked -p golden_engine persistence --lib --target-dir target/t15-check -- --test-threads=1`                                                       | T15 persistence | Windows x64, Rust 1.97                                         | passed  | All 11 persistence/coordinator engine regressions pass through the new persistence-owned codec.                                                                                                                                                                                              |
| `cargo check --locked -p golden_engine --target-dir target/t15-check`                                                                                             | T15 persistence | Windows x64, Rust 1.97                                         | passed  | The engine application, project adapter, and public compatibility paths compile against the persistence-owned document contract.                                                                                                                                                             |
| `cargo fmt --all`; `cargo fmt --manifest-path crates/golden_core/Cargo.toml --all`; `git diff --check`                                                           | T15 persistence | Windows x64                                                    | passed  | Both required Rust workspaces are formatted; the diff has no whitespace errors.                                                                                                                                                                                                              |
| `cargo test --locked -p golden_protocol --target-dir target/t15-check`                                                                                           | T15 protocol    | Windows x64, Rust 1.97                                         | passed  | The crate-external protocol consumer serializes the canonical handshake using only the public protocol package.                                                                                                                                                                               |
| protocol/codegen `cargo tree --locked ... -e normal` forbidden-edge check                                                                                       | T15 protocol    | Windows x64, Rust 1.97                                         | passed  | Neither normal dependency tree contains `golden_engine`, `rquickjs`, `golden_host_desktop`, or `tauri`.                                                                                                                                                                                       |
| `cargo test --locked -p golden_engine --lib --target-dir target/t15-check -- --test-threads=1`                                                                   | T15 protocol    | Windows x64, Rust 1.97                                         | passed  | All 418 active engine regressions pass through protocol-owned DTOs and engine-owned application/projection adapters; three manual measurements remain ignored.                                                                                                                                |
| `cargo clippy --locked -p golden_engine -p golden_protocol -p golden_codegen_support -p golden_script_contract -p golden_model --all-targets --target-dir target/t15-check -- -D warnings` | T15 protocol | Windows x64, Rust 1.97 | passed | Foundation contracts, canonical DTOs, adapters, codegen, and every related target are warning-free. |
| `cargo run --locked -p golden_codegen_support --bin golden_codegen -- ui-protocol packages/golden-ui/generated/rust_protocol`                                   | T15 protocol    | Windows x64, Rust 1.97                                         | passed  | Rust-owned TypeScript bindings regenerate successfully from the engine-independent protocol and script declarations.                                                                                                                                                                         |
| `npm run check`                                                                                                                                                  | T15 protocol    | Windows x64, Node 26                                           | passed  | Golden Audio generated checks and both Svelte workspaces report zero errors and zero warnings.                                                                                                                                                                                               |
| `cargo check --locked -p golden_codegen_support -p Chataigne2 --target-dir target/t15-check`                                                                     | T15 protocol    | Windows x64, default app features                              | failed  | Codegen compiled; the app build reached the known incomplete local ASIO SDK and failed because `asiodrivers.h` is absent. No protocol diagnostic was emitted.                                                                                                                                |
| `cargo test --locked -p golden_script --target-dir target/t15-check`                                                                                             | T15 script      | Windows x64, Rust 1.97                                         | passed  | Three public fake-host contract tests and four safety tests pass, covering load/reload, exports, host-call budgets, oversized inputs, failed-effect discard, clean reload, and the external watchdog.                                                                                         |
| `cargo test --locked -p golden_engine --lib --target-dir target/t15-check`                                                                                       | T15 script      | Windows x64, Rust 1.97                                         | passed  | 415 active engine regressions pass through the extracted script runtime; three explicit measurements remain ignored. VM-private decoding tests now live beside the VM rather than widening its public API.                                                                                |
| `cargo test --locked -p golden_transport_server -p golden_core --target-dir target/t15-check`                                                                    | T15 hosts       | Windows x64, Rust 1.97                                         | passed  | 41 active transport regressions, the transport-only headless consumer, and the full default-host consumer pass; one manual transport measurement remains ignored.                                                                                                                          |
| `cargo clippy --locked -p golden_script -p golden_engine -p golden_transport_server -p golden_core --lib --tests --target-dir target/t15-check -- -D warnings`    | T15 boundaries  | Windows x64, Rust 1.97                                         | passed  | Reusable script runtime, engine adapters, transport-only headless host, full facade, and their test targets are warning-free.                                                                                                                                                               |
| focused `cargo tree --locked -e normal` forbidden-edge checks                                                                                                    | T15 boundaries  | Windows x64, Rust 1.97                                         | passed  | Protocol/codegen exclude engine, QuickJS, desktop, Tauri, and Chataigne; script excludes engine/desktop/Tauri/Chataigne; transport excludes desktop/Tauri/Chataigne; audio excludes Golden Core/Chataigne; the full Golden facade excludes Chataigne.                                      |
| `cargo check --locked -p Chataigne2 --no-default-features --target-dir target/t15-app-check`                                                                      | T15 app         | Windows x64, backend-neutral app                               | passed  | The complete app compiles through the public facade and engine adapters without ASIO/realtime native feature prerequisites.                                                                                                                                                                |
| isolated `golden_codegen ... ui-protocol packages/golden-ui/generated/rust_protocol`                                                                             | T15 final       | Windows x64, Rust 1.97                                         | passed  | The explicit engine-independent generator completes and leaves generated bindings fresh. An isolated target directory avoids contending with the user's running Cargo watch process.                                                                                                     |
| `npm run check`                                                                                                                                                   | T15 final       | Windows x64, Node 26                                           | passed  | Golden Audio generated freshness and both Svelte workspaces report zero errors and zero warnings after the completed boundary extraction.                                                                                                                                                  |

## Preservation and qualification inventory

Automated preservation scope comes from the plan's product contracts: panels/layouts, graph editing
and history, modules and scripting surfaces, Alchemist formulas/contexts/multiplex/state-machine
semantics, transport/reconnect, persistence, host workflows, and audio continuity/recovery. Tests
must use null/mock/synthetic endpoints unless separate real-device authorization is given.

This session can run Windows x64 checks. Linux x64/ARM64 and macOS x64/ARM64 are CI-only here;
Raspberry Pi targets and all physical audio streams, hotplug, native webview, focus, DPI, and
dialog checks are unavailable or deliberately not attempted. The no-stream backend probe inspected
installed hosts; no physical stream was opened.

## Next task

Next dependency-ready work: continue T17's documented cohesive source splits, especially the
remaining graph-canvas node layout/interactions, dashboard/curve editors, and app-owned
formula/state integration. T18 production parallel remains
deferred pending a real sparse-dirty/full-tick benefit and a generation-safe commit boundary.
T19 next needs to measure end-to-end action-to-paint and tail latency, then exercise large live
Formula/state/graph edits, UI/transport, multi-client, and recovery paths at scale. Cross-platform,
native-host, and physical-product evidence remains open.

Known blockers and independent work that can continue: patched-source macOS playback and
Linux/ARM64 compilation are unavailable locally. Hardware qualification remains explicitly open.
