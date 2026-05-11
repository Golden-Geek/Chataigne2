# Contributing

This repository is being refactored toward a clean long-term architecture. Optimize for correct boundaries, readable diffs, and stable ownership, not short-term compatibility.

## Repo Map

- `src/`: thin app-shell bootstrap plus the app-owned node tree.
- `src/module/`: module-system foundation owned by the app crate until it earns a shared boundary, with concrete modules organized by family under `src/module/modules/`.
- `src-ui/`: Svelte 5 app UI shell, `golden_ui` package consumption, transport clients, and state/store composition.
- `submodules/golden_core/crates/core/`: pure engine/runtime crate implementation (`golden_engine` package).
- `submodules/golden_core/crates/core/src/`: canonical source tree for `golden_engine`; new runtime modules belong here, not beside it.
- `submodules/golden_core/crates/core_facade/`: stable `golden_core` facade used by apps.
- `submodules/golden_core/crates/protocol/`: public protocol DTO boundary.
- `submodules/golden_core/crates/persistence/`: public persistence DTO boundary.
- `submodules/golden_core/crates/transport_server/`: built-in HTTP/WebSocket host.
- `submodules/golden_core/crates/host_desktop/`: default desktop/headless host runtime.
- `submodules/golden_core/crates/core_macros/`: proc macros used by the engine and app nodes.
- `submodules/golden_core/crates/codegen_support/`: build-script support APIs intended for app/workspace consumers.

## Local Bootstrap

For a fresh machine, start from the repository root with the platform bootstrap command:

```powershell
.\tools\dev.ps1
```

```sh
bash ./tools/dev.sh
```

These commands initialize submodules, install or verify Rust, Node.js/npm, platform desktop build
dependencies, `src-ui` dependencies, configure the repo-managed Git hooks, and then run the app. On
Windows, the bootstrap selects `stable-msvc` because the desktop host requires the MSVC Rust toolchain.
Once the machine is prepared, `cargo run` is expected to build the bundled Svelte UI automatically.

The bootstrap sets `core.hooksPath` to `.githooks`, which installs the versioned pre-commit hook that
runs `cargo fmt --all` before Rust commits. If you skip bootstrap, run `git config --local core.hooksPath .githooks`
once from the repo root. On macOS/Linux also run `chmod +x .githooks/pre-commit`.

## Formatting

Rust:

```sh
cargo fmt --all
cargo fmt --manifest-path submodules/golden_core/Cargo.toml --all
```

UI:

```sh
cd src-ui
npm run codegen:golden-ui-protocol
npm run format
```

## Hard Boundary Rules

- Do not import submodule internals by filesystem path from app crates or build scripts.
- Do not use `#[path = "..."]` to reach into another crate's private files.
- Inside `golden_engine`, keep `src/` as the real module tree. Do not reintroduce runtime `#[path]` wiring or split one concept across `thing.rs` and `thing/`.
- Keep tests in separate files across this repository. Do not mix production/runtime code and inline `mod tests { ... }` in the same source file.
- For module-local tests, use sibling test files such as `tests.rs` or `*_tests.rs`.
- Inside `golden_ui`, do not add new `$app/*` dependencies or app-local alias coupling. Treat it
  as a package boundary even while it is checked out under `src-ui/src/lib/`.
- If shared build logic is needed, expose it through a dedicated public crate or module.
- Keep `Chataigne2` thin. Reusable engine, default desktop/headless host runtime, protocol, persistence, and UI logic belongs in shared workspaces.
- Do not reimplement the default Tauri/headless launch path in the app shell; override it only when the app genuinely needs custom bootstrap.

## Protocol Rules

- Do not duplicate protocol declarations across Rust and TypeScript.
- Request, response, event, snapshot, and protocol-version types must have one source of truth.
- When code generation exists, update generator inputs, generated output, and consumers together.
- Do not make the app build script write into shared UI package internals.

## Documentation Expectations

- Update docs in the same change when responsibilities or architecture move.
- Keep docs short and architectural.
- Link to deeper design docs in `submodules/golden_core/crates/core/docs/` instead of duplicating them.
- Keep `submodules/golden_core/crates/core/docs/source_layout.md` current when filesystem ownership or module-placement rules change.
- Keep `src-ui/src/lib/golden_ui/docs/source_layout.md` current when UI package ownership or file
  placement rules change.
- Update architecture docs when host/runtime responsibilities move across the app shell and shared crates.
- Update `docs/repo-transition-plan.md` when repo ownership or migration rules change.
