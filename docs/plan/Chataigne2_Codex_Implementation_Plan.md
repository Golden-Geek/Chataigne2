# Chataigne2 implementation plan

This is the single active implementation plan. **“Implement the plan” means this file.**
Execution state lives only in [implementation status](../progress/implementation-status.md).
Architecture pages explain maintained contracts; [archived documents](../archive/2026-10-18/README.md)
preserve evidence and never supply active task, pause, branch, or delivery instructions.

Reconciled on 2026-10-18 against `49d65b0fe71f66b73d338ebe026a572a53c7dcf6` and the
user's uncommitted additions to the previous plan. This plan specifies future source work;
the documentation consolidation does not implement its tasks.

## Start or resume

Give Codex this instruction:

```text
Implement docs/plan/Chataigne2_Codex_Implementation_Plan.md.
Read AGENTS.md and the current status ledger linked from the plan. Reconcile
the checkout, diff, and existing evidence, then continue the earliest
dependency-ready unfinished task. Preserve completed work and unrelated edits.
Work in coherent, reviewable batches, commit and push each verified milestone,
maintain the ledger, and continue safe
independent local work when an external qualification is unavailable. Follow
the plan's scope, acceptance criteria, and device/remote-action limits.
```

Do not restart old T/R phases, execute archived instructions, or treat this document as
permission to overwrite unrelated work. Resume through the ledger after context changes.

## Outcome and boundaries

Chataigne2 will ultimately host lighting creation, video manipulation, projection mapping,
and audio. **This implementation goal delivers foundation and readiness**, including the
existing product, the open audit repairs, and exercised time/resource boundaries needed
by those future domains.

The goal includes current audio and DMX runtime correctness, graph/UI scalability, backend
editing intents, asset preparation, and synchronized scheduling exercised with deterministic
consumers. Full lighting fixture/cue authoring, a production video decoder/compositor,
projection calibration/output editors, new creative workspaces, and plugin hosting remain
a later product roadmap. Do not build unused framework abstractions to imitate those features.

The control engine owns authored state, transactions, validation, history, and coordination.
Audio rendering, future GPU/video execution, and lighting output have specialized runtimes;
they share explicit time, resource, and generation contracts. Browser frame rate, decoding,
network retries, or device cleanup must not determine show output timing.

Follow [AGENTS.md](../../AGENTS.md), [architecture](../../ARCHITECTURE.md), and the focused
[architecture index](../architecture/README.md). Preserve these product contracts:

- Keep the thin app shell, public Golden boundaries, generated wire protocol, and supported
  engine-only, audio-only, protocol-only, headless, and full-host consumers.
- Preserve modules, command nodes, script functions/callbacks/templates, Formula/state editors,
  contexts, dashboards, panels/layouts, previews, undo/redo, and connected-client workflows.
- Preserve deterministic triggers/effects, keyed temporal state through context/lane reorder,
  and one authoritative state-machine state. Transition-global context never silently becomes
  a multiplex lane context.
- Keep the completed [Mapping contract](../architecture/builtin-mapping.md): ordinary owned
  nodes/commands and controls, one typed value/tuple chain, backend warnings, shared execution,
  and explicit reversible compression defaulting Off. Compilation never implies compression.
- Preserve `cargo run`, `cargo run -- --dev`, `cargo xtask watch`, headless serving, bundled UI,
  packaging, ASIO/JACK support, recoverable driver absence, and device reconnection.
- Preserve password-free LAN use without mandatory accounts, passwords, or cloud services.
  Session capabilities and browser-origin checks must coexist with this product requirement.
- Internal API breaks are allowed to improve ownership. Persisted meaning must survive through
  narrow, typed migrations and fixtures when a format changes; no general compatibility layer.

### Scale, feedback, and evidence

Normal projects target **20k–50k authored nodes**. Keep 1k/10k baseline fixtures; treat 100k
authored nodes as a separate stress tier. One million nodes is unqualified. Authored nodes,
runtime values, active lanes, visible nodes, voices/channels, pixels/streams, and DMX universes
are different workload dimensions and must be reported separately.

For ordinary 20k/50k selection, parameter edits, navigation, and canvas actions, provisionally
target p95 first visible feedback within 200 ms on the recorded reference machine. Measure
frame pacing and continued interactivity separately. Bulk operations need truthful prompt
feedback and workload-specific completion ceilings fixed before qualification; a progress
indicator does not excuse a blocked engine. Inspect the existing queued/received/processing/
finishing header before adding another progress mechanism.

Retain old 10k/100k 250/500 ms bulk failures as historical failures. A revised product gate
must record its rationale, fixture, hardware, surface, and measurement boundaries before a run.
The workbench-root/two-frame harness did not mount the graph canvas. Its results cannot qualify
dragging, routing, meter animation, or native webview behavior. See
[performance contracts](../operations/performance.md).

### Execution permissions

Proceed through authorized implementation, checks, milestone commits, and pushes without
per-task approval. Work on `main`; `codex/builtin-mapping` was merged and pushed there at
`f02205af`, as requested by the user on 2026-10-19. This instruction does not authorize releases,
deployments, remote issues/PRs, or desktop automation.
Do not operate the user's keyboard, mouse, ASIO panel, or native desktop through automation.

### Milestone delivery

The standing [implementation plan delivery rules](../../AGENTS.md#implementation-plan-delivery)
apply to every implementation plan; this plan's branch-specific delivery details follow.

Treat each coherent, verified implementation batch as a delivery milestone; do not wait for
an entire P-task or the full plan to finish. Complete the relevant checks and formatting,
update the ledger with evidence and remaining scope, then commit and push the milestone to
`origin/main` before starting the next batch. Preserve unrelated changes and unfinished work
outside the milestone commit. Do not publish known failing implementation as a verified milestone.

At each milestone, refresh the ledger's whole-plan and per-task completion percentages and
acceptance checkpoints using its documented scope-based rubric. Keep estimates distinct from
verified task completion; batch counts and elapsed time are not measures of completion.

Reconcile remote changes before pushing and preserve other contributors' commits. Never force
push or rewrite published history. If a check, merge, or push fails, record the actual failure,
repair it where possible, and retain the local work; do not report delivery until the push succeeds.

The existing named **ASIO4ALL → VB-Audio loopback** authorization remains valid only when the
exact driver and underlying route can be selected and verified. Focusrite drivers are also
installed: never use system-default ASIO selection as a substitute. Stage discovery, a brief
silent loopback output open/close, continuity, and verified loopback input. Record device,
direction, channels, sample rate, buffer size, driver, and artifact. This does not authorize
physical speakers/microphone, audible output, vendor-native ASIO, or manual unplug/replug.
If exact route verification is unavailable, leave that check unavailable and continue other work.

## Reconciled baseline

The October audit found a useful foundation, not a reason to recreate completed subsystems.
Keep atomic persistence, indexed dispatch, immutable read-model capture, bounded transport
recovery, script interruption, explicit audio ownership, and completed Mapping behavior.

| Previous work                  | Reconciled disposition                                                     | New owner of remaining work                       |
| ------------------------------ | -------------------------------------------------------------------------- | ------------------------------------------------- |
| T00–T14                        | Historical repairs retained; later findings do not erase old evidence      | P01–P13 and qualification below                   |
| T01 native gates               | Windows repairs recorded; exact patched Linux/macOS evidence remains open  | P17, P18                                          |
| T03 benchmark gate             | Comparator implemented; comparable hosted reference remains open           | P16                                               |
| T15 package boundaries         | Completed and externally exercised                                         | Preserve in P12, P17                              |
| T16 ordinary audio artifacts   | Implemented and locally qualified on Windows; external gates open          | P17, P18                                          |
| T17 source/docs cleanup        | Partial; documentation consolidation completed separately                  | P12                                               |
| T18 actual computation         | Profiling and test-only worker evidence; production parallel path deferred | P13, P16                                          |
| T19 product qualification      | Open; historical load/idle/projection passes are scoped                    | P16–P18                                           |
| Mapping R00–R08 / N01–N24      | Complete at recorded corrective baseline `ae49424e`                        | Regression preservation; new findings get P tasks |
| Mapping old Phase 11 / M01–M20 | Superseded contract and historical runtime evidence                        | No active task                                    |
| July multiplex recovery        | Completed historical slice                                                 | Preserve behavior; qualify current product in P16 |

The [archive index](../archive/2026-10-18/README.md) links original F01–F16/T00–T19, Mapping
acceptance matrices, commands, source fingerprints, profiles, and delivery observations.
Neither old task completion nor a commit message substitutes for current acceptance evidence.

### Historical finding coverage

| Finding                         | Carry-forward disposition                                                                     |
| ------------------------------- | --------------------------------------------------------------------------------------------- |
| F01 Release failures            | Preserve repaired source; remaining exact-artifact/native checks in P17/P18                   |
| F02 Pending readiness           | Preserve receiver-owned drain/re-arm repair; remaining adapters in P03                        |
| F03 Script interruption/effects | Preserve watchdog/effect safety and public script boundary; regressions in P12/P16/P17        |
| F04 Project replacement         | Preserve prepare/commit/retire and generation fencing; lifecycle/recovery in P03/P16          |
| F05 Cache restoration           | Preserve recoverable-exit tests; edit/schedule changes requalify in P01/P02/P16               |
| F06 Unbounded work/lifecycle    | Preserve migrated primitives; new app/engine gaps remain open in P02/P03                      |
| F07 Topology ties               | Preserve canonical tie-breaking; generation-order defect in P02, execution equivalence in P13 |
| F08 UI index copying            | Preserve persistent projection; canvas/document/collection work in P09/P10/P11/P16            |
| F09 Identity work/full scans    | Preserve sparse selection; actual Formula work in P13/P16                                     |
| F10 Snapshot/encoding           | Preserve off-actor immutable capture; full-workbench contention in P16                        |
| F11 Concurrent saves            | Preserve coordinator/fencing; edit/save/replacement stress in P01/P16                         |
| F12 Dependency gate             | Earlier repairs retained; October Rustls failures are new open work in P06                    |
| F13 Benchmark/product proof     | Comparator retained; matching reference and complete product evidence in P16/P17/P18          |
| F14 Facades/acknowledgement     | Public boundaries retained; failure propagation/intents in P01/P02/P08/P12/P17                |
| F15 Ordinary audio/CPAL         | Preserve features and provenance; P06/P17/P18 qualify the current artifact                    |
| F16 Gitlinks/docs/source size   | Obsolete gitlinks already removed; docs consolidated; remaining decomposition in P12          |

## Task order and completion rules

P00 reconciles the current checkout once per resumed implementation session. Then choose the
earliest numbered unfinished task whose dependencies are satisfied. Independent work may proceed
when a platform/device check is unavailable. Split tasks into coherent batches; extract oversized
responsibilities as they are touched rather than mixing a broad mechanical move into a bug fix.

| Task | Owner / result                                        | Dependencies               |
| ---- | ----------------------------------------------------- | -------------------------- |
| P00  | Current baseline and evidence reconciliation          | None                       |
| P01  | Coherent edit failure and history                     | P00                        |
| P02  | Schedule generations and typed actor failures         | P00                        |
| P03  | Bounded module service and lifecycle                  | P01, P02                   |
| P04  | Ordered audio controls and validated types            | P00                        |
| P05  | Password-free transport trust boundary                | P00                        |
| P06  | Current dependency qualification                      | P00                        |
| P07  | Correct DMX stream runtime                            | P03                        |
| P08  | Backend-owned Formula editing                         | P01                        |
| P09  | Proportional graph-document editing                   | P01                        |
| P10  | Incremental canvas presentation                       | P09                        |
| P11  | Bounded and keyboard-usable workspaces                | P08, P10                   |
| P12  | Package boundaries, readability, and complete gates   | P00; maintained throughout |
| P13  | Actual Formula work selection and execution ownership | P02, P08                   |
| P14  | Production time and scheduled activation              | P03, P04, P07              |
| P15  | Asset identity and preparation                        | P01, P04                   |
| P16  | Current product scale and endurance                   | P08–P15                    |
| P17  | Launch, package, platform, and external consumers     | P05, P06, P12              |
| P18  | Named device/route qualification                      | P04, P14, P17              |

Each ledger row separates implementation, local validation, and external qualification. Record
starting SHA and dirty-tree scope, owning files, invariant, exact commands, environment/features,
results, evidence location, and remaining checks. Performance also needs raw samples, fixture
hashes, measurement boundaries, allocations/memory, and deadline misses. Do not attribute an
uncommitted patch's results to its base SHA.

### P00 — Reconcile before implementation

Record HEAD/branch/diff and the pinned toolchain; inspect changed owning source before using any
line citation. Preserve unrelated edits. Confirm which prior fixes and checks still apply.
Capture failures relevant to the next task; do not rerun every expensive qualification after
every small edit. Treat the October audit observations in the ledger as a baseline, not new runs.

**Acceptance:** the ledger identifies the exact tree, current blockers, and next ready task.

### P01 — Make failed edits coherent

**Evidence:** reproduced valid `AddNode` followed by invalid `SetParam(Folder)` leaves one added
node with `undo_len=0`. `engine/src/engine/apply.rs` drains requests and several `?` exits bypass
common history/reference/context finalization.

Preserve the low-level engine's explicit applied-prefix failure semantics: every accepted edit
must retain its history, index/dirty finalization, and precise failure result. Centralize cleanup
so all recoverable exits follow it. Compound user operations must prevalidate and commit atomically;
their rejection leaves authored graph, history, project dirty state, and publication coherent.
Never claim external device effects can be rolled back after dispatch.

**Acceptance:** valid-first/invalid-middle/invalid-last, lifecycle failure, redo preservation,
references/context caches, and the next edit/tick/save are tested. The original probe retains
undo for the applied prefix; atomic user intents leave no partial mutation.

### P02 — Unify schedule revisions and propagate actor failures

**Evidence:** `ReevaluateGraph` can resolve a new engine order without production recompilation;
`runtime_center.rs` can reuse the old generation and append newly scheduled nodes. Existing
membership checks do not establish dependency order. Public application paths also turn typed
actor overload into `.expect(...)` panics.

Use one authoritative schedule revision for dependency changes, reevaluation, compilation, and
generation installation. Preserve current valid dependency order until a matching generation is
ready; never append a prerequisite after its dependent. Reject stale results. Carry typed overload,
disconnection, and cancellation through application/transport APIs without a panic or false ack.

**Acceptance:** production-runtime tests change dependencies with identical node membership, add
prerequisites during compilation, replace projects, and saturate admission. Verify deterministic
effects, generation fencing, coherent errors, and continued progress.

### P03 — Finish bounded I/O and lifecycle ownership

**Evidence:** HTTP stop performs blocking send/join behind requests/retries from module callbacks;
UDP outbound commands and some other module channels remain unbounded; variable-size pending
payloads use unit weight; HTTP body collection lacks a size cap. External engine edits drain until
empty and lifecycle materialization can stabilize without a pass budget. Signal/metronome command
handling resets elapsed-time baselines and can discard elapsed time.

Audit every app module against the public `golden_io` admission/retirement primitives. Put parsing,
timestamping, endpoint setup, retries, blocking native work, and cleanup at the worker/runtime
boundary. Bound items, retained bytes, service turns, response sizes, lifecycle expansion, and
cancellation. Preserve ownership when retirement is full. Distinguish overload from disconnection;
apply reconnect/backoff without producer-driven thread growth. Coalesce latest values only;
ordered triggers/commands retain multiplicity or receive an explicit admission rejection.
Account for elapsed generator time across ordinary commands; only explicit reset changes phase.

**Acceptance:** controlled slow HTTP/retry, full queues, large responses, continuous producers,
worker failure, repeated reconnect/reconfigure, lifecycle expansion, and generator commands cannot
monopolize the actor, grow memory without bounds, strand work, or lose accepted occurrences.

### P04 — Order audio control and retain type invariants

**Evidence:** gain commands and plan exchanges have independent ordering. A later configuration
can be overwritten by an older gain; a removed channel can trigger UUID lookup/error allocation
inside `RealtimeScope`. Transparent deserialization bypasses validated sample-rate/frame constructors.

Use a production sequence/generation barrier shared by plan and control delivery. Pre-resolve
control targets to dense locations for the appropriate generation; stale controls have explicit
outcomes. Perform no formatted error creation, logging, allocation/final destruction, or linear
UUID resolution on realtime paths. Use checked deserialization at validated-type boundaries,
including gain invariants, rather than patching division sites.

**Acceptance:** deterministic gain/configuration/remove/re-add interleavings preserve command
order and generation identity; allocation/deallocation guards cover plan/control transitions as
well as warmed rendering. Invalid serialized rates, frame counts, and gains fail before construction.

### P05 — Establish transport trust while keeping LAN convenient

**Evidence:** privileged HTTP/WS project/script operations have no session/origin admission and
return wildcard CORS. Default loopback narrows network exposure but does not establish caller trust.
The audit did not demonstrate arbitrary-code execution or a browser exploit.

Introduce host-owned session capabilities and explicit browser-origin/Host checks on HTTP and
WebSocket admission. Same-origin served desktop/LAN UI establishes its session without passwords;
the configured development origin remains supported. Bind cross-origin mutation to an admitted
session rather than accepting arbitrary origins. Preserve existing password-free LAN functionality
and document its trusted-network boundary; do not imply protection from a hostile peer already
authorized by that policy. Revoke sessions on host restart, bound their resources, and scope
project/script/path operations as privileged capabilities. Reuse established HTTP/WS primitives.
Review the currently null Tauri CSP against the supported bundled/dev assets and transport;
preserve browser-safe operation and explicit native command permissions.

**Acceptance:** served desktop, configured dev frontend, LAN browser, and headless clients work;
unapproved origins, mismatched hosts, missing/stale capabilities, and unauthorized privileged
requests fail before mutation or file access. Test HTTP and WS independently.

### P06 — Restore the dependency gate narrowly

**Evidence:** the October audit's advisory gate failed on locked Rustls 0.23.42 with
RUSTSEC-2026-0285/0346/0348/0349; their then-current repair floor was 0.23.46. Default application
features did not enable the Brotli feature required by 0348. No Chataigne exploit was established.

Refresh authoritative advisory data and dependency paths, apply the narrow supported repair, and
review lockfile churn. Do not suppress new advisories or upgrade unrelated major APIs for convenience.
Retain licensing/provenance and the reviewed CPAL patch/removal condition.

**Acceptance:** current locked dependency checks pass; feature reachability and advisory DB
revision/date are recorded; affected network/audio ownership tests remain green.

### P07 — Make DMX a dependable streaming boundary

**Evidence:** current DMX transport sends only on commands, reads one datagram before a 5 ms
park, and Art-Net does not filter the configured universe. sACN source-loss and termination
semantics require more than single-packet round trips.

Own last-frame buffers, periodic refresh, universe filtering, protocol sequencing, synchronization,
source identity/priority, stream termination, and bounded receive/send service in the lighting
worker. Use established Art-Net/sACN crates and verify their versioned behavior. Preserve explicit
send/blackout commands, script functions/callbacks/templates, diagnostics, and recovery. Batch
channel changes per universe; do not persist/reparse 512-channel JSON on every internal sample.
Fixture/cue authoring remains deferred.

**Acceptance:** fake-time/loopback tests cover static-look refresh beyond receiver loss timeout,
wrong-universe traffic, bursts, sequence/source/priority handling, synchronized frames, blackout,
shutdown/termination, overload, and reconnect without delaying engine ticks.

### P08 — Move Formula mutation semantics to backend intents

**Evidence:** Alchemist paste polls for the first unseen child for 750 ms and restores data in
later edits; UI also owns label allocation, identity remapping, and connection replacement policy.

Provide typed app-owned paste/connect operations through public Golden extension contracts.
Paste accepts the complete fragment and placement hints, validates once, restores/remaps identities
in one detached batch, and returns the exact source-to-created identity map. Connect validates
compatibility and replacement policy in the backend. UI, script, transport, and headless callers
share the implementation. Remove polling, inferred identities, UI label policy, and repair writes.
Update Rust DTO/codegen/generated TS and consumers together.

**Acceptance:** concurrent creates, slow projection, invalid fragments/connections, external
references, compressed content, duplicate identities, undo/redo, and project replacement produce
exact results or atomic rejection. No timing-based identity detection remains.

### P09 — Make graph-document edits proportional

**Evidence:** ordered-map `shift_remove`/reinsertion occurs during position updates; repeated
node/edge removals shift storage and scan groups. Source-derived costs can be linear per update
and quadratic across a large deletion.

Separate stable authored order from hot identity/presentation storage where needed. Update
existing presentation entries in place; batch removals, adjacency updates, group membership,
history, and revision publication. Preserve generic Golden Graph ownership and transaction rollback.

**Acceptance:** stable ordering, rollback, undo/redo, groups, incident edges, and multi-selection
drag/delete remain correct. Operation counters or representative measurements establish proportional
work for a fixed delta at 1k/10k/50k/100k document sizes.

### P10 — Keep canvas interaction incremental

**Evidence:** one dragged node rebuilds all effective nodes, ID map, spatial index, and routing
geometry/cache. Activity can advance topology/payload/presentation together. Local projection-only
samples at 100k were 26–51 ms; these exclude DOM/routing/paint and are not an FPS qualification.
Edges crossing the viewport disappear when both endpoints are offscreen.

Use independent topology, geometry, payload, activity, and preview revisions; retained spatial
indexes; transient drag overlays; and incident/obstacle-scoped route invalidation. Cull edges by
their path/bounds as well as endpoint visibility. Bound route work and preserve interactive
previews. Retain DOM/SVG until measurements justify another renderer.

**Acceptance:** mounted-canvas drag, pan/zoom, frame selection, live activity, crossing wires,
loops, and changing obstacles pass correctness tests and action/frame measurements at normal and
stress tiers. A workbench node-count attribute or two animation frames is insufficient evidence.

### P11 — Bound workspaces and complete keyboard interaction

Window flattened manager/catalog lists and historical logs; make free-layout dashboard rendering
viewport-aware. Retain stable focus/selection across virtualized changes. Provide keyboard tree
expansion/navigation, graph movement and connection creation, following the existing keyboard
audio routing pattern. Measure discovery/selection/editing workflows across composed panels.

**Acceptance:** large collections mount bounded views, filtering/scrolling retains identity and
focus, keyboard-only core operations work, and UI feedback remains usable during large batches.
Native usability remains a distinct qualification from Svelte warnings or browser tests.

### P12 — Complete package boundaries, readability, and gates

Move Stream Deck editor/product registration into app UI using the existing module-editor registry.
Split oversized canvas, dashboard, curve, state-manager, transport, macro, and test responsibilities
cohesively; use the [maintained inventory](../reference/source-size-inventory.md). Cover every UI
workspace in test/format/type gates and make generator failures visible. Preserve normalized
frontend view models while replacing unchecked wire casts/fallbacks with exhaustive conversion
and meaningful decoder tests. Do not hand-maintain a second wire protocol.

**Acceptance:** app policy stays outside Golden, public consumers compile, generator freshness
is enforced, all UI package suites execute from root, and touched runtime/tests meet the repository
layout/size rules. Remaining oversized files need a completed split or a specific defensible
documented exception; a generic exception list is not closure.

### P13 — Optimize actual Formula work and resolve execution ownership

Profile real Formula/Mapping/state-machine workloads separately from ID scheduling. Evaluated
formulas still scan topology; retained materialization and sparse subgraph work need explicit
invalidation. Inspect the exported state-machine arbiter's unused production path and incomplete
Blend/LastWriterWins/duration semantics: retain only an honest shared contract used by the real
dispatch path, with typed time units. Do not wire a second competing arbiter merely to use dead code.

The current compiled production generation primarily orders mutable node callbacks; named kernels
and zero-width layouts do not establish a unified compiled media/DSP graph. Script interruption
budgets (including the current 50 ms callback allowance) are safety limits, not show deadlines.
Keep scripts and ordinary control formulas outside hard realtime audio/GPU execution.

Apply serial proportional-work improvements first. Introduce real-kernel workers only when the
same production workload shows a full-tick benefit, including capture, scheduling, commit,
cancellation, state migration, and effects. Compile immutable plans; keep instance/lane state owned
and keyed. A rejected/stale generation must not partially publish state or effects.

**Acceptance:** sparse/dense/unchanged requested evaluation, distinct lane state and reorder,
conflicting outputs, transitions, cancellation, and generation replacement agree deterministically
across supported worker counts. Record a measured decision to retain serial execution if parallel
benefit is insufficient; the task does not mandate parallelism.

### P14 — Exercise a production time boundary

Shared foundation delivered by MapGyver M03: [Golden Time](../../crates/golden_time/README.md)
now supplies typed coordinates, rational mappings, authority/discontinuity epochs and activation
observations, exercised by the [bounded video scheduler](../progress/mapgyver-m03-render-foundations.md).
Reuse this public boundary. P14 remains open for the audio/DMX preparation, authority handoff,
latency and synchronization acceptance below; the null video path does not pass that scope.

Define typed monotonic time, engine sample position, show position, and discontinuity epoch with
explicit conversions. App-owned transport supplies show play/pause/seek/loop/rate policy; reusable
audio owns sample/render scheduling. Distinguish rendered from presented output and propagate
latency/clock observations. Use one integrated authority/handoff path rather than unused helpers.

Extend existing playback with prepare/arm/scheduled activation at a target engine frame; source
offset remains a separate value. Ready sources start together; unready/late admission reports a
typed outcome. Preserve standalone starvation behavior as an explicit policy and add show-locked
timestamp/catch-up behavior. Express audio buffering as validated latency profiles with observable
capacity/target and measured latency; selecting ASIO alone is not a low-latency guarantee.
DMX and a synthetic video consumer use the same epoch/time conversion contract without introducing
a production video stack or forcing control callbacks into the audio render path.

**Acceptance:** two independently prepared sources and a lighting event align to requested time
under a fake clock; decode stall, seek, loop, output loss, authority replacement, drift, late work,
and stale results follow explicit policies. Test no duplicate advancement/effects, bounded queues,
and sample/epoch monotonicity. Measure native synchronization separately when available.

### P15 — Establish asset identity and preparation

Provide persistent project asset IDs and source locators separate from decoded buffers, backend
handles, and cache fingerprints. Keep authored catalog/policy in the app; generic resolution,
preparation, codec/persistence, and execution contracts stay in their reusable owners. Reuse
current audio decode/cache workers. Support project-relative sources, relinking/missing status,
bounded prepared resources, cancellation, and generation fencing. Do not hash or read media on
the control actor. Derived caches are disposable and never project truth.

Migrate existing audio source paths narrowly so project meaning and headless/script calls remain
valid. Exercise collect/relink and scheduled playback with audio plus synthetic media descriptors;
do not build an unused general-purpose media library or new media-pool UI in this goal.

**Acceptance:** project relocation, duplicate references, missing/replaced files, stale preparation,
memory limits, cancellation, sparse/full save/reload, undo, and independent public consumers retain
asset identity and never expose native handles in persistence/protocol.

### P16 — Qualify the complete current product

Extend the existing qualification/reporting tools. Record matching reference fingerprints and
reject missing, malformed, incomparable, or incomplete evidence. Retain 1k/10k and add 20k/50k
normal authored-project tiers; 100k stays separate. Include both 1k×100 and 10k×10 runtime-lane
partitions, stateful/distinct keys, and actual Formula/output work without conflating capacities.

Measure mounted workbench and canvas first feedback, interactivity, and completion through parameter
edits, structural batches, undo/redo, multi-client resync, concurrent saves, script failure, queue
overload, project replacement, and persisted reload. Include preparation, publication, and cleanup
costs rather than reporting idle ticks alone. Preserve the documented multiplex 10 ms target on
its exact workload/host. Keep timing/allocation assertions tied to representative profiles.

**Acceptance:** reports retain p50/p95/p99/max, missed deadlines, action/frame/Long Task evidence,
CPU/retained memory, queue bytes/count/age/rejections, preview freshness, and source/fixture hashes.
Run the existing multi-client soak plus a longer case chosen for actual failure modes. Unmet
budgets remain failed/open and visible; do not bless new baselines automatically.

### P17 — Qualify supported artifacts and launch contracts

Use pinned toolchains and ordinary default features. Verify root/dev/watch/headless launches,
fresh bundled UI, generated contracts, native unsigned packages, and standalone public Golden
consumers. Preserve the supported host/architecture matrix and CPAL provenance. Separate native
webview/dialog/focus/DPI/lifecycle evidence from packaged headless/browser smoke.

**Acceptance:** available Windows checks pass on the exact artifact; Linux x64/ARM64 realtime,
macOS regression tests, supported artifact/catalog matrix, and native package checks receive
exact outcomes. Missing platforms stay explicitly unavailable/pending, never inferred from a
different platform or stale assets. Signing/public distribution is outside this goal.

### P18 — Qualify only the authorized named device route

Implement/use explicit driver and endpoint selection before opening the authorized ASIO4ALL /
VB-Audio route. Stage discovery, silent open/close, continuity, underruns, callback timing,
and verified loopback input where supported. Never fall back to the default driver/device.

**Acceptance:** exact route/artifact/settings and continuity results are recorded. Discovery,
simulated rendering, silent stream smoke, measured loopback, physical latency, native vendor
drivers, JACK, other platforms, and manual hotplug remain distinct evidence classes. Unavailable
or unauthorized physical checks remain outside local completion.

## Validation and final handoff

Use the repository's pinned commands and owning tests; test sources belong under `tests/`.
After a Rust implementation batch run `cargo fmt --all` from root and from `crates/golden_core`.
Run focused regressions first, then affected package/workspace checks appropriate to the change.
Do not repeatedly run broad checks once they pass without a new reason.

The October audit baseline commands/outcomes are in the current ledger. Useful final gates include:

```text
cargo test --locked -p golden_engine -p golden_runtime -p golden_graph -p golden_transport_server -p golden_persistence -p golden_script
cargo test --locked -p golden_audio --no-default-features --features analysis,playback
cargo test --locked -p chataigne_alchemist -p chataigne_processor -p chataigne_state_machine
python -m unittest discover -s tools/qualification/tests
cargo deny --locked check advisories
npm test
npm run check
npm run lint
npm run build
```

P12 must repair root gate coverage; until then also invoke the Golden Graph UI suite directly.
Run workspace check/strict Clippy, affected generators and freshness checks, full app tests, and
default audio feature checks through the supported ASIO setup when reaching their owning gates.
A skipped UI build is not a normal bundled-app build. See [development](../guides/development.md)
and [release readiness](../operations/release-readiness.md) for exact supported workflows.

The foundation goal is locally complete only when P01–P15 are implemented/verified (including an
evidenced serial/parallel decision), all available required P16/P17 checks pass, and the final
ledger records current limits and reproducible remaining external qualification. P18 and unavailable
native/platform checks may remain externally open with exact reasons and instructions. Report
**local foundation complete / external qualification open** when that is the actual result;
never call the application release-ready or hardware-qualified on that basis.

Before stopping a session, leave the diff reviewable, update the one ledger with evidence and the
next ready task, and report failures/limitations candidly. The deferred product roadmap is lighting
fixture/cue workflows, video decode/GPU composition, projection calibration/outputs, and dedicated
creative/operator workspaces built on the exercised foundations above.
