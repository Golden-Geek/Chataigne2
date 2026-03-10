# Repo Map

## Top Level

- `src/`: Chataigne2 app shell, app-specific nodes, and minimal shell wiring.
- `src-ui/`: Svelte 5 UI package.
- `submodules/golden_core/`: shared engine workspace.
- `capabilities/`: Tauri capability and permission configuration.
- `gen/schemas/`: generated Tauri-related schemas.

## App Shell

- `src/app/mod.rs`: app entry wiring.
- `src/app/bootstrap.rs`: project lifecycle hooks for the app node enum.
- `src/nodes/`: app-owned node declarations.

## golden_core Workspace

- `crates/core/`: engine, node system, scripting, default desktop/headless host runtime, UI DTOs, and app lifecycle helpers.
- `crates/core/app/desktop.rs`: reusable default launch flow, CLI parsing, and Tauri bootstrap.
- `crates/core/app/desktop_commands.rs`: Tauri window commands and native file-dialog commands used by the default host.
- `crates/core/app/ui_server.rs`: built-in HTTP and WebSocket host for the current UI runtime.
- `crates/core_macros/`: proc macros.
- `crates/codegen_support/`: build-time node registry generation support.
- `crates/core/docs/`: deeper engine and protocol design notes.

## UI Package

- `src-ui/src/lib/golden_ui/components/`: panels and reusable UI components.
- `src-ui/src/lib/golden_ui/store/`: workbench and focused state stores.
- `src-ui/src/lib/golden_ui/transport/`: UI transport clients.
- `src-ui/src/lib/golden_ui/dockview/`: panel registration and layout persistence.