# Architecture

This repository is converging on a layered workspace with a thin shell, shared engine/runtime packages, and a UI stack that talks to the engine through explicit protocol boundaries.

## Top-Level Layers

### App Shell

`Chataigne2` owns app bootstrap, composition, product-level wiring, and app-specific node registration. It should not become the home for reusable engine logic, default desktop/headless startup, protocol declarations, or persistence formats.

### Core Engine

`golden_core` hosts the shared runtime, default ready-to-launch desktop/headless host flow, node model, macros, and protocol layers. Pure engine modules inside `golden_core` should remain usable without invoking the desktop host path, but the default application runtime belongs in the reusable core workspace rather than each app shell.

### Protocol Boundary

UI request, response, event, snapshot, and version types must have one source of truth. Rust and TypeScript should not manually mirror each other. Build and generation flows should produce the raw transport bindings from the canonical Rust protocol definition, with any UI-local normalization kept as an explicit adapter layer.

### UI Client And Stores

`src-ui` contains the Svelte 5 client, transport layer, and workbench state. Session state should be composed from focused stores behind a thin facade. Transport concerns should sit behind interfaces, not leak directly into state orchestration.

### Host Layers

Desktop startup, browser/headless hosting, native dialogs, and transport servers are host concerns. They should live outside the pure engine modules, but they should still be provided by reusable `golden_core` host layers by default. Apps may override command-line parsing or bootstrap when needed, but they should not need local `src/app/desktop.rs`-style host implementations just to launch.

## Build And Codegen Boundary

App build scripts must consume public support APIs. They must not path-import private submodule files. The app node registry is now generated through `submodules/golden_core/crates/codegen_support/`, which exists specifically to provide a stable build-time boundary.

## Persistence Direction

Serialization contracts, project schema, codecs, and migrations belong in dedicated persistence or protocol layers. Desktop file dialogs and host workflows should call persistence APIs rather than define persistence formats themselves.

## Deeper Design Docs

Existing design docs live under `submodules/golden_core/crates/core/docs/` and currently include:

- `dashboard_system.md`
- `node_blueprints.md`
- `node_contexts.md`
- `parameters_control_modes.md`
- `scripting_schema.md`