# Built-in Mapping implementation status

## Summary

- Overall: IN_PROGRESS
- Baseline commit: `b6ac86eb702d703560593c108b599f95545a417c` (local `main` was one commit ahead of `origin/main` at task start)
- Working branch and approved remote: `codex/builtin-mapping`; `origin` = `git@github.com:Golden-Geek/Chataigne2.git`
- Active phase: 05 — flow control, temporal scheduling, and state correctness (next phase turn)
- Last validated implementation commit: `47f8794053509cfa8e821f343f93bc6c1d3d8f96` (Phase 04)
- Last verified remote implementation commit: `47f8794053509cfa8e821f343f93bc6c1d3d8f96` (Phase 04)
- Current blockers: none; later phases still own flow control, command argument bindings, the built-in asset, and product UI.
- Product checks still outstanding: revised M01–M16 and M18–M20 acceptance cases, desktop/headless/watch and interactive product smoke checks, expanded Mapping qualification benchmarks
- Next concrete action: begin Phase 05 flow and temporal state work after the Phase 04 status checkpoint is verified remotely.
- Last updated: 2026-09-13T14:00:00+02:00

## Phase ledger

| Phase | Status | Implementation | Validation | Delivery | CI | Evidence / blocker |
| --- | --- | --- | --- | --- | --- | --- |
| 00 | COMPLETE | IMPLEMENTED | PASSED | PUSH_VERIFIED | NOT_APPLICABLE_WITH_REASON | `0ba08c66` observed at remote; docs/benchmark phase has no required branch CI. |
| 01 | COMPLETE | IMPLEMENTED | PASSED | PUSH_VERIFIED | NOT_APPLICABLE_WITH_REASON | `7e62ca59` observed at remote; no required CI on direct branch push. |
| 02 | COMPLETE | IMPLEMENTED | PASSED | PUSH_VERIFIED | NOT_APPLICABLE_WITH_REASON | `a4705f61` verified remotely; scalar/tuple source-shape gates pass. Whole-value filter execution belongs to Phase 03/04. |
| 03 | COMPLETE | IMPLEMENTED | PASSED | PUSH_VERIFIED | NOT_APPLICABLE_WITH_REASON | `a783b60c` verified remotely; whole-tuple palette rejects implicit subset routing, and three-operand Math/Sum/Average choices materialize exact sockets. Direct branch pushes have no required CI. |
| 04 | COMPLETE | IMPLEMENTED | PASSED | PUSH_VERIFIED | NOT_APPLICABLE_WITH_REASON | `47f87940` observed at the remote; whole-tuple composition, authored Formula boundaries, shared specializations, isolated state, and previews pass crate/app/workspace gates. Direct branch pushes have no required CI. |
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
| M01 | Three floats → elementwise Remap → Sum → Smooth → two commands | 04, 06 | Processor tuple Formula test sends two OutputSet commands across two ticks; explicit per-command arguments remain Phase 06 | PARTIAL |
| M02 | X/Y/Z → elementwise filter → Pack Vec3 → one 3D command | 04, 06 | Processor tuple Formula test dispatches one Vec3 value; command argument bindings remain Phase 06 | PARTIAL |
| M03 | Mixed float/bool/string → numeric filter diagnostic; explicit conversion | 02, 04 | Mixed float/bool tuple rejects implicit numeric subset; full float/bool/string and conversion catalog remain Phase 07 | PARTIAL |
| M04 | Color conversion or explicit extraction → typed command | 06, 07 | Pending | NOT_RUN |
| M05 | Closed numeric suppressing gate | 05 | Pending | NOT_RUN |
| M06 | Gate changes with steady source | 05 | Pending | NOT_RUN |
| M07 | Hold/default first sample and reopen | 05 | Pending | NOT_RUN |
| M08 | Rename/reorder/add tuple inputs during elementwise smoothing | 02, 05 | Pending | NOT_RUN |
| M09 | Shared structure, isolated processor/context state | 04, 05 | Equivalent stages share compiled graph but keep separate smoothing history and preview identity; processor/context lifecycle remains Phase 05 | PARTIAL |
| M10 | External edits to coefficient, key, stop, binding | 03, 07, 08 | Pending | NOT_RUN |
| M11 | Whole-value fan-out and tuple-element command arguments | 06 | Pending | NOT_RUN |
| M12 | Unavailable source/target and tuple type/arity changes | 02, 06 | Pending | NOT_RUN |
| M13 | Repeat equal-valued triggers | 05 | Pending | NOT_RUN |
| M14 | Persistence, copy, undo/redo, migration | 08 | Pending | NOT_RUN |
| M15 | Managed regions plus extra Formula operations | 04 | Graph-backed processor executes routing before and after managed filter, with command and preview assertions | PASSED |
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
| 2026-09-13T11:38+02:00 | 03/04 | working tree on `7469d0d5` | `.\tools\asio.ps1 -- cargo test-fast --locked -p chataigne_processor --quiet`; `cargo clippy --locked -p chataigne_alchemist -p chataigne_processor --all-targets -- -D warnings`; `.\tools\asio.ps1 -- cargo check --locked --workspace` | Windows x64, pinned ASIO SDK, Rust 1.97.0 | PASSED | 88 processor tests pass after typed Formula integration; strict Clippy and full workspace type-check pass. |
| 2026-09-13T11:38+02:00 | 03/04 | working tree on `7469d0d5` | `CARGO_INCREMENTAL=0 .\tools\asio.ps1 -- cargo test-fast --locked -p Chataigne2 --quiet` | Windows x64, pinned ASIO SDK, Rust 1.97.0 | PASSED | 535 app tests pass, 5 existing manual tests ignored, 1 audio-host integration test passes; includes explicit schema and live parameter snapshot tests. This run began before the final enum boxing, which is covered by the workspace check and crate tests. |
| 2026-09-13T11:40+02:00 | 03/04 | working tree on `7469d0d5` | `.\tools\asio.ps1 -- cargo test-fast --locked -p chataigne_alchemist -p chataigne_processor --quiet`; root and Golden Core `cargo fmt --all`; `cargo fmt --all --check`; staged `git diff --check` | Windows x64, pinned ASIO SDK, Rust 1.97.0 | PASSED | 161 Alchemist and 88 processor tests pass after final runtime boxing; formatter and staged whitespace checks pass. |
| 2026-09-13T12:40+02:00 | 02 revised | working tree on `04c99834` | `cargo test-fast --locked -p chataigne_alchemist -p chataigne_processor --quiet` | Windows x64, Rust 1.97.0 | PASSED | 161 Alchemist and 90 processor tests pass. Scalar/tuple shape test caught source schema loss on reorder; input reconciliation now retains a resolved schema only when authored identity and source reference agree, and replacement becomes unresolved. |
| 2026-09-13T12:46+02:00 | 02/03 revised | working tree on `04c99834` | `CARGO_INCREMENTAL=0 .\tools\asio.ps1 -- cargo test-fast --locked -p Chataigne2 backend_filter_palette_materializes_executable_variants_for_the_current_layout --quiet` | Windows x64, pinned ASIO SDK | PASSED | Backend palette materialization and availability test passed after a default incremental-link failure. |
| 2026-09-13T12:46+02:00 | 02/03 revised | working tree on `04c99834` | `CARGO_INCREMENTAL=0 .\tools\asio.ps1 -- cargo test-fast --locked -p Chataigne2 --bin Chataigne2 --quiet` | Windows x64, pinned ASIO SDK | PASSED | 536 app unit tests passed, 5 existing manual qualification tests ignored. The broader `-p Chataigne2` command could not replace `target/debug/Chataigne2.exe` while another app process used it; the separate audio-host integration check remains outstanding for this revision. |
| 2026-09-13T12:49+02:00 | 02/03 revised | working tree on `04c99834` | `CARGO_INCREMENTAL=0 .\tools\asio.ps1 -- cargo test-fast --locked -p Chataigne2 --test default_audio_hosts --quiet`; `cargo test-fast --locked -p chataigne_alchemist -p chataigne_processor --quiet`; strict crate Clippy; root and Golden Core `cargo fmt --all --check` | Windows x64, pinned ASIO SDK, Rust 1.97.0 | PASSED | Audio-host integration test, 161 Alchemist and 90 processor tests, `-D warnings`, and both format scopes pass. Together with the scoped bin test, these cover the app package tests after the broad command encountered a file lock. |
| 2026-09-13T12:49+02:00 | 02/03 revised | working tree on `04c99834` | `.\tools\asio.ps1 -- cargo check --locked --workspace`; `git diff --check` | Windows x64, pinned ASIO SDK, Rust 1.97.0 | PASSED | Full workspace type-check and whitespace validation pass after the tuple-shape and palette work. |
| 2026-09-13T12:53+02:00 | 02 revised | working tree on `c962867a` | `cargo test-fast --locked -p chataigne_alchemist -p chataigne_processor --quiet`; strict crate Clippy; root and Golden Core `cargo fmt --all --check`; `git diff --check` | Windows x64, Rust 1.97.0 | PASSED | 161 Alchemist and 90 processor tests pass after final metadata/replacement assertions; `-D warnings`, both format scopes, and whitespace checks pass. The app unit and audio-host integration checks passed on the preceding WIP checkpoint, and this delta changes only a processor test and plan wording. |
| 2026-09-13T13:18+02:00 | 03 | working tree before `a783b60c` | `cargo test-fast --locked -p chataigne_alchemist -p chataigne_processor -p chataigne_state_machine --quiet`; `cargo clippy --locked -p chataigne_alchemist -p chataigne_processor --all-targets -- -D warnings` | Windows x64, Rust 1.97.0 | PASSED | 164 Alchemist, 91 processor, and 17 state-machine tests pass; strict Clippy is clean. Includes whole-tuple applicability, three-input Sum/Average evaluation, graph arithmetic, and persisted filter-mode tests. |
| 2026-09-13T13:18+02:00 | 03 | working tree before `a783b60c` | `CARGO_INCREMENTAL=0 .\tools\asio.ps1 -- cargo test-fast --locked -p Chataigne2 --bin Chataigne2 --quiet`; `CARGO_INCREMENTAL=0 .\tools\asio.ps1 -- cargo test-fast --locked -p Chataigne2 --test default_audio_hosts --quiet` | Windows x64, pinned ASIO SDK | PASSED | 536 app unit tests pass, 5 pre-existing manual tests ignored; the audio-host integration test passes. The backend test checks exact three-input Math/Sum/Average creation and X/Y/Z Pack Vec3 availability. |
| 2026-09-13T13:19+02:00 | 03 | working tree before `a783b60c` | `.\tools\asio.ps1 -- cargo check --locked --workspace`; root and Golden Core `cargo fmt --all` and `--check`; staged `git diff --check` | Windows x64, pinned ASIO SDK, Rust 1.97.0 | PASSED | Full workspace check, both formatter scopes, and whitespace check pass. |
| 2026-09-13T13:58+02:00 | 04 | working tree before checkpoint A | `cargo test-fast --locked -p chataigne_alchemist -p chataigne_processor --quiet` | Windows x64, Rust 1.97.0 | PASSED | 164 Alchemist and 102 processor tests, including whole-tuple composition, graph boundaries, shared plans, isolated state, preview attribution, and graph-error suppression. |
| 2026-09-13T13:58+02:00 | 04 | working tree before checkpoint A | `cargo test-fast --locked -p Chataigne2 --quiet` | Windows x64, Rust 1.97.0 | PASSED | 537 app unit tests passed, 5 existing manual tests ignored, and 1 audio-host integration test passed; includes managed metadata with authored graph nodes. |
| 2026-09-13T13:58+02:00 | 04 | working tree before checkpoint A | `cargo check --locked --workspace`; `cargo clippy --locked -p chataigne_alchemist -p chataigne_processor -p Chataigne2 --all-targets -- -D warnings`; root and Golden Core `cargo fmt --all`; `cargo fmt --all --check`; `git diff --check` | Windows x64, Rust 1.97.0 | PASSED | Full workspace and strict targeted Clippy passed, both formatter scopes and whitespace check passed. |

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

### Phase 02 (revised tuple contract)

- Changes and affected public boundaries: the app-owned Alchemist model exposes `MappingValueShape` as an incomplete, scalar, or ordered tuple view over the internal typed layout. Processor InputSet owns source-aligned runtime frames, validity/change/delivery state, and explicit source-schema reconciliation; backend callers can query the declared input layout and Mapping value shape. Historical channel selection/group support remains internal groundwork for custom Formulas.
- Acceptance gates satisfied under the previous plan: focused tests cover mixed typed values, repeated source references, stable reorder, rename, disabled/re-enabled and missing inputs, metadata-only revision, empty inputs, wrong source types, and value updates without a layout rebuild. Extraction and grouping tests cover internal/custom Formula machinery, not the revised standard Mapping UX.
- Revised acceptance gates satisfied: `MappingValueShape` distinguishes incomplete, one typed source, and an ordered heterogeneous tuple without conflating a single array source with a tuple. InputSet retains authored identity, declared position, value validity, and resolved schema/metadata across reorder; source replacement invalidates the old schema. Existing focused tests cover repeated references, rename, disable/reenable, removal, compound values, metadata-only updates, and no value-driven shape rebuild. Input source authoring uses no channel selection or group.
- Remaining Phase 02 work: none. Phase 03/04 own tuple-aware filter applicability and whole-value execution; Phase 09 presents the backend shape query.
- Exact checks and outcomes: see validation table. The revised 161/90 Alchemist/processor tests, strict Clippy, full workspace check, 536 app unit tests plus 1 audio-host integration test, root and Golden Core format, and whitespace checks passed.
- Implementation commit: `a4705f61542be7c33e4c62fd039447b8d8a312e8` (revised phase checkpoint following WIP `1c70c88d8d3c92f147a5ab5f0c57c9f826a97595`). Historical channel-layout checkpoint: `8e82b40c71ab1237ba39d57d3902f19d5ffbd904`.
- Verified remote ref, observed OID, and timestamp: `refs/heads/codex/builtin-mapping` on `origin`, `a4705f61542be7c33e4c62fd039447b8d8a312e8`, 2026-09-13T12:54:05+02:00 (`git ls-remote`).
- CI status and relevant runs: `NOT_APPLICABLE_WITH_REASON` for direct branch push; repository CI triggers on PRs and `main`.
- Decisions/deviations and rationale: preserve dynamic source identity with an unresolved type until a backend schema event; never infer shape from ordinary value samples. The native frame carries tuple-element validity while the managed runner is adapted in Phase 03/04. The user's tuple decision supersedes channel selections and grouping for standard Mapping; the historical channel-layout commit remains in history as internal groundwork.
- Revised WIP checkpoint: `1c70c88d8d3c92f147a5ab5f0c57c9f826a97595`, observed at `refs/heads/codex/builtin-mapping` on `origin` at 2026-09-13T12:50:56+02:00. It adds the scalar/tuple shape query and schema-preserving input reconciliation, but does not close Phase 02.
- Revised Phase 02 checkpoint A is verified. Direct pushes to this branch have no configured mandatory CI workflow; the targeted crate/app/workspace gates above passed.

### Phase 03 (complete)

- Changes and affected public boundaries: ANode declarations resolve capabilities from configured instances; `ManagedApplication` validates signatures, auxiliary types, and state scope. Mapping validation additionally requires the whole ordered value and rejects authored channel selection/grouping. Math retains elementwise and tuple-combine variants; Sum and Average add declared variable-arity reductions using the graph arithmetic kernel. The processor binds live auxiliary values or references through Formula properties without rebuilding compatible state. A persisted filter value mode keeps older custom Formula regions routed and gives Mapping a whole-tuple palette contract.
- Acceptance gates satisfied: a backend parameter edit changes output without resetting unrelated SMA history; graph and managed arithmetic share declaration kernels; unsupported type, arity, socket, selection, and group combinations diagnose. The palette validates the configured variant with the typed stage compiler and uses the same configuration when creating the tree. Three-input Math/Sum/Average nodes materialize three input sockets; X/Y/Z Pack Vec3 is offered for three floats; mixed numeric-incompatible tuples do not silently advertise a subset filter.
- Remaining work: Phase 04 must make tuple-mode runtime execution obey the validated whole-value contract and preserve surrounding Formula graph operations. Phase 08 must declare tuple mode in the built-in Mapping asset; Phase 07 expands the filter catalog. The current asset does not yet expose this palette as a finished product.
- Exact checks and outcomes: see Phase 03 validation rows. A prior incremental MSVC app link failure was avoided by the non-incremental app test run; current crate, app, integration, workspace, Clippy, format, and whitespace gates pass.
- Implementation commit: `a783b60c6424e47d007f1b40df0974a6c30807fe` (following WIP `e3b37a7b96267f4981ac6afe392d489c500f197f`, `fa4d16de11aa75e8fe0d2790f0bdd24da3c0e7e3`, and `1c70c88d8d3c92f147a5ab5f0c57c9f826a97595`).
- Verified remote ref, observed OID, and timestamp: `refs/heads/codex/builtin-mapping` on `origin`, `a783b60c6424e47d007f1b40df0974a6c30807fe`, 2026-09-13T13:19:26+02:00 (`git ls-remote`).
- CI status and relevant runs: `NOT_APPLICABLE_WITH_REASON`; direct pushes to this branch have no configured required CI workflow.
- Decisions/deviations and rationale: keep role, settings, and operation behavior on existing ANode declarations. Config changes remain structural; runtime socket values update compiled properties. Whole-tuple validation is separate from legacy routed validation until Phase 04 changes execution. Existing custom Formula behavior and persisted projects default to routed mode, avoiding a silent semantic change.

### Phase 04 (complete)

- Changes and affected public boundaries: the typed stage chain now compiles each standard Mapping filter against the entire scalar or ordered tuple, with declared sockets carrying each result into the next stage. The app-owned Alchemist evaluator exposes per-instance external graph nodes, and the processor lowers InputSet, FilterPipeline, and OutputSet at explicit Formula graph sockets while retaining surrounding authored operations. App snapshot materialization preserves managed metadata when a Formula also has graph nodes. A bounded manager-owned specialization cache shares compiled stage graphs across equivalent instances; result buffers, bindings, and temporal memory remain local.
- Acceptance gates satisfied: focused tests run three floats through Remap, Sum, and Smooth to two commands, X/Y/Z through Math and Pack Vec3 to a Vec3 command, and Pack/Extract/Math/Pack in tuple mode. A mixed tuple rejects implicit subset application. A custom Formula runs routing operations before and after its managed filter once, with authored graph and filter-item previews. Missing graph boundaries diagnose; a runtime graph error suppresses intents and live previews. Equivalent smooth stages share an executable graph while their histories and preview identities stay separate. Backend metadata survives an authored graph node.
- Remaining work: Phase 05 owns typed suppression/hold/default/trigger flow, temporal dirty tracking, and lane migration. Phase 06 owns explicit per-command argument bindings; the two-command Phase 04 case uses two OutputSet regions. Phase 08 owns the shipped Mapping asset and backend authoring. Trigger managed regions with an authored graph currently diagnose an unsupported boundary instead of silently running a sidecar. The older trigger filter runner is scheduled for replacement during Phase 05. The pre-existing oversized Alchemist runtime source remains Phase 11 cleanup.
- Exact checks and outcomes: 164 Alchemist and 102 processor tests pass with `cargo test-fast`; 537 app tests pass with 5 existing manual tests ignored, plus the audio-host integration test. Full workspace check, strict Clippy across Alchemist/processor/app targets, root and Golden Core format, and whitespace checks pass.
- Implementation commit: `47f8794053509cfa8e821f343f93bc6c1d3d8f96`; prior WIP checkpoints `74f5bd45`, `fa4d16de`, and `1c70c88d` are not closure evidence.
- Verified remote ref, observed OID, and timestamp: `refs/heads/codex/builtin-mapping` on `origin`, `47f8794053509cfa8e821f343f93bc6c1d3d8f96`, 2026-09-13T14:00:00+02:00 (`git ls-remote`).
- CI status and relevant runs: `NOT_APPLICABLE_WITH_REASON`; direct pushes to this branch do not trigger required CI.
- Decisions/deviations and rationale: the graph-free Mapping path retains native typed frames; graph-backed custom Formulas use the ValueSet extension only at real graph sockets. Source schema remains a backend structural input. Runtime graph errors suppress the entire intent batch rather than dispatching an earlier partial result.

## Blockers and handoff

- What failed or changed: the user replaced the standard Mapping channel model with one ordered source tuple and whole-value linear filtering. Phase 04 now enforces that runtime contract and executes custom Formula graph operations at explicit managed boundaries.
- Last known-good checkpoint: Phase 04 `47f8794053509cfa8e821f343f93bc6c1d3d8f96` is validated and verified at `origin/codex/builtin-mapping`.
- Reproduction: no current Phase 04 failure; focused crate, app, workspace, and Clippy gates pass with the available toolchain.
- Next action: start Phase 05 on the next phase turn after this status checkpoint is verified.
