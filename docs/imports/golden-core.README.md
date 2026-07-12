# golden_core

`golden_core` is the shared runtime workspace for the Golden engine and related tooling.

The current architecture keeps the default ready-to-launch desktop/headless runtime in `golden_core` so apps do not need to rebuild that bootstrap themselves. Apps can still override command-line parsing or startup when needed, but the shared default host path lives here.

## Workspace Crates

- `crates/core`: `golden_engine`, the pure engine/runtime implementation crate.
- `crates/core_facade`: `golden_core`, the stable facade apps consume.
- `crates/protocol`: `golden_protocol`, the public protocol DTO boundary.
- `crates/persistence`: `golden_persistence`, the public persistence DTO boundary.
- `crates/transport_server`: built-in HTTP/WebSocket host runtime.
- `crates/host_desktop`: default desktop/headless host runtime.
- `crates/script`: `golden_script`, the public script API boundary.
- `crates/core_macros`: proc macros for node and item declarations.
- `crates/codegen_support`: public build-time helpers intended for app/workspace consumers.

## Design Docs

- `crates/core/docs/dashboard_system.md`
- `crates/core/docs/node_blueprints.md`
- `crates/core/docs/node_contexts.md`
- `crates/core/docs/parameters_control_modes.md`
- `crates/core/docs/scripting_schema.md`

Read these docs before large architectural changes in the engine or UI protocol layers.
