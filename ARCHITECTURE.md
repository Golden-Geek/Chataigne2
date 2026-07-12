# Architecture

This repository is converging on a layered workspace with a thin shell, shared engine/runtime packages, and a UI stack that talks to the engine through explicit protocol boundaries.

## Top-Level Layers

### App Shell

`Chataigne2` owns app bootstrap, composition, product-level wiring, and app-specific node registration. It should not become the home for reusable engine logic, default desktop/headless startup, protocol declarations, or persistence formats.

### Core Engine

`golden_core` is the shared runtime workspace. It now exposes explicit crates for the engine, protocol, persistence, transport server, desktop host, scripting, macros, and build-time support. Pure engine consumers should be able to depend on the engine-facing crates without pulling in desktop host concerns, while app shells still launch through the default reusable host/runtime path by default.

### Alchemist And Statecharts

`crates/golden_alchemist` owns reusable typed graph compilation/runtime and
`crates/golden_statechart` owns hierarchical statechart mechanics. Chataigne value types, Processor
policy, command arbitration, built-in Processor models, and protocol DTOs live in the app-owned
`apps/chataigne/state_machine` crate beside other product behavior.

### Protocol Boundary

UI request, response, event, snapshot, and version types must have one source of truth. Rust and TypeScript should not manually mirror each other. Build and generation flows should produce the raw transport bindings from the canonical Rust protocol definition, with any UI-local normalization kept as an explicit adapter layer.

### UI Client And Stores

`apps/chataigne/ui` contains the app UI shell and consumes `packages/golden-ui` as the reusable
workbench boundary. `packages/golden-alchemist-ui` supplies app-agnostic infinite-canvas node
rendering and interaction. The
Chataigne State Machine panel, DTO adapters, and stores remain app-owned. Session state should be
composed from focused stores behind a thin facade. Transport concerns should sit behind interfaces,
not leak directly into state orchestration.

### Host Layers

Desktop startup, browser/headless hosting, native dialogs, and transport servers are host concerns. They should live outside the pure engine modules, but they should still be provided by reusable `golden_core` host layers by default. Apps may override command-line parsing or bootstrap when needed, but they should not need local `src/app/desktop.rs`-style host implementations just to launch.

## Build And Codegen Boundary

App build scripts must consume public support APIs. They must not path-import private crate files.
The app node registry is generated through `crates/codegen_support/`, which provides the stable
build-time boundary. Shared UI protocol bindings are generated from the root workspace into
`packages/golden-ui/generated/rust_protocol/`.

## Persistence Direction

Serialization contracts, project schema, codecs, and migrations belong in dedicated persistence or protocol layers. Desktop file dialogs and host workflows should call persistence APIs rather than define persistence formats themselves.

## Deeper Design Docs

Existing design docs live under `crates/core/docs/` and currently include:

- `dashboard_system.md`
- `node_blueprints.md`
- `node_contexts.md`
- `parameters_control_modes.md`
- `scripting_schema.md`
