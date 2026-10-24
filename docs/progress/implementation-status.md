# Implementation status

This is the only current execution ledger for the
[implementation plan](../plan/Chataigne2_Codex_Implementation_Plan.md). When asked to
“implement the plan”, start there and resume from this ledger. Historical ledgers and their
pause/branch/push instructions are [archived](../archive/2026-10-18/README.md).

Last reconciled: 2026-10-24 on `main`, based on delivered `62ad19b7`, matching
`origin/main` at milestone start. **C01-C12 are locally closed: 12/12 packages (100%).
P03 is locally complete.** Spatializer closes C12; Node Control remains absent.
Whole-plan scope coverage is approximately 37%; P01-P06 are locally complete.
Hardware/platform and product-scale
qualification retain their later owners.
Initial audit source baseline: `49d65b0fe71f66b73d338ebe026a572a53c7dcf6`,
branch `codex/builtin-mapping`. At audit start the only local modification was the user's main
plan. Its complete contents, including the uncommitted clarification, survive in the
[previous plan](../archive/2026-10-18/plan/Chataigne2_Codex_Implementation_Plan.md).

## Completion overview

**P03 closure packages: 12/12 closed locally (100%); none remain.** These are fixed acceptance
packages, not commit counts. Spatializer's admitted preparation, bounded publication, source/path
fences, identity preservation and passive failure recovery are qualified under C12.

**Whole plan: approximately 37% complete.** P01-P06 are complete against their local
acceptance criteria; P07 has four of six checkpoints complete (67%); P08-P18 have
not started. P00 baseline reconciliation is maintained separately.
**6 of 18 implementation/qualification tasks are fully complete.**

Overall progress is the equal-weight average of P01-P18: `(6 * 100 + 4 * 100 / 6 + 11 * 0) / 18`,
rounded to a whole percent. This measures scope coverage, not elapsed time, remaining effort, or
product readiness. Task sizes differ substantially. P00 housekeeping and preserved pre-plan
features do not inflate this percentage. Later product, platform and device acceptance remains open.

Per-task percentages below use the task's acceptance scope: 0% means not started and 100% means
its required implementation and owning checks are complete. Partial percentages are coarse
estimates supported by the checkpoints below; unverified work receives no completion credit.
Update the overview, task percentages and remaining checkpoints at every milestone. Batch counts
record delivery history and are never the progress denominator.

### P03 completion checkpoints

P03's **100% local scope coverage** covers seven equally weighted workstreams. All seven meet
their owning acceptance criteria. The app-module audit covers C02-C12, **11 of 11 closed**;
Sound Card C01 is the separate workstream below. This does not close external platform, device,
whole-product timing or interaction qualification.

| Workstream                                    | Scope score  | Delivered / remaining acceptance                                                                                                                                                             |
| --------------------------------------------- | ------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Shared IO and engine service/lifecycle bounds | 100%         | Bounded primitives, external-edit service, lifecycle expansion, completion and failure ownership delivered.                                                                                  |
| Six native stream adapters                    | 100%         | UDP, TCP Client/Server, WebSocket Client/Server and Serial ownership/admission/passive service delivered; preserve their regression suites.                                                  |
| HTTP                                          | 100%         | Native lifecycle, admission, response bounds, passive service, worker request conversion and retained input disposal verified.                                                               |
| Generic script/event/property ingress         | 100%         | Method/property/event/log admission, rejected-input disposal, routing lifetime and shared-budget recovery verified.                                                                          |
| Sound Card lifecycle/service                  | 100% locally | C01 closed: command/route/derived/feedback/source-lifetime gates preserved; queued projection path and publication-lifetime guards verified; capped native constraint publication qualified. |
| App module/channel audit                     | 100% locally | C02-C12 are locally closed; Spatializer completes admitted geometry preparation, bounded graph publication, identity preservation and passive recovery. |
| Generator elapsed-time accounting             | 100%         | Ordinary commands settle elapsed time; phase, due-prefix ordering, bounded catch-up, targeted reset and stop cancellation verified.                                                          |

Shared service/lifecycle, stream/HTTP ingress, scripting, Sound Card, app-module ownership and
generator time acceptance are locally verified. Hardware/platform and whole-product qualification
keep their later plan owners.

### P03 closure packages

The user requested a remaining-batch forecast on 2026-10-20. The audit fixed **12 closure packages**
covering **17 implemented module families plus Sound Card**. All twelve are now locally closed.
Supporting commits did not change that denominator. The initial directory inventory counted an empty,
untracked Node Control placeholder as a module; there is no implementation to qualify or build there.

At the initial audit, nine families contained explicit unbounded channels: Buttplug, Joy-Con,
Keyboard, Mouse, Ultraleap, Signals, Metronomes, MIDI and MQTT. Their owning paths now have bounded
admission/service/lifetime and recovery evidence. Spatializer's Cartesian matrix preparation and
publication are also bounded. Absence of a search pattern alone was never acceptance evidence.

**Remaining P03 closure packages: zero open, twelve closed (C01-C12).** P04-P06 are also locally complete; the active task is P07.
General optimization and external qualification retain their explicit later plan owners.

| ID  | Status              | Scope                            | Required closure                                                                                                                                                                                                                                                                                                                                                                                                                    |
| --- | ------------------- | -------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| C01 | **CLOSED (100%)**   | Sound Card                       | Locally verified: bounded native/actor service and lifetime ownership; retained source, path and publication guards; matching application/rejection, isolated failure recovery and capped constraint publication. Hardware/platform and P04 audio ordering remain separate.                                                                                                                                                         |
| C02 | **CLOSED (100%)**   | Signals and Metronomes           | Verified: bounded command/configuration bytes, 64-row publication turns, elapsed-time/occurrence preservation, nonblocking retirement and passive failure/recovery.                                                                                                                                                                                                                                                                 |
| C03 | **CLOSED (100%)**   | MIDI and OSC                     | Locally verified: bounded native/input/output/catalog/feedback/structure ownership, passive service, explicit deadlines, retained occurrences, nonblocking cleanup and failure/recovery. Commands and scripts preserved; hardware/platform qualification remains separate.                                                                                                                                                          |
| C04 | **CLOSED (100%)**   | MQTT                             | Locally verified: bounded native/configuration/input/output ownership, passive receipts, item/byte pressure, source/session churn, stopped-worker and blocked-cleanup recovery, preserved command/script/callback contracts and controlled actor service. Hardware/platform and product-scale qualification remain separate.                                                                                                        |
| C05 | **CLOSED (100%)**   | DMX                              | Locally verified: native setup/cleanup and fixed lifetime slots; captured command/script/property preparation; bounded queues/payloads and actor turns; retained source/session fences, send/cancel outcomes, full mailbox/payload pressure, continuous input, eight reconfigurations and failure recovery before blocked cleanup. Stream correctness remains P07.                                                                  |
| C06 | **CLOSED (100%)**   | App Control and OS               | Locally verified: 50 App Control and 23 OS owning passes; canonical command/script ordering, source retirement, 64-source and byte pressure, bounded catalog/report/callback publication, queued lifetime guards, disabled-backlog completion, repeated reconfiguration and failure/recovery. Native actions use controlled providers; hardware/platform and product-scale qualification remain separate.                           |
| C07 | **CLOSED (100%)**   | Buttplug                         | Locally verified: 40 owning passes cover canonical command/script capture, native-prepared callbacks/catalogs, retained source/publication/domain quotas and queued guards, continuous input, mailbox/callback pressure, eight reconfigurations and failure/recovery before blocked cleanup. Protocol library, reconnect and selected-device identity preserved. Hardware/platform and product-scale qualification remain separate. |
| C08 | **CLOSED (100%)**   | Joy-Con                          | Locally verified: 71 owning passes cover canonical capture, native-prepared callbacks and atomic quota recovery, retained source/input/callback lifetimes, queued target/path/settings guards, 512 ordered edges, eight reconfigurations and failure/recovery before blocked cleanup. Hardware/platform and product-scale qualification remain separate.                                                                            |
| C09 | **CLOSED (100%)**   | Keyboard and Mouse               | Locally verified: 71 Keyboard, 87 Mouse and three shared ordering cases cover canonical capture, worker-prepared callbacks/catalogs, retained quotas, queued source/target/settings guards, passive service, continuous input under output pressure, reconfiguration and failure/recovery. Physical/platform and product-scale qualification remain separate.                                                                       |
| C10 | **CLOSED (100%)**   | Ultraleap                        | Locally verified: 24 owning passes cover bounded SDK/actor lifetimes and publication, prepared tracking and ordered status boundaries, retained source quotas, queued path/retirement/disable guards, 512 ordered statuses, eight enable/disable cycles, idle service and failure recovery before blocked cleanup. Physical SDK/platform and product-scale qualification remain separate.                                           |
| C11 | **CLOSED (100%)** | Gamepad, Kinect2 and Stream Deck | Locally verified: 34 Gamepad, 36 Kinect2 and 37 default / 43 hardware-feature Stream Deck owning passes. Bounded native ownership, prepared input/catalogs, latest feedback, retained source/target guards, nonblocking retirement, recovery and passive service are qualified. Stream Deck page preparation preserves identities and values across bounded turns. Physical/platform and product-scale qualification remain separate. |
| C12 | **CLOSED (100%)** | Spatializer; Node Control absent | Locally verified: 29 owning cases cover admitted worker/retained lifetimes, bounded endpoint/matrix/tree publication, stale source/destination fences, applied-prefix recovery, stable IDs/UUIDs/values, pressure, blocked cancellation, failure/recovery and zero idle polling. |

The [initial ownership/acceptance audit](p03-closure-audit.md) is now complete across C01-C12.
It records confirmed source gaps, existing safeguards, missing qualification and finish tests.
C01-C12 are locally closed. Existing shared IO, stream, HTTP, Mapping and generator time suites
remain preservation gates. No generic snapshot-copy prerequisite chain was added. Snapshot/DTO
scaling stays P16 unless a concrete owning resource, service, retirement or recovery violation
establishes another repair. The discarded borrowed-parameter-view candidate receives no credit.

### P04 completion checkpoints

P04 has four equally weighted acceptance checkpoints. These measure verified scope, not effort
or a promised commit count. **Four of four are complete locally (100%).**

| Checkpoint | Status | Acceptance |
| --- | --- | --- |
| Validated scalar decoding | Complete locally | SampleRate, FrameCount and GainDb reject invalid input during deserialization; valid primitive wire values and nested DTOs are preserved. |
| Ordered command admission | Complete locally | Concurrent producers and shutdown share accepted queue/sequence order; overload, closure, disconnection and exhaustion retain explicit outcomes. |
| Production plan/control barrier | Complete locally | Managed rendering shares plan/gain sequence and generation ownership; channel UUIDs and linear gains resolve on the control worker. Plan markers and gains use one bounded ordered queue. |
| Realtime transition qualification | Complete locally | Ten owning cases cover configuration/gain/remove/re-add ordering, admission/retirement pressure, sidecar acknowledgements and stale outcomes. Allocation/deallocation guards cover the production control transition and rendering path. |

### P05 completion checkpoints

P05 has four equally weighted acceptance checkpoints, fixed before continuing scoped sessions.
These measure verified scope, not effort or a promised number of commits. **All four are complete locally (100%).**

| Checkpoint | Status | Acceptance |
| --- | --- | --- |
| HTTP/WS request admission | Complete locally | Host and Origin checks precede dispatch/upgrade; CORS is exact; framing is bounded and unambiguous; the selected frontend origin is registered explicitly. |
| Scoped session capabilities | Complete locally | Host-owned 128-session registry, one-hour expiry/renewal, revocation/restart fencing, generated capability DTOs and exhaustive HTTP/WS privilege admission verified. |
| Client bootstrap and recovery | Complete locally | Shared credentials, coalesced startup/reconnect, stale-token renewal, observation-only binding and selected LAN endpoint ownership verified. Rejected/uncertain mutations retain explicit outcomes. |
| CSP and local surface qualification | Complete locally | Exact bundled HTML hashes and dev nonces work over loopback, the local LAN interface and a separate configured frontend; real HTTP/WS admission, reload reuse, CSP injection denial and host restart verified. Native permission scope is unchanged; native webview/platform interaction remains P17. |

### P06 completion checkpoints

P06 has four equally weighted checkpoints derived from its dependency-repair acceptance.
**All four are locally complete (100%).**

| Checkpoint | Status | Acceptance |
| --- | --- | --- |
| Current Rust advisories and narrow repair | Complete locally | Refreshed RustSec revision, seven necessary TLS/certificate/runtime package updates, reviewed lockfile and default feature reachability; no new ignores. |
| Current npm advisories and supported framework repair | Complete locally | Targeted safe versions, required SvelteKit 3/static adapter migration, locked install and zero npm vulnerabilities; UI suites and type checks pass. |
| Dependency ownership, licenses and provenance | Complete locally | Full cargo-deny passes; duplicate graph reviewed, unused direct engine runtime dependency removed, cargo-machete passes, vendored CPAL source and removal condition preserved. |
| Runtime and artifact regression checks | Complete locally | 589 correctly selected app protocol/Buttplug cases, 606 engine cases, current audio/IO/transport suites, default bundled app, exact-binary browser/dev/LAN/native matrix and strict workspace Clippy pass. The isolated Serial recovery fixture also passes 20 repetitions and seven discovery cases. |

### P07 completion checkpoints

P07 uses six fixed acceptance workstreams, derived from the plan's explicit streaming requirements.
These are scope checkpoints, not forecasts of effort or remaining commits. **Four of six are
closed locally (67%); two remain.**

| Checkpoint | Status | Acceptance |
| --- | --- | --- |
| Bounded receive and wire admission | Complete locally | Packet-vs-empty distinction, fair burst service, universe/START-code/size filtering, malformed input isolation and valid priority bounds; 41 owning cases, default app and strict Clippy pass. |
| Continuous output and identity | Complete locally | Retained looks, 900 ms static refresh, 23 ms data pacing, sequence wrap and persistent-node CID verified in fake time and native loopback; 50 owning cases, default app and strict workspace Clippy pass. |
| Receive source policy and synchronization | Complete locally | Bounded per-CID protocol receivers, priority/HTP/PAP merge, data/sync admission, Art-Net source/sync policy, loss/recovery and bounded memberships pass 71 owning cases, script API, default app and strict workspace Clippy. |
| Backend frame publication | Complete locally | Typed frame parameters, atomic channel batches and narrow saved-value migration pass 77 DMX cases, Core suites, script API, UI checks, default app and strict workspace Clippy. Native health pressure cannot park the stream clock. |
| Termination and recovery | Open | Correct shutdown/termination destinations and timing, overload, cancellation and reconnect without command replay or engine stalls. |
| Complete local qualification | Open | Fake-time and loopback matrix for both protocols, preserved lifecycle/retention contracts, docs, build and strict gates. |

## Current handoff

- **P07 is in progress: four of six fixed checkpoints complete locally (67%).**
  Typed per-universe publication joins receive/wire admission, clocked output/identity and receive
  source/sync policy. All 77 DMX cases, script-template/API, Core suites, UI checks, default app
  build and strict workspace Clippy pass. Two checkpoints remain: termination/recovery and full
  local qualification. Continue desired-look restoration distinct from command replay, and the
  complete shutdown/overload/reconnect matrix. P03 remains closed with zero packages left.
- **P05 is locally complete: four of four fixed checkpoints (100%).** Bundled/dev/LAN browser
  operation, native HTTP/WS, missing/stale/underprivileged denial, CSP and reload session reuse
  are verified. Native webview/focus/dialog qualification remains P17; it was not automated.
  Native peers retain the trusted-listener policy; capabilities do not establish peer identity.
  Delivered as `788dd415`; P06's final artifact/runtime validation is now complete.
- **P06 is locally complete: four of four fixed checkpoints (100%).** Current locked Rust/npm
  advisories pass without new suppressions. The required SvelteKit 3 migration preserves bundled,
  development and LAN operation. The correctly selected app suite passes **589 cases**; engine
  **606 / three ignored**, Serial discovery **seven**, isolated recovery **20/20 repetitions**.
  Default app and strict Clippy pass; the new-binary surface matrix passes. This milestone
  was delivered as `0044eb3a`. P07's first receive-admission checkpoint is now locally verified.
- Golden Transport passes **74 unit cases and one public headless-host integration case**, with
  one existing manual snapshot benchmark ignored.
  Golden Desktop passes **seven cases / one subprocess helper ignored**; xtask passes **one unit
  and six integration cases**. Golden UI passes **39 cases**, the app UI **93 cases**, and the
  native Node loopback/restart check **one case**. Svelte reports zero errors/warnings; production
  UI and default app builds and strict workspace/all-target Clippy pass. The latest complete
  transport integration outcome is recorded in the milestone evidence below.
- Prior request-admission milestone: Golden Desktop passed seven unit cases, with one subprocess
  helper ignored in direct selection. Those executions predate this session-contract milestone.
- **P04 is locally complete: four of four checkpoints (100%).** Validated scalar decoding,
  ordered command admission, production plan/control barriers and realtime transitions are verified.
  Its delivered milestone is `5a7db40a`.
- Prior P04 Golden Audio evidence: **190 default-feature unit cases**, **21 public integration cases**, and
  **136 no-default-feature unit cases**. These feature runs overlap; do not add them as unique tests.
  Strict workspace/all-target Clippy passes. Primitive numeric serialization is unchanged.

- **P03 is locally complete: C01-C12 closed, 12/12 packages (100%). No P03 packages remain.**
  Node Control remains an empty placeholder outside scope.
- Prior P03 default app artifact: **29 Spatializer + 400 controllers + 1075 disjoint broader cases =
  1504 unique passes / six existing ignored / 1510 selected / 1745 listed**, with 235 outside.
  P03 Core development-profile suite: **606 passed / three ignored / 609 listed**. These artifacts
  predate the scalar-decoding milestone; this milestone's checks are listed above and below.
- The first broad run failed two multiplex timing gates; isolated cases and the full-selection
  comparison passed on the unchanged executable. Every sample is retained below. Thresholds and
  fixtures were unchanged; stable 200 Hz behavior and incremental performance remain P16 work.
- P03's default non-test production build, strict workspace/all-target Clippy, actual root/Core/app
  formatting and final source checks pass. Fifteen tested Rust files remain unchanged and below
  1000 lines. Prior optional Stream Deck feature evidence stays dated at its own milestone.
- Prior-commit CI/product qualification failures remain recorded: dependency advisories/audits
  retain P06, structural gates P12, timing/baseline work P16 and platform qualification P17.
  Cancelled macOS product work is not a pass.
- Whole-plan scope coverage is **~37%**, with **6/18 tasks fully complete**. Continue P07;
  P03 has no remaining closure packages. Keep the full P00-P18 goal active.

## Task ledger

Implementation states: `NOT_STARTED`, `IN_PROGRESS`, `IMPLEMENTED`, `VERIFIED_LOCAL`.
Check outcomes: `PENDING`, `PASSED`, `FAILED`, `UNAVAILABLE`, `SKIPPED_WITH_REASON`.
External qualification is separate from implementation. A historical pass is not a current pass.

| Task                        | Completion | Implementation | Local validation        | External qualification / evidence                                                                                                               |
| --------------------------- | ---------- | -------------- | ----------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------- |
| P00 Baseline                | 100%       | VERIFIED_LOCAL | PASSED                  | HEAD `2e1f392551ee6d4b9a5859bc3c62945e18811fe4`; clean after consolidation committed during reconciliation                                      |
| P01 Edit failures/history   | 100%       | VERIFIED_LOCAL | PASSED batches 1–8      | Applied-prefix/atomic/history rejection, callback/teardown preservation, and next edit/tick/save verified                                       |
| P02 Schedule/actor failures | 100%       | VERIFIED_LOCAL | PASSED batches 1-3      | Dependency order, generation fencing, typed errors, disconnect history, and later-save recovery verified                                        |
| P03 Bounded I/O/lifecycle | 100% | VERIFIED_LOCAL | PASSED C01-C12 | All twelve closure packages locally verified; Spatializer closes the bounded service/lifecycle audit. External qualification retains later owners. |
| P04 Audio order/types | 100% | VERIFIED_LOCAL | PASSED all four checkpoints | Checked scalar decoding, ordered command/shutdown admission, production plan/gain sequence and generation barriers, dense targets and allocation/deallocation transition guards verified. Native/platform qualification retains P17/P18. |
| P05 Transport trust | 100% | VERIFIED_LOCAL | PASSED all four checkpoints | Bundled/dev/LAN browser and native HTTP/WS operation, independent denial gates, exact CSP, reload reuse and restart fencing verified; native webview/platform interaction remains P17. |
| P06 Dependencies | 100% | VERIFIED_LOCAL | PASSED all four checkpoints | Refreshed advisory DB, narrow necessary updates, supported frontend migration, licensing/provenance and correctly selected network/audio/build/browser checks verified. Unix/native platform behavior retains P17. |
| P07 DMX streaming | 67% | IN_PROGRESS | PASSED four of six checkpoints | 77 DMX cases, script API, Core suites, UI checks, default app and strict Clippy pass for wire admission, clocked output/identity, source/sync policy and typed frame publication. Complete termination/recovery and qualification remain open. |
| P08 Formula backend intents | 0%         | NOT_STARTED    | PENDING                 | UI polling/identity and mutation policy verified; corruption not reproduced                                                                     |
| P09 Graph document          | 0%         | NOT_STARTED    | PENDING                 | Ordered-map mutation costs source-derived                                                                                                       |
| P10 Canvas                  | 0%         | NOT_STARTED    | PENDING                 | Projection diagnostic and crossing-edge reproduction; mounted performance open                                                                  |
| P11 Workspace UX            | 0%         | NOT_STARTED    | PENDING                 | Collection/keyboard gaps source-reviewed; native usability open                                                                                 |
| P12 Boundaries/gates        | 0%         | NOT_STARTED    | PENDING                 | App policy leak, omitted graph tests, oversized files, unsafe conversions; recurring incremental native-link artifact gap                       |
| P13 Formula execution       | 0%         | NOT_STARTED    | PENDING                 | Historical profiles retained; production parallelism not mandated                                                                               |
| P14 Production time         | 0%         | NOT_STARTED    | PENDING                 | Clock helper unwired, no target-frame play, starvation/latency policy gaps                                                                      |
| P15 Assets                  | 0%         | NOT_STARTED    | PENDING                 | Audio path/cache identity exists; project asset boundary unimplemented                                                                          |
| P16 Product scale           | 0%         | NOT_STARTED    | PENDING                 | Unqualified historical benchmark baseline blocks CI comparison; snapshot/DTO scaling and full-product 20k/50k/mounted canvas gates remain open. |
| P17 Platforms/artifacts     | 0%         | NOT_STARTED    | PENDING                 | Windows history retained; exact current app/native/platform checks required                                                                     |
| P18 Named devices           | 0%         | NOT_STARTED    | PENDING                 | Exact authorized ASIO4ALL/VB-Audio route must be verified first                                                                                 |

For each implementation batch append a compact row under “New implementation evidence” with
task, source/tree, changed ownership, invariant, exact commands, environment/features, outcome,
evidence path, limitations, and next task. Keep substantial raw measurements in the existing
qualification artifacts and link them. Do not grow another chronological execution plan here.

## Preserved completed baselines

| Baseline                     | Disposition and evidence boundary                                                                                  |
| ---------------------------- | ------------------------------------------------------------------------------------------------------------------ |
| September T00–T14            | Prior fixes retained. New queue/canvas/advisory findings are new scope, not deletion of old passes.                |
| T15 reusable ownership       | Completed; public consumer checks recorded. Preserve dependency boundaries.                                        |
| T16 ordinary audio artifacts | Implemented and Windows-qualified at its recorded source. Hardware/native/platform gates remain open.              |
| T17 readability              | Partial at prior stop. The old 43-file count was dated; use the refreshed inventory below.                         |
| T18 profiling                | Real kernels and test-only workers measured; actual production commit/cancellation/full-tick benefit unresolved.   |
| T19 product                  | Open. Load/idle, synthetic projection, and outliner tests do not qualify a mounted canvas or every live operation. |
| Mapping corrective R00–R08   | Complete; N01–N24 passed at `ae49424e`. No remaining corrective phase.                                             |
| Mapping historical Phase 11  | Superseded authoring contract. Its old “in progress” handoff is historical only.                                   |
| July multiplex recovery      | Completed for the July slice; retain behavior and requalify current product.                                       |

Use the [archive index](../archive/2026-10-18/README.md) for all original matrices, exact SHAs,
commands, raw evidence references, and historical remote observations. Direct feature-branch
pushes did not trigger required CI; recorded `NOT_APPLICABLE_WITH_REASON` is not “CI passed”.

## October audit observations

These observations were made during the 2026-10-18 audit of the source baseline above.
They are carried evidence from that session, **not checks rerun by this documentation edit**.

| Check                                                                                                                                      | Outcome                           | Scope / limits                                                                        |
| ------------------------------------------------------------------------------------------------------------------------------------------ | --------------------------------- | ------------------------------------------------------------------------------------- |
| `cargo test --locked -p golden_engine -p golden_runtime -p golden_graph -p golden_transport_server -p golden_persistence -p golden_script` | PASSED: 548, 7 ignored            | Includes integration/doc tests; does not establish full app/native readiness          |
| `cargo test --locked -p golden_audio --no-default-features --features analysis,playback`                                                   | PASSED: 132 unit + 21 integration | Includes 12 managed/playback cases; no native devices                                 |
| `cargo test --locked -p golden_audio --no-default-features`                                                                                | PASSED: 99 unit + 6 integration   | Subset/alternate feature evidence; do not add overlapping tests into a unique total   |
| `cargo test --locked -p chataigne_alchemist`                                                                                               | PASSED: 179                       | No current full-app or complete state-machine claim follows                           |
| `npm test`                                                                                                                                 | PASSED: 136                       | Audio 27, Golden UI 17, app 92; root omitted Golden Graph UI                          |
| `npm exec -- vitest run --root packages/golden-graph-ui`                                                                                   | PASSED: 13                        | Separately invoked; P12 adds it to root coverage                                      |
| `npm run check`                                                                                                                            | PASSED                            | Zero Svelte errors/warnings; audio generated binding check passed                     |
| `npm run build`                                                                                                                            | PASSED with chunk warning         | Largest client chunk 1,419.69 kB minified / 430.08 kB gzip                            |
| `python -m unittest discover -s tools/qualification/tests`                                                                                 | PASSED: 63                        | Qualification parser/tool tests, not product qualification                            |
| Root and Golden Core `cargo fmt --all --check`                                                                                             | PASSED                            | Read-only formatting checks                                                           |
| `cargo deny --locked check advisories`                                                                                                     | FAILED                            | Rustls 0.23.42: RUSTSEC-2026-0285, 0346, 0348, 0349; then-current fixed floor 0.23.46 |

The default application feature-tree inspection found no Rustls Brotli feature. Advisory 0348
requires that feature; no application exploit or custom-session-secret logging path was established.
Advisory sources: [0285](https://rustsec.org/advisories/RUSTSEC-2026-0285.html),
[0346](https://rustsec.org/advisories/RUSTSEC-2026-0346.html),
[0348](https://rustsec.org/advisories/RUSTSEC-2026-0348.html),
[0349](https://rustsec.org/advisories/RUSTSEC-2026-0349.html).

### Reproduced and diagnostic evidence

- **P01:** an stdin-compiled probe linked to the current validated engine artifact queued valid
  AddNode then invalid SetParam on the root Folder. Observed `before_nodes=1 after_nodes=2
undo_len=0`, `ParamEditTargetMismatch { edit_index: 1, ... }`. The executable was a disposable
  `target/audit-engine-partial-batch.exe`; create a durable owning regression during P01.
- **P10:** a direct call to the real visible-edge selector omitted a crossing wire with both
  endpoints offscreen. The later regression must cover route bounds, not only endpoint boxes.
- **P10:** a Node diagnostic using actual effective-node projection, ID map, and spatial index
  construction measured six retained iterations after two warmups: 10k 2.97–6.51 ms, 50k
  10.41–19.39 ms, 100k 26.09–50.94 ms. This excludes routing/Svelte/DOM/paint; no retained
  benchmark script or browser p95 is claimed. Reproduce under P10/P16 before using as a gate.
- **P14:** eight default 128-frame output queue blocks at 48 kHz represent 21.3 ms capacity;
  a 2,048-frame input bridge target represents 42.7 ms. These are arithmetic capacity/target
  observations, not measured command-to-audible or round-trip latency.

No physical device, hotplug, native usability, browser exploit, full native application build,
long-running current-product soak, or cross-platform qualification was performed by this audit.

## Historical measurement cautions

- Latest September workbench-root `virtual-outliner-final20` evidence was 288 ms p95 at 10k
  and 2,109 ms at 100k, with 41 mounted rows and one Long Task. Earlier better samples do
  not replace it. No mounted graph-canvas capacity follows from that harness.
- The 100k-lane unchanged-input probe requested every lane. Roughly 95–107 ms was requested
  evaluation cost, not idle-engine time; accepted effects did not replay.
- Test-only parallel speedups excluded the complete generation/commit boundary. Smaller real
  workloads spent only part of their tick in kernels; use full-tick benefit for decisions.
- Authored reload/root/Constant identity checks did not prove every generated descendant UUID
  stable. Client-observed save/edit overlap did not alone prove server capture concurrency.
- Mapping's superseded 125-second construction observation was followed by recorded repairs;
  do not reopen it by copying only an intermediate “remaining cost” paragraph.

## Documentation consolidation checks

- PASSED: 159 local links/anchors across 44 Markdown files, including the archives. Two existing
  toolchain-manifest links were corrected during cleanup.
- PASSED: one file in `docs/plan/`, one file in `docs/progress/`, 19 matching P-task headings/
  ledger rows, all 16 historical findings mapped, and an acyclic dependency graph.
- PASSED: all nine archived documents retain their text apart from archive notices and the
  T18 evidence heading. The archived user-edited plan retains the exact 61-added/3-removed
  line diff present at task start; no historical evidence was discarded.
- PASSED: refreshed source inventory, 1,324 scanned files and 46 over 1,000 lines.
- PASSED: configured Prettier check on all 19 maintained/modified Markdown files. Archived
  evidence payloads were preserved rather than reformatted; the new archive index was checked.
- PASSED: root and Golden Core `cargo fmt --all --check`; documentation whitespace/diff scope
  checked. Runtime tests were not rerun for documentation-only changes.

This completes documentation reconciliation only. No P01–P18 source fix or media foundation is
claimed implemented. The next implementation session starts at P00 and preserves this handoff.

## New implementation evidence

P00: 2026-10-18, Windows x86-64, branch `codex/builtin-mapping`, HEAD
`2e1f392551ee6d4b9a5859bc3c62945e18811fe4`. Initial status showed the documentation
consolidation; it was committed externally during reconciliation and a subsequent status was
clean. No owning runtime sources had local edits. `rustc --version`, `cargo --version`,
`node --version`, `npm --version`, and `python --version` match the canonical manifest:
Rust/Cargo 1.97.0, Node 26.5.0, npm 11.17.0, Python 3.14.6. Historical checks remain
historical; P01's early-return paths remain present in `engine/src/engine/apply.rs`.
No devices or remote mutations used. Next: P01.

P01 effort: **XHigh requested; actual effort unchanged** (actual setting unobservable).
No supported client effort control is exposed; continue locally without nested sessions.
Trigger: coupled applied-prefix history, dirty/cache finalization, and atomic intent rollback.

P01 batch 1 (uncommitted on the P00 SHA): Golden engine request errors now converge on common
history/reference/context/UI finalization. Structural insertion/move/replacement/metadata
effects remain explicit when lifecycle completion fails; placement is validated before insert
or detach. Existing prepared-subtree rollback now restores project dirty nodes, and its owning
rejection tests assert that invariant. New regressions live in
`crates/golden_core/engine/src/engine/tests/edit_failures.rs`.

- Reproduced baseline: `cargo test --locked -p golden_engine edit_failures --lib`, one pass and
  one failure (`undo_len=0`, expected 1 after the valid prefix).
- Intermediate focused check: same command, four passed (prefix/history, redo, missing-reference
  warnings/dirty nodes, and context resolution). This predates the expanded lifecycle/placement patch.
- Expanded patch: `cargo check --locked -p golden_engine --tests` passed;
  `cargo test --locked -p golden_engine --lib` passed **461**, with 3 manual benchmarks ignored.
  This includes all seven new failure regressions and the strengthened duplicate rollback tests.
  `cargo clippy --locked -p golden_engine --all-targets -- -D warnings` passed.
  Root and Golden Core `cargo fmt --all` passed. No devices opened or remote mutations performed.
- P01 remains open: compound creation currently applies initializers after live insertion;
  inspect and repair that validation/commit boundary, then qualify compound rejection, active
  sessions, lifecycle failures, next tick/save, and redo. Do not mark P01 complete from this batch.

Focused low-level design and verification complete; return to High requested for routine work.
Actual effort remains unchanged/unobservable. Compound intent rollback is a separate XHigh risk
subtask when resumed. No meaningful repair attempts failed; the baseline failure and corrected
test-construction type mismatch are not escalation-counter events.

The
[maintained source-size inventory](../reference/source-size-inventory.md) is a reference,
not another execution ledger.

P01 batch 2 (same uncommitted P00 tree, Windows x86-64): catalog creation and initializer/history
repair. **XHigh requested; actual effort unchanged/unobservable**; no supported control is exposed.
The previous goal turn made progress; no blocker audit applies.

- Ownership: `engine/persistence/duplicate.rs` prepares catalog subtrees; `ui_sync/creation.rs`
  uses the boundary for ordinary/dashboard creation. Available scalar values are validated
  detached. Context-dependent fields and live reference constraints are validated in the
  provisional live subtree before ready, publication, or history commit; missing fields reject.
- Invariants: explicit values win over init defaults; bound handles and reference caches are
  current before ready. Declared conditional children settle between initializer values.
  Declaration lookup supports hierarchy keys and full declaration-id fallback. Binding batches
  prevalidate all targets/values, and binding/wrap edits now retain undo history.
- Readability: extracted reconstruction/ready ownership to `engine/persistence/lifecycle.rs`;
  parent `mod.rs` is 927 lines. The read-only inventory now scans 1327 files, 45 over 1000;
  no other oversized rows changed. This maintains P12 without claiming P12 completion.
- Diagnostics retained: optimized unit runs returned 464 passed / 3 failed / 3 ignored,
  then 467 / 3 / 3 twice. They exposed missing reference-cache refresh on semantic no-ops,
  conditional/contextual declaration handling, and early return before declaration-id fallback.
  Each received a source-based correction. No actual effort change is claimed.
- Focused verification: `cargo test-fast --locked -p golden_engine intent_atomicity --lib -- --quiet`
  passed 9; `cargo test-fast --locked -p golden_engine dashboard --lib -- --quiet` passed 20.
  The wrap-only test also passed. These filtered suites overlap; do not sum them.
- Final optimized verification: `cargo test --locked -p golden_engine -- --quiet` passed
  **470 unit + 1 integration + 7 doctests**, with 3 manual benchmarks ignored.
  `cargo clippy --locked -p golden_engine --all-targets -- -D warnings` passed on the patch.
  A subsequently added production rejection/projection/dirty/next-save regression passed 1
  under `cargo test-fast --locked -p golden_engine rejected_creation_preserves_production --lib -- --quiet`;
  it was not included in that optimized unit count. It verifies unchanged published nodes,
  history, clean document revision, and exact saved authored JSON after rejection.
- Remaining P01 acceptance: inject lifecycle edits to already-existing metadata/structure,
  followed by failure, and verify rollback plus production publication for those lifecycle writes. The current subtree
  checkpoint captures parameters, pending/publication state, and dirty nodes; it does not by
  itself prove rollback of every existing-node lifecycle mutation. Keep P01 IN_PROGRESS until
  that audit and any resulting repair pass. No devices, commits, pushes, or remote actions used.

Focused creation design/verification complete; return to High requested for routine documentation.
Actual effort remains unchanged/unobservable. Existing-node lifecycle rollback is a separate
coupled-state risk subtask for the next batch.

Final hygiene: root and Golden Core `cargo fmt --all`, maintained Markdown Prettier check,
and `git diff --check` pass. All validation handles from this batch completed successfully;
there is no outstanding build to restart on resume.

P01 batch 3 resumed at the same HEAD and prior P01 diff. Previous turn: progress.
Current focus is lifecycle writes to existing nodes followed by failure. **XHigh requested;
actual effort unchanged/unobservable** because no supported control is exposed. Establish a
durable reproducer before extending the transaction boundary; preserve the previous passes.

Batch 3 evidence in the same uncommitted tree:

- Reproduced with `cargo test-fast --locked -p golden_engine rejected_creation_rolls_back_existing
--lib -- --quiet`: rejected ready callback left an existing root rename and new sibling.
- Added `engine/provisional.rs`: one ordered journal owns structured original-tree edits and
  outer additions. Nested callbacks reserve order before application. Existing undo/redo state
  changes only on successful commit. `persistence/transaction.rs` owns checkpoint/abort handling;
  duplication, imports, and archive restoration share the boundary. External ingress and prior
  pending requests are excluded; rollback errors preserve both typed causes.
- The first journal regression run passed 10/11: rollback was repaired, while successful redo
  repeated ready-authored writes. `HistoryRestore` now distinguishes resource/subscription
  activation from replaying authored edits. Focused `intent_atomicity` then passed 12/12,
  including existing-node removal/move/replacement and nested callback ordering.
- Retained listener-index state on abort and strengthened production projection/clean-revision/
  next-save coverage with a lifecycle failure after existing-node edits. Current optimized
  `cargo test --locked -p golden_engine -- --quiet` passed 474 unit tests, one integration test,
  and seven doctests, with three unit tests ignored. Strict
  `cargo clippy --locked -p golden_engine --all-targets -- -D warnings` passed. Both ran from
  `crates/golden_core` on Windows x86-64 with default features; no device/native qualification.
- Split `node/core/behavior.rs` into focused creation-context, inbox-dispatch, and via-target
  modules while preserving public reexports; the behavior contract is now 935 lines.
- P01 remains IN_PROGRESS: structured edits now have evidence, but opaque mutation/script
  callbacks and compound intents outside prepared-subtree scope still need acceptance coverage
  and any resulting repair. This batch does not claim complete generic rollback or release readiness.

P01 batch 4 (same uncommitted tree): blueprint registry ownership during history/rollback.
The focused coupled-invariant subtask continues the recorded XHigh request; actual effort remains
unchanged/unobservable.

- Extended the existing lifecycle rollback regression with blueprint metadata and reverse-index
  assertions. Its pre-fix run failed for `FailExistingRemove`: the original node was restored,
  but `blueprint_instance_meta` returned `None` instead of its original version/decl lookup.
- Add/remove history now carries detached blueprint metadata for every instance root in the
  subtree; replacement history swaps the live and detached instance metadata with node payloads.
  This also removes stale registry entries during undo of insertion and restores them on redo.
- Added repeated add/remove/replace undo/redo index checks. The first compile caught a test-only
  temporary slice lifetime; fixed with a stable array binding.
  `cargo test-fast --locked -p golden_engine intent_atomicity --lib -- --quiet` passed 13/13;
  `cargo clippy --locked -p golden_engine --all-targets -- -D warnings` passed.
  `cargo test-fast --locked -p golden_engine -- --quiet` passed on the final formatted source:
  475 unit tests, one integration test, seven doctests; three unit tests ignored. Commands ran
  from `crates/golden_core` on Windows x86-64 with default features using the repository's
  optimized dev profile. Batch 3's test-profile pass is separate evidence.
- Remaining P01: opaque authored callback edits and atomic compound UI boundaries outside the
  prepared-subtree checkpoint, followed by the complete task acceptance audit. No devices or
  remote actions were used.

Next P01 source entrypoints: `engine/apply_param.rs` invokes mutable node callbacks/script hooks
without capturing a history step; distinguish runtime-only changes from persisted authored state
and reproduce the latter before changing the contract. `ui_sync.rs::apply_implicit_ui_edit_session`
groups history but does not reverse a failed operation. Extending that boundary must give one
outer owner to the checkpoint and coordinate nested prepared-subtree scopes (the current journal
correctly rejects nesting); do not add a second independent rollback system.

Focused journal/blueprint verification is complete. **Return to High requested; actual effort
unchanged/unobservable** for routine documentation/formatting. The next atomicity design remains
a separate coupled-invariant escalation if needed. Both root and Golden Core `cargo fmt --all`
completed; updated docs passed Prettier and the diff passed whitespace checks.

P01 batch 5 resumed at the same HEAD and accumulated P01 diff. Previous goal turn: progress.
No unrelated changes or live Cargo jobs found. The next focused subtask joins the outer compound
UI intent to the existing journal and preserves enclosing-session history on rejection.
**XHigh requested; actual effort unchanged/unobservable** because no supported control is exposed.

Batch 5 evidence:

- `cargo test-fast --locked -p golden_engine rejected_remove_stabilization --lib -- --quiet`
  reproduced a rejected UI removal that still deleted the original child and renamed the root
  from its callback. This is distinct from prepared-subtree creation rollback.
- Moved the shared publication checkpoint to `engine/transaction.rs`. Implicit compound UI
  operations now own one journal checkpoint even inside an existing edit session; nested
  prepared subtree insertion joins that journal. Persistence retains only its typed error adapter.
  Removed the old cleanup loop that ignored candidate-removal errors; inverse failures preserve
  both typed causes. Prior queued edits/inbox events remain outside the scope and retain order.
- The new removal regression checks original UUID/graph/history/dirty/publication and the next
  tick, plus enclosing-session undo or existing redo. Added prior-work isolation checks for both
  success and rejection. `cargo check --locked -p golden_engine` passed.
  `cargo test-fast --locked -p golden_engine -- --quiet` passed 477 unit tests, one integration
  test, and seven doctests; three unit tests ignored. Strict
  `cargo clippy --locked -p golden_engine --all-targets -- -D warnings` passed. These commands
  ran from `crates/golden_core` in the existing Windows/default-feature environment. The public
  error variant's workspace consumers passed the separate root
  `cargo check --locked --workspace`. A transient Cargo lock belonged to an IDE workspace
  check; it was left intact.
- P01 remains in progress: opaque authored callbacks and remaining direct compound UI operations
  still need coverage. The single existing transaction boundary must own subsequent repairs.

Opaque callback audit note: do not use a sparse project-codec round trip as an in-place memento.
The node macro's `generated_project_encode_data` omits default-valued fields and its decoder
returns early for null data. Restoring a previously default-valued field after a mutable callback
therefore needs a complete authored-state capture or an explicitly reset-aware contract. Begin
that repair with a default-to-nondefault mutation followed by rejection; do not add a snapshot
that silently retains the changed value. Runtime-only fields remain outside authored history.

Batch 5 focused rollback/session verification is complete. **Return to High requested; actual
effort unchanged/unobservable** for routine integration and documentation. The remaining callback
and direct-intent invariant repairs require their own focused escalation; P01 remains IN_PROGRESS.

P01 batch 6 resumed the same HEAD and accumulated P01 diff; previous goal turn was progress.
No unrelated edits or live Cargo jobs found. Focus: complete authored snapshots for mutable
Rust/script callback edits, including a default-valued field, nested queued edits, rollback,
and undo/redo. **XHigh requested; actual effort unchanged/unobservable**; no supported effort
control is exposed. The sparse-codec limitation above remains a required regression.

Batch 6 implementation/evidence (same uncommitted Windows/default-feature tree):

- Added complete authored-data capture/restore hooks to the public Node contract and enum/via
  forwarding. The shared node-field generator now emits sparse project data and complete edit
  data from one declaration list. Extracted that generator to `support/macros/src/node_data.rs`;
  the oversized macro entrypoint shrinks. Automatic `impl_node` now exposes the same declared
  persisted data contract. Parameter capture reuses its typed record; script restore retains
  runtime resources and requests a deferred reload only for a data change.
- `engine/apply_opaque.rs` records Rust mutation, script property, and script method self-state
  before nested requests. Metadata and user role are included; transient runtime state is not.
  Identity/topology/cache bindings stay engine-owned. Publication refreshes parameter caches,
  value/constraint events, node projections, and the shared reference/context finalization.
- Unified `provisional.rs` into `edit_journal.rs` with explicit AppliedPrefix/Atomic modes.
  Ordinary history-enabled drains and nested lifecycle edits share one ordered collector;
  ordinary errors retain the applied prefix, while atomic errors reverse it. Prefix mode avoids
  the atomic scope's whole-tree membership capture. Session boundaries flush accepted steps
  before changing ownership; callbacks cannot replace their enclosing session.
- The initial probe fixture omitted `persist`, so that failure was not valid authored-state
  evidence; corrected it and an integer-width fixture error. A subsequent full run passed
  481 tests but caught another test assumption: queued callbacks are not synchronous ordinary
  drains. The corrected regression exercises actual recursive ready/stabilization application.
  The next full engine run passed 484 unit tests, one integration test, seven doctests, with
  three ignored; strict engine Clippy passed. This includes Rust/property/method defaults,
  atomic rejection, rejected callback prefix undo, nested lifecycle ordering, complete parameter
  settings/cache restoration, composed state/runtime preservation, and binding ownership.
- Final automatic-node coverage and generator hygiene/reload refinements compile with
  `cargo check --locked -p golden_engine --tests`.
  `cargo test-fast --locked -p golden_engine -p golden_core_macros -- --quiet` passed 485 engine
  unit tests, the engine public integration test, the macro generated-consumer integration
  test, and seven doctests; three unit tests ignored. Strict
  `cargo clippy --locked -p golden_engine -p golden_core_macros --all-targets -- -D warnings`
  passed. Root `cargo check --locked --workspace` passed, including transport, desktop,
  Alchemist, state-machine, and Chataigne app consumers of the Node contract.
  These new changes do not alter wire DTOs or project schema. No device or remote actions used.
- P01 remains IN_PROGRESS: direct UI intent atomicity and the full task acceptance audit are
  still required. Do not count this batch as completion of P01 or the foundation goal.

Batch 6 focused callback/journal verification is complete. **Return to High requested; actual
effort unchanged/unobservable** for routine integration and documentation. Later direct-intent
rollback repairs remain a separate coupled-invariant subtask.

P01 batch 7 resumed at the same HEAD and accumulated P01 diff; previous goal turn was progress.
No unrelated edits or live Cargo jobs found. Focus: direct UI edit admission and stabilization,
control-state/child-tree history ownership, production rejection/next-save invariants, and the
final P01 acceptance audit. **XHigh requested; actual effort unchanged/unobservable** because
no supported effort control is exposed.

Batch 7 evidence and implementation (same Windows/default-feature tree):

- Added `engine/tests/direct_intents.rs`. Its baseline command
  `cargo test-fast --locked -p golden_engine direct_intent --lib -- --quiet` failed both tests:
  parameter rejection retained a changed value/root label; control-mode redo generated a new
  expression-child UUID. The tests retain redo/enclosing-session coverage and next-tick checks.
- Direct value/constraint/control/metadata/move/curve/event edits and script configuration now
  enter the shared atomic boundary. Added the internal Rust `Edit::SetParamControlState` so
  control edits use normal ordered history; wire DTOs and generated TypeScript are unchanged.
  Control completion errors retain their scalar effect. History restores recorded control state
  while child trees are restored by their own history payloads. Smart text uses the same edit.
- Extracted control editing/materialization into `engine/controls/edit.rs`. Removed obsolete
  ad hoc control/constraint history helpers and the UI-only edit-drain loop.
- Event and effective-enabled callbacks now capture owned authored self-state in the active
  journal, rather than relying only on their queued edits. Captured self-state finalizes
  references/warnings/contexts once at journal flush. Checkpoints retain the exact existing
  context registry/cache, including aliases, on rejection.
- A full run passed 486 tests but failed two checks: the move fixture did not subscribe to its
  new ancestor route, and the production no-unrelated-snapshot guard failed. The latter also
  failed in isolation against the compiled test binary: prior startup notifications had been
  retained for the next tick. UI admission now settles prior work outside the new transaction.
  Stabilization moved to the engine transaction module, avoiding an engine-to-UI dependency. The move probe
  now uses an explicit subtree subscription; the macro correctly rejected a manual generated
  interest-depth override during fixture compilation, which was removed.
- Extended the production rejection/next-save test with an event callback that directly mutates
  a persisted field. Added repeated exact-tree control undo/redo and context-alias/cache checks.
  Current compiler checks pass; the latest full engine suite and strict engine/context Clippy
  are running. No device/native qualification, commit, push, or remote action was performed.

- The next full run passed 485 unit tests but exposed three established lower-level duplicate
  contracts: root observer notifications must remain deferred and context-free catalog copying
  must not snapshot unrelated nodes. Kept those contracts: generic checkpoint commit does not
  choose notification timing. UI admission now wraps duplication through notification delivery;
  lower-level Rust subtree methods remain deferred. A small internal error adapter lets engine
  and persistence callers share the same journal while retaining their typed original/rollback
  causes. Removed the newly obsolete fixed-error transaction wrapper rather than retaining an alias.
  Final checks on the revision after removing that wrapper:
  `cargo test-fast --locked -p golden_engine -- --quiet` passed 488 unit tests, one public
  integration test, and seven doctests; three unit tests ignored. Strict
  `cargo clippy --locked -p golden_engine -p golden_context --all-targets -- -D warnings`
  passed. Root `cargo check --locked --workspace` passed for current app/host/Alchemist/state
  consumers. Root and Golden Core `cargo fmt --all` completed; final documentation formatting
  and whitespace checks are recorded with the handoff. These are local Windows/default-feature
  checks, not device, mounted UI, scale, native runtime, or cross-platform qualification.

P01 final-audit follow-up: `history.rs::undo`/`redo` still pop their transaction before fallible
replay and use `?` before returning it to a history stack. Inspect payload ownership and partial
replay rejection before claiming history failure coherence. Also audit authored callbacks during
normal history activation/notification delivery; ready filtering alone does not prove every
replay path. These are source-confirmed next entrypoints, not new verified failure claims.

Batch 7 focused direct-intent verification is complete. **Return to High requested; actual
effort unchanged/unobservable** for documentation/integration. History replay/error ownership
requires the next focused coupled-invariant escalation. P01 remains IN_PROGRESS.

P01 batch 8 resumed at the same HEAD and accumulated P01 diff, without live jobs or unrelated
changes. **XHigh requested; actual effort unchanged/unobservable**: history payload ownership,
partial replay rollback, and authored notification replay form coupled state invariants. No
supported effort control is exposed. Reproduce the failures before repairing this engine boundary.

Batch 8 baseline: `cargo test-fast --locked -p golden_engine history_failures -- --nocapture`
reproduced four failures (0 passed): failed undo changed a surviving parameter from its runtime
value, missing-parent restoration left an orphan, ready replay repeated a persisted self write,
and replay notifications repeated authored callback/queued metadata changes. The parameter test
stopped at its undo assertion; redo was source-confirmed and awaits repaired-tree verification.

The repair adds per-step live-state/payload-transition recovery under the shared checkpoint,
retains rejected transactions on their source stack, validates restoration before taking payloads,
and preserves authored state during history callbacks while retaining runtime activation. Replay
notifications settle before the guard ends; pre-existing pending work stays outside the scope.
Disjoint history batching is retained. Replacement now has one stable identity, matching its sole
producer; obsolete dual-identity replay and its unused child-reparenting helper were removed.
`cargo check --locked -p golden_engine --tests` passed, and strict engine/all-target Clippy passed
after boxing the large scalar backup variant. Full package regressions are running; no pass for
the final tree is claimed yet.

Intermediate full-package run: **493 passed, one failed, three ignored**. All six new history
regressions passed, including both undo and redo failure directions, missing-parent retry,
failed batched-ready retry, runtime moves/replacements, and authored activation/notification
preservation. The remaining assertion measured all snapshots as a proxy for ready cost. Replay
now synchronously delivers its notifications, requiring one shared dispatch snapshot; the test
now separately asserts unchanged ready/lifecycle builds and exactly one dispatch build for the
whole batch. No per-node snapshot rebuilding was introduced. Root `cargo check --locked
--workspace` passed on this intermediate tree.

Final lifecycle audit also protects authored self-state during teardown when a journal retains
the payload for undo or rollback; runtime-only destruction still avoids serialization. A seventh
history regression checks successful removal/undo and atomic removal rejection through the next
tick/save. Both ready and destruction hooks continue to update their runtime-only counter.
The final tree passes `cargo check --locked -p golden_engine --tests` and strict all-target engine
Clippy. Final complete engine tests were then run as recorded below.

Final result (same HEAD plus accumulated P01 diff, Windows/default features):

- `cargo test-fast --locked -p golden_engine -- --quiet` passed **495 unit tests**, **one public
  integration test**, and **seven doctests**, with three manual benchmarks ignored. All seven
  new `engine/tests/history_failures.rs` regressions passed.
- `cargo clippy --locked -p golden_engine --all-targets -- -D warnings` and
  `cargo check --locked -p golden_engine --tests` passed on the final source tree.
- Root `cargo check --locked --workspace` passed for the engine, transport, desktop, public
  facade, Alchemist, processor, state machine, and Chataigne app consumers.
- Root and Golden Core `cargo fmt --all`, documentation Prettier checks, and whitespace checks
  passed. The refreshed source inventory contains **1341 files / 44 oversized**; none newly
  exceed the size limit. No devices opened, commits, pushes, or remote publication.

P01 acceptance audit: batches 1–8 cover invalid-first/middle/last applied prefixes, lifecycle
completion failure, atomic compound/direct rejection inside and outside a session, redo
preservation, reference/context/cache/blueprint restoration, next edit/tick/save, complete authored
callback state, control-child identity, and failed undo/redo ownership/retry. **P01 is
VERIFIED_LOCAL.** This is Windows/default-feature local evidence; mounted UI, product scale,
native runtime, devices, and other platforms remain in their later owning tasks.

Focused history design/fix verification is complete. **Return to High requested; actual effort
unchanged/unobservable** for documentation/integration. P02's coupled schedule/generation and
actor concurrency work requires its own focused escalation. Source entrypoints are
`engine/runtime/mod.rs::mark_schedule_dirty`, `runtime_center.rs::run_tick` (old compiled order
plus appended new nodes), compilation admission/completion, and application actor calls with
`.expect("production control actor disconnected")`. P02 has only been read; no P02 source changes
or test pass is claimed. Next dependency-ready task: **P02**.

P02 batch 1 resumed at the same HEAD and accumulated verified P01 diff. Previous goal turn:
**progress** (P01 completed locally). No unrelated changes or live Cargo/Rust jobs found.
**XHigh requested; actual effort unchanged/unobservable** for schedule/generation concurrency;
no supported client control is exposed. Owning boundaries are Golden engine schedule resolution
and production runtime compilation/ordering. First reproduce dependency-only reorder and newly
scheduled prerequisite failures; actor overload/error propagation remains a required P02 subtask.

P02 batch 1 evidence (same uncommitted Windows/default-feature tree):

- Baseline `cargo test-fast --locked -p golden_engine runtime_center::tests::scheduling --
--nocapture` reproduced both production failures: dependency-only reorder and activation of a
  previously passive prerequisite each executed `[a, b]` instead of `[b, a]` (0 passed, 2 failed).
- Resolved schedules now carry an engine-owned revision. Compilation snapshots, installed
  generations, and the actual scheduled-update callback share it. A mismatched compiled order
  selects the engine's resolved canonical order; the old append-new-nodes path is removed.
  Admission compares the resolved revision, replacing retained-UI-event heuristics and the
  UI intent classification list. Idle ticks add only constant-time revision checks.
- Compiler completion checks project epoch, compiler-request sequence, pending invalidation,
  and captured schedule revision before installation. Project candidates resolve before capture
  and reject a changed schedule before cutover. Invalid schedule rates are validated before
  replacing the last usable schedule/revision.
- Compilation admission/completion/install ownership moved to focused
  `runtime_center/compilation.rs`. The runtime coordinator shrinks from 954 to 808 lines;
  the application facade shrinks to 919. Seven production regressions cover pre-applied and
  tick-preamble reevaluation, automatic compilation, newly inserted prerequisites, held stale
  completions, overlapping project epochs, failed resolution/recovery, and changed candidates.
- `cargo check --locked -p golden_engine --tests` and strict
  `cargo clippy --locked -p golden_engine --all-targets -- -D warnings` passed.

Final P02 batch 1 validation:

- `cargo test-fast --locked -p golden_engine -- --quiet` passed **501 unit tests**, **one public
  integration test**, and **seven doctests**; three manual benchmarks remain ignored. All seven
  new production scheduling tests passed. The obsolete retained-event heuristic test was removed
  with that implementation; dependency-only automatic recompilation is verified through effects.
- Root `cargo check --locked --workspace` passed for current engine/host/facade/Alchemist/
  processor/state-machine/app consumers. Strict engine/all-target Clippy and test compilation
  passed before the complete package run. Root and Golden Core `cargo fmt --all`, five maintained
  docs' Prettier checks, and whitespace checks passed. Source inventory: **1344 files / 44
  oversized**; none newly oversized. No devices, commits, pushes, or remote publication.
- Evidence is Windows/default-feature local validation; no mounted UI, device, scale, native
  runtime, or cross-platform qualification is inferred. P01 regressions remain included and pass.

Focused schedule/generation verification is complete. **Return to High requested; actual effort
unchanged/unobservable**. P02 remains **IN_PROGRESS**: `golden_runtime` already exposes
`ControlError` / `ControlErrorKind::{Overloaded, Disconnected}` in
`runtime/control/src/control.rs`; application `call_engine`, transaction submission, and tick
paths still use `.expect("production control actor disconnected")`. Propagate these through
application/transport contracts and audit cancellation/completion semantics, then test saturated
admission, coherent errors, absence of false acknowledgement, and continued progress. This next
concurrency/error-boundary subtask requires its own focused escalation.

P02 batch 2 resumed at the same HEAD and verified schedule/P01 diff. Previous goal turn:
**progress**. No unrelated changes or live Cargo/Rust jobs found. **XHigh requested; actual
effort unchanged/unobservable** for actor admission/completion concurrency and application /
transport error contracts; no supported effort control is exposed. Reproduce the public
overload panic, then preserve typed rejection/cancellation without a false acknowledgement.

P02 batch 2 implementation (same uncommitted Windows/default-feature tree):

- Baseline `cargo test-fast --locked -p golden_engine application::tests::control_failures --
--nocapture` reproduced the public overload panic (0 passed, 1 failed). A full bounded actor
  queue made `apply_ui_transaction` panic on `ControlErrorKind::Overloaded`.
- Fallible construction and all actor-backed facade operations now return `ApplicationError`,
  retaining concrete actor, runtime, and graph-rejection causes. Graph rejection still owns its
  authoritative acknowledgement/history; admission failure has no fabricated acknowledgement.
  Save capture/publication, project replacement/status, queries, scripting, tick, lifecycle, and
  value APIs propagate actor errors. Consumers and existing regressions use the explicit results.
- `application/transactions.rs` owns admitted batches and completion. The public pending-batch
  API allows real WebSocket `Accepted` publication after admission, followed by an authoritative
  result or typed completion error. Runtime reply guards distinguish queued cancellation from a
  lost started-operation result; actor exit clears pending-depth metrics even after a panic.
- Protocol **0.7.0** adds shared HTTP/WebSocket actor categories. Updated the Rust DTO/codegen
  root, generated authoritative bindings through `golden_codegen ui-protocol` into a target
  artifact, and applied their diff with the standard editing tool. Golden UI exposes typed
  `ControlRequestError` values and retains HTTP status/body data. No hand-maintained TS enum.
- Split the transport coordinator (2395 lines) into focused HTTP, WebSocket, hub, delivery, and
  error modules; each is below 1000 lines. The facade is 746 lines; transaction admission is 226. New regressions exercise public saturation/continued progress, cancellation versus actor
  loss, shutdown cancellation, and real HTTP/WebSocket overload without false acceptance.
- Strict `cargo clippy --locked -p golden_engine -p golden_runtime -p golden_transport_server
--all-targets -- -D warnings`, owning test compilation, root workspace compilation, and
  `npm run check` passed. Root `npm test` passed 140 tests before the final HTTP error-helper
  extraction/test. Complete backend suites and final UI checks are pending; no final pass yet.

P02 remains IN_PROGRESS. After this batch's tests pass, audit saturated session cleanup/recovery
and save metadata publication after file commit. Current disconnect cleanup reports rejection
but does not yet retain a failed cleanup for retry. Do not equate typed propagation alone with
completion of the P02 recovery/continued-progress contract.

Final P02 batch 2 validation (same HEAD plus accumulated diff, Windows/default features):

- `cargo test-fast --locked -p golden_engine -p golden_runtime -p golden_transport_server --
--quiet` passed: **503 engine unit + 1 public integration + 7 doctests**, **10 runtime unit +
  2 work-selection integration**, and **44 transport unit + 1 public headless integration**.
  Manual ignored cases: three engine, three runtime scale/selection, and one transport. Public
  overload, queued cancellation versus started-operation loss, shutdown cancellation, real HTTP
  and WebSocket saturation, no false acceptance, and next-request recovery all passed.
- Strict owning all-target Clippy passed. Root `cargo check --locked --workspace --tests` passed,
  including updated app/system consumer tests; the ordinary workspace check also passed.
- Final root `npm test` passed **141** (audio 27, Golden UI 22, app 92). `npm run check` reported
  zero Svelte errors/warnings and passed audio generated-binding verification. `npm run build`
  passed with the existing large-client-chunk warning (1419.99 kB minified / 430.26 kB gzip).
  Canonical protocol generation comparison found **zero binding mismatches**. Golden UI changed
  files use the established app UI Prettier configuration; docs use their normal configuration.
- Source inventory: **1357 files / 43 oversized**; the transport coordinator is now 607 lines,
  with focused modules at 395/369/450/683 lines. No newly oversized files. Formatting/whitespace
  checks are finished with the handoff. Loopback qualification only; no external services,
  hardware, commits, pushes, or remote publication.

Focused actor error-contract verification is complete. **Return to High requested; actual
effort unchanged/unobservable**. Keep P02 **IN_PROGRESS** for the explicitly retained recovery
audit. Source entrypoints: `ui_server/hub.rs` disconnect/session replacement cleanup currently
logs a rejected cancellation and drops it; `application/project_persistence.rs` calls actor
metadata publication after the file commit and returns the typed error without commit-stage
context. Reproduce/repair those cases and prove later operations remain coherent before P02
closeout. Their concurrency/persistence invariants require a new focused escalation.

P02 batch 3 resumed at the same HEAD and verified P01/P02 diff. Previous turn: **progress**.
No unrelated changes or live Cargo/Rust jobs found. **XHigh requested; actual effort unchanged /
unobservable** for connection/session lifetime fencing and committed-save publication failure.
A client-name retry can cancel a replacement connection's new session; design lifetime ownership
at the application/runtime boundary before repairing transport cleanup. No supported effort
control is exposed. Save-stage errors must distinguish a committed file from failed publication.

P02 batch 3 implementation (same HEAD plus accumulated uncommitted repairs):

- Baseline `cargo test-fast --locked --no-fail-fast -p golden_engine -p
golden_transport_server control_failures -- --nocapture` reproduced two defects: a closed
  connection stranded its edit session after rejected cleanup admission, and a committed save
  returned only the overload diagnostic without its durable identity. The prior overload test's
  mutex-poison failure was a cascade from the first regression, not a third independent defect.
- `UiClientLease` belongs to the production runtime boundary. Socket completion and hub eviction
  share a close signal; socket cleanup closes it before waiting for hub admission. The actor
  fences queued requests and finalizes accepted edits into undo history on its next turn. One
  active owner is retained; no retry queue accumulates. Identity binding shares the physical
  signal, anonymous edits finalize before named work, and a second different handshake closes
  the connection. Reused browser/edit IDs cannot transfer ownership to a replacement connection.
  Project cutover clears the previous owner. Failed ticks still publish their accepted prefix
  and released history state. The redundant hub client-name index was removed; peer replacement
  scans the live table, capped at 16 clients, so eviction cannot leave stale index entries.
- `ApplicationError::SavePublication` retains the committed path, request ID, generation,
  document revision, and typed actor cause after successful file commit. Rejected metadata
  admission leaves dirty/path state intact; the next save can commit and publish normally.
- Regressions cover named/anonymous disconnect history, closed queued work, replacement IDs,
  project cutover, failing ticks, handshake identity, simultaneous actor/hub saturation, late
  cleanup, durable save identity, and later-save recovery. The transport saturation test retains
  the hub entry while the socket close signal releases the actor session.
- Strict owning Clippy passed before the final physical-lifetime refinement. Final backend
  suites, strict Clippy, and root workspace/test compilation are running; no final pass claimed.
  Inventory: **1359 source files / 43 oversized**. No devices or external endpoints opened.

Final P02 batch 3 validation (same HEAD, Windows/default-feature local tree):

- `cargo test-fast --locked --no-fail-fast -p golden_engine -p golden_runtime -p
golden_transport_server -- --quiet` passed: **509 engine unit + 1 public integration + 7
  doctests**, **10 runtime unit + 2 work-selection integration**, and **45 transport unit +
  1 public headless integration**. The three engine, three runtime, and one transport manual
  qualifications remain ignored. All six new batch-3 engine tests and the saturated disconnect
  transport test passed, including physical closure before hub cleanup.
- Strict `cargo clippy --locked -p golden_engine -p golden_runtime -p
golden_transport_server --all-targets -- -D warnings` passed. Root `cargo check --locked
--workspace --tests` passed for the app, systems, and default-host consumers. Root and Golden
  Core `cargo fmt --all`, maintained-doc Prettier checks, and whitespace checks passed.
- P02 acceptance audit: production scheduling regressions exercise identical-membership
  dependency changes, new prerequisites during compilation, stale completions, replacement
  epochs, failed resolution, deterministic effects, and continued progress. Actor/facade/
  transport regressions establish typed overload/cancellation/disconnection without false
  acknowledgements, disconnect ownership/history recovery, and post-commit save recovery.
  **P02 VERIFIED_LOCAL**. UI/protocol were unchanged in batch 3; batch-2 frontend and codegen
  evidence remains scoped to those unchanged files. No broader platform/device/scale claim.
- Final source inventory remains **1359 files / 43 oversized**; no newly oversized source.
  No commits, pushes, remote publication, devices, or external endpoints were used.

Focused concurrency/persistence recovery verification is complete. **Return to High requested;
actual effort unchanged/unobservable**. Next dependency-ready task: **P03**, not yet implemented.
Source reconnaissance confirms `apps/chataigne/src/module/modules/protocol/http/transport.rs`
blocks stop on command send/join, admits an unweighted bounded request queue, collects uncapped
responses, and uses an unbounded event channel; its module drains until empty. UDP transport
also uses an unbounded outbound channel and drain loops. Start with a controlled slow-HTTP/full-
queue regression and use public `golden_io` admission/retirement APIs. Cancellation and resource
handoff require a fresh focused XHigh request before design/repair; routine integration stays High.

P03 batch 1 resumed at the same HEAD and accumulated verified P01/P02 diff. Previous goal turn:
**progress**. No unrelated edits or live Cargo/Rust jobs found. App HTTP owns request policy;
public `golden_io` owns reusable admission/lifetime mechanics. **XHigh requested; actual effort
unchanged/unobservable** for saturated HTTP cancellation, reply backpressure, and worker cleanup
ownership. No supported effort control is exposed. Reproduce actor-blocking shutdown first.

P03 batch 1 implementation (validation in progress):

- The old HTTP regression blocked for over 60 seconds behind a stalled request and a full queue;
  its owned test process was terminated. Its fixture originally joined the blocked stopper before
  asserting the timeout; this was corrected so future failures report promptly. This is a measured
  blocked baseline, not a completed failing-assertion run.
- Public `golden_io::BoundedWorker` composes existing weighted pending channels, cancellation,
  capacity waits, and a pre-reserved worker lifetime. The worker owns cleanup and its slot until
  exit; handle drop neither joins nor creates another cleanup thread. Full replies retain the
  result and wait, oversized replies return ownership, and worker death wakes event-driven owners.
  First owning test run: **20 passed**, including five new concurrency/lifetime regressions.
- HTTP moved client construction/destruction, blocking I/O, JSON/script-response preparation,
  retries, and reply backpressure to that worker. Bounds: 16 active/retiring worker lifetimes;
  256 requests / 8 MiB retained request weight; four replies / 32 MiB event weight; 1 MiB response
  bodies including unknown-length streams; bounded JSON expansion. Module turns retain/drain at
  most four responses, borrow parsed values, and use pending/death signals plus capped recovery
  backoff. Native in-flight calls retain their slot until the configured request timeout.
- Owning app/test compilation passed after two extracted callback methods were given the correct
  parent-module visibility. Current focused app tests, final I/O tests, and owning strict Clippy
  are running. No final batch pass or complete P03 acceptance claim yet.
- P03 still requires the complete module/channel audit, UDP/service bounds, engine external edits,
  lifecycle expansion, generator elapsed time, and further cancellation/reconfiguration semantics.
  Requested shutdown cancels queued HTTP work; remote effects already issued cannot be undone.
  Repeated reconfiguration, accepted-command cancellation reporting, and per-response graph
  materialization service budgets need their own evidence.
  Only localhost TCP fixtures were opened; no external endpoints or devices were used.

Final P03 batch 1 validation (same HEAD plus accumulated diff, Windows/default features):

- `cargo test-fast --locked -p Chataigne2 http -- --quiet` passed **17 tests**, including six
  new HTTP transport/expansion regressions. Stalled/full shutdown returns promptly, retry
  cancellation releases capacity, declared and unknown-length oversized bodies reject before
  the next request succeeds, slow consumers preserve reply order/multiplicity, request bytes
  and complexity reject at admission, and repeated large JSON paths hit the retained limit.
- `cargo test-fast --locked -p golden_io -- --quiet` passed **20 tests**, including five new
  worker tests for weighted/item bounds, backpressure, cancelled queues/retries/idle waits,
  retained native-cleanup capacity, oversized replies, worker failure, and later progress.
- Root `cargo check --locked --workspace --tests` passed on the final source. Owning strict
  Clippy and formatter/doc/whitespace checks are recorded at final closeout below. Source
  inventory: **1364 files / 43 oversized**; no newly oversized source. HTTP coordinator is
  868 lines; worker/weights/callbacks and new tests are focused files below 1000 lines.
- Success and failure callback arguments are prepared on the HTTP worker and moved into script
  delivery. A compact failure reply retains the admitted request identity; diagnostics are
  capped before publication. The 32 MiB event-weight bound includes prepared failure arguments
  for the largest admitted request, avoiding an error-reporting capacity dead end.
- P03 remains **IN_PROGRESS**. This is local loopback/admission evidence, not full module,
  sustained load, native/device, or platform qualification. No UI/protocol changes, commits,
  pushes, or remote publication. The source/runtime constraints still open above remain tasks.

P03 batch 1 closeout: strict `cargo clippy --locked -p Chataigne2 -p golden_io --all-targets
-- -D warnings` passed on the final source. Both root and Golden Core `cargo fmt --all`, explicit
Rustfmt for the app's generated-registry-owned HTTP modules, maintained-doc Prettier checks, and
`git -c core.safecrlf=false diff --check` passed. **Return to High requested; actual effort
unchanged/unobservable**. The next focused concurrency/accepted-cancellation or lifecycle-bound
repair needs a fresh escalation; no further P-task is marked complete.

P03 batch 2 resumed at the same HEAD and verified accumulated P01/P02/P03 diff. Previous goal
turn: **progress**. No unrelated changes or live Cargo/Rust jobs found. App-owned HTTP owns
response materialization and completion policy; Golden owns generic snapshot lookup and worker
admission/lifetime contracts. **XHigh requested; actual effort unchanged/unobservable** for
resumable graph application, FIFO callbacks, and accepted request ownership across configuration
changes. No supported effort control is exposed. Preserve large arrays with continuations rather
than narrowing the accepted input to fit a service turn.

P03 batch 2 implementation (same HEAD plus accumulated diff):

- Baseline `cargo test-fast --locked -p Chataigne2
http_response_materialization_is_bounded_per_turn_and_preserves_large_lists -- --nocapture`
  failed its per-turn node-growth assertion (0 passed / 1 failed). A single 1024-element array
  materialized its entire graph in one module update.
- HTTP now retains one bounded FIFO of success/failure events and services its front response
  with a cursor. Each turn has 256 path/value/sibling units and a 64 KiB copy allowance; one
  larger already-bounded scalar may use a full turn. Large arrays remain accepted and resume
  without replaying prior units. Tail pruning scans bounded sibling prefixes with stable next
  markers; external marker removal restarts a bounded scan. Known missing paths still use the
  common detached-subtree planner. Survivor identities and unindexed children are retained.
- Callbacks wait for a subsequent snapshot after queued value work and cannot overtake prior
  events. Conflicting leaf/ancestor paths yield between turns, preserving repeated triggers and
  accepted mutation order. Array elements keep their no-description policy; scalar provenance
  remains available. Optional description in the common batch contract makes that policy explicit;
  the OSC consumer keeps its previous descriptions with `Some(...)`.
- HTTP worker preparation bounds value paths to 256 bytes per segment / 4096 total bytes and
  caps provenance to 4096 UTF-8-safe bytes. Service byte weights include scalar provenance;
  actor URL diagnostics use a bounded prefix. These prevent metadata/path copies from defeating
  the per-turn budget. The repeated-path expansion regression still reaches the 8 MiB bound
  using paths within those limits; a large scalar regression proves progress beyond 64 KiB.
- Generic `ProcessTreeSnapshot::find_child` now indexes exact aliases for parents wider than
  16 siblings, preserving the first sibling across all aliases. This removes repeated whole-
  sibling scans from response continuation. A public snapshot regression covers 16/17/20k
  siblings and collisions; no Chataigne policy was added to Golden.
- First service run passed 21 HTTP tests. Final app checks include the metadata/path refinements
  and external marker deletion. The full engine suite has passed 510 unit tests and its public
  integration; doctests/final diagnostics and final app validation are still being collected.
- Subsequent path/provenance/large-scalar refinements passed **23 HTTP tests**. A final source
  audit distinguished trigger-address creation from the first incoming pulse: the cursor now
  retains that pulse for a real Append edit after target creation. Existing trigger addresses
  still batch appended pulses within a turn. The regression now requires exactly two parameter
  events for two inputs; its final run, strict Clippy, and workspace checks are running.
- Inventory refreshed: **1366 source files / 43 oversized**, no new exception. The response
  service is a focused 300-line module; the HTTP coordinator remains 866 lines. Small formatter
  changes to the touched OSC consumer's tests keep that module consistently formatted.

P03 remains IN_PROGRESS. Reconfiguration still stops/drops the old HTTP handle, so accepted queued
requests can be cancelled without individual completion. Repair this ownership boundary next;
do not report reconfiguration as qualified by the service tests. Also audit queued graph-edit
failure versus cursor progress when checking engine/lifecycle failure paths. Remaining module/
channel, UDP, engine external-edit/lifecycle, generator, and broader qualification tasks remain.

Final P03 batch 2 validation (same HEAD plus accumulated diff, Windows/default features):

- `cargo test-fast --locked -p Chataigne2 http -- --quiet` passed **23 tests** on the final
  source. New service tests prove bounded 1024-value creation/shrinkage, exact two-pulse delivery
  across target creation, input-order replacements, FIFO response/failure completion, survivor
  identity, external trim-marker deletion, preserved unindexed children, and progress for a
  100k-character scalar. Worker tests also cover path/provenance limits. The prior one-tick bulk
  expectation was replaced by bounded-turn completion; large arrays were not removed or truncated.
- `cargo test-fast --locked -p golden_engine -- --quiet` passed **510 unit tests, one public
  integration test, and seven doctests**, with three existing manual qualifications ignored.
  This includes alias lookup/collisions through 20k siblings and all P01/P02 regressions.
- Strict `cargo clippy --locked -p Chataigne2 -p golden_engine --all-targets -- -D warnings`
  passed. Root `cargo check --locked --workspace --tests` passed, including the adjusted OSC
  consumer. Formatter/doc/whitespace closeout is recorded below. Inventory: **1366 source files /
  43 oversized**; no new source-size exception. No UI/protocol changes or native/platform claim.
- Only local graph and localhost TCP fixtures were used. No external endpoints, devices,
  commits, pushes, or remote publication. P03 remains **IN_PROGRESS** for the explicit remaining
  ownership, module, engine/lifecycle, generator, and qualification work above.

Focused response-service/snapshot verification is complete. **Return to High requested; actual
effort unchanged/unobservable**. Next resume at HTTP `refresh_transport` / `stop_transport` and
the public `BoundedWorker` admission/lifetime boundary. Preserve accepted old-generation request
ownership across reconfiguration, including full retirement capacity and slow endpoints, before
claiming that contract. That focused concurrency repair needs a new escalation. Do not restart
the verified response-service or P01/P02 work.

P03 batch 2 closeout: root and Golden Core `cargo fmt --all`, explicit app HTTP/common/OSC
Rustfmt checks, five maintained-doc Prettier checks, and whitespace checks passed. No live
validation jobs remain. The overall foundation/readiness goal is still active.

P03 batch 3 resumed at the same HEAD and verified accumulated P01/P02/P03 diff. Previous goal
turn: **progress**. No unrelated edits or live Cargo/Rust jobs found. App HTTP owns generation
handoff and endpoint ordering; public `golden_io` owns graceful admission closure and worker
lifetime mechanics. **XHigh requested; actual effort unchanged/unobservable** for accepted
request ownership, completion order, and bounded reconfiguration/recovery. No supported effort
control is exposed. Do not drop the old lifetime merely because a new configuration was authored.

P03 batch 3 implementation (same HEAD plus accumulated diff):

- Baseline `cargo test-fast --locked -p Chataigne2
http_reconfiguration_preserves_accepted_requests_execution_and_completion_order -- --nocapture`
  failed (0 passed / 1 failed): the replacement executed ahead of older admitted requests.
- Public `BoundedWorker::finish_admission` closes a producer without cancelling its queue;
  receiver-side admission closure similarly preserves accepted entries for failure completion.
  Two new generic tests passed, including full command/reply mailboxes and a still-live producer.
- App `http/lifecycle.rs` owns at most three generations per module, with the existing shared
  16-slot native lifetime pool. Handoff retains old handles, closes their admission, and gates
  new request execution behind their completion. Both execution and callbacks stay ordered;
  the aggregate drain still admits at most four events to the module per turn. Identical settings
  reuse the live generation; failed/full reconfiguration rejects new requests while preserving
  old ownership and coalescing the latest authored setting through the existing backoff.
- Native cleanup no longer blocks request/result progress. An atomic completion marker is
  observed before draining the final queue, so racing the last publication cannot discard it.
  The native worker retains its slot through client destruction even after the consumer releases
  the drained handle. Gated cancellation uses a paired lock/notification to avoid a lost wake.
- Request execution catches worker panics, closes admission, and returns explicit failures for
  the started request and each queued request. Attempt counts retain possible endpoint effects;
  queued requests report that they were not sent. Startup failure uses the same closed-admission
  drain. Invalid settings and disabling preserve owners; re-enable resumes bounded completion
  delivery. Only destruction cancels the removed destination's lifetime.
- Final owning suites passed **31 HTTP tests** and **22 I/O tests**. Eight new HTTP cases cover
  real loopback execution/completion ordering, a full 256-command mailbox with failed new
  lifetime admission, repeated changes at the owner limit, identical-setting reuse, worker
  failure/recovery, gated stop, blocked native retirement, disabled backlog/re-enable, and
  invalid settings. Strict owning all-target Clippy passed; root workspace/test check is running.
- Inventory: **1370 source files / 43 oversized**, none newly oversized. HTTP coordinator is
  852 lines; lifecycle ownership and fence are 156/82 lines. No protocol/UI declarations changed.
  Only loopback and injected worker fixtures were exercised; no device, native artifact, or
  cross-platform qualification is inferred. No commits, pushes, or remote publication.

Final P03 batch 3 validation (same HEAD plus accumulated diff, Windows/default features):

- `cargo test-fast --locked -p Chataigne2 http -- --quiet`: **31 passed**; no HTTP failures.
  `cargo test-fast --locked -p golden_io -- --quiet`: **22 passed**; no I/O failures. Loopback
  reproduction now preserves both old requests, delays replacement dispatch, and delivers all
  three callbacks in order. The full-mailbox case retains all **257** admitted commands while
  new native capacity is rejected, then recovers. Disabled output remains bounded to four replies
  plus one in flight and all 17 accepted requests complete after re-enable.
- Strict `cargo clippy --locked -p Chataigne2 -p golden_io --all-targets -- -D warnings` and
  root `cargo check --locked --workspace --tests` passed on the final source. Existing engine,
  frontend, protocol-generator, and platform evidence retains its prior scope; those suites were
  not rerun merely for unchanged source. No full product/device qualification is implied.
- Source-size inventory: **1370 files / 43 oversized**. All new/touched owning modules remain
  below 1000 lines. Formatting/docs/whitespace closeout is recorded below. No validation jobs
  remain. P03 is still **IN_PROGRESS** for the outstanding engine/lifecycle/module/generator
  requirements; P04-P18 retain their ledger states.

Focused HTTP handoff/worker-lifetime verification is complete. **Return to High requested;
actual effort unchanged/unobservable**. Resume at P03's queued graph-edit failure/cursor audit,
then remaining module/channel/configuration bounds. Preserve the verified HTTP service and
ownership repairs. The next coupled-state or lifecycle/concurrency repair requires its own
focused escalation; no further P-task or overall goal is marked complete.

P03 batch 3 closeout: root and Golden Core `cargo fmt --all`, explicit app HTTP Rustfmt,
maintained-doc Prettier, and whitespace checks passed. The overall foundation/readiness goal
remains active; no source checks or remote actions are pending for this verified batch.

P03 batch 4 resumed at the same HEAD and accumulated diff. Previous goal turn: **progress**.
Scheduled callbacks accumulate edits before stabilization; HTTP advances its response cursor
before those edits apply. A public callback/queue regression now checks an earlier rejected
request followed by HTTP materialization. Golden owns edit completion/disposition; HTTP owns
the response outcome and must never replay endpoint effects or already-applied pulses.
**XHigh requested; actual effort unchanged/unobservable** for coupled deferred completion,
accepted-prefix errors, structural deferral, and atomic rollback. No supported control is exposed.
The baseline regression is compiling; no failure or repair result is claimed yet.

P03 batch 4 baseline: `cargo test-fast --locked -p Chataigne2
http_rejected_graph_edits_report_failure_without_replaying_the_response -- --nocapture`
**failed (0 passed / 1 failed)**. The public callback/queue fixture discarded the HTTP subtree
behind an earlier invalid parameter edit, then incorrectly emitted `responseReceived`.
This models the shared scheduled-callback queue, without clearing or altering a produced edit.

The repair adds generic `EditQueue::completion` receipts. Tokens follow ingress coercion,
duplicate removal, structural deferral, and journal ownership. Superseded/discarded requests fail;
atomic success waits for commit and rollback fails it. Observed warning no-ops remain queued so
an earlier rejection cannot give them premature success. The focused ingress module replaces
the old inlined validator without adding a second coercion policy. HTTP now waits for each bounded
turn's receipt and reports graph-application failure without retrying the endpoint or pulses.
Owning test compilation passed; full engine/HTTP suites are running. No final pass is claimed yet.

The full engine suite now passes **517 unit tests, one public integration test, and seven
doctests**, with three existing manual qualifications ignored. Seven new tests cover applied
prefix/discard, atomic commit/rollback and prior queued work, observed no-ops, coalescing and
duplicate removal, invalid ingress/teardown, real scheduled-event structural deferral, and an
insertion whose ready callback fails after mutation. Existing P01/P02 regressions remain green.
The HTTP suite also includes a separate callback-delivery ownership regression: the module
currently pops a response before its queued custom event is applied. That baseline is compiling;
the graph fix alone must not be reported as covering callback publication.

P03 batch 4 callback baseline: `cargo test-fast --locked -p Chataigne2 http -- --quiet`
passed **33 tests / failed one**. Graph rejection and the partial-trigger regression passed;
`http_retains_callback_ownership_when_a_shared_queue_discards_delivery` reproduced permanent
loss of the queued callback (zero observed instead of one). This is a second reproduced defect,
not a failed fix attempt for the first graph-cursor issue.

HTTP now retains a shared `CustomEvent` until a receipt for that event alone confirms publication.
Discard/atomic rollback resubmits only the unpublished event, preserving FIFO ownership without
replaying graph values or endpoint effects. Later edit/subscriber errors cannot duplicate an
already-published callback. The app's existing callback-event builder is exposed within the app,
and its JSON wrapper moves prepared arguments instead of serializing a second body copy.
Snapshot requirements now follow the response stage: receipt waits, terminal acknowledgement,
failure delivery, and callback-only turns use no tree snapshot. A new direct callback regression
checks pending receipts and repeated calls without a snapshot; the delivery regression also checks
an error after publication. Final HTTP and app-helper checks are pending.

Final P03 batch 4 validation (same HEAD plus accumulated diff, Windows/default features):

- `cargo test-fast --locked -p golden_engine -- --quiet` passed **517 unit tests, one public
  integration test, and seven doctests**; three existing manual qualifications remain ignored.
  Its seven new completion tests and all prior P01/P02 regressions passed. Final formatting did
  not change that engine source.
- `cargo test-fast --locked -p Chataigne2 http -- --quiet` passed **35 tests** on the final app
  source. Four new cases prove explicit graph rejection, no replay of an applied trigger prefix,
  callback ownership across both earlier/later failures, and repeated receipt/delivery calls
  without a tree snapshot. Existing bounded-service, handoff, recovery, and loopback tests passed.
  `cargo test-fast --locked -p Chataigne2 script_api -- --quiet` passed the one owning app-helper
  test; callback payload shape is also exercised throughout the HTTP suite.
- Strict `cargo clippy --locked -p Chataigne2 -p golden_engine --all-targets -- -D warnings`
  and root `cargo check --locked --workspace --tests` passed on the final source. Root and Golden
  Core `cargo fmt --all`, explicit app HTTP/script-helper Rustfmt, maintained-doc Prettier, and
  whitespace checks passed. No protocol/UI declarations changed; unchanged I/O/frontend/platform
  evidence keeps its previous scope rather than inheriting a new qualification claim.
- Inventory: **1373 source files / 43 oversized**, no new size exception. Engine coordination
  is 796 lines; ingress is 147 and completion tokens are 76. HTTP coordination is 864 lines;
  response service/callback ownership are 344/152 lines. Receipts add work proportional to the
  bounded observed edit set and retain no graph or edit payload copies.
- Only local graph, injected worker, and loopback fixtures were exercised. No external service,
  device, native artifact, cross-platform, or mounted-product qualification is inferred. No
  commits, pushes, or remote publication. P03 remains **IN_PROGRESS**; P04-P18 retain their states.

Focused deferred-completion/HTTP ownership verification is complete. **Return to High requested;
actual effort unchanged/unobservable**. Next audit P03's unbounded engine external edit sender/
drain in `engine/mod.rs`, its typed ingress in `engine/ingress.rs`, and lifecycle materialization
budgets before continuing the remaining app channels/UDP/generator requirements. Preserve the
verified completion tokens through any ingress API change. A new concurrency or coupled-lifecycle
repair needs its own focused escalation. No validation jobs remain for this batch; the overall
foundation/readiness goal remains active.

P03 batch 5 resumed at the same HEAD and accumulated diff. Previous goal turn: **progress**.
No live Cargo/Rust jobs were found. Source audit confirms that `Sender<Edit>` is unbounded and
drains until empty at multiple points per tick; its only production consumer is the passive
Sound Card wake path. Arbitrary boxed nodes and mutable closures have no enforceable retained-
payload weight at that boundary. Module workers already own their actual result mailboxes.
The replacement will carry reserved/coalesced node wake signals only; payload processing and
graph edits stay in node callbacks and public actor operations. It must not strand one-shot
worker completion on capacity rejection or callback absorption failure. Sound Card's bounded
delayed-wake helper also needs reservation/coalescing before its immediate-wake fallback can be
removed. **XHigh requested; actual effort unchanged/unobservable** for queue, generation/lifetime,
and callback-delivery invariants. No supported effort control is exposed. Lifecycle expansion
and the remaining module/channel/UDP/generator audits remain explicit P03 work.

P03 batch 5 baseline (Windows/default features, same HEAD plus accumulated diff):
`cargo test-fast --locked -p golden_engine --test public_graph_editing
external_edit_drain_has_per_turn_service_bound -- --nocapture` **failed (0 passed / 1 failed)**.
One drain consumed all 1024 accepted events rather than leaving a bounded suffix owned. Its
public regression now exercises the replacement API: 512 reserved worker wakes require two
256-entry ticks, followed by an idle tick. Arbitrary cross-thread `Edit` injection has been
removed, not retained behind guessed weights or a compatibility sender. The P02 failed-tick
publication fixture now queues its authored edits through the existing application actor call;
its history/publication assertions are unchanged.

Implementation now provides per-node reservations, coalesced queued signals, a separate bounded
worker-service turn before controls, and a receipt-held acknowledgement stage for deferred edits.
Signals arriving during callback/application waits are retained. Absorption failure or a discarded
edit receipt re-arms readiness; payload/cursor recovery stays in the module. Removed/replaced
sources close and restored IDs receive fresh sources; engine ingress closes before node drop.
Sound Card consumes the generic wake path, releases its own source on destruction, reuses an
unchanged source, and uses one process-wide reserved/coalesced retry timer without immediate-poll
fallback. Configuration/startup-dependent snapshots preserve its former inbox requirements.
Core/app test compilation passed after ordinary migration wiring fixes. Full engine, Sound Card,
and strict Clippy checks are running; no final test result is claimed yet. Sound Card fixtures
use their existing test-only null backend, and new timer fixtures use only Folder nodes. Native
stop/join fallbacks, event-drain budgets, and replace-node lifecycle ownership remain separate
P03 audit work; this wake repair does not qualify the whole audio module.

The first full engine run passed **519 tests / failed one**, with three existing ignores. The
new fixture incorrectly used the UI's latest-scalar projection to count three input occurrences;
source review confirms `ui_coalescable_param_value_event` deliberately retains one scalar value
and every trigger edge. The corrected fixture checks the final scalar state and three real trigger
edges independently. No engine event-coalescing policy was changed to satisfy that assertion.

A subsequent focused design audit separated worker service from timed update ownership:
`Node::on_worker_wake` and `worker_wake_requires_tree_snapshot` now have the standard enum
forwarding. Worker service has zero scheduled delta and leaves replay/coalesced update baselines
untouched. A continuously re-armed passive node and an explicitly woken periodic node run beside
scheduled work with separate budgets; the fixture asserts that the scheduled node still receives
exactly its one-millisecond delta. This prevents a worker notification from advancing generator
time before the scheduled pass. Sound Card now implements the dedicated hook. Its first **36-test**
run passed before that hook separation; final engine/app suites and Clippy are running on the
revised source. App/core test compilation passed. No physical device was opened by these fixtures.

P03 batch 5 focused validation passed **520 engine unit tests, three public integration tests,
and seven doctests**, with three existing manual qualifications ignored. The final public race
fixture uses timeout-bounded channel rendezvous with a real producer during callback execution;
its second wake survives acknowledgement and the following tick becomes idle. Core tests also
prove 512 signals drain in two 256-entry ticks, live/retiring capacity rejects and recovers,
callback-absorption and deferred-application failures re-arm correctly, three trigger edges survive
coalesced notifications, and worker service cannot advance a periodic node's scheduled clock.
The app's **37 Sound Card tests** passed, including three timer cases and a real null-backend
startup/destroy/undo case proving that the passive module resumes through its reserved source and
re-registers after restoration. No physical endpoint was selected. Focused all-target Clippy passed;
final checks are pending after the fixture refinements below.

An additional public-consumer probe using `golden_engine::node` attribute macros failed to compile
with E0433 (emitted `golden_core` crate path). The notification race fixture now implements the
public `Node` trait directly, without adding a crate-alias shim. Direct lower-crate attribute-macro
support remains an unqualified P12 build/consumer audit item; the app's canonical `golden_core`
facade usage compiles. Do not infer that the macro path was repaired by the passing trait fixture.

The HTTP preservation run passed **34 / failed one**: the disabled-backlog fixture received
Windows `WouldBlock` (10035) from its accepted stream before headers arrived. A controlled test
accepting from a nonblocking listener and delaying headers 20 ms reproduced it deterministically
(0 passed / 1 failed). The owning helper now explicitly sets blocking stream mode before its
existing three-second read timeout; production HTTP behavior is unchanged. Its full suite now
includes the delayed-header regression and is running. **Return to High requested; actual effort
unchanged/unobservable** for this routine fixture and final lint/workspace/doc closeout.

Final P03 batch 5 validation (same HEAD plus accumulated diff; Windows/default features,
2026-10-19):

- `cargo test-fast --locked -p golden_engine -- --quiet` passed **520 unit tests**, both public
  integrations then present, and **seven doctests**; three existing manual qualifications remain
  ignored. The final `cargo test-fast --locked -p golden_engine --test public_graph_editing --
--quiet` passed **all three public integrations**, including the timeout-bounded real-producer
  race added afterward. No production engine behavior changed after its full suite passed.
- `cargo test-fast --locked -p Chataigne2 sound_card -- --quiet` passed **37 tests**, including
  passive null-backend startup/destroy/undo and the three bounded timer cases. Test defaults select
  the null backend; no physical device or native-route qualification is inferred.
- Final `cargo test-fast --locked -p Chataigne2 http -- --quiet` passed **36 tests**. The new
  delayed-header case now passes, as does the 17-request disable/backlog/re-enable case that exposed
  the Windows stream-mode assumption. All previous HTTP admission, FIFO, trigger, handoff, failure,
  and callback-publication regressions remain green. Production HTTP code did not change here.
- Final strict `cargo clippy --locked -p golden_engine -p Chataigne2 --all-targets -- -D warnings`
  and root `cargo check --locked --workspace --tests` passed. Maintained docs and source formatting/
  whitespace closeout follows below. Unchanged UI/protocol/platform checks retain their previous
  scope; no new wire declarations or persisted schema were introduced.
- Inventory is **1378 source files / 42 oversized**, with no new exception. Standard formatting
  brings the touched Sound Card integration coordinator from 1006 to 975 lines. The new generic
  wake registry/service are 292/55 lines; engine coordination is 773 lines. New runtime and test
  modules all remain below 1000 lines, with conventional test locations.
- Engine ingress now owns only fixed-size, reserved/coalesced notifications. Module mailboxes
  still own payload bytes and occurrence recovery; a budget-limited drain must re-arm its source.
  Cold timer-registration rejection is explicit startup admission failure, not accepted worker
  work. Full module/native recovery, stop/join fallbacks, event budgets, and replacement lifecycle
  semantics still need their P03 audit; this batch qualifies only the stated wake/timer contracts.

P03 remains **IN_PROGRESS**. Resume at `engine/apply_tree/lifecycle.rs`'s unbounded materialization
loop and `apply_tree/replace.rs`'s payload lifecycle ownership, then the remaining module/channel,
UDP, and generator work. Preserve the verified P01/P02 and HTTP ownership repairs. The next
coupled lifecycle repair needs its own focused escalation; High remains the default, actual effort
unchanged/unobservable. No commits, pushes, remote service calls, physical device opening, or remote
publication were performed. All owned validation jobs are terminal; the overall foundation/
readiness goal remains active.

P03 batch 5 closeout: root and Golden Core `cargo fmt --all`, explicit HTTP/Sound Card Rustfmt
checks, five maintained-doc Prettier checks, and whitespace checks passed. All owned validation
processes have finished. No further source action is pending for this verified batch.

P03 batch 6 resumed at the same HEAD and accumulated diff (167 status paths). Previous goal
turn: **progress**. No live Cargo/Rust jobs or unrelated edits were found. The engine owns this
repair: lifecycle materialization has an unbounded nested drain, and replacement currently swaps
payloads without the outgoing destroy or incoming attach/init/ready pipeline. Bound callback-
generated expansion before changing handoff; preserve known detached-subtree batch scaling,
P01's committed-prefix/atomic contracts, history replay, and fresh worker-wake generations.
**XHigh requested; actual effort unchanged/unobservable** for these coupled lifecycle/history
invariants. No supported control is exposed. Other module/channel/UDP/generator work remains P03.

P03 batch 6 baseline: the new public lifecycle fixture reproduced both gaps: a ten-generation
init chain ignored a two-pass limit, and replacement emitted no destroy/init/ready callbacks.
The original baseline session was no longer observable after context rollover; the bounded
rerun failed both assertions and supplies the baseline evidence. The focused public tests now
pass five cases (including both undo/redo failure directions). Broader validation is pending.

Batch 6 implementation:

- Added engine-owned `LifecycleLimits` (64 passes, 65,536 generated requests/new nodes by default)
  and typed exhaustion errors, including the existing UI rejection boundary. Nested structural
  drains share the admitted scope. Known detached inputs are not capped; generated trees and
  user-item forests reserve work before coercion/insertion. Rejected suffixes fail receipts and
  cannot silently continue on the next tick. Ordinary prefixes remain undoable; atomic rollback
  keeps its existing inverse journal/publication rules. These limits do not preempt callback code.
- Replacement now destroys only the outgoing root payload, fences its waker/subscriptions, and
  runs incoming attached/init/ready (deferred ready for project load). Retained children stay live.
  History swaps destroy the outgoing runtime and re-ready the incoming payload without re-init;
  both atomic-journal and failed-history rollback track replacement roots for reactivation.
  Both rollback paths deduplicate restored IDs; multiple replacements of one root activate
  the final restored payload once, covered by a runtime-start counter regression.
- Public regressions cover pass/work admission, full generated forest reservation, explicit wide
  trees, suffix failure receipts, subsequent-tick stability, retained child IDs, stale wake handles,
  and repeated failed history activation with retry. Added unit checks for project-load expansion
  and atomic replacement rollback, plus a null-backend Sound Card replacement/undo smoke test.
  Those owning-unit/app checks are running; no final batch/P03 acceptance claim yet.

Batch 6 final-source engine verification passed: `cargo test-fast --locked -p golden_engine
-- --quiet` from Golden Core ran **522 unit tests**, **five public lifecycle integration tests**,
**three public graph-editing integration tests**, and **seven doctests**; three existing manual
benchmarks remained ignored. This includes the final repeated-root rollback activation counter.
Sound Card/HTTP preservation, strict Clippy, and workspace/test compilation are still pending.

Returning to **High requested; actual effort unchanged/unobservable** after the coupled engine
invariants passed. The app preservation and formatting/lint closeout remain routine validation.

Final P03 batch 6 validation (same HEAD plus accumulated diff; Windows/default features,
controlled loopback/null backend, no physical devices):

- Golden Core `cargo test-fast --locked -p golden_engine -- --quiet`: **522 unit tests,
  eight public integration tests, seven doctests passed**; three existing manual benchmarks
  ignored. The new checks include ordinary-prefix undo/redo, known wide inputs, generated
  tree/forest preflight, project-load augmentation, failed history activation/retry, retained
  children, stale worker generations, and repeated-root atomic rollback with one restart.
- Root `cargo test-fast --locked -p Chataigne2 sound_card -- --quiet`: **38 passed**,
  including repeated live null-runtime replacement/undo/redo, in addition to prior timer/wake
  tests. Root `cargo test-fast --locked -p Chataigne2 http -- --quiet`: **36 passed**;
  error logs are intentional injected admission/application/recovery failures.
- Root `cargo clippy --locked -p golden_engine -p Chataigne2 --all-targets -- -D warnings`
  and `cargo check --locked --workspace --tests` passed. All owned build jobs reached terminal
  results; long builds were observed through their original sessions without restarting.
- Root/Golden Core formatters and checks, explicit app Sound Card Rustfmt, maintained-doc
  Prettier, and `git diff --check` passed. Inventory: **1380 source files; 42 over 1000 lines**;
  the new budget module/public suite introduce no exception.
- No project schema or wire DTO changed. No GUI, physical device, commit, push, or publication.
  P03 remains **IN_PROGRESS** for the broader module/UDP/generator work; P04-P18 states stand.

Next confirmed source gap: `apps/chataigne/src/module/modules/protocol/stream/udp/transport.rs`
uses unbounded `mpsc::channel` commands and assigns unit weight to variable-size events through
the item-bounded `pending_channel`; retained packet bytes therefore lack an effective bound.
It drains both commands and received packets until empty, resolves/binds in `spawn` on its caller,
and joins in `stop`. Review its module
ownership/recovery and received-value processing before choosing the next bounded worker shape.

P03 batch 7 starts at the same HEAD and accumulated diff (169 status paths); batch 6's jobs are
terminal. Previous work is progress. UDP transport/module owns this next repair; generic
admission/cancellation/lifetime contracts stay in `golden_io`. **XHigh requested; actual effort
unchanged/unobservable** for endpoint generation, accepted-command ownership, and retirement
races. No supported effort control is exposed. Establish a controlled baseline before repair;
preserve command/script behavior, explicit overload outcomes, endpoint recovery, and loopback-only
qualification. No physical endpoints, commit/push, or publication are authorized by this work.

Batch 7 source audit (no runtime repair yet): UDP also runs at 120 Hz, parses received bytes on
the node, and enumerates interfaces on the caller. `StreamingModuleBase` caches parser settings
and owns a bounded incoming-message queue, so preserve that shared stream contract while moving
IO preprocessing to its worker. `golden_io::BoundedWorker` already provides weighted admission,
reserved worker lifetimes, cancellation independent of mailbox capacity, and graceful admission
closure. It currently exposes only blocking `WorkerContext::receive`; unsolicited UDP input
needs a bounded nonblocking/timed command service API before reusing it. Avoid copying the
HTTP-specific generation model blindly: UDP needs explicit socket configuration ordering,
fair receive/send turns, failure/reconnect backoff, and retained command outcomes across changes.

Batch 7 reconciliation confirmed the same HEAD/169-path starting diff and no live Cargo/Rust
process at resume. The first Cargo baseline filter matched zero tests because app module paths
are generated (`app::module_modules_protocol_stream_udp`); inspected the built binary's test
list and reran the exact owning prefix. Zero tests are not validation evidence.

Added `WorkerContext::drain_commands` for bounded nonblocking service beside unsolicited native
input. Golden Core `cargo test-fast --locked -p golden_io -- --quiet` passed **24 tests**,
including FIFO/budget behavior through graceful closure and explicit cancellation; worker-thread
assertions are acknowledged back to the parent so a detached panic cannot masquerade as success.
UDP has not consumed the new API yet; the native worker repair follows its controlled baseline.

Batch 7 controlled baseline: the exact UDP transport test prefix ran **two regressions, both
failed**. A gated native setup prevented the caller receiving its handle until release; an
impossible 65,536-byte UDP datagram was admitted instead of rejected. Gates were released and
baseline workers reached terminal cleanup. The first repair compile check (`cargo check
--locked -p Chataigne2 --tests`) passed. A subsequent test build caught an incorrect test-only
module path before execution; that fixture now uses the bound socket directly. No alias/private
crate import was introduced. The corrected owning test suite is running in session 9908.

Batch 7 implementation (validation pending):

- UDP consumes `WorkerContext::drain_commands` through `BoundedWorker`: 256 command items/1 MiB,
  64 event items/512 KiB, retained allocations charged by capacity, 16 command/receive operations
  per native turn, and four events per node drain. Payloads above IPv4 UDP's 65,507-byte maximum
  and host strings above 1024 bytes fail admission explicitly. Bind/DNS/socket setup and
  destination resolution live on the reserved worker; stop/cancellation never joins native work.
- An app-owned `UdpTransports` state machine owns active/retiring generations. Configuration
  changes and disable fence admission and preserve accepted sends/results; a new bind waits for
  old socket release. Only destruction abandons the destination. Async Ready controls connected
  state; failures close admission, resolve queued sends explicitly, and use 250 ms-to-5 s
  reconnect backoff. Native lifetimes share a 16-slot pool, including blocked cleanup.
- Regressions cover blocked setup/cancel, byte admission, reserved lifetime saturation, 32 ordered
  sends across graceful closure, failed-bind completion, same-port reconfiguration, reconnect
  deadlines, and continuous receive pressure alongside bounded owner turns and outgoing sends.
- P03 remains incomplete: UDP's existing 120 Hz node service, interface enumeration, parser work,
  shared parser line retention, structural queue weight, and graph-application cursor still need
  the next coupled repair. This native ownership batch does not qualify those existing paths.

Batch 7 final-source owning checks passed: Golden IO **24 tests**, app UDP **10 tests**,
including public script `sendText` FIFO delivery and disabled rejection. The standalone native
baseline's two failures are now green. The earlier nine-case pass predates the script test and
test cleanup refinement; the ten-case rerun supplies final-source app evidence. Strict
`golden_io`/app Clippy passed before that last fixture addition and is rerunning alongside
workspace/test compilation and HTTP/Sound Card preservation. Inventory refreshed to **1383
source files / 42 above 1000 lines**, with unchanged oversized rows.

Final P03 batch 7 validation (same HEAD plus accumulated diff; Windows/default features,
controlled loopback and null backend):

- Golden Core `cargo test-fast --locked -p golden_io -- --quiet`: **24 passed**. Root
  `cargo test-fast --locked -p Chataigne2 module_modules_protocol_stream_udp -- --quiet`:
  **10 passed**, including final fixture cleanup and script send/disabled-admission preservation.
  The initial zero-match filter is excluded from evidence; the inspected owning prefix was used.
- Root HTTP **36 passed** and Sound Card **38 passed**, preserving the previous bounded
  completion, retry/wake, lifecycle replacement, and timer contracts. HTTP error logs are
  intentional injected failures. No hardware or external endpoints were opened.
- Root `cargo clippy --locked -p golden_io -p Chataigne2 --all-targets -- -D warnings` and
  `cargo check --locked --workspace --tests` passed on final source. All owned jobs are terminal.
  Root/Golden Core formatters, explicit UDP Rustfmt, maintained-doc Prettier, and diff checks
  are the final closeout checks. Source inventory is **1383 / 42 oversized**; no new exception.
- Local datagram order and explicit failure/admission are qualified; remote UDP delivery is not
  an acknowledgement contract. Native calls that cannot be interrupted retain their lifetime
  slot until return, as established by controlled blocked-setup/cancellation/saturation tests.
- No wire DTO, project schema, GUI, physical device, commit, push, or publication change.
  P03 stays **IN_PROGRESS** for the shared stream/node and wider module/generator audits;
  P04-P18 states remain unchanged.

Returning to **High requested; actual effort unchanged/unobservable** after native command,
generation handoff, cancellation, and recovery checks passed. The next streaming/parser/cursor
work needs its own focused effort audit; do not treat this return as completing P03.

Batch 7 closeout: root and Golden Core `cargo fmt --all`/checks, explicit UDP Rustfmt,
six maintained-document Prettier checks, and `git diff --check` passed. No owned jobs remain live.

Milestone delivery reconciliation: `main` is an ancestor of `codex/builtin-mapping` (87 commits
behind), including the local outliner-windowing commit ahead of `origin/main`; the requested
merge can fast-forward without discarding that work. The unfinished shared-stream baseline
tests and their formatting are excluded from the verified milestone. Its controlled run
executed 22 tests: 18 passed and four reproduced open retained-byte, parser-line, receipt,
and ordered-value defects. These failures are not evidence against the verified batch-7 tree
and must be repaired in P03 batch 8 after switching to `main`.

P03 batch 8 resumed on `main` at delivered `f02205af`, with only nine shared-stream WIP paths
restored and no live owned jobs. The controlled baseline ran 22 tests (18 passed, four failed),
reproducing retained string capacity, unbounded unterminated lines, premature queue acknowledgement,
and an ordered return to the snapshot value being omitted. Owning app-common received-value and
streaming layers must preserve HTTP's verified cursor/receipt behavior. **XHigh requested;
actual effort unchanged/unobservable** for coupled cursor/receipt/ordering invariants; no supported
effort control is exposed. UDP worker parsing/notifications remain a later P03 batch.

Batch 8 implementation on `main` (validation pending):

- App-common `received_values` now owns one HTTP/stream message model, dynamic allocation
  accounting, and the proven HTTP receipt-driven graph cursor. Stream queues admit complete
  FIFO batches under 65,536-message / 16 MiB / 64-path-level limits, including active batches
  waiting for engine receipts. Overflow rejects the whole new batch with backend diagnostics.
  The obsolete incoming-value parameter-ignore set was removed: transport invalidation already
  compares only its bound connection controls, which are distinct from incoming value nodes.
- Shared planning compares repeated existing values against its ordered prefix, fixing `0 → 1 → 0`
  within one snapshot. Existing compatible targets can share a cursor turn; structural conflicts
  and newly created trigger addresses retain their fresh-snapshot/pulse rules. Turns retain the
  256 work-unit / roughly 64 KiB copy targets, allowing one admitted larger scalar rather than
  stranding it after path lookup consumes work units. Graph failure/discard abandons the suffix
  without replay and allows later batches to recover.
- Line parsing retains at most 1 MiB with capped geometric growth and chunk/delimiter scans,
  rejecting each oversized frame once through its delimiter while preserving surrounding frames.
  Grammar changes reject partial old lines explicitly. Raw/JSON preparation expansion, node-side
  parsing/callback preparation, UDP interface discovery/periodic scheduling, wider module bounds,
  and generator accounting remain open P03 work.
- The first repair check found one missed HTTP expansion-weight helper after extraction; the
  owning shared payload method replaced it. The next app test command compiled but failed linking
  an additional non-test app binary with 97 unresolved MSVC symbols in unrelated generated
  modules; no tests ran. Scoped `--bin Chataigne2` test validation follows. This diagnostic does
  not qualify the new implementation. Inventory refreshed to **1386 source files / 42 oversized**.

Batch 8 owning and preservation checks passed on final source (Windows/default features):

- Root `cargo test-fast --locked -p Chataigne2 --bin Chataigne2 common::streaming -- --nocapture`:
  **33 passed**, including all four failing baselines, capacity retained through receipts, whole-batch
  admission, ordered repeated values, a 256 KiB scalar, 1000 list values over bounded turns,
  all 65,507 raw datagram bytes without truncation, applied-trigger-prefix failure, dropped-context
  failure, parser boundary/recovery, and configuration fencing. Test execution took 0.15 s;
  this is functional evidence, not production latency qualification.
- The resulting final-source app unit-test executable ran owning prefixes directly: HTTP
  **36 passed**, concrete stream modules **31 passed**, Sound Card module/commands **37 passed**
  plus the exact null-backend product-evidence test **1 passed**, and generic OSC **18 passed**.
  The `received_value` discovery filter matched zero tests and supplies no validation evidence.
  HTTP injected failures and nonexistent serial-port recovery diagnostics are expected. A default
  TCP-server fixture logged a port-9002 collision while all 31 concrete stream checks passed;
  this run does not qualify exclusive ownership of that default port or production endpoint behavior.
- Root `cargo clippy --locked -p Chataigne2 --all-targets -- -D warnings` and
  `cargo check --locked --workspace --tests` passed. The separate native app binary's earlier
  unresolved-symbol link is still being checked with an app-package-only clean/rebuild; no source
  workaround or global dependency-cache deletion was introduced.

Batch 8 native build closeout: `cargo clean -p Chataigne2 --profile dev` followed by root
`cargo build --locked -p Chataigne2 --bin Chataigne2` passed in 2m15s. The clean affected only
the app package's accumulated build artifacts; shared dependency caches and source were retained.
The earlier unresolved-symbol failure did not recur after that rebuild. No production source
workaround was needed. All owned jobs are terminal. Root/Golden Core Cargo formatting, explicit
app-module Rustfmt, maintained-doc Prettier, and diff checks passed; inventory remains **1386 / 42**.

Returning to **High requested; actual effort unchanged/unobservable** after focused graph/receipt
repair and validation. P03 remains **IN_PROGRESS** for preparation admission, worker notifications,
the wider module/channel audit, and generators. P04-P18 states remain unchanged. Commit and push
this verified batch on `main` before beginning the next milestone.

P03 batch 9 resumed at `9d8500b49091b08a3d1c978bd91b41016ff09411`, clean and tracking
`origin/main`; the previous batch was committed and pushed. The development watcher exists,
but no owned implementation job was live at resume. App-common stream preparation is the
owning boundary: the queue now bounds retained messages after parsing, while raw/JSON expansion
still allocates before those checks. Add input, decoded-value, path, and retained-expansion
admission before migrating UDP parsing/callback preparation to its worker. Keep ordinary bounded
parsing work at High; no concurrency/worker-generation repair is included in this first subtask.
Four controlled raw/JSON admission regressions are building before the repair.

Batch 9 controlled baseline ran four tests and **all four failed**, proving missing raw-frame,
JSON-input, JSON-depth, and retained path-expansion admission. The first repair
`cargo check --locked -p Chataigne2 --tests` passed. Owning preservation tests are building.

Concurrent checkout changes appeared during validation in `apps/chataigne/build.rs`,
`golden_core/support/codegen` (a new build-cache module and tests), and
`docs/guides/development.md`. These are unrelated work and must remain outside this milestone.
Validation on this shared checkout includes those build-cache changes; the stream repair does
not depend on their new API. Source scans also found remaining unbounded channels or native joins
in TCP/WebSocket, MIDI, MQTT, OSC, Buttplug, Joy-Con, keyboard/mouse, Ultraleap, OS/App Control,
Sound Card, and signal/metronome workers. This is source evidence for the remaining P03 audit,
not a claim that every match is a defect: string/path `.join` matches were excluded and each
native ownership path still needs its owning behavioral check before repair.

Batch 9 implementation and owning evidence (Windows/default features; shared checkout):

- `common/streaming/admission.rs` owns `PreparedMessages`: immutable received values and cached
  retained allocation weight. Both the parser and node queue use the same message/byte/path limits;
  queue admission no longer walks every message. Spare vector/string capacity is charged, and
  consumers cannot mutate admitted data without constructing and validating a new batch.
- Raw input above 65,536 bytes rejects before expansion. JSON input above 1 MiB rejects before
  whitespace scanning/decode; structural work/depth is checked before conversion or cloning.
  Paths support 64 levels / 4096 bytes, retained expansion stops at 16 MiB, and provenance uses
  UTF-8-safe prefixes. List limits derive from the value-storage budget: **100,000 list values**
  remain valid. Raw/JSON failures publish no partial frame. Individual rejected line frames preserve
  surrounding valid frames and later turns recover; the existing 1 MiB partial-line limit remains.
- The temporary serde JSON DOM and conversion scratch are additional allocations bounded by
  input/work/value limits, not included in a claimed 16 MiB process-RSS ceiling. Parser work,
  callback preparation, UDP interface discovery, and periodic service still need worker ownership.
- Root `cargo test-fast --locked -p Chataigne2 --bin Chataigne2 common::streaming -- --nocapture`:
  **44 passed**, including all four failing baselines, boundary/recovery, large-list preservation,
  maximum raw UDP data, retained capacity, ordering, and receipt-failure tests. Execution took
  0.16 s; this does not establish production latency. The final-source executable also passed
  concrete stream **31**, HTTP **36**, Sound Card/null **38**, and generic OSC **18** cases:
  **167 owning/preservation tests** total. As in batch 8, the default TCP fixture logged a
  port-9002 collision without failing its owning suite; exclusive default-port qualification
  remains outside this evidence. Strict lint/workspace/native-build closeout follows.

Preservation commands used the built
`target/debug/deps/Chataigne2-c47f0ddae7812c83.exe` with each of
`module_modules_protocol_stream`, `module_modules_protocol_http`, `sound_card`, and
`module_modules_protocol_osc_generic_osc_module` as its filter and `--quiet`.

The concurrent build-cache work was committed separately as `5b81c44e` while batch 9 was
underway; `main` now contains that parent and the owned stream diff. Preserve that commit.
Root strict app Clippy passed; root/Golden Core formatter checks and explicit app-module Rustfmt
checks passed. The shared checkout and eventual milestone tree now both contain **1391 source
files / 42 oversized**, including the two separately committed cache sources and three new stream
preparation sources. No new source-size exception. Workspace/test compilation is running next.

Root `cargo check --locked --workspace --tests` passed on the final stream implementation plus
the separately committed `5b81c44e` parent. Native `cargo build --locked -p Chataigne2 --bin
Chataigne2` is the remaining check before this milestone is committed and pushed. Source and
wire formats outside the app-owned stream boundary are unchanged by this batch.

Batch 9 closeout:

- Native build produced and linked `target/debug/deps/Chataigne2.exe` at 08:58:19 local time,
  after the last stream runtime source change at 08:51:09. SHA-256:
  `78bf9a0bddd8e0cf6800317f99ee02543ad09bc1dbabf0560d140c734d7efdc7`.
  Cargo then failed its final replacement of `target/debug/Chataigne2.exe` with Windows
  `Access is denied (os error 5)`. The user's running app, PID 31696 from 08:55:29, holds that
  executable open. **The Cargo build command did not pass**; native compilation/linking produced
  its artifact, while executable replacement is **UNAVAILABLE** until that process releases it.
  The process was left untouched. This does not qualify a fresh native launch.
- Owning/preservation **167 tests**, strict app Clippy, and workspace/test compilation passed.
  Root and Golden Core `cargo fmt --all -- --check`, explicit app Rustfmt, maintained-doc Prettier,
  and diff checks passed. No new protocol DTO, project schema, device qualification, or source-size
  exception. All owned jobs are terminal; the user's watcher/app remain outside this work.
- Keep P03 **IN_PROGRESS**. The next worker-ownership repair must include configuration fencing,
  prepared callback delivery, event/exit notifications, reconnect deadlines, interface discovery,
  and bounded disposal of large prepared batches. Opaque cached weights remove the actor's
  admission scan; they do not qualify large-batch destruction or move parsing off the actor.
  Preserve the current receipt/ordering/maximum-input regressions. No effort escalation was
  needed for this ordinary bounded parsing subtask; audit the next coupled worker design separately.
- Commit and push this verified preparation milestone on `main`, preserving `5b81c44e`, before
  beginning the next batch. P04-P18 states remain unchanged.

P03 batch 10 resumed on clean `main` at delivered
`b93f18ada220aa4adfa4b6fbdf90104eaab08a11`, matching `origin/main`. The previous turn was
progress: stream preparation admission was implemented, verified, committed, and pushed.
No owned jobs remain live; the user's watcher/app are not implementation jobs. The next change
belongs in generic Golden IO notification/deadline primitives and app-owned UDP stream/runtime
adapters. **XHigh requested; actual effort unchanged/unobservable** for worker/configuration,
retry/exit, prepared callback, and payload retirement invariants. No supported effort control
is exposed. Preserve the nine verified P03 batches and avoid joining or cancelling the user's app.

Batch 10 implementation and intermediate evidence:

- Generic `golden_io` now provides payload-free `WorkerNotifier`, notified pending channels and
  `BoundedWorker::spawn_notified`. Readiness is published after enqueue, outside the queue lock;
  final sender exit also notifies, including a worker panic. `DelayedNotifier` moves the existing
  shared Sound Card timer into Golden IO: one host timer thread, 1024 weak registrations,
  coalesced earliest deadlines, and explicit cancellation. Sound Card retains its node-generation
  adapter and now consumes that generic timer. No app type entered Golden IO.
- `PayloadPool` reserves both items and retained bytes before sharing data. One background
  disposer holds the final strong owner, performs destructors outside its state lock, and releases
  capacity only after destruction. Blocked cleanup retains its reservation; destructor panic does
  not strand the pool. Consumer release is a short ownership notification, never a join or payload
  destructor. App stream preparation uses one shared 256-payload / 256 MiB pool; immutable
  message batches and UDP prepared packets remain registered until their consumers release them.
  Domain values copied into the graph and callback payload copies retained by the engine have
  their own engine lifetimes; this pool is not a whole-process RSS or event-history budget.
- UDP is passive: no scheduled `update()` or compiled periodic kernel. Native reply/exit wakes
  and shared retry deadlines drive bounded service. Bind/DNS, input parsing, callback arguments,
  bounded logging prefixes, and interface discovery execute on the native worker. Interface
  discovery runs at most once per second there, suppresses unchanged results, and rejects more
  than 256 choices / 64 KiB without replacing existing choices. The native turn still services
  at most 16 commands and 16 receives; a node drains at most four events into one retained batch.
  Reply admission is now 64 items / 32 MiB so a maximum raw datagram plus prepared callbacks fits.
- Processing settings are copied at the worker input boundary. Grammar/processing generations
  discard old partial lines explicitly; logging-only changes do not change the grammar generation.
  Processing changes update the live socket's shared settings without rebinding. Already prepared
  results retain their interpretation through later settings changes, disable, and retirement.
  Only destruction abandons the destination; destroy/undo clears queued runtime input instead of
  resurrecting it in a fresh worker generation.
- Script callback receipt delivery moved from HTTP into app `script_api/callback_delivery.rs` and
  is reused by UDP. Each event retains its own publication receipt; rejection retries only an
  unpublished event. UDP emits prepared callbacks in FIFO order, then admits its prepared values
  to the existing bounded graph cursor. It never reparses received bytes or builds byte arrays
  in the node callback. Engine wake deliveries gate subsequent turns on their aggregate receipt.
- Golden IO passed **33 tests**, including reply/exit publication, panic/disconnect, coalesced
  deadlines, capacity recovery, background final drop, blocked cleanup, and destructor-panic
  recovery. The UDP baseline failed because its rule requested 120 Hz. The repaired owning suite
  passed **18 tests**, including passive live input, exact callbacks, processing generation fencing,
  same-socket updates, callback-publication retry, graceful sends, rebind/backoff, and destroy/undo
  stale-input rejection. Two additional maximum-input/passive-retry regressions are running.
  Strict root `cargo clippy --locked -p golden_io -p Chataigne2 --all-targets -- -D warnings`
  passed before those last two fixture additions; rerun its test targets at final closeout.

Final UDP source passed **20 tests**, including the maximum raw worker reply and passive retry
after a controlled occupied-port failure. The preservation run passed concrete streams **41**,
HTTP **36**, Sound Card/null **36**, and OSC **18**. Two Sound Card timer-only cases moved into
Golden IO coverage rather than disappearing; its engine-adapter timer test remains app-owned.
The shared stream run exposed **one failure of 44**: adding the disposer header after preparation
could exceed the 16 MiB cap at a tight boundary. Admission now reserves that header before
acceptance and charges it once throughout the payload lifetime; the owning rerun passed **44/44**.
Final-source preservation/lint/workspace/native closeout follows this correction.

Final-source owning evidence after the header correction: shared streams **44**, concrete
streams **41** (including all **20 UDP** cases), HTTP **36**, Sound Card/null **36**, and OSC **18**
passed using the rebuilt app test executable. Together with Golden IO **33**, this is **208
owning/preservation tests**; UDP is included in concrete streams and not double-counted.
Root strict IO/app Clippy and `cargo check --locked --workspace --tests` passed. Controlled
HTTP failures, native capacity pressure, occupied-port recovery, and the existing TCP fixture's
default-port collision emitted diagnostics without failing those owning checks.

Native closeout hit a link failure with **496 unresolved anonymous MSVC/LLVM symbols** in the
app artifact, while the app unit-test executable linked and passed. As with batch 8's earlier
failure, an app-package-only `cargo clean -p Chataigne2 --profile dev` and fresh native build
were used to distinguish stale artifact behavior from a source/link defect. No runtime
source workaround, global dependency-cache removal, or user-process interruption is involved.
The fresh `cargo build --locked -p Chataigne2 --bin Chataigne2` then **passed in 1m55s**.
The recurring incremental/native artifact behavior remains P12 tooling qualification evidence;
the clean build passes, while the stale-artifact reproduction has no source-level diagnosis yet.

Batch 10 closeout: root and Golden Core `cargo fmt --all` and their `--check` runs passed;
maintained-doc Prettier and `git diff --check` passed. All owned test/check/build jobs are terminal.
The required formatter also normalized the app module coordinator and missing helper newlines;
those changes have no intentional behavioral effect. Native checks use Windows/default features; endpoint tests use
controlled loopback and null audio only. Physical devices, other platforms, and full-product load
remain unqualified.

Separate concurrent UI work appeared after validation in `packages/golden-ui/components/app/`
(`AppHeader.svelte`, `AppFooter.svelte`, and new `EngineStatus.svelte`). It is preserved unstaged
and excluded from this milestone. The successful native build and source-size count describe the
verified milestone before those edits, not the newer combined working tree.

Focused passive-input/configuration/lifetime verification is complete. **Return to High requested;
actual effort unchanged/unobservable** because no supported control is exposed. P03 remains
**IN_PROGRESS**. In particular, `udp/mod.rs::queue_send_request` still performs synchronous output
collection and per-output packet admission; partial acceptance can precede a combined error. Audit
that outbound path before extending the passive-worker pattern to remaining stream modules.
Engine-retained event/history payload copies have separate lifetimes and are not covered by the
new stream disposal pool's budget. P04-P18 retain their prior implementation states.

P03 batch 11 resumed on `main` at delivered `e59ad7d5430e2649926fe6ece4e3e6c20c4a9396`.
Previous goal turn: **progress**, with batch 10 verified, committed, and pushed. The three
separate header/footer UI paths remain untouched. App-owned UDP owns frozen destination plans
and native send policy. **XHigh requested; actual effort unchanged/unobservable** for atomic
fanout admission and native cursor/retirement ownership; no supported effort control is exposed.
First repair replaces packet-per-destination queueing with one weighted whole-command envelope
and a bounded native destination cursor. Actor-side output-tree collection remains a separate
read-path repair; this batch does not qualify that traversal or all of P03.

Batch 11 implementation and owning evidence:

- `udp/outbound.rs` owns an immutable-after-admission command: one moved datagram, ordered
  destinations, outgoing-log policy, and cached allocation weight. The 1 MiB reservation includes
  payload capacity, allocated destination slots, and hostname capacity. Payloads over 65,507 bytes,
  hosts over 1024 bytes, oversized plans, and empty plans reject before native admission. A failed
  destination append invalidates the builder, so its earlier valid prefix cannot be submitted.
- Script/custom-event sends construct the entire plan before one native mailbox admission.
  Queue pressure rejects that command as a whole. The old per-output payload clone/admission loop
  and its partly-accepted combined-error result are removed. Native delivery remains best effort:
  a destination failure reports its index/count and continues later destinations without replay.
- The native worker retains one command cursor outside the 256-item / 1 MiB mailbox, adding at
  most one further 1 MiB envelope. A turn sends at most 16 destinations across command boundaries
  and receives at most 16 packets. Graceful closure finishes the current cursor before later
  commands and socket release. Terminal receive failure closes admission and uses the same cursor
  to drain accepted sends. Destruction retains explicit cancellation semantics.
- Outgoing logs now describe successful native sends. Their first-1024-byte prefixes are built
  on the worker and published through the existing weighted, four-event owner drain; the node
  no longer formats the complete datagram once per output.
- Six new tests cover a 10,000-destination plan retaining the original maximum-datagram allocation,
  exact capacity accounting, poisoned plan rejection, full-mailbox whole-fanout rejection,
  40-destination FIFO retirement spanning native turns, and an IPv6-destination failure on an IPv4
  socket preserving prior/later send multiplicity and log outcomes. UDP passed **26 tests**;
  the full concrete-stream filter passed **47** (including UDP), and shared streams passed **44**.
- Initial `cargo check --locked -p Chataigne2 --tests` caught two stale packet-loop references
  during cursor integration; both were repaired before test compilation. The subsequent
  `cargo test-fast --locked -p Chataigne2 module_modules_protocol_stream_udp -- --quiet` built the
  app test executable, but its companion native binary failed with **148 unresolved MSVC/LLVM
  anonymous/internal symbols**. That command is **FAILED**. Direct execution of its successfully
  linked test binary `target/debug/deps/Chataigne2-c47f0ddae7812c83.exe` passed the filters above.
  Strict Clippy, workspace compilation, and fresh-native artifact closeout are still pending.
- Concurrent footer UI work was committed separately as `3b4bdc7b`; preserve that parent. The
  updated size scan includes it: **1406 source files / 42 oversized**, with three new outbound
  model/test files and one separately committed `EngineStatus.svelte`; no new size exception.

Actor-side `collect_outputs_recursive`, output-ID accumulation, and hostname extraction remain
open read-path work. The bounded native envelope does not qualify their scan, transient storage,
or cleanup costs. Do not replace that audit with an arbitrary small output count that prevents
large graphs, nor pin unbudgeted full snapshots per queued command. TCP/WebSocket/serial worker
ownership, the wider module audit, and generator elapsed-time accounting remain open after it.

Final batch 11 validation (Windows/default features, current source plus separate parent `3b4bdc7b`):

- Direct app test filters passed **47 concrete-stream tests** (including **26 UDP**) and **44 shared
  stream tests**, **91 distinct tests** total. The UDP filter also passed separately and is not
  double-counted. All six new outbound cases passed on controlled loopback; no physical endpoints
  were opened and no other platform or full-product throughput qualification is claimed.
- `cargo clippy --locked -p golden_io -p Chataigne2 --all-targets -- -D warnings` passed (41.87s).
  `cargo check --locked --workspace --tests` passed (47.88s, including its build-lock wait).
- After the failed incremental companion link, `cargo clean --locked -p Chataigne2 --profile dev`
  removed only app artifacts (2809 files / 4.9 GiB). The fresh
  `cargo build --locked -p Chataigne2 --bin Chataigne2` **passed in 2m12s**. No source workaround,
  dependency-cache removal, or user-process interruption was used. Recurring anonymous-symbol
  incremental failures remain P12 tooling evidence; the successful fresh build qualifies this batch.
- Root and Golden Core `cargo fmt --all` / `--check`, maintained-doc Prettier, and `git diff --check`
  passed. All owned jobs are terminal. The separate UI commit remains preserved as the parent.

Focused whole-command admission/cursor/retirement verification is complete. **Return to High
requested; actual effort unchanged/unobservable** because no supported control is exposed.

Commit and push this verified milestone on `main`. Keep P03 **IN_PROGRESS** and resume the
actor-side output-tree preparation/cleanup audit before the remaining module and generator work.

P03 batch 12 resumed at clean delivered `95c64618376b6835841d8800ff1cbbaba0c72f1e` on `main`,
matching `origin/main`. Previous goal turn: **progress**, with batch 11 verified and delivered.
No owned jobs remain live at resume. Prepared output plans need a revision checked against
applied state before callbacks run; app callback-only invalidation would admit stale destinations
after a parameter edit followed immediately by a script send. Generic subtree change identity
belongs in Golden Engine, with UDP target policy and plan ownership in the app. **XHigh requested;
actual effort unchanged/unobservable** because no supported control is exposed.

Batch 12 ownership and intermediate evidence:

- Golden Engine now exposes opaque, engine-lifetime `SubtreeRevision` identities through process
  snapshots. Semantic change targets feed project dirty tracking and local revision updates in
  one pass. Move targets include both parents; deleted-node entries are removed. Opaque authored
  publication and uncaptured edit callbacks mark their target directly rather than depending on
  UI events; script configuration uses the same path. Snapshots copy the index once when already
  projecting the tree, then folds descendant revisions iteratively in linear time. Mutation marks
  only changed nodes, avoiding both ancestor walks and whole-map cloning. Parameter overlays use a fresh
  revision namespace; unchanged clones preserve identity. Ancestor enable state stays explicit.
- UDP now caches an immutable, pooled destination plan under its root and subtree revision.
  Script/custom-event sends check applied state before admitting a command. Repeated sends retain
  the shared plan and move one datagram; they do not rescan outputs or copy target strings. Native
  commands retain the old plan across later edits. Cache replacement, disable/destroy, and queue
  rejection cannot perform final target cleanup on the actor; the existing payload pool owns it.
- Cold preparation follows folder sibling/parent links with constant auxiliary space and a
  whole-snapshot node bound, eliminating recursive stack growth and output-ID accumulation.
  Host strings are length-checked while borrowed before trim/copy. Failed preparation is not
  cached, allowing unchanged settings to recover after pool pressure. Cold traversal and failed
  builder disposal still run on the actor and remain an explicit P03 repair.
- Initial core checking passed. Three initial revision tests passed before the subsequent opaque
  and metadata coverage was added. Initial app/test checking passed with one obsolete unused
  `child_string_param` helper warning; the unused helper was removed. Final engine and UDP tests
  are running. New app fixtures cover 20,000 nested folders, cache storage reuse, invalid-host
  recovery, and immediate endpoint edits preserving an already-accepted send's interpretation.

Do not qualify a cold builder by running a small cursor against a fresh full-tree snapshot on every
turn: that creates graph-size work per slice and can become quadratic. Finish its preparation and
failure cleanup with an explicitly bounded public read/runtime ownership boundary. The remaining
module/channel and generator elapsed-time audits are still required; P03 is not complete.

Before the final constant-time mutation / iterative snapshot-fold repair, the full engine run
passed **526 unit tests**, **8 integration tests**, and **7 doctests**; **3 existing unit tests
were ignored**. Final-source reruns include an additional 20,000-node deep revision-fold fixture.
That final core rerun and the owning UDP run are live; no final acceptance claim yet. The source
scan is **1412 files / 42 oversized**, with six new focused sources and no new size exception.

The final revision-fold source passed **527 engine unit tests**, **8 integration tests**, and
**7 doctests**, with the same **3 ignored** manual cases. The first UDP run passed 28/29: its
native failure fixture counted logs after only one four-event drain, so an interface-discovery
event could defer one expected log. The fixture now drains bounded turns to disconnection; the
owning rerun passed **29/29**. Concrete streams **50**, shared streams **44**, HTTP **36**,
Sound Card/null **36**, OSC/OSCQuery **25**, and the first-formula-tick cache regression **1**
passed; enumerating their test names confirms **192 distinct app tests**, with no overlap.
Strict engine/IO/app Clippy and workspace/test checking also passed before the final admission
preflight addition below.

Final ownership audit found that unavailable modules could prepare a plan before discovering
closed native admission. `PendingSender::is_disconnected` and
`BoundedWorker::is_accepting_commands` now expose cheap, explicitly non-reserving hints. UDP checks
them before preparation, releases cached data on closure/disconnection, and keeps its reusable
plan under capacity pressure. This prevents failed/retiring modules from pinning shared payload
slots. Existing IO fixtures now assert native closure before reply drain, cancellation/graceful
close, and full-versus-closed behavior; all **33 IO tests passed**. One further UDP regression
checks that unavailable native admission retains no plan. Final app/lint/workspace/native checks
are running or pending after this small IO/app addition; core engine source remains unchanged
since its full 542-test pass.

Final admission-preflight source passed **30 UDP tests**, including the new unavailable-transport
case, and strict `cargo clippy --locked -p golden_engine -p golden_io -p Chataigne2 --all-targets
-- -D warnings`. Final direct app preservation filters passed concrete streams **51**, shared
streams **44**, HTTP **36**, Sound Card/null **36**, OSC/OSCQuery **25**, and initial formula cache
**1**: **193 distinct app tests** (UDP is included, not counted twice). With engine **542** and IO
**33**, this batch records **768 passing tests** plus the same three ignored engine measurements.
Workspace/test compilation and native build closeout follow; no live-job completion is assumed.

Final `cargo check --locked --workspace --tests` passed (11.20s). Native default compilation
then reproduced the known incremental MSVC/LLVM anonymous-symbol failure, this time **204
unresolved externals**; that command is **FAILED**. With all owned jobs terminal and no live
native app/compiler observed, app-package-only `cargo clean --locked -p Chataigne2 --profile dev`
removed **4300 files / 7.0 GiB**. A fresh native build is running. No runtime source workaround,
dependency-cache deletion, or user-process interruption was used. Keep the recurring incremental
artifact defect open under P12; do not claim native success before the fresh command completes.

The fresh `cargo build --locked -p Chataigne2 --bin Chataigne2` then **passed in 2m23s**.
Batch 12 closeout: root and Golden Core `cargo fmt --all` / `--check`, maintained-doc Prettier,
and `git diff --check` passed. All owned jobs are terminal. No source changes followed the final
admission checks; the app-package clean removed build artifacts only. Native validation uses
Windows/default features; endpoint fixtures use controlled loopback, missing serial endpoints,
and null audio. The existing default TCP fixture port collision and controlled injected errors
were diagnostics in passing suites, not exclusive-port/device qualification. Other platforms,
physical device routes, ignored measurements, and full-product scale remain unqualified.

Focused revision/cache/admission ownership verification is complete. **Return to High requested;
actual effort unchanged/unobservable** because no supported control is exposed. Commit and push
the verified batch on `main`. P03 remains **IN_PROGRESS** for cold preparation, failed-builder
cleanup, and the wider module/channel/generator audit. P04-P18 retain their implementation states.

P03 batch 13 resumed at clean delivered `a617d313cb3e70191d53033b1ea13921e5f47201` on `main`,
matching `origin/main`. Previous goal turn: **progress**, with batch 12 verified and delivered.
No owned jobs remain live at resume. Cold worker preparation needs a byte-budgeted snapshot
lease whose final owner stays off the actor, including ordinary engine-held Arc clones. Golden
Engine owns allocation accounting and a canonical snapshot lease pool; Golden IO owns reusable
shared-Arc lifetime mechanics. UDP preparation and target policy remain app-owned. **XHigh
requested; actual effort unchanged/unobservable** because no supported control is exposed.

### P03 batch 13: native cold UDP preparation and shared snapshot lifetime

- Golden IO can retain an existing immutable Arc, deduplicating its reservation without wrapping
  away ordinary raw owners. Its disposer alone checks external releases every 100 ms, keeps item
  and byte capacity through final cleanup, and survives the last pool handle. New regressions
  verify deduplication, original-Arc rejection, ordinary-owner capacity retention, and off-consumer
  final destruction. A final ownership review also replaced the collector's strong-count-only
  destruction with an atomic unwrap claim: a concurrent weak-reference upgrade returns the pool
  owner for retry instead of moving final destruction onto a consumer. A direct regression covers
  both a rejected claim and weak invalidation after success. This generic correction postdates the
  first app pass; final validation must use the corrected IO implementation.
- Golden Engine owns cached conservative allocation accounting for snapshot records, dynamic
  parameter/control/constraint/presentation/script/dashboard data, spare capacities, aliases, and
  revision indexes. Snapshot clones share overlays as well as base storage. The canonical
  384-allocation / 1 GiB snapshot lease reserves the outer view and each shared allocation, so
  ordinary clones cannot move final shared-data cleanup back onto the actor. No protocol or
  persistence format changed; these reservations do not claim whole-process RSS qualification.
- UDP submits preparation and send as one bounded native envelope. The first admitted command
  owns a frozen snapshot lease and constant-space traversal cursor; matching revisions share its
  pending or completed ticket. Preparation yields after 64 traversal steps, checks cancellation
  per step, validates borrowed hosts before copying, and seals the entire plan before sending.
  Builder failure and cancellation cleanup run on the worker. Async errors truthfully report no
  output sent; native setup failure resolves every admitted ticket. Transient failures may retry.
  Cache installation follows successful native admission, preventing stranded pending tickets.
- Pending envelopes reserve a conservative plan bound (at most the command budget less payload
  headroom); prepared envelopes charge actual retained capacity. Snapshot storage has its separate
  canonical reservation. The existing 256-item / 1 MiB command mailbox, single extra native
  cursor, FIFO fanout, graceful retirement, passive wake/recovery, and reply limits remain.
  Default output creation now checks only whether its manager is empty, preserving user folders
  without an actor-side search.
- New controlled tests cover a 20k-deep tree, bounded native traversal, cancellation, pending-ticket
  reuse, invalid preparation after blocked setup, and accepted old-configuration ownership while
  a replacement envelope is rejected. Engine tests cover dynamic spare capacity, COW overlays,
  every variable-sized DTO group, and a lease of a 50k-record immutable view. This is deterministic
  boundary evidence, not full-product scale or native interaction qualification.
- Intermediate compile failures were corrected: an accounting match initially assumed a widget
  enum was Copy; fixture migration left stale helper calls; a request destructure omitted
  `frame_kind`. Missing public error-field docs were also fixed. The subsequent combined
  engine/app test compilation passed. These diagnostics do not establish the separate final gates.
- PASSED `cargo test-fast --locked -p Chataigne2 --bin Chataigne2
module_modules_protocol_stream_udp`: **34/34**. Final app preservation filters on that exact
  binary passed concrete streams **55** (including UDP), shared streams **44**, HTTP **36**,
  Sound Card/null **36**, OSC/OSCQuery **25**, and initial Formula cache **1**: **197 distinct app
  tests**. Controlled failures and an occupied default TCP fixture port were diagnostics in passing
  suites. No physical device or GUI automation was used.
- Source inventory: **1416 files / 42 oversized**, no new exception. Runtime source is frozen for
  final full engine/IO, strict Clippy, workspace/test, and native build checks; those commands are
  pending/running and are not yet acceptance evidence. The user's Cargo watcher was left running;
  validation waited for its native compiler to release the shared build lock.

The optimized `cargo test --locked -p golden_engine -p golden_io` completed successfully:
engine **530 unit + 8 integration + 7 doctests**, with **3 ignored**, and IO **35 tests**. This
binary predates the atomic ownership-claim addition; the fresh dev-profile engine/IO and app
runs below must qualify that final change. `cargo check --locked --workspace --tests` passed.
The first strict Clippy run rejected the new disposer callback's complex function type; named
erased-payload/callback types replace it. That lint rerun and the native/default build remain live.

The first native/default build passed without cache cleaning; a second UDP run also passed 34/34.
The final ownership review tightened atomic claims further: claim/reinsertion happens under the
registry lock, with destruction deferred outside it. This prevents weak revival plus concurrent
registration from creating a duplicate pool owner while a claim is in flight. The direct regression
also asserts that claiming alone never invokes a destructor. This last synchronization change
requires the fresh core/IO, app, lint, and native checks below; earlier passes are preserved evidence.

Final synchronization source passed `cargo test-fast --locked -p golden_io -p golden_engine`:
engine **530 unit + 8 integration + 7 doctests** and IO **36 tests**, with the same **3 ignored**
engine measurements. Focused shared-snapshot/Arc ownership verification is complete. **Return
to High requested; actual effort unchanged/unobservable** because no supported control is exposed.
The next Clippy pass found an engine revision-lease result type needing an alias and a test's
default-then-field initializer; both were mechanically corrected without behavior changes.
Routine final app/native/lint closeout remains. No further runtime design changes are planned.

The final `cargo test-fast --locked -p Chataigne2 --bin Chataigne2
module_modules_protocol_stream_udp` passed **34/34**. All six preservation filters were rerun
against that rebuilt binary and passed the same **197 distinct app tests**. Together with
engine **545** and IO **36**, this milestone records **778 passing tests**, plus **3 ignored**
manual engine measurements; overlapping profiles/UDP subsets are not added again. The full
engine/IO pass preceded only the revision result-type alias and equivalent test initializer
cleanup; final Clippy recompiles those sources. The final native build and lint remain live.

Final `cargo clippy --locked -p golden_engine -p golden_io -p Chataigne2 --all-targets -- -D
warnings` and `cargo build --locked -p Chataigne2 --bin Chataigne2` **PASSED**. No native-link
cache clean or runtime workaround was needed in this batch; the prior recurring artifact issue
remains open under P12. Workspace/test compilation passed before the mechanical alias/initializer
cleanup; final strict lint and native compilation cover those sources. All owned jobs are terminal.
Windows/default features were used. Controlled loopback, missing serial endpoints, and null audio
fixtures do not qualify physical devices, other platforms, ignored measurements, or full-product scale.

Commit and push this verified milestone on `main`. P03 remains **IN_PROGRESS** for the native
reply-backpressure fairness and broader module/channel/generator audits. P04-P18 retain their
implementation states. Preserve the user's running watcher and all earlier delivered work.

Closeout formatting evidence: root and Golden Core `cargo fmt --all` / `--check`, maintained-doc
Prettier, and `git diff --check` passed. Source inventory remains **1416 / 42 oversized**. No new
size exception, generated protocol drift, dependency change, or unrelated source edit is included.

P03 batch 14 resumed at clean delivered `df0c787621162dead9623751d989478cf09198ae` on `main`,
matching fetched `origin/main`. Previous goal turn: **progress**; batch 13 was verified, committed,
and pushed. No owned jobs are live. Golden IO owns nonblocking weighted reply admission and its
queue diagnostics; the UDP adapter owns native per-source reply continuations and fairness.
Current source blocks the socket loop inside `publish_weighted` when replies fill, including
unsolicited input/interface results. That can strand admitted output behind input backpressure.
**XHigh requested; actual effort unchanged/unobservable**, with no supported control exposed.

### P03 batch 14: duplex UDP reply backpressure

- Golden IO exposes one-attempt weighted reply admission returning the original event on `Full`
  or disconnection, plus read-only event queue metrics. Sequential blocking publication retains
  its existing cancellable behavior. All **38 IO tests passed**, including full/oversized ownership,
  independent command service under reply pressure, cancellation, and the previous lifetime cases.
- A controlled native regression with one reply slot filled by input reproduced the defect before
  the UDP change: the admitted cold 20k-deep output command timed out at its loopback receiver.
  `cargo test-fast --locked -p Chataigne2 --bin Chataigne2
full_input_replies_do_not_strand` was **FAILED** as expected for that baseline; cancellation
  still terminated its worker. This is an observed starvation case, not only a source finding.
- The app-owned adapter now keeps one pending input, output-outcome, and interface result in a
  shared FIFO. Local bounds are 32 MiB, 64 KiB, and 64 KiB plus the fixed event envelope, respectively.
  Input/discovery backpressure pauses only those producers. Native cold preparation and successful
  unlogged sends can progress; a pending output outcome pauses the already-advanced destination
  cursor. Queue retry never repeats a native send. Retirement drains all results before exit;
  cancellation abandons explicitly cancelled work without joining. Oversized details report a
  bounded failure instead of stranding later results. Discovery errors are bounded before caching.
- Tests now cover exact input resumption, pending-source FIFO reuse, logged fanout under pressure,
  retirement, cancellation, and oversized-result recovery. `cargo check --locked -p Chataigne2
-p golden_io --tests` passed the adapter change. Final runtime and strict lint checks are live;
  no final acceptance claim yet. No engine, protocol, persistence, or UI source changed.

Final `cargo test-fast --locked -p Chataigne2 --bin Chataigne2
module_modules_protocol_stream_udp` **passed 39/39**, including the previously failing cold-send
case, exact input resumption, source FIFO, logged fanout/retirement, cancellation, and oversized
reply recovery. Strict `cargo clippy --locked -p golden_io -p Chataigne2 --all-targets -- -D
warnings` and `cargo check --locked --workspace --tests` **passed**. Direct preservation filters
on that binary passed concrete streams **60** (including UDP), shared streams **44**, HTTP **36**,
Sound Card/null **36**, OSC/OSCQuery **25**, and initial Formula cache **1**: **202 distinct app
tests**, or **240** with the **38 IO** tests. Prior full engine passes are not added to this batch.

The default native build **failed** with the known incremental MSVC/LLVM anonymous-symbol
artifact defect, **74 unresolved externals**. After all owned checks were terminal and no live
native app/compiler was observed, the resolved Cargo target was verified inside this workspace.
App-package-only `cargo clean --locked -p Chataigne2 --profile dev` removed **6849 files / 9.6 GiB**.
A fresh `cargo build --locked -p Chataigne2 --bin Chataigne2` is running; no native pass is assumed.
No user process was interrupted, dependency cache removed, or runtime workaround introduced.
The recurring artifact issue remains P12 work. Source inventory: **1419 / 42 oversized**.

Focused duplex reply/admitted-send ownership verification is complete. **Return to High requested;
actual effort unchanged/unobservable** because no supported control is exposed. Only routine
native-build/documentation/delivery closeout remains for this batch; P03 is still **IN_PROGRESS**.

The fresh native build **PASSED in 1m53s**, completing the Windows/default-feature build gate.
All owned jobs are terminal. Source stayed unchanged after the final strict lint, workspace/test,
and runtime passes; the package clean removed build artifacts only. `cargo test-fast --locked
-p golden_io` passed **38 tests**. The **202 distinct app** preservation cases make **240 current
passing tests**, with no duplicated UDP/profile totals. Full engine/runtime/platform/device/scale
qualification is not inferred from these filters. The fixtures used controlled loopback, missing
serial endpoints, and null audio; occupied default-port/injected-error diagnostics occurred in
passing suites. No physical device or GUI automation was used.

Root and Golden Core `cargo fmt --all` / `--check`, explicit app UDP `rustfmt --edition 2021
--check`, maintained-doc Prettier, and `git diff --check` passed. No new source-size exception,
protocol-generation drift, dependency change, or unrelated source edit is included.
Commit and push the verified batch on `main`, then resume TCP/WebSocket/serial worker ownership,
the broader module/channel audit, and generator time accounting. P03 remains **IN_PROGRESS**;
P04-P18 retain their states, including the recurring P12 native incremental-artifact gap.

P03 batch 15 resumed at clean delivered `0196c0f88fbfa3306996ade41f692ddd00ff32f1` on `main`,
matching fetched `origin/main`. Previous turn: **progress**; batch 14 was verified and delivered.
No owned jobs are live. TCP client currently resolves DNS on the caller, owns an unbounded command
channel, joins on stop, and uses `write_all` on a nonblocking socket without a resumable offset.
The app owns TCP connection policy and native cursor/generation semantics; Golden IO supplies
bounded lifetime/admission/cancellation primitives. Shared stream reply continuations will be
reused by TCP and UDP. **XHigh requested; actual effort unchanged/unobservable**, with no supported
control exposed. TCP actor-side input preparation/passive service remains an explicit following
step at resume; passive owner service is now included, while worker-side input preparation remains
open. This batch establishes the native boundary and graceful generation ownership first.

### P03 batch 15: TCP client native ownership and completion service

- TCP client now uses Golden IO's bounded worker with 16 global native lifetimes, 256 commands /
  1 MiB, one extra 1 MiB write cursor, and 64 replies / 32 MiB; owner drains four events. Endpoint
  resolution/connect/reconnect, health checks, writes, outgoing logs, and native cleanup stay on
  the worker. Stop cancels without a join; blocked setup retains its lifetime reservation.
- Commands capture their connection generation. Partial writes retain their suffix and submit
  at most 64 KiB per step; `WouldBlock`/interruption never restart a prefix. A five-second absolute
  command deadline fails stalled output, and connection failure explicitly reports written bytes
  without replaying a command on a replacement socket. Work turns bound command steps and reads.
- Shared app stream reply continuations and wake/deadline adapters serve TCP and UDP. UDP keeps
  its app-owned limits/error policy; no private compatibility wrapper is retained. Its output
  plan sources were also formatted with the repository formatter. TCP nodes use passive owner
  service, preserve one retiring generation across endpoint edits/disable, and consume outgoing
  log completion events instead of formatting full payloads or declaring sends on the actor.
- Initial engine/app test compilation and strict app Clippy passed. The first concrete stream
  run passed **66/67**; root TCP disable timed out waiting for retirement. Inspection found that
  Golden Engine unconditionally skipped disabled-node worker callbacks. This is an owning engine
  contract gap, not an endpoint timeout. A public default-false `worker_wake_when_disabled` hook
  now permits bounded accepted-completion service. Enum dispatch forwards it for built-ins and
  consumer variants; TCP and UDP opt in only while lifecycle/accepted work remains. A regression
  checks default suppression, explicit opt-in, ending the opt-in, timed-clock separation, and
  removal fencing. Native/module tests also cover disabled graceful draining and endpoint handoff.
- Final core/app compilation passed the hook change. Full engine/IO and concrete stream runs are
  pending/live; no final verification claim yet. TCP worker-side input parsing, callback preparation,
  and grammar/connection-generation input fencing remain the next P03 boundary. TCP server,
  WebSocket, serial, Sound Card/channel audits, and generator time accounting remain open.
- Source scan: **1425 files / 42 oversized**, no new exception. Runtime/test files remain split
  into focused modules; no wire format, persistence schema, dependency, or UI source changed.

Retirement admission is now explicit at the generic IO boundary: `command_admission_closed`
observes a closed producer before its accepted prefix is drained. TCP/UDP stop new input and
reconnect/discovery work in that state, while retaining admitted sends/results. A connector that
returns after closure cannot install a new TCP connection. The existing IO command-turn regression
also checks that closure is visible before either queued command is consumed. This additive IO
query postdates the initial core run's compilation; final IO/app checks must cover it. No existing
Golden IO method changed behavior in this step.

After the disabled-completion hook, concrete streams **passed 70/70**, including TCP root
disable/re-enable, disabled draining of a saturated admitted prefix, endpoint handoff, idle
passivity, partial-write/deadline/generation cases, and all UDP regressions. The engine unit
suite passed **531** with **3 ignored**, and its **8 integration tests** passed; IO passed **38**.
The same command's **7 doctests failed** with `E0460` because the IO source changed during the
validation run and its newer artifact no longer matched the engine artifact. This was a
validation sequencing error; a source-settled doc-only rerun is queued. No doctest pass is assumed.

A final native-routing review added snapshot-based TCP endpoint admission, checked before moving
the payload to the worker. A parameter edit followed by a script call in the same batch must
reject pending configuration instead of sending on the prior endpoint. The saturated endpoint
handoff regression now includes that rejected call and still requires the old admitted prefix
to arrive exactly once at its original peer. This app-only change postdates the 70-test run;
final stream/lint/native checks must use the updated source. Runtime source is now frozen.

The source-settled IO rerun passed **38/38**, including pre-drain admission closure. The first
native/default build failed with the recurring incremental MSVC/LLVM artifact issue, **19
unresolved externals**. Final source still needs a fresh native build after the current app/doc/lint
jobs finish. No cache cleanup, user-process interruption, or runtime workaround has been performed
for that failure yet. Workspace/test compilation and the prior strict engine/IO/app Clippy run
passed before the final applied-snapshot admission addition; the latest lint rerun is pending.

Final applied-snapshot source passed **70/70 concrete stream tests** and strict
`cargo clippy --locked -p golden_engine -p golden_io -p Chataigne2 --all-targets -- -D warnings`.
The doc-only settled rerun `cargo test-fast --locked -p golden_engine --doc` passed **7/7**;
with **531 engine unit + 8 integration**, that is **546 engine tests**, plus the same **3 ignored**
manual measurements. Source-settled IO passed **38/38**. App preservation filters passed shared
streams **44**, HTTP **36**, Sound Card/null **36**, OSC/OSCQuery **25**, and initial Formula cache
**1**, making **212 distinct app tests** with concrete streams. Total current evidence is **796
passing tests**; overlapping profiles and UDP subsets are not counted twice.

All owned tests/lint/check jobs were terminal, and no live native app/compiler was observed.
The resolved Cargo target was verified inside the workspace before app-package-only artifact
cleanup. A fresh default native build is now running. The user watcher remains untouched; no
dependency cache was removed and no runtime workaround was introduced. Native success is pending.

The app-only cleanup command `cargo clean --locked -p Chataigne2 --profile dev` removed **4955
files / 8.1 GiB**. Focused native cursor/generation, applied-snapshot admission, and disabled
completion verification is complete. **Return to High requested; actual effort unchanged/unobservable**
because no supported control is exposed. Routine native-build/documentation/delivery closeout
remains; TCP worker-side input preparation is still the next dependency-ready P03 task.

The fresh `cargo build --locked -p Chataigne2 --bin Chataigne2` **PASSED in 2m14s**. The final
`cargo check --locked --workspace --tests` against settled source **passed in 32.97s**. All owned
jobs are terminal. Root and Golden Core formatting, explicit formatting of touched app modules,
maintained-doc Prettier, and whitespace checks passed. Source inventory remains **1425 / 42
oversized**, with no new exception. The package clean changed build artifacts only.

This milestone records **796 distinct passing tests** plus **3 ignored** manual engine cases;
earlier failure evidence is preserved above rather than relabelled. Validation used Windows/default
features, controlled loopback/missing-serial endpoints, and null audio. Occupied default-port and
injected-error logs were diagnostics within passing suites. No physical-device, other-platform,
ignored-measurement, or full-product scale qualification follows. No GUI automation or user-process
interruption was used. Commit and push this verified batch on `main`, then continue the remaining
TCP input boundary. P03 remains **IN_PROGRESS**; P04-P18 retain their states, including the P12
incremental native-link artifact issue.

### P03 batch 16: TCP worker input preparation

Resumed clean delivered `1663106e9742fc67dc900295fb5674ddddd8c108` on `main`, matching fetched
`origin/main`; no owned jobs are live. Shared stream input preparation/delivery belongs in app
common streaming code; TCP connection/parser generations remain transport policy. This batch
moves TCP parsing, callback JSON, and bounded input logs off the actor and preserves receipt-owned
delivery. Partial frames must not span grammar/processing or connection generations. Requested
**XHigh; actual effort unchanged/unobservable** because no supported control is exposed.

Shared UDP preparation/delivery moved into app common streaming modules and TCP now consumes the
same immutable retained input envelope. Live input settings update independently of socket identity;
TCP partial frames are fenced by parser and connection generations. Owner callbacks use individual
receipts, a bounded four-event batch, and snapshot-free callback service. The stale TCP warning
lookup now targets the bound connection folder. The first check found one old UDP test type after
extraction; it was corrected. Five native generation/configuration/retirement regressions and three
owner delivery/passivity/destroy regressions were added; the first settled stream test job is live.
No validation pass was claimed at that point. The full P03/module/channel/generator work remains open.

The settled stream run passed **77/77** in **2.39s** after **2m53s** compilation. This includes
five native input tests and three owner tests added here; the UDP callback receipt regression
moved to common streaming, so its count is not duplicated. Shared streaming passed **45**, HTTP
**36**, Sound Card **35** plus null-device product evidence **1**, OSC **25**, and initial Formula
cache **1**: **220 distinct app tests passed**. Direct executable filters were checked against
the compiled test inventory; unmatched/zero-test attempts were corrected and add no pass evidence.
`cargo clippy --locked -p golden_io -p Chataigne2 --all-targets -- -D warnings` passed in **49.02s**;
`cargo check --locked --workspace --tests` passed in **21.29s**. Golden runtime source is unchanged
in this batch; previous engine/IO passes remain historical evidence, not new executions.

Focused input generation/receipt work is verified. **Return to High requested; actual effort
unchanged/unobservable** because no supported control is exposed. Default native build is live;
delivery remains pending that gate. Root/Core Cargo formatting, explicit app module formatting,
maintained-doc Prettier and whitespace checks have passed against the current source.

Review also identified a distinct remaining P03 lifetime boundary: callback JSON `Arc` copies
held by routing/replay can outlive a prepared input envelope's reservation. The shared stream
pool owns final envelope disposal, while downstream event copies currently follow engine event
lifetimes. Audit byte admission/final disposal at that handoff before expanding native input to
the next transports; do not infer a routing/replay byte budget from the envelope budget. Then
continue TCP server/WebSocket/serial, Sound Card/channel ownership, and generator time accounting.

The first default native build **FAILED** with MSVC `LNK1120`: **1081 unresolved externals**,
including compiler-generated `anon.*.llvm.*` and local function symbols. This matches the recurring
incremental app-artifact failure class retained under P12; tests/lint/checks are not relabelled as
native success. All owned jobs are terminal. Verify no live app/compiler and the resolved target
inside the workspace, clean the app package's dev artifacts only, then require a fresh native pass.

No live app, Cargo, Rust compiler, or linker process was observed before cleanup; the resolved
target was verified as `D:\Projects\Dev\Chataigne2\target`. The app-only command
`cargo clean --locked -p Chataigne2 --profile dev` removed **2231 files / 4.7 GiB**. Fresh default
native build is live. Dependency caches, compiler flags, and user processes were left alone.
Fresh source-size scan (`rg --files apps crates packages`, Rust/TS/Svelte/JS/MJS) is **1614 files /
42 oversized**; all touched source stays below 1000 lines (largest TCP node: **559**). This explicit
broader scan includes more files than the earlier inventory; it does not imply new size exceptions.

The fresh `cargo build --locked -p Chataigne2 --bin Chataigne2` **PASSED in 2m02s** against the
same settled runtime source. The failed incremental attempt remains evidence above; scoped app
artifact cleanup is recovery, not a P12 fix. All owned jobs are terminal. Final root/Core Cargo
formatting, explicit app formatting, maintained-doc Prettier, and whitespace checks pass. Milestone
evidence is **220 distinct passing app tests**, strict IO/app Clippy, workspace/test compilation,
and fresh default native build. Validation used Windows/default features, controlled loopback and
missing-serial endpoints, and null audio. Physical devices, other platforms, full-product scale,
and UI/native interaction were not qualified by this batch. Commit and push this milestone on
`main`, then audit the downstream callback JSON lifetime boundary. P03 remains **IN_PROGRESS**;
P04-P18 retain their states.

### P03 batch 17: callback JSON lifetime admission

Resumed clean delivered `3166e80574cdff1d1557f851ecf808222612a59b` on `main`, matching fetched
`origin/main`; no owned jobs are live. Previous turn: **progress**. Golden IO already owns atomic
final claims of externally cloned immutable Arcs. App module script policy owns callback JSON
construction and admission, so this batch adds a canonical app callback reservation and wires
native TCP/UDP preparation through it. Payload storage must remain charged after its input
envelope/delivery is gone and through ordinary event/routing/replay clones; final JSON destruction
belongs to its disposer. HTTP's response/callback envelope boundary is a following integration,
not evidence supplied by stream-only checks. **XHigh requested; actual effort unchanged/unobservable**
because no supported control is exposed.

`NativeCallbackPayload` now measures/reserves JSON on the preparing worker and binds origin/topic
without copying its JSON on the owner. The app-owned canonical pool uses Golden IO external-Arc
retention with **16,384 allocations / 256 MiB**, separate from stream envelopes/values (**256 /
256 MiB**). Each callback admits at most **32 MiB / 262,144 values / 64 levels**, including spare
vector/string capacity. TCP/UDP preparation now uses that typed payload and cached retained weight;
the old stream-local recursive JSON estimator was removed. Callback multiplicity, names, argument
shape, replay policy, and per-publication receipts remain intact. Seven lifetime/admission tests
cover raw event copies, replay/export, failure/retry, retained spare storage, per-payload limits,
and shallow binding. The first focused test job is live; no pass is claimed yet.

The focused run passed **7/7** callback tests in **0.10s** after **2m17s** compilation. Current
Golden IO verification passed **38/38**, including atomic final claims/Weak revival, externally
cloned Arc admission, producer/pool-owner exit, destructor thread ownership, panic cleanup, and
blocked cleanup capacity. Source-settled executable preservation filters passed script API **8**
(including the same seven callback tests), concrete streams **77**, shared streams **45**, HTTP
**36**, Sound Card **35** plus null product evidence **1**, OSC **25**, and initial Formula cache
**1**. Their compiled-name sets were checked for overlap: **228 distinct app tests**, **266**
including IO. Strict IO/app Clippy and workspace/test compilation are pending/live; native build
and final delivery are still required. This does not qualify unintegrated HTTP callback ownership.

Strict `cargo clippy --locked -p golden_io -p Chataigne2 --all-targets -- -D warnings` passed in
**47.52s**. Focused lifetime/publication work is verified; **return to High requested, actual
effort unchanged/unobservable** because no supported control is exposed. Workspace/native and
routine documentation/delivery closeout remain; other callback producers keep their own open audit.

`cargo check --locked --workspace --tests` passed in **48.84s**, including queue wait. Source is
frozen for the default native build. A fresh explicit source inventory (`rg --files apps crates
packages`, Rust/TS/Svelte/JS/MJS) is **1616 files / 42 oversized**; the new native payload module
is **119** lines and its separate test module **149**. No new size exception is introduced.
Maintained-doc Prettier and whitespace checks pass. Other producer integration remains open, in
particular HTTP's retained response/body/parsed-values cleanup and worker-prepared callback binding.

The default `cargo build --locked -p Chataigne2 --bin Chataigne2` **PASSED in 1m35s**, including
queue wait, without artifact cleanup or flag changes this batch. All owned jobs are terminal.
Final root/Core Cargo formatting, explicit app-module Rustfmt, maintained-doc Prettier, and
whitespace checks pass. Evidence: **266 distinct passing IO/app tests**, strict IO/app Clippy,
workspace/test compilation, and default native build. Source inventory remains **1616 / 42**;
no new size exception, wire shape, persistence schema, dependency, or UI behavior changed.
Checks used Windows/default features, controlled loopback/missing-serial endpoints, and null
audio. Physical-device, other-platform, UI/native-interaction, and full-product scale qualification
remain separate. Commit and push this verified milestone on `main`, then integrate HTTP callback
and retained envelope ownership. P03 remains **IN_PROGRESS**; P04-P18 retain their states, including
the previously recorded P12 incremental-link artifact issue.

### P03 batch 18: retained HTTP result ownership

Resumed clean delivered `8c2c5ff866674b214670c96ee7b3a2120a01c52e` on `main`; previous turn:
**progress**, no owned jobs live. App HTTP owns response parsing, callback shape, and result policy;
Golden IO supplies retained immutable lifetimes. HTTP replies currently transfer body/parsed-value
containers and callback args to the owner. This batch retains their final cleanup off the actor,
prepares normal and graph-application-failure callbacks on the worker, and reserves callback JSON
through the canonical native callback pool. Result-admission exhaustion must produce an explicit
bounded rejection, never unbudgeted callback JSON or an automatic resend; close native admission
and report each remaining queued request without sending it. Requested **XHigh; actual effort
unchanged/unobservable**, with no supported control exposed. Broader P03 audits remain open.

HTTP now separates native `HttpOutcome` from typed retained owner replies. Bodies, request metadata,
and parsed values use a **256-allocation / 256 MiB** HTTP pool; normal, failure, and graph-application-
failure callback JSON is prepared/reserved on the worker through the existing native callback pool.
Owner selection/binding is shallow; raw callback argument vectors and owner-built failure payloads
were removed. Cached reply weights include callback reservations, container capacities, and parse
error storage. Ordinary HTTP response/JSON bounds still produce a prepared failure callback.
Result/callback resource exhaustion instead returns bounded identity/attempt feedback, closes native
command admission, and resolves the accepted suffix without further endpoint sends or unbudgeted
callback payloads. Owner warning service needs no snapshot and clears its result warning on recovery.

The first test-compilation check failed on fixture visibility and the new rejection's snapshot
match arm; both were corrected. `cargo check --locked -p Chataigne2 --tests` then **passed in
22.80s**. A final small owning pass renamed lifecycle feedback from failed to ended, since result
pressure can intentionally end a generation, and bounded parse-error log detail. Six retained/native
tests plus a node warning/recovery regression were added. The first settled HTTP test job is live;
no final pass is claimed yet. HTTP's existing periodic owner service remains a following P03 audit,
alongside TCP server/WebSocket/serial, Sound Card/channels, and generator time accounting.

The first settled HTTP run passed **39/43**; four failures were retained as evidence. One receipt
regression found that unconditional warning clearing inserted a second edit in a callback-only turn.
The large lifetime fixture retained all **257** completion envelopes in its collector, exhausting
the **256** global result slots and causing collateral parallel fixture rejections. The owning
fixture now consumes four-result batches and retains only path evidence; the production limit was
not enlarged. Warning clearing now checks committed metadata, and rejected-result warnings keep
their own application receipt before advancing (retrying only unpublished warning edits). The
second HTTP run is live against this correction; no final pass is assumed.

The corrected HTTP run **passed 43/43** in **0.23s** after **1m47s** compilation, including the
bounded collector, original callback-only receipt contract, graph failure preservation, and native
rejection/queued-suffix checks. Two focused tests were then added for warning publication discarded
by an earlier failed edit and for alternate-callback admission failing after normal callback
preparation; runtime behavior is unchanged by that addition. The final HTTP run, IO contract suite,
and strict IO/app Clippy are live/queued against settled source. No whole-suite/native success is
assumed yet. HTTP warning delivery retries only an unpublished warning and never emits a callback
outside its reservation or reissues a native request.

The **45/45** HTTP run passed in **0.23s** after **1m32s** compilation. Strict IO/app Clippy also
passed (**56.50s**, including queue wait), and IO passed **38/38**. A final queue-ordering review
then identified that warning clearing also needed an independent receipt: an earlier rejected edit
could otherwise discard the clear while a later callback retry succeeded. HTTP callback delivery
now waits for an observed clear only when a result warning exists; ordinary callback-only turns
keep their original shape. A focused earlier-failure/clear/callback regression was added, and the
**46-test** HTTP rerun is live. The prior passes precede that final runtime correction and are
not relabelled as final-source evidence. Full preservation/lint/workspace/native checks remain.

Final settled HTTP source passed **46/46** in **0.24s** after **1m55s** compilation. The strict
IO/app Clippy rerun passed in **1m10s**, including queue wait. IO's **38/38** pass is against unchanged
IO source in this batch, and supplies final-claim/Arc/disposer/retirement contract evidence. Focused
HTTP lifetime/generation/publication work is verified: **return to High requested; actual effort
unchanged/unobservable** with no supported control exposed. The broader preservation filters and
workspace/test check are live; default native build and delivery remain. Explicit source inventory
is **1618 files / 42 oversized** (same Rust/TS/Svelte/JS/MJS scope); no new exception is introduced.

Final preservation filters passed script API **8**, concrete streams **77**, shared streams **45**,
Sound Card **35** plus null product evidence **1**, OSC **25**, and initial Formula cache **1**.
With final HTTP **46**, their compiled-name sets contain **238 distinct passing app tests**; IO
adds **38**, for **276 current IO/app passes**. Earlier overlapping HTTP runs are not added again.
`cargo check --locked --workspace --tests` passed in **18.89s**. Root/Core Cargo formatting,
explicit owning app Rustfmt, maintained-doc Prettier, and whitespace checks pass. Runtime source
is settled; the default native build is live. HTTP's passive owner conversion remains the next
P03 boundary before the broader transport/module/channel/generator audit.

The default `cargo build --locked -p Chataigne2 --bin Chataigne2` **PASSED in 1m36s**. All owned
jobs are terminal; no artifact cleanup or compiler flag changes were needed this batch. Final
root/Core Cargo formatting, explicit HTTP-module Rustfmt, maintained-doc Prettier and whitespace
checks pass. Milestone evidence: **276 distinct IO/app tests** (238 app + 38 IO), final strict
IO/app Clippy, workspace/test compilation, and default native build. Earlier failures and
intermediate passes remain labelled above. Inventory remains **1618 / 42 oversized**; the HTTP
node is **891** lines, response service **123**, prepared results **238**, and their native tests
**246**, with no new size exception. No wire shape, persistence schema, dependency, or UI source
changed. Checks used Windows/default features, controlled HTTP/stream loopbacks, missing-serial
fixtures, and null audio. They do not qualify physical devices, other platforms, full-product
scale, or UI/native interaction. Commit and push this milestone on `main`, then convert HTTP's
periodic owner to worker/deadline wakeups. P03 remains **IN_PROGRESS**; P04-P18 retain their states.

### P03 batch 19: passive HTTP service

Resumed clean delivered `7836d1bf0bad9d6c4b7b0c89a2e1944d0f051587` on `main`; previous turn:
**progress**, no owned jobs live. HTTP still selects a 120 Hz compiled kernel. This batch moves
owner service to worker/deadline wakes, generalizes the app's existing stream wake adapter into
module-common code, and signals resolved HTTP generations before potentially blocked native cleanup.
Preserve HTTP's current disabled policy: accepted results stay bounded/paused until enable explicitly
re-arms the owner. Destruction must discard pending runtime input so undo cannot revive it. Script
argument helpers will be split out before the node coordinator grows past its size guardrail.
Requested **XHigh; actual effort unchanged/unobservable**, with no supported control exposed.

Review also identified an existing HTTP admission/configuration boundary for a following focused
step: script admission currently trusts the dirty flag rather than fencing an applied parameter edit
before callbacks dispatch; transport configuration reads cached handles and needs its own input-bound
audit. This is separate from notification ownership and must not be inferred fixed by passive service.

HTTP now uses passive worker callbacks and module-common `ModuleWake` notifications/deadlines;
TCP/UDP use the same moved adapter with no legacy alias. Native HTTP workers notify on result
publication, resolved/aborted generation phase, and producer exit. Phase notification precedes
potentially blocked client cleanup. Changes to transport inputs, auth enablement, or effective
enable re-arm the owner; retry deadlines replace periodic polling. Pending graph/callback work
re-arms bounded wake service, and callback-only service needs no process snapshot. Disabled
completions remain paused by the default engine policy; enable explicitly wakes the backlog.
Destroy now clears pending owner results and deadline state, fixing undo reviving queued runtime
input. Script helpers moved to a focused sibling file to keep the coordinator readable.

`cargo check --locked -p Chataigne2 --tests` passed in **19.91s**. The first HTTP run passed
**51/51** in **0.31s** after **2m05s** compilation, including five new passive/phase tests and
the original disabled/reconfigure/receipt/lifetime regressions. The undo test was strengthened
to assert that notifying the removed source fires no restored-owner callback; a settled rerun
is live. No runtime behavior changed after the first pass. Incidental formatting from recursing
through module-common was reverted through patches before that run; only owning files remain.
Full preservation, strict lint, workspace/test compilation, native build and delivery are pending.

The strengthened settled HTTP run passed **51/51** in **0.31s** after **57.03s** compilation;
the removed source fired **zero** callbacks on the restored owner. IO passed **38/38** in
**0.20s**, and strict `cargo clippy --locked -p golden_io -p Chataigne2 --all-targets -- -D warnings`
passed in **24.40s**. Preservation filters passed script API **8**, concrete streams **77**, shared
streams **45**, Sound Card **35** plus null product evidence **1**, OSC **25**, and first Formula
cache **1**. Their disjoint compiled-name sets plus HTTP contain **243 distinct app tests**; with IO,
**281** tests passed. Repeated HTTP profiles are not added twice. Source is settled. Focused wake/
lifetime work is verified: **return to High requested; actual effort unchanged/unobservable**
because no supported control is exposed. Workspace/test and default native closeout remain.

The final `cargo check --locked --workspace --tests` **passed in 19.48s** and default
`cargo build --locked -p Chataigne2 --bin Chataigne2` **passed in 1m43s**. All owned jobs are
terminal. Final root/Core Cargo formatting, explicit owning app Rustfmt, maintained-doc Prettier,
and whitespace checks pass. Source inventory is **1620 files / 42 oversized**; the HTTP coordinator
is **774** lines, script helpers **189**, shared wake adapter **35**, and passive tests **165**.
No new size exception, wire shape, persistence schema, dependency, or UI source changed. Evidence
is **281 distinct IO/app passes** (243 app + 38 IO), strict IO/app Clippy, workspace/test compilation,
and default native build. No artifact cleanup or compiler flag changes were needed this batch.
Validation used Windows/default features, controlled loopbacks/injected native gates, missing-serial
fixtures, and null audio; physical devices, other platforms, full-product scale, and UI/native
interaction remain separate qualifications. Commit and push this milestone on `main`, then finish
the HTTP applied-configuration admission and configuration-bound audit recorded above. P03 remains
**IN_PROGRESS**; broader TCP server/WebSocket/serial, Sound Card/channel and generator work remains,
and P04-P18 retain their states.

### P03 batch 20: HTTP configuration admission

Resumed clean delivered `0edb6f489d203af76c97f1788405d165dd42f057` on `main`, matching fetched
`origin/main`; previous turn **progress**, no owned jobs live. App HTTP owns authored settings and
admission. This batch reads configuration from the applied process snapshot, compares bounded
active inputs before command/script sends, and captures inputs only for successfully configured
generations. Configuration validation remains an edit-time backend operation; admission compares
borrowed values without reparsing or cloning settings. Source input and native retained-capacity
limits must be checked before allocation/cloning at their respective boundaries. Inactive auth
fields must not retain unused credentials or cause transport identity changes. Requested **XHigh;
actual effort unchanged/unobservable**, with no supported control exposed.

Implemented bounded borrowed `HttpSettings` from applied snapshots, captured only after successful
generation configuration. Endpoint/auth edits fence script/command admission before parameter
callbacks dispatch. Auth-folder metadata uses the same effective-settings comparison: disabled or
inactive auth edits and enabling mode `None` do not unnecessarily close admission. Preparation
checks source limits before URL/header parsing and retains only active auth fields. Native manager
and spawn boundaries validate capacities/ranges before configuration clone or lifetime reservation.
ACK/log path descriptions are bounded. Settings validation lives in `http/configuration.rs`, keeping
the coordinator at 711 lines; new tests live under the HTTP feature's existing `tests/` directory.

Initial `cargo check --locked -p Chataigne2 --tests` passed in **18.06s**; the initial HTTP run passed
**58/58** after **2m20s** compilation. Adding the mode-None metadata regression exposed a test-fixture
setup error: `create_http_module` deliberately disables transport, so waiting without a configuration
edit timed out (**58 passed / 1 failed**, **1m38s** compilation). Corrected that test to activate a local
configuration before exercising admission; this was not a runtime failure. Corrected HTTP run passed
**59/59** in **0.30s** after **1m25s** compilation. Preservation filters passed **192** distinct tests:
script API 8, concrete streams 77, common streaming 45, Sound Card 35, OSC 25, initial Formula 1,
and null-device Sound Card evidence 1. HTTP plus preservation totals **251 distinct app tests**.
Initial strict Clippy found a now-test-only import and a needless test-helper borrow; corrected both.
Focused verification complete; requested return to **High**, actual effort unchanged/unobservable.
Final `cargo clippy --locked -p golden_io -p Chataigne2 --all-targets -- -D warnings` passed in
**24.86s** after the import/borrow cleanup. `cargo test-fast --locked -p golden_io` passed **38/38**
in **0.20s** (7.25s including lock wait); combined with the disjoint app suites, **289 distinct tests**
passed. `cargo check --locked --workspace --tests` passed in **19.76s**. Native
`cargo build --locked -p Chataigne2 --bin Chataigne2` passed in **1m45s** including lock wait;
no linker repair or cache deletion was needed. Root/Core `cargo fmt --all`, explicit HTTP rustfmt,
maintained-doc Prettier, and whitespace checks pass. Inventory is **1622 source files / 42 oversized**;
HTTP settings and configuration tests are 259 and 323 lines. All owned jobs are terminal.

Evidence is the source and checked-in configuration regressions plus terminal commands recorded
here. These Windows/default-feature checks use controlled local HTTP fixtures, native injected
executors, existing stream/OSC loopbacks, missing-serial recovery, and null-device audio fixtures;
they do not qualify live endpoints, hardware, other platforms, native UI, or product-scale timing.
Commit and push this verified milestone on `main`, then continue TCP server/WebSocket/serial and
the broader module/channel/generator audit. P03 remains **IN_PROGRESS**; P04-P18 retain their states.

### P03 batch 21: TCP server native ownership

Resumed clean delivered `36e69aeb6d267f08760902eebf13c6dc8ad58bde` on `main`, matching fetched
`origin/main`; previous turn **progress**, no owned jobs live. The app-owned TCP server transport
still performed actor-side resolve/bind, unbounded command admission, join-on-stop, nonblocking
`write_all`, and unbounded client/receive service. This batch moves setup/recovery and socket
cleanup to a bounded native owner, limits commands/replies/clients/service, and gives broadcasts
fixed-cohort per-client cursors without suffix replay. Requested **XHigh; actual effort
unchanged/unobservable**, no supported control exposed.

Source ownership remains under `protocol/stream/tcpserver`: transport admission and event weights
in `transport.rs`, native setup/duplex service in `transport/worker.rs`, broadcast cursors in
`transport/outbound.rs`, and tests under their owning `tests/` directories. The existing shared
`NativeReplies` continuation mechanism handles input/output/status independently. Module admission
also checks applied port/enabled state before using a previous transport, and background
Listening/Recovering events own connected status. The first compile caught an associated-source
visibility mismatch; corrected it to match the crate-visible event contract. Focused tests pending.

The first focused `cargo test-fast --locked -p Chataigne2 --bin Chataigne2
module_modules_protocol_stream_tcpserver -- --test-threads=4` passed **15/15** in **0.95s** after
**2m11s** compilation. Added applied port/disable-before-callback routing regressions, and an owning
capacity-retry deadline check: failed worker admission schedules a 250ms retry instead of stranding
the module or requesting full snapshots every periodic turn. Expanded verification pending.

Expanded owning run passed **17/17** in **1.49s** after **1m54s** compilation. Disjoint compiled-name
preservation suites passed script API **8**, concrete streams **90** (including those 17), common
streaming **45**, HTTP **59**, Sound Card **35**, OSC **25**, initial Formula **1**, and null-device
Sound Card evidence **1**: **264 distinct app tests**, not an added sum of overlapping focused runs.
`cargo test-fast --locked -p golden_io` passed **38/38** in **0.20s** (3.13s including lock wait),
so **302 distinct app/IO tests** passed. Strict
`cargo clippy --locked -p golden_io -p Chataigne2 --all-targets -- -D warnings` passed in **2m03s**
including build-lock wait. Focused verification complete; requested return to **High**, actual effort
unchanged/unobservable.

`cargo check --locked --workspace --tests` passed in **20.57s**. Native
`cargo build --locked -p Chataigne2 --bin Chataigne2` passed in **2m08s** including build-lock wait;
no linker repair or cache deletion was needed. Root/Core `cargo fmt --all`, explicit TCP server
rustfmt, maintained-doc Prettier and whitespace checks pass. Inventory: **1627 source files /
42 oversized**. TCP server module/transport/native worker/outbound cursor are **574/141/279/85**
lines. Tests are separate beside each feature. All owned jobs are terminal.

Evidence is the checked-in native/module admission regressions and terminal commands recorded here.
These Windows/default-feature checks use controlled TCP/HTTP/stream/OSC loopbacks, injected native
setup/cursor failures, missing-serial recovery, and null-device audio fixtures. They do not qualify
external devices/endpoints, other platforms, native UI, or product-scale timing. Commit and push
this milestone on `main`, then continue TCP server owner integration. P03 remains **IN_PROGRESS**;
P04-P18 retain their states.

This native milestone does not complete TCP server integration: passive owner wake/receipt delivery,
worker-prepared per-client grammar/callbacks, and accepted-prefix generation handoff across app
reconfigure/disable remain next. Existing module script functions/callbacks/templates/client folders
remain preservation contracts. Broader WebSocket/serial, module/channel and generator work remains.

### P03 batch 22: TCP server passive input and lifecycle integration

Resumed clean delivered `01158b6dc1c6a1971dc079a97a126543322d8f64` on `main`, matching fetched
`origin/main`; previous turn **progress**, no owned jobs live. Server input previously used one
actor parser across all clients, immediate callback JSON construction/publication, and cancellation
on ordinary reconfiguration/disable. This batch gives each native client a parser and generation,
prepares source-tagged values/callbacks/logs on the worker, retains lifecycle callbacks through their
publication receipts, and uses passive module wakes/deadlines. One retiring owner drains accepted
commands/results before a replacement starts; disabled service explicitly opts in for that prefix.
Destroy/restore must discard old runtime deliveries and fence the previous wake source. Requested
**XHigh; actual effort unchanged/unobservable**, no supported control exposed.

Initial compile caught one missed shared-input call site and two log-macro arms needing terminating
semicolons; corrected all three. Initial focused run passed **17/20** after **2m09s** compilation:
native per-client input/grammar/source isolation passed, while three old module fixtures assumed a
single tick completed port replacement. Those fixtures now service authoritative wakes until
configuration/retirement settles. Added passive idle, source-tagged graph input, processing changes,
discarded callback receipt recovery, destroy/undo source fencing, accepted-output reconfiguration,
and disabled input/output drain regressions. Expanded run passed **26/26** in **3.25s** after **1m38s**
compilation. Review then corrected connected status while waiting for a retiring generation,
treated intentional native closure as a clean disconnect, and strengthened exact lifecycle/input
callback counts plus disabled-processing/log-only grammar coverage. Final focused run passed
**27/27** in **3.25s** after **1m37s** compilation. Disjoint preservation suites passed script API
**8**, concrete streams **100** (including those 27), common streaming **45**, HTTP **59**, Sound Card
**35**, OSC **25**, initial Formula **1**, and null-device Sound Card evidence **1**: **274 distinct
app tests**. `cargo test-fast --locked -p golden_io` passed **38/38** in **0.20s** (0.17s preparation),
for **312 distinct app/IO tests**. Initial strict Clippy identified a Listening address used only
in test builds after removal of derived equality; the module now reports the real bound address
in its connection-success diagnostic. Focused invariants verified; requested return to **High**,
actual effort unchanged/unobservable. Strict Clippy subsequently passed in **21.31s**, workspace/test
checking in **31.22s**, and native app build in **1m50s**. Final review then found same-endpoint input
updates bypassed the startup mode-capacity guard. Native updates now reject invalid mode storage
before changing the active plan, with a regression preserving its generation, parser and live
connection. Its focused run passed **28/28** in **3.27s** after **1m34s** compilation. Configuration
validation is now shared by the module and native startup, so invalid authored settings use the
configuration-error path rather than the capacity-retry path. Native input updates use the same
input validator. The settled owning run passed **28/28** in **3.24s** after **1m37s** compilation.
Repeating the disjoint preservation set after this last change passed **275 distinct app tests**
(concrete streams now **101**; other counts unchanged). With IO, **313 distinct tests** passed.
Final strict Clippy passed in **1m40s** and workspace/test checking in **1m18s**, both including
build-lock waits. Final native `cargo build --locked -p Chataigne2 --bin Chataigne2` passed in
**2m22s** including build-lock wait, without linker repair or cache deletion. Earlier passes remain
labelled above. Root/Core `cargo fmt --all`, explicit changed-feature rustfmt, maintained-doc
Prettier and whitespace checks pass. Inventory is **1630 source files / 42 oversized**. TCP server
module/transport/native worker are **702/214/337** lines; tests remain separate beside each feature.
All owned jobs are terminal.

Evidence is the checked-in native input, callback receipt, passive idle, lifecycle and undo
regressions, plus terminal commands recorded here. Checks use Windows/default features, controlled
stream/HTTP/OSC loopbacks, injected failure/receipt cases, missing-serial recovery, and null-device
audio fixtures. They do not qualify external hardware/endpoints, other platforms, product-scale
timing, or native UI interaction. Commit and push this milestone on `main`, then continue WebSocket/
serial and the broader module/channel/producer/generator audit. P03 remains **IN_PROGRESS**;
P04-P18 retain their states.

### P03 batch 23: WebSocket client native and passive ownership

Resumed clean delivered `5a2cb36842f11ca9a2b3e1bf3eece857acb2d1b1` on `main`, matching fetched
`origin/main`; previous turn **progress**, no owned jobs live. WebSocket client resolution, unbounded
commands, join-on-stop and unbounded send/flush lived outside the reusable native boundary; its
owner also parsed/constructed callback JSON on a 120 Hz periodic path. This batch moves those
responsibilities to bounded native ownership and passive retained owner delivery. Requested
**XHigh; actual effort unchanged/unobservable**, with no supported control exposed.

The existing locked Tungstenite 0.29 implementation owns framing, masking, HTTP handshake and
control messages through its public nonblocking API. The app's `BudgetSocket` bounds per-turn
read/write bytes and syscall counts, including library fragment/flush loops. Native lifetimes,
command/reply queues, frame sizes, write buffers and service turns are bounded. Frame cursors submit
once, then flush retained encoded bytes; connection generations reject the accepted stale suffix
without moving effects to a replacement. Setup polls both cancellation and canonical admission
closure, with bounded connect/handshake/control deadlines and reconnect backoff. Local invalid UTF-8
fails one frame without reconnecting. New input envelopes retain frame kind for text/binary callback
semantics; shared input validation replaces the server-only validator.

The app owner now reserves `ModuleWake`, uses passive receipt-driven input/graph service, captures
applied host/port/path before admission, and retains one old producer through accepted-frame drain.
Disabled service explicitly finishes that prefix; destroy/undo discards runtime input and fences
the previous wake source. Script methods, commands and app-owned snippets retain their contracts.
Shared stream envelope rejection now invalidates partial parser state, including callback admission
failure, so later bytes cannot join across a dropped preparation boundary. WebSocket mailbox
oversize feedback also invalidates that source's partial frame.

`cargo check --locked -p Chataigne2 --tests` passed in **21.05s** with the temporary empty test-module
import warning. The initial focused run passed **15/15** in **0.42s** after **2m13s** compilation.
The expanded run passed **20/21** after **1m35s** compilation: a Windows test peer inherited its
listener's nonblocking mode; made that fixture explicitly blocking. Added callback-complexity gap
recovery, oversized wire rejection, unavailable-endpoint recovery, and partial-frame reconnect
coverage. The next focused run passed **25/25** in **0.89s** after **1m26s** compilation. After adding
the mailbox gap guard and independent syscall-count assertions, **25/25** passed again in **0.89s**
after **1m23s** compilation; these overlapping runs are not summed.

Strict Clippy found the HTTP test callback closure inferred the library's fixed large error type.
The fixture now implements Tungstenite's public `Callback` trait, matching the library boundary
without an allowance. App fixtures now materialize the module disabled, configure their ephemeral
endpoint, then enable it, avoiding default-endpoint contact. Requested return to **High** after focused
invariants passed; actual effort unchanged/unobservable. Final focused/integration/lint/native
validation remains at that intermediate point. The settled focused run passed **25/25** in **0.79s**
after **1m27s** compilation. Disjoint compiled-name preservation suites passed script API **8**,
concrete streams **125** (including those 25), common streaming **45**, HTTP **59**, Sound Card **35**,
OSC **25**, initial Formula **1**, and null-device Sound Card evidence **1**: **299 distinct app tests**.
`cargo test-fast --locked -p golden_io` passed **38/38** in **0.20s** (0.13s preparation), for
**337 distinct app/IO tests**. Repeated and overlapping profiles are not added twice.

Final `cargo clippy --locked -p golden_io -p Chataigne2 --all-targets -- -D warnings` passed in
**1m38s** including build-lock wait; `cargo check --locked --workspace --tests` passed in **17.91s**.
Native `cargo build --locked -p Chataigne2 --bin Chataigne2` passed in **1m31s**, without linker
repair or cache deletion. Root/Core `cargo fmt --all`, explicit changed-feature rustfmt,
maintained-doc Prettier and whitespace checks pass. Inventory: **1638 source files / 42 oversized**.
WebSocket owner/transport/native worker/socket/cursor are **590/194/309/119/93** lines; tests are
separate beside each feature. All owned jobs are terminal.

Evidence is the checked-in transport, cursor, passive module and shared input regressions plus
terminal commands recorded here. These Windows/default-feature checks use controlled WebSocket/
TCP/UDP/HTTP/OSC loopbacks, injected native/receipt failures, missing-serial recovery, and null-device
audio. They do not qualify external endpoints/hardware, other platforms, native UI interaction or
product-scale timing. Commit and push this milestone on `main`, then continue WebSocket server,
serial, shared producer argument admission/disposal, Sound Card/channel and generator audits.
P03 remains **IN_PROGRESS**; P04-P18 retain their states.

### P03 batch 24: WebSocket server bounded native ownership

Resumed clean delivered `e9230f86f75fe1a15493192f56563f46f8f18757` on `main`, matching fetched
`origin/main`; previous turn **progress**, no owned jobs live. Reconfirmed that the mapping branch
is already merged and the plan requires each verified milestone to commit/push to `origin/main`.
The next owning layer was app WebSocket server transport: actor-side resolution/bind, unbounded
commands/replies, sequential three-second handshakes, unbounded async send/flush and join-on-stop.
Requested **XHigh; actual effort unchanged/unobservable**, with no supported control exposed.

The app now uses the locked Tungstenite 0.29 public nonblocking server handshake/framing API.
Shared app WebSocket code owns the finite codec configuration and socket byte/syscall budget;
client connection setup stays client-owned. The obsolete copying message helper/private client
socket aliases are removed. The server reserves at most 16 live/retiring native workers, with
256 commands / 8 MiB, a 1 MiB retained command bound before copying, 64 replies / 32 MiB and one
retained continuation per input/output/status producer. At most 128 active peers and pending
handshakes share capacity. Each native turn advances at most four accepts, interrupted handshakes,
output recipients and input messages; each socket direction has 64 KiB/four-syscall limits.
Handshake deadline is three seconds; idle HTTP peers cannot block valid upgrades or output.
Exact and wildcard paths retain existing semantics with bounded authored/request path storage.

Broadcasts capture native peer identities when each command begins and share message storage.
Each recipient keeps independent submit/flush state and a five-second deadline, rotating fairly;
WouldBlock never resubmits the encoded frame. Failures reject the original suffix explicitly
without moving it to replacements, including an identical public label. Invalid UTF-8 fails one
broadcast without reconnecting. Input and output reply pressure have independent continuations.
Ping/close remain serviced with application receive disabled. Successful server close flush is
normal disconnection, following the library's documented ConnectionClosed result.

Native cancellation/drop never joins the engine. Blocked setup keeps its reserved lifetime;
bind retries back off 250ms–5s. Native Listening/Recovering events own connection feedback.
Applied port/path/enabled values fence script/command admission before callbacks. Temporary native
capacity exhaustion retries after a 250ms owner deadline and defers snapshots until due. The owner
still uses periodic actor-side callback/parsing with a shared parser and explicitly cancels on
ordinary disable/reconfigure. Passive receipts, per-peer prepared input and accepted-prefix
retirement are the next batch; this milestone does not claim those gaps fixed.

The initial compile found overlapping mutable borrows in native handshake setup; staged the
library call before advancing state. The initial focused run passed **17/17** in **0.57s** after
**2m14s** compilation; expanded ownership/admission coverage passed **21/21** in **0.60s** after
**1m23s** compilation. The next run passed **22/23** after **1m40s** compilation: the occupied-port
app fixture bound loopback while the module binds wildcard, permitting a different bind outcome
on Windows. It now binds the same wildcard address and waits for the explicit warning before
asserting disconnected state. Strict Clippy rejected the large native handshake outcome enum;
boxed the established socket during native handoff. The corrected run passed **23/23** in
**0.42s** after **1m49s** compilation. Added explicit normal-close handling and strengthened that
test; the settled source passed **23/23** in **0.42s** after **1m39s** compilation. Overlapping runs
are not summed. Requested return to **High** after focused proof; actual effort unchanged/unobservable.

Disjoint compiled-name preservation suites passed script API **8**, concrete streams **147**
(including those 23), common streaming **45**, HTTP **59**, Sound Card **35**, OSC **25**,
initial Formula **1**, and null-device Sound Card evidence **1**: **321 distinct app tests**.
These run the built `target/debug/deps/Chataigne2-c47f0ddae7812c83.exe` with the recorded owning
filters and `--test-threads=4`; null-device evidence uses its exact name and one thread.
`cargo test-fast --locked -p golden_io` passed **38/38** in **0.20s** (0.14s preparation):
**359 distinct app/IO tests** total.

Final `cargo clippy --locked -p golden_io -p Chataigne2 --all-targets -- -D warnings` passed in
**1m45s** including lock wait. `cargo check --locked --workspace --tests` passed in **1m48s**
including lock wait. Native `cargo build --locked -p Chataigne2 --bin Chataigne2` passed in
**1m29s** including lock wait, without linker repair or cache deletion. Root/Core Cargo formatting,
explicit changed-feature rustfmt, maintained-doc Prettier and whitespace checks pass.
Inventory: **1649 source files / 42 oversized**; all new runtime/test files remain below 1000 lines.
All owned jobs are terminal.

Evidence is the checked-in native transport/cursor/peer/pressure and owner admission regressions,
plus terminal commands recorded here. Checks use Windows/default features, controlled local
WebSocket/TCP/UDP/HTTP/OSC loopbacks, injected setup/queue/receipt failures, missing-serial recovery
and null-device audio. They do not qualify external endpoints/hardware, other platforms,
native UI interaction or product-scale memory/timing. Commit and push this milestone on `main`,
then continue server passive/per-peer input/retirement, serial, shared producer argument
admission/disposal, Sound Card/channel and generator audits. P03 remains **IN_PROGRESS**;
P04-P18 retain their states.

### P03 batch 25: WebSocket server passive input, retirement and projection ownership

Resumed clean delivered `b991a9ea13ac2cc777951d90df3a1acc0fe4ee5f` on `main`, matching fetched
`origin/main`; previous turn **progress**, no owned jobs live. The native server from batch 24
still handed raw frames to a shared actor parser and immediate callbacks on a 120 Hz periodic
path; ordinary disable/reconfigure cancelled accepted output. This batch completes its app-owned
native/passive boundary through existing public IO/engine and common stream contracts.
Requested **XHigh; actual effort unchanged/unobservable**, with no supported control exposed.

Each native peer owns a parser and input generation. Grammar/processing changes invalidate that
peer's partial frame; logging-only changes preserve it. Prepared input retains graph values,
source-tagged text/binary callbacks, bounded logs and rejection counts. Preparation/complexity or
mailbox-envelope rejection invalidates only that peer's parser; another peer's partial line survives.
Connection/disconnection callbacks use canonical native payload reservations; disconnect reports
abandoned partial lines. Validated input updates reuse existing listeners/connections. Obsolete
immediate server lifecycle callback helpers and unused constants are removed.

The owner reserves a wake before native work and uses passive callback receipt/graph service,
producer exit notifications and coarse retry deadlines. One bounded native drain stays owned
through its callbacks; discarded publication retries only that callback. Ordinary reconfigure and
disable close admission, retain one old producer through its accepted frame/input/callback prefix,
and wait for producer disconnection before replacement. Disabled wakes finish that prefix without
restoring retired clients to the live projection. Destroy cancels producers, discards runtime input
and releases the wake; undo reserves a fresh source. Applied port/path/disable admission fencing,
handshake/peer limits, original-peer frame cursors and independent reply-pressure checks remain.

Review reproduced a projection failure: after discarding an edit batch, the runtime map retained
a client but its public client node/count never recovered without another peer event. The focused
`discarded_client_projection_reconciles` test failed **0/1** after **1m34s** compilation. A focused
`feedback.rs` now retains desired connection/warning state and a separate projection receipt.
Failure reconciles against a fresh snapshot, preserving committed child IDs rather than replaying
structural edits or trusting optimistic parameter-handle caches. Staging explicitly wakes service.
This adds no callback/frame replay. TCP Server has the analogous source pattern; its owning
failure reproduction/publication audit is the next P03 task, separate from historical passing
input/native/retirement evidence.

Initial `cargo check --locked -p Chataigne2 --tests` passed in **21.63s** with four obsolete callback
helper warnings, then those helpers were removed. Initial focused coverage passed **34/34** in
**0.81s** after **2m21s** compilation. Expanded per-peer envelope-gap/configuration-capacity coverage
passed **35/35** in **0.99s** after **1m31s** compilation; strict IO/app Clippy passed in **1m49s**.
Projection review then produced the separate failing regression above. The mechanical feedback
split's check caught helper visibility and an unused import; scoped helpers to their parent and
removed the import. Its first run passed **35/36** after **2m00s** compilation, with the projection
regression timing out because staging had not requested its own wake. Added that wake and partial
prefix/warning-set/clear coverage. Settled coverage passed **38/38** in **0.99s** after **1m59s**
compilation. Overlapping runs are not summed. Requested return to **High** after focused proof;
actual effort unchanged/unobservable.

Disjoint compiled-name preservation suites passed script API **8**, concrete streams **162**
(including those 38), common streaming **45**, HTTP **59**, Sound Card **35**, OSC **25**,
initial Formula **1**, and null-device Sound Card evidence **1**: **336 distinct app tests**.
They run `target/debug/deps/Chataigne2-c47f0ddae7812c83.exe` with those owning filters and
`--test-threads=4`; null-device evidence uses its exact name and one thread.
`cargo test-fast --locked -p golden_io` passed **38/38** in **0.20s** (0.13s preparation), for
**374 distinct app/IO tests** total.

Final `cargo clippy --locked -p golden_io -p Chataigne2 --all-targets -- -D warnings` passed in
**2m07s** including lock wait. `cargo check --locked --workspace --tests` passed in **22.85s**
including lock wait. Native `cargo build --locked -p Chataigne2 --bin Chataigne2` passed in
**0.68s** with warm artifacts, without linker repair or cache deletion. Root/Core Cargo formatting,
explicit changed-feature rustfmt, maintained-doc Prettier and whitespace checks pass. Inventory:
**1655 source files / 42 oversized**. Owner/feedback/transport/peer/worker are
**701/141/208/204/237** lines; all tests remain separate beside their feature. All owned jobs are terminal.

Evidence is the checked-in native input, passive delivery, projection and retirement regressions,
plus terminal commands recorded here. Checks use Windows/default features, controlled local
WebSocket/TCP/UDP/HTTP/OSC loopbacks, injected queue/edit/receipt failures, missing-serial recovery
and null-device audio. They do not qualify external hardware/endpoints, other platforms,
native UI interaction or product-scale memory/timing. Commit and push this milestone on `main`,
then continue TCP/other-owner projection publication, serial, shared producer admission/disposal,
Sound Card/channel and generator audits. P03 remains **IN_PROGRESS**; P04-P18 retain their states.

### P03 batch 26: TCP and shared server projection receipts

Resumed clean delivered `86f380c80eaf710dd764a5c9749004ea175b2b8f` on `main`, matching fetched
`origin/main`; previous turn **progress**, no owned jobs live. The next owning layer was TCP
Server's analogous connection/client projection path. It cleared dirty state before the engine
applied its edits and trusted optimistic parameter caches. The focused
`discarded_tcp_client_projection_reconciles` regression failed **0/1** in **0.01s** after
**1m37s** compilation: the runtime retained its client but the public client node never recovered.
Requested **XHigh; actual effort unchanged/unobservable**, with no supported control exposed.

App `common/streaming/server_feedback.rs` now owns one desired connection/warning state and one
in-flight projection receipt, shared by TCP and WebSocket Server. Small protocol adapters supply
target/warning IDs and client-parameter factories, preserving their existing metadata and public
boundaries. A discarded/partial projection reconciles against a fresh snapshot without replaying
structural edits, losing committed child identities or trusting optimistic handle values. Newer
desired state remains dirty while the previous projection is pending, then replaces its displayed
state after that receipt settles. Projection staging explicitly wakes service. Callback, graph
input and native frame delivery stay independent; projection retries do not replay those effects.
TCP now includes feedback receipts in passive snapshot/wake, disabled completion and destroy/undo
ownership. The duplicated immediate projection implementation is removed; the verified WebSocket
implementation is factored into the shared app component rather than copied into TCP.

`cargo check --locked -p Chataigne2 --tests` passed in **13.47s**. The corrective focused TCP run
passed **32/32** in **2.64s** after **1m57s** compilation. The compiled WebSocket consumer passed
**39/39** in **0.83s**. These include discarded/partial projection recovery, stable client IDs,
warning set/clear recovery, one in-flight projection and retention of newer desired connection,
warning and client state. Existing input, native cursor, pressure, admission, retirement and undo
regressions also pass. Requested return to **High** after focused proof;
actual effort unchanged/unobservable.

Disjoint compiled-name preservation suites passed script API **8**, concrete streams **167**
(including those 71 server tests), common streaming **45**, HTTP **59**, Sound Card **35**, OSC **25**,
initial Formula **1**, and null-device Sound Card evidence **1**: **341 distinct app tests**.
They run `target/debug/deps/Chataigne2-c47f0ddae7812c83.exe` with those owning filters and
`--test-threads=4`; null-device evidence uses its exact name and one thread.
`cargo test-fast --locked -p golden_io` passed **38/38** in **0.20s** (0.14s preparation), for
**379 distinct app/IO tests** total. Overlapping focused/preservation runs are not summed twice.

`cargo clippy --locked -p golden_io -p Chataigne2 --all-targets -- -D warnings` passed in **1m07s**
including lock wait. `cargo check --locked --workspace --tests` passed in **13.01s** including lock
wait. Native `cargo build --locked -p Chataigne2 --bin Chataigne2` passed in **0.68s** with warm
artifacts, without linker repair or cache deletion. Root/Core Cargo formatting, explicit changed
feature rustfmt, maintained-doc Prettier and whitespace checks pass. Inventory: **1658 source files /
42 oversized**. Shared feedback is **142** lines, each protocol adapter **40**; TCP/WebSocket owners
are **663/700** lines. Tests remain separate beside their feature. All owned jobs are terminal.

Evidence is the checked-in owning TCP/WebSocket feedback regressions and preserved native/input/
lifecycle suites, plus terminal commands recorded here. Checks use Windows/default features,
controlled local protocol loopbacks, injected queue/edit/receipt failures, missing-serial recovery
and null-device audio. They do not qualify external hardware/endpoints, other platforms, native UI
interaction or product-scale timing/memory. Commit and push this milestone on `main`, then continue
serial, shared producer admission/disposal, Sound Card/channel and generator audits. P03 remains
**IN_PROGRESS**; P04-P18 retain their states.

### P03 batch 27: Serial native connection and discovery ownership

Resumed clean delivered `d2e8764c1d4a4119fccc06f1b5a198c8652edf44` on `main`, matching fetched
`origin/main`; previous turn **progress**, no owned jobs live. Serial connection lived in the
shared app serial file with unbounded command admission, join-on-stop, whole-buffer writes,
silent full-result drops and no original-connection fence for queued output. Discovery also joined
the actor behind enumeration. Requested **XHigh; actual effort unchanged/unobservable**, with no
supported control exposed. The app common layer now separates connection, discovery and metadata
through a conventional facade; no shell/Golden-private boundary or persisted schema changes.

The locked serialport 4.9 public API still owns native device handling. Connection work reserves
at most 16 live/retiring workers, with 256 commands / 256 KiB queued, 256 KiB retained per command
checked before copying, 64 replies / 2 MiB, and one retained input/output/status continuation.
Each turn services at most four command operations; write slices and read buffers are at most
8 KiB. Native calls request 20ms I/O timeouts, while blocked open/write/cleanup retains its slot
rather than joining the actor or allowing unlimited replacement threads. Configuration bounds
port retained capacity to 1024 bytes without NUL and rejects zero baud before reserving a worker.
DTR/RTS setup failures now enter recovery before publishing connected.

Write cursors keep confirmed offsets on the original connection with a five-second active-command
deadline. WouldBlock/Interrupted preserve the suffix. The pinned Windows COMPort implementation
returns WriteFile errors without exposing its byte count, so a timeout may hide partial effects:
these errors report an unconfirmed outcome and never retry the original frame. Generation tags
reject the queued old suffix after reconnect; reopen backs off 250ms?5s. Input/output pressure
retains FIFO results independently instead of silently dropping them. Native graceful admission
closure drains the admitted output prefix; explicit cancellation may abandon it. Cancellation is
checked before each additional operation after a blocked call returns.

Discovery uses one live/retiring bounded worker. Stop/last-unsubscribe cancels without joining;
blocked enumeration keeps its slot and cancelled/ownerless results cannot restore the catalog.
Enumeration errors preserve the last good list and recover on later scans. Registration identity
exhaustion rejects instead of reusing an identity. Temporary connection/discovery capacity retries
use coarse owner deadlines. Applied port, baud, DTR, RTS and enabled snapshot values fence new
script/command admission before callbacks. Outgoing logs describe queue admission and cap their
byte prefix. Test-only injected providers exercise these contracts without opening hardware.

Initial `cargo check --locked -p Chataigne2 --tests` passed in **21.31s** with temporary scaffold
warnings, subsequently removed. The first native-test compile caught an unnecessary Debug
requirement from `unwrap_err`; the fixture now inspects the error without adding production traits.
The next serial-filtered run passed **28/28** in **1.05s** after **2m13s** compilation. Expanded
nonjoining discovery/enumeration recovery coverage passed **30/30** in **2.26s** after **2m07s**
compilation. That broad name filter includes three unrelated serialization matches; they are not
summed into serial ownership proof. The exact owning suites contain **16 common serial tests** and
**11 serial-module tests**. Requested return to **High** after focused proof; actual effort remains
unchanged/unobservable.

Disjoint compiled-name preservation suites passed script API **8**, concrete streams **170**,
common streaming **45**, common serial **16**, HTTP **59**, Sound Card **35**, OSC **25**,
initial Formula **1**, and null-device Sound Card evidence **1**: **360 distinct app tests**.
They run `target/debug/deps/Chataigne2-c47f0ddae7812c83.exe` with those owning filters and
`--test-threads=4`; null-device evidence uses its exact name and one thread.
`cargo test-fast --locked -p golden_io` passed **38/38** in **0.20s** (4.29s preparation/lock wait),
for **398 distinct app/IO preservation tests**. Overlapping focused runs are not counted twice.

`cargo clippy --locked -p golden_io -p Chataigne2 --all-targets -- -D warnings` passed in **1m39s**
including lock wait. `cargo check --locked --workspace --tests` passed in **22.70s** including lock
wait. Native `cargo build --locked -p Chataigne2 --bin Chataigne2` passed in **0.66s** with warm
artifacts, without linker repair or cache deletion. Root/Core Cargo formatting, explicit serial
feature rustfmt, maintained-doc Prettier and whitespace checks pass. Inventory: **1669 source files /
42 oversized**. Serial facade/metadata/discovery/connection/worker/cursor are
**16/229/258/158/188/59** lines; module owner **585**. Runtime/tests are separate. All owned jobs are terminal.

The module still uses periodic actor-side discovery/input/callback processing and cancels on
ordinary disable/reconfigure. Bounded retained catalog/registration storage, options preparation/
disposal, producer-exit notifications/recovery, native input generations, passive receipts and
accepted-prefix retirement are the next serial work. These checks do not qualify main-thread
catalog budgets, physical line timing, external devices, other platforms, native UI interaction
or product-scale throughput/memory. Evidence is checked-in native cursor/pressure/lifecycle,
discovery and applied-admission regressions plus terminal commands above. Commit and push this
milestone on `main`, then continue passive serial and the broader producer/module/Sound Card/
generator audits. P03 remains **IN_PROGRESS**; P04-P18 retain their states.

### P03 batch 28: Serial retained catalog and passive service ownership

Resumed clean delivered `2066f735b44d6d32059ab100fb24420ee321baf8` on `main`, matching fetched
`origin/main`; previous turn **progress**, no owned jobs live. Requested **XHigh** for catalog/
subscription generations, producer-exit recovery, native input generations, receipt delivery and
accepted-prefix retirement; actual effort unchanged/unobservable, with no supported control exposed.
Ownership remains in app common serial/stream components and the concrete serial module.

Discovery now admits at most **1024 subscribers**, one live/retiring producer, **128 retained ports**
and **2 MiB** including prepared enum options. It bounds retained name/label/details/tag storage,
prepares and compares catalogs on the native producer, and publishes a header plus canonical retained
lease. Identical catalogs reuse the lease; failed enumeration or catalog admission retains the last
good options. Final payload disposal uses the shared bounded worker disposer. Subscriber callbacks
run outside the catalog lock, including startup-failure/producer-exit paths. Last unsubscribe
invalidates the epoch and cancels without joining; blocked old enumeration retains the single slot
and cannot restore a new subscription's state. Producer exit notifies owners and restarts through a
one-second deadline, avoiding failure-driven worker churn.

The connection worker now owns parsing, canonical byte/text callbacks and bounded incoming logs.
Input/connection generations discard partial frames across reconnect, grammar and processing changes,
including disable/re-enable without intervening bytes. Reply storage is **64 items / 32 MiB** for
prepared envelopes, with the existing independent input/output/status continuations. Output cursors,
original-device generation fencing, timeout-as-unknown-effect policy and native slot limits survive.
No protocol implementation, schema, Golden/app boundary or shell bootstrap changes were needed.

The module is **passive**: worker replies, receipts, configuration/metadata edits and coarse retry
deadlines drive service. A bounded native drain waits behind callback receipts; canonical delivery
retries only discarded publication. Ordinary disable/reconfigure closes admission and retains one
old worker until accepted output and pending input complete, including while disabled. Retiring
status cannot restore connected; destroy cancels and clears deliveries/subscriptions, and undo
reserves a fresh wake source. One isolated receipt owns each connected/warning/catalog projection;
only successful completion acknowledges its captured catalog/selection generation. Failed/partial
edits reconcile against a fresh snapshot and newer desired state remains pending. Queued enum
mutation retains the catalog lease rather than copying its option tree before application.
The now-unused actor parser state and immediate byte-callback helper were removed from shared
streaming code after all concrete streams moved to native preparation.

Initial app/test compilation caught a fixture NodeId constructor and consumption of the final reply
continuation; the corrected check passed in **24.12s**, with obsolete actor-helper warnings then
removed. The first expanded run passed **38/41** matched tests after **2m20s** compilation: failures
were two old polling/synchronous-retirement fixture assumptions and a fixture waiting for status
instead of admitted input. Fixtures now wait for the native prefix and asynchronous owner state.
Additional disabled-binary, callback-failure/undo and partial feedback coverage was added. The final
serial-filtered run passed **44/44** in **2.26s** after **2m04s** compilation. Three matches are unrelated
serialization tests; the owning suites contain **24 common serial** and **17 serial-module** tests.
Requested return to **High** after focused proof; actual effort remains unchanged/unobservable.

Disjoint compiled-name preservation suites passed script API **8**, concrete streams **176**,
common streaming **45**, common serial **24**, HTTP **59**, Sound Card **35**, OSC **25**, initial
Formula **1**, and null-device Sound Card evidence **1**: **374 distinct app tests**. The command
runs `target/debug/deps/Chataigne2-c47f0ddae7812c83.exe` with these owning filters and
`--test-threads=4`, checking compiled-name sets for overlap before summing.
`cargo test-fast --locked -p golden_io` passed **38/38** in **0.20s** (0.13s preparation):
**412 distinct app/IO preservation tests**. Overlapping focused runs are not counted twice.

Strict `cargo clippy --locked -p golden_io -p Chataigne2 --all-targets -- -D warnings` passed in
**29.43s**; `cargo check --locked --workspace --tests` passed in **23.38s**. Native
`cargo build --locked -p Chataigne2 --bin Chataigne2` passed in **0.49s** with warm artifacts,
without linker repair or cache deletion. Root/Core Cargo formatting, explicit serial/shared-stream
rustfmt, maintained-doc Prettier and whitespace checks pass. Inventory: **1676 source files /
42 oversized**. Serial connection/discovery/metadata/native-worker/catalog/discovery-worker are
**190/277/235/225/62/145** lines; module owner/projection are **589/121**. Runtime/tests stay separate.
All owned jobs are terminal. Fetched `origin/main` still matches the parent with no divergence.

These checks use deterministic injected ports/providers, missing serial endpoints and null audio.
They do not qualify physical device/line timing, other platforms, native UI interaction, or
product-scale latency/memory. Evidence is the checked-in catalog/subscription/native-input/passive
lifecycle/receipt regressions and terminal commands above. Commit and push this verified milestone
on `main`, then continue shared stream/HTTP argument and disposal admission, Sound Card/module/
channel ownership and generator elapsed-time accounting. P03 remains **IN_PROGRESS**; P04-P18
retain their states.

### P03 batch 29: Native outgoing completion logging

Resumed clean delivered `2c24b1197902232505d2fe75ba6966ea39854124` on `main`, matching fetched
`origin/main`; previous turn **progress**, no owned jobs live. Source audit confirmed shared
stream script/command helpers still clone unbounded arguments and recursively encode value trees
before native admission. That boundary remains open. This batch removes the remaining actor-side
outgoing byte-log formatting from Serial, TCP Server and WebSocket Server, completing native log
preparation alongside their existing cursor ownership. No Golden/app or persistence/protocol
boundary changes were needed.

Requested **XHigh** for coupled cursor/recipient outcomes, bounded reply continuations and accepted
retirement; actual effort unchanged/unobservable, with no supported control exposed. The three native
send APIs now capture logging policy explicitly with each admitted command. TCP's typed command
weight includes that policy, and a cheap closed-admission preflight rejects before copying a payload.
Size limits, queue limits, original-recipient ownership, write deadlines and no-replay behavior survive.

App common `streaming/outgoing.rs` formats at most **1024 payload bytes** on the native owner.
Serial publishes success only after its write cursor completes. Broadcast recipients now use typed
**Pending / Confirmed / Failed** states; one completion summary reports only confirmed members of
the original native cohort. A failed recipient or submitted-but-unflushed WebSocket frame cannot
count as confirmed. All-failed commands emit their existing explicit failures without a success log.
Messages describe native send completion rather than application acknowledgement by remote peers.

One output continuation retains a completed command's bounded log behind a full mailbox, pausing
further output while independent input still progresses. Ordinary retirement drains every admitted
completion result; explicit cancellation may abandon them. The actor only forwards the prepared
message, including while servicing a retiring worker. It no longer inspects/formats outgoing bytes
for these protocols. Admission/traffic feedback continues to say queued; completed output says sent.
TCP/WebSocket Client and UDP already owned their outgoing log preparation natively and are preserved.

The initial app/test check found two old test call sites missing the new explicit policy argument.
The first completion-test compile caught a missing fixture drain-state import; corrected source then
passed **11/11** completion-matched tests in **0.41s**, after **3m00s** preparation including a build
lock wait behind the remaining binary job. Six are newly added owning completion tests; the seventh
new test rejects serial timeout-as-success and runs in the common serial suite. Process handles were
observed through terminal state; observation timeouts did not trigger process replacement.

Owning suites pass common serial **27**, serial module **17**, TCP Server **34** and WebSocket
Server **41**: **119 distinct tests**. New regressions exercise confirmed recipient subsets,
partial/unknown effects, WebSocket flush ownership, silent logging policy, bounded payload prefixes,
full completion mailboxes, independent input, FIFO logs and accepted-prefix retirement without replay.
After final review added TCP's closed-admission preflight, its **34/34** owning suite passed again in
**3.14s**, following **2m28s** preparation. Requested return to **High** after focused proof; actual
effort remains unchanged/unobservable.

Disjoint compiled-name preservation filters passed script API **8**, concrete streams **180**,
common streaming **45**, common serial **27**, HTTP **59**, Sound Card **35**, OSC **25**, initial
Formula **1**, and null-device Sound Card evidence **1**: **381 distinct app tests**. They run
`target/debug/deps/Chataigne2-c47f0ddae7812c83.exe` with those filters and `--test-threads=4`,
checking compiled-name sets for overlap before summing. Only the final TCP preflight changed after
that pass, covered by the final owning rerun above. `cargo test-fast --locked -p golden_io` passed
**38/38** in **0.20s**, with **2m18s** preparation/lock wait: **419 distinct app/IO tests**.
Overlapping focused runs are not counted twice.

Final strict `cargo clippy --locked -p golden_io -p Chataigne2 --all-targets -- -D warnings` passed
in **12.17s** (earlier full pass **30.02s**). Final `cargo check --locked --workspace --tests` passed
in **7.53s** (earlier pass **22.68s**). Native `cargo build --locked -p Chataigne2 --bin Chataigne2`
passed in **0.49s** with warm artifacts, without cache deletion or linker repair. Root/Core Cargo
formatting, explicit affected-feature rustfmt, maintained-doc Prettier and whitespace checks pass.
Inventory: **1680 source files / 42 oversized**. Shared outgoing helper is **18** lines; Serial/TCP
Server/WebSocket Server owners are **585/654/691**. Their native cursor modules are **70/136/139**;
runtime/tests stay separate. All owned jobs are terminal. Fetched `origin/main` has no divergence.

Checks used deterministic injected serial ports and controlled loopback. They do not qualify physical
device/line timing, other platforms, native UI, or product-scale latency/memory. Evidence is checked-in
native cursor/logging/mailbox/retirement regressions and the commands above. Commit and push this
verified milestone on `main`. Next, repair shared stream command capture/materialization/disposal
(`common/streaming/commands` and `modules/protocol/stream/commands`), including unbounded script
string conversions, recursive value collection/JSON creation and command-envelope ownership before
native admission. Then continue HTTP argument conversion/disposal, Sound Card/module/channel
ownership and generator elapsed-time accounting. P03 stays **IN_PROGRESS**; P04-P18 retain their
states.

### P03 batch 30: Bounded native stream command preparation

Resumed clean delivered `ed6f1039a957d41903685b57d99cc0679eba7827` on `main`, matching fetched
`origin/main`; previous turn **progress**, no live owned jobs at resume. This batch moves the five
stream command node formats (string, bytes, hex, values and values-as-JSON) into one bounded app
preparation service. Golden owns only reusable typed local events and exact node-lifetime guards;
app common owns stream captures, budgets, preparation, cancellation and FIFO publication. The six
concrete Serial/TCP/WebSocket/UDP modules consume prepared requests through public Golden APIs.

Requested **XHigh** for snapshot/override ownership, bounded admission, receipt retry and
destroy/undo fencing. No supported effort control is exposed; actual effort remains unchanged and
unobservable. Command nodes capture fixed parameter IDs and a canonical immutable snapshot lease.
The shared worker admits **64 queued jobs**, **256 live tickets**, and **16 tickets per owner**.
Its single native lifetime blocks on notification-safe receive while idle. Source wake slots are
reserved lazily and released after the queue settles, preserving the engine's 1024 live/retiring
registration bound without reserving a slot for every authored command.

Invocation JSON is preflighted before clone/typed decoding: **256 KiB**, **4096 values**, and
**16 levels**, with a canonical **64-allocation / 16 MiB** override pool. Only the dispatch target
is decoded on the actor, under a smaller **4 KiB / 128-value / eight-level** bound. Native work
decodes other overrides, builds the immutable overlay, visits values, formats strings and encodes
bytes/JSON. Tree preparation bounds **4096 values**, **16384 visited nodes**, **64 folder levels**
and **4 MiB source data**. JSON output additionally limits depth to **32**. The incremental writer
limits encoded output to **1 MiB**, including repeated separators, JSON escaping and mixed script
byte arguments. Transport-specific limits still apply before native send.

Prepared commands retain the shared stream payload reservation through routing and final disposal.
Typed `CustomEvent::local` attachments are shallow-cloned, skipped by serialization and always
transient, even if public retention metadata changes. They never enter UI replay or clear redo.
Ordinary JSON events retain their existing wire representation; no generated protocol changes are
needed. A new public shared-JSON constructor preserves native callback payload identity without
requiring a struct literal that bypasses the local attachment invariant.

Each source publishes one occurrence through an isolated edit receipt. A discarded publication
retries the same lease; an applied occurrence is removed permanently. Native failure resolves
the active and queued tickets explicitly, and a later admission can restart the bounded worker.
Source cancellation invalidates already-published requests; an exact target guard prevents stale
delivery after removal/undo. The wake registry tracks attached identities independently of callback
snapshots, so an old read model cannot register a removed target. Current source/module enable state
is also checked at admission. Preparation acceptance does not claim native send completion.

Initial app/test compilation found mechanical API mismatches in reference paths, declared child
handles and borrowed transport sends; corrected source passed. A later test fixture used struct
update syntax on a Drop type; explicit field initialization fixed it. Review found that timed
cancellation waits do not return on ordinary notifications; the native loop now uses the existing
version-safe blocking receiver. Review also caught numeric script arguments bypassing a full byte
buffer's output limit; all byte argument shapes now use the same bounded writer and regression.
Strict Clippy found a manual slice-size calculation; `size_of_val` fixed it.

The full Golden Engine suite passed **535 unit tests**, **8 integration tests**, and **7 doctests**:
**550 passed**, with **3 existing ignored tests**. Four new owning tests verify shallow local
delivery/serialization, forced transient retention/redo preservation, target lifetime capture and
stale-snapshot/removal/undo fencing. The focused common command suite passed **15/15** in **0.04s**
after **2m16s** preparation. Its eight new tests cover retained source capacity, output/JSON/token
bounds, owner/native/global admission, producer failure recovery, discarded publication, wake-slot
release and nonjoining destroy/undo cancellation. An earlier end-to-end UDP run passed **3/3** in
**6.02s**; final current-source preservation is recorded below when complete.

Focused verification complete: requested return to **High** for routine integration and delivery;
actual effort remains unchanged/unobservable. P03 stays **IN_PROGRESS**. Native stream script
argument capture/conversion/disposal, generic rejected-ingress disposal, HTTP argument conversion,
Sound Card/module/channel ownership and generator elapsed-time accounting remain open. This batch
does not qualify generic engine bound-parameter synchronization costs or product-scale actor latency.

Final current-source common commands passed **15/15** in **0.06s**, after **2m19s** preparation.
Disjoint compiled-name preservation filters passed script API **8**, concrete streams **183**,
common streaming **53**, common serial **27**, HTTP **59**, Sound Card **35**, OSC **25**, initial
Formula **1**, and null-device Sound Card evidence **1**: **392 distinct app tests**. The concrete
stream pass includes all three new end-to-end UDP command tests against the final implementation;
its **183/183** pass took **6.09s**. Common streaming passed **53/53** in **0.18s**. Tests run the
compiled `target/debug/deps/Chataigne2-c47f0ddae7812c83.exe` with these filters and
`--test-threads=4`; compiled name sets are checked for overlap before summing. Initial broad
discovery filters overlapped HTTP handshakes/product evidence; those were narrowed before the run
and contribute no extra pass count. `cargo test-fast --locked -p golden_io` passed **38/38** in
**0.20s**, following **0.15s** preparation. Together with Golden Engine's **550**, this is **980
distinct passed tests**; focused/overlapping reruns are counted once.

Strict `cargo clippy --locked -p golden_engine -p golden_io -p Chataigne2 --all-targets -- -D warnings`
passed in **30.09s** after the slice-size repair. Final
`cargo check --locked --workspace --tests` passed in **28.22s**. Native
`cargo build --locked -p Chataigne2 --bin Chataigne2` passed in **11.60s** without cache deletion
or linker repair. Root/Core Cargo formatting, explicit affected-feature rustfmt, maintained-doc
Prettier and whitespace checks pass. Inventory: **1692 source files / 42 oversized**, with no new
size exceptions. Common command codecs/capture/preparation/native worker are **292/146/297/130**
lines; the concrete command registry is **385**. Runtime and test sources remain separate.
All owned jobs are terminal; fetched `origin/main` has no divergence from the delivered parent.

These tests use deterministic gates, injected serial ports and controlled loopback. They do not
qualify physical device/line timing, other platforms, native UI or product-scale latency/memory.
Commit and push this verified milestone on `main`, then continue the native script boundary above.

### P03 batch 31: Native stream script argument ownership

Resumed clean delivered `b8c4c4e7e494eed492f973a24c84301f15256ee8` on `main`, matching fetched
`origin/main`; previous turn **progress**, no owned jobs live at resume. The six stream module
script methods still converted/cloned arguments and encoded payloads on the actor. This batch
moves that work into their existing native send queues. Golden supplies only an invocation-scoped
shared argument capture API; app common owns stream method selection, shape/storage bounds,
retained input leases and native conversion. Endpoint/configuration policy remains in the modules.

Requested **XHigh** for coupled argument lifetimes, conservative output admission, FIFO ordering,
original-connection generations and accepted-prefix retirement. No supported effort control is
exposed; actual effort remains unchanged/unobservable. `ProcessCtx::capture_script_args` captures
the original `Arc<Vec<ParamValue>>` without cloning values and validates the supplied buffer/length.
The engine installs the capability only during script invocation and clears it on success or error.
Standalone and ordinary mutation callbacks cannot capture arguments from an earlier invocation.
No wire, project-format or generated protocol declarations change.

App `ScriptSend` preflights at most **4096 values / 4 MiB retained storage**, including spare vector,
string, reference-name and reference-path capacities. Canonical input admission is **64 allocations /
16 MiB**, separate from existing transport queue budgets. Native queue rejection, ordinary retirement,
destroy and conversion failure retain background final-disposal ownership once an input lease exists.
Inputs rejected before that reservation still belong to the broader generic ingress/disposal audit.

`OutgoingBytes` keeps input unencoded at admission. Queue weight reserves a conservative output
bound using borrowed string/reference lengths and fixed scalar expansion bounds, capped by each
transport's existing output/retained-byte limit. Small calls can share queue capacity. Escape
decoding, scalar conversion, byte/hex parsing, scratch-capacity trimming and final payload creation
run natively under the existing bounded codecs. Output cannot be retained beyond its reservation;
conversion failure is terminal for that occurrence and cannot become an empty successful send.
Feedback does not invent an encoded byte count before conversion has completed.

Script inputs join the same queues and original socket/connection generations as encoded command
payloads. Ordinary disable/reconfigure drains the admitted prefix; explicit destroy may cancel it.
Cancellation is checked again after native encoding before output writes. Invalid content produces
native error feedback without reconnecting an otherwise healthy endpoint or replaying a prefix.
Synchronous failures still cover argument shape/storage, native capacity and current endpoint
admission; content/output conversion errors are asynchronous. Native queue admission is not a
remote application acknowledgement.

Review found two correctness gaps and added owning regressions. UDP must seal its shared destination
ticket independently of each payload: a malformed script cannot poison a queued valid send using
that ticket. Reference hints also retain spare name/path capacity; counting only their visible
lengths underclaimed input storage. The common value budget now includes their actual capacities.
Shape-based output reservations and trimmed byte-list scratch avoid charging every small script
the entire transport maximum while preserving a hard bound before native send.

Initial `cargo check --locked -p Chataigne2 --tests` passed in **31.39s**. The focused Golden
argument-capture test passed after **6m29s** preparation, preserving the original vector pointer
and spare capacity on both accepted and rejected callbacks. The full Golden Engine suite then
passed **536 unit tests**, **8 integration tests**, and **7 doctests**: **551 passed**, with
**3 existing ignored tests**. Preparation was **1m38s**, including a shared build-lock wait behind
the user's existing development watcher. The watcher is not an owned job and was not stopped.
These Core checks include all prior local-event, wake, history and edit-failure regressions.

Focused common command tests passed **20/20** in **0.05s**, after **4m20s** preparation including
a shared build-lock wait. Five new tests cover canonical argument identity, retained vector/string/
reference capacity, deferred encoding, terminal failure, conservative output bounds and scalar
extremes. Serial admission/retirement tests passed **5/5** in **0.17s**, including two new owning
regressions for blocked output plus queued script arguments across disable, and conversion failure
without reconnecting or blocking the next valid send. The new UDP destination-ticket regression
passed **1/1** in **0.02s**. Requested return to **High** after this focused proof; actual effort
remains unchanged/unobservable.

Routine integration exposed incorrect formatting syntax in existing stream logger calls: this
logger accepts separate messages rather than standard formatting arguments. The touched stream
owners/command preparation now pass a fully formatted message, preserving context and error in one
record. Strict Clippy found six needless temporary-vector borrows after the generic send-input API;
the tests now transfer those temporary vectors directly without suppressions. The first broader
matrix passed **397/400**; three regressions correctly caught numerical capacity limits missing from
the new shared error path. Protocol-specific 256 KiB / 1 MiB context was restored in the constructors,
preserving those tests' existing expectations. All three focused capacity regressions then passed.

Final disjoint compiled-name preservation filters passed script API **8**, concrete streams **186**,
common streaming **58**, common serial **27**, HTTP **59**, Sound Card **35**, OSC **25**, initial
Formula **1**, and null-device Sound Card evidence **1**: **400 distinct app tests**. They run
`target/debug/deps/Chataigne2-c47f0ddae7812c83.exe` with those filters and `--test-threads=4`,
checking name sets for overlap before summing. Concrete streams passed in **6.27s**, common
streaming in **0.19s**, and common serial in **2.26s**. `cargo test-fast --locked -p golden_io`
passed **38/38** in **0.20s**, after **0.13s** preparation. Together with Golden Engine's **551**,
this is **989 distinct passed tests**. Focused/overlapping reruns are counted once.

The final stream Cargo test invocation initially failed its native application link with
**LNK2019/LNK1120**, while the current test executable linked successfully. Missing anonymous LLVM
symbols came from the unchanged StateTransition descriptor and StateTransitionManager item-kind
object in `Chataigne2-2lml0tamgzb10`. After every affected compiler was terminal, a recursive cache
removal attempt was rejected by automatic review before execution. A safer, reversible rename of
that single app incremental directory to `Chataigne2-2lml0tamgzb10-retired-batch31` preserved its
files; both paths were checked beneath the workspace's `target/debug/incremental` directory.
The default `cargo build --locked -p Chataigne2 --bin Chataigne2` then passed in **2m04s** using
a fresh app cache. No source workaround or compiler/profile override was needed. Repeating
`cargo test-fast --locked -p Chataigne2 module_modules_protocol_stream -- --test-threads=4`
passed **186/186** in **6.48s**, after **0.51s** preparation. The recurring incremental artifact
gap remains P12 work; the preserved cache is an ignored diagnostic artifact, not source delivery.

Final strict `cargo clippy --locked -p golden_engine -p golden_io -p Chataigne2 --all-targets -- -D warnings`
passed in **14.23s**. Final `cargo check --locked --workspace --tests` passed in **29.98s**.
Root/Core Cargo formatting, explicit affected-feature rustfmt, maintained-doc Prettier and whitespace
checks pass. Inventory: **1696 source files / 42 oversized**, with no new size exceptions.
Outgoing bytes/script input/their new tests are **131/101/93** lines; serial admission tests are
**246**. Runtime and tests remain separate. All owned jobs are terminal; the user's development
watcher remains independent of this milestone.

Tests use deterministic gates, injected serial ports and controlled loopback. They do not qualify
physical devices, other platforms, native UI or product-scale latency/memory. Commit and push this
verified milestone on `main`. Next, continue P03 encoded outbound copying/rejected payload ownership
and generic ingress/disposal, followed by HTTP argument conversion/disposal, Sound Card/module/
channel ownership and generator elapsed-time accounting. P03 stays **IN_PROGRESS**; P04-P18 retain
their states.

### P03 batch 32: Canonical encoded outbound ownership

Resumed clean delivered `cccd449e798c8b0f7d175a403e1d3a6e2eb0915b` on `main`, matching fetched
`origin/main`; previous turn **progress**, no owned jobs live at resume. The user's independent
development watcher was present. Source audit confirmed encoded commands still borrowed their
prepared byte slice into native constructors, which cloned it on the actor before queue admission.
Full/disconnected queue rejection then destroyed that private vector on the actor. This batch
extends the existing canonical preparation lease through all six native send queues.

Requested **XHigh** for retained lifetimes, rejected admission/final-owner disposal, byte accounting
and accepted-prefix semantics. No supported effort control is exposed; actual effort remains
unchanged/unobservable. App common `StreamSend`/`SendInput` now require a retained prepared command
or script input in production. Borrowed/owned raw byte variants and conversion conveniences are
test fixtures, with their implementations under the adjacent outgoing `tests/` directory.
No Golden, wire, persistence, generated protocol, app-shell startup or UI contracts change.

`OutgoingBytes` has one exclusive state: canonical prepared data, retained script input, owned
native bytes or terminal failure. It cannot hold conflicting sources or turn failed preparation
into an empty successful output. Prepared admission performs a shallow lease clone and length
check. Serial, TCP Client and UDP keep the original allocation through partial writes/fanout.
WebSocket frames and TCP broadcasts create private ownership-consuming buffers on their native
owner. Failed queue admission releases only the small retained handle on the actor; the payload
pool continues to own final destruction of its vector, metadata and guards.

The existing canonical pool charges actual storage, including spare capacity. Native queue weight
reserves its own header plus logical output bytes instead of charging the shared allocation again
for every lease. Output limits still apply before native admission. The module validates source/
target eligibility at that boundary. After acceptance, the original transport generation/cursor
owns the prefix; source retirement does not revoke already admitted bytes. Native cancellation,
disabled/reconfigured drain, output logs, UDP destination tickets and no-replay cursors survive.

Initial app/test compilation passed in **49.94s**, with a later expanded check passing in **23.90s**.
The first ordinary Cargo test attempt failed its native application link with **LNK2019/LNK1120**:
1067 unresolved anonymous LLVM symbols in unchanged macro-generated inbox/Alchemist/node code.
Its compiler was observed through terminal state, without restarting on observation timeouts.
The source state was subsequently consolidated into the exclusive outgoing enum above.

`cargo test-fast --locked -p Chataigne2 --bin Chataigne2 common::streaming::commands -- --test-threads=4`
passed **23/23** in **0.06s**, after **1m46s** preparation. Three new common tests verify pointer
identity, spare canonical capacity with a free item slot, bounded native owned conversion, full
queue rejection, independent retirement of a rejected input and final reservation release while an
accepted prefix remains blocked. Six new owning tests, one per transport, passed individually.
They prove shallow admission, slice-cursor allocation identity through partial writes/retirement,
and worker-side ownership-consuming conversion. Retired engine guards intentionally model an
already accepted native prefix; the tests prevent accidental revalidation after admission.
Requested return to **High** after focused proof; actual effort remains unchanged/unobservable.

Strict IO/app Clippy passed first in **41.13s**, then finally in **14.40s**. Final
`cargo check --locked --workspace --tests` passed in **23.94s**. After affected compilers were
terminal, the native app incremental directory `Chataigne2-2lml0tamgzb10` was preserved by a
reversible rename to `Chataigne2-2lml0tamgzb10-retired-batch32`, with both paths checked beneath
the workspace's `target/debug/incremental` directory. No files were deleted or compiler/profile
settings changed. `cargo build --locked -p Chataigne2 --bin Chataigne2` passed in **0.53s** with
a warm native artifact independently produced by the development watcher. Its executable timestamp
was **23:26:20**, after the last runtime source edit at **23:21:23**. This is not a measured cold
rebuild. Repeating the original ordinary Cargo command test path passed **23/23** in **0.06s**,
after **0.90s** preparation. The incremental artifact gap remains P12 work.

Final disjoint compiled-name preservation filters passed script API **8**, concrete streams **191**,
common streaming **61**, common serial **28**, HTTP **59**, Sound Card **35**, OSC **25**, initial
Formula **1**, and null-device Sound Card evidence **1**: **409 distinct app tests**. They run
`target/debug/deps/Chataigne2-c47f0ddae7812c83.exe` with those filters and `--test-threads=4`,
checking name sets for overlap before summing. Streams passed in **6.44s**, common streaming in
**0.18s**, and common serial in **2.26s**. `cargo test-fast --locked -p golden_io` passed **38/38**
in **0.20s**, after **0.15s** preparation. The full Golden Engine command passed **536 unit tests**,
**8 integration tests** and **7 doctests**: **551 passed**, with **3 existing ignored tests**, after
**0.25s** preparation. Together this is **998 distinct passed tests**; focused reruns count once.

Root/Core Cargo formatting, explicit affected-feature rustfmt, maintained-doc Prettier and whitespace
checks pass. Inventory: **1697 source files / 42 oversized**, with no new size exceptions. Outgoing
runtime/tests are **143/143** lines. Runtime and tests remain separate. All owned jobs are terminal;
the development watcher remains independent. Fetched `origin/main` has no divergence from the parent.

Tests use deterministic gates, controlled loopback and injected serial ports, and do not qualify
physical devices, other platforms, native UI or product-scale latency/memory. Commit and push this
verified milestone on `main`. Next, continue generic script ingress/rejected-input disposal, HTTP
argument conversion/disposal, Sound Card/module/channel ownership and generator elapsed-time
accounting. P03 stays **IN_PROGRESS**; P04-P18 retain their states.

### P03 batch 33: Retained script-method ingress

Continued the retained runtime diff from `b7a5c9a2`; the independently requested completion
overview was committed/pushed as `bf6b6d09` during this batch. Previous goal turn was **progress**.
The live validation job was observed to completion, never restarted on observation timeouts.

Golden IO now admits `PayloadReservation` before materialization. Cancellation releases capacity;
sealing retains the original allocation, releases unused bytes and keeps item/used-byte capacity
through background destruction. Reservations outlive the original pool handle. Ordinary prepared
retention preserves its single-lock admission path; all admission forms share one accounting rule.

`golden_script::ScriptMethodCall` / `ScriptArguments` replace raw method/argument ownership in
QuickJS host ops, host bridges and engine edits. The canonical pool bounds 4096 allocations /
64 MiB, with 4096 arguments / 4 MiB per call and 1024 retained method-name bytes. Rust producer
rejection returns original allocations. Missing targets, unsupported methods and unvisited
host/edit suffixes release shallow handles. QuickJS reserves before typed conversion and retains
partial vectors on conversion failure. Method JSON parsing bounds aggregate nodes/depth before
tree expansion and rejects malformed input explicitly. Public `ParamValue::retained_bytes`
centralizes spare string/reference/vector accounting shared with app command bounds.

Native stream input wrappers retain the Golden argument lease, preserving their existing 64-item /
16 MiB quota without independent final owners of the same raw Arc. Twenty test producers migrated
to explicit preparation. No wire DTO, persistence, UI or app-shell startup contract changed.
See [host ownership](../architecture/hosts.md) and
[app contracts](../architecture/application-contracts.md) for maintained boundaries and limits.

XHigh was requested for the coupled lifetime/admission work; actual effort remained unchanged /
unobservable. Initial test import and reused-runtime fixture errors were corrected. Return to
High was requested after the final owning suites passed, before ordinary app integration.

| Final check                                                                                                                            | Result                                                                                                                                                                                                                |
| -------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `cargo test-fast --locked -p golden_parameters -p golden_io -p golden_script -p golden_engine -- --quiet`                              | 619 passed, three existing ignored manual benchmarks; 4m24s preparation. Engine 537 unit + 8 integration + 7 doc, IO 42, parameters 7, script 11 unit + 7 integration.                                                |
| `cargo clippy --locked -p golden_parameters -p golden_io -p golden_script -p golden_engine -p Chataigne2 --all-targets -- -D warnings` | Passed, 47.13s.                                                                                                                                                                                                       |
| `cargo test-fast --locked -p Chataigne2 --bin Chataigne2 --no-run`                                                                     | Passed, 2m30s; fresh `Chataigne2-d57ae873474d576f.exe`.                                                                                                                                                               |
| Disjoint app filters on that executable, `--test-threads=4 --quiet`                                                                    | 409 passed of 906 listed: script API 8, concrete streams 191, common streaming 61, common serial 28, HTTP 59, Sound Card 35, OSC 25, initial Formula 1, null-device audio 1. Name-set overlap checked before summing. |
| `cargo check --locked --workspace --tests`                                                                                             | Passed, 30.53s.                                                                                                                                                                                                       |
| `cargo build --locked -p Chataigne2 --bin Chataigne2`                                                                                  | Passed, 1m56s; no incremental-cache intervention. Frontend inputs unchanged, existing UI bundle reused.                                                                                                               |

**1,028 distinct tests passed**; focused/intermediate reruns count once. Seven new source files
are at most 182 lines. The documented generated/vendor exclusions count 1517 source files and
42 existing oversized files; no new exception. Root/Core Cargo formatting, explicit affected-app
rustfmt, maintained-doc Prettier and whitespace checks pass. All owned jobs are terminal.

Controlled loopback, injected serial and null-audio fixtures do not qualify physical devices,
native UI, other platforms or product-scale latency/memory. Commit/push this milestone on `main`.
P03 stays approximately **50%**, overall approximately **14%**: the generic ingress workstream
remains partial. Next finish property/event/log admission and rejected-input disposal, HTTP request
conversion/disposal, Sound Card lifecycle/service, remaining module/channel audit and generator
elapsed-time accounting. P04–P18 remain unchanged.

### P03 batch 34: Shared script host-input admission and final ownership

Resumed the runtime diff from `03c76798`; the user's removal of automatic effort-switching
rules was committed and pushed separately in `d3a45301`. All retained runtime changes belong
to this milestone. Existing validation processes were observed through completion rather than
restarted on observation timeouts.

`golden_script` now owns prepared `ScriptPropertyWrite` and `ScriptCustomEvent` contracts beside
method ingress. Methods, properties, events and staged logs share **16384 items / 64 MiB** of
canonical lifetime capacity. Per-value retained storage is bounded, including spare string/vector
capacity, reference paths and JSON structure. Event object containers are normalized before
sharing so hidden spare map capacity cannot bypass accounting. Names/topics are bounded at
1024 bytes and logs at 64 KiB. Producer rejection returns the original data.

Golden IO reservations can publish a fresh protected Arc. Custom-event routing and replay keep
its canonical reservation through their existing raw Arc owners; only the background disposer
performs final destruction and polls external ownership. Small-event headroom accommodates the
normal replay window and its deferred compaction. A full lifetime budget explicitly rejects
new input and recovers when consumers release their owners.

QuickJS reserves before property/event JSON materialization, reuses the bounded parser, and
stages shallow retained handles. Its host-op queue has a hard 4096-item ceiling independent of
the configured callback call budget. Malformed/wide JSON and oversized log-level labels fail
explicitly; callback failure discards the staged prefix and host commit failure discards its
unvisited suffix. Engine property edits and node hooks borrow the retained value. Parameter
constraints validate borrowed storage before making an accepted domain-owned copy.

Owning regressions cover accepted/rejected/unsupported/missing property targets, original heap
identity, raw event routing/replay lifetime, reservation recovery and shared-budget pressure.
An isolated watchdog fixture fills the 64 MiB budget, verifies rejection and recovery across
input types, and holds 13000 raw event owners without the former 4096-item bottleneck. The
Mapping compressed-archive script/headless rejection fixture uses the new typed property intent.
No wire DTO, persistence format, frontend behavior or app-shell startup contract changes.

| Final check                                                                                                                            | Result                                                                                                                                                                                                                                   |
| -------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `cargo test-fast --locked -p golden_engine -p chataigne_processor -- --quiet`                                                          | 677 passed: engine 539 unit + 8 integration + 7 doc; processor 123. Three existing ignored manual engine benchmarks. Preparation 5m56s.                                                                                                  |
| `cargo test-fast --locked -p golden_parameters -p golden_io -p golden_script -- --quiet`                                               | 80 passed: IO 43, parameters 8, script 20 unit + 9 integration. Final preparation 1.67s, command 3.23s; isolated ingress fixture 0.30s.                                                                                                  |
| `cargo clippy --locked -p golden_parameters -p golden_io -p golden_script -p golden_engine -p Chataigne2 --all-targets -- -D warnings` | Passed, 47.29s.                                                                                                                                                                                                                          |
| `cargo test-fast --locked -p Chataigne2 --bin Chataigne2 --no-run`                                                                     | Passed, 2m10s; fresh `Chataigne2-d57ae873474d576f.exe`.                                                                                                                                                                                  |
| Disjoint compiled-name app filters, `--test-threads=4 --quiet`                                                                         | 410 passed of 906 listed: script API 8, streams 191, common streaming 61, common serial 28, HTTP 59, Sound Card 35, OSC 25, initial Formula 1, null-device audio 1, compressed-archive write rejection 1. Name sets checked for overlap. |
| `cargo check --locked --workspace --tests`                                                                                             | Passed, 22.04s.                                                                                                                                                                                                                          |
| `cargo build --locked -p Chataigne2 --bin Chataigne2`                                                                                  | Passed, 1m27s; existing UI bundle reused. No cache intervention.                                                                                                                                                                         |

**1167 distinct tests passed**; intermediate reruns count once. The documented source scan
counts **1527 files / 42 oversized**; ten new source files are at most 141 lines, with no new
size exception. The existing central node-forwarding registry and oversized engine fixture
receive only signature/constructor migrations. Runtime and tests remain separate.
Root/Core Cargo formatting, explicit affected-app rustfmt, maintained-doc Prettier and
whitespace checks pass. All owned validation jobs are terminal.

JSON scratch parsing, VM conversion, bounded-label cleanup and accepted domain copies retain
their own execution/lifetime costs. Logger records and UI projections may copy accepted data;
this is not whole-process RSS or deadline qualification. Controlled loopback, injected serial
and null-audio tests do not qualify physical devices, native UI or other platforms.

The generic ingress checkpoint is now verified: P03 advances from approximately **50% to 57%**;
the equal-weight whole-plan estimate still rounds to **14%**. HTTP request conversion/disposal,
Sound Card lifecycle/service, the remaining module/channel audit and generator elapsed-time
accounting remain open. P04–P18 retain their states.

### P03 batch 35: HTTP retained input and worker conversion

Resumed clean `main` at delivered `da3becc6`, matching fetched `origin/main`. The previous
goal turn was progress. Runtime and test work in this milestone is owned; no unrelated edits
were present. Live validation jobs were observed to terminal state without timeout-driven restarts.

HTTP native queues now accept only script argument leases or captured command invocations.
Command nodes retain a frozen snapshot and bounded override JSON instead of parsing bodies,
constructing requests, serializing them into an event and decoding them again on the actor.
App-owned `CapturedInvocation` also supplies stream command target/capture policy. Only bounded
dispatch-target decoding stays on the actor; override overlays and content conversion run on
the consuming worker. HTTP parameters admit 4 MiB total retained storage, including spare
capacity; overridden source weight is checked again before conversion. Header/form parsing
stops at 256 entries; command JSON uses a non-materializing serde preflight before expanding
at most 16384 nodes / depth 64. Converted requests retain the existing 8 MiB bound.

Source and target wake identities fence pending command admission across retirement/restore.
Applied settings still fence destination/credential changes. Once the native queue accepts an
input, its frozen data and original transport generation own the occurrence; source removal or
ordinary disable/reconfiguration does not revoke it. Module destruction may cancel it. Native
queue rejection and discarded routing copies release shallow leases, leaving original input
destruction to their canonical owners. Raw owned-request construction is confined to native
test fixtures; there is no production compatibility path.

Content conversion failure now produces an ordered warning with zero endpoint attempts and
explicitly says the request was not sent. It neither disconnects a healthy generation nor blocks
later valid requests. An unexpected preparation panic closes admission through existing worker
failure/recovery handling. Request/response retries, passive service, result receipts, native
callbacks and no-replay generation handoff are preserved.

Golden IO adds `SharedPayload` with a transferable original lease beside raw Arc routing.
Script events forward that owner through `CustomEvent::from_shared_payload`; asynchronous
capture reuses it via `try_lease_payload`. Plain JSON uses one engine fallback pool. This fixes
the cross-pool final-owner cycle exposed by script-generated command overrides. Lifetime
metadata is runtime-only and does not change event JSON or logical equality. Replacing a public
event payload cannot reuse its obsolete owner. IO/engine regressions prove pointer identity,
raw-routing retention, canonical capacity recovery and serialization equality.

The first app preservation run caught an extra test-only input pool capped below the native
mailbox's in-flight-plus-256 capacity, causing three fixture failures. That redundant quota was
removed; native fixtures now exercise their actual queue bounds. A fresh Cargo test invocation
includes all final tests, including late lifecycle and ordinary-trigger/invocation coverage.

| Final check                                                                                                                                           | Result                                                                                                                                                                                                                                                                         |
| ----------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `cargo test-fast --locked -p golden_io -p golden_script -p golden_engine -- --quiet`                                                                  | 629 passed: engine 541 unit + 8 integration + 7 doc, IO 44, script 20 unit + 9 integration; three existing ignored manual benchmarks. Preparation 6m24s.                                                                                                                       |
| Strict all-target Clippy for IO/script/engine/app; final `cargo clippy --locked -p Chataigne2 --all-targets -- -D warnings` after app fixture changes | Passed; final app run 33.72s.                                                                                                                                                                                                                                                  |
| `cargo test-fast --locked -p Chataigne2 --bin Chataigne2 module_modules_protocol_http -- --test-threads=4 --quiet`                                    | Final 66/66 passed in 0.29s after 1m34s preparation. Tests include deferred/frozen conversion, spare source capacity, source retirement, malformed-then-valid service, full native queues and actual trigger/invocation routing.                                               |
| Disjoint compiled-name app filters on fresh `Chataigne2-d57ae873474d576f.exe`, `--test-threads=4 --quiet`                                             | 429/429 passed of 914 listed: script API 8, HTTP 66, common JSON 1, module command 11, streams 191, common streaming 61, common serial 28, Sound Card 35, OSC 25, initial Formula 1, null-device audio 1, compressed-archive write rejection 1. Name sets checked for overlap. |
| `cargo check --locked --workspace --tests`                                                                                                            | Final passed, 9.76s.                                                                                                                                                                                                                                                           |
| `cargo build --locked -p Chataigne2 --bin Chataigne2`                                                                                                 | Final passed, 45.64s; no cache intervention. Frontend inputs unchanged, existing UI bundle reused.                                                                                                                                                                             |

**1058 distinct tests passed**; repeated HTTP/focused checks count once. This selection differs
from batch 34's standalone processor/parameter selection and is not a progress denominator.
Inventory: **1535 source files / 42 oversized**; eight new source files are at most 199 lines.
No new size exception, wire DTO, persistence format, frontend or app-shell startup change.
Root/Core Cargo formatting, explicit affected-app rustfmt, maintained-doc Prettier and whitespace
checks pass. All owned validation jobs are terminal; commit and push this verified milestone on `main`.

Conversion scratch, materialized request copies, graph-owned values, logger records and UI
projections have separate costs; this is not whole-process RSS or hard deadline qualification.
Fixtures use injected HTTP executors, controlled loopback, injected serial and null audio, with
no physical-device or native-UI qualification. The HTTP checkpoint is verified: P03 advances
from approximately **57% to 64%**, and the whole-plan estimate from approximately **14% to 15%**.
Next: Sound Card lifecycle/service, remaining module/channel audit and generator elapsed-time
accounting. P03 remains **IN_PROGRESS**; P04–P18 retain their states.

### P03 batch 36: Sound Card native ownership and bounded startup lifecycle

Resumed clean `main` at delivered `3e9c9cc0`, matching fetched `origin/main`; no unrelated edits.
The previous goal turn was progress. Live validation processes were observed through terminal
state without timeout-driven restarts. This milestone closes native stop/join ownership, not
the whole Sound Card service/admission/configuration checkpoint.

`SoundCardRuntime` is now a facade containing `AudioControl` and observation readers. A
prestarted native owner keeps the actual `AudioEngine` and event receiver, performs shutdown,
joins and final engine destruction, and holds one of **16 global native lifetimes** through
cleanup. Admission reserves that lifetime before engine construction. Facade stop requests
bounded control shutdown and signals cancellation; full control queues and blocking cleanup
are handled on the native owner. Facade drop never joins or creates a retirement thread.

The native producer closes its event publication and notifies the actor before cleanup can
block. Published event prefixes drain before native-failure recovery. A separate completion
lifetime resolves after actual cleanup; the same startup worker waits before constructing
a replacement. Destroy/restore remains a fresh node lifetime with existing backend recovery.
Unexpected native exit schedules the module's capped recovery deadlines.

The lifecycle service now uses `BoundedWorker`: **8 global startup lifetimes, 8 queued requests,
2 results**, with no drop-time retirement thread or synchronous join fallback. One fixed 10ms
coalescing window plus one bounded eight-item drain chooses the latest admitted selection;
continuous edits cannot restart that window indefinitely. The actor handles at most two startup
results per service turn and re-arms readiness. Full admission retains the worker; only
disconnection discards it. Driver identifiers are limited to 128 bytes at startup admission,
errors to 4096 UTF-8-safe bytes, and request identities reject exhaustion rather than saturating.
Obsolete runtime-retirement queues, detached cleanup fallbacks and blocked-retirement state
were removed from the app layer.

Four owning regressions exercise a blocked constructor, blocked native cleanup, reservation
recovery, early producer-exit notification with its retained event prefix, full startup queues
and newest-selection coalescing. Drop acknowledgement must arrive before a deliberately blocked
native operation is released; this proves ownership/ordering, not a product latency percentile.
Existing null-runtime remove/replace/undo wake-source tests remain preserved.

| Final check                                                                                                           | Result                                                                                                                                                                                                                                                                         |
| --------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `cargo test-fast --locked -p Chataigne2 --bin Chataigne2 module_modules_audio_sound_card -- --test-threads=4 --quiet` | 39/39 passed in 0.06s after 1m42s preparation.                                                                                                                                                                                                                                 |
| Disjoint compiled-name preservation filters on fresh `Chataigne2-d57ae873474d576f.exe`, `--test-threads=4 --quiet`    | 433/433 passed of 918 listed: Sound Card 39, null-device audio 1, script API 8, HTTP 66, common JSON 1, module command 11, streams 191, common streaming 61, common serial 28, OSC 25, initial Formula 1, compressed-archive write rejection 1. Name sets checked for overlap. |
| `cargo clippy --locked -p Chataigne2 --all-targets -- -D warnings`                                                    | Final passed, 11.49s.                                                                                                                                                                                                                                                          |
| `cargo check --locked --workspace --tests`                                                                            | Passed, 11.69s.                                                                                                                                                                                                                                                                |
| `cargo build --locked -p Chataigne2 --bin Chataigne2`                                                                 | Passed, 1m22s; existing UI bundle reused, no cache intervention.                                                                                                                                                                                                               |

**433 distinct tests passed**; focused Sound Card reruns count once. No Golden crate runtime
source changed, so earlier owning-core evidence is not reported as rerun here. The initial
missing generic annotation was corrected before final validation. Inventory: **1537 source
files / 42 oversized**; the two new test sources are 74/100 lines, and touched runtime modules
remain below 1000 lines. No new size exception, protocol DTO, persistence format, UI or app-shell
startup change. Root/Core Cargo formatting, affected-app rustfmt, maintained-doc Prettier and
whitespace checks pass. All owned jobs are terminal; commit and push on `main`.

Sound Card remains **50% (partial)** in the seven-workstream rubric: its event bridge still
uses unit-weight pending admission and an unbounded-to-empty actor drain. Golden Audio's
upstream `publish_event` uses `try_send` and increments `dropped_event_count` on failure;
reliable occurrence delivery under pressure is not qualified. Next audit those contracts,
disabled-node completions and ignored `set_enabled` admission failures, lifecycle-slot recovery
when deselecting a driver, and configuration construction/admission/disposal bounds. Facade
metadata, observations and graph copies retain separate memory/service costs. No physical
device, audible output, native UI, cross-platform or product-scale qualification occurred.

P03 stays approximately **64%**, whole plan approximately **15%**. Complete the remaining Sound
Card gates, module/channel audit and generator elapsed-time accounting before closing P03.
P04–P18 retain their states.

### P03 batch 37: Sound Card ordered event pressure and disabled completion service

Resumed `main` at delivered `424150f7`, matching fetched `origin/main`; the previous goal turn
was progress. The user's requested removal of effort-switching rules is already delivered in
`d3a45301`; it remains removed. No unrelated edits or physical-device activity occurred.

`golden_audio::AudioEngineBuilder::with_ordered_events` opts into bounded, FIFO backpressure.
The non-realtime control worker waits for event capacity instead of losing full-queue events.
Receiver consumption/closure and forced engine shutdown wake capacity waits. Default consumers
retain their existing best-effort contract. Graceful ordered shutdown uses `AudioControl::shutdown`
plus concurrent drain to EOF; forced `AudioEngine::shutdown`/drop cancels further publication
before blocking command admission/join, preserving its already buffered prefix. Condvar waits
use one absolute deadline and tolerate unrepresentable timeout horizons without an Instant panic.

Sound Card opts into that contract. Its native bridge retains an event while the 64-item app
mailbox is full, waiting for actor capacity or cancellation. Each actor service drains at most
eight events and re-arms remaining work; facade stop wakes a blocked bridge. Disabled nodes
continue admitted event/completion service without a tree snapshot or new startup/configuration
work. Full control admission retains the latest desired enable/disable state for retry, and
successful configuration admission invalidates the cached enable state. Lifecycle workers
suppress queued constructors and discard in-flight outcomes while disabled; they retain the
previous native-cleanup fence for later startup.

Seven new Golden Audio tests cover best-effort overflow, ordered multiplicity, receiver drop,
explicit cancellation, full-queue forced shutdown with a live unread receiver, graceful terminal
delivery and final sender closure with an extreme timeout. Five app tests prove full downstream
pressure before draining, all 80 admitted configuration generations in order, eight-item service,
zero native event drops, cancellation/lifetime recovery, retained disable admission, disabled
worker gating/resume and actual disabled-module passive wakes. All audio fixtures use NullBackend.

| Final check                                                                                              | Result                                                                                                                                                                                                                                                      |
| -------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `cargo test-fast --locked -p golden_audio --no-default-features --features analysis,playback -- --quiet` | 139 unit + 21 integration passed; seven new delivery cases.                                                                                                                                                                                                 |
| `cargo test-fast --locked -p golden_audio --no-default-features -- --quiet`                              | 106 unit + 6 integration passed; overlapping alternate-feature evidence, not added to the distinct total.                                                                                                                                                   |
| App dev-profile unit build and `module_modules_audio_sound_card --test-threads=4 --quiet`                | Prepared in 1m45s; all 44 Sound Card tests passed in 0.13s.                                                                                                                                                                                                 |
| Disjoint compiled-name preservation filters on fresh `Chataigne2-d57ae873474d576f.exe`                   | 438/438 of 923 listed: Sound Card 44, null-device audio 1, script API 8, HTTP 66, common JSON 1, module command 11, streams 191, common streaming 61, common serial 28, OSC 25, initial Formula 1, compressed-archive write rejection 1. No filter overlap. |
| Strict all-target Clippy: no-default `golden_audio` with `analysis,playback`; default `Chataigne2`       | Passed in 11.48s / 18.07s.                                                                                                                                                                                                                                  |
| `cargo check --locked --workspace --tests`                                                               | Passed in 13.54s.                                                                                                                                                                                                                                           |
| `cargo build --locked -p Chataigne2 --bin Chataigne2`                                                    | Passed in 1m24s; existing UI bundle reused, no cache intervention.                                                                                                                                                                                          |

**598 distinct tests passed**; repeated focused and alternate-feature cases count once. Initial
test-fixture import/boxing errors were corrected before final owning checks. Source inventory:
**1539 files / 42 oversized**; two new audio source files, no new size exception. No wire DTO,
generated protocol, persistence format, frontend or app-shell startup change. Root/Core Cargo
formatting, affected-app rustfmt, maintained-doc Prettier and whitespace checks pass. All owned
validation jobs are terminal. Commit and push this verified milestone on main.

Sound Card remains **50% (partial)**. Item limits and service counts do not bound variable event
bytes, observation copies, callback JSON preparation or final payload disposal on the actor.
Those contracts, idle lifecycle-slot recovery after driver deselection and configuration
construction/admission/disposal remain open. No hardware, audible output, native UI, cross-platform
or product-scale qualification is claimed. Whole plan remains **15%**, P03 **64%**; P04–P18 retain
their states. Next close the remaining Sound Card gates, then finish the module/channel audit and
generator elapsed-time accounting.

### P03 batch 38: generator elapsed-time accounting

Resumed clean `main` at delivered `d9fb2af1`, matching fetched `origin/main`. The previous Sound
Card milestone was progress. This batch closes the independent generator **time** checkpoint;
it does not claim generator channel, payload, actor-service or retirement bounds.

Signals and Metronomes now settle elapsed worker time under the active configuration before
applying each ordinary command. Their receive timeout uses an absolute sample deadline, so
continuous unchanged-rate configuration/manual commands cannot restart the timeout indefinitely.
Command service handles one received command per turn, replacing drain-until-empty loops.
Explicit reset restarts the selected items; other items retain their clocks and due occurrences.

Signals preserves accumulated phase when frequency changes, retaining its cycle baseline across
shape/seed edits. Authored phase offsets still deliberately change the waveform. Initial delayed
service accounts for all crossed cycles instead of suppressing them as a first sample. Its
waveform model and sampling math now live in app-owned `signals/waveform.rs`; the schema/lifecycle
module falls from 1095 to 895 lines. The obsolete sampled-once/reset-on-configuration state is gone.

Metronomes retains elapsed time across interval/configuration edits and manual ticks. Catch-up
keeps overdue time instead of clamping it away after 128 natural ticks per item. Further turns
can continue with zero new elapsed time. One pending command waits until the old configuration's
due prefix is accounted for before applying; the continuation does not charge that wall time
twice. An out-of-band stop flag cancels between catch-up turns, so explicit stop does not wait
for an arbitrarily old elapsed backlog. Synchronous join remains an open retirement-audit item.

Ten new owning regressions use controlled `Instant` values on the production worker state:
continuous commands/deadlines, frequency/shape continuity, initial delayed cycles, targeted reset,
manual/natural tick multiplicity, interval edits, 600 overdue ticks across bounded turns, old-tempo
prefix ordering before a tempo command, and a real worker cancelling a billion-tick backlog.
Existing generator node/script/persistence tests and module preservation suites remain intact.

| Final check                                                                                         | Result                                                                                                                                                                                                                                                                     |
| --------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `cargo test-fast --locked -p Chataigne2 --bin Chataigne2 --no-run`                                  | Final dev-profile executable prepared in 1m22s after the last fixture correction.                                                                                                                                                                                          |
| Disjoint compiled-name app filters on `Chataigne2-d57ae873474d576f.exe`, `--test-threads=4 --quiet` | 484/484 of 933 listed: generators 46, Sound Card 44, null-device audio 1, script API 8, HTTP 66, common JSON 1, module command 11, streams 191, common streaming 61, common serial 28, OSC 25, initial Formula 1, compressed-archive write rejection 1. No filter overlap. |
| `cargo clippy --locked -p Chataigne2 --all-targets -- -D warnings`                                  | Final passed in 11.63s.                                                                                                                                                                                                                                                    |
| `cargo check --locked --workspace --tests`                                                          | Passed in 17.86s.                                                                                                                                                                                                                                                          |
| `cargo build --locked -p Chataigne2 --bin Chataigne2`                                               | Passed in 1m32s; existing UI bundle reused, no cache intervention.                                                                                                                                                                                                         |

**484 distinct app tests passed.** Source inventory: **1542 files / 41 oversized**; three new
cohesive source/test files, all touched generator runtime/schema files below 1000 lines. No new
size exception, protocol DTO, generated binding, persistence format, UI or app-shell startup change.
Golden Audio's batch 37 owning evidence remains scoped to that unchanged source. Root/Core
Cargo formatting, affected-generator rustfmt, maintained-doc Prettier (preserving line endings)
and whitespace checks pass. Commit and push this verified milestone on `main`.

Generator elapsed-time accounting advances **0% to 100%** in the seven-workstream rubric.
P03 advances **64% to 79%**, `(100 + 100 + 100 + 100 + 50 + 0 + 100) / 7`; the equal-weight
whole-plan average still rounds to **15%**. Sound Card remains partial and the app-module audit
remains open. That audit includes the generators' unbounded command channels, variable event
payloads, coalesced callback fan-out and synchronous shutdown joins. No physical-device, native-UI,
platform or product-scale qualification is claimed. Next: Sound Card retained-byte/preparation/
admission gates, then the remaining app-module audit. P04–P18 retain their states.

### P03 batch 39: Sound Card idle lifecycle-slot recovery

Resumed clean `main` at delivered `93236757`, matching fetched `origin/main`. The previous goal
turn was progress: Sound Card event pressure and generator time milestones were pushed. This
milestone closes the known idle startup-slot leak while retaining the native-cleanup fence.

Clearing the audio driver now submits an idempotent idle selection to the existing lifecycle
worker. It cancels queued startup construction through the shared gate, retains that worker
during in-flight construction and native cleanup, then exits and releases its slot. Full idle
admission keeps ownership and schedules a capped retry; the cancelled startup identity is
invalidated even when the idle mailbox is full. Reselecting that same driver therefore admits
fresh startup instead of waiting forever on a discarded result. A newer admitted selection
can supersede idle in the bounded coalescing drain; reselection after worker exit uses existing
recovery. Expected idle exit clears lifecycle state without reporting an unexpected failure.

Idle retirement also runs while the module is disabled, and its completion service does not
request a tree snapshot. Idle uses reserved identity zero without advancing the startup counter,
so identity exhaustion cannot prevent retirement. Startup IDs remain checked and monotonic.
The app startup/retry coordinator now lives in `sound_card/integration/lifecycle.rs` (289 lines);
the integration coordinator falls from 940 to 701 lines. No lifecycle thread or join was added.

Seven new owning regressions verify idle retirement while disabled/identity-exhausted, blocked
construction, deliberately blocked native cleanup, newer-selection coalescing, repeated real
null-runtime deselect/reselect, disabled module deselection, and full idle admission followed by
reselection of the cancelled driver. The blocked-cleanup test checks that both native and startup
capacity stay occupied until the release gate opens. All native endpoints are NullBackend.

| Final check                                                                                                           | Result                                                                                                                                                                                                                                                                     |
| --------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `cargo test-fast --locked -p Chataigne2 --bin Chataigne2 module_modules_audio_sound_card -- --test-threads=4 --quiet` | Final 51/51 passed in 0.38s after 1m25s preparation.                                                                                                                                                                                                                       |
| Disjoint compiled-name preservation filters on fresh `Chataigne2-d57ae873474d576f.exe`                                | 491/491 of 940 listed: Sound Card 51, generators 46, null-device audio 1, script API 8, HTTP 66, common JSON 1, module command 11, streams 191, common streaming 61, common serial 28, OSC 25, initial Formula 1, compressed-archive write rejection 1. No filter overlap. |
| `cargo clippy --locked -p Chataigne2 --all-targets -- -D warnings`                                                    | Final passed in 11.68s.                                                                                                                                                                                                                                                    |
| `cargo check --locked --workspace --tests`                                                                            | Passed in 8.14s.                                                                                                                                                                                                                                                           |
| `cargo build --locked -p Chataigne2 --bin Chataigne2`                                                                 | Passed in 1m11s; existing UI bundle reused, no cache intervention.                                                                                                                                                                                                         |

**491 distinct app tests passed**; repeated focused cases count once. A fixture constant import
was corrected before final checks. Inventory: **1543 source files / 41 oversized**; one new
cohesive app lifecycle source, no new size exception. No Golden runtime, protocol DTO, generated
binding, persistence format, UI or app-shell startup change. Earlier audio evidence is retained
at its unchanged source, not claimed as rerun here. Root/Core Cargo formatting, affected-app
rustfmt, maintained-doc Prettier with preserved line endings and whitespace checks pass. All owned
validation jobs are terminal; commit and push this verified milestone on `main`.

Sound Card remains **50% (partial)**: retained event bytes, worker-side callback/log preparation,
final variable-payload disposal, configuration preparation/admission and remaining failure/
reconfiguration fencing still need qualification. Fixed item counts do not bound retained bytes.
The rest of the app-module/channel audit remains open, including generator retirement and event
fan-out. Whole plan stays **15%**, P03 **79%**; P04–P18 retain their states. No physical-device,
audible output, native-UI, cross-platform or product-scale qualification occurred.

### P03 batch 40: audio event byte budgets and shutdown outcomes

Resumed clean `main` at delivered `412eb7ca`, matching fetched `origin/main`. The preceding
idle-lifecycle milestone was progress. This batch bounds event value storage at its native
publication boundary and the app mailbox, preserving accepted playback identities and paths.

Golden Audio's event channel now admits against item capacity **and 8 MiB retained event bytes**;
each prepared event admits at most **1 MiB**. Ordered delivery waits for either capacity, while
default best-effort delivery rejects the new event and retains its prefix. Cancellation, sender
closure and receiver drains use the same mutex/condition-variable state, including exact byte
release on dequeue and final-sender notification. `AudioEvent::retained_bytes` is the public,
exhaustively matched accounting boundary: inline event values, spare string/path capacity and a
conservative B-tree node allowance. Sound Card uses it for its **64-event / 8 MiB** mailbox;
native bridge capacity waits and eight-event actor service remain preserved.

Immutable string IDs enforce **64 KiB** through constructors and deserialization and compact
spare capacity without changing accepted text. Playback path storage is checked against **128 KiB**
before command admission/sequence allocation, including spare capacity. Diagnostic/display text
is compacted on the non-realtime control worker with visible truncation: 4096-byte text,
256-byte codes/context keys, 16 context entries with 1024-byte values plus a truncation marker.
Accepted playback IDs/paths stay verbatim. Current worst-case status and playback event tests
prove these bounds fit the per-event limit, rather than truncating accepted identities afterward.

The strengthened large-payload fixture initially reproduced **79 terminal outcomes for 80 accepted
requests** during graceful shutdown. The control worker previously broke directly to device/render
cleanup and `ShutdownComplete`, leaving a loading playback without a terminal outcome. It now
resolves returned render events and every remaining registration before the terminal marker.
Pending/current playbacks and already-stopping voice generations are tracked separately; replacing
a playback ID cannot erase an older requested-stop outcome. Remaining stop notifications retain
their prior reason or use `StopAll`, in original start order. Final observation counts become zero.
External renderer owners must quiesce their callbacks; managed/device shutdown remains native-owned.

Thirteen new Golden Audio regressions cover byte-only pressure/recovery, cancellation, sender
lifetimes, spare capacity, identifier deserialization, marked diagnostic compaction, worst-case
identity preservation, path admission before sequence allocation, pending/playing shutdown,
replacement generations, already-returned outcomes and earlier stop reasons. The new real
NullBackend app fixture fills byte capacity before the item limit, then drains **all 80 accepted
requests**, exact IDs/paths, bounded service, zero drops and `ShutdownComplete`. It uses an invalid
overlong filename, creating no media fixture and opening no physical audio endpoint.

| Final check                                                                                                           | Result                                                                                                                                                                                                                                                                     |
| --------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `cargo test-fast --locked -p golden_audio --no-default-features --features analysis,playback -- --quiet`              | Final 152 unit + 21 integration passed.                                                                                                                                                                                                                                    |
| `cargo test-fast --locked -p golden_audio --no-default-features -- --quiet`                                           | Final 115 unit + 6 integration passed; overlapping alternate-feature evidence, not added to the distinct total.                                                                                                                                                            |
| `cargo test-fast --locked -p Chataigne2 --bin Chataigne2 module_modules_audio_sound_card -- --test-threads=4 --quiet` | Final 52/52 passed in 0.38s after 1m32s preparation, including the strengthened 80-request shutdown fixture.                                                                                                                                                               |
| Disjoint compiled-name preservation filters on fresh `Chataigne2-d57ae873474d576f.exe`                                | 492/492 of 941 listed: Sound Card 52, generators 46, null-device audio 1, script API 8, HTTP 66, common JSON 1, module command 11, streams 191, common streaming 61, common serial 28, OSC 25, initial Formula 1, compressed-archive write rejection 1. No filter overlap. |
| Strict all-target Clippy: no-default `golden_audio` with `analysis,playback`; default `Chataigne2`                    | Final passed in 1.68s / 14.00s.                                                                                                                                                                                                                                            |
| `cargo check --locked --workspace --tests`                                                                            | Final passed in 9.36s.                                                                                                                                                                                                                                                     |
| `npm run check:generated --workspace golden_audio_ui`                                                                 | Passed after final source changes; generator output matches checked-in bindings. Wire representations/fields are unchanged; Rust identity/path validation is stricter.                                                                                                     |
| `cargo build --locked -p Chataigne2 --bin Chataigne2`                                                                 | Final passed in 1m11s; existing UI bundle reused, no cache intervention.                                                                                                                                                                                                   |

**665 distinct tests passed**; repeated focused and alternate-feature cases count once. Temporary
test-module/fixture wiring errors were corrected before final checks; the 79/80 reproduction led
to the lifecycle repair rather than a weaker assertion. Inventory: **1546 source files / 41
oversized**. Three new cohesive storage/test sources; touched audio runtime files stay below 1000
lines. No new size exception, persisted project format, frontend or app-shell startup change.
Root/Core Cargo formatting, affected-app rustfmt, maintained-doc Prettier with preserved line
endings and whitespace checks pass. All owned jobs are terminal; commit and push on `main`.

Sound Card stays **50% (partial)**. Queue value budgets exclude fixed channel metadata/spare deque
storage, one normalized in-flight event per publishing stage, producer preparation scratch,
observations, decoder/media resources, actor copies and callback/replay owners. Final raw-event
disposal, callback/log preparation and publication receipts still run on or need coordination with
the actor; observation/configuration preparation, command admission and remaining failure/
reconfiguration fencing remain open. These are not whole-process RSS or actor deadline claims.
Next finish those Sound Card contracts, then the remaining app-module/channel audit. Whole plan
stays **15%**, P03 **79%**; P04–P18 retain their states. No physical-device, audible output,
native-UI, cross-platform or product-scale qualification occurred.

### P03 batch 41: custom-event replay bytes and callback quota recovery

Resumed clean delivered `6b145fc5` on `main`, matching fetched `origin/main`. Preparing Sound Card
callbacks exposed a prerequisite: replay could retain enough large callbacks to exhaust the shared
payload pool before the 8192-event limit, preventing capacity recovery on the publishing worker.

Golden Engine and its independent UI read-model log now target **64 MiB of custom JSON**, alongside
their existing item limits. Native callback preparation attaches its full retained weight, including
spare strings/arrays and a conservative B-tree node allowance. Runtime-only weight metadata follows
the immutable Arc and invalidates after replacement or `Arc::make_mut`; serialization and logical
equality are unchanged. Unprepared producers get a bounded 4096-value / 64-level accounting walk;
unknown complexity is charged as an oversized entry. Replay accounting does not rescan prepared
callbacks; DTO capture still copies JSON on the actor and remains separate work.

`collect_event_batch` now requires exclusive engine access: capture the live batch and cached weights,
then enforce replay bytes. All production, project-replacement and test consumers use that boundary.
This preserves every live occurrence within the existing count limit even when one mutation turn
exceeds the byte target. Engine eviction clears payload leases from the lazy dead prefix immediately,
without repeated full-vector compaction. Read-model eviction updates indexes, weights and the existing
resync watermark; an empty capture also applies a lowered budget. Latest replacement and ordinary
callback retention policies are preserved.

Both logs retain one oversized entry. Hard byte guarantees therefore require producer admission below
the replay target; native callback payloads admit at most 32 MiB, below the default 64 MiB. Temporary
live captures, exported event owners, other event kinds, projection storage and final actor-side DTO
copy/disposal costs remain separate. This is not a whole-process RSS or actor-deadline qualification.

Final validation passed:

| Check                                                                                                                                                           | Result                                                                                                                                                                                                                                                                                                        |
| --------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `cargo test-fast --locked -p golden_engine -p golden_io -p golden_script -p golden_protocol -p golden_transport_server -- --quiet`                              | Passed: 550 engine unit + 8 integration + 7 doc, 44 IO, 29 script, 1 protocol, 46 transport; **685 distinct**. Four existing ignored tests remain ignored.                                                                                                                                                    |
| Fresh app executable, compiled-name disjoint preservation filters                                                                                               | Passed: **497/497 of 942 listed**. Script API 9; Sound Card 52; generators 46; null-device audio 1; HTTP 66; common JSON 1; module command 11; streams 191; common streaming 61; common serial 28; OSC 25; initial Formula 1; compressed-archive write rejection 1; state/read-model consumers 4. No overlap. |
| `cargo clippy --locked -p golden_engine -p golden_io -p golden_script -p golden_protocol -p golden_transport_server -p Chataigne2 --all-targets -- -D warnings` | Passed.                                                                                                                                                                                                                                                                                                       |
| `cargo check --locked --workspace --tests`                                                                                                                      | Passed.                                                                                                                                                                                                                                                                                                       |
| `cargo build --locked -p Chataigne2 --bin Chataigne2`                                                                                                           | Passed; existing UI bundle reused.                                                                                                                                                                                                                                                                            |

Inventory: **1547 source files / 41 oversized** using the maintained scan
policy; one new focused replay test file and no new size exception. Next finish Sound Card callback/log
preparation and publication receipts, retained raw-event disposal, observation/configuration admission
and failure/reconfiguration fences, then the remaining app-module/channel audit. Progress stays
**15% whole plan / 79% P03 / 50% Sound Card**; P04–P18 remain unstarted.

Root/Core Cargo formatting, affected app rustfmt, maintained-doc Prettier and whitespace checks
passed. No protocol DTO fields, persisted project format, frontend or app-shell startup changed.
All validation jobs are terminal. No physical endpoint, native UI, cross-platform or product-scale
qualification occurred. Commit and push this verified milestone on `main` before the next batch.

### P03 batch 42: Sound Card prepared events and publication receipts

Resumed clean `main` at delivered `dd6aeb7e`, matching fetched `origin/main`; the preceding replay
milestone was progress. The app's Sound Card native owner now builds immutable event envelopes
before mailbox admission: raw `AudioEvent`, callback JSON with its existing retention policy,
outgoing log and connection log/warning. The 64-item / 8 MiB mailbox charges all prepared storage.
One shared **2048-item / 256 MiB** pool keeps final envelope ownership off the actor through queue
pressure, callback receipts, cancellation and node destruction. Callback JSON keeps its existing
canonical app pool; the envelope never installs another final owner for that raw JSON Arc.

Measured callback drafts survive full callback/envelope lifetime admission without serialization
retries or outcome replacement. Only the native owner waits, on a cancellable 100ms capacity retry;
there is no new actor polling or producer-driven worker. Prepared playback identities/paths remain
verbatim. Defensive preparation-contract rejection preserves the raw native result and publishes
an explicit warning; current normalized maximum playback fields fit without rejection/truncation.

The actor retains one FIFO batch of at most eight events and drains no next batch while publication
is pending. Connection warnings and callbacks own separate single-edit receipts. An unpublished edit
retries, while a delivered callback is never replayed after a later edit/subscriber failure. Logs
emit once. The shared receipt helper also serves existing HTTP and generic callback delivery.
Device feedback revalidates selection and a bound parameter before retries: deselection or selector
removal cannot trap the original callback behind an invalid warning target. Disabled nodes finish
pending receipts even after the native facade disappears, without requiring a tree snapshot.
Fallible preparation setup installs the native owner before setup can fail, preserving the native
lifetime through blocking cleanup on that failure path as well as normal retirement.

Fifteen added Sound Card regressions cover retained-pool recovery, unchanged
callback shape/retention, largest playback fields, final disposal off the consumer, receipt failure
before/after publication, deselection, removed warning targets, disabled facade-free service,
native capacity cancellation/recovery and failed preparation setup through blocked cleanup. Existing
80-request byte-pressure/shutdown preservation remains in the owning suite.

Selection validation caches the bound parameter and actual driver/selection values while warning
receipts wait; 128 pending-service calls perform one validation. Feedback closes before callback
admission and cannot reopen after a later selection change. Selection/configuration parsing and
control-value fence copies remain explicit next-step work, alongside apply-time generation fences.
Final validation below covers this refinement.

| Check                                                                                                                 | Result                                                                                                                                                                                                                                                                                                                    |
| --------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `cargo test-fast --locked -p Chataigne2 --bin Chataigne2 module_modules_audio_sound_card -- --test-threads=4 --quiet` | Final 67/67 passed in 0.53s after 1m42s preparation. NullBackend only.                                                                                                                                                                                                                                                    |
| Fresh compiled-name disjoint preservation filters                                                                     | 445/445 passed: generators 46, null-device audio 1, script API 9, HTTP 66, common JSON 1, module command 11, streams 191, common streaming 61, common serial 28, OSC 25, initial Formula 1, compressed-archive write rejection 1, state/read-model consumers 4. Together with Sound Card: **512 distinct of 957 listed**. |
| `cargo clippy --locked -p Chataigne2 --all-targets -- -D warnings`                                                    | Final passed in 22.88s.                                                                                                                                                                                                                                                                                                   |
| `cargo check --locked --workspace --tests`                                                                            | Final passed in 9.78s.                                                                                                                                                                                                                                                                                                    |
| `cargo build --locked -p Chataigne2 --bin Chataigne2`                                                                 | Final passed in 1m29s, existing UI bundle reused.                                                                                                                                                                                                                                                                         |

Inventory: **1552 source files / 41 oversized**, using the maintained scan policy. Five focused
runtime/test sources; no new size exception. Sound Card remains **50% partial**, P03 **79%**, whole
plan **15%**. Observation/active-voice/value copies, logger/UI DTO copies, configuration preparation
and admission, remaining reconfiguration/failure fences and the app-module/channel audit remain
open. These event ownership bounds do not qualify whole-process RSS, actor deadlines, physical
devices, native UI, other platforms or product-scale workloads.

Root/Core Cargo formatting, affected app rustfmt, maintained-doc Prettier and whitespace checks
passed. No protocol DTO fields, persisted project format, frontend or app-shell startup changed.
Temporary test-import/borrow wiring errors were corrected before final checks. All owned jobs are terminal. Commit and push this verified milestone to `origin/main` before the next batch.

### P03 batch 43: Native observation capture and shared UI JSON

Resumed clean `main` at delivered `cbb30938`, matching fetched `origin/main`. Golden Audio now
supports borrowed snapshot inspection and a bounded owned-storage walk before copying, including
spare capacities, nested device/fingerprint metadata and analysis results. Sound Card's native owner
reserves up to **8 MiB / 65,536 collection entries** per capture; a shared **128-item / 256 MiB**
pool retains copied snapshots through final background disposal. The actor reads a shared handle
and small health fields under a short publication lock. Native copying holds neither that lock nor
the event-capacity lock. Full mailboxes and callback-preparation pressure continue capture on timed,
cancellable waits. Repeated reads of one sequence skip value/telemetry reconstruction.

Capture failure preserves the last good frame, suppresses stale value/telemetry publication and
publishes an independent warning with a retryable single-edit receipt. Health scalars keep updating;
tests verify control admission and recovery while capture quota is full. Metering errors do not
disable audio. Warning receipts remain serviceable after native facade retirement. Configuration
reads device metadata separately through a retained opaque reader; its **64-handle / 64 KiB** pool
charges wrapper metadata only. The mutable engine-owned source snapshot and configuration copies
remain outside the capture budget. Its final source-reader release occurs off the actor.

UI custom-event DTOs share the canonical JSON Arc through live publication, replay and exported DTO
clones. Their wire JSON and generated TypeScript payload type stay unchanged; generated documentation
was refreshed from Rust. Regression tests check pointer identity, serialization roundtrip, exported
replay quota retention and final capacity recovery. The former connection helper was removed after
strict Clippy identified it as obsolete; its expanded test now exercises the production health header.

| Check                                                                                                                                                                                                                           | Result                                                                                                                                        |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------- |
| `cargo test-fast --locked -p Chataigne2 --bin Chataigne2 module_modules_audio_sound_card -- --test-threads=4 --quiet`                                                                                                           | Final 69/69 passed in 0.51s after 1m41s preparation; NullBackend only.                                                                        |
| Fresh compiled-name disjoint preservation filters from batch 42                                                                                                                                                                 | 445/445 passed again after final app rebuild; **514 distinct of 959 listed** with Sound Card. Includes canonical callback replay quota tests. |
| `cargo test-fast --locked -p golden_engine -p golden_io -p golden_script -p golden_protocol -p golden_transport_server -p golden_audio --no-default-features --features golden_audio/analysis,golden_audio/playback -- --quiet` | Passed after 7m02s preparation: core/IO/script/protocol/transport **686**, audio **177**; four existing ignored.                              |
| `cargo test-fast --locked -p golden_audio --no-default-features -- --quiet`                                                                                                                                                     | 125/125 passed; alternate feature subset, not an additional distinct total.                                                                   |
| `cargo clippy --locked -p Chataigne2 -p golden_audio -p golden_engine -p golden_io -p golden_script -p golden_protocol -p golden_transport_server --all-targets -- -D warnings`                                                 | Final passed in 25.59s. Earlier obsolete-helper finding corrected before final checks.                                                        |
| `cargo check --locked --workspace --tests`                                                                                                                                                                                      | Final passed in 30.98s.                                                                                                                       |
| `cargo build --locked -p Chataigne2 --bin Chataigne2`                                                                                                                                                                           | Final passed; existing UI bundle reused.                                                                                                      |
| `npm run check`; `npm test`; `npm exec -- vitest run --root packages/golden-graph-ui`                                                                                                                                           | Zero Svelte errors/warnings, generated audio binding check passed; 141 + 13 tests passed.                                                     |
| `cargo run --locked -p golden_codegen_support --bin golden_codegen -- ui-protocol target/p03-b43-ui-protocol`                                                                                                                   | Generated output exactly matches checked-in protocol after applying the Rust-generated comment change.                                        |

Root/Core Cargo formatting, affected app rustfmt, maintained-doc Prettier and whitespace checks passed.
Inventory: **1558 source files / 41 oversized**; six focused new sources, no new size exception.
Sound Card remains **50% partial**, P03 **79%**, whole plan **15%**. Telemetry DTO/JSON preparation,
value-service bounds/receipts, active-voice/logger copies, configuration preparation/admission,
remaining reconfiguration/apply-time/failure fences and the app-module/channel audit remain open.
These limits do not qualify whole-process RSS, atomic health/meter sampling, actor deadlines,
physical devices, native UI, other platforms or product-scale workloads. No persisted project format
or app-shell startup changed. All owned jobs are terminal. Commit and push this verified milestone
to `origin/main` before the next batch.

### P03 batch 44: Native telemetry preparation and bounded value publication

Resumed clean `main` at delivered `ffb0eb41`, matching fetched `origin/main`; the prior observation
milestone was progress. Sound Card's native owner now maintains the active voice catalog, builds
telemetry DTOs, validates encoded JSON and retains its canonical JSON allocation. Actor service
creates shared event handles and checks publication receipts; it no longer clones telemetry DTOs,
sorts/copies active voices, serializes telemetry or compares entire telemetry trees. Equal native
JSON reuses its payload. One pending telemetry publication retries an unapplied edit without
optimistic acknowledgement; exported live/replay DTOs retain the original lifetime quota.

Preparation admits two stages before allocating their variable storage: typed/encoding/validation
scratch (at most **25 MiB**) and JSON construction scratch derived from validated node counts/wire
bytes (at most **128 MiB**). A shared **128-item / 512 MiB** pool accounts for both stages, then
seals final JSON at its measured weight. Temporary buffers drop before their reservations release.
Small-frame admission below the maximum allowance is verified. Frames admit **4 MiB wire / 32 MiB
retained / 65,536 JSON values / depth 64**. The shared JSON preflight now exposes validated value
counts; its storage estimator also covers sparse B-tree nodes rather than charging 256 bytes per
entry. HTTP/stream/script consumers retain their regression coverage under that correction.

The native catalog admits **256 voices / 128 MiB**, matching the app's explicit native voice limit.
Terminal outcomes match the full voice identity. Golden Audio's `PlaybackFailure::terminated_voice`
now distinguishes an actual decoder retirement from preparation/control-admission failure. All
Rust producers/consumers were updated together. Old-generation failures preserve replacements;
failed loads/stops preserve still-playing voices. Script callback arguments and UI telemetry fields
remain unchanged; generated audio bindings match. Obsolete actor playback/error caches were removed.

Value delivery services at most **64 receipt checks/cursor positions per turn**, counting skipped
and unchanged entries. At most 64 pending/retry cells retain their own receipts. A pinned frame
finishes under continuous newer captures. Applied edits alone populate a balanced value cache;
binding changes invalidate that cache and reject old-generation receipt promotion. Failed prefixes
retry one cell at a time, once per frame, isolating missing targets from their valid suffix. Permanent
failure then waits for a new native capture instead of spinning actor continuations. Pitch note
copies admit 64 UTF-8 bytes. These are latest-value projections: retirement can cancel them while
ordered native lifecycle callbacks retain the previously verified FIFO policy. Configuration still
observes the last valid native plan while replacement configuration is pending/rejected.

| Check                                                                                                                 | Result                                                                                                                              |
| --------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------- |
| `cargo test-fast --locked -p Chataigne2 --bin Chataigne2 module_modules_audio_sound_card -- --test-threads=4 --quiet` | Final **79/79** passed in 0.52s after 2m01s preparation; NullBackend only.                                                          |
| Fresh compiled-name disjoint preservation filters from batch 43                                                       | Final **446/446** passed (common JSON now 2 cases); **525 distinct of 970 listed** with Sound Card.                                 |
| `cargo test-fast --locked -p golden_audio --no-default-features --features analysis,playback -- --quiet`              | **179/179** passed (158 unit + 21 integration). Includes decoder failure/replacement identity and full stop-admission preservation. |
| `cargo test-fast --locked -p golden_audio --no-default-features -- --quiet`                                           | **125/125** passed; overlapping alternate-feature evidence, not an added distinct total.                                            |
| `cargo clippy --locked -p Chataigne2 -p golden_audio --all-targets -- -D warnings`                                    | Final passed in 16.55s.                                                                                                             |
| `cargo check --locked --workspace --tests`                                                                            | Final passed in 10.42s.                                                                                                             |
| `npm run check:generated --workspace golden_audio_ui`                                                                 | Passed; no generated UI contract drift. The new optional terminal identity belongs to the Rust native event API.                    |
| `cargo build --locked -p Chataigne2 --bin Chataigne2`                                                                 | Final passed; existing UI bundle reused.                                                                                            |

Final Root/Core Cargo formatting, affected app rustfmt, maintained-doc Prettier and whitespace checks
passed. Inventory: **1564 source files / 41 oversized**; seven focused new sources and one obsolete
actor playback source removed. Initial import/visibility/fixture wiring was corrected before final
checks; the stop-admission test now fills the fixture's actual command capacity.

Sound Card remains **50% partial**, P03 **79%**, whole plan **15%**. Configuration preparation/admission
and cache retirement, downstream logger ownership, remaining apply-time/reconfiguration/failure
fences and the rest of the app-module/channel audit remain open. These bounds do not qualify
whole-process RSS, atomic health/meter sampling, realtime control ordering, actor deadlines,
physical devices, native UI, other platforms or product-scale workloads. No persisted project format
or app-shell startup changed. All owned jobs are terminal. Commit and push this verified milestone
to `origin/main` before the next batch.

### P03 batch 45: Worker configuration conversion and acknowledged bindings

Resumed clean `main` at delivered `77e4ec51`, matching fetched `origin/main`. Sound Card's pure
configuration conversion now lives in `runtime/configuration` and runs on one shared native
preparation worker. Actor requests retain a canonical Core snapshot lease and an opaque native
reader instead of copying tree/device data. The existing declaration-aware Sound Card resolver
selects revision roots consistently with conversion; admission keys check connection/parameter/value
subtree revisions, module UUID and effective enabled state before submitting a prepared result.
Actual edits invalidate old work before inbox callbacks run. Cancellation and dropped worker jobs
have explicit outcomes; disabled/retired modules cannot apply their old preparation.

Preparation admits 16 queued requests, 32 live tickets and one worker lifetime. Source conversion
admits **131,072 nodes / 32 MiB label/value text**, with individual text limited to 1 MiB. The
existing Core snapshot pool separately retains source storage through final disposal. Device-only
inspection admits **8 MiB / 131,072 entries** independently of oversized metering storage. The
worker reserves **256 MiB** scratch before metadata copying/conversion; retained plans admit
**64 MiB each** through a shared **128-item / 1 GiB** lifetime pool. Warning/error messages compact
to 4 KiB on the worker, before scratch release. Configuration capacities and binding/warning
storage are counted; prepared plans dispose off the actor. These are ownership budgets, not RSS
or measured actor/realtime deadline qualification.

Golden Audio configuration commands now carry `Arc<AudioConfiguration>`; all constructors and
consumers changed together. The JSON serialization shape is unchanged. Full queue admission drops
only a shared handle; the producer can retry its original configuration, and ownership/cloning
occurs on the native control worker. The app retains the ready plan across retries. Temporary
preparation exhaustion retries after 250 ms without disabling existing audio; permanent authored
errors retain the existing disable behavior. Readonly value projection pauses while preparing or
awaiting application so its own writes cannot continuously invalidate a prepared source. Health,
telemetry, ordered events and disabled service continue.

Only a matching native `ConfigurationApplied` promotes the retained binding plan and resets its
value-cache epoch. Matching rejection preserves the old plan/epoch; duplicate submissions are
refused while application is pending. Tests exercise a real native rejection after a newer
generation, alongside stale-source invalidation, oversized input admission, cancellation, dropped
jobs, capacity failure, configuration storage and same-Arc full-queue retry.

| Check                                                                                                                 | Result                                                                     |
| --------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------- |
| `cargo test-fast --locked -p Chataigne2 --bin Chataigne2 module_modules_audio_sound_card -- --test-threads=4 --quiet` | Final **84/84** passed in 0.52s after 1m44s preparation; NullBackend only. |
| Fresh compiled-name disjoint preservation filters from batch 44                                                       | Final **446/446** passed; **530 distinct of 975 listed** with Sound Card.  |
| `cargo test-fast --locked -p golden_audio --no-default-features --features analysis,playback -- --quiet`              | **182/182** passed (161 unit + 21 integration).                            |
| `cargo test-fast --locked -p golden_audio --no-default-features -- --quiet`                                           | **128/128** passed; overlapping alternate-feature evidence.                |
| `cargo clippy --locked -p Chataigne2 -p golden_audio --all-targets -- -D warnings`                                    | Final passed in 14.24s.                                                    |
| `cargo check --locked --workspace --tests`                                                                            | Final passed in 8.27s.                                                     |
| `npm run check:generated --workspace golden_audio_ui`                                                                 | Passed; no generated contract drift.                                       |
| `cargo build --locked -p Chataigne2 --bin Chataigne2`                                                                 | Final passed in 1m51s; existing UI bundle reused.                          |

Inventory: **1570 source files / 41 oversized**, using the maintained scan policy; six focused
new source files, no new size exception. Root/Core Cargo formatting, affected app rustfmt and
maintained-doc formatting/whitespace checks passed. Fixture path resolution
and enum constraints were corrected before final tests. One intermediate linker failure came
from running the previous test executable concurrently; final rebuild/test ran without that lock.
Prior core/IO/script/protocol/transport and frontend/graph evidence remains historical batch 43.

Sound Card remains **50% partial**, P03 **79%**, whole plan **15%**. Remaining Sound Card work:
probe/selection and raw inspector ownership, derived structure/default routes, enum choices,
warning publication/cache retirement, logger ownership and remaining reconfiguration/apply-time/
failure fences. The remaining module/channel audit is also open. Matching control acknowledgement
does not qualify P04 render ordering. No persisted project format or app-shell startup changed;
physical devices, native UI, other platforms and product-scale workloads remain unqualified.
Commit and push this verified milestone to `origin/main` before the next batch.

### P03 batch 46: Route-warning reconciliation and bounded publication

Resumed clean `main` at delivered `5e8e4d5a`, matching fetched `origin/main`. The previous
turn was progress. Sound Card configuration workers now reconcile route diagnostics against the
snapshot's applied warning state. Changed messages/details update correctly, unchanged diagnostics
produce no edit, obsolete diagnostics clear, and other warning owners are preserved. The actor's
whole-set string cache and optimistic acknowledgement were removed. Reconciliation owns only the
two existing unresolved-channel/physical-channel IDs within the module subtree. Reparenting or
other mutations after source validation remain part of the open apply-time fence checkpoint.

The worker bounds the combined node/warning scan and resulting patch count at **262,144**.
Configuration and warning plans are separate immutable payloads in the existing **128-item / 1 GiB**
pool, with **64 MiB combined final storage** per preparation. They consume two lifetime items;
warning sealing can temporarily charge final warning storage alongside the existing **256 MiB**
conversion scratch allowance. Warning text/details retain their **4 KiB** limits. All diff building,
text comparison and final plan disposal stay off the actor. Warning-plan capacity pressure is
retryable and preserves current audio, including exhaustion after successful conversion.

Actor warning service checks or stages at most **16 receipts/actions per turn**, with at most 16
pending/retry cells. Failed prefixes retry in isolation once. Missing targets cannot poison valid
suffixes or spin actor continuations; an otherwise idle failure requests fresh reconciliation after
250 ms. Before capturing replacement preparation, the runtime cancels unqueued old warning actions
and settles already admitted receipts. This prevents late old warning writes from invalidating the
replacement diff. Warning projection starts after configuration admission and continues independently
of native binding acknowledgement. These diagnostics are latest state, not ordered trigger occurrences.

Presentation-only metadata events no longer request Sound Card snapshots or redrive configuration.
Labels/enabled-state edits retain their previous behavior. Seven new tests replace the old optimistic
cache test: changed/unchanged/removed warnings and owner isolation, 200-warning bounded service,
failed-prefix retry, removed targets, replacement quiescence, retained quota through receipts, and
the real runtime's preparation gate. Existing preparation-capacity coverage now also exhausts the
warning-plan lifetime slot after conversion, while preserving the native enabled state.

Preservation testing exposed an existing nonblocking WebSocket fixture assumption: sender completion
does not guarantee the next segment is already readable after a budget reset. The fixture now waits
for readiness before its two read phases. Runtime transport limits are unchanged; the full stream
suite and five consecutive focused fixture runs pass. Batch 45's ledger entry was moved after batch
44 to restore chronological order without changing its evidence.

| Check                                                                                                                 | Result                                                                                                   |
| --------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------- |
| `cargo test-fast --locked -p Chataigne2 --bin Chataigne2 module_modules_audio_sound_card -- --test-threads=4 --quiet` | Final **90/90** passed in 0.53s after 1m44s preparation; NullBackend only.                               |
| Fresh compiled-name disjoint preservation filters from batch 45                                                       | Final **446/446** passed; **536 distinct of 981 listed** with Sound Card. Includes all 191 stream tests. |
| Exact socket-budget fixture repeated after correction                                                                 | **5/5** passed; repeated evidence, not extra distinct tests.                                             |
| `cargo clippy --locked -p Chataigne2 --all-targets -- -D warnings`                                                    | Final passed in 14.12s.                                                                                  |
| `cargo check --locked --workspace --tests`                                                                            | Final passed in 8.07s.                                                                                   |
| `cargo build --locked -p Chataigne2 --bin Chataigne2`                                                                 | Final passed in 1m38s; existing UI bundle reused.                                                        |

Inventory: **1573 source files / 41 oversized**, using the maintained scan policy; three focused
new sources, no new size exception. Final Root/Core Cargo formatting, affected app rustfmt and
maintained-doc formatting/whitespace checks passed. Initial strict Clippy
caught two test-only needless borrows, corrected before final checks. No public protocol DTO,
generated contract, persisted project format or app-shell startup changed. Audio and broader
Core/frontend evidence remains historical as identified in the handoff above.

Sound Card remains **50% partial**, P03 **79%**, whole plan **15%**. Remaining checkpoints are
probe/selection and raw inspector ownership, derived structure/default routes, enum choices,
value-cache retirement, downstream logger ownership and remaining reconfiguration/apply-time/
failure fences, followed by the remaining app-module/channel audit. These bounds do not qualify
whole-process RSS, actor/realtime deadlines, physical devices, native UI, other platforms or
product-scale workloads. Commit and push this verified milestone to `origin/main` before the next batch.

### P03 batch 47: Fixed value-cache storage and native lifetime ownership

Resumed clean `main` at delivered `89824529`, matching fetched `origin/main`. The previous
turn was progress. Sound Card value caches now belong to their native-prepared configuration.
The worker allocates and sorts a fixed slot array before publication. Actor service performs
binary lookup and updates admitted slots; it cannot grow a map or retain unrelated node IDs.
Binding replacement and facade retirement no longer clear or finally destroy a variable cache
on the actor. The existing configuration lifetime pool owns its storage through final disposal.

The cache admits **131,072 slots / 16 MiB**, counted within the existing **64 MiB combined
configuration/warning-plan budget** and its conversion scratch allowance. It uses no additional
lifetime item or cleanup thread. Cache cells store booleans, integers, floats and up to **64 UTF-8
bytes** of text inline. A source string's large spare capacity cannot escape into the cache;
unsupported value shapes are not cached. Frame admission and cache storage share the same text
limit. Slot storage remains fixed under updates and unknown-node turnover.

Each cached cell carries its binding generation. Stale generations do not suppress current edits,
and changing generations needs no whole-array clear. Existing applied-only receipt promotion,
epsilon comparison, pinned frame progress, 64-position/receipt service budget and isolated failure
retry remain intact. The actor is the only writer, holding the configuration's mutex for one
bounded service turn. Preparing workers do not touch the cache after publication, and final
configuration destruction cannot race a borrowed actor cache guard. Empty, unapplied bindings
no longer traverse observation values without a consumer.

Five new tests cover fixed storage under bound/unknown node churn, generation changes and epsilon,
inline UTF-8 text including a 1 MiB source capacity, oversize slot admission, and quota retention
through the last cloned owner. The populated-cache destruction test checks that the disposer thread,
not the releasing caller, runs final cache destruction before capacity becomes reusable. Existing
value receipt, failed suffix, stale generation and continuous producer tests now exercise this cache.

| Check                                                                                                                 | Result                                                                                                                                 |
| --------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------- |
| `cargo test-fast --locked -p Chataigne2 --bin Chataigne2 module_modules_audio_sound_card -- --test-threads=4 --quiet` | Final **95/95** passed in 0.56s after 19.71s incremental preparation; NullBackend only. Initial full preparation also passed in 2m21s. |
| Fresh compiled-name disjoint preservation filters from batch 46                                                       | Final **446/446** passed; **541 distinct of 986 listed** with Sound Card.                                                              |
| `cargo clippy --locked -p Chataigne2 --all-targets -- -D warnings`                                                    | Final passed in 14.52s.                                                                                                                |
| `cargo check --locked --workspace --tests`                                                                            | Final passed in 21.37s.                                                                                                                |
| `cargo build --locked -p Chataigne2 --bin Chataigne2`                                                                 | Final passed in 1m58s; existing UI bundle reused.                                                                                      |

Inventory: **1575 source files / 41 oversized**, using the maintained scan policy; two focused
new sources, no new size exception. Final Root/Core Cargo formatting, affected app rustfmt and
maintained-doc formatting/whitespace checks passed. No public protocol DTO,
generated contract, persisted project format or app-shell startup changed. Audio and broader
Core/frontend suites remain historical as identified in the handoff above.

Sound Card remains **50% partial**, P03 **79%**, whole plan **15%**. The value-cache ownership
checkpoint is now verified. Probe/selection and raw inspector ownership, derived structure/default
routes, enum choices, downstream logger ownership and remaining reconfiguration/apply-time/failure
fences stay open, followed by the remaining app-module/channel audit. These bounds do not qualify
whole-process RSS, actor/realtime deadlines, physical devices, native UI, other platforms or
product-scale workloads. Commit and push this verified milestone to `origin/main` before the next batch.

### P03 batch 48: Native selection/probe planning and device-state fences

Resumed clean `main` at delivered `818368bf`, matching fetched `origin/main`. The previous turn
was progress. Sound Card configuration selection now uses a typed preparation stage on the existing
shared worker before full configuration conversion. Selector decoding, probe targets, ASIO release
policy, pending capabilities and physical channel copying stay off the actor. One selector reader
serves both stages. The actor's repeated serialized-selection copies/availability parsing were removed,
along with the obsolete availability helper. Endpoint recovery uses native events and coarse retries.

Both stages share the existing **one worker / 16 queued requests / 32 admission tickets**,
**256 MiB scratch**, canonical Core snapshot leases and **128-item / 1 GiB** output pool. Source
leases cover queued/running requests; ready plans keep their output admission and revision fences,
without unnecessarily retaining a completed source snapshot. Selection plans admit **16 MiB combined**
for their body and canonical targets, separate from the existing **64 MiB configuration/warning-plan**
limit. Canonical targets remain charged through raw native command Arcs. Unchanged targets reuse
that payload; actor admission compares pointers. The steady-state cache keeps only targets, allowing
completed inventory plan storage to retire. All final storage disposal stays off the actor.

Selection keys check published revisions of the four selector parameters, module UUID and effective
enabled state. Unrelated value edits do not restart selection. Native device metadata is bounded
independently of metering before copying. Both successful and failed results fence their captured
device revision. Golden Audio publishes that revision with device state and exposes an atomic reader;
actor checks cannot wait for a large snapshot publication. Metering does not advance it. The native
snapshot revision is omitted from serialization; generated UI contracts remain unchanged. Stale
errors cannot disable current audio. Native execution after admission remains an open fence boundary.

`SetDeviceProbeInterests` now accepts shared immutable targets; all callers changed together, and
the native control worker obtains/clones ownership. Serialized command shape is unchanged. Full
probe/configuration queues defer for **250 ms** while retaining ready work. Catalog-only targets
and unavailable selected endpoints recover without actor JSON polling. Preparation/metadata capacity
pressure remains retryable and preserves current audio. Exclusive-driver release before probing
retains its previous product policy and is tested with metadata only, without opening ASIO hardware.

Eight app tests cover no-endpoint selection, catalog/capability transition and canonical reuse,
exclusive-driver planning, quota retention through command Arcs, target capacity, actual selector
changes before callbacks, stale failure rejection and deferred ready-work ownership. Three audio
tests cover atomic reads under a held writer lock, serialization omission and same-Arc full-queue
retry. The existing input/output transition test also checks published device revisions and metering
independence. Existing NullBackend startup/idle recovery, routing, callbacks and playback tests pass.

| Check                                                                                                                 | Result                                                                                                                         |
| --------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------ |
| `cargo test-fast --locked -p Chataigne2 --bin Chataigne2 module_modules_audio_sound_card -- --test-threads=4 --quiet` | Final **103/103** passed in 0.53s after 24.66s incremental preparation. Initial preparation passed in 3m01s; NullBackend only. |
| Fresh compiled-name disjoint preservation filters from batch 47                                                       | Final **446/446** passed; **549 distinct of 994 listed** with Sound Card.                                                      |
| `cargo test-fast --locked -p golden_audio --no-default-features --features analysis,playback -- --quiet`              | **185/185** passed (164 unit + 21 integration).                                                                                |
| `cargo test-fast --locked -p golden_audio --no-default-features -- --quiet`                                           | **131/131** passed; overlapping alternate-feature evidence.                                                                    |
| `cargo clippy --locked -p Chataigne2 -p golden_audio --all-targets -- -D warnings`                                    | Final passed in 28.39s.                                                                                                        |
| `cargo check --locked --workspace --tests`                                                                            | Final passed in 25.86s.                                                                                                        |
| `npm run check:generated --workspace golden_audio_ui`                                                                 | Passed; no generated contract drift.                                                                                           |
| `cargo build --locked -p Chataigne2 --bin Chataigne2`                                                                 | Final passed in 2m11s; existing UI bundle reused.                                                                              |

Inventory: **1578 source files / 41 oversized**, using the maintained scan policy; three focused
new sources, no new size exception. Final Root/Core Cargo formatting, affected app rustfmt and
maintained-doc formatting/whitespace checks passed. The initial strict check
identified the device-revision field needing its final pre-admission guard; corrected before final
checks. No persisted project format, app-shell startup or UI wire shape changed. Prior core and
frontend/graph suites remain historical batch 43 evidence.

Sound Card remains **50% partial**, P03 **79%**, whole plan **15%**. Remaining Sound Card work is
derived structure/default-route creation, enum-choice and route-intent inspector ownership,
event-feedback selection parsing/control-value copies, logger ownership and remaining
reconfiguration/apply-time/failure fences. The remaining app-module/channel audit is also open.
These bounds do not qualify whole-process RSS, actor/realtime deadlines, physical devices, native
UI, other platforms or product-scale workloads. Commit and push this verified milestone to
`origin/main` before the next batch.

### P03 batch 49: Native route lookup and captured-inventory fences

Resumed clean `main` at delivered `0b52eec5`, matching fetched `origin/main`; the previous turn
was progress. Route-intent validation no longer clones the live device inspector or scans device
and physical-channel vectors on the actor. The app-owned native observation collector resolves
active/selected endpoints and directional system defaults, then prepares sorted physical keys
in an immutable route frame. Actor validation borrows keys and performs
a binary lookup. Atomic native device-revision checks before and after lookup reject stale
inventories with a retry message. Graph creation and validation of module-owned application channels
remain backend responsibilities. Later graph-edit/native-configuration races remain open.

Metering-only captures reuse the same frame while the device revision is unchanged. Sorting runs
outside the original reader lock. Routing uses its own **64-item / 256 MiB** lifetime pool, reserving
**8 MiB** before copying. Source device metadata admits **8 MiB / 131,072 entries** independently
of analysis/metering; final frames charge actual vector/key capacities and metadata within **8 MiB**.
The original reservation stays charged through exported views, replacement and final background disposal.
Capacity failures retry on native observation wakes, preserving the last good frame. Oversized metadata
waits for a changed device revision. Both successful and failed route outcomes fence their source
revision. No synchronous inspector fallback was added. An unchanged last-good route frame remains
usable under pressure, and metering capture exhaustion/oversized analysis cannot prevent route updates.

Eight new app tests cover direction-specific borrowed lookup, stale revisions, active versus pending
selection, backend/direction-specific system defaults, a reversed **32,768-channel** inventory,
oversized analysis, oversized metadata and actual NullBackend full-pool recovery, reuse, removal and
disposal. The native regression keeps the metering pool full through route updates and independently
fills/releases route capacity. Existing capture-capacity/recovery, value, telemetry, callback, routing,
startup and idle-recovery regressions remain preservation requirements.
Two fixture errors were corrected before final verification: enabled output initially lacked a physical
inventory, then requested a mono stream from the stereo NullBackend. The final fixture validates its
configuration and uses the actual two-channel shape. Tests use NullBackend or pure metadata only.

| Check                                                                                                                 | Result                                                                       |
| --------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------- |
| `cargo test-fast --locked -p Chataigne2 --bin Chataigne2 module_modules_audio_sound_card -- --test-threads=4 --quiet` | Final **111/111**, **0.54s**, after **2m22s** preparation.                   |
| Fresh compiled-name disjoint preservation filters                                                                     | **446/446**; **557 distinct app tests** with Sound Card, of **1002 listed**. |
| `cargo clippy --locked -p Chataigne2 --all-targets -- -D warnings`                                                    | Final passed in **30.20s**.                                                  |
| `cargo check --locked --workspace --tests`                                                                            | Final passed in **22.63s**.                                                  |
| `cargo build --locked -p Chataigne2 --bin Chataigne2`                                                                 | Final passed in **1m50s**; existing UI bundle reused.                        |

Inventory: **1580 source files / 41 oversized** using the maintained scan policy; two new focused
sources, no new size exception. No persisted format, app-shell startup, protocol declaration,
generated output or UI wire shape changed. Audio/core/frontend suites from previous milestones
remain historical evidence unless explicitly rerun here. Final Root/Core Cargo formatting, affected
app rustfmt, maintained-doc Prettier and whitespace checks passed. All owned jobs are terminal.

Sound Card remains **50% partial**, P03 **79%**, whole plan **15%**. Remaining Sound Card work:
derived structure/default routes, enum-choice inspector/publication ownership, event-feedback selection
parsing/control-value copies, logger ownership and reconfiguration/apply-time/failure fences. Then
finish the remaining app-module/channel audit. No whole-process RSS, actor/realtime deadline,
physical-device, native-UI, cross-platform or product-scale qualification is claimed. Commit and
push this verified milestone to `origin/main` before starting the next batch.

### P03 batch 50: Native prepared-event logging and passive completion

Resumed clean `main` at delivered `5fd8310e`, matching fetched `origin/main`; the previous turn
was progress. Prepared playback, connection and event-preparation log messages now append on a
shared app-owned worker. Actor feedback forwards the original retained event handle, selection
policy and module origin; it no longer copies these messages or enters the synchronous logger.
Admission freezes policy/origin, while policy changes before admission can suppress an optional
message without waiting for retry. Native publication preserves severity and FIFO event order.

The service admits **32 live completion tickets / 32 queued metadata handles**, with one worker
lifetime slot including retirement. Jobs retain the original canonical prepared-event lease and
its existing byte quota through publication, cancellation and actor/facade removal. No independent
payload copy or quota registration is introduced. The native writer bounds display text to
**16 KiB UTF-8 with a truncation marker**; callback identities/paths remain intact. Logger timestamps
are assigned when the native append commits. Core logger/UI storage and best-effort stdout retain
their own contracts; this does not qualify whole-process logger memory or log/actor deadlines.

Only the FIFO front advances. Actor service waits for a native receipt before warning/callback
publication, and native waits do not self-wake. Full admission retries after **250 ms**; an earlier
coalesced wake re-arms the remaining deadline without attempting admission early. Writer panic is
contained. Accepted queued jobs dropped during cancellation settle as failed, and a blocked retiring
writer cannot cause replacement-worker growth. Failure of an uncertain committed prefix is never
replayed: a separate retryable warning edit records the failure before the original callback continues.
Ordinary edit-receipt continuation and disabled/facade-retired callback service remain intact.

Twelve new app tests cover native slow-writer ownership/wakes, full admission and coarse retry,
earlier coalesced deadlines, policy changes before/after admission, severity/origin/order,
post-write panic without replay, cancellation/retirement recovery, event/callback quota retention
after actor release, UTF-8 display bounds, passive FIFO callback gating and failed-warning retries.
Existing warning/callback receipt tests now await native logging only in their test helper; runtime
service has no blocking waits. Tests use pure events, controlled sinks or the existing NullBackend
fixtures; no physical endpoint or desktop automation is used.

| Check                                                                                                                 | Result                                                                                                              |
| --------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------- |
| `cargo test-fast --locked -p Chataigne2 --bin Chataigne2 module_modules_audio_sound_card -- --test-threads=4 --quiet` | Final **123/123**, **0.63s**, after **1m16s** preparation. Includes a cutoff inside a multibyte character.          |
| Fresh compiled-name disjoint preservation filters                                                                     | **446/446**; **569 distinct app tests** with Sound Card, of **1014 listed**. No duplicate credit for repeated runs. |
| `cargo clippy --locked -p Chataigne2 --all-targets -- -D warnings`                                                    | Final passed in **13.85s**.                                                                                         |
| `cargo check --locked --workspace --tests`                                                                            | Final passed in **21.93s**.                                                                                         |
| `cargo build --locked -p Chataigne2 --bin Chataigne2`                                                                 | Final passed in **1m35s**; existing UI bundle reused.                                                               |

Inventory: **1582 source files / 41 oversized**, using the maintained scan policy; two focused
new sources, no new size exception. No persisted format, app-shell startup, protocol declaration,
generated output or UI wire shape changed. Prior audio/core/frontend suites remain historical
evidence unless explicitly rerun for this milestone. Final Root/Core Cargo formatting, affected app
rustfmt, maintained-doc Prettier and whitespace checks passed. All owned jobs are terminal.

Sound Card remains **50% partial**, P03 **79%**, whole plan **15%**. Remaining Sound Card work is
derived structure/default routes, enum-choice inspector/publication ownership, feedback selection
parsing/control-value copies, command/error diagnostic preparation and remaining reconfiguration,
apply-time and failure fences. Then finish the remaining app-module/channel audit. No physical-device,
native-UI, cross-platform or product-scale qualification is claimed. Commit and push this verified
milestone to `origin/main` before starting the next batch.

### P03 batch 51: Shared enum domains and native inspector ownership

Resumed clean `main` at delivered `82ada44c`, matching fetched `origin/main`; the previous turn
was progress. The foundation now owns immutable `ParameterEnumOptions`, backed by one shared
vector. Live constraints, process snapshots and retained typed history records clone a handle
instead of every option and nested string/value. Producers replace complete lists; app, script,
macro and engine consumers use the same public contract. The sized vector allocation can also
adopt the exact canonical Arc protected by a native retained payload. Empty defaults share storage;
explicit spare capacity is preserved and counted. JSON and generated TypeScript remain arrays.
Wire materialization and authored history callback serialization still copy; this milestone does
not claim bounded history replay or whole-process enum memory.

Sound Card's native inventory now captures the complete bounded inspector once per device
revision and indexes physical channels without duplicating keys. Route lookup and device-choice
callbacks retain the same frame; actor callbacks no longer clone inspector metadata. Inventory
admission remains independent of metering/analysis, with **8 MiB / 131,072 source entries**,
**16 MiB prepared frames**, and a **64-item / 512 MiB** lifetime pool. Final charges include actual
metadata/index capacities and Arc storage. Replacement, exported catalogs and final disposal
retain the original quota. A dirty flag retries missing current catalogs on native observation
wakes, independently of metering pressure. Choice construction, comparison, serialization and
publication still run on the actor and remain open.

New tests cover 32,768-option domains shared by 128 constraint copies, immutable copy-on-write
forks, sparse/wire round trips, shared empty defaults, spare capacity, snapshot/history retention,
correct undo/redo values, canonical native quota across exported constraints, and catalog export
identity/lifetime. Existing route-pressure tests now exercise the full inspector plus indices.
An initially invalid enum fixture was corrected to preserve the current value in both domains.
The replay test also exposed the existing serialization boundary; assertions and claims now
distinguish retained typed records from reconstructed live state. Runtime history behavior was
not changed to satisfy a pointer-identity assertion.

| Check                                                                                                                 | Result                                                                                                                                                                                                                                     |
| --------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `cargo test-fast --locked -p golden_parameters -p golden_engine -- --quiet`                                           | **579 passed / 3 existing ignored**: 553 engine unit, 8 integration, 7 doc, 11 parameter tests.                                                                                                                                            |
| `cargo test-fast --locked -p chataigne_alchemist -- --quiet`                                                          | **179/179** passed.                                                                                                                                                                                                                        |
| `cargo test-fast --locked -p Chataigne2 --bin Chataigne2 module_modules_audio_sound_card -- --test-threads=4 --quiet` | **124/124** passed, **0.64s** execution; NullBackend/pure metadata only.                                                                                                                                                                   |
| Fresh compiled-name disjoint app filters                                                                              | Original **446/446** preservation cases passed; 25 added paging, simulated Stream Deck and pure controller/selector cases passed. Formula **47 passed / 3 failed**. Total **642 passed / 3 failed**, 645 distinct selected of 1015 listed. |
| `cargo clippy --locked --workspace --all-targets -- -D warnings`                                                      | Final passed; redundant conversion removed and serial test fixture adapted to immutable replacement.                                                                                                                                       |
| `cargo check --locked --workspace --tests`                                                                            | Final passed after fixture correction.                                                                                                                                                                                                     |
| `cargo build --locked -p Chataigne2 --bin Chataigne2`                                                                 | Passed; existing UI bundle reused.                                                                                                                                                                                                         |
| `npm run codegen:golden-ui-protocol --workspace chataigne-ui` and generated-directory `git diff --exit-code`          | Passed; generated Rust protocol bindings unchanged.                                                                                                                                                                                        |
| `npm run test --workspace golden_ui`; `npm run check --workspace chataigne-ui`                                        | **22/22** passed; zero Svelte errors/warnings.                                                                                                                                                                                             |

The expanded Formula suite exposed three creation failures: the catalog-wide ANode round trip,
hosted curve-key resource editing, and hosted gradient-stop resource editing all reject a scalar
parameter as expecting a float-compatible value. Each failure also reproduced from the retained
`target/debug/deps/Chataigne2-c47f0ddae7812c83.exe`, last written **2026-10-19 23:28:34 local**,
SHA-256 `7fb5239ee831f5383699d2768b169a5894909e2060d835e512ebeea4ea896a68`. That is a historical
artifact comparison, not an exact-HEAD baseline rebuild; its source SHA is not established here.
These failures remain open under P08/P12, with no full Formula-suite pass claimed. The initial
direct-binary Formula run also used the repository root instead of Cargo's crate directory;
its resource-file failure passed when rerun from `apps/chataigne`. The three creation failures
persist from that correct directory. All recorded tests use the current compiled artifact except
the explicitly identified historical comparison. Root/Core formatting, affected app rustfmt,
maintained-doc Prettier and whitespace checks passed; all owned jobs are terminal.

Inventory: **1584 source files / 41 oversized**, using the maintained scan policy. Two focused
foundation sources added; route sources renamed to inventory. Existing oversized API consumers
received mechanical changes and consistent formatting, with no new size exception. No persisted
format, app-shell startup or UI wire shape changed. Sound Card remains **50% partial**, P03 **79%**,
whole plan **15%**. Next: native choice construction/publication, derived structure/default routes,
feedback selector/value preparation, command diagnostics and remaining reconfiguration/apply-time
failure fences, then the remaining module/channel audit. No physical-device, native-UI,
cross-platform, actor deadline or full-RSS qualification is claimed. Commit and push the verified
milestone to `origin/main` before the next batch.

### P03 batch 52: Native device/numeric choice planning and receipt settlement

Resumed clean `main` at delivered `21d96134`, matching fetched `origin/main`; the previous turn
was progress. Device/numeric option construction, identity serialization, capability parsing,
missing-selection preservation and domain comparison now run on the existing bounded preparation
worker. It retains the canonical tree snapshot and shared inventory, with a typed source handle
instead of another inspector copy. A target index replaces repeated catalog-wide descriptor scans.
Actor-side choice builders and reset adapters were removed. See [host ownership](../architecture/hosts.md)
for queue, scratch, output and lifetime allowances.

Choice input reads up to six **1 MiB** selector values and respects absent inactive DSL branches.
Candidates admit **32,768 entries / 32 MiB per domain**, with a **1 MiB escaped-JSON identity writer**.
At most **four patches / 64 MiB** are prepared. Core exposes its existing exhaustive constraint
allocation visitor as a public native admission API. The plan uses the configuration-result pool;
canonical enum vectors have a separate **512-allocation / 512 MiB** pool so exported DTO/history
owners leave configuration tickets available. Exported raw enum Arcs preserve their original quota.

The actor moves the patch vector once and retains its plan through a Core batch receipt. Applied
revision state advances only after success. Failed prefixes are prepared afresh from applied state;
source invalidation settles pending receipts before replacement. Completed/queued results from
superseded inventory are cancelled even without a new snapshot, releasing stale prepared storage.
Pending native work, pending Core receipts and missing inventory avoid choice-driven snapshot
requests. Admission/publication pressure retries after **250 ms**, re-arming deadlines after earlier
wakes. Permanent source errors await changes and use a separate failure stamp, never an applied
acknowledgement. A receipt-backed warning remains serviceable after disable/facade retirement.

Sixteen new tests cover native quota/export ownership, one-shot transfer, unchanged-domain omission,
empty authored numeric values, duplicate catalog targets, option count and escaped-byte admission,
active-selector/device revision fences before callbacks, disjoint edits, cancellation/worker failure,
partial publication, fresh-prefix recovery, passive snapshot decisions, coarse retry and warning
receipts. The initial source check incorrectly required inactive selectors; it now follows the DSL's
actual shape. A fence fixture initially wrote the existing `null` driver; it now asserts an actual
change. Empty numeric identities remain missing choices rather than becoming rejected publications.
Tests use NullBackend, pure metadata, controlled queues and ordinary engine ticks only.

| Check                                                                                                                 | Result                                                                               |
| --------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------ |
| `cargo test-fast --locked -p Chataigne2 --bin Chataigne2 module_modules_audio_sound_card -- --test-threads=4 --quiet` | Final **140/140** passed, **0.73s** execution.                                       |
| Fresh compiled-name disjoint preservation filters                                                                     | Final **446/446** passed; **586 distinct app tests of 1031 listed** with Sound Card. |
| `cargo test-fast --locked -p golden_parameters -p golden_engine -- --quiet`                                           | **579 passed / 3 existing ignored**, including integration and doc tests.            |
| `cargo clippy --locked --workspace --all-targets -- -D warnings`                                                      | Final passed.                                                                        |
| `cargo check --locked --workspace --tests`                                                                            | Final passed.                                                                        |
| `cargo build --locked -p Chataigne2 --bin Chataigne2`                                                                 | Final passed; unchanged UI bundle reused.                                            |

Inventory: **1590 source files / 41 oversized**, using the maintained scan policy; no new size
exception. No persisted format, app-shell startup, protocol DTO or UI wire shape changed.
Alchemist, broader Formula/controller, frontend, audio and graph UI suites retain their explicitly
dated evidence; the three Formula creation failures from batch 51 remain open, not new passes.
Root/Core Cargo formatting, affected app rustfmt, maintained-doc Prettier and whitespace checks
passed. All owned jobs are terminal. Commit and push this verified milestone before continuing.

Sound Card remains **50% partial**, P03 **79%**, whole plan **15%**. Next: bound and guard Core
constraint snapshot/normalization/equality/publication, then derived structure/default routes,
feedback selector/value preparation, command diagnostics and remaining reconfiguration/failure
fences, followed by the remaining module/channel audit. Native preparation does not qualify the
existing Core publication costs or post-admission races. No physical-device, native-UI,
cross-platform, actor-deadline or full-RSS qualification is claimed. Commit and push this verified
milestone to `origin/main` before the next batch.

### P03 batch 53: Application-time read fences for prepared choices

Resumed clean `main` at delivered `669163a9`, matching fetched `origin/main`; the previous goal turn
was progress. Golden Engine now provides bounded `Edit::CheckPreconditions` reads before prepared
writes enter their ordinary validation/history path. Admission permits **32 node reads / eight native
guards**, including spare vector capacity. Authored reads compare node UUID/effective enabled state
and a live local revision against the captured subtree upper bound in the same engine epoch. Every
later local write receives a greater sequence, so checking does not rebuild or fold the live tree.
Identity-only reads check epoch, UUID and enabled state while ignoring unread presentation metadata.
Synthetic views/overlays cannot authorize application through another revision namespace.

`golden_io` supplies a monotonic native revision owner, read-only readers and captured guards.
Owner retirement closes outstanding guards; exhaustion cannot wrap and revive an old revision.
Golden Audio uses that generic clock for its existing native device revision, without exposing a
mutable atomic or acquiring the snapshot lock for checks. Guards retain no device payload or worker;
inventory and plan admission conservatively charge their shared clock allocation. Device revision
metadata remains omitted from saved/wire observations.

The choice worker captures at most eight reads: module identity/enabled state, connection authored
state and present selectors. Connection reads cover inactive DSL branch membership changes. It
prepares the native inventory fence and both publication vectors, then moves them once to the actor.
Core checks before the first constraint patch. A rejected fence fails the batch receipt and discards
the drain suffix under ordinary prefix semantics; preceding accepted edits remain committed. A
successful fence has no authored effect, undo step or redo invalidation. Normal constraint validation,
events, history, later patch failures and fresh-prefix recovery remain intact. This is an admission
boundary, not an atomic transaction or a lock on future native changes.

Fourteen added tests cover stale reads before callbacks, accepted prefixes and failed receipts,
constraint events and undo/redo, unrelated writes, descendant-vs-local scope, child membership,
overlays/engine epochs, UUIDs, inherited disable, native advancement/retirement, bounded guard storage,
cross-thread clocks, exhaustion, each active late selector and disjoint gain writes. Existing native
observation and partial-publication tests were strengthened. The first app run exposed a real scope
issue: a broad module read rejected its own warning receipt on retry. Module identity-only reads now
ignore that unrelated presentation write; connection and selector reads retain authored checks.
The corrected Sound Card suite and all selected preservation cases pass. Tests use controlled queues,
pure metadata, NullBackend and loopback only. A default audio invocation was stopped during compilation
before its native host-discovery test; the completed audio suite disables desktop features.

| Check                                                                                                                 | Result                                                                                       |
| --------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------- |
| `cargo test-fast --locked -p Chataigne2 --bin Chataigne2 module_modules_audio_sound_card -- --test-threads=4 --quiet` | Final **143/143** passed, **0.73s** execution.                                               |
| Fresh compiled-name disjoint preservation filters                                                                     | **446/446** passed; **589 distinct app tests of 1034 listed** with Sound Card.               |
| `cargo test-fast --locked -p golden_engine -- --quiet`                                                                | Final **576 passed / three existing ignored**: 561 unit, eight integration, seven doc tests. |
| `cargo test-fast --locked -p golden_io -- --quiet`                                                                    | Final **47/47** passed, including all three new clock cases.                                 |
| `cargo test-fast --locked -p golden_audio --no-default-features --features analysis,playback -- --quiet`              | **185/185** passed: 164 unit, 21 integration; desktop disabled.                              |
| `cargo check --locked --workspace --tests`                                                                            | Final passed.                                                                                |
| `cargo clippy --locked --workspace --all-targets -- -D warnings`                                                      | Final passed.                                                                                |
| `cargo build --locked -p Chataigne2 --bin Chataigne2`                                                                 | Final passed; unchanged UI bundle reused.                                                    |

Inventory: **1595 source files / 41 oversized**, using the maintained `rg --files` scan policy.
Five focused sources added; no new size exception. No persisted format, app-shell startup,
protocol DTO or UI wire shape changed. Root/Core Cargo formatting, affected app rustfmt,
maintained-doc Prettier and whitespace checks passed. All owned jobs are terminal. Parameter,
Alchemist, broader Formula/controller, frontend and graph suites retain their dated evidence;
the three Formula creation failures from batch 51 remain open, not new passes.

Sound Card remains **50% partial**, P03 **79%**, whole plan **15%**. Next: Core snapshot,
normalization/equality, owned-payload disposal and history/projection costs; derived structure/default
routes, feedback selector/value preparation, command diagnostics and other native execution/
reconfiguration fences; then the remaining app-module/channel audit. No physical-device, native-UI,
cross-platform, actor-deadline or full-RSS qualification is claimed. Commit and push this verified
milestone to `origin/main` before the next batch.

### P03 batch 54: Frozen indexed enum domains and cached snapshot weights

Resumed clean `main` at delivered `389e3fa3`, matching fetched `origin/main`; the previous goal turn
was progress. The parameter foundation now owns `ParameterEnumDomain`: immutable option rows,
sorted slot indexes and a cached conservative allocation charge. `ParameterEnumOptions` shares that
whole object. Adopting a prepared shared domain and cloning snapshots never sort or copy entries.
The Rust shared-storage API accepts the opaque domain rather than an externally mutable vector;
no compatibility adapter preserves a way to mutate entries behind stale indexes.

The producer builds one variant index, plus an exact-value index for custom values. Canonical
`Enum(variant_id)` domains share the variant index. Authored order, duplicate variants, value-kind
identity, reference UUID/projection equality, signed zeros and NaN non-membership remain intact.
Core normalization uses **O(log n) comparisons**, with a final exact equality check for generic
values. Invalid membership errors report the option count in a bounded message instead of cloning
all allowed IDs. Source strings are borrowed unless normalization changes the accepted value.

Generic reference/value/option allocation visitors now live with the parameter types. They count
nested cached hints and spare capacity independently of equality. Fresh Core snapshot weights read
the domain's cached charge without visiting its rows or strings. Native choice admission includes
the domain header, row capacities and boxed indexes within **32 MiB per domain / 64 MiB per plan**.
The existing option pool owns the exact domain Arc, so raw exported handles retain both options and
index admission through final native disposal. The separate configuration/option pools and all
source/application fences remain intact.

Eight new foundation tests cover mixed-kind linear-membership equivalence, duplicate IDs, signed
zero, nonfinite values, reference hints/projections, aliases, preserved order, 32,768-entry domains,
bounded rejection diagnostics with huge IDs, index sharing, nested/spare charges and semantic equality
with different retained weights. A new Core case repeatedly constructs fresh snapshots around one
large shared domain. The native quota test now keeps a raw exported domain after other owners drop.
An initial weight-difference assertion overlooked String capacity shrinking during clone; both
fixtures now start from equivalent clones before adding the cached hint. Runtime accounting was
not changed to satisfy the fixture.

A standalone optimized-dev fixture constructed/froze **32,768 options in 12.1334 ms** and performed
**3,072 indexed membership lookups in 739.5 us**, during concurrent compilation. The construction
measurement includes fixture row/string allocation. A separate Core fixture constructed and weighed
**128 fresh one-node snapshots sharing a 32,768-entry domain in 129.2 us**. These are local component
observations, not 20k/50k-node product measurements, an actor deadline or full-RSS qualification.

| Check                                                                                                                 | Result                                                                                 |
| --------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------- |
| `cargo test-fast --locked -p golden_parameters -- --quiet`                                                            | Final **19/19** passed, including reversed authored-order wire serialization.          |
| `cargo test-fast --locked -p golden_engine -- --quiet`                                                                | **577 passed / three existing ignored**: 562 unit, eight integration, seven doc tests. |
| `cargo test-fast --locked -p Chataigne2 --bin Chataigne2 module_modules_audio_sound_card -- --test-threads=4 --quiet` | **143/143** passed, **0.73s** execution.                                               |
| Fresh compiled-name disjoint preservation filters                                                                     | **446/446** passed; **589 distinct app tests of 1034 listed** with Sound Card.         |
| `cargo check --locked --workspace --tests`                                                                            | Final passed.                                                                          |
| `cargo clippy --locked --workspace --all-targets -- -D warnings`                                                      | Final passed.                                                                          |
| `cargo build --locked -p Chataigne2 --bin Chataigne2`                                                                 | Final passed; existing UI bundle reused.                                               |
| `npm run codegen:golden-ui-protocol --workspace chataigne-ui` and generated-directory diff/status                     | Passed; generated bindings unchanged.                                                  |
| `npm run test --workspace golden_ui`; `npm run check --workspace chataigne-ui`                                        | **22/22** passed; zero Svelte errors/warnings.                                         |

Inventory: **1600 source files / 41 oversized**, using the maintained scan policy. Five focused
sources added; no new size exception. Root/Core Cargo formatting, affected app rustfmt, four maintained
docs' Prettier and whitespace checks passed. All owned jobs are terminal. No persisted format,
app-shell startup, protocol DTO or UI wire shape changed. IO/audio retain batch 53 evidence; broader
Formula/controller, Alchemist and graph results retain dated evidence. The three Formula creation
failures from batch 51 remain open, not new passes. No physical devices or desktop automation used.

Domain equality, non-enum metadata cloning, kind coercion and value copies, history serialization/
reconstruction (including index rebuilding), DTO projection/encoding and owned-payload disposal
remain open, followed by derived structure/default routes, feedback/command/native failure fences
and the remaining app-module/channel audit. Sound Card stays **50% partial**, P03 **79%**, whole
plan **15%** under the existing scope rubric. Commit and push this verified milestone to
`origin/main` before the next batch.

### P03 batch 55: Native authored snapshots preserve enum ownership

Resumed clean `main` at delivered `7ee3f7ff`, matching fetched `origin/main`; the previous goal turn
was progress. Core owns the new opaque `NodeAuthoredData` in-memory capture contract. Built-in
Parameter snapshots retain every authored field directly, including the exact immutable enum
domain and its original quota. Callback restoration and opaque undo/redo no longer materialize
option-array JSON or reconstruct indexes. A consuming restore moves unique native fields; shared
history records clone ordinary fields while keeping the enum domain shared. Metadata and binding
restoration stay Engine-owned, and mutable runtime services remain with the live node.

Generated inherited nodes compose their base capture with complete declared own fields, including
defaults. Custom project encoders retain their JSON schema and paired project decoder. Generated
complete-field restoration bypasses decoder-only project-load resets; an explicit authored restore
still takes precedence. Script restoration keeps its existing runtime reload marker. Public Node,
ViaTarget and enum dispatch signatures change together; no old JSON signature adapter remains.
Actual project serialization uses the same fields and wire format as before.

Native parameter equality distinguishes persisted reference display/recovery hints from cached
runtime node IDs. It checks domain identity first, then authored entries for distinct domains.
Exact nonfinite bits survive in-memory checkpoints without weakening incoming-value validation.
Exhaustive field destructuring makes future Parameter/state additions require capture/restore review.

Ten new tests exercise native quota lifetime with a 32,768-entry domain and 128 restores, every
authored field, wrong-payload rejection, float-bit preservation, reference hint equality, inherited
callbacks and metadata/bindings, undo/redo, encoder-only codecs and decoder-only project-load resets.
The existing constraint-history test now requires the original shared domain after undo and redo.
The first run passed 571 Core unit tests and exposed one fixture error: an empty reference clears
its hints during normal cache refresh. The fixture now uses a missing persistent target, whose
recovery hint must survive. Runtime reference normalization is unchanged.

Windows toolchain: Rust/Cargo 1.97.0, Node 26.5.0, npm 11.17.0. Final checks so far:

| Check                                                                                                                 | Result                                                                                                                                                              |
| --------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `cargo test-fast --locked -p golden_engine -p golden_core_macros -p golden_codegen_support -- --quiet`                | **587 Core passed / three existing ignored** (572 unit, eight integration, seven doc); **12 code-generation support tests + one macro Clippy fixture passed**.      |
| `cargo test-fast --locked -p Chataigne2 --bin Chataigne2 module_modules_audio_sound_card -- --test-threads=4 --quiet` | **143/143** passed, **0.74s** execution.                                                                                                                            |
| Fresh compiled-name disjoint preservation filters, from `apps/chataigne`                                              | Final **446/446** passed; **589 distinct app passes of 1034 listed** with Sound Card. Initial stream selection had one transient wake-count failure; details below. |
| Fresh app binary `app::systems_alchemist_formula:: --test-threads=4 --quiet`, from `apps/chataigne`                   | **47 passed / the same three failed** as the retained pre-change binary; exit 101, not a full-suite pass.                                                           |
| `cargo check --locked --workspace --tests`                                                                            | Passed.                                                                                                                                                             |
| `cargo clippy --locked --workspace --all-targets -- -D warnings`                                                      | Passed.                                                                                                                                                             |
| `cargo build --locked -p Chataigne2 --bin Chataigne2`                                                                 | Passed; existing UI bundle reused.                                                                                                                                  |
| `npm run codegen:golden-ui-protocol --workspace chataigne-ui` and generated-directory diff/status                     | Passed; generated bindings unchanged.                                                                                                                               |
| `npm run test --workspace golden_ui`; `npm run check --workspace chataigne-ui`                                        | **22/22** passed; zero Svelte errors/warnings.                                                                                                                      |

The first 191-test stream selection passed 190 and failed TCP server
`destroy_undo_discards_runtime_delivery_and_rejects_the_previous_wake_source`: the final global
wake count was one instead of zero. Three isolated executions passed, then the complete 191-test
selection passed with the same four-thread setting. The assertion counts every live worker wake,
including a replacement server's asynchronous notifications; the old `NodeWaker` generation is
closed on removal. No source change or relaxed assertion was used to obtain the rerun. This
transient remains explicit evidence and a fixture/recovery-audit concern, not a repaired test.

Formula baseline: retained batch-54 binary SHA256
`9e5b17188a7a31a3d5f360fca6150c6f9ce42ea8050e4869a3275113a4c9253c`, modified
2026-10-20 21:50:13 local, ran before the app rebuild. The fresh patched binary SHA256 is
`7dcef3cf774d5683e4af6ac29879c51ecf73e1a961621a943e502992ffc65db2`, modified 22:51:20.
Both runs fail catalog materialization, hosted curve-key editing and hosted gradient-stop editing
at creation with the same float-compatible-value rejection. They are retained build artifacts,
not clean exact-HEAD baseline rebuilds. No additional Formula failure appeared; P08/P12 remain open.

The fresh optimized-dev Core binary's exact
`parameter::tests::authored::authored_checkpoint_keeps_large_native_domain_admission_through_raw_restore`
test, run alone with `--exact --nocapture --test-threads=1`, measured **128 native restores of a
32,768-entry shared domain in 52 us**, during concurrent app compilation. The timed loop includes
replacement, restore and pointer/membership/value assertions; fixture/domain construction is outside
it. Core binary SHA256: `2476bc86e00e8cd972cfa5c30a0b8e38a5f7161b2f749f5e1581fbbd0d0814df`.
This is one local component observation on the patched tree, not an actor deadline, RSS bound,
allocation instrumentation or 20k/50k-node product qualification.

Inventory: **1604 source files / 41 oversized**, using the maintained scan policy. Four focused
sources added, with no new size exception. The existing large dispatch/expansion registries have
only mechanical signature edits; their broader decomposition remains P12 work. No app-shell startup,
persisted schema or protocol DTO changed. No physical devices or desktop automation used.
Root/Core Cargo formatting, maintained-doc Prettier and whitespace checks passed. All owned jobs
are terminal. Foundation parameter, IO and audio suites retain their dated batch 54/53 evidence.

Remaining: non-enum metadata/value copies, kind coercion, equality of distinct domains, custom JSON
codecs, DTO projection/encoding and owned-payload disposal; then derived structure/default routes,
feedback/command/native failure fences and the remaining app-module/channel audit. Sound Card
stays **50% partial**, P03 **79%**, whole plan **15%** under the existing scope rubric.
Commit and push this verified milestone to `origin/main` before the next batch.

### P03 batch 56: Borrow or move parameter coercion inputs

Resumed clean `main` at delivered `5ff3b1cd`, matching fetched `origin/main`; the previous goal turn
was progress. Kind coercion now belongs to a focused parameter-foundation module. The borrowed
API returns `Cow` for unchanged inputs; the consuming API moves owned values. The ordinary owned
result API copies unchanged borrowed storage only for its final result, instead of cloning the
source first and cloning its strings/reference hints again during conversion. Forward and reverse
coercion share the same implementation, and compatibility checks use borrowed results.

All twelve value kinds retain their coercion meaning. Allowed text-to-text conversions move the
original string allocation; file-to-enum conversion remains rejected. CSS source units and explicit
projection target components remain intact. Parameter preparation and typed history restoration
consume accepted input. Constraint replacement normalizes the current value by reference, retaining
its storage when unchanged and assigning only an owned normalization result. Validation completes
before replacing either value or constraints; rejection preserves both.

Nine new tests cover all twelve borrowed value kinds, the text conversion direction matrix and
allocation identity, scalar/CSS/vector/reference conversions, forward/reverse projection and
compatibility, parameter preparation, repeated constraint replacement, failed updates and typed
history restoration. Two of these are isolated integration tests with a thread-local counting
allocator: a 6 MiB enum identifier incurs **zero allocations** for unchanged borrowed coercion,
rejected integer coercion and accepted owned conversion to string. Direct reference binding
compatibility and consuming reference coercion also allocate **zero** with 7 MiB of display hints
and an 8 MiB recovery segment. Fixture construction is outside counting. This does not measure
deallocation, peak RSS, complete edits or product-scale service deadlines.

Initial Core/foundation checking exposed a private projection-helper import after the module extraction;
the helpers now have sibling-module visibility without becoming public exports. Strict Clippy
then requested direct initialization of two test references; their fixtures were corrected.
Final workspace checking and strict Clippy passed. Windows toolchain: Rust/Cargo 1.97.0,
Node 26.5.0, npm 11.17.0. Final verification:

| Check                                                                                                                 | Result                                                                                                                                                                              |
| --------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `cargo test-fast --locked -p golden_parameters -p golden_engine -- --quiet`                                           | **590 Core passed / three existing ignored** (575 unit, eight integration, seven doc); **25 foundation parameter tests passed** (23 unit, two allocation-instrumented integration). |
| `cargo check --locked --workspace --tests`                                                                            | Passed.                                                                                                                                                                             |
| `cargo clippy --locked --workspace --all-targets -- -D warnings`                                                      | Passed.                                                                                                                                                                             |
| `cargo test-fast --locked -p Chataigne2 --bin Chataigne2 module_modules_audio_sound_card -- --test-threads=4 --quiet` | Final **143/143** passed, **0.73s** execution.                                                                                                                                      |
| Fresh compiled-name disjoint app preservation filters, from `apps/chataigne`, `--test-threads=4 --quiet`              | Final **446/446** passed; **589 distinct app passes of 1034 listed** with Sound Card. Final stream selection: **191/191** passed after fixture corrections.                         |
| Fresh app binary `app::systems_alchemist_formula:: --test-threads=4 --quiet`, from `apps/chataigne`                   | **47 passed / the same three failed**, **0.17s**, exit 101; no full-suite pass.                                                                                                     |
| `cargo build --locked -p Chataigne2 --bin Chataigne2`                                                                 | Final passed; existing UI bundle reused.                                                                                                                                            |
| `npm run codegen:golden-ui-protocol --workspace chataigne-ui` and generated-directory diff/status                     | Passed; bindings unchanged.                                                                                                                                                         |

The initial preservation selection passed 190 stream tests and failed the WebSocket-server
destroy/undo fixture on the same global wake-count assertion exposed for TCP server in batch 55.
Both fixtures start asynchronous replacement workers before asserting that no worker whatsoever
woke. A replacement worker may legitimately wake during that interval. TCP server, WebSocket
server/client and Serial fixtures now retain the exact old `NodeWaker`, require `is_closed()` and
`wake() == Err(Closed)` after undo, then exercise the old notifier and retain their discarded-input
and stale-callback assertions. A test-only `ModuleWake` source accessor lives under its adjacent
`tests/` directory. No runtime wake policy, retirement behavior or timing delay was changed.

Fresh final app binary SHA256:
`af5db617711ad75b1d3919ffd9d076091a1df2cd39be146e06a40f511a4d431e`, modified 23:16:42 local.
Formula still rejects catalog materialization, hosted curve-key editing and hosted gradient-stop
editing at creation with the same float-compatible-value error as batch 55. P08/P12 stay open.

Inventory: **1609 source files / 41 oversized**, using the maintained scan policy. Five focused
sources added, with no new size exception. App changes are confined to fixture support and source
fencing assertions; no app runtime behavior, project schema, protocol DTO, wire shape or startup
path changed. No physical devices or desktop automation used.
Root/Core Cargo formatting, affected app rustfmt, four maintained docs' Prettier and whitespace
checks passed. All owned jobs are terminal. UI and macro/code-generation-support suites retain
batch 55 evidence; IO/audio retain batch 53 evidence.

Remaining: non-enum snapshot metadata/value copies, parsing and formatting conversions,
distinct-domain equality, custom JSON codecs, DTO projection/encoding and owned-payload disposal;
then derived structure/default routes, feedback/command/native failure fences and the remaining
app-module/channel audit. Sound Card stays **50% partial**, P03 **79%**, whole plan **15%** under
the existing scope rubric.
Commit and push this verified milestone to `origin/main` before the next batch.

### P03 closure audit: C01-C12 ownership and acceptance gaps

The previous goal turn was progress, pushed as `53b2de57`. Traced the primary runtime constructors,
module update/drain callbacks, command handlers, stop/drop paths and existing recovery mechanisms
across all 12 fixed closure packages. Runtime source is still `e6de11cd`; this audit does not claim
new test passes. [The audit artifact](p03-closure-audit.md) links source evidence, existing safeguards
and required finish tests for each family.

Confirmed unbounded channels/drains and synchronous setup/join/native-I/O paths now drive the
next implementation work. Existing OSC receive budgets, DMX fixed-frame admission, Joy-Con
retirement reservation and Sound Card's bounded preparation/receipts remain preservation gates.
The directory-only count was corrected from 18 to **17 implemented remaining module families**:
Node Control is an empty, untracked placeholder. This removes no implemented feature or P03 gate.

The borrowed-parameter-view candidate compiled but had no demonstrated P03 failure requiring its
new API. Its six source files were restored/removed using the standard editor before delivery.
Snapshot/DTO copy/scaling investigations remain P16 work unless a concrete P03 violation is found.
No runtime change is included in this audit milestone.

Source-link/line checks, tracked module inventory, root/Core formatting, maintained-document
Prettier and whitespace checks passed. No native devices, hooks, desktop automation or host-control
commands were operated. **12 closure packages remain open, zero closed**; P03 stays **79%**,
whole plan **15%**. Source review does not substitute for repair or acceptance qualification.
Next implement C02's bounded generator admission/service, occurrence ownership and nonblocking
retirement as one closure package, preserving its completed elapsed-time checks.

### P03 C02 closed: Signals/Metronomes ownership and service

Completed on 2026-10-21 after `115eaf59`. This is one of the fixed closure packages, not another
open-ended prerequisite batch. Implementation boundaries and limits are maintained in
[timed generators](../architecture/generators.md).

Both runtimes now use `golden_io::BoundedWorker`: 16 command items / 32 MiB retained weight,
eight publication batches / 1 MiB, and one 64-row drain per actor wake. Source preflight bounds
4096 items, 1024-byte labels and aggregate curve structure/script bytes before configuration
construction. Runtime validation includes vector/string capacity and bounds the current/in-flight
configuration and event state as well as queued storage. Resetting stale IDs cannot grow state.
The global 64-worker lifetime pool is reserved before spawn and held through actual worker exit.
Stop/drop/disable/destroy cancel without a join or retirement-handoff thread; blocked cleanup
retains its slot until completion, after which admission recovers.

Workers wait for commands or sampling deadlines through the new public `receive_until` API.
It preserves queued commands before an elapsed deadline, accepted prefixes through graceful
closure and explicit cancellation priority. Predicate-based condition-variable waits avoid lost
notifications; cancellation interrupts deadline and output-capacity waits. Node service is passive:
published batches and producer exit wake the exact node lifetime, pending work re-arms it, and
configuration overload/failure retries after 250 ms. Full admission keeps the existing worker and
accepted prefix. Invalid authored input stops processing with a node warning; a valid edit recovers.
Label changes also invalidate configuration through normal metadata callbacks.

Elapsed time still settles under the previous configuration. Existing phase, targeted reset,
metronome due-prefix/catch-up and enormous-backlog cancellation fixtures pass. Backpressure
preserves output counts across bounded batches; accepted manual ticks retain individual ordered
publications. Script function/callback/template surfaces and natural count-bearing callback
arguments remain intact. Parameter triggers keep their existing coalescing contract. Removal/undo
checks the retired source's closure and starts a fresh runtime, rather than asserting that no
replacement worker can wake.

Runtime helpers now live in focused `lifecycle.rs` files; node registration and authoring stay in
the module files. All touched sources remain under 1000 lines. No host bootstrap, persistence
schema, protocol DTO, physical device or desktop automation changed.

Final checks:

| Check                                                                          | Result                                                                                          |
| ------------------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------- |
| `cargo test-fast --locked -p golden_io -- --quiet`                             | 52 passed, including five deadline/cancellation/prefix tests.                                   |
| App generator selection                                                        | 64 passed: 46 existing cases and 18 admission, service and lifecycle cases.                     |
| App generator preflight selection                                              | Three passed: item/label bounds, aggregate curve structure and script bytes before cloning.     |
| Disjoint app preservation selections                                           | 543 other passes; 610 distinct app tests with the 67 generator/preflight cases, of 1055 listed. |
| Workspace/test check; strict all-target Clippy                                 | Passed.                                                                                         |
| Native app build; UI protocol codegen                                          | Passed; generated bindings unchanged.                                                           |
| Root/Core Cargo format; touched app rustfmt; maintained-doc format; whitespace | Passed.                                                                                         |

Final app test executable SHA256:
`b9e40dd44cacb7e89dd7d6c2386b1ab8e7ef6fcc5474deac3b6855863b232086`.
All 610 selections ran against that executable after the lifecycle-file split. The existing
41 oversized sources remain cleanup debt; this package adds no size exception.

Core/foundation, UI and full Formula evidence remains dated as stated in the handoff; these
unchanged suites are not claimed as rerun. Product-scale timing stays P16. C02 is **100% locally
verified**, while **C01 and C03-C12 remain open (11 packages)**. Remaining-module closure coverage
is now 1/11; P03 is approximately **80%**, whole plan **16%**. P04-P18 retain their prior states.
Commit and push this verified package before starting C03.

### P03 C03 progress: OSC native worker ownership

Implemented after `9a4079c6` on 2026-10-21. C03 remains open: this delivers OSC's native worker
boundary, not full OSC/MIDI qualification. The [OSC ownership contract](../architecture/osc-runtime.md)
records the delivered limits and remaining actor-side responsibilities.

OSC now uses `BoundedWorker` with 16 lifetime slots reserved before thread creation, a
256-item / 4 MiB command mailbox, 128 KiB per-command retained-weight admission, and a
128-item / 2 MiB event mailbox. Checks include spare string/vector storage, expanded argument
count and reference shape. A packet framing/shape guard bounds datagram length, bundle width/
depth and array nesting before rosc performs codec work. Encoded output over 65507 bytes is
rejected on the worker before native transmission.

Binding resolution, socket/poll setup and final native cleanup now belong to the worker.
Explicit stop/drop cancels without joining. A blocked setup/cleanup keeps its lifetime slot;
new admission recovers only after actual exit. Failed native wake preserves the accepted
command result, with bounded fallback service instead of reporting false rejection or replay.
Native wake-handle ownership is cleared on the worker before releasing its lifetime slot.
Native replies backpressure input while independent output continues. An admitted bundle's
remaining messages survive a fatal poll error behind a full mailbox. Setup failure closes
admission and reports each accepted unsent command exactly once.

Module reconfiguration retains the old handle through graceful command/input-prefix retirement
before starting a replacement. Connected follows a consumed Ready event with the actual local
address. Fatal failure is distinct from per-command errors, carries a node warning, and retries
after 250 ms. Disable/removal remain explicit cancellation boundaries; destruction clears stale
pending actor state. Native cleanup cannot run in those actor callbacks.

The prior connection fixture assumed one engine tick completed native binding. It now drives
normal edit/inbox/tick service until readiness is consumed, using a deadline rather than a fixed
sleep. The one-packet fixture drains until the packet itself arrives: a scheduling hint can
outlive a Ready event consumed before its final notification. Tests retain their no-extra-packet
requirement. The oversized OSC helper/test files were split into cohesive source/test modules,
with no new size exception.

OSC actor-side remote DNS, interface enumeration, payload capture/prevalidation, recursive output
fan-out and received-value/script publication remain open, as does passive module scheduling.
MIDI remains untouched. These concrete C03 gaps are not deferred to P16 or waived by worker tests.
**11 closure packages remain open; C02 is the only closed package. P03 stays 80%, whole plan 16%.**

Final verification:

| Check                                                                             | Result                                                                                   |
| --------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------- |
| OSC compiled-name selection, `--test-threads=4 --quiet`                           | 39 passed: 25 preserved and 14 new native ownership, pressure, shape and recovery cases. |
| Disjoint app preservation selections                                              | 585 other passes; **624 distinct app passes of 1069 listed** with OSC.                   |
| `cargo check --locked --workspace --tests`                                        | Passed.                                                                                  |
| `cargo clippy --locked --workspace --all-targets -- -D warnings`                  | Passed.                                                                                  |
| `cargo build --locked -p Chataigne2 --bin Chataigne2`                             | Passed; existing UI bundle reused.                                                       |
| UI protocol codegen and generated-directory diff                                  | Passed; bindings unchanged.                                                              |
| Root/Core Cargo format, touched app rustfmt, maintained-doc format and whitespace | Passed.                                                                                  |

Final test executable SHA256:
`78b15c870b4537e543a6022431eca5ef2c2fec42ddd50931e81148431628b360`.
All selections ran from `apps/chataigne`. Windows permits a wildcard UDP bind while a specific
loopback binding holds the same port; the recovery fixture now reserves the same wildcard
address as the module. It proves an actual bind failure, then recovery after releasing that port.
The source scan finds **1639 files / 39 oversized**, using the maintained extensions/directory
exclusions; the two touched oversized OSC files were split, with no new size exception.
No new dependencies, protocol DTOs, persistence formats, hardware or desktop automation were used.
All owned jobs are terminal. Commit and push this milestone before continuing C03 actor work.

### P03 C03 progress: OSC outgoing capture and destination service

Implemented after `508d6286` on 2026-10-21. This closes the confirmed outgoing actor-work gaps;
it does not close OSC or C03. Interface discovery, received-value/callback publication, passive
scheduling and MIDI ownership remain explicit acceptance work. **11 closure packages remain
open; C02 is the only closed package. P03 stays 80%, whole plan 16%.**

Command triggers, execute events, all three script aliases and auto feedback share frozen outgoing
intents. The actor retains public snapshot/script-argument leases; command overrides, value copies,
destination traversal, encoding and DNS run on the native worker. Each request reserves 1 MiB
of the 4 MiB queue before preparation, allowing four queued requests plus one active continuation.
Snapshot, execution-event and script-argument lifetimes remain charged to existing canonical pools.
Overrides select borrowed parameter values instead of cloning arbitrary node metadata into snapshot
overlays. No new Core API or dependency is required.

Destination preparation yields every 64 traversal edges and rejects over 16384 edges, 256 targets
or 256 KiB of plan storage including reserved vector capacity. Hosts are limited to 1024 bytes;
payloads retain the 128 KiB / 1024 expanded-argument bounds and 65507-byte wire limit. The entire
destination plan and packet are sealed before any DNS lookup/send: an invalid late destination
cannot leave an earlier destination with a partial admission effect. Each destination gets one
ordered attempt; DNS/send failures preserve later destinations without replaying earlier effects.
Partial datagrams report unconfirmed effects. Successful native replies own outgoing traffic and
bounded log publication.

Source liveness is tied to the exact producer NodeWaker. Both source retirement and transport
cancellation are rechecked after potentially blocking DNS, before transmission. Tuple feedback
coalesces by its parent target, preserving latest state without duplicate packets for changed
children; full output admission retains that state with a 250 ms retry. At most 256 pending targets
and 16 capture attempts are owned by an actor turn. Interface enumeration and incoming publication
are not claimed bounded by this outgoing work.

Final verification:

| Check                                                                             | Result                                                                                                                                                                      |
| --------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| OSC compiled-name selection, `--test-threads=4 --quiet`                           | **57 passed**: 39 preserved and 18 new outgoing cases.                                                                                                                      |
| Disjoint app preservation selections                                              | **642 distinct passes of 1087 listed**, including OSC; all on the final test executable.                                                                                    |
| Workspace/test compilation and strict all-target Clippy                           | Passed `cargo check --locked --workspace --tests` and `cargo clippy --locked --workspace --all-targets -- -D warnings`; final Clippy includes the borrowed override lookup. |
| Native app build                                                                  | Passed `cargo build --locked -p Chataigne2 --bin Chataigne2`.                                                                                                               |
| UI protocol generation and generated-directory diff                               | Passed; bindings unchanged.                                                                                                                                                 |
| Root/Core Cargo format, 17 touched app Rust files, maintained docs and whitespace | Passed.                                                                                                                                                                     |

The new cases cover frozen values/destinations, bounded traversal and cycles, disabled subtrees,
late whole-plan rejection, argument expansion/wire overflow, exact producer retirement, blocked-DNS
source/cancellation fences, partial sends without replay, later-destination outcomes, command
overrides, indexed tuple behavior, complete-request byte pressure and all script aliases over UDP.
Final review replaced snapshot overlays with borrowed override lookup so arbitrary node metadata
cannot escape the outgoing preparation reservation. Non-parameter arguments remain invalid even
when an invocation supplies an override.

Final executable: `target/debug/deps/Chataigne2-b14ffd1b9e52f9ac.exe`, SHA256
`e6eaf77e47201bbd8d55832f606b813d0d99ff5a5011388f3ea4cfc09dad0d4e`.
Selections ran from `apps/chataigne`. The maintained source inventory is **1645 files / 39 oversized**;
no touched OSC source exceeds 1000 lines and no new exception was introduced. No dependencies,
protocol DTOs or persistence formats changed. No hardware or desktop automation was used.
Core/foundation/IO/UI evidence retains its previously recorded dates; the full Formula gate is
still open for the same three creation failures. All owned jobs are terminal. Commit and push this
verified milestone, then continue C03's explicit input/options/scheduling and MIDI work.

### P03 C03 progress: bounded OSC incoming preparation and publication

Implemented after `822d5ee5` on 2026-10-21. This delivers worker preparation and bounded
received-value / `messageReceived` publication. OSC interface discovery and passive scheduling,
plus MIDI ownership/admission/recovery, remain open. **11 closure packages remain open; C02 is
still the only closed package. P03 remains 80%, whole plan 16%.**

Incoming address splitting, value conversion, callback JSON and bounded log preparation now run
on the native worker. Up to 16 adjacent valid messages from an admitted datagram share one
prepared envelope, preserving decoder-error boundaries and callback order. The batch admits its
complete weighted storage before publication, with a 2 MiB ceiling, 64-segment / 4096-byte paths
and the existing 128 KiB / 1024-value payload guard. Canonical received-value ownership is bounded
by 1024 envelopes / 32 MiB; native callbacks retain their existing shared 256 MiB pool. Final
value/JSON disposal follows those leases off the actor, including ordinary event/replay copies.

If either canonical pool is full, one measured draft retains its values and admitted callback
prefix and retries capacity without rebuilding JSON. Input pauses while independent outgoing
service continues. Coarse worker waits recover later capacity and explicit cancellation interrupts
terminal-prefix waits. No producer-driven threads or blocking actor joins are introduced.

The actor stops its 64-event drain after retaining one prepared input and leaves later inputs in
the weighted worker mailbox. `ReceivedValueCursor` applies detached subtrees, value writes and
indexed-child trimming with 256 work units / 64 KiB per turn; one larger admitted scalar gets a
whole turn. Graph edits have owning completions; failed edits abandon their unapplied suffix with
an explicit diagnostic and never replay an applied prefix. At most 16 callbacks publish together,
each with its own receipt so only unpublished events retry. Counted feedback guards cover repeated
internal writes to one parameter and clear after the envelope completes.

Native receive/poll failure clears readiness before a blocked accepted prefix is published.
Module disconnection handling also waits for its final retained input instead of discarding it
when the producer has already exited. The test-only decoded-message adapter remains separate
from the production prepared-input path. Generic parameter-event conversion costs and product-scale
throughput/latency retain their owning qualification gates; this does not claim a universal actor
deadline or close the remaining OSC discovery/scheduling work.

Final verification:

| Check                                                                             | Result                                                                                   |
| --------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------- |
| OSC compiled-name selection, `--test-threads=4 --quiet`                           | **69 passed**: 57 preserved and 12 new incoming cases.                                   |
| Disjoint app preservation selections                                              | **654 distinct passes of 1099 listed**, including OSC; all on the final test executable. |
| `cargo check --locked --workspace --tests`                                        | Passed on the final source.                                                              |
| `cargo clippy --locked --workspace --all-targets -- -D warnings`                  | Passed, including production and test paths.                                             |
| `cargo build --locked -p Chataigne2 --bin Chataigne2`                             | Passed.                                                                                  |
| UI protocol generation and generated-directory diff                               | Passed; bindings unchanged.                                                              |
| Root/Core Cargo format, 17 touched app Rust files, maintained docs and whitespace | Passed.                                                                                  |

New checks cover callback/path preservation, invalid input shape, the 16-message preparation cap,
value- and callback-pool saturation/recovery, a 700-value tuple's bounded detached trees and edits,
repeated trigger/callback occurrences, later failure after callback publication, prepared-mailbox
backpressure with independent output, terminal-prefix readiness and real UDP feedback/vector casts.
The former injected three-float multi-value fixture bypassed the native Vec3 cast; loopback tests
now cover true multi-value input and the existing vector cast separately. The terminal-pressure
fixture waits for an actual item/byte rejection and verifies the prefix of datagrams actually read;
it does not assume that batching must fill the item quota before the byte quota.

Final executable: `target/debug/deps/Chataigne2-b14ffd1b9e52f9ac.exe`, SHA256
`c85616c56d3d4ebb2a5fbd24bbe5aa453802064da90a9c62cf29e49db5fc27b5`.
Selections ran from `apps/chataigne`. The maintained source inventory is **1650 files / 39 oversized**;
no touched OSC source exceeds 1000 lines and no new exception was introduced. No dependencies,
protocol DTOs or persistence formats changed. No hardware or desktop automation was used.
Core/foundation/IO/UI evidence retains its previously recorded dates. The same three Formula
creation failures remain unqualified under P08/P12. All owned jobs are terminal. Commit and push
this verified milestone, then finish OSC interface discovery/passive scheduling and MIDI under C03.

### P03 C03: OSC discovery/passive service and local family qualification

Implemented after `3f935b4f` on 2026-10-21. OSC's remaining P03 discovery and scheduled-service
gaps are closed locally. **C03 remains open for MIDI; 11 packages remain open, C02 alone is
closed. P03 remains 80%, whole plan 16%.** The remaining inventory is 14 implemented families
plus Sound Card. This qualifies one family within the fixed package, without adding another
completion denominator or claiming hardware, other platforms or product-scale performance.

Native interface enumeration runs on an independent worker with 16 admitted live/cancelling
lifetimes. Its one-slot refresh command coalesces the latest selection behind a short profile
lock; generation checks reject results from older selections. Queries run outside that lock,
once per second or on selection change. One latest event has a 128 KiB mailbox allowance.
Blocked queries and final provider cleanup retain lifetime capacity without joining on the actor.

Before app label conversion, native inventory is limited to 256 retained slots and 512-byte
names. Catalogs admit at most 64 retained options / 64 KiB, including an unavailable row for
a missing selected address. A 63-row catalog can add that row without geometric capacity growth.
The configured endpoint never silently changes to Any. Unchanged domains share storage and
do not wake the actor. A global 128-domain / 8 MiB pool tracks the exact immutable domain Arc,
including parameter/snapshot/history copies, and owns final disposal away from the actor.

A narrow public `Edit::SetParamEnumOptions` fixes a concrete deferred native-result race:
application checks the live UUID and selected value, then changes only the current enum domain.
Stale/missing/replaced targets succeed without changing state or aborting later edits. Current
metadata and other constraints survive; ordinary events, history and undo/redo remain intact.
The shared network-choice helper used by UDP now uses this edit instead of whole-parameter
replacement and preserves an unavailable configured selection. This is an internal Rust edit,
not a wire DTO, persisted format or generic snapshot-optimization prerequisite.

OSC now uses passive `ModuleWake` service while retaining its compiled-kernel identifier.
Socket/output retries use absolute 250 ms deadlines; failed discovery startup/exit retries after
one second. Earliest-deadline selection prevents unrelated traffic postponing recovery. Stable
inventory produces no actor wake, scheduled callback or idle snapshot. Fatal native readiness
loss notifies the actor before an accepted input prefix waits for publication capacity. Disable,
removal and changed node lifetimes cancel workers/deadlines without blocking cleanup.

The new tests cover catalog bounds and missing-device recovery, unchanged storage, coalesced
selection generations, automatic query recovery, blocked native query/destructor admission,
choice/metadata races, undo/redo, idle service and disable/enable script-output recovery. Existing
terminal-prefix pressure now proves failure notification before the full mailbox is drained.

Final verification:

| Check                                                                             | Result                                                                                                                                  |
| --------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------- |
| OSC compiled-name selection, four test threads                                    | **77 passed**, including six discovery and two passive-service cases.                                                                   |
| Shared network-choice helper                                                      | **2 passed**: missing endpoint and deferred selection/label race.                                                                       |
| Disjoint app preservation selections                                              | **664 distinct passes of 1109 listed**: 77 OSC and 587 other cases, all on the final executable.                                        |
| `golden_engine --lib`, four test threads                                          | **578 passed / three existing ignored**. Includes three new guarded-choice tests.                                                       |
| `golden_core --lib`                                                               | Compiled and ran **zero tests**; compile evidence only.                                                                                 |
| Workspace/test check and strict all-target Clippy                                 | Passed `cargo check --locked --workspace --tests` and `cargo clippy --locked --workspace --all-targets -- -D warnings` on final source. |
| Native app build                                                                  | Passed `cargo build --locked -p Chataigne2 --bin Chataigne2`.                                                                           |
| UI protocol generation and generated-directory diff                               | Passed; generated bindings unchanged.                                                                                                   |
| Root/Core Cargo format, 15 touched app Rust files, maintained docs and whitespace | Passed.                                                                                                                                 |

Final executable: `target/debug/deps/Chataigne2-b14ffd1b9e52f9ac.exe`, SHA256
`216f3b902dec1e73583ba741c7864529305d890f47366b9ed5e7da7ab971afeb`.
Selections ran from `apps/chataigne`. The explicit source-policy scan (`rg --files apps crates
packages tools`, maintained extensions and directory exclusions) currently counts **1647 files /
39 oversized**. All 20 touched Rust sources are below 1000 lines; seven new focused files add no
size exception. No dependencies, protocol DTOs or persistence formats changed. No hardware or
desktop automation was used. The older 590-test aggregate, parameter/IO/UI gates and the three
existing Formula creation failures retain their previously recorded dates and qualification scope.

All owned jobs are terminal. Commit and push this verified milestone, then continue C03 with
MIDI native ownership, callback admission, bounded service, retirement and endpoint recovery.

### P03 C03: MIDI native discovery, connection, message and cleanup ownership

Implemented after `2a083ceb` on 2026-10-21. Native Midir discovery, input/output construction,
port resolution, sends, presence checks and final connection destruction now run on bounded
workers. Midly remains the codec. **C03 is still open: MIDI actor preparation/publication and
passive scheduling are not qualified by this native boundary. Counts remain 11 open / one
closed, P03 80%, whole plan 16%, and 14 implemented families plus Sound Card remain to qualify.**

All native workers share 32 admitted live/cancelling lifetime slots, reserved before thread
creation. Blocking setup, native send, query or final destructor retains its slot until it returns;
stop/drop cancels without joining. Input/output report native readiness rather than treating
thread startup as a successful connection. Failure clears readiness independently of reply
capacity, and an unwind guard clears it before native cleanup. Module-owned one-second retries
recover startup failure, device loss, stopped workers and lifetime-capacity exhaustion.

The input callback timestamps arrival and admits at most 256 messages / 2 MiB, with a 64 KiB
message limit. It never waits for downstream capacity: full or oversized input has an explicit
pre-admission rejection count. Worker-side decode and 24-tick clock analysis preserve accepted
message order and arrival-time BPM. Prepared input has 256 events / 4 MiB, including duplicate
decoded SysEx storage. A 32-message worker turn and bounded publication waits retain input
under pressure. The actor admits one batch of at most 32 events and leaves later native input
queued while graph materialization catches up. This does not qualify the remaining graph/edit
and callback completion work.

Output admits 256 packets / 2 MiB, at most 64 KiB each with a 512-byte description, plus one
active native send. Replies have a separate 256-item / 2 MiB bound. Outgoing traffic and bounded
worker-prepared logs follow actual successful sends. A failed native attempt is unconfirmed and
never replayed; admission closes and every accepted unsent suffix packet receives a not-sent
outcome. Ordinary output reconfiguration finishes accepted commands and receipts before opening
a replacement. A blocked retiring producer does not force repeated full-tree snapshots.

Discovery has one bounded result slot / 256 KiB, publishes changed observations and retries
queries once per second. Native inventory admits 64 retained ports per direction and 512-byte
names before app conversion; result metadata also bounds variant IDs and details. Availability
and connection resolution now agree: an indexed selected port cannot match another index solely
by label. Missing selected ports remain configured. Catalog application uses the public guarded
choice edit, preserving current labels and newer selections instead of replacing whole parameters.
Discovery warning IDs remain separate from input/output connection failures.

The MIDI coordinator shrank from **3165 to 614 lines**. Cohesive incoming, structure, timing,
outgoing, events, transport, script conversion and shared policy modules preserve the existing
command, script callback/template, CC and clock/MTC surfaces. Native implementation and tests
have separate focused files. No generic Golden API, protocol DTO, persistence format or
dependency was added. See the [native contract](../architecture/midi-runtime.md).

Remaining MIDI acceptance is explicit: bounded actor command/script/SysEx preparation,
delayed-packet admission and service, graph/edit/callback completion ownership, feedback
occurrence accounting, prepared catalog lifetimes, and passive clock/MTC/output deadlines.
The existing 240 Hz module rule remains until that service path is replaced. Do not close C03
or move these concrete gaps into another task merely because native calls are now off the actor.

Final verification:

| Check                                                                             | Result                                                                                                                             |
| --------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------- |
| MIDI compiled-name selection, four test threads                                   | **37 passed**: 22 existing module/codec/command cases plus 11 native-runtime and four port/catalog regressions.                    |
| Disjoint app preservation selections                                              | **701 distinct passes of 1124 listed**, including MIDI and the preserved 664 OSC/generator/audio/HTTP/stream/script/Mapping cases. |
| Workspace/test compilation                                                        | Passed `cargo check --locked --workspace --tests` on final source.                                                                 |
| Strict all-target Clippy                                                          | Passed `cargo clippy --locked --workspace --all-targets -- -D warnings` on final source.                                           |
| Native app build                                                                  | Passed `cargo build --locked -p Chataigne2 --bin Chataigne2`, including a final run after UI bundle regeneration.                  |
| UI protocol generation and generated-directory diff                               | Passed; bindings unchanged.                                                                                                        |
| Root/Core Cargo format, 18 changed app Rust files, maintained docs and whitespace | Passed.                                                                                                                            |

The native tests cover blocked setup/send/query/destructor without actor joins, lifetime admission
and later recovery, separate item/byte saturation, graceful accepted-send ordering, one failed
native effect without suffix replay, callback rejection accounting and ordered input, source-time
clock BPM and device disappearance. Port tests cover pre-conversion inventory/name bounds,
consistent indexed selection, missing selection and deferred catalog/metadata races. Initial test
fixture compilation and two zero-sized fake-port capacity assertions were corrected before the
final checks; no failing native implementation is claimed as verified.

Final test executable: `target/debug/deps/Chataigne2-b14ffd1b9e52f9ac.exe`, SHA256
`0aa1860043104823f267e917f051ae919d21b511b19b32a50d7bc963ea019562`.
Selections ran from `apps/chataigne`. The maintained source-policy scan counts **1663 files /
38 oversized**, with sixteen new focused files and the old oversized MIDI coordinator removed
from that count. All eighteen substantively changed Rust files remain below 1000 lines.

No physical endpoint was connected and no desktop automation was used. Native ownership uses
fake drivers for qualification; hardware, other platforms and native UI remain separate. The prior
Core/foundation/IO/UI test evidence and three existing Formula creation failures retain their
previous dates and scope. All owned jobs are terminal. Commit and push this milestone before
continuing MIDI's remaining actor preparation, lifetime/receipt and passive-service acceptance.

### P03 C03: MIDI complete outgoing preparation, admission and deadlines

Implemented after `2e471a2b` on 2026-10-21. MIDI command nodes and scripts now capture immutable
canonical inputs and submit complete outgoing requests. The existing output worker performs
override decoding, borrowed parameter lookup, byte parsing, SysEx framing and packet preparation.
The serialized MIDI module-request consumer and unbounded actor delayed-packet vector are removed.
**C03 remains open for MIDI input/callback/feedback/catalog and passive module service. Counts
remain 11 open / one closed, P03 80%, whole plan 16%, and 14 families plus Sound Card to qualify.**

All thirteen command builders and twelve script aliases use this path. Canonical snapshot,
execution and script argument leases retain their existing budgets; the actor does not clone
argument strings, serialize payloads or build a snapshot overlay. Command override lookup borrows
the selected parameter value, with the last override winning. Source retained weight is capped
at 256 KiB; script sources have at most 4096 arguments and selectors at most 128 bytes. Shared
byte conversion bounds source text to 256 KiB and resulting storage to 64 KiB. SysEx reserves its
complete framed size before copying, including F0/F7. Extreme octave addition now rejects through
checked arithmetic rather than unwinding a native worker.

Every captured command/script reserves 1 MiB before queue admission; fixed-size feedback reserves
8 KiB. The output mailbox remains 256 requests / 2 MiB. Before the first native effect, the worker
validates every packet, retained capacity, description and deadline in the one- or two-packet plan.
Full queues reject a complete request: a full note cannot send its note-on and then fail to reserve
its note-off. After preparation, unused bytes are released and the measured plan keeps a budget
lease through its complete packet lifetime, including a 4 KiB allocation/scheduler allowance.
Global capacity is 4096 requests / 128 MiB; each output lifetime admits at most 2048 live requests
across queued, preparing and scheduled work. Moving a request out of its mailbox does not release
its delayed-work admission. Native plan destruction precedes final lease/permit release.

The worker orders batches by capture-time monotonic deadline and arrival sequence, preparing one
source and servicing at most 16 native effects per turn. Delays retain the authored i32 millisecond
limit and checked Instant construction. Later immediate work can pass an older future note-off;
zero-delay pairs stay ordered. Actor ticks are not needed to transmit a due packet. Ordinary
reconfiguration finishes follow-ups on the admitted native lifetime, never on a replacement port.
Retirement before a first effect cancels the complete request; once an effect succeeds, source
deletion cannot discard that batch's accepted note-off. Explicit module/transport cancellation
still owns the remaining suffix. Failed native effects are unconfirmed and never replayed;
accepted unsent requests/suffixes receive explicit diagnostics. A successful send clears an earlier
output warning. Reply backpressure retains the current native result and admitted pending work.

Final verification:

| Check                                                                             | Result                                                                                     |
| --------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------ |
| MIDI compiled-name selection, four test threads                                   | **54 passed**: 37 preserved plus 17 new parser, command, scheduling and integration cases. |
| Disjoint app preservation selections                                              | **718 distinct passes of 1141 listed**, including MIDI and the preserved 664 app cases.    |
| Workspace/test compilation                                                        | Passed `cargo check --locked --workspace --tests`.                                         |
| Strict all-target Clippy                                                          | Passed `cargo clippy --locked --workspace --all-targets -- -D warnings` on final source.   |
| Native build                                                                      | Passed `cargo build --locked -p Chataigne2 --bin Chataigne2` on final source.              |
| UI protocol generation and generated-directory diff                               | Passed; bindings unchanged.                                                                |
| Root/Core Cargo format, 18 touched app Rust files, maintained docs and whitespace | Passed.                                                                                    |

Tests cover whole-batch rejection for a late invalid packet/deadline, full command and reply queues,
2048 future requests retaining admission after mailbox transfer, later immediate output, graceful
follow-up ownership, source retirement before and after note-on, and fixed-size 14-bit feedback
reservation. All thirteen builders have packet-preservation checks; frozen overrides, typed note
mode/CC behavior, bounded text/bytes/framing and extreme octave input are exercised. Engine tests
drive all twelve script aliases plus actual command execute overrides and manual triggers into a
fake native output. The old 37-case suite and the earlier expanded 47-case run also passed; they
are preservation evidence, not additional distinct pass counts.

Final test executable: `target/debug/deps/Chataigne2-b14ffd1b9e52f9ac.exe`, SHA256
`79c2fa0fac44e371d5e82397da2f8ca94e1c6a6e7f14c78d1443f76f22f31d65`.
Selections ran from `apps/chataigne`. The maintained source-policy scan counts **1672 files /
37 oversized**, with nine new focused source/test files. The touched MIDI command registry is
now **708 lines**, down from 1166; all eighteen changed Rust sources are below 1000 lines.
No dependencies, protocol DTOs or persisted formats changed. No physical endpoint was connected
and no desktop automation was used. Older Core/foundation/IO/UI evidence and the three existing
Formula creation failures retain their previous dates and scope. All owned jobs are terminal.

Commit and push this milestone before continuing MIDI input graph/edit/callback completion,
canonical input lifetimes, feedback occurrence accounting, catalogs and passive clock/MTC/recovery
service. These are still C03 acceptance; the outgoing worker milestone does not close the package.

### P03 C03: MIDI input preparation and graph/callback completion

Implemented after `17348c6a` on 2026-10-21. The native worker now prepares immutable incoming
messages, SysEx hex, generic/specific callback JSON and bounded logs. `midi_input` owns measurement
and canonical retention; `input_delivery` owns the fixed graph plan and completion cursors.
The old retry path could repeat note-info triggers before a missing value child materialized.
Each accepted message now advances domain state once and resumes only its unapplied graph suffix.
**C03 stays open for feedback accounting/CC authored structure, catalog lifetime and passive
clock/MTC/recovery service. Counts remain 11 open / one closed, P03 80%, whole plan 16%.**

Raw input retains the existing 256-slot / 2 MiB mailbox and 64 KiB wire-message limit.
Prepared events have 256 slots / 16 MiB, including both callback weights. One maximum wire-sized
SysEx produces 65534 payload bytes and bounded hex/JSON storage. Canonical envelopes share one
1024-item / 64 MiB disposer; callbacks retain the existing shared 16384-item / 256 MiB disposer.
A blocked preparer keeps the same draft and admitted callback prefix without rebuilding JSON.
Callback events/replay owners retain their canonical reservation after the input envelope retires.

The actor holds one graph cursor and at most sixteen callback-waiting deliveries; native input
consumption stops at that limit. Plans have at most twelve scalar operations. Each graph service
turn has sixteen operations / 64 KiB, with one bounded larger SysEx scalar allowed its own turn.
Touched parents are limited to 256 children. A missing path is one detached `NodeTree`, including
CC selector metadata. Numeric predecessor insertion preserves note/CC order; only a new minimum
needs a follow-up move after its runtime ID exists. Scalar creation does not produce a duplicate
value write, and a new trigger emits one occurrence after materialization.

Graph edit failure reports and abandons its unapplied suffix without replaying the applied prefix.
Auto-add/CC-width/precision changes fence stale suffixes; a changed rotary mechanism or missing
relative CC target cannot silently apply a stale relative value. Generic and specific callback
publications have separate receipts, so partial callback failure retries only the unpublished one.
Callback waits do not stall the next clock input after graph completion. Arrival Instants are
normalized against engine elapsed time; native BPM remains based on captured callback timestamps.
Disable/removal cancels queued input/graph/callback ownership and clears held-note state.

Verification on the current Windows/MSVC checkout:

| Check                                       | Result                                                                                                |
| ------------------------------------------- | ----------------------------------------------------------------------------------------------------- |
| MIDI compiled-name selection, four threads  | **70 passed**: 54 preserved and 16 new preparation, ownership, bounds and receipt cases.              |
| Disjoint app preservation selections        | **734 distinct passes of 1157 listed**, including MIDI and the unchanged 664-case preservation scope. |
| Workspace/test compilation                  | Passed `cargo check --locked --workspace --tests`.                                                    |
| Strict workspace all-target Clippy          | Passed `cargo clippy --locked --workspace --all-targets -- -D warnings`.                              |
| Native app build                            | Passed `cargo build --locked -p Chataigne2 --bin Chataigne2`.                                         |
| UI protocol generation                      | Passed `npm run codegen:golden-ui-protocol --workspace chataigne-ui`; generated bindings unchanged.   |
| Root/Core Cargo format and touched app Rust | Passed; maintained docs and whitespace checks passed.                                                 |

New cases cover maximum SysEx, retained spare-capacity rejection, velocity-zero callback meaning,
value/callback capacity recovery, canonical event lifetime after envelope disposal, pending receipts,
missing-subtree batching, trigger occurrence preservation, partial graph/callback failure,
configuration fencing, oversized touched parents, missing relative targets, large-scalar yielding,
and module-disable cancellation. The old MTC fixture now allows receipt-driven delivery to settle
its nine queued messages rather than assuming an unbounded four-turn drain. Note/CC sorting,
14-bit CC conversion, clock/MTC values, commands, scripts and native ownership gates are preserved.
The initial five regression failures were repaired; the final expanded 70-case run passed.

Test executable: `target/debug/deps/Chataigne2-b14ffd1b9e52f9ac.exe`, SHA256
`af2a7bb9f7aa80675f3ac8d19bc04488863afd108114a6a30665347e894c1bfc`.
Selections ran from `apps/chataigne`. The maintained source-policy scan counts **1678 sources /
37 oversized**; all eighteen changed Rust sources remain below 1000 lines (largest: the existing
MIDI test module, 947). No dependencies, protocol declarations or persisted formats changed.
No physical endpoint was connected and no desktop automation was used. Prior Core/foundation/IO/UI
evidence and the known Formula creation failures keep their previous scope and dates. All owned
checks are terminal.

Commit and push this verified milestone, then finish MIDI feedback/CC authored structure,
catalog lifetime and passive service inside C03. Keep the remaining eleven packages explicit;
this input ownership milestone does not close C03 or advance its package-closure score.

### P03 C03: MIDI feedback provenance and captured event values

Implemented after `9eeb154f` on 2026-10-21. The previous per-parameter suppression count could
consume a later authored change after an unchanged received write or a rejected graph suffix.
Feedback also read the final snapshot value for every event, so appended values and relative CC
deltas could be repeated or misreported. These concrete occurrence failures justify the narrow
Golden event contract below; this is not a generic snapshot/copy optimization prerequisite.
**C03 remains open for authored CC structure, catalog lifetime and passive clock/MTC/recovery
service. Counts remain 11 open / one closed, P03 80%, whole plan 16%.**

`golden_engine` now owns per-request parameter provenance through `EditQueue::push_from`,
`EditRequest::with_param_source` and `ProcessCtx::set_param_from`. Direct `SetParam` and
`SetParamConstraints` changes carry the selected runtime producer in `EventKind::ParamChanged`.
Ingress preserves it, coalescing replaces it with the retained request, and failed/no-op writes
leave no future suppression state. Standard node inbox dispatch scopes `ProcessCtx::current_event`
to the actual event without copying its payload. Lifecycle follow-up edits do not inherit a
request's provenance. History replay has no original runtime producer. This metadata is skipped
by event serialization and excluded from UI DTOs; it is not an authorization mechanism.

MIDI received values, transport state and CC conversion/clamping use the module's producer ID.
The ignore-next-change map and its retry/cleanup bookkeeping are removed. App-owned `feedback`
uses the captured old/new values, bounded fixed MIDI paths and typed scalar matching. It retains
individual note values, relative deltas, complete 14-bit pairs and clock/system trigger
occurrences even when the tree snapshot already contains the last batch value. Static clock
IDs are resolved using the DSL's scoped leaf declaration. String-to-number parsing and arbitrary
ancestor walks are removed from feedback resolution. Native feedback keeps the existing 8 KiB
reservation and complete-batch admission/service contract.

Rust consumers and manually constructed Formula/state-machine test events now handle the
runtime-only event field. Existing touched app fixtures were formatted consistently; their
snapshot/value behavior is preserved. No dependency, persisted format or UI protocol declaration
changed. Architecture contracts explain Core ownership and app feedback policy in the same change.

Verification on the Windows/MSVC checkout:

| Check                                             | Result                                                                                                                 |
| ------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------- |
| MIDI selection, four threads                      | **79 passed**: 70 preserved and nine new fake-output feedback cases.                                                   |
| Full Golden engine library suite                  | **586 passed / three existing ignored**, including eight new provenance cases; 589 listed.                             |
| Disjoint app preservation selections              | **792 distinct passes of 1166 listed**: 79 MIDI + 664 existing preservation + 49 newly exercised event-consumer cases. |
| Workspace/test compilation                        | Passed `cargo check --locked --workspace --tests`.                                                                     |
| Strict workspace Clippy                           | Passed `cargo clippy --locked --workspace --all-targets -- -D warnings`.                                               |
| Native app build                                  | Passed `cargo build --locked -p Chataigne2 --bin Chataigne2`.                                                          |
| UI protocol generation                            | Passed `npm run codegen:golden-ui-protocol --workspace chataigne-ui`; generated bindings unchanged.                    |
| Root/Core and touched app format; docs/whitespace | Passed; maintained docs and whitespace checks passed.                                                                  |

Core tests exercise unchanged writes, multiple sources/values, coalesced receipt rejection,
applied-prefix failure, constraint clamping, serialization/source stripping, callback scope and
undo/redo callback provenance. The first undo test incorrectly inspected an inbox already consumed
by history replay; its corrected observer verifies actual callbacks, and the full suite passes.
Fake MIDI output covers unchanged received writes followed by user edits, mixed received/authored
values, rejected suffixes, separate relative deltas and 14-bit pairs, repeated clock triggers,
other runtime producers, attributed clamping and 14-to-7-bit conversion suppression. The initial
clock feedback test exposed scoped DSL IDs; the resolver was repaired and all 79 MIDI cases pass.

App test executable: `target/debug/deps/Chataigne2-b14ffd1b9e52f9ac.exe`, SHA256
`6d17c9e0bc6aae9e5fbfadec4448a93495a8e64ce18dff3c1aa6c68148926006`.
Core test executable: `target/debug/deps/golden_engine-acb0109b8761871d.exe`, SHA256
`5ab78d3f4d4c8298404d8c6769f61c9ba8b2e8ab1e2e2bae07603ec8c24146df`.
App selections ran from `apps/chataigne`. They include the existing 664-case preservation scope
plus Formula snapshot/value tests (four), managed-node tests (22), processor snapshot gates
(four), state-machine command dispatch (13) and state-machine snapshot gates (six).

The maintained source-policy scan counts **1681 sources / 37 oversized**. This change touches
41 Rust sources, including the narrow consumer/fixture field updates; its three new files are
focused runtime/test modules. The pre-existing centralized engine/macro and app test files retain
P12's open readability gate; no oversized-file cleanup credit is claimed. No physical endpoint
was connected and no desktop automation was used. Broader Formula, device/platform and
product-scale qualifications remain open at their owning tasks. All owned checks are terminal.

Commit and push this verified milestone before continuing bounded MIDI CC authored structure,
prepared catalog lifetime and passive service. Feedback occurrence repair does not close C03 or
advance the remaining-package score on its own.

### P03 C03: bounded MIDI authored structure and startup locking

Started from `3bcb3514` on 2026-10-21, Windows/MSVC 1.97.0, default workspace features.
The previous goal turn was a status response (**no progress**); the checkout was revalidated,
and implementation resumed from the existing uncommitted MIDI/Core changes without restarting
completed native, input, output or feedback work.

The app owns CC conversion and MIDI value permissions. One service turn prepares one channel's
fixed 32 CC pairs: at most 200 guarded data requests plus one deferred ordering callback, a
256-child parent bound and a shared 256-node touched/retired footprint. New CC parameters include
rotary selectors in one detached tree. Existing value/default/range changes normalize first and
commit in one public parameter node mutation. Read fences precede all effects. Failed applied
prefixes reconcile from actual declared ranges, preserving completed widths without double shifts.
Narrowing writes the low-bit destination before changing the combined source, preserving every
bit across an interrupted split.
Rejected unapplied requests restore the previous width unless superseded. Numeric ordering is a
separate pass of at most 129 requests, with no per-child snapshot callback.

Startup locking follows the typed MIDI declaration surface in at most 64 cursor steps / 24 metadata
patches per turn, retaining at most five ancestry frames. Control-owned descendants retain their
permissions. Receipts retain accepted work without repeated edits or snapshots while pending.
Input consumption pauses during structure work; worker wakes finish authored work while disabled
without opening native IO. The first coalesced callback has its own completion receipt, so
discarding it cannot leave input permanently paused. Removal cancels cursors and invalidates
deferred callbacks.

Golden's existing runtime provenance API now covers direct parameter state mutations; their
queued follow-up edits do not inherit attribution. No new edit variant, dependency, persisted
format or UI protocol declaration is introduced. See [MIDI ownership](../architecture/midi-runtime.md)
and [Core attribution](../architecture/application-contracts.md#parameter-event-attribution).

Verification on the current Windows/MSVC checkout:

| Check                                             | Result                                                                                                                       |
| ------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------- |
| MIDI, four threads                                | **91 passed**: 79 preserved and twelve new structure/recovery cases.                                                         |
| Disjoint app preservation                         | **804 distinct passes of 1178 listed**: 91 MIDI + 664 existing preservation + 49 Formula/state-machine event-consumer cases. |
| Full Golden engine library suite                  | **588 passed / three existing ignored**, 591 listed, including ten provenance cases.                                         |
| Workspace/test compilation                        | Passed `cargo check --locked --workspace --tests`; final all-target Clippy also checks the expanded test sources.            |
| Strict workspace Clippy                           | Passed `cargo clippy --locked --workspace --all-targets -- -D warnings`.                                                     |
| Native app build                                  | Passed `cargo build --locked -p Chataigne2 --bin Chataigne2` with `CARGO_INCREMENTAL=0`.                                     |
| UI protocol generation                            | Passed `npm run codegen:golden-ui-protocol --workspace chataigne-ui`; generated bindings unchanged.                          |
| Root/Core and touched app format; docs/whitespace | Passed.                                                                                                                      |

The final app selections use the same 713-case preservation scope as the previous milestone.
New cases verify complete 32-pair conversions, rapid width coalescing, pending receipts, discarded
callback recovery, stale read rejection, oversized retirement, startup idempotence, bounded locking
with control permissions preserved, disabled worker-wake completion, numeric ordering, and failed
prefix recovery in both conversion directions. Review found and repaired narrowing's low-bit
publication order and the discarded-callback latch. Initial fixture failures assumed nested edits
settled in one drain; fixtures now observe the ordinary follow-up drain and inbox delivery.
Core tests verify direct parameter mutation provenance, independent follow-up provenance, coherent
state after a later failure, and undo/redo of the complete parameter state.

The initial default native build failed with LNK2019/LNK1120 (1146 unresolved anonymous/incremental
symbols), matching the recurring P12 artifact gate. The explicit nonincremental retry passed;
no source or global Cargo configuration workaround was added. Native compilation is qualified for
that retry configuration only. This does not close the incremental artifact gate or establish
native UI/device qualification.

App test executable: `target/debug/deps/Chataigne2-b14ffd1b9e52f9ac.exe`, SHA256
`40d72527371c101b23324f263e5d48eeff5f358304d7e2a0e0b6a11691b7c92c`.
Core test executable: `target/debug/deps/golden_engine-acb0109b8761871d.exe`, SHA256
`ab52f20a66596a34454d17f61fea590b9767c091d67afadf0ded1bf8801529bc`.
Native executable: `target/debug/Chataigne2.exe`, SHA256
`083a33d8a02e52d2692a5fdc72bb8e5acf79754ed9fb0f7f67a0ba952ea983c6`.
App selections ran from `apps/chataigne` with four threads. The maintained source-policy scan
counts **1684 sources / 37 oversized**; all thirteen touched/new Rust sources are below 1000 lines.
All owned check processes are terminal. Prior IO/UI/macro and broader Formula evidence retain
their dates and scope; no full Formula pass is claimed.

**C03 remains open for prepared catalogs and passive clock/MTC/recovery service.** Output deadlines
are already worker-owned; the overall 240 Hz MIDI rule is not yet removed. Counts remain **11 open /
one closed, P03 80%, whole plan 16%**. No physical endpoint or desktop automation was used.

### P03 C03: MIDI prepared catalog lifetimes and guarded publication

Implemented from `1f2ddedd` on 2026-10-21, Windows/MSVC 1.97.0, default workspace features.
The preceding goal turn delivered authored structure (**progress**); this checkout began clean.
The app-owned discovery boundary now prepares enum choices instead of passing raw port inventories
back for actor conversion and disposal. No Golden, protocol, dependency or persisted format changed.

Native inventory admission still limits each direction to 64 ports and its existing metadata
capacities. Prepared directions contain at most 66 options / 256 KiB, including None and a missing
selection. Both authored selections have 1024-byte bounds. A one-slot command coalesces the latest
selection profile; generation-tagged stale results cannot replace newer configuration. The worker
retains one pending latest result behind a one-slot / 1 MiB result mailbox and reuses canonical
choice identity when content is unchanged.

The public `PayloadPool::try_retain_arc` contract admits at most 128 direction catalogs / 32 MiB
across all modules, parameters, snapshots, history and queued edits. Only the pool's background
disposer releases the last canonical array. Capacity pressure reports explicitly and recovers after
old owners release; failed disposer startup remains retryable. Native queries and temporary raw
inventories stay on the already admitted discovery worker, including cancellation during a blocked
query. Actor publication uses at most six guarded choice/warning edits and one owning receipt.
Pending receipts neither repeat publication nor request another snapshot; failed/dropped requests
reuse the same canonical arrays. Read fences cover stale warning clears as well as choice updates.
Disable/removal cancels discovery and its pending delivery. Discovery restart remains on the existing
one-second module refresh path; overall passive service is deliberately still an open criterion.

Current verification:

| Check                                             | Result                                                                                                                       |
| ------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------- |
| MIDI, four threads                                | **96 passed**; seven new catalog cases replace two older publication fixtures.                                               |
| Disjoint app preservation                         | **809 distinct passes of 1183 listed**: 96 MIDI + 664 existing preservation + 49 Formula/state-machine event-consumer cases. |
| Workspace/test compilation                        | Passed `cargo check --locked --workspace --tests`; final strict Clippy checks the completed sources.                         |
| Strict workspace Clippy                           | Passed `cargo clippy --locked --workspace --all-targets -- -D warnings`.                                                     |
| Native app build                                  | Passed `cargo build --locked -p Chataigne2 --bin Chataigne2` with `CARGO_INCREMENTAL=0`.                                     |
| UI protocol generation                            | Passed `npm run codegen:golden-ui-protocol --workspace chataigne-ui`; bindings unchanged.                                    |
| Root/Core and touched app format; docs/whitespace | Passed.                                                                                                                      |

New cases cover 64 ports plus a maximum-length missing selection, retained spare-capacity and
metadata rejection, duplicate variants, canonical identity reuse under full lifetime admission,
external snapshot owners holding capacity, release/recovery, changed selection generations with
a full result slot, stale publication preserving authored values/metadata, pending receipts and
discarded-request retries. Existing discovery failure/recovery and blocked-query cancellation
cases use the new result shape. Both intermediate and final 96-case selections passed.

App test executable: `target/debug/deps/Chataigne2-b14ffd1b9e52f9ac.exe`, SHA256
`434d484d7cb99d4998cf1fdef7bca7837a6096653890e049d9be9e7649baf551`.
Selections ran from `apps/chataigne` with four threads. The maintained source-policy scan counts
**1688 sources / 37 oversized**; all twelve touched/new Rust sources remain below 1000 lines.
Core/IO/UI/macro evidence retains its prior scope and dates; Core sources are unchanged. No physical
endpoint or desktop automation was used. Default incremental linking retains the known P12 gate.

**C03 remains open only for MIDI passive clock/MTC/recovery scheduling.** OSC is locally qualified;
C01 and C04-C12 remain open separately. Counts stay **11 open / one closed, P03 80%, whole plan 16%**.

Final native executable: `target/debug/Chataigne2.exe`, SHA256
`91011da612f95aea63366b71b45117a4c50e14262692154962d60a79957f61b6`.
Root and Core Cargo format, all twelve touched app Rust files, maintained docs and whitespace
checks passed. All owned processes are terminal; no default incremental-link repair is claimed.
Commit and push this verified milestone before continuing passive MIDI service.

### P03 C03: passive MIDI service and local package closure

Implemented from `a41f7dd1` on 2026-10-21, Windows/MSVC 1.97.0, default workspace features.
The preceding goal turn delivered catalog ownership (**progress**); this checkout began clean.
No Golden, protocol, dependency, persisted format or app-shell source changed. The fixed C03
acceptance is now locally verified for MIDI and OSC; external device/platform gates remain separate.

The app-owned `service` boundary replaces MIDI's 240 Hz update rule and one-second actor refresh
counter with discrete worker wakes. Input, output, retiring output and discovery readiness are
observed from bounded worker mailboxes. Endpoint setup uses current bound handles, and failures
retain one retry deadline one second ahead. Blocked setup/cleanup retains its existing native
lifetime reservation without polling or repeated snapshots. A retiring output's actual exit wakes
latest-state reconfiguration; its admitted delayed packets remain output-worker-owned.

Clock/MTC expiry uses captured arrival times normalized to engine elapsed time and one reusable
shared `ModuleWake` deadline: earliest 500 ms clock / 250 ms MTC expiry or native/discovery retry.
Later pulses replace the prior activity deadline. Start/Continue without a pulse retains its
explicit transport state. Disable cancels native work and deadlines; accepted authored structure
can finish while disabled. Removal drops runtime registration and invalidates late node wakes.
Registration publishes capabilities from the current bound configuration, including explicit values
applied between initialization and ready. Authored transport changes request service and reset retry.

Worker snapshot requests cover structure/catalog reads or input graph work that can advance.
Queued input does not force a snapshot while structure is waiting. Status/reply, timeout and retry
service use bound handles. Existing graph/callback/catalog receipts and bounded per-turn work remain
in place. Healthy discovery, idle modules and blocked native retirement request no scheduled actor
callbacks, recurring worker polls or idle snapshots. Commands, script aliases/templates/callbacks,
feedback provenance, CC identity/ranges, note occurrences and clock/MTC values are preserved.

Verification on the final uncommitted tree:

| Check                                             | Result                                                                                                                                                        |
| ------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| MIDI, four threads                                | **106 passed**: 96 preserved plus ten new passive/startup/backpressure cases.                                                                                 |
| Disjoint app preservation                         | **819 distinct passes of 1193 listed**: 106 MIDI + 664 existing preservation + 49 Formula/state-machine event-consumer cases.                                 |
| Workspace/test compilation                        | Passed final `cargo check --locked --workspace --tests`.                                                                                                      |
| Strict workspace Clippy                           | Passed final `cargo clippy --locked --workspace --all-targets -- -D warnings`.                                                                                |
| Native app build                                  | Passed final `cargo build --locked -p Chataigne2 --bin Chataigne2` with `CARGO_INCREMENTAL=0`; default incremental linking retains P12's known artifact gate. |
| UI protocol generation                            | Passed `npm run codegen:golden-ui-protocol --workspace chataigne-ui`; bindings unchanged.                                                                     |
| Root/Core and touched app format; docs/whitespace | Passed.                                                                                                                                                       |

Ten new cases verify no scheduled callbacks/worker polling/idle snapshots across 100 steady turns,
native input delivery through passive graph turns, real earliest MTC/clock deadline wakeups,
snapshot-free retry, failed-worker retry ownership, blocked retiring cleanup with held admission
and reconfiguration after release, cancellation/re-enable, worker-driven discovery error recovery,
queued-input snapshot suppression behind structure, and startup capability publication. The first
104- and 105-case suites and strict Clippy passed before the final startup case; the final 106-case
suite passed. Fixture compilation issues (warning presentation fields and private capability access)
were corrected using the public node snapshot. No failed runtime assertion was waived.

App test executable: `target/debug/deps/Chataigne2-b14ffd1b9e52f9ac.exe`, SHA256
`753171fdeb8c9319728dd82e9a0ad81c452a5694025e4ce280de07405c6e1254`.
Native executable: `target/debug/Chataigne2.exe`, SHA256
`065f3a5118a369f4dd89324cda9753a68395647aa2c2c743f537e8636924a9a3`.
App selections ran from `apps/chataigne` with four threads. Existing Core **588 passes / three
ignored**, IO/UI/macro and broader Formula evidence keep their previous dates and scope; no full
Formula pass is claimed. No physical endpoint or desktop automation was used. The source-policy
scan counts **1690 sources / 37 oversized**; all eleven touched/new Rust sources are under 1000 lines.

**C03 closes locally.** Fixed closure counts advance to **10 open / two closed (C02/C03)**.
The remaining-module workstream is **2/11 (~18%)**, P03 approximately **81%**, whole plan **16%**;
Sound Card remains **50% partial** and P04-P18 remain unstarted. Next finish C01's concrete P03
criteria without pulling P04 ordering or P16 generic scaling into this package.

Final root/Core Cargo format, all eleven touched app Rust files, maintained docs and whitespace
checks passed. All owned jobs are terminal. Commit and push this C03 closure milestone before
continuing C01; keep the full P00-P18 goal active.

## C01 Sound Card command preparation and publication

Implemented from `98f303f0` on 2026-10-21, Windows/MSVC Rust 1.97.0, default workspace features.
The preceding goal turn only reported status (**no progress**); this turn changed app-owned code,
qualified its command boundary and preserved the prior module/consumer gates. No Golden, protocol,
dependency, persisted schema or app-shell source changed. C01 remains OPEN; no package or percentage
completion credit is claimed for its still-unfinished structure/default-route and feedback criteria.

Five command nodes dispatch frozen `CapturedInvocation` leases through a typed module callback.
The event predicate checks only topic/ID, avoiding an earlier full JSON decode before admission.
Five script methods retain the engine-owned argument lease. Script success means preparation-queue
admission; later parsing/native failures publish bounded backend warnings. Existing script snippets,
method names, argument shapes and native callback surfaces are preserved. Command nodes retain the
structured command-result topic and native sequence. The serialized internal request round trip,
request serde derives and obsolete request handler/type validator were removed.

One passive preparation worker per active Sound Card uses the public `golden_io` primitives.
A global 64-item pool reserves 4 MiB per request before queue transfer; the conservative 256 MiB
reservation covers decoded override/argument preparation and retained results, while canonical
snapshot/JSON/script leases keep their independent budgets. Sources admit at most 256 KiB, command
paths at most 64 KiB and script calls at most four arguments. UI control JSON is checked before
cloning/decoding at 64 KiB, 256 values and depth eight. Eight worker lifetime slots include retiring
workers. Full/stopped capacity rejects explicitly. Source/module cancellation and final disposal
stay off the actor; worker teardown never joins it.

Worker preparation borrows indexed command parameters and applies last-override precedence without
copying snapshot node metadata. Source retirement cancels work. Native retirement with a live
source retains a failure result; it does not silently discard an admitted occurrence. Module lifetime
wakers fence dispatch before enqueue, and a captured runtime lifetime fences native application.
A replacement module/runtime cannot receive an old command. Preparation has no native effects.

The actor services at most four prepared deliveries per wake. Only pending gain work requests a
command-service tree snapshot. It checks target UUID, parent and float type before native admission;
absolute gains do not require the old gain value, so queued gains preserve FIFO behavior. Native
submission occurs once. Authored gain writeback, bounded warning edits and result publication have
independent receipts; a failed edit prefix cannot replay an admitted command. If authored target
retirement prevents writeback after native admission, the failure result retains its admitted
sequence. Diagnostic text is bounded to 4 KiB on the worker. Disable/removal cancels the module's
command service. A failed preparation worker retires and a later command can restart it.

Verification on the final source:

| Check                                          | Result                                                                                                                                       |
| ---------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------- |
| Sound Card and command selection, four threads | **156 passed**: 143 preserved plus 13 new command/preparation/retirement cases.                                                              |
| Disjoint app preservation                      | **676 passed**: generators, HTTP, six native streams, shared ingress, OSC, MIDI, module commands and scoped Formula/state-machine consumers. |
| Current app totals                             | **832 distinct passes / 1206 listed**; selector sets were checked nonempty and disjoint. No full-app/Formula pass claimed.                   |
| Workspace/test compilation                     | Passed `cargo check --locked --workspace --tests`.                                                                                           |
| Strict workspace Clippy                        | Passed `cargo clippy --locked --workspace --all-targets -- -D warnings`.                                                                     |
| Native app build                               | Passed on the final source with `CARGO_INCREMENTAL=0`. Default incremental MSVC artifact failures remain P12.                                |
| UI protocol generation                         | Passed; generated bindings unchanged.                                                                                                        |
| Formatting and source policy                   | Root/Core actual fmt and touched app fmt passed; source scan **1693 / 37 oversized**. Docs and whitespace passed.                            |

The Null/fake fixtures exercise passive delivery of all five script methods, two queued absolute
master gains and a real command dispatch/result through worker wakes. The missing-file play request
opens no physical audio endpoint. Other new cases cover UTF-8 error bounds, byte/item recovery,
input freezing, full-mailbox cancellation, retired source, removed gain target, replacement module,
replacement runtime, retained native-retirement failure, worker restart and non-replay after a
failed warning-publication prefix. Initial fixture failures (snapshot-budget saturation and
an unaddressed execute event) were corrected; the final full owning selection passed.

Core remains at its previously dated 588 passes / three existing ignored because its source is
unchanged. The three known Formula catalog/curve/gradient creation failures, advisory/native
incremental artifact gates, physical devices/routes, native UI, platforms and product-scale
qualification remain at their owning tasks. P04 gain/configuration ordering is not qualified here.
Next: C01 authored structure/default-route receipts and bounded scans, then remaining feedback and
native reconfiguration/failure criteria. Counts stay **10 open / two closed**; P03 **~81%**, full plan
**~16%**, Sound Card **50% partial**. Commit/push this verified milestone on `main`; keep P00-P18 active.

Final artifact provenance for this C01 source and its 832-case selection:

- App tests: `target/debug/deps/Chataigne2-b14ffd1b9e52f9ac.exe`, SHA-256
  `e723897ff9796228c5b96a9ab69a3929c8c61de21283d159581f3b9780008027`.
- Native app: `target/debug/Chataigne2.exe`, SHA-256
  `3518c4e5d5e33c95fdae094321ac38c3582cfa72ecf610e22a029184572cbb3b`, built with
  `CARGO_INCREMENTAL=0`. The final native build, workspace/test check, strict all-target Clippy and
  all owned test processes are terminal and passed. No app process or physical endpoint was launched.

## C01 route reset and default-seed receipts

Implemented from `f1a7175e` on 2026-10-21, Windows/MSVC Rust 1.97.0, default workspace features.
The preceding goal turn committed/pushed command ownership (**progress**); this checkout began clean.
The app-owned `routes` boundary now owns reset/default flags and graph publication. No Golden,
protocol, dependency, persisted schema or app-shell source changed. C01 is still OPEN.

Reset service uses one 16-root deletion budget across input/output. It reads a bounded prefix from
indexed child slices instead of cloning/scanning the complete folder. Pending reset/seed receipts
backpressure another reset turn; device selection cannot clear routes before an earlier accepted
seed settles. A reset flag clears only after completion resolves and a fresh snapshot sees the
folder empty. On prefix failure, the next turn reads only surviving roots rather than replaying
committed deletions. Existing stale-route pruning is suspended while that direction is owned by
reset/seed work, preventing competing deletions from the same snapshot.

Defaults look up the first two derived logical channels and deterministic route UUIDs directly.
They no longer build an all-channel reference vector. Existing committed roots are checked for
parent/type/materialized parameter ownership, so a partial two-tree prefix retries only the missing
tree. Pending flags survive until acknowledgement. Names keep the existing semantic channel label
and cache behavior; physical keys admit at most 1024 bytes and naming metadata 64 KiB per field
before copying. An invalid direction waits until the other direction's accepted writes settle,
then reports a fixed backend warning instead of abandoning their receipt. Missing inventory or
unpublished channels wait for subsequent native/structural publication.

Each graph batch fences module/parent/target reads and the app module's publication lifetime using
public Golden preconditions/revisions. Disable, destroy, replacement and restoration invalidate
old work, including when authored UUIDs survive replacement or history retains the old node.
Active reset/seed phases reject conflicting route intents explicitly. Re-enable requests fresh
reconciliation. Short-name-only channel edits now request reconciliation too, restoring the
existing label-derived naming invariant without depending on native telemetry.

Verification on the final source:

| Check                                          | Result                                                                                                                                                     |
| ---------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Sound Card and command selection, four threads | **166 passed**: 156 preserved plus ten new route cases.                                                                                                    |
| Disjoint app preservation                      | **676 passed** across generators, HTTP, six native stream adapters, shared ingress, OSC, MIDI, module commands and scoped Formula/state-machine consumers. |
| Current app total                              | **842 distinct passes / 1216 listed**; selector sets checked nonempty/disjoint. The repeated timing case is not added again.                               |
| Workspace/test compilation                     | Passed `cargo check --locked --workspace --tests`.                                                                                                         |
| Strict workspace Clippy                        | Passed `cargo clippy --locked --workspace --all-targets -- -D warnings`.                                                                                   |
| Native app build                               | Passed with `CARGO_INCREMENTAL=0`; default incremental artifact failures remain P12.                                                                       |
| UI protocol generation                         | Passed; generated bindings unchanged.                                                                                                                      |
| Formatting / source policy                     | Root/Core actual fmt and touched app fmt passed; **1695 source files / 37 oversized**. Docs/whitespace checked before delivery.                            |

The pure-tree and Null cases cover empty reset, shared turn admission and pending backpressure,
five committed removals followed by a failed suffix, one committed default tree followed by a failed
second tree, seed/reset ordering, replacement preserving UUIDs, disable/enable, oversized key/name
rejection, one invalid direction with another accepted direction, and passive Null disconnect.
The 4096-route fixture records **36.9 microseconds**, preparing exactly 17 edits (16 removals and
one fence); setup/application/full teardown and snapshot/UI costs are excluded. This single sample
proves no end-to-end percentile or cross-platform/device bound. The owning suite also verifies
short-name normalization without telemetry. Initial failures identified missing reset-cohort
backpressure and a test assumption that contradicted the existing label-derived naming policy;
both are resolved in the final passing suite.

Final artifact provenance:

- App tests: `target/debug/deps/Chataigne2-b14ffd1b9e52f9ac.exe`, SHA-256
  `15a889eade7aadabcedb5e0395066bc6e5a69ed34e89b157b9d21be0be8aa270`.
- Native app: `target/debug/Chataigne2.exe`, SHA-256
  `70155c2d7405824a88a3dad045d8287752a4f1e42284550552b89534b68e327e`, built with
  `CARGO_INCREMENTAL=0`. All owned jobs are terminal. New scenarios use pure authored fixtures and
  the explicit Null backend, with no real-device qualification.

Remaining C01 work: bound derived gain/level/migration creation, stale-route scans and publication,
manual route lookup, feedback and remaining native failure/reconfiguration fences. The route
subsystem does not establish those broader criteria or P04 audio ordering. Core retains its
previously dated 588 passes / three existing ignored. Formula/advisory/native-incremental/UI/device/
platform/product-scale gates retain their owners. Counts remain **10 open / two closed**;
P03 **~81%**, Sound Card **50% partial**, whole plan **~16%**. Commit/push on `main`; keep P00-P18 active.

## C01 bounded route sweeps and manual matching

Implemented from `7028dedc` on 2026-10-21, Windows/MSVC Rust 1.97.0, default workspace features.
The preceding goal turn delivered reset/default receipts (**progress**); this checkout began clean.
No Golden, protocol, dependency, persistence schema or app-shell source changed. C01 stays OPEN.

`routes::prune` replaces the full stale-route child-list clone/scan with a canonical snapshot sweep.
Each turn reads at most 32 roots and queues at most eight deletions. Current UUID/parent/reference
checks preserve corrected/moved/removed nodes; bounded intent/target/parent reads and the module
lifetime guard fence writes. A pending receipt blocks another deletion turn. Failed prefixes recheck
survivors without replaying committed removals. Own deletions and concurrent changes trigger a final
fresh sweep rather than restarting the complete scan after every prefix. Sweeps release their source
when the folder disappears. Snapshot admission failure retains a 250 ms retry deadline and suppresses
snapshot requests and immediate busy wakes while waiting. Reset takes ownership only after accepted
prune edits settle. Cleanup now proceeds before the driver-None lifecycle return; it needs no native
runtime or periodic actor update.

`routes::lookup` removes synchronous manual route matching and per-row physical-key string clones.
One worker borrows the frozen snapshot, checks cancellation every 64 rows, and limits matching to
65536 roots. The existing first matching child wins, including routes with arbitrary authored UUIDs.
Requests remain FIFO through graph completion. Each module admits 32 requests, with 256 global live
request permits, eight live/retiring worker permits and one weighted 8 KiB command/reply slot. Key and
UUID-token retained capacities admit at most 1024/128 bytes before queueing. Snapshot storage keeps
its independent canonical budget. Source stamps are rechecked on actor resumption, and bounded
read/lifetime fences protect publication. Stale/failed prefixes retry at most eight times. Selection
or lifetime changes cancel old requests explicitly. A failed worker reports its request and later
requests can restart; an empty queue retires the worker without joining. Worker waits require no
route snapshot. Name caches are bounded and constructed only for new route trees; rename and
unlink validation use identity without cloning an unused old name. Successful idempotent requests
clear the routing warning too. The Rust UI request DTO/topic and existing callback/script surfaces
are unchanged.

Verification on final source:

| Check                                          | Result                                                                                                                                 |
| ---------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------- |
| Sound Card and command selection, four threads | **179 passed**: 166 preserved plus 13 new scan/query/recovery cases.                                                                   |
| Disjoint app preservation                      | **676 passed**: generators, HTTP, six stream adapters, shared ingress, OSC, MIDI, commands and scoped Formula/state-machine consumers. |
| Current app total                              | **855 distinct passes / 1229 listed**; selector sets checked nonempty/disjoint. No full-app/Formula pass claimed.                      |
| Workspace/test compilation                     | Passed `cargo check --locked --workspace --tests`.                                                                                     |
| Strict Clippy                                  | Passed `cargo clippy --locked --workspace --all-targets -- -D warnings`.                                                               |
| Native build                                   | Passed with `CARGO_INCREMENTAL=0`; default incremental MSVC artifacts retain their P12 gate.                                           |
| Protocol generation                            | Passed; generated bindings unchanged.                                                                                                  |
| Formatting / source policy                     | Root/Core actual fmt and touched app fmt passed; **1699 files / 37 oversized**. Docs/whitespace checked before delivery.               |

New pure-tree/Null cases verify idle ownership, fixed scan/delete turns and pending backpressure,
failed-prefix recovery, fresh sweeps after rewiring, correction between admission/application,
cleanup with no driver, coarse capacity retry without snapshots, authored UUID/first-duplicate order,
two ordered duplicate disconnects, retained-byte/queue rejection, selection cancellation, worker exit
and recovery, and UI disconnect/connect/repeated-connect through actual worker wakes. Existing
4096-route reset and native lifetime/command suites remain preserved. No new end-to-end latency or
physical/cross-platform claim follows from these bounds.

Artifacts:

- App tests: `target/debug/deps/Chataigne2-b14ffd1b9e52f9ac.exe`, SHA-256
  `0a647bcd9521f5601e08bf1708923d8e03eaa5b3809034f50763e590081521b7`.
- Native app: `target/debug/Chataigne2.exe`, SHA-256
  `1f92694f45c08511948ae8a3f76f39062b122cea457004a52385558ad58271ca`, built with
  `CARGO_INCREMENTAL=0`. All owned jobs are terminal; new endpoint scenarios use explicit Null only.

Remaining C01: bounded gain/level/migration structure and publication, feedback and remaining native
failure/reconfiguration fences. Keep those acceptance items open; general snapshot/DTO costs stay
P16 and audio ordering stays P04. Core's 588 passes / three existing ignored keep their prior date.
Formula/advisory/default-incremental/UI/real-device/platform/product-scale gates retain their owners.
Counts remain **10 open / two closed**, Sound Card **50% partial**, P03 **~81%**, full plan **~16%**.
Commit/push on `main`; keep the full P00-P18 goal active.

## C01 bounded derived structure and migration

Implemented on 2026-10-21?22 from `e0fa4b09`, Windows/MSVC Rust 1.97.0, default workspace
features. Source began clean on `main`. No Golden/protocol/dependency/schema/app-shell source
changed. C01 remains OPEN for feedback and remaining native failure/reconfiguration fences.

The app-owned `structure` boundary now has a typed staged service instead of complete child-list
clones/maps and unbounded channel/migration edit loops. Each turn performs at most **32 steps and
eight writes**, behind one fence with at most **32 node reads** and one module lifetime guard.
One graph receipt backpressures publication. Four UUID membership sets grow incrementally to at
most 256 identities each; canonical snapshot storage retains its independent shared quota. Naming
copies admit 64 KiB. Snapshot/error retry waits 250 ms without repeated snapshot requests. No
worker thread or periodic actor update is added; existing passive wakes resume work.

A failed prefix retries fresh committed state. Container deletion stops the turn before later
stages can address its descendants. A legacy gain/level becomes a typed parameter with the same
UUID and copied value before its old children retire in bounded turns; a failed suffix cannot
erase that value. A legacy deterministic spectral bool is replaced in place. Other legacy enabled
toggles merge enabled intent into the typed folder before removal, fencing both owners. This
normalization preserves enabled legacy intent while removing duplicate declarations. Nodes moved
out of a cleanup scope survive; reserved identities owned by another folder fail explicitly.
Authored topology/meta callbacks and owned writes request a final fresh pass. Meter/value-only
writes cannot keep a long scan invalidated. Disable, destroy and restoration invalidate pending
writes; driver None can finish cleanup and settle configuration without a native runtime.

Final-source evidence:

| Check                                      | Result                                                                                                               |
| ------------------------------------------ | -------------------------------------------------------------------------------------------------------------------- |
| Sound Card/command selection, four threads | **190 passed**; 179 preserved and 11 new.                                                                            |
| App preservation selection                 | **682 passed / five existing ignored**; 687 selected, disjoint from Sound Card.                                      |
| Current app evidence                       | **872 distinct passes / five ignored / 1240 listed**. This is scoped evidence, not a full-app/Formula qualification. |
| Workspace/tests compilation                | Passed `cargo check --locked --workspace --tests`.                                                                   |
| Strict workspace Clippy                    | Passed `cargo clippy --locked --workspace --all-targets -- -D warnings`.                                             |
| Native app                                 | Passed with `CARGO_INCREMENTAL=0`; default incremental MSVC artifacts retain their P12 gate.                         |
| Protocol generation                        | Passed; generated bindings unchanged.                                                                                |
| Formatting / source policy                 | Actual root/Core fmt and touched app fmt passed; 1703 files / 37 oversized. Docs/whitespace checked before delivery. |

The 11 new cases cover idle ownership, growth/receipt backpressure and failed prefixes, value-first
legacy migration with partial child cleanup, selection changes between admission/application,
lifetime cancellation, metadata-byte backoff, 4096 stale gains plus a late addition, moved cleanup
targets, spectral bool/duplicate recovery, a 512-folder scan under continuous value writes, and
256-channel completion through actual passive Null wakes. They use pure authored fixtures and the
explicit Null backend, not physical audio endpoints. Fixed turn bounds do not establish a P16
end-to-end latency or platform/device qualification.

Preservation selection uses shared module/common/script tests, module commands/manager, Signals,
Metronomes, HTTP, all six stream adapters, MIDI/OSC, the two Formula snapshot-cache and two value-change
cases, managed nodes, processor snapshot gates, state-machine command dispatch/snapshot gates and
nine generic Alchemist command cases. The five ignored `module::tests::authored_scale` scenarios
keep their opt-in scale owner; they were not silently counted as passes. Selector sets were checked
nonempty and disjoint. Historical counts used different selected scopes and are not added here.

App test artifact: `target/debug/deps/Chataigne2-b14ffd1b9e52f9ac.exe`, SHA-256
`58dd3c1c8009f529ef90149045b9075e63522abfdfa57c1680c4c309f8cf362c`.
Native artifact: `target/debug/Chataigne2.exe`, SHA-256
`cbfa072abed888e8221b5aae49c89982130ffec1e130d060b8dcbe374e0b5518`, built with
`CARGO_INCREMENTAL=0`. All owned processes are terminal. Core's 588 passes / three existing ignored
retain their earlier date because Core is unchanged. Default incremental MSVC artifacts remain P12.

Sound Card advances from its coarse **50%** to **75% partial**: derived structure/migration now meets
local acceptance; feedback/native failure/reconfiguration closure retains the remaining quarter.
P03 becomes **~85%**, full plan stays **~16%**, and packages stay **10 open / two closed**. Commit and
push this verified milestone on `main`, then continue the remaining C01 criteria. The full P00-P18
goal remains active.

## C01 bounded native feedback selection matching

Implemented on 2026-10-22 from clean `3ddee8cd` on `main`, Windows/MSVC Rust 1.97.0, default
workspace features. The initial actor-side selection check parsed JSON and copied unbounded
control values. Matching now runs on the existing app-owned native event service, renamed from
`native_logging` to `native_feedback`. One worker/lifetime quota covers matching and log appends;
its existing ten logging tests move with their owner. No Golden/protocol/schema/dependency or
app-shell source changed.

Capture checks borrowed lengths before allocating: driver **128 bytes**, selected device text
**256 KiB**. A separate retained input pool admits **32 sources / 16 MiB** and reserves before both
copies. Cache and queued owners hold the same charge until final native disposal. Jobs share
**32 live tickets / 32 queued handles / one live-or-retiring worker**; event data keeps its existing
canonical lease. Parsing/matching has no actor-side serde work. Cached results compare the current
preprocessed driver/device values and bound parameter before use; replacement controls cannot
inherit an older result. None/removal bypasses pending matching. Native waits do not self-wake or
request a tree snapshot. Capacity retries use 250 ms and preserve a consumed earlier deadline.

A failed inspection publishes a separate warning through its own edit receipt before forwarding
the unchanged status callback. A failed warning suffix retries without duplicating that occurrence.
Log policy remains frozen only at admission, and uncertain log prefixes are never replayed.
Malformed in-budget JSON is an unmatched selection, preserving existing behavior. Driver startup
rejects oversized text before copying/formatting; sample-rate parsing rejects text beyond 32 bytes.
The app still uses the same typed controls, commands, script callbacks and generated UI protocol.

Final-source verification:

| Check                                                       | Result                                                                                                                             |
| ----------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------- |
| Sound Card/command selection, four threads                  | **200 passed**: 190 preserved plus ten new.                                                                                        |
| Same disjoint preservation scope as the preceding milestone | **682 passed / five existing ignored**, 687 selected.                                                                              |
| Current app evidence                                        | **882 distinct passes / five selected ignored / 1250 listed**. Nonempty/disjoint selector sets checked; no full-app/Formula claim. |
| Workspace/test compilation                                  | Passed `cargo check --locked --workspace --tests`.                                                                                 |
| Strict workspace Clippy                                     | Passed `cargo clippy --locked --workspace --all-targets -- -D warnings`.                                                           |
| Native build                                                | Passed with `CARGO_INCREMENTAL=0`; default incremental MSVC artifact gate remains P12.                                             |
| Protocol generation                                         | Passed; generated bindings unchanged.                                                                                              |
| Formatting / policy                                         | Actual root/Core fmt, touched app fmt, doc/whitespace checks pass; **1706 files / 37 oversized**.                                  |

Ten new cases cover idle ownership; slow native inspection with zero busy wakes; cached-result
reuse; changed/removed/replacement controls; byte rejection before pool initialization/admission;
32 cached-source quota exhaustion and release; cancelled queued work and later recovery; coarse
job-admission retry after an earlier wake; encoded/max-size malformed selectors and caller spare
capacity; rejected-feedback warning failure with original callback preservation; and bounded startup
controls. Tests use pure prepared fixtures, controlled native gates and explicit Null audio, not
physical endpoints. The first two added integration fixtures bypassed normal parameter-handle
preprocessing; they now assert the authored values and preprocess the handles before direct service
calls. The final suite passes all 200 cases. Five authored-scale cases keep their existing opt-in
owner; they are not counted as passes. Core is unchanged and keeps its prior 588 passes / three ignored.

Artifacts:

- App tests: `target/debug/deps/Chataigne2-b14ffd1b9e52f9ac.exe`, SHA-256
  `d38fcff5c601d39de7f87f8cbc43c829d274caf3d186f299ca6cd631135a5163`.
- Native app: `target/debug/Chataigne2.exe`, SHA-256
  `32a0895351e1a22f050df95adabaa997f461e847782ca1e197ad6d6db434900a`, built with
  `CARGO_INCREMENTAL=0`. All owned checks are terminal.

C01 remains OPEN. Its closing audit must exercise slow configuration preparation under continuous
value-only writes and confirm native completion/application/failure fences. `Preparation::Key::read`
currently includes `values`; that source observation is a candidate to reproduce before choosing a
repair. Do not substitute broad snapshot/DTO optimization for an owning P03 violation. General
snapshot scaling stays P16, audio ordering P04, and other historical qualification gates retain their
owners. Sound Card remains **75% partial**, P03 **~85%**, whole plan **~16%**, packages **10 open / two
closed**. Commit/push this verified milestone on `main`; keep the full P00-P18 goal active.

## C01 configuration source lifetime through value-only updates

Implemented from clean `3be60310` on 2026-10-22, Windows/MSVC Rust 1.97.0, default workspace
features. The source-reviewed candidate became a reproduced failure: a real retained configuration
job held before execution was cancelled by the first of eight Values writes. The unmodified key
watched the complete Values authored subtree. Dropping that dependency wholesale would have hidden
binding-layout changes. The repair distinguishes those responsibilities at the public read boundary.

Golden's new opaque `SubtreeStructureRevision` identifies topology/order, node identity/kind,
label/short-name aliases and enablement for a specific root/lifetime. Scalar value, control,
constraint and presentation-only edits do not advance it; opaque writes are conservative. Ancestor
enablement remains a separate root read. Both structure and full authored counters live in one
per-node entry, fold together iteratively and retain their original canonical snapshot lease.
Allocation weights include the complete entry. There is no second revision allocation, ancestor
fan-out, per-binding actor scan, new protocol field or app policy in Golden.

The Sound Card configuration key keeps authored connection/parameter scopes, adds their root ids,
and uses the structure token for Values. Readiness remains constant-time indexed scope lookup.
A queued job now survives unchanged bindings under continuous values, completes that original job
and submits its result. Aliases, same-UUID replacements, removal and formerly missing binding roots
still invalidate before callbacks. Full authored gain/selector and native-device fences remain in
place. The Core revision tests are now grouped under `tests/tree_revisions/mod.rs` with their
focused structure suite, following the maintained folder layout.

| Verification on final source                      | Result                                                                                                                             |
| ------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------- |
| Owning Sound Card/command selection, four threads | **203 passed**: 200 preserved and three new source-lifetime cases.                                                                 |
| Same disjoint app preservation scope              | **682 passed / five existing ignored**, 687 selected.                                                                              |
| Current app evidence                              | **885 distinct passes / five selected ignored / 1253 listed**; selector sets checked nonempty/disjoint. No full-app/Formula claim. |
| Full Golden engine library suite                  | **593 passed / three existing ignored, 596 listed**; five new structure cases, plus extended existing 20k-depth coverage.          |
| Workspace/test compilation                        | Passed `cargo check --locked --workspace --tests`.                                                                                 |
| Strict workspace Clippy                           | Passed `cargo clippy --locked --workspace --all-targets -- -D warnings`.                                                           |
| Native build                                      | Passed with `CARGO_INCREMENTAL=0`; default incremental MSVC artifacts retain their P12 gate.                                       |
| Protocol generation                               | Passed; generated bindings unchanged.                                                                                              |
| Formatting / source policy                        | Actual root/Core fmt and touched app fmt passed; 1708 files / 37 oversized. Docs/whitespace checked before delivery.               |

Core cases distinguish value/constraint/warning changes from aliases, enablement and same-UUID
replacement; cover move/remove/history, disjoint and retained views, root-specific tokens with an
identical folded sequence, synthetic overlay lifetimes, and value-versus-alias graph transactions.
The 20k-deep fixture verifies a value-only stamp then a structural stamp without recursion or
rewriting an older view. App cases keep the same queued cancellation state through eight writes,
finish its real job, preserve warnings-only readiness, and reject alias/replacement/missing/new
binding roots. They use pure tree fixtures and explicit Null audio. This is not a device/platform
or end-to-end P16 latency qualification. The initial short `--exact` selector ran zero cases and
was not counted; the correctly selected regression failed before repair, then the full suite passed.

App test artifact: `target/debug/deps/Chataigne2-b14ffd1b9e52f9ac.exe`, SHA-256
`97c63f9acbb8fa9c2158983a370c44ff98f971156e61f58a1574a5fac13f0e9e`.
Core test artifact: `target/debug/deps/golden_engine-acb0109b8761871d.exe`, SHA-256
`5cdf5a63744d87fb953ca28f3b3ee44d6313e9f64b649bcc226fdd0c8f3092c0`.
Native artifact: `target/debug/Chataigne2.exe`, SHA-256
`68b73d8b4e1061d3ffc51c2f2b4298230cab0dc0e340b3080f1e39719c08e54d`, built with
`CARGO_INCREMENTAL=0`. All owned checks are terminal. The final Core run follows test-folder
normalization. Historical Core/app figures are retained by date and are not accumulated.

C01 is still OPEN: qualify native constraint publication (including the distinct-domain equality
slice fallback) and post-admission application/failure ownership before closure. The fallback is
source-reviewed, not a measured violation in this milestone. General DTO/snapshot projection stays
P16 and audio control ordering stays P04. No protocol, dependency, persistence schema or app-shell
source changed. Sound Card **75% partial**, P03 **~85%**, full plan **~16%**, packages **10 open / two
closed**. Commit/push on `main` after all checks are terminal; keep the full P00-P18 goal active.

## C01 closed: guarded value projection and capped constraint publication

Completed 2026-10-22 after `1178cd05`. The corrected real-app regression failed before the repair:
a queued playback-status publication changed a node from `0` to `7` after the node moved outside
its owning module. The first attempted fixture lacked materialized meter nodes and failed before
exercising publication; that fixture failure is not counted as the reproduced defect.

Core now offers structural node reads through the existing bounded edit preconditions. A typed
read-mode enum keeps authored, identity-only and structural semantics distinct. Checks use existing
local counters; there is no new index, ancestor propagation, serialization field or app policy.
Sound Card prepares every target-to-module path on its configuration worker (131072 targets,
32 nodes per path, 32 MiB total read storage), retains it with the existing 64 MiB configuration
admission and copies only the bounded path for a scalar publication. Fences and writes both count
against the existing 64-item actor turn. Cross-module or overdeep bindings fail native preparation.
A publication clock fences values at configuration submission, matching acknowledgement, runtime
stop and final publisher disposal. Receipts keep old generations out of a new cache; rejected
prefixes retry once in isolation and signal fresh coarse preparation without an actor poll loop.
Tests cover moved targets/ancestors, aliases, generation reset/retirement, real native submit/stop,
path admission and valid suffix recovery. Matching native application/rejection and source-lifetime
suites remain green. One old assertion counted queue entries; it now counts value writes, allowing
for the required additional precondition entry.

The distinct-domain equality candidate does **not** require a fingerprint, interner or broader
Core refactor. Worker planning omits equal domains; Core retains exact comparison during application
and event emission. Sound Card caps each domain at 32768 options / 32 MiB, and a plan at four patches /
64 MiB. The explicit fixture passes real 8 MiB metadata intake and publishes two full 32768-entry
native domains with late catalog changes. On Windows x86_64 MSVC, Intel Core Ultra 9 275HX (24 logical
processors), Rust 1.97.0, optimized test profile, final complete Core application samples in ns were:
`[11449400, 11551600, 12252400, 11716200, 11758900, 12624400, 12711800, 11883800]`
(**11.45-12.71 ms**). Preparation is outside this interval. These are local change-driven publication
samples, not a universal worst-case actor deadline, whole-process RSS, UI or device qualification.
The 32768-entry foundation comparison fixture also records its raw samples on explicit invocation.

Verified on the final app artifact:

- `cargo test --locked -p Chataigne2 --bin Chataigne2 app::module_modules_audio_sound_card -- --test-threads=4 --quiet`:
  **208 passed / one explicit measurement ignored by default**. Running
  `capped_native_catalog_publication_samples --ignored --nocapture --test-threads=1` passes separately.
- The preserved disjoint module/command/manager, Signals/Metronomes, HTTP/stream, MIDI/OSC,
  Formula snapshot/value-change/managed-node/generic-command and state-machine snapshot/command
  scopes pass **682 / five existing ignored**. Selection lists verify no overlap with the 209 owning
  cases. Combined: **891 distinct app passes / five existing ignored / 1259 listed**; full app and
  the known Formula failures remain outside this claim.
- `cargo test --locked -p golden_engine --lib -- --test-threads=4 --quiet`: **595 passed / three
  existing ignored / 598 listed**. Parameters: **23 default passes**, and its explicit comparison
  fixture passes. Strict workspace Clippy, workspace/test check, native build with
  `CARGO_INCREMENTAL=0`, protocol codegen, root/Core/app formatting, docs formatting and whitespace
  checks pass; generated bindings are unchanged. Source scan: **1713 files / 37 oversized**.

Artifacts (SHA-256):

- App tests `target/debug/deps/Chataigne2-b14ffd1b9e52f9ac.exe`:
  `881f1b7299df4b215867714cfdf3783a963757af5ab27a535d7757886eca3dd3`.
- Core tests `target/debug/deps/golden_engine-acb0109b8761871d.exe`:
  `f5e9da6abab883f0b60d1d3c64b11d2dcf5b23c65bbcd6832ba85ed8d929e75a`.
- Native app `target/debug/Chataigne2.exe`: `26d90f13f22deaaeae3f25f29122d4e340c899f8d673a381a8f34fe076ca51ca`.

**C01 closes locally. C01-C03 are closed; nine packages remain open (C04-C12), covering thirteen
module families.** Sound Card becomes 100%; P03 becomes ~88% scope coverage and the whole plan stays
~16%. Hardware/platform qualification remains separate; P04 ordering and P16 generic scaling retain
their owners. All owned jobs are terminal. Commit/push this verified milestone on `main`, then start
C04 MQTT's event/byte/service/retirement and accepted-publish ownership work. Keep P00-P18 active.

## C04 MQTT native ownership, publication outcomes and passive service

Implemented 2026-10-22 after `a08ddec5`. Source confirmed the old unbounded event channel,
unlimited actor/native drains and blocking stop-send/join. The owning app module now uses Golden IO
admission and retirement primitives while retaining rumqttc as the protocol implementation.
The transport is a conventional focused folder; payload conversion and passive service are separate
app modules. MQTT's entrypoint is below 1000 lines; the source-size inventory is reconciled.

Native bounds and ownership:

- Sixteen global live/retiring worker slots, retained through native teardown; cancellation is out of
  band. A blocked destructor or DNS lookup cannot authorize replacement thread growth.
- Command mailbox: 100 / 16 MiB, eight native commands per turn. Event mailbox: 128 / 4 MiB.
  Separate global input/output lifetime pools each admit 512 / 64 MiB. Output charges retain source
  data and its library copy through protocol disposition or teardown; input charges survive drain.
- The existing 1 MiB packet ceiling now includes publish headers and retained capacities; topic and
  description caps are 4096/256 bytes. Configuration caps include 128 subscriptions, 1024 filter bytes,
  4096 bytes per credential field, capped keepalive and retained storage. Native diagnostics retain
  at most 1024 bytes. These are admission bounds, not whole-process RSS or universal deadlines.
- Publish receipts distinguish pending, dispatched/unconfirmed QoS 0, acknowledged QoS 1/2, and
  failed/unconfirmed work. Protocol retransmission reuses the same owner; stopped/failed publications
  are never replayed by the app. PUBACK/PUBCOMP and buffered transitions from failed flushes settle
  the correct owners. A new broker session resets stale collision/event identities using the public
  MqttState API; resumed sessions preserve their state. Teardown disposes library payloads first.
- Native DNS resolves serially, rotates up to sixteen addresses and checks cancellation before socket
  progress. The app-owned current-thread runtime retains one owned EventLoop poll across short waits.
  This avoids repeatedly cancelling connect/flush futures or accumulating runtime DNS jobs. It uses
  the already-present public rumqttc async API and Tokio dependency; no dependency versions changed.
- Network recovery and actor startup-capacity retry use capped 250 ms-8 s backoff. Native connection
  attempts retain the library's one-second timeout. The actor is passive; early retry wakes rearm
  their remaining deadline. Events/receipts service 16 entries each; pending input/publication queues
  cap at 64/100. One-message actor turns are an interim bound, not complete payload/row qualification.

Verification on Windows/default features:

- `cargo test --locked -p Chataigne2 --bin Chataigne2 module_modules_protocol_mqtt -- --test-threads=4 --quiet`:
  **22 passed**, including command/script/template preservation, real loopback PUBACK, QoS 2,
  session loss/resumption, stale state reset, full queues, retained input, blocked teardown, panic,
  blocked DNS/no late connect, owned-poll continuation/drop, and passive/early retry snapshot gates.
  The first wire fixture inherited Windows nonblocking mode and was corrected; that fixture failure
  is not claimed as a production failure. An EngineTime fixture used the engine's actual time.
  Strict Clippy caught an unused size helper; production now shares its pre-copy admission calculation.
- Preservation: **890 passed / six selected ignored / 896 selected**, disjoint from the 22 MQTT cases;
  full artifact lists **1274**. This is **912 distinct passes**, not a full-app pass. The six ignores
  are five existing authored-scale cases and the Sound Card catalog measurement already qualified
  at C01. Preserved scopes are the prior module/command/manager, Signals/Metronomes, HTTP/streams,
  MIDI/OSC and targeted Formula/state-machine scopes, plus the full default Sound Card owning suite.
  Golden Core source and the existing Formula/advisory/platform qualification owners are unchanged.
- Strict workspace/all-target Clippy, workspace/test check, native app build with `CARGO_INCREMENTAL=0`,
  protocol codegen and root/Core/app/docs formatting/whitespace checks pass. Generated bindings are
  unchanged. Source policy: **1725 files / 36 oversized**; no new size exception or P12 closure claim.

Final artifact SHA-256:

- `target/debug/deps/Chataigne2-b14ffd1b9e52f9ac.exe`:
  `9f6854d9ae46a49488d01bb7a9b6dd95fcf68278bfc0103cf9b4b843d1a58743`.
- `target/debug/Chataigne2.exe`:
  `f3b843aab65a32d49329ce3d955e23d55bce765eef140a20313fcd039b6ec286`.

**C04 remains OPEN** for native configuration/payload/command preparation, bounded receipt-driven
row and callback publication, source/failure lifetimes, pressure feedback and full actor acceptance.
Keep the fixed count at **nine open / three closed**, P03 ~88% scope coverage and the whole plan ~16%.
No physical endpoint or external broker qualification is claimed; new endpoint tests use a local
fixture listener. All owned checks are terminal. Commit/push this native/service milestone on `main`
before continuing C04's actor boundary; retain the full P00-P18 goal.

## C04 MQTT bounded configuration capture

Implemented 2026-10-22 after `cee7643a` on `main`, starting with three unfinished, untracked
configuration sources. The app-owned `mqtt/configuration/` now owns capture, preparation and
application. The actor captures a fixed revision key and canonical snapshot lease. One bounded
worker validates borrowed fields before copying, walks subscriptions iteratively in authored order,
compares configurations and admits their retained storage. Limits: 32 queued/running/ready jobs,
8192 scanned subscription nodes and 128 enabled subscriptions; existing field/native limits remain.
Cancellation retains admission until queued/running work releases it. An abandoned job produces
an explicit retryable completion; ready results keep one admitted lease across native-slot retries.
Unchanged capture reuses its prior lease. No actor configuration-string copy, recursive scan or deep comparison
remains in this configuration path. No Core API, protocol DTO, dependency or app-shell change.

The configuration key excludes runtime Connected and Values traffic and includes endpoint fields,
credentials, subscription topology/order/enablement, UUID and engine lifetime. Inbox handling notices
structural edits, and application checks the current snapshot before accepting a result. Waiting
preparation is snapshot-free; failure uses the existing coarse retry and authored edits cancel stale
work. Fresh empty managers create their default `#`; loaded empty managers preserve authored intent
without a recursive initialization scan. No persisted format changes. See the maintained
[ownership contract](../architecture/mqtt-runtime.md).

Windows/default features, Rust 1.97.0:

- MQTT command/module/transport/configuration filter: **37 passed**, including 15 new cases covering
  pre-copy field limits, credentials, subscription count/order/disablement, bounded scan frontier,
  cancellation/admission recovery, abandoned work, shared ready/unchanged leases, passive completion,
  retry, topology invalidation before callbacks and loaded-empty meaning. The initial move fixture
  used an invalid destination; it was corrected to an accepting subscription manager.
- Disjoint preservation scopes: **890 passed / six existing ignored / 896 selected**. Together:
  **927 distinct passes / six selected ignored / 1289 listed**, with 356 tests outside these scopes.
  No full-app/Formula or physical-device claim. Core is untouched; its prior C01 595-pass result
  remains baseline evidence rather than an additional run.
- Strict workspace/all-target Clippy, workspace/test check, protocol codegen, root/Core Cargo fmt
  and explicit MQTT Rustfmt, native build with `CARGO_INCREMENTAL=0`, docs formatting and whitespace
  checks pass. Generated bindings are unchanged. All owned jobs are terminal. Source inventory: **1733 files / 36 oversized**; MQTT root 650 lines.
- Final test artifact `target/debug/deps/Chataigne2-b14ffd1b9e52f9ac.exe` SHA-256:
  `30bd73bc4172659179c602a74914801712aff748d4c03079252d2b33f2c25d90`.
- Final native artifact `target/debug/Chataigne2.exe` SHA-256:
  `2fe03115979a014ee6b307685e6d8eb73350ab80e365fb859cee003b37977626`.

**C04 stays open; nine packages remain and three are closed.** Next move incoming JSON/mode/log/
callback and outgoing command/script preparation behind bounded IO ownership, then qualify
receipt-driven graph rows, source lifetimes without prefix replay, visible pressure and actor load.
P03 stays ~88% scope coverage, whole plan ~16%; P04-P18 retain their owners. Commit/push this
verified configuration milestone on `main`; keep the full P00-P18 goal active.

## C04 MQTT native incoming preparation and retained graph delivery

Implemented 2026-10-22 after `57400e38`, starting clean on `main`. The app-owned incoming path
now validates/prepares once on a bounded worker, retains raw and derived storage through delivery,
and advances a receipt-driven graph cursor instead of reparsing each retry. Repeated triggers before
missing fields and an applied prefix before failure are not replayed. The original raw callback
publishes once after value completion/failure; a retired or changed source cancels its suffix.
Processing revisions detect changes back to the same value, preventing stale callback retry loops.

Core exposes the generic `EditQueue::retain_payload` boundary because a removed producer otherwise
released its source while copied edits were queued. It retains only a public admitted IO lease;
ordinary completion observers do not extend that lifetime. No MQTT types/policy enter Core. The
unused incremental helper is removed; shared cursor/batch planning remains. See the maintained
[MQTT contract](../architecture/mqtt-runtime.md) and
[queue lifetime contract](../architecture/application-contracts.md).

Verification on Windows/default features, Rust 1.97.0:

- MQTT filter: **50 passed**, including 13 new receive-path tests covering limits, callback shape,
  a 700-value bounded tuple, trigger occurrence preservation, partial failure, source retirement,
  processing changes/change-back, callback receipts, queued source retention, draft identity under
  pressure, admission recovery and abandoned/cancelled preparation. An intermediate 49-test run
  passed before the final revision, copy-allowance and full-queue continuation repairs.
- Disjoint preservation scopes: **890 passed / six existing ignores / 896 selected**. Final artifact
  lists **1302** cases: **940 distinct passes / six selected ignores**, 356 outside these scopes.
  Module/command/manager, Sound Card, Signals/Metronomes, MIDI/OSC, HTTP/streams and the targeted
  Formula/state-machine scopes are preserved. No full-app/Formula, device or product-scale claim.
- Strict workspace/all-target Clippy passes. Core's first full run passed 596 cases and failed one
  new atomic fixture: it queued work before the transaction, whose checkpoint intentionally parks
  prior edits. The corrected fixture admits its request inside the transaction. The final
  `cargo test --locked -p golden_engine --lib -- --test-threads=4 --quiet` run passes
  **597 tests / three existing ignores / 600 listed**. Workspace/test checking and codegen pass;
  generated bindings are unchanged. Native build with `CARGO_INCREMENTAL=0`, root/Core Cargo fmt,
  explicit app Rustfmt and docs formatting/whitespace checks pass. All owned jobs are terminal.
- Source inventory: **1742 files / 36 oversized**; MQTT root 543 lines. Application input/JSON/unit/
  retained-byte limits and source/callback policy are documented; they are not whole-process RSS
  measurements or full actor-load qualification.

Final artifact SHA-256:

- App `target/debug/deps/Chataigne2-b14ffd1b9e52f9ac.exe`:
  `305f834845bde9e5adfd9be14035691bc0bbb7c84b251291788aeb81ef33942d`.
- Core `target/debug/deps/golden_engine-acb0109b8761871d.exe`:
  `2127af6bd74a9ebc804616b8cd588b9424439b75c0e1584048e3f6bee0801c08`.
- Native `target/debug/Chataigne2.exe`:
  `698b16e0bc507651aa2dbc86c607d4cbad1579648846bfd299c3cd0e2cb5eca4`.

C04 remains open for outgoing capture/preparation, deeper target-path/source fences, and complete
pressure/passive-receipt/failure/actor acceptance. Counts remain **nine open / three closed**, P03
~88% scope coverage, full plan ~16%. Commit/push this verified milestone on `main` and keep the full goal active.

## C04 MQTT queued target-path fences

Implemented 2026-10-22 after `3a38df47`, starting clean on `main`. App-owned `mqtt/input/targets.rs`
captures existing value/replacement/removal targets and insertion parents, then walks their ancestor
paths to Values. Shared ancestors are visited once. Work is bounded by the cursor's 256 work units
plus the native path allowance: at most 282 target reads, with public groups of at most 32 guards.
All source/target groups precede every mutation in the prefix. This preserves intended parent
replacement/child insertion while rejecting stale paths before any receive-prefix write applies.
A rejected graph suffix is not replayed; its original raw receive callback still has its own receipt.

Preparation reserves worst-case queued guard/request/receipt storage alongside the existing value
copy allowance. A byte-full pool retains the same draft for later admission; input and queued edits
keep its reservation after producer removal. No new Core API, graph revision system, whole-tree scan,
protocol DTO, dependency or app-shell change. The Core 597-pass/three-ignore run at `3a38df47` remains
baseline evidence, not another run or an addition to app counts.

Verification on Windows/default features, Rust 1.97.0:

- An initial 54-case run passed. The strengthened 55-case run passed with typed precondition-error
  assertions and applied-move checks; wide prefixes now exercise parent replacement plus child
  creation, and a stale final read group prevents all prefix mutations. The 56-case run adds queued
  guard byte admission/draft recovery and passed before a numerically identical lint cleanup.
- Final owning filter: **56 passed**. Disjoint preservation scopes: **890 passed / six existing
  ignored / 896 selected**. Final artifact: **946 distinct passes / six selected ignored / 1308
  listed**, with 356 outside these scopes; no full-app/Formula or product-scale claim.
- Strict workspace/all-target Clippy, workspace/test check, protocol codegen, native build with
  `CARGO_INCREMENTAL=0`, root/Core Cargo fmt, explicit MQTT Rustfmt and docs formatting/whitespace
  checks pass. Generated bindings are unchanged. The lint cleanup expresses eight metadata slots as
  an array size rather than a multiplication that Clippy interpreted as a bit-count calculation.
- Source inventory: **1744 files / 36 oversized**; MQTT entrypoint stays 543 lines. Architecture and
  ownership docs are updated in the same change.

Final artifact SHA-256:

- App `target/debug/deps/Chataigne2-b14ffd1b9e52f9ac.exe`:
  `d27c49dc83d83e93d6710a9effaae4b864ec25995727801ea6d15f82f08edfd6`.
- Native `target/debug/Chataigne2.exe`:
  `beccffc105cd812923ae95cef8f274085ffdbafb5d16ea4d489c05443823603a`.

C04 stays open for outgoing command/script preparation and complete pressure/passive-receipt,
failure, cross-turn churn and actor/load qualification. Counts remain **nine open / three closed**,
P03 ~88% scope coverage and whole plan ~16%. All owned jobs are terminal. Commit/push this verified
milestone on `main`; retain the full P00-P18 goal.

## C04 MQTT outgoing source capture and native preparation

Implemented 2026-10-22 after `b0b5a167`, starting clean on `main`. App-owned `mqtt/output/`
captures canonical command snapshots/execute events and Core script arguments. The existing native
worker owns escape/override conversion, bounded JSON encoding, wire validation and one prepared packet
across backpressure. Output/copy/scratch admission precedes queueing; original inputs keep their
canonical leases. Invalid preparation or producer retirement before dispatch reports **not sent**,
without an empty substitute or automatic replay. Command admission failure preserves unrelated edits.
The old actor JSON relay and outgoing script converter are removed. No Core API, protocol DTO,
dependency, persisted format or app-shell startup changes.

Verification on Windows/default features, Rust 1.97.0:

| Check                                                                                                         | Result / boundary                                                                                                                                                                                                                                                                                                                 |
| ------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `cargo test --locked -p Chataigne2 --bin Chataigne2 module_modules_protocol_mqtt -- --test-threads=4 --quiet` | **64 passed**. Eight new cases cover frozen command text/escapes, last override, canonical script arguments, JSON expansion/topic refusal, ACK-held quota/recovery, rejection without dispatch, prepared producer retirement and actual engine script/manual/execute dispatch. Fake brokers and existing ephemeral loopback only. |
| Final test artifact preservation scopes                                                                       | **890 passed / six existing ignored / 896 selected**, disjoint from MQTT. **954 distinct passes / six selected ignores / 1316 listed**; 356 outside these scopes. No full-app/Formula or product-scale acceptance claim.                                                                                                          |
| Strict workspace/all-target Clippy; workspace/test check                                                      | Passed. An initial lint pass found two unnecessary test mutabilities; both were removed before the passing pipeline.                                                                                                                                                                                                              |
| Protocol codegen; `CARGO_INCREMENTAL=0 cargo build --locked -p Chataigne2 --bin Chataigne2`                   | Passed; generated bindings unchanged.                                                                                                                                                                                                                                                                                             |
| Root/Core `cargo fmt --all`, explicit app/MQTT Rustfmt, docs formatting and whitespace                        | Passed. Source inventory **1749 files / 36 oversized**; MQTT entrypoint **519 lines**. Core unchanged; its prior 597-pass/three-ignore/600-listed qualification remains baseline, not a rerun or an addition to app counts.                                                                                                       |

Final artifact SHA-256:

- App `target/debug/deps/Chataigne2-b14ffd1b9e52f9ac.exe`:
  `3a4de2afdd4076c9e6dd1e6213d87f18b50230065314865bf0bb84463b40fc15`.
- Native `target/debug/Chataigne2.exe`:
  `df14f7621a424702f3a10ce028fac5b271238934b44bbd3a2aadd0dca5416677`.

**C04 remains open** for complete pressure/passive-receipt/failure, cross-turn churn and controlled
actor/load qualification. Then continue C05-C12. **3/12 audit packages closed; nine remain**;
P03 ~88% scope coverage and whole plan ~16%, neither a remaining-effort estimate. All owned jobs are
terminal. Commit/push this verified milestone on `main`; retain the full P00-P18 goal.

## C04 MQTT local closure

Qualified 2026-10-22 after `38fab0cc`, starting clean on `main`. Two concrete app-owned runtime
repairs complete the remaining acceptance work: transport warnings now target Connection rather
than a watched configuration scalar, and an independent worker-termination flag precedes native
cleanup. Failure feedback preserves coarse retry deadlines; stopped workers can retire even when
cleanup or event capacity delays channel disconnection. Original native slots and payload quotas
remain occupied through teardown. No Core API, wire DTO, dependency, persisted schema or app-shell change.

| Acceptance                                        | Current evidence                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| ------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Native setup, cancellation, retirement and quotas | `transport/tests/lifecycle.rs` and `transport/worker/tests/`: blocked cleanup/DNS, cancelled late connect, panic outcomes, retained payloads, fixed slots and one owned async poll.                                                                                                                                                                                                                                                                  |
| Input/output item and byte pressure               | `transport/tests/pressure.rs`, lifecycle and outgoing suites: continuous input stops at 128 events; captured 96 KiB outputs hit mailbox bytes before its 100-item ceiling; cancellation and quota recovery preserve ownership.                                                                                                                                                                                                                       |
| Configuration/source capture                      | `configuration/tests/`: worker bounds, stale authored/structural source rejection, ready-result reuse, preparation cancellation and warning/clear-warning preservation of the coarse retry deadline.                                                                                                                                                                                                                                                 |
| Prepared values and callbacks                     | `input/tests/preparation.rs` and `targets.rs`: raw/derived/callback limits, canonical draft reuse, queued-source retention, target/ancestor guards before all mutations and no writes through stale bindings.                                                                                                                                                                                                                                        |
| Prefix/callback occurrence ownership              | `input/tests/delivery.rs`: failed suffixes never replay applied triggers or callbacks; processing churn after a committed 700-value prefix cancels the suffix, then fresh input completes with one new trigger/callback.                                                                                                                                                                                                                             |
| Passive, bounded actor receipt service            | `transport/tests/service.rs`: 40 synthetic broker receipts, out-of-order ACKs, at most 16 terminal removals per service turn, snapshot-free completion and 100 idle ticks without scheduled callbacks, worker callbacks or snapshots.                                                                                                                                                                                                                |
| Failure and reconnect/reconfiguration churn       | `service/recovery.rs` and `outcomes.rs`: stopped-worker recovery before a blocked destructor releases its slot/payload, coarse engine retry to the authored ephemeral endpoint with real rumqttc/PUBACK, eight alternating resumed/lost sessions without replay or worker growth.                                                                                                                                                                    |
| Public module behavior                            | Command tester, methods/templates, native script/manual/execute dispatch, QoS 0/1/2 receipts, input callback shape and graph materialization suites pass.                                                                                                                                                                                                                                                                                            |
| Controlled actor/load acceptance                  | 256 continuous messages preserve all 256 callbacks in order across 1536 turns; unrelated parameter edits progress every turn, app input stays within 64 entries, native events within 128 / 4 MiB, applied edits within 512 per tick and no scheduled MQTT polling. Whole fixture turn p95 171 us / max 359 us on Intel Core Ultra 9 275HX (24 logical processors), Windows x86-64, optimized test profile. This is not product-scale qualification. |

Final owning command:
`cargo test --locked -p Chataigne2 --bin Chataigne2 module_modules_protocol_mqtt -- --test-threads=4 --nocapture`
passes **73**. Disjoint preservation scopes pass **890**, with **six existing ignores / 896 selected**.
Final app artifact: **963 distinct passes / six selected ignores / 1325 listed**, 356 outside these scopes.
Core source is unchanged; its prior 597-pass/three-ignore/600-listed run remains baseline, not another run.

Strict workspace/all-target Clippy, workspace/test check, protocol codegen, native build with
`CARGO_INCREMENTAL=0`, root/Core Cargo fmt, explicit MQTT Rustfmt and docs formatting/whitespace pass.
Generated bindings are unchanged. Source inventory: **1752 files / 36 oversized**; MQTT entrypoint 521 lines.

An initial engine fixture let setup replace its fake transport and fall back to localhost:1883.
The corrected setup drains lifecycle work while disabled and constrains any fallback to a fixture-owned
ephemeral listener. Other intermediate fixture corrections cover inherited nonblocking sockets,
receipt rotation and filtering only `messageReceived` replay events. **The final qualifying run uses
fake providers and ephemeral loopback only.** Injected worker panics are expected test inputs.

Final artifact SHA-256:

- App `target/debug/deps/Chataigne2-b14ffd1b9e52f9ac.exe`:
  `e6e75d4f4eb18d83c69d1dcd7b982cd52d05b0603940c6b037cfc9e8e1cab648`.
- Native `target/debug/Chataigne2.exe`:
  `0a012348e2f5f18504304fad16162167b609001d01da70fcb8e4e5232165fca2`.

**C04 is locally closed. Four of twelve audit packages are closed; eight remain (C05-C12).** Continue
C05 DMX's P03 ownership and qualification. Hardware/platform and product-scale checks retain their
later plan owners. P03 ~90% scope coverage; whole plan ~16%. All owned jobs are terminal.
Commit/push this verified milestone on `main`; retain the full P00-P18 goal.

## C05 DMX native ownership and passive service

Implemented 2026-10-22 after `d0901774`, starting clean on `main`. `dmx/transport/native.rs`
preserves the existing Art-Net/sACN implementations behind a bounded worker. Endpoint open, IO,
termination and destructors run natively; stop cancels out of band and never joins. Sixteen worker
slots include opening and retiring endpoints. Separate status and latest-frame delivery preserve
failure ownership under input replacement pressure. Outgoing admission validates 512-slot retained
capacity and the configured universe; 64 command slots / 1 MiB and separate 512-record / 8 MiB
input/output pools bound queued and consumer-owned work. Every admitted send has an explicit native
completion/cancellation receipt; no failed output is replayed automatically.

Input prepares channel JSON and canonical callbacks on the worker. Nine fixed source/target reads
and the transport lifetime fence queued projection/callback edits. Public Core retained-edit payloads
hold the original input quota until application or rejection. Passive wakes replace 120 Hz actor
polling; startup/recovery uses coalesced deadlines. Disable retires immediately. Internal output uses
Core parameter-event attribution, preserving authored changes after same-value updates. Refused
commands leave persisted output state unchanged. Existing commands, script functions/callbacks,
templates, distinct compiled kernel identities and sparse persistence remain wired.

Verification on Windows x86-64, Rust 1.97.0, default features:

| Check                                                                                                        | Outcome / scope                                                                                                                                                                                                                                                                  |
| ------------------------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `cargo test --locked -p Chataigne2 --bin Chataigne2 module_modules_protocol_dmx -- --test-threads=4 --quiet` | **25 passed**. Slow setup/cleanup, late cancellation, full command queues, payload pressure, retained consumption, queued-source rejection, error/frame separation, same-value echo, passive 100-tick idle, native script output and maximum 512-channel request/callback cases. |
| Disjoint preservation filters, including all 73 MQTT cases                                                   | **963 passed / six existing ignored / 969 selected**. Final app artifact **988 distinct passes / six selected ignores / 1337 listed**, with 343 outside these scopes. No full-app/Formula or product-scale claim.                                                                |
| Strict workspace/all-target Clippy; workspace/test check                                                     | Passed. Initial compiler findings were corrected before the passing pipeline; an intermediate 22-pass/one-fail echo run led to explicit event-source attribution.                                                                                                                |
| Protocol codegen; native build with `CARGO_INCREMENTAL=0`                                                    | Passed; generated bindings unchanged. A maximum-frame review corrected command-envelope admission to 64 KiB / 2048 values and excludes non-command topics before validation.                                                                                                     |
| Root/Core Cargo fmt, explicit DMX Rustfmt, docs formatting and whitespace                                    | Passed. Source inventory **1759 files / 36 oversized**; DMX entrypoint **791 lines**. Core is unchanged; prior 597-pass/three-ignore/600-listed evidence remains baseline, not a rerun.                                                                                          |

Final tests use controlled fakes and ephemeral loopback. The pinned sACN crate always terminates via
multicast; loopback tests suppress multicast delivery through its public TTL/loopback options while
still exercising destruction. This does not qualify production termination behavior, which stays P07.
No Core API, dependency, wire DTO, persisted schema or app-shell change.

Final artifact SHA-256:

- App `target/debug/deps/Chataigne2-b14ffd1b9e52f9ac.exe`:
  `ef64540f6552abe9ef5f09e665089cebb14248df3943a78d2194ef7e5a9f99af`.
- Native `target/debug/Chataigne2.exe`:
  `5ac3b9051c7bba100631ce3e202a94e673573932cc0c44aa671dd395f2a51859`.

**C05 remains open** for bounded command/script invocation capture/preparation and full engine
pressure, reconfiguration and failure/recovery qualification. In particular, legacy execution
overlays still copy authored command values. P07 keeps refresh, filtering, sequencing, source-loss,
synchronization and termination correctness. Four of twelve audit packages remain closed; eight
remain open. P03 ~90% scope coverage and whole plan ~16%, without additional closure credit.
All owned jobs are terminal. Commit/push the milestone on `main`; retain the full P00-P18 goal.

## C05 DMX local closure

Qualified 2026-10-22 after `ab7df40f`, starting clean on `main`. App-owned `outgoing/` replaces
whole-snapshot command overlays and the JSON relay with canonical capture, bounded preparation
and typed operations. Native output retains the original source through completion/cancellation;
queued reflection retains it through application/rejection. No Core API, dependency, wire DTO,
persisted schema or app-shell startup change.

| Acceptance                           | Current evidence                                                                                                                                                                                                                                                                                                                                                                                            |
| ------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Native ownership and retirement      | Preserved slow setup, blocked send/cleanup, fixed-slot saturation/recovery, source retirement, retained queued input and Art-Net/sACN ephemeral-loopback gates. Native stop never joins.                                                                                                                                                                                                                    |
| Captured inputs and operation order  | `outgoing/tests/`: shared script argument identity, 1 MiB authored command replaced by bounded last override, 512-slot command, channel-delta burst, authored property occurrences, ordinary `run_tick` command/script order and newer authored-frame projection.                                                                                                                                           |
| Item/byte pressure and actor service | 64-source refusal; full 64-frame native mailbox and exhausted native payload pool retain the same prepared operation, preserve endpoint ownership and resume once without replay. One hundred pressure-idle turns have no worker callbacks or snapshots.                                                                                                                                                    |
| Failure, retirement and recovery     | Native error and panic recover through the actor's retry path while the old cleanup still blocks and owns its slot. Removing a command after native admission yields `NotSent` without stopping the endpoint. Eight universe reconfigurations discard old sources without replay.                                                                                                                           |
| Continuous input                     | One module receiving continuous 512-slot frames: 256 full engine turns, 68 wake turns, at most one module worker callback per turn; p95 157 us / max 203 us. After input stops, 100 turns have no worker callbacks or snapshots. Intel Core Ultra 9 275HX, 24 logical processors, Windows x86-64, Rust 1.97.0 optimized test profile. Small controlled fixture only; product-scale performance remains P16. |
| Public behavior and preservation     | Project-creatable command nodes, method semantics, passive idle, internal output provenance and sparse persistence pass. The disjoint preservation run includes all shared module/command, stream, HTTP, generator, MIDI/OSC and MQTT scopes.                                                                                                                                                               |

Final owning command:
`cargo test --locked -p Chataigne2 --bin Chataigne2 module_modules_protocol_dmx -- --test-threads=4 --nocapture`
passes **36**. Preservation passes **963**, with **six existing ignores / 969 selected**.
The final app artifact lists **1348** cases: **999 distinct selected passes / six selected ignores**,
343 outside these scopes. Core is unchanged; its previous 597-pass/three-ignore/600-listed result
remains dated baseline.

Qualification exposed and repaired a genuine feedback loop: an unadmitted native output's destructor
published cancellation readiness, immediately reawakening a pressured actor. Refusal now disarms
that unpublished receipt. Absolute output retry deadlines also survive unrelated worker wakes.
The ordinary-tick test exposed a source key watching unrelated command-tester structure; it now
tracks only consumed paths. The continuous fixture initially lacked the loopback enum choice;
the final fixture installs a typed catalog before selecting it. Expected native panics are injected.
All final qualification uses controlled adapters and fixture-owned ephemeral loopback.

Strict workspace/all-target Clippy, workspace/test check, codegen, native `CARGO_INCREMENTAL=0`
build, root/Core Cargo fmt, explicit app Rustfmt and docs formatting/whitespace pass. Generated
bindings remain unchanged. Source inventory: **1765 files / 36 oversized**, DMX root **704 lines**.

Final artifact SHA-256:

- App `target/debug/deps/Chataigne2-b14ffd1b9e52f9ac.exe`:
  `c524d5302b3649fdda207e29a57ef415def1bb04e81b8d57ddb0250fefcf79fe`.
- Native `target/debug/Chataigne2.exe`: `8fb6a173d00bd72d77eae40a579523e292374b5304f03ceef02f6e7fa16da092`.

**C05 is locally closed. Five of twelve audit packages are closed; seven remain (C06-C12).**
Continue C06 App Control and OS. P03 ~91% scope coverage and whole plan ~16%, neither an effort
forecast. P07 retains refresh/filter/sequence/source-loss/synchronization/termination correctness;
physical, cross-platform and full-product qualification remain separate. Keep the full goal active.

## C06 OS bounded ownership and command service

Implemented 2026-10-22 after `6deb59fd`, starting clean on `main`. **C06 remains open** for
App Control and the remaining package acceptance. This milestone changes the OS-owned boundary;
it does not add a closure package or increase the plan's completion score.

OS node construction no longer creates a metrics thread or queries native host state. Engine-owned
service starts a bounded provider; native setup, metrics, command parsing, DNS/UDP, process creation,
child reaping and cleanup run off actor. Sysinfo refreshes only the current process. Opening and
retiring providers retain one of sixteen native slots; cancellation and failure retire source fences
and announce termination before blocked destruction. No Core API, dependency, wire DTO, project
schema or app-shell startup change.

Command nodes and scripts use canonical capture and an inbox-only typed ordering boundary.
The worker resolves consumed command fields, preserving the last override without cloning a full
snapshot overlay. Source permits remain charged through internal inboxes, native work, completed
outcomes, queued callbacks and disposal. Limits and ownership live in the
[system contract](../architecture/system-runtime.md).

| Acceptance delivered                  | Current evidence                                                                                                                                                                                                                                                                                                    |
| ------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Native ownership and cancellation     | Blocked setup retains its slot; cancellation prevents late metrics/actions. A blocked fake command fills the 64-entry mailbox; every accepted suffix resolves as not sent. Blocked cleanup cannot join or release the slot early.                                                                                   |
| Command capture and resource pressure | A 1 MiB authored MAC field is replaced by the bounded last override without an overlay. Captured 100 KiB override records hit 4 MiB mailbox bytes before 64 items. Module admission refuses source 65 before native work; all 64 accepted fake actions complete.                                                    |
| Operation and outcome preservation    | All four methods use the fake provider. Ordinary `run_tick` preserves an earlier command before a later script without explicit inbox flushing. Callback admission rejection preserves a known successful native outcome and cannot cause replay.                                                                   |
| Bounded passive actor service         | Sixty-four completed outcomes drain in sixteen-record turns. Snapshot-free wakes retain their result until projection can run. Connected idle has 100 turns with no scheduled callbacks, worker callbacks or snapshot rebuilds.                                                                                     |
| Queued source ownership               | Original metric quota survives queuing; retiring the runtime rejects all value writes before releasing the source. Actor/native fences and fifteen target reads precede value/constraint/callback edits.                                                                                                            |
| Failure and pressure distinctions     | Native metrics failure/panic schedules actor recovery before blocked cleanup releases its slot; a subsequent controlled provider recovers after cleanup. Metrics quota pressure withholds sampling while keeping the provider, then resumes when quota returns. Native command failure also preserves the provider. |
| Public behavior                       | Existing OS values, script/template callback names, command types, MAC formats and 102-byte magic packet remain. Test builds disable native host-control execution; all action tests use controlled adapters. No shutdown, reboot, logout, Wake-on-LAN or desktop operation was performed.                          |

Final owning command:
`cargo test --locked -p Chataigne2 --bin Chataigne2 module_modules_system_os -- --test-threads=4 --nocapture`
passes **23**. Disjoint preservation passes **999**, with **six existing ignores / 1005 selected**.
The app artifact lists **1366** cases: **1022 distinct selected passes / six selected ignores**,
338 outside these scopes. Core source is unchanged; its previous 597-pass/three-ignore/600-listed
result remains baseline.

The initial 16-case run passed. Review then identified a cross-entrypoint ordering gap: a direct
script could reach native execution before an earlier command's inbox dispatch. The typed local
boundary repairs that gap; the final ordinary-tick test qualifies it. Source registration also
leaves declaration-only contexts inert. Callback pressure is an observational rejection, rather
than erasure of a native result. Queued cancellation and interrupted execution have distinct outcomes.
Recovery qualification also exposed an early backoff reset: readiness before the first sample could
prevent escalation after repeated first-sample failures. Reset now follows a healthy sample;
the final test verifies 250/500/1000 ms escalation and reset. Failed disposer initialization stays
retryable and preserves one quota owner after successful startup.

An intermediate preservation run had 998 passes and one MQTT fixture failure. A lost-session
receipt can resolve before ConnAck publishes reconnect readiness; the churn test assumed those
were the same boundary. It now waits for `is_connected()` before the next round. Five focused
runs and the final full preservation pass qualify the barrier. MQTT production code is unchanged.

Strict workspace/all-target Clippy, workspace/test check, protocol codegen, root/Core Cargo fmt
and explicit OS/app Rustfmt pass. Native `CARGO_INCREMENTAL=0` build and docs formatting/whitespace checks pass. Source inventory:
**1773 files / 36 oversized**; OS entrypoint 416 lines, runtime 273, service 356.

Artifact SHA-256:

- App `target/debug/deps/Chataigne2-b14ffd1b9e52f9ac.exe`:
  `051f5b14c3b90b2fc5ad3a555c78f496f5433572cd101cded1e3e28a9a462c8b`.
- Native `target/debug/Chataigne2.exe`: `3abc7e95eb41c87199d815e43d14e05cebc1640807e9de5ae4152303b76af429`.

**Seven P03 packages remain (C06-C12).** Continue C06 App Control: move filesystem traversal,
process/window actions, metrics setup and cleanup behind bounded native ownership; bound watched
configurations, retained results, graph publication and callbacks. Use controlled providers and
preserve commands, scripts and templates. Finish package pressure/failure/recovery qualification
before closing C06. Hardware/platform and whole-product qualification remain separate.

## C06 App Control native ownership and passive publication

Implemented 2026-10-22 after `52c8b35e` on `main`. The previous goal turn was a status-only
response; this turn changes the implementation. **C06 remains open**, with no new closure credit:
five of twelve packages closed, seven remain; P03 ~91%, whole plan ~16%.

App Control construction is inert. A bounded native provider owns process/window actions,
sysinfo discovery, recursive filesystem sampling, process handles and cleanup. Opening and retiring
providers retain their original slot; stop never joins. Watch configurations, folder traversal,
source/outcome storage and mailboxes have explicit item/byte limits. The maintained
[system contract](../architecture/system-runtime.md#app-control) records limits and ownership.

Commands retain canonical snapshots/execute overrides; scripts retain shared arguments. Typed
local inbox attachments order capture before native dispatch. Authored Running events capture
their desired value; observation writes carry module provenance to suppress feedback. Native
outcomes preserve completed effects through retirement and report cancellation separately.

The actor is passive, authors detached value trees, walks at most 64 catalog DFS steps and
publishes at most eight watch rows per service turn. Reports fence watch membership,
configuration/native lifetimes and value targets; queued edits retain the original report.
Folder changes append distinct triggers and use callbacks prepared on the worker. Incoming
traffic and outgoing logging remain wired.

Verification found and repaired catalog jobs invalidating unrelated edit drains and a nested
parameter-replacement delay. Superseded catalog work now skips itself; current work reads the live
selection and changes scalar choices/value in the same mutation callback. Existing immediate
selector, missing-selection, UI creation, stable-folder and script/template cases pass. The
scheduling regression checks observed target values and zero scheduled callbacks rather than
inspecting a dirty flag. Initial empty-watch reports retry payload pressure before becoming idle.

Owning evidence: **26 App Control passes**, comprising 15 preserved behavior cases and 11 new
cases for blocked startup/cleanup, a full 64-command mailbox, cancelled accepted suffixes,
configuration retirement, retained source admission, outcome quota refusal, panic before blocked
cleanup, initial-report quota recovery, and owned temporary-directory delta/baseline/depth/entry limits.
No real application, process/window or desktop-control action was executed; native actions are
explicitly disabled in test builds.

Preservation: **1022 passed / six existing ignored / 1028 selected**, including OS's 23 cases,
DMX's 36 and MQTT's 73. The app artifact lists 1377 tests; owning/preservation selections are
disjoint: **1048 distinct passes / six ignored / 1054 selected**, with 323 outside these scopes.
Core is unchanged; its previously recorded 597 passes / three ignored remain dated evidence,
not a fresh run or part of the app totals.

Strict workspace/all-target Clippy, workspace/test check, protocol generation with unchanged
bindings, actual root/Core formatting, explicit app Rustfmt, and the native `CARGO_INCREMENTAL=0`
build pass. Source inventory: **1782 files / 36 oversized**. App Control entrypoint is 736 lines,
service 426, runtime 363, catalog 85 and watch projection 373; every touched source stays below 1000.
Documentation formatting and whitespace are checked before commit.

Artifact SHA-256:

- App `target/debug/deps/Chataigne2-b14ffd1b9e52f9ac.exe`:
  `0e4193ad57d2a3096f9db8c6294c5b83fe41ba0ee62a0e4681cd94c5d0bec061`.
- Native `target/debug/Chataigne2.exe`:
  `dda585ef377fcda2de931ccd4493f21f0e72a7a5b55cc05baccc05b0ad5a7150`.

Remaining C06 acceptance stays fixed:

1. Exercise ordinary-tick command/script ordering, captured overrides and source removal/disable
   against a blocked controlled provider while unrelated edits continue.
2. Qualify bounded catalog/value publication, queued report/callback ownership and fence rejection,
   retained finished outcomes without snapshots, and pressure without lost accepted occurrences.
3. Exercise repeated reconfiguration and worker failure/recovery, shared native-slot and payload
   pressure, and filesystem failure distinctions at the engine boundary.

Then close C06 only after those criteria pass and continue C07-C12. Hardware/platform and
whole-product scaling retain their later owners. Keep the full implementation goal active.

## C06 closed: system module pressure, publication and recovery

Implemented 2026-10-22 after `e413a8c5`, starting clean on `main`; final reconciliation and
delivery checks continue on 2026-10-23. **C06 is locally closed: six packages closed, six remain
(C07-C12).** This milestone completes the three fixed C06 finish criteria above. It adds no
prerequisite package. P03 advances from ~91% to ~92% scope coverage; the whole plan remains ~16%.

The owning app boundary gains controlled-provider engine qualification. Ordinary ticks preserve
command-before-script order and the last execute override without an overlay, even with a 1 MiB
authored field. Removing a queued command or disabling its module cancels unstarted work;
already started work keeps its known outcome. Unrelated edits progress while fake execution or
cleanup is blocked. All five App Control script methods and authored Running intents use the
same native boundary; observation provenance prevents launch/kill feedback.

Qualification also repairs concrete production failures:

- Params DSL omits inactive controls. Command decoding now reads only the selected launch/target
  branch, and catalog jobs skip inactive watched-app selectors before queuing a mutation against
  a child whose removal may already be queued.
- Only filesystem `NotFound` means missing. Permission/interruption/other metadata failures
  reject the observation, preserving accepted folder baselines and values. Rejected sysinfo
  inventories release their SDK cache on the worker. Borrowed watch paths are checked before
  copying; whitespace-only watches publish their initial observation and settle idle.
- App Control and OS rearm completed receipt backlogs before their disabled early return.
  Sixty-four completed receipts continue draining in sixteen-record turns while disabled;
  completed actions do not disappear when readiness notifications coalesce.

Controlled acceptance covers module source 65 rejection, 4 MiB mailbox byte pressure, bounded
sixteen-result turns and snapshot-free retention, a full native mailbox with absolute 250 ms
retry and no wake loop, eight-row report turns with original quota through final application,
queued native/configuration fence rejection, two accepted folder callback/trigger occurrences,
and a 512-node catalog fixture traversed in 64-step turns. Sixty-five watch targets and a 1 MiB
path are refused without partial configuration; later bounded input recovers. Eight reconfigurations
reuse one native slot. Report quota pressure withholds sampling and later resumes. Sample errors,
command errors and panics preserve their distinct recovery behavior; backoff escalates
250/500/1000 ms and resets after a healthy report. Connected idle has 100 actor turns with zero
scheduled callbacks, worker callbacks or snapshot rebuilds.

The final combined command
`cargo test --locked -p Chataigne2 --bin Chataigne2 -- module_modules_system_app_control module_modules_system_os --test-threads=4`
passes **73** (50 App Control, 23 OS). Preservation passes **1022 / six existing ignored /
1028 selected** on the same artifact, including OS, all 36 DMX and 73 MQTT cases. The app lists
**1401** tests: **1072 distinct selected passes / six ignored / 1078 selected**, with 323 outside
these scopes. OS overlaps the combined run and preservation and is not counted twice. Core is
unchanged; its prior 597-pass/three-ignore result remains dated baseline, not a current run.

The initial new suite exposed inactive-field decoding and catalog mutation failures, which were
repaired. Fixture corrections distinguish normalized File values from Str, wait for notified
provider work before declaring idle, and account for fair rotation of incomplete receipts.
The final 73-case run passes. Subsequent changes to two test predicates only expand formatting;
they do not change expressions or behavior. All action tests use controlled providers; native
host-control actions are disabled in test builds. No real process/window/desktop action occurred.

Strict workspace/all-target Clippy, workspace/test check and protocol generation pass; generated
bindings remain unchanged. Root/Core Cargo fmt and explicit app Rustfmt pass. No Core API,
wire DTO, persistence schema, dependency or app-shell startup change. Source inventory:
**1786 files / 36 oversized**; every touched source is below 1000 lines. The largest new test
file has 456 lines. This is bounded ownership/service qualification, not a 20k/50k project or SDK
transient-allocation/RSS qualification. Hardware/platform and product-scale checks retain their
later plan owners. Continue **C07 Buttplug**, preserving its protocol library and reconnect policy.

Native `CARGO_INCREMENTAL=0` build passes in 2m23s. Final documentation formatting and whitespace
checks pass. Final artifact SHA-256:

- App `target/debug/deps/Chataigne2-b14ffd1b9e52f9ac.exe`:
  `c9ebcd3fe4b7dfc74f6d4125676ec6cc7b476daa60d2c358fbd0cc7667276126`.
- Native `target/debug/Chataigne2.exe`:
  `bfb11d6793e1b20746c2ce7f332aa8e0eb37630bb4685985cb1198b838c42228`.

## C07 Buttplug bounded native ownership and passive service

Implemented 2026-10-23 after `febe5704`, starting clean on `main`, matching `origin/main`.
**C07 remains open; six packages remain (C07-C12).** This milestone replaces unbounded
app-owned channels, joining retirement and 120 Hz actor polling with bounded public IO and
retained command receipts. P03 stays ~92% scope coverage; whole plan stays ~16%. There is
no new closure credit and no additional prerequisite package.

The worker owns the existing Buttplug client/WebSocket protocol implementation, current-thread
Tokio runtime, handshake, device enumeration, command execution, reconnect and native cleanup.
Construction/declaration materialization is inert; engine readiness registers passive service.
Sixteen global lifetime slots include blocked opening/active/retiring providers. Cancellation
never queues a Stop command or joins. Worker termination and queued cancellation results precede
provider destruction, even on panic; blocked destruction retains its original slot. Stopped-worker
and spawn failures use absolute actor recovery deadlines. Existing reconnect backoff remains
250 ms to five seconds; commands are never replayed.

Review identified a required replacement ordering: native cleanup sends stop-all. Each module
now holds one connection lease through provider destruction, and reconfiguration/recovery
retries wait until it is released. This prevents late cleanup from stopping a replacement
connection's output while the actor remains responsive. Global lifetime admission still bounds
opening/active/retiring providers across modules.

Native configuration/command/catalog metadata, item/byte mailboxes and retained lifetime pools
have explicit bounds recorded in [Buttplug ownership](../architecture/buttplug-runtime.md).
Each actor turn consumes at most sixteen native events and sixteen completed receipts;
completed result backlogs rearm while disabled. A full mailbox refuses admission explicitly,
and rejected unpublished receipts do not emit cancellation wakes. Completed actions remain owned
until receipt disposal; not-sent cancellation differs from a started, unconfirmed panic.
Native endpoint creation is disabled in test builds. No real server/device/output operation ran.

Owning command:
`$env:CARGO_INCREMENTAL='0'; cargo test --locked -p Chataigne2 --bin Chataigne2 module_modules_controllers_buttplug -- --test-threads=4`
passes **18**, including nine preserved authoring/script/template/output-conversion cases and
nine new controlled qualification cases:

- Blocked opening retains one slot; eight replacement attempts refuse before opening a provider.
  Dropping the actor handle returns without joining, and capacity recovers after release.
- A blocked fake action admits 32 queued commands before the 256 KiB byte gate (ahead of the
  64-item cap). Cancellation preserves the started result, resolves every queued suffix as not
  sent before blocked cleanup, and performs exactly one native action.
- A panic during fake execution reports unconfirmed for the started request and not sent for
  the accepted suffix before blocked cleanup releases its slot.
- A full event mailbox pauses native consumption. All 128 admitted worker-side device-added
  records later drain in order; cancellation releases a worker waiting for event capacity.
- Completed command receipts retain their original global source quota; disposal releases it.
- Two connection failures respect backoff; a later lost session reconnects without replaying
  the completed action. Oversized configuration/command metadata and a 65-device catalog refuse
  admission, while the 64-device boundary passes.
- Engine construction opens no native provider. After a controlled connection settles, 100 idle
  actor turns have zero scheduled callbacks, worker callbacks and snapshot rebuilds.
- A blocked old provider retains the module's connection lease through cleanup; a replacement
  connects only after release. The lease remains charged independently of termination readiness.

Preservation on the final app test artifact passes **1072 / six existing ignored / 1078 selected**,
including all 50 App Control, 23 OS, 36 DMX and 73 MQTT cases. The artifact lists **1410** tests:
**1090 distinct selected passes / six ignored / 1096 selected**, with 314 outside these scopes.
Core is unchanged; its prior 597-pass/three-ignore result remains dated evidence, not another run.

The first check exposed Rust visibility/type errors; those are repaired. Rust 1.97 also reported
an incremental fingerprint compiler panic. Final owning tests, strict workspace/all-target Clippy
and workspace/test check pass with `CARGO_INCREMENTAL=0`; no dependency or compiler workaround
was added to repository source. Actual root/Core Cargo fmt and explicit app Rustfmt pass.
Source inventory: **1792 files / 36 oversized**. Buttplug root 864 lines, native adapter 414,
transport entrypoint 206, service 194 and new lifecycle suite 441; all touched sources stay
below 1000 lines. No Core API, wire DTO, persistence schema, dependency or app-shell startup change.

Remaining C07 criteria stay fixed:

1. Canonical command/script capture, bounded preparation before variable-size copies, ordering
   across entrypoints and command/module source retirement before native dispatch.
2. Native-prepared callbacks/catalogs and queued publication retaining original quotas behind
   current source/session/target guards, preserving accepted occurrences and selected-device identity.
3. Engine-level pressure, repeated reconfiguration, worker failure/recovery and blocked cleanup
   qualification of the completed package. Hardware/platform and full-product scale remain separate.

Continue those C07 criteria, then C08-C12. Keep the full P00-P18 goal active.

Final native `CARGO_INCREMENTAL=0` build passes in 2m13s. Protocol generation leaves generated
bindings unchanged. Documentation formatting and whitespace checks pass; all owned jobs are
terminal. Final artifact SHA-256:

- App `target/debug/deps/Chataigne2-b14ffd1b9e52f9ac.exe`:
  `5cb3553351c3ec523d0f5e2bf7266e2fe19785161c3860dbbe27c10db0941cfd`.
- Native `target/debug/Chataigne2.exe`:
  `78a1b0c927ec364ab98d0b61a6c7c905eb80a16d7e59edf4ae285c4b91cd2ccf`.

## C07 Buttplug canonical capture and source retirement

Implemented 2026-10-23 after `9cf9941e`, starting clean on `main`, matching `origin/main`.
**C07 remains open; six packages remain (C07-C12).** Canonical command/script capture and
producer/session retirement are delivered. Queued native-event/callback/catalog publication
and full package qualification remain. P03 stays ~92% scope coverage; whole plan stays ~16%.
No new package or general optimization prerequisite is added.

Command nodes now lease their canonical snapshot and execute payload, rather than constructing
a request JSON relay or a full execute-overlay snapshot. The worker reads only consumed fields
and honors the last matching override. Scripts capture engine-owned argument arrays and a
snapshot. Both entrypoints pass through typed local inbox attachments and ordered original
engine timestamps before native dispatch. Selected-device aliases, max-output policy, numeric
conversion and bounded string copies run on the worker against the captured snapshot. Obsolete
request DTOs and actor-side script/command parsing helpers are removed.

Each module admits 64 source lifetimes across local inboxes, outgoing/native queues, completed
receipts, queued publication and disposal. Global source/configuration storage admits 512 /
32 MiB. Captured inputs charge 64 KiB plus three times execute-override storage; the native
mailbox admits 64 / 4 MiB. Scripts admit four arguments / 64 KiB. Worker text limits and the
8 KiB configuration charge remain explicit in [ownership](../architecture/buttplug-runtime.md).
The actor submits eight captured sources and projects sixteen completed receipts per turn.
Full native admission keeps the same source behind an absolute 250 ms deadline, without
unpublished-receipt cancellation wakes.

Production source construction requires a producer, actor/native/session guards and its original
admission permit. Removed/disabled producers, transport retirement, worker termination and lost
sessions invalidate unstarted work. Reconnect never makes an old session's captured source live.
Already completed native effects retain their result: snapshot-free wakes keep receipts, disabled
modules rearm backlogs, and queued result edits retain the original source behind the current
module identity guard through application or rejection. Existing native cancellation/unconfirmed
outcome and cleanup-ordering contracts remain intact.

Final owning command:
`$env:CARGO_INCREMENTAL='0'; cargo test --locked -p Chataigne2 --bin Chataigne2 module_modules_controllers_buttplug -- --test-threads=4`
passes **27**, including all eighteen earlier owning cases and nine new engine cases:

- Ordinary ticks preserve an earlier command before a later script. The bounded last override
  replaces a consumed 1 MiB authored target without modifying it or copying a snapshot overlay.
- Captured script defaults retain the earlier max-output value after an authored change.
- Source 65 rejects before native work; all 64 admitted actions complete. Snapshot-free service
  retains finished results, sixteen-result turns leave 48, and queued edits retain the source
  admission until application/disposal.
- Removing a queued command prevents its native action while thirty unrelated edits continue
  behind a blocked fake action. Disabling the module preserves its started result and finishes
  all 63 unstarted cancellations through bounded disabled wakes.
- All eleven script methods share worker preparation; a 100 KiB argument refuses admission.
- Captured 100 KiB override records fill native mailbox bytes before its item limit; cancellation
  resolves every accepted suffix without another native effect.
- A captured source from a lost session reports not sent after reconnect. A fresh call works.
- A consumed 1 MiB field without an override fails before native I/O; the same provider remains
  usable for a later command.

The native-only small fixture now reaches its 64-item queue gate after the byte capacity expands
to 4 MiB; the captured-override case qualifies byte pressure separately. All action tests use
controlled providers. Test builds disable real native endpoint creation; no real server/device/
desktop operation ran.

Preservation passes **1072 / six existing ignored / 1078 selected**, including all 50 App Control,
23 OS, 36 DMX and 73 MQTT cases. The final app artifact lists **1419** tests: **1099 distinct
selected passes / six ignored / 1105 selected**, with 314 outside these scopes. Core is unchanged;
its prior 597-pass/three-ignore result remains dated evidence, not another run.

Strict workspace/all-target Clippy and workspace/test check pass with `CARGO_INCREMENTAL=0`.
Root/Core Cargo fmt and explicit app Rustfmt pass. Protocol generation leaves bindings unchanged.
The app facade gains one typed dispatch re-export; app-shell startup, Core APIs, dependencies,
wire DTOs and persistence schemas are unchanged. Source inventory: **1796 files / 36 oversized**;
Buttplug root 676 lines, command registry 211, service 221, outgoing boundary 127, native input
218 and engine command suite 375. All touched sources stay below 1000 lines.

Continue C07 queued native-event/callback/catalog ownership and guards, then full package
pressure/reconfiguration/failure qualification. Hardware/platform and product-scale checks retain
their later owners. Keep the full P00-P18 goal active.

Final native `CARGO_INCREMENTAL=0` build passes in 2m15s. Documentation formatting and whitespace
checks pass; all owned jobs are terminal. Final artifact SHA-256:

- App `target/debug/deps/Chataigne2-b14ffd1b9e52f9ac.exe`:
  `211522d83b289d68391fced44326f458846d4886ae7454ee3a41b618d063fb7b`.
- Native `target/debug/Chataigne2.exe`:
  `2f417323d6d7f4465e57d944eb24d1b8bcf4d8f724e6d0d1ca94be1e1db6c370`.

## C07 Buttplug closure: guarded native publication and package qualification

Implemented 2026-10-23 after `7b0f5767`, starting clean on `main`, matching `origin/main`.
**C07 is locally closed: seven of twelve packages closed; five remain (C08-C12).**
Next is C08 Joy-Con. P03 ~94% scope coverage and whole plan ~16% remain scope scores,
not forecasts of remaining effort or batch counts. No additional closure package is introduced.

The native worker now prepares callback JSON and shared immutable device-choice domains.
Publication retains its original item/byte lease through queued application or rejection;
canonical callback and domain quota follows ordinary event, live parameter and snapshot copies.
Equal catalogs reuse their allocation. Choice-only edits preserve parameter UUIDs and authored
selection, including a missing-device entry; a queued catalog cannot overwrite a later selection.
Actor/native/session and bounded target/ancestor guards precede all queued writes and callbacks.
Retirement explicitly cancels old input. Completed command results retain their separate,
past-effect contract and original source admission independently of session retirement.

Prepared input admits 128 global lifetimes / 128 MiB, charged at 1 MiB per record; the
64-item / 4 MiB native mailbox therefore fits four records. Catalogs admit 128 allocations /
64 MiB, with a 512 KiB allocation cap. Callback/publication pressure holds one pending native
record/draft, publishes a passive warning and pauses consumption without replacing the provider.
Cancellation wakes capacity waits. Actor input and completion turns remain capped at sixteen.
The [ownership contract](../architecture/buttplug-runtime.md) records the complete bounds.

The final app artifact passes **40 Buttplug owning + 1072 disjoint preservation = 1112 passes /
six existing ignored / 1118 selected / 1432 listed**, with 314 outside these scopes. Thirteen new
owning cases qualify guarded quota retention/rejection, snapshot-free input, repeated callbacks,
all 512 ordered input occurrences with bounded actor turns and companion output, input/callback
pressure and cancellation, shared catalogs/missing selection, absolute retry deadlines, eight
authored reconfigurations, and panic recovery before blocked cleanup. The continuous-input
fixture explicitly settles its command into the captured-output inbox before starting the stream.
All actions use controlled providers; test builds disable real native endpoint creation.
The earlier capture/source-retirement suite remains passing. Core is unchanged; its prior
597-pass/three-ignore result remains dated baseline, not an additional run.

Strict workspace/all-target Clippy, workspace/test check, actual root/Core Cargo fmt and explicit
app Rustfmt pass with `CARGO_INCREMENTAL=0`. Protocol generation leaves bindings unchanged.
Source inventory is **1801 files / 36 oversized**; all touched sources remain under 1000 lines.
No Core API, wire DTO, persistence schema, dependency or app-shell startup change was required.
Hardware/platform qualification and full-product scale retain P17/P18 and P16 ownership;
SDK transient enumeration allocation and process RSS are not qualified by these local tests.
Continue C08 Joy-Con; keep the full P00-P18 goal active.

Final native `CARGO_INCREMENTAL=0` build passes in 2m11s. Final artifact SHA-256:

- App `target/debug/deps/Chataigne2-b14ffd1b9e52f9ac.exe`:
  `b2033d975b5b8ad82a4bb7d33072a6b332f86822cffe2ffec3c0859b356add8e`.
- Native `target/debug/Chataigne2.exe`:
  `0450090b7ce31ce87a1348c3a06b05812a51b1ec37c2f065814ce44ccc5cd3ac`.

## C08 Joy-Con bounded runtime and passive service

Implemented 2026-10-23 after `d0064438`, starting clean on `main`, matching `origin/main`.
**C08 remains open. Seven of twelve packages are closed; five remain (C08-C12).**
P03 stays ~94% scope coverage and the whole plan stays ~16%; neither forecasts remaining effort.

Joy-Con now uses public bounded workers rather than unbounded channels and a joining retirement
fallback. Four native lifetime slots and one connection lease per module cover opening, active
work and blocked cleanup. Cancellation needs neither queue capacity nor a retirement thread.
Termination and queued command disposition precede native destruction. Completed command effects
retain their original source quota through receipts, queued projection and disposal; unstarted
cancellation is not sent and a started panic is unconfirmed. Commands never replay.

The worker services eight commands per turn. Its input mailbox admits 32 records / 256 KiB;
global input lifetimes admit 128 / 1 MiB. Output mailboxes admit 64 / 256 KiB; modules retain at
most 64 pending outcomes and global output lifetimes admit 512 / 2 MiB. Pending bounded input
pauses native reads while command handling and rumble-stop maintenance continue. Failed stops
retain per-controller 250 ms retry deadlines without delaying another controller's stop.
See the [ownership contract](../architecture/joycon-runtime.md) for limits and remaining scope.

The actor registers its wake on node readiness and has no periodic polling. Two input records
and sixteen outcomes are projected per service turn. Button/connection changes retain order;
analog-only state follows the processing cap. Activity uses its own absolute deadline, processing
time cannot accumulate idle debt, and startup recovery respects absolute backoff deadlines.
The first engine harness exposed registration before node readiness; the production lifecycle
hook was corrected, without weakening the six engine acceptance cases.

Final Joy-Con owning command:
`$env:CARGO_INCREMENTAL='0'; cargo test --locked -p Chataigne2 --bin Chataigne2 module_modules_controllers_joycon -- --test-threads=4`
passes **47**: thirty preserved cases and seventeen new cases. These cover 64 retained sources,
all 512 ordered input states, command/maintenance progress under input pressure, nonblocking
startup/retirement, completed/cancelled/unconfirmed outcomes before blocked cleanup, original
input quota retention, startup diagnostic lifetime, independent rumble retries, ordered engine
button callbacks, two-input turns, queued retirement rejection, snapshot-free service, long-idle
timing, absolute recovery deadlines and 100 idle ticks with zero callbacks/snapshot rebuilds.

The final app artifact also passes **40 Buttplug + 1072 disjoint preservation**: **1159 distinct
passes / six existing ignored / 1165 selected / 1449 listed**, with 284 outside these scopes.
Core is unchanged; its prior 597-pass/three-ignore result remains dated baseline, not a new run.
Strict workspace/all-target Clippy, workspace/test check, actual root/Core Cargo fmt and explicit
app Rustfmt pass with `CARGO_INCREMENTAL=0`. Protocol generation leaves bindings unchanged.
Source inventory: **1807 files / 35 oversized**. The former oversized Joy-Con module is split
into focused runtime, native, service and state files; all touched sources remain below 1000 lines.
No Core API, wire DTO, persistence schema, dependency or app-shell startup change was required.

Continue C08 canonical command/script capture and producer retirement, native callback preparation
and callback-capacity recovery, complete queued target/path guards, and engine-level package
pressure/reconfiguration/failure qualification. Native tests use controlled providers; real native
endpoint creation is disabled in test builds. Hardware/platform and product-scale acceptance stay
with P17/P18 and P16. No prerequisite package is added; keep the full P00-P18 goal active.

Final native `CARGO_INCREMENTAL=0` build passes in 2m31s. Final artifact SHA-256:

- App `target/debug/deps/Chataigne2-b14ffd1b9e52f9ac.exe`:
  `f29e6b69b46d84e5fe1d212b95a13f280cfc8d1c412966eca78776a6e38be7ba`.
- Native `target/debug/Chataigne2.exe`:
  `cb9473b72e5c9237780cd425d8f66b85adfde788da8b3e5b352060e9f9e9c656`.

## C08 Joy-Con canonical capture and producer/session retirement

Implemented 2026-10-23 after `47d6f7a7`, starting clean on `main`, matching `origin/main`.
**C08 remains open. Seven of twelve packages are closed; five remain (C08-C12).**
P03 stays ~94% scope coverage; the whole plan stays ~16%. No closure credit or new prerequisite
package is added. Continue native callbacks, queued target/path guards and full package qualification.

Command nodes retain canonical invocation snapshots and execute payloads instead of building
request JSON or a full execute-overlay snapshot. Both script methods retain engine-owned argument
arrays. Defaults, borrowed field limits, numeric parsing, target/LED validation and native request
construction share worker preparation. Scripts use static defaults and do not lease an unused
module snapshot. Original source quota follows local inboxes, outgoing/native queues, completed
receipts, queued projection and disposal. Producer identity and actor/native/session guards fence
unstarted work; any controller-slot connection change retires the captured pair's session.
Reconnect never revives a previous session's request. Completed effects retain their results.

The mixed command/script test exposed an ordering gap under partial inbox dispatch: a deferred
command handoff could reach the module after a later script was submitted. Each admitted source
now owns an ingress permit until arrival or disposal. Service submits the sorted timestamp prefix
only after staged handoffs settle; releasing a permit wakes service. Native mailbox pressure retains
the same source behind an absolute 250 ms deadline, without unpublished-receipt cancellation wakes.

The original per-module source limit remains 64 across all these stages. Production sources charge
64 KiB plus three times retained execute-override storage; global output storage admits 512 /
32 MiB, and the native mailbox admits 64 / 4 MiB. Scripts admit five arguments / 64 KiB; consumed
text/numeric-text/diagnostic limits are 256/32/1024 bytes. Actor submission and completion turns
remain eight and sixteen. The [ownership contract](../architecture/joycon-runtime.md) records the
source boundary, session and ingress rules. Earlier native/input/rumble bounds remain unchanged.

Final owning run passes **60**, including all 47 earlier cases and thirteen new capture cases.
Owning evidence covers ordinary ticks and partial-dispatch ordering, last overrides, frozen
snapshot values after later authored edits, 64-source pressure through queued disposal,
snapshot-free receipts, removed producers, disabled-backlog completion, disconnect/reconnect
retirement, both script methods, oversized arguments and numeric fields, bounded malformed-payload
diagnostics, mailbox bytes and item pressure, absolute retry/idle service, and undelivered-ingress
disposal. Request overrides latch explicit request values; unoverridden fields use the snapshot
at capture, not a reconstructed earlier event-time snapshot. No acceptance criterion was weakened.

On the final app artifact, **60 Joy-Con + 40 Buttplug + 1072 disjoint preservation = 1172 distinct
passes / six existing ignored / 1178 selected / 1462 listed**, with 284 outside these scopes.
Core is unchanged; its prior 597-pass/three-ignore result remains dated baseline, not another run.
Strict workspace/all-target Clippy, workspace/test check, actual root/Core Cargo fmt and explicit
app/command Rustfmt pass with `CARGO_INCREMENTAL=0`. Protocol generation leaves bindings unchanged.
Source inventory: **1810 files / 35 oversized**; all touched sources remain below 1000 lines.
One typed app dispatch re-export is added; Core APIs, wire DTOs, persistence, dependencies and
app-shell startup are unchanged. Tests use controlled providers; native endpoints remain disabled
in test builds. Hardware/platform and product-scale qualification retain their later plan owners.

Final native `CARGO_INCREMENTAL=0` build passes in 2m18s. Final artifact SHA-256:

- App `target/debug/deps/Chataigne2-b14ffd1b9e52f9ac.exe`:
  `97967488e3c918ff8569d38625469e9ab75c3b09451b963b3fbbf1973b000d54`.
- Native `target/debug/Chataigne2.exe`:
  `3ff69149a880d3ef159a3cbd274e4bddd5800d2aafdc5b6e2a4fcc7448b7dba4`.

Continue C08 native-prepared callbacks and capacity recovery, complete queued publication guards,
then engine-level package pressure/reconfiguration/failure qualification. Keep the full P00-P18 goal active.

## C08 Joy-Con local closure

Implemented 2026-10-23 after `3a7250f2`, starting on `main` matching `origin/main`.
**C01-C08 are locally closed: eight of twelve packages closed, four remain (C09-C12).**
P03 advances to ~95% scope coverage; whole-plan coverage remains ~16%. Continue C09 Keyboard
and Mouse after delivering this milestone. The full P00-P18 goal remains active.

Native workers prepare at most 50 callback candidates / 8 MiB per draft and cache unchanged
controller rows. The actor selects already-prepared payloads against the last applied state,
preserving analog coalescing and cross-controller updates. Atomic canonical Arc-batch admission
in public Golden IO prevents partially admitted frames from stranding callback quota. Event Arc
copies retain that quota independently; analog pending state retains its original input reservation.
Configuration revisions cancel old preparation and regenerate the current-session raw frame
without another native lifetime or command replay. Fixed target/ancestor guards and an authored
settings fence precede queued values and callbacks. See [Joy-Con ownership](../architecture/joycon-runtime.md).

Windows/default-feature validation uses `CARGO_INCREMENTAL=0`:

- `cargo test --locked -p Chataigne2 --bin Chataigne2 app::module_modules_controllers_joycon -- --test-threads=1`:
  **71 passed**. All 60 earlier cases remain green. The eleven new cases qualify callback pressure
  with continued command/rumble maintenance, cancellation, full two-side callback vocabulary and
  raw-copy quota, queued leaf/ancestor rename/reparent and settings rejection, original input
  quota through rejection, analog coalescing, eight reconfigurations, blocked-cleanup recovery,
  and 512 ordered input edges with two-record actor turns and continued output/authored edits.
- On that exact app artifact, **40 Buttplug + 1072 disjoint preservation passed**, with six existing
  ignored. Combined app evidence is **1183 passes / six ignored / 1189 selected / 1473 listed**;
  284 cases are outside these scopes. Selection preserves the prior module/command/stream/HTTP/
  generator/Formula/state-machine gates; listing and scope disjointness were checked before runs.
- `cargo test --locked -p golden_io --lib`: **57 passed**, including five atomic-batch cases for
  all-or-none item/byte admission, raw Arc lifetime, duplicate weights/order, existing registrations
  and overflow. The earlier Core facade 597-pass/three-ignore run remains dated baseline.
- Strict workspace/all-target Clippy and `cargo check --locked --workspace --tests` passed.
  Public protocol generation passed and left generated bindings unchanged. Actual root/Core
  Cargo fmt and explicit app Rustfmt ran; content hashes confirm formatting changed no tested source.

Refreshed inventory: **2010 source files / 35 oversized**, using Rust/TS/TSX/Svelte/JS/MJS under
`apps`, `crates`, `packages` and `xtask`, including generated files. This explicitly broader inventory
replaces the prior discovery scope; all touched sources remain below 1000 lines. The Golden IO
addition is domain-neutral. No wire DTO, persistence schema, dependency or app-shell change.
Controlled providers only; native endpoints are disabled in test builds. Physical/platform and
product-scale qualification remain with P17/P18 and P16.

Final app test artifact SHA-256 (`target/debug/deps/Chataigne2-b14ffd1b9e52f9ac.exe`):
`e374f153103f71ab11c2c757c11b0724ef8c651491eab8c81f4752c4fb742afe`.

Final native `CARGO_INCREMENTAL=0 cargo build --locked -p Chataigne2 --bin Chataigne2` passed
in 2m36s. SHA-256 (`target/debug/Chataigne2.exe`):
`1aee315354d4e555e5ad9f1ea5721332a01371d3eeede756b441708c7291975f`.

## C09 Keyboard native input ownership

Implemented 2026-10-23 after `b146ccf1`, starting clean on `main` matching `origin/main`.
**C09 remains open: eight of twelve packages closed, four remain (C09-C12).** P03 ~95% scope
coverage and whole-plan ~16% are unchanged. This milestone delivers the Keyboard input boundary;
continue the remaining Keyboard and Mouse acceptance within C09 before moving to C10.

The app-owned input handle starts and cancels without waiting for native readiness or joining a
thread. A bounded IO owner retains four global slots and a per-module lease through native cleanup.
The platform thread keeps native message processing/device queries, using a 64-record / 2 MiB
handoff with explicit overload. An independent stopped signal is observable before sender or
native cleanup disposal. The owner publishes through 32 records / 8 MiB and a global 128-lifetime /
32 MiB pool. Pressure pauses forwarding without another native read; cancellation remains available.
Catalog/name/packet bounds precede native allocation or publication. Actor turns drain at most 16.
Original publication quota follows queued application/rejection, and current catalogs retain their
input publication. Actor/native revisions, fixed target paths and authored settings fence projection.

The burst test exposed a conflict between retained reads and the old device-parameter replacement.
Device choices now use public `SetParamEnumOptions`, preserving parameter identity instead of
replacing the node. The existing narrow persisted label-suffix normalization stays in the backend.
Native input/output operations are blocked in test builds; controlled providers exercise this work.
See [Keyboard ownership](../architecture/keyboard-runtime.md) for layers, bounds and remaining gaps.

Windows/default-feature checks use `CARGO_INCREMENTAL=0`:

- `cargo test --locked -p Chataigne2 --bin Chataigne2 app::module_modules_controllers_keyboard -- --test-threads=1`:
  **16 passed** (four preserved and twelve new). Cases cover raw item/byte overload, terminal state
  before sender disposal, disabled native endpoints, blocked startup and cleanup with both leases
  retained, original payload quota/recovery, capped actor drain, bounded catalogs/IDs, 64 ordered
  edges, and rejection after retirement, target rename or authored receive-disable.
- On the exact final app artifact, **71 Joy-Con + 40 Buttplug + 7 Mouse + 1072 disjoint preservation
  passed**, with six existing ignored. Combined evidence: **1206 passes / six ignored / 1212 selected /
  1485 listed**, 273 outside scope. Listing and scope disjointness were checked before selection.
  Core is unchanged; the prior Golden IO 57-pass and Core facade 597-pass/three-ignore results
  remain dated baselines rather than new runs.
- Strict workspace/all-target Clippy, `cargo check --locked --workspace --tests`, actual root/Core
  Cargo fmt and explicit app Rustfmt passed. Content hashes verify formatting changed no tested
  source. Protocol generation passed with no generated binding diff. Native build passed in 2m21s.

Source inventory: **2020 files / 34 oversized**, using the same explicit Rust/TS/TSX/Svelte/JS/MJS
scope under `apps`, `crates`, `packages`, `xtask`, including generated files. Every touched source
is below 1000 lines. Runtime, platform, selection, value and test responsibilities are split into
cohesive files. No Core API, wire DTO, persistence, dependency or app-shell startup change.

**Next:** Keyboard output/canonical command and script capture, native-prepared callbacks and
canonical choice lifetimes, passive service and full recovery qualification; process-owned Windows
raw-input registration with bounded subscriber fan-out; then Mouse. Windows permits one registered
window per input device class per process, so the existing per-module registration policy needs
repair within C09, not another prerequisite package. Physical/cross-platform and full-product
qualification remain with P17/P18 and P16. No desktop input was captured or injected during checks.

Final app artifact SHA-256 (`target/debug/deps/Chataigne2-b14ffd1b9e52f9ac.exe`):
`47bb469d3da86edfbdec526e626bea57ccc8d5ac7d01a387789ce296d954ae8d`.

Final native artifact SHA-256 (`target/debug/Chataigne2.exe`):
`64b8dbe31035289e264adaef313163d99409725b20d433bd47e9cee3ff946fdc`.

## C09 Keyboard canonical output ownership

Implemented 2026-10-23 after `d351fcef`, starting clean on `main` matching `origin/main`.
**C09 remains open: eight of twelve packages closed, four remain (C09-C12).** P03 ~95% and
whole-plan ~16% scope coverage are unchanged. Continue the remaining Keyboard input publication,
shared endpoint ownership and full package qualification, then Mouse; no prerequisite package is added.

Keyboard output now has a bounded worker owning native creation, parsing, key actions and cleanup.
Four global lifetimes and one per-module lease include blocked opening/execution/cleanup. Command
nodes retain canonical invocation snapshots and original execute payloads; scripts retain engine
argument arrays. The worker applies the last consumed override and prepares a typed key/action
without an execute-overlay snapshot or request JSON relay. The obsolete request DTO and synchronous
module controller path are removed; a typed app dispatch re-export is the supported boundary.

Per-module admission covers 64 sources across local inboxes, staged/native requests, receipts and
queued application/rejection/disposal. Global sources admit 512 / 32 MiB; native mailboxes admit
64 / 4 MiB. Sources charge 64 KiB plus three times retained override storage. Scripts admit one
argument / 64 KiB; key/action text admits 64 bytes. Original timestamps and ingress settlement
preserve command/script order, including partial inbox dispatch. Full mailbox admission retains
the same source behind an absolute 250 ms deadline without false cancellation notifications.

Producer and actor/native guards fence unstarted effects. Completion and unconfirmed/not-sent
outcomes precede provider cleanup, retain original quota, and do not replay. Passive output service
uses eight submissions / sixteen completions per turn, disabled-backlog wakes and absolute
250 ms-to-five-second startup/failure recovery. Current target/path guards protect result writes
independently of retired native sources. Readiness uses one captured value and its own native
fence after completion messages, preserving an applied effect-message prefix while rejecting
retired ready status. See [Keyboard worker ownership](../architecture/keyboard-runtime.md).

Windows/default-feature checks use `CARGO_INCREMENTAL=0`:

- Owning command `cargo test --locked -p Chataigne2 --bin Chataigne2 app::module_modules_controllers_keyboard -- --test-threads=1`:
  **37 passed**, preserving sixteen input cases and adding twenty-one output cases. Evidence covers
  native test guards, blocked startup/cleanup, original source quota through receipts/disposal,
  ingress settlement, panic/cancellation outcomes, bounded diagnostics and recovery, passive idle,
  snapshot-free and retired completions, result rename/readiness rejection, ordinary/partial-dispatch
  ordering, all three script actions, frozen snapshots, removed producers, disabled backlogs,
  64-source actor pressure, native mailbox byte pressure with same-source retry and eight reconfigurations.
- The exact final artifact passes **71 Joy-Con + 40 Buttplug + 7 Mouse + 1072 disjoint preservation**,
  with six existing ignored. Combined: **1227 passes / six ignored / 1233 selected / 1506 listed**;
  273 are outside scope. Listing and selection disjointness were checked before runs.
- The first preservation run overlapped native compilation: two existing multiplex timing gates
  failed at 5.880/6.062 ms averages against their 5 ms limits (1070 passed / two failed / six ignored).
  With builds idle, the two isolated gates passed at 3.734/4.653 ms, with no deadline misses.
  The full preservation selection was then rerun without a concurrent build: **1072 passed /
  zero failed / six ignored** in 82.18s. No threshold or test was changed.
- Strict workspace/all-target Clippy, `cargo check --locked --workspace --tests`, protocol generation
  with no generated binding diff, actual root/Core Cargo fmt and explicit app Rustfmt passed.
  Hash comparison confirmed formatting changed no tested source. Final native build passed in 2m09s.

Inventory: **2030 source files / 34 oversized**, using Rust/TS/TSX/Svelte/JS/MJS under
`apps`, `crates`, `packages`, `xtask`, including generated files. All touched sources are below
1000 lines. No Core API, wire DTO, persistence schema, dependency or app-shell startup change.
Tests use controlled providers; native keyboard input/output operations are disabled in test builds.
No desktop input was captured or injected. Physical/cross-platform and full-product qualification
remain with P17/P18 and P16.

Final SHA-256:

- App `target/debug/deps/Chataigne2-b14ffd1b9e52f9ac.exe`:
  `5b72e14e5ed00af0e34af5a0c2593c15ef3d7c03ecf02febb3230ea9560e27d0`.
- Native `target/debug/Chataigne2.exe`:
  `a8ff9e3cb8dfc078591313076024c8f5b21b0b012d533435ca397d547c4b32c4`.

## C09 Keyboard shared Windows input registration

Implemented 2026-10-23 after `45c7f3a9`, starting clean on `main`. One C09 ownership gap is
locally qualified: Keyboard modules share one process-owned Windows raw-input registration.
**C09 remains open: eight of twelve packages closed, four remain (C09-C12).** P03 ~95% and
whole-plan ~16% scope coverage are unchanged. Continue native input callback/catalog preparation,
canonical published choices, passive input service, shared output coordination and package
qualification, then Mouse. No new prerequisite package is added.

The app-owned registry runs behind existing input IO owners. Overlapping startup shares one
endpoint; dropping one subscription leaves its peers running. The final subscription stops and
joins the native thread behind the IO boundary. A separate single process lifetime permit survives
the join, refusing replacement registration while cleanup is blocked. Window cleanup removes
registration before destruction. Failure and panic are observable to subscribers before window cleanup.

Fan-out admits four subscriber lifetimes, each with 64 records / 2 MiB. Input charges 4 KiB;
catalogs charge 512 KiB and retain the existing 64-device / 1024-byte field limits. A rejected
subscriber keeps its slot and accepted queue prefix until disposal; peers continue in order.
Latest catalog seeding and subsequent edges share one lock, preserving catalog-before-edge order
and original device identities. Existing module recovery handles native failure, overload and
endpoint retirement. The duplicated Windows event enum and per-module native registration are removed.
See [Keyboard worker ownership](../architecture/keyboard-runtime.md).

Windows/default-feature validation uses `CARGO_INCREMENTAL=0`:

- `cargo test --locked -p Chataigne2 --bin Chataigne2 app::module_modules_controllers_keyboard -- --test-threads=1`:
  **45 passed**, including eight new controlled-endpoint cases and all 37 preserved Keyboard cases.
  New cases cover shared startup, independent retirement and ordered device edges, four-slot
  admission including a failed subscriber, item/catalog byte pressure, accepted prefixes, catalog
  seeding, blocked cleanup/replacement refusal, invalid metadata, and eight panic/recovery cycles.
- The final artifact passes **71 Joy-Con + 40 Buttplug + 7 Mouse + 1072 disjoint preservation**,
  with six existing ignored: **1235 passes / six ignored / 1241 selected / 1514 listed**;
  273 are outside scope. Listing and disjointness were checked before runs.
- The initial preservation run had **1071 passed / one failed / six ignored** in 88.72s:
  the existing active multiplex timing gate averaged 5.197 ms against its 5 ms limit, with zero
  deadline misses. No build was running; no cause is established. Isolation passed at 3.807 ms
  with uncaptured output and 3.788 ms with the normal captured-output harness, both without
  deadline misses. The complete selection then passed **1072 / zero failed / six ignored** in
  90.51s on the same executable. No runtime source, test or threshold was changed between runs.
- Strict workspace/all-target Clippy and `cargo check --locked --workspace --tests` passed.
  Actual root/Core Cargo fmt and explicit app Rustfmt passed; source hashes remain unchanged
  through final validation. Native `cargo build --locked -p Chataigne2 --bin Chataigne2` passed in 2m39s.

Inventory: **2032 source files / 34 oversized**, using Rust/TS/TSX/Svelte/JS/MJS under
`apps`, `crates`, `packages`, `xtask`, including generated files. All four touched Rust sources
are below 1000 lines. No Core API, wire DTO, generated binding, persistence, dependency or app
startup change. Prior Golden IO/Core results remain dated baselines. Tests use fake endpoints;
native input/output remain disabled in test builds. No desktop input was captured or injected.
Physical/cross-platform and full-product qualification remain with P17/P18 and P16.

Final SHA-256:

- App `target/debug/deps/Chataigne2-b14ffd1b9e52f9ac.exe`:
  `af14c9c4cc9e574d752701fa29ea4a0bc39ef44e7503cf4911b9c4beb4ac9132`.
- Native `target/debug/Chataigne2.exe`:
  `983c16699079336d026b3666697390624c9f29a27e3c9ccf08186eb201cc65fc`.

## C09 Keyboard prepared input and passive service

Implemented 2026-10-23 after `a2fbfb4b`, starting clean on `main`. Input matching, held/modifier
state, callback JSON, messages and canonical native choices now belong to the input worker.
The actor projects immutable frames and drains notifications, replacing its 120 Hz update and
elapsed retry clock with absolute 250 ms-to-five-second recovery deadlines. **C09 remains open:
eight of twelve packages closed, four remain (C09-C12).** P03 ~95% and whole-plan ~16% scope
scores are unchanged. Finish shared output coordination and full package qualification, then Mouse.

Native input envelopes charge 64 KiB and shared catalog metadata charges 2 MiB under the original
128-source / 32 MiB pool. Frames retain the metadata's canonical Arc quota after the catalog
envelope disappears. Separate choice-domain and callback quotas follow live, snapshot, event,
replay and exported allocations. Four callback candidates admit atomically; no JSON construction
or native catalog conversion remains in actor input projection. See [ownership and limits](../architecture/keyboard-runtime.md).

Selection revisions cancel old staged/queued work without reopening native input. A turn checks
authored settings and targets before its own choice edits; each publication also fences its native
source at projection. Current snapshot selection/receive checks reject old frames before parameter
notifications dispatch. No Input Device retains discovery choices while filtering keys on the worker.
Native metadata validation bounds spare capacity as well as contents. The old actor event bypass
in tests is removed; the existing value regression now runs through the real preparation worker.

Windows/default-feature checks use `CARGO_INCREMENTAL=0`:

- `cargo test --locked -p Chataigne2 --bin Chataigne2 app::module_modules_controllers_keyboard -- --test-threads=1`:
  **61 passed** (45 preserved, sixteen new). Seven actor cases cover idle/no-input service with
  zero updates/snapshots/wakes, 512 ordered edges via replay observation, pre-notification settings
  rejection, snapshot-free service, deadline recovery and continued actor edits before blocked cleanup.
  Five preparation cases cover frozen key/modifier/device/output-state candidates, selected-device
  loss/reset, legacy/missing choices, canonical domain export and atomic callback/replay retention.
  Four worker cases cover callback/domain pressure without polling or replay, eight selection changes
  with one native open and shared domains, and metadata lifetime beyond both envelopes and owner.
- The exact final artifact also passes **71 Joy-Con + 40 Buttplug + 7 Mouse + 1072 disjoint preservation**:
  **1251 passes / six existing ignored / 1257 selected / 1530 listed**, with 273 outside scope.
  Listing/disjointness was checked. Preservation passed **1072 / zero failed / six ignored** in 87.97s
  with builds idle; no timing threshold was changed.
- Strict workspace/all-target Clippy, `cargo check --locked --workspace --tests`, actual root/Core
  Cargo fmt and explicit app Rustfmt passed. All sixteen touched source hashes remain unchanged
  through final verification. Native `cargo build --locked -p Chataigne2 --bin Chataigne2` passed in 2m59s.

Inventory: **2037 source files / 34 oversized**, using Rust/TS/TSX/Svelte/JS/MJS under `apps`,
`crates`, `packages`, `xtask`, including generated files. All touched sources remain below 1000 lines;
Keyboard's root module shrinks from 666 to 398 lines. No Core API, wire/generated protocol,
persistence, dependency or app-shell startup change. Prior Golden IO/Core passes remain dated.
Tests use controlled providers; native input/output remain disabled in test builds. No desktop
input was captured or injected. Physical/platform and full-product gates remain with P17/P18 and P16.

Final SHA-256:

- App `target/debug/deps/Chataigne2-b14ffd1b9e52f9ac.exe`:
  `0a94e69207bdba798fe0de13dd720a7989ac8f76d20d2b6e2a13192e6e35b70d`.
- Native `target/debug/Chataigne2.exe`:
  `ec5c4dac153e178965a7e5c2d73f22a48c938b8fd25b6addcc12620cb5a5e8e9`.

## C09 Keyboard shared output and local qualification

Implemented 2026-10-23 after `2e89db03`, starting clean on `main`. **Keyboard is locally qualified;
C09 remains open for Mouse. Eight of twelve packages are closed; four remain (C09-C12), covering
six unqualified implemented families.** P03 ~95% and whole-plan ~16% scope scores are unchanged.
Physical/platform and full-product qualification keep their P17/P18 and P16 owners.

Keyboard output now shares a process coordinator with 512 bounded weak claims. Pending captured
requests sort by original engine time and capture sequence, including local staging. One permit
covers a whole native request, preventing cross-module tap interleaving. Cancellation/liveness is
checked before starting; retired unsubmitted heads cannot strand peers. Finished receipts retain
original source quotas without retaining ordering. Panic releases the permit before blocked provider
cleanup while IO lifetime leases remain held. No automatic effect replay is added.

Combined turns project completed output reports, input values/callbacks and finally derived
connection status. Status samples output readiness once and fences every contributing active source.
Input never writes output-ready state. Core `CheckPreconditions` now stops scalar/removal coalescing:
a later rejected scope cannot erase an earlier prefix or borrow its completion. This repairs the
concrete combined-service failure boundary without adding an app-specific Core contract. Outgoing
success logging again follows the existing module flag. See [Keyboard ownership](../architecture/keyboard-runtime.md)
and [application contracts](../architecture/application-contracts.md).

Windows/default-feature checks use `CARGO_INCREMENTAL=0`:

- Strict workspace/all-target Clippy and `cargo check --locked --workspace --tests` passed.
- `cargo test --locked -p golden_engine --lib edit::tests -- --test-threads=1`: **three passed**.
  Full final Core engine artifact `golden_engine-acb0109b8761871d.exe --test-threads=1`, run from
  `crates/golden_core/engine`: **600 passed / three existing ignored / 603 listed**, in 1.21s.
  Regressions cover guarded prefix/completion retention, ordinary in-scope scalar coalescing and
  duplicate-removal scope boundaries.
- `cargo test --locked -p Chataigne2 --bin Chataigne2 app::module_modules_controllers_keyboard -- --test-threads=1`:
  **73 passed** (61 preserved, twelve new). Three coordinator cases cover time/tie ordering,
  retired unsubmitted heads and 512-claim pressure. Four worker cases cover compound atomicity,
  local/native staging order, waiting cancellation and peer progress before blocked cleanup.
  Five combined cases cover idle service, both native failures/recovery without replay, independent
  report/input/readiness rejection prefixes, and 256 ordered input callbacks while 64 output sources
  remain retained behind a blocked native effect. Deterministic gates arrange the pending report.
- Final app preservation: **71 Joy-Con + 40 Buttplug + 7 Mouse + 1072 disjoint preservation**.
  Combined: **1263 passes / six ignored / 1269 selected / 1542 listed**, with 273 outside scope.
  Listing/disjointness was checked; preservation passed **1072 / zero failed / six ignored** in
  82.58s with builds idle. No test threshold changed.
- Actual root/Core Cargo fmt, explicit app Rustfmt, documentation formatting and diff checks passed;
  tested source hashes remain unchanged. Native build passed in 3m00s. Generated protocol is unchanged;
  there is no DTO, persistence, dependency or app-shell startup change.

Inventory: **2043 source files / 34 oversized**, using Rust/TS/TSX/Svelte/JS/MJS under `apps`,
`crates`, `packages`, `xtask`, including generated files. All touched sources are below 1000 lines.
Tests use controlled providers; native input/output remain disabled in test builds. No desktop
input was captured or injected. Next: Mouse under the existing C09 package, then C10-C12.

Final SHA-256:

- App `target/debug/deps/Chataigne2-b14ffd1b9e52f9ac.exe`:
  `7c410ca6483de2f8280a9fcaa9c7f23af82bc969e0df3267a5bfab592f533de4`.
- Native `target/debug/Chataigne2.exe`:
  `13b24f780e6ad6736d601da8ae99b99181e31fbb6083002a11cee04daaee7d28`.

## C09 Mouse native input ownership and passive service

Implemented 2026-10-23 after `dc5e5609`, starting clean on `main`. **C09 remains open for
Mouse output, prepared publication and combined qualification. Eight of twelve packages are
closed (67%); four remain (C09-C12), covering six unqualified implemented families.** P03 ~95%
and whole-plan ~16% scope scores are unchanged. The 67% is closure-package completion, not an
effort forecast or a replacement for the documented scope rubric.

Mouse native discovery, readiness waits, polling and joins now run on IO owners rather than
module callbacks. Each module holds one lifetime slot; the process permits four owners, including
blocked setup/cleanup. Stop invalidates retained sources immediately; termination becomes visible
before cleanup. Input sources retain shared 128-item / 32 MiB quotas until final disposal. Records
weigh 4 KiB; catalogs weigh 512 KiB, capped at 64 devices / 1024 backing bytes per string field.
The actor mailbox admits 32 records / 8 MiB, with at most 16 records per service turn and provider poll.
Full lifetime/mailbox capacity pauses the worker through cancellable waits.

Windows now shares one raw-input endpoint across four bounded subscriptions (64 records / 2 MiB
each). Catalog seeding precedes later input; motion deltas and button occurrences retain order.
An overloaded subscription rejects explicitly and drains its accepted prefix while peers progress.
Capture selection belongs to each subscription; native messages update the optional hook, and
an actor lifetime guard revokes capture immediately before disposal or blocked cleanup. The final
registration slot survives the final join. RAII teardown removes the hook/registration/window on
failure or unwind. Native inventory/name allocations are capped; raw packets use aligned fixed storage.
Portable polling directly owns `device_query::DeviceState` on the IO owner without an extra producer.

Worker notifications replace the 120 Hz engine update. Explicit two-second retry deadlines and one
short delta-clear deadline provide recovery and pointer presentation without recurring idle work.
Warning cleanup covers failure, receive-disable and module-disable. The first 27-case run exposed wake
registration during `init` (26 passed / one failed); registering in `on_node_ready` restores preserved
value updates. Output remains synchronous in the extracted `output_native.rs`; test builds disable
both native input and output. See [Mouse ownership](../architecture/mouse-runtime.md).

Final Windows/default-feature checks use `CARGO_INCREMENTAL=0`:

- Strict workspace/all-target Clippy passed in 40.18s; `cargo check --locked --workspace --tests`
  passed in 32.45s.
- `cargo test --locked -p Chataigne2 --bin Chataigne2 app::module_modules_controllers_mouse -- --test-threads=1`:
  **31 passed** (seven preserved, 24 new). Eight owner cases cover retained quota recovery,
  bounded ordered turns, cancellation, blocked startup/cleanup, native failure, oversized metadata
  and repeated failures. Twelve broker cases cover shared startup, four-subscription limits,
  catalog ordering/byte pressure, independent overload, endpoint lifetime/recovery and capture
  selection, configuration notifications and immediate actor revocation. Four engine cases cover
  idle service, healthy worker publication, failure clearing held values and pressure-warning cleanup.
- Final disjoint preservation: **73 Keyboard + 71 Joy-Con + 40 Buttplug = 184 passes**;
  **1072 other passes / six existing ignored**, in **MOUSE_PRES_TIME** with builds idle.
  Combined: **1287 passes / six ignored / 1293 selected / 1566 listed**, with 273 outside scope.
  Listing/disjointness and an unchanged executable hash were checked. No timing threshold changed.
- Native `cargo build --locked -p Chataigne2 --bin Chataigne2` passed in **MOUSE_NATIVE_TIME**.
  Actual root/Core Cargo fmt, explicit app Rustfmt, documentation formatting and diff checks passed;
  all fourteen tested source hashes remain unchanged. Generated protocol is unchanged. No Core API,
  DTO, persistence, dependency or app-shell startup change; prior Core/Golden IO results remain dated.

Inventory: **2054 source files / 32 oversized**, using Rust/TS/TSX/Svelte/JS/MJS under `apps`,
`crates`, `packages`, `xtask`, including generated files. Every touched source is below 1000 lines;
Mouse root shrinks from 1104 to 909 lines and its input runtime from 1275 to 323. No desktop input
was captured or injected. Physical/platform and product-scale gates retain P17/P18 and P16 owners.

**Next within C09:** canonical command/script output capture and bounded ordered receipts;
worker-prepared input callbacks/catalogs, retained publication quotas and queued source/target/settings
fences (including matching-generation catalog/readiness on restart); then combined pressure,
reconfiguration and failure/recovery qualification. Do not close C09 from this native input slice alone.

Final SHA-256:

- App `target/debug/deps/Chataigne2-b14ffd1b9e52f9ac.exe`:
  `79a13faa4e41a7c29e71d5e47e31a2e05a236c314a7913a1bb38c7f507fca625`.
- Native `target/debug/Chataigne2.exe`:
  `4a4659709347906d89171d8e51e3e2688e488ec3d085992574495d0e66572bd1`.

## C09 Mouse canonical output ownership and shared OS ordering

Implemented 2026-10-23 after `b02a1f94`, starting clean on `main`. **C09 remains open for
Mouse prepared input publication and combined qualification. Eight of twelve packages are closed
(67%); four remain (C09-C12), covering six unqualified implemented families.** P03 ~95% and
whole-plan ~16% scope scores are unchanged. No new package or generic prerequisite is introduced.

Move, button and scroll commands now capture canonical leased snapshots and original execute
payloads; all five scripts capture engine-owned argument arrays. Worker preparation applies the
last consumed override and validates bounded text, finite/ranged coordinates and scroll values.
Native pointer/display queries, movement, clicks and wheel injection execute on bounded IO owners.
Methods acknowledge admission; validation/native outcomes arrive through explicit receipts.

One per-module and four process worker lifetimes include blocked setup/execution/cleanup. Each
module admits 64 original source lifetimes across local staging, native queues, receipts and queued
reports. Shared Mouse output sources admit 512 items / 32 MiB; native queues admit 64 / 4 MiB.
Sources weigh 64 KiB plus three times retained override bytes; scripts admit four arguments / 64 KiB.
Option text admits 64 bytes; diagnostics admit 1024 UTF-8 bytes. Source/snapshot/argument quotas
survive draining, sharing, cancellation and queued projection until final disposal. Actor turns
submit eight sources and service sixteen receipts. Retry keeps the same source and backs off
250 ms to five seconds; native effects are never automatically replayed.

The existing Keyboard coordinator and its three tests moved into app-owned
`module/common/desktop_output.rs`. Both Keyboard and Mouse use its single process instance and
512 weak ordering claims. Pending work sorts by original engine time/capture sequence, with local
ingress settlement before native submission. A permit covers a whole effect, including click
press/release and pointer queries; existing started prefixes remain ordered. Waiting retirement
reports not sent; panic after start is unconfirmed. Ordering releases before report disposal and
blocked provider cleanup, while original source and IO lifetime quotas remain held.

Completed output reports precede independent readiness fences. Target/settings preconditions
reject stale queued reports; runtime guards reject stale readiness while preserving completed
prefixes. Disabled worker wakes drain accepted receipts. Input service no longer writes output
readiness or shared connection state, and input restart clears its prior catalog before reopening.
Success logging respects the existing flag. Native input and output (including direct provider
calls) are disabled in test builds. See [Mouse ownership](../architecture/mouse-runtime.md) and
[Keyboard ownership](../architecture/keyboard-runtime.md).

Mouse was the final consumer of the old module-command request JSON relay. Its unused topic/DTO,
serializer/decoder and actor-side execute-overlay helpers are removed; worker snapshot views remain.
Authored command choice IDs now match their existing canonical defaults and backend parser values;
module choice labels use the existing typed device-options builder. A first 63-case run passed 60
and exposed three enum constraint failures; corrected declarations and one added editor-choice
regression pass in the final 64-case suite.

Final Windows/default-feature checks use `CARGO_INCREMENTAL=0`:

- Strict workspace/all-target Clippy passed in 31.35s; `cargo check --locked --workspace --tests`
  passed in 26.31s.
- `cargo test --locked -p Chataigne2 --bin Chataigne2 -- --test-threads=1 app::module_modules_controllers_mouse app::module::common::desktop_output`:
  **64 passed** (61 Mouse / three relocated shared cases), in 3.40s after a 3m27s build.
  Thirty new Mouse cases cover all command/script variants, movement modes, signed scroll axes,
  editor enum choices, original snapshots/last overrides, partial and ordinary dispatch order,
  removed producers, 64-source and byte pressure, retained/queued source quotas, cancellation,
  blocked setup/cleanup, panic/error outcomes, compound click ordering, disabled completion,
  independent report/readiness prefixes, eight send reconfigurations and idle service.
- Exact artifact scopes were listed and checked disjoint: **61 Mouse / 181 neighboring controllers /
  1081 preservation**, with **1596 listed and 273 outside scope**. Neighbors passed **70 Keyboard +
  71 Joy-Con + 40 Buttplug = 181**, in 13.38s. Preservation passed **1075 / zero failed / six existing
  ignored**, in 82.84s with builds idle. Combined: **1317 passes / six ignored / 1323 selected**.
  The relocated shared cases are in preservation and are counted once. Artifact hash stayed unchanged;
  no timing threshold changed.
- Native `cargo build --locked -p Chataigne2 --bin Chataigne2` passed in 2m42s. Actual root/Core
  Cargo fmt, explicit app Rustfmt, documentation formatting and diff checks passed; all 24 tested
  live source hashes remain unchanged. No Golden API, generated protocol, dependency, project codec
  or app-shell startup change. Prior Core/Golden IO qualification remains dated.

Inventory: **2067 source files / 32 oversized**, using Rust/TS/TSX/Svelte/JS/MJS under `apps`,
`crates`, `packages`, `xtask`, including generated files. Every touched source is below 1000 lines;
Mouse root shrinks from 909 to 783 lines. No desktop input was captured or injected. Physical/platform
and product-scale gates retain P17/P18 and P16 ownership.

**Next within C09:** worker-prepared input callbacks/catalogs, retained publication quotas and
queued source/target/settings fences, then combined pressure/reconfiguration/failure recovery.
The output-only checks do not close those input or combined acceptance items.

Final SHA-256:

- App `target/debug/deps/Chataigne2-b14ffd1b9e52f9ac.exe`:
  `f73cd31c0f75ad801be0bca81de9246c06e7a403ba6c8955ebeae161b45f981a`.
- Native `target/debug/Chataigne2.exe`:
  `70c47d67c54cb637502206093756ec1755cdc1ee4635e66984eb723bf423ba16`.

## C09 Mouse prepared input and local closure

Implemented 2026-10-23 after `7e647d66`, continuing its unfinished prepared-input work on `main`.
**C01-C09 are locally closed: nine of twelve packages (75%). Three remain (C10-C12), covering
five families.** Continue Ultraleap. P03 scope coverage advances to approximately 96%; whole-plan
coverage remains approximately 16%. These are scope scores, not remaining-effort forecasts.

The Mouse IO owner now prepares device matching, immutable pointer/button state, callback JSON,
messages and shared choice domains. Motion admits two callback candidates; button edges admit four
atomically, covering both observed output-readiness states. Callback, domain and catalog metadata
quotas follow original shared storage through queued edits, history, exported Arcs and disposal.
Pressure pauses provider polling and retries the same staged occurrence. A worker-owned value-only
delta reset removes the actor's short timer. See [Mouse ownership](../architecture/mouse-runtime.md)
for the exact pool, mailbox, frame and native limits.

Actor/native/configuration guards fence each publication; authored settings and leaf/ancestor
guards precede the complete bounded turn. Queued work retains its original source through
application, rejection or disposal. Missing snapshots preserve pending rows. Input and output
prefixes precede independently fenced final readiness. Healthy combined service has no recurring
actor callbacks, snapshots or wakes; stopped owners recover with bounded absolute backoff.

Selection changes without capture advance a revision without reopening the native endpoint.
Capture-mode and captured-device changes retire mismatched owners. Notifications and worker service
share configuration reconciliation, preserving matching owners and values already received from
them. Notification guard preludes precede owned warning edits, so missing-device feedback cannot
invalidate its own settings reads. These repairs also preserve Keyboard notification behavior.

The public `Edit::SetParamEnumOptions` gains an optional atomic `selected_value`. UUID/current-value
guards still skip stale results; disjoint old/new domains and normalized selections validate and
apply together, preserving identity, metadata, events and reversible history. Failed validation
preserves both fields. Keyboard and Mouse use this for their existing narrow persisted-ID
normalization; other callers retain choice-only behavior. No dependency, wire DTO, project codec,
UI policy or app-shell startup change. See [application contracts](../architecture/application-contracts.md).

Final Windows/default-feature gates use `CARGO_INCREMENTAL=0`:

- Strict workspace/all-target Clippy passed in 41.87s; workspace/test compilation passed in 35.22s.
- Core: `cargo test --locked -p golden_engine --lib -- --test-threads=1` passed **602 / zero failed /
  three existing ignored / 605 listed**, in 1.09s after a 7m40s build. New atomic-domain/history and
  invalid-update cases pass; stale publication also skips an invalid proposed selection.
- App owning gate: `cargo test --locked -p Chataigne2 --bin Chataigne2 -- --test-threads=1 app::module_modules_controllers_mouse app::module::common::desktop_output app::module_modules_controllers_keyboard`
  passed **161** in 14.00s after a 5m01s build: **87 Mouse / 71 Keyboard / three shared**. New Mouse
  coverage includes callback/domain pressure and exported lifetimes, 512 ordered edges, eight
  selector revisions, passive recovery, blocked cleanup, queued rename/reparent/settings/retirement,
  retained rejected-source disposal, missing-device and persisted-ID publication, fresh values
  across delayed notifications, and combined input/output pressure and independent prefixes.
- Native `cargo build --locked -p Chataigne2 --bin Chataigne2` passed in 3m20s. The final artifact's
  listed scopes are disjoint: **87 Mouse / 182 neighboring controllers / 1081 preservation**, with
  **1623 listed / 273 outside scope**. Keyboard is already covered by the owning gate; the other
  **111 Joy-Con/Buttplug cases passed** in 7.42s. Preservation finally passed **1075 / zero failed /
  six existing ignored**, in 82.34s with builds idle. Shared ordering cases are counted once:
  **1344 unique app passes / six ignored / 1350 selected**.
- The initial preservation run passed 1073 but failed two unchanged multiplex timing gates
  (active average 6357us; production average 6989us). Both passed in isolation; the complete
  preservation rerun then passed on the same artifact. No fixture, threshold or implementation
  changed between these runs. Background Code CPU use was observed; it does not establish a cause.
  Keep this timing variance visible rather than attributing an isolated pass to the original run.
- Actual root/Core Cargo fmt, explicit app Rustfmt, documentation formatting and diff checks pass.
  All 31 tested live Rust source hashes remain unchanged through final formatting. Inventory:
  **2074 source files / 32 oversized**, with every touched source below 1000 lines. Golden IO is
  unchanged; its prior 57-pass result remains dated.

No physical mouse/keyboard capture or injection was operated. Physical/platform and product-scale
qualification retain P17/P18 and P16 ownership. Keep the full P00-P18 implementation goal active.

Final SHA-256:

- App `target/debug/deps/Chataigne2-b14ffd1b9e52f9ac.exe`:
  `25449c293ad485a2fbcf04eaaa41549dced9600c9934e5c0e7e3c96f64469235`.
- Native `target/debug/Chataigne2.exe`:
  `4e2840849302ddb071c0e52178a9a173da469e8e5b0d3452608edb74f31f0434`.

## C10 Ultraleap local closure

Implemented 2026-10-23 after `2e3bc6db`, starting clean on `main`.
**C01-C10 are locally closed: ten of twelve packages (83%). Two remain (C11-C12), covering
four families.** Continue Gamepad/Kinect2/Stream Deck, then Spatializer. P03 scope coverage is
approximately 97%; whole-plan scope coverage rounds to 17% under the existing equal-weight rubric.
Neither score forecasts remaining effort. No new closure package or generic prerequisite is added.

Ultraleap no longer waits for SDK startup, drains an unbounded channel or joins a thread on the
actor. Its app-owned IO runtime uses public Golden admission, lifetime, payload, wake and recovery
primitives. Four process lifetimes and one per-module lifetime include blocked setup and cleanup.
Actor retirement fences queued publications immediately; worker failure becomes observable before
provider cleanup. The original source reservation survives draining, cloning, queued application,
rejection and disposal. Source/mailbox pressure retains the same occurrence and pauses polling.

The source pool admits 128 lifetimes / 2 MiB, charging 16 KiB per publication. The mailbox admits
32 rows / 512 KiB; provider and actor turns admit at most 16 rows. Consecutive native poses
coalesce within that provider turn only while service/device and hand-presence state match;
status messages and presence transitions preserve ordered boundaries. Native SDK calls and waits,
device enumeration, frame conversion, finite/count validation, distance and tracking messages run
on IO owners. Device enumeration admits at most 64 entries before allocation; hand traversal
admits two hands and rejects invalid/null/duplicate identity data. Message backing storage admits
1024 bytes. Existing LeapC bindings and dynamic library locations remain in use.

The actor projects prepared typed state behind module/leaf/ancestor and actor/native source guards,
retaining the original source through application or rejection. Missing snapshots keep input queued.
Healthy idle service has no periodic actor updates, snapshots or wakes. SDK service-unavailable
responses use a 250 ms native retry; startup, failure and full lifetime capacity use absolute
actor recovery deadlines with 250 ms to five-second backoff. Disable/destroy retire the source;
enable requests a bounded replacement. Existing parameters, base functions/callbacks, empty
module-specific command/method registries and app-owned template remain unchanged. See
[Ultraleap ownership](../architecture/ultraleap-runtime.md).

Final Windows/default-feature gates use `CARGO_INCREMENTAL=0`:

- Strict workspace/all-target Clippy passed in 39.23s; `cargo check --locked --workspace --tests`
  passed in 30.02s.
- `cargo test --locked -p Chataigne2 --bin Chataigne2 -- --test-threads=1 app::module_modules_controllers_ultraleap`:
  **24 passed**, in 2.13s after a 4m18s build. Twenty new cases cover blocked open/cleanup,
  bounded retained and mailbox pressure, SDK decode/count/finite limits, prepared coalescing and
  status boundaries, queued path/source/disable rejection, original-source disposal, missing
  snapshots, 512 ordered status applications with unrelated actor progress, eight enable/disable
  cycles, idle service and failure recovery. The first run passed 17 and exposed seven fixture
  path lookup failures; fixtures now use stable declaration IDs. Ordered application is observed
  at the engine boundary rather than through the UI's latest-value retention.
- Native `cargo build --locked -p Chataigne2 --bin Chataigne2` passed in 2m54s.
  Final artifact scopes are disjoint: **24 Ultraleap / 269 controllers / 1081 preservation**;
  **1643 listed / 269 outside scope**. Controller preservation passed **87 Mouse + 71 Keyboard +
  71 Joy-Con + 40 Buttplug = 269**, in 21.30s. Broader preservation passed **1075 / zero failed /
  six existing ignored**, in 84.50s with builds idle. Total: **1368 unique app passes / six ignored /
  1374 selected**. Shared ordering cases are included once in preservation. Artifact hashes remain
  unchanged across owning and preservation runs; no timing threshold or fixture was relaxed.
- Actual root/Core Cargo fmt, explicit app Rustfmt, documentation formatting and diff checks pass;
  all 15 tested live Rust source hashes remain unchanged through final formatting. Inventory:
  **2086 source files / 32 oversized**, with every touched source below 1000 lines. Core, Golden IO,
  Golden UI, dependencies, generated protocol, project codec and app-shell startup are unchanged.
  Prior Core **602 passes / three ignored** at `2e3bc6db` and Golden IO 57-pass evidence remain dated.

Tests use fake providers and in-memory SDK records; native SDK endpoints cannot open in test builds.
No tracking service or physical device was operated. Vendor SDK internal queues/memory, physical
service/device behavior, cross-platform builds and product scale remain unqualified here and keep
P17/P18 and P16 ownership. The complete P00-P18 goal remains active.

Final SHA-256:

- App `target/debug/deps/Chataigne2-b14ffd1b9e52f9ac.exe`:
  `1d7e368530c7de8f8f6b3c6e508004985c71c3f5f0721aa1a3e0ea9af5a4e329`.
- Native `target/debug/Chataigne2.exe`:
  `09c31b0500601afd85af55dcede491cb272fc1e436804fea617e1cd868a271aa`.

## C11 Gamepad local qualification

Implemented 2026-10-23 on `main`, based on delivered `d59a8f2d`. Gamepad is one of C11's three
families; its delivery does not close C11. Kinect2 and Stream Deck remain in C11, followed by
C12 Spatializer. The fixed twelve-package denominator and whole-plan objective remain unchanged.

Gilrs setup, polling, discovery, selected-state queries and destruction now belong to bounded
app-owned workers. Preparation owns calibration, D-pad synthesis, immutable choices, callback
JSON and messages. Actor projection is passive, drains at most sixteen rows, and retains original
source/domain/callback quotas through queued application or rejection. Configuration revisions
cancel stale input without reopening the native owner or waiting for free callback capacity.
Queued target/ancestor/selection/calibration/disable/retirement guards reject stale values and
callbacks. Failure becomes observable before blocked cleanup; retries use absolute backoff.
See [Gamepad ownership](../architecture/gamepad-runtime.md) for bounds and responsibilities.

Current Windows/default-feature gates use `CARGO_INCREMENTAL=0`:

- Strict workspace/all-target Clippy passed in 37.66s; workspace test compilation passed in
  30.33s.
- `cargo test --locked -p Chataigne2 --bin Chataigne2 -- --test-threads=1 app::module_modules_controllers_gamepad`:
  **34 passed**, in 3.02s after a 3m57s build. These include the five preserved cases and 29 new
  cases for blocked setup/cleanup, full mailbox, distinct item/byte pressure, choice/callback
  pressure, original quota retention, finite/backing limits, 512 ordered callbacks with unrelated
  actor progress, queued path/settings/source rejection, eight configuration and enable/disable
  cycles, passive idle, recovery, held-state calibration, missing selection and disconnect/reconnect.
- The initial run passed 28/31; three fixture failures used insufficient catalog lifetime bytes or
  attempted an enum value without its restored missing choice. Corrected fixtures passed 33/33.
  Review then found that multiple queued catalogs could skip their final choice domain after
  blank-to-Auto normalization. The service now tracks that value through the turn; the added
  regression verifies the final domain's canonical storage. No production bound or timing
  threshold was relaxed.
- Actual root/Core Cargo fmt and explicit app Rustfmt pass; all fourteen tested live Rust hashes
  remain unchanged through formatting. The Gamepad entrypoint is 409 lines and every touched
  source is below 1000 lines. Core, Golden IO/UI, dependencies, protocol and startup are unchanged.
- Native build passed in 3m06s. Disjoint controller preservation passed **293 / zero failed**
  in 23.52s. The artifact lists **1672 tests**, with disjoint **34 Gamepad / 293 controllers /
  1081 broader preservation**, leaving 264 outside these scopes.
- Initial broader preservation passed **1074 / one failed / six existing ignored** in 89.95s.
  Multiplex active-runtime timing averaged **5142us** against its unchanged **5000us** limit.
  The isolated case passed on the same artifact with **4543us average / 6324us p95 / 7278us p99**,
  no deadline misses. Multiplex test sources and its fixture are unchanged; the fixture contains
  no Gamepad. Complete broader preservation rerun passed **1075 / zero failed / six existing
  ignored**, in 85.05s. Builds were idle; app and native hashes remained unchanged. Final unique
  selected evidence is **1402 passes / six ignored / 1408 selected / 1672 listed / 264 outside**.
  Repeated timing sensitivity remains an evidence limit; this result does not qualify P16 scale.

Tests use controlled providers; native Gamepad endpoints cannot open in test builds. Physical
device/platform behavior, Gilrs' private queues and product-scale qualification remain with their
later plan owners. The complete P00-P18 goal remains active.

Owning app SHA-256:
`d499f1b9af09b074d1289deed94558f579e2cbfbb630dd677d4badca22c824fb`.
Native SHA-256:
`84e3333f77938e173e2ee57b393151bb37a7b3a7dbc5abafea31b891c70c5dfb`.

## C11 Kinect2 local qualification

Implemented 2026-10-23 on clean `main` at delivered `cf7b45b5`. **Kinect2 is locally qualified**,
joining Gamepad within C11. C11 remains open for Stream Deck; C12 Spatializer follows. Two
unfinished families remain under the same two packages. No package is added.

Native SDK loading, sensor/reader calls, body collection and cleanup now belong to bounded
app-owned IO lifetimes. Preparation owns the 25-joint transform, nearest-body selection, hand
metrics, SDK-timestamp speed history, tracking messages and existing noise threshold. Reference
revisions reproject held raw frames without reopening the sensor; the actor is passive and
projects behind source/settings/target/ancestor guards. Missing snapshots leave input queued.
See [Kinect2 ownership](../architecture/kinect2-runtime.md).

The Windows adapter now owns all six returned body references before fallible inspection,
preventing an early body error from leaking unvisited references. Fake COM tests cover that
failure path. Fixed native joint decoding validates identities, state and finite positions;
signed SDK timestamps reject negative values before conversion. Native endpoints are disabled
in test builds. Existing script/base callbacks and the empty custom command/method surfaces remain.

Current Windows/default-feature gates use `CARGO_INCREMENTAL=0`:

- Strict workspace/all-target Clippy passed in 38.92s; workspace test compilation passed
  in 31.97s.
- `cargo test --locked -p Chataigne2 --bin Chataigne2 -- --test-threads=1 app::module_modules_controllers_kinect2`:
  **36 passed**, in 3.95s after a 3m46s build. The four existing cases remain, including real
  worker/engine reference-space and hand-metric projection. Thirty-two new cases cover blocked
  setup/poll/cleanup, bounded mailbox/item/byte pressure, original source retention, 512 ordered
  tracking transitions with unrelated actor progress, queued source/path/settings/disable
  rejection, eight reference and enable/disable cycles, sensor loss/recovery and passive idle.
  Three cases use fake Windows COM objects or SDK records to verify complete body-reference
  cleanup, joint decode bounds and signed timestamp conversion.
- The initial 34-case run passed. Review then strengthened source-revision coverage: rapid
  reference changes under a full mailbox and cancellation of an unprojected tracking-loss
  snapshot. Prepared tracking messages now carry current snapshot state, preserving acquired
  and lost status through cancellation; unchanged messages use normal parameter no-op behavior.
- Actual root/Core Cargo fmt and explicit app Rustfmt pass. Seventeen tested live Rust hashes
  remain unchanged; every touched source is below 1000 lines (largest: 750). Core, Golden IO/UI,
  dependencies, protocol, persistence and app-shell startup are unchanged.
- Native build passed in 2m26s. The artifact lists **1704 tests**, with disjoint
  **36 Kinect2 / 327 controllers / 1081 broader preservation**, leaving 260 outside these scopes.
  Controller preservation passed **327 / zero failed** in 26.70s. Broader preservation passed
  **1075 / zero failed / six existing ignored** in 83.76s with compilation idle. Final unique
  evidence is **1438 passes / six ignored / 1444 selected / 1704 listed / 260 outside**. App and
  native hashes remained unchanged across owning and preservation runs.

Owning app SHA-256:
`cd19be6625663c73166c8a7d68e7e69bcb1884a4d14f9f30e7fe0c2d8f5801d9`.
Native SHA-256:
`200e51950285ddb4ece4100c10d1a2e894608e3348448eb5cf7b71816091c429`.

Remote evidence for delivered `cf7b45b5`: [benchmark workflow 40005299037](https://github.com/Golden-Geek/Chataigne2/actions/runs/40005299037)
failed at evidence comparison because the historical baseline explicitly lacks a qualified CPU,
toolchain, profile, feature, raw-sample and runner-image fingerprint. Tick and dispatch benchmark
commands succeeded; **the regression comparison did not run**. This is P16 baseline qualification
work, not evidence of a measured Gamepad regression. Do not replace or waive the baseline to make
the gate green. CI and cross-platform product qualification were still running when observed.

### Follow-up remote evidence for Kinect2 delivery

Observed 2026-10-23 for delivered `4a3e6638`: [CI 40010170344](https://github.com/Golden-Geek/Chataigne2/actions/runs/40010170344)
completed with a Linux Golden Audio failure. Its platform audio test command reported 168 passes
and one failure: `qualification::tests::device_soak::null_backend_soak_advances_signal_and_completes_planned_recovery`
rejected "1 backend runtime warnings were observed" at `device_soak.rs:70`. This is an observed
failure, not a qualified timing flake or a passed platform gate. Audio/P16 qualification must
investigate the warning; the assertion and acceptance criteria remain unchanged. macOS audio
and the UI artifact jobs passed. Cross-platform product qualification 40010170429 was still live.
Benchmark workflow 40010170426 also failed its outcome gate; this entry does not claim a valid
regression comparison or supersede the unqualified-baseline evidence above.

## C11 Stream Deck local qualification

Implemented 2026-10-23 on `main`, based on delivered `4a3e6638`. Stream Deck qualifies C11's
third family, joining Gamepad and Kinect2. **C01-C11 are locally closed: 11/12 packages (92%);
only C12 Spatializer remains.** P03 becomes ~99% scope coverage; the whole plan stays ~17%.
This is local qualification; physical-device, platform and product-scale acceptance remain open.

Discovery, connection, HID input/output, brightness, image loading/decoding and cleanup now belong
to bounded native owners. Passive projection retains original input/frame/domain quotas and queued
source/target guards. Ordered inputs survive pressure while latest visual feedback continues.
Read/render failure invalidates and notifies before cleanup; cleanup retains its lifetime slots.
Coarse discovery/retry deadlines replace the actor's 60 Hz hardware/page loop.

Page authoring captures bounded immutable templates at invocation. Layout preparation scans bounded
metadata slices, builds one page per turn, preserves existing parameter UUIDs/values and uses detached
subtrees for missing structure. Edit application or rejected-suffix disposal wakes its next turn.
Script simulation requires no native owner. Typed enum edits preserve selected missing serials and
parameter identity. The unused common deck implementation and unbounded paging routines are removed.
See [ownership and limits](../architecture/streamdeck-runtime.md).

Current Windows checks use `CARGO_INCREMENTAL=0`:

| Gate | Current result |
| --- | --- |
| Strict workspace/all-target Clippy | Passed, 51.06s; hardware-feature Clippy also passed during qualification. |
| Default owning unit cases | **37 passed**, 11.73s after the final 4m36s rebuild. The preceding same-source run passed 37 in 11.92s. |
| Hardware-feature owning unit cases | **43 passed**, 12.01s after a 5m04s build; native endpoint construction is disabled in tests. |
| Default standalone production build | Passed, 2m45s. |
| Hardware-feature production linking | Passed, 3m39s; separate from the guarded test constructor. |
| Controller preservation | **363 passed**, 30.62s on the final artifact. |
| Broader preservation | **1075 passed / six existing ignored**, 83.26s on the final artifact. |
| Formatting/source bounds | Actual root/Core Cargo fmt and explicit app Rustfmt; 18 tested Rust hashes unchanged, every source under 1000 lines. |

Owning cases cover blocked setup/poll/cleanup; item, byte, mailbox, callback and domain pressure;
512 ordered edges; original-frame retention through shutdown/disposal; eight visual and enable
cycles; queued physical/script source/path/retirement rejection; empty-mailbox pressure warnings;
device loss/recovery; passive idle without actor callbacks/snapshots; canonical template capture;
all five models; UUID/value preservation; and 64-page preparation with unrelated edits between
bounded turns. Feature-only cases cover native-constructor exclusion and image file, dimension,
decoded-allocation and alpha-compositing bounds. Existing script templates/callbacks, model resizing,
page mirrors, active-page input and Unpaged appearance remain covered.

Qualification repaired two failures. The first 33-case run had 32 passes and a harness failure:
the 64-page fixture inspected pending work before dispatching its structure notification. The next
34-case run had 33 passes and found a real XL regression: preflight mixed materialized node storage
into the raw 256 KiB template allowance. Raw and materialized storage now have separate admission
within the unchanged 1 MiB authoring lifetime budget. Later hardware runs passed 42 and 43 cases;
no cases or limits were waived to obtain those results.

Preserve all multiplex timing samples against the unchanged 5000 us development budget:

| Artifact / selection | Average us | p95 us | p99 us | Max us | Result |
| --- | ---: | ---: | ---: | ---: | --- |
| Earlier candidate, broad run 1 | 5187 | 7597 | 8524 | 9537 | 1074 pass / one fail / six ignored; 85.96s |
| Earlier candidate, isolated case | 4192 | 6152 | 6483 | 6842 | One pass; 1.18s |
| Earlier candidate, broad run 2 | 5932 | 8631 | 9196 | 9771 | 1074 pass / one fail / six ignored; 91.33s |
| Delivered `4a3e6638`, isolated checkout | 5005 | 6115 | 6854 | 7793 | 1074 pass / one fail / six ignored; 82.97s |
| Final rebuilt candidate, broad run | 4892 | 6070 | 6157 | 6321 | 1075 pass / six ignored; 83.26s |

Each measured case recorded zero deadline misses, snapshot/provider rebuilds and rejected
actions/intents. Both checkouts used fixture SHA-256
`5ebd05f9390462b5d666c6b54e833bf30f97d6ad2b391146c288861202f09390`; the fixture contains
no Stream Deck nodes. The detached baseline used the same pinned toolchain, default features and
optimized test profile, reusing prebuilt UI through supported `GC_UI_ASSUME_BUILT=1`; this
CPU/engine fixture does not mount UI. Its build passed in 4m49s. The shared-target baseline build
replaced Cargo's default test-binary filename; the implementation source remained frozen, and its
current artifact was rebuilt and its owning/full-preservation selections rerun. These observations
show the gate also fails before this change. They do not establish stable 200 Hz performance or
rule out an incremental performance change. P16 retains that investigation; thresholds, fixtures
and implementation were unchanged between candidate runs.

The default unit artifact lists **1729** cases with disjoint **37 / 363 / 1081** selections:
**1475 unique passes / six ignored / 1481 selected / 248 outside**. The feature artifact lists
**1735** cases and adds exactly six feature-only owning names; its other 37 owning names overlap
the default run. Across both artifacts the union is **1481 unique passes / six ignored /
1487 selected / 1735 listed / 248 outside**. No physical endpoint or desktop automation was used.

Final default unit SHA-256:
`75cc13d5d27507347c884196e36a6d882b831b1d67a8613516ae7da8e75347a3`.
Hardware-feature unit:
`0d3c72fdce912aecaecfb13ca42968c228798031a5051d0c325fc0eee6ed18f6`.
Standalone default:
`756bef659bec7678915d881f6a2f5c864a9d15d1b99f4536767cb466c7da6ccc`.
Hardware-feature executable:
`fab8d46572c935bf62b22c529afe0df5484ff1f61686c53f4aa74becccba611a`.
Earlier candidate unit:
`a02b4e2180c1dc3d6568b064e4666f6d2b545c1770b1edca2f2eec3f3543aa38`.
Detached baseline unit:
`474f01f30ff7e4b98aa561fcb8b26f6bd9f8f69876a128a2a4bf189138c49009`.

### Prior-commit product qualification follow-up

Completed Linux job [120609653347](https://github.com/Golden-Geek/Chataigne2/actions/runs/40010170429/job/120609653347)
from the prior Kinect2 product workflow also failed. Its gates reported Cargo advisories, unused
dependencies, the reviewed duplicate-version baseline, npm production audit and workspace tests.
The app test summary was **1682 passed / six failed / ten ignored**. Failures were Mouse combined
native-service recovery, Serial partial catalog-suffix reconciliation, Alchemist catalog materialization,
hosted curve and gradient resource edits, and processor property order. RustSec IDs observed by that
job were `2026-0285`, `2026-0346`, `2026-0348`, `2026-0349`, `2026-0381`, `2026-0382`, `2026-0383`
and `2026-0386`. Dependency repairs belong to P06, structural/dependency gates to P12, and the
platform/product qualification to P16/P17; these are not local passes or grounds to waive gates. Compatibility builds for Linux ARM32,
Linux ARM64 and Windows ARM64 passed; Windows and macOS product jobs were still running.

The prior Kinect2 product workflow subsequently completed with failure: Windows and macOS product
jobs were cancelled, the three compatibility builds passed, and aggregation failed its schema-v1
report gate. This is the final observed state of run 40010170429, not a completed platform pass.

## C12 Spatializer closed: P03 locally complete

2026-10-24, based on delivered `9561a399` on `main`. **C12 and P03 are locally complete.**
This is the last fixed P03 closure package; no additional package or generic optimization chain
has been introduced. Whole-plan scope coverage remains ~17%; P03 is 100% locally, with 12/12
packages closed and 3/18 implementation/qualification tasks locally complete. Continue P04.

Environment: Windows 11 Home build 26300, Intel Core Ultra 9 275HX (24 logical processors),
Rust 1.97.0 `2d8144b78`, Cargo 1.97.0 `c980f4866`, x86_64 MSVC. Owned Cargo commands used
`CARGO_INCREMENTAL=0`. App tests used default features and the optimization-level-3 test profile;
Core's final suite used the explicitly recorded development profile. No physical endpoint or
desktop automation was used.

Spatializer now owns passive, admitted preparation and receipt-driven matrix publication under
`apps/chataigne/src/module/modules/generators/spatializer`. Geometry runs on a bounded worker;
callbacks normalize one endpoint, service 16 existing pairs, or prepare 32 detached child nodes
per turn. The existing 512-source/1,024-target mathematical fixture is preserved. Counts, retained
bytes, worker slots, queued edits and detached rows are admitted before construction. Idle modules
release prepared storage. See the [ownership contract](../architecture/spatializer-runtime.md)
for exact limits, identity rules and the separate P09/P16 scale boundary.

Two concrete Golden contracts support this package: an explicit whole-subtree read fence rejects
descendant changes before their callbacks run; move routing also notifies the former parent's
membership ancestry without duplicating shared recipients. Cold subtree fences may rebuild an
ordinary snapshot. These changes do not qualify full-product snapshot or interaction performance.

Qualification retained the failures that drove repairs:

- First owning run: **six passed / seven failed**. Control lookup used a generated short name for
  `value_layout`; it now uses the public declaration-ID lookup.
- Expanded run: **18 passed / eight failed**. The async harness could return while publication was
  active, invalid configuration retained normalization state, and reload had not activated the
  runtime. The harness now waits for true quiescence, rejection clears pending preparation, and
  reload uses `prepare_engine_for_runtime`.
- Next run: **26 passed / three failed**, 2.45s after a 5m45s optimized build. Rejected/settled state
  could outlive its input revision, and a moved row exposed the former-parent routing gap. Both
  owning fixes and a Core move/removal regression case are now in the final validation candidate.
  The 3-by-97 callback sample used 172 turns, p95 **34 us**, maximum **73 us**, excluding snapshot
  construction and edit application; it is not a whole-product latency result.
- Core's first optimized suite: **605 passed / one failed / three ignored / 609 listed**, 1.45s
  after an 8m48s build. All subtree guards passed. The new routing fixture used non-observing
  `Folder` nodes; its corrected version uses observing parameters and a subscription to exercise
  shared-recipient deduplication. Runtime routing code is unchanged by that fixture correction.
- Corrected Core suite: **606 passed / three ignored / 609 listed**, 1.22s after an 8m41s build,
  using `cargo test -p golden_engine --profile dev --lib -- --test-threads=1` (development profile,
  optimization level 1, incremental disabled). All functional cases passed; this is not a Core
  performance qualification. Artifact SHA-256:
  `72213001f9160659b62a921b5bcc9540749a4c6be1b7bea363a1bb28757e6ab3`.
- Final owning app run: **29 passed**, 2.58s after a 4m38s optimized build;
  `cargo test -p Chataigne2 --bin Chataigne2 spatializer -- --test-threads=1 --show-output`.
  Strict workspace/all-target Clippy also passed in 1m26s. App unit artifact SHA-256:
  `73baf23e37decbff286d5b958463182eb1a10452bca788cd703895221bd74688`.
- Controller preservation passed **400 cases**, 45.51s. The first broader selection reported
  **1073 passed / two failed / six ignored**, 85.18s. Both failures were multiplex timing gates:
  active average **6368 us**, p95 **12223 us**, maximum **25177 us**, 28 deadline misses;
  production average **8614 us**, p95 **15999 us**, maximum **31258 us**, 57 deadline misses.
  Active snapshot/provider rebuilds and rejected actions/intents remained zero.
- Separate processes on that same unchanged executable passed both timing cases: active average
  **3762 us**, p95 **5392 us**, maximum **6287 us**, zero deadline misses (1.06s); production
  average **4862 us**, p95 **6439 us**, maximum **7209 us**, zero deadline misses (1.32s).
  The full-selection comparison then passed **1075 cases / six ignored**, 84.12s. Active average
  was **3493 us**, p95 **4903 us**, maximum **5701 us**; production average **5296 us**, p95
  **6475 us**, maximum **8421 us**. Both recorded zero deadline misses; active snapshot/provider
  rebuilds and rejected actions/intents remained zero. No source, fixture or threshold changed.
  These observations do not qualify stable 200 Hz behavior or exclude an incremental regression;
  retain the earlier delivered-baseline failures and the P16 timing owner.

The current default artifact lists **1745** cases. Its disjoint selections are **29 Spatializer /
400 controllers / 1081 broader preservation**: **1504 unique passes / six existing ignored /
1510 selected / 235 outside**. Compilation was idle during preservation. Standalone default-feature
production linking passed in **2m44s** with `cargo build -p Chataigne2 --bin Chataigne2` and
incremental disabled. This compiles non-test paths with default ASIO/JACK/realtime support; the
executable was not launched and no physical endpoint was operated. Standalone SHA-256:
`b5fcea031dbdea72758c7f2fc84b006e3f85a2c3cb4f4be5b35b4e1a85625954`.

Final Core tests, owning tests, preservation, strict Clippy, production linking and formatting
pass. Actual root and Core `cargo fmt` plus explicit app `rustfmt` leave all **15** Rust source
hashes unchanged; the largest touched source is **866 lines**. `git diff --check` passes.
The owning checks cover count/byte admission, four process-wide and one per-module blocked
worker slots, latest-input recovery, failure, teardown, source/destination fences, applied prefixes,
detached construction, stable identities/values, empty collections, callbacks, persistence activation
and zero idle polling/snapshots. **C12 closes locally; all C01-C12 packages and all seven P03
workstreams are locally complete.** Commit/push this verified milestone, then continue P04.

### Delivered Stream Deck CI follow-up

All observations below refer to **prior commit `9561a399`**, not the Spatializer candidate.
CI [40027958206](https://github.com/Golden-Geek/Chataigne2/actions/runs/40027958206) finished with
failure: every product build, UI job, Core tests and Windows/macOS audio host job passed; Linux
audio failed its null-backend soak with **168 passed / one failed** and
`1 backend runtime warnings were observed`. This remains a failed gate, not a waived assertion.

Benchmarks [40027958251](https://github.com/Golden-Geek/Chataigne2/actions/runs/40027958251) ran
tick/dispatch commands and evidence tests successfully, but comparison did not run because the
historical baseline lacks a qualified fingerprint. P16 retains that work.

Product workflow [40027958198](https://github.com/Golden-Geek/Chataigne2/actions/runs/40027958198)
finished with failure. Windows reported **1713 passed / six failed / ten ignored**: Mouse combined
native recovery, Serial inflight catalog selection/warning projection, three Alchemist catalog/
hosted curve/gradient cases and processor property order. Linux reported **1708 passed / five
failed / ten ignored**: Serial partial catalog suffix reconciliation and the same four Alchemist/
processor cases. macOS was cancelled; the three ARM compatibility builds passed. The schema-v1
aggregate gate failed. Dependencies remain P06, structural gates P12, timing P16 and product/
platform qualification P17. These dated failures neither close those tasks nor expand P03.

## P04 validated scalar decoding

2026-10-24, based on delivered `d8fbb3d9`, clean and matching `origin/main` at milestone start.
The owning layer is reusable `golden_audio`; no app or UI policy moved into it. `SampleRate`,
`FrameCount` and `GainDb` now implement deserialization through their checked constructors while
retaining transparent numeric serialization. Invalid nested configurations and command gains fail
before construction; JSON and non-JSON Serde sources cannot bypass finite/range/nonzero invariants.
Valid persisted values keep the same wire representation, so no DTO/generator shape changed.

Four added cases cover scalar limits, numeric round trips, non-finite gains through a raw Serde
floating-point deserializer, nested engine configurations, and invalid gain commands. These checks
exercise the boundary rather than patching render division or gain-conversion call sites.

Validation on Windows x86_64 MSVC with Rust/Cargo 1.97.0 and `CARGO_INCREMENTAL=0`:

- `cargo test -p golden_audio --lib tests:: -- --test-threads=1`: **173 passed**, 1.85s after a
  20.27s build; the filter selects the complete unit inventory, whose names all include `tests::`.
- `cargo test -p golden_audio -- --test-threads=1`: default unit and public integration suites passed.
- `cargo test -p golden_audio --no-default-features --lib -- --test-threads=1`: **129 passed**, 0.14s.
  Those cases overlap the default-feature inventory and are not 129 additional unique tests.
- `cargo clippy --workspace --all-targets -- -D warnings`: passed in 54.61s, including app consumers.
- Actual root/Core formatting and final diff/source checks pass. No device stream or GUI was operated.

Default unit artifact SHA-256:
`f0485bc3ae10e03b31c5cb447f0b6a029b9ebe476d47d8239a116525cb7aa134`.
No-default unit artifact SHA-256:
`a9f2a679b5d120a4ab87fc6feaa2f8a5c5f91b0a602e5c50e68704c3c262cbc8`.

**P04 is 25% complete (one of four fixed acceptance checkpoints).** Command sequences currently
allocate before the ingress producer lock, and managed plan/gain delivery still uses independent
queues with render-thread UUID lookup. Continue the shared admission/order and dense generation
barrier, then qualify production transitions with allocation/deallocation guards. Whole-plan scope
coverage becomes ~18%; P01-P03 remain the three locally complete tasks. Commit/push this verified
milestone before editing the next checkpoint; keep the full P00-P18 goal active.

## P04 ordered command and shutdown admission

2026-10-24, based on delivered `bf3ccc88`, matching `origin/main` at milestone start. The private
`golden_audio::control::ingress` boundary now owns sequence allocation, producer serialization and
closure. Callers submit a command rather than a pre-numbered envelope. Rejection consumes no
sequence, so accepted queue order is authoritative even with concurrent `AudioControl` clones.

Nonblocking shutdown commits closure only when it can publish its barrier. Forced host shutdown
closes admission first and retains the terminal sequence while waiting for queue capacity; later
commands cannot enter behind it. Repeated shutdown observes that same sequence. The final counter
value is reserved for shutdown, and consumer loss rejects admission/unblocks a pending close.
The control worker terminates on producer disconnection as well as an explicit shutdown command.
No new queue, retry thread or app-specific policy was introduced.

Seven added cases bring the owning ingress suite to **11**. They cover 2,000 concurrent accepted
commands, concurrent shutdown, full nonblocking/forced close, disconnection before/during a pending
close, terminal-sequence exhaustion and immutable configuration/probe reuse after rejection.
The final test-only cleanup removed a raw state-byte assertion; observable ordering, closure and
outcome checks remain. No runtime behavior changed during that cleanup.

Validation with Rust/Cargo 1.97.0, Windows x86_64 MSVC, incremental disabled:

- Full default-feature `cargo test -p golden_audio -- --test-threads=1`: **180 unit + 21 public
  integration passes**, with no ignores/failures. The seven integration binaries cover clocks,
  devices, offline rendering, playback ordering, public API/realtime contracts and analysis.
- `cargo test -p golden_audio --no-default-features --lib -- --test-threads=1`: **136 passed**,
  0.13s after a 7.22s build. Feature inventories overlap and are not added as unique cases.
- Final owning ingress selection: **11 passed**, after an 18.93s build.
- Strict workspace/all-target Clippy passed in **41.11s**; final package/all-target Clippy after
  the test-only cleanup passed in **12.80s**. App consumers compile through the public boundary.
- Actual root/Core formatting and final diff/source checks pass; the largest touched Rust file
  is 680 lines. No physical stream or desktop automation was used.

Final default unit SHA-256:
`33b48660bef5cf4a0a77f7b4db1ceccb1e930c2c28e5ecaa19692742a93c723c`.
Final no-default unit SHA-256:
`63d089ad347695d135180e4ee9975fd828193a0c5933cf841c74c4457bfcd1b6`.

**P04 is 50% complete: two of four fixed acceptance checkpoints.** Production render plans and
gains still require a shared sequence/generation barrier and dense control-thread target resolution;
realtime allocation/deallocation and remove/re-add transitions remain unqualified. Whole-plan
scope coverage becomes ~19%, with P01-P03 still the three locally complete tasks. Commit/push
this verified milestone before editing the remaining production boundary.

## P04 production plan/control order and realtime transitions

2026-10-24, based on delivered `189175c914fb085726b9aa9eecaafa622ef0c3a7`, matching
`origin/main` at milestone start and after the final fetch. The reusable Golden Audio managed
runtime owns this repair; no application policy or protocol declaration was added.

`control::render_runtime::controls` pairs each prepared plan with a marker in the same bounded
4,096-slot queue as gains. Accepted ingress sequences reach configuration and gain delivery.
The control worker resolves output UUIDs against the latest accepted generation and converts
validated gains to linear values. Rendering checks sequence, generation and dense index using
scalar outcomes; rejection counts become diagnostics only on the control worker. UUID-based
processor setters explicitly reject realtime use. Queue rejection happens before the device
runtime's desired gain is mutated.

Plan publication preflights marker capacity before transferring ownership. A rejected pending
plan preserves the previous generation and target map. The render worker consumes a finite
initial queue prefix, applies old gains before the replacement marker and new gains after it.
Retirement pressure retains old owners and defers following controls; final destruction stays
off realtime paths. Publication uses explicit acknowledgement reclamation so the control owner
can retire the associated analysis controller without losing its acknowledgement to an implicit
publisher drain. Generic acknowledged exchange behavior remains available to its existing users.

Ten owning cases exercise older master/channel gains followed by configuration, gains after
configuration, channel removal/re-addition and dense-index reuse, full control queues, rejected
pending plans, sidecar acknowledgement ownership, saturated returns, bounded service under refill,
stale/missing/invalid/out-of-order controls, and disconnection. Allocation/deallocation guards
measure the same `RenderControlReader::begin_block` used by the managed render thread, together
with compiled rendering, across initial adoption, replacement, remove/re-add, retained returns
and rejection. All measured allocation/retained-byte deltas are zero.

The first owning selection had **eight passes and one failure**: the saturation fixture injected
its dummy return before publication, allowing the publisher to reclaim it before the measured
transition. Injecting after publication exercises actual saturated returns. The corrected nine-case
selection passed; the final full suite includes the additional sidecar acknowledgement case.
No assertion, gain ramp, queue capacity or expected order was weakened.

Validation on Windows x86_64 MSVC, Rust/Cargo 1.97.0, `CARGO_INCREMENTAL=0`:

- `cargo test --locked -p golden_audio -- --test-threads=1`: **190 unit + 21 public integration
  passes**, no failures/ignores. Unit execution 1.85s after a 24.74s build; playback integration
  includes the managed native-bridge/recovery and null-clock paths using controlled backends.
- `cargo test --locked -p golden_audio --no-default-features --lib -- --test-threads=1`:
  **136 passed**, 0.14s after an 8.15s build. This overlaps the default inventory.
- `cargo clippy --locked --workspace --all-targets -- -D warnings`: passed in **45.12s**,
  including application consumers.
- Actual root/Core formatting, final diff checks and source-size checks pass. The largest touched
  Rust file is 730 lines. Primitive wire serialization and generated DTO shapes are unchanged.
  No physical device stream, desktop automation or platform qualification was performed.

Final default unit SHA-256:
`ab7da167c8eb869b10a8945ebf620698e5a7531e825c6fcdc734780c2aa70c51`.
Final no-default unit SHA-256:
`86dc552fcc3f13e30866c0128959921dd8298dc30fe083c38428cc656ccb60c7`.

**P04 is locally complete: four of four acceptance checkpoints (100%).** Whole-plan coverage
is **~22%**, with P01-P04 the four locally complete tasks. P03 remains closed at 12/12 packages;
do not reopen a generic optimization chain. Native/platform/device qualification retains P17/P18.
Commit/push this verified milestone, then continue the earliest unfinished task, P05. The full
P00-P18 goal remains active.

## P05 HTTP/WebSocket request admission

2026-10-24, based on delivered `5a7db40a`, matching `origin/main` at milestone start and final
fetch. The previous goal turn made verified progress by delivering P04; it was not a no-progress
turn. This milestone implements P05's request-admission checkpoint in the reusable Golden host.

`golden_transport_server::UiServerConfig` now exposes bounded exact `allowed_origins` and
`allowed_hosts` lists. The private `ui_server/access.rs` boundary validates Host against the
accepted socket's local address and listener port or an explicitly admitted DNS authority.
It then admits the same HTTP origin or an explicit frontend origin. Arbitrary hostnames are
never resolved to infer trust. IPv4/IPv6 LAN addresses, loopback localhost, native/headless
requests without Origin and ordinary asset navigation remain usable. Cross-origin browser API
Fetch Metadata cannot omit Origin to bypass admission.

Golden Desktop registers the origin of the selected development or `GC_UI_FRONTEND_URL`
frontend, including the existing `cargo xtask watch` path. `GC_UI_ALLOWED_HOSTS` supplies explicit
comma-separated DNS authorities with their listener port. UI components and the app shell do not
own trust policy. Invalid configuration rejects startup; each list has at most 32 entries of at
most 2,048 bytes. Wildcards, opaque origins, userinfo and malformed authorities are rejected.

HTTP admission precedes API body parsing/dispatch and WebSocket upgrade. A response context
carries only its admitted origin, replacing wildcard CORS with exact ACAO and `Vary: Origin`.
Preflight checks methods and headers. WebSocket upgrades have their own Origin gate and no
wildcard CORS header. The existing overload error path also uses this response boundary.

`ui_server/http/request.rs` replaces manual request/header splitting with existing locked
`httparse 1.10.1`; `url 2.5.8` supplies authority/origin parsing. Cargo.lock only gains those two
direct dependency edges; no resolved package version changed. The parser retains 32 KiB header
and 16 MiB total byte limits, adds a 128-field cap, rejects ambiguous authority/credential/framing
duplicates, transfer encoding and non-origin-form targets, and preserves supported list headers
and JSON bodies. HTTP connections still close after one request. No protocol DTO shape changed.

Fourteen new owning tests cover policy and parsing, actual loopback HTTP/WS requests, exact
development CORS, preflight rejection, rebinding/duplicate Host, rejected privileged payloads,
headless access, body truncation and framing/resource bounds. Synthetic socket addresses cover
LAN/IPv6 admission without opening a physical endpoint. The initial unit run had **58 passed,
one failed and one ignored**: the URL parser accepted a literal wildcard hostname. The policy
now rejects wildcard entries explicitly; the assertion and accepted-origin scope were retained.

Final Windows x86_64 MSVC validation with Rust/Cargo 1.97.0, `CARGO_INCREMENTAL=0`:

- `cargo test --locked -p golden_transport_server -- --test-threads=1`: **59 unit passes /
  one existing ignored manual measurement**, plus **one public headless-host integration pass**.
  Unit execution 0.07s after a 1m15s build. The initial pre-correction build took 1m50s.
- `cargo test --locked -p golden_host_desktop --lib -- --test-threads=1`: **seven passed /
  one subprocess helper ignored**, 0.33s after a 44.69s build. Controlled Windows process-job
  tests preserve frontend descendant cleanup; no desktop input or GUI automation was performed.
- `cargo clippy --locked --workspace --all-targets -- -D warnings`: passed in **40.09s**.
- Actual root/Core formatting, final source/diff checks and local documentation links pass.
  Touched Rust sources remain below 1,000 lines. No physical stream or native-webview qualification
  was performed. App/browser end-to-end, CSP and session enforcement remain open checkpoints.

Transport unit artifact SHA-256:
`17a18ce729de8acf61ca1ce6931b40c6800e0ca8722c7fb00237403f6f3264df`.

**P05 is 25% complete: one of four fixed acceptance checkpoints.** Origin/Host admission is
implemented; scoped session capabilities are not yet implemented and are the next checkpoint.
This does not authenticate an already trusted native/network peer or qualify all of P05.
Whole-plan scope coverage is **~24%**, with P01-P04 still the four locally complete tasks.
Commit/push this verified milestone before implementing the remaining P05 boundary. Keep the
full P00-P18 goal active; P03 remains closed at 12/12 packages.

## P05 scoped sessions and authenticated client recovery

2026-10-24, based on delivered `90c5264e83d6e2d68b6af333daebd5d2ad61b15d`, matching
`origin/main` at milestone start and final fetch. The previous goal turn delivered request
admission and is classified as progress. This milestone completes two more P05 checkpoints;
the complete P00-P18 goal remains active.

`ui_server/session.rs` owns a per-host registry of at most 128 live credentials. OS randomness
comes from existing locked `getrandom 0.4.3`; `base64 0.22.1` encodes 32 random bytes as an opaque
URL-safe token. Cargo.lock only gains these two direct edges; package versions did not change.
Each grant retains its exact admitted Host/Origin, independent observation/authoring/project-file/
script flags, atomic validity and a one-hour renewable lifetime. Renewal retains its registry slot
and cannot change privileges. Capacity returns 503 without evicting live clients. Expiry, revocation,
registry teardown and a new host registry invalidate old credentials. The trusted listener still
determines who can bootstrap without an account/password; this does not authenticate a hostile
peer already inside that policy. Authoring can invoke application side effects and is privileged.

`golden_protocol` owns `UiSessionCapabilities`, `UiSessionRequest` and `UiSessionResponse`.
The canonical protocol version is now **0.8.0**. Codegen exports the three new DTOs and version
with the rest of the bindings. The response supplies the WebSocket offers, so clients do not
duplicate the credential protocol declaration. Debug output redacts secrets. HTTP uses a bearer
header; the WebSocket server selects only the non-secret application protocol. Credentials stay
out of URLs and handshake response headers. API/session responses use `Cache-Control: no-store`.

The typed `http/routes.rs` set owns both route dispatch and exhaustive capability selection.
WebSocket message privilege matching is also exhaustive; a new operation requires an explicit
decision rather than inheriting observation rights. HTTP gates privileged handlers before payload
interpretation/file access. Snapshot edit cancellation additionally checks authoring before its
effect. Every WS operation checks its grant before hub admission; observation-only clients cannot
bind an authoring identity or forward intents. Expiry/revocation closes existing sockets with
policy code 1008 and retires their application client leases. Previously admitted effects retain
their existing completion/cancellation contract.

`golden_ui/transport/session.ts` coalesces bootstrap, caches current credentials and renews them
before reconnect. An invalidation revision rejects late stale renewals. Socket opening is shared
across concurrent callers and starts only after credential admission. Observation-only clients
omit authoring identity binding. A selected socket URL also supplies the HTTP/bootstrap base,
preserving LAN callers that supply only a socket endpoint. HTTP retries only a pre-dispatch 401;
403, 503, timeout and network outcomes are not replayed. A timed-out intent cannot be resurrected
when a delayed bootstrap finally opens its socket.

The Node 26 qualification helper imports the same credential implementation and has a bounded
16-origin cache. Both native qualification scripts now authenticate their HTTP/WS calls and use
the server's canonical protocol version, replacing the transport probe's old hard-coded 0.4.0.
The HTTP codec extraction is mechanical: values/constraints/control descriptions, document/event/
intent conversion and project/script conversion have focused owners below 1,000 lines. HTTP
delivery and socket bookkeeping remain separate. Existing domain-inference cleanup is not claimed
complete by this move; P08/P12 retain their acceptance scope. Tests inject a credential provider
and wait for bootstrap rather than bypassing production authentication.

Final validation on Windows x86_64 MSVC, Rust/Cargo 1.97.0, incremental disabled:

- `cargo test --locked -p golden_transport_server -- --test-threads=1`: **71 unit passes /
  one existing manual measurement ignored**, plus **one public headless-host integration pass**.
  Final typed-route execution 0.10s after a 1m29s build. The 12 added cases cover bound/scoped
  credentials, full registry pressure/renewal/recovery, revocation/teardown/new host, real HTTP and
  WS rejection, denied save without file creation, read snapshots, and live WS revocation.
- `cargo clippy --locked --workspace --all-targets -- -D warnings`: passed in **47.25s**;
  the final repeat after frontend corrections passed in **45.53s**, rebuilding the current UI bundle.
  The first run rejected a nested conditional; it was collapsed without suppressing the lint.
  The first socket-test build also caught the explicit `HandshakeError::Failure` wrapper; its
  corrected test retained the 401-before-upgrade assertion. No security assertion was weakened.
- Golden UI: **35 passed / five files**. App UI: **92 passed / 25 files**. The final app run
  took 4.89s. Svelte check reports **zero errors and zero warnings**.
- `node --test tools/qualification/tests/ui-session.test.mjs`: **one pass**, exercising native
  TypeScript import, actual loopback requests, restart renewal and no replay after 503. A separate
  loopback observation confirmed Node's WebSocket handshake omits Origin, matching native binding.
- `npm run build --workspace chataigne-ui`: passed; client build 2.27s and server/static build
  5.24s. The >500 kB minified-chunk warning remains visible, with no threshold change. Bundle and
  product-scale qualification retain P16.
- Regeneration of **118 bindings** was byte-identical: aggregate path/content SHA-256
  `97240d59d9ee0277e18cbc69c4fbe3c1ceff24971f57e72f2a55e2c562f84e1e` before and after.
- Actual root/Core formatting, final diff/source-size and local documentation-link checks pass.
  The largest touched source is 965 lines. No GUI input, physical stream or native-webview test
  was performed. The final small frontend endpoint/binding corrections were followed by the
  complete UI suites, Svelte check, production UI build and final workspace Clippy. Rust source
  was unchanged; all 37 touched code-file hashes also match across final formatting/checks.

Final transport unit artifact SHA-256:
`b9ecd71558a490daf5f82f174971ba5e7eb68658f454ce94625c9c9f28096dc5`.

**P05 is 75% complete: three of four checkpoints locally verified.** CSP and the complete
supported-surface matrix remain open, including real served/dev/LAN/browser behavior and reload/
session-pressure qualification. Whole-plan coverage is **~26%**, with P01-P04 still the four
locally complete tasks. Commit/push this verified contract milestone before continuing that final
checkpoint. P03 remains closed at 12/12 packages; keep the full P00-P18 goal active.

## P05 closed: frontend policy and supported transport surfaces

2026-10-24 on `main`, based on delivered `cbd3e277`. The previous user-facing turn answered a
status question without changing authoritative state and is classified as no progress; this turn
revalidated the checkout and took the next ready P05 action. The old transport test handle was
missing, so it was rerun; the prior UI build handle was terminal/successful. No work was restarted
on P03 or P04. **P05 is now locally complete, four of four checkpoints; whole-plan coverage ~28%.**

Golden Transport prepares bounded HTML5-normalized SHA-256 bootstrap hashes before starting its
runtime, then emits asset-specific CSP and exact admitted-authority WebSocket sources. Scripts
have no inline/eval exception; inline styles remain necessary for Svelte presentation. MIME
sniffing, objects, base changes, framing and form submission are disabled. Existing locked
`dom_query 0.27.0` and `sha2` provide the parser/hash primitives; Cargo.lock only adds their two
direct transport edges, without package upgrades. SvelteKit supplies framework hashes/nonces;
the app development response hook retains those while limiting backend and HMR connections.
Tauri's null policy is replaced by explicit production/dev baselines. Native permissions remain
unchanged and limited to the main window's declared loopback remote origins.

The shared client persists only its opaque token in per-tab session storage and revalidates it
on reload, retaining one host slot. Storage failures preserve in-memory operation. Stale/revoked
tokens are discarded without trusting stored privilege/expiry claims. The default launcher and
watch frontend forward the selected backend port instead of silently falling back to 7010.

Local validation, Windows x86_64 MSVC / Rust 1.97.0 / Node 26, incremental disabled:

- `cargo test --locked -p golden_transport_server -- --test-threads=1`: **74 unit passes /
  one manual benchmark ignored**, **one public integration pass**; final run 3.91s build / 0.10s
  unit execution. New CSP cases cover HTML5 newline/raw-script text, exact IPv6 authority,
  asset-specific hashes, invalid UTF-8, duplicate paths and script-count rejection. Preserved
  HTTP/WS suites independently prove denied file/intent/script access before dispatch.
- `cargo test --locked -p golden_host_desktop --lib`: **seven passes / one helper ignored**.
  `cargo test --locked -p xtask`: **one unit and six integration passes**.
- Golden UI: **39 passes / six files**; app UI: **93 passes / 25 files**. Session reload and denied
  storage cases, CSP nonce/exact-endpoint assertions and selected backend-port cases are included.
  `node --test tools/qualification/tests/ui-session.test.mjs`: **one pass**.
- Svelte check: **zero errors / zero warnings** after replacing direct `process.env` access with
  SvelteKit's private server environment API. Production UI build passed (server/static build
  5.22s); the existing >500 kB chunk warning remains visible with P16 ownership.
- `cargo build --locked -p Chataigne2`: **passed, 2m50s**, default features and a freshly rebuilt
  bundled UI. `cargo clippy --locked --workspace --all-targets -- -D warnings`: **passed, 51.24s**.

The new network/DOM-only harness ran against that exact binary, SHA-256
`49eb15bd7382f0e30af7bc48735c7df862a712238ea70f207a3bfcf8763d1024`, using a separate
headless Edge **156.0.4314.42** profile and isolated preferences. The
[raw surface report](evidence/p05-transport-security-2026-10-24.json) records **PASSED**:

- Real bundled pages at loopback and the machine's private LAN interface, plus a separately
  configured Vite origin/backend port, mount the workbench, establish authenticated subscriptions
  and return authorized snapshots. Each survives three reloads with the same credential.
- All three browser surfaces have zero runtime errors and zero unexpected CSP violations.
  Both injected inline and data-URL scripts are blocked on each surface.
- A separate unapproved browser origin cannot bootstrap HTTP or upgrade WS. Native HTTP and WS
  independently reject missing/revoked tokens; observation-only sessions read snapshots but cannot
  dispatch edits or privileged project/script requests. Host mismatch and unapproved Origin fail.
- Restarting the real host rejects its old credential and admits a fresh HTTP/WS session. Existing
  unit pressure tests retain the 128-session cap and in-place renewal contract.

No keyboard/pointer synthesis, desktop control, physical stream or native webview was used.
LAN evidence uses the local NIC address from the same machine, not a second physical device.
Native webview/dialog/focus/DPI, other platforms, proxy/TLS behavior and product-scale qualification
retain P17/P16 owners. Owned headless app, Vite and headless browser processes were closed.
Actual root/Core formatting, diff hygiene and touched-source limits are checked before delivery.
Commit/push this verified P05 milestone, then refresh authoritative dependency/advisory evidence
for P06. Keep the full P00-P18 objective active; P03 remains closed at 12/12 packages.

## P06 current dependency repair and required frontend migration

2026-10-24 on `main`, after delivered P05 `788dd415`. The refreshed advisory database is
`21d909e4c8bd4e14c7aea58bf980c881a0053ab1` (commit 2026-10-24 10:30:13 +02:00), checked with
`cargo-deny 0.20.2`. The baseline failed eight RustSec advisories and npm reported nine affected
packages (four high, four moderate, one low). The
[dependency evidence](evidence/p06-dependencies-2026-10-24.json) records IDs, versions, feature
reachability and provenance. This is new current evidence, not a claim that an exploit was shown.

The Rust lock changes only seven package versions: Rustls **0.23.46**, WebPKI **0.103.16**,
Tokio **1.53.3**, both WebPKI root packages **1.0.10**, and the AWS-LC pair **1.18.2 / 0.46.0**
required by Rustls's new `aws-lc-rs = 1.18` constraint. Tokio stays below the unrelated 1.54 line.
Twelve unrelated broad-range dependency edge rewrites from Cargo were restored; locked resolution
and compilation accept the preserved graph. The existing WebPKI 0.26.11 wrapper now references
the repaired 1.x roots. Default app Rustls features are AWS-LC, ring, std and TLS 1.2; Brotli stays
disabled. The current Unix AsyncFd advisory is repaired in the lock; Windows execution does not
qualify Unix I/O safety behavior.

Maintainer advisories
[GHSA-pfhg-q3qv-9q6r](https://github.com/sveltejs/kit/security/advisories/GHSA-pfhg-q3qv-9q6r) and
[GHSA-27mw-82g8-phjj](https://github.com/sveltejs/kit/security/advisories/GHSA-27mw-82g8-phjj)
list SvelteKit **3.0.0** as their repair. This directly related major update is necessary to restore
the unsuppressed gate. Static adapter **4.0.0** and Svelte **5.57.1** satisfy its supported peers;
Vite **8.1.6** and Vitest **4.1.12** repair their advisories. Other updated npm transitives belong
to these packages or the named devalue, PostCSS, NanoID and source-map-js repairs. No blanket
forced update or new advisory suppression is used.

Following the [maintainer migration](https://svelte.dev/docs/kit/migrating-to-sveltekit-3), app
adapter/CSP/prerender settings move to the existing Vite plugin and the obsolete config file is
removed. Native `#lib/*` package imports replace the removed alias, environment/hooks imports use
the supported modules, and TypeScript uses `$app/tsconfig` with Vite-owned emission. CSP source
literals retain their types without making Golden UI depend on SvelteKit. An unused undeclared
SvelteKit import in Golden's logger was exposed by the changed install layout and removed; the
library does not gain a framework dependency. Audio UI declares Vite client ambient types for its
asset/HMR imports rather than relying on an app's generated declarations. Explicit Node 26 types
support the frontend build configuration.

The duplicate gate now covers **66 reviewed crate names**. Seven missing families already had
the same versions in `788dd415`: GTK versus PipeWire build helpers, platform audio/desktop
DBus/Mach bindings and existing engine versus audio/processor Criterion benches. Their immediate
parents are recorded in the evidence; no unrelated major benchmark API is upgraded. The engine's
unused direct QuickJS dependency is removed. Golden Script still owns QuickJS and the Linux ARM
bindgen/parallel features, confirmed by target-specific Cargo feature resolution. `cargo machete`
passes. Vendored CPAL **0.18.1** and its exact-ASIO lookup patch, license, source path and documented
upstream-removal/independent-consumer condition remain unchanged.

Checks completed before final artifact validation:

- Full `cargo deny --locked check` passes advisories, licenses, sources and bans with **zero
  errors**. Existing duplicate warnings (59), one license-list warning and four advisory notes
  remain visible. Existing quick-xml exceptions are unchanged; no new ignore is added.
- `npm audit --json`: **zero vulnerabilities**. `npm ci --ignore-scripts`: passes; generated
  contract freshness is checked by the normal root check separately.
- UI suites: Audio UI **27**, Golden UI **39**, app UI **93**, Graph UI **13** passes.
  Production UI build passes with the existing >500 kB chunk warning. App and Audio UI checks
  report zero errors/warnings after the explicit ambient/configuration corrections. Root lint passes.
- Updated `cargo test --locked -p golden_transport_server -p golden_io -p golden_audio --
  --test-threads=1` passes (2m27s build), including **190 audio unit cases**, retained public audio
  consumers and **74 transport unit cases / one manual benchmark ignored**, plus the public
  headless-host integration case. No physical stream is opened by qualification.

**P06 remains 75% while final engine/app/build/Clippy and exact new-binary browser qualification
run.** A live IDE Cargo check temporarily held the shared target lock; it was allowed to finish,
not terminated or bypassed with a second target directory. No owned job was restarted on timeout.
Do not claim the final artifact gate before the remaining results are recorded. Keep P00-P18 active.

The early frontend/runtime isolation run caught a SvelteKit 3 migration regression: a route marked
`prerender = true` is treated as prerendered in development too, placing CSP in meta instead of
the response header. The development hook consequently had no header to constrain. The
[failure report](evidence/p06-frontend-transition-failure-2026-10-24.json) retains the unchanged
header assertion and real 200 response. The root layout now prerenders only production pages;
development retains framework-generated response nonces and the exact endpoint header. A repeat
on the same previous Rust binary passes the new SvelteKit 3 development surface, three reloads,
authenticated snapshots/subscriptions and both injection denials, with no unexpected CSP/runtime
errors. That interim run isolates frontend repair; it does not substitute for final qualification
of the rebuilt Rust/bundled artifact. The harness now records response status and Vite diagnostics
on failure without printing credentials. Security assertions remain intact.

Final core validation: `cargo test --locked -p golden_engine --lib` passed **606 cases / three
manual measurements ignored**, 10m00s compilation and 0.53s execution, on the removed direct
QuickJS edge and repaired lock. The chained next command mistakenly selected an app library
target; Cargo reported that Chataigne2 has only binary targets, so no app test was executed by
that command. The corrected binary-target protocol/Buttplug test, default build and strict Clippy
chain is now running. Clean-install root `npm run check` also passed, including generated audio
freshness and zero Svelte errors/warnings; `npm audit --json` remains at zero vulnerabilities.

Continuation at **13:13 local**: command handle **22661** is confirmed live, with two current
Chataigne2 compiler processes (PIDs 42556/42848, started 13:10:20). It owns the corrected protocol
tests, then Buttplug tests, default app build and strict workspace/all-target Clippy, stopping on
failure. Poll this handle before scheduling another Rust job; an observation timeout is not
termination. The engine/check/generated-install handles are terminal. After the app chain passes,
run the security surface harness against the **new** binary, retain its hash/report, finish actual
root/Core fmt and diff hygiene, close the fourth P06 checkpoint and commit/push the milestone.
Do not reuse the old `49eb15bd...` binary as final P06 artifact evidence. Current P06 work remains
uncommitted; P05 is already delivered. No other plan task was reopened or substituted for P00-P18.

Final app validation update: the filesystem-style filters selected **zero tests** despite the
successful 6m03s/5m03s test builds. Those empty runs are not acceptance evidence. The actual
inventory uses generated `app::module_modules_*` names. Direct execution of the built test
artifact, SHA-256 `bfb198fb9d262389f9695cdd6398bab485abe8f8c90b2abd7da89ab645c8eee9`,
selected **549 protocol cases: 548 passed / one failed** in 34.67s. The failure was Serial's
`partly_applied_catalog_feedback_reconciles_its_suffix_and_preserves_port_identity`, at the
final no-catalog-warning assertion (`tests/passive.rs:331`). It passed alone and in its six-case
passive suite. The original fixture relied on the shared real discovery service even though
it is testing projection recovery against a healthy catalog. This is an uncontrolled dependency;
the original assertion did not print which live catalog warning remained.

The fixture now owns a separate discovery registration with an empty successful detector, using
the same production discovery service implementation. The helper lives only in `discovery/tests/`;
production Serial behavior is unchanged. Applied-prefix rejection, warning clearance, connection
state and exact port identity assertions remain intact. Only test files were edited while the
production-only compiler was active. The attempted cancellation of the redundant empty-filter
compiler found that it was already gone; **no process was stopped**.

The default `cargo build --locked -p Chataigne2` passed in **2m49s** and strict workspace/all-target
Clippy passed in **48.85s**, including the isolated fixture. The exact new default binary SHA-256 is
`4c47c2759e14ba407148b13c743989016f3749cbaf27724397c33d1358cc5a08`. Its
[final browser/native surface report](evidence/p06-transport-security-2026-10-24.json) is **PASSED**:
bundled loopback/LAN, SvelteKit 3 development, native HTTP/WS, denied origins/credentials,
reload/session reuse, injected-script denial and restart fencing. Each browser surface records
zero unexpected CSP/runtime errors. This is current binary evidence, not the earlier isolation run.

The correctly selected verification is now command handle **15921**:
`cargo test --locked -p Chataigne2 --bin Chataigne2 -- module_modules_protocol_
module_modules_controllers_buttplug --test-threads=1`. A pre-run test inventory confirms **589
matching cases**. This one test-target build avoids another empty-filter or unrelated integration
target build. Keep P06 at 75% until those executed results pass; then finish formatting/diff checks,
close P06 and commit/push before implementing P07. Prior handle 22661 is terminal/successful.

### P06 final closure

The corrected single test-target command completed at 13:30 local: **589 passed / zero failed /
zero ignored / 1156 filtered**, after 4m53s compilation and 39.72s execution. The final test binary
SHA-256 is `b9ed008f740f00a248643f826ac725252559eaceaaebb69f1637a25ec3375fee`.
The previously failing Serial assertion passed **20/20 further exact repetitions**, with each
run checked to execute one case. The actual `app::module::common::serial::discovery::` namespace
also passed **seven cases** in 3.01s. An attempted generated-style common-serial filter selected
zero cases and receives no validation credit. No assertion or production Serial behavior was
relaxed. These executions supplement the earlier retained 548/549 failure; they do not erase it.

The current default binary still hashes to `4c47c2759e14ba407148b13c743989016f3749cbaf27724397c33d1358cc5a08`,
matching the final browser/native report. The final full cargo-deny recheck again reports zero
errors across advisories/licenses/sources/bans, with the same explicit warnings/notes. All owned
verification handles are terminal. Actual root/Core formatting, touched-source limits (largest
867 lines), JSON validity and diff hygiene pass. No desktop input/native webview or physical
stream qualification was performed; those later owners remain explicit.

**P06 closes locally at 100%; whole-plan coverage is ~33%, six of eighteen tasks complete.**
P03 stays closed at 12/12 packages. Deliver this milestone on `main`, then continue P07's native
lighting worker. Preliminary read-only inspection confirms its current send-only refresh gap,
one-packet receive turns, missing Art-Net universe filtering and discarded sACN sync traffic.
The established `sacn 0.11.1` source still terminates by multicast after unicast sends. Before
coding, review the maintained pure protocol-core alternative `x39-sacn 0.2.1` (MIT/Apache-2.0,
MSRV 1.91, registry source commit `5259dd464c77e05ccaa3fd3695ba2ea2de551daa`), which exposes
fixed-capacity storage and caller-supplied time. No P07 source or dependency change has started.
Do not rebuild a protocol that an adequate established crate already supplies. Keep P00-P18 active.

## P07 native receive admission and burst service

Started 2026-10-24 after delivered `0044eb3a`, from a clean `main`. The previous goal turn made
progress by delivering P05 and implementing P06; this continuation completed and pushed P06,
then began the next dependency-ready P07 work. P03/P04/P05/P06 remain closed at their local scope.

The DMX lighting worker now distinguishes empty sockets, ignored packets, invalid packets and
frames. It drains up to 64 datagrams or two milliseconds per turn, returning to its eight-command
and cancellation service without an idle wait when a receive budget is exhausted. No new engine
poll is introduced. Whole UDP datagrams use a reusable native 64 KiB buffer before the established
Art-Net/sACN codecs receive their bounded inputs. Malformed/oversized packets no longer retire a
healthy endpoint; their warning is latched once per native lifetime. Real I/O failures retain
explicit outcomes and the existing recovery strategy.

Art-Net input filters the configured one-based universe. sACN input filters non-zero DMX START
codes and accepts the protocol's valid zero priority. Frame/native bounds, backend preparation and
parameter constraints now consistently accept 0–200. Existing command/script/callback/template
entrypoints and retained source/publication fences are preserved. No new dependency is introduced
by this admission step. Full source/sequence/priority arbitration, sync, periodic output and correct
termination remain explicit later P07 work; the raw sACN receive adapter still lacks that state.

New owning cases cover whole oversized datagrams, wrong universes, malformed traffic followed by
valid traffic, START-code rejection, priority zero and an 8,192-packet ignored burst while output
still completes. Packet tests are loopback-only; existing sACN multicast teardown remains contained
by its test socket settings and is not claimed correct. The native scratch bound is 1 MiB across
sixteen slots, separate from retained pools and third-party library storage, not a total-RSS claim.

The initial strict workspace/all-target Clippy passed in **35.53s**. The first owning run
(handle **88796**) compiled in 4m01s and executed **39 passes / one failure**: the older Art-Net
latest-frame fixture configured universe 1 but inherited the codec's default port address 1,
which represents universe 2. Correct universe filtering exposed that fixture error. The test now
explicitly encodes port address 0; its latest-frame and pressure assertions remain unchanged.

The corrected verification (handle **72784**) passed strict workspace/all-target Clippy in
**34.88s**, then compiled in **3m48s** and executed **41 passes / zero failures / zero ignored**
in **2.78s**, including malformed-flood warning bounds. Its queued default app build passed in
**2m35s**; the handle is terminal. The first of six P07 checkpoints is locally complete
(approximately **17%**); whole-plan coverage is approximately **34%**, with six fully complete
tasks. Five P07 checkpoints remain open. Actual root/Core and explicit app module formatting,
diff hygiene and touched-source limits pass. Explicit app formatting also normalizes nearby
DMX modules reached through Rust module declarations; those additional changes are formatting
only. The default app artifact SHA-256 before that formatting-only pass is
`d7ccc533cc71ecb91e2ab838965bae1291b78775bfdae58f491703b3320208cf`.
No physical device, native webview or external platform qualification is claimed. Deliver this
verified receive milestone on `main`, then continue protocol-state integration. All six P07
workstreams and the full P00-P18 objective remain intact.

## P07 clocked output and protocol source integration

Resumed delivered `ba17603d` from clean `main`, matching `origin/main`. The previous goal turn
was **progress**: receive-admission verification completed, the ledger was reconciled and the
milestone was committed/pushed. This turn continues the second fixed P07 checkpoint; no P03
closure package is reopened and no additional checkpoint denominator is introduced.

The native worker now consumes an app-owned, caller-clocked output state. Art-Net retains one
look, owns non-zero sequence numbers, paces explicit sends at 23 ms and refreshes static looks
every 900 ms. sACN now uses **x39-sacn 0.2.1**, imported as `sacn`, with only its `std` feature:
the protocol core supplies pacing, suppression/refresh, wire sequencing, discovery and timed
termination without owning sockets or a Tokio runtime. Its source storage admits one transmit
universe and fixed derived pending buffers; unused receiver/catalog capacities are zero. The
old `sacn 0.11.1` dependency and its multicast-only termination path are removed entirely.

The source CID comes from the module node's persistent UUID. Reopening keeps that identity,
but begins without a retained look, preserving the existing no-replay rule for old commands.
The worker retains at most eight FIFO commands while awaiting the send clock, alongside the
existing bounded mailbox. Each keeps its original payload/source admission and Pending receipt
until actual local submission; identical explicit frames and blackout are not coalesced. Native
input and out-of-band cancellation remain serviced while output is paced. Output-only workers
wait on command/stream deadlines instead of a recurring five-millisecond receive poll.

Unicast data and termination use the same configured destination; discovery is skipped for
unicast-only modules. Timed termination runs only on the retiring native worker, capped at
200 ms, with no engine join. This is implemented behavior awaiting owning execution, not closure
of the separate termination/recovery checkpoint. Receive source arbitration, sequence/loss/sync
policy and typed backend frame publication remain open.

The first `cargo check` found one required wildcard arm for the dependency's non-exhaustive
`Route` enum; unsupported future routes now report an explicit error. The corrected app/all-target
strict Clippy passed in **59.15s**. Full locked cargo-deny reports **advisories ok, bans ok,
licenses ok, sources ok**, without new suppressions. The lockfile adds the source core and its
four necessary transitive packages (`derive-where`, `hash32`, `heapless`, `syn 3`), removes the
legacy `sacn` package and does not update unrelated versions.

Command handle **93653** completed successfully: **48 passed / zero failed / zero ignored**
in **2.49s**, after **4m15s** compilation. Added cases exercise eight
seconds of static refresh in fake time, 260 explicit identical/blackout frames with sequence
wraparound, identity/no-replay on recreation, three-packet unicast termination, submission errors,
paced FIFO receipts with continuous input, and cancellable clock waits retaining original
quotas. A final integration case now checks the actual output-only native worker: FIFO frames,
blackout, automatic refresh without a new command, prompt stop and three termination packets
received at the original loopback unicast destination. The obsolete duplicate destination
argument was removed from the worker's endpoint send interface; endpoint configuration owns it.

Final command handle **49572** runs actual root/Core formatting, strict workspace/all-target
Clippy, all DMX cases and the default app build, in that order. It initially waited behind the
IDE's live `cargo check --workspace` process (PID 588, observed with its rustc child); no process
was stopped or restarted. Poll this exact handle before another Rust build or source edit. No
additional completion credit is assigned until these final executed checks pass.

That workspace Clippy passed in **1m17s** (including the IDE-lock wait). The expanded suite
compiled in **4m50s** and executed **49 passes / zero failures / zero ignored** in **3.55s**.
The new native case passes for both protocols, including automatic output-only refresh and
all three sACN termination packets observed at the configured unicast receiver. The default app
build in handle **49572** is still running; no compiler was restarted.

The final source review identified a pacing-policy refinement before closing checkpoint two:
the imported source uses a 22 ms quantum. Apply a conservative **23 ms** caller-side floor to
universe data transmissions for both protocols, including scheduled sACN bursts and termination,
without replacing the core's source/suppression/sequence state machine. Follow its deadlines
through this floor rather than polling a past deadline repeatedly. Tighten the fake-time pacing
assertions and repeat the owning checks after the active compiler is terminal. This remains the
same output/identity milestone, not an additional scope checkpoint.

Handle **49572** is terminal/successful; its default app build passed in **3m00s**. The pacing
floor is now implemented at the caller boundary for both explicit and scheduled universe data,
including termination. The next-deadline getter includes the floor, and an early poll is a no-op
without rearming a past deadline. A new fake-time case specifically exercises 22/23/46 ms,
and all refresh/explicit/termination timing assertions now require at least 23 ms.

Final verification is now command handle **27375**: strict workspace/all-target Clippy, the
full **50-case** DMX selection, then the default app build. Poll that exact live handle before
another Rust build or relevant source edit. Earlier 48/49-case executions remain evidence of
the preceding implementation; do not substitute them for this changed pacing policy's result.

The final pacing-policy verification passed strict workspace/all-target Clippy in **44.98s**,
then compiled the test target in **4m36s** and executed **50 passed / zero failed / zero ignored /
1709 filtered** in **3.57s**. This includes the tightened 23 ms assertions, timer-floor case and
actual native loopback. The default app build in **27375** is still live. After it passes, finish
actual root/Core formatting and diff/source-size checks, close the second checkpoint at **2/6
(approximately 33%)**, update overall scope to approximately **35%**, then commit/push this
single output/identity milestone before continuing receive source/synchronization policy.

### P07 output/identity milestone closure

Handle **27375** is terminal/successful. Its final default app build passed in **2m19s**;
the artifact SHA-256 is `acb68f02114e22e21808fb867c619bab79993ed9967c5d580bc169bcc097e3da`.
The final 50-case execution, strict workspace/all-target Clippy, unchanged-lock dependency
checks, actual root/Core formatting, diff hygiene and touched-source limits all pass (largest
705 lines). All owned verification processes are terminal. No physical stream/device or native
desktop interaction was used; loopback and fake-time evidence do not close later platform work.

**Two of six P07 checkpoints are complete locally (approximately 33%); four remain. Whole-plan
scope coverage is approximately 35%, with six tasks fully complete.** Deliver this milestone on
`main` before further implementation. Next integrate bounded receive source/priority/sequence/loss
and synchronization policy. The crate exposes fixed-capacity `Receiver`, caller-supplied time,
borrowed merged outcomes and `sync_group_interest`; preserve source admission, multicast-group
lifetime bounds and truthful metadata for merged frames. Do not invent a single wire sequence
for a merged result. Backend typed frame publication and full termination/overload/reconnect
qualification remain separate open checkpoints. Reconnect desired-look restoration must be
distinguished from replaying old command receipts. The full P00-P18 objective remains active.

## P07 receive arbitration and synchronization

Resumed clean delivered `1961cf20` on `main`, matching `origin/main`. The previous turn was
**progress**: clocked output/identity was verified, committed and pushed. P07 stays at two of six
verified checkpoints while receive source/synchronization execution is pending. P03 remains closed.

The app-owned receive layer now separates input metadata from output frames. Art-Net admits
two IP/Physical identities, HTP-merges their levels, rejects duplicate/old nonzero sequence
numbers, handles disabled ordering and wrap, and follows matching-controller ArtSync with a
four-second fallback. Different-IP merges ignore sync; ten-second source expiry releases
admission. Losing every source holds the last look and marks the frame stale.

sACN admits eight CIDs and composes a fixed-capacity protocol receiver per CID with the crate's
fixed-capacity combined merger. Source review found that x39-sacn 0.2.1's merged receiver passes
`PacketOutcome::Sync { sync_address, .. }` into an address-only release path, dropping CID and
sequence. Per-CID protocol state and explicit sync admission prevent cross-controller release;
the library continues to own data sequencing, loss, PAP, buffered release and sync timeout.
Native multicast membership follows current declared interests, removing obsolete groups before
joining replacements; dynamic memberships remain bounded through churn. No receive state is
allocated for output-only endpoints.

Frame metadata carries bounded source identities, effective slot priorities and stale/sync
flags. Sequence and singular priority are nullable for combined results; no fake wire sequence
is invented. The existing callback signature and app-owned snippet are retained. Separate
input-loss/recovery status clears the module's input warning on recovery or a new ready endpoint.
JSON backend projection still exists and remains the fourth P07 checkpoint.

Initial Clippy found a missed `Deref` return type, a public enum exposing private source state,
and explicit `drop` calls on borrowed non-Drop outcomes. These are corrected using an opaque
receiver wrapper and lexical scopes; no lint suppression was added. Strict app/all-target
Clippy then passed in **36.92s**. Command handle **88791** is now building/running the 63-case
DMX selection. Poll that exact handle before another Rust build or relevant source edit.
Additional sync-history, cross-source, timeout and native loss/recovery coverage remains to be
added before closing this checkpoint, even if the first execution passes.

The first owning execution (handle **88791**) compiled in **4m59s** and ran **54 passes /
nine failures** in **3.60s**. All nine failing sACN cases expected a frame from their first packet.
The wrapper had ended zero-duration sampling before feeding that packet; the library then
entered its initial PAP handshake and withheld it even with a zero wait. Native continuous-source
round trips passed because a later packet happened to release that state. This was an adapter
ordering bug, not proof that input worked. Source construction now preserves the sampling window
until the first packet is fed, then drains the library's due publication at the same caller time.

Synchronization admission now has eight independent address/sequence histories per CID and
uses the standard's signed sequence-discard window. Added owning cases cover same-address sync
from two CIDs, stale sequence rejection after address changes, both sync-loss policies and
recovery, PAP fallback on live levels, sixteen membership replacements with the bound checked
after every join/leave, failed membership bookkeeping, and native source-loss/held-look/recovery.
No new dependency or lint suppression is introduced.

Command handle **57105** is running corrected strict app/all-target Clippy and the expanded
DMX suite (expected 70 cases). Poll it before another Rust build or relevant source edit.
Keep receive scope uncredited until the actual executions and final default/workspace gates pass.

The expanded run (handle **57105**) passed strict app/all-target Clippy in **36.65s**, compiled
in **4m19s**, then executed **68 passes / two failures** in **6.10s**. All first-packet, native
loss/recovery, cross-CID release, priority/PAP, sequence, pressure and membership cases passed.
The remaining termination fixture discarded the recomputed frame returned immediately by
`handle` and then unwrapped an empty later `poll`; it now asserts that immediate result. The
sync-history fixture's exact unchanged-artifact rerun/backtrace located its failure at the address
switch (`sacn_cases.rs:208`): Force_Synchronization=0 keeps the old coherent frame held until
the new address's release. That expectation now asserts withholding while retaining the old-sync
rejection and exact final-level assertions. Production behavior is unchanged by those corrections.

Wire admission now also rejects PAP values above 200 before they consume source or sequence
state. Its new owning case keeps all eight source slots usable and the next valid sequence
accepted. Final command handle **54494** runs actual root/Core formatting, strict workspace/
all-target Clippy, the full **71-case** DMX selection and the default app build. Poll that exact
handle before another Rust build or relevant source edit; previous failed executions remain
recorded and do not close the checkpoint.

The final strict workspace/all-target Clippy passes in **1m00s**; handle **54494** is now
compiling the 71-case test target, with the default app build queued. No further Rust edit has
been made during that compiler. This does not close the remaining backend publication or
termination/recovery scope. In particular, the latter must qualify diagnostic/status backpressure
without parking the stream clock, alongside desired-look restoration distinct from command replay.

Final owning execution passed: **71 DMX cases / zero failed / zero ignored / 1709 filtered**
in **6.09s**, after **4m14s** compilation. The actual current test artifact also lists and passes
the **one** `app::module::script_api::tests` case (0.46s), which checks available functions and
callback documentation across module templates. No empty-filter run is credited. The final default
app build remains live in **54494**. After it passes, finish root/Core formatting and close the
third checkpoint at **3/6 (50%)**, overall scope approximately **36%**, then commit/push before
continuing backend frame publication. P07's other three checkpoints and P00-P18 remain open.

### P07 receive source/synchronization closure

Handle **54494** is terminal/successful. The final default app build passed in **2m25s**,
SHA-256 `ccb322339fbef1bd82fbe09fc2a3fafbbc84d6c981f6b86bef4b1975650a776c`. The current
test artifact, SHA-256 `ae70741313d35ab4e56bf849967d7a1a07598693b1bd6c4ffa557a9b7395fde3`,
passed the 71-case DMX selection and the one separate script-template/API case. Strict workspace/
all-target Clippy, actual root/Core formatting, diff hygiene and touched-source limits pass;
the largest touched source is 717 lines. Cargo.lock and package dependencies are unchanged.
All owned test/build processes are terminal. Qualification used fake time, controlled membership
operations and IPv4 loopback; no physical device/stream, desktop input or native webview was used.

**P07 closes checkpoint three locally: 3/6 (50%); whole-plan coverage is approximately 36%.**
Commit/push this receive milestone on `main` before source work for the next checkpoint. Keep
backend frame publication, termination/recovery and full local qualification open. The next
implementation must make per-universe typed state/batching authoritative rather than persisting
and reparsing 512-channel JSON at internal sample cadence. Preserve command/script/authoring
ordering, source guards, explicit receipts and persisted meaning. The later recovery checkpoint
also owns desired-look restoration, diagnostic backpressure that cannot park output timing,
and complete overload/reconnect/shutdown qualification. The full P00-P18 goal remains active.

### P07 typed frame publication — implementation and qualification

Resumed on `62ad19b7` / `main`. The previous goal turn answered a status question and made
no implementation progress; checkout revalidation confirms the unfinished byte-array work and
no blocker. P03 remains closed. P07 stays at 3/6 until this checkpoint's owning checks pass.

- Added generic binary parameter storage (`ByteArray` / `ParamValue::Bytes`), raw canonical
  runtime bytes, capacity accounting, script tagging, parameter factories and authored restore.
  Protocol version 0.9.0 and TypeScript DTOs are generated from Rust in the same change.
- DMX output/input/Send Frame command parameters now use bounded typed channel buffers.
  `setChannels` and Set DMX Channels apply one contiguous range in one ordered universe frame;
  scalar command ordering, retained invocation identity and final authored projection remain.
  Internal channel edits no longer reparse or persist JSON text. Explicit JSON-string
  `sendFrame` calls remain a parsing boundary; receive callback shape is preserved.
- Added a public borrow-by-default child-record migration hook before sparse matching/dense
  decoding. Only app-owned historical DMX frame strings migrate, retaining saved/default
  values and identities; ordinary string parameters gain no implicit binary conversion.
- Svelte check passes with zero errors/warnings. All seven Golden UI test files pass:
  44 cases, including five binary transport round trips. Core and DMX qualification remains
  pending. Intermediate compilation errors in exhaustive matches and test setup were corrected;
  they do not provide completion evidence.

Owned handle **15129** is running the development-profile Core parameter/context/protocol/engine
library suites. Poll that exact handle before editing their sources. Then run the DMX owning
selection, script-template/API check, default build, strict workspace gates and generated freshness.
No checkpoint credit or milestone commit/push has been claimed for this unfinished work.

Further review found that a rejected trailing command could suppress reflection of the last
valid admitted frame. Output reflection now retains that final prepared identity until the
queue drains, then applies the existing source/configuration/authored-value fences. The new
batch-command/invalid-script loopback case checks that boundary without admitting a partial frame.
All 93 app UI cases also pass, and generated protocol hashes match a fresh regeneration.
The Core suite remains live; the 75-case DMX selection is queued behind its Cargo build lock.

Core handle **15129** completed successfully: development-profile compilation **9m38s**;
Golden Engine **608 passed / three ignored / 611 listed** (1.30s), parameters **25 passed /
one ignored / 26 listed** (0.05s), context **three passed**. The protocol library has zero unit
cases and is not credited as protocol coverage; its public integration test still needs execution.
DMX handle **24929** now owns the 75-case app selection. Poll it before relevant Rust edits.

Handle **24929** completed: optimized test build **7m46s**, **73 passes / two failures /
75 selected** (9.00s). Both new batch cases and both migration cases pass. The command-deletion
fixture still authored a string into the now-typed command parameter; corrected to bytes without
changing its cancellation assertions. The pacing/continuous-input failure reproduces on the
unchanged artifact with a full test name and backtrace (one case, not the earlier empty short-name
`--exact` selection). Faster input exposes the previously recorded blocking diagnostic mailbox.

The native loop now publishes bounded latest health through independent readiness/warning/input
state, with a dominant terminal error. No actor drain or larger queue is required for output to
advance. The paced FIFO case now deliberately flips input health continuously while consuming
no statuses; added bounded UTF-8 warning and terminal-error regressions. This addresses the
observed stall at the transport boundary; full checkpoint-five recovery/termination remains open.
The standalone protocol integration test also passes (one case, development profile).

Strict workspace/all-target Clippy passes in **1m11s** after the native health fix. Actual root,
Core and touched app formatting passes. The expanded **77-case** DMX selection and default app
build are now the remaining live gates. Core's 608/25/3 owning passes remain valid; no Core
source behavior changed after that execution. Keep the checkpoint open until these app gates pass.

Handle **7498** compiled the corrected optimized test artifact in **5m04s** and passed all
**77 DMX cases / zero failed / zero ignored / 1709 filtered** (6.28s). The same executable
passes the **one script-template/API case** (0.49s) and **77 OSC adapter/runtime cases** (1.86s).
Test artifact SHA-256: `17aa0ad297f4f21915a879696377e19a685bd94a32039e3199147899a76852c7`.
The default app build is still running in **7498**; poll it before relevant source edits.

No new source exceeds 1,000 lines; the largest new source is the 132-line migration fixture.
The five already-oversized touched integration/registry/inspector files remain explicit P12 work;
this milestone does not claim that the repository's source-size gate is clean. Manifest and
lockfiles are unchanged. `origin/main` was fetched and still matches the delivered baseline.

### P07 typed frame publication closure

Handle **7498** is terminal and successful: the default non-test app build passes in **3m41s**.
Default executable SHA-256: `b4560cd3e0446758fb34bd68975f76c3577759defaff6ef9a18384fba6fb1565`.
The optimized test artifact also passes four focused Alchemist catalog/value-bridge regressions
(two tagged exports, one legacy import and one non-finite runtime boundary). These supplement
the 77 DMX, 77 OSC and one script API cases; they do not claim full application-suite coverage.

**P07 closes checkpoint four locally: 4/6 (67%); whole-plan scope is approximately 37%.**
Typed authored/received/command frame buffers and contiguous channel batches are authoritative;
no internal sample persists/reparses a 512-channel JSON string. Saved/default meaning and UUIDs
survive the narrow app migration in sparse and dense projects. Source fences, native receipts,
authored ordering, script functions/callbacks/templates and final valid output reflection remain
covered. Continuous unconsumed health cannot block output; terminal errors remain observable.

Actual root/Core/app formatting, generated freshness, Svelte check, all 44 Golden UI and 93 app UI
tests, strict workspace/all-target Clippy, Core suites and default build pass. All owned jobs are
terminal. Qualification uses controlled workers and loopback, with no physical device, desktop
automation or native-webview claim. Commit and push this verified milestone on `main`.

Two fixed checkpoints remain: **termination/recovery** (including desired-look restoration
without replaying old command receipts) and **complete local qualification**. Do not reopen P03,
claim all of P07 complete, or shrink the full P00-P18 goal. Continue from this delivered boundary.
