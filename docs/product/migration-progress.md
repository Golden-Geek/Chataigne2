# Product-Preserving Migration Progress

Updated: 2026-07-22

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

- State: `CONSTRUCTION`; Phase 9 final qualification and governed deletion is underway. This state
  does not claim complete-product parity beyond the immutable Phase 8 checkpoint.
- Last runnable checkpoint — State: `CHECKPOINT_RUNNABLE`; Phase 8 remains qualified at exact
  commit `b45a9b0a7a01ebee386e24a91daa42f897054bc6`.
- Current subphase: Phase 9A, 9B, and 9D are runnable; the Phase 9B 100,000-value runtime, 10,000-node
  full-workbench, and 622-row functional-parity gates pass locally on Win-x64. The Windows clean
  package and five-minute three-client/hardware soak gates also pass, and product-owner UX review
  is approved. macOS/Linux clean packages and exact-commit cross-platform qualification still
  block the final checkpoint.
- Construction objective: finish the product-owner, soak, clean-package, and cross-platform gates
  recorded in
  [`phase9-qualification.v1.json`](manifests/phase9-qualification.v1.json).
- Affected layers: product evidence records, migration qualification tooling, performance and
  full-workbench fixtures, release packaging, and final governed deletion.
- Expected breakages: `python tools/migration/phase9_readiness.py --json` intentionally exits
  non-zero until the remaining external and duration-bound qualification evidence is recorded. No
  Phase 8 product workflow is expected to regress during the Phase 9 construction interval.
- Focused checks: the 78 migration unit tests, Phase 2 through Phase 9 contract checkers, product
  manifest drift/schema validation, the complete product gate, the short multi-client soak
  rehearsal, and the Windows clean-package workflow all pass.
- Phase 9A identity proof: all 583 capability IDs from the immutable Phase 0 inventory remain in
  the current 622-row inventory. The 39 post-baseline IDs are locked by SHA-256
  `e6183e50674c3d45045f9e503a33060509d1875d7b714f91c9b40d29a5ff33e4`; the focused identity
  command and all 78 migration tests pass. The authored evidence manifest now qualifies all 622
  rows through the current full-product report; identity preservation alone is not used as proof.
- Phase 9B scalar-scale proof: local Win-x64 report
  `target/phase9/scale/phase9-100k-local/phase9-scale-report.json` passed both mandated 100,000-lane
  partitions with 1,000 samples. Worst dense p95/p99 was 3.336/3.433 ms, worst 1%-dirty sparse p95
  was 0.032 ms, worst no-dirty p95 was 0.005 ms, and no 16.67 ms deadline was missed. Digests were
  deterministic across 1/2/4/8 workers and output capacity stayed bounded.
- Phase 9B graph-scale proof: release Win-x64 report
  `target/phase9/graph-scale/phase9-graph-10k-local/phase9-graph-scale-report.json` passed the real
  bundled full-workbench workflow on tested tree
  `0221f9644a62869f92274c61a390d067991ae52f`. The formula editor retained exactly 10,000 graph
  nodes while rendering the 414 visible nodes, below the 1,000-node DOM ceiling. Outliner and
  inspector mutation, live feedback, formula and state-machine interaction, Save/Open Last,
  persistence verification, New-project cleanup, WebSocket traffic, and all browser issue checks
  passed. The browser artifact hash is
  `b1580bd76869cc3baff88eb70ade94927623c22f763e82ee76d6b136238e2048`.
- Current Phase 9 functional-parity proof:
  `target/phase9/product-parity/phase9-final/phase9-product-parity-report.json` qualifies all 622
  rows on tested tree `d37e43bde8b86ed96ade3cdc61e9a2e2b28c8f4b`. The complete product
  report passed the Rust workspace and UI gates, all three root launch workflows, mounted real-app
  mutation and Save/Open Last persistence, OSC loopback, and non-loopback LAN interaction. The
  wrapper SHA-256 is `336e208c8dea9bce73ece98b0512bda4209d451359c4c183127d3ea59032d22e`.
  This evidence does not claim the cross-platform, soak, or signed product-UX parity gates.
- Windows clean-package proof:
  `target/phase9/package/windows-final/phase9-clean-package-report.json` records tested tree
  `d37e43bde8b86ed96ade3cdc61e9a2e2b28c8f4b`. NSIS and MSI bundles were produced; an
  isolated NSIS install ran the canonical ten-step mounted UI workflow from the packaged binary,
  including mutation and Save/Open Last, and the generated uninstaller removed the installation.
  The report SHA-256 is `1c6bc908e081c3c167fe10acb8d98d7787ad4d4c69b8ccd3d0aeafba33a58b87`.
- Phase 9 migration soak proof:
  `target/phase9/soak/phase9-final/phase9-soak-report.json` records tested tree
  `7108f5d91da56dfc07df364707bbcb9eb280b8df`. Three hardware-simulator cycles and a
  five-minute mounted-product interval with three independent clients completed 263 synchronized
  mutations. Each client supplied nine heap and queue samples; median heap growth was 1.0-4.3 MB
  against the 64 MB ceiling, reported queue peak was at most two, every final queue depth was zero,
  and the host logs contained no failure marker. The report SHA-256 is
  `0927697d49add483426d3bcd10cadae635146b72d04337139ea893195e55c7a5`.
- Product UX approval: the product owner reviewed the loaded, interacted, and reloaded captures from
  the complete mounted-app workflow and explicitly approved Phase 9 UX parity. The signed decision
  is recorded in `docs/product/manifests/phase9-ux-approval.v1.json` with SHA-256
  `4a3b4f80250164e8401fb79dcd8894e760158fad57d82bc775fc67d747846ab8`.
- Built-in formula parity proof: the directory-derived qualifier at
  `target/phase9/builtin-formulas/phase9-builtin-formulas-local/phase9-builtin-formulas-report.json`
  matched every bundled JSON to generated inventory and canonical hashes, then passed catalog,
  default-project, palette, read-only, processor, warning, and sparse save/reload coverage with no
  ignored tests. The two current formula rows are qualified; additions to the folder are discovered
  by the build, runtime catalog tests, and qualifier without adding formula names to bundling policy.
- ANode catalog proof: the declaration-driven qualifier at
  `target/phase9/anode-catalog/phase9-anode-catalog-local/phase9-anode-catalog-report.json`
  matched all 37 generated rows to the live registry, exposed every declaration through the real
  formula creation menu, materialized each through the app intent path, computed each live
  signature, and preserved the exact catalog through sparse save/reload on tested tree
  `0933be2295394d507d95334e4b6fea990708b017`. The report SHA-256 is
  `e30eaad67cb6028ace41c951b9406f25b5bfdd9547688be2139359e422f225c2`. The 146-test Alchemist
  suite and 65 active processor tests pass. Two obsolete pre-manager processor fixtures are
  explicitly recorded as ignored. All 37 ANode rows are now fully characterized. Routing covers menu exposure,
  collapsed presentation, generic type inference, pure typed pass-through behavior, absence of
  effects, and persistence. Condition Gate covers every declared mode, hold-last state, trigger
  blocking, whole-ValueSet behavior, per-lane lowering, catalog exposure, and persistence. Math
  covers all five operators, all supported numeric shapes, forced conversion, idle scheduling, and
  zero-divisor diagnostics.
  Constant and Property are fully characterized across every primitive runtime value shape;
  Property additionally covers schema typing, processor overrides, dependency-scoped invalidation,
  shared compile reuse, stable-reference authoring, undo, and reload.
  Debug Value and Debug Log are characterized across every primitive payload shape: Debug Value is
  a pure observable pass-through with no intents, while Debug Log is an effect-only typed intent
  emitter with explicit disabled-node suppression and no fabricated graph output.
  The transform-family matrix characterizes Remap interpolation, extrapolation, and zero-range
  errors; Clamp across every numeric shape and invalid bounds; One Minus, Inverse, and Negate across
  every numeric shape; Pack Vec3 component and ValueSet lane order; both angle-conversion modes; and
  both Cartesian/polar coordinate directions including the origin. Clamp now reports inverted
  bounds as a node diagnostic instead of panicking.
  The pure-node matrix additionally covers all 15 Function modes and dynamic Atan2 arity; all four
  Convert To String formats; Concatenate ordering and decoration; Split delimiter, Unicode, trim,
  and empty-part policies; complete AND/OR/XOR truth tables; all 11 Compare modes with open-generic
  primitive typing; and Gate open/closed behavior with trigger identity preserved.
  The stateful matrix covers every Smooth Filter method with independent lane memory, Speed
  derivatives, Counter reset precedence, all LFO shapes and update-rate holds, every deterministic
  bounded Noise algorithm, all Metronome timing modes and phase outputs, Trigger On/Off edge and
  toggle state machines, and Delay One Tick bootstrap and previous-value feedback semantics.
  The color matrix covers default and authored gradients, stop normalization, every interpolation
  policy, and all RGBA, HSVA, HSLA, and CMYKA conversion/extraction modes. The app-owned manager
  matrix covers Conditions lane override and global fallback, Filters pass-through and empty
  defaults, Inputs revision propagation, and Outputs signal-triggered and explicit-triggered command
  emission without duplicate effects.
- Generator-family parity proof: the source-and-kind-driven qualifier at
  `target/phase9/generator-modules/phase9-generator-modules-local/phase9-generator-modules-report.json`
  matched 26 non-visual rows across Signals (9), Metronomes (9), and Spatializer (8) on tested tree
  `2fc7b23d165a957b44199ac9eb92148106063fcc`; its SHA-256 is
  `31b0f294f5651252d135a813a375fcad878607901e95cefd6f521413b51ebb1b`.
  All 36 focused generator tests passed with zero ignored tests, as did the shared script-template
  API fixture and Phase 8 contract check. Coverage includes catalog and declared-tree creation,
  recursive parameter enablement, deterministic signal cycles and metronome tick multiplicity,
  exact callback payloads, direct-value identity, sparse persistence, script descriptors and
  expansion, every Spatializer weighting mode, Voronoi continuity, rename/layout synchronization,
  and the 512-source by 1,024-target sparse-Delaunay scale fixture. The mounted full-product report
  supplies current functional evidence for the related visual surfaces; product-owner UX signoff
  remains a separate Phase 9 gate.
- Phase 9D finalization proof:
  `target/phase9/finalization/phase9-final-local/phase9-finalization-report.json` passed governed
  deletion, final documentation, manifest drift/schema, and every Phase 2 through Phase 8
  architecture contract on tested tree `1eaf2d055efd46136fd509c86041e0e45c8d6021`. Its
  SHA-256 is `ed072b82413272c3b65d3fd1f678f691bab323dfebce934a36bf5f53cd430806`;
  the Phase 9D dashboard is now runnable with no carried temporary adapter.
- Next named checkpoint: Phase 9 `CHECKPOINT_RUNNABLE` after every recorded gate passes.
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
- 8G proof: the full Spatializer behavior suite passes, including a deterministic
  1,024-target by 512-source scale fixture with a linear-size Delaunay neighbour graph. Dashboard
  backend and UI contracts enforce authored pages/widgets, bindings, drag/drop, resize,
  multi-selection, viewer routing, and live value intents. Local Win-x64 report
  `target/product-gate/20260716T113731Z/product-gate-report.json` passed all 38 required checks.
- 8H focused proof: public `golden_script` fixtures cover export execution, typed host-call budget
  failures, and cached-manifest replacement. Generated manifests enforce all 23 module templates
  plus the shared base template, 51 methods, 51 callbacks, 40 snippets, Action/Mapping hashes, 90
  assets, and every module icon. Local Win-x64 report
  `target/product-gate/20260716T115920Z/product-gate-report.json` passed all 38 required checks.
- 8I focused proof: project/preferences/upload writes use atomic replacement with a last-complete
  backup and versioned recovery journal; a corrupt primary recovers through the existing user
  confirmation flow and is atomically repaired before the next save can rotate it. A 10,002-node
  sparse project round-trips, open-LAN discovery is versioned, and `npm run package:check` builds
  the optimized Tauri application with pinned release tooling.
  Local Win-x64 report `target/product-gate/20260716T125330Z/product-gate-report.json` passed all 38
  required checks, including the real-app save/reload and non-loopback LAN workflows.
- 8J proof: Art-Net and sACN are distinct catalog modules over one bounded DMX
  worker/frame foundation, with protocol loopbacks, latest-wins input diagnostics, reconnect,
  commands, scripts, callbacks, persistence, and app-owned icons. The Node module provides stable
  ID/UUID parameter references for set and trigger operations through commands and scripts. These
  are approved new capabilities and are not recorded as restored baseline parity. Local Win-x64
  report `target/product-gate/phase8-final-candidate/product-gate-report.json` passed all 38
  required checks. Exact commit `b45a9b0a7a01ebee386e24a91daa42f897054bc6` passed native
  Windows/macOS/Linux, Windows ARM64, Linux AArch64, Linux ARMHF, and aggregate qualification in
  [GitHub Actions run 29580856581](https://github.com/Golden-Geek/Chataigne2/actions/runs/29580856581).
- Focused proof: local Win-x64 report
  `target/product-gate/20260716T095204Z/product-gate-report.json` passed all 38 required checks for
  the 8A tree; 7 non-required dependency/platform checks were `NOT_RUN`.
- Last runnable checkpoint: Phase 8 at exact commit
  `b45a9b0a7a01ebee386e24a91daa42f897054bc6`.
- Remaining Phase 8 work: none.

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
| Phase 8 — Migrate every module and specialized product subsystem               | `CHECKPOINT_RUNNABLE` | Complete            | `PASS`                 | Exact commit `b45a9b0a7a01ebee386e24a91daa42f897054bc6`; [six-platform product gate run 29580856581](https://github.com/Golden-Geek/Chataigne2/actions/runs/29580856581)       |
| Phase 9 — Final qualification, approved UX improvements, and deletion          | `CHECKPOINT_RUNNABLE` | `CONSTRUCTION`      | `BLOCKED`              | 622/622 parity, UX, all scale/soak gates, Windows package, documentation, and deletion pass; macOS/Linux packages and cross-platform qualification remain                         |

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

## Phase 8 Module And Specialized-Subsystem Checkpoint

| Subphase | Status         | Current evidence or next proof                                                                                         |
| -------- | -------------- | ---------------------------------------------------------------------------------------------------------------------- |
| 8A       | `RUNNABLE`     | `golden_io` owns pending signaling, reconnect backoff, bounded queues, worker tasks, and deterministic test transports; all 38 required product-gate checks pass |
| 8B       | `RUNNABLE`     | Signal and Metronome have compile-assigned family kernels and deterministic worker fixtures; all 38 required product-gate checks pass |
| 8C       | `RUNNABLE`     | OSC and MIDI have compile-assigned family kernels and enforced parity/recovery fixtures; all 38 required product-gate checks pass |
| 8D       | `RUNNABLE`     | Transport families have compile-assigned kernels, bounded queues, and enforced recovery/loopback fixtures; all 38 required product-gate checks pass |
| 8E       | `RUNNABLE`     | All controller/hardware entries have family kernels and named deterministic adapter evidence; all 38 required product-gate checks pass |
| 8F       | `RUNNABLE`     | App Control and OS have family kernels, named workers, lifecycle, and enforced platform fixtures; all 38 required product-gate checks pass |
| 8G       | `RUNNABLE`     | Spatializer uses sparse Delaunay topology at the declared scale; dashboard/editor workflows are enforced; all 38 required product-gate checks pass |
| 8H       | `RUNNABLE`     | Public runtime/cache/budget contracts and the complete generated script/asset/registration inventory are enforced; all 38 required product-gate checks pass |
| 8I       | `RUNNABLE`     | Durable project recovery, desktop/headless/LAN discovery, active native bundles, signing/notarization hooks, and release assets are enforced; all 38 required product-gate checks pass |
| 8J       | `RUNNABLE`     | Art-Net/sACN/DMX and Node implementation, focused contracts, and all 38 required local product-gate checks pass       |

## Required Root Workflow Status

| Command or workflow  | Status                         | Required result                                                                            |
| -------------------- | ------------------------------ | ------------------------------------------------------------------------------------------ |
| `cargo run`          | `PASS` on Phase 8 checkpoint tree | Complete Chataigne app, real backend, bundled/default UI, connected engine                 |
| `watch`              | `PASS` on Phase 8 checkpoint tree | One orchestrator, explicit readiness, correct restart/shutdown, and released product ports |
| `cargo run -- --dev` | `PASS` on Phase 8 checkpoint tree | Complete app with live frontend/dev server and connected engine                            |
| Root product gate    | `PASS` on Phase 8 checkpoint tree | Full Rust/UI/product/fixture/Playwright/manifest/loopback/LAN/cross-platform matrix        |

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
