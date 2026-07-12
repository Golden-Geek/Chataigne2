# Repository Map

Phase 1A is a single monorepo. The root `Cargo.toml`, `Cargo.lock`, `package.json`, and
`package-lock.json` are authoritative; no Git submodule initialization or nested package-manager
install is required.

## Chataigne application

- `apps/chataigne/src/`: executable shell and app-owned module tree.
- `apps/chataigne/state_machine/`: Chataigne Processor/state-machine policy and protocol generation.
- `apps/chataigne/ui/`: Svelte 5 routes, product panels, stores, branding, and assets.
- `apps/chataigne/builtin_formulas/`: shipped Action and Mapping formula files.
- `apps/chataigne/capabilities/`, `apps/chataigne/icons/`, and
  `apps/chataigne/tauri.conf.json`: desktop permissions and packaging resources.
- `apps/chataigne/build.rs`: app registry and bundled-UI generation through public workspace APIs.

## Shared Rust crates

- `crates/core/`: pure engine/runtime implementation (`golden_engine`).
- `crates/core_facade/`: stable `golden_core` facade used by applications.
- `crates/protocol/`, `crates/persistence/`, `crates/script/`: public protocol, persistence, and
  scripting boundaries.
- `crates/transport_server/`, `crates/host_desktop/`: reusable transport and host runtimes.
- `crates/core_macros/`, `crates/codegen_support/`: proc macros and public build/codegen support.
- `crates/golden_alchemist/`, `crates/golden_statechart/`: reusable typed graph and statechart
  engines.
- `crates/core/docs/`: deeper engine/runtime design notes.

## Shared UI packages

- `packages/golden-ui/`: reusable workbench, panels, stores, transport adapters, and generated Rust
  protocol bindings.
- `packages/golden-alchemist-ui/`: reusable graph canvas and graph interaction components.

App UI code consumes these packages through their public package exports. App-specific panels,
registrations, branding, and policy stay under `apps/chataigne/ui/`.

## Workspace tooling and records

- `tools/`: bootstrap, checks, migration manifests, and product-gate tooling.
- `xtask/`: root `watch` orchestration.
- `docs/product/`: immutable source revisions, parity manifests, and executed migration evidence.
- `docs/imports/`: repository-level notices preserved from the imported source repositories.
