# Built-in Mapping implementation status

## Summary

- Overall: IN_PROGRESS
- Baseline commit: `b6ac86eb702d703560593c108b599f95545a417c` (local `main` was one commit ahead of `origin/main` at task start)
- Working branch and approved remote: `codex/builtin-mapping`; `origin` = `git@github.com:Golden-Geek/Chataigne2.git`
- Active phase: 01 — direct results and native handoff (not started)
- Last validated implementation commit: `0ba08c66eee40b65c1e91cd78816549f70d657fc` (Phase 00)
- Last verified remote implementation commit: `0ba08c66eee40b65c1e91cd78816549f70d657fc` (Phase 00)
- Current blockers: none; the ambient `CPAL_ASIO_DIR` points at an incomplete SDK, so Windows default-feature checks require the repository's pinned `tools/asio.ps1` wrapper
- Product checks still outstanding: desktop/headless/watch and interactive product smoke checks; expanded Mapping qualification benchmarks; all M01–M20 acceptance cases
- Next concrete action: implement Phase 01 direct output slots and native managed ValueSet handoff
- Last updated: 2026-09-13T09:37:15+02:00

## Phase ledger

| Phase | Status | Implementation | Validation | Delivery | CI | Evidence / blocker |
| --- | --- | --- | --- | --- | --- | --- |
| 00 | COMPLETE | IMPLEMENTED | PASSED | PUSH_VERIFIED | NOT_APPLICABLE_WITH_REASON | `0ba08c66` observed at remote; docs/benchmark phase has no required branch CI. |
| 01 | NOT_STARTED | NOT_STARTED | NOT_RUN | NOT_COMMITTED | NOT_RUN | Direct result slots and native ValueSet handoff. |
| 02 | NOT_STARTED | NOT_STARTED | NOT_RUN | NOT_COMMITTED | NOT_RUN | Typed layouts and identity. |
| 03 | NOT_STARTED | NOT_STARTED | NOT_RUN | NOT_COMMITTED | NOT_RUN | Declarative applications and live bindings. |
| 04 | NOT_STARTED | NOT_STARTED | NOT_RUN | NOT_COMMITTED | NOT_RUN | Composable managed compilation. |
| 05 | NOT_STARTED | NOT_STARTED | NOT_RUN | NOT_COMMITTED | NOT_RUN | Flow, temporal state, revision safety. |
| 06 | NOT_STARTED | NOT_STARTED | NOT_RUN | NOT_COMMITTED | NOT_RUN | Inputs and command argument bindings. |
| 07 | NOT_STARTED | NOT_STARTED | NOT_RUN | NOT_COMMITTED | NOT_RUN | Required filter catalog. |
| 08 | NOT_STARTED | NOT_STARTED | NOT_RUN | NOT_COMMITTED | NOT_RUN | Backend authoring, asset, persistence. |
| 09 | NOT_STARTED | NOT_STARTED | NOT_RUN | NOT_COMMITTED | NOT_RUN | Svelte Mapping inspector. |
| 10 | NOT_STARTED | NOT_STARTED | NOT_RUN | NOT_COMMITTED | NOT_RUN | Conversion to custom Formula. |
| 11 | NOT_STARTED | NOT_STARTED | NOT_RUN | NOT_COMMITTED | NOT_RUN | Qualification, performance, cleanup. |

## Acceptance coverage

| Acceptance ID | Scenario | Owning phase | Test/evidence | Status |
| --- | --- | --- | --- | --- |
| M01 | Remap → Sum → Smooth → two commands | 04, 06 | Pending | NOT_RUN |
| M02 | Pack Vec3 → Extract → reorder → Pack Vec3 | 04, 07 | Pending | NOT_RUN |
| M03 | Mixed float/bool/string; float selection | 02, 04 | Pending | NOT_RUN |
| M04 | Color extract, component smoothing, rebuild | 05, 07 | Pending | NOT_RUN |
| M05 | Closed numeric suppressing gate | 05 | Pending | NOT_RUN |
| M06 | Gate changes with steady source | 05 | Pending | NOT_RUN |
| M07 | Hold/default first sample and reopen | 05 | Pending | NOT_RUN |
| M08 | Rename/reorder/add channels during smoothing | 02, 05 | Pending | NOT_RUN |
| M09 | Shared structure, isolated processor/context state | 04, 05 | Pending | NOT_RUN |
| M10 | External edits to coefficient, key, stop, binding | 03, 07, 08 | Pending | NOT_RUN |
| M11 | Fan-out and multi-argument commands | 06 | Pending | NOT_RUN |
| M12 | Unavailable source/target, type/selection changes | 02, 06 | Pending | NOT_RUN |
| M13 | Repeat equal-valued triggers | 05 | Pending | NOT_RUN |
| M14 | Persistence, copy, undo/redo, migration | 08 | Pending | NOT_RUN |
| M15 | Managed regions plus extra Formula operations | 04 | Pending | NOT_RUN |
| M16 | Convert configured Mapping to Formula | 10 | Pending | NOT_RUN |
| M17 | Preview-off execution | 01 | Pending | NOT_RUN |
| M18 | Structural edit during compile/evaluation | 05 | Pending | NOT_RUN |
| M19 | Mapping controls Mapping or itself | 05, 08 | Pending | NOT_RUN |
| M20 | Inspector/context switching and large lists | 09, 11 | Pending | NOT_RUN |

## Validation evidence

| Timestamp | Phase | Tested revision | Command/scenario | Environment | Result | Evidence |
| --- | --- | --- | --- | --- | --- | --- |
| 2026-09-13T09:30+02:00 | 00 | `b6ac86eb` | `cargo metadata --no-deps --format-version 1` | Windows x64, Rust 1.97.0 | PASSED | Workspace/package identities resolved. |
| 2026-09-13T09:30+02:00 | 00 | `b6ac86eb` | `cargo fmt --all --check` | Windows x64, Rust 1.97.0 | PASSED | No formatting diff. |
| 2026-09-13T09:30+02:00 | 00 | `b6ac86eb` | `cargo test --locked -p chataigne_alchemist -p chataigne_processor -p chataigne_condition -p chataigne_state_machine` | Windows x64, Rust 1.97.0 | PASSED | All four package test suites and doc tests passed. |
| 2026-09-13T09:30+02:00 | 00 | `b6ac86eb` | `cargo check --locked --workspace` | Windows x64, incomplete ambient ASIO SDK | ENVIRONMENT_BLOCKED | `asio-sys` could not find `asiodrivers.h`; not a source failure. |
| 2026-09-13T09:31+02:00 | 00 | `b6ac86eb` | `.\tools\asio.ps1 -- cargo check --locked --workspace` | Windows x64, pinned ASIO SDK and LLVM | PASSED | Full workspace check completed. |
| 2026-09-13T09:31+02:00 | 00 | `b6ac86eb` | `npm run check`, `npm test`, `npm run lint`, `npm run build` | Node 26.5.0, npm 11.17.0 | PASSED | 0 Svelte errors/warnings, 135 UI tests, Prettier and Vite build pass. |
| 2026-09-13T09:35+02:00 | 00 | `b6ac86eb` + benchmark fixture | `cargo bench --locked -p chataigne_processor --bench mapping_baseline -- --warm-up-time 1 --measurement-time 2 --sample-size 10` | Windows x64, Intel Core Ultra 9 275HX, Rust 1.97.0, bench profile | PASSED | Center estimates: depth 1, 1/8/32 channels = 1.681/13.499/51.411 µs; depth 8, 8/32 channels = 33.988/141.72 µs. |
| 2026-09-13T09:36+02:00 | 00 | `b6ac86eb` + benchmark fixture | `cargo bench --locked -p chataigne_processor --bench mapping_baseline --no-run`; root and Golden Core `cargo fmt --all`; `cargo fmt --all --check`; `git diff --check` | Windows x64, Rust 1.97.0 | PASSED | Benchmark rebuilt without warnings after replacing deprecated helper; formatter and whitespace checks passed. |

## Phase reports

### Phase 00

- Changes and affected public boundaries: canonical plan, status ledger, Mapping architecture documentation, and a managed-runner benchmark fixture; no public runtime boundary changed.
- Acceptance gates satisfied: baseline owner and known runtime limitations identified; M01–M20 assigned to phases; baseline Rust/UI gates and representative pre-change benchmark pass.
- Remaining work: no Phase 00 work. Full product and expanded performance qualification belong to later phases.
- Exact checks and outcomes: see validation evidence; default Windows ASIO check requires the documented pinned SDK wrapper.
- Implementation commit: `0ba08c66eee40b65c1e91cd78816549f70d657fc`.
- Verified remote ref, observed OID, and timestamp: `refs/heads/codex/builtin-mapping` on `origin`, `0ba08c66eee40b65c1e91cd78816549f70d657fc`, 2026-09-13T09:37:15+02:00 (`git ls-remote`).
- CI status and relevant runs: `NOT_APPLICABLE_WITH_REASON` for this docs/benchmark phase; repository CI is configured for PRs and pushes to `main`, not a direct push to this branch.
- Decisions/deviations and rationale: branched from local `main` to preserve its one unpublished commit; moved the untracked plan to the path required by the plan. The existing Golden Engine benchmark baseline is unqualified and cannot establish a Mapping latency threshold.

## Blockers and handoff

- What failed: no implementation gate currently failed. Plain workspace check encountered a pre-existing incomplete ambient ASIO SDK; the pinned wrapper passed.
- Last known-good checkpoint: Phase 00 implementation `0ba08c66eee40b65c1e91cd78816549f70d657fc`, verified on `origin/codex/builtin-mapping`.
- Reproduction: `cargo check --locked --workspace` with the ambient `CPAL_ASIO_DIR` fails in `asio-sys`; `.\tools\asio.ps1 -- cargo check --locked --workspace` passes.
- Next action: publish this factual Phase 00 status checkpoint, then start Phase 01.
