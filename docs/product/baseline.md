# Phase 0 Baseline Record

Status: repository provenance captured; product, platform, visual, and manual evidence remain in
progress.

Recorded: 2026-07-11

This record distinguishes the branch starting point, the working-product oracle, and the failed
rewrite donor. They serve different purposes and must not be substituted for one another.

## Repository Revisions

| Role | Ref | Exact commit | Recorded immutable ref | Use |
| --- | --- | --- | --- | --- |
| Fetched branch start | `origin/main` | `fb0f3a58f3593df8994bf8bd46f88ddd7612f41d` | `baseline/revised-plan-main-fb0f3a5` | Start of the canonical migration branch |
| Migration branch | `architecture/aaa-product-rewrite` | `fb0f3a58f3593df8994bf8bd46f88ddd7612f41d` | `baseline/revised-plan-main-fb0f3a5` | Current Phase 0 worktree |
| Working-product oracle | historical `main` product | `f0a7c2fe4d192a076c7649c6fe3e6a5ab193a435` | `baseline/chataigne-product-f0a7c2f` | Behavior, UX, fixtures, modules, and performance comparison |
| Failed rewrite donor | `rewrite/golden-architecture` | `174fc5096ac7ab4546b3acc76569bca6a1c9e01d` | `donor/failed-rewrite-174fc50` | Reviewable donor units only; never a migration base |

The working-product commit is an ancestor of the branch start. The branch start adds the revised
plan but does not replace the working-product commit as the behavioral oracle.

## Exact Baseline Gitlinks

The branch-start tree and working-product tree contain the same four gitlinks:

| Path | Repository | Exact commit |
| --- | --- | --- |
| `submodules/golden_core` | `https://github.com/Golden-Geek/golden_core` | `0cd6e706f93b5ee49b326748ecc491fd8e8deffd` |
| `submodules/golden_alchemist_core` | `https://github.com/Golden-Geek/golden_alchemist_core.git` | `1bfe1c5b15d02068695fff5a71073c8b081f72e0` |
| `src-ui/src/lib/golden_ui` | `https://github.com/Golden-Geek/golden_ui` | `0dcb6baf8ff44d80e904d017a1528662579917e7` |
| `src-ui/src/lib/golden_alchemist_ui` | `https://github.com/Golden-Geek/golden_alchemist_ui.git` | `b4b9fe6fa06c328e7bc0f5b487a3779320b6666f` |

Inventories and characterization must use these revisions recursively. A newer repository head is
not baseline evidence.

## Provenance Checks Executed

The following read-only checks were executed in the Windows worktree on 2026-07-11:

| Check | Result | Scope |
| --- | --- | --- |
| Resolve `HEAD` and `origin/main` | `PASS` | Both resolved to the recorded branch-start commit |
| List gitlinks from branch-start tree | `PASS` | Four paths and commits matched the table above |
| List gitlinks from working-product tree | `PASS` | Same four paths and commits matched |
| Verify donor object exists | `PASS` | Object is a Git commit |
| Verify working product is an ancestor of branch start | `PASS` | Git ancestry only |

These checks prove provenance only. They do not prove that the product builds, launches, connects,
or preserves behavior.

## Evidence Still Required

| Evidence area | Status | Note |
| --- | --- | --- |
| Windows MSVC clean bootstrap/build | `NOT_RUN` | Pending the build/toolchain slice |
| macOS clean bootstrap/build | `NOT_RUN` | Requires a macOS runner |
| Linux clean bootstrap/build | `NOT_RUN` | Requires a Linux runner with documented UI prerequisites |
| Bare `cargo run` smoke | `NOT_RUN` | Must prove the complete app and engine-connected UI |
| Root `watch` workflow smoke | `NOT_RUN` | Must characterize processes, ports, readiness, restart, and shutdown |
| `cargo run -- --dev` smoke | `NOT_RUN` | Must prove the live frontend and engine connection |
| Canonical project load, live feedback, save/reload | `NOT_RUN` | Requires the real backend and frontend |
| Reference screenshots and deterministic Playwright traces | `NOT_RUN` | No visual parity claim is made |
| `P50-L1` and `P5-L127` real-application fixtures | `NOT_RUN` | No performance claim is made |
| Loopback modules and semantic digests | `NOT_RUN` | Pending executable characterization |
| Manual UX and hardware matrix | `NOT_RUN` | No manual sign-off has been performed |

Phase 0 is not green until the applicable evidence is executed and recorded under the
[parity ledger contract](parity-ledger-schema.md).
