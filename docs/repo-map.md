# Repo Map

## Top Level

- `src/`: Chataigne2 app shell, desktop host glue, built-in UI server host glue, and app-specific nodes.
- `src-ui/`: Svelte 5 UI package.
- `submodules/golden_core/`: shared engine workspace.
- `capabilities/`: Tauri capability and permission configuration.
- `gen/schemas/`: generated Tauri-related schemas.

## App Shell

- `src/app/mod.rs`: app entry wiring.
- `src/app/bootstrap.rs`: project lifecycle hooks for the app node enum.
- `src/app/desktop.rs`: desktop startup and Tauri window host.
- `src/app/ui_server.rs`: built-in HTTP and WebSocket host for the current UI runtime.
- `src/nodes/`: app-owned node declarations.

## golden_core Workspace

- `crates/core/`: engine, node system, scripting, UI DTOs, and app lifecycle helpers.
- `crates/core_macros/`: proc macros.
- `crates/codegen_support/`: build-time node registry generation support.
- `crates/core/docs/`: deeper engine and protocol design notes.

## UI Package

- `src-ui/src/lib/golden_ui/components/`: panels and reusable UI components.
- `src-ui/src/lib/golden_ui/store/`: workbench and focused state stores.
- `src-ui/src/lib/golden_ui/transport/`: UI transport clients.
- `src-ui/src/lib/golden_ui/dockview/`: panel registration and layout persistence.