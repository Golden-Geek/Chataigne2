# Contributing

This repository is being refactored toward a clean long-term architecture. Optimize for correct boundaries, readable diffs, and stable ownership, not short-term compatibility.

## Repo Map

- `apps/chataigne/`: thin executable shell, app-owned modules, product systems, resources, and UI.
- `apps/chataigne/systems/alchemist/`: Formula, conditions, processors, Inputs, Filters, and Outputs.
- `apps/chataigne/systems/state_machine/`: state/transition model, runtime, arbitration, and protocol.
- `apps/chataigne/ui/`: Svelte 5 app UI shell, reusable package consumption, and app stores.
- `crates/golden_core/`: stable facade with grouped engine, foundation, runtime, service, host, and
  support crates beneath it.
- `crates/golden_graph/`: reusable graph document and transaction system.
- `packages/golden-ui/`, `packages/golden-graph-ui/`: reusable Svelte workbench and graph packages.

## Local Bootstrap

For a fresh machine, start from the repository root with the platform bootstrap command:

```powershell
.\tools\dev.ps1
```

```sh
bash ./tools/dev.sh
```

These commands install the exact Rust host and checksum-verified portable Node.js/npm distribution
from the canonical manifest, verify Python, install platform desktop dependencies and root workspace
dependencies, and then run the app. On Windows, the bootstrap selects the manifest's versioned MSVC
host. See [docs/guides/development.md](docs/guides/development.md) for prerequisites, diagnostics, root workflows,
qualification commands, and cache policy.

## Formatting

Rust:

```sh
cargo fmt --all
```

UI:

```sh
npm run codegen:golden-ui-protocol --workspace chataigne-ui
npm run format
```

## Hard Boundary Rules

- Do not import another crate's private internals by filesystem path from app crates or build scripts.
- Do not use `#[path = "..."]` to reach into another crate's private files.
- Inside `golden_engine`, keep `src/` as the real module tree. Do not reintroduce runtime `#[path]` wiring or split one concept across `thing.rs` and `thing/`.
- Keep tests in separate files across this repository. Do not mix production/runtime code and inline `mod tests { ... }` in the same source file.
- For module-local tests, use sibling test files such as `tests.rs` or `*_tests.rs`.
- Inside `golden_ui`, do not add new `$app/*` dependencies or app-local alias coupling. Treat it
  as a package boundary under `packages/golden-ui/`.
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
- Link to deeper design docs in `crates/golden_core/engine/docs/` instead of duplicating them.
- Keep `crates/golden_core/engine/docs/source_layout.md` current when filesystem ownership or module-placement rules change.
- Keep `packages/golden-ui/docs/source_layout.md` current when UI package ownership or file
  placement rules change.
- Update architecture docs when host/runtime responsibilities move across the app shell and shared crates.
- Update `ARCHITECTURE.md` and the relevant focused guide when ownership changes.
