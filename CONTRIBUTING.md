# Contributing

This repository is being refactored toward a clean long-term architecture. Optimize for correct boundaries, readable diffs, and stable ownership, not short-term compatibility.

## Repo Map

- `src/`: thin app-shell bootstrap, app-specific nodes, and minimal shell wiring.
- `src-ui/`: Svelte 5 app UI shell, `golden_ui` package consumption, transport clients, and state/store composition.
- `submodules/golden_core/crates/core/`: pure engine/runtime crate implementation (`golden_engine` package).
- `submodules/golden_core/crates/core_facade/`: stable `golden_core` facade used by apps.
- `submodules/golden_core/crates/protocol/`: public protocol DTO boundary.
- `submodules/golden_core/crates/persistence/`: public persistence DTO boundary.
- `submodules/golden_core/crates/transport_server/`: built-in HTTP/WebSocket host.
- `submodules/golden_core/crates/host_desktop/`: default desktop/headless host runtime.
- `submodules/golden_core/crates/core_macros/`: proc macros used by the engine and app nodes.
- `submodules/golden_core/crates/codegen_support/`: build-script support APIs intended for app/workspace consumers.

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
- Update architecture docs when host/runtime responsibilities move across the app shell and shared crates.
- Update `docs/repo-transition-plan.md` when repo ownership or migration rules change.
