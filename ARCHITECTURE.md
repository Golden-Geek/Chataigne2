# Architecture

This repository is converging on a layered workspace with a thin shell, shared engine/runtime packages, and a UI stack that talks to the engine through explicit protocol boundaries.

## Top-Level Layers

### App Shell

`Chataigne2` owns app bootstrap, composition, and product-level wiring. It now also owns the desktop and built-in UI server host glue that was previously embedded inside `golden_core`. It should not become the home for reusable engine logic, protocol declarations, or persistence formats.

### Core Engine

`golden_core` currently hosts the shared runtime, node model, macros, and several mixed concerns that are being split apart. The desired end state is a pure engine/core layer that remains usable without desktop-only dependencies.

### Protocol Boundary

UI request, response, event, snapshot, and version types must have one source of truth. Rust and TypeScript should not manually mirror each other. Build and generation flows should produce the raw transport bindings from the canonical Rust protocol definition, with any UI-local normalization kept as an explicit adapter layer.

### UI Client And Stores

`src-ui` contains the Svelte 5 client, transport layer, and workbench state. Session state should be composed from focused stores behind a thin facade. Transport concerns should sit behind interfaces, not leak directly into state orchestration.

### Host Layers

Desktop startup, browser/headless hosting, native dialogs, and transport servers are host concerns. They should live outside the pure engine boundary. The current refactor state keeps those responsibilities in the app shell instead of `golden_core`, with the long-term direction still pointing toward dedicated host or transport crates.

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