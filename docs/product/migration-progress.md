# Product-Preserving Migration Progress

Updated: 2026-07-14

The canonical branch is `architecture/aaa-product-rewrite`, started from
`fb0f3a58f3593df8994bf8bd46f88ddd7612f41d`. All named phases are planned as `RUNNABLE`.
An intentionally non-runnable interval may exist only on a private/topic branch and is never a
completed canonical phase.

## Validation Cadence

Ordinary migration work uses the complete applicable local product gate on
`x86_64-pc-windows-msvc`. Cross-platform CI is a qualification gate at the end of Phases 1B, 3, 6,
8, and 9, and is also required for changes that alter platform hosts, native dependencies, target
selection, or packaging. Deferred platform rows remain `NOT_RUN`; a Windows pass is never recorded
as evidence for another platform.

The long-lived migration branch does not require a permanently open pull request. Focused PRs are
opened when a review, named qualification, or merge point is ready. This keeps routine pushes local
while preserving full cross-platform closure before affected cutovers and final integration.

## Phase Status

| Phase                                                                          | Required validation | Implementation status | Product gate             | Dependency or next proof                                                                                                                                                    |
| ------------------------------------------------------------------------------ | ------------------- | --------------------- | ------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Phase 0 — Branch from `main`, prove the product, and freeze the contract       | `RUNNABLE`          | Complete              | `PASS`                   | Exact commit `82a72b3ef517aefe32e4a6907e6cba66aab52022`; [six-platform product gate run 29195670582](https://github.com/Golden-Geek/Chataigne2/actions/runs/29195670582)    |
| Phase 1A — Form the monorepo by importing the complete working product         | `RUNNABLE`          | Complete              | `PASS` (Win-x64 local)   | Cross-platform import qualification is deliberately combined with the Phase 1B toolchain qualification instead of validating a toolchain that Phase 1B immediately replaces |
| Phase 1B — Modernize and unify the toolchain without changing the product      | `RUNNABLE`          | Complete              | `PASS`                   | Exact commit `0e780f9025be2b86eed3f5474ed257e0da898e2a`; [six-platform product gate run 29323403758](https://github.com/Golden-Geek/Chataigne2/actions/runs/29323403758)    |
| Phase 2 — Establish stable product seams and shadow infrastructure             | `RUNNABLE`          | Complete              | `PASS` (Win-x64 local)   | Tested tree based on `2ce92015c60a9958402c0517417c5e8988b358a4`; local report `target/product-gate/20260714T115625Z/product-gate-report.json`                                  |
| Phase 3 — Extract foundations and `golden-graph` through the live product      | `RUNNABLE`          | In progress           | Slice 1 `PASS` (Win-x64) | Identifiers and canonical values are cut over and product-gated; extract parameters and context next                                                                         |
| Phase 4 — Migrate Alchemist as a complete authoring-to-runtime slice           | `RUNNABLE`          | Pending               | `BLOCKED`                | Complete the common graph cutovers                                                                                                                                          |
| Phase 5 — Migrate statecharts, conditions, contexts, and processors vertically | `RUNNABLE`          | Pending               | `BLOCKED`                | Complete relevant graph and Alchemist boundaries                                                                                                                            |
| Phase 6 — Replace the runtime center behind the continuously working app       | `RUNNABLE`          | Pending               | `BLOCKED`                | Compiled domains and product composition must be proven                                                                                                                     |
| Phase 7 — Migrate protocol, observation, and UI stores panel by panel          | `RUNNABLE`          | Pending               | `BLOCKED`                | Runtime planes and generation semantics must be stable                                                                                                                      |
| Phase 8 — Migrate every module and specialized product subsystem               | `RUNNABLE`          | Pending               | `BLOCKED`                | New runtime/protocol/UI foundations must be runnable                                                                                                                        |
| Phase 9 — Final qualification, approved UX improvements, and deletion          | `RUNNABLE`          | Pending               | `BLOCKED`                | Every parity row and release gate must pass                                                                                                                                 |

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

| Slice                                  | Status   | Evidence or remaining governed migration work                                                                                                      |
| -------------------------------------- | -------- | ---------------------------------------------------------------------------------------------------------------------------------------------------- |
| Production application facade         | Complete | Project transactions, graph edits, runtime values, observation, persistence, and host lifecycle route through `golden_application`/`ProductionRuntime` |
| Side-effect-safe shadow infrastructure | Complete | Semantic-digest shadow execution is pure and cannot acquire effect authority; matching, mismatch, and failure behavior have executable Rust tests     |
| Deterministic module I/O boundary      | Complete | Injectable clock, input/output recordings, and effect-authority-gated output dispatch are tested; embedded module I/O remains a governed Phase 8 adapter |
| Adapter and seam dashboard             | Complete | Revision 1 is recorded in [`manifests/phase2-seams.v1.json`](manifests/phase2-seams.v1.json) with owners, expiry phases, deletion criteria, and tests   |
| Package and host boundary contracts    | Complete | `tools/migration/check_phase2_contracts.py` rejects app-policy imports and raw shared engine ownership in transport/host code                          |
| Win-x64 product qualification          | `PASS`   | Local report `target/product-gate/20260714T115625Z/product-gate-report.json`; all required results passed and macOS/Linux were non-required `NOT_RUN`   |

## Phase 3 Foundation Cutovers

| Slice                       | Status      | Evidence or next proof                                                                                   |
| --------------------------- | ----------- | -------------------------------------------------------------------------------------------------------- |
| Stable model identities     | Cut over    | `golden_model` owns wire-compatible `NodeId`, `NodeUuid`, and `DeclId`; engine ownership is contract-tested |
| Canonical runtime values    | Cut over    | `golden_values::Value` is used by Alchemist; f64 colors and parameter extension round trips are tested   |
| Parameters and context      | Pending     | Extract behind current production APIs in the next independently runnable supercommit                    |
| Common graph and graph UI   | Pending     | Begin only after parameter and context ownership is stable                                               |
| Revisioned cutover evidence | `PASS`      | [`manifests/phase3-cutovers.v1.json`](manifests/phase3-cutovers.v1.json); local report `target/product-gate/20260714T140050Z/product-gate-report.json` |

## Required Root Workflow Status

| Command or workflow  | Status                              | Required result                                                                            |
| -------------------- | ----------------------------------- | ------------------------------------------------------------------------------------------ |
| `cargo run`          | `PASS` on Phase 3 slice 1 tree | Complete Chataigne app, real backend, bundled/default UI, connected engine                 |
| `watch`              | `PASS` on Phase 3 slice 1 tree | One orchestrator, explicit readiness, correct restart/shutdown, and released product ports |
| `cargo run -- --dev` | `PASS` on Phase 3 slice 1 tree | Complete app with live frontend/dev server and connected engine                            |
| Root product gate    | `PASS` on Phase 3 slice 1 tree | Full Rust/UI/product/fixture/Playwright/manifest/loopback/LAN/Windows matrix               |

No command above is inferred to pass from source inspection or repository provenance.

## Phase-Closing Rules

Every phase-closing supercommit must:

1. keep the complete applicable Chataigne product independently buildable and launchable;
2. update this table and the parity ledger with exact completed, shadowing, cut-over, removed, and
   remaining work;
3. commit only evidence that was genuinely executed;
4. record exact command, commit or tested-tree identity, toolchain, target/features, exit code,
   ignored tests, artifacts, and manual checks;
5. pass the applicable local Win-x64 product gate and the three stable root workflow contracts;
6. leave intentionally broken structural work off the canonical branch;
7. avoid deletion until the corresponding parity rows prove the replacement.

Phases 1B, 3, 6, 8, and 9 additionally require the cross-platform qualification profile. Changes
to host startup, native dependencies, target selection, packaging, or platform-specific code also
require that profile before the affected cutover is accepted. Deferred cross-platform evidence is
recorded as `NOT_RUN` and blocks the applicable qualification point, not unrelated intermediate
migration slices.

Phase 1B and Phase 3 intentionally require multiple focused, runnable supercommits rather than one
opaque phase-sized change.
