# Product-Preserving Migration Progress

Updated: 2026-07-16

The canonical branch is `architecture/aaa-product-rewrite`, started from
`fb0f3a58f3593df8994bf8bd46f88ddd7612f41d`. Every named phase ends at a
`CHECKPOINT_RUNNABLE` gate. The canonical branch may carry a declared `CONSTRUCTION` interval
between checkpoints; such an interval never claims complete-product parity.

## Validation Cadence

Ordinary construction work runs focused compile, unit, contract, serialization, migration, and
performance checks for the affected layers. The complete local product gate on
`x86_64-pc-windows-msvc` runs at the end of Phases 4, 5, 6, 7, 8, and 9. Cross-platform CI remains a
qualification gate at the end of Phases 1B, 3, 6, 8, and 9, and is also required for changes that
alter platform hosts, native dependencies, target selection, or packaging. Deferred platform rows
remain `NOT_RUN`; a Windows pass is never recorded as evidence for another platform.

## Current Checkpoint

- State: `CONSTRUCTION`; the Phase 8 module and specialized-subsystem interval is open from the
  immutable Phase 7 checkpoint `1a1609a`.
- Current subphase: 8G Spatializer, dashboards, custom editors, live workflows, and scale
  fixtures. Phase 8A established app-agnostic IO ownership in `golden_io`; Phases 8B through 8F
  cut generator, protocol, controller, App Control, and OS families over to compiled kernels.
- 8B proof: deterministic worker fixtures prove fixed-delta values, cycles, tick multiplicity, and
  counts. Local Win-x64 report `target/product-gate/20260716T101347Z/product-gate-report.json`
  passed all 38 required checks; 7 non-required checks were `NOT_RUN`.
- 8C proof: OSC and MIDI declare distinct compiled family kernels. Existing loopback,
  codec, dynamic-value, command, script, persistence, interface-refresh, and port-recovery fixtures
  are enforced by the Phase 8 contract checker. Local Win-x64 report
  `target/product-gate/20260716T103001Z/product-gate-report.json` passed all 38 required checks.
- 8D proof: Serial, MQTT, HTTP, TCP, UDP, and WebSocket declare family kernels; TCP and
  WebSocket client/server share their family key. Incoming stream values and HTTP/MQTT request
  channels are bounded, with explicit overload behavior. Local Win-x64 report
  `target/product-gate/20260716T104707Z/product-gate-report.json` passed all 38 required checks.
- 8E proof: all eight controller/hardware modules declare distinct family kernels. The
  hardware evidence matrix records each platform scope, deterministic adapter, executable fixture,
  and physical-device status. Local Win-x64 report
  `target/product-gate/20260716T110028Z/product-gate-report.json` passed all 38 required checks.
- 8F proof: App Control and OS declare family kernels; their recurring system inspection runs on
  named background workers with explicit stop/unpark/join lifecycle and platform-scoped
  implementations. Local Win-x64 report
  `target/product-gate/20260716T111256Z/product-gate-report.json` passed all 38 required checks.
- Focused proof: local Win-x64 report
  `target/product-gate/20260716T095204Z/product-gate-report.json` passed all 38 required checks for
  the 8A tree; 7 non-required dependency/platform checks were `NOT_RUN`.
- Last runnable checkpoint: Phase 8F, based on Phase 8E commit `ec0486b`.
- Remaining Phase 8 work: gate module families 8G through 8J and close with the
  full local and required cross-platform product qualifications.

The long-lived migration branch does not require a permanently open pull request. Focused PRs are
opened when a review, named qualification, or merge point is ready. This keeps routine pushes local
while preserving full cross-platform closure before affected cutovers and final integration.

## Phase Status

| Phase                                                                          | Required validation | Implementation status | Product gate           | Dependency or next proof                                                                                                                                                        |
| ------------------------------------------------------------------------------ | ------------------- | --------------------- | ---------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Phase 0 — Branch from `main`, prove the product, and freeze the contract       | `CHECKPOINT_RUNNABLE` | Complete            | `PASS`                 | Exact commit `82a72b3ef517aefe32e4a6907e6cba66aab52022`; [six-platform product gate run 29195670582](https://github.com/Golden-Geek/Chataigne2/actions/runs/29195670582)        |
| Phase 1A — Form the monorepo by importing the complete working product         | `CHECKPOINT_RUNNABLE` | Complete            | `PASS` (Win-x64 local) | Cross-platform import qualification is deliberately combined with the Phase 1B toolchain qualification instead of validating a toolchain that Phase 1B immediately replaces     |
| Phase 1B — Modernize and unify the toolchain without changing the product      | `CHECKPOINT_RUNNABLE` | Complete            | `PASS`                 | Exact commit `0e780f9025be2b86eed3f5474ed257e0da898e2a`; [six-platform product gate run 29323403758](https://github.com/Golden-Geek/Chataigne2/actions/runs/29323403758)        |
| Phase 2 — Establish stable product seams and shadow infrastructure             | `CHECKPOINT_RUNNABLE` | Complete            | `PASS` (Win-x64 local) | Tested tree based on `2ce92015c60a9958402c0517417c5e8988b358a4`; local report `target/product-gate/20260714T115625Z/product-gate-report.json`                                   |
| Phase 3 — Extract foundations and `golden-graph` through the live product      | `CHECKPOINT_RUNNABLE` | Complete            | `PASS`                 | Exact commit `1f23dbef04e544f215611771b5003489f67752f3`; [Windows/macOS/Linux product gate run 29366201894](https://github.com/Golden-Geek/Chataigne2/actions/runs/29366201894) |
| Phase 4 — Migrate Alchemist as a complete authoring-to-runtime slice           | `CHECKPOINT_RUNNABLE` | Complete            | `PASS` (Win-x64 local) | Local report `target/product-gate/20260715T130619Z/product-gate-report.json`; all 32 required checks passed                                                                       |
| Phase 5 — Migrate statecharts, conditions, contexts, and processors vertically | `CHECKPOINT_RUNNABLE` | Complete            | `PASS` (Win-x64 local) | Local report `target/product-gate/20260715T163517Z/product-gate-report.json`; all 33 required checks passed                                                                       |
| Phase 6 — Replace the runtime center behind the continuously working app       | `CHECKPOINT_RUNNABLE` | Complete            | `PASS`                 | Exact commit `c1e604f95cd11e7d17e4db31686b0caadd2bae10`; [six-platform product gate run 29450944686](https://github.com/Golden-Geek/Chataigne2/actions/runs/29450944686)       |
| Phase 7 — Migrate protocol, observation, and UI stores panel by panel          | `CHECKPOINT_RUNNABLE` | Complete            | `PASS` (Win-x64 local) | Tested tree based on `6e02f0f6300e550e18b64aec324c5a15f2be4ebe`; local report `target/product-gate/20260716T070444Z/product-gate-report.json`; all 37 required checks passed |
| Phase 8 — Migrate every module and specialized product subsystem               | `CHECKPOINT_RUNNABLE` | `CONSTRUCTION`      | `PASS` (8F Win-x64)    | System kernels passed `target/product-gate/20260716T111256Z/product-gate-report.json`; 8G is next                                                                                |
| Phase 9 — Final qualification, approved UX improvements, and deletion          | `CHECKPOINT_RUNNABLE` | Pending             | `BLOCKED`              | Every parity row and release gate must pass                                                                                                                                     |

## Phase 0 Governance Slice

| Item                                                        | Status   | Evidence                                                                                  |
| ----------------------------------------------------------- | -------- | ----------------------------------------------------------------------------------------- |
| Exact branch, product, donor, and gitlink refs              | Complete | [Baseline record](baseline.md)                                                            |
| Migration policy clarification in `AGENTS.md`               | Complete | Policy is explicit about product preservation and governed adapters                       |
| Required architecture decisions                             | Complete | [ADR index](../architecture/decisions/README.md)                                          |
| Parity and temporary-adapter field contract                 | Complete | [Parity ledger schema](parity-ledger-schema.md)                                           |
| Generated parity ledger and registries                      | Complete | Versioned schemas and generated manifests under `docs/product/`                           |
| Windows MSVC build and product smoke                        | Complete | Product gate run `29195670582`                                                            |
| macOS build and product smoke                               | Complete | Product gate run `29195670582`                                                            |
| Linux build and product smoke                               | Complete | Product gate run `29195670582`                                                            |
| Linux ARMHF, Linux AArch64, and Windows ARM64 compatibility | Complete | Product gate run `29195670582`                                                            |
| Reference visual/interaction evidence                       | Complete | Native product-gate hook artifacts in run `29195670582`                                   |
| Manual UX/hardware evidence                                 | Recorded | Hardware/manual rows remain explicit in the parity ledger; no platform result is inferred |

## Phase 1B Toolchain Modernization

| Slice                                 | Status   | Evidence or next proof                                                                                                                                                                                                                                                                            |
| ------------------------------------- | -------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Validation cadence                    | Complete | The product gate defaults to the current platform; CI, benchmark, and cross-platform product workflows run on `main` or explicit dispatch instead of every PR push                                                                                                                                |
| Persistent migration PR               | Removed  | Stale draft PR `#1` closed; the migration branch and commits remain intact                                                                                                                                                                                                                        |
| Rust/native toolchain                 | Complete | Rust/Cargo 1.97.0, current compatible Cargo dependencies, Tauri 2.11.5, and Buttplug 10.0.3 pass the Win-x64 gate                                                                                                                                                                                 |
| JavaScript/UI toolchain               | Complete | Node 26.5.0/npm 11.17.0 and the standard official Svelte CLI stack pass check, lint, unit, production-build, and real-app browser gates                                                                                                                                                           |
| Win-x64 product qualification         | `PASS`   | Local report `target/product-gate/20260713T170046Z/product-gate-report.json`; all 32 required Windows and dependency-profile results passed and macOS/Linux were non-required `NOT_RUN`                                                                                                           |
| Developer orchestration               | Complete | Canonical first-clone scripts, root/editor/debug commands, Python diagnostics, dependency qualification, release/benchmark tasks, cache policy, and onboarding are product-gated                                                                                                                  |
| Dependency governance                 | Complete | Pinned cargo-deny/cargo-machete, RustSec/license/source/bans policy, unused dependency cleanup, reviewed duplicate baseline, and npm production audit pass                                                                                                                                        |
| Cross-platform Phase 1B qualification | `PASS`   | Exact commit `0e780f9025be2b86eed3f5474ed257e0da898e2a`; native Windows, macOS, and Linux product gates plus Linux ARMHF, Linux AArch64, Windows ARM64 compatibility and exact-commit aggregation passed in [run 29323403758](https://github.com/Golden-Geek/Chataigne2/actions/runs/29323403758) |

## Phase 2 Stable Product Seams

| Slice                                  | Status   | Evidence or remaining governed migration work                                                                                                            |
| -------------------------------------- | -------- | -------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Production application facade          | Complete | Project transactions, graph edits, runtime values, observation, persistence, and host lifecycle route through `golden_application`/`ProductionRuntime`   |
| Side-effect-safe shadow infrastructure | Complete | Semantic-digest shadow execution is pure and cannot acquire effect authority; matching, mismatch, and failure behavior have executable Rust tests        |
| Deterministic module I/O boundary      | Complete | Injectable clock, input/output recordings, and effect-authority-gated output dispatch are tested; embedded module I/O remains a governed Phase 8 adapter |
| Adapter and seam dashboard             | Complete | Revision 1 is recorded in [`manifests/phase2-seams.v1.json`](manifests/phase2-seams.v1.json) with owners, expiry phases, deletion criteria, and tests    |
| Package and host boundary contracts    | Complete | `tools/migration/check_phase2_contracts.py` rejects app-policy imports and raw shared engine ownership in transport/host code                            |
| Win-x64 product qualification          | `PASS`   | Local report `target/product-gate/20260714T115625Z/product-gate-report.json`; all required results passed and macOS/Linux were non-required `NOT_RUN`    |

## Phase 3 Foundation Cutovers

| Slice                       | Status   | Evidence or next proof                                                                                                                                                                                                                                                  |
| --------------------------- | -------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Stable model identities     | Cut over | `golden_model` owns wire-compatible `NodeId`, `NodeUuid`, and `DeclId`; engine ownership is contract-tested                                                                                                                                                             |
| Canonical runtime values    | Cut over | `golden_values::Value` is the only public value API; Alchemist and Chataigne import it directly, and the retired compatibility alias is contract-tested                                                                                                                 |
| Parameters and context      | Cut over | `golden_parameters` and `golden_context` own the contracts; engine paths are governed temporary re-exports                                                                                                                                                              |
| Common graph contract       | Adapted  | `golden_graph` owns typed domains, domain-declared connection policies, indexed documents, atomic transactions, revisions/deltas, presentation, traversal, and persistence envelopes                                                                                    |
| Typed test graph domain     | Cut over | Contract, rollback, persistence, traversal, and 10,000-node localized mutation tests use `TestDomain`                                                                                                                                                                   |
| Alchemist graph domain      | Adapted  | Typed ANode payloads, declaration-driven stable ports, localized validation, common persistence, and a pure governed old/new document adapter have executable tests                                                                                                     |
| Statechart graph domain     | Adapted  | Typed state, port, transition, hierarchy, active-configuration, and runtime metadata preserve parallel and multi-incoming transitions through a pure governed old/new document adapter                                                                                  |
| Graph UI                    | Adapted  | `golden_graph_ui` owns the revisioned presentation contract, generic canvas, spatial visible-node queries, and incident-edge indexes; the old transport bridge is governed through Phase 7                                                                              |
| Revisioned cutover evidence | `PASS`   | [`manifests/phase3-cutovers.v1.json`](manifests/phase3-cutovers.v1.json) revision 8; local report `target/product-gate/20260714T181319Z/product-gate-report.json`; [cross-platform run 29366201894](https://github.com/Golden-Geek/Chataigne2/actions/runs/29366201894) |

## Phase 4 Alchemist Vertical Migration

| Slice                              | Status      | Evidence or next proof                                                                                                  |
| ---------------------------------- | ----------- | ----------------------------------------------------------------------------------------------------------------------- |
| Product ownership decision         | Complete    | [ADR 0008](../architecture/decisions/0008-chataigne-owned-alchemist.md) makes Alchemist and its processor composition app-owned |
| Rust ownership relocation          | Implemented | The complete crate moved to `apps/chataigne/alchemist` as `chataigne_alchemist`; targeted all-target compilation passed |
| UI ownership relocation            | Implemented | Real Alchemist UI and its formula/lock assets are app-owned; the imported `golden_alchemist_ui` package was removed    |
| Former Golden package names        | Removed     | Rust consumers use `chataigne_alchemist`; the old UI package, imports, and workspace paths are gone                    |
| Generic graph dependency direction | Verified    | Phase 3 foundation and graph UI ownership contracts pass at the relocated path                                         |
| Typed formula authoring document   | Cut over    | `AlchemistFormula::graph` is an app-owned `AlchemistGraphDocument`; overrides commit through one revisioned transaction |
| Formula persistence                | Cut over    | Reads and writes use the versioned typed graph envelope; no shipped legacy Formula graph payload was found             |
| Compiler and type-solver boundary  | Cut over    | Compiler and solver consume typed document semantics directly; no public or internal whole-graph legacy lowering remains |
| Managed pipeline graph builders    | Cut over    | Formula filter lowering and state-machine `ValueSet` builders commit typed documents atomically without the legacy adapter |
| Production graph construction      | Cut over    | Live Formula snapshots and transition guard/effect runtimes preserve identity and compile typed documents without conversion |
| Former graph model and adapters    | Removed     | Legacy graph storage, serializer, conversion adapter, and compatibility reads are absent from Rust source and shipped assets |
| Revisioned cutover evidence        | Complete    | [`manifests/phase4-cutovers.v1.json`](manifests/phase4-cutovers.v1.json) revision 7 and `tools/migration/check_phase4_contracts.py` |
| Win-x64 product qualification      | `PASS`      | Local report `target/product-gate/20260715T130619Z/product-gate-report.json`; all 32 required checks passed and 7 non-required checks were `NOT_RUN` |

## Phase 5 Statechart, Condition, Context, And Processor Vertical

| Slice                                    | Status   | Evidence or next proof                                                                                         |
| ---------------------------------------- | -------- | -------------------------------------------------------------------------------------------------------------- |
| Canonical statechart graph document      | Cut over | `Statechart` owns `StatechartGraphDocument`; mutations use graph transactions and the former adapter is gone  |
| Statechart UI document                   | Cut over | `StateMachinePanel` uses `golden_statechart_ui::StatechartDocumentView` with the existing Golden graph canvas |
| Compiled condition IR                    | Cut over | Input Value, Input Node, Group, and Script compile to flat instructions, bindings, observations, and dense state |
| Steady-state condition execution         | Cut over | Runtime and inspector paths consume compiled programs/observations without walking authored condition nodes   |
| Processor ownership                      | Relocated | Processor, context/lane, `ValueSet`, and managed pipelines live in `apps/chataigne/processor`                |
| Context lane compilation and migration   | Verified | Stable lane keys preserve retained state; P50-L1 and P5-L127 backend fixtures pass                           |
| Live lane UI                             | Verified | UI projection fixtures cover P50-L1 and P5-L127, and the mounted real-app workflow passes                    |
| Action and Mapping composition           | Cut over | The two shipped formulas use the single Processor, condition, `ValueSet`, and output-intent path              |
| Pure semantic shadow comparison          | Verified | Compiled comparator results match reference outcomes without any effect host                                 |
| Revisioned cutover evidence              | Complete | [`manifests/phase5-cutovers.v1.json`](manifests/phase5-cutovers.v1.json) and `tools/migration/check_phase5_contracts.py` |
| Win-x64 product qualification            | `PASS`   | Local report `target/product-gate/20260715T163517Z/product-gate-report.json`; all 33 required checks passed and 7 non-required checks were `NOT_RUN` |

## Phase 6 Runtime-Center Cutover And Qualification

| Slice                                  | Status       | Evidence or remaining work                                                                                              |
| -------------------------------------- | ------------ | ----------------------------------------------------------------------------------------------------------------------- |
| Actor-owned control plane              | Cut over     | `ProductionRuntime` owns `ProductionState` through `golden_runtime::ControlActor`; hosts and transports cannot lock it |
| Immutable generation/compiler plane    | Cut over     | Async compilation, atomic publication, compatible state migration, and old-generation continuity run in production   |
| Dense input/data plane                 | Cut over     | `RuntimeValues` publishes through compiled slots with latest/lossless delivery and race-safe generation handoff       |
| Persistent scheduler and effect commit | Cut over     | Dense inputs and due domain callbacks use compile-assigned work IDs; stable actor commit and the production OSC loopback prove external order |
| Runtime diagnostics UI                 | Cut over     | Generated protocol metrics surface through the existing engine-rate performance indicator                              |
| App-domain semantic runtime cutover    | Cut over     | Production uses `run_tick_with_compiled_schedule`; `Engine::run_tick` is rollback-only and mutable app-node kernels are governed through Phase 8 |
| Win-x64 product qualification          | `PASS`       | Report `target/product-gate/20260715T201806Z/product-gate-report.json`: all 35 required checks passed; 8 non-required dependency/platform checks were `NOT_RUN` |
| Cross-platform qualification           | `PASS`       | Exact commit `c1e604f95cd11e7d17e4db31686b0caadd2bae10`; native Windows/macOS/Linux, three compatibility targets, and aggregate exact-commit reporting passed in [run 29450944686](https://github.com/Golden-Geek/Chataigne2/actions/runs/29450944686) |

## Phase 7 Generated Protocol, Observation, And Panel Migration

| Slice                                  | Status   | Evidence or remaining work                                                                                              |
| -------------------------------------- | -------- | ----------------------------------------------------------------------------------------------------------------------- |
| Generated multi-plane protocol         | Cut over | Rust owns the versioned client/server messages and codegen emits every TypeScript protocol declaration                 |
| Control lifecycle                      | Cut over | Intents report received, accepted, applied, or rejected without exposing an engine lock to transport clients           |
| Interests, replay, and scoped resync   | Cut over | View-scoped plane interests filter deltas; snapshots and replay use the read model over the same WebSocket protocol    |
| Observation backpressure               | Cut over | Bounded queues coalesce values/observations/previews, preserve structure/triggers, and isolate reliable-queue overflow |
| Coherent UI frame commit               | Cut over | Golden UI stages merged EventTime-ordered plane deltas and advances cursors only on `requestAnimationFrame` commit     |
| Panel-area migrations 7A through 7F    | Cut over | Existing workbench, authoring, graph, Alchemist, state-machine, module, specialized, packaged, and LAN surfaces use the final client/store path |
| Old protocol/runtime HTTP adapter      | Removed  | Hand-maintained WebSocket declarations, runtime HTTP polling/fallback, and `createHttpUiClient` are absent             |
| Revisioned cutover evidence            | Complete | [`manifests/phase7-cutovers.v1.json`](manifests/phase7-cutovers.v1.json) and `tools/migration/check_phase7_contracts.py` |
| Win-x64 product qualification          | `PASS`   | Report `target/product-gate/20260716T070444Z/product-gate-report.json`: all 37 required checks passed; 7 non-required checks were `NOT_RUN` |

## Phase 8 Module And Specialized-Subsystem Construction

| Subphase | Status         | Current evidence or next proof                                                                                         |
| -------- | -------------- | ---------------------------------------------------------------------------------------------------------------------- |
| 8A       | `RUNNABLE`     | `golden_io` owns pending signaling, reconnect backoff, bounded queues, worker tasks, and deterministic test transports; all 38 required product-gate checks pass |
| 8B       | `RUNNABLE`     | Signal and Metronome have compile-assigned family kernels and deterministic worker fixtures; all 38 required product-gate checks pass |
| 8C       | `RUNNABLE`     | OSC and MIDI have compile-assigned family kernels and enforced parity/recovery fixtures; all 38 required product-gate checks pass |
| 8D       | `RUNNABLE`     | Transport families have compile-assigned kernels, bounded queues, and enforced recovery/loopback fixtures; all 38 required product-gate checks pass |
| 8E       | `RUNNABLE`     | All controller/hardware entries have family kernels and named deterministic adapter evidence; all 38 required product-gate checks pass |
| 8F       | `RUNNABLE`     | App Control and OS have family kernels, named workers, lifecycle, and enforced platform fixtures; all 38 required product-gate checks pass |
| 8G       | Pending        | Gate Spatializer, dashboards, custom editors, live workflows, and scale fixtures                                      |
| 8H       | Pending        | Gate the shared script host and every method, callback, snippet/template, asset, icon, and registration               |
| 8I       | Pending        | Gate persistence/recovery, desktop/headless/LAN hosts, discovery, packaging, and release assets                       |
| 8J       | Pending        | Implement approved Art-Net/sACN/DMX and Node modules to the complete module standard                                  |

## Required Root Workflow Status

| Command or workflow  | Status                         | Required result                                                                            |
| -------------------- | ------------------------------ | ------------------------------------------------------------------------------------------ |
| `cargo run`          | `PASS` on Phase 7 checkpoint tree | Complete Chataigne app, real backend, bundled/default UI, connected engine                 |
| `watch`              | `PASS` on Phase 7 checkpoint tree | One orchestrator, explicit readiness, correct restart/shutdown, and released product ports |
| `cargo run -- --dev` | `PASS` on Phase 7 checkpoint tree | Complete app with live frontend/dev server and connected engine                            |
| Root product gate    | `PASS` on Phase 7 checkpoint tree | Full Rust/UI/product/fixture/Playwright/manifest/loopback/LAN/Windows matrix               |

No command above is inferred to pass from source inspection or repository provenance.

## Phase-Closing Rules

Every named runnable checkpoint must:

1. keep the complete applicable Chataigne product independently buildable and launchable;
2. update this table and the parity ledger with exact completed, shadowing, cut-over, removed, and
   remaining work;
3. commit only evidence that was genuinely executed;
4. record exact command, commit or tested-tree identity, toolchain, target/features, exit code,
   ignored tests, artifacts, and manual checks;
5. pass the applicable local Win-x64 product gate and the three stable root workflow contracts;
6. close or explicitly carry forward every construction interval and temporary adapter in scope;
7. prove the corresponding parity rows after any direct replacement or deletion.

Phases 1B, 3, 6, 8, and 9 additionally require the cross-platform qualification profile. Changes
to host startup, native dependencies, target selection, packaging, or platform-specific code also
require that profile before the affected cutover is accepted. Deferred cross-platform evidence is
recorded as `NOT_RUN` and blocks the applicable qualification point, not unrelated intermediate
migration slices.

Construction commits remain focused and reviewable even though they are not individually required
to launch the complete product.
