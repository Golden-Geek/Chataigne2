# Product-Preserving Migration Progress

Updated: 2026-07-12

The canonical branch is `architecture/aaa-product-rewrite`, started from
`fb0f3a58f3593df8994bf8bd46f88ddd7612f41d`. All named phases are planned as `RUNNABLE`.
An intentionally non-runnable interval may exist only on a private/topic branch and is never a
completed canonical phase.

## Phase Status

| Phase | Required validation | Implementation status | Product gate | Dependency or next proof |
| --- | --- | --- | --- | --- |
| Phase 0 — Branch from `main`, prove the product, and freeze the contract | `RUNNABLE` | Complete | `PASS` | Exact commit `82a72b3ef517aefe32e4a6907e6cba66aab52022`; [six-platform product gate run 29195670582](https://github.com/Golden-Geek/Chataigne2/actions/runs/29195670582) |
| Phase 1A — Form the monorepo by importing the complete working product | `RUNNABLE` | Complete | `NOT_RUN` | Exact-commit Windows product gate passes locally; the canonical six-platform matrix runs after the supercommit is pushed |
| Phase 1B — Modernize and unify the toolchain without changing the product | `RUNNABLE` | Pending | `BLOCKED` | Complete runnable Phase 1A import first |
| Phase 2 — Establish stable product seams and shadow infrastructure | `RUNNABLE` | Pending | `BLOCKED` | Complete product-gated Phase 1B |
| Phase 3 — Extract foundations and `golden-graph` through the live product | `RUNNABLE` | Pending | `BLOCKED` | Phase 2 seams and side-effect-safe shadowing must exist |
| Phase 4 — Migrate Alchemist as a complete authoring-to-runtime slice | `RUNNABLE` | Pending | `BLOCKED` | Complete the common graph cutovers |
| Phase 5 — Migrate statecharts, conditions, contexts, and processors vertically | `RUNNABLE` | Pending | `BLOCKED` | Complete relevant graph and Alchemist boundaries |
| Phase 6 — Replace the runtime center behind the continuously working app | `RUNNABLE` | Pending | `BLOCKED` | Compiled domains and product composition must be proven |
| Phase 7 — Migrate protocol, observation, and UI stores panel by panel | `RUNNABLE` | Pending | `BLOCKED` | Runtime planes and generation semantics must be stable |
| Phase 8 — Migrate every module and specialized product subsystem | `RUNNABLE` | Pending | `BLOCKED` | New runtime/protocol/UI foundations must be runnable |
| Phase 9 — Final qualification, approved UX improvements, and deletion | `RUNNABLE` | Pending | `BLOCKED` | Every parity row and release gate must pass |

## Phase 0 Governance Slice

| Item | Status | Evidence |
| --- | --- | --- |
| Exact branch, product, donor, and gitlink refs | Complete | [Baseline record](baseline.md) |
| Migration policy clarification in `AGENTS.md` | Complete | Policy is explicit about product preservation and governed adapters |
| Required architecture decisions | Complete | [ADR index](../architecture/decisions/README.md) |
| Parity and temporary-adapter field contract | Complete | [Parity ledger schema](parity-ledger-schema.md) |
| Generated parity ledger and registries | Complete | Versioned schemas and generated manifests under `docs/product/` |
| Windows MSVC build and product smoke | Complete | Product gate run `29195670582` |
| macOS build and product smoke | Complete | Product gate run `29195670582` |
| Linux build and product smoke | Complete | Product gate run `29195670582` |
| Linux ARMHF, Linux AArch64, and Windows ARM64 compatibility | Complete | Product gate run `29195670582` |
| Reference visual/interaction evidence | Complete | Native product-gate hook artifacts in run `29195670582` |
| Manual UX/hardware evidence | Recorded | Hardware/manual rows remain explicit in the parity ledger; no platform result is inferred |

## Required Root Workflow Status

| Command or workflow | Status | Required result |
| --- | --- | --- |
| `cargo run` | `PASS` at Phase 0 commit | Complete Chataigne app, real backend, bundled/default UI, connected engine |
| `watch` | `PASS` at Phase 0 commit | One orchestrator, explicit readiness, correct restart/shutdown, no orphan processes |
| `cargo run -- --dev` | `PASS` at Phase 0 commit | Complete app with live frontend/dev server and connected engine |
| Root product gate | `PASS` at Phase 0 commit | Full Rust/UI/product/fixture/Playwright/manifest/loopback/LAN/platform matrix |

No command above is inferred to pass from source inspection or repository provenance.

## Phase-Closing Rules

Every phase-closing supercommit must:

1. keep the complete applicable Chataigne product independently buildable and launchable;
2. update this table and the parity ledger with exact completed, shadowing, cut-over, removed, and
   remaining work;
3. commit only evidence that was genuinely executed;
4. record exact command, commit or tested-tree identity, toolchain, target/features, exit code,
   ignored tests, artifacts, and manual checks;
5. pass the continuous product gate and the three stable root workflow contracts;
6. leave intentionally broken structural work off the canonical branch;
7. avoid deletion until the corresponding parity rows prove the replacement.

Phase 1B and Phase 3 intentionally require multiple focused, runnable supercommits rather than one
opaque phase-sized change.
