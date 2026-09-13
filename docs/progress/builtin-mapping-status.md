# Built-in Mapping implementation status

## Summary

- Overall: IN_PROGRESS
- Baseline commit: `b6ac86eb702d703560593c108b599f95545a417c` (local `main` was one commit ahead of `origin/main` at task start)
- Working branch and approved remote: `codex/builtin-mapping`; `origin` = `git@github.com:Golden-Geek/Chataigne2.git`
- Active phase: 03/04 — backend palette availability and composable typed compilation (both in progress)
- Last validated implementation commit: `e3b37a7b96267f4981ac6afe392d489c500f197f` (Phase 03 WIP; Phase 02 is the last complete phase)
- Last verified remote implementation commit: `e3b37a7b96267f4981ac6afe392d489c500f197f` (Phase 03 WIP)
- Current blockers: no environment blocker; Phase 03's palette integration and Phase 04's source-schema/Formula-boundary integration remain open
- Product checks still outstanding: desktop/headless/watch and interactive product smoke checks; expanded Mapping qualification benchmarks; M01–M16 and M18–M20 acceptance cases
- Next concrete action: wire explicit backend source schemas, then connect the typed stage chain to managed Formula execution and the backend palette
- Last updated: 2026-09-13T11:09:01+02:00

## Phase ledger

| Phase | Status | Implementation | Validation | Delivery | CI | Evidence / blocker |
| --- | --- | --- | --- | --- | --- | --- |
| 00 | COMPLETE | IMPLEMENTED | PASSED | PUSH_VERIFIED | NOT_APPLICABLE_WITH_REASON | `0ba08c66` observed at remote; docs/benchmark phase has no required branch CI. |
| 01 | COMPLETE | IMPLEMENTED | PASSED | PUSH_VERIFIED | NOT_APPLICABLE_WITH_REASON | `7e62ca59` observed at remote; no required CI on direct branch push. |
| 02 | COMPLETE | IMPLEMENTED | PASSED | PUSH_VERIFIED | NOT_APPLICABLE_WITH_REASON | `8e82b40c` observed at remote; direct branch push has no required CI. |
| 03 | IN_PROGRESS | IN_PROGRESS | PASSED | PUSH_VERIFIED | NOT_RUN | WIP `e3b37a7b` observed at remote; compiler-backed availability query now tested locally, but the backend palette is not wired to it. |
| 04 | IN_PROGRESS | IN_PROGRESS | PASSED | PUSH_PENDING | NOT_RUN | Typed channel stage chain runs Remap → Sum → Smooth and Extract Color in focused tests; source-schema and Formula-boundary integration remain open. |
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
| M17 | Preview-off execution | 01 | Direct result-slot, codec-call, and capture-off Rust tests; app Action and workspace gates pass | PASSED |
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
| 2026-09-13T09:43+02:00 | 01 | working tree on `1ceb7fb6` | `cargo test --locked -p chataigne_alchemist -p chataigne_processor` | Windows x64, Rust 1.97.0 | PASSED | 151 Alchemist and 72 processor unit tests; direct-slot, capture-off, and native-handoff assertions included. |
| 2026-09-13T09:43+02:00 | 01 | working tree on `1ceb7fb6` | `cargo clippy --locked -p chataigne_alchemist -p chataigne_processor --all-targets -- -D warnings` | Windows x64, Rust 1.97.0 | PASSED | Projection variant boxed after first strict run identified a new large-variant finding. |
| 2026-09-13T09:47+02:00 | 01 | working tree on `1ceb7fb6` | `.\tools\asio.ps1 -- cargo test --locked -p Chataigne2` | Windows x64, pinned ASIO SDK | PASSED | 531 app tests passed, 5 existing manual qualification tests ignored; default audio-host compile test passed. Run began before optional-capture API change, so subsequent checks cover that API. |
| 2026-09-13T09:48+02:00 | 01 | working tree on `1ceb7fb6` | `cargo test-fast --locked -p chataigne_alchemist -p chataigne_processor`; `.\tools\asio.ps1 -- cargo check --locked --workspace`; strict crate Clippy | Windows x64, Rust 1.97.0, pinned ASIO SDK for workspace | PASSED | 151 Alchemist and 72 processor tests pass after optional-capture change; full workspace type-check and strict Clippy pass. |
| 2026-09-13T09:49+02:00 | 01 | working tree on `1ceb7fb6` | `cargo bench --locked -p chataigne_processor --bench mapping_baseline -- --warm-up-time 1 --measurement-time 2 --sample-size 10` | Windows x64, Intel Core Ultra 9 275HX, Rust 1.97.0, bench profile | PASSED | Five center estimates = 0.713/5.803/22.653/13.116/52.838 µs, versus pre-change 1.681/13.499/51.411/33.988/141.72 µs. |
| 2026-09-13T09:52+02:00 | 01 | working tree on `1ceb7fb6` | `.\tools\asio.ps1 -- cargo test-fast --locked -p Chataigne2 action`; `cargo test-fast --locked -p chataigne_state_machine`; root and Golden Core `cargo fmt --all`; `cargo fmt --all --check`; scoped `git diff --check` | Windows x64, Rust 1.97.0, pinned ASIO SDK for app | PASSED | 7 Action-filtered app tests and 17 state-machine tests passed; formatting and whitespace checks clean. |
| 2026-09-13T10:06+02:00 | 02 | working tree on `cec206c9` | `cargo test-fast --locked -p chataigne_alchemist -p chataigne_processor --quiet`; strict crate Clippy | Windows x64, Rust 1.97.0 | PASSED | 156 Alchemist and 78 processor tests; `-D warnings` clean. |
| 2026-09-13T10:07+02:00 | 02 | working tree on `cec206c9` | `.\tools\asio.ps1 -- cargo check --locked --workspace`; root and Golden Core `cargo fmt --all`; root `cargo fmt --all --check`; scoped `git diff --check` | Windows x64, pinned ASIO SDK, Rust 1.97.0 | PASSED | Full workspace type-check and formatting pass. |
| 2026-09-13T10:10+02:00 | 02 | working tree on `cec206c9` | `.\tools\asio.ps1 -- cargo test-fast --locked -p Chataigne2 --quiet` | Windows x64, pinned ASIO SDK, Rust 1.97.0 | PASSED | 531 app tests passed, 5 existing manual qualification tests ignored; 1 audio-host integration test passed. |
| 2026-09-13T10:50+02:00 | 03 | working tree on `8440ed73` | `cargo test-fast --locked -p chataigne_alchemist -p chataigne_processor --quiet`; strict crate Clippy | Windows x64, Rust 1.97.0 | PASSED | 161 Alchemist and 83 processor tests pass, including contextual auxiliary binding and live memory preservation; `-D warnings` clean. |
| 2026-09-13T10:50+02:00 | 03 | working tree on `8440ed73` | `.\tools\asio.ps1 -- cargo check --locked --workspace`; root and Golden Core `cargo fmt --all`; `cargo fmt --all --check`; scoped `git diff --check` | Windows x64, pinned ASIO SDK, Rust 1.97.0 | PASSED | Full workspace check, format, and whitespace checks pass. |
| 2026-09-13T10:50+02:00 | 03 | working tree on `8440ed73` | `CARGO_INCREMENTAL=0 .\tools\asio.ps1 -- cargo test-fast --locked -p Chataigne2 --quiet` | Windows x64, pinned ASIO SDK, Rust 1.97.0 | PASSED | 532 app tests pass, 5 existing manual tests ignored, and 1 audio-host integration test passes; includes live managed socket edit through the host event path. |
| 2026-09-13T11:09+02:00 | 03/04 | working tree on `fe690a73` | `cargo test-fast --locked -p chataigne_alchemist -p chataigne_processor --quiet`; strict crate Clippy; `.\tools\asio.ps1 -- cargo check --locked --workspace` | Windows x64, pinned ASIO SDK, Rust 1.97.0 | PASSED | 161 Alchemist and 87 processor tests pass; typed stage composition, mixed pass-through, and multi-output Color extraction covered. Strict Clippy and workspace check pass. |

## Phase reports

### Phase 00

- Changes and affected public boundaries: canonical plan, status ledger, Mapping architecture documentation, and a managed-runner benchmark fixture; no public runtime boundary changed.
- Acceptance gates satisfied: baseline owner and known runtime limitations identified; M01–M20 assigned to phases; baseline Rust/UI gates and representative pre-change benchmark pass.
- Remaining work: no Phase 00 work. Full product and expanded performance qualification belong to later phases.
- Exact checks and outcomes: see validation evidence; default Windows ASIO check requires the documented pinned SDK wrapper.
- Implementation commit: `0ba08c66eee40b65c1e91cd78816549f70d657fc`.
- Verified remote ref, observed OID, and timestamp: `refs/heads/codex/builtin-mapping` on `origin`, `0ba08c66eee40b65c1e91cd78816549f70d657fc`, 2026-09-13T09:37:15+02:00 (`git ls-remote`).
- Published status checkpoint: `1ceb7fb650ee7c90e918d21cc738014a21f6768d`, observed at `refs/heads/codex/builtin-mapping` before Phase 01 implementation.
- CI status and relevant runs: `NOT_APPLICABLE_WITH_REASON` for this docs/benchmark phase; repository CI is configured for PRs and pushes to `main`, not a direct push to this branch.
- Decisions/deviations and rationale: branched from local `main` to preserve its one unpublished commit; moved the untracked plan to the path required by the plan. The existing Golden Engine benchmark baseline is unqualified and cannot establish a Mapping latency threshold.

### Phase 01

- Changes and affected public boundaries: in progress; compiled result-slot access belongs to reusable Alchemist runtime, managed native handoff to the app-owned processor crate.
- Acceptance gates satisfied: initialized result slots remain available when unchanged nodes skip; managed preview-off evaluation passes no debug sink and has no debug samples or ValueSet codec calls in focused tests. Existing Remap, Smooth, aggregate, and Pack Vec3 tests pass.
- Remaining work: no Phase 01 behavior work. Pre-existing `runtime.rs` length remains a Phase 11 source-layout cleanup item; no new file exceeds the source-size limit.
- Exact checks and outcomes: see validation evidence. Focused Alchemist/processor/state-machine tests, full app test before the final optional-capture API edit, Action-filtered app test after it, full workspace check, strict Clippy, and five-case benchmark pass.
- Implementation commit: `7e62ca593b925a62f32e5064b53187ccbcf4a7d7`.
- Verified remote ref, observed OID, and timestamp: `refs/heads/codex/builtin-mapping` on `origin`, `7e62ca593b925a62f32e5064b53187ccbcf4a7d7`, 2026-09-13T09:53:39+02:00 (`git ls-remote`).
- CI status and relevant runs: `NOT_APPLICABLE_WITH_REASON` for direct branch push; this repository triggers CI on PRs and `main`, and Phase 01 has no separately designated mandatory CI run.
- Decisions/deviations and rationale: retain the JSON extension codec at actual graph/persistence boundaries; the managed processor handoff is native. Reuse scratch memory for stateless projections and make debug capture optional in the Alchemist evaluator. Remaining per-lane/property/evaluator allocations are documented, not claimed eliminated. A concurrent edit to the older implementation plan is unrelated and excluded from Mapping staging.

### Phase 02

- Changes and affected public boundaries: Alchemist now owns immutable channel layouts, stable authored IDs, selection/group resolution, and pre-evaluation layout projections. Processor InputSet owns source-aligned runtime frames, validity/change/delivery state, and explicit source-schema reconciliation; backend callers can query the declared input layout.
- Acceptance gates satisfied: focused tests cover mixed typed values, repeated source references, stable reorder, rename, disabled/re-enabled and missing channels, removal, extraction/group insertion, metadata-only revision, empty inputs, wrong source types, and value updates without a layout rebuild. The full app suite remains green.
- Remaining work: no Phase 02 behavior work. Phase 04 will migrate the homogeneous managed runner from ValueSet stage shape to layout/frame; Phase 06 will connect Golden source schema events; Phase 09 will present backend layout queries in the inspector.
- Exact checks and outcomes: see validation table. The final 156/78 Alchemist/processor tests, strict Clippy, full workspace check, 531 app tests plus 1 audio-host integration test, root and Golden Core format, and whitespace checks passed.
- Implementation commit: `8e82b40c71ab1237ba39d57d3902f19d5ffbd904`.
- Verified remote ref, observed OID, and timestamp: `refs/heads/codex/builtin-mapping` on `origin`, `8e82b40c71ab1237ba39d57d3902f19d5ffbd904`, 2026-09-13T10:10:55+02:00 (`git ls-remote`).
- CI status and relevant runs: `NOT_APPLICABLE_WITH_REASON` for direct branch push; repository CI triggers on PRs and `main`.
- Decisions/deviations and rationale: preserve dynamic source identity with an unresolved type until a backend schema event; never infer layout from ordinary value samples. The native frame remains the authoritative source of channel validity while the existing managed ValueSet runner is migrated in Phase 04.

### Phase 03 (in progress)

- Changes and affected public boundaries: ANode declarations now resolve capabilities from configured instances; `ManagedApplication` validates signatures, selections, groups, auxiliary types, and state scope. Math has per-channel and combine modes using its graph kernel. The processor binds auxiliary socket values or references through Formula properties and resolves contextual references per channel. Backend managed socket edits update a live binding and the authored instance without rebuilding the processor.
- Acceptance gates satisfied: host-level parameter edit changes output without scheduling a rebuild; processor tests show unrelated SMA history survives an auxiliary edit; graph and Mapping Math use the same operation; invalid selection, arity, mode, and auxiliary types diagnose.
- Remaining work: the palette must query actual managed compiler availability. The current homogeneous runner rejects some declaratively valid layouts and multi-output applications; its composable typed replacement is Phase 04 work. Do not mark Phase 03 complete or advertise those choices before that boundary is implemented.
- Exact checks and outcomes: see Phase 03 validation rows. An incremental MSVC app link failed with unresolved symbols in an unrelated module; the non-incremental full app run passed.
- Implementation commit: WIP `e3b37a7b96267f4981ac6afe392d489c500f197f`; this is not the Phase 03 checkpoint A.
- Verified remote ref, observed OID, and timestamp: `refs/heads/codex/builtin-mapping` on `origin`, `e3b37a7b96267f4981ac6afe392d489c500f197f`, 2026-09-13T10:51:19+02:00 (`git ls-remote`).
- CI status and relevant runs: `NOT_RUN` while the phase is in progress.
- Decisions/deviations and rationale: keep role, settings, and operation behavior on the existing ANode declaration. Config changes remain structural; runtime socket values update compiled properties. No second kernel or UI-owned behavior registry was added.

### Phase 04 (in progress)

- Changes and affected public boundaries: a typed managed stage compiles a configured ANode once for selected channel groups, retains group-local Alchemist memory, and emits a stable output layout/frame. A chain composes stages without the terminal-projection restriction. The availability query compiles candidate applications through the current runner before reporting them executable.
- Acceptance gates satisfied: focused tests execute `Remap → Sum → Smooth` with an unselected Boolean passing through and extract a Color into stable component channels; evaluation keeps state across ticks. Full workspace type-check passes.
- Remaining work: connect explicit source schemas and typed frames to managed Formula execution; support the complete Pack/Extract/Math chain, surrounding Formula operations, plan specialization sharing, authored preview attribution, and stale/unsupported graph diagnostics.
- Exact checks and outcomes: see the 03/04 validation row. The app test suite passed before the new stage runtime was added; the workspace check covers current app compilation.
- Implementation commit: pending in-progress checkpoint, not Phase 04 checkpoint A.
- Verified remote ref, observed OID, and timestamp: pending.
- CI status and relevant runs: `NOT_RUN` while the phase is in progress.
- Decisions/deviations and rationale: endpoint `StableRef` types describe endpoint identity, so ordinary values cannot supply stage types. Source schema must arrive as a backend structural event before compiling the typed chain.

## Blockers and handoff

- What failed: the compiler-backed availability query is not yet used by the backend palette; the typed chain is not yet integrated with explicitly typed managed input layouts or surrounding Formula operations. An incremental MSVC app link failed on unrelated module symbols; disabling incremental compilation passed the full app suite. Plain workspace check requires the pinned ASIO SDK wrapper.
- Last known-good checkpoint: Phase 02 complete at `8e82b40c71ab1237ba39d57d3902f19d5ffbd904`; Phase 03 WIP `e3b37a7b96267f4981ac6afe392d489c500f197f` is validated and verified on `origin/codex/builtin-mapping`.
- Reproduction: `cargo check --locked --workspace` with the ambient `CPAL_ASIO_DIR` fails in `asio-sys`; `.\tools\asio.ps1 -- cargo check --locked --workspace` passes.
- Next action: publish the Phase 04 WIP checkpoint, then wire backend source schemas and the typed stage chain into managed Formula execution.
